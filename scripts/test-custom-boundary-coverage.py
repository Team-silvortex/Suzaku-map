#!/usr/bin/env python3
"""Source-only freeze guards for custom-language ownership/Compose boundaries.

This file never imports the native suite, opens a bus or starts a model/host.
Passing proves fixture structure and historical preservation, not native,
physical-keyboard, installation or desktop acceptance.
"""
import ast
import hashlib
from pathlib import Path
import unittest
from fixtures.native_partition_projection import historical_source


ROOT = Path(__file__).resolve().parent.parent
NATIVE_PATH = ROOT / "scripts/test-native-sync.py"
SOURCE = historical_source(NATIVE_PATH)
TREE = ast.parse(SOURCE)
HELPER = "check_custom_language_pending_boundaries"
PREDECESSOR = "check_custom_language_pending_long"
NATIVE_BEFORE_SHA256 = "918bc1306587da6294de8f08e3f3b0afe77ca9be464c8659818578c5af6fa78c"
GUARD_BEFORE_SHA256 = {
    "scripts/test-custom-language-coverage.py":
        "6b86aa387936bb2148f2a7f12542bb5631463c6b0cb7ba410f804aab09905510",
    "scripts/test-held-language-coverage.py":
        "a36b23993170009bf17471d49f8ee4c7c3b545967ce370bb735a3948295230e1",
    "scripts/test-native-model-fixture.py":
        "e5c7cb7d1b3a0ffaddcdba6d398032dfc8b080ac74b233550fdfdecb6f2dfd08",
}
UNCHANGED_FILE_SHA256 = {
    "scripts/test-linux-ci.sh":
        "521c5da0c7cc6a586e64fdb7fd57312a1d534363a3b9ccce376fbc57b748adbb",
    "src/linux/ibus_engine_bridge.c":
        "61b117525190f43b8999a2e4b3ee9408777aa603d9bb7b22d05b288dfa9daedb",
    "src/linux/ibus_language.inc.c":
        "0b40a1a36294ac235674b03941572ec3fbd356f1f3a7288720c16c25d2a7f139",
    "scripts/fixtures/slow-settings-fsync.c":
        "3696481daa17557741df76d6ab340f7c447e69018151cf8f6d1b95a61b083748",
}
BOUNDARY_MAP = '''BOUNDARY_APPENDED_CALLS = {
    "check_custom_language_pending_boundaries": ("check_custom_language_pending_long", 1),
}
'''
HELD_LOOP_BEFORE = "    for name, (predecessor, count) in (APPENDED_CALLS | CUSTOM_APPENDED_CALLS).items():\n"
HELD_LOOP_AFTER = "    for name, (predecessor, count) in (APPENDED_CALLS | CUSTOM_APPENDED_CALLS | BOUNDARY_APPENDED_CALLS).items():\n"
CUSTOM_LOOP_BEFORE = "    for name, (predecessor, count) in APPENDED_CALLS.items():\n"
CUSTOM_LOOP_AFTER = "    for name, (predecessor, count) in (APPENDED_CALLS | BOUNDARY_APPENDED_CALLS).items():\n"
MODEL_MAP_ADDITION = '        appended |= {"check_custom_language_pending_boundaries": "check_custom_language_pending_long"}\n'
MODEL_SEEN_ADDITION = '        self.assertEqual(seen.pop("check_custom_language_pending_boundaries"), 1)\n'
CUSTOM_BOUNDARY_CONSTANTS = """HELD_BOUNDARY_ADDITION = '''BOUNDARY_APPENDED_CALLS = {
    "check_custom_language_pending_boundaries": ("check_custom_language_pending_long", 1),
}
'''
HELD_BOUNDARY_LOOP = "    for name, (predecessor, count) in (APPENDED_CALLS | CUSTOM_APPENDED_CALLS | BOUNDARY_APPENDED_CALLS).items():\\n"
MODEL_BOUNDARY_MAP_ADDITION = '        appended |= {"check_custom_language_pending_boundaries": "check_custom_language_pending_long"}\\n'
MODEL_BOUNDARY_SEEN_ADDITION = '        self.assertEqual(seen.pop("check_custom_language_pending_boundaries"), 1)\\n'
"""
CUSTOM_HELD_PREPROCESS = '''        source = replace_once(source, HELD_BOUNDARY_ADDITION, "")
        source = replace_once(source, HELD_BOUNDARY_LOOP, HELD_LOOP_AFTER)
'''
CUSTOM_MODEL_PREPROCESS = '''        source = replace_once(source, MODEL_BOUNDARY_MAP_ADDITION, "")
        source = replace_once(source, MODEL_BOUNDARY_SEEN_ADDITION, "")
'''
CONTINUATION_MAP = '''CONTINUATION_APPENDED_CALLS = {
    "check_custom_language_pending_continuation": ("check_custom_language_pending_boundaries", 1),
}
'''
HELD_CONTINUATION_LOOP = "    for name, (predecessor, count) in (APPENDED_CALLS | CUSTOM_APPENDED_CALLS | BOUNDARY_APPENDED_CALLS | CONTINUATION_APPENDED_CALLS).items():\n"
CUSTOM_CONTINUATION_LOOP = "    for name, (predecessor, count) in (APPENDED_CALLS | BOUNDARY_APPENDED_CALLS | CONTINUATION_APPENDED_CALLS).items():\n"
MODEL_CONTINUATION_MAP_ADDITION = '        appended |= {"check_custom_language_pending_continuation": "check_custom_language_pending_boundaries"}\n'
MODEL_CONTINUATION_SEEN_ADDITION = '        self.assertEqual(seen.pop("check_custom_language_pending_continuation"), 1)\n'
CUSTOM_CONTINUATION_CONSTANTS = """HELD_CONTINUATION_ADDITION = '''CONTINUATION_APPENDED_CALLS = {
    "check_custom_language_pending_continuation": ("check_custom_language_pending_boundaries", 1),
}
'''
HELD_CONTINUATION_LOOP = "    for name, (predecessor, count) in (APPENDED_CALLS | CUSTOM_APPENDED_CALLS | BOUNDARY_APPENDED_CALLS | CONTINUATION_APPENDED_CALLS).items():\\n"
MODEL_CONTINUATION_MAP_ADDITION = '        appended |= {"check_custom_language_pending_continuation": "check_custom_language_pending_boundaries"}\\n'
MODEL_CONTINUATION_SEEN_ADDITION = '        self.assertEqual(seen.pop("check_custom_language_pending_continuation"), 1)\\n'
"""
CUSTOM_CONTINUATION_HELD_PREPROCESS = '''        source = replace_once(source, HELD_CONTINUATION_ADDITION, "")
        source = replace_once(source, HELD_CONTINUATION_LOOP, HELD_BOUNDARY_LOOP)
'''
CUSTOM_CONTINUATION_MODEL_PREPROCESS = '''        source = replace_once(source, MODEL_CONTINUATION_MAP_ADDITION, "")
        source = replace_once(source, MODEL_CONTINUATION_SEEN_ADDITION, "")
'''
LANGUAGE_CASES = '''[("en", "zh-Hans", "Earlier note. " * 30, "hel", "hello"),
                     ("zh-Hans", "en", "前文。" * 100, "hui yi", "会议")]'''
