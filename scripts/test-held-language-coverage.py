#!/usr/bin/env python3
"""Source-only guards for the appended held-key/language transition probes.

These checks never import or run the native suite. Passing them is structural
evidence, not private IBus, physical-keyboard, installation or desktop acceptance.
"""
import ast
import hashlib
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parent.parent
NATIVE_PATH = ROOT / "scripts/test-native-sync.py"
SOURCE = NATIVE_PATH.read_text()
TREE = ast.parse(SOURCE)

# Independently frozen before either new held-language helper was inserted.
NATIVE_BEFORE_HASH = "4407f379a4e703f794bebc3a138eace98cdc2effe087e3386d3f20d3168ed447"
UNCHANGED_FILE_HASHES = {
    "scripts/test-linux-ci.sh":
        "521c5da0c7cc6a586e64fdb7fd57312a1d534363a3b9ccce376fbc57b748adbb",
    "src/linux/ibus_language.inc.c":
        "0b40a1a36294ac235674b03941572ec3fbd356f1f3a7288720c16c25d2a7f139",
}
BRIDGE_BEFORE_HASH = "37b05a781f60f9a208d6b7c75cb0da1ded3009f54cf226454543627b6d669c67"
FEEDBACK_CLEAR_BEFORE = (
    "    if (self->language_feedback != NULL && self->language_job == NULL) {\n"
    "        g_clear_pointer(&self->language_feedback, g_free);\n"
    "        suzaku_ibus_engine_render(self);\n"
    "    }\n"
)
FEEDBACK_CLEAR_AFTER = FEEDBACK_CLEAR_BEFORE.replace(
    "suzaku_ibus_engine_render(self);", "suzaku_ibus_engine_render_presentation(self);")
UNCHANGED_DEFINITION_HASHES = {
    "check_adoption_key_repeats":
        "ffe989bbd620813a141697706378ef8a9338b48cf3d25ab6107a975c120eee2d",
    "check_mixed_keyboard":
        "f53e875b7759cf654677561eb13b909641ad1724b8c5d9feb8c74f0ca7b775ef",
    "check_candidate_page_boundaries":
        "f36be9314cb915b50a1a9d06820ed9a7ca18a586d735760e718deaf1608f1646",
    "check_candidate_row_boundaries":
        "504e4c2b67794ad151c79adbffdd79c28007ba9c8f96436ae58793364b0dc7eb",
    "check_language_shortcut":
        "a27ead8fbad4e83ae2fcb8dad17361ca77aee1dfaadd9e8e6659c55596e40365",
    "check_language_shortcut_slow_io":
        "39da0e20099762c1321a2121e548e4edb17b102ee435861394647e59ff511378",
    "check_language_adoption_workflows":
        "d764e06a83f34f000a73fc8a68360bed730e9da0ea8e5cf74df39fc52684d17a",
    "check_language_pending_adoption":
        "eebd5d0314a806a9432f84dbe186cf8eb1e74e9ce7650ddf924ed033a02ebcde",
    "type_seed":
        "8ccb8135d0048a9b59ab7f27f9937f7a786bd72043f704eaca00d68a264bcd82",
    "select_keyboard_candidate":
        "c2878c1f100e655ff2a64f673325428ac74d230f30267c2bf1ffcebf0fd8a948",
    "EnginePeer":
        "151b908eb61809773a5a4914e19066eca71fa16c8896bb85203a7abb30f6defc",
}
APPENDED_CALLS = {
    "check_held_language_transitions": ("check_language_shortcut", 2),
    "check_held_language_pending": ("check_language_shortcut_slow_io", 1),
}
CUSTOM_APPENDED_CALLS = {
    "check_custom_language_long_workflows": ("check_held_language_transitions", 2),
    "check_custom_language_pending_long": ("check_held_language_pending", 1),
}
BOUNDARY_APPENDED_CALLS = {
    "check_custom_language_pending_boundaries": ("check_custom_language_pending_long", 1),
}
CONTINUATION_APPENDED_CALLS = {
    "check_custom_language_pending_continuation": ("check_custom_language_pending_boundaries", 1),
}
NORMAL_CASES = [("en", "zh-Hans", "number-boundary"),
                ("zh-Hans", "en", "enter-paging")]
PENDING_CASES = [("success", "zh-Hans", "hell"), ("failure", "en", "hel")]


