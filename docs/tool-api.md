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
| `LayerIndex` | 层下标越界 |
| `LastLayer` | 删掉仅剩的一层 |
| `LayerName` | 层名是空的、只有空白、含控制字符，或超过 64 个字符 |
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

没有写出的 `result` 字段就是 `{}`。只读工具（`inspect`、`scene`、`saveProject`、两个导出、`previewFrame`、`diffFrames`、`listLayers`）不进撤销栈。和当前值相同的绘制、矩形填充、矩形擦除、矩形复制或移动、水平翻转、画布缩放、改色、改时长、锁定、改层名、选层，以及不改变文档的导入和压平，也不进栈。

`paint`、`erase`、`fillRect`、`eraseRect`、`copyRect`、`moveRect`、`flipHorizontal`、`resizeCanvas` 可以带可选布尔 `reportDirty`。省略或 `false` 时，这些工具的 `result` 与以前相同。见下面的「脏区」。这不是命令日志。

撤销栈上限是 `MAX_UNDO`（100）。一次 `fillRect`、`eraseRect`、`copyRect`、`moveRect`、`flipHorizontal` 或 `resizeCanvas` 本身就是一步撤销。多次绘制或多块矩形要并成一步时，用 `beginGesture` … `endGesture` 包住。

*`result` is `{}` when a tool does not list fields. Read-only tools (`inspect`, `scene`, `saveProject`, both exports, `previewFrame`, `diffFrames`, and `listLayers`) do not push undo entries. A paint, rectangle fill, rectangle erase, rectangle copy or move, horizontal flip, canvas resize, recolor, delay, or lock that does not change the current value, and an import that does not change the document, are not recorded either.*

*`paint`, `erase`, `fillRect`, `eraseRect`, `copyRect`, `moveRect`, `flipHorizontal`, and `resizeCanvas` take an optional boolean `reportDirty`. Omit it, or send `false`, and those results stay as they were. See Dirty region below. This is not a command log.*

*The undo stack keeps `MAX_UNDO` (100) entries. One `fillRect`, `eraseRect`, `copyRect`, `moveRect`, `flipHorizontal`, or `resizeCanvas` is already one undo step. Wrap several paints or several rectangles in `beginGesture` … `endGesture` when they should undo together.*

### `inspect`

参数：无。

*Params: none.*

`result`：

| 字段 Field | 含义 Meaning |
| --- | --- |
| `width`, `height` | 画布边长，像素 |
| `palette` | `{ "r", "g", "b", "a" }` 数组。索引 0 是透明 |
| `paletteHash` | 调色板的 FNV-1a 64 位哈希，16 位小写十六进制。算法见 `scene` |
| `paletteLocked` | 锁定后不能改色、追加、导入 |
| `frameCount` | 帧数，至少 1 |
| `activeFrame` | 当前帧，从 0 计 |
| `delayMs` | 当前帧的时长，毫秒 |
| `frameDelays` | 每一帧的时长，时间轴顺序 |
| `pixels` | **当前层**、当前帧的调色板索引，行优先，长度 `width * height`。0 是空格子。这是下一次绘制会改的缓冲，不是合成图 |
| `composite` | 当前帧按层序叠出来的索引，规则见下面的「图层」 |
| `layerCount` | 层数，至少 1。每一帧都有同样多的层 |
| `activeLayer` | 当前层，从 0 计。0 在最下面 |
| `layers` | `{ "index", "name" }` 数组，从底到顶 |
| `canUndo`, `canRedo` | 撤销 / 重做是否可用 |
| `gestureOpen` | 是否有未结束的 gesture |

`inspect` 带上当前层的 `pixels` 和合成 `composite`。只要画布、层、调色板和帧时长时，用 `scene`。

### `scene`

参数：无。只读。不进撤销栈。不返回像素缓冲。

*Params: none. Read-only. Does not push undo. Does not return a pixel buffer.*

`result`：

| 字段 Field | 含义 Meaning |
| --- | --- |
| `width`, `height` | 画布边长，格子 |
| `activeFrame`, `activeLayer` | 当前帧、当前层，从 0 计 |
| `layerCount` | 层数，至少 1 |
| `layers` | `{ "index", "name", "visible", "opacity" }`，从底到顶 |
| `paletteLocked` | 调色板是否锁定 |
| `paletteCount` | 调色板项数，含索引 0 |
| `paletteHash` | 见下 |
| `palette` | 与 `inspect` 相同的 `{ "r", "g", "b", "a" }` 数组 |
| `frameCount` | 帧数 |
| `delayMs` | 当前帧时长，毫秒 |
| `frames` | `{ "index", "delayMs" }`，时间轴顺序 |

