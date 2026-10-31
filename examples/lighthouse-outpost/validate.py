#!/usr/bin/env python3
"""Check the lighthouse trial assets and that build.py still reproduces them.

Also checks the mini-game source points at the exported base.tmj, tiles.png,
and lighthouse PNG, and that the Tiled flip word on ground (0, 0) is the
horizontal bit. Exit 0 only when those files agree. A drifted sprite, a
missing beam, or a flattened project fails this script.
"""

from __future__ import annotations

import filecmp
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from pngrgba import decode

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[1]
BASE = ROOT.parent / "base-tiles"

# Same room as examples/base-tiles/validate.py. Wall tile id is mask + 1.
CORNERS = {
    (2, 2): 7,
    (7, 2): 13,
    (2, 5): 4,
    (7, 5): 10,
    (4, 2): 11,
    (4, 5): 11,
    (2, 3): 6,
    (7, 3): 6,
}

FLIP_H = 0x80000000
GID_MASK = 0x1FFFFFFF
STABLE = (
    "lighthouse.json",
    "lighthouse.png",
    "lighthouse-before.png",
    "lighthouse.preview.png",
    "lighthouse-before.preview.png",
)


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


def split_panels(rgba: bytes, width: int, height: int) -> tuple[bytes, bytes]:
    """Left and right halves of a side-by-side RGBA buffer."""
    if width % 2 != 0:
        raise SystemExit("compare PNG width is not two panels")
    half = width // 2
    left = bytearray()
    right = bytearray()
    row = width * 4
    cell = half * 4
    for y in range(height):
        start = y * row
        left.extend(rgba[start : start + cell])
        right.extend(rgba[start + cell : start + row])
    return bytes(left), bytes(right)


def partial_alpha(rgba: bytes) -> int:
    count = 0
    for index in range(0, len(rgba), 4):
        alpha = rgba[index + 3]
        if 0 < alpha < 255:
            count += 1
    return count


def cell(data: list, width: int, x: int, y: int):
    return data[y * width + x]


