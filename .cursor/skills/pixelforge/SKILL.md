---
name: pixelforge
description: >-
  Edit PixelForge indexed-pixel projects with the pixelforge CLI and the kernel
  Tool API. Use when the user says 用 pixelforge, 改工程, or CLI, or when
  installing or running pixelforge, creating or changing a pixelforge-project,
  painting or erasing cells, filling or erasing a rectangle, copying or moving
  a rectangle, editing the palette, locking, quantizing, or remapping it,
  crushing a PNG onto the palette, adding or duplicating frames, undoing,
  running inspect, scene, tools, or call, or exporting a PNG sequence or sprite sheet.
  Other creative skills (pixelforge-draw, pixelforge-sprite-4dir,
  pixelforge-layers, pixelforge-critique) must follow this skill's index-color
  invariants before their own workflow.
---

# PixelForge（shared）

用 `pixelforge` CLI 改索引像素。格子里只存调色板索引。不要自己发明 RGB，不要在应用里做多轮聊天，也不要加 MCP 服务器。

Edit indexed pixels only through the `pixelforge` CLI. The Rust kernel owns the pixels. You do not invent RGB, you do not build an in-app multi-turn chat, and you do not add an MCP server.

`pixelforge-draw`、`pixelforge-sprite-4dir`、`pixelforge-layers`、`pixelforge-critique` 都先遵守本 skill 的不变量，再套各自的流程。造型步骤在那些 skill 里。本 skill 不判断好不好看。

*Creative skills follow these invariants first. Staging a figure, a four-direction sheet, layers, and critique live in those skills. This skill does not judge taste.*

## When to use

创建或修改 `pixelforge-project`、调色板、帧、导出，或任何一次 `pixelforge` 调用。产品路径是：外部 Agent → `pixelforge` CLI → `Document::call_tool`。网页里的一轮面板是可选演示。嵌在应用里的多轮对话（#14）不是产品。MCP 不在范围内；以后的封装只调用这支 CLI。

画角色、立绘、剪影：同时读 `pixelforge-draw`。四向走加待机的精灵表：同时读 `pixelforge-sprite-4dir`。身体、装备、四肢分层：同时读 `pixelforge-layers`。只读挑错：读 `pixelforge-critique`，不要改像素。

*Use this skill for every project edit. Read the matching creative skill for how to stage the picture.*

## Install and run

在仓库根目录：

```bash
cargo install --path crates/pixelforge-cli
pixelforge --help
```

不安装也可以：

```bash
cargo run -p pixelforge-cli -- --help
```

需要 Rust stable（`cargo` 1.83 或更新）。没有 Python 服务。

*From the repository root, `cargo install --path crates/pixelforge-cli`, or `cargo run -p pixelforge-cli -- --help`. Rust stable (`cargo` 1.83 or newer). There is no Python service.*

## Invariants

- 格子存的是调色板索引。索引 0 是透明。`paint` 和 `fillRect` 写入已经存在的索引（从 1 起），不写 RGB。`eraseRect` 写入索引 0。`copyRect` 和 `moveRect` 原样复制索引，包括 0。
- 每一帧共用同一个层栈。下标 0 在最下面。绘制写当前层，或者 `--layer` 指定的那一层，别的层不动。`--layer` 不切换当前层。`preview`、`diff`、`export png` / `export sheet` 看的是合成图：索引 0 露出下面的层，其他索引盖住下层。层与层之间不按调色板 alpha 混合。`preview --layer N` 和 `diff --layer N` 只看那一层。`layer flatten` 把合成结果压成名为 `Layer 1` 的单层。仍然是这一层默认状态时，文件里不写 `layers` 和 `activeLayer`。分层怎么摆见 `pixelforge-layers`。
- 新建工程带工作室调色板，索引 1 到 8 可以直接画。要新颜色，先 `palette add` 或 `palette set`，再用返回的索引去画。
- `palette lock` 之后仍可用 `paint` 和 `fillRect` 画已有索引，但不能改色、追加、导入或 `remap`。`apply-png` 只写索引，锁定时仍可用。
- 画笔默认拒绝越界索引（`indexPolicy: reject`）。`palette enforce --mode clamp` 才把越界索引写成最后一项不透明色。索引 0 仍然拒绝。只有发生钳制时，结果里才有 `requestedIndex`。`import` 的 `replace` 仍把消失的索引清成 0，不看这个策略。
- 不要手改工程 JSON 去塞一个颜色。越界索引会让文件打不开。不要把扩散图写回工程。
- 一次进程是一份文档。同一次 `pixelforge call` 里的多个 `--request` 共享撤销栈。工程文件不保存撤销栈。下一个进程是空栈。对刚打开的文件执行 `pixelforge undo` 会得到 `NothingToUndo`。
- 失败的那一次调用不改文档。一批请求里，失败之前的成功调用会保存，失败之后的不再执行。
- 导出在内核里把索引解析成颜色。不要自己给 PNG 重新上色。`diff` / `diffFrames` 画出洋葱皮，不写回像素。`flip` / `hflip` 只搬索引，不改高光颜色。镜像之后要修高光时见 `pixelforge-sprite-4dir`。

