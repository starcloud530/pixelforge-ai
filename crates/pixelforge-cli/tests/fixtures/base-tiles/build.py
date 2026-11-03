#!/usr/bin/env python3
"""Build the base ground + wall tileset, autotile a room, and export Tiled .tmj.

Run from anywhere. Outputs land next to this script. Existing outputs are replaced.
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


def binary() -> str:
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


def run(cmd: list[str]) -> None:
    proc = subprocess.run(cmd, check=False, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if proc.returncode != 0:
        raise SystemExit(proc.stderr.strip() or proc.stdout)


def call(pix: str, project: Path, requests: list[dict]) -> None:
    args = [pix, "call", "--project", str(project)]
    for request in requests:
        args.extend(["--request", json.dumps(request, separators=(",", ":"))])
    run(args)


def fill(x: int, y: int, width: int, height: int, color: int) -> dict:
    return {
        "tool": "fillRect",
        "params": {"x": x, "y": y, "width": width, "height": height, "colorIndex": color},
    }


def wall_requests(mask: int) -> list[dict]:
    body = [fill(0, 0, 16, 16, 4)]
    if mask & 1 == 0:
        body.append(fill(0, 0, 16, 3, 1))
    if mask & 4 == 0:
        body.append(fill(0, 13, 16, 3, 1))
    if mask & 8 == 0:
        body.append(fill(0, 0, 3, 16, 1))
    if mask & 2 == 0:
        body.append(fill(13, 0, 3, 16, 1))
    return [{"tool": "setActiveFrame", "params": {"index": mask + 1}}, *body]


def main() -> None:
    pix = binary()
    os.chdir(ROOT)
    for name in (
        "tileset.json",
        "wall-rules.json",
        "map.json",
        "tiles.png",
        "tileset.meta.json",
        "map.png",
        "base.tmj",
    ):
        path = ROOT / name
        if path.exists():
            path.unlink()

    project = ROOT / "tileset.json"
    run([pix, "new", "--width", "16", "--height", "16", "--output", str(project)])
    run(
        [
            pix,
            "tileset",
            "set",
            "--project",
            str(project),
            "--tile-width",
            "16",
            "--tile-height",
            "16",
        ]
    )
    call(
        pix,
        project,
        [
            fill(0, 0, 16, 16, 6),
            fill(2, 3, 3, 2, 5),
            fill(10, 11, 3, 2, 4),
            *[{"tool": "addFrame"} for _ in range(16)],
        ],
    )
    for mask in range(16):
        call(pix, project, wall_requests(mask))

    rules = {
        "format": "pixelforge-autotile",
        "version": 1,
        "neighbors": 4,
        "bits": ["n", "e", "s", "w"],
        "rules": [{"mask": mask, "tile": mask + 1} for mask in range(16)],
    }
    (ROOT / "wall-rules.json").write_text(json.dumps(rules, indent=2) + "\n", encoding="utf-8")

    run(
        [
            pix,
            "tilemap",
            "new",
            "--width",
            "10",
            "--height",
            "8",
            "--tile-width",
            "16",
            "--tile-height",
            "16",
            "--tileset",
            "tileset.json",
            "--output",
            "map.json",
            "--layer",
            "ground",
            "--layer",
            "wall",
        ]
    )
    run(
        [
            pix,
            "tilemap",
            "fill",
            "--map",
            "map.json",
            "--name",
            "ground",
            "--x",
            "0",
            "--y",
            "0",
            "--width",
            "10",
            "--height",
            "8",
            "--tile",
            "0",
        ]
    )
    run(
        [
            pix,
            "tilemap",
            "set",
            "--map",
            "map.json",
            "--name",
            "ground",
            "--x",
            "0",
            "--y",
            "0",
            "--tile",
            "0",
            "--flip-h",
        ]
    )
    run(
        [
            pix,
            "tilemap",
            "fill",
            "--map",
            "map.json",
            "--name",
            "wall",
            "--x",
            "2",
            "--y",
            "2",
            "--width",
            "6",
            "--height",
            "4",
            "--tile",
            "1",
        ]
    )
    run(
        [
            pix,
            "tilemap",
            "clear",
            "--map",
            "map.json",
            "--name",
            "wall",
            "--x",
            "3",
            "--y",
            "3",
            "--width",
            "4",
            "--height",
            "2",
        ]
    )
    run(
        [
            pix,
            "autotile",
            "apply",
            "--map",
            "map.json",
            "--rules",
            "wall-rules.json",
            "--name",
            "wall",
        ]
    )
    run(
        [
            pix,
            "export",
            "tileset",
            "--project",
            "tileset.json",
            "--output",
            "tiles.png",
            "--meta",
            "tileset.meta.json",
            "--image",
            "tiles.png",
        ]
    )
    run(
        [
            pix,
            "export",
            "tilemap",
            "--map",
            "map.json",
            "--tileset",
            "tileset.json",
            "--output",
            "map.png",
            "--tmj",
            "base.tmj",
            "--image",
            "tiles.png",
        ]
    )
    run(
        [
            pix,
            "validate-tiles",
            "--project",
            "tileset.json",
            "--map",
            "map.json",
            "--rules",
            "wall-rules.json",
        ]
    )


if __name__ == "__main__":
    try:
        main()
    except SystemExit as err:
        print(err, file=sys.stderr)
        raise
