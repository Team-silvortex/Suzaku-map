#!/usr/bin/env python3
"""Physical XTest keys, XKB locks/layouts and real GTK input in an owned display."""
import ctypes as C
from contextlib import contextmanager
import importlib.util
from pathlib import Path
import sys

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location(
    "suzaku_app_qa", Path(__file__).with_name("test-linux-apps.py"))
qa = importlib.util.module_from_spec(spec)
spec.loader.exec_module(qa)  # Validate private session/display/config BEFORE any XKB changes.
assert qa.os.environ["SUZAKU_APP_QA_SUITE"] == "keyboard"
custom_shortcuts_mode = qa.os.environ.get("SUZAKU_KEYBOARD_CUSTOM_SHORTCUTS_ONLY", "0")
assert custom_shortcuts_mode in ("0", "1"), "custom shortcut scope must be 0 or 1"
IBus = qa.IBus
passed = 0


class State(C.Structure):
    # XkbStateRec from X11/extensions/XKBstr.h, including its ABI padding.
    _fields_ = [("group", C.c_ubyte), ("locked_group", C.c_ubyte),
                ("base_group", C.c_ushort), ("latched_group", C.c_ushort)] + [
                    (name, C.c_ubyte) for name in
                    ("mods", "base_mods", "latched_mods", "locked_mods", "compat_state",
                     "grab_mods", "compat_grab_mods", "lookup_mods", "compat_lookup_mods")
                ] + [("ptr_buttons", C.c_ushort)]


def layout(name="us", variant=""):
    qa.subprocess.run(["setxkbmap", "-layout", name, "-variant", variant,
                       "-option", "", "-option", "compose:menu"], check=True, timeout=3)
    qa.pump()


class Keyboard:
    def __init__(self, x):
        self.x = x
        for name, args, result in [
            ("XkbGetState", [C.c_void_p, C.c_uint, C.POINTER(State)], C.c_int),
            ("XkbGetIndicatorState", [C.c_void_p, C.c_uint, C.POINTER(C.c_uint)], C.c_int),
            ("XkbKeysymToModifiers", [C.c_void_p, C.c_ulong], C.c_uint),
            ("XkbSetAutoRepeatRate", [C.c_void_p, C.c_uint, C.c_uint, C.c_uint], C.c_int),
            ("XkbGetNamedIndicator", [C.c_void_p, C.c_ulong, C.POINTER(C.c_int),
                                     C.POINTER(C.c_int), C.c_void_p, C.POINTER(C.c_int)], C.c_int),
        ]:
            function = getattr(x.x, name)
            function.argtypes, function.restype = args, result
        # Capture physical positions under US once. Later layout changes must be
        # resolved by XKB/GTK, NOT by turning desired Unicode into fake keyvals.
        symbols = list("abcdefghijklmnopqrstuvwxyz0123456789'.,/;`-=[]\\ ") + [
            "Shift_L", "Shift_R", "Control_L", "Alt_L", "Alt_R", "Caps_Lock", "Num_Lock",
            "Return", "KP_Enter", "BackSpace", "Escape", "Tab", "Menu", "F11",
            "KP_Add", "KP_Subtract", "KP_Multiply", "KP_Divide", "KP_Decimal",
        ] + [f"KP_{digit}" for digit in range(10)]
        self.codes = {}
        for name in symbols:
            symbol = ord(name) if len(name) == 1 else getattr(IBus, "KEY_" + name)
            code = x.x.XKeysymToKeycode(x.display, symbol)
            assert code, (name, symbol)
            self.codes[name] = code
        self.caps = x.x.XkbKeysymToModifiers(x.display, IBus.KEY_Caps_Lock)
        self.num = x.x.XkbKeysymToModifiers(x.display, IBus.KEY_Num_Lock)
        assert self.caps and self.num and not (self.caps & self.num)
        # Only this owned Xvfb display: make real server-generated repeats
        # deterministic, without injecting repeated IBus method calls.
        assert x.x.XkbSetAutoRepeatRate(x.display, 0x100, 250, 40)

    def state(self):
        state, leds = State(), C.c_uint()
        assert self.x.x.XkbGetState(self.x.display, 0x100, C.byref(state)) == 0
        assert self.x.x.XkbGetIndicatorState(self.x.display, 0x100, C.byref(leds)) == 0
        assert state.base_mods == 0 and state.latched_mods == 0, "stuck physical modifier"
        for name, mask in [(b"Caps Lock", self.caps), (b"Num Lock", self.num)]:
            atom = self.x.x.XInternAtom(self.x.display, name, 1)
            index, lit = C.c_int(), C.c_int()
            assert atom and self.x.x.XkbGetNamedIndicator(
                self.x.display, atom, C.byref(index), C.byref(lit), None, None)
            assert 0 <= index.value < 32
            assert bool(lit.value) == bool(leds.value & (1 << index.value))
            assert bool(lit.value) == bool(state.locked_mods & mask), "XKB lock/indicator mismatch"
        return state.locked_mods, leds.value, state.group

    @contextmanager
    def held(self, name, *modifiers):
        """Release owned keys even when a focus/typing assertion fails."""
        pressed = []
        try:
            for key in [*modifiers, name]:
                code = self.codes[key]
                assert self.x.xt.XTestFakeKeyEvent(self.x.display, code, 1, 0)
                pressed.append(code)
            self.x.x.XSync(self.x.display, 0)
            yield pressed
        finally:
            for code in reversed(pressed):
                assert self.x.xt.XTestFakeKeyEvent(self.x.display, code, 0, 0)
            self.x.x.XSync(self.x.display, 0)
        qa.time.sleep(.025)
        qa.pump()

    def press(self, name, *modifiers, duration=.015, release_modifiers_after=None):
        with self.held(name, *modifiers) as pressed:
            started = qa.time.monotonic()
            until = started + duration
            while qa.time.monotonic() < until:
                if (release_modifiers_after is not None and len(pressed) > 1 and
                        qa.time.monotonic() - started >= release_modifiers_after):
                    for code in reversed(pressed[:-1]):
                        assert self.x.xt.XTestFakeKeyEvent(self.x.display, code, 0, 0)
                    del pressed[:-1]
                    self.x.x.XSync(self.x.display, 0)
                qa.time.sleep(.01)
                qa.pump()

    def type(self, text, *modifiers):
        for name in text:
            self.press(name, *modifiers)

    def locks(self, caps=False, num=False):
        for enabled, mask, name in [(caps, self.caps, "Caps_Lock"), (num, self.num, "Num_Lock")]:
            if bool(self.state()[0] & mask) != enabled:
                self.press(name)
            assert bool(self.state()[0] & mask) == enabled

    def map(self):
        return qa.subprocess.check_output(["xkbcomp", "-xkb", qa.os.environ["DISPLAY"], "-"],
                                          stderr=qa.subprocess.DEVNULL, timeout=3)