`visible` 恒为 `true`。`opacity` 恒为 `null`。内核没有逐层隐藏，也没有逐层透明度。非 0 索引盖住下层，不按 alpha 混合。

`paletteHash` 是 FNV-1a 64 位。初值 `14695981039346656037`，乘数 `1099511628211`。按索引顺序把每一项的 `r`、`g`、`b`、`a` 各作为一个字节送进去。结果是 16 位小写十六进制。颜色和顺序不变，哈希就不变。它用来认出调色板，不是口令。

CLI：`pixelforge scene --project hero.json`。

*Every layer is `visible: true`. `opacity` is `null`. The kernel does not store a hide flag or a per-layer opacity. A non-zero index covers the layer under it and does not blend. `paletteHash` is FNV-1a 64-bit: offset basis `14695981039346656037`, prime `1099511628211`, each entry's `r`, `g`, `b`, `a` in index order, written as 16 lowercase hex digits. The same colors in the same order hash the same. It identifies the palette. It is not a secret.*

### 脏区 Dirty region

`paint`、`erase`、`fillRect`、`eraseRect`、`copyRect`、`moveRect`、`flipHorizontal`、`resizeCanvas` 的 `params` 可以多一个 `reportDirty`（布尔）。别的工具带上这个字段仍是 `InvalidParams`。

`true` 时，在这个工具原有的 `result` 字段上再加：

| 字段 Field | 含义 Meaning |
| --- | --- |
| `changed` | 索引变了的格子数。一格都没变是 `0` |
| `dirty` | 这些格子的外接矩形 `{ "x", "y", "width", "height" }`。`changed` 为 0 时是 `null` |
| `regions` | 每个改过的帧和层一条：`frame`、`layer`、`changed`、`x`、`y`、`width`、`height`。没改的层不出现 |
| `clipped` | 只有 `resizeCanvas` 写这个字段。新画布上已经没有的坐标里，原来非 0 的格子数 |

矩形包住的是索引真的变了的格子，不是请求里的整块矩形。同一坐标上，新索引和旧索引不同才算 `changed`。画布变大时，新坐标上的 0 不算变化。跨帧时 `dirty` 是各 `regions` 的并集，帧别看 `regions`。`resizeCanvas` 里，坐标还在、只是索引换了，算 `changed`，不算 `clipped`。

CLI 的 `--dirty` 就是 `reportDirty: true`。不写这个旗标，结果里就没有这些字段。

*The box covers cells whose stored index changed, not the whole rectangle the tool was asked to touch. A new cell that stays 0 is not a change. Across frames, `dirty` is the union of `regions`. On `resizeCanvas`, a cell that still has a coordinate and holds a different index counts as `changed`, not `clipped`. The CLI flag `--dirty` sends `reportDirty: true`. Without it, these fields are absent.*

### `paint`

`params`：`x`、`y`（`0..=65535`，还须落在画布内）、`colorIndex`（`1..=255`，且必须是已有的不透明索引）。

写入的是索引，不是 RGB。锁定的调色板不阻止绘制。可选 `layer` 指定要改的层，不写就是当前层；这次调用不改变 `activeLayer`。别的层一格都不动。

*Writes an index, not RGB. A locked palette does not block paint. Optional `layer` chooses the layer and does not change `activeLayer`. Other layers stay put.*

错误：`OutOfBounds`、`PaletteIndex`、`LayerIndex`。

### `fillRect`

`params`：`x`、`y`（矩形左上角，含这一格，`0..=65535`，还须落在画布内）、`width`、`height`（`1..=65535`，矩形的每一格都须落在画布内）、`colorIndex`（`1..=255`，且必须是已有的不透明索引）。

在当前帧上把这块轴对齐矩形写成同一个索引。写入的是索引，不是 RGB。锁定的调色板不阻止这次填充。已经是该索引的格子不记入撤销。整块矩形是一步撤销；如果此时有未结束的 gesture，这些格子并进那一步。矩形里只要有一格出界，或者宽高加出来越过画布，整次调用失败，一格都不写。`width` 或 `height` 为 0、缺了、或不是该范围内的整数，是 `InvalidParams`（`message` 形如 `fillRect: width must be an integer from 1 to 65535`）。

