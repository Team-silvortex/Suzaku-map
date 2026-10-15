#!/usr/bin/env python3
"""Source-only guards; no fixture imports, native/UI or desktop actions.

Passing does not prove native, physical-keyboard or installation acceptance.
Only the legacy private suite/runner are frozen; production/corpus are not.
"""
import ast
import hashlib
from pathlib import Path
import textwrap
import unittest
from fixtures.native_partition_projection import historical_source

ROOT = Path(__file__).resolve().parent.parent
SOURCE_PATH = ROOT / "scripts/test-custom-shortcuts-ibus.py"
SOURCE = SOURCE_PATH.read_text()
TREE = ast.parse(SOURCE)
# Independently captured before the new helper and adjacent main call.
FIXTURE_HASH = "ea2c9e0ed59a791d479dc46723142fffa4468fb7d4128ac5ae6ad29a15db4dc1"
RUNNER_HASH = "521c5da0c7cc6a586e64fdb7fd57312a1d534363a3b9ccce376fbc57b748adbb"


def only(nodes):
    nodes = list(nodes)
    if len(nodes) != 1:
        raise AssertionError(f"Expected exactly one AST match, found {len(nodes)}")
    return nodes[0]


def same(left, right):
    return ast.dump(left, include_attributes=False) == ast.dump(right, include_attributes=False)


def stmt(text):
    return ast.parse(text).body[0]


def function(name, owner=TREE):
    return only(node for node in owner.body if isinstance(node, ast.FunctionDef) and node.name == name)


def helper(name):
    return function(name, function("run_long_checks"))


def outer_try(node):
    return only(item for item in node.body if isinstance(item, ast.Try))


def loop(body, target):
    return only(item for item in body if isinstance(item, ast.For)
                and same(item.target, stmt(target + " = None").targets[0]))


def project(source):
    """Remove only the new definition/two blanks and adjacent single call."""
    tree = ast.parse(source)
    new = function("run_long_checks", tree)
    if new.decorator_list:
        raise AssertionError("Unexpected helper decorator")
    body = outer_try(function("main", tree)).body
    old = only(item for item in body if same(item, stmt("run_checks(fixture)")))
    call = only(item for item in body if same(item, stmt("run_long_checks(fixture)")))
    if body.index(call) != body.index(old) + 1:
        raise AssertionError("New call must immediately follow the legacy call")
    only(item for item in ast.walk(tree) if isinstance(item, ast.Call)
         and isinstance(item.func, ast.Name) and item.func.id == "run_long_checks")
    lines = source.splitlines(keepends=True)
    if lines[new.end_lineno:new.end_lineno + 2] != ["\n", "\n"]:
        raise AssertionError("Expected exactly two following helper blanks")
    if lines[call.lineno - 1:call.end_lineno] != ["        run_long_checks(fixture)\n"]:
        raise AssertionError("Expected the exact single adjacent call line")
    return "".join(line for index, line in enumerate(lines)
                   if not new.lineno - 1 <= index < new.end_lineno + 2 and index != call.lineno - 1)