def expect(seed):
    qa.wait(lambda: qa.seed_is(seed), f"physical keys must produce draft {seed!r}")


def clean():
    # Housekeeping shortcuts use US; no personal display/layout is ever connected.
    layout()
    keyboard.locks()
    keyboard.press("Escape")
    expect("")
    qa.clear_document(x, document)


def finish(seed, label, initial, keymap, enter="Return"):
    global passed
    expect(seed)
    qa.save_document(x, document, "")
    expect(seed)  # Ctrl+S is an application shortcut, never draft submission.
    assert keyboard.state() == initial and keyboard.map() == keymap
    keyboard.press(enter)
    expect("")
    qa.save_document(x, document, seed + "\n")
    assert keyboard.state() == initial and keyboard.map() == keymap
    assert x.focused() == window and editor.poll() is None
    passed += 1
    print("PASS:", label, flush=True)


CUSTOM_BINDINGS = {
    "previous": "Ctrl+P",
    "next": "Ctrl+N",
    "previous-page": "Alt+Shift+B",
    "next-page": "Ctrl+Shift+F",
    "adopt": "Alt+Shift+A",
    "switch-language": "Ctrl+Shift+L",
}


def settings():
    return qa.json.loads(qa.command("S"))["settings"]


def fresh_snapshot():
    observed = qa.Watch()
    try:
        def ready():
            observed.drain()
            return observed.latest is not None
        qa.wait(ready, "fresh owned subscription snapshot")
        return observed.latest
    finally:
        observed.sock.close()


def update(patch):
    result = qa.json.loads(qa.command("U" + qa.json.dumps(patch, separators=(",", ":"))))
    assert result["ok"], result
    assert qa.json.loads((qa.root / "ime.json").read_bytes()) == result["settings"], \
        "a shortcut control must acknowledge its actual persisted settings"
    return result["settings"]


def select_custom(text):
    target = next(i for i, candidate in enumerate(qa.watch.latest["candidates"])
                  if candidate["text"] == text)
    while qa.watch.latest["selected"] != target:
        before = qa.watch.latest["selected"]
        forward = before < target
        keyboard.press("n" if forward else "p", "Control_L")
        qa.wait(lambda: qa.watch.latest["selected"] == before + (1 if forward else -1),
                "physical custom candidate navigation")


