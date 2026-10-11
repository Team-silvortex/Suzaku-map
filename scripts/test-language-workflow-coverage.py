#!/usr/bin/env python3
"""Static language-workflow guards; never imports or runs the native suite.

These checks protect fixture structure and appended coverage. They do not prove
private IBus, physical keyboard, model, installation or desktop acceptance.
"""
import ast
import copy
import hashlib
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parent.parent
SOURCE_PATH = ROOT / "scripts/test-native-sync.py"
SOURCE = SOURCE_PATH.read_text()
TREE = ast.parse(SOURCE)
BRIDGE = (ROOT / "src/linux/ibus_engine_bridge.c").read_text()

# Captured independently before either new workflow helper was inserted.
LEGACY_HASHES = {
    "check_language_shortcut":
        "fbe2e568b5da9a0288e41fe2bf896a6645a28153e4ac0f99a999414004da409e",
    "check_language_shortcut_slow_io":
        "d7411b428bf2f5f219b0c4869dd7fa4ea48175df1b456debbd41f4bef5949f24",
}
SHARED_HASHES = {
    "type_seed":
        "8ccb8135d0048a9b59ab7f27f9937f7a786bd72043f704eaca00d68a264bcd82",
    "select_keyboard_candidate":
        "c2878c1f100e655ff2a64f673325428ac74d230f30267c2bf1ffcebf0fd8a948",
    "EnginePeer":
        "151b908eb61809773a5a4914e19066eca71fa16c8896bb85203a7abb30f6defc",
}
APPENDED_CALLS = {
    "check_language_shortcut": "check_language_adoption_workflows",
    "check_language_shortcut_slow_io": "check_language_pending_adoption",
}
SUCCESS_CASES = [
    ("en", "zh-Hans", "nihao", "你好", "你好，很高兴认识你。"),
    ("zh-Hans", "en", "hel", "hello", "hello, how are you?"),
    ("en", "zh-Hans", "API。nihao", "API。你好", "API。你好，很高兴认识你。"),
]
PENDING_CASES = ["success-adopt", "success-adopt-undo", "success-adopt-edit", "failed-adopt"]
PENDING_DIRECTIONS = [("en", "zh-Hans", "hel", "hello"),
                      ("zh-Hans", "en", "nihao", "你好")]


def definition(name):
    matches = [node for node in TREE.body
               if isinstance(node, (ast.FunctionDef, ast.ClassDef))
               and node.name == name]
    if len(matches) != 1:
        raise AssertionError(f"Expected exactly one top-level definition: {name}")
    return matches[0]


def digest(node):
    return hashlib.sha256(ast.dump(node, include_attributes=False).encode()).hexdigest()


def statement(text):
    return ast.parse(text).body[0]


def same_ast(left, right):
    return ast.dump(left, include_attributes=False) == ast.dump(right, include_attributes=False)


def only(nodes, label):
    nodes = list(nodes)
    if len(nodes) != 1:
        raise AssertionError(f"Expected one {label}, found {len(nodes)}")
    return nodes[0]


def outer_try(node):
    return only((item for item in node.body if isinstance(item, ast.Try)), "outer try")


def direct_loop(body, target):
    expected = statement(target + " = None").targets[0]
    return only((item for item in body if isinstance(item, ast.For)
                 and same_ast(item.target, expected)), f"loop over {target}")


def top_literal(node, name):
    item = only((item for item in node.body if isinstance(item, ast.Assign)
                 and len(item.targets) == 1 and isinstance(item.targets[0], ast.Name)
                 and item.targets[0].id == name), f"literal assignment {name}")
    return ast.literal_eval(item.value)


def exact_bus_call(node, name):
    return (isinstance(node, ast.Expr) and isinstance(node.value, ast.Call)
            and isinstance(node.value.func, ast.Name) and node.value.func.id == name
            and len(node.value.args) == 1 and isinstance(node.value.args[0], ast.Name)
            and node.value.args[0].id == "bus" and not node.value.keywords)


def project_legacy(node, helper):
    """Delete only the one appended, argument-exact outer-try expression."""
    projected = copy.deepcopy(node)
    owners = [statement for statement in projected.body if isinstance(statement, ast.Try)]
    if len(owners) != 1:
        raise AssertionError(f"Unexpected outer try structure in {node.name}")
    owner = owners[0]
    if owner.body and exact_bus_call(owner.body[-1], helper):
        owner.body.pop()
    return projected


