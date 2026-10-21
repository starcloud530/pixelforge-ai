# 给外部 Agent 的 skill Agent skill

产品路径是：**仓库外的 Agent → 本地 `pixelforge` CLI → Rust 内核 Tool API**。Agent 不改界面上的像素缓冲，也不在这个仓库里做多轮聊天。网页里的一轮面板是可选演示，见 [ai-driver.md](ai-driver.md)。[#14](https://github.com/starcloud530/pixelforge-ai/issues/14) 那种嵌在应用里的多轮对话不是产品。MCP 不在这一阶段；以后若要接，只做这支 CLI 的薄封装。

Cursor 从 [.cursor/skills/pixelforge/SKILL.md](../.cursor/skills/pixelforge/SKILL.md) 读取同一份操作说明。Codex、Claude Code 或其他能跑命令的 Agent 按本文件做即可。

*The product path is an agent outside this repository, then the local `pixelforge` CLI, then the Rust kernel Tool API. The agent does not patch the pixel buffer the view draws, and this repository does not host a multi-turn chat. The one-turn panel in the web shell is an optional demo; see [ai-driver.md](ai-driver.md). An embedded multi-turn chat like [#14](https://github.com/starcloud530/pixelforge-ai/issues/14) is not the product. MCP is out of this stage. A later wrapper, if any, would be a thin layer over this CLI.*

*Cursor loads [.cursor/skills/pixelforge/SKILL.md](../.cursor/skills/pixelforge/SKILL.md). Codex, Claude Code, and any other agent that can run a command follow this file.*

## 安装与运行 Install and run

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

*From the repository root, `cargo install --path crates/pixelforge-cli` puts `pixelforge` on the path. `cargo run -p pixelforge-cli -- --help` runs it without installing. Rust stable (`cargo` 1.83 or newer) is enough. There is no Python service.*

## 输出 Output

stdout 是紧凑 JSON，一行一个响应，给 Agent 解析。stderr 是给人看的短句，例如 `pixelforge: wrote hero.json`。`preview` 和 `--preview` 还会把当前帧的 ASCII 图画到 stderr，那一块要读，用来看形状；其余 stderr 不要当接口解析。`preview` 的 stdout 里，`result.rows` 是同一张图，一行一个字符串。

`call` 打出的就是 Tool API 响应，和 [tool-api.md](tool-api.md) 一致。`export` 子命令把 PNG 写到磁盘，stdout 里是路径和网格尺寸，不带 base64。要用 base64，就 `call` 工具 `exportPngSequence` 或 `exportSpriteSheet`，那两个工具不写文件。

退出码：`0` 全部 `ok: true`。`1` 某个工具 `ok: false`，stdout 仍是那个错误对象。`2` 工具没跑起来（文件打不开、工程损坏、请求文件不是 JSON）。`preview` 和 `diff` 在内核拒绝时也是退出码 `2`，stdout 仍是那个错误对象。缺了必填参数时 clap 把说明打到 stderr，也是退出码 `2`，这时没有 JSON 行。

*Stdout is compact JSON, one response per line. Stderr is a short note such as `pixelforge: wrote hero.json`. Parse stdout. `preview` and `--preview` also write an ASCII picture of the active frame to stderr; read that picture. `preview` puts the same rows in `result.rows`. `call` prints the Tool API response from [tool-api.md](tool-api.md). The `export` subcommand writes PNG files and prints paths and the grid size, without base64. Base64 comes from `call` with `exportPngSequence` or `exportSpriteSheet`, and those calls do not write files.*

*Exit `0` means every tool returned `ok: true`. Exit `1` means a tool returned `ok: false` and stdout still holds that error object. Exit `2` means the tool did not run (unreadable file, bad project, request file is not JSON). `preview` and `diff` also exit `2` when the kernel refuses, and stdout is still that error object. A missing flag is also exit `2`: clap writes the usage text to stderr and there is no JSON line.*

## 不变量 Invariants

- 格子存的是调色板索引。索引 0 是透明。`paint` 和 `fillRect` 写入已经存在的 `1..=255` 索引，不写 RGB。`eraseRect` 写入索引 0。`copyRect` 和 `moveRect` 原样复制索引，包括 0。
- 每一帧共用同一个层栈。下标 0 在最下面。绘制写当前层，或者 `--layer` 指定的那一层，别的层不动。`--layer` 不切换当前层。`preview`、`diff`、`export png` / `export sheet` 看的是合成图：索引 0 露出下面的层，其他索引盖住下层。层与层之间不按调色板 alpha 混合。`preview --layer N` 和 `diff --layer N` 只看那一层。`layer flatten` 把合成结果压成名为 `Layer 1` 的单层，文件就回到旧的单层工程。仍然是这一层默认状态时，文件里不写 `layers` 和 `activeLayer`。
- 新建工程带工作室调色板，索引 1 到 8 可以直接画。要新颜色，先 `palette add` 或 `palette set`，再用返回的索引去画。
- `palette lock` 之后仍可用 `paint` 和 `fillRect` 画已有索引，但不能改色、追加或导入。
- 不要手改工程 JSON 去塞一个颜色。越界索引会让文件打不开。不要把扩散图写回工程。
- 一次进程是一份文档。同一次 `pixelforge call` 里的多个 `--request` 共享撤销栈。工程文件不保存撤销栈，下一个进程是空栈，和在编辑器里重新打开这份文件一样。对刚打开的文件执行 `pixelforge undo` 会得到 `NothingToUndo`。
- 失败的那一次调用不改文档。一批请求里，失败之前的成功调用会保存，失败之后的不再执行。
- 导出在内核里把索引解析成 RGBA。不要自己给 PNG 重新上色。`diff` / `diffFrames` 画出洋葱皮视图，不把它写回像素。`flip` 只搬索引，不改高光颜色。

*Cells store palette indices. Index 0 is transparent. `paint` and `fillRect` write an existing index, never RGB. `eraseRect` writes index 0. `copyRect` and `moveRect` copy stored indices, including 0. Every frame shares one layer stack. Index 0 is the bottom. Drawing writes the active layer, or `--layer`, and leaves the other layers alone. `preview`, `diff`, and export show the composite: index 0 shows the layer underneath, and any other index covers it. Palette alpha is not blended between layers. `layer flatten` writes that composite into one layer named `Layer 1`. A new project ships the studio palette, indices 1 through 8. Add or recolor a palette entry before painting a new color. A locked palette still accepts paint and rectangle fill of an existing index and refuses recolor, append, and import. Do not hand-edit the project JSON to plant a color, and do not write a diffusion image back into the project.*

*One process is one document. Several `--request` flags on one `pixelforge call` share the undo stack. The project file does not store that stack, so the next process starts empty, the same as reopening the file. `pixelforge undo` on a freshly opened file returns `NothingToUndo`. A refused call does not change the document. Earlier calls in that batch are saved; later calls are not run. Export resolves indices in the kernel. `diff` / `diffFrames` draws an onion view and does not write it into pixels. `flip` moves indices only and does not recolor highlights.*

## 画布尺寸 Canvas size

在 `pixelforge new` 里定尺寸。太小的全身会变成示意图。画布选小了可以 `resize`：每一帧的索引拷进更大的画布，默认居中，也可以 `--offset-x` / `--offset-y`。放大只搬已有索引，不会凭空补细节。

*Choose the size in `pixelforge new`. A full body on a tiny grid looks like a diagram. `resize` copies every frame into a larger canvas, centered unless you pass `--offset-x` and `--offset-y`. Expanding only moves indices you already stored.*

| 题材 Subject | 尺寸 Size |
| --- | --- |
| 俯视小怪、图标、地块 Top-down minion, icon, tile | 16–32 |
| 全身游戏精灵 Full-body game sprite | 大约 32–64 about 32–64 |
| 站立立绘 / 变身故事 Standing 立绘 / transformation story | 至少 64×96，优先 **64×128** 或 **96×128** at least 64×96; prefer **64×128** or **96×128** |
| 胸像、脸部特写 Bust or face-focused portrait | 至少 64×64 at least 64×64 |

**48×64 的全身会显得示意图化**：头只有几格，姿势读不出来，下一帧也放不下破损、衣服或第二套剪影。不要用 16×16 或 32×32 起一张站立立绘。

*A **48×64 full body looks schematic**: the head is a few pixels, the pose cannot read, and later frames have nowhere to put damage, cloth, or a second silhouette. Do not start a standing figure at 16×16 or 32×32.*

## 先看再改 Observe, then revise

不要蒙着把所有帧画完。每个大阶段结束都要看图再改：剪影、平涂、细节、姿势。

*Do not paint every frame blind. After each major stage — silhouette, flats, details, pose — look, then fix.*

1. 用 `fill-rect`、`erase-rect`、`copy-rect` 或 `move-rect`，再加一次短的 `call`，画完这一阶段。
2. `pixelforge preview --project … --scale 4`（要看别的帧就加 `--frame N`，不会切当前帧），打开 `result.path`。两帧要对齐时用 `diff --frame A --with B --scale 4`。
3. 读 stderr 上的 ASCII（或 `result.rows`）看破洞和剪影。读放大后的彩色 PNG 看橙衣、灰绿皮肤、金属面罩和相邻色阶是否分开。
4. 改完形状再进入下一阶段。

*Draw that stage with `fill-rect`, `erase-rect`, `copy-rect`, or `move-rect`, and a short `call` batch. Then `pixelforge preview --project … --scale 4` (`--frame N` reads another frame without selecting it) and open `result.path`. Use `diff --frame A --with B --scale 4` when two poses must line up. Read the ASCII grid for holes and the scaled PNG for color. Fix the shape before the next stage.*

字形：`.` 是透明（索引 0），`1`–`9` 是这些索引，`A`–`Z` 是 10–35，`a`–`z` 是 36–61，`*` 是 62 及以上（stderr 的 legend 会写出那些索引和 `#rrggbb`）。stdout 仍然是 JSON。`preview --frame N` 画第 N 帧，不改 `activeFrame`，也不改工程文件。`result.frame` 是这张图，`result.activeFrame` 仍是文档的当前帧。`preview` 还会按 `--scale` 写出彩色 PNG，路径在 `result.path`（默认工程旁边的 `<stem>.preview.png`）。`--scale` 和 `export` 一样，是 ≥ 1 的整数，默认 1。

*Glyphs: `.` is transparent (index 0), `1`–`9` are those indices, `A`–`Z` are 10–35, `a`–`z` are 36–61, and `*` is index 62 or above (the stderr legend names those indices and a `#rrggbb`). Stdout stays JSON. `preview --frame N` draws frame N and does not change `activeFrame` or the project file. `result.frame` is the picture; `result.activeFrame` is still the document's active frame. `preview` also writes a color PNG at `result.path` (default `<stem>.preview.png` beside the project). `--scale` matches export: an integer ≥ 1, default 1.*

`export png --scale K` 和 `export sheet --scale K` 在内核里做最近邻放大（`K` 是 ≥ 1 的整数，默认 1）。每个索引变成 `K × K` 的块，再由内核解析颜色。只为看颜色时，不要再用外部工具放大 PNG。

*`export png --scale K` and `export sheet --scale K` nearest-neighbor upscale inside the kernel (`K` is an integer ≥ 1, default 1). Each index becomes a `K` by `K` block, then the kernel resolves color. Do not scale the PNG externally when you only need to judge color.*

平涂优先 `fillRect`，挖剪影优先 `eraseRect`，挪姿势或道具优先 `copyRect` / `moveRect`。一次 `pixelforge call` 里放多个 `--request`，不要为了一块纯色打出几千次 `paint` 或 `erase`。一块 `fillRect`、`eraseRect`、`copyRect` 或 `moveRect` 已经是一步撤销。好几块要一起撤销时，用 `beginGesture` / `endGesture` 包住。

*Prefer `fillRect` for a solid region, `eraseRect` for cutting a silhouette, and `copyRect` / `moveRect` for a pose shift or a prop that moves. One `pixelforge call` with several `--request` flags beats thousands of lone `paint` or `erase` calls. A single `fillRect`, `eraseRect`, `copyRect`, or `moveRect` is already one undo step. Wrap several rects in `beginGesture` / `endGesture` when they should undo together.*

## 配方 Recipes

### 空白工程 Blank project

```bash
pixelforge new --width 16 --height 16 --output hero.json
```

stdout 是 `inspect` 的响应。文件是 version 1 `pixelforge-project`。`new` 不会覆盖已经存在的文件。

*Stdout is the `inspect` response. The file is version 1 `pixelforge-project`. `new` refuses to replace a file that already exists.*

### 画一格、填一块、擦一格 Paint, fill, and erase

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

`fill-rect` 在当前帧上用一个调色板索引填一块轴对齐矩形。`x`、`y` 是左上角。`width` 和 `height` 至少为 1，整块必须落在画布里；只要有一格会出界，一格都不写。色板锁定时仍然可以填。`call` 里的工具名是 `fillRect`。

*`fill-rect` writes one palette index into an axis-aligned rectangle on the active frame. `x` and `y` are the top-left cell. `width` and `height` are at least 1, and the whole rectangle must sit inside the canvas; if any cell would fall outside, nothing is written. A locked palette still allows the fill. The tool name inside `call` is `fillRect`.*

`erase-rect` 把同样的矩形清成索引 0（`eraseRect`）。`copy-rect` 原样复制索引，包括 0（`copyRect` 的参数是 `srcX`、`srcY`、`width`、`height`、`dstX`、`dstY`，外加可选的 `fromFrame`、`toFrame`）。`move-rect` 在同一步撤销里搬走这些索引，并把不在目标里的源格子清成 0（`moveRect`）。不写 `--from-frame` / `--to-frame` 时仍在当前帧。写了也不改变 `activeFrame`。跨帧的 `move-rect` 会把整个源矩形清掉。源或目标出界时，一格都不写。

*`erase-rect` clears that rectangle to index 0 (`eraseRect`). `copy-rect` copies indices as they are, including 0 (`copyRect`: `srcX`, `srcY`, `width`, `height`, `dstX`, `dstY`, optional `fromFrame`, optional `toFrame`). `move-rect` moves those indices and clears source cells outside the destination, as one undo step (`moveRect`). Omit `--from-frame` / `--to-frame` to stay on the active frame. Those flags do not change `activeFrame`. A cross-frame move clears the whole source rectangle. If the source or the destination would leave the canvas, nothing is written.*

```bash
pixelforge copy-rect --project hero.json --from-frame 0 --to-frame 2 \
  --src-x 16 --src-y 20 --width 32 --height 36 --dst-x 16 --dst-y 20
pixelforge hflip --project hero.json --frame 2
pixelforge flip --horizontal --project hero.json --x 0 --y 40 --width 64 --height 24
pixelforge resize --project hero.json --width 64 --height 64
```

`hflip` 和 `flip --horizontal` 只做索引镜像（`x` 收到 `origin + width - 1 - x`），不改颜色。不写矩形就是整帧。`resize` 仍写出 version 1 工程。不写偏移就居中；两个偏移都写就指定旧原点。越出新画布的格子被裁掉。一步撤销回到原来的画布。

*`hflip` and `flip --horizontal` mirror indices and do not recolor. Omit the rectangle to flip the whole frame. `resize` keeps the version 1 project. Omit the offsets to center. Content outside the new canvas is clipped. One undo restores the previous canvas.*

```bash
pixelforge call --project hero.json \
  --request '{"tool":"beginGesture"}' \
  --request '{"tool":"fillRect","params":{"x":8,"y":4,"width":16,"height":20,"colorIndex":1}}' \
  --request '{"tool":"fillRect","params":{"x":10,"y":24,"width":12,"height":28,"colorIndex":2}}' \
  --request '{"tool":"endGesture"}' \
  --preview
```

### 站立立绘 Standing 立绘

都市人变成丧尸这类站立图，用 `new --width 64 --height 128`（或 96×128）。先用 `fill-rect` 铺头、躯干、腿，`preview`，再用 `erase-rect` 改剪影，不要几百次 `erase`。衣服和皮肤继续用矩形，再 `preview`。眼睛、牙、破布才用 `paint`。手臂或掉落的手机用 `move-rect` / `copy-rect` 挪，然后再 `preview`。`duplicateFrame` 做丧尸姿势之后，必须 `setActiveFrame` / `frame select` 到返回的新下标——复制不会切到新帧——再重复「画一段、看一眼、再改」。`preview --frame N` 可以不切帧就对比另一帧。某一阶段要当真正的图像看时，`export png --scale 2`（或 4）再打开那张 PNG。不要自己编 RGB，也不要为一块平涂逐格 `paint` 或 `erase`。

*For a standing portrait such as a city person turning into a zombie, `new --width 64 --height 128` (or 96×128). Block the head, torso, and legs with `fill-rect`, `preview`, then correct the silhouette with `erase-rect` instead of hundreds of `erase` calls. Add cloth and skin as more rects, `preview` again, then `paint` only the details (eyes, teeth, torn cloth). Shift an arm or a dropped phone with `move-rect` or `copy-rect`, then `preview`. `duplicateFrame` for the zombie pose, then `setActiveFrame` / `frame select` the returned index — duplicate does not switch to the new frame — and repeat the same look-then-fix loop. `preview --frame N` compares another frame without selecting it. `export png --scale 2` (or 4) when a stage should be read as a real image. Do not invent RGB, and do not emit a `paint` or `erase` per pixel for a flat area.*

### 一笔撤销 A stroke as one undo

手势和撤销必须在同一次进程里：

*The gesture and the undo have to be the same process:*

```bash
pixelforge call --project hero.json \
  --request '{"tool":"beginGesture"}' \
  --request '{"tool":"paint","params":{"x":1,"y":2,"colorIndex":3}}' \
  --request '{"tool":"paint","params":{"x":2,"y":2,"colorIndex":3}}' \
  --request '{"tool":"endGesture"}'
```

要在进程结束前撤回这一笔，把 `--request '{"tool":"undo"}'` 加在同一条命令里。进程结束之后栈就没了；需要回头时先复制文件（`cp hero.json hero.json.bak`）。

*Add `--request '{"tool":"undo"}'` to that same command to revert the stroke before the process exits. After it exits the stack is gone. Copy the file first when you may need the previous pixels.*

### 走路帧 Walk frames

```bash
pixelforge call --project hero.json \
  --request '{"tool":"duplicateFrame","params":{"index":0}}' \
  --request '{"tool":"setActiveFrame","params":{"index":1}}' \
  --request '{"tool":"setFrameDelay","params":{"index":1,"delayMs":120}}' \
  --request '{"tool":"paint","params":{"x":0,"y":0,"colorIndex":1}}'
pixelforge export sheet --project hero.json --output hero-sheet.png
pixelforge export png --project hero.json --out-dir ./hero-frames
```

`duplicateFrame` 把像素和时长复制到下一格，返回的 `index` 是新帧。**它不会把当前帧改成这份拷贝。** 当前帧仍显示原来的像素：若它的下标在插入点或之后，下标会加一；否则下标不变。要画拷贝，先 `frame select` / `setActiveFrame` 到返回的下标。网页「复制」按钮会代为选中；CLI 和 `duplicateFrame` 不会。`frame add` 在末尾追加一帧 100 毫秒的空白帧，不改变当前帧。`export png` 写出 `pixelforge-000.png` 起的文件。`export png --scale K` 和 `export sheet --scale K` 在内核里最近邻放大（`K` ≥ 1，默认 1）。`export sheet` 不传 `--cols` 时网格是 `ceil(sqrt(帧数))` 列；`--cols N` 指定列数，一行一个方向时 `N` 就是这个方向的帧数。`scale` 不改变列数。约定见 [export.md](export.md)。

```bash
pixelforge preview --project hero.json --frame 0 --scale 4
pixelforge preview --project hero.json --frame 1 --scale 4
pixelforge diff --project hero.json --frame 0 --with 1 --scale 4
pixelforge export sheet --project hero.json --output hero-sheet.png --cols 4 --scale 2
```

`diff` 只读。stderr 上 `.` 是索引相同，`!` 是两边都有墨且不同，别的字形是唯一有墨的那边。`result.path` 的 PNG 是三格：frame、with、洋葱皮。洋葱皮不改像素。脚滑和穿插先看这张，再重画。

*`diff` is read-only. On stderr, `.` is the same index, `!` means both cells differ, and any other glyph is the only ink. The PNG at `result.path` has three panels: frame, with, onion. The onion does not change pixels.*

*`duplicateFrame` copies pixels and delay into the next slot and returns that index. It does not select the copy. The active frame keeps showing the same pixels: an index at or after the insertion point increases by one, and an earlier index stays put. `frame select` / `setActiveFrame` that index before painting the copy. The web duplicate button selects it; the CLI and `duplicateFrame` do not. `frame add` appends a blank 100 ms frame and does not change the active frame. `export png` writes `pixelforge-000.png` onward. `export png --scale K` and `export sheet --scale K` nearest-neighbor upscale in the kernel (`K` ≥ 1, default 1). `export sheet` uses `ceil(sqrt(frame count))` columns unless you pass `--cols N`. Scale does not change that count. See [export.md](export.md).*

同一批修改也可以拆成子命令。子命令各自是一次进程，撤销栈不跨进程：

*The same edits as separate commands. Each command is its own process, so the undo stack does not carry across them:*

```bash
pixelforge frame duplicate --project hero.json --index 0
pixelforge frame select --project hero.json --index 1
pixelforge frame delay --project hero.json --index 1 --delay-ms 120
pixelforge frame delete --project hero.json --index 1
```

### 图层 Layers

身体放在下层，装备放在上面的层。改装备不用重画身体。`preview` 能同时看见两层。`export png` 写出合成图。

*Put the body on the bottom layer and the equipment above it. Repainting the equipment does not redraw the body. `preview` shows both. `export png` writes the composite.*

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

`layer add` 在当前层上面插入一层透明索引，每一帧都有，并选中它。不写 `--name` 时，名字是从「当前层数 + 1」起第一个还没被占用的 `Layer N`。`layer list` 打印层栈。`layer select --index 0` 回到身体。`layer remove` 删当前层，也可以 `--index N`；最后一层不能删。`layer reorder --from 1 --to 0` 把装备挪到身体下面，`to` 是移动之后的下标。`fill-rect --layer 0` 画身体，但不切换当前层。新工程和旧工程都是一层，名字是 `Layer 1`。

*`layer add` inserts a transparent layer above the active one on every frame and selects it. Omit `--name` and the name is the next free `Layer N`, starting at one past the current count. `layer list` prints the stack. `layer select --index 0` goes back to the body. `layer remove` deletes the active layer, or `--index N`; the last layer is refused. `layer reorder --from 1 --to 0` moves gear under the body. `fill-rect --layer 0` paints the body without selecting it. A new project and an old project are one layer named `Layer 1`.*

### 调色板 Palette

```bash
pixelforge palette add --project hero.json --r 20 --g 30 --b 40
pixelforge palette set --project hero.json --index 1 --r 10 --g 20 --b 30
pixelforge palette lock --project hero.json
pixelforge palette unlock --project hero.json
pixelforge palette import --project hero.json --mode merge --file colors.gpl
```

`palette add` 的结果里有新索引，画笔用那个索引。`import` 的 `mode` 是 `replace` 或 `merge`，文件是 `.gpl` 或十六进制列表。色板锁定时导入会被拒绝。

*`palette add` returns the new index. Paint that index. `import` takes `replace` or `merge` and a `.gpl` file or a hex list. A locked palette refuses the import.*

### 查看与原始调用 Inspect and raw calls

```bash
pixelforge inspect --project hero.json
pixelforge tools
pixelforge call --project hero.json --request-file edits.json
```

`edits.json` 可以是一个 JSON 对象、一个 JSON 数组，或每行一个 JSON 值。也可以把 JSON 从 stdin 送进 `pixelforge call --project hero.json`。每个 `--request` 是一个请求对象，原样交给 `Document::call_tool`。

工具名、参数和 `error.code` 见 [tool-api.md](tool-api.md)。

*`edits.json` may be one JSON object, a JSON array, or one JSON value per line. Stdin works the same way: `pixelforge call --project hero.json`. Each `--request` is one request object and is passed to `Document::call_tool`. Names, params, and `error.code` values are in [tool-api.md](tool-api.md).*
