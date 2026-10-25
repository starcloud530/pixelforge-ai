---
name: pixelforge
description: >-
  Edit PixelForge indexed-pixel projects with the pixelforge CLI and the kernel
  Tool API. Use when the user says 用 pixelforge, 改工程, or CLI, or when
  installing or running pixelforge, creating or changing a pixelforge-project,
  painting or erasing cells, filling or erasing a rectangle, copying or moving
  a rectangle, editing the palette, locking, quantizing, or remapping it,
  crushing a PNG onto the palette, importing an indexed .aseprite file,
  flipping a rectangle horizontally or vertically, rotating it by 90, 180, or
  270 degrees, naming parts and anchors, translating, rotating, or flipping a
  named part on the pixel grid, keyframing that part's rotation, translation,
  or flip and sampling the curve, solving a follow, attach, or pin constraint
  so a hand, a gun butt, or a planted foot tracks an anchor, baking those
  sampled poses into editable pixel frames, adding or
  duplicating frames, undoing,
  running inspect, scene, tools, or call, or exporting a PNG sequence or sprite sheet.
  Take the shortest edit path: duplicate a same-pose frame and edit the diff,
  merge adjacent same-color fillRects into one taller rect, and count every
  drawing request inside one batched call.
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

画角色、立绘、剪影：同时读 `pixelforge-draw`。四向走加待机的精灵表：同时读 `pixelforge-sprite-4dir`。身体、装备、四肢分层：同时读 `pixelforge-layers`。只读挑错：读 `pixelforge-critique`，不要改像素。多帧、同姿只改差、或要缩短 `fillRect` 次数时，读下面的「最少编辑路径」。

*Use this skill for every project edit. Read the matching creative skill for how to stage the picture. Read Shortest edit path before a multi-frame edit.*

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
- 矩形选区写在工程文件的 `selection` 里。没有选区时省略这个字段。它不是套索，也不是魔棒。有选区时，`paint` / `fillRect` / `erase` / `eraseRect` / `copyRect` / `flipHorizontal` / `flipVertical` / `rotate` / `replaceIndex` 不改选区外的格子。翻转和旋转还要求读到的源格也在选区内。`moveRect` 的源和目标必须都在选区内，否则 `SelectionProtected`。选区不改 `activeLayer`。只改头盔时用 `--layer` 指向装备层。`remapPalette` 和 `applyPng` 不看选区。
- 命名部件写在工程文件的 `parts` 里。没有部件时省略。部件不是层，也不是像素缓冲。每个部件有画布上的原点 `(x, y)`，那一格是本地 `(0, 0)`。可选 `layer` 指向像素所在的层，不切换当前层。锚点的 `x`、`y` 是相对原点的像素格，范围 `-256..=256`。静止时锚点的画布位置是原点加这个偏移。矩形翻转和旋转不搬锚点。`resize` 把原点按同一偏移挪走，原点离开新画布时这个部件被去掉。部件变换见「部件变换」。关键帧曲线见「部件曲线」：它按帧下标采样，不写像素。约束见「约束」：`constraint solve` 才把从动部件的锚点跟到目标，挂接层的墨水在 `--apply` 时一起平移。空的约束列表不写进文件。`bake` 把曲线和约束采样写进一段帧的像素里，源帧的像素留着，曲线和约束也不删。见「烘焙」。
- 失败的那一次调用不改文档。一批请求里，失败之前的成功调用会保存，失败之后的不再执行。
- 导出在内核里把索引解析成颜色。不要自己给 PNG 重新上色。`diff` / `diffFrames` 画出洋葱皮，不写回像素。`flip` / `hflip` / `vflip` / `rotate` 只搬索引，不改高光颜色。左右镜像之后要修高光时见 `pixelforge-sprite-4dir`。

*Cells store palette indices. Index 0 is transparent. Drawing writes the active layer, or `--layer`, and does not select that layer. `preview`, `diff`, and export show the composite. A locked palette still accepts paint of an existing index and refuses recolor, append, import, and remap. Out-of-range paint indices are rejected unless `indexPolicy` is `clamp`. Do not hand-edit the project JSON. One process is one document; the file does not store the undo stack. A rectangular `selection` is stored in the project and omitted when clear. Paint tools then skip cells outside it. There is no lasso. `flip` moves indices and does not recolor. `bake` writes sampled poses into pixel frames and leaves the curves in the file.*

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

`scene` 的 `result`：`width`、`height`、`activeFrame`、`activeLayer`、`selection`（矩形或 `null`）、`layers`（`index`、`name`、`visible`、`opacity`）、`palette`、`paletteCount`、`paletteHash`、`paletteLocked`、`indexPolicy`、`frameCount`、`delayMs`、`frames`（每帧 `index` 和 `delayMs`）、`tags`（`index`、`name`、`from`、`to`、`loop`；没有标签时是 `[]`）、`parts`（`index`、`name`、`x`、`y`、`layer`、`anchors`；没有部件时是 `[]`）、`curves`（`index`、`part`、`channel`、`keys`；没有曲线时是 `[]`）、`constraints`（`index`、`name`、`kind`、`part`、`anchor`、`target`、`targetAnchor`、`x`、`y`；没有约束时是 `[]`）。没有 `pixels`。要格子用 `inspect` 或 `preview`。`indexPolicy` 默认 `reject`。没有选区时 `selection` 是 `null`，工程文件省略该字段。没有标签时工程文件也省略 `tags`。没有部件时工程文件也省略 `parts`。没有曲线时工程文件也省略 `curves`。没有约束时工程文件也省略 `constraints`。

`visible` 恒为 `true`，`opacity` 恒为 `null`。内核没有逐层隐藏，也没有逐层透明度。非 0 索引盖住下层。

`paletteHash` 是 FNV-1a 64 位，16 位小写十六进制。初值 `14695981039346656037`，乘数 `1099511628211`，按索引把每一项的 `r`、`g`、`b`、`a` 送进去。颜色和顺序不变，哈希就不变。`inspect` 的 `result.paletteHash` 是同一个值。

