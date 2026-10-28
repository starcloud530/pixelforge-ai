#!/usr/bin/env python3
"""Shared CLI driver for the #81 harbor-courier cases.

Pixels are palette indices written with fillRect, eraseRect, paint, moveRect,
copyRect, and flipHorizontal. This module does not call an image model and
does not invent RGB in a PNG. Side facings are redrawn, not rotated. The walk
is a duplicated pose plus limb moves, not an IK gait.

Canvas is the formal 64 tier. The lowest opaque row of every frame is y=56
so a shared pivot (32, 57) passes validate-atlas.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

CASES = Path(__file__).resolve().parent
REPO = CASES.parents[1]
WIDTH = 64
HEIGHT = 64
SOLE = 56
PIVOT = {"x": 32, "y": 57}

BODY = 0
GEAR = 1

OL, SKIN, SKIN_S, HAIR, COAT, COAT_S, CLOAK, CLOAK_S = range(1, 9)
PANTS, BOOT, LANTERN, GLOW, EYE, HI, WOOD, WOOD_D, ROPE, RED = range(9, 19)

COLORS = {
    OL: (32, 24, 36),
    SKIN: (236, 196, 164),
    SKIN_S: (186, 132, 108),
    HAIR: (54, 36, 40),
    COAT: (46, 108, 118),
    COAT_S: (24, 64, 74),
    CLOAK: (92, 78, 138),
    CLOAK_S: (52, 42, 86),
    PANTS: (62, 54, 68),
    BOOT: (42, 32, 36),
    LANTERN: (228, 168, 64),
    GLOW: (255, 224, 150),
    EYE: (28, 24, 32),
    HI: (210, 228, 220),
    WOOD: (156, 108, 64),
    WOOD_D: (96, 66, 40),
    ROPE: (196, 168, 116),
    RED: (176, 56, 52),
}

# Front-view limb boxes. Inners sit inside these so one moveRect carries them.
# Sleeves are gear. Hands, legs, and boots are body. Same-layer boxes do not
# overlap, so a move does not have to share ink with another limb.
D_L_SLEEVE = (12, 26, 8, 12)
D_R_SLEEVE = (44, 26, 8, 12)
D_L_HAND = (13, 38, 6, 4)
D_R_HAND = (45, 38, 6, 4)
D_L_LEG = (24, 40, 7, 10)
D_R_LEG = (33, 40, 7, 10)
D_L_BOOT = (22, 50, 10, 7)
D_R_BOOT = (33, 50, 10, 7)
FRONT_LAMP = (33, 29, 6, 7)

# Left-profile limb boxes. The nose points at screen-left.
S_F_SLEEVE = (16, 26, 10, 12)
S_B_SLEEVE = (44, 28, 6, 10)
S_F_HAND = (14, 38, 6, 4)
S_B_HAND = (46, 38, 5, 4)
S_F_LEG = (22, 40, 8, 10)
S_B_LEG = (36, 40, 8, 10)
S_F_BOOT = (18, 50, 10, 7)
S_B_BOOT = (36, 50, 10, 7)
SIDE_LAMP = (32, 31, 5, 5)
# Light sits above the eye. The rear-hair rect is where hflip should put light.
SIDE_FACE_LIGHT = (30, 16, 3, 2)
SIDE_REAR_HAIR = (40, 9, 4, 3)
SIDE_COAT_LIGHT = (29, 26, 3, 5)
SIDE_COAT_SHADE = (38, 26, 3, 8)

# Hood rims sit on cells the body leaves empty. Swap skips a rect that would
# cover body ink.
FRONT_HOOD = ((26, 5, 12, 3), (20, 10, 4, 8), (40, 10, 4, 8))
SIDE_HOOD = ((34, 5, 12, 3), (48, 11, 3, 5))
BACK_HOOD = ((26, 4, 12, 3), (18, 10, 3, 6), (42, 10, 3, 6))

CHARACTER_TAGS = [
    "idle_down",
    "walk_down",
    "idle_left",
    "walk_left",
    "idle_right",
    "walk_right",
    "idle_up",
    "walk_up",
]
PROP_TAGS = ["satchel", "lantern", "crate", "buoy"]


class Draw:
    def __init__(self, layer: int) -> None:
        self.layer = layer
        self.ops: list[dict] = []

    def fill(self, x: int, y: int, w: int, h: int, color: int) -> Draw:
        self._rect("fillRect", x, y, w, h, color)
        return self

    def erase(self, x: int, y: int, w: int, h: int) -> Draw:
        self._rect("eraseRect", x, y, w, h, None)
        return self

    def paint(self, x: int, y: int, color: int) -> Draw:
        if not (0 <= x < WIDTH and 0 <= y < HEIGHT):
            raise ValueError(f"paint off canvas {x},{y}")
        self.ops.append(
            {
                "tool": "paint",
                "params": {"x": x, "y": y, "colorIndex": color, "layer": self.layer},
            }
        )
        return self

    def move(self, box: tuple[int, int, int, int], dst_x: int, dst_y: int) -> Draw:
        x, y, w, h = box
        if dst_x < 0 or dst_y < 0 or dst_x + w > WIDTH or dst_y + h > HEIGHT:
            raise ValueError(f"move dest off canvas {dst_x},{dst_y} {w}x{h}")
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
                    "layer": self.layer,
                },
            }
        )
        return self

    def _rect(
        self, tool: str, x: int, y: int, w: int, h: int, color: int | None
    ) -> None:
        if w <= 0 or h <= 0:
            raise ValueError(f"empty rect {x},{y} {w}x{h}")
        if x < 0 or y < 0 or x + w > WIDTH or y + h > HEIGHT:
            raise ValueError(f"{tool} off canvas {x},{y} {w}x{h}")
        params: dict = {
            "x": x,
            "y": y,
            "width": w,
            "height": h,
            "layer": self.layer,
        }
        if color is not None:
            params["colorIndex"] = color
        self.ops.append({"tool": tool, "params": params})


class Cli:
    def __init__(self, binary: str, project: Path) -> None:
        self.binary = binary
        self.project = project

    def run(self, args: list[str], allow_fail: bool = False) -> subprocess.CompletedProcess[str]:
        proc = subprocess.run(
            [self.binary, *args],
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if proc.returncode != 0 and not allow_fail:
            raise SystemExit(
                f"pixelforge {' '.join(args)} failed ({proc.returncode})\n"
                f"{proc.stderr}\n{proc.stdout}"
            )
        return proc

    def call(self, requests: list[dict]) -> list[dict]:
        if not requests:
            return []
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
        bodies = []
        for line in proc.stdout.splitlines():
            if not line.strip():
                continue
            body = json.loads(line)
            if not body.get("ok", False):
                raise SystemExit(f"tool refused: {line}")
            bodies.append(body)
        return bodies


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
    raise SystemExit(
        "pixelforge is not on PATH. Install a prebuilt binary from Releases, "
        "or cargo build -p pixelforge-cli, or set PIXELFORGE."
    )


def mirror_rect(rect: tuple[int, int, int, int]) -> tuple[int, int, int, int]:
    x, y, w, h = rect
    return (WIDTH - x - w, y, w, h)


def outlined(draw: Draw, box: tuple[int, int, int, int], color: int) -> None:
    x, y, w, h = box
    draw.fill(x, y, w, h, OL)
    if w > 2 and h > 2:
        draw.fill(x + 1, y + 1, w - 2, h - 2, color)


def sole_outline(draw: Draw, box: tuple[int, int, int, int], color: int) -> None:
    """Fill a boot and leave the bottom row as outline, which is the sole."""
    x, y, w, h = box
    draw.fill(x, y, w, h, OL)
    if w > 2 and h > 2:
        draw.fill(x + 1, y + 1, w - 2, h - 2, color)


def new_project(cli: Cli) -> None:
    cli.project.unlink(missing_ok=True)
    cli.project.parent.mkdir(parents=True, exist_ok=True)
    cli.run(
        [
            "new",
            "--width",
            str(WIDTH),
            "--height",
            str(HEIGHT),
            "--output",
            str(cli.project),
        ]
    )


def apply_palette(cli: Cli) -> None:
    sets = []
    for index in range(1, 9):
        red, green, blue = COLORS[index]
        sets.append(
            {
                "tool": "setPaletteColor",
                "params": {"index": index, "r": red, "g": green, "b": blue},
            }
        )
    cli.call(sets)
    for index in range(9, 19):
        red, green, blue = COLORS[index]
        bodies = cli.call(
            [
                {
                    "tool": "addPaletteColor",
                    "params": {"r": red, "g": green, "b": blue},
                }
            ]
        )
        got = bodies[0]["result"]["index"]
        if got != index:
            raise SystemExit(f"palette add returned {got}, expected {index}")


def apply(cli: Cli, frame: int, *draws: Draw) -> None:
    ops: list[dict] = [{"tool": "setActiveFrame", "params": {"index": frame}}]
    for draw in draws:
        ops.extend(draw.ops)
    cli.call(ops)


def copy_layers(cli: Cli, src: int, dst: int, layers: tuple[int, ...]) -> None:
    ops = []
    for layer in layers:
        ops.append(
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
                    "layer": layer,
                },
            }
        )
    cli.call(ops)


def hflip_layers(cli: Cli, frame: int, layers: tuple[int, ...]) -> None:
    cli.call(
        [
            {
                "tool": "flipHorizontal",
                "params": {"frame": frame, "layer": layer},
            }
            for layer in layers
        ]
    )


def front_body() -> Draw:
    draw = Draw(BODY)
    draw.fill(26, 8, 12, 2, OL)
    draw.fill(24, 10, 16, 10, OL)
    draw.fill(26, 20, 12, 2, OL)
    draw.erase(24, 10, 2, 2)
    draw.erase(38, 10, 2, 2)
    draw.fill(25, 8, 14, 4, HAIR)
    draw.fill(24, 11, 3, 3, HAIR)
    draw.fill(37, 11, 3, 3, HAIR)
    draw.fill(26, 12, 12, 8, SKIN)
    draw.fill(26, 12, 3, 3, HI)
    draw.fill(35, 13, 3, 4, SKIN_S)
    draw.paint(28, 15, EYE)
    draw.paint(33, 15, EYE)
    draw.paint(30, 18, OL)
    draw.fill(29, 22, 6, 3, SKIN)
    outlined(draw, D_L_HAND, SKIN)
    outlined(draw, D_R_HAND, SKIN)
    outlined(draw, D_L_LEG, PANTS)
    outlined(draw, D_R_LEG, PANTS)
    sole_outline(draw, D_L_BOOT, BOOT)
    sole_outline(draw, D_R_BOOT, BOOT)
    return draw


def front_coat() -> Draw:
    draw = Draw(GEAR)
    draw.fill(23, 25, 18, 15, OL)
    draw.fill(24, 26, 16, 13, COAT)
    draw.fill(24, 26, 4, 5, HI)
    draw.fill(36, 26, 4, 10, COAT_S)
    draw.fill(24, 36, 16, 2, BOOT)
    draw.fill(20, 26, 4, 6, COAT)
    draw.fill(40, 26, 4, 6, COAT_S)
    outlined(draw, D_L_SLEEVE, COAT)
    outlined(draw, D_R_SLEEVE, COAT_S)
    draw.fill(*FRONT_LAMP, OL)
    draw.fill(34, 30, 4, 5, LANTERN)
    draw.fill(35, 31, 2, 2, GLOW)
    return draw


def shift(box: tuple[int, int, int, int], dx: int, dy: int) -> tuple[int, int]:
    return (box[0] + dx, box[1] + dy)


def join_sleeve(
    gear: Draw,
    box: tuple[int, int, int, int],
    dx: int,
    dy: int,
    shoulder: tuple[int, int, int, int],
    color: int,
) -> None:
    """Fill the gap so a swung sleeve still touches its shoulder stub."""
    sx, sy = shift(box, dx, dy)
    sw, sh = box[2], box[3]
    hx, hy, hw, hh = shoulder
    y0 = max(sy, hy)
    y1 = min(sy + sh, hy + hh)
    if y1 <= y0:
        return
    if sx + sw < hx:
        gear.fill(sx + sw, y0, hx - (sx + sw), y1 - y0, color)
    elif sx > hx + hw:
        gear.fill(hx + hw, y0, sx - (hx + hw), y1 - y0, color)


def front_walk_a() -> tuple[Draw, Draw]:
    """Lift the screen-left foot. The other sole stays on y=56."""
    body, gear = Draw(BODY), Draw(GEAR)
    # Move the upper piece first when the pair shifts up, so the boot does
    # not get copied away by the leg move.
    body.move(D_L_LEG, *shift(D_L_LEG, -4, 0))
    body.move(D_L_BOOT, *shift(D_L_BOOT, -4, -4))
    gear.move(D_L_SLEEVE, *shift(D_L_SLEEVE, -4, 0))
    body.move(D_L_HAND, *shift(D_L_HAND, -4, 0))
    join_sleeve(gear, D_L_SLEEVE, -4, 0, (20, 26, 4, 6), COAT)
    gear.move(D_R_SLEEVE, *shift(D_R_SLEEVE, 4, -4))
    body.move(D_R_HAND, *shift(D_R_HAND, 4, -4))
    join_sleeve(gear, D_R_SLEEVE, 4, -4, (40, 26, 4, 6), COAT_S)
    return body, gear


def front_walk_b() -> tuple[Draw, Draw]:
    """Passing pose. Both soles stay where idle put them."""
    body, gear = Draw(BODY), Draw(GEAR)
    gear.move(D_L_SLEEVE, *shift(D_L_SLEEVE, 3, 0))
    body.move(D_L_HAND, *shift(D_L_HAND, 3, 0))
    gear.move(D_R_SLEEVE, *shift(D_R_SLEEVE, -3, 0))
    body.move(D_R_HAND, *shift(D_R_HAND, -3, 0))
    return body, gear


def front_walk_c() -> tuple[Draw, Draw]:
    """Lift the screen-right foot."""
    body, gear = Draw(BODY), Draw(GEAR)
    body.move(D_R_LEG, *shift(D_R_LEG, 4, 0))
    body.move(D_R_BOOT, *shift(D_R_BOOT, 4, -4))
    gear.move(D_L_SLEEVE, *shift(D_L_SLEEVE, -2, -4))
    body.move(D_L_HAND, *shift(D_L_HAND, -2, -4))
    join_sleeve(gear, D_L_SLEEVE, -2, -4, (20, 26, 4, 6), COAT)
    gear.move(D_R_SLEEVE, *shift(D_R_SLEEVE, 4, 0))
    body.move(D_R_HAND, *shift(D_R_HAND, 4, 0))
    join_sleeve(gear, D_R_SLEEVE, 4, 0, (40, 26, 4, 6), COAT_S)
    return body, gear


def left_body() -> Draw:
    draw = Draw(BODY)
    draw.erase(0, 0, WIDTH, HEIGHT)
    draw.fill(36, 8, 8, 2, OL)
    draw.fill(34, 10, 12, 12, OL)
    draw.erase(34, 10, 2, 2)
    draw.erase(44, 10, 2, 2)
    draw.fill(28, 16, 8, 5, OL)
    draw.erase(28, 16, 2, 2)
    draw.fill(35, 8, 10, 4, HAIR)
    draw.fill(42, 11, 4, 3, HAIR)
    draw.fill(30, 16, 12, 5, SKIN)
    draw.fill(*SIDE_FACE_LIGHT, HI)
    draw.paint(32, 18, EYE)
    draw.paint(29, 19, OL)
    draw.fill(45, 13, 3, 5, SKIN)
    draw.fill(36, 22, 6, 3, SKIN)
    outlined(draw, S_F_HAND, SKIN)
    outlined(draw, S_B_HAND, SKIN)
    outlined(draw, S_F_LEG, PANTS)
    outlined(draw, S_B_LEG, PANTS)
    sole_outline(draw, S_F_BOOT, BOOT)
    sole_outline(draw, S_B_BOOT, BOOT)
    return draw


def left_coat() -> Draw:
    draw = Draw(GEAR)
    draw.erase(0, 0, WIDTH, HEIGHT)
    draw.fill(28, 25, 14, 15, OL)
    draw.fill(29, 26, 12, 13, COAT)
    draw.fill(*SIDE_COAT_LIGHT, HI)
    draw.fill(*SIDE_COAT_SHADE, COAT_S)
    draw.fill(29, 36, 12, 2, BOOT)
    draw.fill(26, 26, 3, 6, COAT)
    draw.fill(42, 28, 2, 5, COAT_S)
    outlined(draw, S_F_SLEEVE, COAT)
    outlined(draw, S_B_SLEEVE, COAT_S)
    draw.fill(*SIDE_LAMP, OL)
    draw.fill(33, 32, 3, 3, LANTERN)
    draw.fill(34, 33, 1, 1, GLOW)
    return draw


def left_walk_a() -> tuple[Draw, Draw]:
    """Reach the front foot. The back sole stays."""
    body, gear = Draw(BODY), Draw(GEAR)
    body.move(S_F_LEG, *shift(S_F_LEG, -6, 0))
    body.move(S_F_BOOT, *shift(S_F_BOOT, -6, -4))
    gear.move(S_F_SLEEVE, *shift(S_F_SLEEVE, -6, 0))
    body.move(S_F_HAND, *shift(S_F_HAND, -6, 0))
    join_sleeve(gear, S_F_SLEEVE, -6, 0, (26, 26, 3, 6), COAT)
    gear.move(S_B_SLEEVE, *shift(S_B_SLEEVE, 2, -2))
    body.move(S_B_HAND, *shift(S_B_HAND, 2, -2))
    join_sleeve(gear, S_B_SLEEVE, 2, -2, (42, 28, 2, 5), COAT_S)
    return body, gear


def left_walk_b() -> tuple[Draw, Draw]:
    body, gear = Draw(BODY), Draw(GEAR)
    gear.move(S_F_SLEEVE, *shift(S_F_SLEEVE, 2, 0))
    body.move(S_F_HAND, *shift(S_F_HAND, 2, 0))
    # The back arm shifts down, so the hand leaves before the sleeve follows.
    body.move(S_B_HAND, *shift(S_B_HAND, 2, 2))
    gear.move(S_B_SLEEVE, *shift(S_B_SLEEVE, 2, 2))
    join_sleeve(gear, S_B_SLEEVE, 2, 2, (42, 28, 2, 5), COAT_S)
    return body, gear


def left_walk_c() -> tuple[Draw, Draw]:
    """Plant the front foot and lift the back foot."""
    body, gear = Draw(BODY), Draw(GEAR)
    body.move(S_B_LEG, *shift(S_B_LEG, 4, 0))
    body.move(S_B_BOOT, *shift(S_B_BOOT, 4, -4))
    gear.move(S_B_SLEEVE, *shift(S_B_SLEEVE, 2, -2))
    body.move(S_B_HAND, *shift(S_B_HAND, 2, -2))
    join_sleeve(gear, S_B_SLEEVE, 2, -2, (42, 28, 2, 5), COAT_S)
    body.move(S_F_HAND, *shift(S_F_HAND, -4, 2))
    gear.move(S_F_SLEEVE, *shift(S_F_SLEEVE, -4, 2))
    join_sleeve(gear, S_F_SLEEVE, -4, 2, (26, 26, 3, 6), COAT)
    return body, gear


def to_back() -> tuple[Draw, Draw]:
    """Cover the face. Feet and the belt stay."""
    body = Draw(BODY)
    body.fill(24, 8, 16, 14, HAIR)
    body.fill(22, 12, 3, 6, SKIN)
    body.fill(39, 12, 3, 6, SKIN)
    body.fill(26, 9, 4, 3, HI)
    gear = Draw(GEAR)
    gear.fill(*FRONT_LAMP, COAT)
    gear.fill(31, 27, 2, 10, COAT_S)
    return body, gear


def right_light() -> tuple[Draw, Draw]:
    """World light stays upper-left after hflip."""
    body = Draw(BODY)
    mx, my, mw, mh = mirror_rect(SIDE_FACE_LIGHT)
    body.fill(mx, my, mw, mh, SKIN)
    rx, ry, rw, rh = mirror_rect(SIDE_REAR_HAIR)
    body.fill(rx, ry, rw, rh, HI)
    gear = Draw(GEAR)
    gx, gy, gw, gh = mirror_rect(SIDE_COAT_LIGHT)
    gear.fill(gx, gy, gw, gh, COAT_S)
    sx, sy, sw, sh = mirror_rect(SIDE_COAT_SHADE)
    gear.fill(sx, sy, sw, sh, HI)
    return body, gear


def load_project(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def frame_buffers(frame: dict) -> list[list[int]]:
    layers = frame.get("layers") or []
    if layers:
        return [layer["pixels"] for layer in layers]
    return [frame["pixels"]]


def buffers(path: Path) -> list[list[list[int]]]:
    data = load_project(path)
    return [frame_buffers(frame) for frame in data["frames"]]


def composite(layers: list[list[int]]) -> list[int]:
    image = list(layers[0])
    for layer in layers[1:]:
        for index, value in enumerate(layer):
            if value:
                image[index] = value
    return image


def opaque_bottom(pixels: list[int]) -> int | None:
    bottom = None
    for index, value in enumerate(pixels):
        if value:
            bottom = index // WIDTH
    return bottom


def row_slice(pixels: list[int], x0: int, x1: int) -> tuple[int, ...]:
    return tuple(pixels[SOLE * WIDTH + x] for x in range(x0, x1))


def first_double_ink(layers: list[list[int]]) -> tuple[int, int] | None:
    if len(layers) < 2:
        return None
    for index, (body, gear) in enumerate(zip(layers[0], layers[1])):
        if body and gear:
            return (index % WIDTH, index // WIDTH)
    return None


def assert_coat_frames(path: Path) -> None:
    frames = buffers(path)
    if len(frames) != 16:
        raise SystemExit(f"expected 16 frames, got {len(frames)}")
    front = [composite(frames[index]) for index in range(4)]
    left = [composite(frames[index]) for index in range(4, 8)]
    back = [composite(frames[index]) for index in range(12, 16)]
    for index, layers in enumerate(frames):
        image = composite(layers)
        bottom = opaque_bottom(image)
        if bottom != SOLE:
            raise SystemExit(f"frame {index} opaque bottom is y={bottom}, expected {SOLE}")
        gear_bottom = opaque_bottom(layers[1])
        if gear_bottom is None or gear_bottom > 48:
            raise SystemExit(
                f"frame {index} gear reaches y={gear_bottom}; the sole belongs to the body"
            )
        overlap = first_double_ink(layers)
        if overlap is not None:
            raise SystemExit(f"frame {index} body and gear both ink {overlap}")
    left_sole = (D_L_BOOT[0], D_L_BOOT[0] + D_L_BOOT[2])
    right_sole = (D_R_BOOT[0], D_R_BOOT[0] + D_R_BOOT[2])
    idle_left = row_slice(front[0], *left_sole)
    idle_right = row_slice(front[0], *right_sole)
    if not any(idle_left) or not any(idle_right):
        raise SystemExit("front idle is missing a boot on the sole row")
    if any(row_slice(front[1], *left_sole)) or row_slice(front[1], *right_sole) != idle_right:
        raise SystemExit("front walk A slid the planted foot or left the lifted sole down")
    if row_slice(front[2], *left_sole) != idle_left or row_slice(front[2], *right_sole) != idle_right:
        raise SystemExit("front walk B moved a sole")
    if row_slice(front[3], *left_sole) != idle_left or any(row_slice(front[3], *right_sole)):
        raise SystemExit("front walk C slid the planted foot or left the lifted sole down")
    front_side = (S_F_BOOT[0], S_F_BOOT[0] + S_F_BOOT[2])
    back_side = (S_B_BOOT[0], S_B_BOOT[0] + S_B_BOOT[2])
    side_front = row_slice(left[0], *front_side)
    side_back = row_slice(left[0], *back_side)
    if not any(side_front) or not any(side_back):
        raise SystemExit("left idle is missing a boot on the sole row")
    if any(row_slice(left[1], *front_side)) or row_slice(left[1], *back_side) != side_back:
        raise SystemExit("left walk A slid the planted foot")
    if row_slice(left[2], *front_side) != side_front or row_slice(left[2], *back_side) != side_back:
        raise SystemExit("left walk B moved a sole")
    if row_slice(left[3], *front_side) != side_front or any(row_slice(left[3], *back_side)):
        raise SystemExit("left walk C slid the planted foot")
    for index in range(4):
        if row_slice(back[index], *left_sole) != row_slice(front[index], *left_sole):
            raise SystemExit(f"back frame {12 + index} changed the sole")
        if row_slice(back[index], *right_sole) != row_slice(front[index], *right_sole):
            raise SystemExit(f"back frame {12 + index} changed the sole")


def scene(cli: Cli) -> dict:
    proc = cli.run(["scene", "--project", str(cli.project)])
    return json.loads(proc.stdout)["result"]


def palette_hash(binary: str, project: Path) -> str:
    return scene(Cli(binary, project))["paletteHash"]


def write_rules(path: Path, body: dict) -> None:
    path.write_text(json.dumps(body, indent=2) + "\n", encoding="utf-8")


def character_rules() -> dict:
    return {
        "format": "pixelforge-atlas-rules",
        "version": 1,
        "mode": "strict",
        "sizes": [64],
        "requireTags": CHARACTER_TAGS,
        "fourDirection": "on",
        "tagPolicy": "required",
        "pivot": PIVOT,
    }


def prop_rules() -> dict:
    return {
        "format": "pixelforge-atlas-rules",
        "version": 1,
        "mode": "strict",
        "sizes": [64],
        "requireTags": PROP_TAGS,
        "fourDirection": "off",
        "tagPolicy": "optional",
        "pivot": PIVOT,
    }


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


def tag_walk(cli: Cli) -> None:
    ops = []
    facings = ("down", "left", "right", "up")
    for index in range(16):
        delay = 200 if index % 4 == 0 else 120
        ops.append(
            {"tool": "setFrameDelay", "params": {"index": index, "delayMs": delay}}
        )
    for row, facing in enumerate(facings):
        start = row * 4
        ops.append(
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
        ops.append(
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
    cli.call(ops)


def build_coat(binary: str, project: Path) -> Cli:
    cli = Cli(binary, project)
    new_project(cli)
    apply_palette(cli)
    cli.call(
        [
            {"tool": "renameLayer", "params": {"name": "body"}},
            {"tool": "addLayer", "params": {"name": "gear"}},
        ]
    )
    apply(cli, 0, front_body(), front_coat())
    cli.call([{"tool": "duplicateFrame", "params": {"index": 0}} for _ in range(15)])
    apply(cli, 1, *front_walk_a())
    apply(cli, 2, *front_walk_b())
    apply(cli, 3, *front_walk_c())
    apply(cli, 4, left_body(), left_coat())
    for dst in (5, 6, 7):
        copy_layers(cli, 4, dst, (BODY, GEAR))
    apply(cli, 5, *left_walk_a())
    apply(cli, 6, *left_walk_b())
    apply(cli, 7, *left_walk_c())
    for src, dst in ((4, 8), (5, 9), (6, 10), (7, 11)):
        copy_layers(cli, src, dst, (BODY, GEAR))
        hflip_layers(cli, dst, (BODY, GEAR))
        apply(cli, dst, *right_light())
    for src, dst in ((0, 12), (1, 13), (2, 14), (3, 15)):
        copy_layers(cli, src, dst, (BODY, GEAR))
        apply(cli, dst, *to_back())
    tag_walk(cli)
    assert_coat_frames(project)
    return cli


def body_snapshot(path: Path) -> list[tuple[int, ...]]:
    return [tuple(layers[0]) for layers in buffers(path)]


def gear_changed(before: list[list[int]], after: list[list[int]]) -> int:
    changed = 0
    for left, right in zip(before, after):
        changed += sum(1 for a, b in zip(left, right) if a != b)
    return changed


def rect_clear(body: list[int], rect: tuple[int, int, int, int]) -> bool:
    x, y, w, h = rect
    for yy in range(y, y + h):
        row = yy * WIDTH
        for xx in range(x, x + w):
            if body[row + xx]:
                return False
    return True


def hood_for(frame: int) -> tuple[tuple[int, int, int, int], ...]:
    if frame < 4:
        return FRONT_HOOD
    if frame < 8:
        return SIDE_HOOD
    if frame < 12:
        return tuple(mirror_rect(rect) for rect in SIDE_HOOD)
    return BACK_HOOD


def lamp_for(frame: int) -> tuple[int, int, int, int]:
    if frame < 4 or frame >= 12:
        return FRONT_LAMP
    if frame < 8:
        return SIDE_LAMP
    return mirror_rect(SIDE_LAMP)


def regear_cloak(binary: str, project: Path) -> dict:
    """Lock the palette, refuse a new RGB, then recolor gear in place.

    Body pixels are not redrawn. Cloak and cloak-shade indices already exist,
    so the lock does not have to be lifted to change clothes.
    """
    cli = Cli(binary, project)
    before_scene = scene(cli)
    before_hash = before_scene["paletteHash"]
    before_body = body_snapshot(project)
    before_gear = [layers[1] for layers in buffers(project)]
    cli.call([{"tool": "setPaletteLocked", "params": {"locked": True}}])
    refused = cli.run(
        [
            "palette",
            "add",
            "--project",
            str(project),
            "--r",
            "1",
            "--g",
            "2",
            "--b",
            "3",
        ],
        allow_fail=True,
    )
    if refused.returncode != 1:
        raise SystemExit(
            f"locked palette add should exit 1, got {refused.returncode}\n{refused.stdout}"
        )
    error = json.loads(refused.stdout)
    code = error.get("error", {}).get("code")
    if code != "PaletteLocked":
        raise SystemExit(f"expected PaletteLocked, got {refused.stdout}")
    locked_hash = scene(cli)["paletteHash"]
    if locked_hash != before_hash:
        raise SystemExit("palette hash changed while refusing a new color")

    ops: list[dict] = []
    skipped: list[dict] = []
    frames = buffers(project)
    for frame in range(16):
        ops.append({"tool": "setActiveFrame", "params": {"index": frame}})
        for src, dst in (
            (COAT, CLOAK),
            (COAT_S, CLOAK_S),
            (LANTERN, CLOAK),
            (GLOW, CLOAK),
        ):
            ops.append(
                {
                    "tool": "replaceIndex",
                    "params": {
                        "fromIndex": src,
                        "toIndex": dst,
                        "layer": GEAR,
                    },
                }
            )
        lamp = lamp_for(frame)
        ops.append(
            {
                "tool": "fillRect",
                "params": {
                    "x": lamp[0],
                    "y": lamp[1],
                    "width": lamp[2],
                    "height": lamp[3],
                    "colorIndex": CLOAK,
                    "layer": GEAR,
                },
            }
        )
        for rect in hood_for(frame):
            if rect_clear(frames[frame][0], rect):
                ops.append(
                    {
                        "tool": "fillRect",
                        "params": {
                            "x": rect[0],
                            "y": rect[1],
                            "width": rect[2],
                            "height": rect[3],
                            "colorIndex": CLOAK,
                            "layer": GEAR,
                        },
                    }
                )
            else:
                skipped.append({"frame": frame, "rect": list(rect)})
    cli.call(ops)
    after = scene(cli)
    after_body = body_snapshot(project)
    after_frames = buffers(project)
    changed = gear_changed(before_gear, [layers[1] for layers in after_frames])
    if after_body != before_body:
        raise SystemExit("outfit swap changed body-layer pixels")
    if after["paletteHash"] != before_hash:
        raise SystemExit("outfit swap changed the palette hash")
    if not after["paletteLocked"]:
        raise SystemExit("palette did not stay locked")
    if changed == 0:
        raise SystemExit("outfit swap did not change the gear layer")
    for index, layers in enumerate(after_frames):
        image = composite(layers)
        bottom = opaque_bottom(image)
        if bottom != SOLE:
            raise SystemExit(f"cloak frame {index} opaque bottom is y={bottom}")
        overlap = first_double_ink(layers)
        if overlap is not None:
            raise SystemExit(f"cloak frame {index} double ink at {overlap}")
    return {
        "format": "pixelforge-case-record",
        "case": "02-outfit-change",
        "paletteHashBefore": before_hash,
        "paletteHashAfter": after["paletteHash"],
        "paletteLocked": True,
        "lockRefusal": {"tool": "addPaletteColor", "code": code},
        "bodyLayerUnchanged": True,
        "framesCompared": 16,
        "gearCellsChanged": changed,
        "hoodRectsSkipped": skipped,
        "reservedIndices": {"cloak": CLOAK, "cloakShade": CLOAK_S},
    }


def satchel(draw: Draw) -> None:
    draw.fill(22, 18, 20, 8, OL)
    draw.fill(23, 19, 18, 6, COAT)
    draw.fill(28, 8, 8, 12, OL)
    draw.fill(29, 9, 6, 10, ROPE)
    draw.fill(18, 26, 28, 31, OL)
    draw.fill(19, 27, 26, 28, BOOT)
    draw.fill(19, 27, 8, 6, HI)
    draw.fill(30, 34, 8, 8, COAT)
    draw.fill(32, 36, 4, 4, LANTERN)


def lantern_prop(draw: Draw) -> None:
    draw.erase(0, 0, WIDTH, HEIGHT)
    draw.fill(30, 8, 4, 10, ROPE)
    draw.fill(22, 16, 20, 22, OL)
    draw.fill(24, 18, 16, 18, LANTERN)
    draw.fill(26, 20, 12, 12, GLOW)
    draw.fill(28, 22, 2, 6, OL)
    draw.fill(20, 38, 24, 19, OL)
    draw.fill(21, 39, 22, 16, COAT_S)
    draw.fill(24, 48, 16, 3, COAT)


def crate(draw: Draw) -> None:
    draw.erase(0, 0, WIDTH, HEIGHT)
    draw.fill(10, 24, 44, 33, OL)
    draw.fill(11, 25, 42, 30, WOOD)
    draw.fill(11, 25, 42, 3, HI)
    draw.fill(11, 50, 42, 5, WOOD_D)
    draw.fill(28, 25, 4, 30, WOOD_D)
    draw.fill(18, 36, 12, 12, COAT)
    draw.fill(21, 39, 6, 6, LANTERN)
    for x, y in ((14, 28), (48, 28), (14, 50), (48, 50)):
        draw.fill(x, y, 2, 2, OL)


def buoy(draw: Draw) -> None:
    draw.erase(0, 0, WIDTH, HEIGHT)
    draw.fill(26, 12, 12, 45, OL)
    draw.fill(27, 13, 10, 42, RED)
    draw.fill(27, 20, 10, 6, HI)
    draw.fill(27, 32, 10, 6, HI)
    draw.fill(27, 44, 10, 6, COAT)
    draw.fill(16, 28, 10, 3, ROPE)
    draw.fill(38, 28, 10, 3, ROPE)


def build_props(binary: str, project: Path) -> Cli:
    cli = Cli(binary, project)
    new_project(cli)
    apply_palette(cli)
    makers = (satchel, lantern_prop, crate, buoy)
    bag = Draw(BODY)
    satchel(bag)
    apply(cli, 0, bag)
    cli.call([{"tool": "duplicateFrame", "params": {"index": 0}} for _ in range(3)])
    for index, maker in enumerate(makers[1:], start=1):
        draw = Draw(BODY)
        maker(draw)
        apply(cli, index, draw)
    ops = []
    for index, name in enumerate(PROP_TAGS):
        ops.append(
            {"tool": "setFrameDelay", "params": {"index": index, "delayMs": 100}}
        )
        ops.append(
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
    cli.call(ops)
    for index, layers in enumerate(buffers(project)):
        bottom = opaque_bottom(layers[0])
        if bottom != SOLE:
            raise SystemExit(f"prop {index} opaque bottom is y={bottom}, expected {SOLE}")
    return cli


def export_character(binary: str, project: Path, png: Path, atlas: Path, rules_path: Path) -> None:
    cli = Cli(binary, project)
    export_sheet(cli, png, atlas, 4)
    write_rules(rules_path, character_rules())
    validate(binary, atlas, rules_path)


def export_props(binary: str, project: Path, png: Path, atlas: Path, rules_path: Path) -> None:
    cli = Cli(binary, project)
    export_sheet(cli, png, atlas, 4)
    write_rules(rules_path, prop_rules())
    validate(binary, atlas, rules_path)
