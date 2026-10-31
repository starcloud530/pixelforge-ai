#!/usr/bin/env python3
"""Draw a 32×64 lighthouse with the current CLI and export it.

The layered project stays editable. The game sprite is the flattened export:
layer opacity 128 on the lamp beam is snapped to the nearest palette index
(#112). A second export keeps the unflattened RGBA composite so the beam's
partial alpha can be compared.

Run from anywhere:

    cargo build -p pixelforge-cli
    python3 examples/lighthouse-outpost/build.py

`--out DIR` writes the same files somewhere else. The default is this directory.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

from pngrgba import decode

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[1]

WIDTH = 32
HEIGHT = 64
BEAM_OPACITY = 128

INK = 1
CREAM = 2
RED = 3
WOOD = 4
GOLD = 5
GREEN = 6
GLASS = 7
STONE = 9
SHADOW = 10

MASONRY = 0
LANTERN = 1
BEAM = 2


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


class Cli:
    def __init__(self, pix: str) -> None:
        self.pix = pix
        self.seconds = 0.0
        self.commands = 0

    def run(self, args: list[str], check: bool = True) -> subprocess.CompletedProcess[str]:
        started = time.perf_counter()
        proc = subprocess.run(
            [self.pix, *args],
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.seconds += time.perf_counter() - started
        self.commands += 1
        if check and proc.returncode != 0:
            raise SystemExit(
                (proc.stderr or proc.stdout or f"pixelforge exited {proc.returncode}").strip()
            )
        return proc

    def call(self, project: Path, requests: list[dict]) -> list[dict]:
        args = ["call", "--project", str(project)]
        for request in requests:
            args.extend(["--request", json.dumps(request, separators=(",", ":"))])
        proc = self.run(args)
        rows = []
        for line in proc.stdout.splitlines():
            if line.strip():
                rows.append(json.loads(line))
        return rows

    def scene(self, project: Path) -> dict:
        proc = self.run(["scene", "--project", str(project)])
        return json.loads(proc.stdout)["result"]

    def preview_pixels(self, project: Path, layer: int | None, dest: Path) -> list[int]:
        args = ["preview", "--project", str(project), "--scale", "1", "--output", str(dest)]
        if layer is not None:
            args.extend(["--layer", str(layer)])
        proc = self.run(args)
        body = json.loads(proc.stdout)
        pixels = body["result"]["pixels"]
        if len(pixels) != WIDTH * HEIGHT:
            raise SystemExit(f"preview pixels are {len(pixels)}, expected {WIDTH * HEIGHT}")
        return [int(pixel) for pixel in pixels]


def fill_rect(layer: int, x: int, y: int, width: int, height: int, color: int) -> dict:
    return {
        "tool": "fillRect",
        "params": {
            "layer": layer,
            "x": x,
            "y": y,
            "width": width,
            "height": height,
            "colorIndex": color,
        },
    }


def stroke_rect(layer: int, x: int, y: int, width: int, height: int, color: int) -> dict:
    return {
        "tool": "strokeRect",
        "params": {
            "layer": layer,
            "x": x,
            "y": y,
            "width": width,
            "height": height,
            "colorIndex": color,
        },
    }


def line(
    layer: int,
    x0: int,
    y0: int,
    x1: int,
    y1: int,
    color: int,
    pixel_perfect: bool = False,
) -> dict:
    params: dict = {
        "layer": layer,
        "x0": x0,
        "y0": y0,
        "x1": x1,
        "y1": y1,
        "colorIndex": color,
    }
    if pixel_perfect:
        params["pixelPerfect"] = True
    return {"tool": "line", "params": params}


def ellipse(layer: int, x: int, y: int, width: int, height: int, color: int, filled: bool) -> dict:
    params: dict = {
        "layer": layer,
        "x": x,
        "y": y,
        "width": width,
        "height": height,
        "colorIndex": color,
    }
    if filled:
        params["fill"] = True
    return {"tool": "ellipse", "params": params}


def polygon(layer: int, points: list[tuple[int, int]], color: int, filled: bool) -> dict:
    params: dict = {
        "layer": layer,
        "points": [{"x": x, "y": y} for x, y in points],
        "colorIndex": color,
    }
    if filled:
        params["fill"] = True
    return {"tool": "polygon", "params": params}


def flood(layer: int, x: int, y: int, color: int) -> dict:
    return {
        "tool": "fill",
        "params": {"layer": layer, "x": x, "y": y, "colorIndex": color, "connectivity": 4},
    }


def paint(layer: int, x: int, y: int, color: int) -> dict:
    return {
        "tool": "paint",
        "params": {"layer": layer, "x": x, "y": y, "colorIndex": color},
    }


def drawing_requests() -> list[dict]:
    tower = [(12, 20), (19, 20), (22, 51), (9, 51)]
    dome = (10, 2, 12, 8)
    # A short cone: lantern edge, one red stripe, and empty sky. It must not
    # cover the tower face. Opacity 128 then flatten is what the game sprite uses.
    beam = [(19, 14), (31, 10), (31, 32), (18, 27)]
    return [
        ellipse(MASONRY, 1, 54, 30, 10, STONE, True),
        ellipse(MASONRY, 0, 56, 8, 7, GREEN, True),
        polygon(MASONRY, [(4, 49), (27, 49), (29, 57), (2, 57)], STONE, True),
        ellipse(MASONRY, 8, 58, 16, 6, SHADOW, True),
        polygon(MASONRY, tower, CREAM, True),
        polygon(MASONRY, tower, INK, False),
        {
            "tool": "selectConnected",
            "params": {"layer": MASONRY, "x": 15, "y": 32, "tolerance": 0, "connectivity": 4},
        },
        fill_rect(MASONRY, 6, 26, 20, 4, RED),
        fill_rect(MASONRY, 6, 36, 20, 4, RED),
        fill_rect(MASONRY, 6, 46, 20, 3, RED),
        {"tool": "clearSelection"},
        fill_rect(MASONRY, 14, 43, 5, 8, WOOD),
        paint(MASONRY, 17, 47, GOLD),
        ellipse(LANTERN, dome[0], dome[1], dome[2], dome[3], RED, True),
        ellipse(LANTERN, dome[0], dome[1], dome[2], dome[3], INK, False),
        line(LANTERN, 16, 0, 16, 3, INK),
        fill_rect(LANTERN, 9, 9, 14, 2, INK),
        stroke_rect(LANTERN, 11, 11, 10, 8, INK),
        flood(LANTERN, 14, 14, GOLD),
        line(LANTERN, 16, 11, 16, 18, INK),
        fill_rect(LANTERN, 8, 19, 16, 2, INK),
        line(LANTERN, 8, 20, 3, 30, INK, pixel_perfect=True),
        stroke_rect(LANTERN, 14, 31, 4, 5, INK),
        flood(LANTERN, 15, 33, GLASS),
        polygon(BEAM, beam, GOLD, True),
    ]


def require(pixels: list[int], x: int, y: int, color: int, label: str) -> None:
    got = pixels[y * WIDTH + x]
    if got != color:
        raise SystemExit(f"{label} at ({x},{y}) is index {got}, expected {color}")


def export_png(cli: Cli, project: Path, dest: Path, scale: int) -> None:
    with tempfile.TemporaryDirectory() as tmp:
        folder = Path(tmp)
        cli.run(
            [
                "export",
                "png",
                "--project",
                str(project),
                "--out-dir",
                str(folder),
                "--scale",
                str(scale),
            ]
        )
        shutil.copyfile(folder / "pixelforge-000.png", dest)


def partial_alpha_cells(rgba: bytes) -> list[int]:
    cells = []
    for index in range(WIDTH * HEIGHT):
        alpha = rgba[index * 4 + 3]
        if 0 < alpha < 255:
            cells.append(index)
    return cells


def change_report(
    before_index: list[int],
    after_index: list[int],
    before_rgba: bytes,
    after_rgba: bytes,
) -> tuple[dict[str, int], list[dict]]:
    histogram: dict[str, int] = {}
    samples = []
    seen: set[tuple[int, int]] = set()
    for index, (left, right) in enumerate(zip(before_index, after_index)):
        if left == right:
            continue
        key = f"{left}->{right}"
        histogram[key] = histogram.get(key, 0) + 1
        pair = (left, right)
        if pair in seen:
            continue
        seen.add(pair)
        samples.append(
            {
                "x": index % WIDTH,
                "y": index // WIDTH,
                "beforeIndex": left,
                "afterIndex": right,
                "beforeRgba": list(before_rgba[index * 4 : index * 4 + 4]),
                "afterRgba": list(after_rgba[index * 4 : index * 4 + 4]),
            }
        )
    return histogram, samples


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=ROOT)
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)

    cli = Cli(binary())
    project = out / "lighthouse.json"
    if project.exists():
        project.unlink()

    cli.run(["new", "--width", str(WIDTH), "--height", str(HEIGHT), "--output", str(project)])
    cli.run(["layer", "rename", "--project", str(project), "--name", "masonry"])
    added_stone = cli.run(
        ["palette", "add", "--project", str(project), "--r", "132", "--g", "124", "--b", "112"]
    )
    added_shadow = cli.run(
        ["palette", "add", "--project", str(project), "--r", "72", "--g", "66", "--b", "62"]
    )
    stone_index = json.loads(added_stone.stdout)["result"]["index"]
    shadow_index = json.loads(added_shadow.stdout)["result"]["index"]
    if stone_index != STONE or shadow_index != SHADOW:
        raise SystemExit(f"palette grew to {stone_index}, {shadow_index}, expected {STONE}, {SHADOW}")
    cli.run(["layer", "add", "--project", str(project), "--name", "lantern"])
    cli.run(["layer", "add", "--project", str(project), "--name", "beam"])
    cli.run(
        [
            "layer",
            "opacity",
            "--project",
            str(project),
            "--index",
            str(BEAM),
            "--opacity",
            str(BEAM_OPACITY),
        ]
    )

    requests = drawing_requests()
    counts: dict[str, int] = {}
    for request in requests:
        counts[request["tool"]] = counts.get(request["tool"], 0) + 1
    cli.call(project, requests)

    with tempfile.TemporaryDirectory() as tmp:
        folder = Path(tmp)
        masonry = cli.preview_pixels(project, MASONRY, folder / "masonry.png")
        lantern = cli.preview_pixels(project, LANTERN, folder / "lantern.png")
        beam = cli.preview_pixels(project, BEAM, folder / "beam.png")
        require(masonry, 15, 28, RED, "stripe")
        require(masonry, 2, 28, 0, "sky beside the stripe")
        require(masonry, 16, 46, WOOD, "door")
        require(masonry, 17, 47, GOLD, "knob")
        require(lantern, 0, 0, 0, "lantern corner")
        require(lantern, 14, 14, GOLD, "lantern glass")
        require(lantern, 15, 33, GLASS, "tower window")
        require(beam, 30, 20, GOLD, "beam over empty sky")
        require(masonry, 15, 24, CREAM, "tower face stays cream")
        if lantern[0] == GOLD:
            raise SystemExit("lantern flood fill leaked to (0,0)")

        before_png = out / "lighthouse-before.png"
        before_preview = out / "lighthouse-before.preview.png"
        flat_png = out / "lighthouse.png"
        flat_preview = out / "lighthouse.preview.png"
        export_png(cli, project, before_png, 1)
        export_png(cli, project, before_preview, 4)

        flat_project = folder / "flat.json"
        shutil.copyfile(project, flat_project)
        cli.run(["layer", "flatten", "--project", str(flat_project)])
        export_png(cli, flat_project, flat_png, 1)
        export_png(cli, flat_project, flat_preview, 4)

        faint_project = folder / "faint.json"
        shutil.copyfile(project, faint_project)
        cli.run(
            [
                "layer",
                "opacity",
                "--project",
                str(faint_project),
                "--index",
                str(BEAM),
                "--opacity",
                "127",
            ]
        )
        cli.run(["layer", "flatten", "--project", str(faint_project)])
        faint_pixels = cli.preview_pixels(faint_project, None, folder / "faint.png")
        before_indices = cli.preview_pixels(project, None, folder / "before-index.png")
        after_indices = cli.preview_pixels(flat_project, None, folder / "after-index.png")
        require(before_indices, 15, 24, CREAM, "composite tower face")

    before_w, before_h, before_rgba = decode(before_png)
    flat_w, flat_h, flat_rgba = decode(flat_png)
    if (before_w, before_h, flat_w, flat_h) != (WIDTH, HEIGHT, WIDTH, HEIGHT):
        raise SystemExit("exported PNG size is not 32x64")
    preview_w, preview_h, _preview = decode(flat_preview)
    if (preview_w, preview_h) != (WIDTH * 4, HEIGHT * 4):
        raise SystemExit("scale-4 preview is not 128x256")

    partial = partial_alpha_cells(before_rgba)
    if len(partial) < 16:
        raise SystemExit(f"layered export has {len(partial)} partial-alpha cells; the beam missed empty sky")
    flat_partial = partial_alpha_cells(flat_rgba)
    if flat_partial:
        raise SystemExit(f"flattened export still has {len(flat_partial)} partial-alpha cells")
    cleared = sum(1 for index in partial if faint_pixels[index] == 0)
    changed = sum(1 for left, right in zip(before_indices, after_indices) if left != right)
    histogram, samples = change_report(before_indices, after_indices, before_rgba, flat_rgba)
    scene = cli.scene(project)
    metrics = {
        "canvas": {"width": WIDTH, "height": HEIGHT},
        "palette": scene["palette"],
        "paletteCount": scene["paletteCount"],
        "layers": [
            {"name": layer["name"], "opacity": layer["opacity"], "visible": layer["visible"]}
            for layer in scene["layers"]
        ],
        "drawingRequests": len(requests),
        "requestsByTool": counts,
        "cliCommands": cli.commands,
        "cliSeconds": round(cli.seconds, 3),
        "flatten": {
            "beamOpacity": BEAM_OPACITY,
            "partialAlphaBefore": len(partial),
            "partialAlphaAfter": len(flat_partial),
            "indexChanges": changed,
            "indexChangeHistogram": histogram,
            "samples": samples,
            "opacity127ClearsPartialSky": cleared,
            "opacity127PartialCells": len(partial),
        },
    }
    (out / "metrics.json").write_text(json.dumps(metrics, indent=2) + "\n", encoding="utf-8")
    print(
        json.dumps(
            {
                "drawingRequests": len(requests),
                "cliSeconds": metrics["cliSeconds"],
                "partialAlphaBefore": len(partial),
                "indexChanges": changed,
                "opacity127ClearsPartialSky": cleared,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    try:
        main()
    except SystemExit as err:
        print(err, file=sys.stderr)
        raise
