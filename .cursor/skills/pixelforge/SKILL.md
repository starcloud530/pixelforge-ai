---
name: pixelforge
description: >-
  Edit PixelForge indexed-pixel projects with the pixelforge CLI and the kernel
  Tool API. Use when creating or changing a pixelforge-project, painting or
  erasing cells, editing the palette, adding or duplicating frames, setting
  delays, undoing, or exporting a PNG sequence or sprite sheet.
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

- Cells store palette indices. Index 0 is transparent. `paint` writes an existing index from 1 up, never raw RGB.
- A blank project already has the studio palette (indices 1 through 8). Paint those. To introduce a color, `palette add` or `palette set`, then paint the index the tool returns.
- `palette lock` still allows paint of an existing index. It refuses recolor, append, and import.
- Do not hand-edit the project JSON to plant a color. A bad index fails to load. Do not write a diffusion image back into the project.
- One process is one document. Several `--request` flags on one `pixelforge call` share the undo stack. The project file does not store that stack. The next process starts empty, same as opening the file in the editor. `pixelforge undo` on a freshly opened file returns `NothingToUndo`.
- A refused call does not change the document. In a batch, earlier calls are saved and later calls are not run.
- Export resolves indices through the palette inside the kernel. Do not recolor the PNG yourself. Onion skin is a view; there is no tool that writes it into pixels.
- Stdout is compact JSON, one response per line. Stderr is a short note (`wrote hero.json`). Parse stdout. Branch on `error.code`.

Exit codes: `0` every tool returned `ok: true`. `1` a tool returned `ok: false` (stdout still has the error object). `2` the command did not run the tool (missing file, bad project, bad request file). Clap usage errors also exit `2` and print help to stderr, without a JSON line.

## Recipes

### Blank project

```bash
pixelforge new --width 16 --height 16 --output hero.json
```

Stdout is the Tool API `inspect` response. The file is version 1 `pixelforge-project` JSON. `new` refuses to replace a file that already exists.

### Paint and erase one cell

```bash
pixelforge paint --project hero.json --x 1 --y 2 --color-index 3
pixelforge erase --project hero.json --x 1 --y 2
```

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
