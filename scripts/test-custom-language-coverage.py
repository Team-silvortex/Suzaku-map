#!/usr/bin/env python3
"""Source-only guards for appended custom-language long-draft workflows.

Never imports the native suite or starts a bus, display, host or model. These
checks preserve historical fixture coverage; they are not native acceptance.
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
NATIVE_BEFORE_SHA256 = "b101512a7dadccfa924ee7a7835d21be684f3904d2b8f9237dffebc8bf0f3dc8"
GUARD_BEFORE_SHA256 = {
    "scripts/test-held-language-coverage.py":
        "29527c50a30f058d98060934ea2edf643cad3413e86c99796e531c5835b4aedd",
    "scripts/test-native-model-fixture.py":
        "d500485ce92f8ada62039d93246a591faa838aaf22773f1285ba322671b4a16d",
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
APPENDED_CALLS = {
    "check_custom_language_long_workflows": ("check_held_language_transitions", 2),
    "check_custom_language_pending_long": ("check_held_language_pending", 1),
}
BOUNDARY_APPENDED_CALLS = {
    "check_custom_language_pending_boundaries": ("check_custom_language_pending_long", 1),
}
CONTINUATION_APPENDED_CALLS = {
    "check_custom_language_pending_continuation": ("check_custom_language_pending_boundaries", 1),
}
LANGUAGE_CASES = '''[("en", "zh-Hans", "Earlier note. " * 30, "hel", "hello"),
                     ("zh-Hans", "en", "前文。" * 100, "hui yi", "会议")]'''
BINDINGS = {"previous": "Ctrl+P", "next": "Ctrl+N", "previous-page": "Alt+Shift+B",
            "next-page": "Ctrl+Shift+F", "adopt": "Alt+Shift+A", "switch-language": "Ctrl+Shift+L"}
HELD_MAP_ADDITION = '''CUSTOM_APPENDED_CALLS = {
    "check_custom_language_long_workflows": ("check_held_language_transitions", 2),
    "check_custom_language_pending_long": ("check_held_language_pending", 1),
}
'''
HELD_LOOP_BEFORE = "    for name, (predecessor, count) in APPENDED_CALLS.items():\n"
HELD_LOOP_AFTER = "    for name, (predecessor, count) in (APPENDED_CALLS | CUSTOM_APPENDED_CALLS).items():\n"
MODEL_MAP_BEFORE = '                    "check_held_language_pending": "check_language_shortcut_slow_io"}\n'
MODEL_MAP_AFTER = '''                    "check_held_language_pending": "check_language_shortcut_slow_io",
                    "check_custom_language_long_workflows": "check_held_language_transitions",
                    "check_custom_language_pending_long": "check_held_language_pending"}
'''
MODEL_SEEN_ADDITION = '''        custom_seen = {name: seen.pop(name) for name in ["check_custom_language_long_workflows",
                                                       "check_custom_language_pending_long"]}
        self.assertEqual(custom_seen, {"check_custom_language_long_workflows": 2,
                                       "check_custom_language_pending_long": 1})
'''
HELD_BOUNDARY_ADDITION = '''BOUNDARY_APPENDED_CALLS = {
    "check_custom_language_pending_boundaries": ("check_custom_language_pending_long", 1),
}
'''
HELD_BOUNDARY_LOOP = "    for name, (predecessor, count) in (APPENDED_CALLS | CUSTOM_APPENDED_CALLS | BOUNDARY_APPENDED_CALLS).items():\n"
MODEL_BOUNDARY_MAP_ADDITION = '        appended |= {"check_custom_language_pending_boundaries": "check_custom_language_pending_long"}\n'
MODEL_BOUNDARY_SEEN_ADDITION = '        self.assertEqual(seen.pop("check_custom_language_pending_boundaries"), 1)\n'
HELD_CONTINUATION_ADDITION = '''CONTINUATION_APPENDED_CALLS = {
    "check_custom_language_pending_continuation": ("check_custom_language_pending_boundaries", 1),
}
'''
HELD_CONTINUATION_LOOP = "    for name, (predecessor, count) in (APPENDED_CALLS | CUSTOM_APPENDED_CALLS | BOUNDARY_APPENDED_CALLS | CONTINUATION_APPENDED_CALLS).items():\n"
MODEL_CONTINUATION_MAP_ADDITION = '        appended |= {"check_custom_language_pending_continuation": "check_custom_language_pending_boundaries"}\n'
MODEL_CONTINUATION_SEEN_ADDITION = '        self.assertEqual(seen.pop("check_custom_language_pending_continuation"), 1)\n'


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
    return (isinstance(node, ast.Expr) and isinstance(node.value, ast.Call)
            and isinstance(node.value.func, ast.Name) and node.value.func.id == name
            and len(node.value.args) == 1 and isinstance(node.value.args[0], ast.Name)
            and node.value.args[0].id == "bus" and not node.value.keywords)


def project_native(source):
    """Strip only two new definitions and three exact adjacent bus calls."""
    tree = ast.parse(source)
    lines, omitted = source.splitlines(keepends=True), set()
    for name, (predecessor, count) in (APPENDED_CALLS | BOUNDARY_APPENDED_CALLS | CONTINUATION_APPENDED_CALLS).items():
        helper = definition(tree, name)
        if helper.decorator_list or lines[helper.end_lineno:helper.end_lineno + 2] != ["\n", "\n"]:
            raise AssertionError("New helper must be undecorated with two following blank lines")
        omitted.update(range(helper.lineno - 1, helper.end_lineno + 2))
        calls = [node for node in ast.walk(tree) if isinstance(node, ast.Expr)
                 and isinstance(node.value, ast.Call) and isinstance(node.value.func, ast.Name)
                 and node.value.func.id == name]
        if len(calls) != count:
            raise AssertionError(f"Expected exactly {count} {name} calls")
        for call in calls:
            if not exact_call(call, name) or call.lineno != call.end_lineno:
                raise AssertionError("Only an exact single-line bus call may be removed")
            indent = lines[call.lineno - 1].split(name + "(", 1)[0]
            if lines[call.lineno - 1] != indent + name + "(bus)\n":
                raise AssertionError("The call line must contain no extra source")
            if lines[call.lineno - 2] != indent + predecessor + "(bus)\n":
                raise AssertionError("The new call must follow its exact held-language predecessor")
            omitted.add(call.lineno - 1)
    return "".join(line for index, line in enumerate(lines) if index not in omitted)


def replace_once(source, before, after):
    if source.count(before) != 1:
        raise AssertionError("Guard projection requires exactly one fixed source block")
    return source.replace(before, after, 1)


def project_guard(path, source):
    if path == "scripts/test-held-language-coverage.py":
        source = replace_once(source, HELD_CONTINUATION_ADDITION, "")
        source = replace_once(source, HELD_CONTINUATION_LOOP, HELD_BOUNDARY_LOOP)
        source = replace_once(source, HELD_BOUNDARY_ADDITION, "")
        source = replace_once(source, HELD_BOUNDARY_LOOP, HELD_LOOP_AFTER)
        source = replace_once(source, HELD_MAP_ADDITION, "")
        return replace_once(source, HELD_LOOP_AFTER, HELD_LOOP_BEFORE)
    if path == "scripts/test-native-model-fixture.py":
        source = replace_once(source, MODEL_CONTINUATION_MAP_ADDITION, "")
        source = replace_once(source, MODEL_CONTINUATION_SEEN_ADDITION, "")
        source = replace_once(source, MODEL_BOUNDARY_MAP_ADDITION, "")
        source = replace_once(source, MODEL_BOUNDARY_SEEN_ADDITION, "")
        source = replace_once(source, MODEL_MAP_AFTER, MODEL_MAP_BEFORE)
        return replace_once(source, MODEL_SEEN_ADDITION, "")
    raise AssertionError("Unknown historical guard")


class CustomLanguageCoverageTests(unittest.TestCase):
    def helper(self, name):
        return definition(TREE, name)

    def nested(self, node, name):
        return only((item for item in node.body if isinstance(item, ast.FunctionDef)
                     and item.name == name), name)

    def assigned(self, node, name):
        assignment = only((item for item in node.body if isinstance(item, ast.Assign)
                           and len(item.targets) == 1 and isinstance(item.targets[0], ast.Name)
                           and item.targets[0].id == name), f"assignment {name}")
        return assignment.value

    def assert_has(self, node, text):
        expected = statement(text)
        self.assertTrue(any(same_ast(item, expected) for item in ast.walk(node)), text)

    def assert_predicate(self, node, text):
        predicate = ast.parse(text, mode="eval").body
        self.assertTrue(any(same_ast(item, predicate) for item in ast.walk(node)), text)

    def assert_sequence(self, body, texts):
        position = 0
        for text in texts:
            candidates = [index for index in range(position, len(body))
                          if same_ast(body[index], statement(text))]
            self.assertTrue(candidates, f"Missing or out-of-order statement: {text}")
            position = candidates[0] + 1

    def profile_and_case(self, name):
        node = self.helper(name)
        owner = only((item for item in node.body if isinstance(item, ast.Try)), "outer try")
        profile = only((item for item in owner.body if isinstance(item, ast.For)), "profile loop")
        self.assertEqual(ast.unparse(profile.target), "profile")
        self.assertEqual(ast.literal_eval(profile.iter), ["standard", "home-row"])
        language = only((item for item in profile.body if isinstance(item, ast.For)), "language loop")
        self.assertTrue(same_ast(language.target, statement("original, target, prefix, tail, word = None").targets[0]))
        if name.endswith("pending_long"):
            case = only((item for item in language.body if isinstance(item, ast.For)), "case loop")
            self.assertEqual((ast.unparse(case.target), ast.unparse(case.iter)), ("case", "cases"))
            return node, profile, only((item for item in case.body if isinstance(item, ast.Try)), "case try")
        return node, profile, only((item for item in language.body if isinstance(item, ast.Try)), "case try")

    def test_native_syntax_without_execution(self):
        compile(TREE, str(NATIVE_PATH), "exec")

    def test_whole_native_fixture_is_only_two_helpers_and_three_adjacent_calls(self):
        projected = project_native(SOURCE)
        self.assertEqual(hashlib.sha256(projected.encode()).hexdigest(), NATIVE_BEFORE_SHA256)

    def test_historical_guards_only_add_exact_custom_projection_support(self):
        for path, expected in GUARD_BEFORE_SHA256.items():
            with self.subTest(path=path):
                source = historical_source(ROOT / path)
                compile(ast.parse(source), path, "exec")
                self.assertEqual(hashlib.sha256(project_guard(path, source).encode()).hexdigest(), expected)

    def test_runner_workers_deadlines_and_bridge_are_unchanged(self):
        for path, expected in UNCHANGED_FILE_SHA256.items():
            with self.subTest(path=path):
                self.assertEqual(hashlib.sha256(historical_source(ROOT / path).encode()).hexdigest(), expected)

    def test_native_projection_rejects_changed_arguments_moved_duplicate_or_legacy_edits(self):
        for name in APPENDED_CALLS:
            lines = SOURCE.splitlines(keepends=True)
            calls = [node for node in ast.walk(TREE) if exact_call(node, name)
                     and node.lineno == node.end_lineno]
            self.assertEqual(len(calls), APPENDED_CALLS[name][1])
            call = calls[0]
            original = lines[call.lineno - 1]
            indent = original[:len(original) - len(original.lstrip())]
            for changed in [f"{name}(other)", f"{name}(bus, bus)", f"{name}(bus=bus)"]:
                with self.subTest(changed=changed):
                    modified = list(lines)
                    modified[call.lineno - 1] = indent + changed + "\n"
                    with self.assertRaises(AssertionError):
                        project_native("".join(modified))
            moved = list(lines)
            moved[call.lineno - 2:call.lineno] = reversed(moved[call.lineno - 2:call.lineno])
            with self.assertRaises(AssertionError):
                project_native("".join(moved))
            duplicated = list(lines)
            duplicated.insert(call.lineno, original)
            with self.assertRaises(AssertionError):
                project_native("".join(duplicated))
        changed = SOURCE.replace("time.sleep(0.01)", "time.sleep(0.02)", 1)
        self.assertNotEqual(hashlib.sha256(project_native(changed).encode()).hexdigest(), NATIVE_BEFORE_SHA256)

    def test_historical_projection_cannot_hide_assertion_or_timeout_changes(self):
        for path, digest in GUARD_BEFORE_SHA256.items():
            with self.subTest(path=path):
                source = historical_source(ROOT / path)
                changed = source + "\n# unrelated guard mutation\n"
                self.assertNotEqual(hashlib.sha256(project_guard(path, changed).encode()).hexdigest(), digest)
        held = historical_source(ROOT / "scripts/test-held-language-coverage.py")
        with self.assertRaises(AssertionError):
            project_guard("scripts/test-held-language-coverage.py", held.replace(
                HELD_MAP_ADDITION, HELD_MAP_ADDITION.replace('("check_held_language_pending", 1)',
                                                           '("check_held_language_pending", 2)'), 1))

    def test_fixed_long_vectors_two_profiles_and_exact_four_or_sixteen_cases(self):
        expected = ast.parse(LANGUAGE_CASES, mode="eval").body
        for name in APPENDED_CALLS:
            node, _profile, _case = self.profile_and_case(name)
            pending = name.endswith("pending_long")
            self.assertTrue(same_ast(self.assigned(node, "languages" if pending else "cases"), expected))
            if pending:
                self.assertEqual(ast.literal_eval(self.assigned(node, "cases")),
                                 ["success", "failure", "cancel-reset", "cancel-commit"])
            self.assert_has(node, f"assert checked == {16 if pending else 4}, checked")
            self.assertEqual(sum(same_ast(item, statement("checked += 1")) for item in ast.walk(node)), 1)
            self.assert_has(node, 'assert len(raw) > 256 and raw != adopted[:-1], "success and failure undo oracles must differ"')
        for prefix, tail, word in [("Earlier note. " * 30, "hel", "hello"),
                                   ("前文。" * 100, "hui yi", "会议")]:
            self.assertGreater(len(prefix + tail), 256)
            self.assertNotEqual(prefix + tail, (prefix + word)[:-1])

    def test_full_six_action_U_uses_actual_protocol_and_private_original_language(self):
        for name in APPENDED_CALLS:
            node, profile, owner = self.profile_and_case(name)
            self.assertEqual(ast.literal_eval(self.assigned(node, "bindings")), BINDINGS)
            self.assert_has(node, 'assert json.loads(command("P0"))["ok"]')
            self.assert_has(profile, 'assert json.loads(command("U" + json.dumps({"shortcut_profile": profile, "shortcut_bindings": bindings})))["ok"]')
            self.assert_has(node, 'assert json.loads(command("L" + original))["ok"]')
            self.assert_has(owner, 'assert peer.process_key_event(IBus.KEY_A, 38, alt | shift)')
            self.assert_has(owner, 'assert peer.process_key_event(IBus.KEY_L, 46, ctrl | shift)')
            self.assert_has(owner, 'adopted = prefix + word')
            selection = self.nested(node, "select")
            self.assert_has(selection, 'assert peer.process_key_event(IBus.KEY_n if forward else IBus.KEY_p, 0, ctrl)')
            self.assert_predicate(selection, 'row["text"] == text and row["source"] == "local"' +
                                  (' and (kind is None or row["kind"] == kind)' if not name.endswith("pending_long") else ''))
            self.assert_has(selection, 'assert watch.latest["seed"] == before["seed"] and watch.latest["candidates"] == rows')
            self.assertFalse(any(isinstance(item, ast.Call) and isinstance(item.func, ast.Name)
                                 and item.func.id == "select_keyboard_candidate" for item in ast.walk(node)))

    def test_fresh_complete_snapshots_reset_ack_and_drained_real_tail_keys(self):
        for name in APPENDED_CALLS:
            node, _profile, owner = self.profile_and_case(name)
            fresh = self.nested(node, "fresh")
            self.assert_predicate(fresh, "lambda: probe.latest is not None and watch.latest == probe.latest")
            self.assert_has(fresh, 'assert probe.latest["focused"] and not probe.latest["private"]')
            unchanged = self.nested(node, "unchanged")
            self.assert_has(unchanged, 'assert fresh() == before, ' + repr(
                "pending long custom inert event changed the full snapshot" if name.endswith("pending_long")
                else "long custom language inert event changed the full snapshot"))
            self.assert_has(unchanged, 'assert peer.commits == commits')
            self.assert_sequence(owner.body, ['old_context = fresh()["context"]', 'peer.event("Reset")',
                                              'assert action(fresh(), "T" + prefix)', 'raw = prefix'])
            self.assert_predicate(owner, 'lambda: watch.latest["context"] > old_context and not watch.latest["seed"]')
            typing = only((item for item in owner.body if isinstance(item, ast.For)
                           and ast.unparse(item.target) == "character"), "drained tail loop")
            self.assertEqual(ast.unparse(typing.iter), "tail")
            self.assert_sequence(typing.body, ['assert peer.process_key_event(IBus.unicode_to_keyval(character))',
                                               'raw += character'])
            self.assert_predicate(typing, 'lambda: watch.latest["seed"] == raw')

    def test_normal_success_retires_undo_holds_both_codes_reuses_chord_and_commits_once(self):
        node, _profile, owner = self.profile_and_case("check_custom_language_long_workflows")
        self.assert_has(owner, 'select(peer, adopted, "word")')
        self.assert_predicate(owner, 'lambda: json.loads(command("S"))["settings"]["language"] == target and watch.latest["language"] == target and watch.latest["context"] > before["context"] and watch.latest["seed"] == adopted')
        self.assert_has(owner, 'assert json.loads(config.read_bytes())["language"] == target')
        self.assert_has(owner, 'assert not action(before, "Tstale long custom draft")')
        self.assert_has(owner, 'unchanged(peer, [(IBus.KEY_P, 0, ctrl, True), (IBus.KEY_B, 0, alt | shift, True), (IBus.KEY_a, 38, 0, True), (IBus.KEY_Return, 38, 0, True), (IBus.KEY_l, 46, 0, True), (IBus.KEY_L, 46, ctrl | shift, True), (IBus.KEY_a, 39, release, False)])')
        self.assert_sequence(owner.body, ['assert peer.process_key_event(IBus.KEY_BackSpace, 14)',
                                          'spelling = adopted[:-1]',
                                          'unchanged(peer, [(IBus.KEY_a, 38, 0, True), (IBus.KEY_l, 46, 0, True), (IBus.KEY_a, 38, release, False), (IBus.KEY_l, 46, release, False)])',
                                          'assert peer.process_key_event(IBus.KEY_7, 8, alt)',
                                          'assert peer.process_key_event(IBus.KEY_l, 46, ctrl | shift)',
                                          'assert not peer.process_key_event(IBus.KEY_l, 46, release)',
                                          'assert peer.process_key_event(IBus.KEY_space)',
                                          'expected = spelling + "7 "', 'select(peer, expected)',
                                          'assert not peer.commits', 'assert peer.process_key_event(IBus.KEY_Return, 36)'])
        self.assert_predicate(owner, 'lambda: peer.commits == [expected] and not watch.latest["seed"]')
        self.assert_has(owner, 'unchanged(peer, [(IBus.KEY_Return, 36, release, False), (IBus.KEY_Return, 36, 0, False)])')

    def test_real_fsync_barrier_busy_lease_and_unambiguous_success_failure_undo(self):
        node, _profile, owner = self.profile_and_case("check_custom_language_pending_long")
        self.assert_has(node, 'armed, entered, released, finished, failed = [runtime / ("control-io." + name) for name in ["arm", "entered", "release", "finished", "fail"]]')
        self.assert_has(owner, 'assert any(row["text"] == adopted and row["kind"] == "word" and row["source"] == "local" for row in fresh()["candidates"])')
        self.assert_sequence(owner.body, ['assert peer.process_key_event(IBus.KEY_A, 38, alt | shift)',
                                          'before = fresh()', 'armed.touch()',
                                          'assert peer.process_key_event(IBus.KEY_L, 46, ctrl | shift)',
                                          'wait(entered.exists, "pending long custom actual durable-write barrier")',
                                          'assert workers() == 1',
                                          'assert json.loads(command("S", timeout=.25))["settings"]["language"] == original',
                                          'assert config.read_bytes() == before_disk and not peer.commits',
                                          'busy = json.loads(command("P0", timeout=.25))',
                                          'assert not busy["ok"] and "正在保存" in busy["error"], "pending custom job released writer permit early"',
                                          'assert workers() == 1 and config.read_bytes() == before_disk',
                                          'if case == "failure":\n    failed.touch()', 'released.touch()'])
        self.assert_predicate(owner, 'lambda: finished.exists() and workers() == 0 and not list(runtime.glob(".suzaku-*.tmp"))')
        self.assert_has(owner, 'assert config.read_bytes() == before_disk and fresh()["context"] == before["context"]')
        self.assert_has(owner, 'expected = adopted[:-1] if case == "success" else raw')
        self.assert_has(owner, 'assert peer.process_key_event(IBus.KEY_BackSpace, 14)')
        self.assert_predicate(owner, 'lambda: watch.latest["seed"] == expected')

    def test_cancel_reset_or_commit_precedes_new_owned_draft_real_typing_and_worker_release(self):
        _node, _profile, owner = self.profile_and_case("check_custom_language_pending_long")
        cancel = only((item for item in owner.body if isinstance(item, ast.If)
                       and ast.unparse(item.test) == "case in ['cancel-reset', 'cancel-commit']"), "cancel branch")
        boundary = cancel.body[0]
        self.assertIsInstance(boundary, ast.If)
        self.assertEqual(ast.unparse(boundary.test), "case == 'cancel-reset'")
        self.assertEqual([ast.unparse(item) for item in boundary.body], ["peer.event('Reset')"])
        self.assertEqual([ast.unparse(item) for item in boundary.orelse],
                         ["select(peer, adopted)", "assert peer.process_key_event(IBus.KEY_Return, 36)", "expected_commits = [adopted]"])
        self.assert_predicate(cancel, 'lambda: watch.latest["context"] > before["context"] and not watch.latest["seed"] and peer.commits == expected_commits')
        self.assert_sequence(cancel.body, ['assert not action(before, "Tstale canceled long custom draft")',
                                           'assert not peer.process_key_event(IBus.KEY_a, 38, release)',
                                           'assert not peer.process_key_event(IBus.KEY_l, 46, release)',
                                           'replacement = prefix + ("after reset " if case == "cancel-reset" else "after commit ")',
                                           'assert action(fresh(), "T" + replacement)',
                                           'assert peer.process_key_event(IBus.KEY_x, 53)',
                                           'expected = replacement + "x"'])
        self.assert_has(owner, 'assert config.read_bytes() == before_disk')
        self.assert_has(owner, 'assert fresh()["language"] == original and watch.latest["seed"] == expected')
        self.assert_sequence(owner.body, ['assert peer.commits == expected_commits',
                                          'assert peer.process_key_event(IBus.KEY_space)', 'expected += " "',
                                          'select(peer, expected)', 'assert peer.commits == expected_commits',
                                          'assert peer.process_key_event(IBus.KEY_Return, 36)'])
        self.assert_predicate(owner, 'lambda: peer.commits == expected_commits + [expected] and not watch.latest["seed"]')

    def test_preserved_deadlines_worker_cleanup_full_settings_restore_and_post_cleanup_PASS(self):
        for name in APPENDED_CALLS:
            node, _profile, owner = self.profile_and_case(name)
            for call in (item for item in ast.walk(node) if isinstance(item, ast.Call)):
                if isinstance(call.func, ast.Name) and call.func.id == "wait":
                    self.assertEqual(len(call.args), 2)
                    self.assertFalse(call.keywords)
                if isinstance(call.func, ast.Attribute) and call.func.attr == "sleep":
                    self.fail("No new sleep may hide deadline or acknowledgement races")
                if isinstance(call.func, ast.Name) and call.func.id == "command" and call.keywords:
                    self.assertEqual([(key.arg, ast.literal_eval(key.value)) for key in call.keywords], [("timeout", .25)])
            outer = only((item for item in node.body if isinstance(item, ast.Try)), "outer try")
            self.assert_has(owner, 'assert time.monotonic() - started < .25, ' + repr(
                "pending long custom switch blocked input" if name.endswith("pending_long")
                else "long custom language chord blocked input"))
            restore = only((item for item in outer.finalbody if isinstance(item, ast.With)
                            and any(same_ast(child, statement('assert json.loads(command("S"))["settings"] == saved'))
                                    for child in ast.walk(item))), "full restoration block")
            self.assert_sequence(restore.body, ['assert json.loads(command("L" + saved["language"]))["ok"]',
                                                'assert json.loads(command("U" + json.dumps({"llm_enabled": saved["llm_enabled"], "shortcut_profile": saved["shortcut_profile"], "shortcut_bindings": saved["shortcut_bindings"]})))["ok"]',
                                                'assert json.loads(command("S"))["settings"] == saved',
                                                'assert json.loads(config.read_bytes()) == saved'])
            self.assertTrue(any(isinstance(item, ast.With) and any(isinstance(call, ast.Call)
                                and isinstance(call.func, ast.Name) and call.func.id == "preserve_primary_failure"
                                for call in ast.walk(item)) for item in owner.finalbody))
            if name.endswith("pending_long"):
                self.assert_has(owner, 'assert time.monotonic() - started_edit < .25, "canceled worker blocked continued native input"')
                self.assert_has(node, 'released.touch(exist_ok=True)')
                self.assert_has(node, 'peer.connection.signal_unsubscribe(subscription)')
                self.assert_predicate(owner.finalbody[0], 'lambda: finished.exists() and workers() == 0 and not list(runtime.glob(".suzaku-*.tmp"))')
            self.assertLess(node.body.index(outer), next(index for index, item in enumerate(node.body)
                            if isinstance(item, ast.Assert) and ast.unparse(item.test).startswith("checked ==")))
            self.assertIsInstance(node.body[-1], ast.Expr)
            self.assertEqual(ast.unparse(node.body[-1].value.func), "print")


if __name__ == "__main__":
    unittest.main()