`--dirty`（Tool API 是 `reportDirty: true`）只加在 `paint`、`erase`、`fillRect`、`eraseRect`、`copyRect`、`moveRect`、`flipHorizontal`、`flipVertical`、`rotate`、`resizeCanvas`、`translatePart`、`rotatePart`、`flipPart` 上。结果多 `changed`、`dirty`（`{x,y,width,height}`，没变化是 `null`）、`regions`（每个改过的帧和层一条）。`resize` 再多 `clipped`：新画布上已经没有的坐标里，原来非 0 的格子数。盒子包住的是索引真的变了的格子，不是请求里的整块矩形。不写 `--dirty`，这些字段就不出现，旧的 `result` 保持原样。这不是命令日志。`importAseprite` 不接受这个字段。

`preview` 除了 `rows`，还有 `ink`（非 0 格数）、`bounds`、`indices`（每个索引用了几次）、`pixels`（这一张视图的索引，行优先）。字形 `*` 把 62 及以上折成同一个字符，读索引用 `pixels`。

`diff` 除了 `diffRows` 和 `changed`，还有 `bounds`、`kinds`（`same`、`differ`、`onlyFrame`、`onlyWith`）、`changes`（`x`、`y`、`left`、`right`）。`!` 不带两边的索引，索引在 `changes` 里。

*Read `scene` before painting. It does not write the file and it does not return the pixel buffer. `visible` is always true and `opacity` is always null. `tags` is the named frame ranges, or `[]`. `parts` is the named parts and anchors, or `[]`. `curves` is the part-parameter tracks, or `[]`. `constraints` is the follow, attach, and pin list, or `[]`. `paletteHash` is FNV-1a 64 of each entry's `r,g,b,a` in index order. `--dirty` adds `changed`, `dirty`, and `regions` on the drawing tools and on part translate, rotate, and flip; omit it and those fields stay absent. `preview` adds `ink`, `bounds`, `indices`, and `pixels`. `diff` adds `bounds`, `kinds`, and `changes`. This is not a command log.*

## Pictures, sheets, and canvas commands

这些是命令语义。剪影怎么画、四向怎么排、层怎么命名，不在这里。

`preview` 子命令只读。它把 ASCII 打到 stderr，并写出最近邻彩色 PNG。`--scale K` 与导出相同（整数 ≥ 1，默认 1）。路径在 `result.path`（默认工程旁边的 `<stem>.preview.png`，或 `--output`）。`--frame N` 画第 N 帧，不改 `activeFrame`，也不改文件。`--layer N` 只画那一层，不选中它。`result.frame` 是这张图，`result.activeFrame` 仍是文档的当前帧。字形：`.` 透明（索引 0），`1`–`9` 是这些索引，`A`–`Z` 是 10–35，`a`–`z` 是 36–61，`*` 是 62 及以上（stderr 的 legend 写出索引）。会改像素的命令上的 `--preview` 只有 ASCII，不写这张彩色 PNG。

`diff` 只读。`--frame A --with B`。stderr：`.` 索引相同，`!` 两边都有墨且不同，其它字形是唯一有墨的那边。彩色 PNG 在 `result.path`（默认 `<stem>.diff.png`），三格：frame、with、洋葱皮。洋葱皮不改像素。`--scale` 同上。`--layer N` 只比较那一层。

`hflip` 等于 `flip --horizontal`，不需要再写 `--horizontal`。`vflip` 等于 `flip --vertical`。`flip` 必须恰好写其中一个方向，否则退出码 `2`。两者都只搬索引，不改颜色。水平：`x` 收到 `origin + width - 1 - x`。垂直：`y` 收到 `origin + height - 1 - y`。`rotate --degrees` 是 `90`、`180` 或 `270`，顺时针，只搬索引。90 和 270 要求正方形矩形，不改变画布尺寸；不是正方形时是 `RotateSquare`，一格都不写。不写矩形就是整帧；写矩形时 `x`、`y`、`width`、`height` 要齐。`--frame N` 不切换当前帧。`--layer N` 只改那一层。有选区时，目标格和源格都在选区内才写入。一步撤销。

`resize --width W --height H` 把每一帧、每一层的索引导进新画布，格式仍是 version 1。不写偏移就居中；`--offset-x` 和 `--offset-y` 要成对出现。越出新画布的格子被裁掉。放大不会凭空补细节。边长 `1..=256`。96×96、128×128，以及需要时的 96×128、64×128，都在这条边长里，是 `pixelforge-draw` 的正式档，不是另一条命令。默认 `new` 仍是 16×16。`preview` 的 ASCII 按整帧输出，一格一个字形，不裁行。96 和 128 的整张格子很长，阶段检查看 `preview --scale` 的彩色 PNG，ASCII 抽局部行。画布该选哪一档见 `pixelforge-draw`。

`export png --scale K` 和 `export sheet --scale K` 在内核里最近邻放大，再解析调色板。只为看颜色时，用这个或 `preview --scale`，不要再用外部工具放大。`export sheet` 不传 `--cols` 时列数是 `ceil(sqrt(帧数))`。`--cols N` 指定列数，行数是 `ceil(帧数 / N)`，`scale` 不改变列数。一行一个朝向时 `N` 是该朝向的帧数，见 `pixelforge-sprite-4dir`。`--atlas FILE` 再写一份 `pixelforge-atlas` JSON（帧矩形、`delayMs`、`columns`、标签）。不传它，结果仍只有 PNG 字段。`export` 子命令把文件写到磁盘并返回路径。`call` 里的 `exportPngSequence` / `exportSpriteSheet` 返回 base64，不写文件；`exportSpriteSheet` 的 `atlas: true` 把同一份对象放进 `result.atlas`。

平涂用 `fillRect`，挖块用 `eraseRect`，挪一块用 `copyRect` / `moveRect`。一次 `pixelforge call` 里放多个 `--request`。一块矩形已经是一步撤销。好几块要一起撤销时，用 `beginGesture` / `endGesture` 包住。撤销步数和内核操作次数不是一回事，见「最少编辑路径」。

