#!/usr/bin/env python3
"""Deterministic restarted-host readiness checks; no GI, D-Bus or child process."""
import ast
import contextlib
import copy
import io
import os
from pathlib import Path
import re
import sys
import tempfile
from types import SimpleNamespace
import unittest

from fixtures.native_probe_cleanup import preserve_primary_failure


SOURCE = Path(__file__).with_name("test-preference-host.py")
SHARED = Path(__file__).with_name("test-custom-shortcuts-ibus.py")


def frame(**changes):
    result = dict(host="new-host", focused=False, private=False, seed="", candidates=[])
    result.update(changes)
    return result


class Scenario:
    def __init__(self, *, socket_at=.04, ack_at=.12, value=None,
                 engine_error=None, watch_error=None, close_error=None, death_at=None):
        self.now = 0.0
        self.socket_at, self.ack_at = socket_at, ack_at
        self.value = frame() if value is None else value
        self.engine_error, self.watch_error = engine_error, watch_error
        self.close_error, self.death_at = close_error, death_at
        self.events, self.waits, self.watchers = [], [], []
        self.watch, self.peer = None, object()
        self.socket = SimpleNamespace(exists=lambda: self.socket_at is not None and
                                      self.now >= self.socket_at)
        self.processes = [SimpleNamespace(poll=lambda: 7 if self.death_at is not None and
                                         self.now >= self.death_at else None)]
        self.ns = dict(time=SimpleNamespace(monotonic=lambda: self.now, sleep=self.sleep))
        tree = ast.parse(SOURCE.read_text())
        helper = next(node for node in tree.body if isinstance(node, ast.FunctionDef) and
                      node.name == "attach_restarted_peer")
        shared = ast.parse(SHARED.read_text())
        fixture = next(node for node in shared.body if isinstance(node, ast.ClassDef) and
                       node.name == "Fixture")
        wait = next(node for node in fixture.body if isinstance(node, ast.FunctionDef) and
                    node.name == "wait")
        exec(compile(ast.Module(body=[helper, wait], type_ignores=[]), str(SOURCE), "exec"), self.ns)
        self.qa = SimpleNamespace(Watch=self.open_watch, EnginePeer=self.create_engine,
                                  preserve_primary_failure=preserve_primary_failure)

    def sleep(self, seconds):
        self.now += seconds

    def pump(self):
        self.now += .01
        if (self.watch is not None and self.ack_at is not None and
                self.now >= self.ack_at and "ack" not in self.events):
            self.watch.latest = self.value
            self.events.append("ack")

    def wait(self, predicate, label, timeout=5):
        self.waits.append(timeout)
        return self.ns["wait"](self, predicate, label, timeout)

    def open_watch(self, fixture):
        assert fixture is self and self.socket.exists(), "subscription before socket binding"
        self.events.append("watch")
        if self.watch_error is not None:
            raise self.watch_error
        assert self.watch is None, "subscription was retried"
        self.watch = SimpleNamespace(latest=None, close=self.close_watch)
        self.watchers.append(self.watch)
        return self.watch

    def close_watch(self):
        self.events.append("close")
        self.watchers.remove(self.watch)
        if self.close_error is not None:
            raise self.close_error

    def create_engine(self, fixture):
        assert fixture is self
        self.events.append("engine")
        if self.engine_error is not None:
            raise self.engine_error
        return self.peer

    def run(self):
        return self.ns["attach_restarted_peer"](self, self.qa, "old-host")