class CustomLongCoverageTests(unittest.TestCase):
    def sequence(self, body, contract):
        """Require ordered direct statements, not comments/dormant strings."""
        position = 0
        for expected in ast.parse(textwrap.dedent(contract)).body:
            matches = [index for index in range(position, len(body)) if same(body[index], expected)]
            self.assertTrue(matches, ast.unparse(expected))
            position = matches[0] + 1

    def test_parse_without_execution_and_preserve_whole_legacy_fixture_and_runner(self):
        compile(TREE, str(SOURCE_PATH), "exec")
        self.assertEqual(hashlib.sha256(project(SOURCE).encode()).hexdigest(), FIXTURE_HASH)
        self.assertEqual(hashlib.sha256(historical_source(ROOT / "scripts/test-linux-ci.sh").encode()).hexdigest(), RUNNER_HASH)

    def test_projection_rejects_moved_duplicate_or_argument_changed_call(self):
        exact = "        run_checks(fixture)\n        run_long_checks(fixture)\n"
        self.assertEqual(SOURCE.count(exact), 1)
        for replacement in [exact.replace("run_long_checks(fixture)", "run_long_checks(other)"),
                            exact.replace("run_long_checks(fixture)", "run_long_checks(fixture=fixture)"),
                            "        run_long_checks(fixture)\n        run_checks(fixture)\n",
                            exact + "        run_long_checks(fixture)\n"]:
            with self.subTest(replacement=replacement), self.assertRaises(AssertionError):
                project(SOURCE.replace(exact, replacement, 1))
        changed = SOURCE.replace("time.sleep(0.01)", "time.sleep(0.02)", 1)
        self.assertNotEqual(hashlib.sha256(project(changed).encode()).hexdigest(), FIXTURE_HASH)

    def test_fixed_eight_authored_cases_plus_two_page_cases(self):
        node = function("run_long_checks")
        self.sequence(node.body, '''
            profiles = ["standard", "home-row"]
            cases = [("en", "Earlier note. " * 30, "please sen", "please send", " me the details."), ("zh-Hans", "前文。" * 100, "hui yi", "会议", "什么时候开始？")]
        ''')
        profile = loop(outer_try(node).body, "profile")
        language = loop(profile.body, "language, prefix, tail, word, suffix")
        kind = loop(language.body, "kind")
        self.assertEqual([ast.unparse(item.iter) for item in [profile, language, kind]],
                         ["profiles", "cases", "['word', 'sentence']"])
        self.sequence(profile.body[:profile.body.index(language)], '''
            fixture.update({"llm_enabled": False, "shortcut_profile": profile, "shortcut_bindings": BINDINGS})
            assert fixture.settings()["shortcut_bindings"] == BINDINGS
            assert not fixture.settings()["llm_enabled"]
        ''')
        self.sequence(kind.body, 'raw = prepare(language, prefix, tail)\nexpected = prefix + word + (suffix if kind == "sentence" else "")\nadopt(expected, kind)\nchecked += 1')
        self.assertEqual(sum(isinstance(item, ast.AugAssign) and ast.unparse(item) == "checked += 1" for item in ast.walk(node)), 2)
        completed = node.body[node.body.index(outer_try(node)) + 1:]
        self.sequence(completed, "assert checked == 10, checked")
        prints = [item for item in ast.walk(node) if isinstance(item, ast.Call)
                  and isinstance(item.func, ast.Name) and item.func.id == "print"]
        self.assertEqual(len(prints), 1)
        self.assertIsInstance(completed[-1], ast.Expr)
        self.assertIs(completed[-1].value, prints[0], "Report PASS only after cleanup and checked == 10")

    def test_owned_prefix_then_each_real_native_tail_is_drained_and_lossless(self):
        node = helper("prepare")
        self.sequence(node.body, '''
            peer.reset(watch)
            fixture.control("L" + language)
            frame = ready()
            assert not frame["seed"]
            assert fixture.command(f'A{frame["host"]} {frame["revision"]} T{prefix}') == b"1"
            typed = prefix
            for character in tail:
                assert peer.key(ibus.unicode_to_keyval(character))
                typed += character
                wait(lambda: watch.latest["seed"] == typed, "long custom drained native tail")
            raw = prefix + tail
            assert len(raw) > 256 and ready()["seed"] == raw
            assert any(row["text"] == raw for row in watch.latest["candidates"])
            assert all(row["text"].startswith(prefix) and row["source"] == "local" for row in watch.latest["candidates"])
            return raw
        ''')

    def test_fixed_payload_selection_uses_custom_rows_and_kind_not_labels(self):
        self.sequence(helper("select_text").body, '''
            candidates = ready()["candidates"]
            index = next(i for i, row in enumerate(candidates) if row["text"] == expected and row["source"] == "local" and (kind is None or row["kind"] == kind))
            for _ in range(len(candidates)):
                selected = ready()["selected"]
                if selected == index:
                    return
                forward = selected < index
                navigation(ibus.KEY_n if forward else ibus.KEY_p, ctrl, selected + (1 if forward else -1))
            raise AssertionError("long custom selection did not reach the fixed payload")
        ''')

    def test_navigation_requires_real_revision_or_whole_snapshot_boundary(self):
        self.sequence(helper("ready").body, 'frame = fixture.fresh()\nwait(lambda: watch.latest == frame, "long custom current snapshot publication")\nassert frame["focused"] and not frame["private"]\nreturn frame')
        self.sequence(helper("navigation").body, '''
            before, commits = ready(), peer.commits.copy()
            assert peer.key(key, mask), "long custom navigation was not consumed"
            if target == before["selected"]:
                fixture.unchanged(before, peer, commits, "long custom boundary changed full snapshot")
            else:
                wait(lambda: watch.latest["revision"] > before["revision"] and watch.latest["selected"] == target, "long custom navigation acknowledgement")
                after = ready()
                for field in ["host", "context", "seed", "candidates", "language"]:
                    assert after[field] == before[field], f"long custom navigation changed {field}"
                assert peer.commits == commits, "long custom navigation committed text"
        ''')

    def test_adoption_is_one_shot_until_matching_physical_release(self):
        self.sequence(helper("adopt").body, '''
            select_text(expected, kind)
            before, commits = ready(), peer.commits.copy()
            assert peer.key(ibus.KEY_A, alt | shift | locks, code=38)
            wait(lambda: watch.latest["revision"] > before["revision"] and watch.latest["seed"] == expected, "long custom editable adoption")
            adopted = ready()
            for key, mask in [(ibus.KEY_A, alt | shift), (ibus.KEY_a, 0), (ibus.KEY_a, shift)]:
                assert peer.key(key, mask, code=38)
                fixture.unchanged(adopted, peer, commits, "long custom held adoption repeated")
            assert not peer.key(ibus.KEY_a, release, code=39)
            assert peer.key(ibus.KEY_a, 0, code=38)
            fixture.unchanged(adopted, peer, commits, "long custom wrong release freed adoption")
            assert not peer.key(ibus.KEY_a, release, code=38)
            fixture.unchanged(adopted, peer, commits, "long custom matching release edited text")
        ''')

    def test_full_undo_space_word_to_sentence_and_literal_digits_are_required(self):
        profile = loop(outer_try(function("run_long_checks")).body, "profile")
        kind = loop(loop(profile.body, "language, prefix, tail, word, suffix").body, "kind")
        self.sequence(kind.body, '''
            adopt(expected, kind)
            navigation(ibus.KEY_p, ctrl, 0)
            navigation(ibus.KEY_B, alt | shift, 0)
            assert peer.key(ibus.KEY_BackSpace)
            wait(lambda: watch.latest["seed"] == raw, "long custom undo restores full raw draft")
            adopt(expected, kind)
            assert peer.key(ibus.KEY_space)
            draft = expected + " "
            wait(lambda: watch.latest["seed"] == draft, "long custom Space stays in the draft")
            if kind == "word":
                sentence = draft + (suffix.lstrip(" ") if language == "en" else suffix)
                adopt(sentence, "sentence")
                assert peer.key(ibus.KEY_BackSpace)
                wait(lambda: watch.latest["seed"] == draft, "long custom sentence undo preserves full adopted word and Space")
                adopt(sentence, "sentence")
                assert peer.key(ibus.KEY_space)
                draft = sentence + " "
                wait(lambda: watch.latest["seed"] == draft, "long custom sentence Space continuity")
            assert peer.key(ibus.KEY_7, alt), "Alt+digit must remain literal after long adoption"
            expected_commit = draft + "7"
            wait(lambda: watch.latest["seed"] == expected_commit, "long custom literal digit")
            finish(expected_commit, commits)
            checked += 1
        ''')

    def test_second_page_and_forward_backward_full_boundaries_are_mandatory(self):
        profile = loop(outer_try(function("run_long_checks")).body, "profile")
        self.sequence(profile.body, '''
            prefix = "note " * 60
            raw = prepare("en", prefix, "hel")
            count = len(ready()["candidates"])
            assert count > PAGE_SIZE, "long custom paging requires a real second fallback page"
            navigation(ibus.KEY_n, ctrl, 1)
            navigation(ibus.KEY_p, ctrl, 0)
            navigation(ibus.KEY_F, ctrl | shift | locks, PAGE_SIZE)
            navigation(ibus.KEY_B, alt | shift | locks, 0)
            last_start = (count - 1) // PAGE_SIZE * PAGE_SIZE
            for start in range(PAGE_SIZE, last_start + 1, PAGE_SIZE):
                navigation(ibus.KEY_f, ctrl | shift, start)
            navigation(ibus.KEY_f, ctrl | shift, last_start)
            for start in range(last_start - PAGE_SIZE, -1, -PAGE_SIZE):
                navigation(ibus.KEY_b, alt | shift, start)
            navigation(ibus.KEY_b, alt | shift, 0)
            expected = prefix + "hello"
            adopt(expected, "word")
            navigation(ibus.KEY_b, alt | shift, 0)
            assert peer.key(ibus.KEY_BackSpace)
            wait(lambda: watch.latest["seed"] == raw, "long custom paged adoption full undo")
            adopt(expected, "word")
            finish(expected, commits)
            checked += 1
        ''')

    def test_commit_is_exactly_once_and_empty_enter_returns_to_the_app(self):
        self.sequence(helper("finish").body, '''
            select_text(expected)
            before = ready()
            assert peer.commits == commits, "long custom draft committed before Enter"
            assert peer.key(ibus.KEY_Return, code=36)
            wait(lambda: not watch.latest["seed"] and watch.latest["context"] > before["context"] and peer.commits == commits + [expected], "long custom exact single commit")
            empty = ready()
            assert not peer.key(ibus.KEY_Return, release, code=36)
            assert not peer.key(ibus.KEY_Return, code=36), "empty Enter should return to the application"
            fixture.unchanged(empty, peer, commits + [expected], "long custom Enter duplicated commit")
        ''')

    def test_deadlines_failures_and_l_then_narrow_u_restoration_are_preserved(self):
        node = function("run_long_checks")
        for item in ast.walk(node):
            if isinstance(item, ast.Try):
                self.assertFalse(item.handlers, "Do not catch native assertion failures")
            self.assertNotIsInstance(item, ast.Continue, "Do not skip authored rows/pages")
            if isinstance(item, ast.Call) and isinstance(item.func, ast.Name) and item.func.id == "wait":
                self.assertEqual(len(item.args), 2)
                self.assertFalse(item.keywords, "No wait deadline overrides")
            if isinstance(item, ast.Call) and isinstance(item.func, ast.Attribute):
                self.assertNotIn(item.func.attr, ["sleep", "skipTest", "Popen", "run", "select"])
            if isinstance(item, ast.Subscript) and isinstance(item.slice, ast.Constant):
                self.assertNotEqual(item.slice.value, "label", "Decorated labels are not payloads")
        self.assertEqual(len(outer_try(node).finalbody), 4)
        self.sequence(outer_try(node).finalbody, '''
            with preserve_primary_failure("long custom owned engine teardown"):
                peer.close()
            with preserve_primary_failure("long custom owned subscription teardown"):
                watch.close()
            with preserve_primary_failure("long custom private language restoration"):
                fixture.control("L" + saved["language"])
            with preserve_primary_failure("long custom private settings restoration"):
                fixture.update({"llm_enabled": saved["llm_enabled"], "shortcut_profile": saved["shortcut_profile"], "shortcut_bindings": saved["shortcut_bindings"]})
                assert fixture.settings() == saved
                assert json.loads(fixture.config.read_bytes()) == saved
        ''')


if __name__ == "__main__":
    unittest.main()
