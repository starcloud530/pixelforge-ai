# 导出 Export

内核把图层合成解析成 RGBA，再编码成 PNG。网页只负责下载这些字节。隐藏层和 `opacity` 为 0 的层不进入这张图。索引 0 是透明（`r = g = b = a = 0`）。`opacity` 为 255 时，非 0 索引用调色板上的 RGBA 替换下层，调色板 alpha 不与下层混合；画笔颜色是不透明的。`opacity` 为 1 到 254 时，在导出里按直通 alpha 做 source-over，源 alpha 是 `round(调色板 a × opacity / 255)`，一半时远离 0。这个混合不写回格子。导出本身不收成最近的调色板色。索引预览在压平之前仍看见盖住的索引。`layer flatten` 才把这一张 RGBA 按 `apply-png` 的规则收成索引，见 [tool-api.md](tool-api.md) 的 `flattenLayers`。`--scale` 在混合之后把格子最近邻放大。导出不改文档，也不进撤销栈。洋葱皮使用同一套合成 RGBA 做视图叠加，但 PNG 里只有该帧自己的合成像素，不含前后帧。精灵表和它旁边的 atlas 用的是同一张合成：atlas 仍是帧矩形和标签，隐藏层不会从格子里删掉一帧，只是这一帧的像素不含那一层。

*The kernel composites layers to RGBA and encodes the PNG. The page only downloads those bytes. A hidden layer, or opacity 0, is left out. Index 0 is transparent (`r = g = b = a = 0`). Opacity 255 replaces the cell with the palette RGBA and does not blend palette alpha into the layer underneath. Paint colors are opaque. Opacity 1..=254 blends in the export with straight-alpha source-over. Source alpha is `round(palette.a * opacity / 255)`, ties away from zero. The blend is not written back. Export does not snap it to a palette index. The index preview still shows the covering index until `layer flatten`, which snaps this RGBA with the `apply-png` rule. See `flattenLayers` in [tool-api.md](tool-api.md). `--scale` repeats the blended cell. Export does not change the document and does not push an undo entry. Onion skin uses that same RGBA as a view overlay. A PNG still contains only that frame's own pixels, not its neighbors. The sprite sheet and its atlas use that composite. The atlas stays frame rectangles and tags. A hidden layer does not remove a frame from the grid.*

## 并排对照 Side-by-side

`pixelforge preview --compare` 把当前帧写成一张图，左右相接，中间没有间隔。左是上面的未量化 RGBA。右是 `layer flatten` 会为这一帧写下的索引，再按调色板着色，和压平之后的彩色 preview 相同。工程文件不动，也不需要先压平。半透明层（灯塔 `beam`）在左边仍是混合色，右边是收成的索引色。ASCII 和 `result.pixels` 仍是盖住索引；右边的索引在 `result.flattenPixels`。默认文件是工程旁边的 `<stem>.compare.png`。`--scale` 两格一起最近邻放大。不要和 `--layer` 或蒙版一起用。

```bash
pixelforge preview --project lighthouse.json --compare --scale 4
```

*`pixelforge preview --compare` writes one image, left beside right, with no gutter. The left panel is the unquantized RGBA above. The right panel is the palette color of the indices `layer flatten` would store for this frame, the same picture as a color preview after flatten. The project file is not changed, and flatten does not have to run first. A translucent layer stays blended on the left and snapped on the right. ASCII and `result.pixels` stay the covering index. `result.flattenPixels` is the right panel. The default file is `<stem>.compare.png` beside the project. `--scale` nearest-neighbor repeats both panels. Do not pass `--layer` or a mask.*

时长留在时间轴和工程文件的 `delayMs` 里。PNG 序列和 sprite sheet 里没有每帧时长。默认时长是 100 毫秒，也就是 10 FPS。要在一个文件里循环播放，用下面的 APNG 预览。

*Delays stay on the timeline and in the project file's `delayMs`. A PNG sequence and a sprite sheet do not carry per-frame timing. The default delay is 100 milliseconds, which is 10 FPS. A single looping file is the APNG preview below.*

## PNG 序列 PNG sequence

