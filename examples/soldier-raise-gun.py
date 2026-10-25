#!/usr/bin/env python3
"""Raise a soldier's gun with an external script, then bake the pixels.

PixelForge has no script runtime and no animation DSL. This file is ordinary
Python. It drives the `pixelforge` CLI: draw a rest pose, duplicate that frame,
store a rotation curve and follow / attach / pin constraints, then

    pixelforge bake --from 0 --to 11 --fps 12

Bake samples the curve, solves the constraints, and writes ordinary pixel
frames. The source frame stays the rest pose. Curves, constraints, and the
stored part origins stay in the file. A later process can paint, select, and
tag those frames. One bake is one undo step inside the process that runs it.
The project file does not store that stack.

Usage:
    PIXELFORGE=/path/to/pixelforge python3 examples/soldier-raise-gun.py soldier.json
"""

import json
import os
import shutil
import subprocess
import sys

WIDTH = 16


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: soldier-raise-gun.py PROJECT.json")
    binary = os.environ.get("PIXELFORGE") or shutil.which("pixelforge")
    if not binary:
        raise SystemExit("set PIXELFORGE to the pixelforge binary")
    project = sys.argv[1]
    cli = Cli(binary, project)

    cli.run(["new", "--width", "16", "--height", "16", "--output", project])
    cli.run(["layer", "add", "--name", "hand"])
    cli.run(["layer", "add", "--name", "foot"])
    cli.run(["layer", "add", "--name", "body"])
    for layer, x, y, color in (
        (0, 4, 4, 1),
        (0, 2, 4, 2),
        (0, 6, 4, 3),
        (1, 6, 4, 4),
        (2, 3, 11, 5),
        (3, 8, 8, 6),
    ):
        cli.run(
            [
                "paint",
                "--layer",
                str(layer),
                "--x",
                str(x),
                "--y",
                str(y),
                "--color-index",
                str(color),
            ]
        )
    cli.run(["part", "add", "--name", "shoulder", "--x", "2", "--y", "4"])
    cli.run(["part", "anchor", "set", "--part", "shoulder", "--name", "joint", "--x", "0", "--y", "0"])
    cli.run(["part", "add", "--name", "gun", "--layer", "0", "--x", "4", "--y", "4"])
    cli.run(["part", "anchor", "set", "--part", "gun", "--name", "butt", "--x", "-2", "--y", "0"])
    cli.run(["part", "anchor", "set", "--part", "gun", "--name", "grip", "--x", "2", "--y", "0"])
    cli.run(["part", "add", "--name", "hand", "--layer", "1", "--x", "6", "--y", "4"])
    cli.run(["part", "anchor", "set", "--part", "hand", "--name", "palm", "--x", "0", "--y", "0"])
    cli.run(["part", "add", "--name", "foot", "--layer", "2", "--x", "3", "--y", "10"])
    cli.run(["part", "anchor", "set", "--part", "foot", "--name", "sole", "--x", "0", "--y", "1"])
    cli.run(
        [
            "constraint",
            "add",
            "--name",
            "hand-grip",
            "--kind",
            "follow",
            "--part",
            "hand",
            "--anchor",
            "palm",
            "--target",
            "gun",
            "--target-anchor",
            "grip",
        ]
    )
    cli.run(
        [
            "constraint",
            "add",
            "--name",
            "butt-shoulder",
            "--kind",
            "stick",
            "--part",
            "gun",
            "--anchor",
            "butt",
            "--target",
            "shoulder",
            "--target-anchor",
            "joint",
        ]
    )
    cli.run(
        [
            "constraint",
            "add",
            "--name",
            "plant-foot",
            "--kind",
            "pin",
            "--part",
            "foot",
            "--anchor",
            "sole",
            "--x",
            "4",
            "--y",
            "11",
        ]
    )
    # Duplicate the rest pose before the keys. Existing frames keep layers that
    # no part moves (the body). Keys set after this stay on these indices.
    for _ in range(11):
        cli.run(["frame", "duplicate", "--index", "0"])
    cli.run(["curve", "set", "--part", "gun", "--channel", "rotation", "--frame", "0", "--value", "0"])
    cli.run(["curve", "set", "--part", "gun", "--channel", "rotation", "--frame", "11", "--value", "90"])

    baked = cli.run(["bake", "--from", "0", "--to", "11", "--fps", "12"])
    if baked["wrote"] is not True or baked["delayMs"] != 83 or baked["appended"] != 0:
        raise SystemExit(f"unexpected bake report: {baked}")
    if baked["frames"][0]["rest"] is not True or baked["frames"][0]["layers"] != []:
        raise SystemExit("source frame was rewritten")
    gun = baked["frames"][11]["parts"][1]
    if gun["name"] != "gun" or gun["x"] != 2 or gun["y"] != 6 or gun["ink"] != "moved":
        raise SystemExit(f"frame 11 gun pose: {gun}")

    rest = cli.pixels(0, 0)
    raised = cli.pixels(11, 0)
    mid = cli.pixels(6, 0)
    if rest[4 * WIDTH + 4] != 1 or rest[4 * WIDTH + 6] != 3:
        raise SystemExit("frame 0 gun moved")
    if raised[6 * WIDTH + 2] != 1 or raised[4 * WIDTH + 2] != 2 or raised[8 * WIDTH + 2] != 3:
        raise SystemExit(f"frame 11 gun pixels: origin {raised[6 * WIDTH + 2]}")
    if cli.pixels(11, 1)[8 * WIDTH + 2] != 4 or cli.pixels(11, 2)[11 * WIDTH + 4] != 5:
        raise SystemExit("hand or foot did not follow")
    if cli.pixels(11, 3)[8 * WIDTH + 8] != 6 or cli.pixels(0, 3)[8 * WIDTH + 8] != 6:
        raise SystemExit("body layer was rewritten")
    if mid == rest or mid == raised:
        raise SystemExit("frame 6 is not between the rest pose and the raised gun")

    scene = cli.run(["scene"])
    if scene["frameCount"] != 12:
        raise SystemExit(f"frame count {scene['frameCount']}")
    if any(frame["delayMs"] != 83 for frame in scene["frames"]):
        raise SystemExit(f"delays {scene['frames']}")
    stored = next(part for part in scene["parts"] if part["name"] == "gun")
    if stored["x"] != 4 or stored["y"] != 4:
        raise SystemExit(f"stored gun origin moved: {stored}")
    if len(scene["curves"]) != 1 or len(scene["constraints"]) != 3:
        raise SystemExit("bake removed curves or constraints")

    cli.run(["frame", "select", "--index", "11"])
    cli.run(["paint", "--layer", "1", "--x", "2", "--y", "8", "--color-index", "7"])
    cli.run(["tag", "add", "--name", "raise_gun", "--from", "0", "--to", "11"])
    if cli.pixels(11, 1)[8 * WIDTH + 2] != 7:
        raise SystemExit("baked hand cell rejected paint")
    scene = cli.run(["scene"])
    if scene["tags"] != [{"index": 0, "name": "raise_gun", "from": 0, "to": 11, "loop": "forward"}]:
        raise SystemExit(f"tag: {scene['tags']}")

    saved = json.loads(open(project, encoding="utf-8").read())
    if saved.get("version") != 1 or "bake" in saved:
        raise SystemExit("project file changed shape")
    print(f"baked {project}: 12 frames at 83 ms, gun raised, curves kept")


class Cli:
    def __init__(self, binary: str, project: str) -> None:
        self.binary = binary
        self.project = project

    def run(self, args: list[str]) -> dict:
        if args[0] != "new":
            args = self._with_project(args)
        proc = subprocess.run(
            [self.binary, *args],
            check=False,
            capture_output=True,
            text=True,
        )
        if proc.returncode != 0:
            raise SystemExit(f"{args}\n{proc.stderr}\n{proc.stdout}")
        line = proc.stdout.strip().splitlines()[-1]
        body = json.loads(line)
        if body.get("ok") is not True:
            raise SystemExit(body)
        return body["result"]

    def _with_project(self, args: list[str]) -> list[str]:
        for index, arg in enumerate(args):
            if arg.startswith("--"):
                return [*args[:index], "--project", self.project, *args[index:]]
        return [*args, "--project", self.project]

    def pixels(self, frame: int, layer: int) -> list[int]:
        result = self.run(
            [
                "call",
                "--request",
                json.dumps(
                    {
                        "tool": "previewFrame",
                        "params": {"frame": frame, "layer": layer},
                    }
                ),
            ]
        )
        return result["pixels"]


if __name__ == "__main__":
    main()