def c_function_body(source, name):
    # Strip comments first so a brace in an explanatory comment cannot change
    # the selected C function. This is deliberately not a C compiler substitute.
    source = re.sub(r"/\*.*?\*/|//[^\n]*", "", source, flags=re.S)
    match = re.search(r"\b" + re.escape(name) + r"\s*\([^;{}]*\)\s*\{", source)
    if match is None:
        raise AssertionError(f"Missing C function: {name}")
    start = match.end()
    depth = 1
    for offset in range(start, len(source)):
        depth += (source[offset] == "{") - (source[offset] == "}")
        if depth == 0:
            return source[start:offset]
    raise AssertionError(f"Unterminated C function: {name}")


class LanguageWorkflowCoverageTests(unittest.TestCase):
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

    def test_native_source_compiles_without_importing_or_starting_it(self):
        compile(TREE, str(SOURCE_PATH), "exec")

    def test_old_language_matrices_and_deadlines_are_unchanged(self):
        for name, expected in LEGACY_HASHES.items():
            with self.subTest(name=name):
                self.assertEqual(digest(project_legacy(definition(name), APPENDED_CALLS[name])),
                                 expected)

    def test_shared_typing_selection_and_private_peer_are_unchanged(self):
        for name, expected in SHARED_HASHES.items():
            with self.subTest(name=name):
                self.assertEqual(digest(definition(name)), expected)

    def test_projection_only_accepts_the_exact_appended_call(self):
        for name, helper in APPENDED_CALLS.items():
            with self.subTest(name=name):
                legacy = project_legacy(definition(name), helper)
                expected = digest(legacy)
                valid = copy.deepcopy(legacy)
                owner = next(node for node in valid.body if isinstance(node, ast.Try))
                owner.body.append(ast.parse(f"{helper}(bus)").body[0])
                self.assertEqual(digest(project_legacy(valid, helper)), expected)
                for call in [f"{helper}(other)", f"{helper}(bus, bus)",
                             f"{helper}(bus=bus)", f"{helper}(bus, timeout=10)"]:
                    invalid = copy.deepcopy(legacy)
                    owner = next(node for node in invalid.body if isinstance(node, ast.Try))
                    owner.body.append(ast.parse(call).body[0])
                    self.assertNotEqual(digest(project_legacy(invalid, helper)), expected)
                moved = copy.deepcopy(valid)
                owner = next(node for node in moved.body if isinstance(node, ast.Try))
                owner.body.insert(0, owner.body.pop())
                self.assertNotEqual(digest(project_legacy(moved, helper)), expected)
                duplicate = copy.deepcopy(valid)
                owner = next(node for node in duplicate.body if isinstance(node, ast.Try))
                owner.body.append(ast.parse(f"{helper}(bus)").body[0])
                self.assertNotEqual(digest(project_legacy(duplicate, helper)), expected)

    def test_new_helpers_are_only_appended_once_in_the_old_outer_try(self):
        for name, helper in APPENDED_CALLS.items():
            with self.subTest(name=name):
                node = definition(name)
                owners = [statement for statement in node.body if isinstance(statement, ast.Try)]
                self.assertEqual(len(owners), 1)
                self.assertTrue(exact_bus_call(owners[0].body[-1], helper))
                calls = [call for call in ast.walk(node) if isinstance(call, ast.Call)
                         and isinstance(call.func, ast.Name) and call.func.id == helper]
                self.assertEqual(len(calls), 1)
                self.assertIsInstance(definition(helper), ast.FunctionDef)

    def test_success_matrix_is_three_fixed_routes_two_profiles_and_two_kinds(self):
        node = definition("check_language_adoption_workflows")
        self.assertEqual(top_literal(node, "cases"), SUCCESS_CASES)
        profile = direct_loop(outer_try(node).body, "profile")
        self.assertEqual(ast.literal_eval(profile.iter), ["standard", "home-row"])
        route = direct_loop(profile.body, "original, target, seed, word, sentence")
        self.assertEqual(ast.unparse(route.iter), "cases")
        choice = direct_loop(route.body, "kind, adopted")
        self.assertEqual(ast.unparse(choice.iter), "[('word', word), ('sentence', sentence)]")
        self.assert_has_statement(node, "assert checked == 12")
        self.assertEqual(sum(isinstance(item, ast.AugAssign)
                             and ast.unparse(item) == "checked += 1"
                             for item in ast.walk(node)), 1)

    def test_success_chain_keeps_exact_undo_space_digits_and_raw_commit(self):
        node = definition("check_language_adoption_workflows")
        for text in [
            'type_seed(peer, seed)',
            'adopt(peer, adopted, kind)',
            'assert peer.process_key_event(IBus.KEY_BackSpace)',
            'assert peer.process_key_event(IBus.KEY_space)',
            'assert peer.process_key_event(IBus.KEY_7, 16, IBus.ModifierType.MOD1_MASK)',
            'assert not peer.process_key_event(IBus.KEY_7, 16, release)',
            'adopt(peer, adopted[:-1])',
            'assert peer.process_key_event(IBus.KEY_Return)',
            'assert not peer.process_key_event(IBus.KEY_Return)',
            'assert peer.commits == [adopted[:-1]]',
        ]:
            self.assert_has_statement(node, text)
        deletion = only((item for item in ast.walk(node) if isinstance(item, ast.For)
                         and ast.unparse(item.target) == "expected"), "ordinary deletion loop")
        self.assertEqual(ast.unparse(deletion.iter),
                         "[adopted + ' ', adopted, adopted[:-1]]")
        checks = [item for item in ast.walk(node) if isinstance(item, ast.Lambda)]
        for test in ["lambda: watch.latest['seed'] == seed",
                     "lambda: watch.latest['seed'] == adopted + ' '",
                     "lambda: watch.latest['seed'] == adopted + ' 7'",
                     "lambda: peer.commits == [adopted[:-1]] and not watch.latest['seed']"]:
            self.assertTrue(any(same_ast(item, ast.parse(test, mode="eval").body)
                                for item in checks), test)
        profile = direct_loop(outer_try(node).body, "profile")
        route = direct_loop(profile.body, "original, target, seed, word, sentence")
        choice = direct_loop(route.body, "kind, adopted")
        owner = only((item for item in choice.body if isinstance(item, ast.Try)), "success case try")
        self.assert_direct_statement_sequence(owner.body, [
            'adopt(peer, adopted, kind)',
            'assert peer.process_key_event(IBus.KEY_BackSpace)',
            'wait(lambda: watch.latest["seed"] == seed, "new-language exact adoption undo")',
            'adopt(peer, adopted, kind)',
            'assert peer.process_key_event(IBus.KEY_space)',
            'wait(lambda: watch.latest["seed"] == adopted + " ", "adopted draft Space continuation")',
            'assert peer.process_key_event(IBus.KEY_7, 16, IBus.ModifierType.MOD1_MASK)',
            'assert not peer.process_key_event(IBus.KEY_7, 16, release)',
            'wait(lambda: watch.latest["seed"] == adopted + " 7", "literal digit after adoption")',
            ast.unparse(deletion),
            'assert not peer.commits',
            'adopt(peer, adopted[:-1])',
            'assert peer.process_key_event(IBus.KEY_Return)',
            'wait(lambda: peer.commits == [adopted[:-1]] and not watch.latest["seed"], '
            '"workflow exactly one final commit")',
            'assert not peer.process_key_event(IBus.KEY_Return)',
            'pump()',
            'assert peer.commits == [adopted[:-1]]',
        ])
        enter_calls = [call for call in ast.walk(node) if isinstance(call, ast.Call)
                       and isinstance(call.func, ast.Attribute)
                       and call.func.attr == "process_key_event" and call.args
                       and ast.unparse(call.args[0]) == "IBus.KEY_Return"]
        self.assertEqual(len(enter_calls), 2, "One committing Enter and one empty-draft passthrough")

    def test_numeric_adoption_uses_fixed_local_rows_and_releases_physical_keys(self):
        node = definition("check_language_adoption_workflows")
        adopt = only((item for item in node.body if isinstance(item, ast.FunctionDef)
                      and item.name == "adopt"), "local numeric-adoption helper")
        for text in [
            'select_keyboard_candidate(peer, watch, index, "workflow exact keyboard row")',
            'key = IBus.KEY_1 + index % 6',
            'assert peer.process_key_event(key, 10 + index % 6)',
            'assert not peer.process_key_event(key, 10 + index % 6, release)',
            'assert not peer.commits',
        ]:
            self.assert_has_statement(adopt, text)
        self.assertIn("row['text'] == text and row['source'] == 'local' and "
                      "(kind is None or row['kind'] == kind)", ast.unparse(adopt))

    def test_pending_matrix_is_four_fixed_branches_in_both_directions(self):
        node = definition("check_language_pending_adoption")
        self.assertEqual(top_literal(node, "cases"), PENDING_CASES)
        route = direct_loop(outer_try(node).body, "original, target, seed, word")
        self.assertEqual(ast.literal_eval(route.iter), PENDING_DIRECTIONS)
        case = direct_loop(route.body, "case")
        self.assertEqual(ast.unparse(case.iter), "cases")
        self.assert_has_statement(node, "assert checked == 8")
        self.assertEqual(sum(isinstance(item, ast.AugAssign)
                             and ast.unparse(item) == "checked += 1"
                             for item in ast.walk(node)), 1)

    def test_pending_progress_is_checked_after_keys_before_releasing_barrier(self):
        node = definition("check_language_pending_adoption")
        route = direct_loop(outer_try(node).body, "original, target, seed, word")
        case = direct_loop(route.body, "case")
        owner = only((item for item in case.body if isinstance(item, ast.Try)), "pending case try")
        body = owner.body
        adoption = only((item for item in body if isinstance(item, ast.Assert)
                         and ast.unparse(item.test) == "peer.process_key_event(key, 10 + index)"),
                        "pending numeric adoption")
        adopted = only((item for item in body if same_ast(item, statement(
            'wait(lambda: watch.latest["seed"] == word, "adopt while language save is pending")'))),
            "pending adopted draft wait")
        edits = only((item for item in body if isinstance(item, ast.If)
                      and ast.unparse(item.test) == "case == 'success-adopt-undo'"),
                     "pending undo/edit branches")
        edited = only((item for item in body if same_ast(item, statement(
            'wait(lambda: watch.latest["seed"] == expected, "latest draft before switch acknowledgement")'))),
            "latest pre-acknowledgement draft wait")
        checks = [item for item in body if isinstance(item, ast.Assert)
                  and ast.unparse(item.test) == "'正在切换' in feedback['text']"]
        self.assertEqual(len(checks), 3,
                         "Check progress before adoption, immediately after adoption, and after pending edits")
        release = only((item for item in body if same_ast(item, statement("released.touch()"))),
                       "main-path barrier release")
        self.assertLess(checks[0].lineno, adoption.lineno)
        self.assertLess(adoption.lineno, adopted.lineno)
        self.assertLess(adopted.lineno, checks[1].lineno)
        self.assertLess(checks[1].lineno, edits.lineno)
        self.assertLess(edits.end_lineno, edited.lineno)
        self.assertLess(edited.lineno, checks[2].lineno)
        self.assertLess(checks[2].lineno, release.lineno)
        fail = only((item for item in body if isinstance(item, ast.If)
                     and ast.unparse(item.test) == "case == 'failed-adopt'"
                     and any(same_ast(child, statement("failed.touch()")) for child in item.body)),
                    "pre-release failure marker")
        self.assertLess(checks[2].lineno, fail.lineno)
        self.assertLess(fail.end_lineno, release.lineno)
        self.assert_has_statement(node, "assert not peer.commits and watch.latest['context'] == before['context']")
        self.assert_has_statement(node, "assert not peer.process_key_event(key, 10 + index, release)")

    def test_pending_success_retires_old_undo_and_failure_preserves_it(self):
        node = definition("check_language_pending_adoption")
        for text in [
            'expected = seed', 'expected = word + "x"', 'expected = word[:-1]',
            'failed.touch()', 'assert config.read_bytes() == before_disk',
            'assert watch.latest["context"] == before["context"]',
            'assert json.loads(config.read_bytes())["language"] == target',
            'assert not peer.commits',
        ]:
            self.assert_has_statement(node, text)
        route = direct_loop(outer_try(node).body, "original, target, seed, word")
        case = direct_loop(route.body, "case")
        owner = only((item for item in case.body if isinstance(item, ast.Try)), "pending case try")
        edits = only((item for item in owner.body if isinstance(item, ast.If)
                      and ast.unparse(item.test) == "case == 'success-adopt-undo'"),
                     "pre-acknowledgement adoption undo/edit")
        self.assertEqual([ast.unparse(item) for item in edits.body],
                         ["assert peer.process_key_event(IBus.KEY_BackSpace)", "expected = seed"])
        self.assertEqual(len(edits.orelse), 1)
        edit = edits.orelse[0]
        self.assertIsInstance(edit, ast.If)
        self.assertEqual(ast.unparse(edit.test), "case == 'success-adopt-edit'")
        self.assertEqual([ast.unparse(item) for item in edit.body],
                         ["type_seed(peer, 'x')", "expected = word + 'x'"])
        outcome = only((item for item in owner.body if isinstance(item, ast.If)
                        and ast.unparse(item.test) == "case == 'failed-adopt'"
                        and any(isinstance(child, ast.Assign) for child in item.body)),
                       "post-barrier failure/success outcomes")
        self.assert_direct_statement_sequence(outcome.body, [
            'wait(lambda: "未保存" in feedback["text"], "failed pending switch feedback")',
            'assert json.loads(command("S"))["settings"]["language"] == original',
            'assert config.read_bytes() == before_disk',
            'assert watch.latest["context"] == before["context"]',
            'assert peer.process_key_event(IBus.KEY_BackSpace)',
            'expected = seed',
        ])
        success = only((item for item in outcome.orelse if isinstance(item, ast.If)),
                       "post-success adoption undo lifetime")
        self.assertEqual(ast.unparse(success.test), "case == 'success-adopt'")
        self.assertEqual([ast.unparse(item) for item in success.body],
                         ["assert peer.process_key_event(IBus.KEY_BackSpace)", "expected = word[:-1]"])
        self.assertEqual(len(success.orelse), 1)
        edited = success.orelse[0]
        self.assertIsInstance(edited, ast.If)
        self.assertEqual(ast.unparse(edited.test), "case == 'success-adopt-edit'")
        self.assert_direct_statement_sequence(edited.body, [
            'assert peer.process_key_event(IBus.KEY_BackSpace)',
            'wait(lambda: watch.latest["seed"] == word, "delete latest pending edit")',
            'assert peer.process_key_event(IBus.KEY_BackSpace)',
            'expected = word[:-1]',
        ])
        self.assert_has_statement(node, 'with preserve_primary_failure("pending language adoption cleanup"):\n'
                                  '    released.touch(exist_ok=True)\n'
                                  '    peer.close()\n'
                                  '    peer.connection.signal_unsubscribe(subscription)\n'
                                  '    if entered.exists():\n'
                                  '        wait(lambda: finished.exists() and not list(runtime.glob(".suzaku-*.tmp")),\n'
                                  '             "pending language cleanup retires its worker")')

    def test_new_helpers_do_not_inject_drafts_or_relax_wait_deadlines(self):
        for name in APPENDED_CALLS.values():
            with self.subTest(name=name):
                node = definition(name)
                for call in (item for item in ast.walk(node) if isinstance(item, ast.Call)):
                    if isinstance(call.func, ast.Name) and call.func.id == "action":
                        self.assertEqual(ast.unparse(call), "action(before, 'K0')")
                    if isinstance(call.func, ast.Name) and call.func.id == "wait":
                        self.assertEqual(len(call.args), 2)
                        self.assertFalse(call.keywords, "New wait must keep the existing default limit")
                self.assert_has_statement(node, "peer = EnginePeer(bus)")
                self.assert_has_statement(node, "type_seed(peer, seed)")

    def test_ordinary_key_feedback_clear_is_only_after_the_language_job_finishes(self):
        body = c_function_body(BRIDGE, "suzaku_ibus_engine_process_key_event")
        branches = re.findall(
            r"if\s*\(([^)]*)\)\s*\{\s*"
            r"g_clear_pointer\(&self->language_feedback,\s*g_free\);\s*"
            r"suzaku_ibus_engine_render_presentation\(self\);\s*\}", body)
        self.assertEqual(len(branches), 1, "Expected the single ordinary-key feedback clear")
        condition = re.sub(r"\s+", "", branches[0])
        self.assertIn(condition, {
            "self->language_feedback!=NULL&&self->language_job==NULL",
            "self->language_job==NULL&&self->language_feedback!=NULL",
        })


if __name__ == "__main__":
    unittest.main()
