# Tool API

稳定的机器可读接口，供 Agent 调用。它在 Rust 内核里，入口是 `Document::call_tool`。每一次调用都作用在和界面同一份 `Document` 上：画笔、橡皮、调色板、帧、时长、撤销栈、工程文件和 PNG 导出都走已有的内核方法。Agent 不改界面上的像素缓冲。

版本是 **1**（`TOOL_API_VERSION`）。请求可以省略 `apiVersion`，省略和 `null` 都表示 1。响应里始终带 `"apiVersion": 1`。

浏览器里的同一入口是 wasm 的 `callTool(request) -> string`。外壳的类型化封装在 `apps/web/src/kernel/toolApi.ts`（`callDocumentTool`）。手势仍然可以直接调用 `paint` 等方法；那些方法和 Tool API 写的是同一份文档。本接口不调用外部模型，也不读取 API key。外部模型的一轮编排在外壳里，见 [ai-driver.md](ai-driver.md)：模型返回的 tool_calls 交给这个接口，编排层不改像素缓冲。

*A stable machine-readable API for an agent. It lives in the Rust kernel at `Document::call_tool`. Every call uses the same `Document` as the UI: paint, erase, palette, frames, delay, the undo stack, project files, and PNG export go through the kernel methods that already exist. An agent does not patch a pixel buffer owned by the shell.*

*The version is **1** (`TOOL_API_VERSION`). Omitting `apiVersion`, or sending `null`, means 1. Every response includes `"apiVersion": 1`.*

*In the browser the same entry is wasm `callTool(request) -> string`. The typed shell helper is `apps/web/src/kernel/toolApi.ts` (`callDocumentTool`). Gestures may still call `paint` and the other methods directly; those methods and the Tool API write one document. This API does not call an external model and does not read an API key. The one-turn driver lives in the shell; see [ai-driver.md](ai-driver.md). It forwards model tool calls into this API and does not patch a pixel buffer.*

本地 Agent 的首选入口是 `pixelforge` CLI（`crates/pixelforge-cli`）。它从磁盘打开 `pixelforge-project`，把下面的 JSON 交给 `Document::call_tool`，并把响应写到 stdout。一次进程是一份文档，所以同一次调用里的多个请求共享撤销栈；工程文件本身不保存撤销栈。网页面板是可选演示。见 [agent-skill.md](agent-skill.md)。

*The preferred local agent entry is the `pixelforge` CLI (`crates/pixelforge-cli`). It opens a `pixelforge-project` from disk, passes the JSON below to `Document::call_tool`, and writes the response to stdout. One process is one document, so several requests in that process share the undo stack; the project file does not store the stack. The web panel is an optional demo. See [agent-skill.md](agent-skill.md).*

## 请求与响应 Envelope

请求是一个 JSON 对象。响应是紧凑 JSON，没有结尾换行。`id` 可以省略；给出时必须是字符串或数字，并原样出现在响应里。请求解析失败时响应不带 `id`。

*A request is one JSON object. A response is compact JSON with no trailing newline. `id` may be omitted. When present it is a string or a number and is copied onto the response. A request that does not parse omits `id`.*

```json
{ "apiVersion": 1, "id": "step-1", "tool": "paint", "params": { "x": 1, "y": 2, "colorIndex": 3 } }
```

成功：

*Success:*

```json
{ "ok": true, "apiVersion": 1, "id": "step-1", "result": {} }
```

失败：

*Failure:*

```json
{
  "ok": false,
  "apiVersion": 1,
  "id": "step-1",
  "error": { "code": "PaletteLocked", "message": "palette is locked" }
}
```

`params` 可以省略或为 `null`，两者都当成 `{}`。它必须是对象。未知字段会被拒绝。整数用 JSON 数字，不用字符串。分支看 `error.code`；`message` 给人读。失败时文档保持调用前的状态。`loadProject` 只有在工程文件解析成功后才换掉文档。

