#!/usr/bin/env python3
"""Actual Rust settings -> C adapter -> private IBus shortcut acceptance.

The caller supplies rebuilt binaries and a memory-capped, 120-second private
dbus-run-session. Set SUZAKU_CUSTOM_SHORTCUT_QA=1 and put all fixture paths under
an owned /tmp/suzaku-custom-shortcuts.* root, including a private Compose copy.
No desktop engine, global input injection, live model or installed binary is used.
--preflight-only validates isolation and dependencies without starting processes.
"""
import argparse
import json
import os
from pathlib import Path
import socket
import stat
import subprocess
import sys
import time

sys.dont_write_bytecode = True
from fixtures.compose_fixture import require_private_compose
from fixtures.ibus_signal_lifetime import IBusSignalObjects
from fixtures.native_probe_cleanup import preserve_primary_failure, report_native_diagnostic


BINDINGS = {
    "previous": "Ctrl+P",
    "next": "Ctrl+N",
    "previous-page": "Alt+Shift+B",
    "next-page": "Ctrl+Shift+F",
    "adopt": "Alt+Shift+A",
    "switch-language": "Ctrl+Shift+L",
}
PAGE_SIZE = 6
MAX_RESPONSE = 65536


def require_fixture(environment):
    """Fail closed before starting IBus or writing configuration."""
    if environment.get("SUZAKU_CUSTOM_SHORTCUT_QA") != "1":
        raise ValueError("private shortcut QA requires explicit SUZAKU_CUSTOM_SHORTCUT_QA=1")
    runtime = Path(environment.get("XDG_RUNTIME_DIR", ""))
    if (runtime.parent != Path("/tmp") or not runtime.name.startswith("suzaku-custom-shortcuts.")
            or not runtime.is_dir() or runtime.is_symlink()
            or runtime.stat().st_uid != os.getuid()
            or stat.S_IMODE(runtime.stat().st_mode) & 0o077):
        raise ValueError("shortcut QA root must be an owned private /tmp/suzaku-custom-shortcuts.* directory")
    if environment.get("DISPLAY") or environment.get("WAYLAND_DISPLAY"):
        raise ValueError("shortcut QA must not use a desktop display")
    if not environment.get("DBUS_SESSION_BUS_ADDRESS"):
        raise ValueError("run shortcut QA inside a private dbus-run-session")
    if environment.get("IBUS_ADDRESS") != f"unix:path={runtime}/ibus.sock":
        raise ValueError("shortcut QA requires its private IBus address")
    if environment.get("SUZAKU_LINUX_IME_SOCKET") != str(runtime / "suzaku-ime/host.sock"):
        raise ValueError("shortcut QA requires its private host socket")
    if environment.get("SUZAKU_IME_CONFIG") != str(runtime / "ime.json"):
        raise ValueError("shortcut QA requires its private ime.json")
    for variable, directory in [("XDG_CONFIG_HOME", "config"), ("XDG_DATA_HOME", "data"),
                                ("XDG_CACHE_HOME", "cache")]:
        path = runtime / directory
        if (environment.get(variable) != str(path) or not path.is_dir() or path.is_symlink()
                or path.stat().st_uid != os.getuid()):
            raise ValueError(f"shortcut QA requires private {variable}")
    if environment.get("GSETTINGS_BACKEND") != "memory" or environment.get("GIO_USE_VFS") != "local":
        raise ValueError("shortcut QA requires memory-only GSettings and local GIO")
    if environment.get("LD_PRELOAD") or environment.get("SUZAKU_IBUS_INLINE_PREEDIT") == "1":
        raise ValueError("shortcut QA uses the default preedit path without injected libraries")
    if environment.get("SUZAKU_LEXICON_DIR"):
        raise ValueError("shortcut QA uses the built-in fallback corpus, not a personal lexicon")
    require_private_compose(runtime, environment)
    for path in [runtime / "ibus.sock", runtime / "suzaku-ime"]:
        if path.exists() or path.is_symlink():
            raise ValueError(f"shortcut QA requires a fresh runtime path: {path.name}")
    config = runtime / "ime.json"
    if config.exists() or config.is_symlink():
        raise ValueError("shortcut QA starts with a new owned ime.json")
    if not environment.get("SUZAKU_NATIVE_QA_BIN_DIR"):
        raise ValueError("shortcut QA requires an explicit rebuilt binary directory")
    binaries = Path(environment["SUZAKU_NATIVE_QA_BIN_DIR"]).resolve(strict=True)
    host = binaries / "linux_ime_host"
    if not binaries.is_dir() or not host.is_file() or not os.access(host, os.X_OK):
        raise ValueError("shortcut QA cannot find its rebuilt linux_ime_host")
    return runtime, config, host


