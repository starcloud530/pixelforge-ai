#!/usr/bin/env python3
"""Rebuild the #81 cloak swap on the same courier body.

The script draws the coat walk, locks the palette, records the refused
palette add, then recolors the gear layer with indices that were already
in the palette. Body-layer pixels are compared before and after.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import courier_kit as kit


def main() -> None:
    binary = kit.find_binary()
    root = Path(__file__).resolve().parent
    project = root / "courier-cloak.json"
    kit.build_coat(binary, project)
    record = kit.regear_cloak(binary, project)
    (root / "identity.json").write_text(
        json.dumps(record, indent=2) + "\n", encoding="utf-8"
    )
    kit.export_character(
        binary,
        project,
        root / "courier-cloak-sheet.png",
        root / "courier-cloak-sheet.json",
        root / "courier-cloak-rules.json",
    )
    print(
        f"wrote {project} gearCellsChanged={record['gearCellsChanged']} "
        f"lockRefusal={record['lockRefusal']['code']}"
    )


if __name__ == "__main__":
    main()