*Fills an axis-aligned rectangle on the active frame with one palette index. `x` and `y` are the inclusive top-left cell. Writes an index, not RGB. A locked palette does not block the fill. Cells that already hold the index are not recorded. The rectangle is one undo step; an open gesture absorbs those cells. If any cell would fall outside the canvas, the call fails and writes nothing. A `width` or `height` of 0, a missing field, or a number outside `1..=65535` is `InvalidParams` (for example `fillRect: width must be an integer from 1 to 65535`).*

错误：`OutOfBounds`、`PaletteIndex`、`LayerIndex`。可选 `layer` 与 `paint` 相同。

### `erase`

`params`：`x`、`y`。可选 `layer`。把该格设回索引 0。只改这一层。

错误：`OutOfBounds`、`LayerIndex`。

### `eraseRect`

`params`：`x`、`y`（矩形左上角，含这一格）、`width`、`height`（`1..=65535`）。

在当前帧上把这块轴对齐矩形设回索引 0。边界和 `fillRect` 相同：只要有一格出界，整次失败，一格都不写。已经是透明的格子不记入撤销。整块是一步撤销；未结束的 gesture 会收进这些格子。锁定的调色板不阻止这次擦除。改剪影时用它，不要逐格 `erase`。

*Clears an axis-aligned rectangle on the active frame to index 0. Bounds match `fillRect`: if any cell would fall outside the canvas, the call fails and writes nothing. Cells that are already transparent are not recorded. The rectangle is one undo step; an open gesture absorbs those cells. A locked palette does not block the erase. Prefer this over erasing cell by cell when reshaping a silhouette.*

错误：`OutOfBounds`、`LayerIndex`。`width` 或 `height` 为 0 是 `InvalidParams`。可选 `layer` 与 `paint` 相同。

### `copyRect`

`params`：`srcX`、`srcY`、`width`、`height`、`dstX`、`dstY`。宽高是 `1..=65535`。坐标是源矩形和目标矩形的左上角。

在一帧上把源矩形的调色板索引原样抄到另一帧的目标矩形，包括索引 0。源矩形保持不动。可选 `fromFrame` 是源帧，可选 `toFrame` 是目标帧；省略时都是当前帧。这次调用不改变 `activeFrame`。源或目标只要有一格出界，整次失败，一格都不写。同一帧上重叠时先读源，再写目标。改变了的格子是一步撤销；未结束的 gesture 会收进这些格子。没有变化不进栈。锁定的调色板不阻止这次复制。

*Copies palette indices, including 0, from one rectangle to another. Optional `fromFrame` is the source frame and optional `toFrame` is the destination; omit either one and it is the active frame. The call does not change `activeFrame`. The source stays. If the source or the destination would fall outside the canvas, the call fails and writes nothing. Overlap on the same frame reads the source before writing the destination. Changed cells are one undo step; an open gesture absorbs them. A copy that changes nothing is not recorded. A locked palette does not block the copy.*

错误：`OutOfBounds`、`FrameIndex`、`LayerIndex`。可选 `layer`：源和目标都用这一层，不在层与层之间抄像素，也不改变 `activeLayer`。

### `moveRect`

`params` 与 `copyRect` 相同，包括可选的 `fromFrame` 和 `toFrame`。

把源矩形的索引搬到目标矩形，包括索引 0。不在目标里的源格子变成索引 0。跨帧时目标在另一帧上，所以整个源矩形都会清掉。源和目标都要整块落在画布内，否则一格都不写。同一帧上重叠时以搬动前的源为准，落在目标里的源格子保留搬过去的索引，不会被清掉。整次移动是一步撤销。挪到自己身上等于没改，不进栈。不改变 `activeFrame`。

*Moves palette indices, including 0, to the destination and clears source cells that are not part of the destination. `fromFrame` and `toFrame` follow `copyRect`. Across frames the whole source rectangle is cleared, because the destination is on the other frame. Both rectangles must lie inside the canvas. Overlap on the same frame uses the source from before the move, so a source cell that lands inside the destination keeps the moved index. The move is one undo step. A move onto the same rectangle changes nothing and is not recorded. The active frame stays put.*