*Omit `params`, or send `null`, and both mean `{}`. It must be an object. Unknown fields are rejected. Integers are JSON numbers, not strings. Branch on `error.code`. `message` is for humans. On failure the document stays as it was before the call. `loadProject` replaces the document only after the project file parses.*

## 错误码 Errors

协议错误（文档不变）：

*Protocol errors (the document is unchanged):*

| code | 何时 When |
| --- | --- |
| `InvalidRequest` | 请求不是 JSON 对象，缺 `tool`，`tool` 不是字符串，`id` 或 `apiVersion` 类型不对，或信封上有未知字段 |
| `UnsupportedVersion` | `apiVersion` 不是 1 |
| `UnknownTool` | `tool` 不在下表里 |
| `InvalidParams` | 这个工具的 `params` 缺字段、类型不对，或有未知字段。`message` 形如 `paint: missing colorIndex` |

内核错误原样穿过，`code` 与 `KernelError::code()` 相同，`message` 与它的 `Display` 相同：

*Kernel errors pass through. `code` is `KernelError::code()` and `message` is its `Display`:*

| code | 含义 Meaning |
| --- | --- |
| `OutOfBounds` | 格子超出画布 |
| `PaletteIndex` | 画笔索引是 0，或超出调色板 |
| `TransparentSlot` | 改了索引 0 |
| `PaletteLocked` | 锁定时改色、追加或导入 |
| `PaletteFile` | 导入文本不是 `.gpl` 或十六进制列表 |
| `PaletteImportMode` | `mode` 不是 `replace` 或 `merge` |
| `EmptyPalette` | 导入文件没有颜色 |
| `PaletteTooLarge` | 调色板会超过 256 色 |
| `FrameIndex` | 帧下标越界 |
| `LastFrame` | 删掉仅剩的一帧 |
| `FrameDelay` | `delayMs` 不在 `1..=65535` |
| `NothingToUndo` | 撤销栈是空的 |
| `NothingToRedo` | 重做栈是空的 |
| `OpenGesture` | 已经有一个未结束的 gesture，或重做时 gesture 里已经有改动 |
| `ExportScale` | 导出倍数是 0，或放大后的尺寸放不进一张 PNG |
| `SheetColumns` | sprite sheet 的列数是 0，或那么多列放不进一张 PNG |
| `ProjectFile` | `loadProject` 的文本不是 version 1 工程文件 |

`InvalidRequest` 的 `message`：`request is not JSON`、`request must be a JSON object`、`missing tool`、`tool must be a string`、`unknown field <name>`、`id must be a string or number`、`apiVersion must be an integer from 0 to 4294967295`。

`UnknownTool` 的 `message`：`unknown tool "<name>"`。

`UnsupportedVersion` 的 `message`：`tool api version <n> is not supported`。

## 工具 Tools

没有写出的 `result` 字段就是 `{}`。只读工具（`inspect`、`saveProject`、两个导出、`previewFrame`、`diffFrames`）不进撤销栈。和当前值相同的绘制、矩形填充、矩形擦除、矩形复制或移动、水平翻转、画布缩放、改色、改时长、锁定，以及不改变文档的导入，也不进栈。

撤销栈上限是 `MAX_UNDO`（100）。一次 `fillRect`、`eraseRect`、`copyRect`、`moveRect`、`flipHorizontal` 或 `resizeCanvas` 本身就是一步撤销。多次绘制或多块矩形要并成一步时，用 `beginGesture` … `endGesture` 包住。

*`result` is `{}` when a tool does not list fields. Read-only tools (`inspect`, `saveProject`, both exports, `previewFrame`, and `diffFrames`) do not push undo entries. A paint, rectangle fill, rectangle erase, rectangle copy or move, horizontal flip, canvas resize, recolor, delay, or lock that does not change the current value, and an import that does not change the document, are not recorded either.*