class Fixture:
    def __init__(self, runtime, config, host, ibus, glib, gio):
        self.runtime, self.config, self.host_path = runtime, config, host
        self.IBus, self.GLib, self.Gio = ibus, glib, gio
        self.socket = runtime / "suzaku-ime/host.sock"
        self.processes, self.watchers, self.peers = [], [], []
        self.bus, self.daemon = None, None
        self.signals = IBusSignalObjects()
        self.started, self.phase = time.monotonic(), "startup"

    def pump(self):
        self.signals.drain(self.GLib.MainContext.default())
        for peer in self.peers:
            if peer.callback_error is not None:
                raise peer.callback_error
        for watch in self.watchers:
            watch.drain()

    def wait(self, predicate, label, timeout=5):
        until = time.monotonic() + timeout
        while time.monotonic() < until:
            self.pump()
            if predicate():
                return
            if any(process.poll() is not None for process in self.processes):
                raise AssertionError(f"owned native process exited during {label}")
            time.sleep(0.01)
        raise AssertionError(label)

    def command(self, request):
        payload = request.encode() if isinstance(request, str) else request
        with socket.socket(socket.AF_UNIX) as client:
            client.settimeout(2)
            client.connect(str(self.socket))
            client.sendall(payload)
            client.shutdown(socket.SHUT_WR)
            response = b""
            while True:
                chunk = client.recv(8192)
                if not chunk:
                    return response
                response += chunk
                assert len(response) <= MAX_RESPONSE, "unbounded control response"

    def settings(self):
        return json.loads(self.command("S"))["settings"]

    def control(self, request):
        result = json.loads(self.command(request))
        assert result["ok"], result
        return result["settings"]

    def update(self, patch):
        # This is exactly the narrow panel U protocol, not a config-file/R bypass.
        return self.control("U" + json.dumps(patch, separators=(",", ":")))

    def fresh(self):
        watch = Watch(self)
        try:
            self.wait(lambda: watch.latest is not None, "fresh companion snapshot")
            return watch.latest
        finally:
            watch.close()

    def unchanged(self, frame, peer, commits, label):
        assert self.fresh() == frame, label
        assert peer.commits == commits, f"{label}: unexpected commit"

    def start(self):
        self.config.write_text(json.dumps({"language": "en", "llm_enabled": False,
                                          "shortcut_profile": "standard", "shortcut_bindings": {}}))
        self.config.chmod(0o600)
        self.daemon = subprocess.Popen(["ibus-daemon", "--single",
                                        "--address=" + os.environ["IBUS_ADDRESS"], "--cache=none"],
                                       stdout=subprocess.DEVNULL)
        self.processes.append(self.daemon)
        self.wait(lambda: (self.runtime / "ibus.sock").exists(), "private IBus startup")
        self.processes.append(subprocess.Popen([str(self.host_path)], stdout=subprocess.DEVNULL,
                                              env=dict(os.environ)))
        self.wait(self.socket.exists, "private host startup")
        self.IBus.init()
        self.bus = self.IBus.Bus.new()
        assert self.bus.is_connected(), "private IBus connection unavailable"
        # list_engines enumerates the registry, not our dynamically registered
        # component. Prove private name ownership here; EnginePeer below then
        # requires a successful real Factory.CreateEngine response before keys.
        self.wait(lambda: self.bus.get_connection().call_sync(
            "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "NameHasOwner",
            self.GLib.Variant("(s)", (EnginePeer.destination,)), None,
            self.Gio.DBusCallFlags.NONE, 2000, None).unpack()[0], "private component name ownership")
        assert not self.settings()["llm_enabled"]

    def close(self):
        for peer in reversed(self.peers.copy()):
            with preserve_primary_failure("owned engine teardown"):
                peer.close()
        for watch in self.watchers.copy():
            with preserve_primary_failure("owned subscription teardown"):
                watch.close()
        for process in reversed(self.processes):
            with preserve_primary_failure("owned process teardown"):
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=3)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=3)


class Watch:
    def __init__(self, fixture):
        self.fixture = fixture
        self.sock = socket.socket(socket.AF_UNIX)
        self.sock.settimeout(2)
        self.sock.connect(str(fixture.socket))
        self.sock.sendall(b"W")
        self.sock.shutdown(socket.SHUT_WR)
        self.sock.setblocking(False)
        self.buffer, self.latest = b"", None
        fixture.watchers.append(self)

    def drain(self):
        while True:
            try:
                chunk = self.sock.recv(MAX_RESPONSE)
            except BlockingIOError:
                return
            if not chunk:
                return
            self.buffer += chunk
            while b"\n" in self.buffer:
                line, self.buffer = self.buffer.split(b"\n", 1)
                assert len(line) <= MAX_RESPONSE, "unbounded companion frame"
                frame = json.loads(line)
                assert frame["version"] == 1
                assert "committed_text" not in frame and "surrounding_text" not in frame
                if not frame["focused"] or frame["private"]:
                    assert not frame["seed"] and not frame["candidates"], "private frame exposed text"
                self.latest = frame
            assert len(self.buffer) <= MAX_RESPONSE, "unbounded companion buffer"

    def close(self):
        self.fixture.watchers.remove(self)
        self.sock.close()