一帧一个文件，顺序就是时间轴顺序。网页下载的文件名是 `pixelforge-000.png`、`pixelforge-001.png`，至少三位、从 0 起，按文件名排序就是播放顺序。网页导出的尺寸等于画布。`pixelforge export png` 写出同样的文件名，并可以用 `--scale K`（整数，至少 1，默认 1）在内核里做最近邻放大：每个索引重复成 `K × K` 的块，再按调色板解析成 RGBA，没有插值。`pixelforge export sheet --scale K` 同样放大每一格。不传 `--cols` 时列数和行数仍是接近正方形的网格；`--cols N` 指定列数，行数是 `ceil(帧数 / N)`，`scale` 不改变列数。`sheetWidth` / `sheetHeight` 乘上 `K`。`K` 为 1 时字节与不传 `--scale` 相同。

*One file per frame, in timeline order. The page downloads `pixelforge-000.png`, `pixelforge-001.png`, and so on: at least three digits, starting at 0, so a filename sort is the playback order. The page exports at canvas size. `pixelforge export png` writes those same file names. `--scale K` (an integer ≥ 1, default 1) nearest-neighbor upscales inside the kernel: each palette index is repeated into a `K` by `K` block, then resolved to RGBA, with no filtering. `pixelforge export sheet --scale K` scales each cell the same way. Without `--cols`, columns and rows stay the nearly square grid. `--cols N` sets the column count and the row count becomes `ceil(frame count / N)`. Scale does not change the column count. `sheetWidth` / `sheetHeight` are multiplied by `K`. Scale 1 matches the bytes written when `--scale` is omitted.*

## 可播放预览 APNG

`pixelforge export apng` 把一段动画写成一张能循环播放的 APNG。浏览器和常见看图软件直接打开。它不是 sprite sheet，也不能代替 `export sheet` 进游戏。`validate-atlas` 不读这个文件。

选 APNG，不选 GIF。合成结果已经是直通 RGBA：调色板项带 alpha，层 `opacity` 在 1 到 254 时还会混出调色板里没有的颜色。GIF 只有一张最多 256 色的索引表，并且只有一个透明色，半透明必须铺底或抖动。帧时长是毫秒，`bake --fps 12` 得到 83。GIF 的单位是百分之一秒，83 毫秒存不下来。APNG 用 `delay_num = delayMs`、`delay_den = 1000`，毫秒是精确的。像素和 `frame_rgba` / `export png` 是同一张合成，最近邻 `--scale` 也相同。没有调色板量化，没有抖动。

```bash
pixelforge export apng --project hero.json --output walk.apng --tag walk_down
```

`--tag` 用该标签的闭区间和 `loop`。不写标签时，`--from` / `--to` 是闭区间，缺省是整条时间轴；`--loop` 缺省 `forward`。`--tag` 不能和 `--from`、`--to`、`--loop` 一起写。`forward`、`reverse`、`pingpong` 无限循环（APNG `num_plays` 为 0）。`once` 播一遍（`num_plays` 为 1）。`pingpong` 两端不多停一帧：0 到 3 的播放序是 0、1、2、3、2、1，中间帧的 `delayMs` 用两次。两帧或一帧的 `pingpong` 不额外重复端点。`reverse` 从 `to` 播到 `from`。每一格仍是该帧自己的 `delayMs`。

隐藏层和 opacity 0 不进入画面。每一帧都是整幅合成，用 APNG 的 source 混合盖住上一帧，所以透明格不会漏出上一帧。不改文档，不进撤销栈。

Tool API 是 `exportApng`。`call` 的 `result.apng` 是标准 base64。CLI 把文件写到 `--output`，stdout 里是路径、`frames`（播放序的 `index` 和 `delayMs`）、`loop`、`plays`，没有 base64。

