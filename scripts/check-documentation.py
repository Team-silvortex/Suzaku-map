#!/usr/bin/env python3
"""Read-only navigation, bounded-entry and reversible-history checks (stdlib)."""
import argparse
import hashlib
import json
from pathlib import Path
import posixpath
import re
import unicodedata
from urllib.parse import quote, unquote, urlsplit, urlunsplit


ARCHIVE = "docs/history/2026-10/pre-reorganization"
SOURCE_COMMIT = "e3ce76e1620fd52ece28d2fe88dedd2bee0e1938"
# Immutable source bytes, independent of the editable archive manifest.
SOURCES = {
    "README.md": ("project-readme.md", "542a4ac6267c7b12463c5c77c03d336519949b7a0e169929cbcd67493a64c3ff"),
    "DEVELOPMENT.md": ("development.md", "625eae98b838772202de04fe829b56d2c31c6a884c46154600776517340147a6"),
    "data/lexicons/README.md": ("lexicons.md", "bc348ea9db049c07f5a24be6ef1b26926fddd7526aeceb408d7e55172df4112e"),
    "docs/functional-network.md": ("functional-network.md", "d2d4702eb583e97866e41f059cb912df4395f10b849e2c02996633e3de5e9fc5"),
    "docs/functional-network.mmd": ("functional-network.mmd", "574f42231591fa6720370cdbfa5357f4e6a27e2591d5e9b29e50cc124ed3eb66"),
    "docs/functional-tensor.json": ("functional-tensor.json", "2587e382220c0e633f4c61bd29baaf8b7150e5e489d5a3235da6175fde09085a"),
}
MAPPING = {source: f"{ARCHIVE}/{value[0]}" for source, value in SOURCES.items()}
ENTRY_LIMITS = {
    "README.md": 120, "DEVELOPMENT.md": 120, "data/lexicons/README.md": 120,
    "docs/README.md": 120, "docs/users/README.md": 120,
    "docs/users/getting-started.md": 200, "docs/development/README.md": 120,
    "docs/development/architecture.md": 200, "docs/development/testing.md": 200,
    "docs/development/lexicons.md": 200, "docs/development/documentation.md": 120,
    "docs/history/README.md": 120, "docs/releases/README.md": 120,
    "docs/functional-network.md": 350,
}
MAX_BYTES = 16 * 1024 * 1024


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read_file(root, relative):
    path = root / relative
    require(not path.is_symlink() and path.is_file(), f"Missing regular file: {relative}")
    require(path.resolve().is_relative_to(root.resolve()), f"Path escapes root: {relative}")
    require(path.stat().st_size <= MAX_BYTES, f"Oversized document: {relative}")
    return path.read_bytes()


def markdown_targets(raw):
    """Repository Markdown inline/reference destinations, excluding fenced examples.

    Byte offsets are used by the archive ledger. This is deliberately not a
    Markdown renderer: targets containing spaces use the standard angle syntax.
    """
    offset, fence = 0, None
    for line in raw.splitlines(keepends=True):
        marker = re.match(rb"^\s*(`{3,}|~{3,})", line)
        if marker:
            if fence is None:
                fence = marker[1]
            elif marker[1][:1] == fence[:1] and len(marker[1]) >= len(fence):
                fence = None
        elif fence is None:
            for match in re.finditer(rb"\]\((<[^>\n]+>|[^\s)]+)", line):
                yield offset + match.start(1), offset + match.end(1), match[1]
            match = re.match(rb"^ {0,3}\[[^\]\n]+\]:\s*(<[^>\n]+>|[^\s]+)", line)
            if match:
                yield offset + match.start(1), offset + match.end(1), match[1]
        offset += len(line)