class EnginePeer:
    """Own a real engine on the fixture bus; no desktop context is addressed."""
    destination = "org.freedesktop.IBus.Suzaku"
    interface = "org.freedesktop.IBus.Engine"

    def __init__(self, fixture):
        self.fixture = fixture
        self.connection = fixture.bus.get_connection()
        self.path = self.connection.call_sync(
            self.destination, "/org/freedesktop/IBus/Factory", "org.freedesktop.IBus.Factory",
            "CreateEngine", fixture.GLib.Variant("(s)", ("dev.suzaku.linux.ime",)),
            None, fixture.Gio.DBusCallFlags.NONE, 2000, None).unpack()[0]
        assert self.path.startswith("/") and self.path != "/", "factory returned no real engine path"
        self.commits, self.auxiliary, self.subscriptions = [], "", []
        self.callback_error = None

        def observed(_connection, _sender, _path, _interface, signal, params, *_data):
            try:
                text = fixture.IBus.Serializable.deserialize_object(params.get_child_value(0).get_variant())
                if signal == "CommitText":
                    self.commits.append(text.get_text())
                else:
                    self.auxiliary = text.get_text()
            except Exception as error:
                self.callback_error = error

        for signal in ["CommitText", "UpdateAuxiliaryText"]:
            self.subscriptions.append(self.connection.signal_subscribe(
                self.destination, self.interface, signal, self.path, None,
                fixture.Gio.DBusSignalFlags.NONE, observed))
        self.event("SetCapabilities", fixture.GLib.Variant("(u)", (int(
            fixture.IBus.Capabilite.FOCUS | fixture.IBus.Capabilite.PREEDIT_TEXT |
            fixture.IBus.Capabilite.LOOKUP_TABLE | fixture.IBus.Capabilite.AUXILIARY_TEXT),)))
        fixture.peers.append(self)

    def event(self, method, parameters=None):
        if self.callback_error is not None:
            raise self.callback_error
        return self.connection.call_sync(self.destination, self.path, self.interface, method,
                                         parameters, None, self.fixture.Gio.DBusCallFlags.NONE,
                                         2000, None).unpack()

    def key(self, keyval, state=0, code=0):
        return self.event("ProcessKeyEvent", self.fixture.GLib.Variant("(uuu)",
                         (int(keyval), int(code), int(state))))[0]

    def content(self, purpose, hints=0):
        self.connection.call_sync(self.destination, self.path, "org.freedesktop.DBus.Properties", "Set",
            self.fixture.GLib.Variant("(ssv)", (self.interface, "ContentType",
                self.fixture.GLib.Variant("(uu)", (int(purpose), int(hints))))),
            None, self.fixture.Gio.DBusCallFlags.NONE, 2000, None)

    def type(self, seed):
        for char in seed:
            assert self.key(self.fixture.IBus.unicode_to_keyval(char)), ("literal fixture key", char)

    def reset(self, watch, seed=""):
        # A latest frame that was already empty is not an acknowledgement.
        previous_context = self.fixture.fresh()["context"]
        self.event("Reset")
        self.fixture.wait(lambda: watch.latest is not None and
                          watch.latest["context"] > previous_context and not watch.latest["seed"],
                          "reset context+empty acknowledgement")
        if seed:
            self.type(seed)
            self.fixture.wait(lambda: watch.latest["seed"] == seed and watch.latest["candidates"],
                              "literal fixture draft")

    def select(self, watch, index):
        assert 0 <= index < len(watch.latest["candidates"]), (index, watch.latest)
        while watch.latest["selected"] != index:
            before = watch.latest
            forward = before["selected"] < index
            target = before["selected"] + (1 if forward else -1)
            assert self.key(self.fixture.IBus.KEY_Down if forward else self.fixture.IBus.KEY_Up)
            self.fixture.wait(lambda: watch.latest["revision"] > before["revision"] and
                              watch.latest["selected"] == target, "relative native selection")

    def close(self):
        try:
            self.event("FocusOut")
            self.connection.call_sync(self.destination, self.path, "org.freedesktop.IBus.Service",
                "Destroy", None, None, self.fixture.Gio.DBusCallFlags.NONE, 2000, None)
        finally:
            for subscription in self.subscriptions:
                self.connection.signal_unsubscribe(subscription)
            self.fixture.peers.remove(self)