*This is one looping APNG a browser or an image viewer can play. It is not a sprite sheet and it does not replace `export sheet` for a game. `validate-atlas` does not read it. APNG is the format, not GIF: the composite is already straight RGBA, palette entries carry alpha, and opacity 1..=254 blends colors that are not palette indices. GIF has one transparent index and would have to matte or dither those pixels. Delays are milliseconds; 12 fps is 83 ms, which GIF's centisecond unit cannot store. APNG stores `delayMs / 1000` exactly. Pixels match `frame_rgba` and `export png`. There is no palette quantization and no dither. `--tag` uses that tag's range and loop. Without a tag, `--from` / `--to` default to the whole timeline and `--loop` defaults to `forward`. Do not pass `--tag` together with `--from`, `--to`, or `--loop`. `forward`, `reverse`, and `pingpong` loop forever (`plays` 0). `once` plays once (`plays` 1). Ping-pong does not hold either end: 0 through 3 plays 0, 1, 2, 3, 2, 1, and a middle frame uses its `delayMs` twice. Two frames or one frame stay that long. `reverse` plays from `to` back to `from`. Each step keeps that frame's delay. A hidden layer or opacity 0 is left out. Each frame is the full composite and replaces the previous frame, so a transparent cell does not show the frame before it. The document and the undo stack stay as they were. Tool API `exportApng` returns standard base64 in `result.apng`. The CLI writes `--output` and prints the path, playback `frames` (`index` and `delayMs`), `loop`, and `plays`, without base64.*

## Sprite sheet

所有帧打进一张 PNG。不指定列数时格子接近正方形：

- `columns = ceil(sqrt(frame_count))`
- `rows = ceil(frame_count / columns)`

`pixelforge export sheet --cols N` 和 Tool API 的 `columns` 改用 `N` 列、`ceil(frame_count / N)` 行。一行一个方向时，`N` 就是该方向的帧数。`N` 大于帧数时，多出来的格子是透明空位。网页导出仍用上面的正方形公式。

从左到右排满一行，再换到下一行。默认格子大小等于帧大小；`--scale K` 时格子是帧大小乘 `K`。帧数不是列数的整数倍时，最后一行末尾是完全透明的空位，那些空位不是帧，放大后仍是透明块。

例子：1 → 1×1，2 → 2×1，3 → 2×2，4 → 2×2，5 → 3×2，9 → 3×3。

网页文件名带上这个网格，例如两列一行是 `pixelforge-sheet-2x1.png`。改名之后仍用上面的公式。

*Every frame is packed into one PNG. With no column count, the grid is nearly square: `columns = ceil(sqrt(frame_count))` and `rows = ceil(frame_count / columns)`. `pixelforge export sheet --cols N` and Tool API `columns` use `N` columns and `ceil(frame_count / N)` rows, so one facing can stay on one row. Columns past the frame count are transparent padding. The web export still uses the square formula. Frames fill left to right, then wrap. Each cell is the frame size, or the frame size times `--scale`. When the frame count is not a multiple of `columns`, the leftover cells on the last row are fully transparent and are not frames. Scaled padding stays transparent.*

*Examples: 1 → 1×1, 2 → 2×1, 3 → 2×2, 4 → 2×2, 5 → 3×2, 9 → 3×3.*

*The downloaded name includes that grid, for example `pixelforge-sheet-2x1.png` for two columns and one row. After a rename, use the formula above.*

## Atlas JSON

`pixelforge export sheet --atlas hero-sheet.json` 在 PNG 旁边再写一份 `pixelforge-atlas` version 1。不传 `--atlas` 时只写 PNG，stdout 也和以前一样。Tool API 里对应的是 `exportSpriteSheet` 的 `atlas: true`，结果里多一个对象，不写文件。

矩形是放大之后的图上像素。`columns` 是帧网格，`scale` 不改变它。`frames` 只列真实帧，每项有 `index`、`x`、`y`、`width`、`height`、`delayMs`。末尾的透明空位不是帧。`tags` 是文档里的命名区间：`name`、`from`、`to`、`loop`。`from` 和 `to` 是帧下标，两端都含。CLI 写出的文件另有 `image`，值是 PNG 文件名。

循环模式是 `forward`、`reverse`、`pingpong`、`once`。这是给读取 atlas 的一方用的。文档自己的循环预览仍按整条时间轴走，不按单个标签播。标签不是 cel 链接：帧与帧不共用像素。

