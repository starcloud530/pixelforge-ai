#!/usr/bin/env python3
"""Rebuild the #81 harbor prop sheet on the courier palette.

fourDirection is off. The pivot matches the character sole line. If the
coat project from case 01 is already on disk, this script checks that the
palette hash matches.
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import courier_kit as kit


def main() -> None:
    binary = kit.find_binary()
    root = Path(__file__).resolve().parent
    project = root / "harbor-props.json"
    kit.build_props(binary, project)
    coat = root.parent / "01-four-dir-walk" / "courier.json"
    if coat.exists():
        coat_hash = kit.palette_hash(binary, coat)
        prop_hash = kit.palette_hash(binary, project)
        if coat_hash != prop_hash:
            raise SystemExit(
                f"prop palette {prop_hash} does not match courier {coat_hash}"
            )
        print(f"paletteHash {prop_hash} matches the courier")
    kit.export_props(
        binary,
        project,
        root / "harbor-props-sheet.png",
        root / "harbor-props-sheet.json",
        root / "harbor-props-rules.json",
    )
    print(f"wrote {project}")


if __name__ == "__main__":
    main()