def run_checks(fixture):
    ibus, wait = fixture.IBus, fixture.wait
    ctrl, alt, shift, release = (ibus.ModifierType.CONTROL_MASK, ibus.ModifierType.MOD1_MASK,
                                ibus.ModifierType.SHIFT_MASK, ibus.ModifierType.RELEASE_MASK)
    locks = ibus.ModifierType.LOCK_MASK | ibus.ModifierType.MOD2_MASK
    watch, peer = Watch(fixture), EnginePeer(fixture)
    peer.event("FocusIn")
    wait(lambda: watch.latest is not None and watch.latest["focused"], "private engine focus")

    fixture.phase = "six-binding-U-and-navigation"
    peer.reset(watch, "hel")
    before, settings = fixture.fresh(), fixture.settings()
    patched = fixture.update({"shortcut_bindings": BINDINGS})
    assert patched == dict(settings, shortcut_bindings=BINDINGS)
    assert json.loads(fixture.config.read_bytes()) == patched, "U did not persist complete settings"
    fresh = fixture.fresh()
    for field in ["context", "seed", "selected", "candidates"]:
        assert fresh[field] == before[field], f"U changed {field}"
    wait(lambda: watch.latest["revision"] >= fresh["revision"], "U snapshot publication")
    assert len(watch.latest["candidates"]) > PAGE_SIZE, "paging fixture requires a second fallback page"
    commits = peer.commits.copy()

    def navigate(key, mask, target, code=0):
        baseline = watch.latest
        assert peer.key(key, mask, code), "configured navigation was not consumed"
        if target == baseline["selected"]:
            fixture.unchanged(baseline, peer, commits, "clamped custom navigation changed state")
        else:
            wait(lambda: watch.latest["revision"] > baseline["revision"] and
                 watch.latest["selected"] == target, "configured navigation acknowledgement")
            assert watch.latest["seed"] == baseline["seed"]
            assert watch.latest["candidates"] == baseline["candidates"]
            assert peer.commits == commits

    # Custom aliases are additive: original navigation/adoption remains usable.
    navigate(ibus.KEY_Tab, 0, 1)
    navigate(ibus.KEY_Down, 0, 2)
    navigate(ibus.KEY_p, ctrl, 1)
    navigate(ibus.KEY_N, ctrl | locks, 2)
    navigate(ibus.KEY_F, ctrl | shift | locks, PAGE_SIZE)
    navigate(ibus.KEY_B, alt | shift | locks, 0)
    navigate(ibus.KEY_n, ctrl, 1, code=57)
    navigate(ibus.KEY_n, ctrl, 2, code=57)  # Navigation still repeats.
    assert not peer.key(ibus.KEY_n, release, code=57)
    navigate(ibus.KEY_Page_Down, 0, PAGE_SIZE)
    navigate(ibus.KEY_Page_Up, 0, 0)
    navigate(ibus.KEY_p, ctrl, 0)
    navigate(ibus.KEY_b, alt | shift, 0)
    print("PASS: actual six-field U persistence, candidate aliases, case/lock normalization, row/page navigation and repeat/clamp boundaries", flush=True)

    fixture.phase = "custom-adoption-undo-and-hold"
    word = next(index for index, row in enumerate(watch.latest["candidates"]) if row["text"] == "hello")
    peer.select(watch, word)
    assert peer.key(ibus.KEY_A, alt | shift | locks, code=38)
    wait(lambda: watch.latest["seed"] == "hello", "custom adoption is an editable draft")
    adopted = fixture.fresh()
    for mask in [alt | shift, 0, shift]:
        assert peer.key(ibus.KEY_a, mask, code=38)
        fixture.unchanged(adopted, peer, commits, "held adoption repeated after modifier release")
    assert not peer.key(ibus.KEY_a, release, code=38)
    assert peer.key(ibus.KEY_BackSpace)
    wait(lambda: watch.latest["seed"] == "hel", "custom adoption exact spelling undo")
    peer.select(watch, next(index for index, row in enumerate(watch.latest["candidates"])
                           if row["text"] == "hello"))
    assert peer.key(ibus.KEY_Return, shift, code=36), "legacy Shift+Enter adoption removed"
    assert not peer.key(ibus.KEY_Return, release, code=36)
    wait(lambda: watch.latest["seed"] == "hello", "legacy editable adoption")
    assert peer.key(ibus.KEY_BackSpace)
    wait(lambda: watch.latest["seed"] == "hel", "legacy adoption undo")
    peer.select(watch, next(index for index, row in enumerate(watch.latest["candidates"])
                           if row["text"] == "hello"))
    assert peer.key(ibus.KEY_a, alt | shift)
    wait(lambda: watch.latest["seed"] == "hello", "readopt before commit")
    assert peer.key(ibus.KEY_space)
    wait(lambda: watch.latest["seed"] == "hello ", "Space continues after custom adoption")
    assert peer.commits == commits
    assert peer.key(ibus.KEY_Return)
    wait(lambda: not watch.latest["seed"] and peer.commits[len(commits):] == ["hello "],
         "custom adoption commits exact payload once only on Enter")
    commits = peer.commits.copy()
    print("PASS: custom and legacy editable adoption, physical hold/release, exact undo, Space continuation and one explicit commit", flush=True)

    fixture.phase = "language-replacement-and-hold"
    for draft in ["", "nihao"]:
        peer.reset(watch, draft)
        baseline = fixture.fresh()
        assert not peer.key(ibus.KEY_space, ctrl | shift), "replaced default language chord still active"
        fixture.unchanged(baseline, peer, commits, "disabled default language chord changed state")
        original = fixture.settings()["language"]
        target = "zh-Hans" if original == "en" else "en"
        assert peer.key(ibus.KEY_L, ctrl | shift | locks, code=46)
        wait(lambda: fixture.settings()["language"] == target and watch.latest["context"] > baseline["context"]
             and watch.latest["seed"] == draft, "custom language durable context+draft boundary")
        assert json.loads(fixture.config.read_bytes())["language"] == target
        switched = fixture.fresh()
        for mask in [ctrl | shift, 0, shift]:
            assert peer.key(ibus.KEY_l, mask, code=46)
            fixture.unchanged(switched, peer, commits, "held language switched/typed again")
            assert fixture.settings()["language"] == target
        assert not peer.key(ibus.KEY_l, release, code=46)
        assert "Ctrl+Shift+Space" not in peer.auxiliary, "success feedback claims a replaced key"
        assert fixture.command(f'A{baseline["host"]} {baseline["revision"]} Tstale') == b"0"
    # Release and focus boundaries permit a genuinely new gesture immediately.
    before = fixture.fresh()
    assert peer.key(ibus.KEY_l, ctrl | shift, code=46)
    wait(lambda: watch.latest["context"] > before["context"], "released language gesture acknowledgement")
    peer.event("FocusOut")
    wait(lambda: not watch.latest["focused"] and not watch.latest["seed"], "focus-out retires draft and held keys")
    assert not peer.key(ibus.KEY_l, ctrl | shift, code=46), "unfocused language shortcut consumed"
    assert not peer.key(ibus.KEY_n, ctrl), "unfocused candidate shortcut consumed"
    peer.event("FocusIn")
    wait(lambda: watch.latest["focused"], "refocus after held language")
    expected = "zh-Hans" if fixture.settings()["language"] == "en" else "en"
    assert peer.key(ibus.KEY_l, ctrl | shift, code=46)
    wait(lambda: fixture.settings()["language"] == expected, "focus cleared held language identity")
    assert not peer.key(ibus.KEY_l, release, code=46)
    assert peer.commits == commits
    fixture.control("Len")
    print("PASS: custom language replaces default, empty/nonempty drafts survive, durable save, stale action rejection and held/focus/release ownership", flush=True)

    fixture.phase = "modifier-compose-empty-and-private-guards"
    peer.reset(watch, "hel")
    baseline = fixture.fresh()
    forbidden = [ibus.ModifierType.MOD3_MASK, ibus.ModifierType.MOD4_MASK, ibus.ModifierType.MOD5_MASK,
                 ibus.ModifierType.SUPER_MASK, ibus.ModifierType.HYPER_MASK, ibus.ModifierType.META_MASK,
                 ibus.ModifierType.BUTTON1_MASK, 1 << 27]
    for extra in forbidden + [alt]:
        assert not peer.key(ibus.KEY_n, ctrl | extra)
        assert not peer.key(ibus.KEY_l, ctrl | shift | extra)
        fixture.unchanged(baseline, peer, commits, "forbidden modifier intercepted shortcut")
        assert fixture.settings()["language"] == "en"
    peer.reset(watch)
    baseline = fixture.fresh()
    for key, mask in [(ibus.KEY_p, ctrl), (ibus.KEY_n, ctrl), (ibus.KEY_b, alt | shift),
                      (ibus.KEY_f, ctrl | shift), (ibus.KEY_a, alt | shift)]:
        assert not peer.key(key, mask), "candidate alias consumed an empty draft"
        fixture.unchanged(baseline, peer, commits, "empty candidate alias changed state")
    peer.reset(watch, "caf")
    assert peer.key(ibus.KEY_dead_acute)
    wait(lambda: "Compose…" in peer.auxiliary, "pending private Compose fixture")
    baseline = fixture.fresh()
    for key in [ibus.KEY_Control_L, ibus.KEY_Control_R, ibus.KEY_Shift_L, ibus.KEY_Shift_R,
                ibus.KEY_Alt_L, ibus.KEY_Alt_R]:
        assert not peer.key(key, ctrl | alt | shift), "pure modifier took pending Compose"
        fixture.unchanged(baseline, peer, commits, "pure modifier canceled pending Compose")
        assert "Compose…" in peer.auxiliary
    for key, mask in [(ibus.KEY_n, ctrl), (ibus.KEY_a, alt | shift), (ibus.KEY_l, ctrl | shift)]:
        assert not peer.key(key, mask), "custom alias took pending Compose"
        fixture.unchanged(baseline, peer, commits, "custom alias canceled pending Compose")
        assert fixture.settings()["language"] == "en" and "Compose…" in peer.auxiliary
    assert peer.key(ibus.KEY_e)
    wait(lambda: watch.latest["seed"] == "café", "Compose completed after ignored custom chords")
    for purpose, hints in [(ibus.InputPurpose.PASSWORD, 0), (ibus.InputPurpose.NUMBER, 0),
                           (ibus.InputPurpose.FREE_FORM, 1 << 11)]:
        peer.content(purpose, hints)
        wait(lambda: watch.latest["private"], "private shortcut context")
        if hints:
            peer.type("hel")  # Private local typing is allowed, optional aliases are not.
        baseline = fixture.fresh()
        for key, mask in [(ibus.KEY_n, ctrl), (ibus.KEY_a, alt | shift), (ibus.KEY_l, ctrl | shift)]:
            assert not peer.key(key, mask), "custom shortcut took private/bypass context"
            fixture.unchanged(baseline, peer, commits, "private shortcut changed redacted frame")
            assert fixture.settings()["language"] == "en"
        peer.content(ibus.InputPurpose.FREE_FORM)
        wait(lambda: not watch.latest["private"], "leave private shortcut context")
    print("PASS: System/Super/AltGr/unknown modifiers, empty candidate draft, pending Compose/pure modifiers, password/number/private guards", flush=True)

    fixture.phase = "home-row-and-invalid-U-atomicity"
    fixture.update({"shortcut_profile": "home-row"})
    peer.reset(watch, "hel")
    navigate(ibus.KEY_j, alt, 1)
    navigate(ibus.KEY_k, alt, 0)
    navigate(ibus.KEY_l, alt, PAGE_SIZE)
    navigate(ibus.KEY_h, alt, 0)
    peer.select(watch, next(index for index, row in enumerate(watch.latest["candidates"])
                           if row["text"] == "hello"))
    assert peer.key(ibus.KEY_semicolon, alt)
    wait(lambda: watch.latest["seed"] == "hello", "fixed HomeRow punctuation alias retained")
    assert peer.key(ibus.KEY_BackSpace)
    wait(lambda: watch.latest["seed"] == "hel", "HomeRow exact undo with custom bindings")
    invalid_patches = [
        {"shortcut_bindings": {"previous": "Ctrl+Q", "next": "ctrl+q"}},
        {"shortcut_bindings": {"unknown": "Ctrl+Q"}},
        {"shortcut_bindings": {"adopt": True}},
        {"shortcut_bindings": {"adopt": "Super+A"}},
        {"shortcut_bindings": {"adopt": "Ctrl+Alt+A"}},
        {"shortcut_bindings": {"adopt": "Alt+Space"}},
        {"shortcut_bindings": {"adopt": "Ctrl+Shift+Space"}},
        {"shortcut_bindings": {"adopt": "Alt+J"}},
        {"llm_enabled": True, "shortcut_bindings": {"adopt": "Alt+J"}},
    ]
    for patch in invalid_patches:
        saved, raw, baseline = fixture.settings(), fixture.config.read_bytes(), fixture.fresh()
        result = json.loads(fixture.command("U" + json.dumps(patch, separators=(",", ":"))))
        assert not result["ok"], ("invalid U accepted", patch)
        assert result["settings"] == saved and fixture.settings() == saved
        assert fixture.config.read_bytes() == raw, "invalid U partially persisted configuration"
        fixture.unchanged(baseline, peer, commits, "invalid U changed draft/selection")
        assert not fixture.settings()["llm_enabled"]
    print("PASS: original HomeRow aliases retained and illegal narrow patches leave settings/draft/config bytes unchanged", flush=True)

    fixture.phase = "U-size-limits-reset-and-reload"
    patch = "U" + json.dumps({"shortcut_bindings": BINDINGS}, separators=(",", ":"))
    assert 128 < len(patch.encode()) < 1024, "six bindings no longer exercise extended U path"
    padded = patch.encode() + b" " * (1024 - len(patch.encode()))
    assert len(padded) == 1024 and json.loads(fixture.command(padded))["ok"]
    saved, raw, baseline = fixture.settings(), fixture.config.read_bytes(), fixture.fresh()
    assert fixture.command(padded + b" ") == b"0", "1025-byte U accepted"
    assert fixture.settings() == saved and fixture.config.read_bytes() == raw
    fixture.unchanged(baseline, peer, commits, "oversize U partially applied")
    for operation in ["S", "L", "P", "R"]:
        legacy = operation.encode() + b" " * 126
        assert len(legacy) == 127
        # An invalid-but-in-bounds old control reaches the typed error handler.
        result = json.loads(fixture.command(legacy))
        assert not result["ok"] and result["settings"] == saved
        assert fixture.command(legacy + b" ") == b"0", "non-U 128-byte limit changed"
        assert fixture.settings() == saved and fixture.config.read_bytes() == raw
    assert fixture.control("R")["shortcut_bindings"] == BINDINGS, "custom bindings failed actual reload"
    peer.reset(watch, "hel")
    navigate(ibus.KEY_n, ctrl, 1)
    fixture.update({"shortcut_profile": "standard", "shortcut_bindings": {}})
    assert fixture.settings()["shortcut_bindings"] == {}
    peer.reset(watch, "hel")
    baseline = fixture.fresh()
    for key, mask in [(ibus.KEY_n, ctrl), (ibus.KEY_a, alt | shift), (ibus.KEY_l, ctrl | shift)]:
        assert not peer.key(key, mask), "reset left a custom shortcut active"
        fixture.unchanged(baseline, peer, commits, "removed custom shortcut changed state")
    assert peer.key(ibus.KEY_space, ctrl | shift, code=65), "reset did not restore default language chord"
    wait(lambda: fixture.settings()["language"] == "zh-Hans" and watch.latest["seed"] == "hel",
         "default language shortcut restored after U reset")
    assert not peer.key(ibus.KEY_space, release, code=65)
    assert peer.commits == commits and not fixture.settings()["llm_enabled"]
    print("PASS: real U 1024/1025 bounds, unchanged old 127/128 controls, persisted reload, custom reset and default language restoration", flush=True)


