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
| `NoPaintColor` | 量化或 PNG 压盘时，调色板没有不透明色可映射 |
| `PngFile` | `applyPng` 的字节不是可解码的图像 |
| `PngSize` | PNG 的宽高不是画布大小。压盘不缩放 |
| `FrameIndex` | 帧下标越界 |
| `LastFrame` | 删掉仅剩的一帧 |
| `LayerIndex` | 层下标越界 |
| `LastLayer` | 删掉仅剩的一层 |
| `LayerName` | 层名是空的、只有空白、含控制字符，或超过 64 个字符 |
| `FrameDelay` | `delayMs` 不在 `1..=65535` |
| `TagName` | 标签名是空的、只有空白、含控制字符，或超过 64 个字符 |
| `TagExists` | 已经有同名标签 |
| `TagMissing` | 没有这个名字的标签 |
| `TagRange` | `from` 大于 `to`，或闭区间超出帧列表 |
| `PartName` | 部件名是空的、只有空白、含控制字符，或超过 64 个字符 |
| `PartExists` | 已经有同名部件 |
| `PartMissing` | 没有这个名字的部件 |
| `AnchorName` | 锚点名是空的、只有空白、含控制字符，或超过 64 个字符 |
| `AnchorMissing` | 这个部件上没有这个名字的锚点 |
| `AnchorCoord` | 锚点的本地 `x` 或 `y` 不在 `-256..=256` |
| `PartAngle` | 部件角度不是有限数，或绝对值大于 1000000 |
| `PartOffCanvas` | 部件变换会把原点或某个非 0 格放到画布外。一格都不写 |
| `NothingToUndo` | 撤销栈是空的 |
| `NothingToRedo` | 重做栈是空的 |
| `OpenGesture` | 已经有一个未结束的 gesture，或重做时 gesture 里已经有改动 |
| `SelectionProtected` | `moveRect` 的源矩形或目标矩形有格子在选区外。部件变换时，挂接层上有非 0 格或其落点不在选区内。整次调用不写 |
| `ExportScale` | 导出倍数是 0，或放大后的尺寸放不进一张 PNG |
| `SheetColumns` | sprite sheet 的列数是 0，或那么多列放不进一张 PNG |
| `ProjectFile` | `loadProject` 的文本不是 version 1 工程文件。`selection` 不是画布内的矩形，某个 `tags` 项的名字、区间、循环模式、重名不合法，或某个 `parts` 项的名字、原点、层、锚点不合法时，也是这个码 |
| `AsepriteFile` | `.aseprite` 字节截断、魔数不对、压缩 cel 解不开，或层 / cel 结构读不完 |
| `AsepriteSize` | 画布边长不在 `1..=256` |
| `AsepriteColorMode` | 颜色深度不是索引 8 位 |
| `AsepritePalette` | 没有可用的索引调色板 |
| `AsepriteTilemap` | 文件含 tilemap 层或 tilemap cel。整文件不导入 |
| `AsepritePixel` | 像素索引超出调色板长度 |
| `AsepriteLayers` | 跳过组和参考层之后，没有图像层可导入 |
| `RotateSquare` | `rotate` 的 90° 或 270° 矩形不是正方形。一格都不写 |

`InvalidRequest` 的 `message`：`request is not JSON`、`request must be a JSON object`、`missing tool`、`tool must be a string`、`unknown field <name>`、`id must be a string or number`、`apiVersion must be an integer from 0 to 4294967295`。

`UnknownTool` 的 `message`：`unknown tool "<name>"`。

`UnsupportedVersion` 的 `message`：`tool api version <n> is not supported`。

## 工具 Tools

没有写出的 `result` 字段就是 `{}`。只读工具（`inspect`、`scene`、`saveProject`、两个导出、`previewFrame`、`diffFrames`、`listLayers`、`listParts`）不进撤销栈。和当前值相同的绘制、矩形填充、矩形擦除、矩形复制或移动、水平翻转、垂直翻转、矩形旋转、画布缩放、改色、改时长、锁定、索引策略、改层名、选层、改标签、改部件、改锚点、部件平移、部件旋转、部件翻转，以及不改变文档的导入、重映射、PNG 压盘和压平，也不进栈。`importAseprite` 和 `loadProject` 一样换掉整份文档，新撤销栈是空的。

`paint`、`erase`、`fillRect`、`eraseRect`、`copyRect`、`moveRect`、`flipHorizontal`、`flipVertical`、`rotate`、`resizeCanvas`、`translatePart`、`rotatePart`、`flipPart` 可以带可选布尔 `reportDirty`。省略或 `false` 时，这些工具的 `result` 与以前相同。见下面的「脏区」。这不是命令日志。

撤销栈上限是 `MAX_UNDO`（100）。一次 `fillRect`、`eraseRect`、`copyRect`、`moveRect`、`flipHorizontal`、`flipVertical`、`rotate`、`resizeCanvas`、`translatePart`、`rotatePart` 或 `flipPart` 本身就是一步撤销。多次绘制或多块矩形要并成一步时，用 `beginGesture` … `endGesture` 包住。

*`result` is `{}` when a tool does not list fields. Read-only tools (`inspect`, `scene`, `saveProject`, both exports, `previewFrame`, `diffFrames`, `listLayers`, and `listParts`) do not push undo entries. A paint, rectangle fill, rectangle erase, rectangle copy or move, horizontal flip, vertical flip, rectangle rotation, canvas resize, recolor, delay, lock, index policy, tag edit, part edit, anchor edit, or part translate / rotate / flip that does not change the current value, and an import, remap, or PNG crush that does not change the document, are not recorded either. `importAseprite`, like `loadProject`, replaces the document and starts with an empty undo stack.*

*`paint`, `erase`, `fillRect`, `eraseRect`, `copyRect`, `moveRect`, `flipHorizontal`, `flipVertical`, `rotate`, `resizeCanvas`, `translatePart`, `rotatePart`, and `flipPart` take an optional boolean `reportDirty`. Omit it, or send `false`, and those results stay as they were. See Dirty region below. This is not a command log.*

*The undo stack keeps `MAX_UNDO` (100) entries. One `fillRect`, `eraseRect`, `copyRect`, `moveRect`, `flipHorizontal`, `flipVertical`, `rotate`, `resizeCanvas`, `translatePart`, `rotatePart`, or `flipPart` is already one undo step. Wrap several paints or several rectangles in `beginGesture` … `endGesture` when they should undo together.*

