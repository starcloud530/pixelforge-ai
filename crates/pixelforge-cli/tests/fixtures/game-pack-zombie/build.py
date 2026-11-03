#!/usr/bin/env python3
"""Build the #18 zombie walker and base-prop sheets with the pixelforge CLI.

The script does not invent RGB in the PNG. It sets palette indices, then
fillRect / eraseRect / paint / moveRect / copyRect / flipHorizontal. Walk
frames are duplicates of a pose with the limb rectangles moved. The right
facing is the left facing mirrored, then the upper-left light is painted back.
The back facing is the front facing with the face covered.

Every frame keeps its lowest opaque row on y=56 so a shared pivot passes
validate-atlas. Export does not write that pivot; the rules files do.

Usage:
    cargo build -p pixelforge-cli
    python3 examples/game-pack-zombie/build.py
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[1]
WIDTH = 64
HEIGHT = 64
SOLE = 56

# Zombie indices. 1-8 replace the studio palette; 9-14 are appended.
OL, BONE, BLOOD, SORE, LIT, SKIN, SHADE = 1, 2, 3, 4, 5, 6, 7
CLOTH, CLOTH_S, PANTS, BOOT, HAIR, EYE, HI = 8, 9, 10, 11, 12, 13, 14

ZOMBIE_COLORS = {
    1: (22, 18, 24),
    2: (236, 226, 196),
    3: (176, 42, 40),
    4: (122, 48, 42),
    5: (198, 214, 132),
    6: (132, 168, 86),
    7: (72, 102, 54),
    8: (62, 78, 108),
    9: (36, 46, 68),
    10: (74, 58, 44),
    11: (42, 36, 32),
    12: (42, 32, 28),
    13: (228, 196, 72),
    14: (232, 236, 196),
}

# Prop indices.
P_OL, WOOD_L, WOOD, WOOD_D, NAIL = 1, 2, 3, 4, 5
METAL_L, METAL, METAL_D, YELLOW, RED = 6, 7, 8, 9, 10
CLOTH_P, RUST, GLASS, WHITE = 11, 12, 13, 14

PROP_COLORS = {
    1: (28, 20, 14),
    2: (214, 176, 112),
    3: (156, 108, 58),
    4: (96, 66, 36),
    5: (214, 210, 198),
    6: (198, 206, 212),
    7: (118, 126, 134),
    8: (62, 68, 76),
    9: (228, 186, 48),
    10: (176, 42, 40),
    11: (86, 118, 74),
    12: (146, 74, 46),
    13: (86, 156, 186),
    14: (236, 232, 220),
}

# Front-view limb boxes. Inners sit inside these so one moveRect carries them.
D_L_ARM = (12, 31, 8, 16)
D_R_ARM = (44, 33, 8, 15)
D_L_THIGH = (22, 43, 8, 9)
D_L_BOOT = (21, 52, 10, 5)
D_R_THIGH = (34, 43, 8, 9)
D_R_BOOT = (34, 52, 10, 5)

# Left-profile limb boxes.
S_FRONT_ARM = (18, 32, 10, 14)
S_BACK_ARM = (42, 33, 6, 12)
S_FRONT_THIGH = (24, 44, 8, 6)
S_FRONT_BOOT = (24, 50, 8, 7)
S_BACK_THIGH = (36, 44, 8, 6)
S_BACK_BOOT = (36, 50, 8, 7)


class Draw:
    def __init__(self) -> None:
        self.ops: list[dict] = []

    def fill(self, x: int, y: int, w: int, h: int, color: int) -> Draw:
        if w <= 0 or h <= 0:
            raise ValueError(f"empty rect {x},{y} {w}x{h}")
        if x < 0 or y < 0 or x + w > WIDTH or y + h > HEIGHT:
            raise ValueError(f"rect out of canvas {x},{y} {w}x{h}")
        self.ops.append(
            {
                "tool": "fillRect",
                "params": {
                    "x": x,
                    "y": y,
                    "width": w,
                    "height": h,
                    "colorIndex": color,
                },
            }
        )
        return self

    def erase(self, x: int, y: int, w: int, h: int) -> Draw:
        if x < 0 or y < 0 or x + w > WIDTH or y + h > HEIGHT:
            raise ValueError(f"erase out of canvas {x},{y} {w}x{h}")
        self.ops.append(
            {
                "tool": "eraseRect",
                "params": {"x": x, "y": y, "width": w, "height": h},
            }
        )
        return self

    def paint(self, x: int, y: int, color: int) -> Draw:
        self.ops.append(
            {
                "tool": "paint",
                "params": {"x": x, "y": y, "colorIndex": color},
            }
        )
        return self

    def move(self, box: tuple[int, int, int, int], dst_x: int, dst_y: int) -> Draw:
        x, y, w, h = box
        if dst_x < 0 or dst_y < 0 or dst_x + w > WIDTH or dst_y + h > HEIGHT:
            raise ValueError(f"move dest out of canvas {dst_x},{dst_y} {w}x{h}")
        self.ops.append(
            {
                "tool": "moveRect",
                "params": {
                    "srcX": x,
                    "srcY": y,
                    "width": w,
                    "height": h,
                    "dstX": dst_x,
                    "dstY": dst_y,
                },
            }
        )
        return self


class Cli:
    def __init__(self, binary: str, project: Path) -> None:
        self.binary = binary
        self.project = project

    def run(self, args: list[str]) -> subprocess.CompletedProcess[str]:
        proc = subprocess.run(
            [self.binary, *args],
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if proc.returncode != 0:
            raise SystemExit(
                f"pixelforge {' '.join(args)} failed ({proc.returncode})\n"
                f"{proc.stderr}\n{proc.stdout}"
            )
        return proc

    def call(self, requests: list[dict]) -> None:
        if not requests:
            return
        with tempfile.NamedTemporaryFile(
            "w", suffix=".json", delete=False, encoding="utf-8"
        ) as handle:
            json.dump(requests, handle)
            path = handle.name
        try:
            proc = self.run(
                ["call", "--project", str(self.project), "--request-file", path]
            )
        finally:
            os.unlink(path)
        for line in proc.stdout.splitlines():
            if not line.strip():
                continue
            body = json.loads(line)
            if not body.get("ok", False):
                raise SystemExit(f"tool refused: {line}")

    def preview_pixels(self, frame: int) -> list[int]:
        proc = self.run(
            [
                "preview",
                "--project",
                str(self.project),
                "--frame",
                str(frame),
                "--scale",
                "1",
                "--output",
                str(ROOT / ".preview-tmp.png"),
            ]
        )
        body = json.loads(proc.stdout)
        return body["result"]["pixels"]


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


def palette_requests(colors: dict[int, tuple[int, int, int]]) -> list[dict]:
    requests: list[dict] = []
    for index, (red, green, blue) in colors.items():
        if index <= 8:
            requests.append(
                {
                    "tool": "setPaletteColor",
                    "params": {"index": index, "r": red, "g": green, "b": blue},
                }
            )
        else:
            requests.append(
                {
                    "tool": "addPaletteColor",
                    "params": {"r": red, "g": green, "b": blue},
                }
            )
    return requests


def apply(cli: Cli, frame: int, draw: Draw) -> None:
    cli.call(
        [{"tool": "setActiveFrame", "params": {"index": frame}}, *draw.ops]
    )


def copy_frame(cli: Cli, src: int, dst: int) -> None:
    cli.call(
        [
            {
                "tool": "copyRect",
                "params": {
                    "fromFrame": src,
                    "toFrame": dst,
                    "srcX": 0,
                    "srcY": 0,
                    "width": WIDTH,
                    "height": HEIGHT,
                    "dstX": 0,
                    "dstY": 0,
                },
            }
        ]
    )


def hflip(cli: Cli, frame: int) -> None:
    cli.call([{"tool": "flipHorizontal", "params": {"frame": frame}}])


def clear(draw: Draw) -> Draw:
    return draw.erase(0, 0, WIDTH, HEIGHT)


def front_idle() -> Draw:
    draw = Draw()
    # Head, stepped so the skull is not one rectangle.
    draw.fill(28, 10, 8, 2, OL)
    draw.fill(25, 12, 14, 2, OL)
    draw.fill(23, 14, 18, 10, OL)
    draw.fill(25, 24, 14, 3, OL)
    draw.erase(23, 14, 2, 2)
    draw.erase(39, 14, 2, 2)
    draw.erase(23, 22, 1, 2)
    draw.erase(40, 22, 1, 2)
    draw.fill(26, 12, 12, 3, HAIR)
    draw.fill(24, 15, 4, 3, HAIR)
    draw.fill(36, 15, 4, 3, HAIR)
    draw.fill(25, 16, 14, 7, SKIN)
    draw.fill(26, 23, 12, 3, SKIN)
    draw.fill(25, 17, 3, 5, LIT)
    draw.fill(36, 18, 3, 5, SHADE)
    # Milky left eye, ruined right eye.
    draw.fill(27, 18, 4, 3, BONE)
    draw.fill(28, 19, 2, 2, EYE)
    draw.paint(29, 19, OL)
    draw.fill(34, 18, 4, 3, SORE)
    draw.fill(35, 19, 2, 2, BLOOD)
    draw.fill(29, 22, 6, 2, OL)
    draw.paint(30, 22, BONE)
    draw.paint(32, 22, BONE)
    draw.paint(33, 23, BLOOD)
    # Neck and shirt.
    draw.fill(29, 27, 6, 3, SKIN)
    draw.fill(29, 27, 1, 3, OL)
    draw.fill(34, 27, 1, 3, OL)
    draw.fill(21, 29, 22, 14, OL)
    draw.fill(22, 30, 20, 12, CLOTH)
    draw.fill(22, 30, 4, 6, HI)
    draw.fill(35, 30, 7, 12, CLOTH_S)
    draw.fill(28, 34, 6, 5, SKIN)
    draw.fill(28, 34, 6, 1, SORE)
    draw.fill(30, 36, 3, 2, BLOOD)
    # Arms. Boxes match D_L_ARM / D_R_ARM.
    draw.fill(*D_L_ARM, OL)
    draw.fill(13, 32, 6, 14, SKIN)
    draw.fill(13, 32, 2, 6, LIT)
    draw.fill(13, 44, 6, 2, SHADE)
    draw.fill(*D_R_ARM, OL)
    draw.fill(45, 34, 6, 13, SHADE)
    draw.fill(45, 44, 6, 3, SKIN)
    # Shoulder bridges sit mostly outside the arm boxes so a swing keeps a stub.
    draw.fill(19, 32, 5, 4, OL)
    draw.fill(20, 33, 3, 2, SKIN)
    draw.fill(40, 34, 5, 4, OL)
    draw.fill(41, 35, 3, 2, CLOTH_S)
    # Legs and boots. The outline row y=56 is the sole.
    draw.fill(*D_L_THIGH, OL)
    draw.fill(23, 44, 6, 7, PANTS)
    draw.fill(*D_L_BOOT, OL)
    draw.fill(22, 53, 8, 3, BOOT)
    draw.fill(22, 53, 2, 2, HI)
    draw.fill(*D_R_THIGH, OL)
    draw.fill(35, 44, 6, 7, PANTS)
    draw.fill(*D_R_BOOT, OL)
    draw.fill(35, 53, 8, 3, BOOT)
    return draw


def front_walk_a() -> Draw:
    """Lift the screen-left foot. The other sole stays on y=56."""
    draw = Draw()
    draw.move(D_L_THIGH, 18, 43)
    draw.move(D_L_BOOT, 16, 49)
    draw.move(D_L_ARM, 10, 35)
    draw.move(D_R_ARM, 46, 30)
    draw.fill(16, 36, 5, 3, SKIN)
    draw.fill(42, 31, 5, 3, SHADE)
    draw.fill(20, 42, 4, 3, CLOTH)
    draw.fill(18, 48, 4, 2, PANTS)
    return draw


def front_walk_b() -> Draw:
    """Passing pose. Both soles stay where idle put them."""
    draw = Draw()
    draw.move(D_L_ARM, 14, 30)
    draw.move(D_R_ARM, 42, 36)
    return draw


def front_walk_c() -> Draw:
    """Lift the screen-right foot."""
    draw = Draw()
    draw.move(D_R_THIGH, 38, 43)
    draw.move(D_R_BOOT, 40, 49)
    draw.move(D_L_ARM, 14, 29)
    draw.move(D_R_ARM, 42, 36)
    draw.fill(36, 42, 4, 3, CLOTH)
    draw.fill(38, 48, 4, 2, PANTS)
    return draw


def left_idle() -> Draw:
    draw = Draw()
    clear(draw)
    # Skull and muzzle, nose toward screen-left.
    draw.fill(32, 10, 10, 2, OL)
    draw.fill(30, 12, 14, 12, OL)
    draw.fill(32, 24, 10, 3, OL)
    draw.fill(22, 17, 10, 9, OL)
    draw.erase(30, 12, 2, 2)
    draw.erase(42, 12, 2, 2)
    draw.erase(22, 17, 2, 2)
    draw.fill(32, 11, 10, 5, HAIR)
    draw.fill(30, 14, 4, 3, HAIR)
    draw.fill(31, 16, 11, 7, SKIN)
    draw.fill(23, 18, 8, 6, SKIN)
    draw.fill(38, 17, 4, 6, SHADE)
    draw.fill(25, 19, 3, 3, BONE)
    draw.fill(26, 20, 2, 2, EYE)
    draw.paint(26, 20, OL)
    draw.fill(23, 23, 5, 2, OL)
    draw.paint(24, 23, BONE)
    draw.paint(26, 24, BLOOD)
    draw.fill(42, 17, 3, 5, SKIN)
    draw.fill(43, 18, 2, 2, BLOOD)
    draw.fill(44, 17, 1, 5, OL)
    draw.fill(34, 27, 5, 3, SKIN)
    draw.fill(30, 29, 14, 15, OL)
    draw.fill(31, 30, 12, 13, CLOTH)
    draw.fill(39, 32, 4, 10, CLOTH_S)
    draw.fill(33, 35, 5, 4, SKIN)
    draw.fill(34, 37, 3, 2, BLOOD)
    draw.fill(*S_FRONT_ARM, OL)
    draw.fill(19, 33, 8, 12, SKIN)
    draw.fill(27, 33, 5, 4, OL)
    draw.fill(28, 34, 3, 2, CLOTH)
    draw.fill(*S_BACK_ARM, OL)
    draw.fill(43, 34, 4, 10, SHADE)
    draw.fill(*S_FRONT_THIGH, OL)
    draw.fill(25, 45, 6, 4, PANTS)
    draw.fill(*S_FRONT_BOOT, OL)
    draw.fill(25, 51, 6, 5, BOOT)
    draw.fill(*S_BACK_THIGH, OL)
    draw.fill(37, 45, 6, 4, PANTS)
    draw.fill(*S_BACK_BOOT, OL)
    draw.fill(37, 51, 6, 5, BOOT)
    return draw


def left_walk_a() -> Draw:
    """Reach the front (screen-left) foot. The back sole stays."""
    draw = Draw()
    draw.move(S_FRONT_THIGH, 20, 44)
    draw.move(S_FRONT_BOOT, 16, 47)
    draw.move(S_FRONT_ARM, 14, 34)
    draw.move(S_BACK_ARM, 44, 31)
    draw.fill(22, 36, 7, 3, SKIN)
    draw.fill(26, 48, 4, 3, PANTS)
    return draw


def left_walk_b() -> Draw:
    draw = Draw()
    draw.move(S_FRONT_ARM, 20, 30)
    draw.move(S_BACK_ARM, 40, 35)
    return draw


def left_walk_c() -> Draw:
    """Plant the front foot forward and lift the back foot."""
    draw = Draw()
    draw.move(S_BACK_THIGH, 38, 43)
    draw.move(S_BACK_BOOT, 40, 47)
    draw.move(S_FRONT_THIGH, 20, 44)
    draw.move(S_FRONT_BOOT, 18, 50)
    draw.move(S_FRONT_ARM, 16, 36)
    draw.move(S_BACK_ARM, 44, 30)
    draw.fill(24, 36, 8, 4, SKIN)
    draw.fill(27, 42, 5, 4, CLOTH)
    draw.fill(22, 49, 4, 2, PANTS)
    return draw


def to_back(draw: Draw) -> Draw:
    """Cover the face. Feet and the shirt tear stay."""
    draw.fill(25, 12, 14, 14, HAIR)
    draw.fill(24, 16, 2, 5, SKIN)
    draw.fill(38, 16, 2, 5, SKIN)
    draw.fill(23, 16, 1, 5, OL)
    draw.fill(40, 16, 1, 5, OL)
    draw.fill(28, 23, 8, 3, SHADE)
    draw.fill(25, 13, 3, 5, LIT)
    draw.fill(31, 32, 2, 8, CLOTH_S)
    return draw


def right_highlights() -> Draw:
    """World light stays upper-left after hflip, so it hits the back of the head."""
    draw = Draw()
    draw.fill(20, 12, 4, 5, LIT)
    draw.fill(18, 33, 3, 5, HI)
    draw.fill(21, 18, 2, 3, SKIN)
    return draw


def left_highlights() -> Draw:
    """Upper-left light on the face. The head does not move in the walk."""
    draw = Draw()
    draw.fill(23, 18, 3, 3, LIT)
    draw.fill(32, 12, 2, 3, HI)
    return draw


def barricade() -> Draw:
    draw = Draw()
    draw.fill(8, 22, 6, 35, P_OL)
    draw.fill(50, 22, 6, 35, P_OL)
    draw.fill(9, 23, 4, 32, WOOD_D)
    draw.fill(51, 23, 4, 32, WOOD_D)
    draw.fill(9, 23, 2, 32, WOOD)
    for top in (28, 38, 48):
        draw.fill(12, top, 40, 7, P_OL)
        draw.fill(13, top + 1, 38, 5, WOOD)
        draw.fill(13, top + 1, 38, 1, WOOD_L)
    draw.erase(28, 39, 8, 5)
    draw.fill(30, 43, 3, 2, WOOD_D)
    for x in (16, 42):
        for y in (31, 41, 51):
            draw.fill(x, y, 2, 2, NAIL)
    return draw


def crate() -> Draw:
    draw = Draw()
    clear(draw)
    draw.fill(16, 28, 32, 29, P_OL)
    draw.fill(17, 29, 30, 27, WOOD)
    draw.fill(17, 29, 30, 2, WOOD_L)
    draw.fill(17, 52, 30, 3, WOOD_D)
    draw.fill(17, 36, 30, 2, P_OL)
    draw.fill(29, 29, 4, 27, WOOD_D)
    draw.fill(30, 29, 2, 27, WOOD)
    for x, y in ((18, 30), (44, 30), (18, 52), (44, 52)):
        draw.fill(x, y, 2, 2, NAIL)
    draw.fill(20, 40, 6, 2, WOOD_L)
    return draw


def generator() -> Draw:
    draw = Draw()
    clear(draw)
    draw.fill(40, 16, 8, 10, P_OL)
    draw.fill(41, 17, 6, 8, METAL_D)
    draw.fill(42, 14, 4, 3, RUST)
    draw.fill(14, 24, 36, 33, P_OL)
    draw.fill(15, 25, 34, 30, METAL)
    draw.fill(15, 25, 8, 12, METAL_L)
    draw.fill(40, 25, 9, 30, METAL_D)
    draw.fill(15, 40, 34, 6, YELLOW)
    for x in (15, 27, 39):
        draw.fill(x, 40, 6, 6, P_OL)
    draw.fill(22, 28, 10, 9, P_OL)
    draw.fill(23, 29, 8, 7, GLASS)
    draw.fill(26, 30, 2, 4, RED)
    draw.fill(18, 50, 28, 3, METAL_D)
    draw.fill(20, 52, 4, 2, YELLOW)
    return draw


def medkit() -> Draw:
    draw = Draw()
    clear(draw)
    draw.fill(26, 28, 12, 6, P_OL)
    draw.fill(27, 29, 10, 4, CLOTH_P)
    draw.fill(18, 34, 28, 23, P_OL)
    draw.fill(19, 35, 26, 20, WHITE)
    draw.fill(19, 35, 6, 8, WOOD_L)
    draw.fill(19, 51, 26, 4, METAL)
    draw.fill(29, 38, 6, 14, RED)
    draw.fill(24, 42, 16, 6, RED)
    return draw


def assert_soles(cli: Cli, frames: range) -> None:
    for frame in frames:
        pixels = cli.preview_pixels(frame)
        opaque = [index for index, value in enumerate(pixels) if value]
        if not opaque:
            raise SystemExit(f"frame {frame} is empty")
        bottom = max(index // WIDTH for index in opaque)
        if bottom != SOLE:
            raise SystemExit(
                f"frame {frame} opaque bottom is y={bottom}, expected {SOLE}"
            )


def write_rules(path: Path, body: dict) -> None:
    path.write_text(json.dumps(body, indent=2) + "\n", encoding="utf-8")


def export_sheet(cli: Cli, png: Path, atlas: Path, cols: int) -> None:
    cli.run(
        [
            "export",
            "sheet",
            "--project",
            str(cli.project),
            "--output",
            str(png),
            "--cols",
            str(cols),
            "--scale",
            "1",
            "--atlas",
            str(atlas),
        ]
    )


def validate(binary: str, atlas: Path, rules: Path) -> None:
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
        raise SystemExit(proc.stdout + proc.stderr)
    body = json.loads(proc.stdout)
    if not body.get("ok") or body["result"]["errorCount"] != 0:
        raise SystemExit(proc.stdout)


def build_zombie(binary: str) -> None:
    folder = ROOT / "zombie"
    folder.mkdir(parents=True, exist_ok=True)
    project = folder / "zombie.json"
    project.unlink(missing_ok=True)
    cli = Cli(binary, project)
    cli.run(
        [
            "new",
            "--width",
            str(WIDTH),
            "--height",
            str(HEIGHT),
            "--output",
            str(project),
        ]
    )
    cli.call(palette_requests(ZOMBIE_COLORS))
    apply(cli, 0, front_idle())
    cli.call([{"tool": "duplicateFrame", "params": {"index": 0}} for _ in range(15)])
    apply(cli, 1, front_walk_a())
    apply(cli, 2, front_walk_b())
    apply(cli, 3, front_walk_c())
    apply(cli, 4, left_idle())
    for dst in (5, 6, 7):
        copy_frame(cli, 4, dst)
    apply(cli, 5, left_walk_a())
    apply(cli, 6, left_walk_b())
    apply(cli, 7, left_walk_c())
    for src, dst in ((4, 8), (5, 9), (6, 10), (7, 11)):
        copy_frame(cli, src, dst)
        hflip(cli, dst)
    apply(cli, 4, left_highlights())
    apply(cli, 5, left_highlights())
    apply(cli, 6, left_highlights())
    apply(cli, 7, left_highlights())
    for dst in (8, 9, 10, 11):
        apply(cli, dst, right_highlights())
    for src, dst in ((0, 12), (1, 13), (2, 14), (3, 15)):
        copy_frame(cli, src, dst)
        apply(cli, dst, to_back(Draw()))
    delays: list[dict] = []
    for index in range(16):
        delay = 200 if index % 4 == 0 else 120
        delays.append(
            {
                "tool": "setFrameDelay",
                "params": {"index": index, "delayMs": delay},
            }
        )
    facings = ("down", "left", "right", "up")
    for row, facing in enumerate(facings):
        start = row * 4
        delays.append(
            {
                "tool": "addTag",
                "params": {
                    "name": f"idle_{facing}",
                    "from": start,
                    "to": start,
                    "loop": "forward",
                },
            }
        )
        delays.append(
            {
                "tool": "addTag",
                "params": {
                    "name": f"walk_{facing}",
                    "from": start + 1,
                    "to": start + 3,
                    "loop": "forward",
                },
            }
        )
    cli.call(delays)
    cli.run(
        [
            "direction",
            "set",
            "--project",
            str(project),
            "--count",
            "4",
            "--columns",
            "4",
        ]
    )
    assert_soles(cli, range(16))
    png = folder / "zombie-sheet.png"
    atlas = folder / "zombie-sheet.json"
    export_sheet(cli, png, atlas, 4)
    rules = {
        "format": "pixelforge-atlas-rules",
        "version": 1,
        "mode": "strict",
        "sizes": [64],
        "requireTags": [
            "idle_down",
            "walk_down",
            "idle_left",
            "walk_left",
            "idle_right",
            "walk_right",
            "idle_up",
            "walk_up",
        ],
        "fourDirection": "on",
        "tagPolicy": "required",
        "pivot": {"x": 32, "y": 57},
    }
    write_rules(folder / "zombie-rules.json", rules)
    validate(binary, atlas, folder / "zombie-rules.json")
    print(f"wrote {png}")


def build_props(binary: str) -> None:
    folder = ROOT / "props"
    folder.mkdir(parents=True, exist_ok=True)
    project = folder / "props.json"
    project.unlink(missing_ok=True)
    cli = Cli(binary, project)
    cli.run(
        [
            "new",
            "--width",
            str(WIDTH),
            "--height",
            str(HEIGHT),
            "--output",
            str(project),
        ]
    )
    cli.call(palette_requests(PROP_COLORS))
    apply(cli, 0, barricade())
    cli.call([{"tool": "duplicateFrame", "params": {"index": 0}} for _ in range(3)])
    apply(cli, 1, crate())
    apply(cli, 2, generator())
    apply(cli, 3, medkit())
    names = ("barricade", "crate", "generator", "medkit")
    requests: list[dict] = []
    for index, name in enumerate(names):
        requests.append(
            {
                "tool": "setFrameDelay",
                "params": {"index": index, "delayMs": 100},
            }
        )
        requests.append(
            {
                "tool": "addTag",
                "params": {
                    "name": name,
                    "from": index,
                    "to": index,
                    "loop": "once",
                },
            }
        )
    cli.call(requests)
    assert_soles(cli, range(4))
    png = folder / "props-sheet.png"
    atlas = folder / "props-sheet.json"
    export_sheet(cli, png, atlas, 4)
    rules = {
        "format": "pixelforge-atlas-rules",
        "version": 1,
        "mode": "strict",
        "sizes": [64],
        "requireTags": list(names),
        "fourDirection": "off",
        "tagPolicy": "optional",
        "pivot": {"x": 32, "y": 57},
    }
    write_rules(folder / "props-rules.json", rules)
    validate(binary, atlas, folder / "props-rules.json")
    print(f"wrote {png}")


def main() -> None:
    binary = find_binary()
    which = set(sys.argv[1:])
    if not which or "--zombie" in which or "--all" in which:
        build_zombie(binary)
    if not which or "--props" in which or "--all" in which:
        build_props(binary)
    preview = ROOT / ".preview-tmp.png"
    preview.unlink(missing_ok=True)


if __name__ == "__main__":
    main()
