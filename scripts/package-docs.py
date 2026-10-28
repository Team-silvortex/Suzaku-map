#!/usr/bin/env python3
"""Stage and verify the static documentation layout of future Linux packages.

The docs tree has one policy: copy regular .md/.mmd/.json files, preserving paths
and bytes. Symlinks, special files and other extensions are errors, not silently
omitted inputs. Code and user-data directories are never traversed.
"""
import argparse
from dataclasses import dataclass
import os
from pathlib import Path, PurePosixPath
import posixpath
import re
import stat
from urllib.parse import urlsplit, urlunsplit


ROOT_DOCUMENTS = ("README.md", "SECURITY.md", "CONTRIBUTING.md", "DEVELOPMENT.md")
FLAT_GUIDES = (
    "linux-packaging-data.md", "model-providers.md", "ibus-candidates.md",
    "translation.md", "interface-languages.md",
)
STATIC_SUFFIXES = frozenset((".md", ".mmd", ".json"))
# These portable guides link only to sibling authored payloads. Their original
# runtime copies stay unchanged; doc-root mirrors point to the existing payloads.
COMPANION_GUIDES = (
    ("data/offline-packs/README.md", "../../../../suzaku/offline-packs"),
    ("sdk/offline-packs/README.md", "../../../../suzaku/offline-pack-sdk"),
)
MAX_FILES = 4096
MAX_FILE_BYTES = 16 * 1024 * 1024
MAX_TOTAL_BYTES = 128 * 1024 * 1024


@dataclass(frozen=True)
class Document:
    source: Path
    relative: PurePosixPath


def regular_file(path):
    info = path.lstat()
    if not stat.S_ISREG(info.st_mode):
        raise ValueError(f"Documentation must be a regular file: {path}")
    if info.st_size > MAX_FILE_BYTES:
        raise ValueError(f"Documentation exceeds file-size limit: {path}")
    return info.st_size


def directory(path):
    if not stat.S_ISDIR(path.lstat().st_mode):
        raise ValueError(f"Documentation directory must not be a symlink or special file: {path}")


def source_documents(repo):
    """Validate the entire selection before writing any destination files."""
    repo = Path(repo)
    directory(repo)
    docs = repo / "docs"
    directory(docs)
    selected = []

    def visit(current):
        for entry in sorted(current.iterdir()):
            mode = entry.lstat().st_mode
            if stat.S_ISDIR(mode):
                visit(entry)
            elif stat.S_ISREG(mode) and entry.suffix in STATIC_SUFFIXES:
                selected.append(Document(entry, PurePosixPath(entry.relative_to(repo).as_posix())))
            else:
                raise ValueError(f"Unexpected documentation input: {entry}")

    visit(docs)
    for relative in ROOT_DOCUMENTS:
        selected.append(Document(repo / relative, PurePosixPath(relative)))
    # Keep the source lexicon guide reachable from source-relative documentation
    # links. Its JSON resources remain compiled into the binaries, not copied.
    for parent in (repo / "data", repo / "data/lexicons"):
        directory(parent)
    selected.append(Document(repo / "data/lexicons/README.md", PurePosixPath("data/lexicons/README.md")))
    for relative, _ in COMPANION_GUIDES:
        path = repo / relative
        for parent in reversed(path.parent.relative_to(repo).parents):
            directory(repo / parent)
        directory(path.parent)
        selected.append(Document(path, PurePosixPath(relative)))
    if len(selected) > MAX_FILES:
        raise ValueError("Documentation exceeds file-count limit")
    if sum(regular_file(item.source) for item in selected) > MAX_TOTAL_BYTES:
        raise ValueError("Documentation exceeds total-size limit")
    sources = {str(item.relative) for item in selected}
    for guide in FLAT_GUIDES:
        if f"docs/{guide}" not in sources:
            raise ValueError(f"Missing portable documentation guide: {guide}")
    return selected


def relocate_target(target, prefix, source_base="docs"):
    """Move a guide target from docs/ to a flat copy without changing URLs."""
    uri = urlsplit(target)
    if uri.scheme or uri.netloc or not uri.path or uri.path.startswith("/"):
        return target
    # Work on URI path text so percent escapes, query text and anchors survive.
    path = posixpath.normpath(posixpath.join(source_base, uri.path))
    relocated = posixpath.normpath(posixpath.join(prefix, path))
    return urlunsplit(("", "", relocated, uri.query, uri.fragment))