### `inspect`

参数：无。

*Params: none.*

`result`：

| 字段 Field | 含义 Meaning |
| --- | --- |
| `width`, `height` | 画布边长，像素 |
| `palette` | `{ "r", "g", "b", "a" }` 数组。索引 0 是透明 |
| `paletteHash` | 调色板的 FNV-1a 64 位哈希，16 位小写十六进制。算法见 `scene` |
| `paletteLocked` | 锁定后不能改色、追加、导入、重映射 |
| `indexPolicy` | `reject`（默认）或 `clamp`。见 `setIndexPolicy` |
| `frameCount` | 帧数，至少 1 |
| `activeFrame` | 当前帧，从 0 计 |
| `delayMs` | 当前帧的时长，毫秒 |
| `frameDelays` | 每一帧的时长，时间轴顺序 |
| `pixels` | **当前层**、当前帧的调色板索引，行优先，长度 `width * height`。0 是空格子。这是下一次绘制会改的缓冲，不是合成图 |
| `composite` | 当前帧按层序叠出来的索引，规则见下面的「图层」 |
| `layerCount` | 层数，至少 1。每一帧都有同样多的层 |
| `activeLayer` | 当前层，从 0 计。0 在最下面 |
| `selection` | 矩形选区 `{ "x", "y", "width", "height" }`，或 `null`。见「选区」 |
| `tags` | `{ "index", "name", "from", "to", "loop" }` 数组，按添加顺序。没有标签时是 `[]`。见「动画标签」 |
| `parts` | `{ "index", "name", "x", "y", "layer", "anchors" }` 数组，按添加顺序。没有部件时是 `[]`。见「部件与锚点」 |
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
| `selection` | 与 `inspect` 相同。没有选区时是 `null` |
| `layerCount` | 层数，至少 1 |
| `layers` | `{ "index", "name", "visible", "opacity" }`，从底到顶 |
| `paletteLocked` | 调色板是否锁定 |
| `indexPolicy` | `reject` 或 `clamp`。省略工程字段时是 `reject` |
| `paletteCount` | 调色板项数，含索引 0 |
| `paletteHash` | 见下 |
| `palette` | 与 `inspect` 相同的 `{ "r", "g", "b", "a" }` 数组 |
| `frameCount` | 帧数 |
| `delayMs` | 当前帧时长，毫秒 |
| `frames` | `{ "index", "delayMs" }`，时间轴顺序 |
| `tags` | 与 `inspect` 相同。没有标签时是 `[]` |
| `parts` | 与 `inspect` 相同。没有部件时是 `[]` |

`visible` 恒为 `true`。`opacity` 恒为 `null`。内核没有逐层隐藏，也没有逐层透明度。非 0 索引盖住下层，不按 alpha 混合。

`paletteHash` 是 FNV-1a 64 位。初值 `14695981039346656037`，乘数 `1099511628211`。按索引顺序把每一项的 `r`、`g`、`b`、`a` 各作为一个字节送进去。结果是 16 位小写十六进制。颜色和顺序不变，哈希就不变。它用来认出调色板，不是口令。

CLI：`pixelforge scene --project hero.json`。

*Every layer is `visible: true`. `opacity` is `null`. The kernel does not store a hide flag or a per-layer opacity. A non-zero index covers the layer under it and does not blend. `paletteHash` is FNV-1a 64-bit: offset basis `14695981039346656037`, prime `1099511628211`, each entry's `r`, `g`, `b`, `a` in index order, written as 16 lowercase hex digits. The same colors in the same order hash the same. It identifies the palette. It is not a secret.*

### 脏区 Dirty region

`paint`、`erase`、`fillRect`、`eraseRect`、`copyRect`、`moveRect`、`flipHorizontal`、`flipVertical`、`rotate`、`resizeCanvas`、`translatePart`、`rotatePart`、`flipPart` 的 `params` 可以多一个 `reportDirty`（布尔）。别的工具带上这个字段仍是 `InvalidParams`。

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

### 选区 Selection

矩形选区是文档状态，不是套索，也不是魔棒。`setSelection` 写入一块轴对齐矩形，`clearSelection` 清掉它。没有选区时，画布上每一格都可以写。

选区**写在工程文件里**。字段是 `selection`：`{ "x", "y", "width", "height" }`。清掉之后文件省略这个字段，旧文件没有这个字段时按没有选区读入。撤销栈仍然不进文件：同一次进程里 `undo` 能撤回 `setSelection`，重新打开文件不能。这和 `activeLayer` 一样是下一次 CLI 进程还要接着用的状态，不是手势。

选区是画布坐标上的一块矩形，所有帧共用，不绑定某一层，也不改变 `activeLayer`。要只改头盔，把矩形框在头盔上，并用 `layer` / `--layer` 指向装备层。选区内只有你指定的那一层会被写；选区外任何层都不写。

有选区时：

| 工具 | 选区外 |
| --- | --- |
| `paint`、`erase`、`fillRect`、`eraseRect` | 跳过。请求的矩形仍须整块在画布内，否则 `OutOfBounds`，一格都不写 |
| `copyRect` | 只跳过目标格。源格可以在选区外，源不被改 |
| `flipHorizontal`、`flipVertical`、`rotate` | 目标格和它读到的源格必须都在选区内，才写入这一格 |
| `replaceIndex` | 只替换选区内的格子 |
| `moveRect` | 源矩形和目标矩形必须都整块在选区内，否则 `SelectionProtected`，一格都不写 |
| `translatePart`、`rotatePart`、`flipPart` | 挂接层上每一个非 0 格和它的落点都必须在选区内，否则 `SelectionProtected`，一格都不写。没有挂接层时不看选区 |

`remapPalette`、`applyPng`、调色板编辑、帧结构、层结构、`flattenLayers` 不看选区。只想改头盔的索引时用 `replaceIndex`，不要用整画布重映射。`resizeCanvas` 不按选区裁像素；选区跟着同一个偏移平移，平移后不能整块留在新画布上就在同一步撤销里清掉。

没有选区时，`replaceIndex` 替换当前帧、指定层（默认当前层）上的全部匹配格子。`toIndex` 为 0 时把这些格子清成透明。其它 `toIndex` 和 `paint` 一样解析。`fromIndex` 必须是调色板里已有的索引，包括 0。不改变 `activeLayer`，也不改其它帧。

