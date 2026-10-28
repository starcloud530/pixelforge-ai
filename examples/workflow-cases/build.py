#!/usr/bin/env python3
"""Rebuild all three #81 cases. Each case directory also has its own build.py."""

from __future__ import annotations

import runpy
from pathlib import Path

ROOT = Path(__file__).resolve().parent


def main() -> None:
    for name in (
        "01-four-dir-walk/build.py",
        "02-outfit-change/build.py",
        "03-style-pack/build.py",
    ):
        print(f"== {name}")
        runpy.run_path(str(ROOT / name), run_name="__main__")


if __name__ == "__main__":
    main()