*The undo stack keeps `MAX_UNDO` (100) entries. One `fillRect`, `eraseRect`, `copyRect`, `moveRect`, `flipHorizontal`, or `resizeCanvas` is already one undo step. Wrap several paints or several rectangles in `beginGesture` … `endGesture` when they should undo together.*

### `inspect`

参数：无。

*Params: none.*

`result`：

| 字段 Field | 含义 Meaning |
| --- | --- |
| `width`, `height` | 画布边长，像素 |
| `palette` | `{ "r", "g", "b", "a" }` 数组。索引 0 是透明 |
| `paletteLocked` | 锁定后不能改色、追加、导入 |
| `frameCount` | 帧数，至少 1 |
| `activeFrame` | 当前帧，从 0 计 |
| `delayMs` | 当前帧的时长，毫秒 |
| `frameDelays` | 每一帧的时长，时间轴顺序 |
| `pixels` | 当前帧的调色板索引，行优先，长度 `width * height`。0 是空格子 |
| `canUndo`, `canRedo` | 撤销 / 重做是否可用 |
| `gestureOpen` | 是否有未结束的 gesture |

### `paint`

`params`：`x`、`y`（`0..=65535`，还须落在画布内）、`colorIndex`（`1..=255`，且必须是已有的不透明索引）。

写入的是索引，不是 RGB。锁定的调色板不阻止绘制。

*Writes an index, not RGB. A locked palette does not block paint.*

错误：`OutOfBounds`、`PaletteIndex`。

### `fillRect`

`params`：`x`、`y`（矩形左上角，含这一格，`0..=65535`，还须落在画布内）、`width`、`height`（`1..=65535`，矩形的每一格都须落在画布内）、`colorIndex`（`1..=255`，且必须是已有的不透明索引）。

在当前帧上把这块轴对齐矩形写成同一个索引。写入的是索引，不是 RGB。锁定的调色板不阻止这次填充。已经是该索引的格子不记入撤销。整块矩形是一步撤销；如果此时有未结束的 gesture，这些格子并进那一步。矩形里只要有一格出界，或者宽高加出来越过画布，整次调用失败，一格都不写。`width` 或 `height` 为 0、缺了、或不是该范围内的整数，是 `InvalidParams`（`message` 形如 `fillRect: width must be an integer from 1 to 65535`）。

*Fills an axis-aligned rectangle on the active frame with one palette index. `x` and `y` are the inclusive top-left cell. Writes an index, not RGB. A locked palette does not block the fill. Cells that already hold the index are not recorded. The rectangle is one undo step; an open gesture absorbs those cells. If any cell would fall outside the canvas, the call fails and writes nothing. A `width` or `height` of 0, a missing field, or a number outside `1..=65535` is `InvalidParams` (for example `fillRect: width must be an integer from 1 to 65535`).*

错误：`OutOfBounds`、`PaletteIndex`。

### `erase`

`params`：`x`、`y`。把该格设回索引 0。

错误：`OutOfBounds`。

### `eraseRect`

`params`：`x`、`y`（矩形左上角，含这一格）、`width`、`height`（`1..=65535`）。

在当前帧上把这块轴对齐矩形设回索引 0。边界和 `fillRect` 相同：只要有一格出界，整次失败，一格都不写。已经是透明的格子不记入撤销。整块是一步撤销；未结束的 gesture 会收进这些格子。锁定的调色板不阻止这次擦除。改剪影时用它，不要逐格 `erase`。

*Clears an axis-aligned rectangle on the active frame to index 0. Bounds match `fillRect`: if any cell would fall outside the canvas, the call fails and writes nothing. Cells that are already transparent are not recorded. The rectangle is one undo step; an open gesture absorbs those cells. A locked palette does not block the erase. Prefer this over erasing cell by cell when reshaping a silhouette.*

错误：`OutOfBounds`。`width` 或 `height` 为 0 是 `InvalidParams`。