def relocate_markdown(text, prefix, source_base="docs"):
    """Relocate inline/reference link targets, leaving fenced examples alone.

    This matches the repository's portable Markdown links, including angle
    destinations and optional titles; it is not a general Markdown renderer.
    """
    inline = re.compile(r"(\]\(\s*)(<[^>\n]+>|[^\s)]+)(?=[\s)])")
    reference = re.compile(r"^(\s{0,3}\[[^\]\n]+\]:\s*)(<[^>\n]+>|[^\s]+)")

    def replace(match):
        target = match[2]
        if target.startswith("<"):
            target = f"<{relocate_target(target[1:-1], prefix, source_base)}>"
        else:
            target = relocate_target(target, prefix, source_base)
        return match[1] + target

    result = []
    fence = None
    for line in text.splitlines(keepends=True):
        marker = re.match(r"^\s*(`{3,}|~{3,})", line)
        if marker:
            value = marker[1]
            if fence is None:
                fence = value
            elif value[0] == fence[0] and len(value) >= len(fence):
                fence = None
        elif fence is None:
            line = reference.sub(replace, inline.sub(replace, line))
        result.append(line)
    return "".join(result)


def package_documents(repo, layout):
    documents = source_documents(repo)
    expected = {}
    base = PurePosixPath("share/doc/suzaku")
    for item in documents:
        expected[str(base / item.relative)] = item.source.read_bytes()
    for relative, prefix in COMPANION_GUIDES:
        text = (Path(repo) / relative).read_text(encoding="utf-8")
        expected[str(base / relative)] = relocate_markdown(text, prefix, "").encode("utf-8")
    for guide in FLAT_GUIDES:
        text = (Path(repo) / "docs" / guide).read_text(encoding="utf-8")
        expected[str(base / guide)] = relocate_markdown(text, "").encode("utf-8")
        if layout == "tar":
            root_name = "README.md" if guide == "linux-packaging-data.md" else guide
            expected[root_name] = relocate_markdown(text, str(base)).encode("utf-8")
    return expected


def safe_destination(tree, relative, *, must_be_new):
    tree = Path(tree)
    directory(tree)
    current = tree
    parts = PurePosixPath(relative).parts
    for part in parts[:-1]:
        current = current / part
        if os.path.lexists(current):
            directory(current)
    target = tree.joinpath(*parts)
    if os.path.lexists(target):
        if must_be_new:
            raise ValueError(f"Refusing to overwrite packaged documentation: {target}")
        regular_file(target)
    elif not must_be_new:
        raise ValueError(f"Missing packaged documentation: {target}")
    return target


def stage(repo, tree):
    expected = package_documents(repo, "tar")
    destinations = [(safe_destination(tree, name, must_be_new=True), data)
                    for name, data in expected.items()]
    for destination, data in destinations:
        destination.parent.mkdir(parents=True, exist_ok=True)
        # Exclusive creation prevents silently replacing a concurrently added file.
        with destination.open("xb") as stream:
            stream.write(data)
        destination.chmod(0o644)
    return len(destinations)


def verify(repo, tree, layout):
    expected = package_documents(repo, layout)
    for name, data in expected.items():
        destination = safe_destination(tree, name, must_be_new=False)
        if destination.read_bytes() != data:
            raise ValueError(f"Packaged documentation differs from source/layout: {destination}")
    # The recursive docs tree is exact; dependencies/licenses and root metadata
    # have separate existing package checks, so do not broaden this traversal.
    docs = Path(tree) / "share/doc/suzaku/docs"
    actual = set()
    for current, directories, files in os.walk(docs, followlinks=False):
        for name in directories:
            directory(Path(current) / name)
        for name in files:
            path = Path(current) / name
            regular_file(path)
            actual.add(path.relative_to(tree).as_posix())
    intended = {name for name in expected if name.startswith("share/doc/suzaku/docs/")}
    if actual != intended:
        raise ValueError("Packaged docs tree contains missing or unexpected files")
    return len(expected)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=("stage", "check"))
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("--tree", required=True, type=Path)
    parser.add_argument("--layout", choices=("tar", "deb"), default="tar")
    args = parser.parse_args()
    if args.operation == "stage":
        if args.layout != "tar":
            parser.error("stage creates the tar tree; the Debian package reuses its share/ directory")
        count = stage(args.repo, args.tree)
    else:
        count = verify(args.repo, args.tree, args.layout)
    print(f"PASS: {count} static documentation files {args.operation} ({args.layout} layout).")


if __name__ == "__main__":
    main()