def custom_panel_recording():
    """Click the real installed panel, then observe the real U acknowledgement."""
    assert qa.seed_is("") and x.focused_within(window)
    companion = qa.spawn([str(qa.bins / "panel")])
    index = qa.processes.index(companion)

    def frame(kind):
        assert companion.poll() is None, "owned installed companion exited"
        lines = (qa.root / f"process-{index}.log").read_text().splitlines()
        for line in reversed(lines):
            if line.startswith("Suzaku frame: "):
                try:
                    value = qa.json.loads(line.removeprefix("Suzaku frame: "))
                except qa.json.JSONDecodeError:
                    continue
                if value.get("window") == kind:
                    return value
        return {}

    def click(kind, surface="Settings"):
        title = "Suzaku · Panel Settings" if surface == "Settings" else "Suzaku · Input"
        qa.wait(lambda: (value := frame(surface)).get("viewport_settled") is True and
                any(item["kind"] == kind for item in value.get("controls", [])),
                f"installed {surface} presents {kind}")
        xid = x.window(title)
        assert xid is not None and "Map State: IsViewable" in qa.subprocess.run(
            ["xwininfo", "-id", str(xid)], capture_output=True, text=True, timeout=2).stdout
        if surface == "Settings":
            assert x.focused_within(xid), "recording injection requires owned settings focus"
        else:
            assert x.focused_within(window), "no-focus companion must retain owned editor focus"
        value = frame(surface)
        rect = next(item["rect"] for item in value["controls"] if item["kind"] == kind)
        px, py = value["position"]
        x.click(round(px + rect[0] + rect[2] / 2), round(py + rect[1] + rect[3] / 2))

    qa.wait(lambda: frame("Main").get("runtime_font") and x.window("Suzaku · Input"),
            "installed companion is rendered without stealing focus", timeout=30)
    assert x.focused_within(window), "companion launch stole application focus"
    click("SettingsToggle", "Main")
    qa.wait(lambda: x.window("Suzaku · Panel Settings"), "actual installed settings window")
    settings_window = x.window("Suzaku · Panel Settings")
    x.focus(settings_window)
    click("SetSettingsCategory(Shortcuts)")
    click("SettingsSearchInput")
    keyboard.type("previous")
    click("RecordShortcut(Previous)")
    keyboard.press("g", "Control_L", "Shift_L", duration=.65, release_modifiers_after=.1)
    qa.wait(lambda: settings()["shortcut_bindings"].get("previous") == "Ctrl+Shift+G",
            "installed recorder received its durable native acknowledgement")
    saved = qa.json.loads((qa.root / "ime.json").read_bytes())
    assert saved == settings() and saved["shortcut_bindings"] == {"previous": "Ctrl+Shift+G"}
    # The still-held command after modifiers are released must not contaminate
    # the search. If it did, the original action row would disappear here.
    click("ClearShortcutBinding(Previous)")
    qa.wait(lambda: settings()["shortcut_bindings"] == {}, "real single-action clear acknowledged")
    assert qa.json.loads((qa.root / "ime.json").read_bytes()) == settings()
    click("SettingsToggle")
    qa.wait(lambda: "Map State: IsViewable" not in qa.subprocess.run(
        ["xwininfo", "-id", str(settings_window)], capture_output=True, text=True,
        timeout=2).stdout, "real installed settings close hides its own window")
    x.focus(window)
    qa.settle_input(bus, x, window)
    expect("")
    qa.save_document(x, document, "")
    print("PASS: installed panel click/physical recording/early modifier release/durable U ACK/single clear/close", flush=True)


