#!/usr/bin/env python3
"""Static and temporary-tree checks for packaged documentation; no binaries run."""
import importlib.util
import os
from pathlib import Path
import shutil
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location("suzaku_package_docs", ROOT / "scripts/package-docs.py")
DOCS = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = DOCS
SPEC.loader.exec_module(DOCS)


class PackageDocumentation(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="suzaku-package-docs-test-")
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.repo = self.base / "source"
        self.tree = self.base / "tar"
        self.repo.mkdir()
        self.tree.mkdir()
        for name in DOCS.ROOT_DOCUMENTS:
            self.write(name, f"# {name}\n\n[Documentation](docs/README.md).\n")
        self.write("data/lexicons/README.md", "# Lexicons\n")
        self.write("data/offline-packs/README.md", "# Packs\n\n[Pack](example.json).\n")
        self.write("sdk/offline-packs/README.md", "# SDK\n\n[Tool](suzaku_pack.py) [Example](example.py).\n")
        for name in DOCS.FLAT_GUIDES:
            self.write(f"docs/{name}", "# Guide\n\n[Details](development/nested.md#details).\n")
        for name, text in {
            "README.md": "# Documentation\n\n[Users](users/README.md).\n",
            "users/README.md": "# Users\n\n[Guide](../ibus-candidates.md).\n",
            "development/nested.md": "# Details\n",
            "history/2026-10/archive.md": "# History\n",
            "history/2026-10/manifest.json": '{"format":1}\n',
            "functional-network.mmd": "flowchart LR\n  A-->B\n",
            "functional-tensor.json": '{"format":1}\n',
            "releases/README.md": "# Source versions\n",
            "releases/0.9.9.md": "# 0.9.9\n",
            "releases/0.10.0.md": "# 0.10.0\n",
        }.items():
            self.write(f"docs/{name}", text)

    def write(self, relative, text):
        path = self.repo / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
        return path

    def staged(self):
        DOCS.stage(self.repo, self.tree)
        return self.tree / "share/doc/suzaku"

    def test_nested_indexes_archives_and_all_release_notes_keep_original_bytes(self):
        docroot = self.staged()
        DOCS.verify(self.repo, self.tree, "tar")
        for source in (self.repo / "docs").rglob("*"):
            if source.is_file():
                self.assertEqual((docroot / source.relative_to(self.repo)).read_bytes(), source.read_bytes())
        self.assertEqual((docroot / "data/lexicons/README.md").read_bytes(), b"# Lexicons\n")

    def test_both_flat_layouts_relocate_nested_guide_links(self):
        docroot = self.staged()
        for guide in DOCS.FLAT_GUIDES:
            self.assertIn("](docs/development/nested.md#details)", (docroot / guide).read_text())
            outer_name = "README.md" if guide == "linux-packaging-data.md" else guide
            self.assertIn("](share/doc/suzaku/docs/development/nested.md#details)",
                          (self.tree / outer_name).read_text())
            self.assertEqual((docroot / "docs/development/nested.md").read_text(), "# Details\n")

    def test_debian_layout_reuses_exact_share_tree_without_outer_flat_guides(self):
        self.staged()
        deb = self.base / "deb/usr"
        shutil.copytree(self.tree / "share", deb / "share")
        DOCS.verify(self.repo, deb, "deb")
        self.assertFalse((deb / "README.md").exists())

    def test_companion_guides_point_to_existing_payload_layout_without_copying_programs(self):
        docroot = self.staged()
        pack_guide = docroot / "data/offline-packs/README.md"
        sdk_guide = docroot / "sdk/offline-packs/README.md"
        self.assertIn("](../../../../suzaku/offline-packs/example.json)", pack_guide.read_text())
        self.assertIn("](../../../../suzaku/offline-pack-sdk/suzaku_pack.py)", sdk_guide.read_text())
        self.assertIn("](../../../../suzaku/offline-pack-sdk/example.py)", sdk_guide.read_text())
        self.assertEqual((pack_guide.parent / "../../../../suzaku/offline-packs").resolve(),
                         self.tree / "share/suzaku/offline-packs")
        self.assertEqual((sdk_guide.parent / "../../../../suzaku/offline-pack-sdk").resolve(),
                         self.tree / "share/suzaku/offline-pack-sdk")
        self.assertFalse((sdk_guide.parent / "suzaku_pack.py").exists())

    def test_relocation_preserves_urls_fragments_titles_and_reference_links(self):
        source = (
            '[Nested](users/README.md#start) [Source](../DEVELOPMENT.md?q=one#build)\n'
            '[Space](<users/quick start.md#start> "A title")\n'
            '[Remote](https://example.test/a.md#x) [Anchor](#here) [Absolute](/tmp/a.md)\n'
            '[user]: users/README.md#start "Title"\n'
            '```md\n[Example](example.md)\n```\n'
        )
        result = DOCS.relocate_markdown(source, "share/doc/suzaku")
        self.assertEqual(result, (
            '[Nested](share/doc/suzaku/docs/users/README.md#start) '
            '[Source](share/doc/suzaku/DEVELOPMENT.md?q=one#build)\n'
            '[Space](<share/doc/suzaku/docs/users/quick start.md#start> "A title")\n'
            '[Remote](https://example.test/a.md#x) [Anchor](#here) [Absolute](/tmp/a.md)\n'
            '[user]: share/doc/suzaku/docs/users/README.md#start "Title"\n'
            '```md\n[Example](example.md)\n```\n'
        ))

    def test_unselected_source_and_personal_data_are_not_copied(self):
        self.write("src/main.rs", "not documentation\n")
        self.write("data/lexicons/resources.json", "not a selected document\n")
        self.write("home/person/preferences.json", "private\n")
        self.staged()
        copied = {path.relative_to(self.tree).as_posix() for path in self.tree.rglob("*") if path.is_file()}
        self.assertFalse(any("preferences" in name or "main.rs" in name or "resources.json" in name
                             for name in copied))

    def test_source_file_symlink_is_rejected_before_any_copy(self):
        (self.repo / "docs/linked.md").symlink_to(self.repo / "README.md")
        with self.assertRaisesRegex(ValueError, "Unexpected documentation input"):
            DOCS.stage(self.repo, self.tree)
        self.assertEqual(list(self.tree.iterdir()), [])

    def test_source_directory_symlink_is_rejected(self):
        (self.repo / "docs/linked").symlink_to(self.repo / "data", target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "Unexpected documentation input"):
            DOCS.stage(self.repo, self.tree)

    def test_selected_root_symlink_is_rejected(self):
        (self.repo / "README.md").unlink()
        (self.repo / "README.md").symlink_to(self.repo / "SECURITY.md")
        with self.assertRaisesRegex(ValueError, "regular file"):
            DOCS.stage(self.repo, self.tree)

    def test_unexpected_extension_and_fifo_are_rejected(self):
        script = self.write("docs/do-not-execute.py", "raise SystemExit(99)\n")
        with self.assertRaisesRegex(ValueError, "Unexpected documentation input"):
            DOCS.stage(self.repo, self.tree)
        script.unlink()
        os.mkfifo(self.repo / "docs/fifo.md")
        with self.assertRaisesRegex(ValueError, "Unexpected documentation input"):
            DOCS.stage(self.repo, self.tree)

    def test_missing_portable_guide_is_rejected_before_copy(self):
        (self.repo / "docs/translation.md").unlink()
        with self.assertRaisesRegex(ValueError, "Missing portable documentation guide"):
            DOCS.stage(self.repo, self.tree)
        self.assertEqual(list(self.tree.iterdir()), [])

    def test_document_size_and_count_limits_are_enforced(self):
        previous = DOCS.MAX_FILE_BYTES, DOCS.MAX_FILES, DOCS.MAX_TOTAL_BYTES
        self.addCleanup(lambda: self.restore_limits(previous))
        DOCS.MAX_FILE_BYTES = 1
        with self.assertRaisesRegex(ValueError, "file-size limit"):
            DOCS.stage(self.repo, self.tree)
        DOCS.MAX_FILE_BYTES = previous[0]
        DOCS.MAX_FILES = 1
        with self.assertRaisesRegex(ValueError, "file-count limit"):
            DOCS.stage(self.repo, self.tree)
        DOCS.MAX_FILES = previous[1]
        DOCS.MAX_TOTAL_BYTES = 1
        with self.assertRaisesRegex(ValueError, "total-size limit"):
            DOCS.stage(self.repo, self.tree)

    @staticmethod
    def restore_limits(previous):
        DOCS.MAX_FILE_BYTES, DOCS.MAX_FILES, DOCS.MAX_TOTAL_BYTES = previous

    def test_preexisting_files_and_destination_symlinks_are_not_overwritten(self):
        external = self.base / "outside"
        external.mkdir()
        (self.tree / "share").symlink_to(external, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "must not be a symlink"):
            DOCS.stage(self.repo, self.tree)
        self.assertEqual(list(external.iterdir()), [])
        (self.tree / "share").unlink()
        self.staged()
        with self.assertRaisesRegex(ValueError, "Refusing to overwrite"):
            DOCS.stage(self.repo, self.tree)

    def test_verification_rejects_missing_changed_extra_and_symlink_files(self):
        docroot = self.staged()
        target = docroot / "docs/users/README.md"
        original = target.read_bytes()
        target.unlink()
        with self.assertRaisesRegex(ValueError, "Missing packaged documentation"):
            DOCS.verify(self.repo, self.tree, "tar")
        target.write_bytes(b"changed\n")
        with self.assertRaisesRegex(ValueError, "differs from source/layout"):
            DOCS.verify(self.repo, self.tree, "tar")
        target.write_bytes(original)
        extra = docroot / "docs/extra.md"
        extra.write_text("unexpected\n")
        with self.assertRaisesRegex(ValueError, "missing or unexpected"):
            DOCS.verify(self.repo, self.tree, "tar")
        extra.unlink()
        target.unlink()
        target.symlink_to(self.repo / "docs/users/README.md")
        with self.assertRaisesRegex(ValueError, "regular file"):
            DOCS.verify(self.repo, self.tree, "tar")

    def test_packaging_and_both_smokes_use_the_single_document_policy(self):
        package = (ROOT / "scripts/package-linux.sh").read_text()
        smoke = (ROOT / "scripts/test-linux-package.sh").read_text()
        self.assertIn('scripts/package-docs.py stage --repo "$suzaku_package_root"', package)
        self.assertNotIn("sed -E 's@\\]", package)
        self.assertIn('package-docs.py" check', smoke)
        self.assertIn('--layout tar', smoke)
        self.assertIn('--layout deb', smoke)
        self.assertIn('test-package-docs.py"', smoke)


if __name__ == "__main__":
    unittest.main()
