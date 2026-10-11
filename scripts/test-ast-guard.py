#!/usr/bin/env python3
"""Source-only AST-format and execution-scope regressions; no native input."""
import ast
from contextlib import redirect_stderr
import copy
import hashlib
import importlib.util
import io
from pathlib import Path
import sys
import unittest
from unittest.mock import patch


SOURCE = Path(__file__).with_name("run-ast-guard.py")
SPEC = importlib.util.spec_from_file_location("suzaku_ast_guard", SOURCE)
GUARD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GUARD)


class StableAstFormatTests(unittest.TestCase):
    def test_fixed_strings_keep_the_historical_compact_format(self):
        cases = {
            "f()": "Module(body=[Expr(value=Call(func=Name(id='f', ctx=Load())))])",
            "f(None)": "Module(body=[Expr(value=Call(func=Name(id='f', ctx=Load()), args=[Constant(value=None)]))])",
            "x = []": "Module(body=[Assign(targets=[Name(id='x', ctx=Store())], value=List(ctx=Load()))])",
            "x = None": "Module(body=[Assign(targets=[Name(id='x', ctx=Store())], value=Constant(value=None))])",
            "def f():\n    pass\n": "Module(body=[FunctionDef(name='f', args=arguments(), body=[Pass()])])",
            "if ready:\n    f()\n": "Module(body=[If(test=Name(id='ready', ctx=Load()), body=[Expr(value=Call(func=Name(id='f', ctx=Load())))])])",
            "assert ready": "Module(body=[Assert(test=Name(id='ready', ctx=Load()))])",
            "raise": "Module(body=[Raise()])",
        }
        for source, expected in cases.items():
            with self.subTest(source=source):
                self.assertEqual(GUARD.stable_ast_dump(ast.parse(source)), expected)

    def test_required_none_and_literal_empty_list_are_never_dropped(self):
        self.assertEqual(GUARD.stable_ast_dump(ast.Constant(value=None)), "Constant(value=None)")
        self.assertEqual(GUARD.stable_ast_dump(ast.Constant(value=[])), "Constant(value=[])")
        self.assertEqual(GUARD.stable_ast_dump(ast.Constant(value=[None, []])),
                         "Constant(value=[None, []])")
        self.assertNotEqual(GUARD.stable_ast_dump(ast.parse("f(None)")),
                            GUARD.stable_ast_dump(ast.parse("f()")))
        self.assertNotEqual(GUARD.stable_ast_dump(ast.parse("x = []")),
                            GUARD.stable_ast_dump(ast.parse("x = None")))

    def test_literal_text_that_looks_like_an_empty_field_is_not_rewritten(self):
        value = "keywords=[], orelse=[], value=None, 界😀"
        self.assertEqual(GUARD.stable_ast_dump(ast.Constant(value=value)),
                         "Constant(value=" + repr(value) + ")")

    def test_formatter_does_not_mutate_fields_attributes_or_original_tree(self):
        tree = ast.parse("def f(x=None):\n    try:\n        assert x is None\n    finally:\n        cleanup()\n")
        original = copy.deepcopy(tree)
        before = ast.dump(tree, include_attributes=True)
        result = GUARD.stable_ast_dump(tree)
        self.assertIsInstance(result, str)
        self.assertEqual(ast.dump(tree, include_attributes=True), before)
        self.assertEqual(ast.dump(tree, include_attributes=True),
                         ast.dump(original, include_attributes=True))

    def test_unsupported_dump_options_fail_explicitly(self):
        tree = ast.parse("f()")
        for options in [{"annotate_fields": False}, {"annotate_fields": 1},
                        {"include_attributes": True}, {"include_attributes": 0},
                        {"indent": 0}, {"indent": ""}, {"indent": 4}]:
            with self.subTest(options=options), self.assertRaises(ValueError):
                GUARD.stable_ast_dump(tree, **options)
        with self.assertRaises(TypeError):
            GUARD.stable_ast_dump(tree, show_empty=True)
        with self.assertRaises(TypeError):
            GUARD.stable_ast_dump("not an AST")

    def test_fields_calls_assertions_deadlines_and_cleanup_still_change_sha(self):
        source = ('def guarded(bus):\n'
                  '    try:\n'
                  '        response = request(bus, timeout=2)\n'
                  '        assert response == "full"\n'
                  '    finally:\n'
                  '        cleanup()\n')

        def digest(text):
            return hashlib.sha256(GUARD.stable_ast_dump(ast.parse(text)).encode()).hexdigest()

        before = digest(source)
        for old, new in [("request(bus", "request(peer"),
                         ("request(bus", "other_request(bus"),
                         ('response == "full"', 'response == "partial"'),
                         ('assert response == "full"', "pass"),
                         ("timeout=2", "timeout=3"), ("cleanup()", "pass")]:
            with self.subTest(change=(old, new)):
                self.assertEqual(source.count(old), 1)
                self.assertNotEqual(digest(source.replace(old, new)), before)


