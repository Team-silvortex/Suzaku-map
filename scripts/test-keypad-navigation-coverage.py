#!/usr/bin/env python3
"""Lightweight guards for appended keypad navigation; no native/UI execution.

NumLock-off navigation is a new bounded compatibility feature. These source
checks do not constitute real keyboard, installation or complete-gate evidence.
"""
import ast
import copy
import hashlib
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parent.parent
NATIVE_PATH = ROOT / "scripts/test-native-sync.py"
KEYBOARD_PATH = ROOT / "scripts/test-linux-keyboard.py"
NATIVE_SOURCE = NATIVE_PATH.read_text()
KEYBOARD_SOURCE = KEYBOARD_PATH.read_text()
NATIVE = ast.parse(NATIVE_SOURCE)
KEYBOARD = ast.parse(KEYBOARD_SOURCE)
BRIDGE = (ROOT / "src/linux/ibus_engine_bridge.c").read_text()

# Independently captured before the two new helpers/calls or C aliases existed.
ROW_BOUNDARY_HASH = "e39198fcdeb5fcfa596374ee083e6dda915498ebb9f9aa7bae9e49a8790af175"
KEYBOARD_TRY_HASH = "691933ec8840089a5f5d1ff36d83a27439ebc7f4395c97a09a7b338cbc644748"
KEYBOARD_CLASS_HASH = "39abb116a04783fc0d28660cb611266196eacef2a2f1fd89ba650c06060dcfa9"
KEYBOARD_BEFORE_HASH = "18752912fbff9f87ed4c11a09ac058de41cbf4b185d764150f38327e25fe5326"
UNCHANGED_NATIVE_HASHES = {
    "check_mixed_keyboard": "f53e875b7759cf654677561eb13b909641ad1724b8c5d9feb8c74f0ca7b775ef",
    "check_adoption_key_repeats": "ffe989bbd620813a141697706378ef8a9338b48cf3d25ab6107a975c120eee2d",  # gitleaks:allow -- fixed regression AST SHA-256, not a credential
    "check_candidate_page_boundaries": "f36be9314cb915b50a1a9d06820ed9a7ca18a586d735760e718deaf1608f1646",
    "type_seed": "8ccb8135d0048a9b59ab7f27f9937f7a786bd72043f704eaca00d68a264bcd82",
    "select_keyboard_candidate": "c2878c1f100e655ff2a64f673325428ac74d230f30267c2bf1ffcebf0fd8a948",
    "EnginePeer": "151b908eb61809773a5a4914e19066eca71fa16c8896bb85203a7abb30f6defc",
}
DIGIT_REGION_HASH = "03b6ab2968d7973f99151c3cc0b0eab1e396267ff1eb0badbf20f3b14ba21043"
BRIDGE_BEFORE_HASH = "04a6651dd86b7af615b352515df3052d23900dbc522aa9199a7d2f6436e63787"
KEYVALS = ["KP_Up", "KP_Down", "KP_Left", "KP_Right", "KP_Prior", "KP_Next"]
NATIVE_CASES = """[
    ("KP_Up", IBus.KEY_KP_Up, "row", False),
    ("KP_Down", IBus.KEY_KP_Down, "row", True),
    ("KP_Left", IBus.KEY_KP_Left, "row", False),
    ("KP_Right", IBus.KEY_KP_Right, "row", True),
    ("KP_Prior", IBus.KEY_KP_Prior, "page", False),
    ("KP_Next", IBus.KEY_KP_Next, "page", True),
]"""
LANGUAGES = [("en", "hel", "hello"), ("zh-Hans", "nihao", "你好")]
C_ALIAS_ADDITIONS = [
    ("    /* NumLock-off keypad navigation is the same gesture, not a literal digit.\n"
     "     * Keep raw keysyms through Compose above; do not remap physical keycodes. */\n", ""),
    ("if (keyval == IBUS_KEY_Page_Up || keyval == IBUS_KEY_Page_Down ||\n"
     "        keyval == IBUS_KEY_KP_Prior || keyval == IBUS_KEY_KP_Next)",
     "if (keyval == IBUS_KEY_Page_Up || keyval == IBUS_KEY_Page_Down)"),
    ("suzaku_ibus_engine_change_page(self,\n"
     "            keyval == IBUS_KEY_Page_Down || keyval == IBUS_KEY_KP_Next);",
     "suzaku_ibus_engine_change_page(self, keyval == IBUS_KEY_Page_Down);"),
    ("if (keyval == IBUS_KEY_Up || keyval == IBUS_KEY_Left ||\n"
     "        keyval == IBUS_KEY_KP_Up || keyval == IBUS_KEY_KP_Left)",
     "if (keyval == IBUS_KEY_Up || keyval == IBUS_KEY_Left)"),
    ("if (keyval == IBUS_KEY_Down || keyval == IBUS_KEY_Right || keyval == IBUS_KEY_Tab ||\n"
     "        keyval == IBUS_KEY_KP_Down || keyval == IBUS_KEY_KP_Right)",
     "if (keyval == IBUS_KEY_Down || keyval == IBUS_KEY_Right || keyval == IBUS_KEY_Tab)"),
]