*`preview` writes ASCII plus a color PNG at `result.path`. `--preview` on a mutating command is ASCII only. `diff` is read-only. `hflip` and `vflip` mirror indices and do not recolor. `rotate --degrees 90|180|270` turns indices clockwise; 90 and 270 need a square and do not resize the canvas. `resize` copies stored indices. `export sheet --cols N` sets the column count. `--atlas FILE` writes atlas JSON; omit it and the result stays the PNG fields. Edges of 96 and 128 are inside `1..=256`. ASCII on those canvases is the full grid; read the scaled PNG and a local slice of rows. One undo step is not one kernel operation; see Shortest edit path.*

## 最少编辑路径

同一张图，内核操作越少越好。2026-10-25 的士兵射击动画走了 1,191 次工具操作，其中 1,133 次是 `fillRect`：第 3、4、7 帧和已有帧同姿，却各自整帧重画；躯干按扫描行拆成高度为 1 的矩形；新帧先是空白，再把已经画过的身体填回去。下面四条用现有的 `duplicateFrame`、`copyRect`、`fillRect`、`eraseRect`、`diff` 把这条路径缩短。不新增内核工具。

*Shortest edit path. Fewer kernel operations for the same picture. That soldier shoot made 1,191 tool calls, 1,133 of them `fillRect`. These four rules use tools that already exist.*

### 同姿先复制，再改差

新帧的身体已经在某一帧上，并且新帧可以紧挨在源帧后面时：`duplicateFrame` 源帧，`setActiveFrame` 选中返回的下标，只改不一样的格子。`duplicateFrame` 把拷贝插在源帧后面，返回的 `index` 才是新帧，它不选中这份拷贝。

之前，帧 0 已是持枪站立。帧 1 仍是这个姿势、只抬手臂，路径却是空白帧再把身体填一遍（头、躯干、腿各一次 `fillRect`，手臂还没算）：

```bash
pixelforge call --project hero.json \
  --request '{"tool":"addFrame"}' \
  --request '{"tool":"setActiveFrame","params":{"index":1}}' \
  --request '{"tool":"fillRect","params":{"x":24,"y":8,"width":16,"height":16,"colorIndex":3}}' \
  --request '{"tool":"fillRect","params":{"x":20,"y":24,"width":24,"height":28,"colorIndex":3}}' \
  --request '{"tool":"fillRect","params":{"x":22,"y":52,"width":20,"height":24,"colorIndex":3}}'
```

之后，复制帧 0，只擦掉旧手臂、填上抬起的手臂。身体那三块不用再填：

```bash
pixelforge call --project hero.json \
  --request '{"tool":"duplicateFrame","params":{"index":0}}' \
  --request '{"tool":"setActiveFrame","params":{"index":1}}' \
  --request '{"tool":"eraseRect","params":{"x":40,"y":28,"width":12,"height":8}}' \
  --request '{"tool":"fillRect","params":{"x":36,"y":22,"width":16,"height":6,"colorIndex":3}}'
pixelforge diff --project hero.json --frame 0 --with 1 --scale 4
```

目标下标已经占死时（表里的第 3、4、7 帧已经在，不能再插入），用 `copyRect` 把同姿帧的身体拷进那个下标，再改差。`duplicateFrame` 会把新帧插在源帧后面并后移后续下标，填不进一个已经定死的槽。整个人都相同就拷整块外接矩形，包括里面的索引 0；只有枪或头相同就只拷那一块。

```bash
pixelforge copy-rect --project hero.json --from-frame 1 --to-frame 3 \
  --src-x 16 --src-y 4 --width 32 --height 60 --dst-x 16 --dst-y 4
pixelforge frame select --project hero.json --index 3
```

*`duplicateFrame` when the new frame may sit just after the source. `copyRect` with `fromFrame` / `toFrame` when that slot already exists. Then change the cells that differ. `diff` after the edit; `changed` should sit on that delta.*

### 相邻同色 fillRect 合成更高的一块

同一 `colorIndex`、同一 `x`、同一 `width`，下一块的 `y` 等于上一块的 `y + height`，中间没有缝，并起来仍是一块轴对齐矩形：一次 `fillRect`，高度是各块高度之和。

之前，三行扫描，三次 `fillRect`：

```bash
pixelforge call --project hero.json \
  --request '{"tool":"fillRect","params":{"x":8,"y":20,"width":12,"height":1,"colorIndex":4}}' \
  --request '{"tool":"fillRect","params":{"x":8,"y":21,"width":12,"height":1,"colorIndex":4}}' \
  --request '{"tool":"fillRect","params":{"x":8,"y":22,"width":12,"height":1,"colorIndex":4}}'
```

之后，一块更高的矩形，一次 `fillRect`：

```bash
pixelforge call --project hero.json \
  --request '{"tool":"fillRect","params":{"x":8,"y":20,"width":12,"height":3,"colorIndex":4}}'
```

左右相接用同一条规则合成更宽的一块：同一 `colorIndex`、同一 `y`、同一 `height`，下一块的 `x` 等于上一块的 `x + width`。L 形、两块之间有缝、或中间要留别的索引，保持多块。没有把任意同色区域收成最少矩形的新工具；合并在调用前做。小高光留在大矩形之后，用一块小 `fillRect` 或几笔 `paint` 盖上去。

*Three stacked one-pixel rows of the same index, x, and width are one `fillRect` whose height is 3. Abutting columns of the same index, y, and height are one wider rect. An L stays two rects. Merge before the call.*

### 按帧差改

`diff --frame A --with B`（Tool API 是 `diffFrames`）给出 `changed`、`bounds` 和 `changes`（含两边的索引）。下一帧只改这个差集里的格子：`fillRect`、`eraseRect`、`copyRect`、`moveRect`。差集外面的索引保持不动。

`addFrame` 得到的是全透明帧。空帧上再把另一帧已经有的身体填一遍，差集就变成了整个人。同姿走上面的复制。画完再 `diff` 一次，确认 `changed` 落在这一笔上。