`result`：`setSelection` 和 `clearSelection` 是 `{ "selection": { "x", "y", "width", "height" } }` 或 `{ "selection": null }`。`replaceIndex` 是 `{ "fromIndex", "toIndex", "changed" }`。`toIndex` 是写进去的索引。只有发生钳制时才另加 `indexPolicy` 和 `requestedIndex`。同一个选区、已经没有选区、或一格都没变，都不进撤销栈。

CLI：`pixelforge selection set`、`pixelforge selection clear`、`pixelforge replace-index`。

*The rectangle is document state. It is not a lasso and not a magic wand. `setSelection` stores it. `clearSelection` removes it. With no selection, every cell is writable.*

*The project file stores `selection` while one is set and omits the field when it is clear. A file that has no field loads with no mask. The undo stack is still not stored: `undo` in the same process restores `setSelection`, and reopening the file does not. This matches `activeLayer`: the next CLI process needs it.*

*One rectangle in canvas coordinates applies to every frame. It does not belong to a layer and it does not change `activeLayer`. To edit a helmet, set the rectangle on the helmet and pass `layer` for the gear layer. Only that layer is written, and only inside the rectangle.*

*`paint`, `erase`, `fillRect`, and `eraseRect` skip cells outside the selection. The requested rectangle must still lie on the canvas. `copyRect` skips destination cells outside it and does not change the source. `flipHorizontal`, `flipVertical`, and `rotate` write a cell only when that cell and the cell it reads are both inside. `replaceIndex` replaces matching cells inside the rectangle, or on the whole targeted layer of the active frame when no selection is set. `toIndex` 0 clears those cells. `moveRect` returns `SelectionProtected` and writes nothing unless both rectangles lie inside the selection. `translatePart`, `rotatePart`, and `flipPart` do the same when any ink cell on the linked layer, or the cell it would land on, lies outside the selection.*

*`remapPalette`, `applyPng`, palette edits, frame structure, layer structure, and `flattenLayers` ignore the selection. `resizeCanvas` translates the rectangle by the same offset and clears it, in that same undo step, when the translated rectangle would leave the new canvas.*

### `paint`

`params`：`x`、`y`（`0..=65535`，还须落在画布内）、`colorIndex`（`1..=255`，且必须是已有的不透明索引）。

写入的是索引，不是 RGB。锁定的调色板不阻止绘制。可选 `layer` 指定要改的层，不写就是当前层；这次调用不改变 `activeLayer`。别的层一格都不动。

默认 `indexPolicy` 是 `reject`：`colorIndex` 必须是已有的不透明索引，否则 `PaletteIndex`，文档不变。`clamp` 时，大于等于调色板长度的索引写成最后一项不透明色。索引 0 两种模式都拒绝。只有索引被钳制时，`result` 才多出 `indexPolicy`（`"clamp"`）、`requestedIndex` 和 `colorIndex`；范围内的绘制不带这些字段。`importPalette` 的 `replace` 仍把不再存在的索引清成 0，不看 `indexPolicy`。

*Writes an index, not RGB. A locked palette does not block paint. Optional `layer` chooses the layer and does not change `activeLayer`. Other layers stay put. The default `indexPolicy` is `reject`: an unknown `colorIndex` is `PaletteIndex` and the document stays put. `clamp` writes an index past the palette as the last opaque entry. Index 0 is refused in both modes. `indexPolicy`, `requestedIndex`, and `colorIndex` appear on the result only when the index was clamped. `importPalette` `replace` still clears orphaned indices to 0 and does not read `indexPolicy`.*

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

错误：`OutOfBounds`、`FrameIndex`、`LayerIndex`、`SelectionProtected`。可选 `layer` 与 `copyRect` 相同。有选区时，源矩形和目标矩形必须都整块落在选区内，否则整次失败。

### `setSelection`

`params`：`x`、`y`、`width`、`height`。宽高至少为 1，整块必须在画布内。

写入矩形选区。不改像素，不改 `activeLayer`。同一个矩形不进撤销栈。`result.selection` 是这块矩形。详见上面的「选区」。

*Stores the rectangle. Does not change pixels or `activeLayer`. The same rectangle is not recorded. `result.selection` is that rectangle. See Selection above.*

错误：`OutOfBounds`。宽或高为 0 是 `InvalidParams`。

### `clearSelection`

参数：无。

清掉选区。`result.selection` 是 `null`。已经没有选区时不进撤销栈。

*Clears the rectangle. `result.selection` is `null`. Already clear is not recorded.*

### `replaceIndex`

`params`：`fromIndex`、`toIndex`（都是 `0..=255`）。可选 `layer`。

在当前帧上，把等于 `fromIndex` 的格子写成 `toIndex`。有选区时只改选区内。`toIndex` 为 0 时清成透明。其它 `toIndex` 按 `paint` 解析，包括 `clamp`。`fromIndex` 必须已经在调色板里，包括 0。不改变 `activeLayer`，也不改其它帧。`result` 是 `fromIndex`、`toIndex`（写进去的索引）、`changed`。只有发生钳制时才另加 `indexPolicy` 和 `requestedIndex`。一格都没变不进栈。

*Replaces `fromIndex` with `toIndex` on the active frame. A selection limits that to the rectangle. `toIndex` 0 clears those cells. Any other `toIndex` resolves like `paint`. `fromIndex` must already be in the palette, including 0. The active layer and other frames stay put. `toIndex` in the result is the stored index. `indexPolicy` and `requestedIndex` appear only when the index was clamped. No change is not recorded.*

错误：`PaletteIndex`、`LayerIndex`。

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

`params`：`locked`（布尔）。`true` 锁定，`false` 解锁。画笔不受影响。同一个值不进撤销栈。这个标记会写入工程文件。锁定还拒绝 `remapPalette`。它不拒绝 `applyPng`，也不拒绝 `setIndexPolicy`。

### `setIndexPolicy`

`params`：`mode`（`reject` 或 `clamp`）。

`reject` 是默认。越界的 `paint` / `fillRect` 索引返回 `PaletteIndex`。`clamp` 把大于等于调色板长度的索引写成最后一项不透明色；调色板只有透明槽时仍然是 `PaletteIndex`。索引 0 不是钳制对象。同一个值不进撤销栈。只有 `clamp` 写入工程文件的 `indexPolicy`；缺这个字段的旧文件按 `reject` 读入。

