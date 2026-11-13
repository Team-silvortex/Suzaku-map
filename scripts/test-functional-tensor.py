#!/usr/bin/env python3
"""Stdlib-only tensor fixtures and read-only architecture documentation checks."""
import copy
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import re
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout


sys.dont_write_bytecode = True
SPEC = importlib.util.spec_from_file_location(
    "functional_tensor", Path(__file__).with_name("functional-tensor.py"))
TENSOR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(TENSOR)


class TensorTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="suzaku-tensor-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        for name, content in (("src/core.rs", "fn core() {}\n"),
                              ("tests/core.rs", "test\n"), ("docs/run.md", "evidence\n")):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")
        self.data = {
            "schema_version": 1,
            "snapshot": {"date": "2026-10-03", "head": "baseline", "dirty": True,
                         "source_sha256": "", "note": "Static fixture snapshot"},
            "semantics": {
                "axes": "Architecture, function, implementation, criterion",
                "missing": "Missing coordinates and null are not zero",
                "score_zero": "Explicitly absent or failed",
                "aggregation": "No averages",
                "evidence": "History does not certify today's snapshot",
                "coverage": "Declared relationships only",
                "exclusions": ["No implicit current delivery"]
            },
            "architectures": [{"id": "A01", "title": "Core"}],
            "criteria": [{"id": key, "title": key,
                          "levels": [f"level {number}" for number in range(5)]}
                         for key in TENSOR.CRITERIA],
            "implementations": [{"id": "core", "a": "A01", "title": "Engine",
                                 "scope": "Unit fixture", "paths": ["src/core.rs"],
                                 "tests": ["tests/core.rs"]}],
            "functions": [{"id": "F01", "title": "Input", "parents": []}],
            "evidence": [{"id": "E01", "kind": "source", "paths": ["src/core.rs"],
                          "scope": "Static", "note": "Not an execution claim"}],
            "cells": [{"a": "A01", "f": "F01", "i": "core", "m": [2, 2, None, 0],
                       "evidence": ["E01"], "limit": "No real desktop acceptance"}],
        }
        self.refresh_digest()

    def refresh_digest(self):
        self.data["snapshot"]["source_sha256"] = TENSOR.source_digest(self.data, self.root)

    def invalid(self, pattern):
        with self.assertRaisesRegex(TENSOR.TensorError, pattern):
            TENSOR.validate(self.data, self.root)

    def test_valid_dirty_snapshot_preserves_null_distinct_from_zero(self):
        TENSOR.validate(self.data, self.root)
        entries = list(TENSOR.coo(self.data))
        self.assertEqual([entry["m"] for entry in entries], ["I", "T", "V", "O"])
        self.assertIsNone(entries[2]["value"])
        self.assertEqual(entries[3]["value"], 0)
        self.assertIsNone(TENSOR.slice_cells(self.data, "F01")[0]["m"]["V"])

    def test_duplicate_coordinates_are_rejected(self):
        self.data["cells"].append(copy.deepcopy(self.data["cells"][0]))
        self.invalid("duplicate coordinate")

    def test_catalog_ids_are_unique_within_each_axis(self):
        for name in ("architectures", "criteria", "implementations", "functions", "evidence"):
            with self.subTest(catalog=name):
                original = self.data[name][:]
                self.data[name].append(copy.deepcopy(self.data[name][0]))
                self.invalid("duplicate id")
                self.data[name] = original

    def test_catalog_ids_cannot_collide_across_slice_axes(self):
        original = copy.deepcopy(self.data)
        for catalog, collision in (("architectures", "F01"),
                                   ("architectures", "core"),
                                   ("implementations", "F01")):
            with self.subTest(catalog=catalog, collision=collision):
                self.data = copy.deepcopy(original)
                self.data[catalog][0]["id"] = collision
                self.invalid("axis catalog collision")

    def test_evidence_and_criteria_keep_separate_id_namespaces(self):
        self.data["evidence"][0]["id"] = "F01"
        self.data["cells"][0]["evidence"] = ["F01"]
        self.data["architectures"][0]["id"] = "I"
        self.data["implementations"][0]["a"] = "I"
        self.data["cells"][0]["a"] = "I"
        TENSOR.validate(self.data, self.root)
        self.assertEqual(len(TENSOR.slice_cells(self.data, "I")), 1)

    def test_dangling_cell_ids_and_evidence_are_rejected(self):
        for field in ("a", "f", "i"):
            with self.subTest(field=field):
                original = self.data["cells"][0][field]
                self.data["cells"][0][field] = "missing"
                self.invalid("unknown")
                self.data["cells"][0][field] = original
        self.data["cells"][0]["evidence"] = ["missing"]
        self.invalid("unknown evidence")

    def test_implementation_architecture_must_match_cell(self):
        self.data["architectures"].append({"id": "A02", "title": "Host"})
        self.data["cells"][0]["a"] = "A02"
        self.invalid("architecture does not match")
        self.data["cells"][0]["a"] = "A01"
        self.data["implementations"][0]["a"] = "missing"
        self.invalid("unknown architecture")

    def test_scores_are_integers_or_null_never_booleans(self):
        for invalid in (True, False, -1, 5, 2.0, "2"):
            with self.subTest(value=invalid):
                self.data["cells"][0]["m"][0] = invalid
                self.invalid("integer 0..4 or null")
        self.data["cells"][0]["m"][0] = None
        TENSOR.validate(self.data, self.root)

    def test_high_scores_require_referenced_evidence_of_the_right_kind(self):
        original = copy.deepcopy(self.data)
        for position, value, kind in ((1, 3, "historical_run"),
                                      (3, 1, "delivery_contract"),
                                      (3, 2, "delivery_contract")):
            with self.subTest(kind=kind):
                self.data = copy.deepcopy(original)
                self.data["cells"][0]["m"][position] = value
                self.invalid(f"requires {kind}")
                self.data["evidence"].append({"id": "E02", "kind": kind,
                                             "paths": ["docs/run.md"], "scope": "Fixture",
                                             "note": "Exact controlled scope"})
                self.invalid(f"requires {kind}")
                self.data["cells"][0]["evidence"].append("E02")
                TENSOR.validate(self.data, self.root)

    def test_history_and_contracts_cannot_claim_current_snapshot_high_scores(self):
        for number, kind in enumerate(("historical_run", "manual_acceptance", "delivery_contract"), 2):
            identifier = f"E{number:02}"
            self.data["evidence"].append({"id": identifier, "kind": kind,
                                         "paths": ["docs/run.md"], "scope": "Historical fixture",
                                         "note": "Not current snapshot acceptance"})
            self.data["cells"][0]["evidence"].append(identifier)
        original = self.data["cells"][0]["m"][:]
        for position, value in ((0, 4), (1, 4), (2, 1), (2, 4), (3, 3), (3, 4)):
            with self.subTest(criterion=TENSOR.CRITERIA[position], value=value):
                self.data["cells"][0]["m"] = original[:]
                self.data["cells"][0]["m"][position] = value
                self.invalid("schema v1 cannot verify current-snapshot")

    def test_semantics_and_snapshot_notes_are_validated(self):
        self.data["snapshot"]["note"] = ""
        self.invalid("snapshot.note")
        self.data["snapshot"]["note"] = "No current acceptance"
        self.data["semantics"]["missing"] = False
        self.invalid("semantics.missing")
        self.data["semantics"]["missing"] = "Unknown is not zero"
        self.data["semantics"]["exclusions"] = [1]
        self.invalid("semantics.exclusions")

    def test_unknown_evidence_kind_and_empty_paths_are_rejected(self):
        self.data["evidence"][0]["kind"] = "trust_me"
        self.invalid("unknown kind")
        self.data["evidence"][0]["kind"] = "source"
        self.data["evidence"][0]["paths"] = []
        self.invalid("must cite a file")

    def test_paths_reject_parent_absolute_windows_missing_and_directories(self):
        for name in ("../outside", str(self.root / "src/core.rs"),
                     "C:/outside.txt", "..\\outside", "missing.rs", "src"):
            with self.subTest(path=name):
                self.data["implementations"][0]["paths"] = [name]
                self.invalid("path")

    def test_symlink_escape_is_rejected_but_internal_file_link_is_safe(self):
        with tempfile.TemporaryDirectory(prefix="suzaku-tensor-outside-") as outside:
            target = Path(outside) / "outside.txt"
            target.write_text("outside", encoding="utf-8")
            (self.root / "escape").symlink_to(target)
            self.data["implementations"][0]["paths"] = ["escape"]
            self.invalid("symlink escapes")
        (self.root / "inside").symlink_to(self.root / "src/core.rs")
        self.data["implementations"][0]["paths"] = ["inside"]
        self.refresh_digest()
        TENSOR.validate(self.data, self.root)

    def test_digest_uses_sorted_unique_source_and_test_paths_not_evidence(self):
        expected = hashlib.sha256()
        for name in ("src/core.rs", "tests/core.rs"):
            content_hash = hashlib.sha256((self.root / name).read_bytes()).hexdigest().encode("ascii")
            expected.update(name.encode() + b"\0" + content_hash)
        self.assertEqual(TENSOR.source_digest(self.data, self.root), expected.hexdigest())
        self.data["implementations"][0]["paths"].extend(["tests/core.rs", "src/core.rs"])
        self.assertEqual(TENSOR.source_digest(self.data, self.root), expected.hexdigest())
        (self.root / "docs/run.md").write_text("new evidence", encoding="utf-8")
        self.assertEqual(TENSOR.source_digest(self.data, self.root), expected.hexdigest())
        (self.root / "src/core.rs").write_text("changed source", encoding="utf-8")
        self.invalid("digest drift")

    def test_digest_frames_adversarial_content_and_path_boundaries(self):
        first = {"a": b"foo", "b": b"b\0bar"}
        second = {"a": b"foob\0", "b": b"bar"}

        def old_unframed_bytes(contents):
            return b"".join(name.encode() + b"\0" + contents[name] for name in sorted(contents))

        self.assertEqual(old_unframed_bytes(first), old_unframed_bytes(second))
        self.data["implementations"][0]["paths"] = ["a", "b"]
        self.data["implementations"][0]["tests"] = []
        digests = []
        for contents in (first, second):
            for name, content in contents.items():
                (self.root / name).write_bytes(content)
            digests.append(TENSOR.source_digest(self.data, self.root))
        self.assertNotEqual(*digests)

    def test_parent_references_cycles_and_missing_cells_are_rejected(self):
        self.data["functions"][0]["parents"] = ["F99"]
        self.invalid("unknown parent")
        self.data["functions"][0]["parents"] = ["F01"]
        self.invalid("parent cycle")
        self.data["functions"][0]["parents"] = []
        self.data["functions"].append({"id": "F69", "title": "Future", "parents": ["F01"]})
        self.invalid("functions without cells")
        cell = copy.deepcopy(self.data["cells"][0])
        cell["f"] = "F69"
        self.data["cells"].append(cell)
        TENSOR.validate(self.data, self.root)

    def test_fixed_criteria_order_and_snapshot_types(self):
        self.data["criteria"].reverse()
        self.invalid("ordered I, T, V, O")
        self.data["criteria"].reverse()
        self.data["snapshot"]["dirty"] = 1
        self.invalid("expected a boolean")
        self.data["snapshot"]["dirty"] = True
        self.data["schema_version"] = True
        self.invalid("integer 1")

    def test_slice_by_any_axis_and_unknown_id(self):
        for identifier in ("F01", "A01", "core"):
            result = TENSOR.slice_cells(self.data, identifier)
            self.assertEqual(len(result), 1)
            self.assertEqual(result[0]["paths"], ["src/core.rs"])
            self.assertEqual(result[0]["evidence"][0]["id"], "E01")
        with self.assertRaisesRegex(TENSOR.TensorError, "unknown"):
            TENSOR.slice_cells(self.data, "missing")

    def test_cli_default_check_digest_and_coo_are_read_only(self):
        path = self.root / "docs/functional-tensor.json"
        path.write_text(json.dumps(self.data), encoding="utf-8")
        before = path.read_bytes()
        base = ["--root", str(self.root)]
        for flags in ([], ["--check"], ["--digest"], ["--coo"], ["--slice", "F01"]):
            with self.subTest(flags=flags), redirect_stdout(io.StringIO()) as output:
                self.assertEqual(TENSOR.main(base + flags), 0)
                self.assertTrue(output.getvalue())
                self.assertEqual(path.read_bytes(), before)
        self.data["snapshot"]["source_sha256"] = ""
        path.write_text(json.dumps(self.data), encoding="utf-8")
        with redirect_stdout(io.StringIO()):
            self.assertEqual(TENSOR.main(base + ["--digest"]), 0)
        with redirect_stderr(io.StringIO()) as error:
            self.assertEqual(TENSOR.main(base), 1)
        self.assertIn("source_sha256", error.getvalue())

    def test_duplicate_json_fields_are_not_silently_overwritten(self):
        path = self.root / "duplicate.json"
        path.write_text('{"schema_version": 1, "schema_version": 1}', encoding="utf-8")
        with redirect_stderr(io.StringIO()) as error:
            self.assertEqual(TENSOR.main(["--root", str(self.root), "--data", str(path)]), 1)
        self.assertIn("duplicate object field", error.getvalue())