### `copyRect`

`params`：`srcX`、`srcY`、`width`、`height`、`dstX`、`dstY`。宽高是 `1..=65535`。坐标是源矩形和目标矩形的左上角。

在一帧上把源矩形的调色板索引原样抄到另一帧的目标矩形，包括索引 0。源矩形保持不动。可选 `fromFrame` 是源帧，可选 `toFrame` 是目标帧；省略时都是当前帧。这次调用不改变 `activeFrame`。源或目标只要有一格出界，整次失败，一格都不写。同一帧上重叠时先读源，再写目标。改变了的格子是一步撤销；未结束的 gesture 会收进这些格子。没有变化不进栈。锁定的调色板不阻止这次复制。

*Copies palette indices, including 0, from one rectangle to another. Optional `fromFrame` is the source frame and optional `toFrame` is the destination; omit either one and it is the active frame. The call does not change `activeFrame`. The source stays. If the source or the destination would fall outside the canvas, the call fails and writes nothing. Overlap on the same frame reads the source before writing the destination. Changed cells are one undo step; an open gesture absorbs them. A copy that changes nothing is not recorded. A locked palette does not block the copy.*

错误：`OutOfBounds`、`FrameIndex`。

### `moveRect`

`params` 与 `copyRect` 相同，包括可选的 `fromFrame` 和 `toFrame`。

把源矩形的索引搬到目标矩形，包括索引 0。不在目标里的源格子变成索引 0。跨帧时目标在另一帧上，所以整个源矩形都会清掉。源和目标都要整块落在画布内，否则一格都不写。同一帧上重叠时以搬动前的源为准，落在目标里的源格子保留搬过去的索引，不会被清掉。整次移动是一步撤销。挪到自己身上等于没改，不进栈。不改变 `activeFrame`。

*Moves palette indices, including 0, to the destination and clears source cells that are not part of the destination. `fromFrame` and `toFrame` follow `copyRect`. Across frames the whole source rectangle is cleared, because the destination is on the other frame. Both rectangles must lie inside the canvas. Overlap on the same frame uses the source from before the move, so a source cell that lands inside the destination keeps the moved index. The move is one undo step. A move onto the same rectangle changes nothing and is not recorded. The active frame stays put.*

错误：`OutOfBounds`、`FrameIndex`。

### `setPaletteColor`

`params`：`index`、`r`、`g`、`b`，都是 `0..=255`。索引 0 不能改。格子仍保存原来的索引，所以每一帧用到该项的地方都会变成新颜色。

错误：`PaletteLocked`、`TransparentSlot`、`PaletteIndex`。

### `addPaletteColor`

`params`：`r`、`g`、`b`。在末尾追加一个不透明色。

`result`：`{ "index": <新索引> }`。

错误：`PaletteLocked`、`PaletteTooLarge`。

### `importPalette`

`params`：`text`（文件原文）、`mode`（`replace` 或 `merge`）。

格式与 `Document::import_palette` 相同，见 [dev-setup.md](dev-setup.md)。`replace` 用文件颜色换掉可绘制项，并把不再存在的索引清成透明。`merge` 只追加还没有的不透明颜色。索引 0 保持透明。一次导入是一步撤销。

错误：`PaletteLocked`、`PaletteImportMode`、`PaletteFile`、`EmptyPalette`、`PaletteTooLarge`。

### `setPaletteLocked`

`params`：`locked`（布尔）。`true` 锁定，`false` 解锁。画笔不受影响。同一个值不进撤销栈。这个标记会写入工程文件。

### `setActiveFrame`

`params`：`index`（`0..=4294967295`，还须是已有帧）。选中当前帧。相同的下标不进撤销栈。

错误：`FrameIndex`。

### `addFrame`

参数：无。在末尾追加一帧空白帧，时长 100 毫秒。当前帧不变。

`result`：`{ "index": <新帧> }`。

