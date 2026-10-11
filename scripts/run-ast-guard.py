#!/usr/bin/env python3
"""Run only frozen source guards with their historical AST fingerprint format.

Python 3.12 includes empty AST list fields in ast.dump; Python 3.13+ omits
them. These five guards froze the latter representation independently of
their later fixture additions. Keep their source, hashes and assertions intact.
This entry point never selects the native suite or a general Python script.
"""
import argparse
import ast
from pathlib import Path
import runpy
import sys


SCRIPTS_DIR = Path(__file__).resolve().parent
GUARDS = (
    "test-native-model-fixture.py",
    "test-language-workflow-coverage.py",
    "test-keypad-navigation-coverage.py",
    "test-held-language-coverage.py",
    "test-vocabulary-partitions.py",
)


def stable_ast_dump(node, annotate_fields=True, include_attributes=False, *, indent=None):
    """Fixed annotated, attribute-free format; never change the supplied tree."""
    if annotate_fields is not True or include_attributes is not False or indent is not None:
        raise ValueError("Only annotated, attribute-free, unindented AST fingerprints are supported")
    if not isinstance(node, ast.AST):
        raise TypeError("expected AST, got %r" % node.__class__.__name__)

    def format_value(value):
        if isinstance(value, ast.AST):
            fields = []
            for name, item in ast.iter_fields(value):
                # Constant.value is literal data, not an AST list field. Even
                # unusual manually constructed [] literals must stay visible.
                literal_value = isinstance(value, ast.Constant) and name == "value"
                if isinstance(item, list) and not item and not literal_value:
                    continue
                # Optional AST fields have class defaults of None. A required
                # Constant(value=None), unlike its optional kind, is retained.
                if item is None and getattr(type(value), name, ...) is None:
                    continue
                fields.append("%s=%s" % (name, format_value(item)))
            return "%s(%s)" % (type(value).__name__, ", ".join(fields))
        if isinstance(value, list):
            return "[" + ", ".join(format_value(item) for item in value) + "]"
        return repr(value)

    return format_value(node)


def run_guard(name):
    if not isinstance(name, str) or name not in GUARDS:
        raise ValueError("Only the five fixed source-guard basenames are allowed")
    target = (SCRIPTS_DIR / name).resolve()
    if target.parent != SCRIPTS_DIR or target.name != name or not target.is_file():
        raise ValueError("Source guard must be a file in the fixed scripts directory")
    previous_dump, previous_argv = ast.dump, sys.argv
    try:
        ast.dump = stable_ast_dump
        sys.argv = [str(target)]
        return runpy.run_path(str(target), run_name="__main__")
    finally:
        ast.dump = previous_dump
        sys.argv = previous_argv


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("guard", choices=GUARDS)
    options = parser.parse_args(argv)
    run_guard(options.guard)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