*Aim the edit at `diff`. A blank frame plus a full-body refill paints every cell again.*

### 一次批量 call 是多次编辑

同一次 `pixelforge call` 的多个 `--request` 共用一个进程、一份撤销栈。每一个 `--request` 仍是一次 `Document::call_tool`。上面扫描行的三条 `fillRect` 放在一条命令里仍是三次 `fillRect`；合并后是一次。`beginGesture` / `endGesture` 把它们收成一步撤销，不把操作次数收成 1。

先复制同姿、先把相接的同色矩形合成一块，再把剩下的绘制请求放进同一次 `call`。计数时数 `fillRect`、`eraseRect`、`paint`、`erase`、`copyRect`、`moveRect`。士兵射击那 1,133 次 `fillRect` 就是没做这两步。

*A batched `call` is one process, not one edit. `beginGesture` / `endGesture` group undo only. Count each drawing request. Duplicate and merge first, then send what remains.*

枪已经是挂在自己那一层上的部件时，抬枪用一次 `part rotate`，或一条旋转曲线。一帧用 `constraint solve --frame N --apply`。整段用 `bake`。手跟握持用一条 `follow`，枪托贴肩用一条 `attach`，脚底固定用一条 `pin`。挂接层的墨水跟着走，不必把整只手再 `fillRect` 一遍。省略 `--apply` 只读这一帧。一次 `solveConstraints` 或一次 `bake` 是一次内核操作、一步撤销。上面四条仍然管还没有变成部件的身体和扫描行。矩形 `hflip` / `rotate` 不搬部件。

*When the gun is already a part on its own layer, one `part rotate` poses it. A raise on one frame is a rotation curve plus `constraint solve --frame N --apply`. A raise across frames is `bake`. A `follow` puts the hand on the grip, an `attach` puts the butt on the shoulder, and a `pin` plants the foot. Linked ink moves with the part, so the hand is not a second pass of `fillRect`. Omit `--apply` to read that one frame. One `solveConstraints` or one `bake` is one kernel operation and one undo step. The four rules still cover a body that is not a part. Rectangle `hflip` and `rotate` do not move parts.*

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
pixelforge vflip --project hero.json
pixelforge rotate --project hero.json --degrees 90
pixelforge rotate --project hero.json --degrees 180 --x 0 --y 0 --width 8 --height 8
pixelforge resize --project hero.json --width 64 --height 64
pixelforge resize --project hero.json --width 64 --height 96 --offset-x 0 --offset-y 0
```

### 导入 Aseprite Import an indexed `.aseprite`

只读索引色 8 位：画布、调色板、图像层像素、帧时长。层按文件从底到顶映射，不先合成。组摊平，参考层跳过，隐藏层导入后可见。透明索引不是 0 时和索引 0 对调，槽 0 再强制全透明。`--output` 新建且拒绝已有文件；`--project` 成功后替换。两个目标要且只要一个。不写回 `.aseprite`，没有插件。灰度、RGBA、tilemap、边长超出 `1..=256` 都拒绝。不兼容项看 `result.aseprite`，全文在 `docs/tool-api.md` 的 `importAseprite`。

*Indexed 8-bit only. Layers map bottom to top. `--output` creates and refuses an existing file. `--project` replaces after success. There is no `.aseprite` writer and no plugin API. Read `result.aseprite` and `importAseprite` in `docs/tool-api.md`.*

```bash
pixelforge import-aseprite --file hero.aseprite --output hero.json
pixelforge import-aseprite --file hero.aseprite --project hero.json
```

```json
{ "tool": "importAseprite", "params": { "bytes": "<base64>" } }
{ "tool": "flipVertical", "params": { "frame": 0, "layer": 0, "reportDirty": true } }
{ "tool": "rotate", "params": { "degrees": 90 } }
{ "tool": "rotate", "params": { "degrees": 180, "x": 0, "y": 0, "width": 8, "height": 8 } }
```

### 只改头盔 Edit only the helmet

选区框住头盔，`--layer` 指向装备层。整画布的填充也只会落在矩形里。身体层不动。做完 `selection clear`。没有自由套索。

*Set the rectangle on the helmet and pass `--layer` for the gear layer. A full-canvas fill still lands only inside that rectangle. The body layer stays. `selection clear` when the edit is done. There is no lasso.*

```bash
pixelforge selection set --project hero.json --x 10 --y 4 --width 12 --height 10
pixelforge replace-index --project hero.json --layer 1 --from-index 4 --to-index 6
pixelforge fill-rect --project hero.json --layer 1 --x 0 --y 0 --width 32 --height 48 --color-index 2
pixelforge selection clear --project hero.json
```

`move-rect` 在选区激活时，源和目标都要整块在矩形内。`remap` 不看选区。

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
pixelforge tag add --project hero.json --name walk_down --from 1 --to 3 --loop forward
pixelforge tag set --project hero.json --name walk_down --to 4 --loop pingpong
pixelforge tag remove --project hero.json --name walk_down
pixelforge export sheet --project hero.json --output hero-sheet.png --cols 4 --scale 2 --atlas hero-sheet.json
pixelforge export png --project hero.json --out-dir ./hero-frames --scale 2
```

`tag add` 给闭区间起名。`loop` 是 `forward`（缺省）、`reverse`、`pingpong` 或 `once`。`tag set` 改区间、循环或改名。`tag remove` 只删标签。没有标签时工程文件不写 `tags`。标签不是 cel 链接，复制帧仍然复制像素。整条时间轴的循环预览不按标签播放。四向表的行、列、标签名和脚滑检查见 `pixelforge-sprite-4dir`。

*A tag names an inclusive range. `loop` is `forward` (the default), `reverse`, `pingpong`, or `once`. An empty tag list is omitted from the project file. Tags are not cel links. Playback of the whole timeline is unchanged. Four-direction names live in `pixelforge-sprite-4dir`.*

### 士兵的枪和握持点 Gun and grip

