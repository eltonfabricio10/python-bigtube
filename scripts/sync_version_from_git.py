#!/usr/bin/env python3
"""
Sync project version from git history into pyproject.toml and PKGBUILD.

After tag v2.0.7:
  - 0 commits on tag  -> 2.0.7
  - 1 commit after    -> 2.0.8
  - N commits after   -> patch = tag_patch + N

Run automatically via pre-commit or CI before/after pushes.
"""
from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PYPROJECT = ROOT / "pyproject.toml"
PKGBUILD = ROOT / "PKGBUILD"
PO_DIR = ROOT / "po"
RELEASE_WORKFLOW = ROOT / ".github/workflows/release.yml"


def _git_describe_long() -> str:
    """Returns e.g. v2.0.7-3-g1a2b3c4 or v2.0.7."""
    return subprocess.check_output(
        ["git", "describe", "--tags", "--long", "--always"],
        cwd=ROOT,
        text=True,
        stderr=subprocess.DEVNULL,
    ).strip()


def version_from_git() -> str:
    """
    PEP 440-ish semver: bump patch for each commit after the latest tag.
    """
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
    return f"{major}.{minor}.{patch + distance}"


def _replace_version(path: Path, pattern: str, replacement: str) -> bool:
    text = path.read_text(encoding="utf-8")
    new_text, count = re.subn(pattern, replacement, text, count=1, flags=re.MULTILINE)
    if count != 1:
        raise RuntimeError(f"Could not update version in {path}")
    if new_text == text:
        return False
    path.write_text(new_text, encoding="utf-8")
    return True


def _sync_po_files(version: str) -> bool:
    """Update Project-Id-Version in .po / .pot catalogs."""
    changed = False
    if not PO_DIR.is_dir():
        return False
    for path in PO_DIR.glob("*.po"):
        changed |= _replace_version(
            path,
            r'^"Project-Id-Version:.*\\n"',
            f'"Project-Id-Version: BigTube {version}\\n"',
        )
    pot = PO_DIR / "bigtube.pot"
    if pot.is_file():
        changed |= _replace_version(
            pot,
            r'^"Project-Id-Version:.*\\n"',
            f'"Project-Id-Version: BigTube {version}\\n"',
        )
    return changed


def _sync_release_workflow(version: str) -> bool:
    if not RELEASE_WORKFLOW.is_file():
        return False
    text = RELEASE_WORKFLOW.read_text(encoding="utf-8")
    new_text = text
    new_text, _ = re.subn(
        r'default: "[0-9.]+"',
        f'default: "{version}"',
        new_text,
        count=1,
    )
    if new_text == text:
        return False
    RELEASE_WORKFLOW.write_text(new_text, encoding="utf-8")
    return True


def _sync_user_agents(version: str) -> bool:
    """HTTP User-Agent strings that embed the app version."""
    changed = False
    patterns = [
        (ROOT / "src/bigtube/core/network_checker.py", r'"User-Agent": "BigTube/[0-9.]+"'),
        (ROOT / "src/bigtube/core/image_loader.py", r'"User-Agent": "Mozilla/5.0 \(compatible; BigTube/[0-9.]+\)"'),
    ]
    for path, pattern in patterns:
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        if "network_checker" in str(path):
            repl = f'"User-Agent": "BigTube/{version}"'
        else:
            repl = f'"User-Agent": "Mozilla/5.0 (compatible; BigTube/{version})"'
        new_text, count = re.subn(pattern, repl, text, count=1)
        if count and new_text != text:
            path.write_text(new_text, encoding="utf-8")
            changed = True
    return changed


def sync_version_files() -> tuple[str, bool]:
    version = version_from_git()
    changed = False
    changed |= _replace_version(PYPROJECT, r'^version = ".*"', f'version = "{version}"')
    changed |= _replace_version(PKGBUILD, r"^pkgver=.*", f"pkgver={version}")
    changed |= _sync_po_files(version)
    changed |= _sync_release_workflow(version)
    changed |= _sync_user_agents(version)
    return version, changed


def main() -> int:
    check_only = "--check" in sys.argv
    try:
        version, changed = sync_version_files()
    except (subprocess.CalledProcessError, RuntimeError) as exc:
        print(f"sync_version_from_git: {exc}", file=sys.stderr)
        return 1

    if check_only:
        if changed:
            print(
                f"Version out of sync (expected {version}). "
                "Run: python scripts/sync_version_from_git.py",
                file=sys.stderr,
            )
            return 1
        print(version)
        return 0

    print(version)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