*Cells store palette indices. Index 0 is transparent. Drawing writes the active layer, or `--layer`, and does not select that layer. `preview`, `diff`, and export show the composite. A locked palette still accepts paint of an existing index and refuses recolor, append, import, and remap. Out-of-range paint indices are rejected unless `indexPolicy` is `clamp`. Do not hand-edit the project JSON. One process is one document; the file does not store the undo stack. `flip` moves indices and does not recolor.*

## Output and exit codes

stdout 是紧凑 JSON，一行一个响应。解析 stdout 的 `ok` 和 `error.code`。stderr 是短句（`wrote hero.json`），加上 `preview`、`diff` 或 `--preview` 时的 ASCII 图。要看图就读 stderr 的格子，或 `preview` 的 `result.rows`。

退出码：`0` 每个工具都是 `ok: true`。`1` 某个工具 `ok: false`（stdout 仍是那个错误对象）。`2` 工具没跑起来（文件打不开、工程损坏、请求文件不是 JSON）。`preview` 和 `diff` 在内核拒绝时也是退出码 `2`，stdout 仍是那个错误对象。clap 的用法错误也是退出码 `2`，说明在 stderr，没有 JSON 行。

*`0` all ok, `1` tool error with JSON on stdout, `2` the tool did not run or clap rejected the command. `preview` and `diff` use `2` when the kernel refuses.*

## How to read state

先读状态，再画。`scene` 不改文件，也不把整幅格子打出来。

```bash
pixelforge scene --project hero.json
pixelforge inspect --project hero.json
pixelforge paint --project hero.json --x 1 --y 2 --color-index 3 --dirty
pixelforge preview --project hero.json --scale 4
pixelforge diff --project hero.json --frame 0 --with 1
```

`scene` 的 `result`：`width`、`height`、`activeFrame`、`activeLayer`、`layers`（`index`、`name`、`visible`、`opacity`）、`palette`、`paletteCount`、`paletteHash`、`paletteLocked`、`indexPolicy`、`frameCount`、`delayMs`、`frames`（每帧 `index` 和 `delayMs`）。没有 `pixels`。要格子用 `inspect` 或 `preview`。`indexPolicy` 默认 `reject`。

`visible` 恒为 `true`，`opacity` 恒为 `null`。内核没有逐层隐藏，也没有逐层透明度。非 0 索引盖住下层。

`paletteHash` 是 FNV-1a 64 位，16 位小写十六进制。初值 `14695981039346656037`，乘数 `1099511628211`，按索引把每一项的 `r`、`g`、`b`、`a` 送进去。颜色和顺序不变，哈希就不变。`inspect` 的 `result.paletteHash` 是同一个值。

`--dirty`（Tool API 是 `reportDirty: true`）只加在 `paint`、`erase`、`fillRect`、`eraseRect`、`copyRect`、`moveRect`、`flipHorizontal`、`resizeCanvas` 上。结果多 `changed`、`dirty`（`{x,y,width,height}`，没变化是 `null`）、`regions`（每个改过的帧和层一条）。`resize` 再多 `clipped`：新画布上已经没有的坐标里，原来非 0 的格子数。盒子包住的是索引真的变了的格子，不是请求里的整块矩形。不写 `--dirty`，这些字段就不出现，旧的 `result` 保持原样。这不是命令日志。