`result`：`{ "mode": "reject" | "clamp" }`。

*Sets the document index policy. `reject` is the default and is omitted from the project file. `clamp` is stored. The same value is not recorded. A locked palette does not freeze this flag.*

CLI：`pixelforge palette enforce --mode reject|clamp`。

### `remapPalette`

`params`：`text` 或 `maxColors`，必须二选一。

`text` 是 `.gpl` 或十六进制列表，格式与 `importPalette` 相同。调色板换成「透明 + 文件颜色」，每一格按平方 RGB 距离（`dr²+dg²+db²`）映射到最近的目标色。距离相同取较小的目标索引。不发明新的 RGB。

`maxColors` 是 `1..=255`，只数不透明色，不含索引 0。先从最高索引丢掉没有像素的颜色，再把最近的一对并进像素更多的那一项（一样多则保留较小索引）。存活颜色的 RGB 保持原样，不做平均。

一次调用是一步撤销。调色板和索引都没变时不进栈。锁定时拒绝，文档不变。这里没有 Lospec 在线浏览器；调色板文本由调用方提供。

`result`：

| 字段 Field | 含义 Meaning |
| --- | --- |
| `pixelsChanged` | 存储索引变了的格子数。只换了颜色、索引号没变的格子不算 |
| `paletteLength` | 调用之后的调色板长度，含索引 0 |
| `misses` | 没有精确命中目标 RGB 的源颜色。见下 |
| `merges` | 被折进别的项的颜色，包括压紧后只改了索引号的精确命中 |
| `dropped` | 因为没人用、又超过 `maxColors` 而删掉的颜色。`text` 重映射时是 `[]` |

`misses` 的一条：`r`、`g`、`b`、`pixels`、`intoIndex`、`into`（`r`、`g`、`b`）、`distance`。来自调色板项时还有 `fromIndex`。`distance` 为 0 的不在这里。

`merges` 的一条：`fromIndex`（PNG 没有）、`from`、`intoIndex`、`into`、`pixels`、`distance`、`exact`。`exact: true` 表示 RGB 相同，只是索引因为压紧或重复色而移动。

`dropped` 的一条：`index`、`r`、`g`、`b`。

*One of `text` or `maxColors`. `text` installs that palette and nearest-maps every index. `maxColors` caps opaque entries. Squared RGB distance, lower index wins a tie, survivor RGB is not averaged. One undo step. Refused while locked. No Lospec browser: the caller supplies the palette text.*

CLI：`pixelforge palette remap --max-colors 8` 或 `--file colors.hex`。

错误：`PaletteLocked`、`PaletteFile`、`EmptyPalette`、`PaletteTooLarge`。两个参数一起给，或一个都不给，或 `maxColors` 越界，是 `InvalidParams`。

### `applyPng`

`params`：`png`（标准 Base64，可以含空白）。可选 `frame`、`layer`，省略时是当前帧和当前层。这次调用不改变 `activeFrame` 或 `activeLayer`。

把这一张 PNG 压进当前调色板，不追加颜色，不缩放，不切精灵表。宽高必须等于画布，否则 `PngSize`。alpha 小于 128 写成索引 0；大于等于 128 当成不透明，按平方 RGB 距离选最近的不透明索引。调色板 alpha 不参与混合。调色板锁定时仍然可以写，因为不改调色板。索引变了才进撤销栈，一步撤销。

只读第一帧。接受展开后的 RGBA、RGB、灰度和带调色板的 PNG。大于画布上限（256）的图不展开像素。

`result` 在 `remapPalette` 的 `pixelsChanged`、`misses`、`merges`、`dropped` 之外还有 `frame`、`layer`、`width`、`height`。PNG 没有源调色板索引，所以 `merges` 不带 `fromIndex`，并且和 `misses` 是同一批未精确命中的颜色。`dropped` 恒为 `[]`。

*Crushes one PNG onto the current palette. The image must match the canvas; there is no scale and no sheet slice. Alpha below 128 becomes index 0. A locked palette still allows the write. `merges` and `misses` list the same near-colors. This is not a Lospec browser.*

CLI：`pixelforge palette apply-png --file sprite.png`。可选 `--frame` 和 `--layer`。

错误：`PngFile`、`PngSize`、`NoPaintColor`、`FrameIndex`、`LayerIndex`。`png` 不是 Base64 时是 `InvalidParams`。

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

### 动画标签 Animation tags

标签是名字、闭区间帧范围，和循环模式。它写在工程文件的 `tags` 里；没有标签时省略这个字段。旧文件没有该字段时按空列表读入。撤销栈仍然不进文件。

这不是 Aseprite 的 cel 链接。每一帧仍有自己的像素。`duplicateFrame` 仍然复制索引，不让两帧共用一块缓冲。文档的 `tick` 仍按整条时间轴循环；`loop` 只存下来，并写进 atlas，不另开一个播放时钟。没有重复次数，没有 ping-pong reverse，也没有标签颜色。

`loop`：

| 值 | 含义 |
| --- | --- |
| `forward` | 从 `from` 播到 `to`（两端都含），然后重复。缺省是这个 |
| `reverse` | 从 `to` 播到 `from`，然后重复 |
| `pingpong` | 从 `from` 播到 `to` 再回到 `from`，然后重复。两端不多停一帧 |
| `once` | 从 `from` 播到 `to` 一次 |

区间可以重叠。名字唯一，去掉首尾空白后是 1 到 64 个字符，不能含控制字符。

插帧和删帧会在同一步撤销里挪标签。在下标 `i` 插入一帧时：起点 `>= i` 的标签整体右移，新帧不在里面；起点更早、且已经盖住 `i` 的标签，`to` 加一。删掉下标 `i` 时：整段在它后面的标签左移；盖住它的标签缩短；只盖住这一帧的标签被删掉。在末尾 `addFrame` 不拉长已有标签。复制某一帧是在它后面插入，所以复制标签的最后一帧时，拷贝不进这个标签。

*A tag is a name, an inclusive frame range, and a loop mode. The project file stores `tags` and omits the field when the list is empty. A file without the field loads with no tags. The undo stack is still not stored.*

