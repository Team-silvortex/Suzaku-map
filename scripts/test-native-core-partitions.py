#!/usr/bin/env python3
"""Source/dispatch guards for fresh native core sessions; no bus, build or model.

Read actual routing and execute only the runner's shell functions with stubs.
Exact historical projections retain every original physical assertion.
"""
import ast
import hashlib
import os
from pathlib import Path
import runpy
import subprocess
import sys
import unittest

sys.dont_write_bytecode = True
from fixtures.native_partition_projection import (
    MODEL_DISPATCH_GUARD, historical_source, project_guard, project_native, project_runner,
)

ROOT = Path(__file__).resolve().parent.parent
STABLE_AST_DUMP = runpy.run_path(str(ROOT / "scripts/run-ast-guard.py"))["stable_ast_dump"]
CORE = "SUZAKU_NATIVE_CORE_ONLY"
PART = "SUZAKU_NATIVE_CORE_PART"
SCOPES = ("core-lifecycle", "core-composition", "recovery", "fallback")
FLAGS = (
    "SUZAKU_NATIVE_PACKS_ONLY", "SUZAKU_NATIVE_PRESENTATION_ONLY",
    "SUZAKU_NATIVE_LANGUAGE_ONLY", "SUZAKU_NATIVE_CONTROL_IO_ONLY",
    "SUZAKU_NATIVE_RECOVERY_ONLY", "SUZAKU_NATIVE_JAPANESE_ONLY",
    "SUZAKU_NATIVE_DRAFT_LIMITS_ONLY", "SUZAKU_NATIVE_ROW_BOUNDARIES_ONLY",
    "SUZAKU_NATIVE_SCREEN_KEYS_ONLY", "SUZAKU_NATIVE_COMPOSE_UNDO_ONLY",
    "SUZAKU_NATIVE_BILINGUAL_ONLY", "SUZAKU_NATIVE_FALLBACK_ONLY",
    "SUZAKU_NATIVE_SHORTCUTS_ONLY", "SUZAKU_NATIVE_CURSOR_ONLY",
    "SUZAKU_NATIVE_ACTIVATION_ONLY", CORE,
)
LIFECYCLE = (
    "check_native_presentation", "check_tray_activation", "check_language_shortcut",
    "check_held_language_transitions", "check_custom_language_long_workflows",
    "check_post_process_preedit", "check_candidate_page_boundaries",
    "check_candidate_row_boundaries", "check_screen_keys", "check_english_prose_workflows",
    "check_japanese_daily_workflows", "check_engine_event_boundaries",
    "check_prediction_cancellation", "check_native_draft_limits",
    "check_prediction_length_boundaries", "check_commit_target_boundaries",
    "check_cancel_target_boundaries", "check_draft_end_prediction_boundaries",
    "check_language_target_boundaries", "check_content_type_commit_boundaries",
    "check_content_type_prediction_boundaries", "check_engine_destruction",
    "check_engine_destruction_prediction", "check_committed_model_context",
)
NATIVE_HASH = "79f48467d0ef49a9610bbc542e6b4dc5631fcd8599163917eb4496cb890d3c77"
RUNNER_HASH = "521c5da0c7cc6a586e64fdb7fd57312a1d534363a3b9ccce376fbc57b748adbb"
GUARD_HASHES = {
    "test-native-model-fixture.py":
        "046672f04f97a65ca5cd3039dddcb31a8a248c55a87cdf02faa9585efc4fc26d",
    "test-held-language-coverage.py":
        "d48d5134d13e0efc3e394c92b20d3c92204dd0ada4d212c9e3ab3c128b613638",
    "test-custom-language-coverage.py":
        "fd165452814115d0b775644b074e26394f4f0b27f0f777e8fe7e69f0f415dd2d",
    "test-custom-boundary-coverage.py":
        "a6cdf655ece5ed6c3f058c0e593a1e4b0ba4f3d03e6a0ff842cc10cb3208f15c",
    "test-custom-continuation-coverage.py":
        "dce6bd9101f9432e120c554955305ff59f50620d2cf06472f08f1043109788e2",
    "test-custom-long-coverage.py":
        "34de9a5c52a6f8fa4bb6ddd6a8b59fec31feb9f00e549ee6958a5cdec9fb9ba8",
}