### `deleteFrame`

`params`：`index`。至少保留一帧。

错误：`FrameIndex`、`LastFrame`。只剩一帧时，任何删除都是 `LastFrame`，包括越界下标。

*Errors: `FrameIndex`, `LastFrame`. With one frame left, every delete returns `LastFrame`, including an index past the list.*

### `duplicateFrame`

`params`：`index`。把该帧（像素和时长）插到它后面。

`result`：`{ "index": <新帧> }`。

**不会把当前帧改成这份拷贝。** 返回的 `index` 才是新帧。要画拷贝，接着调用 `setActiveFrame`（CLI 是 `frame select`）。当前帧若在插入点或它后面，下标会加一，画面仍是原来那一帧；插在当前帧前面时，当前下标不变。网页里的「复制」按钮会在同一次 gesture 里再 `setActiveFrame` 到新下标。CLI 的 `frame duplicate` 和这个工具不会。

*Does not select the copy. `index` in the result is the new frame. Call `setActiveFrame` (CLI: `frame select`) before painting it. If the active frame sits at or after the insertion point, its index increases by one and it still shows the same pixels. An earlier active index stays put. The web editor's duplicate button then calls `setActiveFrame` on the new index inside the same gesture. `frame duplicate` and this tool do not.*

错误：`FrameIndex`。

### `setFrameDelay`

`params`：`index`、`delayMs`。`delayMs` 先要能放进 `u32`，内核再要求 `1..=65535`。

错误：`FrameIndex`、`FrameDelay`。

### `beginGesture` / `endGesture`

参数：无。把之后的成功修改收成一步撤销，直到 `endGesture`。嵌套的 `beginGesture` 返回 `OpenGesture`，已打开的 gesture 保持打开。没有打开的 `endGesture` 不报错，也不进栈。空的 gesture 不进栈。

### `undo` / `redo`

参数：无。撤销或重做最近一步内核操作。打开且已经有改动的 gesture 会先封口，再被一次 `undo` 整段撤销。

`result`：`{ "canUndo": <bool>, "canRedo": <bool> }`。

错误：`NothingToUndo`、`NothingToRedo`、`OpenGesture`（重做时 gesture 里已经有改动）。

### `saveProject`

参数：无。

`result`：`{ "json": "<pixelforge-project version 1>" }`。`json` 是内核写出的工程文本，含结尾换行。撤销栈和播放时钟不在文件里。

### `loadProject`

`params`：`json`（工程文件文本，不是对象）。解析成功后用 [`Document::load_project`] 换掉当前文档。新文档的撤销栈是空的，播放是停着的。

`result`：与 `inspect` 相同。

错误：`ProjectFile`。解析失败时原来的文档还在。

### `exportPngSequence`

`params`：可选 `scale`（整数 `1..=4294967295`，缺省为 1）。不改文档，不进撤销栈。

`scale` 在内核里做最近邻放大：每个调色板索引先重复成 `scale × scale` 的块，再解析成 RGBA，没有插值，也没有抗锯齿。`scale` 为 1 时和以前的字节相同。

`result`：`{ "frames": ["<base64>", ...], "scale", "width", "height" }`。每一项是时间轴上的一帧 RGBA PNG。`width` 和 `height` 是放大后的像素尺寸。Base64 是标准字母表，带 `=` 填充，没有换行，也没有 `data:` 前缀。

*`scale` is optional and defaults to 1. The kernel repeats each palette index into a `scale` by `scale` block, then resolves RGBA. There is no filtering. Scale 1 matches the previous bytes. `width` and `height` are the scaled pixel size.*

`scale` 为 0 或不是该范围的整数时是 `InvalidParams`。放大后放不进 PNG 时是 `ExportScale`。

### `exportSpriteSheet`

`params`：可选 `scale`，规则与 `exportPngSequence` 相同。不改文档，不进撤销栈。

`result`：