def run_custom_shortcuts():
    """Mutually exclusive installed-binary acceptance, not the legacy 49-group gate."""
    global passed
    assert x.focused_within(window) and editor.poll() is None
    clean()
    initial_settings = settings()
    before = fresh_snapshot()
    payload = {"llm_enabled": False, "shortcut_profile": "standard",
               "shortcut_bindings": CUSTOM_BINDINGS}
    assert 128 < len(("U" + qa.json.dumps(payload, separators=(",", ":"))).encode()) <= 1024
    saved = update(payload)
    assert saved == dict(initial_settings, **payload)
    acknowledged = fresh_snapshot()
    qa.wait(lambda: qa.watch.latest["revision"] >= acknowledged["revision"],
            "full shortcut patch subscription publication")
    for field in ("context", "seed", "selected", "candidates"):
        assert acknowledged[field] == before[field], ("shortcut control changed draft", field)
    print("PASS: actual full six-key U request persists beyond the legacy 128-byte control boundary", flush=True)

    # Both languages use real XTest events, the actual application IM context,
    # subscription observations and exact saved-buffer assertions.
    for language, original, adopted in [("en", "hel", "hello"), ("zh-Hans", "nihao", "你好")]:
        for early_release in (False, True):
            clean()
            revision = qa.watch.latest["revision"]
            assert qa.json.loads(qa.command("L" + language))["ok"]
            qa.wait(lambda: qa.watch.latest["revision"] > revision, "physical custom test language")
            initial, keymap = keyboard.state(), keyboard.map()
            keyboard.type(original)
            expect(original)
            assert len(qa.watch.latest["candidates"]) > 1
            candidates = qa.watch.latest["candidates"]
            keyboard.press("n", "Control_L")
            qa.wait(lambda: qa.watch.latest["selected"] == 1, "custom next row")
            keyboard.press("p", "Control_L")
            qa.wait(lambda: qa.watch.latest["selected"] == 0, "custom previous row")
            if language == "en":
                assert len(candidates) > 6, "physical custom paging needs a second fallback page"
                keyboard.press("f", "Control_L", "Shift_L")
                qa.wait(lambda: qa.watch.latest["selected"] == 6, "custom next page")
                keyboard.press("b", "Alt_L", "Shift_L")
                qa.wait(lambda: qa.watch.latest["selected"] == 0, "custom previous page")
                # Server-generated navigation repeats remain continuous, unlike adoption.
                duration = .4 + len(candidates) * .06
                keyboard.press("n", "Control_L", duration=duration)
                qa.wait(lambda: qa.watch.latest["selected"] == len(candidates) - 1,
                        "held custom next clamps at final row")
                keyboard.press("p", "Control_L", duration=duration)
                qa.wait(lambda: qa.watch.latest["selected"] == 0,
                        "held custom previous returns to first row")
            assert qa.watch.latest["candidates"] == candidates
            expect(original)
            select_custom(adopted)
            keyboard.press("a", "Alt_L", "Shift_L", duration=.65,
                           release_modifiers_after=.1 if early_release else None)
            expect(adopted)
            qa.save_document(x, document, "")
            keyboard.press("BackSpace")
            expect(original)
            # A genuine release allows immediate reuse, not a time debounce.
            select_custom(adopted)
            keyboard.press("a", "Alt_L", "Shift_L")
            expect(adopted)
            keyboard.press("BackSpace")
            expect(original)
            select_custom(adopted)
            keyboard.press("a", "Alt_L", "Shift_L")
            expect(adopted)
            keyboard.press(" ")
            finish(adopted + " ", f"custom {language}, early release={early_release}: rows/pages/repeat/adopt/undo/reuse/Space/single commit",
                   initial, keymap)

    for draft, early_release in [("", False), ("nihao", True)]:
        clean()
        assert qa.json.loads(qa.command("Len"))["ok"]
        keyboard.type(draft)
        expect(draft)
        baseline = qa.watch.latest.copy()
        keyboard.press(" ", "Control_L", "Shift_L")
        expect(draft)
        assert settings()["language"] == "en", "custom binding did not replace the default language chord"
        assert qa.watch.latest["context"] == baseline["context"]
        initial, keymap = keyboard.state(), keyboard.map()
        keyboard.press("l", "Control_L", "Shift_L", duration=.65,
                       release_modifiers_after=.1 if early_release else None)
        qa.wait(lambda: settings()["language"] == "zh-Hans" and
                qa.watch.latest["context"] > baseline["context"] and qa.seed_is(draft),
                "held custom language switches exactly once and preserves the draft")
        qa.save_document(x, document, "")
        switched_context = qa.watch.latest["context"]
        keyboard.press("l", "Control_L", "Shift_L")
        qa.wait(lambda: settings()["language"] == "en" and
                qa.watch.latest["context"] > switched_context and qa.seed_is(draft),
                "released custom language chord is immediately reusable")
        assert keyboard.state() == initial and keyboard.map() == keymap
        qa.save_document(x, document, "")
        passed += 1
        print("PASS: custom language replaces default, hold/early release/real release preserves", repr(draft), flush=True)

    clean()
    assert qa.json.loads(qa.command("Len"))["ok"]
    layout("us", "intl")
    initial, keymap = keyboard.state(), keyboard.map()
    keyboard.press("'")
    keyboard.press("l", "Control_L", "Shift_L")
    assert settings()["language"] == "en", "custom language must not dispatch during pending Compose"
    keyboard.press("a", "Alt_L", "Shift_L")
    keyboard.press("e")
    finish("é", "custom chords and physical pure Ctrl/Shift/Alt preserve pending Compose", initial, keymap)

    clean()
    keyboard.type("hel")
    expect("hel")
    select_custom("hello")
    keyboard.press("a", "Alt_L", "Shift_L")
    expect("hello")
    before, file_before = settings(), (qa.root / "ime.json").read_bytes()
    snapshot = qa.watch.latest.copy()
    for invalid in [
        {"shortcut_bindings": dict(CUSTOM_BINDINGS, previous="Ctrl+N")},
        {"shortcut_bindings": dict(CUSTOM_BINDINGS, adopt="Alt+Space")},
        {"shortcut_bindings": dict(CUSTOM_BINDINGS, adopt="Ctrl+Alt+A")},
        {"shortcut_profile": "home-row", "shortcut_bindings": dict(CUSTOM_BINDINGS, previous="Alt+J")},
    ]:
        invalid["llm_enabled"] = True  # No partial model-toggle application either.
        result = qa.json.loads(qa.command("U" + qa.json.dumps(invalid, separators=(",", ":"))))
        assert not result["ok"], result
        assert settings() == before and (qa.root / "ime.json").read_bytes() == file_before
        qa.pump()
        assert qa.watch.latest == snapshot, "invalid shortcut patch changed the live draft/selection"
    keyboard.press("BackSpace")
    expect("hel")
    qa.save_document(x, document, "")
    print("PASS: duplicate/reserved/AltGr/profile-conflicting patches reject atomically and preserve adoption undo", flush=True)

    clean()
    update({"shortcut_bindings": {}})
    context = qa.watch.latest["context"]
    keyboard.press("l", "Control_L", "Shift_L")
    assert settings()["language"] == "en" and qa.watch.latest["context"] == context
    keyboard.press(" ", "Control_L", "Shift_L")
    qa.wait(lambda: settings()["language"] == "zh-Hans" and qa.watch.latest["context"] > context,
            "clear custom keys restores default physical language chord")
    expect("")
    keyboard.press(" ", "Control_L", "Shift_L")
    qa.wait(lambda: settings()["language"] == "en", "default language switches back after release")
    print("PASS: full clear restores default language chord and removes its previous custom alias", flush=True)
    custom_panel_recording()
    assert passed == 7, passed
    assert x.focused_within(window) and editor.poll() is None and settings()["shortcut_bindings"] == {}
    assert not settings()["llm_enabled"]
    print("RESULT: 7 strict physical custom workflow groups plus full-U/atomic-clear/real installed-recorder gates passed; legacy 49-group gate is separate", flush=True)


