---
name: pixelforge
description: >-
  Edit PixelForge indexed-pixel projects with the pixelforge CLI and the kernel
  Tool API. Use when creating or changing a pixelforge-project, choosing a
  canvas size, painting or erasing cells, filling or erasing a rectangle,
  copying or moving a rectangle across frames, previewing a frame in color,
  diffing two frames, flipping a frame horizontally, resizing the canvas,
  editing the palette,
  adding or duplicating frames, setting delays, editing layers, undoing, or
  exporting a PNG sequence or sprite sheet.
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

- Cells store palette indices. Index 0 is transparent. `paint` and `fillRect` write an existing index from 1 up, never raw RGB. `eraseRect` writes index 0. `copyRect` and `moveRect` copy the stored indices, including 0.
- Every frame shares one layer stack. Index 0 is the bottom. Drawing writes the active layer, or the layer named by `--layer`, and leaves the other layers alone. `--layer` does not select that layer. `preview`, `diff`, and `export png` / `export sheet` show the composite: index 0 shows the layer underneath, and any other index covers it. Palette alpha is not blended between layers. `preview --layer N` and `diff --layer N` show that layer only. `layer flatten` writes the composite back into one layer named `Layer 1`, which is the old single-layer file. A file that is still that one default layer omits `layers` and `activeLayer`.
- A blank project already has the studio palette (indices 1 through 8). Paint those. To introduce a color, `palette add` or `palette set`, then paint the index the tool returns.
- `palette lock` still allows `paint` and `fillRect` of an existing index. It refuses recolor, append, and import.
- Do not hand-edit the project JSON to plant a color. A bad index fails to load. Do not write a diffusion image back into the project.
- One process is one document. Several `--request` flags on one `pixelforge call` share the undo stack. The project file does not store that stack. The next process starts empty, same as opening the file in the editor. `pixelforge undo` on a freshly opened file returns `NothingToUndo`.
- A refused call does not change the document. In a batch, earlier calls are saved and later calls are not run.
- Export resolves indices through the palette inside the kernel. Do not recolor the PNG yourself. `diff` / `diffFrames` draws an onion view and does not write it into pixels. `flip` moves indices only; it does not recolor highlights.
- Stdout is compact JSON, one response per line. Stderr is a short note (`wrote hero.json`), plus the ASCII frame when you pass `--preview` or run `preview`. Parse stdout for `ok` / `error.code`. Read the stderr grid (or `preview`'s `result.rows`) when you need to see the picture.

Exit codes: `0` every tool returned `ok: true`. `1` a tool returned `ok: false` (stdout still has the error object). `2` the command did not run the tool (missing file, bad project, bad request file). `preview` and `diff` also exit `2` when the kernel refuses, and stdout is still that error object. Clap usage errors also exit `2` and print help to stderr, without a JSON line.

## Canvas size

Pick the size in `pixelforge new` before drawing. A full body crammed into a sprite slot looks like a diagram. If the canvas is too small, `resize` copies every frame into a larger one (centered, or at `--offset-x` / `--offset-y`) instead of redrawing the file. Expanding does not invent detail; it only moves the indices you already stored.

| Subject | Size (width × height) |
| --- | --- |
| Top-down minion, icon, tile | 16–32 |
| Full-body game sprite | about 32–64 |
| Standing 立绘 / transformation story | at least 64×96; prefer **64×128** or **96×128** |
| Bust or face-focused portrait | at least 64×64 |

A **48×64 full body looks schematic**: the head is a few pixels, the pose cannot read, and later frames have nowhere to put damage, cloth, or a second silhouette. Do not start a standing figure at 16×16 or 32×32.

## Observe, then revise

Do not paint every frame blind. After each major stage — silhouette, flats, details, pose — look at the picture and fix it before the next stage.

1. Draw that stage with `fill-rect`, `erase-rect`, `copy-rect`, or `move-rect`, and a short `call` batch.
2. `pixelforge preview --project … --scale 4` (add `--frame N` to read another frame without selecting it). Open `result.path`. Use `diff --frame A --with B --scale 4` when two poses must line up.
3. Read the ASCII grid (stderr, or `result.rows`) for holes and the silhouette. Read the scaled PNG for color: orange cloth, gray-green skin, a metal mask, and whether neighboring ramps stay apart.
4. Fix the shape, then go on. Repeat for the next stage.

`preview` and `--preview` print one glyph per cell: `.` is transparent (index 0), `1`–`9` are those indices, `A`–`Z` are 10–35, `a`–`z` are 36–61, and `*` is index 62 or above (the legend on stderr names those indices). The picture is the composite of every layer unless `--layer N` is set. Stdout stays JSON. `preview --frame N` draws frame N and does not change `activeFrame` or the project file. `preview --layer N` draws that layer and does not select it. `result.frame` is the picture; `result.activeFrame` is still the document's active frame. `preview` also writes a nearest-neighbor color PNG. `--scale K` matches export (`K` ≥ 1, default 1). The path is `result.path` (default `<stem>.preview.png` beside the project, or `--output`). `--preview` on a mutating command stays ASCII on stderr and does not write that PNG.

`export png --scale K` and `export sheet --scale K` nearest-neighbor upscale inside the kernel (`K` is an integer ≥ 1, default 1). Each palette index becomes a `K` by `K` block, then the kernel resolves RGB. Do not scale the PNG with an external tool when you only need to judge color. `export sheet --cols N` sets the column count; omit it and the grid stays `ceil(sqrt(frame count))`.

Prefer `fillRect` for a solid region, `eraseRect` for cutting a silhouette, and `copyRect` / `moveRect` for a pose shift or a prop that moves. One `pixelforge call` with several `--request` flags beats thousands of lone `paint` or `erase` calls. Wrap several rects in `beginGesture` / `endGesture` when they should undo as one step. A single `fillRect`, `eraseRect`, `copyRect`, or `moveRect` is already one undo step.

## Recipes

### Blank project

```bash
pixelforge new --width 16 --height 16 --output hero.json
```

Stdout is the Tool API `inspect` response. The file is version 1 `pixelforge-project` JSON. `new` refuses to replace a file that already exists.

### Paint, fill, and erase

```bash
pixelforge fill-rect --project hero.json --x 4 --y 8 --width 16 --height 40 --color-index 3 --preview
pixelforge erase-rect --project hero.json --x 4 --y 40 --width 16 --height 8
pixelforge copy-rect --project hero.json --src-x 8 --src-y 70 --width 6 --height 10 --dst-x 40 --dst-y 78
pixelforge move-rect --project hero.json --src-x 40 --src-y 78 --width 6 --height 10 --dst-x 44 --dst-y 80
pixelforge paint --project hero.json --x 1 --y 2 --color-index 3
pixelforge erase --project hero.json --x 1 --y 2
pixelforge preview --project hero.json
pixelforge preview --project hero.json --frame 1
```

`fill-rect` writes one palette index into an axis-aligned rectangle on the active frame. `x`,`y` is the top-left cell. `width` and `height` are at least 1, and the whole rectangle must sit inside the canvas; if any cell would fall outside, nothing is written. A locked palette still allows the fill. The tool name inside `call` is `fillRect`.

`erase-rect` clears that same kind of rectangle to index 0 (`eraseRect`). `copy-rect` copies indices as they are, including 0 (`copyRect`: `srcX`, `srcY`, `width`, `height`, `dstX`, `dstY`, optional `fromFrame`, optional `toFrame`). `move-rect` does that copy and clears source cells that are not part of the destination, as one undo step (`moveRect`). Omit the frame flags to stay on the active frame. `--from-frame` reads another frame and `--to-frame` writes another; neither flag changes `activeFrame`. Across frames a move clears the whole source rectangle. If the source or the destination would leave the canvas, nothing is written.

```bash
pixelforge copy-rect --project hero.json --from-frame 0 --to-frame 2 \
  --src-x 16 --src-y 20 --width 32 --height 36 --dst-x 16 --dst-y 20
pixelforge hflip --project hero.json --frame 2
pixelforge flip --horizontal --project hero.json --x 0 --y 40 --width 64 --height 24
pixelforge resize --project hero.json --width 64 --height 64
pixelforge resize --project hero.json --width 64 --height 96 --offset-x 0 --offset-y 0
```

`hflip` and `flip --horizontal` mirror indices (`x` becomes `origin + width - 1 - x`). They do not recolor. Omit the rectangle to flip the whole frame. `resize` keeps the version 1 project file. Omit the offsets to center; pass both to place the old origin. Content that falls outside is clipped. One undo restores the previous canvas.

```bash
pixelforge call --project hero.json \
  --request '{"tool":"beginGesture"}' \
  --request '{"tool":"fillRect","params":{"x":8,"y":4,"width":16,"height":20,"colorIndex":1}}' \
  --request '{"tool":"fillRect","params":{"x":10,"y":24,"width":12,"height":28,"colorIndex":2}}' \
  --request '{"tool":"endGesture"}' \
  --preview
```

### Standing 立绘

For a human-to-zombie standing portrait, `new --width 64 --height 128` (or 96×128). Block the head, torso, and legs with `fill-rect`, `preview`, then correct the silhouette with `erase-rect` instead of hundreds of `erase` calls. Add cloth and skin as more rects, `preview` again, then `paint` only the details (eyes, teeth, tears in cloth). Shift an arm or a dropped phone with `move-rect` or `copy-rect`, then `preview`. `duplicateFrame` for the zombie pose, then `setActiveFrame` / `frame select` the returned index — duplicate does not switch to the new frame — and repeat the same look-then-fix loop. `preview --frame N` compares another frame without selecting it. `export png --scale 2` (or 4) when a stage should be read as a real image. Do not invent RGB, and do not emit a `paint` or `erase` per pixel for a flat area.

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

`frame duplicate` copies pixels and delay to the next slot and returns that index. It does **not** select the new frame. `activeFrame` stays on the same pixels: if it sat at or after the insertion point its index increases by one, otherwise it stays put. Paint the copy only after `frame select` / `setActiveFrame` on the returned index. The web editor's duplicate button selects the copy; the CLI and the `duplicateFrame` tool do not. `frame add` appends a blank frame at 100 ms and does not change the active frame. `frame delay` sets one frame. `export png` writes `pixelforge-000.png` onward. `export png --scale K` and `export sheet --scale K` nearest-neighbor upscale in the kernel (`K` ≥ 1, default 1). `export sheet` uses the kernel grid (`columns = ceil(sqrt(frame count))`) unless you pass `--cols N`. Scale does not change the column count. Use `--cols` when one row should stay one facing.

```bash
pixelforge preview --project hero.json --frame 0 --scale 4
pixelforge preview --project hero.json --frame 1 --scale 4
pixelforge diff --project hero.json --frame 0 --with 1 --scale 4
pixelforge export sheet --project hero.json --output hero-sheet.png --cols 4 --scale 2
```

`diff` is read-only. Stderr shows an index diff (`.` same, `!` both differ, any other glyph is the only ink). The PNG at `result.path` is three panels: frame, with, onion. The onion does not change pixels. Compare feet and arms there before repainting the walk.

The same edits as subcommands:

```bash
pixelforge frame duplicate --project hero.json --index 0
pixelforge frame select --project hero.json --index 1
pixelforge frame delay --project hero.json --index 1 --delay-ms 120
```

### Layers

Put the body on the bottom layer and the equipment on the layer above it. Repainting the equipment does not redraw the body. `preview` shows both. `export png` writes the composite.

```bash
pixelforge new --width 32 --height 48 --output hero.json
pixelforge layer rename --project hero.json --name body
pixelforge fill-rect --project hero.json --x 8 --y 10 --width 16 --height 24 --color-index 3
pixelforge layer add --project hero.json --name gear
pixelforge fill-rect --project hero.json --x 10 --y 12 --width 12 --height 10 --color-index 5
pixelforge preview --project hero.json --scale 4
pixelforge fill-rect --project hero.json --x 10 --y 12 --width 12 --height 10 --color-index 6
pixelforge preview --project hero.json --layer 0
pixelforge export png --project hero.json --out-dir ./hero-frames --scale 2
pixelforge layer flatten --project hero.json
```

`layer add` inserts a transparent layer above the active one on every frame and selects it. Omit `--name` and the name is the next free `Layer N`, starting at one past the current count. `layer list` prints the stack. `layer select --index 0` goes back to the body. `layer remove` deletes the active layer, or `--index N`; the last layer is refused. `layer reorder --from 1 --to 0` moves gear under the body (`to` is the index after the move). `fill-rect --layer 0` paints the body without selecting it. A new project and an old project are one layer named `Layer 1`.

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