*This is not an Aseprite cel link. Each frame keeps its own pixels. `duplicateFrame` still copies indices. `tick` still loops the whole timeline. `loop` is stored and written into the atlas. It does not start a second clock. There is no repeat count, no ping-pong reverse, and no tag color.*

*`forward` plays `from` through `to` and repeats (the default). `reverse` plays `to` through `from` and repeats. `pingpong` plays to the end and back without holding either end for an extra frame. `once` plays `from` through `to` a single time.*

*Ranges may overlap. Names are unique after trimming, 1 to 64 characters, with no control character.*

*Inserting a frame at `i` shifts a tag that starts at or after `i`, and grows a tag that starts earlier and already covers `i`. Deleting frame `i` shifts later tags left, shrinks tags that covered it, and drops a tag that named only that frame. `addFrame` does not grow existing tags. Duplicating a frame inserts after it, so duplicating the last frame of a tag leaves the copy outside that tag. The frame edit and the tag adjustment are one undo step.*

### `addTag`

`params`：`name`、`from`、`to`，可选 `loop`（缺省 `forward`）。`from` 和 `to` 都是帧下标，两端都包含。

`result`：`{ "index", "name", "from", "to", "loop" }`。`name` 是去掉空白后的名字。`index` 是列表末尾。

CLI：`pixelforge tag add --name walk_down --from 1 --to 3 --loop forward`。

错误：`TagName`、`TagExists`、`TagRange`。`loop` 不是那四个字符串时是 `InvalidParams`。

### `setTag`

`params`：`name`，外加 `from`、`to`、`loop`、`rename` 里的至少一个。没写的字段保持原样。

`result`：改完之后的 `{ "index", "name", "from", "to", "loop" }`。结果和原来一样时不进撤销栈。

CLI：`pixelforge tag set --name walk_down --loop pingpong --rename walk_south`。

错误：`TagName`、`TagExists`、`TagMissing`、`TagRange`。四个可选字段都缺时是 `InvalidParams`。

### `removeTag`

`params`：`name`。帧还在，只删标签。

`result`：`{ "index", "name" }`。`index` 是删掉之前的位置。`name` 是去掉空白后的名字。

CLI：`pixelforge tag remove --name walk_down`。

错误：`TagName`、`TagMissing`。

### 部件与锚点 Parts and anchors

部件是名字、画布上的静止原点，可选的层下标，和一组命名锚点。它写在工程文件的 `parts` 里；没有部件时省略这个字段。旧文件没有该字段时按空列表读入。撤销栈仍然不进文件。

部件**不是**层，也不是另一块像素缓冲。层仍然是每一帧共用的绘制栈。一个部件可以用 `layer` 指向像素所在的那一层（枪在 `gear` 上），也可以不挂层（肩、肘只是一个点）。多个部件可以指向同一层。`layer` 不切换 `activeLayer`。

原点 `x`、`y` 是画布上的一格，也是这个部件本地坐标的 `(0, 0)`。锚点的 `x`、`y` 是相对原点的像素格，范围 `-256..=256`，不是画布坐标。静止时，锚点的画布位置是原点加上这个偏移。握持点因此可以记在枪上，绕它旋转时不必把每一格 `fillRect` 再画一遍。工程文件不另存旋转角、曲线、约束或烘焙。

矩形 `flipHorizontal` / `flipVertical` / `rotate` 只搬索引，不搬部件，也不搬锚点。部件自己的平移、旋转、翻转见 `translatePart`、`rotatePart`、`flipPart`：它们改这一帧挂接层上的索引，并在同一步撤销里改原点和锚点。`resizeCanvas` 把原点按同一偏移挪走；原点会离开新画布时，这个部件连同锚点一起去掉，和选区在同一撤销步里。锚点的本地偏移不变。

插一层时，挂在该下标或更上面的部件，`layer` 加一。删一层时，挂在那一层上的部件保留名字和锚点，只是去掉 `layer`；更上面的部件下标减一。重排层时，`layer` 跟着那一层走。`flattenLayers` 把已经挂了层的部件改指到层 0，没挂层的不动。这些调整和层操作在同一步撤销里。

