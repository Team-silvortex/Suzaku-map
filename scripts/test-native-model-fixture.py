#!/usr/bin/env python3
"""Exercise the native HTTP reply fixture without starting a model, bus or host."""
import ast
import copy
import hashlib
import io
import itertools
import json
import os
from pathlib import Path
import socket
import subprocess
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, call
from fixtures.native_partition_projection import historical_source


class NativeModelFixtureTests(unittest.TestCase):
    def setUp(self):
        source = Path(__file__).with_name("test-native-sync.py")
        fixture = next(node for node in ast.parse(source.read_text()).body
                       if isinstance(node, ast.ClassDef) and node.name == "ModelFixture")
        # Load only the handler definition. Importing the native suite would
        # require GI, start an HTTP server and later create an isolated IBus host.
        self.clock = SimpleNamespace(monotonic=Mock(return_value=100.0), sleep=Mock())
        self.select = Mock(return_value=([], [], []))
        self.requests, self.gates = [], {}
        namespace = dict(BaseHTTPRequestHandler=object, json=json, socket=socket,
                         time=self.clock, select=SimpleNamespace(select=self.select),
                         model_requests=self.requests, model_reply_gates=self.gates,
                         preference_sentence_batches={})
        exec(compile(ast.Module(body=[fixture], type_ignores=[]), str(source), "exec"), namespace)
        self.handler_type = namespace["ModelFixture"]

    def handler(self, model="synthetic-native-model", committed_context=None):
        payload = {"language": "en", "raw_composition": "hel"}
        if committed_context is not None:
            payload["committed_context"] = committed_context
        request = {"model": model, "messages": [{"content": "synthetic instruction"},
                   {"content": json.dumps(payload)}]}
        body = json.dumps(request).encode()
        handler = self.handler_type()
        handler.headers = {"Content-Length": str(len(body))}
        handler.rfile, handler.wfile = io.BytesIO(body), io.BytesIO()
        handler.connection = Mock()
        handler.send_response, handler.send_header, handler.end_headers = Mock(), Mock(), Mock()
        return handler, request

    def gate(self, model="synthetic-native-model", released=True):
        gate = {key: Mock() for key in ["received", "release", "finished"]}
        gate["release"].wait.return_value = released
        self.gates[model] = gate
        return gate

    def assert_success(self, handler, request):
        self.assertEqual(self.requests, [request])
        handler.send_response.assert_called_once_with(200)
        handler.end_headers.assert_called_once_with()
        body = handler.wfile.getvalue()
        handler.send_header.assert_has_calls([
            call("Content-Type", "application/json"), call("Content-Length", str(len(body)))])
        content = json.loads(json.loads(body)["choices"][0]["message"]["content"])
        self.assertEqual(content["candidates"], [
            {"text": text, "kind": kind} for text, kind in [
                ("helium", "word"), ("helmet", "word"), ("helix", "word"),
                ("hello world", "sentence"), ("hello everyone", "sentence"),
                ("hello internationalization compatibility verification works", "sentence")]])

    def test_ungated_reply_keeps_async_eighty_millisecond_delay(self):
        handler, request = self.handler()
        handler.do_POST()
        self.clock.sleep.assert_called_once_with(0.08)
        self.assert_success(handler, request)

    def test_ready_gate_has_no_second_artificial_delay(self):
        gate = self.gate()
        handler, request = self.handler()
        handler.do_POST()
        gate["received"].set.assert_called_once_with()
        gate["release"].wait.assert_called_once_with(timeout=0.01)
        gate["finished"].set.assert_called_once_with()
        self.select.assert_not_called()
        self.clock.sleep.assert_not_called()
        self.assertNotIn("cancelled_at", gate)
        self.assert_success(handler, request)

    def test_pending_gate_still_waits_for_explicit_release(self):
        gate = self.gate(released=False)
        gate["release"].wait.side_effect = [False, False, True]
        handler, request = self.handler()
        handler.do_POST()
        self.assertEqual(gate["release"].wait.call_args_list, [call(timeout=0.01)] * 3)
        self.assertEqual(self.select.call_args_list,
                         [call([handler.connection], [], [], 0)] * 2)
        gate["received"].set.assert_called_once_with()
        gate["finished"].set.assert_called_once_with()
        self.clock.sleep.assert_not_called()
        self.assert_success(handler, request)

    def test_exact_context_can_be_inspected_before_releasing_reply(self):
        name = "synthetic-exact-context"
        context = ("prefix" + "界😀" * 78 + " café  tail")[-160:]
        gate = self.gate(name, released=False)
        handler, request = self.handler(name, committed_context=context)

        def inspect_pending_payload(*, timeout):
            self.assertEqual(timeout, 0.01)
            gate["received"].set.assert_called_once_with()
            self.assertEqual(self.requests, [request])
            payload = json.loads(self.requests[0]["messages"][1]["content"])
            self.assertEqual(payload, {"language": "en", "raw_composition": "hel",
                                       "committed_context": context})
            gate["finished"].set.assert_not_called()
            handler.send_response.assert_not_called()
            self.assertEqual(handler.wfile.getvalue(), b"")
            self.assertNotIn("cancelled_at", gate)
            # Release only after inspecting the captured request, as N32 does.
            gate["release"].set()
            return True

        gate["release"].wait.side_effect = inspect_pending_payload
        handler.do_POST()
        gate["release"].set.assert_called_once_with()
        gate["finished"].set.assert_called_once_with()
        self.clock.sleep.assert_not_called()
        self.assert_success(handler, request)

    def test_closed_pending_client_still_records_cancellation_without_reply(self):
        gate = self.gate(released=False)
        handler, request = self.handler()
        self.select.return_value = ([handler.connection], [], [])
        handler.connection.recv.return_value = b""
        self.clock.monotonic.side_effect = [100.0, 100.01, 100.02]
        handler.do_POST()
        self.assertEqual(self.requests, [request])
        self.assertEqual(gate["cancelled_at"], 100.02)
        handler.connection.recv.assert_called_once_with(1, socket.MSG_PEEK)
        gate["received"].set.assert_called_once_with()
        gate["finished"].set.assert_called_once_with()
        handler.send_response.assert_not_called()
        self.assertEqual(handler.wfile.getvalue(), b"")
        self.clock.sleep.assert_not_called()

    def test_unreleased_gate_retains_its_five_second_deadline(self):
        gate = self.gate(released=False)
        handler, _ = self.handler()
        self.clock.monotonic.side_effect = [100.0, 105.0]
        with self.assertRaisesRegex(AssertionError, "synthetic model reply gate timed out"):
            handler.do_POST()
        gate["received"].set.assert_called_once_with()
        gate["finished"].set.assert_not_called()
        handler.send_response.assert_not_called()
        self.clock.sleep.assert_not_called()

    def test_unexpected_second_request_remains_an_assertion_failure(self):
        gate = self.gate(released=False)
        handler, _ = self.handler()
        self.select.return_value = ([handler.connection], [], [])
        handler.connection.recv.return_value = b"x"
        with self.assertRaisesRegex(AssertionError, "unexpected second fixture request"):
            handler.do_POST()
        self.assertNotIn("cancelled_at", gate)
        gate["finished"].set.assert_not_called()
        handler.send_response.assert_not_called()
        self.clock.sleep.assert_not_called()

    def test_fallback_error_keeps_empty_503_and_finished_acknowledgement(self):
        name = "synthetic-fallback-en-http-503"
        gate = self.gate(name)
        handler, request = self.handler(name)
        handler.do_POST()
        self.assertEqual(self.requests, [request])
        handler.send_response.assert_called_once_with(503)
        handler.send_header.assert_called_once_with("Content-Length", "0")
        handler.end_headers.assert_called_once_with()
        self.assertEqual(handler.wfile.getvalue(), b"")
        gate["received"].set.assert_called_once_with()
        gate["finished"].set.assert_called_once_with()
        self.clock.sleep.assert_not_called()

    def test_fallback_deadline_client_closure_does_not_send_503(self):
        name = "synthetic-fallback-en-timeout"
        gate = self.gate(name, released=False)
        handler, _ = self.handler(name)
        self.select.return_value = ([handler.connection], [], [])
        handler.connection.recv.return_value = b""
        handler.do_POST()
        self.assertIn("cancelled_at", gate)
        gate["finished"].set.assert_called_once_with()
        handler.send_response.assert_not_called()
        self.clock.sleep.assert_not_called()

    def test_peer_close_during_success_or_error_response_still_finishes_gate(self):
        for name in ["synthetic-native-model", "synthetic-fallback-en-http-503"]:
            for error in [BrokenPipeError, ConnectionResetError]:
                with self.subTest(model=name, error=error):
                    gate = self.gate(name)
                    handler, _ = self.handler(name)
                    handler.send_response.side_effect = error
                    handler.do_POST()
                    gate["received"].set.assert_called_once_with()
                    gate["finished"].set.assert_called_once_with()
                    self.assertEqual(handler.wfile.getvalue(), b"")
        self.clock.sleep.assert_not_called()


