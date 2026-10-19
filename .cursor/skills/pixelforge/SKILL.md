---
name: pixelforge
description: >-
  Edit PixelForge indexed-pixel projects with the pixelforge CLI and the kernel
  Tool API. Use when creating or changing a pixelforge-project, choosing a
  canvas size, painting or erasing cells, filling a rectangle, previewing a
  frame, editing the palette, adding or duplicating frames, setting delays,
  undoing, or exporting a PNG sequence or sprite sheet.
---

# PixelForge

用 `pixelforge` CLI 改索引像素。格子里只存调色板索引。不要自己发明 RGB，不要在应用里做多轮聊天，也不要加 MCP 服务器。

Edit indexed pixels only through the `pixelforge` CLI. The Rust kernel owns the pixels. You do not invent RGB, you do not build an in-app multi-turn chat, and you do not add an MCP server.

## When to use

Use this skill when you create or change a `pixelforge-project` file, its frames, its palette, or a PNG export.

The product path is: external agent → `pixelforge` CLI → `Document::call_tool`. The one-turn panel in the web app is an optional demo. An embedded multi-turn conversation (#14) is not the product. MCP is out of scope; a later wrapper would only call this CLI.

## Install and run

From the repository root:

```bash
cargo install --path crates/pixelforge-cli
pixelforge --help
```

Without installing:

```bash
cargo run -p pixelforge-cli -- --help
```

A new file needs Rust stable (`cargo` 1.83 or newer). There is no Python service.

## Invariants

- Cells store palette indices. Index 0 is transparent. `paint` and `fillRect` write an existing index from 1 up, never raw RGB.
- A blank project already has the studio palette (indices 1 through 8). Paint those. To introduce a color, `palette add` or `palette set`, then paint the index the tool returns.
- `palette lock` still allows `paint` and `fillRect` of an existing index. It refuses recolor, append, and import.
- Do not hand-edit the project JSON to plant a color. A bad index fails to load. Do not write a diffusion image back into the project.
- One process is one document. Several `--request` flags on one `pixelforge call` share the undo stack. The project file does not store that stack. The next process starts empty, same as opening the file in the editor. `pixelforge undo` on a freshly opened file returns `NothingToUndo`.
- A refused call does not change the document. In a batch, earlier calls are saved and later calls are not run.
- Export resolves indices through the palette inside the kernel. Do not recolor the PNG yourself. Onion skin is a view; there is no tool that writes it into pixels.
- Stdout is compact JSON, one response per line. Stderr is a short note (`wrote hero.json`), plus the ASCII frame when you pass `--preview` or run `preview`. Parse stdout for `ok` / `error.code`. Read the stderr grid (or `preview`'s `result.rows`) when you need to see the picture.

Exit codes: `0` every tool returned `ok: true`. `1` a tool returned `ok: false` (stdout still has the error object). `2` the command did not run the tool (missing file, bad project, bad request file). Clap usage errors also exit `2` and print help to stderr, without a JSON line.

## Canvas size

Pick the size in `pixelforge new` before drawing. A canvas that is too small cannot be resized, and a full body crammed into a sprite slot looks like a diagram.

| Subject | Size (width × height) |
| --- | --- |
| Top-down minion, icon, tile | 16–32 |
| Full-body game sprite | about 32–64 |
| Standing 立绘 / transformation story | at least 64×96; prefer **64×128** or **96×128** |
| Bust or face-focused portrait | at least 64×64 |

A **48×64 full body looks schematic**: the head is a few pixels, the pose cannot read, and later frames have nowhere to put damage, cloth, or a second silhouette. Do not start a standing figure at 16×16 or 32×32.

## Observe, then revise

Do not paint every frame blind. After each major stage — silhouette, flats, details, pose — look at the picture and fix it before the next stage.

1. Draw that stage with `fill-rect` and a short `call` batch.
2. `pixelforge preview --project …`, or `export png` and open the PNG.
3. Read the ASCII grid (stderr, or `result.rows`) or the PNG.
4. Fix the shape, then go on. Repeat for the next stage.

`preview` and `--preview` print one glyph per cell: `.` is transparent (index 0), `1`–`9` are those indices, `A`–`Z` are 10–35, `a`–`z` are 36–61, and `*` is index 62 or above (the legend on stderr names those indices). Stdout stays JSON.

Prefer `fillRect` for a solid region, and one `pixelforge call` with several `--request` flags, over thousands of lone `paint` calls. Wrap several rects in `beginGesture` / `endGesture` when they should undo as one step. A single `fillRect` is already one undo step.

## Recipes

### Blank project

```bash
pixelforge new --width 16 --height 16 --output hero.json
```

Stdout is the Tool API `inspect` response. The file is version 1 `pixelforge-project` JSON. `new` refuses to replace a file that already exists.

### Paint, fill, and erase

```bash
pixelforge fill-rect --project hero.json --x 4 --y 8 --width 16 --height 40 --color-index 3 --preview
pixelforge paint --project hero.json --x 1 --y 2 --color-index 3
pixelforge erase --project hero.json --x 1 --y 2
pixelforge preview --project hero.json
```

`fill-rect` writes one palette index into an axis-aligned rectangle on the active frame. `x`,`y` is the top-left cell. `width` and `height` are at least 1, and the whole rectangle must sit inside the canvas; if any cell would fall outside, nothing is written. A locked palette still allows the fill. The tool name inside `call` is `fillRect`.

```bash
pixelforge call --project hero.json \
  --request '{"tool":"beginGesture"}' \
  --request '{"tool":"fillRect","params":{"x":8,"y":4,"width":16,"height":20,"colorIndex":1}}' \
  --request '{"tool":"fillRect","params":{"x":10,"y":24,"width":12,"height":28,"colorIndex":2}}' \
  --request '{"tool":"endGesture"}' \
  --preview
```

### Standing 立绘

For a human-to-zombie standing portrait, `new --width 64 --height 128` (or 96×128). Block the head, torso, and legs with `fill-rect`, `preview`, then correct the silhouette. Add cloth and skin as more rects, `preview` again, then `paint` only the details (eyes, teeth, tears in cloth). `duplicateFrame` for the zombie pose and repeat the same look-then-fix loop. `export png` when a stage should be read as a real image. Do not invent RGB, and do not emit a `paint` per pixel for a flat area.

### A stroke that undoes as one step

Put the gesture and the undo in the **same** process:

```bash
pixelforge call --project hero.json \
  --request '{"tool":"beginGesture"}' \
  --request '{"tool":"paint","params":{"x":1,"y":2,"colorIndex":3}}' \
  --request '{"tool":"paint","params":{"x":2,"y":2,"colorIndex":3}}' \
  --request '{"tool":"endGesture"}'
```

To revert that stroke before the process exits, add `--request '{"tool":"undo"}'` to the same command. Copy the file first (`cp hero.json hero.json.bak`) if you may need the previous pixels after the process exits.

### Walk frames

```bash
pixelforge call --project hero.json \
  --request '{"tool":"duplicateFrame","params":{"index":0}}' \
  --request '{"tool":"setActiveFrame","params":{"index":1}}' \
  --request '{"tool":"setFrameDelay","params":{"index":1,"delayMs":120}}' \
  --request '{"tool":"paint","params":{"x":0,"y":0,"colorIndex":1}}'
pixelforge export sheet --project hero.json --output hero-sheet.png
pixelforge export png --project hero.json --out-dir ./hero-frames
```

`frame duplicate` copies pixels and delay to the next slot. `frame add` appends a blank frame at 100 ms and does not change the active frame. `frame delay` sets one frame. `export png` writes `pixelforge-000.png` onward. `export sheet` uses the kernel grid (`columns = ceil(sqrt(frame count))`).

The same edits as subcommands:

```bash
pixelforge frame duplicate --project hero.json --index 0
pixelforge frame select --project hero.json --index 1
pixelforge frame delay --project hero.json --index 1 --delay-ms 120
```

### Palette

```bash
pixelforge palette add --project hero.json --r 20 --g 30 --b 40
pixelforge palette set --project hero.json --index 1 --r 10 --g 20 --b 30
pixelforge palette lock --project hero.json
pixelforge palette import --project hero.json --mode merge --file colors.gpl
```

`palette add` prints the new index. Paint that index. Lock before importing if the palette must stay fixed; import is then refused.

### Inspect and the raw tool

```bash
pixelforge inspect --project hero.json
pixelforge tools
pixelforge call --project hero.json --request '{"tool":"inspect"}'
```

`call` prints the Tool API response unchanged. `exportPngSequence` and `exportSpriteSheet` on `call` return base64 and do not write files. The `export` subcommand writes files and returns paths instead of base64.

A request file may be one JSON object, a JSON array, or one JSON value per line:

```bash
pixelforge call --project hero.json --request-file edits.json
```

## Catalog

Tool names, params, and `error.code` values: `docs/tool-api.md`. Longer recipes: `docs/agent-skill.md`.