*`A part is a name, a rest-pose origin on the canvas, an optional layer index, and named anchors. The project file stores `parts` and omits the field when the list is empty. A file without the field loads with no parts. The undo stack is still not stored.*

*A part is not a layer and not a second pixel buffer. The layer stack stays the paint stack. `layer` names the paint layer whose pixels belong to the part, and does not select that layer. Omit it for a rig point such as a shoulder. Several parts may name one layer.*

*`x` and `y` on the part are the canvas cell that is local `(0, 0)`. Anchor `x` and `y` are pixel offsets from that cell, in `-256..=256`, not canvas coordinates. At rest, the canvas cell of an anchor is the origin plus that offset. A grip can sit on a gun so a later rotation can turn around it. The file does not store a rotation angle, a curve, a constraint, or a bake.*

*Rectangle `flipHorizontal`, `flipVertical`, and `rotate` move indices only. They do not move parts or anchors. `translatePart`, `rotatePart`, and `flipPart` move the linked layer's indices on one frame and update the origin and anchors in that same undo step. `resizeCanvas` translates each origin by the same offset and drops a part, in that same undo step, when the origin would leave the new canvas. Local anchor offsets stay put.*

*Inserting a layer shifts attachments at or above that index. Deleting a layer clears the attachment of a part that named it and shifts later attachments down. The part and its anchors stay. Reordering follows the layer. `flattenLayers` retargets every attached part to layer 0 and leaves an unattached part alone. The layer edit and the part adjustment are one undo step.*

### `listParts`

参数：无。只读。不进撤销栈。

`result`：`{ "parts": [ ... ] }`。每一项与 `inspect` 的 `parts` 相同。没有部件时 `parts` 是 `[]`。

CLI：`pixelforge part list --project soldier.json`。

### `addPart`

`params`：`name`、`x`、`y`，可选 `layer`。`x`、`y` 是原点所在的画布格。`layer` 是像素所在的层，省略表示不挂层。

`result`：`{ "index", "name", "x", "y", "layer", "anchors" }`。`name` 是去掉空白后的名字。`layer` 没有挂层时是 `null`。`anchors` 这时是 `[]`。`index` 是列表末尾。

CLI：`pixelforge part add --name gun --layer 1 --x 24 --y 12`。

错误：`PartName`、`PartExists`、`OutOfBounds`（原点不在画布上）、`LayerIndex`。

### `setPart`

`params`：`name`，外加 `x`、`y`、`layer`、`clearLayer`、`rename` 里的至少一个。没写的字段保持原样。`clearLayer: true` 去掉层挂接，不能和 `layer` 一起写。锚点的本地坐标不动。

`result`：改完之后的部件对象，形状与 `addPart` 相同。结果和原来一样时不进撤销栈。

CLI：`pixelforge part set --name gun --x 26 --rename rifle`。`pixelforge part set --name gun --clear-layer`。

错误：`PartName`、`PartExists`、`PartMissing`、`OutOfBounds`、`LayerIndex`。可选字段都缺，或 `layer` 与 `clearLayer` 同时出现时，是 `InvalidParams`。

### `removePart`

`params`：`name`。像素还在，部件和它的锚点一起删掉。

`result`：`{ "index", "name" }`。`index` 是删掉之前的位置。`name` 是去掉空白后的名字。

CLI：`pixelforge part remove --name gun`。

错误：`PartName`、`PartMissing`。

### `setAnchor`

`params`：`part`（部件名）、`name`、`x`、`y`。`x`、`y` 是本地像素格，整数，范围 `-256..=256`。同名锚点已存在时改偏移；不存在时追加。

`result`：`{ "part", "partIndex", "index", "name", "x", "y" }`。`part` 和 `name` 是去掉空白后的名字。同一个偏移不进撤销栈。

CLI：`pixelforge part anchor set --part gun --name grip --x 2 --y 2`。

错误：`PartName`、`PartMissing`、`AnchorName`、`AnchorCoord`。`x` 或 `y` 不是整数时是 `InvalidParams`。

### `removeAnchor`

`params`：`part`、`name`。部件还在，只删这个锚点。

`result`：`{ "part", "partIndex", "index", "name" }`。`index` 是删掉之前的位置。

CLI：`pixelforge part anchor remove --part gun --name grip`。

错误：`PartName`、`PartMissing`、`AnchorName`、`AnchorMissing`。

### `translatePart`

`params`：`name`、`dx`、`dy`（整数，可负）。可选 `frame`、`reportDirty`。

把部件原点平移 `(dx, dy)` 格。锚点的本地坐标不变，所以握持点的画布位置跟着原点走。挂了层时，这一帧该层上每一个非 0 索引按同样的位移搬走，源格清成 0；没挂层时只改元数据。不改调色板，不发明新索引。不切换 `activeFrame` 或 `activeLayer`。别的帧的像素不动，部件列表仍是整份文档一份。其它部件的原点不动；和它挂在同一层上的墨水会一起被搬走，所以枪应独占一层。

位移为 0 时不进撤销栈。原点或任一非 0 格的落点出画布时是 `PartOffCanvas`，一格都不写。有选区时，每个非 0 格和它的落点都必须在选区内，否则 `SelectionProtected`。

`result`：改完之后的部件对象，外加 `frame`、`dx`、`dy`、`alignment`（`translate`）、`sampling`（`exact`）、`dropped`（平移恒为 0）、`pivot`（`{x, y, anchor}`，平移时 `anchor` 为 `null`，`x,y` 是平移前的原点）。

CLI：`pixelforge part translate --name gun --dx 2 --dy -1`。`--frame` 指定帧。`--dirty` 即 `reportDirty: true`。

错误：`PartName`、`PartMissing`、`FrameIndex`、`PartOffCanvas`、`SelectionProtected`、`LayerIndex`。`dx` 或 `dy` 不是整数时是 `InvalidParams`。

### `rotatePart`

`params`：`name`、`degrees`（有限数，绝对值不超过 1000000）。可选 `anchor`、`frame`、`reportDirty`。

绕枢轴顺时针转。`anchor` 给出时，枢轴是这个锚点当前的画布格；省略时枢轴是部件原点格。枢轴是这一格的中心（`alignment` 为 `center-on-anchor` 或 `center-on-origin`）。屏幕坐标 `y` 向下。

角度规则：

1. 先对 360° 取模，收进 `(-180, 180]`。`-180` 收成 `180`。
2. 再量化到最近的 0.01°。正好差一半（第三位小数是 5 且后面没有更多小数）时远离 0。例如 `90.004` 和 `89.995` 收成 `90.00`，`90.005` 收成 `90.01`，`20` 和 `20.004` 都是 `20.00`，`20.005` 是 `20.01`，`0.004` 是 `0`，`0.005` 是 `0.01`。
3. 量化后是 90° 的整数倍时，`grid` 为 `quarter`，`sampling` 为 `exact`：用整数映射，不插值。90° 顺时针把相对枢轴的 `(dx, dy)` 送到 `(-dy, dx)`。成功的 90° 再转三次回到原样；`90` 再接 `-90` 也回到原样。`degreesCenti` 为 `0`、`9000`、`18000` 或 `-9000`。
4. 其它角度 `grid` 为 `nearest`，`sampling` 为 `nearest`。每个非 0 格用它的格心绕枢轴格心旋转，再落到最近的格。正好落在两格正中（相对枢轴的坐标是 `n+0.5`）时远离枢轴；和半整数相差小于 `1e-9` 的值先当成那个半整数，免得级数误差翻掉平局。正弦和余弦是内核里的泰勒级数，不是系统 `libm`。`degreesCenti` 是量化后的百分之一度。

两个非 0 格量化到同一格时，行优先较后的那一格留下，较早的索引丢掉，源格仍清成 0。`dropped` 是丢掉的格数。精确的直角、平移和翻转不会重叠，`dropped` 为 0。非直角可以留下空洞：没有墨水落上去、又不是被搬走的源格，就保持原索引。

只改 `frame`（缺省为当前帧）上挂接层的索引，并改这一份原点与锚点。锚点先变成画布坐标再映射，然后重新写成相对新原点的本地格，这样握持点在整数变换里仍停在原来的画布格上。本地结果超出 `-256..=256` 时是 `AnchorCoord`，一格都不写。落点出画布是 `PartOffCanvas`。选区规则与 `translatePart` 相同。量化成 0° 时不进撤销栈。

不做自由仿射、亚像素合成、骨骼界面、脚本、曲线（#56）、约束（#57）或烘焙（#58）。

CLI：`pixelforge part rotate --name gun --degrees 90 --anchor grip`。

错误：`PartName`、`PartMissing`、`AnchorName`、`AnchorMissing`、`AnchorCoord`、`PartAngle`、`PartOffCanvas`、`SelectionProtected`、`FrameIndex`、`LayerIndex`。`degrees` 不是有限数时是 `InvalidParams`。

### `flipPart`

`params`：`name`、`direction`（`horizontal` 或 `vertical`）。可选 `anchor`、`frame`、`reportDirty`。

绕枢轴做镜像。水平和 `flipHorizontal` 一样只搬索引、不改颜色：格 `x` 收到 `2 * pivotX - x`，`y` 不变。垂直则 `y` 收到 `2 * pivotY - y`。枢轴格自己不动。这是绕**一格的中心**，不是矩形 `flipHorizontal` 那种绕矩形中心（宽为偶数时中心在两格之间）。因此整帧 `hflip` 一般不等于绕原点的 `flipPart`。矩形翻转仍然不搬部件。

`sampling` 为 `exact`，`alignment` 同旋转。同一方向再翻一次，墨水还在画布上时回到原样。选区、出界、单帧、一步撤销，与 `translatePart` 相同。结果和原来一样时不进撤销栈。

`result` 另有 `direction`。

CLI：`pixelforge part flip --horizontal --name gun --anchor grip`。必须恰好写 `--horizontal` 或 `--vertical` 之一，否则退出码 `2`。

错误：`PartName`、`PartMissing`、`AnchorName`、`AnchorMissing`、`AnchorCoord`、`PartOffCanvas`、`SelectionProtected`、`FrameIndex`、`LayerIndex`。`direction` 不是那两个字符串时是 `InvalidParams`。

*Translate, rotate, and flip a part on the indexed grid. The pivot is the center of the origin cell, or of a named anchor. Quarter turns, flips, and translates are exact integer maps. Any other angle quantizes to 0.01° (ties away from zero) and then each ink cell center snaps to the nearest cell (ties away from the pivot). Indices move; colors are not invented. One frame's linked layer moves with the origin and anchors in one undo step. Rectangle flip and rotate still leave parts where they are.*

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

### `importAseprite`

`params`：`bytes`（`.aseprite` 或 `.ase` 文件的 Base64）。不接受 `reportDirty`。

成功后换掉当前文档，和 `loadProject` 一样。新文档的撤销栈是空的，没有选区，没有标签，调色板未锁定，`indexPolicy` 是 `reject`，当前帧和当前层都是 0。失败时原来的文档还在。

`result`：与 `inspect` 相同，再加 `aseprite`：

| 字段 Field | 含义 Meaning |
| --- | --- |
| `sourceTransparentIndex` | 文件头里的透明索引 |
| `transparentSwapped` | 透明索引不是 0 时，已经和索引 0 对调 |
| `hiddenLayers` | 文件里标成隐藏、导入后仍可见的层数 |
| `skippedGroups` | 跳过的组文件夹数。子层按文件顺序摊进层栈 |
| `skippedReference` | 跳过的参考层数 |
| `backgroundLayers` | 标成背景的层数。透明索引仍收成索引 0 |
| `nonNormalBlend` | 非 Normal 混合的层数。不混合 |
| `partialOpacity` | 层不透明度不是 255 的层数 |
| `partialCelOpacity` | cel 不透明度不是 255 的次数 |
| `nonzeroZIndex` | z-index 不是 0 的 cel 数。不重排 |
| `durationDefaulted` | 时长为 0、又没有文件头速度、因而写成 100 毫秒的帧数 |
| `clippedCells` | 落在画布外、被丢掉的像素数 |
| `ignored` | 跳过的 chunk 名，已排序 |

只读索引色 8 位。画布边长必须在 `1..=256`。图像层按文件顺序从底到顶放进现有层模型，不先合成再压成一层，这样身体和装备还能分开改。组文件夹本身不是一层，子层按文件顺序摊平。参考层不导入。隐藏层导入后可见，因为内核没有隐藏旗标。索引 0 一律改成全透明。文件的透明索引不是 0 时，先把调色板槽 0 和该索引对调，再对调像素，然后强制槽 0 透明。背景层上的这个索引同样收成 0。帧时长会导入：时长 0 时用文件头速度；两边都是 0 则 100 毫秒。

不导入：标签、切片、用户数据、蒙版、色彩配置、外部文件、tilemap。单独的 tileset chunk 记在 `ignored`；tilemap 层或 tilemap cel 整文件拒绝（`AsepriteTilemap`）。混合模式、层不透明度、cel 不透明度、cel z-index 只计数，不参与叠色。层与层之间仍是非 0 索引盖住下层。没有 `.aseprite` 写回，也没有插件接口。

cel 一边超过 256，或 `宽 × 高 × 导入层数 × 帧数` 超过 16777216，整文件拒绝。伸出画布的 cel 裁掉并计入 `clippedCells`。同一层同一帧上，后画的 cel 盖住先画的，不把层上其它格子清掉。链接 cel 复制最终的原始图像，放在链接 cel 自己的坐标上，不保留链接。

读文件用自写的索引子集解析器，只为取出画布、调色板和索引像素。`asefile` 经 `image` 合成出 RGBA，索引就没了。`aseprite-io` 带写回，且声明的最低 Rust 新于本仓库。`flate2` 只用来解开压缩 cel。

*Replaces the document, the same as `loadProject`. The new undo stack is empty. Failure leaves the previous document. Indexed 8-bit only. Image layers map bottom to top so body and gear stay separate. Groups are not stored; their children join that stack in file order. Reference layers are skipped. Hidden layers become visible. Blend, layer opacity, cel opacity, and z-index are counted and not applied. Index 0 is forced transparent; a non-zero transparent index is swapped with slot 0 first. Tags, slices, user data, masks, color profiles, external files, and tilemaps are not imported. There is no `.aseprite` writer and no plugin API. The reader is a small indexed subset parser: `asefile` returns composited RGBA, and `aseprite-io` writes the format back on a newer Rust than this repo. `flate2` only inflates compressed cels.*

错误：`AsepriteFile`、`AsepriteSize`、`AsepriteColorMode`、`AsepritePalette`、`AsepriteTilemap`、`AsepritePixel`、`AsepriteLayers`。`bytes` 不是 Base64 是 `InvalidParams`。

CLI：`pixelforge import-aseprite --file hero.aseprite --output hero.json` 新建工程，目标已存在则拒绝。`--project hero.json` 在导入成功后替换已有工程。两个目标要且只要一个。

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

可选 `atlas`（布尔）。省略或 `false` 时，`result` 和以前一样，没有 `atlas` 字段。`true` 时在原有字段上多一个 `atlas` 对象，不写文件，也不进撤销栈。形状是 `pixelforge-atlas` version 1：

| 字段 | 含义 |
| --- | --- |
| `format`, `version` | `"pixelforge-atlas"` 和 `1` |
| `columns`, `rows`, `scale` | 与这次导出的帧网格相同。`scale` 不改变列数 |
| `frameWidth`, `frameHeight` | 一格在图上的像素尺寸，已经乘 `scale` |
| `sheetWidth`, `sheetHeight` | 整张图的像素尺寸 |
| `frames` | 真实帧，不含末尾透明空位。每项是 `{ "index", "x", "y", "width", "height", "delayMs" }`。`x`/`y`/`width`/`height` 是图上的像素 |
| `tags` | `{ "index", "name", "from", "to", "loop" }`，与文档里的标签顺序相同。`from`/`to` 是帧下标，不是像素 |

CLI 的 `pixelforge export sheet --atlas hero-sheet.json` 把这个对象写成带缩进的 JSON，并多一个 `image`（PNG 文件名）。stdout 的 `result.atlas` 是这个 JSON 路径，不是整份对象。不传 `--atlas` 时不写这份文件，stdout 也不多这个字段。

*Optional boolean `atlas`. Omit it, or send `false`, and the result stays the PNG fields with no `atlas` key. `true` adds a `pixelforge-atlas` version 1 object and does not write a file or an undo entry. `frames` are real frames in sheet pixels, including `delayMs`, and do not include padding cells. `tags` are the document tags. `columns` is the frame grid.*

*CLI `export sheet --atlas FILE` writes that object as pretty JSON plus `image` (the PNG file name). Stdout `result.atlas` is the JSON path. Without `--atlas`, the file is not written and the stdout result has no `atlas` field.*

`atlas` 不是布尔时是 `InvalidParams`。

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

把矩形里的索引左右对调。格子 `x` 收到镜像列上的索引：整帧宽 64 时就是 `63 - x`。不改调色板，也不改高光颜色。一步撤销。对称的矩形不进栈。不改变 `activeFrame`。出界则一格都不写。有选区时，只有格子和它的镜像伙伴都在选区内才交换。

*Mirrors indices left to right. Cell `x` receives the index from the mirrored column (`63 - x` on a full frame of width 64). The palette is unchanged, including highlight colors. One undo step. A symmetric rectangle is not recorded. `frame` is not selected. An out-of-bounds rectangle writes nothing.*

错误：`OutOfBounds`、`FrameIndex`、`LayerIndex`。可选 `layer`：只镜像这一层。

### `flipVertical`

`params`：与 `flipHorizontal` 相同。可选 `frame`、`layer`、矩形四边，以及 `reportDirty`。

把矩形里的索引上下对调。格子 `(x, y)` 收到 `(x, originY + height - 1 - y)` 上的索引。不改调色板。一步撤销。整块对称时不进栈。不改变 `activeFrame`。出界则一格都不写。有选区时，只有目标格和它读到的源格都在选区内才写入。

*Mirrors indices top to bottom. Cell `(x, y)` receives the index from `(x, originY + height - 1 - y)`. The palette is unchanged. One undo step. A symmetric rectangle is not recorded. `frame` is not selected. An out-of-bounds rectangle writes nothing. With a selection, a cell is written only when it and the cell it reads are both inside.*

错误：`OutOfBounds`、`FrameIndex`、`LayerIndex`。可选 `layer`：只镜像这一层。

CLI：`pixelforge vflip`，或 `pixelforge flip --vertical`。

### `rotate`

`params`：必填 `degrees`，只能是 `90`、`180` 或 `270`（顺时针）。其余与 `flipHorizontal` 相同：可选 `frame`、`layer`、矩形四边、`reportDirty`。其它角度是 `InvalidParams`，文档不变。

在矩形内部重排索引。不改调色板，也不改变画布尺寸，所以其它帧的坐标仍然有效。180° 接受任意矩形。90° 和 270° 要求矩形是正方形，否则 `RotateSquare`，一格都不写。整帧转 90° 或 270° 因此需要正方形画布；要先改边长，用 `resizeCanvas`。

目标格在矩形内的局部坐标是 `(x, y)`，矩形宽 `w`、高 `h`。它读到的源格是：

| 角度 | 源格 |
| --- | --- |
| 90 | `(y, h - 1 - x)` |
| 180 | `(w - 1 - x, h - 1 - y)` |
| 270 | `(w - 1 - y, x)` |

一步撤销。转完和原来相同则不进栈。只改指定的一层（缺省为当前层），不改变 `activeLayer`。有选区时，目标格和源格都在选区内才写入。

*Turns indices clockwise inside the rectangle. The palette and the canvas size stay, so other frames keep their coordinates. 180 accepts any rectangle. 90 and 270 need a square, or the call is `RotateSquare` and writes nothing. A whole-frame quarter turn therefore needs a square canvas; resize first. One undo step. One layer. A selection writes a cell only when the destination and the source cell are both inside.*

错误：`OutOfBounds`、`FrameIndex`、`LayerIndex`、`RotateSquare`。`degrees` 不是 90、180、270 是 `InvalidParams`。

CLI：`pixelforge rotate --degrees 90`。矩形四个边要齐，或全部省略。

### `resizeCanvas`

`params`：`width`、`height`（`1..=256`）。可选 `offsetX`、`offsetY`（`i32`，可负）。两个偏移都省略时居中：`offset = (new - old) / 2`，向 0 取整。只给一个是 `InvalidParams`。

把每一帧的索引拷进新画布。旧原点落在 `(offsetX, offsetY)`。没有源的格子是索引 0。落在新画布外的旧格子被裁掉。时长、调色板、锁定和当前帧不变。工程仍是 version 1，只是 `width`、`height` 和各帧 `pixels` 变了。一步撤销能回到原来的缓冲。尺寸和索引都没变时不进栈。选区跟着同一个偏移平移；平移后不能整块留在新画布上，就在这一步里清掉。像素本身不按选区裁切。

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
