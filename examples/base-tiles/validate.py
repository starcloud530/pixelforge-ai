#!/usr/bin/env python3
"""Check the committed base ground + wall pack, including a missing autotile mask.

Exit 0 only when validate-tiles passes and the .tmj matches the 4-neighbor room.
A broken tileset, map, or rules file fails this script.
"""

from __future__ import annotations

import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[1]

# North 1, east 2, south 4, west 8. Wall tile id is mask + 1.
# The room interior is empty, so a top-edge cell does not see a south neighbor.
CORNERS = {
    (2, 2): 7,  # east + south
    (7, 2): 13,  # west + south
    (2, 5): 4,  # north + east
    (7, 5): 10,  # north + west
    (4, 2): 11,  # east + west
    (4, 5): 11,  # east + west
    (2, 3): 6,  # north + south
    (7, 3): 6,  # north + south
}


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


def run(cmd: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, check=False, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)


def png_size(path: Path) -> tuple[int, int]:
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit(f"{path.name} is not a PNG")
    width, height = struct.unpack(">II", data[16:24])
    return width, height


def cell_index(x: int, y: int) -> int:
    return y * 10 + x


def main() -> None:
    pix = binary()
    checked = run(
        [
            pix,
            "validate-tiles",
            "--project",
            str(ROOT / "tileset.json"),
            "--map",
            str(ROOT / "map.json"),
            "--rules",
            str(ROOT / "wall-rules.json"),
        ]
    )
    if checked.returncode != 0:
        raise SystemExit(checked.stdout or checked.stderr)
    body = json.loads(checked.stdout)
    if body["result"]["tileset"]["tileCount"] != 17 or body["result"]["autotile"]["masks"] != 16:
        raise SystemExit(checked.stdout)

    meta = json.loads((ROOT / "tileset.meta.json").read_text(encoding="utf-8"))
    if meta["format"] != "pixelforge-tileset" or meta["tileCount"] != 17 or meta["image"] != "tiles.png":
        raise SystemExit("tileset metadata does not match the sheet")
    if png_size(ROOT / "tiles.png") != (272, 16):
        raise SystemExit(f"tiles.png is {png_size(ROOT / 'tiles.png')}, expected 272x16")
    if png_size(ROOT / "map.png") != (160, 128):
        raise SystemExit(f"map.png is {png_size(ROOT / 'map.png')}, expected 160x128")

    tilemap = json.loads((ROOT / "map.json").read_text(encoding="utf-8"))
    ground = tilemap["layers"][0]["cells"]
    wall = tilemap["layers"][1]["cells"]
    corner = ground[0]
    if corner.get("tile") != 0 or corner.get("flipH") is not True:
        raise SystemExit("ground (0,0) should be tile 0 flipped horizontally")
    for (x, y), tile in CORNERS.items():
        if wall[cell_index(x, y)] != tile:
            raise SystemExit(f"wall {(x, y)} is {wall[cell_index(x, y)]}, expected {tile}")
    if wall[cell_index(4, 3)] is not None:
        raise SystemExit("room interior should stay empty")

    tmj = json.loads((ROOT / "base.tmj").read_text(encoding="utf-8"))
    if tmj["type"] != "map" or tmj["orientation"] != "orthogonal":
        raise SystemExit("base.tmj is not an orthogonal Tiled map")
    if tmj["width"] != 10 or tmj["height"] != 8 or tmj["tilewidth"] != 16 or tmj["tileheight"] != 16:
        raise SystemExit("base.tmj grid does not match the 10x8 map of 16px tiles")
    tileset = tmj["tilesets"][0]
    if tileset["firstgid"] != 1 or tileset["image"] != "tiles.png" or tileset["tilecount"] != 17:
        raise SystemExit("base.tmj tileset is not the exported sheet")
    if tileset["columns"] != 17 or tileset["imagewidth"] != 272 or tileset["imageheight"] != 16:
        raise SystemExit("base.tmj tileset image is not one row of 17 tiles")
    if len(tmj["layers"]) != 2:
        raise SystemExit("base.tmj should have ground and wall")
    for layer in tmj["layers"]:
        if layer["type"] != "tilelayer" or len(layer["data"]) != 80:
            raise SystemExit("base.tmj layer data is not 10x8")
    ground_gid = tmj["layers"][0]["data"][0]
    if ground_gid != (1 | 0x80000000):
        raise SystemExit(f"ground GID is {ground_gid}, expected horizontal flip")
    for (x, y), tile in CORNERS.items():
        gid = tmj["layers"][1]["data"][cell_index(x, y)]
        if gid != tile + 1:
            raise SystemExit(f"Tiled GID at {(x, y)} is {gid}, expected {tile + 1}")
    if tmj["layers"][1]["data"][cell_index(4, 3)] != 0:
        raise SystemExit("Tiled interior cell should be empty")

    rules = json.loads((ROOT / "wall-rules.json").read_text(encoding="utf-8"))
    rules["rules"] = [rule for rule in rules["rules"] if rule["mask"] != 9]
    with tempfile.TemporaryDirectory() as tmp:
        broken = Path(tmp) / "missing.json"
        broken.write_text(json.dumps(rules), encoding="utf-8")
        refused = run([pix, "validate-tiles", "--rules", str(broken)])
    if refused.returncode != 1:
        raise SystemExit(refused.stdout or refused.stderr or "missing mask should exit 1")
    error = json.loads(refused.stdout)
    if error["error"]["code"] != "AutotileRule" or "9" not in error["error"]["message"]:
        raise SystemExit(refused.stdout)


if __name__ == "__main__":
    try:
        main()
    except SystemExit as err:
        print(err, file=sys.stderr)
        raise
