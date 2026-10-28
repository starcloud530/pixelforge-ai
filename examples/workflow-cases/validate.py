#!/usr/bin/env python3
"""Re-run validate-atlas --strict on the committed #81 cases.

Exit 0 only when the coat walk, the cloak walk, and the prop sheet have no
errors, the three projects share one palette hash, and the cloak record
still says the body layer and the hash were unchanged. This does not rebuild
pixels. A broken PNG, atlas, rules file, or identity record fails the script.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import courier_kit as kit

ROOT = Path(__file__).resolve().parent


def require_atlas(binary: str, atlas: Path, rules: Path, four_direction: bool) -> None:
    proc = kit.Cli(binary, atlas).run(
        [
            "validate-atlas",
            "--atlas",
            str(atlas),
            "--rules",
            str(rules),
            "--strict",
        ]
    )
    print(proc.stderr.strip())
    body = json.loads(proc.stdout)
    checks = body["result"]["checks"]
    if (
        not body.get("ok")
        or body["result"]["errorCount"] != 0
        or checks["mode"] != "strict"
        or checks["pivot"] != "checked"
        or checks["fourDirection"] is not four_direction
    ):
        raise SystemExit(proc.stdout or proc.stderr)


def main() -> None:
    binary = kit.find_binary()
    require_atlas(
        binary,
        ROOT / "01-four-dir-walk" / "courier-sheet.json",
        ROOT / "01-four-dir-walk" / "courier-rules.json",
        True,
    )
    require_atlas(
        binary,
        ROOT / "02-outfit-change" / "courier-cloak-sheet.json",
        ROOT / "02-outfit-change" / "courier-cloak-rules.json",
        True,
    )
    require_atlas(
        binary,
        ROOT / "03-style-pack" / "harbor-props-sheet.json",
        ROOT / "03-style-pack" / "harbor-props-rules.json",
        False,
    )

    coat = kit.scene(kit.Cli(binary, ROOT / "01-four-dir-walk" / "courier.json"))
    cloak = kit.scene(
        kit.Cli(binary, ROOT / "02-outfit-change" / "courier-cloak.json")
    )
    props = kit.scene(kit.Cli(binary, ROOT / "03-style-pack" / "harbor-props.json"))
    if not (coat["paletteHash"] == cloak["paletteHash"] == props["paletteHash"]):
        raise SystemExit(
            "palette hash diverged: "
            f"coat {coat['paletteHash']} cloak {cloak['paletteHash']} props {props['paletteHash']}"
        )
    if coat["paletteLocked"]:
        raise SystemExit("coat project should stay unlocked; case 02 is the lock")
    if not cloak["paletteLocked"]:
        raise SystemExit("cloak project should stay palette-locked")
    record = json.loads(
        (ROOT / "02-outfit-change" / "identity.json").read_text(encoding="utf-8")
    )
    if (
        record.get("bodyLayerUnchanged") is not True
        or record.get("paletteHashBefore") != cloak["paletteHash"]
        or record.get("paletteHashAfter") != cloak["paletteHash"]
        or record.get("lockRefusal", {}).get("code") != "PaletteLocked"
        or record.get("gearCellsChanged", 0) <= 0
    ):
        raise SystemExit(f"identity record does not match the cloak project: {record}")
    print(
        "workflow-cases: ok "
        f"paletteHash={coat['paletteHash']} gearCellsChanged={record['gearCellsChanged']}"
    )


if __name__ == "__main__":
    sys.exit(main())