class PreferenceRestartTests(unittest.TestCase):
    def assert_one_budget(self, case):
        self.assertEqual(case.waits, [5], "socket and snapshot must share one readiness budget")
        self.assertLess(case.now, 5.05, "readiness exceeded the existing five-second bound")

    def assert_no_engine(self, case):
        self.assertNotIn("engine", case.events)
        self.assert_one_budget(case)

    def test_delayed_socket_and_ack_precede_exactly_one_engine_creation(self):
        case = Scenario(socket_at=.05, ack_at=.16)
        watch, peer = case.run()
        self.assertEqual(case.events, ["watch", "ack", "engine"])
        self.assertIs(watch, case.watch)
        self.assertIs(peer, case.peer)
        self.assertEqual(case.watchers, [watch])
        self.assert_one_budget(case)

    def test_socket_binding_is_not_a_dispatch_acknowledgement(self):
        case = Scenario(ack_at=None)
        with self.assertRaises(AssertionError):
            case.run()
        self.assert_no_engine(case)
        self.assertEqual(case.events, ["watch", "close"])
        self.assertEqual(case.watchers, [])
        self.assertGreaterEqual(case.now, 5)

    def test_missing_socket_never_subscribes_or_creates_an_engine(self):
        case = Scenario(socket_at=None)
        with self.assertRaises(AssertionError):
            case.run()
        self.assertEqual(case.events, [])
        self.assert_no_engine(case)

    def test_late_socket_does_not_start_a_second_full_wait(self):
        case = Scenario(socket_at=4.8, ack_at=5.2)
        with self.assertRaises(AssertionError):
            case.run()
        self.assertEqual(case.events, ["watch", "close"])
        self.assert_no_engine(case)

    def test_old_host_cannot_acknowledge_the_replacement(self):
        case = Scenario(value=frame(host="old-host"))
        with self.assertRaises(AssertionError):
            case.run()
        self.assertEqual(case.events.count("close"), 1)
        self.assert_no_engine(case)

    def test_missing_or_malformed_identity_never_reaches_the_factory(self):
        values = [{}, [], "unexpected", frame(host=""), frame(host=None), frame(host=42)]
        for value in values:
            with self.subTest(value=value):
                case = Scenario(value=value)
                with self.assertRaises(Exception):
                    case.run()
                self.assert_no_engine(case)
                self.assertEqual(case.events.count("close"), 1)
                self.assertEqual(case.watchers, [])

    def test_focused_private_or_dirty_startup_is_rejected(self):
        for change in ({"focused": True}, {"private": True}, {"seed": "unexpected"},
                       {"candidates": [{"text": "unexpected"}]}):
            with self.subTest(change=change):
                case = Scenario(value=frame(**change))
                with self.assertRaises(AssertionError):
                    case.run()
                self.assert_no_engine(case)
                self.assertEqual(case.events.count("close"), 1)

    def test_absent_state_fields_are_not_equivalent_to_empty_public_state(self):
        for field in ("focused", "private", "seed", "candidates"):
            with self.subTest(field=field):
                value = frame()
                del value[field]
                case = Scenario(value=value)
                with self.assertRaises(Exception):
                    case.run()
                self.assert_no_engine(case)
                self.assertEqual(case.events.count("close"), 1)

    def test_process_death_during_ack_wait_remains_a_failure(self):
        case = Scenario(ack_at=None, death_at=.08)
        with self.assertRaisesRegex(AssertionError, "owned native process exited"):
            case.run()
        self.assert_no_engine(case)
        self.assertEqual(case.events, ["watch", "close"])
        self.assertLess(case.now, .15)

    def test_process_death_coincident_with_ack_cannot_reach_the_factory(self):
        case = Scenario(ack_at=.12, death_at=.12)
        with self.assertRaisesRegex(AssertionError, "owned native process exited"):
            case.run()
        self.assert_no_engine(case)
        self.assertEqual(case.events, ["watch", "ack", "close"])
        self.assertEqual(case.watchers, [])

    def test_engine_timeout_after_ack_is_fatal_and_never_retried(self):
        failure = TimeoutError("Factory.CreateEngine exceeded its original 2000ms")
        case = Scenario(engine_error=failure)
        with self.assertRaises(TimeoutError) as caught:
            case.run()
        self.assertIs(caught.exception, failure)
        self.assertEqual(case.events, ["watch", "ack", "engine", "close"])
        self.assertEqual(case.watchers, [])
        self.assert_one_budget(case)

    def test_watch_cleanup_failure_cannot_replace_the_engine_timeout(self):
        failure = TimeoutError("Factory.CreateEngine timeout")
        case = Scenario(engine_error=failure, close_error=RuntimeError("watch cleanup failed"))
        with self.assertRaises(TimeoutError) as caught:
            case.run()
        self.assertIs(caught.exception, failure)
        self.assertEqual(case.events, ["watch", "ack", "engine", "close"])
        self.assertTrue(any("watch cleanup failed" in note for note in failure.__notes__))
        self.assertEqual(case.watchers, [])

    def test_watch_cleanup_failure_cannot_replace_missing_ack_failure(self):
        case = Scenario(ack_at=None, close_error=RuntimeError("watch cleanup failed"))
        with self.assertRaises(AssertionError) as caught:
            case.run()
        self.assertTrue(any("watch cleanup failed" in note for note in caught.exception.__notes__))
        self.assert_no_engine(case)
        self.assertEqual(case.events.count("close"), 1)

    def test_subscription_error_is_preserved_without_create_or_retry(self):
        failure = OSError("owned socket disappeared")
        case = Scenario(watch_error=failure)
        with self.assertRaises(OSError) as caught:
            case.run()
        self.assertIs(caught.exception, failure)
        self.assertEqual(case.events, ["watch"])
        self.assertEqual(case.watchers, [])
        self.assert_no_engine(case)

    def test_existing_factory_and_session_deadlines_are_unchanged(self):
        shared = ast.parse(SHARED.read_text())
        peer = next(node for node in shared.body if isinstance(node, ast.ClassDef) and
                    node.name == "EnginePeer")
        factory_calls = [node for node in ast.walk(peer) if isinstance(node, ast.Call) and
                         any(isinstance(arg, ast.Constant) and arg.value == "CreateEngine"
                             for arg in node.args)]
        self.assertEqual(len(factory_calls), 1)
        self.assertEqual(ast.literal_eval(factory_calls[0].args[-2]), 2000)
        tree = ast.parse(SOURCE.read_text())
        runs = [node for node in ast.walk(tree) if isinstance(node, ast.Call) and
                isinstance(node.func, ast.Attribute) and node.func.attr == "run" and
                node.args and isinstance(node.args[0], ast.List) and
                any(isinstance(arg, ast.Constant) and arg.value == "dbus-run-session"
                    for arg in node.args[0].elts)]
        self.assertEqual(len(runs), 1)
        self.assertEqual([ast.literal_eval(arg) for arg in runs[0].args[0].elts[:4]],
                         ["timeout", "--kill-after=3s", "120s", "dbus-run-session"])
        self.assertEqual(next(ast.literal_eval(keyword.value) for keyword in runs[0].keywords
                              if keyword.arg == "timeout"), 125)