def digest(node):
    return hashlib.sha256(ast.dump(node, include_attributes=False).encode()).hexdigest()


def same_ast(left, right):
    return ast.dump(left, include_attributes=False) == ast.dump(right, include_attributes=False)


def statement(text):
    return ast.parse(text).body[0]


def only(nodes, label):
    nodes = list(nodes)
    if len(nodes) != 1:
        raise AssertionError(f"Expected exactly one {label}, found {len(nodes)}")
    return nodes[0]


def definition(tree, name):
    return only((node for node in tree.body
                 if isinstance(node, (ast.FunctionDef, ast.ClassDef)) and node.name == name), name)


def outer_try(node):
    return only((item for item in node.body if isinstance(item, ast.Try)), "outer try")


def direct_loop(body, target):
    expected = statement(target + " = None").targets[0]
    return only((item for item in body if isinstance(item, ast.For)
                 and same_ast(item.target, expected)), f"loop over {target}")


def top_value(node, name):
    item = only((item for item in node.body if isinstance(item, ast.Assign)
                 and len(item.targets) == 1 and isinstance(item.targets[0], ast.Name)
                 and item.targets[0].id == name), f"assignment {name}")
    return item.value


def exact_call(node, name, bus=False):
    return (isinstance(node, ast.Expr) and isinstance(node.value, ast.Call)
            and isinstance(node.value.func, ast.Name) and node.value.func.id == name
            and not node.value.keywords
            and ((len(node.value.args) == 1 and isinstance(node.value.args[0], ast.Name)
                  and node.value.args[0].id == "bus") if bus else not node.value.args))


def project_tail_call(node, name, bus=False):
    projected = copy.deepcopy(node)
    owner = outer_try(projected) if isinstance(projected, ast.FunctionDef) else projected
    if owner.body and exact_call(owner.body[-1], name, bus):
        owner.body.pop()
    return projected


def c_process_body(source):
    match = re.search(r"\bsuzaku_ibus_engine_process_key_event\s*\([^;{}]*\)\s*\{", source)
    if match is None:
        raise AssertionError("Missing native key-event function")
    start, depth = match.end(), 1
    for index in range(start, len(source)):
        depth += (source[index] == "{") - (source[index] == "}")
        if depth == 0:
            return source[start:index]
    raise AssertionError("Unterminated native key-event function")


def brace_end(source, start):
    depth = 1
    for index in range(start + 1, len(source)):
        depth += (source[index] == "{") - (source[index] == "}")
        if depth == 0:
            return index
    raise AssertionError("Unterminated C block")