| 字段 Field | 含义 Meaning |
| --- | --- |
| `png` | 网格 sprite sheet 的 Base64 PNG |
| `columns`, `rows` | 未传 `columns` 时是 `ceil(sqrt(帧数))` 和 `ceil(帧数 / 列数)`。传入 `columns` 时用那个列数，行数是 `ceil(帧数 / columns)`。`scale` 不改变这两格 |
| `sheetWidth`, `sheetHeight` | 整张图的像素尺寸，已乘 `scale` |
| `scale` | 使用的倍数，缺省请求时为 1 |

可选 `columns`（整数 `1..=4294967295`）。缺省时仍是接近正方形的网格。走路或四方向可以传帧数，让一行就是一个方向。列数大于帧数时，多出来的格子是透明空位。`columns` 为 0 是 `InvalidParams`。列数让整张图放不进 PNG 时是 `SheetColumns`。

每一格帧变成 `帧宽 × scale` 乘 `帧高 × scale`。末尾的透明空位同样放大。排布和 Godot / Unity 约定见 [export.md](export.md)。

*Optional `columns` (an integer `1..=4294967295`). Omit it and the grid stays nearly square. Pass the frame count to keep one direction on one row. Columns past the frame count are transparent padding. `0` is `InvalidParams`. A column count that does not fit in a PNG is `SheetColumns`.*

*Each frame cell becomes `frame width × scale` by `frame height × scale`. Columns and rows stay the frame grid. Padding cells scale as transparent blocks.*

### `previewFrame`

`params`：可选 `frame`（缺省为当前帧）、可选 `scale`（规则与 `exportPngSequence` 相同，缺省为 1）。

只读。不改 `activeFrame`，不进撤销栈。`result`：

| 字段 Field | 含义 Meaning |
| --- | --- |
| `frame` | 画出的帧 |
| `activeFrame` | 文档当前帧，不会因为这次预览而改变 |
| `width`, `height` | 画布格子数 |
| `scale` | 使用的倍数 |
| `pngWidth`, `pngHeight` | 放大后的像素尺寸 |
| `png` | 这一帧的 Base64 RGBA PNG。内核先按 `scale` 最近邻重复索引，再解析调色板 |

CLI 的 `pixelforge preview` 另外把 ASCII 行放进 `result.rows`，并把 PNG 写到 `result.path`（默认是工程旁边的 `<stem>.preview.png`），stdout 里不带 base64。

*Read-only. Does not change `activeFrame` and does not push undo. The PNG is this frame after nearest-neighbor `scale`, then palette resolution. The `pixelforge preview` command also puts ASCII rows in `result.rows` and writes the PNG to `result.path` (default `<stem>.preview.png` beside the project) instead of base64.*

错误：`FrameIndex`、`ExportScale`。`scale` 为 0 是 `InvalidParams`。

### `diffFrames`

`params`：`frame`、`with`（都必填），可选 `scale`（缺省为 1）。

只读。比较两个帧的索引，不把洋葱皮写回像素。`result`：

| 字段 Field | 含义 Meaning |
| --- | --- |
| `frame`, `with` | 参与比较的两帧 |
| `activeFrame` | 文档当前帧 |
| `width`, `height` | 画布格子数 |
| `scale` | 使用的倍数 |
| `changed` | 索引不同的格子数 |
| `diffRows` | 一行一个字符串。`.` 表示索引相同；`!` 表示两边都非 0 且不同；否则是那个非 0 索引的字形（和 `preview` 相同：`1`–`9`、`A`–`Z`、`a`–`z`、`*`） |
| `panels` | `["frame", "with", "onion"]` |
| `pngWidth`, `pngHeight` | PNG 像素尺寸。宽是三格并排 |
| `png` | Base64 RGBA PNG。左是 `frame`，中是 `with`，右是洋葱皮 |

