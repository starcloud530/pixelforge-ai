#!/usr/bin/env python3
"""Re-run validate-atlas --strict, direction validate, and project validate.

Exit 0 only when the zombie sheet and the prop sheet both have no atlas
errors, the zombie project passes direction validate (4-direction
idle/walk), and pixelforge.manifest.json passes project validate.
A broken PNG, atlas, rules file, direction layout, or manifest fails
this script. `cargo test -p pixelforge-cli game_pack_zombie` calls the
atlas check; `game_pack_zombie_directions` calls the direction gate;
`game_pack_zombie_manifest` calls the manifest gate.
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


def validate_directions(binary: str, project: Path) -> None:
    proc = subprocess.run(
        [binary, "direction", "validate", "--project", str(project)],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    print(proc.stderr.strip())
    if proc.returncode != 0:
        raise SystemExit(proc.stdout or proc.stderr)
    body = json.loads(proc.stdout)
    result = body["result"]
    if (
        not body.get("ok")
        or result["errorCount"] != 0
        or result["count"] != 4
        or result["columns"] != 4
        or len(result["facings"]) != 4
    ):
        raise SystemExit(proc.stdout)


def validate_manifest(binary: str, manifest: Path) -> None:
    proc = subprocess.run(
        [binary, "project", "validate", "--manifest", str(manifest)],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    print(proc.stderr.strip())
    if proc.returncode != 0:
        raise SystemExit(proc.stdout or proc.stderr)
    body = json.loads(proc.stdout)
    result = body["result"]
    checks = result["checks"]
    if (
        not body.get("ok")
        or result["errorCount"] != 0
        or result["warningCount"] != 0
        or checks["perspective"] != "top"
        or checks["canvases"] != [{"name": "sprite", "width": 64, "height": 64}]
        or [row["projects"] for row in checks["layout"]] != [1, 1]
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
    validate_directions(binary, ROOT / "zombie" / "zombie.json")
    validate_manifest(binary, ROOT / "pixelforge.manifest.json")


if __name__ == "__main__":
    sys.exit(main())
