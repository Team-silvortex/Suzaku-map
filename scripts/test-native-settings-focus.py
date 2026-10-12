#!/usr/bin/env python3
"""Deterministic settings-focus ordering; executes real fixture functions, no UI."""
import ast
import copy
from pathlib import Path
from types import SimpleNamespace
import unittest


SOURCE = Path(__file__).with_name("test-linux-native-candidates.py")
FUNCTIONS = {"set_layout_from_settings", "settings_focus_ack", "wait", "stable", "seed_is"}


def frame(context=10, revision=19, focused=True, **changes):
    return dict(host="owned-host", context=context, revision=revision,
                focused=focused, private=False, seed="", candidates=[], **changes)


class Scenario:
    def __init__(self, *, lose=True, regain=True, dirty=False, inserted=False,
                 late_loss=False, loss_change=None, gain_change=None,
                 return_context="owned-editor", intermediate_fallback=False,
                 late_context_loss=False, intermediate_change=None):
        tree = ast.parse(SOURCE.read_text())
        module = ast.Module(body=[node for node in tree.body
                                  if isinstance(node, ast.FunctionDef)
                                  and node.name in FUNCTIONS], type_ignores=[])
        self.now = 0.0
        self.queue = []
        self.events = []
        self.focus = "editor"
        self.input_context = "owned-editor"
        self.return_context = return_context
        self.intermediate_fallback = intermediate_fallback
        self.intermediate_change = intermediate_change or {}
        self.late_context_loss = late_context_loss
        self.mapped = False
        self.layout = "follow-caret"
        self.line = 0
        self.watch = SimpleNamespace(latest=frame(), frames=[frame()])
        self.insertions = []
        self.lose, self.regain = lose, regain
        self.dirty, self.inserted, self.late_loss = dirty, inserted, late_loss
        self.loss_change, self.gain_change = loss_change or {}, gain_change or {}
        self.ns = dict(
            qa=SimpleNamespace(watch=self.watch, subprocess=SimpleNamespace(
                run=lambda *args, **kwargs: SimpleNamespace(stdout=""))),
            bus=SimpleNamespace(current_input_context=lambda: self.input_context),
            x=SimpleNamespace(display=1, focus=self.settings_focus,
                              window=lambda title: "settings",
                              x=SimpleNamespace(XSetInputFocus=self.restore,
                                                XSync=lambda *args: None)),
            xid="editor", view=SimpleNamespace(grab_focus=lambda: None),
            committed="", insertions=self.insertions, buffer_text=lambda: "",
            show_owned_main=lambda: None, window_frame=self.window_frame,
            click_control=self.click, panel_window_mapped=lambda kind: self.mapped,
            persisted_layout=lambda: self.layout, owned_focus=lambda: self.focus == "editor",
            expect_unchanged_buffer=self.unchanged,
            pump=self.pump, time=SimpleNamespace(monotonic=lambda: self.now, sleep=self.sleep),
        )
        exec(compile(module, str(SOURCE), "exec"), self.ns)
        self.original_stable = self.ns["stable"]
        self.ns["stable"] = self.stable

    def sleep(self, seconds):
        self.now += seconds

    def emit(self, name, value):
        self.events.append(name)
        self.watch.latest = value
        self.watch.frames.append(value)

    def pump(self):
        self.now += .01
        for due, action in list(self.queue):
            if due <= self.now:
                self.queue.remove((due, action))
                action()

    def window_frame(self, kind):
        return dict(_qa_log_line=self.line, detected_layout_mode="bottom-dock",
                    layout_preference=self.layout,
                    layout_mode="bottom-dock" if self.layout == "auto" else self.layout)

    def click(self, kind, window_kind="Main"):
        self.line += 1
        if kind == "SettingsToggle":
            self.mapped = window_kind == "Main"
            self.events.append("open" if self.mapped else "close")
        else:
            assert kind == "SetPanelLayoutMode(Auto)"
            self.layout = "auto"
            self.events.append("choice")
            if self.intermediate_fallback:
                value = frame(12, 21)
                value.update(self.intermediate_change)
                self.emit("fallback-focus", value)

    def settings_focus(self, target):
        assert target == "settings"
        self.focus = target
        self.events.append("settings-focus")
        if self.lose:
            def loss():
                if self.dirty:
                    value = frame(11, 20)
                    value["seed"] = "unexpected"
                    self.emit("dirty", value)
                if self.inserted:
                    self.insertions.append("unexpected")  # Even if the buffer was cleared again.
                value = frame(11, 20, False)
                value.update(self.loss_change)
                self.input_context = "fallback"
                self.emit("loss", value)
            self.queue.append((self.now + .06, loss))

    def restore(self, display, target, revert, timestamp):
        assert target == "editor"
        self.focus = target
        self.events.append("restore")
        # D-Bus may report the editor before its host notification is consumed.
        self.input_context = self.return_context
        if self.regain:
            def gain():
                value = frame(13, 22) if self.intermediate_fallback else frame(12, 21)
                value.update(self.gain_change)
                self.input_context = self.return_context
                self.emit("gain", value)
            self.queue.append((self.now + .09, gain))
        return 1

    def unchanged(self):
        assert self.focus == "editor", "owned focus lost"
        assert not self.insertions, "unexpected insertion"

    def stable(self, *args, **kwargs):
        self.events.append("stable")
        if self.late_loss:
            self.queue.append((self.now + .03,
                               lambda: self.emit("late-loss", frame(13, 22, False))))
        if self.late_context_loss:
            self.queue.append((self.now + .03,
                               lambda: setattr(self, "input_context", "fallback")))
        return self.original_stable(*args, **kwargs)

    def run(self):
        self.ns["set_layout_from_settings"]("auto")


