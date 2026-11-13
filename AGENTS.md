# PixelForge agents

Edit pixels only through the `pixelforge` CLI (`crates/pixelforge-cli`) and the kernel Tool API.

Product skills live in `skills/<name>/SKILL.md`. Read `pixelforge` before any other PixelForge skill, then the skill for the task. A packaged `pixelforge` binary lists and reads the same bundled skills with `pixelforge skills list` and `pixelforge skills read <name>` (optional path, or `--json`). Use those commands when driving the installed CLI so you do not depend on the git checkout. In a checkout, open the files from this list. This repository does not copy product skills into `.cursor/skills/`, `.agents/skills/`, or `.claude/skills/`. The index and the command reference are in `docs/agent-skill.md`.

| Skill | When |
| --- | --- |
| `pixelforge` | 用 pixelforge, 改工程, CLI, palette, parts, anchors, transforms, keyframe curves, constraints, bake, export, `preview --compare`, validate-atlas, tileset, 4-neighbor autotile, tilemap, Tiled `.tmj`, line / ellipse / polygon / flood fill / selection wand, `direction fill`, `project init` / `show` / `validate`, `asset add` / `list` / `show` / `graph` / `dependents` / `validate`, `character matrix` / `character check` (pixelforge-manifest, not a canvas). Index-color invariants, exit codes, `inspect` / `tools` / `call`. Shortest edit path: duplicate a pose and edit the diff, merge fillRects, count each request in a batched call. |
| `pixelforge-draw` | 从零画角色, 立绘, 剪影. Silhouette, then flats, then details. `preview --scale` after each stage. |
| `pixelforge-sprite-4dir` | 四向走 + 闲. Rows are facings, columns are idle plus walk. `export sheet --cols`, tags named `idle_down` / `walk_down`, `--atlas` JSON, `diff` for foot slide, fix highlights after `hflip` or `direction fill --mode mirror`. `copy` is a placeholder, not a gait. After export, run `validate-atlas` before game import. Worked #18 pack: [pixelforge-demos](https://github.com/starcloud530/pixelforge-demos) (demo-zombie / packs when published). Duplicate a column and edit the diff. |
| `pixelforge-layers` | body / gear / limbs. Composite preview, draw with `--layer`, `preview --compare`, `layer flatten`. |
| `pixelforge-critique` | Read-only review. List defects. Do not answer with praise alone. Also flag mergeable fillRects and poses redrawn instead of duplicated. |
| `pixel-art-craft` | 学像素画, 走路循环, 机位, 运动轨迹. Public tutorial links on walk, camera/projection, and motion paths. Not a CLI recipe. When painting in a PixelForge project, follow `pixelforge` invariants, then `pixelforge-draw` or `pixelforge-sprite-4dir`. |

Skills add a look-then-revise loop. They do not replace a pixel artist's judgment.

The in-app AI panel is a one-turn demo. Do not build an embedded multi-turn chat (#14). Do not add an MCP server in this tree; a later wrapper would only call this CLI. Cells store palette indices. A frame can hold several layers of those indices. Draw the active layer; `preview` and `export` show the composite. Named parts and anchors are rig metadata on that grid, not a second pixel buffer and not a bone UI. Do not invent RGB past the kernel.