class GuardExecutionTests(unittest.TestCase):
    def test_only_the_five_original_source_guards_are_allowlisted(self):
        self.assertEqual(GUARD.GUARDS, (
            "test-native-model-fixture.py", "test-language-workflow-coverage.py",
            "test-keypad-navigation-coverage.py", "test-held-language-coverage.py",
            "test-vocabulary-partitions.py",
        ))

    def test_arbitrary_paths_native_scripts_and_extra_arguments_are_rejected(self):
        names = ["test-native-sync.py", "unknown.py", "../test-native-model-fixture.py",
                 str(SOURCE.with_name(GUARD.GUARDS[0])), GUARD.GUARDS[0] + "/",
                 "fixtures/../" + GUARD.GUARDS[0], None, []]
        original_dump, original_argv = ast.dump, sys.argv
        with patch.object(GUARD.runpy, "run_path") as execute:
            for name in names:
                with self.subTest(name=name), self.assertRaises(ValueError):
                    GUARD.run_guard(name)
            for arguments in [[], ["test-native-sync.py"], [GUARD.GUARDS[0], "--verbose"]]:
                with self.subTest(arguments=arguments), redirect_stderr(io.StringIO()):
                    with self.assertRaises(SystemExit) as failure:
                        GUARD.main(arguments)
                    self.assertEqual(failure.exception.code, 2)
            execute.assert_not_called()
        self.assertIs(ast.dump, original_dump)
        self.assertIs(sys.argv, original_argv)

    def test_resolved_paths_cannot_escape_the_fixed_scripts_directory(self):
        with patch.object(GUARD.Path, "resolve", return_value=Path("/tmp/outside-guard.py")), \
                patch.object(GUARD.runpy, "run_path") as execute:
            with self.assertRaisesRegex(ValueError, "fixed scripts directory"):
                GUARD.run_guard(GUARD.GUARDS[0])
            execute.assert_not_called()

    def test_patch_is_active_only_inside_main_execution_and_then_restored(self):
        original_dump, original_argv = ast.dump, sys.argv
        observed = []

        def execute(path, *, run_name):
            self.assertEqual(run_name, "__main__")
            self.assertEqual(Path(path).parent, GUARD.SCRIPTS_DIR)
            self.assertEqual(sys.argv, [path])
            self.assertIs(ast.dump, GUARD.stable_ast_dump)
            self.assertEqual(ast.dump(ast.parse("f()")),
                             "Module(body=[Expr(value=Call(func=Name(id='f', ctx=Load())))])")
            observed.append(Path(path).name)
            return {"completed": True}

        with patch.object(GUARD.runpy, "run_path", side_effect=execute):
            for name in GUARD.GUARDS:
                self.assertEqual(GUARD.run_guard(name), {"completed": True})
                self.assertIs(ast.dump, original_dump)
                self.assertIs(sys.argv, original_argv)
        self.assertEqual(observed, list(GUARD.GUARDS))

    def test_nonzero_unittest_exit_is_propagated_and_state_is_restored(self):
        original_dump, original_argv = ast.dump, sys.argv
        with patch.object(GUARD.runpy, "run_path", side_effect=SystemExit(23)):
            with self.assertRaises(SystemExit) as failure:
                GUARD.main([GUARD.GUARDS[0]])
            self.assertEqual(failure.exception.code, 23)
        self.assertIs(ast.dump, original_dump)
        self.assertIs(sys.argv, original_argv)

    def test_exceptions_are_propagated_and_preexisting_patch_is_restored(self):
        original_argv = sys.argv

        def previous_patch(*args, **kwargs):
            return "previous AST formatter"

        with patch.object(ast, "dump", previous_patch):
            for error_type in [RuntimeError, ValueError]:
                with self.subTest(error=error_type):
                    with patch.object(GUARD.runpy, "run_path", side_effect=error_type("guard failed")):
                        with self.assertRaisesRegex(error_type, "guard failed"):
                            GUARD.main([GUARD.GUARDS[0]])
                    self.assertIs(ast.dump, previous_patch)
        self.assertIs(sys.argv, original_argv)

    def test_zero_unittest_exit_is_not_relabelled_or_swallowed(self):
        original_dump, original_argv = ast.dump, sys.argv
        with patch.object(GUARD.runpy, "run_path", side_effect=SystemExit(0)):
            with self.assertRaises(SystemExit) as success:
                GUARD.run_guard(GUARD.GUARDS[0])
            self.assertEqual(success.exception.code, 0)
        self.assertIs(ast.dump, original_dump)
        self.assertIs(sys.argv, original_argv)


if __name__ == "__main__":
    unittest.main()
