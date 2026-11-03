#!/usr/bin/env python3
"""Bake a two-frame soldier step with both feet planted.

PixelForge has no gait generator and no physics engine. This file is ordinary
Python. It drives the `pixelforge` CLI: draw a rest pose, duplicate it, store
hip translation and a shin translation that would slide the sole, then pin
each sole with a short IK chain and footprint spans. `bake` writes ordinary
pixel frames. A planted span keeps the sole cell. A later span is an explicit
step. A target the chain cannot reach is `IkUnreachable` and writes nothing.

Usage:
    PIXELFORGE=/path/to/pixelforge python3 examples/soldier-walk.py soldier.json
"""

import json
import os
import shutil
import subprocess
import sys

WIDTH = 24


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: soldier-walk.py PROJECT.json")
    binary = os.environ.get("PIXELFORGE") or shutil.which("pixelforge")
    if not binary:
        raise SystemExit("set PIXELFORGE to the pixelforge binary")
    project = sys.argv[1]
    cli = Cli(binary, project)

    cli.run(["new", "--width", "24", "--height", "24", "--output", project])
    cli.run(["layer", "rename", "--name", "thigh-l"])
    cli.run(["layer", "add", "--name", "shin-l"])
    cli.run(["layer", "add", "--name", "thigh-r"])
    cli.run(["layer", "add", "--name", "shin-r"])
    cli.run(["layer", "add", "--name", "body"])
    cli.run(["paint", "--layer", "0", "--x", "8", "--y", "8", "--color-index", "1"])
    cli.run(["paint", "--layer", "1", "--x", "12", "--y", "8", "--color-index", "2"])
    cli.run(["paint", "--layer", "1", "--x", "12", "--y", "12", "--color-index", "3"])
    cli.run(["paint", "--layer", "2", "--x", "8", "--y", "8", "--color-index", "4"])
    cli.run(["paint", "--layer", "3", "--x", "4", "--y", "8", "--color-index", "5"])
    cli.run(["paint", "--layer", "3", "--x", "4", "--y", "12", "--color-index", "6"])
    cli.run(["paint", "--layer", "4", "--x", "8", "--y", "6", "--color-index", "7"])

    cli.run(["part", "add", "--name", "thigh-l", "--layer", "0", "--x", "8", "--y", "8"])
    cli.run(["part", "add", "--name", "shin-l", "--layer", "1", "--x", "12", "--y", "8"])
    cli.run(["part", "anchor", "set", "--part", "shin-l", "--name", "sole", "--x", "0", "--y", "4"])
    cli.run(["part", "add", "--name", "thigh-r", "--layer", "2", "--x", "8", "--y", "8"])
    cli.run(["part", "add", "--name", "shin-r", "--layer", "3", "--x", "4", "--y", "8"])
    cli.run(["part", "anchor", "set", "--part", "shin-r", "--name", "sole", "--x", "0", "--y", "4"])

    cli.run(["frame", "duplicate", "--index", "0"])
    cli.run(["frame", "duplicate", "--index", "0"])
    for part in ("thigh-l", "thigh-r"):
        cli.run(
            [
                "curve",
                "set",
                "--part",
                part,
                "--channel",
                "translate-x",
                "--frame",
                "0",
                "--value",
                "0",
            ]
        )
        cli.run(
            [
                "curve",
                "set",
                "--part",
                part,
                "--channel",
                "translate-x",
                "--frame",
                "2",
                "--value",
                "2",
            ]
        )
    # This curve would slide the left sole. The footprint span cancels it.
    cli.run(
        [
            "curve",
            "set",
            "--part",
            "shin-l",
            "--channel",
            "translate-x",
            "--frame",
            "0",
            "--value",
            "0",
        ]
    )
    cli.run(
        [
            "curve",
            "set",
            "--part",
            "shin-l",
            "--channel",
            "translate-x",
            "--frame",
            "2",
            "--value",
            "4",
        ]
    )

    cli.run(
        [
            "constraint",
            "add",
            "--name",
            "left-leg",
            "--kind",
            "ik",
            "--part",
            "shin-l",
            "--anchor",
            "sole",
            "--chain",
            "thigh-l",
            "--chain",
            "shin-l",
            "--bend",
            "1",
            "--contact",
            "0:1:12:12",
            "--contact",
            "2:14:10",
        ]
    )
    cli.run(
        [
            "constraint",
            "add",
            "--name",
            "right-leg",
            "--kind",
            "ik",
            "--part",
            "shin-r",
            "--anchor",
            "sole",
            "--chain",
            "thigh-r",
            "--chain",
            "shin-r",
            "--bend",
            "1",
            "--contact",
            "0:1:6:10",
            "--contact",
            "2:4:12",
        ]
    )

    baked = cli.run(["bake", "--from", "0", "--to", "2", "--fps", "12"])
    if baked["wrote"] is not True or baked["delayMs"] != 83:
        raise SystemExit(f"unexpected bake report: {baked}")
    if baked["frames"][0]["rest"] is not True:
        raise SystemExit("source frame was rewritten")

    left = [cli.pixels(frame, 1) for frame in range(3)]
    right = [cli.pixels(frame, 3) for frame in range(3)]
    body = [cli.pixels(frame, 4) for frame in range(3)]
    # Planted span: frames 0 and 1 keep the left sole. Frame 1's shin curve
    # would have moved it to x=14. Frame 2 is an explicit step.
    if left[0][12 * WIDTH + 12] != 3 or left[1][12 * WIDTH + 12] != 3:
        raise SystemExit("left sole slid during the plant")
    if left[1][12 * WIDTH + 14] != 0:
        raise SystemExit("left sole followed the shin curve")
    if left[2][10 * WIDTH + 14] != 3 or left[2][12 * WIDTH + 12] != 0:
        raise SystemExit("left sole missed the stepped contact")
    if right[2][12 * WIDTH + 4] != 6:
        raise SystemExit("right sole slid off the plant on frame 2")
    if body[0][6 * WIDTH + 8] != 7 or body[1][6 * WIDTH + 8] != 7 or body[2][6 * WIDTH + 8] != 7:
        raise SystemExit("body layer was rewritten")

    scene = cli.run(["scene"])
    if scene["frameCount"] != 3 or len(scene["constraints"]) != 2 or len(scene["curves"]) != 3:
        raise SystemExit("bake removed curves or constraints")
    if any(part["name"] == "thigh-l" and (part["x"], part["y"]) != (8, 8) for part in scene["parts"]):
        raise SystemExit("stored hip origin moved")
    cli.run(["tag", "add", "--name", "walk", "--from", "0", "--to", "2"])
    saved = json.loads(open(project, encoding="utf-8").read())
    if saved.get("version") != 1 or "bake" in saved:
        raise SystemExit("project file changed shape")
    print(f"baked {project}: feet planted on frames 0-1, left step on frame 2")


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