class KeypadNavigationCoverageTests(unittest.TestCase):
    def assert_has_statement(self, node, text):
        expected = statement(text)
        self.assertTrue(any(same_ast(item, expected) for item in ast.walk(node)), text)

    def test_sources_compile_without_importing_native_or_physical_suites(self):
        compile(NATIVE, str(NATIVE_PATH), "exec")
        compile(KEYBOARD, str(KEYBOARD_PATH), "exec")

    def test_old_row_boundary_matrix_only_gets_one_exact_tail_call(self):
        node = definition(NATIVE, "check_candidate_row_boundaries")
        self.assertEqual(digest(project_tail_call(node, "check_keypad_navigation", bus=True)),
                         ROW_BOUNDARY_HASH)
        self.assertTrue(exact_call(outer_try(node).body[-1], "check_keypad_navigation", bus=True))
        calls = [call for call in ast.walk(node) if isinstance(call, ast.Call)
                 and isinstance(call.func, ast.Name) and call.func.id == "check_keypad_navigation"]
        self.assertEqual(len(calls), 1)

    def test_old_49_and_custom_7_keyboard_matrices_only_get_one_exact_tail_call(self):
        owner = outer_try(KEYBOARD)
        self.assertEqual(digest(project_tail_call(owner, "run_keypad_navigation")), KEYBOARD_TRY_HASH)
        self.assertTrue(exact_call(owner.body[-1], "run_keypad_navigation"))
        self.assertEqual(digest(definition(KEYBOARD, "Keyboard")), KEYBOARD_CLASS_HASH)
        calls = [call for call in ast.walk(owner) if isinstance(call, ast.Call)
                 and isinstance(call.func, ast.Name) and call.func.id == "run_keypad_navigation"]
        self.assertEqual(len(calls), 1)

    def test_whole_gtk_source_only_adds_the_new_helper_and_exact_tail_call(self):
        helper = definition(KEYBOARD, "run_keypad_navigation")
        self.assertFalse(helper.decorator_list)
        call = outer_try(KEYBOARD).body[-1]
        self.assertTrue(exact_call(call, "run_keypad_navigation"))
        lines = KEYBOARD_SOURCE.splitlines(keepends=True)
        self.assertEqual(lines[helper.end_lineno:helper.end_lineno + 2], ["\n", "\n"])
        self.assertEqual(lines[call.lineno - 1:call.end_lineno], ["    run_keypad_navigation()\n"])
        # Delete only the new top-level helper with its following two blank
        # lines and the sole exact outer-try call. Every previous byte remains,
        # including globals, custom-7 checks, all helpers and finally cleanup.
        projected = "".join(line for index, line in enumerate(lines)
                            if not helper.lineno - 1 <= index < helper.end_lineno + 2
                            and not call.lineno - 1 <= index < call.end_lineno)
        self.assertEqual(hashlib.sha256(projected.encode()).hexdigest(), KEYBOARD_BEFORE_HASH)

    def test_old_multilingual_hold_page_and_shared_helpers_remain_unchanged(self):
        for name, expected in UNCHANGED_NATIVE_HASHES.items():
            with self.subTest(name=name):
                self.assertEqual(digest(definition(NATIVE, name)), expected)

    def test_top_row_digit_selection_and_unicode_keypad_digit_path_are_unchanged(self):
        body = c_process_body(BRIDGE)
        start = body.index("    if (self->input->len > 0 && suzaku_host_ime_candidate_count() > 0 &&\n")
        end = body.index("\n    gunichar character", start)
        self.assertEqual(hashlib.sha256(body[start:end].encode()).hexdigest(), DIGIT_REGION_HASH)
        self.assertIn("gunichar character = ibus_keyval_to_unicode(keyval);", body)
        self.assertIn("if (self->input->len == 0 && g_unichar_isdigit(character)) { return FALSE; }", body)

    def test_only_six_keypad_aliases_dispatch_after_compose(self):
        body = c_process_body(BRIDGE)
        compose = body.index("if (suzaku_ibus_engine_process_compose(self, keyval))")
        compose_end = brace_end(body, body.index("{", compose))
        aliases = re.findall(r"\bIBUS_KEY_(KP_(?:Up|Down|Left|Right|Prior|Next))\b", body)
        self.assertEqual(set(aliases), set(KEYVALS))
        for key in KEYVALS:
            self.assertGreater(body.index("IBUS_KEY_" + key), compose_end)
        self.assertEqual(set(re.findall(r"\bIBUS_KEY_(KP_\w+)\b", body)),
                         set(KEYVALS) | {"KP_Enter"})
        self.assertNotRegex(body[:compose], r"keyval\s*=\s*IBUS_KEY_(Up|Down|Left|Right|Page_Up|Page_Down)")

    def test_c_projection_only_removes_the_six_bounded_aliases(self):
        projected = BRIDGE
        # A later, separately tested status-only refresh must not invalidate
        # this historical six-alias projection. Normalize only its exact block;
        # every original byte and the old baseline digest remain protected.
        old_feedback = ("    if (self->language_feedback != NULL && self->language_job == NULL) {\n"
                        "        g_clear_pointer(&self->language_feedback, g_free);\n"
                        "        suzaku_ibus_engine_render(self);\n"
                        "    }\n")
        new_feedback = old_feedback.replace("suzaku_ibus_engine_render(self);",
                                            "suzaku_ibus_engine_render_presentation(self);")
        self.assertEqual(projected.count(new_feedback), 1)
        self.assertEqual(projected.count(old_feedback), 0)
        projected = projected.replace(new_feedback, old_feedback, 1)
        for added, original in C_ALIAS_ADDITIONS:
            self.assertEqual(projected.count(added), 1)
            projected = projected.replace(added, original, 1)
        self.assertEqual(hashlib.sha256(projected.encode()).hexdigest(), BRIDGE_BEFORE_HASH)

    def test_native_matrix_is_six_fixed_keyvals_two_profiles_and_two_languages(self):
        node = definition(NATIVE, "check_keypad_navigation")
        self.assertTrue(same_ast(top_value(node, "cases"), ast.parse(NATIVE_CASES, mode="eval").body))
        profile = direct_loop(outer_try(node).body, "profile")
        self.assertEqual(ast.literal_eval(profile.iter), ["standard", "home-row"])
        language = direct_loop(profile.body, "language, seed, word")
        self.assertEqual(ast.literal_eval(language.iter), LANGUAGES)
        case = direct_loop(language.body, "label, key, kind, forward")
        self.assertEqual(ast.unparse(case.iter), "cases")
        self.assert_has_statement(node, "assert checked == 24")
        self.assertEqual(sum(isinstance(item, ast.AugAssign) and ast.unparse(item) == "checked += 1"
                             for item in ast.walk(node)), 1)

    def test_native_boundary_checks_observe_fresh_snapshots_without_adoption(self):
        node = definition(NATIVE, "check_keypad_navigation")
        unchanged = definition(ast.Module(body=node.body, type_ignores=[]), "unchanged")
        for text in ["baseline = watch.latest", "current = Watch()",
                     "assert current.latest == baseline, (key, baseline, current.latest)",
                     "assert not peer.commits"]:
            self.assert_has_statement(unchanged, text)
        for text in [
            'assert not peer.process_key_event(key), "empty keypad navigation belongs to the app"',
            'assert total > 6, "keypad page fixture must retain multiple pages"',
            'unchanged(peer, key)',
            'assert peer.process_key_event(IBus.KEY_BackSpace)',
            'wait(lambda: watch.latest["seed"] == seed, "keypad boundary preserves adoption undo")',
            'assert not peer.process_key_event(key, 80, mask)',
            'assert watch.latest == baseline and not peer.commits',
        ]:
            self.assert_has_statement(node, text)
        modifiers = only((item for item in ast.walk(node) if isinstance(item, ast.For)
                          and ast.unparse(item.target) == "mask"), "keypad application modifier loop")
        self.assertEqual(ast.unparse(modifiers.iter),
                         "[IBus.ModifierType.CONTROL_MASK, IBus.ModifierType.MOD1_MASK, "
                         "IBus.ModifierType.SUPER_MASK]")

    def test_gtk_matrix_adds_four_physical_flows_without_changing_old_totals(self):
        node = definition(KEYBOARD, "run_keypad_navigation")
        self.assertEqual(ast.literal_eval(top_value(node, "cases")), LANGUAGES)
        language = direct_loop(outer_try(node).body, "language, seed, word")
        self.assertEqual(ast.unparse(language.iter), "cases")
        mode = direct_loop(language.body, "mode")
        self.assertEqual(ast.literal_eval(mode.iter), ["rows", "pages"])
        for text in ["keyboard.locks(num=False)", "keyboard.type(seed)",
                     "initial, keymap = keyboard.state(), keyboard.map()",
                     "assert new_checked == 4, new_checked",
                     "assert passed == initial_passed + 4, passed"]:
            self.assert_has_statement(node, text)
        physical = {item.value for item in ast.walk(node) if isinstance(item, ast.Constant)
                    and isinstance(item.value, str) and re.fullmatch(r"KP_\d", item.value)}
        self.assertEqual(physical, {"KP_2", "KP_4", "KP_6", "KP_8", "KP_3", "KP_9"})
        self.assertFalse(any(isinstance(item, ast.Attribute) and item.attr == "process_key_event"
                             or isinstance(item, ast.Constant) and item.value == "ProcessKeyEvent"
                             for item in ast.walk(node)))

    def test_gtk_movement_and_boundaries_keep_real_editor_focus_and_empty_buffer(self):
        node = definition(KEYBOARD, "run_keypad_navigation")
        for name in ["unchanged", "move"]:
            helper = definition(ast.Module(body=node.body, type_ignores=[]), name)
            self.assert_has_statement(helper, "keyboard.press(key)")
            self.assert_has_statement(helper, "baseline = fresh_snapshot()")
            self.assert_has_statement(helper, "current = fresh_snapshot()")
            self.assert_has_statement(helper, "assert x.focused() == window and editor.poll() is None")
        self.assert_has_statement(node, "assert current == baseline, (label, baseline, current)")
        for text in ['qa.save_document(x, document, "")', 'qa.choose_number(x, word)',
                     'keyboard.press("BackSpace")', 'expect(seed)', 'keyboard.press(" ")',
                     'expect(word + " ")']:
            self.assert_has_statement(node, text)
        self.assert_has_statement(node,
            'finish(word + " ", f"{language} NumLock-off keypad {mode}: '
            'boundaries/moves/undo/Space/single commit", initial, keymap)')

    def test_new_helpers_keep_existing_wait_limits_and_use_no_draft_injection(self):
        for tree, name in [(NATIVE, "check_keypad_navigation"), (KEYBOARD, "run_keypad_navigation")]:
            with self.subTest(name=name):
                node = definition(tree, name)
                for call in (item for item in ast.walk(node) if isinstance(item, ast.Call)):
                    if isinstance(call.func, ast.Name) and call.func.id == "action":
                        self.fail("New keypad fixtures must navigate with actual key events")
                    if (isinstance(call.func, ast.Name) and call.func.id == "wait"
                            or isinstance(call.func, ast.Attribute) and call.func.attr == "wait"):
                        self.assertEqual(len(call.args), 2)
                        self.assertFalse(call.keywords)


if __name__ == "__main__":
    unittest.main()
