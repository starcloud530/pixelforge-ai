#!/usr/bin/env python3
"""Re-run validate-atlas --strict on the committed #18 pack.

Exit 0 only when the zombie sheet and the prop sheet both have no errors.
A broken PNG, atlas, or rules file fails this script. PixelForge does not
need a Rust change for that gate; `cargo test -p pixelforge-cli
game_pack_zombie` calls the same check.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[1]


def find_binary() -> str:
    env = os.environ.get("PIXELFORGE")
    if env:
        return env
    for candidate in (
        REPO / "target" / "debug" / "pixelforge",
        REPO / "target" / "release" / "pixelforge",
    ):
        if candidate.exists():
            return str(candidate)
    found = shutil.which("pixelforge")
    if found:
        return found
    raise SystemExit("cargo build -p pixelforge-cli, or set PIXELFORGE")


def validate(binary: str, atlas: Path, rules: Path, pivot: str, four_direction: bool) -> None:
    proc = subprocess.run(
        [
            binary,
            "validate-atlas",
            "--atlas",
            str(atlas),
            "--rules",
            str(rules),
            "--strict",
        ],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    print(proc.stderr.strip())
    if proc.returncode != 0:
        raise SystemExit(proc.stdout or proc.stderr)
    body = json.loads(proc.stdout)
    checks = body["result"]["checks"]
    if (
        not body.get("ok")
        or body["result"]["errorCount"] != 0
        or checks["mode"] != "strict"
        or checks["pivot"] != pivot
        or checks["fourDirection"] is not four_direction
    ):
        raise SystemExit(proc.stdout)


def main() -> None:
    binary = find_binary()
    validate(
        binary,
        ROOT / "zombie" / "zombie-sheet.json",
        ROOT / "zombie" / "zombie-rules.json",
        "checked",
        True,
    )
    validate(
        binary,
        ROOT / "props" / "props-sheet.json",
        ROOT / "props" / "props-rules.json",
        "checked",
        False,
    )


if __name__ == "__main__":
    sys.exit(main())
