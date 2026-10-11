#!/usr/bin/env python3
"""Lightweight fixture-shape checks; not native shortcut acceptance."""
import ast
import json
from pathlib import Path
import unittest


SOURCE = Path(__file__).with_name("test-linux-keyboard.py").read_text()
TREE = ast.parse(SOURCE)


class CustomKeyboardScopeTests(unittest.TestCase):
    def test_syntax_and_mutually_exclusive_scope(self):
        compile(TREE, "test-linux-keyboard.py", "exec")
        branch = next(node for node in ast.walk(TREE)
                      if isinstance(node, ast.If)
                      and ast.unparse(node.test) == "custom_shortcuts_mode == '1'")
        self.assertEqual([ast.unparse(node) for node in branch.body],
                         ["run_custom_shortcuts()", "sys.exit(0)"])
        owner = next(node for node in TREE.body if isinstance(node, ast.Try))
        self.assertIn(branch, owner.body)
        self.assertEqual(ast.unparse(owner.finalbody[0]), "qa.close()")
        self.assertIn('assert custom_shortcuts_mode in ("0", "1")', SOURCE)

    def test_default_legacy_count_and_deadlines_remain_separate(self):
        self.assertIn('assert passed == 49, passed', SOURCE)
        self.assertIn('RESULT: 49 strict physical-keyboard workflow groups passed', SOURCE)
        self.assertIn('assert passed == 7, passed', SOURCE)
        self.assertNotIn('timeout=', ast.get_source_segment(
            SOURCE, next(node for node in TREE.body
                         if isinstance(node, ast.FunctionDef) and node.name == "run_custom_shortcuts")))
        # The sole explicit startup limit is the existing companion-render limit,
        # not a relaxed key/navigation/control timeout.
        self.assertEqual(SOURCE.count("timeout=30"), 1)

    def test_six_binding_request_exercises_extended_control_limit(self):
        assignment = next(node for node in TREE.body if isinstance(node, ast.Assign)
                          and any(isinstance(target, ast.Name) and target.id == "CUSTOM_BINDINGS"
                                  for target in node.targets))
        bindings = ast.literal_eval(assignment.value)
        self.assertEqual(set(bindings), {"previous", "next", "previous-page", "next-page",
                                        "adopt", "switch-language"})
        self.assertEqual(len(set(bindings.values())), 6)
        payload = "U" + json.dumps({"llm_enabled": False, "shortcut_profile": "standard",
                                    "shortcut_bindings": bindings}, separators=(",", ":"))
        self.assertGreater(len(payload.encode()), 128)
        self.assertLessEqual(len(payload.encode()), 1024)

    def test_real_keys_and_persisted_ack_not_engine_method_injection(self):
        self.assertFalse(any(isinstance(node, ast.Constant) and node.value == "ProcessKeyEvent"
                             or isinstance(node, ast.Attribute) and node.attr == "ProcessKeyEvent"
                             for node in ast.walk(TREE)))
        for text in ['keyboard.press("a", "Alt_L", "Shift_L", duration=.65',
                     'release_modifiers_after=.1', 'keyboard.press("BackSpace")',
                     'keyboard.press(" ")', 'qa.save_document(x, document, "")',
                     '(qa.root / "ime.json").read_bytes()',
                     'keyboard.press("l", "Control_L", "Shift_L")']:
            self.assertIn(text, SOURCE)

    def test_installed_settings_recording_uses_own_observed_hit_targets(self):
        recorder = next(node for node in TREE.body
                        if isinstance(node, ast.FunctionDef) and node.name == "custom_panel_recording")
        recorder_source = ast.get_source_segment(SOURCE, recorder)
        for text in ['qa.spawn([str(qa.bins / "panel")])',
                     'frame(surface)).get("viewport_settled") is True',
                     'x.focused_within(xid)', 'Map State: IsViewable',
                     'click("RecordShortcut(Previous)")',
                     'keyboard.press("g", "Control_L", "Shift_L", duration=.65',
                     'click("ClearShortcutBinding(Previous)")']:
            self.assertIn(text, recorder_source)


if __name__ == "__main__":
    unittest.main()
