#!/usr/bin/env python3
"""
Sync the project version (derived from git history) into the Rust workspace.

Derives the version from git tags (patch bump per commit after the latest tag,
rolling over at 99) and writes it to the Rust targets:

  - rust/Cargo.toml          ([workspace.package] version)
  - rust/packaging/PKGBUILD  (pkgver)

Prints the resolved version to stdout (used by the release workflow).
"""

from __future__ import annotations

import re
import subprocess
import sys
from argparse import ArgumentParser
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CARGO_TOML = ROOT / "rust" / "Cargo.toml"
RUST_PKGBUILD = ROOT / "rust" / "packaging" / "PKGBUILD"


def _git_describe_long() -> str:
    """Returns e.g. v2.0.7-3-g1a2b3c4 or v2.0.7."""
    return subprocess.check_output(
        ["git", "describe", "--tags", "--long", "--always"],
        cwd=ROOT,
        text=True,
        stderr=subprocess.DEVNULL,
    ).strip()


def version_from_git() -> str:
    """PEP 440-ish semver: bump patch for each commit after the latest tag."""
    raw = _git_describe_long()
    if raw.startswith("v"):
        raw = raw[1:]

    # tagged exactly: 2.0.7
    if "-" not in raw:
        return raw

    base, rest = raw.split("-", 1)
    parts = rest.split("-")
    if not parts or not parts[0].isdigit():
        return base

    distance = int(parts[0])
    if distance == 0:
        return base

    segments = base.split(".")
    if len(segments) != 3 or not all(s.isdigit() for s in segments):
        return f"{base}.dev{distance}"

    major, minor, patch = (int(s) for s in segments)
    patch += distance
    minor += patch // 100
    patch %= 100
    major += minor // 100
    minor %= 100
    return f"{major}.{minor}.{patch}"


def _replace(path: Path, pattern: str, replacement: str, *, write: bool = True) -> bool:
    text = path.read_text(encoding="utf-8")
    new_text, count = re.subn(pattern, replacement, text, count=1, flags=re.MULTILINE)
    if count != 1:
        raise RuntimeError(f"Could not update version in {path}")
    if new_text == text:
        return False
    if write:
        path.write_text(new_text, encoding="utf-8")
    return True


def sync_rust_version(*, version: str | None = None, write: bool = True) -> tuple[str, bool]:
    version = version or version_from_git()
    changed = False
    # The first `version = "..."` line in Cargo.toml is [workspace.package].
    changed |= _replace(CARGO_TOML, r'^version = ".*"', f'version = "{version}"', write=write)
    changed |= _replace(RUST_PKGBUILD, r"^pkgver=.*", f"pkgver={version}", write=write)
    return version, changed


def main() -> int:
    parser = ArgumentParser(description="Sync the Rust workspace version metadata.")
    parser.add_argument("--check", action="store_true", help="Check without writing files.")
    parser.add_argument("--version", help="Explicit version instead of deriving it from git.")
    args = parser.parse_args()

    try:
        version, changed = sync_rust_version(version=args.version, write=not args.check)
    except RuntimeError as exc:
        print(f"sync_rust_version: {exc}", file=sys.stderr)
        return 1

    if args.check and changed:
        print(
            f"Rust version out of sync (expected {version}). "
            "Run: python scripts/sync_rust_version.py",
            file=sys.stderr,
        )
        return 1

    print(version)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
