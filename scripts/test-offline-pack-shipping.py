#!/usr/bin/env python3
"""Check source/catalog/shipping lists without building or installing packages.

Artifact content and clean-container installation retain their separate gates.
This check catches stale explicit lists when a data-only collection is added.
"""
from collections import Counter
import json
from pathlib import Path
import re
import shlex
import unittest

ROOT = Path(__file__).resolve().parent.parent


def require_exact(actual, expected):
    if Counter(actual) != Counter(expected):
        raise ValueError(f"pack list differs: actual={actual!r}, expected={expected!r}")


def smoke_names(source):
    match = re.search(r"suzaku_package_test_pack_names=\((.*?)\)", source, re.S)
    if match is None:
        raise ValueError("missing explicit smoke-test pack list")
    return [name + ".json" for name in shlex.split(match[1])]


def navigation_rows(source):
    """Read the deliberately small, portable README table, not all Markdown."""
    start, end = "<!-- pack-catalog:start -->", "<!-- pack-catalog:end -->"
    if source.count(start) != 1 or source.count(end) != 1:
        raise ValueError("missing or duplicated pack navigation boundary")
    if source.index(start) >= source.index(end):
        raise ValueError("reversed pack navigation boundaries")
    lines = source.split(start, 1)[1].split(end, 1)[0].strip().splitlines()
    if len(lines) < 3 or lines[:2] != [
        "| 语言 | 包文件 | ID | 版本 | 主题标签 | 范围 |",
        "| --- | --- | --- | --- | --- | --- |",
    ]:
        raise ValueError("missing pack navigation header or entries")

    def code(value):
        match = re.fullmatch(r"`([^`]+)`", value)
        if match is None:
            raise ValueError("invalid inline metadata field")
        return match[1]

    rows = []
    for line in lines[2:]:
        cells = line.split("|")
        if len(cells) != 8 or cells[0].strip() or cells[-1].strip():
            raise ValueError("pack navigation must have exactly six columns")
        language, link, identifier, version, topics, description = (c.strip() for c in cells[1:-1])
        match = re.fullmatch(r"\[([A-Za-z0-9-]+\.json)\]\(\1\)", link)
        if match is None or not description:
            raise ValueError("pack navigation must use a same-directory JSON link and description")
        rows.append((match[1], code(language), code(identifier), code(version),
                     tuple(code(value) for value in topics.split(", "))))
    return rows


class ShippingLists(unittest.TestCase):
    def test_catalog_and_both_packaging_lists_cover_exactly_the_sources(self):
        expected = sorted(path.name for path in (ROOT / "data/offline-packs").glob("*.json"))
        self.assertTrue(expected)
        catalog = (ROOT / "src/lexicon/packs.rs").read_text()
        names = re.findall(r'include_str!\("../../data/offline-packs/([^"/]+\.json)"\)', catalog)
        require_exact(names, expected)
        shipping = (ROOT / "scripts/package-linux.sh").read_text()
        names = re.findall(r"data/offline-packs/([A-Za-z0-9-]+\.json)", shipping)
        require_exact(names, expected)
        smoke = (ROOT / "scripts/test-linux-package.sh").read_text()
        require_exact(smoke_names(smoke), expected)

    def test_missing_collection_is_rejected(self):
        with self.assertRaises(ValueError):
            require_exact(["en-family.json"], ["en-family.json", "ja-family.json"])

    def test_duplicate_collection_is_not_hidden_by_set_comparison(self):
        with self.assertRaises(ValueError):
            require_exact(["en-family.json", "en-family.json"], ["en-family.json"])

    def test_unknown_collection_is_rejected(self):
        with self.assertRaises(ValueError):
            require_exact(["en-family.json", "unknown.json"], ["en-family.json"])

    def test_missing_smoke_list_is_rejected(self):
        with self.assertRaises(ValueError):
            smoke_names("# no pack list")

    def test_smoke_list_keeps_order_and_duplicate_entries(self):
        source = "suzaku_package_test_pack_names=(en-family\n ja-family en-family)"
        self.assertEqual(smoke_names(source), ["en-family.json", "ja-family.json", "en-family.json"])

    def test_navigation_matches_each_pack_manifest_without_guessing_languages_or_topics(self):
        directory = ROOT / "data/offline-packs"
        expected = []
        for path in sorted(directory.glob("*.json")):
            manifest = json.loads(path.read_text())["manifest"]
            expected.append((path.name, manifest["language"], manifest["id"],
                             manifest["version"], tuple(manifest["topics"])))
        require_exact(navigation_rows((directory / "README.md").read_text()), expected)
        self.assertEqual(len({row[2] for row in expected}), len(expected), "duplicate pack ID")

    def test_navigation_rejects_missing_duplicate_or_changed_metadata(self):
        source = (ROOT / "data/offline-packs/README.md").read_text()
        rows = navigation_rows(source)
        self.assertGreater(len(rows), 1)
        for actual in [rows[:-1], rows + [rows[0]],
                       [(*rows[0][:3], "99.0.0", rows[0][4]), *rows[1:]],
                       [(rows[0][0], "ja", *rows[0][2:]), *rows[1:]],
                       [(*rows[0][:4], ("unknown-topic",)), *rows[1:]]]:
            with self.subTest(rows=actual):
                with self.assertRaises(ValueError):
                    require_exact(actual, rows)

    def test_navigation_rejects_nonportable_links_or_missing_boundaries(self):
        source = (ROOT / "data/offline-packs/README.md").read_text()
        for invalid in [source.replace("<!-- pack-catalog:start -->", ""),
                        source.replace("<!-- pack-catalog:end -->", ""),
                        source + "\n<!-- pack-catalog:end -->",
                        source.replace("](en-outdoors.json)", "](../en-outdoors.json)"),
                        source.replace("| 英文观星与观测用语 |", "| |")]:
            with self.subTest(source=invalid):
                with self.assertRaises(ValueError):
                    navigation_rows(invalid)

    def test_portable_guides_are_explicitly_shipped_and_smoke_checked_for_both_formats(self):
        shipping = (ROOT / "scripts/package-linux.sh").read_text()
        smoke = (ROOT / "scripts/test-linux-package.sh").read_text()
        for source, directory in [("data/offline-packs/README.md", "offline-packs"),
                                  ("sdk/offline-packs/README.md", "offline-pack-sdk")]:
            with self.subTest(source=source):
                self.assertTrue((ROOT / source).is_file())
                self.assertEqual(shipping.count(source), 1)
                for prefix in ["$suzaku_package_test_tree", "$suzaku_package_test_extract/root/usr"]:
                    self.assertIn(f'test -s "{prefix}/share/suzaku/{directory}/README.md"', smoke)

    def test_portable_readme_relative_links_are_existing_sibling_files(self):
        for relative in ["data/offline-packs/README.md", "sdk/offline-packs/README.md"]:
            document = ROOT / relative
            targets = re.findall(r"\]\(([^\s)]+)\)", document.read_text())
            self.assertTrue(targets, relative)
            for target in targets:
                with self.subTest(document=relative, target=target):
                    self.assertEqual(Path(target).name, target, "guide links must survive package relocation")
                    self.assertTrue((document.parent / target).is_file())


if __name__ == "__main__":
    unittest.main()
