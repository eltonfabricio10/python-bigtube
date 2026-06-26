#!/usr/bin/env python3
"""Rust-aware gettext catalog maintenance for BigTube.

The old auto_translate.py targeted the (now removed) Python sources and machine-
translated strings. The project translates by hand, so this tool instead keeps
the catalogs honest against the Rust code:

  * report (default): list catalog entries whose msgid no longer appears in the
    Rust sources (orphans) — i.e. strings that were renamed/removed.
  * --prune: delete those orphan entries from every po/*.po and the .pot.
  * --check: exit non-zero if any orphan exists (handy for CI).

Detection is deliberately conservative: an entry is an orphan only if its msgid
text appears NOWHERE in the Rust sources (plain substring search — no fragile
quote parsing). Anything that shows up as a literal, a label in a const array,
or a format template is kept.
"""

import argparse
import glob
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC_GLOB = os.path.join(ROOT, "rust", "crates", "*", "src", "**", "*.rs")
PO_GLOB = os.path.join(ROOT, "po", "*.po")
POT = os.path.join(ROOT, "po", "bigtube.pot")

_QUOTED = re.compile(r'"((?:[^"\\]|\\.)*)"')


def rust_source() -> str:
    """All Rust source concatenated into one string."""
    return "".join(
        open(f, encoding="utf-8").read()
        for f in glob.glob(SRC_GLOB, recursive=True)
    )


def _unescape(s: str) -> str:
    return s.replace('\\"', '"').replace("\\\\", "\\")


def msgid_of(block: str):
    """Return the (unescaped) msgid of a po entry block, or None."""
    m = re.search(r'(?m)^msgid\s+(.+(?:\n"(?:[^"\\]|\\.)*")*)', block)
    if not m:
        return None
    parts = _QUOTED.findall(m.group(1))
    return _unescape("".join(parts))


def orphans_in(path: str, src: str):
    """msgids present in `path` but absent from the Rust sources."""
    blocks = open(path, encoding="utf-8").read().split("\n\n")
    found = []
    for b in blocks:
        mid = msgid_of(b)
        if mid and mid not in src:  # skip header (msgid "")
            found.append(mid)
    return found


def prune(path: str, src: str) -> int:
    blocks = open(path, encoding="utf-8").read().split("\n\n")
    kept, removed = [], 0
    for b in blocks:
        mid = msgid_of(b)
        if mid and mid not in src:
            removed += 1
            continue
        kept.append(b)
    if removed:
        with open(path, "w", encoding="utf-8") as f:
            f.write("\n\n".join(kept))
    return removed


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--prune", action="store_true", help="remove orphan entries")
    ap.add_argument("--check", action="store_true", help="exit 1 if orphans exist")
    args = ap.parse_args()

    src = rust_source()
    files = sorted(glob.glob(PO_GLOB) + [POT])

    # The reference set of orphans (computed from the template/first catalog).
    ref = orphans_in(files[0], src)
    if args.check:
        if ref:
            print(f"{len(ref)} orphan msgid(s) found (run --prune):")
            for o in ref:
                print("  -", o)
            return 1
        print("i18n catalogs clean — no orphan strings.")
        return 0

    if args.prune:
        total = 0
        for f in files:
            n = prune(f, src)
            total += n
            print(f"pruned {os.path.basename(f)}: -{n}")
        print(f"done — removed {total} entries across {len(files)} files")
        return 0

    print(f"{len(ref)} orphan msgid(s) (use --prune to remove):")
    for o in ref:
        print("  -", o)
    return 0


if __name__ == "__main__":
    sys.exit(main())