洋葱皮只存在于这张视图里：索引相同就用那一个调色板颜色；只有一边非 0 就用那边的颜色；两边都非 0 且不同时，把两个调色板颜色各通道取平均。平均色不写回工程，也不进调色板。

*Read-only. Compares indices and does not write onion skin back into pixels. `diffRows`: `.` same index, `!` both non-zero and different, otherwise the glyph of the only non-zero index. The PNG is three panels: `frame`, `with`, then the onion. The onion uses the shared palette color when indices match, the only ink when one side is transparent, and the average of the two palette colors when both differ. That average is a view. It is not stored and it is not a new palette entry.*

CLI 的 `pixelforge diff` 把 PNG 写到 `result.path`（默认 `<stem>.diff.png`），并在 stdout 里附上两帧的 ASCII `rows` / `withRows`。

错误：`FrameIndex`、`ExportScale`。缺 `frame` 或 `with` 是 `InvalidParams`。

### `flipHorizontal`

`params`：可选 `frame`（缺省为当前帧）。可选 `x`、`y`、`width`、`height`：四个一起给出就是那块矩形，四个都省略就是整帧。只给一部分是 `InvalidParams`。

把矩形里的索引左右对调。格子 `x` 收到镜像列上的索引：整帧宽 64 时就是 `63 - x`。不改调色板，也不改高光颜色。一步撤销。对称的矩形不进栈。不改变 `activeFrame`。出界则一格都不写。

*Mirrors indices left to right. Cell `x` receives the index from the mirrored column (`63 - x` on a full frame of width 64). The palette is unchanged, including highlight colors. One undo step. A symmetric rectangle is not recorded. `frame` is not selected. An out-of-bounds rectangle writes nothing.*

错误：`OutOfBounds`、`FrameIndex`。

### `resizeCanvas`

`params`：`width`、`height`（`1..=256`）。可选 `offsetX`、`offsetY`（`i32`，可负）。两个偏移都省略时居中：`offset = (new - old) / 2`，向 0 取整。只给一个是 `InvalidParams`。

把每一帧的索引拷进新画布。旧原点落在 `(offsetX, offsetY)`。没有源的格子是索引 0。落在新画布外的旧格子被裁掉。时长、调色板、锁定和当前帧不变。工程仍是 version 1，只是 `width`、`height` 和各帧 `pixels` 变了。一步撤销能回到原来的缓冲。尺寸和索引都没变时不进栈。

*Copies every frame's indices onto the new canvas. The old origin lands at `(offsetX, offsetY)`. Omit both offsets to center with `offset = (new - old) / 2`, truncating toward zero. Cells without a source stay index 0. Old cells that fall outside are clipped. Delays, the palette, the lock, and the active frame stay. The file stays version 1. One undo restores the previous buffers. A resize that leaves every index in place is not recorded.*

`result`：`{ "width", "height", "offsetX", "offsetY" }`，偏移是实际使用的值。

错误：`InvalidSize`（内核拒绝的边长）。`width` 超出 `1..=256` 是 `InvalidParams`。

## 不变量 Invariants

- 改像素、调色板、帧和时长只通过 `Document` 的公开方法。Tool API 不写第二份缓冲。
- 这些修改记在同一条撤销栈上。界面随后 `undo`，撤销的是 Agent 的那一步。
- 非法下标、锁定的调色板、坏的工程文件都返回上表里的 `error`，并且不留下半次修改。
- 洋葱皮只是视图。`diffFrames` 会返回一张，但不会把它写回像素。这个接口没有可编辑的洋葱层。

*Pixel, palette, frame, and delay edits go through `Document`'s public methods only. The Tool API does not keep a second buffer.*

*Those edits share the undo stack. A later UI `undo` reverts the agent's step.*

*A bad index, a locked palette, and a bad project file return an `error` from the tables above and do not leave a partial edit.*

*Onion skin is a view. `diffFrames` returns one and does not write it back into pixels. This API has no tool that stores an onion layer.*