BINDINGS = {"previous": "Ctrl+P", "next": "Ctrl+N", "previous-page": "Alt+Shift+B",
            "next-page": "Ctrl+Shift+F", "adopt": "Alt+Shift+A", "switch-language": "Ctrl+Shift+L"}
CASES = ["focus-out", "focus-transfer", "password", "pin", "hidden",
         "compose-pending", "compose-completed"]


def only(nodes, label):
    nodes = list(nodes)
    if len(nodes) != 1:
        raise AssertionError(f"Expected one {label}, found {len(nodes)}")
    return nodes[0]


def definition(tree, name):
    return only((node for node in tree.body if isinstance(node, ast.FunctionDef)
                 and node.name == name), name)


def same_ast(left, right):
    return ast.dump(left, include_attributes=False) == ast.dump(right, include_attributes=False)


def statement(text):
    return ast.parse(text).body[0]


def exact_call(node, name):
    return same_ast(node, statement(name + "(bus)"))


def project_continuation(source):
    """Strip only the later continuation helper and its exact adjacent bus call."""
    tree, lines = ast.parse(source), source.splitlines(keepends=True)
    name, predecessor = "check_custom_language_pending_continuation", "check_custom_language_pending_boundaries"
    helper = definition(tree, name)
    if helper.decorator_list or lines[helper.end_lineno:helper.end_lineno + 2] != ["\n", "\n"]:
        raise AssertionError("Continuation helper must be undecorated with two following blank lines")
    call = only((node for node in ast.walk(tree) if isinstance(node, ast.Expr)
                 and isinstance(node.value, ast.Call) and isinstance(node.value.func, ast.Name)
                 and node.value.func.id == name), "continuation call")
    if not exact_call(call, name) or call.lineno != call.end_lineno:
        raise AssertionError("Continuation must have an exact single-line bus call")
    indent = lines[call.lineno - 1].split(name + "(", 1)[0]
    if lines[call.lineno - 1] != indent + name + "(bus)\n":
        raise AssertionError("Continuation call may contain no extra source")
    if lines[call.lineno - 2] != indent + predecessor + "(bus)\n":
        raise AssertionError("Continuation must immediately follow its boundary predecessor")
    omitted = set(range(helper.lineno - 1, helper.end_lineno + 2)) | {call.lineno - 1}
    return "".join(line for index, line in enumerate(lines) if index not in omitted)