def run_long_checks(fixture):
    """Owned companion prefix + actual IBus tail/chords; not physical typing QA."""
    ibus, wait = fixture.IBus, fixture.wait
    ctrl, alt, shift, release = (ibus.ModifierType.CONTROL_MASK, ibus.ModifierType.MOD1_MASK,
                                ibus.ModifierType.SHIFT_MASK, ibus.ModifierType.RELEASE_MASK)
    locks = ibus.ModifierType.LOCK_MASK | ibus.ModifierType.MOD2_MASK
    profiles = ["standard", "home-row"]
    # Fixed authored fallback payloads, not display labels or observed candidates.
    cases = [("en", "Earlier note. " * 30, "please sen", "please send", " me the details."),
             ("zh-Hans", "前文。" * 100, "hui yi", "会议", "什么时候开始？")]
    saved = fixture.settings()
    watch, peer = Watch(fixture), EnginePeer(fixture)
    checked = 0

    def ready():
        frame = fixture.fresh()
        wait(lambda: watch.latest == frame, "long custom current snapshot publication")
        assert frame["focused"] and not frame["private"]
        return frame

    def navigation(key, mask, target):
        before, commits = ready(), peer.commits.copy()
        assert peer.key(key, mask), "long custom navigation was not consumed"
        if target == before["selected"]:
            fixture.unchanged(before, peer, commits, "long custom boundary changed full snapshot")
        else:
            wait(lambda: watch.latest["revision"] > before["revision"] and
                 watch.latest["selected"] == target, "long custom navigation acknowledgement")
            after = ready()
            for field in ["host", "context", "seed", "candidates", "language"]:
                assert after[field] == before[field], f"long custom navigation changed {field}"
            assert peer.commits == commits, "long custom navigation committed text"

    def select_text(expected, kind=None):
        candidates = ready()["candidates"]
        index = next(i for i, row in enumerate(candidates)
                     if row["text"] == expected and row["source"] == "local"
                     and (kind is None or row["kind"] == kind))
        # Exercise the configured row commands, not EnginePeer.select's arrows.
        for _ in range(len(candidates)):
            selected = ready()["selected"]
            if selected == index:
                return
            forward = selected < index
            navigation(ibus.KEY_n if forward else ibus.KEY_p, ctrl,
                       selected + (1 if forward else -1))
        raise AssertionError("long custom selection did not reach the fixed payload")

    def prepare(language, prefix, tail):
        peer.reset(watch)
        fixture.control("L" + language)
        wait(lambda: fixture.settings()["language"] == language and
             watch.latest["language"] == language, "long custom language applied")
        frame = ready()
        assert not frame["seed"]
        # Han prefixes in an empty draft belong to the application. Prepare an
        # explicitly owned prefix via the real revision-bound companion action,
        # then send the reading as actual IBus events, draining every long frame.
        assert fixture.command(f'A{frame["host"]} {frame["revision"]} T{prefix}') == b"1"
        wait(lambda: watch.latest["seed"] == prefix, "long custom owned prefix")
        typed = prefix
        for character in tail:
            assert peer.key(ibus.unicode_to_keyval(character))
            typed += character
            wait(lambda: watch.latest["seed"] == typed, "long custom drained native tail")
        raw = prefix + tail
        assert len(raw) > 256 and ready()["seed"] == raw
        assert any(row["text"] == raw for row in watch.latest["candidates"])
        assert all(row["text"].startswith(prefix) and row["source"] == "local"
                   for row in watch.latest["candidates"])
        return raw

    def adopt(expected, kind):
        select_text(expected, kind)
        before, commits = ready(), peer.commits.copy()
        assert expected != before["seed"], "long custom case must change the draft"
        assert peer.key(ibus.KEY_A, alt | shift | locks, code=38)
        wait(lambda: watch.latest["revision"] > before["revision"] and
             watch.latest["seed"] == expected, "long custom editable adoption")
        adopted = ready()
        assert adopted["context"] == before["context"]
        for key, mask in [(ibus.KEY_A, alt | shift), (ibus.KEY_a, 0), (ibus.KEY_a, shift)]:
            assert peer.key(key, mask, code=38)
            fixture.unchanged(adopted, peer, commits, "long custom held adoption repeated")
        # An unrelated release cannot free the held physical key identity.
        assert not peer.key(ibus.KEY_a, release, code=39)
        assert peer.key(ibus.KEY_a, 0, code=38)
        fixture.unchanged(adopted, peer, commits, "long custom wrong release freed adoption")
        assert not peer.key(ibus.KEY_a, release, code=38)
        fixture.unchanged(adopted, peer, commits, "long custom matching release edited text")

    def finish(expected, commits):
        select_text(expected)
        before = ready()
        assert peer.commits == commits, "long custom draft committed before Enter"
        assert peer.key(ibus.KEY_Return, code=36)
        wait(lambda: not watch.latest["seed"] and watch.latest["context"] > before["context"]
             and peer.commits == commits + [expected], "long custom exact single commit")
        empty = ready()
        assert not peer.key(ibus.KEY_Return, release, code=36)
        assert not peer.key(ibus.KEY_Return, code=36), "empty Enter should return to the application"
        fixture.unchanged(empty, peer, commits + [expected], "long custom Enter duplicated commit")

    try:
        peer.event("FocusIn")
        wait(lambda: watch.latest is not None and watch.latest["focused"], "long custom engine focus")
        for profile in profiles:
            fixture.update({"llm_enabled": False, "shortcut_profile": profile,
                            "shortcut_bindings": BINDINGS})
            assert fixture.settings()["shortcut_bindings"] == BINDINGS
            assert not fixture.settings()["llm_enabled"]
            assert json.loads(fixture.config.read_bytes())["shortcut_profile"] == profile
            for language, prefix, tail, word, suffix in cases:
                for kind in ["word", "sentence"]:
                    fixture.phase = f"long-custom-{profile}-{language}-{kind}"
                    commits = peer.commits.copy()
                    raw = prepare(language, prefix, tail)
                    expected = prefix + word + (suffix if kind == "sentence" else "")
                    adopt(expected, kind)
                    assert ready()["selected"] == 0
                    navigation(ibus.KEY_p, ctrl, 0)
                    navigation(ibus.KEY_B, alt | shift, 0)
                    assert peer.key(ibus.KEY_BackSpace)
                    wait(lambda: watch.latest["seed"] == raw, "long custom undo restores full raw draft")
                    assert peer.commits == commits
                    adopt(expected, kind)
                    assert peer.key(ibus.KEY_space)
                    draft = expected + " "
                    wait(lambda: watch.latest["seed"] == draft, "long custom Space stays in the draft")
                    assert peer.commits == commits
                    if kind == "word":
                        sentence = draft + (suffix.lstrip(" ") if language == "en" else suffix)
                        adopt(sentence, "sentence")
                        assert peer.key(ibus.KEY_BackSpace)
                        wait(lambda: watch.latest["seed"] == draft,
                             "long custom sentence undo preserves full adopted word and Space")
                        adopt(sentence, "sentence")
                        assert peer.key(ibus.KEY_space)
                        draft = sentence + " "
                        wait(lambda: watch.latest["seed"] == draft, "long custom sentence Space continuity")
                    assert peer.key(ibus.KEY_7, alt), "Alt+digit must remain literal after long adoption"
                    expected_commit = draft + "7"
                    wait(lambda: watch.latest["seed"] == expected_commit, "long custom literal digit")
                    finish(expected_commit, commits)
                    checked += 1

            fixture.phase = f"long-custom-{profile}-real-pages"
            commits = peer.commits.copy()
            prefix = "note " * 60
            raw = prepare("en", prefix, "hel")
            count = len(ready()["candidates"])
            assert count > PAGE_SIZE, "long custom paging requires a real second fallback page"
            navigation(ibus.KEY_n, ctrl, 1)
            navigation(ibus.KEY_p, ctrl, 0)
            navigation(ibus.KEY_F, ctrl | shift | locks, PAGE_SIZE)
            navigation(ibus.KEY_B, alt | shift | locks, 0)
            last_start = (count - 1) // PAGE_SIZE * PAGE_SIZE
            for start in range(PAGE_SIZE, last_start + 1, PAGE_SIZE):
                navigation(ibus.KEY_f, ctrl | shift, start)
            navigation(ibus.KEY_f, ctrl | shift, last_start)
            for start in range(last_start - PAGE_SIZE, -1, -PAGE_SIZE):
                navigation(ibus.KEY_b, alt | shift, start)
            navigation(ibus.KEY_b, alt | shift, 0)
            expected = prefix + "hello"
            adopt(expected, "word")
            navigation(ibus.KEY_b, alt | shift, 0)
            assert peer.key(ibus.KEY_BackSpace)
            wait(lambda: watch.latest["seed"] == raw, "long custom paged adoption full undo")
            adopt(expected, "word")
            finish(expected, commits)
            checked += 1
    finally:
        with preserve_primary_failure("long custom owned engine teardown"):
            peer.close()
        with preserve_primary_failure("long custom owned subscription teardown"):
            watch.close()
        with preserve_primary_failure("long custom private language restoration"):
            fixture.control("L" + saved["language"])
        with preserve_primary_failure("long custom private settings restoration"):
            fixture.update({"llm_enabled": saved["llm_enabled"],
                            "shortcut_profile": saved["shortcut_profile"],
                            "shortcut_bindings": saved["shortcut_bindings"]})
            assert fixture.settings() == saved
            assert json.loads(fixture.config.read_bytes()) == saved
    assert checked == 10, checked
    print("PASS: 10 standard/home-row long English/Chinese custom word/sentence workflows, complete-prefix undo, held releases, Space/literal digits, real page boundaries and exact single commits", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--preflight-only", action="store_true")
    arguments = parser.parse_args()
    runtime, config, host = require_fixture(os.environ)
    import gi
    gi.require_version("IBus", "1.0")
    from gi.repository import IBus, GLib, Gio
    if arguments.preflight_only:
        print("PASS: private shortcut fixture preflight only; no native assertions executed")
        return
    fixture = Fixture(runtime, config, host, IBus, GLib, Gio)
    try:
        fixture.start()
        run_checks(fixture)
        run_long_checks(fixture)
        print("PASS: actual Rust U configuration -> C adapter -> private IBus custom shortcut gate", flush=True)
    finally:
        failure = sys.exc_info()[1]
        if failure is not None:
            report_native_diagnostic("failure-before-teardown", phase=fixture.phase, started=fixture.started,
                                     bus=fixture.bus, daemon=fixture.daemon, processes=fixture.processes,
                                     error_type=type(failure).__name__, error=str(failure)[:1024])
        fixture.close()


if __name__ == "__main__":
    main()