部件不是层。枪的像素画在 `gear` 上，部件记下原点和锚点。肩和肘可以不挂层，只是身体上的点。本地 `(0, 0)` 是 `part add` 的 `x,y`。握持点 `(2, 2)` 的画布格是 `(26, 14)`。抬枪用 `part rotate`，不要再把每一格 `fillRect` 一遍。

*A part is not a layer. The gun pixels stay on `gear`. The part stores the origin and the anchors. A shoulder or an elbow can stay unattached. Local `(0, 0)` is the `x,y` of `part add`. Grip `(2, 2)` is canvas cell `(26, 14)`. Raise the gun with `part rotate` instead of refilling every cell.*

```bash
pixelforge new --width 48 --height 32 --output soldier.json
pixelforge layer rename --project soldier.json --name body
pixelforge fill-rect --project soldier.json --x 16 --y 6 --width 10 --height 18 --color-index 3
pixelforge layer add --project soldier.json --name gear
pixelforge fill-rect --project soldier.json --layer 1 --x 24 --y 12 --width 14 --height 4 --color-index 1
pixelforge part add --project soldier.json --name gun --layer 1 --x 24 --y 12
pixelforge part anchor set --project soldier.json --part gun --name grip --x 2 --y 2
pixelforge part add --project soldier.json --name shoulder --x 22 --y 12
pixelforge part anchor set --project soldier.json --part shoulder --name joint --x 0 --y 0
pixelforge part add --project soldier.json --name elbow --x 26 --y 16
pixelforge part anchor set --project soldier.json --part elbow --name joint --x 0 --y 0
pixelforge part list --project soldier.json
pixelforge scene --project soldier.json
```

`part list` 和 `scene` 的 `parts` 都能读到枪和 `grip`。没有部件时工程文件不写 `parts`。矩形 `hflip` / `vflip` / `rotate` 不搬部件。要让枪和握持点一起动，用下面的部件变换。

*Both `part list` and `scene` return the gun and `grip`. A project with no parts omits `parts`. Rectangle `hflip`, `vflip`, and `rotate` leave the part where it is. Use a part transform when the gun and the grip should move together.*

### 部件变换 Part transform

`part translate`、`part rotate`、`part flip` 把挂接层上的非 0 索引和原点、锚点放进同一步撤销。只搬已有索引，不发明颜色。枢轴是原点格，或 `--anchor` 指名的那一格的中心。没挂层的肩、肘只改原点。

平移是整格位移，锚点的本地坐标不变。翻转是绕枢轴格的镜像：`x` 收到 `2 * pivotX - x`（垂直则是 `y`）。这和矩形 `hflip` 不是同一个枢轴：矩形翻转绕矩形中心，宽为偶数时中心在两格之间，而且不改部件。

旋转的 `--degrees` 可以不是 90。先收到 `(-180, 180]`，再量化到 0.01°，一半时远离 0。量化后是 90° 的倍数就是精确直角，四次 90° 回到原样。其它角度把每个墨水格的中心转到最近的格，一半时远离枢轴。两个格落到同一格时，行优先较后的索引留下，`dropped` 记下丢掉的格数。`result.grid` 是 `quarter` 或 `nearest`，`result.degreesCenti` 是量化后的百分之一度。

只改 `frame`（缺省当前帧）的像素。部件列表仍是整份文档一份，所以这一下的原点描述的是你刚转过的那一帧，其它帧的像素不动。枪和别的墨水不要挂在同一层：变换搬走的是这一层上全部非 0 格。有选区时，这些格和落点必须都在选区内，否则 `SelectionProtected`，一格都不写。落出画布是 `PartOffCanvas`。

不做自由仿射、亚像素合成、骨骼界面、脚本或整链 IK。关键帧只采样，不把采样写进格子。见「部件曲线」。手、枪托和脚底用「约束」里的显式 `constraint solve`，一次调用解当前这一帧。整段帧用「烘焙」。

*A part transform moves the linked layer's ink and the origin and anchors in one undo step. It remaps indices and does not invent colors. The pivot is the center of the origin cell, or of `--anchor`. An unattached shoulder only moves its origin.*

*Translate is a whole-cell shift; local anchors stay. Flip mirrors through the pivot cell. Rectangle `hflip` uses the rectangle center and does not move the part.*

*`--degrees` may be other than 90. It wraps into `(-180, 180]`, quantizes to 0.01° with ties away from zero, and a multiple of 90 is an exact quarter turn. Four 90° turns restore the part. Any other angle snaps each ink-cell center to the nearest cell, ties away from the pivot. A collision keeps the later row-major index and reports `dropped`. Read `grid` and `degreesCenti`.*

*Only `frame` (the active frame by default) changes pixels. The part list is one pose for the document, so the origin then matches that frame. Give the gun its own layer: every non-zero cell on the linked layer moves. A selection must cover each of those cells and its destination, or the call is `SelectionProtected` and writes nothing. A cell that would leave the canvas is `PartOffCanvas`. There is no free affine warp, sub-pixel composite, bone UI, script, or full IK chain. A keyframe curve samples a pose and does not write cells. See Part curves. A hand, a butt, and a planted foot use an explicit `constraint solve` in Constraints. That call poses one frame. A span of frames uses Bake.*

```bash
pixelforge part translate --project soldier.json --name gun --dx 2 --dy 0
pixelforge part rotate --project soldier.json --name gun --degrees 90 --anchor grip
pixelforge part rotate --project soldier.json --name gun --degrees 20.5 --anchor grip
pixelforge part flip --horizontal --project soldier.json --name gun --anchor grip
```

```json
{ "tool": "translatePart", "params": { "name": "gun", "dx": 2, "dy": 0, "reportDirty": true } }
{ "tool": "rotatePart", "params": { "name": "gun", "degrees": 90, "anchor": "grip" } }
{ "tool": "flipPart", "params": { "name": "gun", "direction": "horizontal", "anchor": "grip" } }
```

### 部件曲线 Part curves

