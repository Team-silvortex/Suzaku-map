#!/usr/bin/env python3
"""Freeze the appended custom-language pending word-to-sentence workflows.

This source-only guard never imports the native suite or starts a bus, display,
host or model. Passing preserves fixture structure and historical assertions;
it is not native, physical-keyboard, installation or model acceptance.
"""
import ast
import hashlib
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parent.parent
NATIVE_PATH = ROOT / "scripts/test-native-sync.py"
SOURCE = NATIVE_PATH.read_text()
TREE = ast.parse(SOURCE)
HELPER = "check_custom_language_pending_continuation"
PREDECESSOR = "check_custom_language_pending_boundaries"
NATIVE_BEFORE_SHA256 = "811d7c23545c35c9508477f44d186699de2b2b0806fc1df24dc1d20c09d84670"
GUARD_BEFORE_SHA256 = {
    "scripts/test-custom-boundary-coverage.py":
        "2ad5c48c36af53c1d7502002fd8646489e14d6abb733dc367c504b212c321a85",
    "scripts/test-custom-language-coverage.py":
        "88b36656ee38d1d8fee52e9dba061a4c219b7b41b24eaa3f7db09c9e79cc259a",
    "scripts/test-held-language-coverage.py":
        "421005bd56d62a80574cded801a52daec7d4e08052e1b3a1a853b71d9432d848",
    "scripts/test-native-model-fixture.py":
        "bc3a6c1c9be75c20043aaaa3b259085b953eebd2600389fd9c52f27737b7bc2c",
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
# Freeze each exact added source block as well as the restored whole old file.
# No historical hash is calculated from the changed fixture or replaced here.
MAP_SHA256 = "bbce78dcdacef2e2593cc9e7c4224a6afa7c8af68a04837ca977aeddded98b22"
HELD_LOOP_SHA256 = "3413b4c714fb9897fe4176ffeb7a0bcf4689b6562f725c4f060fda5dc6d0d569"
MODEL_MAP_SHA256 = "e78a9b0398e3f9ca811fac35739890bf1dd808a6f4255ef8fe6964cb2b57ea29"
MODEL_SEEN_SHA256 = "4e591fd80b2287167d500862556bde254e9861e1cb5d1f9d97648d4a86fd84f3"
GUARD_ADDED_STATEMENTS = {
    "scripts/test-held-language-coverage.py": {"CONTINUATION_APPENDED_CALLS": MAP_SHA256},
    "scripts/test-custom-language-coverage.py": {
        "CONTINUATION_APPENDED_CALLS": MAP_SHA256,
        "HELD_CONTINUATION_ADDITION": "c3283dd8f22eacb51c3dbbb23579520190677ce351c58ce41f5607dffa52c35a",
        "HELD_CONTINUATION_LOOP": HELD_LOOP_SHA256,
        "MODEL_CONTINUATION_MAP_ADDITION": MODEL_MAP_SHA256,
        "MODEL_CONTINUATION_SEEN_ADDITION": MODEL_SEEN_SHA256,
    },
    "scripts/test-custom-boundary-coverage.py": {
        "CONTINUATION_MAP": "094b4919847cc18a247868169b96cc0901a0cb820d744d633c5a0d57f164a734",
        "HELD_CONTINUATION_LOOP": HELD_LOOP_SHA256,
        "CUSTOM_CONTINUATION_LOOP": "ec10e6b0d2fc3e5ec685f09a5a5ca6670a74b01c7aa683fb29c42d0d24450762",
        "MODEL_CONTINUATION_MAP_ADDITION": MODEL_MAP_SHA256,
        "MODEL_CONTINUATION_SEEN_ADDITION": MODEL_SEEN_SHA256,
        "CUSTOM_CONTINUATION_CONSTANTS": "89adfc81f43214225c455d1ac665685eac0eb137518dedca0e72fd560e84983d",
        "CUSTOM_CONTINUATION_HELD_PREPROCESS": "c8bddd83b35a6294ee3aa01e6bd866ec00c5eb6bfee366be7f81c33e7cc71d83",
        "CUSTOM_CONTINUATION_MODEL_PREPROCESS": "794dbc52c63967773c710ae4b8f404e47928250f3fa59e78aed3a8826600b22b",
        "project_continuation": "b1063012e70a34a1604ca349aefdaeb3bfae1ac5ea116f3c5dd145eb42759b30",
    },
}
HELD_LOOP_BEFORE = "    for name, (predecessor, count) in (APPENDED_CALLS | CUSTOM_APPENDED_CALLS | BOUNDARY_APPENDED_CALLS).items():\n"
HELD_LOOP_AFTER = "    for name, (predecessor, count) in (APPENDED_CALLS | CUSTOM_APPENDED_CALLS | BOUNDARY_APPENDED_CALLS | CONTINUATION_APPENDED_CALLS).items():\n"
CUSTOM_LOOP_BEFORE = "    for name, (predecessor, count) in (APPENDED_CALLS | BOUNDARY_APPENDED_CALLS).items():\n"
CUSTOM_LOOP_AFTER = "    for name, (predecessor, count) in (APPENDED_CALLS | BOUNDARY_APPENDED_CALLS | CONTINUATION_APPENDED_CALLS).items():\n"
MODEL_MAP_ADDITION = '        appended |= {"check_custom_language_pending_continuation": "check_custom_language_pending_boundaries"}\n'
MODEL_SEEN_ADDITION = '        self.assertEqual(seen.pop("check_custom_language_pending_continuation"), 1)\n'
CUSTOM_HELD_PREPROCESS = '''        source = replace_once(source, HELD_CONTINUATION_ADDITION, "")
        source = replace_once(source, HELD_CONTINUATION_LOOP, HELD_BOUNDARY_LOOP)
'''
CUSTOM_MODEL_PREPROCESS = '''        source = replace_once(source, MODEL_CONTINUATION_MAP_ADDITION, "")
        source = replace_once(source, MODEL_CONTINUATION_SEEN_ADDITION, "")
'''
BOUNDARY_HELD_PREPROCESS = '''        source = replace_once(source, "\\n" + CONTINUATION_MAP, "\\n")
        source = replace_once(source, HELD_CONTINUATION_LOOP, HELD_LOOP_AFTER)
'''
BOUNDARY_CUSTOM_PREPROCESS = '''        source = replace_once(source, "\\n" + CONTINUATION_MAP, "\\n")
        source = replace_once(source, CUSTOM_CONTINUATION_LOOP, CUSTOM_LOOP_AFTER)
        source = replace_once(source, CUSTOM_CONTINUATION_CONSTANTS, "")
        source = replace_once(source, CUSTOM_CONTINUATION_HELD_PREPROCESS, "")
        source = replace_once(source, CUSTOM_CONTINUATION_MODEL_PREPROCESS, "")
'''
LANGUAGE_CASES = '''[("en", "zh-Hans", "Earlier note. " * 30, "please sen", "please send", " me the details."),
                 ("zh-Hans", "en", "前文。" * 100, "hui yi", "会议", "什么时候开始？")]'''
CASES = ["success-sentence", "failure-sentence", "success-edited", "failure-edited"]
BINDINGS = {"previous": "Ctrl+P", "next": "Ctrl+N", "previous-page": "Alt+Shift+B",
            "next-page": "Ctrl+Shift+F", "adopt": "Alt+Shift+A", "switch-language": "Ctrl+Shift+L"}


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
    return same_ast(node, statement(name + "(bus)"))


def project_native(source):
    """Remove only one undecorated helper and its exact control-IO bus call."""
    tree, lines = ast.parse(source), source.splitlines(keepends=True)
    helper = definition(tree, HELPER)
    if helper.decorator_list or lines[helper.end_lineno:helper.end_lineno + 2] != ["\n", "\n"]:
        raise AssertionError("Continuation helper must be undecorated with two following blank lines")
    call = only((node for node in ast.walk(tree) if isinstance(node, ast.Expr)
                 and isinstance(node.value, ast.Call) and isinstance(node.value.func, ast.Name)
                 and node.value.func.id == HELPER), "continuation call")
    if not exact_call(call, HELPER) or call.lineno != call.end_lineno:
        raise AssertionError("Continuation must have exactly one single-line bus call")
    indent = lines[call.lineno - 1].split(HELPER + "(", 1)[0]
    if lines[call.lineno - 1] != indent + HELPER + "(bus)\n":
        raise AssertionError("Continuation call may contain no extra source")
    if lines[call.lineno - 2] != indent + PREDECESSOR + "(bus)\n":
        raise AssertionError("Continuation must immediately follow its boundary predecessor")
    branch = only((node for node in ast.walk(tree) if isinstance(node, ast.If)
                  and same_ast(node.test, ast.parse(
                      'os.environ.get("SUZAKU_NATIVE_CONTROL_IO_ONLY") == "1"', mode="eval").body)
                  and call in node.body), "control-IO continuation dispatch")
    expected = ['check_settings_io_responsiveness(bus)', 'check_language_shortcut_slow_io(bus)',
                'check_held_language_pending(bus)', 'check_custom_language_pending_long(bus)',
                PREDECESSOR + '(bus)', HELPER + '(bus)', 'raise SystemExit(0)']
    if branch.orelse or len(branch.body) != len(expected) or not all(
            same_ast(node, statement(text)) for node, text in zip(branch.body, expected)):
        raise AssertionError("Continuation dispatch must preserve its exact control-IO body")
    omitted = set(range(helper.lineno - 1, helper.end_lineno + 2)) | {call.lineno - 1}
    return "".join(line for index, line in enumerate(lines) if index not in omitted)


def replace_once(source, before, after):
    if source.count(before) != 1:
        raise AssertionError("Guard projection requires exactly one fixed source block")
    return source.replace(before, after, 1)


def project_guard(path, source):
    if path not in GUARD_BEFORE_SHA256:
        raise AssertionError("Unknown historical guard")
    lines, omitted = source.splitlines(keepends=True), set()
    tree = ast.parse(source)
    for name, expected in GUARD_ADDED_STATEMENTS.get(path, {}).items():
        node = only((item for item in tree.body if
                     (isinstance(item, ast.FunctionDef) and item.name == name) or
                     (isinstance(item, ast.Assign) and len(item.targets) == 1 and
                      isinstance(item.targets[0], ast.Name) and item.targets[0].id == name)), name)
        end = node.end_lineno + (2 if isinstance(node, ast.FunctionDef) else 0)
        block = "".join(lines[node.lineno - 1:end])
        if hashlib.sha256(block.encode()).hexdigest() != expected:
            raise AssertionError("Only the exact enumerated projection support may be stripped: " + name)
        omitted.update(range(node.lineno - 1, end))
    source = "".join(line for index, line in enumerate(lines) if index not in omitted)
    if path == "scripts/test-held-language-coverage.py":
        return replace_once(source, HELD_LOOP_AFTER, HELD_LOOP_BEFORE)
    if path == "scripts/test-native-model-fixture.py":
        source = replace_once(source, MODEL_MAP_ADDITION, "")
        return replace_once(source, MODEL_SEEN_ADDITION, "")
    if path == "scripts/test-custom-language-coverage.py":
        source = replace_once(source, CUSTOM_LOOP_AFTER, CUSTOM_LOOP_BEFORE)
        source = replace_once(source, CUSTOM_HELD_PREPROCESS, "")
        return replace_once(source, CUSTOM_MODEL_PREPROCESS, "")
    source = replace_once(source, "    source = project_continuation(source)\n", "")
    source = replace_once(source, BOUNDARY_HELD_PREPROCESS, "")
    source = replace_once(source, CUSTOM_MODEL_PREPROCESS, "")
    return replace_once(source, BOUNDARY_CUSTOM_PREPROCESS, "")


class CustomContinuationCoverageTests(unittest.TestCase):
    def helper(self):
        return definition(TREE, HELPER)

    def nested(self, name):
        return only((node for node in self.helper().body if isinstance(node, ast.FunctionDef)
                     and node.name == name), name)

    def assigned(self, node, name):
        return only((item.value for item in node.body if isinstance(item, ast.Assign)
                     and len(item.targets) == 1 and isinstance(item.targets[0], ast.Name)
                     and item.targets[0].id == name), "assignment " + name)

    def owners(self):
        node = self.helper()
        outer = only((item for item in node.body if isinstance(item, ast.Try)), "outer try")
        profile = only((item for item in outer.body if isinstance(item, ast.For)), "profile loop")
        language = only((item for item in profile.body if isinstance(item, ast.For)), "language loop")
        case = only((item for item in language.body if isinstance(item, ast.For)), "case loop")
        owner = only((item for item in case.body if isinstance(item, ast.Try)), "case try")
        return node, outer, profile, language, case, owner

    def has(self, node, text):
        self.assertTrue(any(same_ast(item, statement(text)) for item in ast.walk(node)), text)

    def predicate(self, node, text):
        self.assertTrue(any(same_ast(item, ast.parse(text, mode="eval").body)
                            for item in ast.walk(node)), text)

    def sequence(self, body, texts):
        position = 0
        for text in texts:
            matches = [index for index in range(position, len(body))
                       if same_ast(body[index], statement(text))]
            self.assertTrue(matches, "Missing or out-of-order direct statement: " + text)
            position = matches[0] + 1

    def branch(self, node, text):
        return only((item for item in ast.walk(node) if isinstance(item, ast.If)
                     and same_ast(item.test, ast.parse(text, mode="eval").body)), "branch " + text)

    def test_native_syntax_without_execution(self):
        compile(TREE, str(NATIVE_PATH), "exec")

    def test_whole_fixture_only_adds_one_helper_and_one_exact_control_IO_call(self):
        self.assertEqual(hashlib.sha256(project_native(SOURCE).encode()).hexdigest(), NATIVE_BEFORE_SHA256)

    def test_historical_guard_assertions_matrices_and_hashes_are_unchanged(self):
        for path, expected in GUARD_BEFORE_SHA256.items():
            with self.subTest(path=path):
                source = (ROOT / path).read_text()
                compile(ast.parse(source), path, "exec")
                self.assertEqual(hashlib.sha256(project_guard(path, source).encode()).hexdigest(), expected)

    def test_runner_preload_and_production_language_implementation_are_unchanged(self):
        for path, expected in UNCHANGED_FILE_SHA256.items():
            with self.subTest(path=path):
                self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), expected)

    def test_projection_rejects_bad_arguments_moved_duplicate_or_extra_calls(self):
        call = only((node for node in ast.walk(TREE) if exact_call(node, HELPER)), "continuation call")
        lines, original = SOURCE.splitlines(keepends=True), SOURCE.splitlines(keepends=True)[call.lineno - 1]
        indent = original[:len(original) - len(original.lstrip())]
        for altered in [f"{HELPER}(other)", f"{HELPER}(bus, bus)", f"{HELPER}(bus=bus)",
                        f"{HELPER}(bus); check_language_shortcut(bus)"]:
            changed = list(lines)
            changed[call.lineno - 1] = indent + altered + "\n"
            with self.subTest(altered=altered), self.assertRaises(AssertionError):
                project_native("".join(changed))
        moved = list(lines)
        moved[call.lineno - 2:call.lineno] = reversed(moved[call.lineno - 2:call.lineno])
        with self.assertRaises(AssertionError):
            project_native("".join(moved))
        duplicate = list(lines)
        duplicate.insert(call.lineno, original)
        with self.assertRaises(AssertionError):
            project_native("".join(duplicate))

    def test_projection_cannot_hide_old_timeout_or_guard_and_new_support_edits(self):
        changed = SOURCE.replace("time.sleep(0.01)", "time.sleep(0.02)", 1)
        self.assertNotEqual(hashlib.sha256(project_native(changed).encode()).hexdigest(), NATIVE_BEFORE_SHA256)
        for path, expected in GUARD_BEFORE_SHA256.items():
            with self.subTest(path=path):
                source = (ROOT / path).read_text()
                changed = source + "\n# unrelated historical guard mutation\n"
                self.assertNotEqual(hashlib.sha256(project_guard(path, changed).encode()).hexdigest(), expected)
                changed = source.replace(PREDECESSOR + '", 1)', PREDECESSOR + '", 2)', 1)
                if changed != source:
                    with self.assertRaises(AssertionError):
                        project_guard(path, changed)

    def test_fixed_16_routes_languages_profiles_bindings_and_distinct_latest_undo(self):
        node, _outer, profile, language, case, owner = self.owners()
        self.assertEqual(ast.literal_eval(self.assigned(node, "bindings")), BINDINGS)
        self.assertTrue(same_ast(self.assigned(node, "languages"), ast.parse(LANGUAGE_CASES, mode="eval").body))
        self.assertEqual(ast.literal_eval(self.assigned(node, "cases")), CASES)
        self.assertTrue(same_ast(profile.target, statement("profile = None").targets[0]))
        self.assertEqual(ast.literal_eval(profile.iter), ["standard", "home-row"])
        self.assertTrue(same_ast(language.target, statement("original, target, prefix, tail, word, suffix = None").targets[0]))
        self.assertTrue(same_ast(language.iter, ast.Name(id="languages", ctx=ast.Load())))
        self.assertTrue(same_ast(case.target, statement("case = None").targets[0]))
        self.assertTrue(same_ast(case.iter, ast.Name(id="cases", ctx=ast.Load())))
        self.has(profile, 'assert json.loads(command("U" + json.dumps({"shortcut_profile": profile, "shortcut_bindings": bindings})))["ok"]')
        self.has(owner, 'assert word_space != sentence[:-1] and raw != word_space, "latest sentence undo oracle must differ"')
        self.has(node, 'assert checked == 16, checked')
        self.assertEqual(sum(same_ast(item, statement("checked += 1")) for item in ast.walk(node)), 1)
        for prefix, tail, word, suffix in [("Earlier note. " * 30, "please sen", "please send", "me the details."),
                                           ("前文。" * 100, "hui yi", "会议", "什么时候开始？")]:
            self.assertGreater(len(prefix + tail), 256)
            self.assertNotEqual(prefix + word + " ", (prefix + word + " " + suffix)[:-1])
            self.assertNotEqual(prefix + tail, prefix + word + " ")

    def test_owned_reset_prefix_drained_real_tail_and_configured_full_payload_selection(self):
        node, _outer, _profile, _language, _case, owner = self.owners()
        self.sequence(owner.body, ['peer.event("FocusIn")', 'old_context = fresh()["context"]',
                                   'peer.event("Reset")', 'assert action(fresh(), "T" + prefix)', 'raw = prefix'])
        self.predicate(owner, 'lambda: watch.latest["context"] > old_context and not watch.latest["seed"]')
        self.has(node, 'assert json.loads(command("L" + original))["ok"]')
        typing = only((item for item in owner.body if isinstance(item, ast.For)
                       and same_ast(item.target, statement("character = None").targets[0])), "tail loop")
        self.assertTrue(same_ast(typing.iter, ast.Name(id="tail", ctx=ast.Load())))
        self.sequence(typing.body, ['assert peer.process_key_event(IBus.unicode_to_keyval(character))', 'raw += character'])
        self.predicate(typing, 'lambda: watch.latest["seed"] == raw')
        select = self.nested("select")
        self.predicate(select, 'row["text"] == text and row["source"] == "local" and (kind is None or row["kind"] == kind)')
        self.has(select, 'assert peer.process_key_event(IBus.KEY_n if forward else IBus.KEY_p, 0, ctrl)')
        self.has(select, 'assert watch.latest["seed"] == before_frame["seed"] and watch.latest["candidates"] == rows')
        self.assertFalse(any(isinstance(item, ast.Call) and isinstance(item.func, ast.Name)
                             and item.func.id == "select_keyboard_candidate" for item in ast.walk(node)))

    def test_complete_fresh_frame_and_inert_events_never_ignore_commits(self):
        fresh = self.nested("fresh")
        self.predicate(fresh, 'lambda: probe.latest is not None and watch.latest == probe.latest')
        self.has(fresh, 'assert probe.latest["focused"] and not probe.latest["private"]')
        self.has(fresh, 'return probe.latest')
        self.has(fresh, 'watchers.remove(probe)')
        self.has(fresh, 'probe.sock.close()')
        unchanged = self.nested("unchanged")
        self.has(unchanged, 'before_frame, commits = fresh(), peer.commits.copy()')
        self.has(unchanged, 'assert peer.process_key_event(key, code, mask) == handled')
        self.has(unchanged, 'assert fresh() == before_frame, "pending continuation inert event changed the full frame"')
        self.has(unchanged, 'assert peer.commits == commits')

    def test_real_write_barrier_precedes_word_adoption_and_keeps_busy_worker_until_exit(self):
        node, _outer, _profile, _language, _case, owner = self.owners()
        self.has(node, 'armed, entered, released, finished, failed = [runtime / ("control-io." + name) for name in ["arm", "entered", "release", "finished", "fail"]]')
        self.sequence(owner.body, ['before = fresh()', 'armed.touch()',
                                   'assert peer.process_key_event(IBus.KEY_L, 46, ctrl | shift)',
                                   'wait(entered.exists, "pending continuation real durable-write barrier before word adoption")',
                                   'assert json.loads(command("S", timeout=.25))["settings"]["language"] == original',
                                   'pending(peer, raw)', 'select(peer, full_word, "word")',
                                   'assert peer.process_key_event(IBus.KEY_A, 38, alt | shift)',
                                   'busy = json.loads(command("P0", timeout=.25))',
                                   'assert not busy["ok"] and "正在保存" in busy["error"], "pending continuation writer permit released early"',
                                   'assert workers() == 1 and config.read_bytes() == before_disk', 'released.touch()'])
        failure = self.branch(owner, 'case.startswith("failure")')
        self.sequence(failure.body, ['failed.touch()'])
        self.predicate(owner, 'lambda: finished.exists() and workers() == 0 and not list(runtime.glob(".suzaku-*.tmp"))')
        worker = self.nested("workers")
        self.has(worker, 'count += entry.read_text().strip() == "suzaku-settings"')
        self.assertTrue(any(isinstance(item, ast.ExceptHandler) and isinstance(item.type, ast.Name)
                            and item.type.id == "FileNotFoundError" for item in ast.walk(worker)))

    def test_each_pending_stage_keeps_owning_context_language_hint_disk_and_no_commit(self):
        pending = self.nested("pending")
        self.sequence(pending.body, ['wait(lambda: watch.latest["seed"] == expected, "pending continuation exact latest draft")',
                                    'frame = fresh()',
                                    'assert frame["context"] == before["context"] and frame["language"] == original',
                                    'assert "正在切换" in feedback["text"], "pending continuation lost the progress hint"',
                                    'assert workers() == 1 and config.read_bytes() == before_disk', 'assert not peer.commits'])
        _node, _outer, _profile, _language, _case, owner = self.owners()
        calls = [item for item in ast.walk(owner) if isinstance(item, ast.Expr)
                 and isinstance(item.value, ast.Call) and isinstance(item.value.func, ast.Name)
                 and item.value.func.id == "pending"]
        self.assertEqual(len(calls), 9)
        self.has(owner, 'pending(peer, sentence + "x")')
        self.has(owner, 'pending(peer, expected)')

    def test_pending_word_space_sentence_undo_readoption_and_held_identity_are_exact(self):
        _node, _outer, _profile, _language, _case, owner = self.owners()
        self.sequence(owner.body, ['select(peer, full_word, "word")',
                                   'assert peer.process_key_event(IBus.KEY_A, 38, alt | shift)', 'pending(peer, full_word)',
                                   'unchanged(peer, [(IBus.KEY_a, 38, release, False)])',
                                   'assert peer.process_key_event(IBus.KEY_space)', 'pending(peer, word_space)',
                                   'select(peer, sentence, "sentence")', 'assert peer.process_key_event(IBus.KEY_A, 38, alt | shift)',
                                   'pending(peer, sentence)', 'assert peer.process_key_event(IBus.KEY_BackSpace, 14)',
                                   'pending(peer, word_space)',
                                   'unchanged(peer, [(IBus.KEY_A, 38, alt | shift, True), (IBus.KEY_l, 46, ctrl | shift, True)])',
                                   'unchanged(peer, [(IBus.KEY_a, 38, release, False)])',
                                   'select(peer, sentence, "sentence")', 'assert peer.process_key_event(IBus.KEY_A, 38, alt | shift)',
                                   'pending(peer, sentence)', 'expected = sentence'])
        self.has(owner, 'unchanged(peer, [(IBus.KEY_Control_L, 37, release, False), (IBus.KEY_Shift_L, 50, release, False), (IBus.KEY_Return, 38, 0, True), (IBus.KEY_l, 46, 0, True), (IBus.KEY_a, 39, release, False), (IBus.KEY_a, 38, 0, True)])')
        self.has(owner, 'unchanged(peer, [(IBus.KEY_p, 0, ctrl, True), (IBus.KEY_B, 0, alt | shift, True), (IBus.KEY_a, 38, 0, True), (IBus.KEY_Return, 38, 0, True), (IBus.KEY_l, 46, 0, True), (IBus.KEY_l, 47, release, False)])')

    def test_success_keeps_latest_draft_and_failure_retains_latest_not_initial_undo(self):
        _node, _outer, _profile, _language, _case, owner = self.owners()
        success = self.branch(owner, 'case.startswith("success")')
        self.predicate(success, 'lambda: json.loads(command("S"))["settings"]["language"] == target and watch.latest["language"] == target and watch.latest["context"] > before["context"] and watch.latest["seed"] == expected')
        self.has(success, 'assert json.loads(config.read_bytes())["language"] == target')
        self.has(success, 'assert not action(before, "Tstale pending continuation")')
        self.sequence(success.orelse, ['wait(lambda: "未保存" in feedback["text"], "pending continuation real fsync failure feedback")',
                                      'assert json.loads(command("S"))["settings"]["language"] == original',
                                      'assert config.read_bytes() == before_disk',
                                      'assert fresh()["context"] == before["context"] and watch.latest["seed"] == expected'])
        undo = self.branch(owner, 'case.endswith("sentence")')
        self.sequence(undo.body, ['expected = word_space if case.startswith("failure") else sentence[:-1]',
                                 'wait(lambda: watch.latest["seed"] == expected, "pending continuation success/failure latest adoption undo")'])

    def test_real_edits_retire_undo_and_settled_held_keys_require_exact_release(self):
        _node, _outer, _profile, _language, _case, owner = self.owners()
        edit = self.branch(owner, 'case.endswith("edited")')
        self.sequence(edit.body, ['started_edit = time.monotonic()',
                                 'assert peer.process_key_event(IBus.KEY_x, 53)', 'pending(peer, sentence + "x")',
                                 'assert peer.process_key_event(IBus.KEY_7, 8, alt)',
                                 'expected = sentence + "x7"', 'pending(peer, expected)'])
        undo = self.branch(owner, 'case.endswith("sentence")')
        self.sequence(undo.orelse, ['wait(lambda: watch.latest["seed"] == sentence + "x", "pending continuation delete only literal digit")',
                                   'assert peer.process_key_event(IBus.KEY_BackSpace, 14)',
                                   'wait(lambda: watch.latest["seed"] == sentence, "pending continuation delete only latest typed letter")',
                                   'assert peer.process_key_event(IBus.KEY_BackSpace, 14)', 'expected = sentence[:-1]',
                                   'wait(lambda: watch.latest["seed"] == expected, "pending continuation edit never revives old sentence undo")'])
        self.sequence(owner.body, ['assert fresh()["selected"] == 0',
                                   'unchanged(peer, [(IBus.KEY_p, 0, ctrl, True), (IBus.KEY_B, 0, alt | shift, True), (IBus.KEY_Return, 38, 0, True), (IBus.KEY_l, 46, 0, True)])',
                                   'assert peer.process_key_event(IBus.KEY_BackSpace, 14)',
                                   'unchanged(peer, [(IBus.KEY_a, 38, 0, True), (IBus.KEY_l, 46, 0, True), (IBus.KEY_a, 38, release, False), (IBus.KEY_l, 46, release, False)])'])

    def test_full_exact_commit_retired_context_cleanup_restore_then_PASS(self):
        node, outer, _profile, _language, _case, owner = self.owners()
        self.sequence(owner.body, ['assert peer.process_key_event(IBus.KEY_space)', 'expected += " "',
                                   'select(peer, expected)', 'final_context = fresh()["context"]', 'assert not peer.commits',
                                   'assert peer.process_key_event(IBus.KEY_Return, 36)',
                                   'unchanged(peer, [(IBus.KEY_Return, 36, release, False), (IBus.KEY_Return, 36, 0, False)])'])
        self.predicate(owner, 'lambda: peer.commits == [expected] and watch.latest["context"] > final_context and not watch.latest["seed"]')
        cleanup = only((item for item in owner.finalbody if isinstance(item, ast.With)), "case cleanup")
        self.sequence(cleanup.body, ['released.touch(exist_ok=True)', 'peer.close()',
                                    'peer.connection.signal_unsubscribe(subscription)'])
        self.predicate(cleanup, 'lambda: finished.exists() and workers() == 0 and not list(runtime.glob(".suzaku-*.tmp"))')
        restore = only((item for item in outer.finalbody if isinstance(item, ast.With)
                        and any(same_ast(child, statement('assert json.loads(command("S"))["settings"] == saved'))
                                for child in ast.walk(item))), "full settings restore")
        self.sequence(restore.body, ['assert json.loads(command("L" + saved["language"]))["ok"]',
                                    'assert json.loads(command("U" + json.dumps({"llm_enabled": saved["llm_enabled"], "shortcut_profile": saved["shortcut_profile"], "shortcut_bindings": saved["shortcut_bindings"]})))["ok"]',
                                    'assert json.loads(command("S"))["settings"] == saved',
                                    'assert json.loads(config.read_bytes()) == saved'])
        self.has(outer.finalbody[0], 'watchers.remove(watch)')
        self.has(outer.finalbody[0], 'watch.sock.close()')
        for cleanup in owner.finalbody + outer.finalbody:
            self.assertTrue(any(isinstance(item, ast.Call) and isinstance(item.func, ast.Name)
                                and item.func.id == "preserve_primary_failure" for item in ast.walk(cleanup)))
        self.assertLess(node.body.index(outer), next(index for index, item in enumerate(node.body)
                        if same_ast(item, statement("assert checked == 16, checked"))))
        self.assertIsInstance(node.body[-1], ast.Expr)
        self.assertEqual(ast.unparse(node.body[-1].value.func), "print")

    def test_original_deadlines_and_fast_native_chord_and_edit_budget_remain_strict(self):
        node = self.helper()
        for call in (item for item in ast.walk(node) if isinstance(item, ast.Call)):
            if isinstance(call.func, ast.Name) and call.func.id == "wait":
                self.assertEqual(len(call.args), 2)
                self.assertFalse(call.keywords)
            if isinstance(call.func, ast.Name) and call.func.id == "command" and call.keywords:
                self.assertEqual([(key.arg, ast.literal_eval(key.value)) for key in call.keywords], [("timeout", .25)])
            if isinstance(call.func, ast.Attribute) and call.func.attr == "sleep":
                self.fail("No new sleeps may hide acknowledgement races")
        self.has(node, 'assert time.monotonic() - started < .25, "pending continuation language gesture blocked input"')
        self.has(node, 'assert time.monotonic() - started_edit < .25, "pending continuation edits blocked input"')


if __name__ == "__main__":
    unittest.main()