四向表的名字约定见 [pixelforge-sprite-4dir](../.cursor/skills/pixelforge-sprite-4dir/SKILL.md)：一行一个朝向时，`idle_down` 是该行第一格，`walk_down` 是该行剩下的走路帧，左、右、上同样用 `idle_left` / `walk_left` 等。

*Passing `--atlas` writes `pixelforge-atlas` version 1 next to the PNG. Omitting it keeps the PNG-only result. Tool API `exportSpriteSheet` with `atlas: true` returns the same object and does not write a file. Rectangles are sheet pixels after scale. `frames` lists real frames and each one carries `delayMs`. Padding cells are absent. `tags` are inclusive frame ranges. The CLI file adds `image`, the PNG file name. Loop modes are metadata for the reader. Document playback still walks the whole timeline. Tags are not linked cels. Four-direction names live in pixelforge-sprite-4dir.*

## 对接校验 validate-atlas

导出之后、把表送进游戏之前先跑校验。它只读 PNG 和上面那份 `pixelforge-atlas`，不改像素，不重写 atlas，也不另起一种格式。

```bash
pixelforge validate-atlas --atlas hero-sheet.json --sheet hero-sheet.png
```

`--sheet` 可以省略。省略时用 atlas 里的 `image`，路径相对 JSON 文件所在目录。

stdout 是一行 JSON。`tool` 是 `validateAtlas`。`result.errors` 和 `result.warnings` 是清单，每项有 `code`、`message`，涉及某一帧或某个标签时还有 `frame` 或 `tag`。`result.checks` 写出这次用的模式、尺寸档、四向检查有没有跑、pivot 是 `checked` 还是 `skipped`。

退出码：

| 情况 | 退出码 |
| --- | --- |
| 没有 error。可以有 warning | `0` |
| 至少一条 error。stdout 仍是这份清单 | `1` |
| 文件打不开、PNG 读不了、或 JSON 不是 `pixelforge-atlas` version 1 | `2` |

`--strict` 是默认。`--loose` 只把约定类问题降成 warning。两种模式下都是：**有 error 才退出 1**；只剩 warning 时退出码仍是 0，包括 strict 里的 `emptyFrame`。

| 检查 | code | strict | loose |
| --- | --- | --- | --- |
| 帧矩形越界 | `outOfBounds` | error | error |
| 帧矩形重叠 | `overlap` | error | error |
| 矩形不在自己的格子上，或 `sheetWidth` 不是 `columns * frameWidth`（高度同理），或行数装不下这些帧 | `grid` | error | error |
| PNG 尺寸不是 atlas 的 `sheetWidth` / `sheetHeight` | `sheetSize` | error | error |
| 不属于任何帧的像素不透明。这就是末尾透明空位，以及帧矩形没有盖住的格子 | `padding` | error | error |
| `frameWidth` 或 `frameHeight` 不能被 `scale` 整除 | `scale` | error | error |
| 帧下标不连续、标签下标不连续、区间越界、`loop` 不是 `forward` / `reverse` / `pingpong` / `once`、标签重名 | `frameIndex` / `tagIndex` / `tagRange` / `loop` / `tagName` | error | error |
| 规则 `requireTags` 里的名字不在 | `missingTags` | error | error |
| 未缩放画布边长不在允许档里。比的是 `frameWidth / scale` 和 `frameHeight / scale`，默认 **64、96、128**。两条边都在档内才通过，所以 64×128 通过，32×64 不通过。`scale` 为 2 的 64×64 仍是 64，不是 128 | `size` | error | warning |
| 已有 `{idle\|walk}_{朝向}` 时，名字或区间不符合方向约定。四向行序是 `down`、`left`、`right`、`up`（S/W/E/N）。后缀也可以是罗盘别名（`south`/`s` 等）。表有 8 行，或出现 `downleft`/`sw` 这类对角名字时，按八向查：前四行不变，后面是 `downleft`、`downright`、`upleft`、`upright`。idle 只占该行第一格；walk 占剩下的格子；这一行没有 idle 时，整行只打 `walk_{朝向}` | `naming`，缺的那一格是 `missingTags` | error | warning |
| 一个标签都没有，而且这次没有按四向在查 | `missingTags` | error | warning |
| `image` 和 PNG 文件名不一致 | `image` | error | warning |
| `delayMs` 不在 `1..=65535` | `delay` | error | warning |
| 原点。只有 atlas 或规则里已经有 `pivot` 才查。`x`、`y` 是帧局部像素，左上为原点，y 向下，远边 `x == width`、`y == height` 仍算在格子内。各帧不透明像素的最底一行到 pivot 的距离必须相同 | `pivot` | 落在格子外是 error；距离不一致是 error | 落在格子外是 error；距离不一致是 warning |
| 一整格都透明 | `emptyFrame` | warning | warning |