class PreferenceRestartDiagnosticTests(unittest.TestCase):
    def setUp(self):
        self.tree = ast.parse(SOURCE.read_text())
        helper = next(node for node in self.tree.body if isinstance(node, ast.FunctionDef) and
                      node.name == "report_failure")
        namespace = dict(os=os, sys=sys)
        exec(compile(ast.Module(body=[helper], type_ignores=[]), str(SOURCE), "exec"), namespace)
        self.report = namespace["report_failure"]
        self.fixture = SimpleNamespace(
            phase="preference restart: CreateEngine",
            processes=[SimpleNamespace(pid=101, poll=lambda: None),
                       SimpleNamespace(pid=102, poll=lambda: 7)],
            watchers=[SimpleNamespace(latest=None)],
        )

    def capture(self, logs):
        output = io.StringIO()
        with contextlib.redirect_stderr(output):
            self.report(self.fixture, logs)
        return output.getvalue()

    def test_only_last_three_host_logs_and_at_most_4096_bytes_each_are_read(self):
        with tempfile.TemporaryDirectory(prefix="suzaku-preference-diagnostic.", dir="/tmp") as owned:
            root = Path(owned)
            logs, expected = {}, {}
            for index in range(5):
                path = root / f"host-{index}.log"
                content = f"discarded-prefix-{index}\n".encode() + bytes([65 + index]) * 5000
                path.write_bytes(content)
                logs[index] = path
                expected[path.name] = content[-4096:].decode()
            output = self.capture(logs)
            self.assertNotIn("host-0.log", output)
            self.assertNotIn("host-1.log", output)
            sections = re.findall(r"Owned (host-\d+\.log) tail \((\d+) of (\d+) bytes\):\n([^\n]*)\n",
                                  output)
            self.assertEqual([name for name, *_ in sections], ["host-2.log", "host-3.log", "host-4.log"])
            for name, count, size, payload in sections:
                self.assertEqual(int(count), 4096)
                self.assertGreater(int(size), 4096)
                self.assertEqual(payload, expected[name])
                self.assertEqual(len(payload.encode()), 4096)
            self.assertNotIn("discarded-prefix", output)

    def test_startup_ready_and_stderr_are_retained_with_process_and_phase_details(self):
        with tempfile.TemporaryDirectory(prefix="suzaku-preference-diagnostic.", dir="/tmp") as owned:
            path = Path(owned) / "host-0.log"
            messages = ("Suzaku IBus host ready: component=owned engine=owned ipc=owned\n"
                        "Synthetic stderr: factory dispatch failed\n")
            path.write_text(messages)
            output = self.capture({0: path})
            self.assertIn(messages, output)
            self.assertIn("Preference QA failure phase: preference restart: CreateEngine", output)
            self.assertIn("Owned process pid=101 exit=None", output)
            self.assertIn("Owned process pid=102 exit=7", output)

        inner = next(node for node in self.tree.body if isinstance(node, ast.FunctionDef) and
                     node.name == "inner")
        restart = next(node for node in inner.body if isinstance(node, ast.FunctionDef) and
                       node.name == "restart")
        spawns = [node for node in ast.walk(restart) if isinstance(node, ast.Call) and
                  isinstance(node.func, ast.Attribute) and node.func.attr == "Popen"]
        self.assertEqual(len(spawns), 1)
        streams = {keyword.arg: keyword.value for keyword in spawns[0].keywords
                   if keyword.arg in ("stdout", "stderr")}
        self.assertEqual(set(streams), {"stdout", "stderr"})
        self.assertEqual(ast.dump(streams["stdout"]), ast.dump(streams["stderr"]))
        self.assertIsInstance(streams["stdout"], ast.Name)

    def test_companion_draft_candidates_and_preference_file_contents_are_not_reported(self):
        self.fixture.watchers = [SimpleNamespace(latest=dict(
            host="owned-host", context=12, revision=24, focused=True, private=False,
            seed="private-draft-marker", candidates=[{"text": "private-candidate-marker"}],
            cache="private-cache-marker"))]
        with tempfile.TemporaryDirectory(prefix="suzaku-preference-diagnostic.", dir="/tmp") as owned:
            root = Path(owned)
            cache = root / "preferences-v1.json"
            cache.write_text("private-cache-file-marker")
            log = root / "host-0.log"
            log.write_text("owned startup metadata only")
            output = self.capture({0: log})
            self.assertEqual(cache.read_text(), "private-cache-file-marker")
            self.assertIn("'host': 'owned-host'", output)
            self.assertIn("'context': 12", output)
            self.assertIn("'revision': 24", output)
            for marker in ("private-draft-marker", "private-candidate-marker", "private-cache-marker",
                           "private-cache-file-marker", "preferences-v1.json", "'seed'", "'candidates'"):
                self.assertNotIn(marker, output)

    def test_missing_log_does_not_suppress_later_owned_logs(self):
        with tempfile.TemporaryDirectory(prefix="suzaku-preference-diagnostic.", dir="/tmp") as owned:
            root = Path(owned)
            missing, second, third = [root / f"host-{index}.log" for index in range(3)]
            second.write_text("second retained")
            third.write_text("third retained")
            output = self.capture({0: missing, 1: second, 2: third})
            self.assertIn("Owned host-0.log log unavailable:", output)
            self.assertIn("second retained", output)
            self.assertIn("third retained", output)

    def failure_lifecycle(self, diagnostics_error=None):
        inner = next(node for node in self.tree.body if isinstance(node, ast.FunctionDef) and
                     node.name == "inner")
        guarded = next(node for node in inner.body if isinstance(node, ast.Try) and
                       any(isinstance(child, ast.Call) and isinstance(child.func, ast.Name) and
                           child.func.id == "report_failure"
                           for handler in node.handlers for child in ast.walk(handler)))
        # Retain the exact real exception/finally clauses, replace only the full
        # native workflow with one controlled failure: no GI or child execution.
        isolated = copy.deepcopy(guarded)
        isolated.body = [ast.Raise(exc=ast.Name(id="original", ctx=ast.Load()), cause=None)]
        executable = compile(ast.fix_missing_locations(ast.Module(body=[isolated], type_ignores=[])),
                             str(SOURCE), "exec")
        events, output = [], io.StringIO()
        original = TimeoutError("original single-shot factory timeout")
        with tempfile.TemporaryDirectory(prefix="suzaku-preference-diagnostic.", dir="/tmp") as owned:
            root = Path(owned)
            log = root / "host-0.log"
            log.write_text("evidence before cleanup")

            def report(fixture, logs):
                events.append("report")
                self.assertTrue(log.is_file(), "diagnostics ran after temporary files were removed")
                self.report(fixture, logs)
                if diagnostics_error is not None:
                    raise diagnostics_error

            self.fixture.close = lambda: events.append("close")
            namespace = dict(original=original, root=root, fixture=self.fixture, host_logs={0: log},
                             report_failure=report, qa=SimpleNamespace(
                                 preserve_primary_failure=preserve_primary_failure))
            with contextlib.redirect_stderr(output):
                with self.assertRaises(TimeoutError) as caught:
                    exec(executable, namespace)
            self.assertIs(caught.exception, original)
            self.assertEqual(events, ["report", "close"])
            self.assertIn("evidence before cleanup", output.getvalue())
            self.assertTrue((root / "preference-save.release").is_file())
            self.assertTrue((root / "preference-clear.release").is_file())
        self.assertFalse(root.exists())
        return original

    def test_real_exception_path_reports_before_finally_and_temporary_cleanup(self):
        self.failure_lifecycle()

    def test_diagnostics_failure_cannot_replace_the_original_factory_error(self):
        original = self.failure_lifecycle(RuntimeError("synthetic diagnostic failure"))
        self.assertTrue(any("synthetic diagnostic failure" in note for note in original.__notes__))


if __name__ == "__main__":
    unittest.main()