def digest(source):
    return hashlib.sha256(source.encode()).hexdigest()


def shape(node):
    return STABLE_AST_DUMP(node)


def call_name(node):
    if isinstance(node, ast.Expr) and isinstance(node.value, ast.Call):
        if isinstance(node.value.func, ast.Name):
            return node.value.func.id
    return None


class NativeCorePartitionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # Actual, unprojected routing is mandatory for every dispatch test.
        cls.native = (ROOT / "scripts/test-native-sync.py").read_text()
        cls.tree = ast.parse(cls.native)
        cls.runner = (ROOT / "scripts/test-linux-ci.sh").read_text()

    def selectors(self):
        names = {"NATIVE_ONLY_FLAGS", "native_only_mode", "native_core_part"}
        nodes = [
            node for node in self.tree.body
            if (isinstance(node, ast.Assign) and any(
                isinstance(target, ast.Name) and target.id in names for target in node.targets))
            or (isinstance(node, ast.FunctionDef) and node.name in names)
        ]
        self.assertEqual(len(nodes), 3, "missing independent core-part selector")
        namespace = {}
        exec(compile(ast.Module(body=nodes, type_ignores=[]), "core-selectors", "exec"), namespace)
        self.assertEqual(namespace["NATIVE_ONLY_FLAGS"], FLAGS)
        return namespace["native_only_mode"], namespace["native_core_part"]

    def shell_function(self, name):
        start_marker = f"{name}() {{\n"
        self.assertEqual(self.runner.count(start_marker), 1)
        start = self.runner.index(start_marker)
        end = self.runner.index("\n}\n", start) + len("\n}\n")
        return self.runner[start:end]

    def dispatch(self, updates=None, execute=False):
        environment = dict(os.environ)
        for name in (*FLAGS, PART, "FAIL_NATIVE_PARTITION", "SUZAKU_IBUS_INLINE_PREEDIT"):
            environment.pop(name, None)
        environment.update(updates or {})
        source = "set -euo pipefail\n" + self.shell_function("suzaku_ci_select_native_partitions")
        if execute:
            source += self.shell_function("suzaku_ci_run_native_partitions")
            source += r'''
suzaku_ci_native_bins=/synthetic/bin
suzaku_ci_panel_test=/synthetic/panel-tests
suzaku_ci_script_dir=/synthetic/scripts
suzaku_ci_tmp=""
cleanup() { printf 'CLEAN:%s\n' "$suzaku_ci_tmp"; }
mktemp() {
  case "$suzaku_ci_native_partition" in
    core-lifecycle) printf '/tmp/suzaku-sync-qa.AAA001\n' ;;
    core-composition) printf '/tmp/suzaku-sync-qa.AAA002\n' ;;
    recovery) printf '/tmp/suzaku-sync-qa.AAA003\n' ;;
    fallback) printf '/tmp/suzaku-sync-qa.AAA004\n' ;;
    focused) printf '/tmp/suzaku-sync-qa.AAA005\n' ;;
    *) printf 'unexpected scope\n' >&2; return 81 ;;
  esac
}
install() { printf 'INSTALL:%s\n' "$*"; }
timeout() {
  printf 'RUN:%s:inline=%s\n' "$suzaku_ci_native_partition" "${SUZAKU_IBUS_INLINE_PREEDIT:-0}"
  printf 'ARGS:%s\n' "$*"
  if [[ ${FAIL_NATIVE_PARTITION:-} == "$suzaku_ci_native_partition" ]]; then return 42; fi
}
suzaku_ci_select_native_partitions
suzaku_ci_run_native_partitions
'''
        else:
            source += 'suzaku_ci_select_native_partitions\nprintf "%s\\n" "${suzaku_ci_native_partitions[@]}"\n'
        return subprocess.run(["bash", "-c", source], env=environment, capture_output=True,
                              text=True, timeout=5)

    def test_default_has_four_required_sessions_with_exact_bounded_fresh_dispatch(self):
        selected = self.dispatch()
        self.assertEqual(selected.returncode, 0, selected.stderr)
        self.assertEqual(selected.stdout.splitlines(), list(SCOPES))
        for inline in ("0", "1"):
            with self.subTest(inline=inline):
                result = self.dispatch({"SUZAKU_IBUS_INLINE_PREEDIT": inline}, execute=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                runs = [line for line in result.stdout.splitlines() if line.startswith("RUN:")]
                self.assertEqual(runs, [f"RUN:{scope}:inline={inline}" for scope in SCOPES])
                arguments = [line for line in result.stdout.splitlines() if line.startswith("ARGS:")]
                self.assertEqual(len(arguments), 4)
                for number, (scope, line) in enumerate(zip(SCOPES, arguments), 1):
                    temporary = f"/tmp/suzaku-sync-qa.AAA00{number}"
                    for token in (
                            "--kill-after=3s 120s", "dbus-run-session -- env",
                            "-u DISPLAY", "-u WAYLAND_DISPLAY", "-u SUZAKU_LEXICON_DIR",
                            f"XDG_RUNTIME_DIR={temporary}", f"XDG_CONFIG_HOME={temporary}/config",
                            f"XDG_DATA_HOME={temporary}/data",
                            f"SUZAKU_IME_CONFIG={temporary}/ime.json",
                            f"SUZAKU_LINUX_IME_SOCKET={temporary}/suzaku-ime/host.sock",
                            f"IBUS_ADDRESS=unix:path={temporary}/ibus.sock",
                            f"XCOMPOSEFILE={temporary}/compose.XCompose",
                            "SUZAKU_NATIVE_QA_BIN_DIR=/synthetic/bin",
                            "SUZAKU_NATIVE_ACTIVATION_TEST=/synthetic/panel-tests",
                            "/usr/bin/python3 -u scripts/test-native-sync.py"):
                        self.assertIn(token, line, scope)
                    if scope.startswith("core-"):
                        self.assertIn("SUZAKU_NATIVE_CORE_ONLY=1", line)
                        self.assertIn("SUZAKU_NATIVE_CORE_PART=" + scope.removeprefix("core-"), line)
                    else:
                        flag = "RECOVERY" if scope == "recovery" else "FALLBACK"
                        self.assertIn(f"SUZAKU_NATIVE_{flag}_ONLY=1", line)
                        self.assertNotIn("SUZAKU_NATIVE_CORE_PART=", line)
                    self.assertIn(
                        f"INSTALL:-m 600 /synthetic/scripts/fixtures/compose.XCompose {temporary}/compose.XCompose",
                        result.stdout)
                    self.assertIn(f"CLEAN:{temporary}", result.stdout)
                self.assertEqual(result.stdout.count("PASS: complete native IBus gate"), 1)

    def test_all_original_focused_modes_and_explicit_core_parts_remain_one_session(self):
        only, part = self.selectors()
        for flag in FLAGS:
            with self.subTest(flag=flag):
                environment = {flag: "1"}
                self.assertEqual(only(environment), flag)
                self.assertIsNone(part(environment, flag))
                result = self.dispatch(environment, execute=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout.count("RUN:"), 1)
                self.assertIn("RUN:focused:", result.stdout)
                self.assertNotIn("PASS: complete native IBus gate", result.stdout)
        for subpart in ("lifecycle", "composition"):
            environment = {CORE: "1", PART: subpart}
            self.assertEqual(part(environment, only(environment)), subpart)
            result = self.dispatch(environment, execute=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout.count("RUN:"), 1)
            self.assertNotIn("PASS: complete native IBus gate", result.stdout)

    def test_failure_in_any_required_session_stops_dispatch_and_never_claims_completion(self):
        for index, scope in enumerate(SCOPES):
            with self.subTest(scope=scope):
                result = self.dispatch({"FAIL_NATIVE_PARTITION": scope}, execute=True)
                self.assertEqual(result.returncode, 42, result.stderr)
                runs = [line.split(":")[1] for line in result.stdout.splitlines()
                        if line.startswith("RUN:")]
                self.assertEqual(runs, list(SCOPES[:index + 1]))
                self.assertEqual(result.stdout.count("CLEAN:"), index)
                self.assertNotIn("PASS: complete native IBus gate", result.stdout)

    def test_core_part_selector_rejects_empty_unknown_unfocused_and_conflicting_requests(self):
        only, part = self.selectors()
        self.assertIsNone(part({}, None))
        self.assertIsNone(part({CORE: "1"}, CORE))
        invalid = [{CORE: "1", PART: value} for value in ("", "unknown", "Lifecycle", "lifecycle ")]
        invalid += [{PART: value} for value in ("lifecycle", "composition")]
        invalid += [{flag: "1", PART: "lifecycle"} for flag in FLAGS if flag != CORE]
        invalid += [{CORE: "1", flag: "1", PART: "composition"} for flag in FLAGS if flag != CORE]
        for environment in invalid:
            with self.subTest(environment=environment):
                with self.assertRaises(ValueError):
                    part(environment, only(environment))
                result = self.dispatch(environment, execute=True)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertEqual(result.stdout, "")
        self.assertIsNone(only({CORE: "0"}))
        with self.assertRaises(ValueError):
            part({CORE: "0", PART: "lifecycle"}, None)
        self.assertEqual(self.dispatch({CORE: "0", PART: "lifecycle"}).returncode, 2)

    def test_selection_rejection_precedes_any_build_runtime_model_or_bus(self):
        call = "  suzaku_ci_select_native_partitions\n"
        self.assertLess(self.runner.rindex(call), self.runner.index("    cargo build"))
        assignments = {
            target.id: index
            for index, node in enumerate(self.tree.body) if isinstance(node, ast.Assign)
            for target in node.targets if isinstance(target, ast.Name)
        }
        self.assertLess(assignments["native_only_flag"], assignments["native_core_partition"])
        self.assertLess(assignments["native_core_partition"], assignments["runtime"])
        self.assertLess(assignments["native_core_partition"], assignments["model"])
        assignment = self.tree.body[assignments["native_core_partition"]]
        self.assertEqual(shape(assignment), shape(ast.parse(
            "native_core_partition = native_core_part(os.environ, native_only_flag)").body[0]))
        self.assertIn("timeout --kill-after=3s 120s", self.shell_function("suzaku_ci_run_native_partitions"))

    def test_lifecycle_composition_and_recovery_route_exact_original_workflows(self):
        body = next(node.body for node in reversed(self.tree.body) if isinstance(node, ast.Try))
        routed = [(index, node) for index, node in enumerate(body) if isinstance(node, ast.If)
                  and ast.unparse(node.test) == "native_core_partition != 'composition'"]
        self.assertEqual(len(routed), 2)
        position, lifecycle = routed[0]
        self.assertEqual(tuple(call_name(node) for node in lifecycle.body), LIFECYCLE)
        self.assertEqual(lifecycle.orelse, [])
        early_exit = body[position + 1]
        self.assertEqual(shape(early_exit), shape(ast.parse(
            'if native_core_partition == "lifecycle":\n    raise SystemExit(0)').body[0]))
        self.assertEqual(shape(body[position + 2]), shape(ast.parse(
            'context = create_context(bus, "suzaku-sync-qa-a")').body[0]))
        recovery_position, recovery = routed[1]
        self.assertEqual([call_name(node) for node in recovery.body],
                         ["check_password_probe_cleanup", "check_engine_recovery"])
        self.assertEqual(recovery.orelse, [])
        self.assertEqual(recovery_position, len(body) - 2)
        for value, expected in ((None, True), ("lifecycle", True), ("composition", False)):
            self.assertEqual(eval(compile(ast.Expression(lifecycle.test), "lifecycle", "eval"),
                                  {"native_core_partition": value}), expected)
            self.assertEqual(eval(compile(ast.Expression(recovery.test), "recovery", "eval"),
                                  {"native_core_partition": value}), expected)
        focused = next(node for node in body if isinstance(node, ast.If)
                       and ast.unparse(node.test) == "os.environ.get('SUZAKU_NATIVE_RECOVERY_ONLY') == '1'")
        self.assertEqual([call_name(node) for node in focused.body[:2]],
                         ["check_password_probe_cleanup", "check_engine_recovery"])
        self.assertEqual(shape(focused.body[-1]), shape(ast.parse("raise SystemExit(0)").body[0]))
        for subpart, expected_calls in (("lifecycle", LIFECYCLE), ("composition", ())):
            calls = []
            namespace = {"native_core_partition": subpart, "bus": object(),
                         "host_environment": {}}
            for name in LIFECYCLE:
                namespace[name] = lambda *args, name=name: calls.append(name)
            fragment = ast.Module(body=[lifecycle, early_exit], type_ignores=[])
            if subpart == "lifecycle":
                with self.assertRaises(SystemExit) as raised:
                    exec(compile(fragment, "lifecycle-dispatch", "exec"), namespace)
                self.assertEqual(raised.exception.code, 0)
            else:
                exec(compile(fragment, "composition-dispatch", "exec"), namespace)
            self.assertEqual(tuple(calls), expected_calls)

    def test_all_prior_native_runner_guard_bytes_and_53_fallback_pairs_are_preserved(self):
        self.assertEqual(digest(project_native(self.native)), NATIVE_HASH)
        self.assertEqual(digest(project_runner(self.runner)), RUNNER_HASH)
        for name, fingerprint in GUARD_HASHES.items():
            with self.subTest(guard=name):
                path = ROOT / "scripts" / name
                self.assertEqual(digest(project_guard(path.read_text(), name)), fingerprint)
                self.assertEqual(digest(historical_source(path)), fingerprint)
        self.assertEqual(digest(historical_source(ROOT / "scripts/test-native-sync.py")), NATIVE_HASH)
        self.assertEqual(digest(historical_source(ROOT / "scripts/test-linux-ci.sh")), RUNNER_HASH)
        fallback = next(node for node in self.tree.body if isinstance(node, ast.FunctionDef)
                        and node.name == "check_model_unavailable_vocabulary")
        cases = next(node.value for node in fallback.body if isinstance(node, ast.Assign)
                     and any(isinstance(target, ast.Name) and target.id == "cases"
                             for target in node.targets))
        self.assertEqual(len(ast.literal_eval(cases)), 53)
        self.assertEqual(hashlib.sha256(shape(fallback).encode()).hexdigest(),
                         "dcfe815c2913307b742d517893b03818f5f52d0d4c1541e58973b41f1461f1f7")

    def test_historical_projection_requires_the_new_ci_guard_entry(self):
        name = "test-native-model-fixture.py"
        guard = (ROOT / "scripts" / name).read_text()
        self.assertEqual(guard.count(MODEL_DISPATCH_GUARD), 1)
        with self.assertRaises(AssertionError):
            project_guard(guard.replace(MODEL_DISPATCH_GUARD, "", 1), name)

    def assert_integrity_rejects(self, projection, source, before, after, expected_hash):
        self.assertTrue(before in source, f"mutation marker missing: {before!r}")
        mutated = source.replace(before, after, 1)
        try:
            projected = projection(mutated)
        except AssertionError:
            return
        self.assertNotEqual(digest(projected), expected_hash, "mutation escaped historical preservation")

    def test_projection_rejects_changed_routing_skipped_calls_physical_assertions_and_deadlines(self):
        for before, after in (
                ('if native_core_partition != "composition":', 'if native_core_partition != "lifecycle":'),
                ('if native_core_partition == "lifecycle":', 'if native_core_partition == "composition":'),
                ('        check_committed_model_context(bus)\n', ''),
                ('        check_password_probe_cleanup(bus)\n', ''),
                ('part not in ("lifecycle", "composition")', 'part not in ("lifecycle", "composition", "")'),
                ('assert fresh.latest["host"] != old_host', 'assert True'),
                ('timeout=15)', 'timeout=16)')):
            with self.subTest(native=before):
                self.assert_integrity_rejects(project_native, self.native, before, after, NATIVE_HASH)
        for before, after in (
                ("core-lifecycle core-composition recovery fallback", "core-lifecycle core-composition fallback"),
                ("--kill-after=3s 120s", "--kill-after=3s 121s"),
                ("SUZAKU_NATIVE_CORE_PART=lifecycle", "SUZAKU_NATIVE_CORE_PART=composition"),
                ("XDG_CONFIG_HOME=\"$suzaku_ci_tmp/config\"", "XDG_CONFIG_HOME=/synthetic/shared-config")):
            with self.subTest(runner=before):
                self.assert_integrity_rejects(project_runner, self.runner, before, after, RUNNER_HASH)
        name = "test-native-model-fixture.py"
        guard = (ROOT / "scripts" / name).read_text()
        self.assert_integrity_rejects(lambda source: project_guard(source, name), guard,
                                     "self.assertEqual(len(cases), 53)",
                                     "self.assertEqual(len(cases), 52)", GUARD_HASHES[name])
        with self.assertRaises(AssertionError):
            project_guard(guard, "unrecognized-native-guard.py")


if __name__ == "__main__":
    unittest.main()