错误：`OutOfBounds`、`FrameIndex`、`LayerIndex`。可选 `layer` 与 `copyRect` 相同。

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

`params`：可选 `frame`（缺省为当前帧）、可选 `scale`（规则与 `exportPngSequence` 相同，缺省为 1）、可选 `layer`。不写 `layer` 时画合成图；写了就只画那一层，并且不改变 `activeLayer`。

只读。不改 `activeFrame`，不进撤销栈。`result`：

| 字段 Field | 含义 Meaning |
| --- | --- |
| `frame` | 画出的帧 |
| `activeFrame` | 文档当前帧，不会因为这次预览而改变 |
| `activeLayer` | 文档当前层 |
| `layer` | 画出的层。合成图时是 `null` |
| `composite` | 没指定 `layer` 时为 `true` |
| `width`, `height` | 画布格子数 |
| `scale` | 使用的倍数 |
| `ink` | 非 0 格子数 |
| `bounds` | 这些格子的外接矩形 `{ "x", "y", "width", "height" }`。没有墨时是 `null` |
| `indices` | `{ "index", "count" }`，按索引升序，只用到的索引，含 0 |
| `pixels` | 这一张视图的调色板索引，行优先。合成或单层，由 `layer` 决定 |
| `pngWidth`, `pngHeight` | 放大后的像素尺寸 |
| `png` | 这一帧的 Base64 RGBA PNG。内核先按 `scale` 最近邻重复索引，再解析调色板 |

`pixels` 是索引，不是 RGB。字形 `*` 会把 62 及以上的索引折成同一个字符，读索引用 `pixels` 或 `indices`。

CLI 的 `pixelforge preview` 另外把 ASCII 行放进 `result.rows`，并把 PNG 写到 `result.path`（默认是工程旁边的 `<stem>.preview.png`），stdout 里不带 base64。`ink`、`bounds`、`indices`、`pixels` 仍在这条 JSON 里。

*Read-only. Does not change `activeFrame` and does not push undo. The PNG is this frame after nearest-neighbor `scale`, then palette resolution. `pixels` are palette indices, row-major. The glyph `*` collapses every index from 62 up, so read `pixels` or `indices` for the real index. The `pixelforge preview` command also puts ASCII rows in `result.rows` and writes the PNG to `result.path` (default `<stem>.preview.png` beside the project) instead of base64. `ink`, `bounds`, `indices`, and `pixels` stay on that JSON line.*

错误：`FrameIndex`、`LayerIndex`、`ExportScale`。`scale` 为 0 是 `InvalidParams`。

### `diffFrames`

`params`：`frame`、`with`（都必填），可选 `scale`（缺省为 1），可选 `layer`。不写 `layer` 时比较两帧的合成索引；写了就只比较那一层。

只读。比较索引，不把洋葱皮写回像素。`result`：

| 字段 Field | 含义 Meaning |
| --- | --- |
| `frame`, `with` | 参与比较的两帧 |
| `activeFrame` | 文档当前帧 |
| `activeLayer` | 文档当前层 |
| `layer` | 参与比较的层。合成图时是 `null` |
| `composite` | 没指定 `layer` 时为 `true` |
| `width`, `height` | 画布格子数 |
| `scale` | 使用的倍数 |
| `changed` | 索引不同的格子数 |
| `bounds` | 这些格子的外接矩形。`changed` 为 0 时是 `null` |
| `kinds` | `{ "same", "differ", "onlyFrame", "onlyWith" }`。`differ` 是两边都非 0 且不同；`onlyFrame` 只有 `frame` 有墨；`onlyWith` 只有 `with` 有墨 |
| `changes` | `{ "x", "y", "left", "right" }`，行优先。`left` 是 `frame` 的索引，`right` 是 `with` 的索引。只列不同的格子 |
| `diffRows` | 一行一个字符串。`.` 表示索引相同；`!` 表示两边都非 0 且不同；否则是那个非 0 索引的字形（和 `preview` 相同：`1`–`9`、`A`–`Z`、`a`–`z`、`*`） |
| `panels` | `["frame", "with", "onion"]` |
| `pngWidth`, `pngHeight` | PNG 像素尺寸。宽是三格并排 |
| `png` | Base64 RGBA PNG。左是 `frame`，中是 `with`，右是洋葱皮 |