class NativePartitionTests(unittest.TestCase):
    """Inspect the complete gate and execute only its shell dispatch with stubs."""

    FLAGS = (
        "SUZAKU_NATIVE_PACKS_ONLY", "SUZAKU_NATIVE_PRESENTATION_ONLY",
        "SUZAKU_NATIVE_LANGUAGE_ONLY", "SUZAKU_NATIVE_CONTROL_IO_ONLY",
        "SUZAKU_NATIVE_RECOVERY_ONLY", "SUZAKU_NATIVE_JAPANESE_ONLY",
        "SUZAKU_NATIVE_DRAFT_LIMITS_ONLY", "SUZAKU_NATIVE_ROW_BOUNDARIES_ONLY",
        "SUZAKU_NATIVE_SCREEN_KEYS_ONLY", "SUZAKU_NATIVE_COMPOSE_UNDO_ONLY",
        "SUZAKU_NATIVE_BILINGUAL_ONLY", "SUZAKU_NATIVE_FALLBACK_ONLY",
        "SUZAKU_NATIVE_SHORTCUTS_ONLY", "SUZAKU_NATIVE_CURSOR_ONLY",
        "SUZAKU_NATIVE_ACTIVATION_ONLY", "SUZAKU_NATIVE_CORE_ONLY",
    )

    @classmethod
    def setUpClass(cls):
        cls.source = Path(__file__).with_name("test-native-sync.py")
        cls.tree = ast.parse(historical_source(cls.source))
        cls.runner = historical_source(Path(__file__).with_name("test-linux-ci.sh"))

    def test_core_session_dispatch_guard(self):
        result = subprocess.run(["/usr/bin/python3", "-B", str(Path(__file__).with_name(
            "test-native-core-partitions.py"))], capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def python_selector(self):
        names = {"NATIVE_ONLY_FLAGS", "native_only_mode"}
        nodes = [node for node in self.tree.body
                 if (isinstance(node, ast.Assign) and any(
                     isinstance(target, ast.Name) and target.id in names for target in node.targets))
                 or (isinstance(node, ast.FunctionDef) and node.name in names)]
        self.assertEqual(len(nodes), 2, "missing fail-closed native scope selector")
        namespace = {}
        exec(compile(ast.Module(body=nodes, type_ignores=[]), str(self.source), "exec"), namespace)
        self.assertEqual(namespace["NATIVE_ONLY_FLAGS"], self.FLAGS)
        return namespace["native_only_mode"]

    def shell_function(self, name):
        signature = f"{name}() {{\n"
        self.assertEqual(self.runner.count(signature), 1)
        start = self.runner.index(signature)
        end = self.runner.index("\n}\n", start) + len("\n}\n")
        return self.runner[start:end]

    def run_dispatch(self, updates=None, execute=False):
        environment = dict(os.environ)
        for flag in self.FLAGS:
            environment.pop(flag, None)
        environment.pop("FAIL_NATIVE_PARTITION", None)
        environment.pop("SUZAKU_IBUS_INLINE_PREEDIT", None)
        environment.update(updates or {})
        script = "set -euo pipefail\n" + self.shell_function("suzaku_ci_select_native_partitions")
        if execute:
            script += self.shell_function("suzaku_ci_run_native_partitions")
            script += r'''
suzaku_ci_native_bins=/synthetic/bin
suzaku_ci_panel_test=/synthetic/panel-tests
suzaku_ci_script_dir=/synthetic/scripts
suzaku_ci_tmp=""
cleanup() { printf 'CLEAN:%s\n' "$suzaku_ci_tmp"; }
mktemp() { printf '/tmp/suzaku-sync-qa.%s\n' "$suzaku_ci_native_partition"; }
install() { :; }
timeout() {
  printf 'RUN:%s:inline=%s\n' "$suzaku_ci_native_partition" "${SUZAKU_IBUS_INLINE_PREEDIT:-0}"
  printf 'ARGS:%s\n' "$*"
  if [[ ${FAIL_NATIVE_PARTITION:-} == "$suzaku_ci_native_partition" ]]; then return 42; fi
}
suzaku_ci_select_native_partitions
suzaku_ci_run_native_partitions
'''
        else:
            script += 'suzaku_ci_select_native_partitions\nprintf "%s\\n" "${suzaku_ci_native_partitions[@]}"\n'
        return subprocess.run(["bash", "-c", script], env=environment, capture_output=True,
                              text=True, timeout=5)

    def test_default_dispatch_requires_both_mutually_exclusive_parts(self):
        self.assertIsNone(self.python_selector()({}))
        result = self.run_dispatch()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.splitlines(), ["core", "fallback"])

    def test_every_existing_focused_mode_remains_one_session(self):
        selector = self.python_selector()
        for flag in self.FLAGS:
            with self.subTest(flag=flag):
                self.assertEqual(selector({flag: "1"}), flag)
                result = self.run_dispatch({flag: "1"})
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout.splitlines(), ["focused"])

    def test_all_conflicting_only_modes_fail_closed(self):
        selector = self.python_selector()
        for left, right in itertools.combinations(self.FLAGS, 2):
            with self.subTest(left=left, right=right):
                environment = {left: "1", right: "1"}
                with self.assertRaisesRegex(ValueError, "mutually exclusive"):
                    selector(environment)
                result = self.run_dispatch(environment)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("mutually exclusive", result.stderr)
                self.assertEqual(result.stdout, "")

    def test_inline_preedit_is_inherited_by_both_parts(self):
        result = self.run_dispatch({"SUZAKU_IBUS_INLINE_PREEDIT": "1"}, execute=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("RUN:core:inline=1", result.stdout)
        self.assertIn("RUN:fallback:inline=1", result.stdout)
        arguments = [line for line in result.stdout.splitlines() if line.startswith("ARGS:")]
        self.assertEqual(len(arguments), 2)
        for part, flag in [("core", "CORE"), ("fallback", "FALLBACK")]:
            line = next(line for line in arguments if f"SUZAKU_NATIVE_{flag}_ONLY=1" in line)
            self.assertIn("--kill-after=3s 120s", line)
            self.assertIn(f"XDG_RUNTIME_DIR=/tmp/suzaku-sync-qa.{part}", line)
            self.assertIn(f"IBUS_ADDRESS=unix:path=/tmp/suzaku-sync-qa.{part}/ibus.sock", line)
            self.assertIn(f"SUZAKU_IME_CONFIG=/tmp/suzaku-sync-qa.{part}/ime.json", line)
            self.assertIn("SUZAKU_NATIVE_QA_BIN_DIR=/synthetic/bin", line)
            self.assertIn("SUZAKU_NATIVE_ACTIVATION_TEST=/synthetic/panel-tests", line)
        self.assertEqual(result.stdout.count("CLEAN:"), 2)
        self.assertIn("PASS: complete native IBus gate; core and all HTTP fallback cases passed.",
                      result.stdout)

    def test_failure_in_either_part_prevents_complete_success(self):
        for part in ["core", "fallback"]:
            with self.subTest(part=part):
                result = self.run_dispatch({"FAIL_NATIVE_PARTITION": part}, execute=True)
                self.assertEqual(result.returncode, 42, result.stderr)
                self.assertNotIn("PASS: complete native IBus gate", result.stdout)
                if part == "core":
                    self.assertNotIn("RUN:fallback:", result.stdout)

    def test_focused_success_does_not_claim_complete_gate(self):
        for flag in ["SUZAKU_NATIVE_PACKS_ONLY", "SUZAKU_NATIVE_CONTROL_IO_ONLY",
                     "SUZAKU_NATIVE_CORE_ONLY", "SUZAKU_NATIVE_FALLBACK_ONLY"]:
            with self.subTest(flag=flag):
                result = self.run_dispatch({flag: "1"}, execute=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout.count("RUN:"), 1)
                self.assertNotIn("PASS: complete native IBus gate", result.stdout)

    def test_conflicts_are_rejected_before_any_build_or_server(self):
        call = "  suzaku_ci_select_native_partitions\n"
        self.assertIn(call, self.runner)
        self.assertLess(self.runner.rindex(call), self.runner.index("    cargo build"))
        assignments = [node for node in self.tree.body if isinstance(node, ast.Assign)]
        index = next(i for i, node in enumerate(assignments) if any(
            isinstance(target, ast.Name) and target.id == "native_only_flag" for target in node.targets))
        server = next(i for i, node in enumerate(assignments) if any(
            isinstance(target, ast.Name) and target.id == "model" for target in node.targets))
        self.assertLess(index, server)

    def test_full_fallback_function_and_all_53_pairs_are_unchanged(self):
        node = next(node for node in self.tree.body if isinstance(node, ast.FunctionDef)
                    and node.name == "check_model_unavailable_vocabulary")
        cases = ast.literal_eval(next(item.value for item in node.body if isinstance(item, ast.Assign)
                                     and any(isinstance(target, ast.Name) and target.id == "cases"
                                             for target in item.targets)))
        self.assertEqual(len(cases), 53)
        # Frozen independently from v0.8.6 before partitioning: includes every
        # assertion, the 500ms request deadline, both failure paths and cleanup.
        digest = hashlib.sha256(ast.dump(node, include_attributes=False).encode()).hexdigest()
        self.assertEqual(digest, "dcfe815c2913307b742d517893b03818f5f52d0d4c1541e58973b41f1461f1f7")

    def test_core_excludes_only_fallback_and_retains_all_later_controls(self):
        body = next(node.body for node in reversed(self.tree.body) if isinstance(node, ast.Try))
        guarded = [(index, node) for index, node in enumerate(body) if isinstance(node, ast.If)
                   and any(isinstance(item, ast.Call) and isinstance(item.func, ast.Name)
                           and item.func.id == "check_model_unavailable_vocabulary"
                           for item in ast.walk(node))]
        # The pre-existing focused fallback branch and new single-call core guard.
        self.assertEqual(len(guarded), 2)
        index, guard = guarded[-1]
        self.assertEqual(ast.dump(guard.test, include_attributes=False), ast.dump(ast.parse(
            'native_only_flag != "SUZAKU_NATIVE_CORE_ONLY"', mode="eval").body,
            include_attributes=False))
        self.assertEqual(len(guard.body), 1)
        self.assertEqual(guard.orelse, [])
        self.assertEqual(ast.dump(guard.body[0], include_attributes=False), ast.dump(ast.parse(
            "check_model_unavailable_vocabulary(other, watch, other_commits, other_lookup)").body[0],
            include_attributes=False))
        prefix = copy.deepcopy(ast.Module(body=body[:index], type_ignores=[]))
        # Retain the independently frozen pre-partition prefix. Later held-key
        # coverage may append only these exact calls immediately after the old
        # language gates; never replace the historical digest with a new one.
        appended = {"check_held_language_transitions": "check_language_shortcut",
                    "check_held_language_pending": "check_language_shortcut_slow_io",
                    "check_custom_language_long_workflows": "check_held_language_transitions",
                    "check_custom_language_pending_long": "check_held_language_pending"}
        appended |= {"check_custom_language_pending_boundaries": "check_custom_language_pending_long"}
        appended |= {"check_custom_language_pending_continuation": "check_custom_language_pending_boundaries"}
        seen = dict.fromkeys(appended, 0)
        owners = [prefix] + [node for node in ast.walk(prefix) if isinstance(node, ast.If)]
        for owner in owners:
            removed = set()
            for position, item in enumerate(owner.body):
                if not (isinstance(item, ast.Expr) and isinstance(item.value, ast.Call)
                        and isinstance(item.value.func, ast.Name)
                        and item.value.func.id in appended):
                    continue
                name = item.value.func.id
                self.assertEqual(ast.dump(item, include_attributes=False),
                                 ast.dump(ast.parse(f"{name}(bus)").body[0], include_attributes=False))
                self.assertGreater(position, 0)
                self.assertEqual(ast.dump(owner.body[position - 1], include_attributes=False),
                                 ast.dump(ast.parse(f"{appended[name]}(bus)").body[0],
                                          include_attributes=False))
                removed.add(position)
                seen[name] += 1
            owner.body = [item for position, item in enumerate(owner.body) if position not in removed]
        self.assertEqual(seen.pop("check_custom_language_pending_boundaries"), 1)
        self.assertEqual(seen.pop("check_custom_language_pending_continuation"), 1)
        custom_seen = {name: seen.pop(name) for name in ["check_custom_language_long_workflows",
                                                       "check_custom_language_pending_long"]}
        self.assertEqual(custom_seen, {"check_custom_language_long_workflows": 2,
                                       "check_custom_language_pending_long": 1})
        self.assertEqual(seen, {"check_held_language_transitions": 2, "check_held_language_pending": 1})
        digest = hashlib.sha256(ast.dump(prefix, include_attributes=False).encode()).hexdigest()
        self.assertEqual(digest, "c2ae47fbcbc30b6fa86ca5abac2ae928ca5b2bdfa7ad8824fa349e373c33672c")
        tail = ast.Module(body=body[index + 1:], type_ignores=[])
        digest = hashlib.sha256(ast.dump(tail, include_attributes=False).encode()).hexdigest()
        self.assertEqual(digest, "3ba60a2dcc432e297ce1b5fbb35dc427c01f72c7c9d45d47c15a6fa72c4f1585")


if __name__ == "__main__":
    unittest.main()