`preview` 除了 `rows`，还有 `ink`（非 0 格数）、`bounds`、`indices`（每个索引用了几次）、`pixels`（这一张视图的索引，行优先）。字形 `*` 把 62 及以上折成同一个字符，读索引用 `pixels`。

`diff` 除了 `diffRows` 和 `changed`，还有 `bounds`、`kinds`（`same`、`differ`、`onlyFrame`、`onlyWith`）、`changes`（`x`、`y`、`left`、`right`）。`!` 不带两边的索引，索引在 `changes` 里。

*Read `scene` before painting. It does not write the file and it does not return the pixel buffer. `visible` is always true and `opacity` is always null. `paletteHash` is FNV-1a 64 of each entry's `r,g,b,a` in index order. `--dirty` adds `changed`, `dirty`, and `regions` on the drawing tools; omit it and those fields stay absent. `preview` adds `ink`, `bounds`, `indices`, and `pixels`. `diff` adds `bounds`, `kinds`, and `changes`. This is not a command log.*

## Pictures, sheets, and canvas commands

这些是命令语义。剪影怎么画、四向怎么排、层怎么命名，不在这里。

`preview` 子命令只读。它把 ASCII 打到 stderr，并写出最近邻彩色 PNG。`--scale K` 与导出相同（整数 ≥ 1，默认 1）。路径在 `result.path`（默认工程旁边的 `<stem>.preview.png`，或 `--output`）。`--frame N` 画第 N 帧，不改 `activeFrame`，也不改文件。`--layer N` 只画那一层，不选中它。`result.frame` 是这张图，`result.activeFrame` 仍是文档的当前帧。字形：`.` 透明（索引 0），`1`–`9` 是这些索引，`A`–`Z` 是 10–35，`a`–`z` 是 36–61，`*` 是 62 及以上（stderr 的 legend 写出索引）。会改像素的命令上的 `--preview` 只有 ASCII，不写这张彩色 PNG。

`diff` 只读。`--frame A --with B`。stderr：`.` 索引相同，`!` 两边都有墨且不同，其它字形是唯一有墨的那边。彩色 PNG 在 `result.path`（默认 `<stem>.diff.png`），三格：frame、with、洋葱皮。洋葱皮不改像素。`--scale` 同上。`--layer N` 只比较那一层。

`hflip` 等于 `flip --horizontal`，不需要再写 `--horizontal`。`flip` 必须写 `--horizontal`，否则退出码 `2`。两者都只镜像索引（`x` 收到 `origin + width - 1 - x`），不改颜色。不写矩形就是整帧；写矩形时 `x`、`y`、`width`、`height` 要齐。`--frame N` 不切换当前帧。一步撤销。

`resize --width W --height H` 把每一帧、每一层的索引导进新画布，格式仍是 version 1。不写偏移就居中；`--offset-x` 和 `--offset-y` 要成对出现。越出新画布的格子被裁掉。放大不会凭空补细节。边长 `1..=256`。96×96、128×128，以及需要时的 96×128、64×128，都在这条边长里，是 `pixelforge-draw` 的正式档，不是另一条命令。默认 `new` 仍是 16×16。`preview` 的 ASCII 按整帧输出，一格一个字形，不裁行。96 和 128 的整张格子很长，阶段检查看 `preview --scale` 的彩色 PNG，ASCII 抽局部行。画布该选哪一档见 `pixelforge-draw`。

`export png --scale K` 和 `export sheet --scale K` 在内核里最近邻放大，再解析调色板。只为看颜色时，用这个或 `preview --scale`，不要再用外部工具放大。`export sheet` 不传 `--cols` 时列数是 `ceil(sqrt(帧数))`。`--cols N` 指定列数，行数是 `ceil(帧数 / N)`，`scale` 不改变列数。一行一个朝向时 `N` 是该朝向的帧数，见 `pixelforge-sprite-4dir`。`export` 子命令把文件写到磁盘并返回路径。`call` 里的 `exportPngSequence` / `exportSpriteSheet` 返回 base64，不写文件。