洋葱皮只存在于这张视图里：索引相同就用那一个调色板颜色；只有一边非 0 就用那边的颜色；两边都非 0 且不同时，把两个调色板颜色各通道取平均。平均色不写回工程，也不进调色板。

*Read-only. Compares indices and does not write onion skin back into pixels. `diffRows`: `.` same index, `!` both non-zero and different, otherwise the glyph of the only non-zero index. The PNG is three panels: `frame`, `with`, then the onion. The onion uses the shared palette color when indices match, the only ink when one side is transparent, and the average of the two palette colors when both differ. That average is a view. It is not stored and it is not a new palette entry.*

`!` 不带两边的索引。要索引就读 `changes`。

CLI 的 `pixelforge diff` 把 PNG 写到 `result.path`（默认 `<stem>.diff.png`），并在 stdout 里附上两帧的 ASCII `rows` / `withRows`。`bounds`、`kinds`、`changes` 仍在这条 JSON 里。

*The `!` glyph does not carry either index. Read `changes` for `left` and `right`. The CLI keeps `bounds`, `kinds`, and `changes` on the JSON line.*

错误：`FrameIndex`、`LayerIndex`、`ExportScale`。缺 `frame` 或 `with` 是 `InvalidParams`。

### `flipHorizontal`

`params`：可选 `frame`（缺省为当前帧）。可选 `x`、`y`、`width`、`height`：四个一起给出就是那块矩形，四个都省略就是整帧。只给一部分是 `InvalidParams`。

把矩形里的索引左右对调。格子 `x` 收到镜像列上的索引：整帧宽 64 时就是 `63 - x`。不改调色板，也不改高光颜色。一步撤销。对称的矩形不进栈。不改变 `activeFrame`。出界则一格都不写。

*Mirrors indices left to right. Cell `x` receives the index from the mirrored column (`63 - x` on a full frame of width 64). The palette is unchanged, including highlight colors. One undo step. A symmetric rectangle is not recorded. `frame` is not selected. An out-of-bounds rectangle writes nothing.*

错误：`OutOfBounds`、`FrameIndex`、`LayerIndex`。可选 `layer`：只镜像这一层。

### `resizeCanvas`

`params`：`width`、`height`（`1..=256`）。可选 `offsetX`、`offsetY`（`i32`，可负）。两个偏移都省略时居中：`offset = (new - old) / 2`，向 0 取整。只给一个是 `InvalidParams`。

把每一帧的索引拷进新画布。旧原点落在 `(offsetX, offsetY)`。没有源的格子是索引 0。落在新画布外的旧格子被裁掉。时长、调色板、锁定和当前帧不变。工程仍是 version 1，只是 `width`、`height` 和各帧 `pixels` 变了。一步撤销能回到原来的缓冲。尺寸和索引都没变时不进栈。

*Copies every frame's indices onto the new canvas. The old origin lands at `(offsetX, offsetY)`. Omit both offsets to center with `offset = (new - old) / 2`, truncating toward zero. Cells without a source stay index 0. Old cells that fall outside are clipped. Delays, the palette, the lock, and the active frame stay. The file stays version 1. One undo restores the previous buffers. A resize that leaves every index in place is not recorded.*

`result`：`{ "width", "height", "offsetX", "offsetY" }`，偏移是实际使用的值。

错误：`InvalidSize`（内核拒绝的边长）。`width` 超出 `1..=256` 是 `InvalidParams`。每一层都跟着缩放。

## 图层 Layers

层是文档级的栈，每一帧各有一块同样尺寸的索引缓冲。下标 0 在最下面，最后一层画在最上面。新建工程和旧的单层文件都是一层，名字是 `Layer 1`，`activeLayer` 是 0。这种文件保存时仍不写 `layers` / `activeLayer`，和以前的 version 1 工程相同。多了层、改了名字，或当前层不是 0 时，文件里才出现这些字段。帧上多于一层时，像素写在 `frames[].layers[].pixels`，不再写旧的 `frames[].pixels`。

合成规则：从底往上叠索引。索引 0 是透明，露出下面的层。任何非 0 索引直接盖住下面的层，不按调色板 alpha 做混合。导出再把赢下的那个索引解析成 RGBA，和单层时一样。