def relocate(raw, source, destination):
    """Reproduce the historical migration, permitting link-target changes only."""
    result, ledger, cursor = bytearray(), [], 0
    for start, end, target in sorted(markdown_targets(raw)):
        require(start >= cursor, "Overlapping Markdown destinations")
        angled = target.startswith(b"<") and target.endswith(b">")
        uri = urlsplit((target[1:-1] if angled else target).decode("utf-8"))
        replacement = target
        if not uri.scheme and not uri.netloc and uri.path and not uri.path.startswith("/"):
            path = posixpath.normpath(posixpath.join(posixpath.dirname(source), unquote(uri.path)))
            if path != ".." and not path.startswith("../"):
                path = posixpath.relpath(MAPPING.get(path, path), posixpath.dirname(destination))
                replacement = urlunsplit(("", "", quote(path, safe="/.-_~"), uri.query, uri.fragment)).encode()
                if angled:
                    replacement = b"<" + replacement + b">"
        result.extend(raw[cursor:start])
        if replacement != target:
            ledger.append({"at": len(result), "old": target.decode(), "new": replacement.decode()})
        result.extend(replacement)
        cursor = end
    result.extend(raw[cursor:])
    return bytes(result), ledger


def verify_record(row, archived):
    require(len(archived) == row["archive_bytes"], "Archive byte count differs")
    require(hashlib.sha256(archived).hexdigest() == row["archive_sha256"], "Archive hash differs")
    ledger = row["link_rewrites"]
    require(isinstance(ledger, list) and len(ledger) <= 5000, "Invalid archive ledger")
    end = 0
    for item in ledger:
        require(set(item) == {"at", "old", "new"}, "Invalid ledger fields")
        require(type(item["at"]) is int and isinstance(item["old"], str)
                and isinstance(item["new"], str), "Invalid ledger types")
        start, new = item["at"], item["new"].encode()
        require(start >= end and new and archived[start:start + len(new)] == new,
                "Invalid or overlapping archive offset")
        end = start + len(new)
    restored = archived
    for item in reversed(ledger):
        start, new = item["at"], item["new"].encode()
        restored = restored[:start] + item["old"].encode() + restored[start + len(new):]
    require(len(restored) == row["source_bytes"], "Original byte count differs")
    require(hashlib.sha256(restored).hexdigest() == row["source_sha256"], "Original hash differs")
    if row["original_path"].endswith(".md"):
        expected, changes = relocate(restored, row["original_path"], row["archive_path"])
    else:
        expected, changes = restored, []
    require(expected == archived and changes == ledger, "Changes extend beyond relocated link targets")
    return restored


def check_archive(root):
    manifest = json.loads(read_file(root, f"{ARCHIVE}/manifest.json"))
    require(manifest["format"] == 1 and manifest["source_commit"] == SOURCE_COMMIT
            and manifest["date"] == "2026-10-28", "Archive provenance differs")
    records = manifest["documents"]
    require(isinstance(records, list) and len(records) == len(SOURCES), "Archive document count differs")
    seen, count = set(), 0
    for row in records:
        source = row["original_path"]
        require(source in SOURCES and source not in seen, "Unexpected or duplicate archive source")
        seen.add(source)
        require(row["archive_path"] == MAPPING[source]
                and row["source_sha256"] == SOURCES[source][1], "Frozen archive source differs")
        verify_record(row, read_file(root, row["archive_path"]))
        count += len(row["link_rewrites"])
    return len(records), count


def headings(raw):
    """GFM-style Unicode heading IDs, duplicate suffixes and explicit HTML IDs."""
    result, used, fence = set(), set(), None
    for line in raw.decode("utf-8").splitlines():
        marker = re.match(r"^\s*(`{3,}|~{3,})", line)
        if marker:
            if fence is None:
                fence = marker[1]
            elif marker[1][0] == fence[0] and len(marker[1]) >= len(fence):
                fence = None
            continue
        if fence:
            continue
        result.update(re.findall(r'<[^>]+\b(?:id|name)=["\']([^"\']+)["\']', line))
        match = re.match(r"^ {0,3}#{1,6}\s+(.+?)\s*#*\s*$", line)
        if not match:
            continue
        text = re.sub(r"<[^>]*>", "", match[1]).lower()
        text = re.sub(r"!?\[([^\]]+)\]\([^)]*\)", r"\1", text)
        slug = "".join(char for char in text if char in "-_ " or unicodedata.category(char)[0] in "LN")
        slug = slug.replace(" ", "-")
        candidate, number = slug, 0
        while candidate in used:
            number += 1
            candidate = f"{slug}-{number}"
        used.add(candidate)
        result.add(candidate)
    return result


