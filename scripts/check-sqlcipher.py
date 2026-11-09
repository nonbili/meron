#!/usr/bin/env python3
"""Close cargo-audit's SQLCipher gap for RUSTSEC-2026-0459 (Python 3.11+)."""

import argparse
import re
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"
PATCH_REV = "0f1ab7393ae5d62d44a94a5a5107c00b23d4ed58"
PATCH_SOURCE = f"git+https://github.com/rusqlite/rusqlite.git?rev={PATCH_REV}#{PATCH_REV}"


def check(lock_path: Path) -> dict:
    with lock_path.open("rb") as lock_file:
        packages = tomllib.load(lock_file)["package"]
    sources = [p for p in packages if p["name"] == "sqlcipher-src"]
    if not sources:
        raise ValueError("sqlcipher-src missing; review how SQLCipher is bundled")
    for package in sources:
        version = package["version"]
        release = re.fullmatch(r"(\d+)\.(\d+)\.(\d+)(?:\+[^\s]+)?", version)
        if not release or tuple(map(int, release.groups())) < (420, 0, 0):
            raise ValueError(
                f"sqlcipher-src {version}: require >=420.0.0 (SQLCipher 4.20 / "
                "SQLite 3.54.1) for RUSTSEC-2026-0459; see "
                "https://github.com/rusqlite/rusqlite/pull/1932"
            )
    bindings = [p for p in packages if p["name"] == "libsqlite3-sys"]
    if len(bindings) != 1:
        raise ValueError("expected exactly one libsqlite3-sys; review audit coverage")
    binding = bindings[0]
    if binding.get("source") != REGISTRY and (
        binding.get("source") != PATCH_SOURCE or binding["version"] != "0.39.1"
    ):
        raise ValueError("unreviewed libsqlite3-sys source/version; review audit coverage")
    return binding


def audit_binding(lock_path: Path, binding: dict) -> None:
    if binding["source"] == REGISTRY:
        return  # Covered by the ordinary cargo audit step.
    # cargo-audit skips Git sources. The reviewed patch changes the SQLCipher
    # source pin and generated bindings, so also audit it as released 0.39.1.
    # Keep the real lockfile untouched; sqlcipher-src is audited normally.
    lock_text = lock_path.read_text().replace(
        f'source = "{PATCH_SOURCE}"', f'source = "{REGISTRY}"'
    )
    with tempfile.TemporaryDirectory(prefix="meron-sqlcipher-audit-") as directory:
        audit_lock = Path(directory) / "Cargo.lock"
        audit_lock.write_text(lock_text)
        subprocess.run(
            ["cargo", "audit", "--file", str(audit_lock)],
            cwd=lock_path.resolve().parent,
            check=True,
        )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "lock_path", nargs="?", type=Path,
        default=Path(__file__).resolve().parents[1] / "meron-core/Cargo.lock",
    )
    parser.add_argument("--audit-patched-binding", action="store_true")
    args = parser.parse_args()
    try:
        binding = check(args.lock_path)
        if args.audit_patched_binding:
            audit_binding(args.lock_path, binding)
    except (OSError, ValueError, KeyError) as error:
        sys.exit(f"SQLCipher audit failed: {error}")
    except subprocess.CalledProcessError as error:
        sys.exit(error.returncode)
    print("SQLCipher source version check passed")
