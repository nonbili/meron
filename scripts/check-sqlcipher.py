#!/usr/bin/env python3
"""Close cargo-audit's SQLCipher gap for RUSTSEC-2026-0459 (Python 3.11+)."""

import argparse
import re
import sys
import tomllib
from pathlib import Path

REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"


def check(lock_path: Path) -> None:
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
    if bindings[0].get("source") != REGISTRY:
        # cargo-audit skips non-registry sources; review audit coverage first.
        raise ValueError("libsqlite3-sys is not from crates.io; review audit coverage")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "lock_path", nargs="?", type=Path,
        default=Path(__file__).resolve().parents[1] / "meron-core/Cargo.lock",
    )
    args = parser.parse_args()
    try:
        check(args.lock_path)
    except (OSError, ValueError, KeyError) as error:
        sys.exit(f"SQLCipher audit failed: {error}")
    print("SQLCipher source version check passed")
