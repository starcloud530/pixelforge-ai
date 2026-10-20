# 导出 Export

内核把索引像素解析成 RGBA，再编码成 PNG。网页只负责下载这些字节。索引 0 是透明（`r = g = b = a = 0`）。其余颜色沿用调色板上存的 RGBA；画笔颜色是不透明的。导出不改文档，也不进撤销栈。洋葱皮使用同一套 RGBA 做视图叠加，但 PNG 里只有该帧自己的像素，不含前后帧。

*The kernel resolves indexed pixels to RGBA and encodes the PNG. The page only downloads those bytes. Index 0 is transparent (`r = g = b = a = 0`). Every other entry keeps the RGBA stored on the palette; paint colors are opaque. Export does not change the document and does not push an undo entry. Onion skin uses that same RGBA as a view overlay. A PNG still contains only that frame's own pixels, not its neighbors.*

时长留在时间轴和工程文件的 `delayMs` 里。PNG 里没有每帧时长。默认时长是 100 毫秒，也就是 10 FPS。

*Delays stay on the timeline and in the project file's `delayMs`. A PNG does not carry per-frame timing. The default delay is 100 milliseconds, which is 10 FPS.*

## PNG 序列 PNG sequence

一帧一个文件，顺序就是时间轴顺序。网页下载的文件名是 `pixelforge-000.png`、`pixelforge-001.png`，至少三位、从 0 起，按文件名排序就是播放顺序。网页导出的尺寸等于画布。`pixelforge export png` 写出同样的文件名，并可以用 `--scale K`（整数，至少 1，默认 1）在内核里做最近邻放大：每个索引重复成 `K × K` 的块，再按调色板解析成 RGBA，没有插值。`pixelforge export sheet --scale K` 同样放大每一格，列数和行数不变，`sheetWidth` / `sheetHeight` 乘上 `K`。`K` 为 1 时字节与不传 `--scale` 相同。

*One file per frame, in timeline order. The page downloads `pixelforge-000.png`, `pixelforge-001.png`, and so on: at least three digits, starting at 0, so a filename sort is the playback order. The page exports at canvas size. `pixelforge export png` writes those same file names. `--scale K` (an integer ≥ 1, default 1) nearest-neighbor upscales inside the kernel: each palette index is repeated into a `K` by `K` block, then resolved to RGBA, with no filtering. `pixelforge export sheet --scale K` scales each cell the same way. Columns and rows stay the frame grid, and `sheetWidth` / `sheetHeight` are multiplied by `K`. Scale 1 matches the bytes written when `--scale` is omitted.*

## Sprite sheet

所有帧打进一张 PNG。格子接近正方形：

- `columns = ceil(sqrt(frame_count))`
- `rows = ceil(frame_count / columns)`

从左到右排满一行，再换到下一行。默认格子大小等于帧大小；`--scale K` 时格子是帧大小乘 `K`。帧数不是列数的整数倍时，最后一行末尾是完全透明的空位，那些空位不是帧，放大后仍是透明块。

例子：1 → 1×1，2 → 2×1，3 → 2×2，4 → 2×2，5 → 3×2，9 → 3×3。

网页文件名带上这个网格，例如两列一行是 `pixelforge-sheet-2x1.png`。改名之后仍用上面的公式。

*Every frame is packed into one PNG. The grid is nearly square: `columns = ceil(sqrt(frame_count))` and `rows = ceil(frame_count / columns)`. Frames fill left to right, then wrap. Each cell is the frame size, or the frame size times `--scale`. When the frame count is not a multiple of `columns`, the leftover cells on the last row are fully transparent and are not frames. Scaled padding stays transparent.*

*Examples: 1 → 1×1, 2 → 2×1, 3 → 2×2, 4 → 2×2, 5 → 3×2, 9 → 3×3.*

*The downloaded name includes that grid, for example `pixelforge-sheet-2x1.png` for two columns and one row. After a rename, use the formula above.*

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