def digest(node):
    return hashlib.sha256(ast.dump(node, include_attributes=False).encode()).hexdigest()


def same_ast(left, right):
    return ast.dump(left, include_attributes=False) == ast.dump(right, include_attributes=False)


def statement(text):
    return ast.parse(text).body[0]


def only(nodes, label):
    nodes = list(nodes)
    if len(nodes) != 1:
        raise AssertionError(f"Expected one {label}, found {len(nodes)}")
    return nodes[0]


def definition(name):
    return only((node for node in TREE.body
                 if isinstance(node, (ast.FunctionDef, ast.ClassDef)) and node.name == name), name)


def exact_bus_call(node, name):
    return (isinstance(node, ast.Expr) and isinstance(node.value, ast.Call)
            and isinstance(node.value.func, ast.Name) and node.value.func.id == name
            and len(node.value.args) == 1 and isinstance(node.value.args[0], ast.Name)
            and node.value.args[0].id == "bus" and not node.value.keywords)


def outer_try(node):
    return only((item for item in node.body if isinstance(item, ast.Try)), "outer try")


def direct_loop(body, target):
    expected = statement(target + " = None").targets[0]
    return only((item for item in body if isinstance(item, ast.For)
                 and same_ast(item.target, expected)), f"loop over {target}")


def top_literal(node, name):
    assignment = only((item for item in node.body if isinstance(item, ast.Assign)
                       and len(item.targets) == 1 and isinstance(item.targets[0], ast.Name)
                       and item.targets[0].id == name), f"assignment {name}")
    return ast.literal_eval(assignment.value)


def appended_call_statements(tree, name):
    return [item for item in ast.walk(tree) if isinstance(item, ast.Expr)
            and isinstance(item.value, ast.Call) and isinstance(item.value.func, ast.Name)
            and item.value.func.id == name]


def project_source(source, tree):
    """Remove only new helpers, their two blank lines and exact adjacent calls."""
    lines = source.splitlines(keepends=True)
    omitted = set()
    for name, (predecessor, count) in (APPENDED_CALLS | CUSTOM_APPENDED_CALLS | BOUNDARY_APPENDED_CALLS | CONTINUATION_APPENDED_CALLS).items():
        helper = only((node for node in tree.body
                       if isinstance(node, ast.FunctionDef) and node.name == name), name)
        if helper.decorator_list or lines[helper.end_lineno:helper.end_lineno + 2] != ["\n", "\n"]:
            raise AssertionError("New helper must be undecorated and have two following blank lines")
        omitted.update(range(helper.lineno - 1, helper.end_lineno + 2))
        calls = appended_call_statements(tree, name)
        if len(calls) != count:
            raise AssertionError(f"Expected exactly {count} appended calls to {name}")
        for call in calls:
            if not exact_bus_call(call, name) or call.lineno != call.end_lineno:
                raise AssertionError("Only a one-line exact bus call may be projected")
            prefix = lines[call.lineno - 1].split(name + "(", 1)[0]
            if lines[call.lineno - 1] != prefix + name + "(bus)\n":
                raise AssertionError("Appended call must contain no additional source")
            if lines[call.lineno - 2] != prefix + predecessor + "(bus)\n":
                raise AssertionError("Appended call must immediately follow the old exact bus call")
            omitted.add(call.lineno - 1)
    return "".join(line for index, line in enumerate(lines) if index not in omitted)


def project_feedback_clear(source):
    """Accept only the single completed-feedback rendering substitution."""
    if source.count(FEEDBACK_CLEAR_BEFORE) + source.count(FEEDBACK_CLEAR_AFTER) != 1:
        raise AssertionError("Exactly one completed-feedback block must match the fixed baseline")
    return source.replace(FEEDBACK_CLEAR_AFTER, FEEDBACK_CLEAR_BEFORE, 1)