def check_links(root, relative, raw=None):
    raw = read_file(root, relative) if raw is None else raw
    count, cache = 0, {}
    for start, _, target in markdown_targets(raw):
        text = target.decode().strip("<>")
        uri = urlsplit(text)
        if uri.scheme or uri.netloc:
            continue
        line = raw[:start].count(b"\n") + 1
        label = f"{relative}:{line} -> {text}"
        path = (root / relative).parent / unquote(uri.path) if uri.path else root / relative
        require(not uri.path.startswith("/") and path.resolve().is_relative_to(root.resolve()),
                f"Non-relative link: {label}")
        require(path.exists(), f"Missing link: {label}")
        if path.is_dir():
            index = path / "README.md"
            # Source-directory links are valid GitHub navigation without an
            # index. A fragment, however, must resolve to an actual document.
            require(not uri.fragment or index.is_file(), f"Directory fragment has no README: {label}")
            if index.is_file():
                path = index
        if uri.fragment and path.suffix.lower() == ".md":
            if path not in cache:
                cache[path] = headings(read_file(root, path.relative_to(root)))
            require(unquote(uri.fragment) in cache[path], f"Missing heading: {label}")
        count += 1
    return count


def check_release_index(root):
    raw = read_file(root, "docs/releases/README.md")
    linked = {urlsplit(target.decode().strip("<>")).path for _, _, target in markdown_targets(raw)}
    missing = {path.name for path in (root / "docs/releases").glob("*.md")
               if path.name != "README.md" and path.name not in linked}
    require(not missing, f"Release notes missing from index: {sorted(missing)}")


def check_moved_backlinks(root):
    """Check incoming anchors from older guides, not only the short new entries.

    Historical snapshots retain their original claims/defects; only live pages
    are scanned here. Other unrelated legacy links are outside this migration.
    """
    moved, count = set(MAPPING) | set(MAPPING.values()), 0
    pages = set(root.glob("*.md")) | set((root / "docs").rglob("*.md"))
    pages.update(root / path for path in ("data/lexicons/README.md", "data/offline-packs/README.md",
                                         "sdk/offline-packs/README.md", "src/00-Map.md"))
    for page in sorted(pages):
        relative = page.relative_to(root).as_posix()
        if relative.startswith(f"{ARCHIVE}/"):
            continue
        for _, _, target in markdown_targets(read_file(root, relative)):
            uri = urlsplit(target.decode().strip("<>"))
            path = posixpath.normpath(posixpath.join(posixpath.dirname(relative), unquote(uri.path)))
            if not uri.scheme and not uri.netloc and path in moved:
                count += check_links(root, relative, b"[moved target](" + target + b")")
    return count


def check(root):
    links = 0
    for relative, limit in ENTRY_LIMITS.items():
        raw = read_file(root, relative)
        require(len(raw.splitlines()) <= limit, f"Entry exceeds {limit} lines: {relative}")
        require(not re.search(rb"<!--\s*[^>\n]*:start\s*-->", raw), f"Batch log in current entry: {relative}")
        links += check_links(root, relative, raw)
    check_release_index(root)
    backlinks = check_moved_backlinks(root)
    documents, rewrites = check_archive(root)
    return (f"PASS: {len(ENTRY_LIMITS)} entries, {links} local links, {backlinks} moved-page backlinks; "
            f"{documents} exact historical sources, {rewrites} reversible link relocations")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    args = parser.parse_args(argv)
    try:
        print(check(args.root.resolve()))
    except (ValueError, OSError, KeyError, TypeError) as error:
        parser.exit(1, f"Documentation check failed: {error}\n")


if __name__ == "__main__":
    main()