class RepositoryArchitectureTests(unittest.TestCase):
    """Keep current summaries in sync; never rewrite historical evidence."""

    @classmethod
    def setUpClass(cls):
        cls.root = Path(__file__).resolve().parent.parent
        cls.data = json.loads((cls.root / "docs/functional-tensor.json").read_text(encoding="utf-8"))
        cls.document = (cls.root / "docs/functional-network.md").read_text(encoding="utf-8")

    def test_current_architecture_counts_match_the_inventory(self):
        match = re.search(
            r"\*\*(\d+) 个架构域、(\d+) 个功能、(\d+) 个实现切片、(\d+) 条有效关联\*\*",
            self.document)
        self.assertIsNotNone(match, "missing current architecture summary")
        self.assertEqual(tuple(map(int, match.groups())), tuple(
            len(self.data[key]) for key in ("architectures", "functions", "implementations", "cells")))
        coo = re.search(r"展开为 \*\*(\d+) 条 COO 记录\*\*", self.document)
        self.assertIsNotNone(coo, "missing current coordinate summary")
        self.assertEqual(int(coo.group(1)), len(list(TENSOR.coo(self.data))))

    def test_embedded_runtime_route_matches_the_standalone_diagram(self):
        # The separate LR diagram describes an ACK timeline, not this route map.
        diagrams = re.findall(r"```mermaid\n(flowchart TB\n.*?)\n```", self.document, re.S)
        self.assertEqual(len(diagrams), 1)
        standalone = (self.root / "docs/functional-network.mmd").read_text(encoding="utf-8")
        current_route = standalone[standalone.index("flowchart TB"):].strip()
        self.assertEqual(diagrams[0].strip(), current_route)

    def test_pack_private_implementations_remain_in_the_source_snapshot(self):
        packs = next(row for row in self.data["implementations"] if row["id"] == "packs")
        facade = (self.root / "src/lexicon/packs.rs").read_text(encoding="utf-8")
        modules = re.findall(r"^mod ([a-z_]+);$", facade, re.M)
        self.assertTrue(modules, "missing private pack implementation modules")
        self.assertEqual(len(modules), len(set(modules)))
        for module in modules:
            path = f"src/lexicon/packs/{module}.rs"
            with self.subTest(path=path):
                self.assertTrue((self.root / path).is_file())
                self.assertIn(path, packs["paths"])
        self.assertIn("tests/offline_pack_boundaries.rs", packs["tests"])

    def test_current_optional_pack_count_matches_data_not_installed_slots(self):
        match = re.search(r"当前可选目录为 \*\*(\d+) 包（EN (\d+) / ZH (\d+) / JA (\d+)）\*\*",
                          self.document)
        self.assertIsNotNone(match, "missing current optional pack summary")
        manifests = [json.loads(path.read_text(encoding="utf-8"))["manifest"]
                     for path in (self.root / "data/offline-packs").glob("*.json")]
        self.assertEqual(tuple(map(int, match.groups())), (
            len(manifests), *(sum(row["language"] == language for row in manifests)
                              for language in ("en", "zh-Hans", "ja"))))

    def test_model_compatibility_facade_does_not_compile_a_second_implementation(self):
        facade = (self.root / "src/languages/model.rs").read_text(encoding="utf-8")
        code = [line.strip() for line in facade.splitlines()
                if line.strip() and not line.lstrip().startswith("//")]
        self.assertEqual(code, ["pub use crate::model::*;"])
        library = (self.root / "src/lib.rs").read_text(encoding="utf-8")
        self.assertEqual(library.count("pub mod model;"), 1)
        self.assertFalse(any((self.root / "src/languages/model").glob("*.rs")))

    def test_canonical_model_sources_and_compatibility_test_remain_in_the_snapshot(self):
        registered = {path for row in self.data["implementations"]
                      for key in ("paths", "tests") for path in row[key]}
        model = next(row for row in self.data["implementations"] if row["id"] == "model")
        self.assertIn("src/model.rs", model["paths"])
        self.assertIn("src/languages/model.rs", model["paths"])
        self.assertIn("src/languages/llama.rs", model["paths"])
        self.assertIn("tests/model_module_compat.rs", model["tests"])
        source = (self.root / "src/model.rs").read_text(encoding="utf-8")
        modules = re.findall(r"^(?:pub )?mod ([a-z_]+);$", source, re.M)
        self.assertTrue(modules, "missing canonical model implementation modules")
        for module in modules:
            path = f"src/model/{module}.rs"
            with self.subTest(path=path):
                self.assertTrue((self.root / path).is_file())
                self.assertIn(path, registered)
        self.assertFalse(any(path.startswith("src/languages/model/") for path in registered))

    def test_model_production_transport_uses_protocol_types_not_legacy_aliases(self):
        # These four existing files keep their inline tests after this exact
        # marker. Test-only legacy imports remain deliberate compatibility checks.
        marker = "#[cfg(test)]\nmod tests {"
        for path in ("src/model.rs", "src/model/runtime.rs", "src/model/transport.rs",
                     "src/model/translation.rs"):
            with self.subTest(path=path):
                source = (self.root / path).read_text(encoding="utf-8")
                self.assertEqual(source.count(marker), 1, "review the production/test boundary")
                production = source.split(marker, 1)[0]
                self.assertIn("use crate::prediction", production)
                self.assertIn("PredictionError", production)
                self.assertNotRegex(production, r"\bLlm(?:Cancellation|ProviderError|Completion(?:Provider|Request)?|LanguagePlugin)\b")
                if path != "src/model/translation.rs":
                    self.assertIn("PredictionCancellation", production)
                    self.assertNotIn("crate::languages", production)
                else:
                    self.assertNotIn("llm::", production)

    def test_legacy_model_adapter_has_one_private_owner(self):
        facade = (self.root / "src/model.rs").read_text(encoding="utf-8")
        bridge = (self.root / "src/model/legacy.rs").read_text(encoding="utf-8")
        implementation = "impl LlmCompletionProvider for HttpModelProvider"
        self.assertNotIn(implementation, facade)
        self.assertEqual(bridge.count(implementation), 1)
        self.assertRegex(facade, r"(?m)^mod legacy;$")
        self.assertNotRegex(facade, r"(?m)^pub(?:\([^)]*\))? mod legacy;$")
        self.assertIn("pub use legacy::", facade)
        for factory in ("default_llama_english_plugin", "llama_english_plugin_with_config"):
            self.assertEqual(bridge.count(f"pub fn {factory}("), 1)
            self.assertNotIn(f"pub fn {factory}(", facade)

    def test_local_http_has_one_private_owner_below_the_model_facade(self):
        marker = "#[cfg(test)]\nmod tests {"
        facade = (self.root / "src/model.rs").read_text(encoding="utf-8").split(marker, 1)[0]
        self.assertRegex(facade, r"(?m)^mod local_http;$")
        self.assertNotRegex(facade, r"(?m)^pub(?:\([^)]*\))? mod local_http;$")
        self.assertIn("pub use local_http::is_local_llm_endpoint;", facade)
        self.assertNotRegex(facade, r"(?m)^pub use local_http::(?!is_local_llm_endpoint;$)")
        for implementation in ("struct ParsedHttpEndpoint", "fn http_request(",
                               "fn http_request_with_cancel(", "fn parse_http_endpoint(",
                               "fn loopback_address(", "fn decode_http_response("):
            self.assertNotIn(implementation, facade)
        # The root's existing child test modules may inherit test-only imports.
        # This explicit source guard is not a general Rust cfg/AST evaluator.
        production_imports = re.sub(r"(?m)^#\[cfg\(test\)\]\nuse [^;]+;\n", "", facade)
        self.assertNotIn("std::net::", production_imports)
        self.assertNotIn("std::io::", production_imports)
        local = (self.root / "src/model/local_http.rs").read_text(encoding="utf-8")
        self.assertIn("TcpStream::connect_timeout", local)
        self.assertIn("use crate::prediction::", local)
        self.assertNotIn("crate::languages", local)
        self.assertNotIn("reqwest", local)
        self.assertNotIn("ModelProviderConfig", local)
        for helper in ("http_request", "http_request_with_cancel", "parse_http_endpoint",
                       "decode_http_response"):
            self.assertIn(f"pub(super) fn {helper}(", local)
        for path in ("src/model/runtime.rs", "src/model/transport.rs"):
            with self.subTest(path=path):
                production = (self.root / path).read_text(encoding="utf-8").split(marker, 1)[0]
                self.assertIn("local_http::", production)

    def test_panel_settings_sync_has_one_private_owner_and_snapshot_registration(self):
        panel = (self.root / "src/bin/panel.rs").read_text(encoding="utf-8")
        controller = (self.root / "src/bin/panel/controller.rs").read_text(encoding="utf-8")
        path = "src/bin/panel/settings_sync.rs"
        leaf = (self.root / path).read_text(encoding="utf-8")
        declaration = '#[path = "panel/settings_sync.rs"]\nmod settings_sync;'
        self.assertEqual(panel.count(declaration), 1)
        self.assertNotRegex(panel, r"(?m)^pub(?:\([^)]*\))? (?:mod|use) "
                                  r"(?:(?:crate|self)::)?settings_sync\b")
        # This is an exact source-layout guard, not a Rust cfg/AST evaluator.
        production = leaf.split("#[cfg(test)]\nmod tests {", 1)[0]
        production = re.sub(r"(?m)^#\[cfg\(test\)\]\nuse [^;]+;\n", "", production)
        for method in ("persist_display_settings", "adopt_settings_from"):
            signature = f"pub(super) fn {method}("
            self.assertEqual(production.count(signature), 1)
            self.assertEqual(len(re.findall(rf"\bfn {method}\s*\(", production)), 1)
            self.assertNotIn(f"fn {method}(", controller)
            self.assertNotIn(f"fn {method}(", panel)
        for dependency in ("PersistedDisplaySettings", "apply_display_settings", "save_display_settings"):
            self.assertNotIn(dependency, controller)
            self.assertIn(dependency, leaf)
        self.assertIn("normalize_pointer_stability_settings", controller)
        # Keep this a settings application boundary, not another input host or GPU owner.
        self.assertNotRegex(production, r"(?m)^[ \t]*pub(?:\([^)]*\))? "
                                        r"(?:struct|enum|mod|use|const|static|type|trait) ")
        public_functions = re.findall(
            r"(?m)^[ \t]*(pub(?:\([^)]*\))?) fn ([A-Za-z_][A-Za-z_0-9]*)\s*(?:<|\()", production)
        self.assertEqual(sorted(public_functions), [
            ("pub(super)", "adopt_settings_from"),
            ("pub(super)", "persist_display_settings"),
        ])
        for unrelated in ("wgpu::", "ImeHostSessionBridge", "NativeOperation::", "CommitOptions"):
            self.assertNotIn(unrelated, production)
        state = (self.root / "src/bin/panel/app_state.rs").read_text(encoding="utf-8")
        self.assertNotRegex(state, r"\bPanelState\b")
        for identifier in ("settings-ui", "settings"):
            implementation = next(row for row in self.data["implementations"]
                                  if row["id"] == identifier)
            with self.subTest(implementation=identifier):
                self.assertIn(path, implementation["paths"])
                self.assertIn(path, implementation["tests"])