def project_native(source):
    """Remove only one undecorated helper and its one exact adjacent bus call."""
    source = project_continuation(source)
    tree = ast.parse(source)
    lines = source.splitlines(keepends=True)
    helper = definition(tree, HELPER)
    if helper.decorator_list or lines[helper.end_lineno:helper.end_lineno + 2] != ["\n", "\n"]:
        raise AssertionError("Boundary helper must be undecorated with two following blank lines")
    calls = [node for node in ast.walk(tree) if isinstance(node, ast.Expr)
             and isinstance(node.value, ast.Call) and isinstance(node.value.func, ast.Name)
             and node.value.func.id == HELPER]
    call = only(calls, "boundary call")
    if not exact_call(call, HELPER) or call.lineno != call.end_lineno:
        raise AssertionError("Only an exact single-line boundary bus call may be projected")
    indent = lines[call.lineno - 1].split(HELPER + "(", 1)[0]
    if lines[call.lineno - 1] != indent + HELPER + "(bus)\n":
        raise AssertionError("Boundary call must contain no extra source")
    if lines[call.lineno - 2] != indent + PREDECESSOR + "(bus)\n":
        raise AssertionError("Boundary call must follow its exact pending-long predecessor")
    branch = only((node for node in ast.walk(tree) if isinstance(node, ast.If)
                  and same_ast(node.test, ast.parse(
                      'os.environ.get("SUZAKU_NATIVE_CONTROL_IO_ONLY") == "1"', mode="eval").body)
                  and call in node.body), "control-IO boundary dispatch")
    expected = ['check_settings_io_responsiveness(bus)', 'check_language_shortcut_slow_io(bus)',
                'check_held_language_pending(bus)', PREDECESSOR + '(bus)', HELPER + '(bus)',
                'raise SystemExit(0)']
    if branch.orelse or len(branch.body) != len(expected) or not all(
            same_ast(node, statement(text)) for node, text in zip(branch.body, expected)):
        raise AssertionError("Boundary dispatch must preserve its exact control-IO body")
    omitted = set(range(helper.lineno - 1, helper.end_lineno + 2)) | {call.lineno - 1}
    return "".join(line for index, line in enumerate(lines) if index not in omitted)


def replace_once(source, before, after):
    if source.count(before) != 1:
        raise AssertionError("Projection requires exactly one fixed source block")
    return source.replace(before, after, 1)


def project_guard(path, source):
    if path == "scripts/test-held-language-coverage.py":
        source = replace_once(source, "\n" + CONTINUATION_MAP, "\n")
        source = replace_once(source, HELD_CONTINUATION_LOOP, HELD_LOOP_AFTER)
        source = replace_once(source, "\n" + BOUNDARY_MAP, "\n")
        return replace_once(source, HELD_LOOP_AFTER, HELD_LOOP_BEFORE)
    if path == "scripts/test-native-model-fixture.py":
        source = replace_once(source, MODEL_CONTINUATION_MAP_ADDITION, "")
        source = replace_once(source, MODEL_CONTINUATION_SEEN_ADDITION, "")
        source = replace_once(source, MODEL_MAP_ADDITION, "")
        return replace_once(source, MODEL_SEEN_ADDITION, "")
    if path == "scripts/test-custom-language-coverage.py":
        source = replace_once(source, "\n" + CONTINUATION_MAP, "\n")
        source = replace_once(source, CUSTOM_CONTINUATION_LOOP, CUSTOM_LOOP_AFTER)
        source = replace_once(source, CUSTOM_CONTINUATION_CONSTANTS, "")
        source = replace_once(source, CUSTOM_CONTINUATION_HELD_PREPROCESS, "")
        source = replace_once(source, CUSTOM_CONTINUATION_MODEL_PREPROCESS, "")
        source = replace_once(source, "\n" + BOUNDARY_MAP, "\n")
        source = replace_once(source, CUSTOM_LOOP_AFTER, CUSTOM_LOOP_BEFORE)
        source = replace_once(source, CUSTOM_BOUNDARY_CONSTANTS, "")
        source = replace_once(source, CUSTOM_HELD_PREPROCESS, "")
        return replace_once(source, CUSTOM_MODEL_PREPROCESS, "")
    raise AssertionError("Unknown historical guard")