曲线记的是相对静止姿势的变换参数，时间轴是帧下标，和标签、`setFrameDelay` 同一条轴。不读 `delayMs`，也不另起时钟。`curve sample` 的 `--frame` 可以是小数：`2` 是第 2 帧上的关键帧，`2.5` 是第 2 帧到第 3 帧的中点（按帧下标，不按毫秒加权）。采样不写像素，也不调用 `part rotate`。把采样交给 `part rotate` / `part translate` / `part flip` 时，要从静止姿势施加；已经转过的那一帧再施加一次会叠加上去。采样本身不写像素。整段写进格子用「烘焙」，不要在 `bake` 已经摆好的帧上再 `part rotate` 一次。

通道是 `rotation`（顺时针角度，和 `part rotate` 一样收到 `(-180, 180]` 再量化到 0.01°）、`translateX`、`translateY`（整格，采样结果可以是小数，交给 `part translate` 之前要自己取整）、`flipHorizontal`、`flipVertical`（`0` 或 `1`）。一个部件的一个通道一条曲线。

关键帧上的 `ease` 是离开这一帧、走向下一帧的方式。最后一帧的 `ease` 先存着，后面再加关键帧才用到。`t` 是这一段里的帧下标比例，`u` 是混合权重，值是 `left + (right - left) * u`：

- `linear`：`u = t`
- `easeInOut`：`u = t² (3 − 2t)`（Hermite smoothstep）。`t = 1/2` 时 `u = 1/2`，中点和线性相同。`t = 1/4` 时 `u = 5/32`
- `hold`：`u = 0`，一直停在左关键帧，直到下一帧的下标，那一帧才取右关键帧。翻转通道只能 `hold`

旋转按存下来的百分之一度做线性混合，不走 ±180° 的短弧。关键帧区间外面夹住端点。正好落在关键帧上就返回那一帧，不混合。

插帧、删帧会在同一步撤销里挪关键帧下标，和标签一样。删掉某一帧上的关键帧；曲线空了就去掉。改名会带着曲线走。删掉部件或 `resize` 把原点裁出画布时，它的曲线一起去掉。没有曲线时工程文件不写 `curves`。版本仍是 1。未知字段仍然拒绝。

*A curve stores transform parameters relative to the rest pose. The timeline is the frame index, the same axis as tags and `setFrameDelay`. It does not read `delayMs` and it does not start another clock. `curve sample --frame` may be fractional: `2` is the key on frame 2, and `2.5` is halfway from frame 2 to frame 3 in index space, not weighted by milliseconds. Sampling does not write pixels and does not call `part rotate`. Apply a sample with `part rotate`, `part translate`, or `part flip` from the rest pose. Applying it again on a frame that was already transformed stacks. `bake` writes a span of those samples. Do not `part rotate` a frame that bake already posed.*

*Channels are `rotation` (clockwise degrees, wrapped into `(-180, 180]` and quantized to 0.01° like `part rotate`), `translateX`, `translateY` (whole cells; a sample may be fractional, so round it before `part translate`), and `flipHorizontal`, `flipVertical` (`0` or `1`). One curve per part per channel.*

*The `ease` on a key is how that key leaves toward the next one. The last key's ease is stored and used only after a later key exists. `t` is the frame-index fraction of the span and `u` is the blend weight. The value is `left + (right - left) * u`.*

- *`linear`: `u = t`*
- *`easeInOut`: `u = t² (3 − 2t)` (Hermite smoothstep). At `t = 1/2`, `u = 1/2`, so the midpoint matches linear. At `t = 1/4`, `u = 5/32`*
- *`hold`: `u = 0` until the next key's frame, which returns that key. Flip channels accept `hold` only*

*Rotation blends the stored centi-degrees. It does not take the short arc across ±180°. Outside the keyed span the end key is held. A sample that lands on a key returns that key.*

*Inserting or deleting a frame moves key indices in that same undo step, as tags do. A key on a deleted frame is dropped, and an empty curve is dropped. Renaming a part renames its curves. Removing a part, or a `resize` that drops its origin, drops its curves. A project with no curves omits `curves`. The version stays 1. Unknown fields are still rejected.*

```bash
pixelforge curve set --project soldier.json --part gun --channel rotation --frame 0 --value 0 --ease ease-in-out
pixelforge curve set --project soldier.json --part gun --channel rotation --frame 4 --value 90
pixelforge curve sample --project soldier.json --part gun --channel rotation --frame 2
pixelforge curve sample --project soldier.json --part gun --frame 1
pixelforge curve list --project soldier.json --part gun
pixelforge curve remove --project soldier.json --part gun --channel rotation --frame 0
```

第 0 帧到第 4 帧的中点是 `--frame 2`。`easeInOut` 在这里和线性一样，都是 45°。`--frame 1` 是这段的四分点：线性是 22.5°，`easeInOut` 是 14.0625°。CLI 也接受 `--ease ease-in-out`。`--channel` 也可以写 `translate-x`。采样结果里的 `interpolation` 是 `key`、`clamp`、`linear`、`easeInOut` 或 `hold`。

*The midpoint of frame 0 and frame 4 is `--frame 2`. `easeInOut` matches linear there: 45°. `--frame 1` is the quarter point: linear is 22.5°, `easeInOut` is 14.0625°. The CLI also accepts `--ease ease-in-out` and `--channel translate-x`. `interpolation` is `key`, `clamp`, `linear`, `easeInOut`, or `hold`.*

```json
{ "tool": "setCurveKey", "params": { "part": "gun", "channel": "rotation", "frame": 0, "value": 0, "ease": "easeInOut" } }
{ "tool": "setCurveKey", "params": { "part": "gun", "channel": "rotation", "frame": 4, "value": 90 } }
{ "tool": "sampleCurve", "params": { "part": "gun", "channel": "rotation", "frame": 2 } }
{ "tool": "samplePart", "params": { "part": "gun", "frame": 2 } }
```

### 约束 Constraints

约束是工程文件里的一份名单，不是第二块像素，也不是骨骼。三种：