def main() -> None:
    pix = binary()
    scene = subprocess.run(
        [pix, "scene", "--project", str(ROOT / "lighthouse.json")],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if scene.returncode != 0:
        raise SystemExit(scene.stdout or scene.stderr)
    body = json.loads(scene.stdout)["result"]
    if body["width"] != 32 or body["height"] != 64:
        raise SystemExit("lighthouse canvas is not 32x64")
    names = [layer["name"] for layer in body["layers"]]
    if names != ["masonry", "lantern", "beam"]:
        raise SystemExit(f"layers are {names}")
    beam = body["layers"][2]
    if beam["opacity"] != 128 or beam["visible"] is not True:
        raise SystemExit("beam layer is not visible at opacity 128")
    if body["paletteCount"] != 11:
        raise SystemExit("palette should be the studio eight plus stone and shadow")

    before_w, before_h, before = decode(ROOT / "lighthouse-before.png")
    flat_w, flat_h, flat = decode(ROOT / "lighthouse.png")
    preview_w, preview_h, _preview = decode(ROOT / "lighthouse.preview.png")
    if (before_w, before_h, flat_w, flat_h) != (32, 64, 32, 64):
        raise SystemExit("sprite PNGs are not 32x64")
    if (preview_w, preview_h) != (128, 256):
        raise SystemExit("lighthouse.preview.png is not the scale-4 export")
    if partial_alpha(before) < 16:
        raise SystemExit("layered export lost the translucent beam")
    if partial_alpha(flat) != 0:
        raise SystemExit("flattened export still has partial alpha")

    metrics = json.loads((ROOT / "metrics.json").read_text(encoding="utf-8"))
    flatten = metrics["flatten"]
    if flatten["partialAlphaBefore"] != partial_alpha(before) or flatten["partialAlphaAfter"] != 0:
        raise SystemExit("metrics.json does not match the exported alpha")
    if flatten["opacity127ClearsPartialSky"] != flatten["partialAlphaBefore"]:
        raise SystemExit("opacity 127 should clear every partial-alpha sky cell")
    if flatten["indexChanges"] != 28 or flatten["indexChangeHistogram"] != {"5->9": 24, "5->4": 4}:
        raise SystemExit("flatten index histogram drifted")
    if metrics["drawingRequests"] != 25 or metrics["canvas"] != {"width": 32, "height": 64}:
        raise SystemExit("drawing request count drifted")

    play = (ROOT / "play.js").read_text(encoding="utf-8")
    html = (ROOT / "play.html").read_text(encoding="utf-8")
    for needle in (
        "../base-tiles/base.tmj",
        "lighthouse.png",
        "lighthouse-before.png",
        "0x80000000",
        "0x40000000",
        "0x20000000",
        "decodeGid",
    ):
        if needle not in play:
            raise SystemExit(f"play.js is missing {needle}")
    if "map.png" in play or "map.png" in html:
        raise SystemExit("the scene must load the tileset and .tmj, not map.png")
    if "play.js" not in html:
        raise SystemExit("play.html does not load play.js")

    tmj = json.loads((BASE / "base.tmj").read_text(encoding="utf-8"))
    if tmj["orientation"] != "orthogonal" or tmj["width"] != 10 or tmj["height"] != 8:
        raise SystemExit("base.tmj is not the 10x8 orthogonal room")
    if tmj["tilewidth"] != 16 or tmj["tileheight"] != 16:
        raise SystemExit("base.tmj is not 16px tiles")
    tileset = tmj["tilesets"][0]
    if tileset["image"] != "tiles.png" or tileset["firstgid"] != 1 or tileset["columns"] != 17:
        raise SystemExit("base.tmj tileset image is not tiles.png")
    sheet_w, sheet_h, _sheet = decode(BASE / "tiles.png")
    if (sheet_w, sheet_h) != (272, 16):
        raise SystemExit("tiles.png is not 272x16")
    ground = tmj["layers"][0]["data"]
    wall = tmj["layers"][1]["data"]
    corner = ground[0]
    if (corner & FLIP_H) == 0 or (corner & GID_MASK) != 1:
        raise SystemExit("ground (0,0) GID is not tile 0 flipped horizontally")
    for (x, y), tile in CORNERS.items():
        gid = cell(wall, 10, x, y)
        if gid != tile + 1:
            raise SystemExit(f"wall {(x, y)} GID is {gid}, expected {tile + 1}")
    if cell(wall, 10, 4, 3) != 0:
        raise SystemExit("room interior should stay empty so the keeper can stand there")

    project_bytes = (ROOT / "lighthouse.json").read_bytes()
    with tempfile.TemporaryDirectory() as compare_dir:
        compare_png = Path(compare_dir) / "lighthouse.compare.png"
        compared = subprocess.run(
            [
                pix,
                "preview",
                "--project",
                str(ROOT / "lighthouse.json"),
                "--compare",
                "--scale",
                "1",
                "--output",
                str(compare_png),
            ],
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if compared.returncode != 0:
            raise SystemExit(compared.stderr or compared.stdout)
        if "project unchanged" not in compared.stderr:
            raise SystemExit("preview --compare did not say the project is unchanged")
        if (ROOT / "lighthouse.json").read_bytes() != project_bytes:
            raise SystemExit("preview --compare rewrote lighthouse.json")
        report = json.loads(compared.stdout)["result"]
        if report.get("compare") is not True or report.get("panels") != ["rgba", "indexed"]:
            raise SystemExit("preview --compare result is not a rgba|indexed pair")
        if report["pngWidth"] != 64 or report["pngHeight"] != 64:
            raise SystemExit("compare PNG is not 64x64")
        if report["indexChanges"] != flatten["indexChanges"]:
            raise SystemExit("compare indexChanges does not match metrics.json")
        if report["pixels"][14 * 32 + 20] != 5 or report["flattenPixels"][14 * 32 + 20] != 9:
            raise SystemExit("beam over stone at (20, 14) is not cover 5 / flatten 9")
        if report["pixels"][26 * 32 + 18] != 5 or report["flattenPixels"][26 * 32 + 18] != 4:
            raise SystemExit("beam over the red band at (18, 26) is not cover 5 / flatten 4")
        compare_w, compare_h, compare_rgba = decode(compare_png)
        if (compare_w, compare_h) != (64, 64):
            raise SystemExit("decoded compare PNG is not 64x64")
        left, right = split_panels(compare_rgba, compare_w, compare_h)
        if left != before or right != flat:
            raise SystemExit("compare panels are not lighthouse-before.png | lighthouse.png")

    with tempfile.TemporaryDirectory() as tmp:
        rebuilt = Path(tmp)
        proc = subprocess.run(
            [sys.executable, str(ROOT / "build.py"), "--out", str(rebuilt)],
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if proc.returncode != 0:
            raise SystemExit(proc.stderr or proc.stdout)
        for name in STABLE:
            if not filecmp.cmp(ROOT / name, rebuilt / name, shallow=False):
                raise SystemExit(f"{name} does not match build.py")
        fresh = json.loads((rebuilt / "metrics.json").read_text(encoding="utf-8"))
        fresh.pop("cliSeconds")
        committed = dict(metrics)
        committed.pop("cliSeconds")
        if fresh != committed:
            raise SystemExit("metrics.json drifted from build.py aside from cliSeconds")


if __name__ == "__main__":
    try:
        main()
    except SystemExit as err:
        print(err, file=sys.stderr)
        raise