导出不写 `pivot`。没有这份字段、规则里也没有时，`checks.pivot` 是 `skipped`。可选的 atlas `pivot` 是 `{ "x", "y" }`，可以写在根上，也可以写在某一帧上。这仍是同一份 version 1 对象，不是第二条格式。

规则文件是 `pixelforge-atlas-rules` version 1。未知字段会拒绝，避免把写错的键当成没写。命令行上出现的旗标覆盖文件里的同名字段。`sizes: []` 表示不查尺寸档。`tagPolicy: optional` 表示空标签列表不记一笔，`requireTags` 仍然要有。`fourDirection` 是 `auto`（默认：出现四向或八向名字才查）、`on` 或 `off`。`--loose` 仍把方向名字降成 warning。

工程里的硬门槛是另一条命令，不读 PNG：`pixelforge direction validate --project hero.json`。它要求已经 `direction set` 的布局。缺方向、缺帧，或 idle/walk 帧段对不上，是 error，没有 loose，退出码 1。八向要把 `count` 设成 8。这条检查不生成步态，通过也不等于动作自然或画面过关。自动命名是 `direction apply`：它只写已有的 idle/walk 标签区间。`direction fill` 才写像素，而且只做水平镜像或整帧拷贝：left 与 right 可以镜像，其余基本朝向用 `copy` 占位。八向的对角行不填。这不是新画，不是自然步态，也不是 PixelLab 自动画四向。

```bash
pixelforge validate-atlas --atlas hero-sheet.json --sheet hero-sheet.png --loose
pixelforge validate-atlas --atlas hero-sheet.json --rules pack-rules.json
```

```json
{
  "format": "pixelforge-atlas-rules",
  "version": 1,
  "mode": "strict",
  "sizes": [64, 96, 128],
  "requireTags": ["idle_down", "walk_down"],
  "fourDirection": "auto",
  "tagPolicy": "required",
  "pivot": { "x": 32, "y": 64 }
}
```

### 和 #18 的关系