平涂用 `fillRect`，挖块用 `eraseRect`，挪一块用 `copyRect` / `moveRect`。一次 `pixelforge call` 里放多个 `--request`。一块矩形已经是一步撤销。好几块要一起撤销时，用 `beginGesture` / `endGesture` 包住。

*`preview` writes ASCII plus a color PNG at `result.path`. `--preview` on a mutating command is ASCII only. `diff` is read-only. `hflip` mirrors indices and does not recolor. `resize` copies stored indices. `export sheet --cols N` sets the column count. Edges of 96 and 128 are inside `1..=256`. ASCII on those canvases is the full grid; read the scaled PNG and a local slice of rows.*

## Recipes

### Blank project

```bash
pixelforge new --width 16 --height 16 --output hero.json
```

stdout 是 `inspect` 响应。文件是 version 1 `pixelforge-project`。`new` 不覆盖已有文件。尺寸按 `pixelforge-draw` 的表选，不要默认 16×16 画全身。

### Paint, fill, erase, blit, flip, resize

```bash
pixelforge fill-rect --project hero.json --x 4 --y 8 --width 16 --height 40 --color-index 3 --preview
pixelforge erase-rect --project hero.json --x 4 --y 40 --width 16 --height 8
pixelforge copy-rect --project hero.json --src-x 8 --src-y 70 --width 6 --height 10 --dst-x 40 --dst-y 78
pixelforge move-rect --project hero.json --src-x 40 --src-y 78 --width 6 --height 10 --dst-x 44 --dst-y 80
pixelforge paint --project hero.json --x 1 --y 2 --color-index 3
pixelforge erase --project hero.json --x 1 --y 2
pixelforge preview --project hero.json --scale 4
pixelforge preview --project hero.json --frame 1 --scale 4
pixelforge diff --project hero.json --frame 0 --with 1 --scale 4
```

`fill-rect` 用一个已有索引填轴对齐矩形。`x`、`y` 是左上角。`width` 和 `height` 至少为 1，整块必须在画布内；只要有一格出界，一格都不写。`call` 里的工具名是 `fillRect`。`erase-rect` 清成索引 0（`eraseRect`）。`copy-rect` 原样复制索引，包括 0（`copyRect`：`srcX`、`srcY`、`width`、`height`、`dstX`、`dstY`，可选 `fromFrame`、`toFrame`）。`move-rect` 在同一步撤销里搬走，并把不在目标里的源格子清成 0（`moveRect`）。`--from-frame` / `--to-frame` 不改变 `activeFrame`。跨帧的 `move-rect` 清掉整个源矩形。源或目标出界时，一格都不写。`--layer N` 画那一层，不选中它。

```bash
pixelforge copy-rect --project hero.json --from-frame 0 --to-frame 2 \
  --src-x 16 --src-y 20 --width 32 --height 36 --dst-x 16 --dst-y 20
pixelforge hflip --project hero.json --frame 2
pixelforge flip --horizontal --project hero.json --x 0 --y 40 --width 64 --height 24
pixelforge resize --project hero.json --width 64 --height 64
pixelforge resize --project hero.json --width 64 --height 96 --offset-x 0 --offset-y 0
```

### One undo step

手势和撤销放在同一次进程里。进程结束后栈就没了；要回头先 `cp hero.json hero.json.bak`。

```bash
pixelforge call --project hero.json \
  --request '{"tool":"beginGesture"}' \
  --request '{"tool":"fillRect","params":{"x":8,"y":4,"width":16,"height":20,"colorIndex":1}}' \
  --request '{"tool":"fillRect","params":{"x":10,"y":24,"width":12,"height":28,"colorIndex":2}}' \
  --request '{"tool":"endGesture"}' \
  --preview
```

要在这次进程里撤回，把 `--request '{"tool":"undo"}'` 加在同一条命令上。

### Frames and export

