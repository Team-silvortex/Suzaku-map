#!/usr/bin/env python3
"""Lightweight vocabulary partition checks, not real GTK/IBus acceptance.

Evaluate only authored case declarations and selector expressions. Shell tests
stub child dispatch; they neither build binaries nor create a display or bus.
"""
import ast
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import subprocess
from types import SimpleNamespace
import unittest


ROOT = Path(__file__).resolve().parent.parent
SOURCE = (ROOT / "scripts/test-linux-vocabulary.py").read_text()
TREE = ast.parse(SOURCE)
RUNNER = (ROOT / "scripts/test-linux-apps.sh").read_text()
REQUEST_CASES = [
    ("en", "could i borrow your cha", "could i borrow your charger",
     "could i borrow your charger until tonight?"),
    ("zh-Hans", "jie zhi bi", "借支笔", "借支笔填个名字，马上还你。"),
    ("ja", "sagashimono", "探し物", "探し物があるので、少し待ってください。"),
    ("en", "i can return it toda", "i can return it today",
     "i can return it today after work."),
    ("zh-Hans", "bang wo di zhi jin", "帮我递纸巾", "帮我递纸巾，我手上有水。"),
    ("ja", "bunbougu", "文房具", "文房具を少し借りてもいいですか。"),
]
CARRY_CASES = [
    ("en", "i need to bring my gla", "i need to bring my glasses",
     "i need to bring my glasses so i can read at work."),
    ("zh-Hans", "wo xian huan shuang xie", "我先换双鞋",
     "我先换双鞋再出门，你在门口等我一下。"),
    ("ja", "boushi", "帽子", "帽子を忘れずに持っていきます。"),
    ("en", "please put on your ja", "please put on your jacket",
     "please put on your jacket before we leave."),
    ("zh-Hans", "wei jin bie wang le", "围巾别忘了", "围巾别忘了，晚上回来可能有点冷。"),
    ("ja", "mochimono", "持ち物", "持ち物を出かける前に確認します。"),
]
CARE_CASES = [
    ("en", "i need to wash my ha", "i need to wash my hands",
     "i need to wash my hands before we leave."),
    ("zh-Hans", "wo xian xi ba lian", "我先洗把脸", "我先洗把脸再出门，你先坐一会儿。"),
    ("ja", "hamigaki", "歯磨き", "歯磨きを済ませてから出かけます。"),
    ("en", "please leave the towel he", "please leave the towel here",
     "please leave the towel here by the sink."),
    ("zh-Hans", "yong wan fang hui yuan chu", "用完放回原处", "用完放回原处，下次拿起来方便。"),
    ("ja", "tojimari", "戸締まり", "戸締まりを確認してから出ます。"),
]
TRANSITION_CASES = [
    ("zaishiyi", "在释义", "再试一次",
     ("再试一次看看能否连接。", "再试一次，不行的话稍后再处理。")),
    ("chumenqianshoushiyixi", "出门签收释义西", "出门前收拾一下",
     ("出门前收拾一下，别把东西落在桌上。", "出门前收拾一下，回来就不用再找了。")),
]
ADOPTED_TRANSITION_CASES = [
    ("you hui quan", "优惠券", "hai zai", "优惠券还在", "youxi", "有西",
     "优惠券还在有效期", "优惠券还在有效期内。"),
    ("deng lu shi bai", "登录失败", "hou ke yi", "登录失败后可以", "zaishiyi", "在释义",
     "登录失败后可以再试一次", "登录失败后可以再试一次。"),
]


def assigned_names(node):
    return {target.id for target in node.targets if isinstance(target, ast.Name)}


def case_selection(scope):
    namespace = {"Counter": Counter,
                 "apps": SimpleNamespace(os=SimpleNamespace(
                     environ={"SUZAKU_VOCABULARY_QA_SCOPE": scope}))}
    nodes = []
    for node in TREE.body:
        if isinstance(node, ast.Assign) and any(
                name.endswith("CASES") or name in {"scope", "PACK_SCOPES"}
                for name in assigned_names(node)):
            nodes.append(node)
        elif isinstance(node, (ast.If, ast.Assert)) and any(
                isinstance(item, ast.Name)
                and item.id in {"scope", "PART_CASES", "PACK_PART_CASES"}
                for item in ast.walk(node)):
            nodes.append(node)
    exec(compile(ast.Module(body=nodes, type_ignores=[]), "case-selection", "exec"), namespace)
    return namespace