class HeldLanguageCoverageTests(unittest.TestCase):
    def assert_has_statement(self, node, text):
        expected = statement(text)
        self.assertTrue(any(same_ast(item, expected) for item in ast.walk(node)), text)

    def assert_direct_statement_sequence(self, body, texts):
        position = 0
        for text in texts:
            expected = statement(text)
            matches = [index for index in range(position, len(body))
                       if same_ast(body[index], expected)]
            self.assertTrue(matches, f"Missing or out-of-order direct statement: {text}")
            position = matches[0] + 1

    def case_try(self, name, target):
        node = definition(name)
        profile = direct_loop(outer_try(node).body, "profile")
        case = direct_loop(profile.body, target)
        return only((item for item in case.body if isinstance(item, ast.Try)), "case try")

    def nested_definition(self, node, name):
        return only((item for item in node.body
                     if isinstance(item, ast.FunctionDef) and item.name == name), name)

    def test_native_source_compiles_without_importing_it(self):
        compile(TREE, str(NATIVE_PATH), "exec")

    def test_old_hold_multilingual_navigation_language_and_shared_definitions_are_unchanged(self):
        for name, expected in UNCHANGED_DEFINITION_HASHES.items():
            with self.subTest(name=name):
                self.assertEqual(digest(definition(name)), expected)

    def test_runner_and_language_worker_are_unchanged(self):
        for name, expected in UNCHANGED_FILE_HASHES.items():
            with self.subTest(name=name):
                self.assertEqual(hashlib.sha256((ROOT / name).read_bytes()).hexdigest(), expected)

    def test_bridge_only_changes_the_completed_feedback_render_to_presentation(self):
        source = (ROOT / "src/linux/ibus_engine_bridge.c").read_text()
        self.assertEqual(source.count(FEEDBACK_CLEAR_AFTER), 1)
        self.assertEqual(source.count(FEEDBACK_CLEAR_BEFORE), 0)
        projected = project_feedback_clear(source)
        self.assertEqual(hashlib.sha256(projected.encode()).hexdigest(), BRIDGE_BEFORE_HASH)

    def test_feedback_projection_cannot_hide_a_changed_pending_guard_or_unrelated_edit(self):
        source = (ROOT / "src/linux/ibus_engine_bridge.c").read_text()
        block = FEEDBACK_CLEAR_AFTER if FEEDBACK_CLEAR_AFTER in source else FEEDBACK_CLEAR_BEFORE
        for before, after in [
            ("self->language_feedback != NULL && self->language_job == NULL",
             "self->language_feedback != NULL"),
            ("g_clear_pointer(&self->language_feedback, g_free);",
             "g_clear_pointer(&self->language_feedback, g_free);\n        self->language_job = NULL;"),
        ]:
            with self.subTest(after=after):
                with self.assertRaises(AssertionError):
                    project_feedback_clear(source.replace(block, block.replace(before, after, 1), 1))
        changed = source + "\n/* unrelated production mutation */\n"
        self.assertNotEqual(hashlib.sha256(project_feedback_clear(changed).encode()).hexdigest(),
                            BRIDGE_BEFORE_HASH)

    def test_whole_native_source_only_adds_two_helpers_and_three_exact_adjacent_calls(self):
        projected = project_source(SOURCE, TREE)
        self.assertEqual(hashlib.sha256(projected.encode()).hexdigest(), NATIVE_BEFORE_HASH)

    def test_projection_rejects_wrong_arguments_moved_or_duplicate_calls(self):
        for name, (_, count) in APPENDED_CALLS.items():
            with self.subTest(name=name):
                calls = appended_call_statements(TREE, name)
                self.assertEqual(len(calls), count)
                call = calls[0]
                lines = SOURCE.splitlines(keepends=True)
                original = lines[call.lineno - 1]
                indent = original[:len(original) - len(original.lstrip())]
                for replacement in [f"{name}(other)", f"{name}(bus, bus)",
                                    f"{name}(bus=bus)", f"{name}(bus, timeout=10)"]:
                    modified = list(lines)
                    modified[call.lineno - 1] = indent + replacement + "\n"
                    source = "".join(modified)
                    with self.assertRaises(AssertionError):
                        project_source(source, ast.parse(source))
                moved = list(lines)
                moved[call.lineno - 2:call.lineno] = list(reversed(moved[call.lineno - 2:call.lineno]))
                source = "".join(moved)
                with self.assertRaises(AssertionError):
                    project_source(source, ast.parse(source))
                duplicated = list(lines)
                duplicated.insert(call.lineno, original)
                source = "".join(duplicated)
                with self.assertRaises(AssertionError):
                    project_source(source, ast.parse(source))

    def test_new_helpers_keep_existing_deadlines_and_do_not_inject_drafts(self):
        for name in APPENDED_CALLS:
            with self.subTest(name=name):
                node = definition(name)
                allowed = statement('assert not action(before, "K0")' +
                                    (' and not peer.commits' if name.endswith("transitions") else ''))
                owner = only((item for item in ast.walk(node) if same_ast(item, allowed)),
                             "exact stale-token rejection assertion")
                allowed_call = only((item for item in ast.walk(owner) if isinstance(item, ast.Call)
                                     and isinstance(item.func, ast.Name) and item.func.id == "action"),
                                    "negative stale-token action")
                for call in (item for item in ast.walk(node) if isinstance(item, ast.Call)):
                    if isinstance(call.func, ast.Name) and call.func.id == "action":
                        self.assertIs(call, allowed_call,
                                      "Only an explicitly rejected old-context token may use companion action")
                    if isinstance(call.func, ast.Name) and call.func.id == "wait":
                        self.assertEqual(len(call.args), 2)
                        self.assertFalse(call.keywords)
                    if isinstance(call.func, ast.Attribute) and call.func.attr == "sleep":
                        self.fail("No new sleep may hide held-key or acknowledgement races")

    def test_normal_and_pending_matrices_are_two_fixed_routes_and_two_profiles_each(self):
        for name, cases, target in [
            ("check_held_language_transitions", NORMAL_CASES, "original, target, case"),
            ("check_held_language_pending", PENDING_CASES, "case, target, spelling"),
        ]:
            with self.subTest(name=name):
                node = definition(name)
                self.assertEqual(top_literal(node, "cases"), cases)
                profile = direct_loop(outer_try(node).body, "profile")
                self.assertEqual(ast.literal_eval(profile.iter), ["standard", "home-row"])
                route = direct_loop(profile.body, target)
                self.assertEqual(ast.unparse(route.iter), "cases")
                self.assert_has_statement(node, "assert checked == 4")
                self.assertEqual(sum(isinstance(item, ast.AugAssign)
                                     and ast.unparse(item) == "checked += 1"
                                     for item in ast.walk(node)), 1)
                self.assert_has_statement(node, 'assert json.loads(command("P0"))["ok"]')
                self.assert_has_statement(node, 'peer = EnginePeer(bus)')
                self.assert_has_statement(node, 'type_seed(peer, "hel")')

    def test_both_boundary_helpers_require_full_fresh_frame_and_no_commits(self):
        for name in APPENDED_CALLS:
            with self.subTest(name=name):
                node = definition(name)
                unchanged = self.nested_definition(node, "unchanged")
                self.assert_direct_statement_sequence(unchanged.body, [
                    'initial = Watch()',
                    'try:\n'
                    '    wait(lambda: initial.latest is not None and watch.latest == initial.latest,\n'
                    '         label + " pre-key snapshot")\n'
                    '    before = initial.latest\n'
                    '    assert watch.latest == before, label\n'
                    'finally:\n'
                    '    watchers.remove(initial)\n'
                    '    initial.sock.close()',
                    'for key, code, mask, handled in events:\n'
                    '    assert peer.process_key_event(key, code, mask) == handled, label',
                    'assert command("Q") == b"1"',
                    'fresh = Watch()',
                ])
                owner = only((item for item in unchanged.body if isinstance(item, ast.Try)
                              and item.body and same_ast(item.body[0], statement(
                                  'wait(lambda: fresh.latest is not None, label + " fresh snapshot")'))),
                             "post-key fresh snapshot try")
                self.assertEqual([ast.unparse(item) for item in owner.body], [
                    "wait(lambda: fresh.latest is not None, label + ' fresh snapshot')",
                    "assert fresh.latest == before and watch.latest == before, "
                    "(label, {field: (before[field], fresh.latest[field]) for field in before "
                    "if before[field] != fresh.latest[field]})",
                    "assert not peer.commits, label",
                ])
                self.assertEqual([ast.unparse(item) for item in owner.finalbody],
                                 ['watchers.remove(fresh)', 'fresh.sock.close()'])

    def test_repeat_helpers_keep_one_physical_code_across_keysym_and_modifier_changes(self):
        normal = self.nested_definition(definition("check_held_language_transitions"), "repeats")
        pending = self.nested_definition(definition("check_held_language_pending"), "repeats")
        self.assertEqual([ast.unparse(item) for item in normal.body], [ast.unparse(statement(
            'unchanged(peer, [(key, code, 0, True), (IBus.KEY_at, code, shift, True), '
            '(key, code, alt, True), (IBus.KEY_Return, code, 0, True)], '
            '"held adoption repeats cannot edit or submit")'))])
        self.assertEqual([ast.unparse(item) for item in pending.body], [ast.unparse(statement(
            'unchanged(peer, [(IBus.KEY_2, 3, 0, True), (IBus.KEY_at, 3, shift, True), '
            '(IBus.KEY_2, 3, alt, True), (IBus.KEY_Return, 3, 0, True)], '
            '"pending held adoption repeats cannot edit or submit")'))])

    def test_normal_adoption_happens_before_chord_and_does_not_release_the_held_key(self):
        node = definition("check_held_language_transitions")
        owner = self.case_try(node.name, "original, target, case")
        adoption = only((item for item in owner.body if isinstance(item, ast.If)
                         and ast.unparse(item.test) == "case == 'number-boundary'"
                         and any(same_ast(child, statement('key, code, adopted = IBus.KEY_2, 3, "hello"'))
                                 for child in item.body)),
                        "initial fixed adoption branch")
        self.assertEqual([ast.unparse(item) for item in adoption.body], [
            "row = watch.latest['candidates'][1]",
            "assert (row['text'], row['kind'], row['source']) == ('hello', 'word', 'local')",
            "key, code, adopted = (IBus.KEY_2, 3, 'hello')",
            "assert peer.process_key_event(key, code)",
        ])
        self.assertEqual([ast.unparse(item) for item in adoption.orelse], [
            "index = next((i for i, row in enumerate(watch.latest['candidates']) if "
            "(row['text'], row['kind'], row['source']) == ('hel', 'literal', 'local')))",
            "select_keyboard_candidate(peer, watch, index, 'held transition exact literal row')",
            "key, code, adopted = (IBus.KEY_Return, 28, 'hel')",
            "assert peer.process_key_event(key, code, shift)",
        ])
        self.assert_direct_statement_sequence(owner.body, [
            ast.unparse(adoption),
            'wait(lambda: watch.latest["seed"] == adopted, "adopt before held language transition")',
            'before = watch.latest',
            'assert peer.process_key_event(IBus.KEY_space, 57, chord)',
            'wait(lambda: json.loads(command("S"))["settings"]["language"] == target '
            'and watch.latest["language"] == target and watch.latest["context"] > before["context"] '
            'and watch.latest["seed"] == adopted, "held transition language acknowledgement")',
            'assert json.loads(config.read_bytes())["language"] == target',
            'assert not action(before, "K0") and not peer.commits',
            'unchanged(peer, [(IBus.KEY_Shift_L, 42, release, False), '
            '(IBus.KEY_Control_L, 37, release, False), (IBus.KEY_space, 57, release, False)], '
            '"other releases cannot unlatch adoption")',
            'assert watch.latest["selected"] == 0',
            'unchanged(peer, [(IBus.KEY_Page_Up, 104, 0, True)], "held transition page boundary")',
            'repeats(peer, key, code)',
        ])

    def test_normal_number_branch_retires_old_undo_but_stays_held_until_matching_code_release(self):
        owner = self.case_try("check_held_language_transitions", "original, target, case")
        branches = [item for item in owner.body if isinstance(item, ast.If)
                    and ast.unparse(item.test) == "case == 'number-boundary'"]
        self.assertEqual(len(branches), 2)
        self.assertEqual([ast.unparse(item) for item in branches[1].body], [ast.unparse(statement(text))
            for text in [
                'assert peer.process_key_event(IBus.KEY_BackSpace, 14)',
                'wait(lambda: watch.latest["seed"] == "hell", "new language retires old adoption undo")',
                'repeats(peer, key, code)',
                'assert not peer.process_key_event(IBus.KEY_a, code, release)',
                'assert peer.process_key_event(IBus.KEY_2, code, alt)',
                'assert not peer.process_key_event(IBus.KEY_2, code, release)',
                'wait(lambda: watch.latest["seed"] == "hell2", "released number can insert immediately")',
                'committed = "hell2 "',
            ]])

    def test_normal_enter_branch_requires_real_pages_before_held_release_and_new_undo(self):
        owner = self.case_try("check_held_language_transitions", "original, target, case")
        branch = [item for item in owner.body if isinstance(item, ast.If)
                  and ast.unparse(item.test) == "case == 'number-boundary'"][1]
        body = branch.orelse
        page = direct_loop(body, "page_key, page_code, selected")
        self.assertTrue(same_ast(page.iter, ast.parse(
            '[(IBus.KEY_Page_Down, 109, 6), (IBus.KEY_Page_Up, 104, 0)]', mode="eval").body))
        self.assert_direct_statement_sequence(page.body, [
            'old = watch.latest',
            'assert peer.process_key_event(page_key, page_code)',
            'wait(lambda: watch.latest["revision"] > old["revision"] '
            'and watch.latest["selected"] == selected, "held transition real page move")',
            'for field in ["host", "context", "focused", "private", "language", "seed", "candidates"]:\n'
            '    assert watch.latest[field] == old[field], (case, field)',
            'repeats(peer, key, code)',
        ])
        self.assert_direct_statement_sequence(body, [
            'assert len(watch.latest["candidates"]) > 6, "held paging fixture needs a real second page"',
            ast.unparse(page),
            'select_keyboard_candidate(peer, watch, index, "held transition exact word row")',
            'repeats(peer, key, code)',
            'assert not peer.process_key_event(IBus.KEY_a, code, release)',
            'assert peer.process_key_event(key, code, shift)',
            'assert not peer.process_key_event(key, code, release)',
            'wait(lambda: watch.latest["seed"] == "hello", "released Enter permits new adoption")',
            'assert peer.process_key_event(IBus.KEY_BackSpace, 14)',
            'wait(lambda: watch.latest["seed"] == "hel", "new gesture restores exact spelling")',
            'select_keyboard_candidate(peer, watch, index, "readopt after held transition undo")',
            'assert peer.process_key_event(key, code, shift)',
            'assert not peer.process_key_event(key, code, release)',
            'wait(lambda: watch.latest["seed"] == "hello", "new gesture readoption")',
            'committed = "hello "',
        ])
        exact_word = "(row['text'], row['kind'], row['source']) == ('hello', 'word', 'local')"
        self.assertEqual(ast.unparse(ast.Module(body=body, type_ignores=[])).count(exact_word), 2)

    def test_pending_adopts_before_arming_and_keeps_repeats_inside_the_unreleased_barrier(self):
        owner = self.case_try("check_held_language_pending", "case, target, spelling")
        self.assert_direct_statement_sequence(owner.body, [
            'type_seed(peer, "hel")',
            'wait(lambda: watch.latest["seed"] == "hel", "pending held original spelling")',
            'row = watch.latest["candidates"][1]',
            'assert (row["text"], row["kind"], row["source"]) == ("hello", "word", "local")',
            'assert peer.process_key_event(IBus.KEY_2, 3)',
            'wait(lambda: watch.latest["seed"] == "hello", "hold adoption before pending switch")',
            'before = watch.latest',
            'for marker in [armed, entered, released, finished, failed]:\n'
            '    marker.unlink(missing_ok=True)',
            'armed.touch()',
            'started = time.monotonic()',
            'assert peer.process_key_event(IBus.KEY_space, 57, chord)',
            'assert time.monotonic() - started < .25, "held switch blocked the input loop"',
            'wait(entered.exists, "held switch reached controlled durable-write barrier")',
            'assert json.loads(command("S", timeout=.25))["settings"]["language"] == "en"',
            'assert config.read_bytes() == before_disk',
            'assert "正在切换" in feedback["text"], feedback["text"]',
            'unchanged(peer, [(IBus.KEY_Shift_L, 42, release, False), '
            '(IBus.KEY_Control_L, 37, release, False), (IBus.KEY_space, 57, release, False)], '
            '"pending other releases cannot unlatch adoption")',
            'assert watch.latest["selected"] == 0',
            'unchanged(peer, [(IBus.KEY_Page_Up, 104, 0, True)], "pending held page boundary")',
            'repeats(peer)',
            'assert "正在切换" in feedback["text"], feedback["text"]',
            'assert not peer.commits and watch.latest["context"] == before["context"]',
            'if case == "failure":\n    failed.touch()',
            'released.touch()',
            'wait(lambda: finished.exists() and not list(runtime.glob(".suzaku-*.tmp")), '
            '"held language worker retires temporary settings")',
        ])
        direct_releases = [item for item in owner.body if same_ast(item, statement('released.touch()'))]
        self.assertEqual(len(direct_releases), 1)

    def test_pending_outcomes_keep_disk_context_and_branch_specific_undo_before_release(self):
        owner = self.case_try("check_held_language_pending", "case, target, spelling")
        outcome = only((item for item in owner.body if isinstance(item, ast.If)
                        and ast.unparse(item.test) == "case == 'failure'" and item.orelse),
                       "post-barrier outcome branch")
        self.assertEqual([ast.unparse(item) for item in outcome.body], [ast.unparse(statement(text))
            for text in [
                'wait(lambda: "未保存" in feedback["text"], "held failed language feedback")',
                'assert json.loads(command("S"))["settings"]["language"] == target',
                'assert config.read_bytes() == before_disk',
                'assert watch.latest["context"] == before["context"]',
            ]])
        self.assertEqual([ast.unparse(item) for item in outcome.orelse], [ast.unparse(statement(text))
            for text in [
                'wait(lambda: json.loads(command("S"))["settings"]["language"] == target '
                'and watch.latest["language"] == target and watch.latest["context"] > before["context"] '
                'and watch.latest["seed"] == "hello", "held pending language acknowledgement")',
                'assert json.loads(config.read_bytes())["language"] == target',
                'assert not action(before, "K0")',
            ]])
        self.assert_direct_statement_sequence(owner.body, [
            ast.unparse(outcome),
            'assert watch.latest["seed"] == "hello" and watch.latest["language"] == target',
            'unchanged(peer, [(IBus.KEY_Page_Up, 104, 0, True)], "settled held page boundary")',
            'repeats(peer)',
            'assert peer.process_key_event(IBus.KEY_BackSpace, 14)',
            'wait(lambda: watch.latest["seed"] == spelling, "held switch branch-specific adoption undo")',
            'repeats(peer)',
            'assert not peer.process_key_event(IBus.KEY_a, 3, release)',
            'assert peer.process_key_event(IBus.KEY_2, 3, alt)',
            'assert not peer.process_key_event(IBus.KEY_2, 3, release)',
            'wait(lambda: watch.latest["seed"] == spelling + "2", "released held key immediately edits")',
        ])

    def test_both_flows_keep_space_in_draft_and_exactly_one_real_enter_commit(self):
        for name, target, label in [
            ("check_held_language_transitions", "original, target, case", "held transition"),
            ("check_held_language_pending", "case, target, spelling", "pending held"),
        ]:
            with self.subTest(name=name):
                owner = self.case_try(name, target)
                self.assert_direct_statement_sequence(owner.body, [
                    'assert peer.process_key_event(IBus.KEY_space)',
                    f'wait(lambda: watch.latest["seed"] == committed, "{label} Space continuity")',
                    'assert not peer.commits',
                    'assert peer.process_key_event(IBus.KEY_Return)',
                    'wait(lambda: peer.commits == [committed] and not watch.latest["seed"], '
                    f'"{label} exactly one explicit commit")',
                    'assert not peer.process_key_event(IBus.KEY_Return)',
                    'pump()',
                    'assert peer.commits == [committed]',
                    'checked += 1',
                ])
                bare_enters = [item for item in ast.walk(owner) if isinstance(item, ast.Call)
                               and isinstance(item.func, ast.Attribute)
                               and item.func.attr == "process_key_event" and len(item.args) == 1
                               and ast.unparse(item.args[0]) == "IBus.KEY_Return"]
                self.assertEqual(len(bare_enters), 2)

    def test_pending_cleanup_releases_fixture_but_waits_for_worker_retirement(self):
        owner = self.case_try("check_held_language_pending", "case, target, spelling")
        cleanup = only((item for item in owner.finalbody if isinstance(item, ast.With)), "guarded cleanup")
        self.assertEqual([ast.unparse(item) for item in cleanup.body], [ast.unparse(statement(text))
            for text in [
                'released.touch(exist_ok=True)',
                'peer.close()',
                'peer.connection.signal_unsubscribe(subscription)',
                'if entered.exists():\n'
                '    wait(lambda: finished.exists() and not list(runtime.glob(".suzaku-*.tmp")), '
                '"pending held cleanup retires worker")',
            ]])


if __name__ == "__main__":
    unittest.main()
