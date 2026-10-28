#!/usr/bin/env python3
"""Rebuild the #81 four-direction courier and its strict atlas.

Pixels come from courier_kit.py through the pixelforge CLI. There is no
image-generation call. From a source checkout, build the CLI first or put a
prebuilt pixelforge on PATH (or set PIXELFORGE).
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import courier_kit as kit


def main() -> None:
    binary = kit.find_binary()
    root = Path(__file__).resolve().parent
    project = root / "courier.json"
    kit.build_coat(binary, project)
    kit.export_character(
        binary,
        project,
        root / "courier-sheet.png",
        root / "courier-sheet.json",
        root / "courier-rules.json",
    )
    print(f"wrote {project}")


if __name__ == "__main__":
    main()
