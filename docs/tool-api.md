# Tool API

稳定的机器可读接口，供 Agent 调用。它在 Rust 内核里，入口是 `Document::call_tool`。每一次调用都作用在和界面同一份 `Document` 上：画笔、橡皮、调色板、帧、时长、撤销栈、工程文件和 PNG 导出都走已有的内核方法。Agent 不改界面上的像素缓冲。

版本是 **1**（`TOOL_API_VERSION`）。请求可以省略 `apiVersion`，省略和 `null` 都表示 1。响应里始终带 `"apiVersion": 1`。

浏览器里的同一入口是 wasm 的 `callTool(request) -> string`。外壳的类型化封装在 `apps/web/src/kernel/toolApi.ts`（`callDocumentTool`）。手势仍然可以直接调用 `paint` 等方法；那些方法和 Tool API 写的是同一份文档。本接口不调用外部模型，也不读取 API key。

*A stable machine-readable API for an agent. It lives in the Rust kernel at `Document::call_tool`. Every call uses the same `Document` as the UI: paint, erase, palette, frames, delay, the undo stack, project files, and PNG export go through the kernel methods that already exist. An agent does not patch a pixel buffer owned by the shell.*

*The version is **1** (`TOOL_API_VERSION`). Omitting `apiVersion`, or sending `null`, means 1. Every response includes `"apiVersion": 1`.*

*In the browser the same entry is wasm `callTool(request) -> string`. The typed shell helper is `apps/web/src/kernel/toolApi.ts` (`callDocumentTool`). Gestures may still call `paint` and the other methods directly; those methods and the Tool API write one document. This API does not call an external model and does not read an API key.*

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
| `ProjectFile` | `loadProject` 的文本不是 version 1 工程文件 |

`InvalidRequest` 的 `message`：`request is not JSON`、`request must be a JSON object`、`missing tool`、`tool must be a string`、`unknown field <name>`、`id must be a string or number`、`apiVersion must be an integer from 0 to 4294967295`。

`UnknownTool` 的 `message`：`unknown tool "<name>"`。

`UnsupportedVersion` 的 `message`：`tool api version <n> is not supported`。

## 工具 Tools

没有写出的 `result` 字段就是 `{}`。只读工具（`inspect`、`saveProject`、两个导出）不进撤销栈。和当前值相同的绘制、改色、改时长、锁定，以及不改变文档的导入，也不进栈。

撤销栈上限是 `MAX_UNDO`（100）。多次绘制要并成一步时，用 `beginGesture` … `endGesture` 包住。

*`result` is `{}` when a tool does not list fields. Read-only tools (`inspect`, `saveProject`, and both exports) do not push undo entries. A paint, recolor, delay, or lock that does not change the current value, and an import that does not change the document, are not recorded either.*

*The undo stack keeps `MAX_UNDO` (100) entries. Wrap several paints in `beginGesture` … `endGesture` when they should undo as one step.*

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

### `erase`

`params`：`x`、`y`。把该格设回索引 0。

错误：`OutOfBounds`。

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

参数：无。不改文档，不进撤销栈。

`result`：`{ "frames": ["<base64>", ...] }`。每一项是时间轴上的一帧 RGBA PNG，和 `Document::export_frame_png` 的字节相同。Base64 是标准字母表，带 `=` 填充，没有换行，也没有 `data:` 前缀。

### `exportSpriteSheet`

参数：无。不改文档，不进撤销栈。

`result`：

| 字段 Field | 含义 Meaning |
| --- | --- |
| `png` | 网格 sprite sheet 的 Base64 PNG |
| `columns`, `rows` | `ceil(sqrt(帧数))` 和 `ceil(帧数 / 列数)` |
| `sheetWidth`, `sheetHeight` | 整张图的像素尺寸 |

排布和 Godot / Unity 约定见 [export.md](export.md)。

## 不变量 Invariants

- 改像素、调色板、帧和时长只通过 `Document` 的公开方法。Tool API 不写第二份缓冲。
- 这些修改记在同一条撤销栈上。界面随后 `undo`，撤销的是 Agent 的那一步。
- 非法下标、锁定的调色板、坏的工程文件都返回上表里的 `error`，并且不留下半次修改。
- 洋葱皮只存在于外壳视图。这个接口不提供把它写回像素的工具。

*Pixel, palette, frame, and delay edits go through `Document`'s public methods only. The Tool API does not keep a second buffer.*

*Those edits share the undo stack. A later UI `undo` reverts the agent's step.*

*A bad index, a locked palette, and a bad project file return an `error` from the tables above and do not leave a partial edit.*

*Onion skin is a shell view. This API has no tool that writes it back into pixels.*