class PanelSettingsBoundaryFixtures(unittest.TestCase):
    def test_panel_settings_boundary_text_fixtures(self):
        """Probe the same exact-layout guard with temporary text, not compiled Rust."""
        panel = "src/bin/panel.rs"
        controller = "src/bin/panel/controller.rs"
        leaf = "src/bin/panel/settings_sync.rs"
        state = "src/bin/panel/app_state.rs"
        declaration = '#[path = "panel/settings_sync.rs"]\nmod settings_sync;\n'
        methods = ("persist_display_settings", "adopt_settings_from")
        dependencies = ("PersistedDisplaySettings", "apply_display_settings", "save_display_settings")
        unrelated = ("wgpu::Device", "ImeHostSessionBridge", "NativeOperation::Select", "CommitOptions")
        files = {
            panel: declaration,
            controller: "use crate::app_state::normalize_pointer_stability_settings;\n",
            leaf: "use crate::app_state::{" + ", ".join(dependencies) + "};\nimpl PanelState {\n"
                  + "".join(f"    pub(super) fn {method}(&mut self) {{}}\n" for method in methods)
                  + "}\n",
            state: "struct PersistedDisplaySettings;\n",
        }
        data = {"implementations": [
            {"id": identifier, "paths": [leaf], "tests": [leaf]}
            for identifier in ("settings-ui", "settings")
        ]}
        with tempfile.TemporaryDirectory(prefix="suzaku-settings-boundary-") as temporary:
            root = Path(temporary)

            def check(contents, inventory=data):
                for name, text in contents.items():
                    target = root / name
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_text(text, encoding="utf-8")
                # A separate TestCase propagates its assertions to assertRaises,
                # including inventory checks inside the guard's own subTest.
                probe = RepositoryArchitectureTests(
                    "test_panel_settings_sync_has_one_private_owner_and_snapshot_registration")
                probe.root, probe.data = root, inventory
                probe.test_panel_settings_sync_has_one_private_owner_and_snapshot_registration()

            for name, suffix in (
                    ("private owner", ""),
                    ("test-only imports", "".join(
                        f"#[cfg(test)]\nuse fixture::{dependency};\n" for dependency in unrelated)),
                    ("test-only module", "#[cfg(test)]\nmod tests {\n"
                     + "\n".join(unrelated) + "\npub fn fixture() {}\n}\n")):
                with self.subTest(accepted=name):
                    positive = dict(files)
                    positive[leaf] += suffix
                    check(positive)

            mutations = [
                ("missing private module", panel, ""),
                ("duplicate module", panel, declaration * 2),
                ("public module", panel, declaration.replace("mod ", "pub mod ")),
                ("crate-visible module", panel, declaration.replace("mod ", "pub(crate) mod ")),
                ("public leaf type", leaf, files[leaf] + "pub struct Escaped;\n"),
                ("public extra helper", leaf, files[leaf] + "pub fn escaped2<T>() {}\n"),
                ("app-state reverse dependency", state, files[state] + "use super::PanelState;\n"),
                ("pointer policy removed", controller, ""),
            ]
            for export in ("pub use settings_sync::*;", "pub(crate) use settings_sync::*;",
                           "pub use crate::settings_sync::*;", "pub use self::settings_sync::*;"):
                mutations.append((export, panel, declaration + export + "\n"))
            for method in methods:
                signature = f"pub(super) fn {method}("
                mutations.extend((
                    (f"duplicate {method}", leaf, files[leaf] + f"fn {method}() {{}}\n"),
                    (f"public {method}", leaf, files[leaf].replace(signature, f"pub fn {method}(")),
                    (f"controller owns {method}", controller,
                     files[controller] + f"fn {method}() {{}}\n"),
                    (f"panel owns {method}", panel, declaration + f"fn {method}() {{}}\n"),
                ))
            for dependency in dependencies:
                mutations.append((f"controller retains {dependency}", controller,
                                  files[controller] + f"use fixture::{dependency};\n"))
            for dependency in unrelated:
                mutations.append((f"production retains {dependency}", leaf,
                                  files[leaf] + f"use fixture::{dependency};\n"))
            for name, target, text in mutations:
                with self.subTest(rejected=name), self.assertRaises(AssertionError):
                    negative = dict(files)
                    negative[target] = text
                    check(negative)
            for index, implementation in enumerate(data["implementations"]):
                for key in ("paths", "tests"):
                    with self.subTest(missing=(implementation["id"], key)), self.assertRaises(AssertionError):
                        missing = copy.deepcopy(data)
                        missing["implementations"][index][key] = []
                        check(files, missing)


if __name__ == "__main__":
    unittest.main()