[#18](https://github.com/starcloud530/pixelforge-ai/issues/18) 是消费侧：丧尸和据点道具的精灵包要真正进游戏原型，并记下游戏才拒绝的失败。`validate-atlas` 是那张清单前面的工具侧门槛。

1. `export sheet --atlas` 写出 PNG 和这份 atlas。
2. 先跑 `validate-atlas`。清单里的 error 回到 PixelForge 改：补标签、把矩形放回格子、换成 64/96/128 画布、改四向名字、擦掉空位上的不透明像素、用规则里的 `pivot` 把各帧着地点对齐。然后重新导出。本命令不自动修图。
3. 退出码是 0 之后，同一对 PNG 和 atlas 才送进游戏。过滤、最近点采样、动画有没有接到 `idle_*` / `walk_*`、引擎 pivot 和规则是不是同一点，这些游戏侧失败记在 #18。退出码 0，以及 `examples/game-pack-zombie/validate.py` 通过，只说明清单里没有 error。画面好不好看、引擎里有没有接上，都不在这次检查里。和精修、旋转、IK 一起写在 [known-limits.md](known-limits.md)。

静态道具可以在规则里把 `sizes` 放宽（例如加上 32），`tagPolicy` 设为 `optional`，`fourDirection` 设为 `off`，并用 `pivot` 写上引擎原点。四向角色用默认 strict。

*Run `validate-atlas` after `export sheet --atlas` and before the sheet is imported. It reads the same `pixelforge-atlas` version 1 object and does not edit pixels or invent a second format. Exit 0 means no errors. Exit 1 means the checklist has an error. Warnings alone stay exit 0 in both modes. `--loose` downgrades canvas tier, four-direction names, an empty tag list, the image file name, delay range, and pivot drift. Overlap, out-of-bounds frames, a broken grid, and opaque padding stay errors. Canvas edges are `frameWidth / scale` and `frameHeight / scale`, defaulting to 64, 96, and 128. Pivot is checked only when the atlas or the rules file already has one. This is the tool-side gate in front of #18: fix those errors in PixelForge and re-export, then let the game record whatever it still rejects. Exit 0, including `examples/game-pack-zombie/validate.py`, means the checklist has no error. It does not accept the picture, and it does not mean the engine import is done. Polish, rotation, IK, and this check are in [known-limits.md](known-limits.md).*

### 本仓库的 #18 包

[examples/game-pack-zombie](../examples/game-pack-zombie/README.md) 是上面三步的一份结果，不是游戏工程。丧尸是 64×64、四行朝向的 idle + walk。据点道具是路障、木箱、发电机、医疗箱，规则里 `fourDirection` 为 `off`、`tagPolicy` 为 `optional`。

```bash
python3 examples/game-pack-zombie/build.py
pixelforge export sheet --project examples/game-pack-zombie/zombie/zombie.json \
  --output examples/game-pack-zombie/zombie/zombie-sheet.png --cols 4 \
  --atlas examples/game-pack-zombie/zombie/zombie-sheet.json
pixelforge validate-atlas --atlas examples/game-pack-zombie/zombie/zombie-sheet.json \
  --rules examples/game-pack-zombie/zombie/zombie-rules.json --strict
python3 examples/game-pack-zombie/validate.py
```

道具把路径里的 `zombie` 换成 `props`。`validate.py` 和 `cargo test -p pixelforge-cli game_pack_zombie` 都要求 strict 退出码 0。失败模式、以及 Godot / Unity 仍要手接的过滤、轴心和 `idle_*` / `walk_*`，写在 [FAILURES.md](../examples/game-pack-zombie/FAILURES.md)。下面两节是那份清单用的导入约定。

*The worked #18 pack is [examples/game-pack-zombie](../examples/game-pack-zombie/README.md). `validate.py` re-runs the strict gate. Godot and Unity import steps below are what the pack's failure log still has to do by hand.*

## Godot 4

这些是导入约定，不是编辑器技术栈。

1. 把 PNG 放进工程。导入时关闭 Filter（Godot 4.3 及以后也可以在节点上把 Texture Filter 设为 Nearest），并关闭 mipmap。像素缩放用整数倍。
2. PNG 序列：新建 `SpriteFrames`，按文件名顺序把每一张加进同一个动画。每一帧的时长（秒）= `delayMs / 1000`。默认 100 毫秒就是 0.1 秒。循环打开，和工作室里的循环预览一致。
3. Sprite sheet：在 `Sprite2D` 上设 `hframes` = 列数、`vframes` = 行数；或在 `SpriteFrames` 里用 “Add frames from sprite sheet”，横向、纵向帧数填同一组列数和行数。帧顺序是从左到右、再从上到下。动画长度用真实帧数，不要把末尾的透明空位播进去。时长算法与序列相同。

*These are import conventions, not the editor stack.*

1. *Put the PNGs in the project. Turn Filter off on import (on Godot 4.3 or later you can also set the node's Texture Filter to Nearest) and turn mipmaps off. Scale pixels by whole numbers.*
2. *PNG sequence: create a `SpriteFrames`, add each file to one animation in filename order, and set each frame's duration in seconds to `delayMs / 1000`. The default 100 milliseconds is 0.1 seconds. Enable looping to match the studio preview.*
3. *Sprite sheet: set `hframes` to the column count and `vframes` to the row count on a `Sprite2D`, or use “Add frames from sprite sheet” in `SpriteFrames` with those same counts. Frame order is left to right, then top to bottom. Stop the animation at the real frame count so the transparent padding cells are not played. Durations use the same rule as the sequence.*

## Unity

1. 选中贴图：Texture Type 用 Sprite (2D and UI)。序列用 Sprite Mode = Single；sheet 用 Multiple。Pixels Per Unit 常用帧高，或固定成 16 / 32，让像素和瓦片对齐。Mesh Type 用 Full Rect，避免透明边被网格裁掉。Filter Mode 用 Point (no filter)，Compression 用 None，并打开 Alpha Is Transparency。
2. PNG 序列：按文件名顺序做动画。每一帧时长相同的话，采样率 = `1000 / delayMs`（默认 10）。时长不同的话，按累计的 `delayMs` 把关键帧放在对应的秒数上。
3. Sprite sheet：Sprite Editor 里按格子切片，Cell Size 等于帧宽和帧高。Unity 会从左上角开始、从左到右再换行，把切片命名为 `名字_0`、`名字_1`。删掉超过真实帧数的空切片，再用剩下的切片按编号做动画。轴心常用 Center；角色踩在格子上时可以用 Bottom。

*1. Select the texture. Texture Type is Sprite (2D and UI). Use Sprite Mode Single for a sequence file and Multiple for the sheet. A common Pixels Per Unit is the frame height, or a fixed 16 or 32, so pixels line up with tiles. Mesh Type Full Rect keeps transparent edges. Filter Mode is Point (no filter), Compression is None, and Alpha Is Transparency is on.*

*2. PNG sequence: build the animation in filename order. When every frame shares one delay, the sample rate is `1000 / delayMs` (10 by default). When delays differ, place keys at the cumulative `delayMs` in seconds.*

*3. Sprite sheet: in the Sprite Editor, slice by cell size equal to the frame width and height. Unity names slices `name_0`, `name_1`, from the top left, left to right, then the next row. Delete slices past the real frame count, then animate the remaining slices in that order. Center is a common pivot; Bottom fits a character standing on a tile.*

## Tiled 地图

`pixelforge export tileset` 写出图块 PNG 和 `pixelforge-tileset` JSON。`pixelforge export tilemap` 写出整张预览 PNG 和 Tiled 能打开的正交 `.tmj`。两条命令的 `--cols` 和 `--scale` 要相同，`.tmj` 里的 `image` 是 PNG 文件名。把 `base.tmj` 和 `tiles.png` 放在同一目录，用 Tiled 打开 `.tmj`。空格的 GID 是 0。tile id 0 的 GID 是 1。水平翻转是 GID 的 `0x80000000`，垂直翻转是 `0x40000000`。

这一版只有一张图块集、四邻接 16 掩码。没有 8 邻接，也没有对角翻转。例子是 [examples/base-tiles](../examples/base-tiles/README.md)。把这份导出放进一个可跑的页面，见 [examples/lighthouse-outpost](../examples/lighthouse-outpost/README.md) 和 [docs/trials/111-lighthouse-tilemap.md](trials/111-lighthouse-tilemap.md)。

* `pixelforge export tileset` writes the tileset PNG and `pixelforge-tileset` JSON. `pixelforge export tilemap` writes a preview PNG and an orthogonal `.tmj` Tiled can open. Use the same `--cols` and `--scale` on both commands. The `.tmj` `image` is the PNG file name. Keep `base.tmj` and `tiles.png` in one directory and open the `.tmj` in Tiled. An empty cell is GID 0. Tile id 0 is GID 1. Horizontal flip is `0x80000000` on the GID. Vertical flip is `0x40000000`. This version is one tileset and the 16 four-neighbor masks. There is no 8-neighbor autotile and no diagonal flip. The example is [examples/base-tiles](../examples/base-tiles/README.md). A page that loads that export is [examples/lighthouse-outpost](../examples/lighthouse-outpost/README.md), with the trial note in [docs/trials/111-lighthouse-tilemap.md](trials/111-lighthouse-tilemap.md).*
