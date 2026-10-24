# 给外部 Agent 的 skill Agent skill

产品路径是：**仓库外的 Agent → 本地 `pixelforge` CLI → Rust 内核 Tool API**。Agent 不改界面上的像素缓冲，也不在这个仓库里做多轮聊天。网页里的一轮面板是可选演示，见 [ai-driver.md](ai-driver.md)。[#14](https://github.com/starcloud530/pixelforge-ai/issues/14) 那种嵌在应用里的多轮对话不是产品。MCP 不在这一阶段；以后若要接，只做这支 CLI 的薄封装。

Skill 补观改流程，不代替专业像素画师的审美。阶段走完、清单通过，图仍然可能不好看。

*The product path is an agent outside this repository, then the local `pixelforge` CLI, then the Rust kernel Tool API. The agent does not patch the pixel buffer the view draws, and this repository does not host a multi-turn chat. The one-turn panel in the web shell is an optional demo; see [ai-driver.md](ai-driver.md). An embedded multi-turn chat like [#14](https://github.com/starcloud530/pixelforge-ai/issues/14) is not the product. MCP is out of this stage. A later wrapper, if any, would be a thin layer over this CLI. Skills add a look-then-revise loop. They do not replace a pixel artist's judgment.*

## 技能表 Skills

Cursor 按每个 `SKILL.md` 的 `description` 选择技能。正文只在 `.cursor/skills/<name>/SKILL.md`。本仓库不复制到 `.agents/skills/` 或 `.claude/skills/`。Codex、Claude Code 读 `AGENTS.md` / `CLAUDE.md` 后打开下表里的文件。某个运行时若只扫描 Agent Skills 标准目录，把 `.cursor/skills` 链过去，不要另写一份正文。

创作类 skill 先遵守 `pixelforge` 的索引色不变量。命令语义以 `pixelforge` 和本文件的速查为准；剪影、四向、分层、挑错的步骤以对应 skill 为准。

*Cursor matches `description` in `.cursor/skills/<name>/SKILL.md`. This repository does not keep a second copy under `.agents/skills/` or `.claude/skills/`. Codex and Claude Code open the files below from `AGENTS.md` and `CLAUDE.md`. Creative skills follow the `pixelforge` invariants first.*

| Skill | 路径 | 何时加载 | 说明 |
| --- | --- | --- | --- |
| `pixelforge` | [.cursor/skills/pixelforge/SKILL.md](../.cursor/skills/pixelforge/SKILL.md) | 用 pixelforge、改工程、CLI、调色板、导出 | 核心。安装、索引色不变量、退出码、`inspect` / `tools` / `call`。其它创作 skill 先遵守这里。 |
| `pixelforge-draw` | [.cursor/skills/pixelforge-draw/SKILL.md](../.cursor/skills/pixelforge-draw/SKILL.md) | 从零画角色、立绘、剪影、选画布 | 强制剪影（单色）→ 色块 → 细节。每阶段 `preview --scale` 加 ASCII。禁止一像素刷屏。读图要查破洞、头身、体积断点，禁止把完成度说高。 |
| `pixelforge-sprite-4dir` | [.cursor/skills/pixelforge-sprite-4dir/SKILL.md](../.cursor/skills/pixelforge-sprite-4dir/SKILL.md) | 四向走 + 闲、精灵表、脚滑、动画标签 | 行是朝向，列是 idle 加 walk。`export sheet --cols`。标签名是 `idle_down` / `walk_down`。`--atlas` 写帧矩形和标签。跨帧 `copy-rect`。`diff` 查脚滑。`hflip` 之后修高光。 |
| `pixelforge-layers` | [.cursor/skills/pixelforge-layers/SKILL.md](../.cursor/skills/pixelforge-layers/SKILL.md) | 分层、装备、身体、四肢 | body / gear / limbs。合成预览、`--layer`、`layer flatten`。 |
| `pixelforge-critique` | [.cursor/skills/pixelforge-critique/SKILL.md](../.cursor/skills/pixelforge-critique/SKILL.md) | 检查、评审、问看起来怎么样 | 只读清单：剪影、头身、侧向深度、腿体积、脚接触、色阶。输出必须列缺陷，不能只说看起来不错。 |

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
- `palette lock` 之后仍可用 `paint` 和 `fillRect` 画已有索引，但不能改色、追加、导入或 `remap`。`apply-png` 只写索引，锁定时仍可用。
- 画笔默认拒绝越界索引（`indexPolicy: reject`）。`palette enforce --mode clamp` 把越界索引写成最后一项不透明色。索引 0 仍然拒绝。`import` 的 `replace` 不看这个策略。
- 不要手改工程 JSON 去塞一个颜色。越界索引会让文件打不开。不要把扩散图写回工程。
- 一次进程是一份文档。同一次 `pixelforge call` 里的多个 `--request` 共享撤销栈。工程文件不保存撤销栈，下一个进程是空栈，和在编辑器里重新打开这份文件一样。对刚打开的文件执行 `pixelforge undo` 会得到 `NothingToUndo`。
- 失败的那一次调用不改文档。一批请求里，失败之前的成功调用会保存，失败之后的不再执行。
- 导出在内核里把索引解析成 RGBA。不要自己给 PNG 重新上色。`diff` / `diffFrames` 画出洋葱皮视图，不把它写回像素。`flip` 只搬索引，不改高光颜色。

*Cells store palette indices. Index 0 is transparent. `paint` and `fillRect` write an existing index, never RGB. `eraseRect` writes index 0. `copyRect` and `moveRect` copy stored indices, including 0. A stored `selection` makes those paint tools skip cells outside the rectangle; the file omits the field when the selection is clear. Every frame shares one layer stack. Index 0 is the bottom. Drawing writes the active layer, or `--layer`, and leaves the other layers alone. `preview`, `diff`, and export show the composite: index 0 shows the layer underneath, and any other index covers it. Palette alpha is not blended between layers. `layer flatten` writes that composite into one layer named `Layer 1`. A new project ships the studio palette, indices 1 through 8. Add or recolor a palette entry before painting a new color. A locked palette still accepts paint and rectangle fill of an existing index and refuses recolor, append, import, and remap. Out-of-range paint indices are rejected unless `indexPolicy` is `clamp`. Do not hand-edit the project JSON to plant a color, and do not write a diffusion image back into the project.*

*One process is one document. Several `--request` flags on one `pixelforge call` share the undo stack. The project file does not store that stack, so the next process starts empty, the same as reopening the file. `pixelforge undo` on a freshly opened file returns `NothingToUndo`. A refused call does not change the document. Earlier calls in that batch are saved; later calls are not run. Export resolves indices in the kernel. `diff` / `diffFrames` draws an onion view and does not write it into pixels. `flip` moves indices only and does not recolor highlights.*

## 命令速查 Command reference

画布尺寸、三阶段画法和完成度用词在 [pixelforge-draw](../.cursor/skills/pixelforge-draw/SKILL.md)。四向行序在 [pixelforge-sprite-4dir](../.cursor/skills/pixelforge-sprite-4dir/SKILL.md)。层怎么摆在 [pixelforge-layers](../.cursor/skills/pixelforge-layers/SKILL.md)。只读清单在 [pixelforge-critique](../.cursor/skills/pixelforge-critique/SKILL.md)。下面是和当前 CLI 一致的命令。

*`resize` copies stored indices into a new canvas and does not invent detail. Omit both offsets to center; pass `--offset-x` and `--offset-y` together. The size table lives in pixelforge-draw.*

### 空白工程 Blank project

```bash
pixelforge new --width 16 --height 16 --output hero.json
```

stdout 是 `inspect` 的响应。文件是 version 1 `pixelforge-project`。`new` 不会覆盖已经存在的文件。16×16 只适合俯视小体型或图标，不适合站立立绘。

*Stdout is the `inspect` response. The file is version 1 `pixelforge-project`. `new` refuses to replace a file that already exists.*

### 只改头盔 Edit only the helmet

选区是工程文件里的一块矩形，不是套索，也不是魔棒。没有选区时文件不写 `selection`。下一次 `pixelforge` 进程会读到它；撤销栈仍然不保存。选区不绑定层，也不改 `activeLayer`。头盔在装备层上时，用 `--layer` 指向那一层：选区外任何层都不写，选区内只写你指定的层。

*The selection is one rectangle in the project file. It is not a lasso and not a magic wand. A clear selection omits `selection`. The next process loads it; the undo stack is still not stored. The rectangle is not tied to a layer and does not change `activeLayer`. Point `--layer` at the gear layer so only that layer is written, and only inside the helmet.*

```bash
pixelforge layer add --project hero.json --name gear
pixelforge fill-rect --project hero.json --layer 0 --x 8 --y 16 --width 16 --height 24 --color-index 3
pixelforge fill-rect --project hero.json --layer 1 --x 10 --y 4 --width 12 --height 10 --color-index 4
pixelforge selection set --project hero.json --x 10 --y 4 --width 12 --height 10
pixelforge replace-index --project hero.json --layer 1 --from-index 4 --to-index 6
pixelforge fill-rect --project hero.json --layer 1 --x 0 --y 0 --width 32 --height 48 --color-index 2
pixelforge selection clear --project hero.json
```

`replace-index` 只把装备层上、选区内的索引 4 换成 6。接着那次整画布 `fill-rect` 也只落在头盔矩形里。身体层的索引 3 不动。`move-rect` 的源和目标必须都整块在选区内，否则 `SelectionProtected`，一格都不写。`remap` 和 `apply-png` 不看选区。

*`replace-index` changes index 4 to 6 on the gear layer inside the rectangle. The following full-canvas `fill-rect` also lands only inside the helmet. Index 3 on the body layer stays. `move-rect` returns `SelectionProtected` and writes nothing unless both rectangles lie inside the selection. `remap` and `apply-png` ignore it.*

### 画、填、擦、复制、移动 Paint, fill, erase, copy, move

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

`fill-rect` 在当前帧上用一个调色板索引填一块轴对齐矩形。`x`、`y` 是左上角。`width` 和 `height` 至少为 1，整块必须落在画布里；只要有一格会出界，一格都不写。色板锁定时仍然可以填。`call` 里的工具名是 `fillRect`。

*`fill-rect` writes one palette index into an axis-aligned rectangle on the active frame. `x` and `y` are the top-left cell. `width` and `height` are at least 1, and the whole rectangle must sit inside the canvas; if any cell would fall outside, nothing is written. A locked palette still allows the fill. The tool name inside `call` is `fillRect`.*

`erase-rect` 把同样的矩形清成索引 0（`eraseRect`）。`copy-rect` 原样复制索引，包括 0（`copyRect` 的参数是 `srcX`、`srcY`、`width`、`height`、`dstX`、`dstY`，外加可选的 `fromFrame`、`toFrame`）。`move-rect` 在同一步撤销里搬走这些索引，并把不在目标里的源格子清成 0（`moveRect`）。不写 `--from-frame` / `--to-frame` 时仍在当前帧。写了也不改变 `activeFrame`。跨帧的 `move-rect` 会把整个源矩形清掉。源或目标出界时，一格都不写。`--layer` 只读写那一层，不切换当前层。

*`erase-rect` clears that rectangle to index 0 (`eraseRect`). `copy-rect` copies indices as they are, including 0 (`copyRect`: `srcX`, `srcY`, `width`, `height`, `dstX`, `dstY`, optional `fromFrame`, optional `toFrame`). `move-rect` moves those indices and clears source cells outside the destination, as one undo step (`moveRect`). Omit `--from-frame` / `--to-frame` to stay on the active frame. Those flags do not change `activeFrame`. A cross-frame move clears the whole source rectangle. If the source or the destination would leave the canvas, nothing is written. `--layer` does not select the layer.*

```bash
pixelforge copy-rect --project hero.json --from-frame 0 --to-frame 2 \
  --src-x 16 --src-y 20 --width 32 --height 36 --dst-x 16 --dst-y 20
pixelforge hflip --project hero.json --frame 2
pixelforge flip --horizontal --project hero.json --x 0 --y 40 --width 64 --height 24
pixelforge resize --project hero.json --width 64 --height 64
pixelforge resize --project hero.json --width 64 --height 96 --offset-x 0 --offset-y 0
```

`hflip` 就是 `flip --horizontal`，不必再写 `--horizontal`。`flip` 必须写 `--horizontal`，否则退出码 `2`。两者只做索引镜像（`x` 收到 `origin + width - 1 - x`），不改颜色。不写矩形就是整帧；写矩形时四个边要齐。`resize` 仍写出 version 1 工程。不写偏移就居中；两个偏移都写就指定旧原点。越出新画布的格子被裁掉。一步撤销回到原来的画布。

*`hflip` mirrors indices and does not need `--horizontal`. `flip` requires `--horizontal`. Neither recolors. `resize` keeps the version 1 project. Omit the offsets to center. Content outside the new canvas is clipped.*

字形：`.` 是透明（索引 0），`1`–`9` 是这些索引，`A`–`Z` 是 10–35，`a`–`z` 是 36–61，`*` 是 62 及以上（stderr 的 legend 会写出那些索引和 `#rrggbb`）。`preview` 子命令按 `--scale` 写出彩色 PNG，路径在 `result.path`（默认工程旁边的 `<stem>.preview.png`，或 `--output`）。`--scale` 是 ≥ 1 的整数，默认 1。`preview --frame N` 不改 `activeFrame`，也不改工程文件。`result.frame` 是这张图，`result.activeFrame` 仍是文档的当前帧。会改像素的命令上的 `--preview` 只有 ASCII，不写彩色 PNG。

`diff` 只读。stderr 上 `.` 是索引相同，`!` 是两边都有墨且不同，别的字形是唯一有墨的那边。`result.path` 默认是 `<stem>.diff.png`，三格：frame、with、洋葱皮。洋葱皮不改像素。

`export png --scale K` 和 `export sheet --scale K` 在内核里做最近邻放大（`K` 是 ≥ 1 的整数，默认 1）。每个索引变成 `K × K` 的块，再由内核解析颜色。`export sheet` 不传 `--cols` 时列数是 `ceil(sqrt(帧数))`；`--cols N` 指定列数，行数是 `ceil(帧数 / N)`。`scale` 不改变列数。见 [export.md](export.md)。

*Glyphs: `.` is transparent, `1`–`9` and `A`–`Z` / `a`–`z` are indices, `*` is index 62 or above. `preview` writes a color PNG at `result.path`. `--preview` on a mutating command is ASCII only. `diff` is read-only. `export sheet --cols N` sets the column count; scale does not.*

平涂优先 `fillRect`，挖剪影优先 `eraseRect`，挪姿势或道具优先 `copyRect` / `moveRect`。一次 `pixelforge call` 里放多个 `--request`。一块矩形已经是一步撤销。好几块要一起撤销时，用 `beginGesture` / `endGesture` 包住。

```bash
pixelforge call --project hero.json \
  --request '{"tool":"beginGesture"}' \
  --request '{"tool":"fillRect","params":{"x":8,"y":4,"width":16,"height":20,"colorIndex":1}}' \
  --request '{"tool":"fillRect","params":{"x":10,"y":24,"width":12,"height":28,"colorIndex":2}}' \
  --request '{"tool":"endGesture"}' \
  --preview
```

### 一笔撤销 A stroke as one undo

手势和撤销必须在同一次进程里：

```bash
pixelforge call --project hero.json \
  --request '{"tool":"beginGesture"}' \
  --request '{"tool":"paint","params":{"x":1,"y":2,"colorIndex":3}}' \
  --request '{"tool":"paint","params":{"x":2,"y":2,"colorIndex":3}}' \
  --request '{"tool":"endGesture"}'
```

要在进程结束前撤回这一笔，把 `--request '{"tool":"undo"}'` 加在同一条命令里。进程结束之后栈就没了；需要回头时先复制文件（`cp hero.json hero.json.bak`）。

*Add `--request '{"tool":"undo"}'` to that same command to revert the stroke before the process exits. After it exits the stack is gone. Copy the file first when you may need the previous pixels.*

### 帧与导出 Frames and export

```bash
pixelforge call --project hero.json \
  --request '{"tool":"duplicateFrame","params":{"index":0}}' \
  --request '{"tool":"setActiveFrame","params":{"index":1}}' \
  --request '{"tool":"setFrameDelay","params":{"index":1,"delayMs":120}}'
pixelforge tag add --project hero.json --name walk_down --from 1 --to 3 --loop forward
pixelforge tag set --project hero.json --name walk_down --loop pingpong --rename walk_south
pixelforge tag remove --project hero.json --name walk_south
pixelforge export sheet --project hero.json --output hero-sheet.png --cols 4 --scale 2 --atlas hero-sheet.json
pixelforge export png --project hero.json --out-dir ./hero-frames --scale 2
```

`duplicateFrame` 把像素和时长复制到下一格，返回的 `index` 是新帧。**它不会把当前帧改成这份拷贝。** 当前帧仍显示原来的像素：若它的下标在插入点或之后，下标会加一；否则下标不变。要画拷贝，先 `frame select` / `setActiveFrame` 到返回的下标。网页「复制」按钮会代为选中；CLI 和 `duplicateFrame` 不会。`frame add` 在末尾追加一帧 100 毫秒的空白帧，不改变当前帧。`export png` 写出 `pixelforge-000.png` 起的文件。时长范围是 `1..=65535`。

*duplicateFrame does not select the copy. `frame add` appends a blank 100 ms frame. `export png` writes `pixelforge-000.png` onward. `export sheet --cols N` keeps one facing on one row when N is the frame count of that facing. `--atlas FILE` writes `pixelforge-atlas` JSON beside the sheet. Omit it and the export result stays the PNG fields. Tag names for a four-direction sheet live in pixelforge-sprite-4dir.*

`tag add` / `addTag` 给一段闭区间起名，并写上 `loop`（`forward`、`reverse`、`pingpong`、`once`，缺省 `forward`）。`tag set` 改区间、循环或改名。`tag remove` 只删标签。没有标签时工程文件不写 `tags`。`inspect` 和 `scene` 始终带 `tags`，空的时候是 `[]`。标签不是 cel 链接：复制帧仍然复制像素。整条时间轴的循环预览不改成按标签播放。插帧、删帧会在同一步撤销里挪已经存在的标签；复制某个标签的最后一帧时，新帧不自动进这个标签。四向名字在帧排好之后再打，见 [pixelforge-sprite-4dir](../.cursor/skills/pixelforge-sprite-4dir/SKILL.md)。

*`tag add` names an inclusive range. `loop` is `forward`, `reverse`, `pingpong`, or `once`, and defaults to `forward`. `tag set` changes the range, the loop, or the name. `tag remove` drops the tag and leaves the frames. A project with no tags omits `tags`. `inspect` and `scene` always include `tags`, and an empty list is `[]`. Tags are not cel links. Playback of the whole timeline is unchanged. Adding or deleting a frame moves existing tags in that same undo step. Duplicating the last frame of a tag does not pull the copy into the tag. Name a four-direction sheet after the frames exist.*

同一批修改也可以拆成子命令。子命令各自是一次进程，撤销栈不跨进程：

```bash
pixelforge frame duplicate --project hero.json --index 0
pixelforge frame select --project hero.json --index 1
pixelforge frame delay --project hero.json --index 1 --delay-ms 120
pixelforge frame delete --project hero.json --index 1
```

### 调色板 Palette

```bash
pixelforge palette add --project hero.json --r 20 --g 30 --b 40
pixelforge palette set --project hero.json --index 1 --r 10 --g 20 --b 30
pixelforge palette lock --project hero.json
pixelforge palette unlock --project hero.json
pixelforge palette import --project hero.json --mode merge --file colors.gpl
pixelforge palette enforce --project hero.json --mode clamp
pixelforge palette remap --project hero.json --max-colors 8
pixelforge palette remap --project hero.json --file colors.hex
pixelforge palette apply-png --project hero.json --file sprite.png
```

`palette add` 的结果里有新索引，画笔用那个索引。`import` 的 `mode` 是 `replace` 或 `merge`，文件是 `.gpl` 或十六进制列表。色板锁定时导入和 `remap` 会被拒绝。

`enforce --mode` 是 `reject`（默认）或 `clamp`。`clamp` 写入工程文件；旧文件没有 `indexPolicy` 时按 `reject` 读。只有索引被钳制时，`paint` / `fillRect` 的结果才多 `requestedIndex` 和 `colorIndex`。

`remap` 在 `--max-colors N` 和 `--file` 里只能给一个。`--max-colors` 是保留的不透明色上限。`--file` 用和 `import` 相同的文本做目标盘，像素按平方 RGB 距离重映射，不平均出新颜色。`apply-png` 把一张与画布同尺寸的 PNG 压进当前调色板：不缩放、不切表、不追加颜色，alpha 小于 128 写成索引 0。锁定色板时仍可压盘。

冲突报告在 `result` 里：`misses` 是没精确命中的颜色，`merges` 是并进了哪项近似色（`exact: true` 表示 RGB 相同、只是索引被压紧），`pixelsChanged` 是索引变了的格子数，`dropped` 是量化时丢掉的未使用色。没有 Lospec 在线浏览器。

```json
{ "tool": "setIndexPolicy", "params": { "mode": "reject" } }
{ "tool": "remapPalette", "params": { "maxColors": 8 } }
{ "tool": "remapPalette", "params": { "text": "ff0000\n00ff00\n" } }
{ "tool": "applyPng", "params": { "png": "<base64>", "frame": 0 } }
```

*`palette add` returns the new index. Paint that index. `import` takes `replace` or `merge` and a `.gpl` file or a hex list. A locked palette refuses import and remap. `enforce` sets `reject` (default) or `clamp`. `remap` takes `--max-colors` or `--file`. `apply-png` crushes one canvas-sized PNG onto the current palette. Read `misses`, `merges`, and `pixelsChanged`. There is no Lospec browser.*

### 查看与原始调用 Inspect and raw calls

```bash
pixelforge scene --project hero.json
pixelforge inspect --project hero.json
pixelforge paint --project hero.json --x 1 --y 2 --color-index 3 --dirty
pixelforge tools
pixelforge call --project hero.json --request-file edits.json
```

`scene` 是紧凑摘要：画布、`activeFrame` / `activeLayer`、`selection`（没有选区时是 `null`）、图层（`visible` 恒为 true，`opacity` 恒为 null）、`paletteHash`、帧数、`delayMs` 和 `tags`（没有标签时是 `[]`）。不返回像素。怎么读这些字段见 [pixelforge skill](../.cursor/skills/pixelforge/SKILL.md) 的 “How to read state”。`--dirty` 只在绘制命令上多返回 `changed` 和脏区；不写它，结果和以前一样。

*`scene` is the compact summary: canvas, active frame and layer, layers, `paletteHash`, frame count, delay, and `tags` (an empty array when there are none). It does not return pixels. `--dirty` adds `changed` and a dirty box on a drawing command. Omit it and the result stays as it was.*

`edits.json` 可以是一个 JSON 对象、一个 JSON 数组，或每行一个 JSON 值。也可以把 JSON 从 stdin 送进 `pixelforge call --project hero.json`。每个 `--request` 是一个请求对象，原样交给 `Document::call_tool`。

工具名、参数和 `error.code` 见 [tool-api.md](tool-api.md)。

*`edits.json` may be one JSON object, a JSON array, or one JSON value per line. Stdin works the same way: `pixelforge call --project hero.json`. Each `--request` is one request object and is passed to `Document::call_tool`. Names, params, and `error.code` values are in [tool-api.md](tool-api.md).*

## 创作流程放在哪 Workflow skills

流程正文不在这里重复。摘要如下，和对应 skill 一致。

*The normative steps are the skills. This list is the same contract, shortened.*

### 从零画角色 pixelforge-draw

剪影只用一个索引，用 `fill-rect` / `erase-rect` 切外形。然后才是皮肤、衣服、金属的色块。眼睛、牙、裂口才用 `paint`。每一阶段跑 `pixelforge preview --project … --scale 4`，读 ASCII，并打开 `result.path`。读图时查破洞、头身各占几行、腿和手臂有没有断掉。完成度只写阶段名（剪影、色块、细节），不要写成高完成度。

站立立绘优先 64×128 或 96×128。站立立绘做成 48×64 是示意图。俯视小体型是 16–32。常见四向游戏精灵大约 32–64。更高细节的俯视或全身精灵用 96×96 或 128×128，需要拉高时用 96×128 或 64×128。这些尺寸都在边长 `1..=256` 里，默认 `new` 仍是 16×16。更大只是格子更多，不会自动更好看。96 和 128 的 ASCII 是整帧，阶段检查看 `preview --scale` 的彩色 PNG，ASCII 只抽局部。16 帧走表不要默认用立绘的高度。`resize` 只搬已有索引。变身用 `duplicateFrame` 之后必须 `frame select` 返回的下标。

### 四向走 + 闲 pixelforge-sprite-4dir

行是朝向（下、左、右、上），列是 idle 然后 walk。`export sheet --cols N` 的 `N` 等于每行帧数。跨帧用 `copy-rect --from-frame --to-frame`。脚是否滑看 `diff --frame A --with B --scale 4`：地面行上横移的 `!` 是脚滑。俯视小体型把接触格留在原位，动上面的身体。`hflip` 只搬索引，不改颜色。高光格子跟着身体翻到另一侧，画面上的光源还在原来的一侧，再用 `paint` 把高光画回去。

### 分层 pixelforge-layers

`body` 在下标 0，需要单独挪的四肢可以加 `limbs`，装备放 `gear`。`layer add` 选中新层。`fill-rect --layer N` 不切换当前层。`preview` 看合成，`preview --layer N` 看一层。`export png` 写合成。`layer flatten` 压成名为 `Layer 1` 的单层。同一格不要在两层各画一个非 0 索引。

### 只读检查 pixelforge-critique

不改文件。用 `inspect`、`preview --scale 4`、`diff`。输出必须逐条写剪影、头身、侧向深度、腿体积连续、脚接触、色阶。有缺陷就列出来。没有缺陷的条写「未发现」并附上帧号或字形。不要只回答「看起来不错」。打不开彩色 PNG 时色阶写「未核」。