class SettingsFocusTests(unittest.TestCase):
    def test_delayed_loss_and_return_never_accept_the_old_focused_snapshot(self):
        case = Scenario()
        case.run()
        self.assertLess(case.events.index("loss"), case.events.index("choice"))
        self.assertLess(case.events.index("gain"), case.events.index("stable"))
        self.assertEqual(case.events.count("restore"), 1)
        self.assertEqual(case.watch.latest["context"], 12)

    def test_missing_loss_ack_fails_before_changing_layout_or_restoring_focus(self):
        case = Scenario(lose=False)
        with self.assertRaises(AssertionError):
            case.run()
        self.assertNotIn("choice", case.events)
        self.assertNotIn("restore", case.events)
        self.assertLess(case.now, 4.1)

    def test_focused_fallback_is_a_handoff_but_never_the_owned_return(self):
        case = Scenario(loss_change={"focused": True})
        case.run()
        self.assertLess(case.events.index("loss"), case.events.index("choice"))
        self.assertLess(case.events.index("gain"), case.events.index("stable"))
        wrong = Scenario(return_context="fallback")
        with self.assertRaises(AssertionError):
            wrong.run()
        self.assertNotIn("stable", wrong.events)

    def test_missing_return_ack_fails_with_original_deadline(self):
        case = Scenario(regain=False)
        with self.assertRaises(AssertionError):
            case.run()
        self.assertNotIn("stable", case.events)
        self.assertLess(case.now, 4.2)
        self.assertEqual(case.events.count("restore"), 1)

    def test_revision_only_cannot_acknowledge_loss(self):
        case = Scenario(loss_change={"context": 10, "revision": 100})
        with self.assertRaises(AssertionError):
            case.run()
        self.assertNotIn("choice", case.events)

    def test_focused_fallback_from_settings_cannot_acknowledge_return(self):
        case = Scenario(intermediate_fallback=True, regain=False)
        with self.assertRaises(AssertionError):
            case.run()
        self.assertNotIn("stable", case.events)
        good = Scenario(intermediate_fallback=True)
        good.run()
        self.assertLess(good.events.index("gain"), good.events.index("stable"))

    def test_revision_only_cannot_acknowledge_return(self):
        case = Scenario(gain_change={"context": 11, "revision": 100})
        with self.assertRaises(AssertionError):
            case.run()
        self.assertNotIn("stable", case.events)

    def test_new_host_or_private_return_cannot_satisfy_the_barrier(self):
        for change in ({"host": "replacement"}, {"private": True}):
            with self.subTest(change=change):
                case = Scenario(gain_change=change)
                with self.assertRaises(AssertionError):
                    case.run()
                self.assertNotIn("stable", case.events)

    def test_host_restart_or_private_frame_during_settings_is_not_a_new_baseline(self):
        for change in ({"host": "replacement"}, {"private": True}):
            with self.subTest(change=change):
                case = Scenario(intermediate_fallback=True, intermediate_change=change,
                                gain_change={"host": change.get("host", "owned-host")})
                with self.assertRaises(AssertionError):
                    case.run()
                self.assertNotIn("restore", case.events)

    def test_transient_draft_cannot_hide_behind_a_later_empty_frame(self):
        case = Scenario(dirty=True)
        with self.assertRaisesRegex(AssertionError, "draft|candidate"):
            case.run()
        self.assertNotIn("choice", case.events)

    def test_transient_insertion_is_not_hidden_by_the_empty_current_buffer(self):
        case = Scenario(inserted=True)
        with self.assertRaises(AssertionError):
            case.run()
        self.assertNotIn("choice", case.events)

    def test_real_focus_loss_after_ack_is_still_an_immediate_stability_failure(self):
        case = Scenario(late_loss=True)
        with self.assertRaises(AssertionError):
            case.run()
        self.assertIn("late-loss", case.events)
        self.assertEqual(case.events.count("restore"), 1)
        self.assertEqual(case.events.count("stable"), 1)

    def test_input_context_loss_after_ack_is_not_hidden_by_focused_snapshot(self):
        case = Scenario(late_context_loss=True)
        with self.assertRaises(AssertionError):
            case.run()
        self.assertEqual(case.events.count("restore"), 1)

    def test_ack_rejects_stale_wrong_private_or_nonempty_frames(self):
        case = Scenario()
        self.assertIn("settings_focus_ack", case.ns)
        baseline = frame()
        good = frame(11, 20, False)
        for change in ({"context": 10}, {"revision": 19}, {"host": "other"},
                       {"focused": True}, {"private": True}, {"seed": "x"},
                       {"candidates": [{"text": "x"}]}):
            with self.subTest(change=change):
                value = copy.deepcopy(good)
                value.update(change)
                case.watch.latest = value
                self.assertFalse(case.ns["settings_focus_ack"](baseline, False))
        case.watch.latest = good
        self.assertTrue(case.ns["settings_focus_ack"](baseline, False))

    def test_original_time_limits_are_not_extended(self):
        case = Scenario()
        self.assertEqual(case.ns["wait"].__defaults__, (4,))
        self.assertEqual(case.original_stable.__defaults__, (.15,))
        runner = SOURCE.with_name("test-linux-apps.sh").read_text()
        self.assertIn("timeout --kill-after=3s 240s", runner)


if __name__ == "__main__":
    unittest.main()