- `follow`：把 `part` 的 `anchor`（省略则是原点）平移到 `target` 的 `targetAnchor`。手掌跟枪的 `grip`。
- `attach`：同样是平移，但两边的锚点都要写。枪托 `butt` 贴肩的 `joint`。CLI 和 Tool API 的 `stick` 存成 `attach`。文件里只写 `attach`。
- `pin`：把 `part` 的 `anchor`（省略则是原点）平移到画布格 `(x, y)`。脚的 `sole` 钉在地上。没有 `target`。

`part rotate`、`curve set` 都不自动解约束。要解就调用 `constraint solve` / `solveConstraints`。这样一次调用仍是一次内核操作。

`--frame` 给出时，先按曲线从**当前存着的姿势**采样，再解约束。通道顺序是绕存着的原点旋转、水平翻转、垂直翻转，然后整格平移。平移的小数取最近格，正好一半时远离 0。曲线的旋转枢轴是原点，不是锚点。要绕枪托转，把原点放在枪托上，或先 `part rotate --anchor butt`，再 `constraint solve` 且不写 `--frame`。写了 `--frame` 时，旋转绕原点，随后 `attach` 再把枪托平移回肩上，墨水的朝向留着。

省略 `--frame` 时不读曲线，用已经存下的姿势，包括 `part rotate` 刚写下的那一帧。

求解顺序：从动部件等它的目标先解完。互不依赖的约束保持文件里的先后。自己跟自己、或绕成环，是 `ConstraintCycle`。同一个部件被两条约束驱动，是 `ConstraintConflict`。两条都会动的部件挂在同一层，是 `ConstraintLayer`：一层的墨水只搬一次。一个会动的部件和同层上不动的部件可以共存，这一层上每一个非 0 格都会跟着走，和 `part rotate` 一样。

失败整次不写：锚点已经删掉是 `AnchorMissing`（约束还留在文件里，下次求解再报）；原点或最终墨水出画布是 `PartOffCanvas`；添加时钉点不在画布上也是 `PartOffCanvas`；选区没盖住源格和落点是 `SelectionProtected`。`apply` 省略或 false 时同样做完这些检查，返回报告，不进撤销栈。

`--apply` 把姿势写进 `--pixel-frame`（缺省当前帧）：挂了层的部件，该层非 0 墨水平移，`ink` 为 `moved`；没挂层的只改原点和锚点，`ink` 为 `metadata`；这一次没有位移的是 `unchanged`。有改动才是一步撤销。再解一次会叠在存下的姿势上：曲线再采样一次就再转一次，已经对齐的约束则 `wrote` 为 false。不建新帧，也不沿时间轴逐帧写。整段帧是「烘焙」。

改名会改写约束里的 `part` 和 `target`。删掉部件会丢掉提到它的约束。`resize` 把钉点按同一偏移挪走，钉点出画布或部件被裁掉时丢掉该约束。删锚点不删约束。空列表不写 `constraints`。版本仍是 1。未知字段拒绝。

*A constraint is a record in the project file. It is not a second pixel buffer and not a bone. `follow` translates `part` so its anchor, or its origin, lands on `targetAnchor`. `attach` is the same translation with both anchors required; CLI and Tool API `stick` is stored as `attach`. `pin` translates an anchor or the origin onto a canvas cell and names no target.*

*`part rotate` and `curve set` do not solve. `constraint solve` is the call, so the edit count stays one operation. With `--frame`, curves are sampled from the stored pose and then constraints run. Channel order is rotation, horizontal flip, vertical flip, each around the stored origin, then whole-cell translation. A fractional translation snaps to the nearest cell, and a tie goes away from zero. The curve pivot is the origin. To turn around the butt, put the origin there, or `part rotate --anchor butt` and then `constraint solve` with no `--frame`. With `--frame`, rotation is around the origin and `attach` then slides the butt back onto the shoulder. The ink keeps that rotation.*

*Omit `--frame` and curves are left unread. The stored pose is used, including one `part rotate` just wrote.*

*Solve order waits for a driven target. Independent constraints keep file order. A self-target or a loop is `ConstraintCycle`. A second driver for one part is `ConstraintConflict`. Two posed parts on one layer are `ConstraintLayer`, because a layer's ink is warped once. One moving part may share a layer with a part that does not move; every non-zero cell on that layer moves with it, as `part rotate` already does.*

*A failure writes nothing. A removed anchor is `AnchorMissing` and the constraint stays for the next solve. An origin or a final ink cell off the canvas is `PartOffCanvas`. A pin cell off the canvas is `PartOffCanvas` at add time. A selection that misses a source or destination cell is `SelectionProtected`. `apply` omitted or false runs the same checks, returns the report, and does not push undo.*

*`--apply` writes the pose onto `--pixel-frame`, or the active frame. A linked layer's non-zero ink translates and `ink` is `moved`. An unattached part moves its origin and anchors and `ink` is `metadata`. A part with no motion is `unchanged`. A change is one undo step. Solving again stacks on the stored pose: another curve sample turns again, and a constraint that is already met returns `wrote` false. The call does not create frames and does not walk the timeline. A span of frames is Bake.*

*Renaming a part rewrites `part` and `target`. Removing a part drops constraints that name it. `resize` shifts pin cells with the canvas and drops a constraint whose pin or part leaves the canvas. Removing an anchor leaves the constraint. An empty list omits `constraints`. The version stays 1. Unknown fields are rejected.*

```bash
pixelforge constraint add --project soldier.json --name hand-grip --kind follow --part hand --anchor palm --target gun --target-anchor grip
pixelforge constraint add --project soldier.json --name butt-shoulder --kind stick --part gun --anchor butt --target shoulder --target-anchor joint
pixelforge constraint add --project soldier.json --name plant-foot --kind pin --part foot --anchor sole --x 4 --y 11
pixelforge constraint list --project soldier.json
pixelforge part rotate --project soldier.json --name gun --degrees 90
pixelforge constraint solve --project soldier.json
pixelforge constraint solve --project soldier.json --apply
pixelforge constraint solve --project soldier.json --frame 4 --apply
```