def tuple_digest(cases):
    return hashlib.sha256(json.dumps(cases, ensure_ascii=False,
                                    separators=(",", ":")).encode()).hexdigest()


def shape_digest(node):
    return hashlib.sha256(ast.dump(node, include_attributes=False).encode()).hexdigest()


class VocabularyPartitionTests(unittest.TestCase):
    def test_adopted_transition_is_four_focused_variants_not_a_complete_gate(self):
        cases = case_selection("adopted-transition")
        self.assertEqual(cases["ADOPTED_TRANSITION_CASES"], ADOPTED_TRANSITION_CASES)
        self.assertEqual(cases["CASES"], [])
        self.assertEqual(cases["TRANSITION_CASES"], TRANSITION_CASES)
        self.assertEqual(self.progress_counts("adopted-transition"), (0, 0))
        self.assertEqual(tuple_digest(cases["ALL_CASES"]),
                         "4631228e14e1714e79b6b33f3c08f2edeb3bef31d6b0a52cf5f2f77ff1133f2e")
        self.assertEqual(tuple_digest(cases["PACK_CASES"]),
                         "8a38a2f4835056119e0c988a207c8a82411d9d53208308a52cd38ef19a0eae47")
        result = self.shell_selection("adopted-transition", execute=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "")
        for invalid in ["adopted-transition ", "adopted-transitions", "adopted-transition-part-1"]:
            with self.subTest(scope=invalid), self.assertRaises(ValueError):
                case_selection(invalid)
            self.assertEqual(self.shell_selection(invalid).returncode, 2)

    def test_adopted_transition_builds_all_han_from_physical_spelling_and_numbers(self):
        functions = {node.name: node for node in TREE.body if isinstance(node, ast.FunctionDef)}
        helper = functions["check_adopted_transitions"]
        chain = next(node for node in helper.body if isinstance(node, ast.For))
        variants = next(node for node in chain.body if isinstance(node, ast.For))
        self.assertEqual(ast.unparse(chain.iter), "ADOPTED_TRANSITION_CASES")
        self.assertEqual(ast.literal_eval(variants.iter), ("", " "))
        self.assertEqual(len(ADOPTED_TRANSITION_CASES) * len(ast.literal_eval(variants.iter)), 4)
        calls = [node for node in ast.walk(helper) if isinstance(node, ast.Call)]
        physical = [node for node in calls if ast.unparse(node.func) == "x.type"]
        self.assertEqual([ast.unparse(node.args[0]) for node in physical],
                         ["start_reading", "bridge_reading", "tail_reading"])
        commands = [node for node in calls if ast.unparse(node.func) == "apps.command"]
        self.assertEqual([ast.literal_eval(node.args[0]) for node in commands], ["Lzh-Hans"])
        adoptions = [node for node in calls if ast.unparse(node.func) == "transition_choose"]
        # Starting dictionary Word and composed Han progress deliberately differ.
        self.assertEqual([(ast.unparse(node.args[-3]), ast.literal_eval(node.args[-2]),
                           ast.literal_eval(node.args[-1])) for node in adoptions[:4]],
                         [("start_word", "word", 0), ("start_word", "word", 0),
                          ("adopted", "sentence", 0), ("adopted", "sentence", 0)])
        source = ast.unparse(helper)
        for required in ["KEY_BackSpace", "KEY_space", "raw_bridge", "raw_tail", "literal",
                         "transition_presented", "transition_target_index", "apps.commit",
                         "transition_committed_once", "variants == 4",
                         "not a complete builtin or packs gate"]:
            self.assertIn(required, source)
        function = functions["check_vocabulary"]
        focused = next(node for node in function.body if isinstance(node, ast.If)
                       and ast.unparse(node.test) == "scope == 'adopted-transition'")
        self.assertIsInstance(focused.body[-1], ast.Return)
        self.assertIn("check_adopted_transitions", ast.unparse(focused))
        self.assertLess(function.body.index(focused),
                        next(index for index, node in enumerate(function.body) if isinstance(node, ast.For)))
        panel = next(node for node in function.body if isinstance(node, ast.Assign)
                     and "panel" in assigned_names(node))
        self.assertTrue(eval(compile(ast.Expression(panel.value.test), "panel-scope", "eval"),
                             {"scope": "adopted-transition", "PACK_SCOPES": case_selection("all")["PACK_SCOPES"]}))

    def test_existing_raw_transition_and_shared_helpers_are_unchanged(self):
        expected = {
            "transition_target_index": "afce290cea9da089c12e473a55ab1c1dc4900c6e24044d498fbf6930ce6577b6",
            "transition_presented": "5076bb432862c7a329a46668f8f33e222c9bfa46352fe0c9ab15a732b8169792",
            "transition_choose": "a5dc596f8fdaf4885b08fe2abd51a2f8b8f152cbb72cd09595605a4adcbb5f3a",
            "transition_committed_once": "eb5ab651087bbf69c4826ed2aa66c9fce4125e3db8fbdafe4d0ec9aca42fd90a",
            "check_whole_reading_transitions": "c3264c324f7b076fc55057a9567dd117a3450f01dbcf201bebfee913c91ca48b",
        }
        functions = {node.name: node for node in TREE.body if isinstance(node, ast.FunctionDef)}
        for name, fingerprint in expected.items():
            with self.subTest(function=name):
                self.assertEqual(shape_digest(functions[name]), fingerprint)

    def test_transition_is_fixed_focused_and_excluded_from_complete_gates(self):
        cases = case_selection("transition")
        self.assertEqual(cases["TRANSITION_CASES"], TRANSITION_CASES)
        self.assertEqual(cases["CASES"], [])
        self.assertEqual(self.progress_counts("transition"), (0, 0))
        self.assertEqual(len(cases["ALL_CASES"]), 167)
        self.assertEqual(tuple_digest(cases["ALL_CASES"]),
                         "4631228e14e1714e79b6b33f3c08f2edeb3bef31d6b0a52cf5f2f77ff1133f2e")
        self.assertEqual(len(cases["PACK_CASES"]), 74)
        self.assertEqual(tuple_digest(cases["PACK_CASES"]),
                         "8a38a2f4835056119e0c988a207c8a82411d9d53208308a52cd38ef19a0eae47")
        result = self.shell_selection("transition", execute=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "")
        for invalid in ["transition ", "transitions", "transition-part-1"]:
            with self.subTest(scope=invalid), self.assertRaises(ValueError):
                case_selection(invalid)
            self.assertEqual(self.shell_selection(invalid).returncode, 2)

    def test_transition_uses_independent_helper_without_entering_old_case_loops(self):
        function = next(node for node in TREE.body
                        if isinstance(node, ast.FunctionDef) and node.name == "check_vocabulary")
        focused = next(node for node in function.body if isinstance(node, ast.If)
                       and ast.unparse(node.test) == "scope == 'transition'")
        self.assertIsInstance(focused.body[-1], ast.Return)
        self.assertIn("check_whole_reading_transitions", ast.unparse(focused))
        first_loop = next(node for node in function.body if isinstance(node, ast.For))
        self.assertLess(function.body.index(focused), function.body.index(first_loop))
        panel = next(node for node in function.body if isinstance(node, ast.Assign)
                     and "panel" in assigned_names(node))
        self.assertTrue(eval(compile(ast.Expression(panel.value.test), "panel-scope", "eval"),
                             {"scope": "transition", "PACK_SCOPES": case_selection("all")["PACK_SCOPES"]}))
        helper = next(node for node in TREE.body if isinstance(node, ast.FunctionDef)
                      and node.name == "check_whole_reading_transitions")
        source = ast.unparse(helper)
        for required in ["TRANSITION_CASES", "transition_choose", "KEY_BackSpace", "KEY_space",
                         "KEY_Escape", "apps.commit", "transition_committed_once",
                         "not a complete builtin or packs gate"]:
            self.assertIn(required, source)
        self.assertNotIn("apps.command(f'A", source)
        self.assertNotIn("apps.command('A", source)

    def test_transition_target_requires_exact_unique_typed_local_candidates_and_page_bound(self):
        function = next(node for node in TREE.body if isinstance(node, ast.FunctionDef)
                        and node.name == "transition_target_index")
        namespace = {}
        exec(compile(ast.Module(body=[function], type_ignores=[]), "transition-target", "exec"), namespace)
        locate = namespace["transition_target_index"]
        def candidate(text="target", kind="sentence", source="local", label="ˢ"):
            return {"text": text, "kind": kind, "source": source, "ibus_label": label}
        choices = [candidate(str(index)) for index in range(12)]
        choices[7] = candidate()
        self.assertEqual(locate(choices, "target", "sentence", 1), 7)
        for invalid in [choices + [candidate("extra")],
                        [candidate(source="model")], [candidate(kind="word")],
                        [candidate(label="word")], [candidate(), candidate()]]:
            with self.subTest(choices=invalid), self.assertRaises(AssertionError):
                locate(invalid, "target", "sentence")
        with self.assertRaises(AssertionError):
            locate(choices, "target", "sentence", 0)
        # Execute only the page/number helper with in-memory key stubs. This
        # verifies absolute-to-page-local mapping without GI, a bus or a display.
        choose = next(node for node in TREE.body if isinstance(node, ast.FunctionDef)
                      and node.name == "transition_choose")
        symbols = SimpleNamespace(KEY_Page_Down=100, KEY_Page_Up=99, KEY_1=1)
        for index, selected, expected in [(8, 0, [100, 3]), (2, 6, [99, 3]), (1, 0, [2])]:
            with self.subTest(target=index, selected=selected):
                rows = [candidate(str(number)) for number in range(12)]
                state = {"candidates": rows, "selected": selected, "seed": "raw", "revision": 1}
                keys = []

                def key(symbol):
                    keys.append(symbol)
                    if symbol in (100, 99):
                        state["selected"] = 6 if symbol == 100 else 0
                    else:
                        slot = state["selected"] // 6 * 6 + symbol - 1
                        state["seed"] = rows[slot]["text"]
                    state["revision"] += 1

                def presented(x, document, window, panel, seed, start=None):
                    self.assertEqual(state["seed"], seed)
                    page = state["selected"] // 6 * 6
                    if start is not None:
                        self.assertEqual(page, start)
                    return {"native_page": {"candidates": [
                        {"index": number, "number": number - page + 1, "text": rows[number]["text"]}
                        for number in range(page, page + 6)]}}

                namespace.update(IBus=symbols, transition_presented=presented,
                                 apps=SimpleNamespace(watch=SimpleNamespace(latest=state),
                                     seed_is=lambda seed: state["seed"] == seed,
                                     wait=lambda predicate, label: self.assertTrue(predicate(), label)))
                exec(compile(ast.Module(body=[choose], type_ignores=[]),
                             "transition-choose", "exec"), namespace)
                namespace["transition_choose"](SimpleNamespace(key=key), None, None, None,
                                                "raw", str(index), "sentence", index // 6)
                self.assertEqual(keys, expected)
                self.assertEqual(state["seed"], str(index))

    def test_transition_page_and_once_commit_observe_actual_panel_editor_and_physical_keys(self):
        functions = {node.name: node for node in TREE.body if isinstance(node, ast.FunctionDef)}
        rendered = ast.unparse(functions["transition_presented"])
        for required in ["runtime_font", "native_revision", "native_context", "native_cursor",
                         "viewport_settled", "native_page", "selected", "number", "index",
                         "panel.poll()", "x.focused()", "editor_observers", "saved_text()"]:
            self.assertIn(required, rendered)
        choose = ast.unparse(functions["transition_choose"])
        for required in ["KEY_Page_Down", "KEY_Page_Up", "KEY_1 + index % 6",
                         "transition_presented", "transition_target_index"]:
            self.assertIn(required, choose)
        committed = ast.unparse(functions["transition_committed_once"])
        for required in ["apps.seed_is('')", "saved_text()", "apps.pump()", "monotonic()"]:
            self.assertIn(required, committed)
        self.assertNotIn("KEY_Return", committed)

    def test_old_all_and_pack_tuples_keep_exact_order_and_expectations(self):
        cases = case_selection("all")
        self.assertEqual(len(cases["ALL_CASES"]), 167)
        self.assertEqual(tuple_digest(cases["ALL_CASES"][:149]),
                         "01b5933c4956cae3948831e9773856684a9497c972ae1067cd2d885cc612d509")
        self.assertEqual(tuple_digest(cases["ALL_CASES"][:155]),
                         "855516fe494082b68e41eae1bdf1c7e20de6ada920a4418aadae8943fb03b390")
        self.assertEqual(tuple_digest(cases["ALL_CASES"][:161]),
                         "4e3584f43a64f93764b1c83e251b475a562cd966f6c84bb94eba23ca06ebb22c")
        self.assertEqual(len(cases["PACK_CASES"]), 74)
        self.assertEqual(tuple_digest(cases["PACK_CASES"][:56]),
                         "7b49d086f856d75164caa006d8caf37313fa2d2ab493f98859de3e08106adafb")
        self.assertEqual(tuple_digest(cases["PACK_CASES"][:62]),
                         "05fcbeab82415107f3cf87535f0238d9c9b6603d7587fb62a0c54965c8a46245")
        self.assertEqual(tuple_digest(cases["PACK_CASES"][:68]),
                         "09fa806e082a67a1432c931dca11a763122b8fb84d95959f7f8bec4c5d4ec7b9")

    def test_new_requests_are_fixed_and_present_in_both_complete_gates(self):
        cases = case_selection("requests")
        self.assertEqual(cases["REQUEST_CASES"], REQUEST_CASES)
        self.assertEqual(cases["CASES"], REQUEST_CASES)
        # Protect this batch's historical positions, not the ever-growing tail.
        self.assertEqual(cases["ALL_CASES"][149:155], REQUEST_CASES)
        self.assertEqual(cases["PACK_CASES"][56:62], REQUEST_CASES)

    def test_new_carry_cases_are_fixed_and_present_in_both_complete_gates(self):
        cases = case_selection("carry")
        self.assertEqual(cases["CARRY_CASES"], CARRY_CASES)
        self.assertEqual(cases["CASES"], CARRY_CASES)
        self.assertEqual(cases["ALL_CASES"][155:161], CARRY_CASES)
        self.assertEqual(cases["PACK_CASES"][62:68], CARRY_CASES)

    def test_new_care_cases_are_fixed_and_present_in_both_complete_gates(self):
        cases = case_selection("care")
        self.assertEqual(cases["CARE_CASES"], CARE_CASES)
        self.assertEqual(cases["CASES"], CARE_CASES)
        self.assertEqual(cases["ALL_CASES"][-6:], CARE_CASES)
        self.assertEqual(cases["PACK_CASES"][-6:], CARE_CASES)

    def test_all_and_pack_partitions_preserve_counts_including_duplicates(self):
        cases = case_selection("all")
        for key, complete, size in [("PART_CASES", "ALL_CASES", 3),
                                    ("PACK_PART_CASES", "PACK_CASES", 2)]:
            with self.subTest(parts=key):
                parts = cases[key]
                self.assertEqual(len(parts), size)
                self.assertEqual(Counter(case for part in parts.values() for case in part),
                                 Counter(cases[complete]))
                for name, part in parts.items():
                    self.assertEqual({case[0] for case in part}, {"en", "zh-Hans", "ja"})
                    self.assertEqual(case_selection(name)["CASES"], part)
        self.assertEqual([len(part) for part in cases["PACK_PART_CASES"].values()], [37, 37])

    def test_python_unknown_scopes_fail_closed(self):
        for scope in ["", "unknown", "pack", "packs-part-3", "requests ", "carry ", "care "]:
            with self.subTest(scope=scope), self.assertRaises(ValueError):
                case_selection(scope)

    def progress_counts(self, scope):
        function = next(node for node in TREE.body
                        if isinstance(node, ast.FunctionDef) and node.name == "check_vocabulary")
        namespace = {"scope": scope}
        nodes = [node for node in function.body if isinstance(node, ast.Assign)
                 and assigned_names(node) & {"progress_cases", "japanese_progress_cases"}]
        exec(compile(ast.Module(body=nodes, type_ignores=[]), "progress-selection", "exec"), namespace)
        return (len(namespace["progress_cases"]), len(namespace["japanese_progress_cases"]))

    def test_all_five_pack_progress_controls_run_once_with_historical_and_current_totals(self):
        self.assertEqual(self.progress_counts("packs"), (3, 2))
        self.assertEqual(self.progress_counts("packs-part-1"), (3, 2))
        self.assertEqual(self.progress_counts("packs-part-2"), (0, 0))
        self.assertEqual(self.progress_counts("requests"), (3, 0))
        self.assertEqual(self.progress_counts("carry"), (3, 0))
        self.assertEqual(self.progress_counts("care"), (3, 0))
        self.assertEqual(self.progress_counts("part-1"), (3, 2))
        self.assertEqual(self.progress_counts("part-2"), (0, 0))
        self.assertEqual(self.progress_counts("part-3"), (0, 0))
        self.assertEqual(len(case_selection("packs")["PACK_CASES"][:62]) + 5, 67)
        self.assertEqual(len(case_selection("all")["ALL_CASES"][:155]) + 5 + 1, 161)
        self.assertEqual(len(case_selection("packs")["PACK_CASES"][:68]) + 5, 73)
        self.assertEqual(len(case_selection("all")["ALL_CASES"][:161]) + 5 + 1, 167)
        self.assertEqual(len(case_selection("packs")["PACK_CASES"]) + 5, 79)
        self.assertEqual(len(case_selection("all")["ALL_CASES"]) + 5 + 1, 173)

    def test_every_pack_part_prepares_all_packs_and_uses_real_companion(self):
        cases = case_selection("all")
        self.assertEqual(cases["PACK_SCOPES"], {"packs", "packs-part-1", "packs-part-2"})
        function = next(node for node in TREE.body
                        if isinstance(node, ast.FunctionDef) and node.name == "check_vocabulary")
        panel = next(node for node in function.body if isinstance(node, ast.Assign)
                     and "panel" in assigned_names(node))
        prepare = next(node for node in ast.walk(TREE) if isinstance(node, ast.If)
                       and any(isinstance(item, ast.Expr) and isinstance(item.value, ast.Call)
                               and isinstance(item.value.func, ast.Name)
                               and item.value.func.id == "prepare_packs" for item in node.body))
        for scope in ["packs", "packs-part-1", "packs-part-2"]:
            namespace = {"scope": scope, "PACK_SCOPES": cases["PACK_SCOPES"]}
            self.assertTrue(eval(compile(ast.Expression(panel.value.test), "panel-scope", "eval"), namespace))
            self.assertTrue(eval(compile(ast.Expression(prepare.test), "prepare-scope", "eval"), namespace))
        prepare_function = next(node for node in TREE.body
                                if isinstance(node, ast.FunctionDef) and node.name == "prepare_packs")
        self.assertEqual(shape_digest(prepare_function),
                         "56cec83cf19fbdfbd0fcd6fc8af989a99e2909a9f64d93da143f9462ccf221af")

    def test_real_editor_case_loops_and_paging_assertions_are_unchanged(self):
        function = next(node for node in TREE.body
                        if isinstance(node, ast.FunctionDef) and node.name == "check_vocabulary")
        loops = [node for node in function.body if isinstance(node, ast.For)]
        self.assertEqual([shape_digest(node) for node in loops], [
            "22104579a5e27fc1c1fd45144399a10146bc8792c76178f2e443c5cce6c20a56",
            "81b1351cae6fa2564c83b663287236c8012a8b126ceea5472369baa5d41c4b12",
            "9d162591d777c7872b1977824b30dc4bbc785c346b5c72a40424c1ed7adabddb"])
        paging = next(node for node in function.body if isinstance(node, ast.If)
                      and "clarification" in ast.unparse(node.test))
        self.assertEqual(shape_digest(paging),
                         "2e782c1cb4eecbe2602a9c5806c4d7f2de9ee79abdac55285fbbf75576551bca")

    def shell_function(self, name):
        signature = f"{name}() {{\n"
        self.assertEqual(RUNNER.count(signature), 1)
        start = RUNNER.index(signature)
        end = RUNNER.index("\n}\n", start) + len("\n}\n")
        return RUNNER[start:end]

    def shell_selection(self, scope, fail=None, execute=False):
        script = "set -euo pipefail\n" + self.shell_function("suzaku_apps_select_vocabulary_partitions")
        script += "\nsuzaku_apps_suite=vocabulary\nsuzaku_apps_bins=/synthetic/bin\n"
        script += "suzaku_apps_root=/synthetic/project\n"
        if execute:
            script += self.shell_function("suzaku_apps_run_vocabulary_partitions")
            script += r'''
env() {
  printf 'RUN:%s\n' "$*"
  if [[ -n ${FAIL_VOCABULARY_PARTITION:-} && $* == *"SUZAKU_VOCABULARY_QA_SCOPE=$FAIL_VOCABULARY_PARTITION "* ]]; then return 42; fi
}
suzaku_apps_select_vocabulary_partitions
suzaku_apps_run_vocabulary_partitions
'''
        else:
            script += 'suzaku_apps_select_vocabulary_partitions\nprintf "%s\\n" "${suzaku_apps_vocabulary_parts[@]}"\n'
        environment = dict(os.environ)
        environment.pop("SUZAKU_VOCABULARY_QA_SCOPE", None)
        if scope is not None:
            environment["SUZAKU_VOCABULARY_QA_SCOPE"] = scope
        environment.pop("FAIL_VOCABULARY_PARTITION", None)
        if fail is not None:
            environment["FAIL_VOCABULARY_PARTITION"] = fail
        return subprocess.run(["bash", "-c", script], env=environment,
                              capture_output=True, text=True, timeout=5)

    def test_shell_and_python_agree_on_complete_and_focused_scopes(self):
        for scope, expected in [("all", ["part-1", "part-2", "part-3"]),
                                ("packs", ["packs-part-1", "packs-part-2"])]:
            result = self.shell_selection(scope)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout.splitlines(), expected)
        for scope in ["home", "daily", "japanese", "followup", "leisure", "clarification",
                      "courtesy", "location", "time", "shopping", "transit", "contact", "routine",
                      "requests", "carry", "care", "part-1", "part-2", "part-3", "packs-part-1", "packs-part-2"]:
            self.assertTrue(case_selection(scope)["CASES"])
            result = self.shell_selection(scope)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout.strip(), "")

    def test_shell_invalid_scope_is_rejected_before_build(self):
        for scope in ["", "unknown", "pack", "packs-part-3", "requests ", "carry ", "care "]:
            result = self.shell_selection(scope)
            self.assertEqual(result.returncode, 2, result.stderr)
            self.assertIn("SUZAKU_VOCABULARY_QA_SCOPE", result.stderr)
            self.assertEqual(result.stdout, "")
        call = "suzaku_apps_select_vocabulary_partitions\n"
        self.assertEqual(RUNNER.count(call), 1)
        self.assertLess(RUNNER.index(call), RUNNER.index("  cargo build"))

    def test_unset_scope_keeps_complete_default_not_empty_scope(self):
        result = self.shell_selection(None)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.splitlines(), ["part-1", "part-2", "part-3"])

    def test_focused_scope_does_not_dispatch_or_claim_complete_success(self):
        for scope in ["requests", "carry", "care", "packs-part-1", "packs-part-2", "part-1"]:
            result = self.shell_selection(scope, execute=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout, "")

    def test_complete_dispatch_keeps_binaries_and_requires_every_part(self):
        result = self.shell_selection("packs", execute=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        runs = [line for line in result.stdout.splitlines() if line.startswith("RUN:")]
        self.assertEqual(len(runs), 2)
        for part, line in zip(["packs-part-1", "packs-part-2"], runs):
            self.assertIn(f"SUZAKU_VOCABULARY_QA_SCOPE={part}", line)
            self.assertIn("SUZAKU_APP_QA_BIN_DIR=/synthetic/bin", line)
            self.assertIn("bash /synthetic/project/scripts/test-linux-apps.sh vocabulary", line)
        self.assertIn("PASS: complete all-14-pack vocabulary scope=packs", result.stdout)
        for part in ["packs-part-1", "packs-part-2"]:
            result = self.shell_selection("packs", fail=part, execute=True)
            self.assertEqual(result.returncode, 42, result.stderr)
            self.assertNotIn("PASS: complete", result.stdout)
            if part == "packs-part-1":
                self.assertNotIn("SUZAKU_VOCABULARY_QA_SCOPE=packs-part-2", result.stdout)

    def test_existing_240_second_deadline_and_pack_binary_guard_remain(self):
        self.assertEqual(RUNNER.count("timeout --kill-after=3s 240s env"), 1)
        marker = "if [[ $suzaku_apps_suite == vocabulary ]]; then\n"
        start = RUNNER.index(marker)
        end = RUNNER.index("\nfi\n", start) + len("\nfi\n")
        for scope in ["packs", "packs-part-1", "packs-part-2", "all", "requests", "carry", "care", "part-1"]:
            script = "set -euo pipefail\nsuzaku_apps_suite=vocabulary\n"
            script += "suzaku_apps_binaries=(panel linux_ime_host)\n" + RUNNER[start:end]
            script += 'printf "%s\\n" "${suzaku_apps_binaries[@]}"\n'
            result = subprocess.run(["bash", "-c", script], capture_output=True, text=True,
                                    env=dict(os.environ, SUZAKU_VOCABULARY_QA_SCOPE=scope), timeout=5)
            self.assertEqual(result.returncode, 0, result.stderr)
            expected = ["panel", "linux_ime_host"]
            if scope in {"packs", "packs-part-1", "packs-part-2"}:
                expected.append("suzaku_tool")
            self.assertEqual(result.stdout.splitlines(), expected)


if __name__ == "__main__":
    unittest.main()