`preview`、`diff`、`export png`、`export sheet` 默认看合成结果。`previewFrame` / `diffFrames` 以及 CLI 的 `--layer N` 只看那一层。绘制工具的 `layer` 只决定改哪一层。

### `listLayers`

参数：无。只读。

`result`：`{ "layerCount", "activeLayer", "layers": [{ "index", "name" }] }`。

### `addLayer`

`params`：可选 `name`。省略时用还没被占用的 `Layer N`，从「当前层数 + 1」起找。名字会去掉首尾空白，长度 1 到 64。新层插在当前层上面，每一帧都是透明索引，并成为当前层。一步撤销。

`result`：`{ "index", "name", "activeLayer" }`。

错误：`LayerName`。

### `deleteLayer`

`params`：可选 `index`，省略时删当前层。至少留一层。删掉当前层时，滑到这个下标上的那一层保持选中；删的是最上面一层时，选中新的最上面一层。

`result`：`{ "index", "activeLayer", "layerCount" }`。`index` 是被删掉的那一层。

错误：`LayerIndex`、`LastLayer`。只剩一层时，任何删除都是 `LastLayer`，包括越界下标。

### `renameLayer`

`params`：`name`，可选 `index`（省略时是当前层）。同一个名字不进撤销栈。

`result`：`{ "index", "name" }`，`name` 是去掉空白之后的名字。

错误：`LayerIndex`、`LayerName`。

### `setActiveLayer`

`params`：`index`。相同下标不进撤销栈。

`result`：`{ "index" }`。

错误：`LayerIndex`。

### `reorderLayer`

`params`：`from`、`to`。`to` 是移动之后的下标。相同下标不进栈。当前层跟着被移动的那一层走，夹在中间的层下标加减一。0 仍然是最下面。

`result`：`{ "from", "to", "activeLayer" }`。

错误：`LayerIndex`。

### `flattenLayers`

参数：无。每一帧按上面的规则压成一层，名字改回 `Layer 1`，`activeLayer` 回到 0。保存出来就是旧的单层工程。已经是这一层时不进栈。一步撤销能回到原来的栈。

`result`：`{ "layerCount": 1, "activeLayer": 0, "name": "Layer 1" }`。

CLI：`layer list`、`layer add --name gear`、`layer remove`（别名 `delete`）、`layer rename`、`layer select`、`layer reorder --from --to`、`layer flatten`。

*Layers are a document-level stack. Every frame has one index buffer per layer, the same size as the canvas. Index 0 is the bottom. A new project and an old single-layer file are one layer named `Layer 1`. That file is still saved without `layers` or `activeLayer`. A stack, a renamed layer, or a non-zero active layer writes those fields. Frames with more than one layer store `frames[].layers[].pixels` and omit the legacy `pixels` array.*

*Composite from bottom to top. Index 0 is transparent and shows the layer under it. Any other index covers that layer. Palette alpha is not blended between layers. Export then resolves the winning index to RGBA, the same as a single layer.*

*`preview`, `diff`, and `export png` / `export sheet` show the composite. `previewFrame`, `diffFrames`, and CLI `--layer N` show one layer. A `layer` param on a drawing tool chooses where the indices go and does not change `activeLayer`.*

*`addLayer` inserts a transparent layer above the active one on every frame and selects it. `deleteLayer` refuses the last layer. `reorderLayer`'s `to` is the index after the move. `flattenLayers` writes the composite back into one layer named `Layer 1`.*

## 不变量 Invariants

- 改像素、调色板、帧和时长只通过 `Document` 的公开方法。Tool API 不写第二份缓冲。
- 这些修改记在同一条撤销栈上。界面随后 `undo`，撤销的是 Agent 的那一步。
- 非法下标、锁定的调色板、坏的工程文件都返回上表里的 `error`，并且不留下半次修改。
- 洋葱皮只是视图。`diffFrames` 会返回一张，但不会把它写回像素。这个接口没有可编辑的洋葱层。

*Pixel, palette, frame, and delay edits go through `Document`'s public methods only. The Tool API does not keep a second buffer.*

*Those edits share the undo stack. A later UI `undo` reverts the agent's step.*

*A bad index, a locked palette, and a bad project file return an `error` from the tables above and do not leave a partial edit.*

*Onion skin is a view. `diffFrames` returns one and does not write it back into pixels. This API has no tool that stores an onion layer.*