class CustomBoundaryCoverageTests(unittest.TestCase):
    def helper(self):
        return definition(TREE, HELPER)

    def nested(self, name):
        return only((node for node in self.helper().body if isinstance(node, ast.FunctionDef)
                     and node.name == name), name)

    def assigned(self, node, name):
        assignment = only((item for item in node.body if isinstance(item, ast.Assign)
                           and len(item.targets) == 1 and isinstance(item.targets[0], ast.Name)
                           and item.targets[0].id == name), "assignment " + name)
        return assignment.value

    def owners(self):
        node = self.helper()
        outer = only((item for item in node.body if isinstance(item, ast.Try)), "outer try")
        profile = only((item for item in outer.body if isinstance(item, ast.For)), "profile loop")
        language = only((item for item in profile.body if isinstance(item, ast.For)), "language loop")
        case = only((item for item in language.body if isinstance(item, ast.For)), "case loop")
        owner = only((item for item in case.body if isinstance(item, ast.Try)), "case try")
        return node, outer, profile, language, case, owner

    def has(self, node, text):
        expected = statement(text)
        self.assertTrue(any(same_ast(item, expected) for item in ast.walk(node)), text)

    def predicate(self, node, text):
        expected = ast.parse(text, mode="eval").body
        self.assertTrue(any(same_ast(item, expected) for item in ast.walk(node)), text)

    def sequence(self, body, texts):
        position = 0
        for text in texts:
            matches = [index for index in range(position, len(body))
                       if same_ast(body[index], statement(text))]
            self.assertTrue(matches, "Missing or out-of-order statement: " + text)
            position = matches[0] + 1

    def branch(self, node, text):
        predicate = ast.parse(text, mode="eval").body
        return only((item for item in ast.walk(node) if isinstance(item, ast.If)
                     and same_ast(item.test, predicate)), "branch " + text)

    def test_native_syntax_without_execution(self):
        compile(TREE, str(NATIVE_PATH), "exec")

    def test_whole_native_fixture_only_adds_one_helper_and_one_control_IO_call(self):
        self.assertEqual(hashlib.sha256(project_native(SOURCE).encode()).hexdigest(), NATIVE_BEFORE_SHA256)

    def test_historical_guards_only_add_exact_boundary_projection_support(self):
        for path, expected in GUARD_BEFORE_SHA256.items():
            with self.subTest(path=path):
                source = historical_source(ROOT / path)
                compile(ast.parse(source), path, "exec")
                self.assertEqual(hashlib.sha256(project_guard(path, source).encode()).hexdigest(), expected)

    def test_runner_preload_bridge_and_language_worker_are_unchanged(self):
        for path, expected in UNCHANGED_FILE_SHA256.items():
            with self.subTest(path=path):
                self.assertEqual(hashlib.sha256(historical_source(ROOT / path).encode()).hexdigest(), expected)

    def test_projection_rejects_wrong_arguments_moved_duplicate_or_extra_source_calls(self):
        call = only((node for node in ast.walk(TREE) if exact_call(node, HELPER)), "boundary call")
        lines = SOURCE.splitlines(keepends=True)
        original = lines[call.lineno - 1]
        indent = original[:len(original) - len(original.lstrip())]
        for altered in [f"{HELPER}(other)", f"{HELPER}(bus, bus)", f"{HELPER}(bus=bus)",
                        f"{HELPER}(bus); check_language_shortcut(bus)"]:
            with self.subTest(altered=altered):
                changed = list(lines)
                changed[call.lineno - 1] = indent + altered + "\n"
                with self.assertRaises(AssertionError):
                    project_native("".join(changed))
        moved = list(lines)
        moved[call.lineno - 2:call.lineno] = reversed(moved[call.lineno - 2:call.lineno])
        with self.assertRaises(AssertionError):
            project_native("".join(moved))
        duplicate = list(lines)
        duplicate.insert(call.lineno, original)
        with self.assertRaises(AssertionError):
            project_native("".join(duplicate))

    def test_projections_cannot_hide_old_fixture_deadline_or_guard_edits(self):
        changed = SOURCE.replace("time.sleep(0.01)", "time.sleep(0.02)", 1)
        self.assertNotEqual(hashlib.sha256(project_native(changed).encode()).hexdigest(), NATIVE_BEFORE_SHA256)
        for path, expected in GUARD_BEFORE_SHA256.items():
            with self.subTest(path=path):
                source = historical_source(ROOT / path)
                changed = source + "\n# unrelated historical guard mutation\n"
                self.assertNotEqual(hashlib.sha256(project_guard(path, changed).encode()).hexdigest(), expected)
                modified = source.replace('("check_custom_language_pending_long", 1)',
                                          '("check_custom_language_pending_long", 2)', 1)
                if modified != source:
                    with self.assertRaises(AssertionError):
                        project_guard(path, modified)

    def test_fixed_28_routes_preserve_languages_profiles_bindings_and_distinct_undo_oracles(self):
        node, _outer, profile, language, case, _owner = self.owners()
        self.assertEqual(ast.literal_eval(self.assigned(node, "bindings")), BINDINGS)
        self.assertTrue(same_ast(self.assigned(node, "languages"), ast.parse(LANGUAGE_CASES, mode="eval").body))
        self.assertEqual(ast.literal_eval(self.assigned(node, "cases")), CASES)
        self.assertTrue(same_ast(profile.target, ast.Name(id="profile", ctx=ast.Store())))
        self.assertEqual(ast.literal_eval(profile.iter), ["standard", "home-row"])
        self.assertTrue(same_ast(language.target, statement("original, target, prefix, tail, word = None").targets[0]))
        self.assertTrue(same_ast(language.iter, ast.Name(id="languages", ctx=ast.Load())))
        self.assertTrue(same_ast(case.target, ast.Name(id="case", ctx=ast.Store())))
        self.assertTrue(same_ast(case.iter, ast.Name(id="cases", ctx=ast.Load())))
        self.has(profile, 'assert json.loads(command("U" + json.dumps({"shortcut_profile": profile, "shortcut_bindings": bindings})))["ok"]')
        self.has(node, 'assert checked == 28, checked')
        self.assertEqual(sum(same_ast(item, statement("checked += 1")) for item in ast.walk(node)), 1)
        self.has(node, 'assert len(raw) > 256 and raw != adopted[:-1], "custom boundary undo oracles must differ"')
        for prefix, tail, word in [("Earlier note. " * 30, "hel", "hello"),
                                   ("前文。" * 100, "hui yi", "会议")]:
            self.assertGreater(len(prefix + tail), 256)
            self.assertNotEqual(prefix + tail, (prefix + word)[:-1])

    def test_owned_prefix_reset_ack_drained_native_tail_and_custom_exact_selection(self):
        node, _outer, _profile, _language, _case, owner = self.owners()
        self.sequence(owner.body, ['peer.event("FocusIn")', 'old_context = fresh()["context"]',
                                   'peer.event("Reset")', 'assert action(fresh(), "T" + prefix)', 'raw = prefix'])
        self.predicate(owner, 'lambda: watch.latest["context"] > old_context and not watch.latest["seed"]')
        self.has(node, 'assert json.loads(command("L" + original))["ok"]')
        typing = only((item for item in owner.body if isinstance(item, ast.For)
                       and same_ast(item.target, ast.Name(id="character", ctx=ast.Store()))), "tail loop")
        self.assertTrue(same_ast(typing.iter, ast.Name(id="tail", ctx=ast.Load())))
        self.sequence(typing.body, ['assert peer.process_key_event(IBus.unicode_to_keyval(character))',
                                    'raw += character'])
        self.predicate(typing, 'lambda: watch.latest["seed"] == raw')
        self.has(owner, 'select(peer, adopted, "word")')
        self.has(owner, 'assert peer.process_key_event(IBus.KEY_A, 38, alt | shift)')
        selection = self.nested("select")
        self.predicate(selection, 'row["text"] == text and row["source"] == "local" and (kind is None or row["kind"] == kind)')
        self.has(selection, 'assert peer.process_key_event(IBus.KEY_n if forward else IBus.KEY_p, 0, ctrl)')
        self.has(selection, 'assert watch.latest["seed"] == before["seed"] and watch.latest["candidates"] == rows')
        self.assertFalse(any(isinstance(item, ast.Call) and isinstance(item.func, ast.Name)
                             and item.func.id == "select_keyboard_candidate" for item in ast.walk(node)))

    def test_fresh_snapshots_support_private_and_unfocused_boundaries_without_frame_shortcuts(self):
        fresh = self.nested("fresh")
        self.predicate(fresh, 'lambda: probe.latest is not None and watch.latest == probe.latest')
        self.has(fresh, 'return probe.latest')
        self.has(fresh, 'watchers.remove(probe)')
        self.has(fresh, 'probe.sock.close()')
        self.assertFalse(any(isinstance(item, ast.Assert) for item in ast.walk(fresh)),
                         "Fresh must also observe private/unfocused full frames")
        unchanged = self.nested("unchanged")
        self.has(unchanged, 'before = fresh()')
        self.has(unchanged, 'commits = (peer.commits.copy(), other.commits.copy())')
        self.has(unchanged, 'assert peer.process_key_event(key, code, mask) == handled')
        self.has(unchanged, 'assert fresh() == before, "custom boundary inert event changed the whole frame"')
        self.has(unchanged, 'assert (peer.commits, other.commits) == commits')

    def test_real_fsync_barrier_busy_lease_and_exact_settled_frame_after_cancellation(self):
        node, _outer, _profile, _language, _case, owner = self.owners()
        self.has(node, 'armed, entered, released, finished, failed = [runtime / ("control-io." + name) for name in ["arm", "entered", "release", "finished", "fail"]]')
        self.sequence(owner.body, ['before = fresh()', 'armed.touch()',
                                   'assert peer.process_key_event(IBus.KEY_L, 46, ctrl | shift)',
                                   'wait(entered.exists, "custom boundary real durable-write barrier")',
                                   'assert workers() == 1 and config.read_bytes() == before_disk',
                                   'assert json.loads(command("S", timeout=.25))["settings"]["language"] == original',
                                   'busy = json.loads(command("P0", timeout=.25))',
                                   'assert not busy["ok"] and "正在保存" in busy["error"], "custom boundary released worker permit early"',
                                   'assert workers() == 1 and config.read_bytes() == before_disk',
                                   'settled = fresh()', 'released.touch()',
                                   'assert json.loads(command("S"))["settings"]["language"] == original',
                                   'assert config.read_bytes() == before_disk',
                                   'assert fresh() == settled, "canceled custom language task published a late frame"'])
        self.predicate(owner, 'lambda: finished.exists() and workers() == 0 and not list(runtime.glob(".suzaku-*.tmp"))')
        self.has(owner, 'assert watch.latest["seed"] == expected and watch.latest["language"] == original')
        self.assertFalse(any(isinstance(item, ast.Call) and isinstance(item.func, ast.Attribute)
                             and isinstance(item.func.value, ast.Name) and item.func.value.id == "failed"
                             and item.func.attr == "touch" for item in ast.walk(node)), "These are cancellations, not forced EIO")
        worker = self.nested("workers")
        self.has(worker, 'count += entry.read_text().strip() == "suzaku-settings"')
        self.assertTrue(any(isinstance(item, ast.ExceptHandler) and isinstance(item.type, ast.Name)
                            and item.type.id == "FileNotFoundError" for item in ast.walk(worker)))

    def test_focus_transfer_and_private_purposes_retire_context_and_clear_both_key_identities(self):
        _node, _outer, _profile, _language, _case, owner = self.owners()
        focus = self.branch(owner, 'case == "focus-out"')
        self.has(focus, 'peer.event("FocusOut")')
        self.has(focus, 'peer.event("FocusIn")')
        self.predicate(focus, 'lambda: watch.latest["context"] > before["context"] and not watch.latest["focused"] and not watch.latest["seed"]')
        transfer = self.branch(focus, 'case == "focus-transfer"')
        self.sequence(transfer.body, ['other.event("FocusIn")', 'owner = other'])
        self.has(owner, 'purpose = {"password": IBus.InputPurpose.PASSWORD, "pin": IBus.InputPurpose.PIN, "hidden": IBus.InputPurpose.FREE_FORM}[case]')
        self.has(owner, 'peer.set_content_type(purpose, (1 << 11) if case == "hidden" else 0)')
        self.predicate(owner, 'lambda: watch.latest["context"] > before["context"] and watch.latest["private"] and not watch.latest["seed"] and not watch.latest["candidates"]')
        hint = self.branch(owner, 'case == "hidden"')
        self.has(hint, 'assert peer.process_key_event(IBus.KEY_a, 38), "private hint still supports local typing"')
        self.has(hint, 'assert fresh()["private"] and not watch.latest["seed"] and not watch.latest["candidates"]')
        self.has(hint, 'unchanged(peer, other, [(IBus.KEY_a, 38, 0, False), (IBus.KEY_1, 10, 0, False), (IBus.KEY_Return, 36, 0, False)])')
        self.has(owner, 'assert not action(before, "Tstale private long draft")')
        self.has(owner, 'assert not action(before, "Tstale field long draft")')
        self.has(owner, 'assert action(fresh(), "T" + replacement)')
        self.has(owner, 'assert owner.process_key_event(IBus.KEY_a, 38)')
        self.has(owner, 'assert owner.process_key_event(IBus.KEY_l, 46)')
        self.has(owner, 'expected = replacement + "al"')
        stale = only((item for item in ast.walk(owner) if isinstance(item, ast.If)
                      and same_ast(item.test, ast.parse('case == "focus-transfer"', mode="eval").body)
                      and any(same_ast(child, statement('snapshot = fresh()')) for child in item.body)), "stale engine transfer branch")
        self.sequence(stale.body, ['snapshot = fresh()', 'peer.event("Reset")', 'peer.event("FocusOut")',
                                  'peer.set_content_type(IBus.InputPurpose.PASSWORD)',
                                  'unchanged(peer, other, [(IBus.KEY_L, 46, ctrl | shift, False), (IBus.KEY_Return, 36, 0, False), (IBus.KEY_a, 38, 0, False)])',
                                  'assert fresh() == snapshot, "stale old engine changed the new field"'])

    def test_pending_Compose_keeps_context_falls_through_chords_and_preserves_complete_raw_undo(self):
        _node, _outer, _profile, _language, _case, owner = self.owners()
        compose = self.branch(owner, 'case in ["compose-pending", "compose-completed"]')
        self.has(compose, 'assert peer.process_key_event(IBus.KEY_dead_acute)')
        self.has(compose, 'assert fresh()["context"] == before["context"]')
        self.predicate(compose, 'lambda: "Compose…" in feedback["text"] and watch.latest["seed"] == adopted')
        pending = self.branch(compose, 'case == "compose-pending"')
        self.has(pending, 'unchanged(peer, other, [(IBus.KEY_a, 38, release, False), (IBus.KEY_l, 46, release, False), (IBus.KEY_Control_L, 37, ctrl, False), (IBus.KEY_Shift_L, 50, ctrl | shift, False), (IBus.KEY_L, 46, ctrl | shift, False), (IBus.KEY_A, 38, alt | shift, False), (IBus.KEY_Shift_L, 50, release, False), (IBus.KEY_Control_L, 37, release, False)])')
        undo = only((item for item in owner.body if isinstance(item, ast.If)
                     and same_ast(item.test, ast.parse('case == "compose-pending"', mode="eval").body)), "post-worker pending Compose undo")
        self.sequence(undo.body, ['assert peer.process_key_event(IBus.KEY_BackSpace, 14)',
                                 'wait(lambda: "Compose…" not in feedback["text"] and watch.latest["seed"] == adopted, "custom boundary first Backspace cancels only pending Compose")',
                                 'assert peer.process_key_event(IBus.KEY_BackSpace, 14)', 'expected = raw',
                                 'wait(lambda: watch.latest["seed"] == expected, "custom boundary second Backspace restores entire raw undo")'])

    def test_completed_Compose_appends_exact_accent_then_scalar_deletes_never_revive_raw_undo(self):
        _node, _outer, _profile, _language, _case, owner = self.owners()
        compose = self.branch(owner, 'case in ["compose-pending", "compose-completed"]')
        pending = self.branch(compose, 'case == "compose-pending"')
        self.sequence(pending.orelse, ['assert peer.process_key_event(IBus.KEY_e, 18)', 'expected = adopted + "é"',
                                      'wait(lambda: watch.latest["seed"] == expected, "custom boundary complete Unicode accent")'])
        undo = only((item for item in owner.body if isinstance(item, ast.If)
                     and same_ast(item.test, ast.parse('case == "compose-pending"', mode="eval").body)), "post-worker Compose undo")
        completed = self.branch(undo, 'case == "compose-completed"')
        self.sequence(completed.body, ['assert peer.process_key_event(IBus.KEY_BackSpace, 14)',
                                       'wait(lambda: watch.latest["seed"] == adopted, "custom boundary deletes only completed accent")',
                                       'assert peer.process_key_event(IBus.KEY_BackSpace, 14)', 'expected = adopted[:-1]',
                                       'wait(lambda: watch.latest["seed"] == expected, "custom boundary completed edit retires full raw undo")',
                                       'unchanged(peer, other, [(IBus.KEY_a, 38, release, False), (IBus.KEY_l, 46, release, False)])'])

    def test_owned_exact_commit_no_duplicate_cleanup_and_full_settings_restore_precede_PASS(self):
        node, outer, _profile, _language, _case, owner = self.owners()
        self.sequence(owner.body, ['assert owner.process_key_event(IBus.KEY_space)', 'expected += " "',
                                   'select(owner, expected)', 'assert not peer.commits and not other.commits',
                                   'assert owner.process_key_event(IBus.KEY_Return, 36)',
                                   'assert (peer.commits, other.commits) == (([], [expected]) if owner is other else ([expected], []))',
                                   'unchanged(owner, peer if owner is other else other, [(IBus.KEY_Return, 36, release, False), (IBus.KEY_Return, 36, 0, False)])'])
        self.predicate(owner, 'lambda: owner.commits == [expected] and not watch.latest["seed"]')
        cleanup = only((item for item in owner.finalbody if isinstance(item, ast.With)), "owned worker cleanup")
        self.has(cleanup, 'released.touch(exist_ok=True)')
        self.has(cleanup, 'peer.close()')
        self.has(cleanup, 'other.close()')
        self.has(cleanup, 'peer.connection.signal_unsubscribe(subscription)')
        self.predicate(cleanup, 'lambda: finished.exists() and workers() == 0 and not list(runtime.glob(".suzaku-*.tmp"))')
        restore = only((item for item in outer.finalbody if isinstance(item, ast.With)
                        and any(same_ast(child, statement('assert json.loads(command("S"))["settings"] == saved'))
                                for child in ast.walk(item))), "full settings restore")
        self.sequence(restore.body, ['assert json.loads(command("L" + saved["language"]))["ok"]',
                                    'assert json.loads(command("U" + json.dumps({"llm_enabled": saved["llm_enabled"], "shortcut_profile": saved["shortcut_profile"], "shortcut_bindings": saved["shortcut_bindings"]})))["ok"]',
                                    'assert json.loads(command("S"))["settings"] == saved',
                                    'assert json.loads(config.read_bytes()) == saved'])
        self.has(outer.finalbody[0], 'watchers.remove(watch)')
        self.has(outer.finalbody[0], 'watch.sock.close()')
        for cleanup in owner.finalbody + outer.finalbody:
            self.assertTrue(any(isinstance(item, ast.Call) and isinstance(item.func, ast.Name)
                                and item.func.id == "preserve_primary_failure" for item in ast.walk(cleanup)))
        self.assertLess(node.body.index(outer), next(index for index, item in enumerate(node.body)
                        if same_ast(item, statement('assert checked == 28, checked'))))
        self.assertIsInstance(node.body[-1], ast.Expr)
        self.assertEqual(ast.unparse(node.body[-1].value.func), "print")

    def test_original_wait_deadlines_and_fast_input_gestures_are_not_relaxed(self):
        node = self.helper()
        for call in (item for item in ast.walk(node) if isinstance(item, ast.Call)):
            if isinstance(call.func, ast.Name) and call.func.id == "wait":
                self.assertEqual(len(call.args), 2)
                self.assertFalse(call.keywords)
            if isinstance(call.func, ast.Name) and call.func.id == "command" and call.keywords:
                self.assertEqual([(key.arg, ast.literal_eval(key.value)) for key in call.keywords], [("timeout", .25)])
            if isinstance(call.func, ast.Attribute) and call.func.attr == "sleep":
                self.fail("No new sleep may hide boundary acknowledgement races")
        self.has(node, 'assert time.monotonic() - started < .25, "custom boundary language gesture blocked input"')
        self.has(node, 'assert time.monotonic() - started_edit < .25, "custom boundary canceled save blocked new typing"')


if __name__ == "__main__":
    unittest.main()