`frame duplicate` / `duplicateFrame` 复制像素和时长，返回新下标。它不选中新帧。当前帧若在插入点或之后，下标加一，像素还是原来的。画拷贝之前要 `frame select` / `setActiveFrame`。网页上的复制按钮会选中；CLI 不会。`frame add` 在末尾追加一帧 100 毫秒空白，不改当前帧。`frame delay` 的范围是 `1..=65535`。`export png` 写出 `pixelforge-000.png` 起的文件。

```bash
pixelforge call --project hero.json \
  --request '{"tool":"duplicateFrame","params":{"index":0}}' \
  --request '{"tool":"setActiveFrame","params":{"index":1}}' \
  --request '{"tool":"setFrameDelay","params":{"index":1,"delayMs":120}}'
pixelforge frame duplicate --project hero.json --index 0
pixelforge frame select --project hero.json --index 1
pixelforge frame delay --project hero.json --index 1 --delay-ms 120
pixelforge export sheet --project hero.json --output hero-sheet.png --cols 4 --scale 2
pixelforge export png --project hero.json --out-dir ./hero-frames --scale 2
```

四向表的行、列和脚滑检查见 `pixelforge-sprite-4dir`。

### Palette

```bash
pixelforge palette add --project hero.json --r 20 --g 30 --b 40
pixelforge palette set --project hero.json --index 1 --r 10 --g 20 --b 30
pixelforge palette lock --project hero.json
pixelforge palette unlock --project hero.json
pixelforge palette import --project hero.json --mode merge --file colors.gpl
pixelforge palette enforce --project hero.json --mode clamp
pixelforge palette remap --project hero.json --max-colors 8
pixelforge palette remap --project hero.json --file colors.hex
pixelforge palette apply-png --project hero.json --file sprite.png --frame 0
```

`palette add` 的结果里有新索引，画的时候用那个索引。`import` 的 `mode` 是 `replace` 或 `merge`。锁定时导入和 `remap` 会被拒绝。

`enforce` 的 `mode` 是 `reject`（默认）或 `clamp`。`clamp` 才写入工程文件。范围内的 `paint` 结果仍是 `{}`（外加可选的脏区字段）。

`remap` 只能给 `--max-colors` 或 `--file` 里的一个。`--file` 是 `.gpl` 或十六进制列表，和 `import` 同一种文本。距离是平方 RGB，平局取较小索引，不平均出新颜色。`apply-png` 把一张和画布同尺寸的 PNG 压进当前调色板，不追加颜色，alpha 小于 128 写成索引 0。两者的 `result` 都有 `pixelsChanged`、`misses`（没精确命中的颜色）、`merges`（并进了哪项近似色，`exact: true` 表示只是索引被压紧）、`dropped`（量化时丢掉的未使用色）。没有 Lospec 在线浏览器。

Tool API：

```json
{ "tool": "setIndexPolicy", "params": { "mode": "clamp" } }
{ "tool": "remapPalette", "params": { "maxColors": 8 } }
{ "tool": "remapPalette", "params": { "text": "ff0000\n00ff00\n" } }
{ "tool": "applyPng", "params": { "png": "<base64>", "frame": 0, "layer": 0 } }
```

*Pass `--max-colors` or `--file` to `remap`, not both. `apply-png` requires the PNG to match the canvas. Read `misses`, `merges`, and `pixelsChanged`. There is no Lospec browser.*

### Inspect and raw calls

```bash
pixelforge inspect --project hero.json
pixelforge tools
pixelforge call --project hero.json --request '{"tool":"inspect"}'
pixelforge call --project hero.json --request-file edits.json
```

`edits.json` 可以是一个 JSON 对象、一个 JSON 数组，或每行一个 JSON 值。`--request` 原样交给 `Document::call_tool`。不带 `--request` 时也可以从 stdin 送入。

## Catalog

工具名、参数、`error.code`：`docs/tool-api.md`。技能表和命令速查：`docs/agent-skill.md`。

| 接着读 | 任务 |
| --- | --- |
| `pixelforge-draw` | 从零画角色、立绘、剪影 |
| `pixelforge-sprite-4dir` | 四向走 + 闲精灵表 |
| `pixelforge-layers` | body / gear / limbs |
| `pixelforge-critique` | 只读挑错 |
