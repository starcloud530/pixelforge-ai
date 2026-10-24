# 导出 Export

内核把索引像素解析成 RGBA，再编码成 PNG。网页只负责下载这些字节。索引 0 是透明（`r = g = b = a = 0`）。其余颜色沿用调色板上存的 RGBA；画笔颜色是不透明的。有多层时，导出先按层序叠索引：非 0 的索引盖住下面的层，索引 0 露出下层，不按调色板 alpha 在层与层之间混合，然后再把赢下的索引解析成 RGBA。导出不改文档，也不进撤销栈。洋葱皮使用同一套合成 RGBA 做视图叠加，但 PNG 里只有该帧自己的合成像素，不含前后帧。

*The kernel resolves indexed pixels to RGBA and encodes the PNG. The page only downloads those bytes. Index 0 is transparent (`r = g = b = a = 0`). Every other entry keeps the RGBA stored on the palette; paint colors are opaque. Export does not change the document and does not push an undo entry. Onion skin uses that same RGBA as a view overlay. A PNG still contains only that frame's own pixels, not its neighbors.*

时长留在时间轴和工程文件的 `delayMs` 里。PNG 里没有每帧时长。默认时长是 100 毫秒，也就是 10 FPS。

*Delays stay on the timeline and in the project file's `delayMs`. A PNG does not carry per-frame timing. The default delay is 100 milliseconds, which is 10 FPS.*

## PNG 序列 PNG sequence

一帧一个文件，顺序就是时间轴顺序。网页下载的文件名是 `pixelforge-000.png`、`pixelforge-001.png`，至少三位、从 0 起，按文件名排序就是播放顺序。网页导出的尺寸等于画布。`pixelforge export png` 写出同样的文件名，并可以用 `--scale K`（整数，至少 1，默认 1）在内核里做最近邻放大：每个索引重复成 `K × K` 的块，再按调色板解析成 RGBA，没有插值。`pixelforge export sheet --scale K` 同样放大每一格。不传 `--cols` 时列数和行数仍是接近正方形的网格；`--cols N` 指定列数，行数是 `ceil(帧数 / N)`，`scale` 不改变列数。`sheetWidth` / `sheetHeight` 乘上 `K`。`K` 为 1 时字节与不传 `--scale` 相同。

*One file per frame, in timeline order. The page downloads `pixelforge-000.png`, `pixelforge-001.png`, and so on: at least three digits, starting at 0, so a filename sort is the playback order. The page exports at canvas size. `pixelforge export png` writes those same file names. `--scale K` (an integer ≥ 1, default 1) nearest-neighbor upscales inside the kernel: each palette index is repeated into a `K` by `K` block, then resolved to RGBA, with no filtering. `pixelforge export sheet --scale K` scales each cell the same way. Without `--cols`, columns and rows stay the nearly square grid. `--cols N` sets the column count and the row count becomes `ceil(frame count / N)`. Scale does not change the column count. `sheetWidth` / `sheetHeight` are multiplied by `K`. Scale 1 matches the bytes written when `--scale` is omitted.*

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