def run_keypad_navigation():
    """NumLock-off physical keypad navigation uses the existing draft controls."""
    saved_language = settings()["language"]
    initial_passed = passed
    new_checked = 0
    cases = [("en", "hel", "hello"), ("zh-Hans", "nihao", "你好")]

    def unchanged(key, label):
        baseline = fresh_snapshot()
        keyboard.press(key)
        current = fresh_snapshot()
        assert current == baseline, (label, baseline, current)
        assert x.focused() == window and editor.poll() is None

    def move(key, target, label):
        baseline = fresh_snapshot()
        keyboard.press(key)
        qa.wait(lambda: qa.watch.latest["selected"] == target, label)
        current = fresh_snapshot()
        assert current["selected"] == target and current["revision"] > baseline["revision"], label
        for field in ["host", "context", "focused", "private", "language", "seed", "candidates"]:
            assert current[field] == baseline[field], (label, field, baseline, current)
        assert x.focused() == window and editor.poll() is None

    try:
        assert not settings()["llm_enabled"]
        for language, seed, word in cases:
            for mode in ["rows", "pages"]:
                clean()
                assert qa.json.loads(qa.command("L" + language))["ok"]
                qa.wait(lambda: settings()["language"] == language
                        and qa.watch.latest["language"] == language and not qa.watch.latest["seed"],
                        "keypad navigation language")
                keyboard.locks(num=False)
                initial, keymap = keyboard.state(), keyboard.map()
                keyboard.type(seed)
                expect(seed)
                qa.save_document(x, document, "")
                count = len(qa.watch.latest["candidates"])
                assert qa.watch.latest["selected"] == 0
                if mode == "rows":
                    assert count >= 3, "keypad row fixture requires adjacent candidates"
                    unchanged("KP_8", "keypad Up at first row")
                    unchanged("KP_4", "keypad Left at first row")
                    for target in range(1, count):
                        move("KP_2" if target % 2 else "KP_6", target,
                             "keypad Down/Right advances one row")
                    unchanged("KP_2", "keypad Down at last row")
                    unchanged("KP_6", "keypad Right at last row")
                    for target in reversed(range(count - 1)):
                        move("KP_8" if target % 2 else "KP_4", target,
                             "keypad Up/Left reverses one row")
                else:
                    assert count > 6, "keypad page fixture requires a second candidate page"
                    unchanged("KP_9", "keypad PageUp at first page")
                    for target in range(6, count, 6):
                        move("KP_3", target, "keypad PageDown selects adjacent page start")
                    unchanged("KP_3", "keypad PageDown at last page")
                    for target in reversed(range(0, (count - 1) // 6 * 6, 6)):
                        move("KP_9", target, "keypad PageUp selects adjacent page start")
                    unchanged("KP_9", "keypad PageUp after returning to first page")
                expect(seed)
                qa.save_document(x, document, "")
                qa.choose_number(x, word)
                expect(word)
                qa.save_document(x, document, "")
                # Only unavailable navigation is tested here: a genuine move
                # deliberately retires adoption undo under the existing contract.
                if mode == "rows":
                    unchanged("KP_8", "adopted word keypad Up preserves undo")
                    unchanged("KP_4", "adopted word keypad Left preserves undo")
                else:
                    unchanged("KP_9", "adopted word missing keypad page preserves undo")
                keyboard.press("BackSpace")
                expect(seed)
                qa.save_document(x, document, "")
                qa.choose_number(x, word)
                expect(word)
                keyboard.press(" ")
                expect(word + " ")
                qa.save_document(x, document, "")
                finish(word + " ", f"{language} NumLock-off keypad {mode}: boundaries/moves/undo/Space/single commit",
                       initial, keymap)
                new_checked += 1
        assert new_checked == 4, new_checked
        assert passed == initial_passed + 4, passed
        print("RESULT: 4 appended strict physical keypad-navigation workflow groups passed", flush=True)
    finally:
        assert qa.json.loads(qa.command("L" + saved_language))["ok"]
        qa.wait(lambda: settings()["language"] == saved_language, "restore keypad navigation language")


try:
    layout()
    bus, x = qa.start()
    keyboard = Keyboard(x)
    document = qa.root / "suzaku-keyboard-qa.txt"
    document.write_text("")
    editor = qa.spawn(["gnome-text-editor", "--standalone", str(document)])
    qa.wait(lambda: x.window(document.name), "physical keyboard QA editor")
    window = x.window(document.name)
    x.focus(window)
    x.click(200, 100)
    assert bus.set_global_engine("dev.suzaku.linux.ime")
    qa.settle_input(bus, x, window)
    qa.prepare_editor(x, document)

    if custom_shortcuts_mode == "1":
        run_custom_shortcuts()
        sys.exit(0)  # finally still closes only this fixture's owned processes.

    # First prove that the private server actually generates autorepeat.
    keyboard.press("q", duration=.65)
    qa.wait(lambda: qa.watch.latest["seed"].count("q") >= 3, "physical autorepeat enabled")
    assert set(qa.watch.latest["seed"]) == {"q"}
    clean()
    assert qa.json.loads(qa.command('U{"shortcut_profile":"home-row"}'))["ok"]
    for language, original, adopted in [("en", "hel", "hello"), ("zh-Hans", "nihao", "你好")]:
        for method, release_early in [("number", False), ("shift-enter", False), ("home-row", False),
                                      ("shift-enter", True), ("home-row", True)]:
            clean()
            revision = qa.watch.latest["revision"]
            assert qa.json.loads(qa.command("L" + language))["ok"]
            qa.wait(lambda: qa.watch.latest["revision"] > revision, "keyboard QA language")
            initial, keymap = keyboard.state(), keyboard.map()
            keyboard.type(original)
            expect(original)
            index = next(i for i, candidate in enumerate(qa.watch.latest["candidates"][:6])
                         if candidate["text"] == adopted)
            if method == "number":
                key, modifiers = str(index + 1), ()
            else:
                for _ in range(index):
                    keyboard.press("j", "Alt_L")
                qa.wait(lambda: qa.watch.latest["selected"] == index, "physical home-row selection")
                key, modifiers = ("Return", ("Shift_L",)) if method == "shift-enter" else (";", ("Alt_L",))
            keyboard.press(key, *modifiers, duration=.65,
                           release_modifiers_after=.1 if release_early else None)
            expect(adopted)
            qa.save_document(x, document, "")
            keyboard.press("BackSpace")
            expect(original)
            # A real release permits immediate reuse, without a time debounce.
            qa.choose_number(x, adopted)
            keyboard.press("BackSpace")
            expect(original)
            qa.choose_number(x, adopted)
            keyboard.press(" ")
            finish(adopted + " ", f"{language} {method}, early modifier release={release_early}: hold/undo/reuse",
                   initial, keymap)
    clean()
    revision = qa.watch.latest["revision"]
    assert qa.json.loads(qa.command("Len"))["ok"]
    qa.wait(lambda: qa.watch.latest["revision"] > revision, "restore English keyboard QA")
    initial, keymap = keyboard.state(), keyboard.map()
    keyboard.type("hel")
    expect("hel")
    count = len(qa.watch.latest["candidates"])
    assert count >= 3, "repeat fixture needs several local candidates"
    navigation_hold = .4 + count * .06
    keyboard.press("j", "Alt_L", duration=navigation_hold)
    qa.wait(lambda: qa.watch.latest["selected"] == count - 1, "home-row navigation still repeats")
    keyboard.press("k", "Alt_L", duration=navigation_hold)
    qa.wait(lambda: qa.watch.latest["selected"] == 0, "reverse navigation still repeats")
    finish("hel", "held Alt+J/K navigate continuously without changing the draft", initial, keymap)
    clean()
    initial, keymap = keyboard.state(), keyboard.map()
    keyboard.type("hel")
    expect("hel")
    keyboard.press("2")
    expect("hello")
    next_word = qa.watch.latest["candidates"][1]["text"]
    assert next_word != "hello"
    keyboard.press("2")
    expect(next_word)
    keyboard.press("BackSpace")
    finish("hello", "rapid separate numeric taps each adopt, undo reverses only the last", initial, keymap)
    clean()
    assert qa.json.loads(qa.command('U{"shortcut_profile":"standard"}'))["ok"]
    assert qa.json.loads(qa.command("Len"))["ok"]

    clean()
    layout("us", "intl")
    initial, keymap = keyboard.state(), keyboard.map()
    keyboard.press("'")
    keyboard.press(" ", "Control_L", "Shift_L")
    assert qa.json.loads(qa.command("S"))["settings"]["language"] == "en"
    keyboard.press("e")
    finish("é", "language chord preserves pending Compose through physical modifier presses", initial, keymap)

    for language, target, seed, adopted in [("en", "zh-Hans", "nihao", "你好"),
                                            ("zh-Hans", "en", "hel", "hello")]:
        for release_early in [False, True]:
            clean()
            assert qa.json.loads(qa.command("L" + language))["ok"]
            keyboard.type(seed)
            expect(seed)
            keyboard.locks(caps=release_early, num=release_early)
            initial, keymap = keyboard.state(), keyboard.map()
            keyboard.press(" ", "Control_L", "Shift_L", duration=.65,
                           release_modifiers_after=.1 if release_early else None)
            qa.wait(lambda: qa.json.loads(qa.command("S"))["settings"]["language"] == target,
                    "physical language chord changes once")
            expect(seed)
            qa.save_document(x, document, "")
            qa.choose_number(x, adopted)
            expect(adopted)
            keyboard.press(" ", "Control_L", "Shift_L")
            qa.wait(lambda: qa.json.loads(qa.command("S"))["settings"]["language"] == language,
                    "released physical language chord switches back")
            expect(adopted)
            keyboard.press(" ")
            finish(adopted + " ", f"{language} language toggle, early release/locks={release_early}: draft/adoption/return/explicit commit",
                   initial, keymap)

    clean()
    assert qa.json.loads(qa.command("Len"))["ok"]
    for caps in [False, True]:
        for shift in [None, "Shift_L", "Shift_R"]:
            clean()
            keyboard.locks(caps=caps)
            initial, keymap = keyboard.state(), keyboard.map()
            keyboard.type("abc", *([shift] if shift else []))
            expected = "ABC" if caps != bool(shift) else "abc"
            finish(expected, f"Caps={caps}, Shift={shift}: case and lock indicators stay aligned", initial, keymap)

    for caps in [False, True]:
        for num in [False, True]:
            clean()
            keyboard.locks(caps=caps, num=num)
            initial, keymap = keyboard.state(), keyboard.map()
            keyboard.type("hel")
            original = "HEL" if caps else "hel"
            expect(original)
            adopted = "HELLO" if caps else "hello"
            qa.choose_number(x, adopted)
            qa.save_document(x, document, "")
            keyboard.press("BackSpace")
            expect(original)
            qa.choose_number(x, adopted)
            keyboard.press(" ")
            finish(adopted + " ", f"Caps={caps}, Num={num}: number adoption, undo and Space continuation", initial, keymap)

    clean()
    keyboard.locks(num=True)
    initial, keymap = keyboard.state(), keyboard.map()
    keyboard.type("v ")
    for digit in range(10):
        keyboard.press(f"KP_{digit}")
    for key in ["KP_Decimal", "KP_Add", "KP_Subtract", "KP_Multiply", "KP_Divide"]:
        keyboard.press(key)
    finish("v 0123456789.+-*/", "NumLock keypad digits/operators stay literal inside a draft", initial, keymap, "KP_Enter")

    clean()
    keyboard.locks(num=True)
    initial, keymap = keyboard.state(), keyboard.map()
    for key in ["KP_1", "KP_2", "KP_Decimal", "KP_5"]:
        keyboard.press(key)
    expect("")
    qa.save_document(x, document, "12.5\n")
    assert keyboard.state() == initial and keyboard.map() == keymap
    passed += 1
    print("PASS: empty-draft keypad digits/decimal go straight to the application", flush=True)

    # Keys below refer to their US physical position. Actual symbols come from
    # the named keyboard layout, with no direct IBus ProcessKeyEvent injection.
    cases = [
        ("us", "", [("a", ()), ("1", ("Shift_L",)), ("2", ("Shift_R",))], "a!@"),
        ("gb", "", [("a", ()), ("2", ("Shift_L",)), ("3", ("Shift_R",))], 'a"£'),
        ("de", "", [("y", ()), ("z", ()), ("q", ("Alt_R",)), ("e", ("Alt_R",)),
                      ("7", ("Alt_R",)), ("0", ("Alt_R",))], "zy@€{}"),
        ("fr", "", [("q", ()), ("a", ()), ("2", ()), ("2", ("Shift_L",)),
                      ("0", ("Alt_R",))], "aqé2@"),
        ("us", "intl", [("c", ()), ("a", ()), ("f", ()), ("'", ()), ("e", ()),
                           (" ", ()), ("'", ("Shift_L",)), ("u", ())], "café ü"),
    ]
    for name, variant, keys, expected in cases:
        clean()
        layout(name, variant)
        initial, keymap = keyboard.state(), keyboard.map()
        for key, modifiers in keys:
            keyboard.press(key, *modifiers)
        finish(expected, f"{name}/{variant or 'default'}: physical letters, Shift, AltGr and dead keys", initial, keymap)

    for side in ["Shift_L", "Shift_R"]:
        clean()
        initial, keymap = keyboard.state(), keyboard.map()
        keyboard.type("hel")
        expect("hel")
        keyboard.press("Tab")
        qa.wait(lambda: qa.watch.latest["selected"] == 1, "Tab selects the second row")
        keyboard.press("Tab", side)
        qa.wait(lambda: qa.watch.latest["selected"] == 0, "Shift Tab returns to first row")
        keyboard.press("Tab")
        keyboard.press("KP_Enter", side)
        expect("hello")
        keyboard.press("BackSpace")
        expect("hel")
        keyboard.type("lo world")
        keyboard.press("BackSpace", "Control_L")
        finish("hello ", f"{side}: reverse navigation, keypad adoption/undo and Ctrl Backspace", initial, keymap)

    clean()
    initial, keymap = keyboard.state(), keyboard.map()
    keyboard.type("v ")
    for digit in "0123456789":
        keyboard.press(digit, "Alt_L")
    keyboard.press("Menu")
    keyboard.press("1")
    keyboard.press("2")
    finish("v 0123456789½", "Alt literal digits and physical Compose never choose numbered candidates", initial, keymap)

    clean()
    initial, keymap = keyboard.state(), keyboard.map()
    keyboard.type("ab")
    keyboard.locks(caps=True)
    keyboard.type("cd")
    keyboard.type("ef", "Shift_R")
    keyboard.locks()
    keyboard.type("gh")
    finish("abCDefgh", "mid-draft Caps changes preserve spelling and release both Shift states", initial, keymap)

    clean()
    initial, keymap = keyboard.state(), keyboard.map()
    keyboard.type("v")
    keyboard.locks(num=True)
    keyboard.press("KP_1")
    keyboard.press("KP_2")
    keyboard.locks()
    expect("v12")
    keyboard.locks(num=True)
    keyboard.press("KP_Decimal")
    keyboard.press("KP_5")
    keyboard.locks()
    finish("v12.5", "mid-draft NumLock changes do not choose candidates or submit", initial, keymap)

    clean()
    initial, keymap = keyboard.state(), keyboard.map()
    keyboard.type("left ")
    layout("de")
    keyboard.press("y")
    expect("left z")
    layout("fr")
    keyboard.press("q")
    expect("left za")
    layout()
    finish("left za", "live US/German/French layout changes preserve the same draft", initial, keymap)

    for cancel in ["Escape", "BackSpace"]:
        clean()
        layout("us", "intl")
        initial, keymap = keyboard.state(), keyboard.map()
        keyboard.type("caf")
        keyboard.press("'")
        expect("caf")
        keyboard.press(cancel)
        expect("caf")
        keyboard.press("e")
        finish("cafe", f"dead-key {cancel} cancels only the unfinished accent", initial, keymap)

    clean()
    initial, keymap = keyboard.state(), keyboard.map()
    keyboard.type("v ")
    keyboard.press("Menu")
    keyboard.press("F11")
    keyboard.press("1")
    finish("v two words", "private custom Compose text stays in the editable draft", initial, keymap)

    # Real focus migration before key-up. Both editors and every observed file
    # belong to this private fixture, never to the user's desktop session.
    clean()
    other_document = qa.root / "suzaku-keyboard-target-qa.txt"
    other_document.write_text("")
    other_editor = qa.spawn(["gnome-text-editor", "--standalone", str(other_document)])
    qa.wait(lambda: x.window(other_document.name), "second keyboard QA editor")
    other_window = x.window(other_document.name)
    x.focus(other_window)
    x.click(200, 100)
    qa.settle_input(bus, x, other_window)
    qa.prepare_editor(x, other_document)
    assert qa.json.loads(qa.command('U{"shortcut_profile":"home-row"}'))["ok"]

    def prepare_choice(seed, word, method):
        keyboard.type(seed)
        expect(seed)
        index = next(i for i, candidate in enumerate(qa.watch.latest["candidates"][:6])
                     if candidate["text"] == word)
        if method == "number":
            return str(index + 1), ()
        for _ in range(index):
            keyboard.press("Tab")
        qa.wait(lambda: qa.watch.latest["selected"] == index, "focus fixture selected word")
        return ("Return", ("Shift_L",)) if method == "shift-enter" else (";", ("Alt_L",))

    for language, original, adopted in [("en", "hel", "hello"), ("zh-Hans", "nihao", "你好")]:
        for method in ["number", "shift-enter", "home-row"]:
            x.focus(window)
            qa.settle_input(bus, x, window)
            clean()
            revision = qa.watch.latest["revision"]
            assert qa.json.loads(qa.command("L" + language))["ok"]
            qa.wait(lambda: qa.watch.latest["revision"] > revision, "focus fixture language")
            initial, keymap = keyboard.state(), keyboard.map()
            key, modifiers = prepare_choice(original, adopted, method)
            with keyboard.held(key, *modifiers):
                expect(adopted)
                old = qa.watch.latest.copy()
                x.focus(other_window)
            qa.settle_input(bus, x, other_window)
            expect("")
            assert qa.watch.latest["context"] != old["context"]
            assert qa.command(f'A{old["host"]} {old["revision"]} K0') == b"0"
            qa.save_document(x, other_document, "")
            # The same physical key is immediately usable in the new context;
            # undo belongs only to that context, then Space/Enter finish once.
            key, modifiers = prepare_choice(original, adopted, method)
            keyboard.press(key, *modifiers)
            expect(adopted)
            keyboard.press("BackSpace")
            expect(original)
            qa.choose_number(x, adopted)
            keyboard.press(" ")
            expect(adopted + " ")
            keyboard.press("Return")
            expect("")
            qa.save_document(x, other_document, adopted + " \n")
            qa.clear_document(x, other_document)
            x.focus(window)
            qa.settle_input(bus, x, window)
            expect("")
            qa.save_document(x, document, "")
            key, modifiers = prepare_choice(original, adopted, method)
            keyboard.press(key, *modifiers)
            expect(adopted)
            keyboard.press("BackSpace")
            expect(original)
            keyboard.press("Escape")
            expect("")
            qa.save_document(x, document, "")
            assert keyboard.state() == initial and keyboard.map() == keymap
            assert x.focused() == window and other_editor.poll() is None and editor.poll() is None
            passed += 1
            print("PASS:", language, method, "key-up in another window, fresh adoption/undo and focus return", flush=True)

    assert passed == 49, passed
    print("RESULT: 49 strict physical-keyboard workflow groups passed", flush=True)
    run_keypad_navigation()
except Exception:
    if "keyboard" in globals():
        print("QA keyboard state:", keyboard.state())
    if "document" in globals():
        print("QA synthetic document:", repr(document.read_text()))
    if "other_document" in globals():
        print("QA second synthetic document:", repr(other_document.read_text()))
    qa.diagnose()
    raise
finally:
    qa.close()
