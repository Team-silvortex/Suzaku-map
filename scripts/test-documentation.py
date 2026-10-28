#!/usr/bin/env python3
"""Synthetic documentation guards and read-only checks of the actual repository."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest


sys.dont_write_bytecode = True
SPEC = importlib.util.spec_from_file_location("documentation", Path(__file__).with_name("check-documentation.py"))
DOCS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DOCS)
ROOT = Path(__file__).resolve().parent.parent


class DocumentationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="suzaku-docs-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.write("README.md", "# 开始使用\n\n[guide](docs/guide.md#word-词库)\n")
        self.write("docs/guide.md", "# `Word` 词库\n\n[home](../README.md#开始使用)\n")
        self.raw = b"# Old guide\n[dev](DEVELOPMENT.md#commands)\n[remote](https://example.com)\n"
        archived, ledger = DOCS.relocate(self.raw, "README.md", DOCS.MAPPING["README.md"])
        self.archived = archived
        self.row = {
            "original_path": "README.md", "archive_path": DOCS.MAPPING["README.md"],
            "source_bytes": len(self.raw), "archive_bytes": len(archived),
            "source_sha256": hashlib.sha256(self.raw).hexdigest(),
            "archive_sha256": hashlib.sha256(archived).hexdigest(), "link_rewrites": ledger,
        }

    def write(self, path, text):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8")

    def test_unicode_heading_and_back_links(self):
        self.assertEqual(DOCS.check_links(self.root, "README.md"), 1)
        self.assertEqual(DOCS.check_links(self.root, "docs/guide.md"), 1)

    def test_inline_reference_angle_and_image_destinations(self):
        self.write("docs/space name.md", "# 入口\n")
        self.write("docs/picture.png", "fixture")
        self.write("README.md", "[x](<docs/space name.md#入口>)\n[x]: docs/guide.md#word-词库\n![img](docs/picture.png)\n")
        self.assertEqual(DOCS.check_links(self.root, "README.md"), 3)

    def test_fenced_examples_are_not_navigation(self):
        self.write("README.md", "```md\n[x](missing.md)\n```\n~~~\n[y](absent.md)\n~~~\n")
        self.assertEqual(DOCS.check_links(self.root, "README.md"), 0)

    def test_external_urls_are_not_fetched(self):
        self.write("README.md", "[https](https://invalid.example/no) [mail](mailto:a@example.com) [web](//invalid.example/no)\n")
        self.assertEqual(DOCS.check_links(self.root, "README.md"), 0)

    def test_missing_file_rejected(self):
        self.write("README.md", "[x](missing.md)\n")
        with self.assertRaisesRegex(ValueError, "Missing link"):
            DOCS.check_links(self.root, "README.md")

    def test_missing_fragment_rejected(self):
        self.write("README.md", "[x](docs/guide.md#absent)\n")
        with self.assertRaisesRegex(ValueError, "Missing heading"):
            DOCS.check_links(self.root, "README.md")

    def test_local_fragment_duplicate_and_explicit_ids(self):
        self.write("README.md", "# One\n# One\n# One-1\n<a id='explicit'></a>\n[x](#one-1) [y](#one-1-1) [z](#explicit)\n")
        self.assertEqual(DOCS.check_links(self.root, "README.md"), 3)

    def test_percent_encoded_path_query_and_unicode_anchor(self):
        self.write("docs/space name.md", "# 中文\n")
        self.write("README.md", "[x](docs/space%20name.md?plain=1#%E4%B8%AD%E6%96%87)\n")
        self.assertEqual(DOCS.check_links(self.root, "README.md"), 1)

    def test_directory_link_and_directory_fragment_resolution(self):
        self.write("README.md", "[x](docs/)\n")
        self.assertEqual(DOCS.check_links(self.root, "README.md"), 1)
        self.write("README.md", "[x](docs/#index)\n")
        with self.assertRaisesRegex(ValueError, "Directory fragment has no README"):
            DOCS.check_links(self.root, "README.md")
        self.write("docs/README.md", "# Index\n")
        self.assertEqual(DOCS.check_links(self.root, "README.md"), 1)

    def test_absolute_parent_escape_and_symlink_escape_rejected(self):
        for target in ("/etc/passwd", "../../outside.md"):
            with self.subTest(target=target):
                self.write("README.md", f"[x]({target})\n")
                with self.assertRaisesRegex(ValueError, "Non-relative link"):
                    DOCS.check_links(self.root, "README.md")
        (self.root / "outside").symlink_to("/etc/passwd")
        self.write("README.md", "[x](outside)\n")
        with self.assertRaisesRegex(ValueError, "Non-relative link"):
            DOCS.check_links(self.root, "README.md")

    def test_fenced_headings_do_not_create_anchors(self):
        self.assertNotIn("fake", DOCS.headings(b"```\n# Fake\n```\n"))

    def test_valid_history_roundtrip(self):
        self.assertEqual(DOCS.verify_record(self.row, self.archived), self.raw)

    def test_changed_archived_bytes_rejected(self):
        with self.assertRaisesRegex(ValueError, "Archive hash"):
            DOCS.verify_record(self.row, self.archived.replace(b"guide", b"Guide", 1))

    def test_coordinated_archive_hash_edit_still_checks_original(self):
        changed = self.archived.replace(b"guide", b"Guide", 1)
        self.row["archive_sha256"] = hashlib.sha256(changed).hexdigest()
        with self.assertRaisesRegex(ValueError, "Original hash"):
            DOCS.verify_record(self.row, changed)

    def test_incorrect_archive_and_original_lengths_rejected(self):
        for key, message in (("archive_bytes", "Archive byte"), ("source_bytes", "Original byte")):
            with self.subTest(key=key):
                row = copy.deepcopy(self.row)
                row[key] += 1
                with self.assertRaisesRegex(ValueError, message):
                    DOCS.verify_record(row, self.archived)

    def test_invalid_overlapping_and_boolean_offsets_rejected(self):
        for offset in (-1, True, len(self.archived)):
            with self.subTest(offset=offset):
                row = copy.deepcopy(self.row)
                row["link_rewrites"][0]["at"] = offset
                with self.assertRaisesRegex(ValueError, "offset|types"):
                    DOCS.verify_record(row, self.archived)
        self.row["link_rewrites"] *= 2
        with self.assertRaisesRegex(ValueError, "overlapping"):
            DOCS.verify_record(self.row, self.archived)

    def test_reversible_non_link_edit_is_not_allowed(self):
        changed = self.archived.replace(b"Old guide", b"New guide")
        row = copy.deepcopy(self.row)
        row["archive_sha256"] = hashlib.sha256(changed).hexdigest()
        row["link_rewrites"].insert(0, {"at": 2, "old": "Old", "new": "New"})
        with self.assertRaisesRegex(ValueError, "beyond relocated link targets"):
            DOCS.verify_record(row, changed)

    def test_relocation_retains_fragments_query_urls_and_code(self):
        raw = b"[x](docs/guide.md?q=1#heading)\n[z]: <docs/guide.md#heading>\n```\n[x](README.md)\n```\n"
        moved, _ = DOCS.relocate(raw, "README.md", DOCS.MAPPING["README.md"])
        self.assertIn(b"../../../guide.md?q=1#heading", moved)
        self.assertIn(b"<../../../guide.md#heading>", moved)
        self.assertIn(b"```\n[x](README.md)\n```", moved)

    def test_non_markdown_archive_never_changes_bytes(self):
        raw = b"flowchart TB\n  a --> b\n"
        row = dict(self.row, original_path="docs/functional-network.mmd", link_rewrites=[],
                   source_bytes=len(raw), archive_bytes=len(raw),
                   source_sha256=hashlib.sha256(raw).hexdigest(), archive_sha256=hashlib.sha256(raw).hexdigest())
        self.assertEqual(DOCS.verify_record(row, raw), raw)

    def test_entry_size_and_batch_log_rejected(self):
        original = DOCS.ENTRY_LIMITS
        try:
            DOCS.ENTRY_LIMITS = {"README.md": 2}
            with self.assertRaisesRegex(ValueError, "exceeds"):
                DOCS.check(self.root)
            self.write("README.md", "<!-- batch:start -->\n")
            with self.assertRaisesRegex(ValueError, "Batch log"):
                DOCS.check(self.root)
        finally:
            DOCS.ENTRY_LIMITS = original

    def test_real_archive_has_six_frozen_originals_and_reversible_ledger(self):
        self.assertEqual(DOCS.check_archive(ROOT), (6, 846))

    def test_release_index_cannot_omit_source_notes(self):
        self.write("docs/releases/README.md", "# Versions\n")
        self.write("docs/releases/1.0.0.md", "# Source version\n")
        with self.assertRaisesRegex(ValueError, "missing from index"):
            DOCS.check_release_index(self.root)
        self.write("docs/releases/README.md", "# Versions\n[x](1.0.0.md)\n")
        DOCS.check_release_index(self.root)

    def test_older_guides_cannot_keep_removed_entry_anchors(self):
        self.write("docs/older.md", "[x](../DEVELOPMENT.md#old-heading)\n")
        self.write("DEVELOPMENT.md", "# New entry\n")
        for path in ("data/lexicons/README.md", "data/offline-packs/README.md",
                     "sdk/offline-packs/README.md", "src/00-Map.md"):
            self.write(path, "# Fixture\n")
        with self.assertRaisesRegex(ValueError, "Missing heading"):
            DOCS.check_moved_backlinks(self.root)
        self.write("docs/older.md", "[x](../DEVELOPMENT.md#new-entry)\n")
        self.assertEqual(DOCS.check_moved_backlinks(self.root), 2)

    def test_real_manifest_cannot_replace_frozen_source_hash(self):
        manifest = json.loads((ROOT / DOCS.ARCHIVE / "manifest.json").read_bytes())
        manifest["documents"][0]["source_sha256"] = "0" * 64
        self.write(f"{DOCS.ARCHIVE}/manifest.json", json.dumps(manifest))
        with self.assertRaisesRegex(ValueError, "Frozen archive source"):
            DOCS.check_archive(self.root)

    def test_actual_entry_navigation_and_archive(self):
        self.assertTrue(DOCS.check(ROOT).startswith("PASS:"))


if __name__ == "__main__":
    unittest.main()