`constraint solve` 不带 `--apply` 时文件不动。`--frame 4` 在枪的旋转曲线上取 90°，再把枪托贴回肩、手跟到新的握持、脚钉在 `(4, 11)`。`stick` 的列表结果是 `kind: attach`。

*Without `--apply`, `constraint solve` leaves the file unchanged. `--frame 4` reads 90° from the gun's rotation curve, then seats the butt, follows the grip, and plants the foot on `(4, 11)`. A stored `stick` lists as `kind: attach`.*

```json
{ "tool": "addConstraint", "params": { "name": "hand-grip", "kind": "follow", "part": "hand", "anchor": "palm", "target": "gun", "targetAnchor": "grip" } }
{ "tool": "addConstraint", "params": { "name": "butt-shoulder", "kind": "stick", "part": "gun", "anchor": "butt", "target": "shoulder", "targetAnchor": "joint" } }
{ "tool": "addConstraint", "params": { "name": "plant-foot", "kind": "pin", "part": "foot", "anchor": "sole", "x": 4, "y": 11 } }
{ "tool": "solveConstraints", "params": { "frame": 4, "apply": true } }
```

### 烘焙 Bake

`bake` 把曲线采样和约束求解写进普通像素帧。写完之后用现有的 `paint`、选区和 `tag` 接着改。没有内置脚本，也没有动画 DSL。外部程序驱动 CLI，例如 `examples/soldier-raise-gun.py`：画好静止姿势，复制帧，写下旋转曲线和 `follow` / `attach` / `pin`，再 `bake --fps 12`。

闭区间是 `--from` 到 `--to`，缺省从 0 到最后一帧。`--source` 缺省第 0 帧，是静止墨水。这一帧的像素不改，所以再烤一次不会叠加上去。报告里的部件 `x`、`y` 是摆好的原点；文件里的原点仍是静止姿势。曲线和约束留下，不自动删。

每一帧都从静止墨水摆姿势，通道顺序和 `constraint solve --frame` 相同：绕存着的原点旋转、水平翻转、垂直翻转，然后整格平移。默认先解约束。`--curves-only`（Tool API 的 `constraints: false`）只采样曲线。`--part` 可重复，只摆这些部件，从动部件也只跟被选中的目标。`--layer` 可重复，只改这些层。两个都省略时，只改这一帧里真的动了的挂接层，没挂部件的身体留在已经复制好的帧上。所以先 `frame duplicate` 静止帧，再打关键帧，再 `bake`。`--to` 超过现有帧数时追加帧，追加的帧先整帧复制源帧，再盖上摆好的层。`from` 可以等于当前帧数（只追加）。`from` 大于帧数会留出空档，是 `BakeRange`。`to` 必须小于 4096。

`--fps` 把区间里每一帧（含源帧）的 `delayMs` 设成 `round(1000/fps)`。12 fps 是 83 毫秒。区间外的帧不动。fps 不是大于 0 的有限数是 `BakeFps`。四舍五入后的时长不在 `1..=65535` 是 `FrameDelay`。

像素或时长有变化才记一步撤销。失败（出画布、缺锚点、环、两条驱动、同一层两个会动的部件、选区没盖住）整次不写。烘焙后的格子就是普通索引，选区、画笔、标签都认它们。工程文件仍是 version 1，没有 `bake` 字段。

*bake writes curve samples and the constraint solve into ordinary pixel frames. Paint, selection, and tags then edit those frames. There is no script runtime and no animation DSL. An external program drives the CLI. `examples/soldier-raise-gun.py` draws the rest pose, duplicates it, stores the rotation curve and the follow / attach / pin constraints, then runs `bake --fps 12`.*

*`--from` and `--to` are an inclusive range. The default is frame 0 through the last frame. `--source` defaults to frame 0 and is the rest ink. That frame's pixels are not rewritten, so a second bake does not stack. Reported part `x` and `y` are the posed origin. The file keeps the rest origin. Curves and constraints stay.*

*Each frame is posed from the rest ink. Channel order matches `constraint solve --frame`: rotation, horizontal flip, and vertical flip around the stored origin, then whole-cell translation. Constraints are solved unless `--curves-only` (`constraints: false`) is set. Repeat `--part` to pose only those parts; a follower tracks a target only when that target is included. Repeat `--layer` to write only those layers. With neither filter, only a layer that a moving part uses is replaced, so a body with no part stays on frames you duplicated first. Duplicate the rest frame, then set keys, then bake. `--to` past the last frame appends. An appended frame copies every source layer, then overwrites the posed layers. `from` may equal the frame count. `from` past the frame count is `BakeRange`, because it would leave a gap. `to` must be below 4096.*

*`--fps` sets `delayMs` to `round(1000/fps)` on every frame in the range, including the source. 12 fps is 83 ms. Frames outside the range keep their delay. A non-finite or non-positive fps is `BakeFps`. A rounded delay outside `1..=65535` is `FrameDelay`.*

*A change to pixels or delays is one undo step. A failure writes nothing: off canvas, a missing anchor, a cycle, a second driver, two moving parts on one layer, or a selection that misses a cell. Baked cells are ordinary indices, so selection, paint, and tags accept them. The project stays version 1 and has no `bake` field.*

```bash
pixelforge frame duplicate --project soldier.json --index 0
pixelforge curve set --project soldier.json --part gun --channel rotation --frame 0 --value 0
pixelforge curve set --project soldier.json --part gun --channel rotation --frame 4 --value 90
pixelforge bake --project soldier.json --from 0 --to 4 --fps 12
pixelforge bake --project soldier.json --part gun --part hand --layer 0
pixelforge bake --project soldier.json --from 4 --to 4 --curves-only
```

```json
{ "tool": "bake", "params": { "from": 0, "to": 11, "fps": 12 } }
{ "tool": "bake", "params": { "parts": ["gun", "hand"], "layers": [0], "constraints": false } }
```

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
