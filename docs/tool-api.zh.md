# Tool API { #tool-api }

Tool API 版本 1 提供工程读取、索引色编辑和动画导出接口。可通过 CLI 的 `pixelforge call`、Rust 的 `Document::call_tool` 或 WASM 的 `callTool` 调用。本页按工具列出参数、结果和错误码。

请求省略 `apiVersion` 或将其设为 null 时使用版本 1。每个响应均包含 `"apiVersion": 1`。同一 CLI 进程内的请求共享撤销记录。需要跨进程撤销时，为各次修改指定同一份检查点文件。[CLI 操作指南](agent-skill.md)

## 请求与响应 { #envelope }

Rust 与 WASM 返回紧凑的 JSON 字符串。CLI 将 JSON 写为以换行结尾的一行；本地像素工程的调用还会报告 `revision`。可选的 `id` 可以是字符串或数字；可解析的请求会在响应中返回该标识。工具签名中带 `?` 的参数可以省略，具体组合条件见各工具说明。

```json
{ "apiVersion": 1, "id": "step-1", "tool": "paint", "params": { "x": 1, "y": 2, "colorIndex": 3 } }
```

在新建的 16×16 工程上，CLI 返回：

```json
{ "ok": true, "apiVersion": 1, "id": "step-1", "result": { "revision": "535a25d889238788" }, "revision": "535a25d889238788" }
```

在 16×16 画布上将同一请求的 `x` 改为 16 时的 CLI 响应：

```json
{
  "ok": false,
  "apiVersion": 1,
  "id": "step-1",
  "error": { "code": "OutOfBounds", "message": "cell 16,2 is outside 16x16" },
  "revision": "535a25d889238788"
}
```

`params` 可以省略或为 `null`，两者都当成 `{}`。它必须是对象。未知字段会被拒绝。整数用 JSON 数字，不用字符串。分支看 `error.code`；`message` 给人读。失败时文档保持调用前的状态。`loadProject` 只有在工程文件解析成功后才换掉文档。

## 错误码 { #errors }

协议错误（文档不变）：

| code | 何时 When |
| --- | --- |
| `InvalidRequest` | 请求不是 JSON 对象，缺 `tool`，`tool` 不是字符串，`id` 或 `apiVersion` 类型不对，或信封上有未知字段 |
| `UnsupportedVersion` | `apiVersion` 不是 1 |
| `UnknownTool` | `tool` 不在下表里 |
| `InvalidParams` | 这个工具的 `params` 缺字段、类型不对，或有未知字段。`message` 形如 `paint: missing colorIndex` |

内核错误原样穿过，`code` 与 `KernelError::code()` 相同，`message` 与它的 `Display` 相同：

| code | 含义 Meaning |
| --- | --- |
| `OutOfBounds` | 格子超出画布。线段、椭圆、多边形、矩形轮廓只要有一端或一格出界，整次不写 |
| `PolygonPoints` | 多边形少于 3 个顶点。Tool API 在进内核前用 `InvalidParams` 拒绝；直接调用文档方法时是这个码。一格都不写 |
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
| `EngineUnknown` | `convertPng` / `setTransformEngine` 的引擎 id 没有注册 |
| `PaletteStrategyUnknown` | `convertPng` 的 `palette.strategy` 不是 `source`、`limit`、`project`。不会改跑另一个策略 |
| `PaletteInUse` | 策略 `source` 或 `limit` 会换掉共用调色板，但别的帧或层还有墨水。整次不写 |
| `PaletteDefault` | 策略 `project` 要对着内置默认色板映射。那不是调用方选过的色板。整次不写，`--output` 也不建文件 |
| `EngineUnavailable` | 引擎已注册但这一构建没有实现。`pixelforge-native` 是这个码。不会改跑另一个引擎 |
| `EngineParams` | 引擎私有参数缺了、类型不对，或有未知字段 |
| `ConvertFeature` | `features` 超过 32 条，或某一条是空的、含控制字符、超过 64 个字符 |
| `ConvertSource` | 源 PNG 的一边超过 4096。像素不展开 |
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
| `CurveValue` | 曲线的值对这个通道不合法。旋转要有限且绝对值不超过 1000000。平移必须是 `i32` 里的整数格。翻转是 `0` 或 `1` |
| `CurveEase` | 这个通道不接受这个缓动。翻转只能 `hold` |
| `CurveMissing` | 这个部件没有该通道的曲线 |
| `CurveKeyMissing` | 这条曲线在该帧上没有关键帧 |
| `CurveFrame` | 采样位置不是有限数 |
| `ConstraintName` | 约束名是空的、只有空白、含控制字符，或超过 64 个字符 |
| `ConstraintExists` | 已经有同名约束 |
| `ConstraintMissing` | 没有这个名字的约束 |
| `ConstraintFields` | 字段和 `follow`、`attach`、`pin`、`ik`、`plant` 对不上。链长不是 2 或 3、脚印重叠、或目标和钉点同时出现，也是这个码 |
| `ConstraintCycle` | 约束自己指向自己，或几条约束绕成环。一格都不写 |
| `ConstraintConflict` | 同一个部件被两条约束驱动。一条 `ik` 驱动链上的每一节。一格都不写 |
| `ConstraintLayer` | 同一层上有两个会动的部件。给每个会动的部件单独一层 |
| `PolishLayer` | `bake` 要重建的层是 `polish` 覆盖层，而且曲线或约束管着挂在这层上的部件。整次不写。把部件挪到 `default` 层 |
| `IkUnreachable` | 短链 IK 不能把末端正好落到目标格。一格都不写，也不会改放到旁边的格 |
| `BakeRange` | `bake` 的 `from` 大于 `to`，`from` 大于现有帧数，或 `to` 大于等于 4096 |
| `BakeFps` | `bake` 的 `fps` 不是大于 0 的有限数 |
| `NothingToUndo` | 撤销栈是空的 |
| `NothingToRedo` | 重做栈是空的 |
| `OpenGesture` | 已经有一个未结束的 gesture，或重做时 gesture 里已经有改动 |
| `MaskMissing` | 没有存选区，却对它做 `intersect` / `subtract`，或 `previewFrame` / `diffFrames` 传了 `selection: true`。文档不变 |
| `SelectionProtected` | `moveRect` 的源矩形或目标矩形有格子在选区外。部件变换时，挂接层上有非 0 格或其落点不在选区内。整次调用不写 |
| `ExportScale` | 导出倍数是 0，或放大后的尺寸放不进一张 PNG |
| `SheetColumns` | sprite sheet 的列数是 0，或那么多列放不进一张 PNG |
| `ProjectFile` | `loadProject` 的文本不是 version 1 工程文件。`selection` 不是画布内的矩形蒙版，某个 `tags` 项的名字、区间、循环模式、重名不合法，某个 `parts` 项的名字、原点、层、锚点不合法，某个 `curves` 项的部件、通道、关键帧、缓动、数值不合法，或某个 `constraints` 项的名字、种类、部件、锚点名、钉点、环或重复驱动不合法时，也是这个码 |
| `AsepriteFile` | `.aseprite` 字节截断、魔数不对、压缩 cel 解不开，或层 / cel 结构读不完 |
| `AsepriteSize` | 画布边长不在 `1..=256` |
| `AsepriteColorMode` | 颜色深度不是索引 8 位 |
| `AsepritePalette` | 没有可用的索引调色板 |
| `AsepriteTilemap` | 文件含 tilemap 层或 tilemap cel。整文件不导入 |
| `AsepritePixel` | 像素索引超出调色板长度 |
| `AsepriteLayers` | 跳过组和参考层之后，没有图像层可导入 |
| `RotateSquare` | `rotate` 的 90° 或 270° 矩形不是正方形。一格都不写 |
| `TileAlign` | 图块边长不是 16 或 32，或画布不是正好一块图块。地图格子尺寸和 tileset 不一致时也是这个码。整次不写 |
| `TilesetMissing` | 工程没有 tileset 尺寸。像素命令仍可用，导出图块集会拒绝 |
| `TileId` | tile id 不在 `0..tileCount`。整次不写 |
| `TileCount` | 地图的 `tileCount` 和 tileset 帧数不一致，或数量不在 `1..=65536` |
| `TileBounds` | 地图格子或矩形超出地图。整次不写 |
| `MapSize` | 地图宽或高不在 `1..=256` |
| `TileLayer` | 图层名字不合法、重名，或没有这一层 |
| `TileImage` | 图块集图片名不是相对的 `.png` 文件名 |
| `TileMapFile` | 不是 version 1 的 `pixelforge-tilemap` |
| `AutotileFile` | 不是 version 1 的 `pixelforge-autotile` |
| `AutotileNeighbors` | `neighbors` 不是 4。仅支持四邻接规则 |
| `AutotileRule` | 16 种掩码缺了、重复了，或位序不是 `n,e,s,w`。整次不写 |
| `DirectionMissing` | 方向命令需要已经存下的布局，工程里没有 |
| `DirectionFacing` | 朝向名不在这份布局里。四向没有对角 |
| `DirectionFrames` | 帧数不是 `count * columns`。整次不写 |
| `DirectionSegment` | 同一个动作在同一个朝向有两个名字。整次不写 |
| `DirectionFill` | `fillDirections` 不能填这个朝向：对角行跳过，`mirror` 只配 left 与 right，或源朝向缺帧段、帧段不在该行里。整次不写 |

`InvalidRequest` 的 `message`：`request is not JSON`、`request must be a JSON object`、`missing tool`、`tool must be a string`、`unknown field <name>`、`id must be a string or number`、`apiVersion must be an integer from 0 to 4294967295`。

`UnknownTool` 的 `message`：`unknown tool "<name>"`。

`UnsupportedVersion` 的 `message`：`tool api version <n> is not supported`。

## 工具 { #tools }

没有写出的 `result` 字段就是 `{}`。只读工具（`inspect`、`scene`、`saveProject`、两个导出、`previewFrame`、`diffFrames`、`listLayers`、`listParts`、`listCurves`、`sampleCurve`、`samplePart`、`listConstraints`）不进撤销栈。和当前值相同的绘制、矩形填充、矩形擦除、矩形复制或移动、水平翻转、垂直翻转、矩形旋转、画布缩放、改色、改时长、锁定、索引策略、改层名、选层、改层角色、改可见性、改透明度、改标签、改部件、改锚点、部件平移、部件旋转、部件翻转、改关键帧、姿势没有变化的 `solveConstraints`，以及像素和时长都没变的 `bake`，结果相同的 `combineMask` 和 `selectConnected`，还有不改变文档的导入、重映射、PNG 压盘和压平，也不进栈。`importAseprite` 和 `loadProject` 一样换掉整份文档，新撤销栈是空的。

`paint`、`erase`、`fillRect`、`strokeRect`、`line`、`ellipse`、`polygon`、`fill`、`eraseRect`、`copyRect`、`moveRect`、`flipHorizontal`、`flipVertical`、`rotate`、`resizeCanvas`、`translatePart`、`rotatePart`、`flipPart` 可以带可选布尔 `reportDirty`。省略或 `false` 时，这些工具的 `result` 与以前相同。见下面的「脏区」。这不是命令日志。

撤销栈上限是 `MAX_UNDO`（100）。一次 `fillRect`、`strokeRect`、`line`、`ellipse`、`polygon`、`fill`、`eraseRect`、`copyRect`、`moveRect`、`flipHorizontal`、`flipVertical`、`rotate`、`resizeCanvas`、`translatePart`、`rotatePart`、`flipPart`、`solveConstraints`、`bake` 或 `fillDirections` 本身就是一步撤销。多次绘制或多块矩形要并成一步时，用 `beginGesture` … `endGesture` 包住。

### `inspect` { #inspect }

`inspect()`

参数：无。

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
| `selection` | 一块矩形 `{ "x", "y", "width", "height" }`，多块或空蒙版 `{ "rects": [...] }`，或 `null`。见「选区」 |
| `tags` | `{ "index", "name", "from", "to", "loop" }` 数组，按添加顺序。没有标签时是 `[]`。见「动画标签」 |
| `parts` | `{ "index", "name", "x", "y", "layer", "anchors" }` 数组，按添加顺序。没有部件时是 `[]`。见「部件与锚点」 |
| `curves` | `{ "index", "part", "channel", "keys" }` 数组，按添加顺序。没有曲线时是 `[]`。见「部件曲线」 |
| `constraints` | `{ "index", "name", "kind", "part", "anchor", "target", "targetAnchor", "x", "y", "chain", "bend", "contacts" }` 数组，按添加顺序。没有约束时是 `[]`。见「约束」 |
| `layers` | `{ "index", "name", "role", "part", "anchor", "visible", "opacity" }` 数组，从底到顶。`opacity` 是 `0..=255` |
| `canUndo`, `canRedo` | 撤销 / 重做是否可用 |
| `gestureOpen` | 是否有未结束的 gesture |
| `directionLayout` | 按行排列的朝向布局，或 null |
| `tileset` | 图块宽高声明，或 null |
| `engine` | 当前 PNG 转换引擎 ID |
| `transform` | 最近的转换快照，或 null |

`inspect` 带上当前层的 `pixels` 和合成 `composite`。只要画布、层、调色板和帧时长时，用 `scene`。

### `scene` { #scene }

`scene()`

参数：无。只读。不进撤销栈。不返回像素缓冲。

| 字段 Field | 含义 Meaning |
| --- | --- |
| `width`, `height` | 画布边长，格子 |
| `activeFrame`, `activeLayer` | 当前帧、当前层，从 0 计 |
| `selection` | 与 `inspect` 相同。没有选区时是 `null` |
| `layerCount` | 层数，至少 1 |
| `layers` | `{ "index", "name", "role", "part", "anchor", "visible", "opacity" }`，从底到顶。`opacity` 是 `0..=255`，255 完全不透明 |
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
| `curves` | 与 `inspect` 相同。没有曲线时是 `[]` |
| `constraints` | 与 `inspect` 相同。没有约束时是 `[]` |
| `directionLayout` | 按行排列的朝向布局，或 null |
| `tileset` | 图块宽高声明，或 null |
| `engine` | 当前 PNG 转换引擎 ID |
| `transform` | 最近的转换快照，或 null |

`visible` 是布尔，缺省 `true`。`opacity` 是整数 `0..=255`，缺省 `255`，不再是 `null`。`false` 或 `0` 让这一层退出合成。`1..=254` 在索引合成里仍整格覆盖，在 RGBA 导出里混合。`flattenLayers` 才把这份混合收成最近索引。同一层的这两个值所有帧共用。旧工程没有这两个字段时读成可见且不透明。

`paletteHash` 是 FNV-1a 64 位。初值 `14695981039346656037`，乘数 `1099511628211`。按索引顺序把每一项的 `r`、`g`、`b`、`a` 各作为一个字节送进去。结果是 16 位小写十六进制。颜色和顺序不变，哈希就不变。它用来认出调色板，不是口令。

### 脏区 { #dirty-region }

`paint`、`erase`、`fillRect`、`strokeRect`、`line`、`ellipse`、`polygon`、`fill`、`eraseRect`、`copyRect`、`moveRect`、`flipHorizontal`、`flipVertical`、`rotate`、`resizeCanvas`、`translatePart`、`rotatePart`、`flipPart` 的 `params` 可以多一个 `reportDirty`（布尔）。别的工具带上这个字段仍是 `InvalidParams`。

`true` 时，在这个工具原有的 `result` 字段上再加：

| 字段 Field | 含义 Meaning |
| --- | --- |
| `changed` | 索引变了的格子数。一格都没变是 `0` |
| `dirty` | 这些格子的外接矩形 `{ "x", "y", "width", "height" }`。`changed` 为 0 时是 `null` |
| `regions` | 每个改过的帧和层一条：`frame`、`layer`、`changed`、`x`、`y`、`width`、`height`。没改的层不出现 |
| `clipped` | 只有 `resizeCanvas` 写这个字段。新画布上已经没有的坐标里，原来非 0 的格子数 |

矩形包住的是索引真的变了的格子，不是请求里的整块矩形。同一坐标上，新索引和旧索引不同才算 `changed`。画布变大时，新坐标上的 0 不算变化。跨帧时 `dirty` 是各 `regions` 的并集，帧别看 `regions`。`resizeCanvas` 里，坐标还在、只是索引换了，算 `changed`，不算 `clipped`。

CLI 的 `--dirty` 就是 `reportDirty: true`。不写这个旗标，结果里就没有这些字段。

### 选区 { #selection }

选区是文档状态，不是套索。`setSelection` 写入一块轴对齐矩形。`selectConnected` 把种子格的索引色连通域收成同一份蒙版。`combineMask` 用并、交、差把它收成一组互不相交的轴对齐矩形。`clearSelection` 清掉它。没有选区（`selection` 为 `null`）时，画布上每一格都可以写。差集或交集得到空蒙版时，`selection` 是 `{ "rects": [] }`，每一格都受保护，接下来的 `fillRect` 不会把整幅涂满。

选区**写在工程文件里**。一块矩形仍是 `{ "x", "y", "width", "height" }`。多块或空蒙版是 `{ "rects": [ { "x", "y", "width", "height" }, ... ] }`。清掉之后文件省略这个字段。旧文件没有这个字段时按没有选区读入。相邻的矩形在写入时并成一块，所以文件里的划分是确定的。撤销栈仍然不进文件：同一次进程里 `undo` 能撤回 `setSelection`、`selectConnected` 和 `combineMask`，重新打开文件不能。这和 `activeLayer` 一样是下一次 CLI 进程还要接着用的状态，不是手势。

选区是画布坐标上的蒙版，所有帧共用，不绑定某一层，也不改变 `activeLayer`。要只改头盔，把矩形框在头盔上，并用 `layer` / `--layer` 指向装备层。蒙版内只有你指定的那一层会被写；蒙版外任何层都不写。

有选区时：

| 工具 | 选区外 |
| --- | --- |
| `paint`、`erase`、`fillRect`、`strokeRect`、`line`、`ellipse`、`polygon`、`fill`、`eraseRect` | 跳过。请求的端点、顶点或矩形仍须整块在画布内，否则 `OutOfBounds`，一格都不写。`fill` 不穿过选区外的格子，选区外是墙 |
| `copyRect` | 只跳过目标格。源格可以在选区外，源不被改 |
| `flipHorizontal`、`flipVertical`、`rotate` | 目标格和它读到的源格必须都在选区内，才写入这一格 |
| `replaceIndex` | 只替换选区内的格子 |
| `moveRect` | 源矩形和目标矩形必须都整块在选区内，否则 `SelectionProtected`，一格都不写 |
| `translatePart`、`rotatePart`、`flipPart` | 挂接层上每一个非 0 格和它的落点都必须在选区内，否则 `SelectionProtected`，一格都不写。没有挂接层时不看选区 |

`remapPalette`、`applyPng`、调色板编辑、帧结构、层结构、`flattenLayers` 不看选区。只想改头盔的索引时用 `replaceIndex`，不要用整画布重映射。`resizeCanvas` 不按选区裁像素；每一块矩形跟着同一个偏移平移。只要有一块会离开新画布，整份蒙版就在同一步撤销里清掉。

没有选区时，`replaceIndex` 替换当前帧、指定层（默认当前层）上的全部匹配格子。`toIndex` 为 0 时把这些格子清成透明。其它 `toIndex` 和 `paint` 一样解析。`fromIndex` 必须是调色板里已有的索引，包括 0。不改变 `activeLayer`，也不改其它帧。

`result`：一块矩形时 `setSelection` 的 `selection` 是 `{ "x", "y", "width", "height" }`。多块或空蒙版是 `{ "rects": [...] }`。`clearSelection` 是 `{ "selection": null }`。`replaceIndex` 是 `{ "fromIndex", "toIndex", "changed" }`。`toIndex` 是写进去的索引。只有发生钳制时才另加 `indexPolicy` 和 `requestedIndex`。同一个选区、已经没有选区、或一格都没变，都不进撤销栈。

### `paint` { #paint }

`paint(x, y, colorIndex, layer?, reportDirty?)`

`params`：`x`、`y`（`0..=65535`，还须落在画布内）、`colorIndex`（`1..=255`，且必须是已有的不透明索引）。

写入的是索引，不是 RGB。锁定的调色板不阻止绘制。可选 `layer` 指定要改的层，不写就是当前层；这次调用不改变 `activeLayer`。别的层一格都不动。

默认 `indexPolicy` 是 `reject`：`colorIndex` 必须是已有的不透明索引，否则 `PaletteIndex`，文档不变。`clamp` 时，大于等于调色板长度的索引写成最后一项不透明色。索引 0 两种模式都拒绝。只有索引被钳制时，`result` 才多出 `indexPolicy`（`"clamp"`）、`requestedIndex` 和 `colorIndex`；范围内的绘制不带这些字段。`importPalette` 的 `replace` 仍把不再存在的索引清成 0，不看 `indexPolicy`。

错误：`OutOfBounds`、`PaletteIndex`、`LayerIndex`。

### `fillRect` { #fillrect }

`fillRect(x, y, width, height, colorIndex, layer?, reportDirty?)`

`params`：`x`、`y`（矩形左上角，含这一格，`0..=65535`，还须落在画布内）、`width`、`height`（`1..=65535`，矩形的每一格都须落在画布内）、`colorIndex`（`1..=255`，且必须是已有的不透明索引）。

在当前帧上把这块轴对齐矩形写成同一个索引。写入的是索引，不是 RGB。锁定的调色板不阻止这次填充。已经是该索引的格子不记入撤销。整块矩形是一步撤销；如果此时有未结束的 gesture，这些格子并进那一步。矩形里只要有一格出界，或者宽高加出来越过画布，整次调用失败，一格都不写。`width` 或 `height` 为 0、缺了、或不是该范围内的整数，是 `InvalidParams`（`message` 形如 `fillRect: width must be an integer from 1 to 65535`）。

错误：`OutOfBounds`、`PaletteIndex`、`LayerIndex`。可选 `layer` 与 `paint` 相同。

### `strokeRect` { #strokerect }

`strokeRect(x, y, width, height, colorIndex, layer?, reportDirty?)`

`params`：与 `fillRect` 相同（`x`、`y`、`width`、`height`、`colorIndex`，可选 `layer`）。

画轴对齐矩形的轮廓，内部不写。宽或高为 1 时整条都画上，和细的 `fillRect` 一样。边界、锁定调色板、索引 0、选区和一步撤销都与 `fillRect` 相同。已经是该索引的格子不记入撤销。

错误：`OutOfBounds`、`PaletteIndex`、`LayerIndex`。

### `line` { #line }

`line(x0, y0, x1, y1, colorIndex, pixelPerfect?, layer?, reportDirty?)`

`params`：`x0`、`y0`、`x1`、`y1`（两端，含这两格，须在画布内）、`colorIndex`。可选 `pixelPerfect`（布尔，缺省 `false`）、可选 `layer`。

在当前层画一条 1 像素宽的直线。默认是连续 Bresenham：相邻格子共边或共角，没有断点，两端都画上。`pixelPerfect: true` 沿较长的那条轴一步一格，仍然 8 连通，拐角不叠成两格宽。写入的是索引，不是 RGB。没有抗锯齿。锁定的调色板不阻止。索引 0 拒绝。两端只要有一个出界，整次不写。选区外的格子跳过。改变的格子是一步撤销；已经是该索引的不记入。同一条线再画一次、格子都没变，不进撤销栈。

CLI：`pixelforge line --x0 --y0 --x1 --y1 --color-index N`。`--pixel-perfect` 对应 `pixelPerfect: true`。

错误：`OutOfBounds`、`PaletteIndex`、`LayerIndex`。`pixelPerfect` 不是布尔是 `InvalidParams`。

### `ellipse` { #ellipse }

`ellipse(x, y, width, height, colorIndex, fill?, layer?, reportDirty?)`

`params`：`x`、`y`、`width`、`height`（外接矩形，规则与 `fillRect` 相同）、`colorIndex`。可选 `fill`（布尔，缺省 `false`）、可选 `layer`。

椭圆内切于这块矩形。缺省只画轮廓。`fill: true` 再涂上内部，轮廓上的格子都在填充里。宽或高为 1 时轮廓和填充都是那一条线。整数中点椭圆，没有抗锯齿。同一输入得到同一组格子。锁定、索引 0、越界、选区、一步撤销与 `fillRect` 相同。

CLI：`pixelforge ellipse`。`--fill` 对应 `fill: true`。

错误：`OutOfBounds`、`PaletteIndex`、`LayerIndex`。

### `polygon` { #polygon }

`polygon(points, colorIndex, fill?, layer?, reportDirty?)`

`params`：`points`（至少 3 个、至多 4096 个 `{ "x", "y" }`，按顺序）、`colorIndex`。可选 `fill`（布尔，缺省 `false`）、可选 `layer`。

闭合多边形。边用和 `line` 相同的连续直线，最后一点连回第一点。`fill: true` 在轮廓上再加偶奇规则的内部，所以边和 `line` 走的格子一致，内部被涂上。每个顶点都必须在画布内，否则 `OutOfBounds`，一格都不写。少于 3 个顶点是 `InvalidParams`（`polygon: points needs at least 3 vertices`），不调用内核。自交按偶奇填充。没有抗锯齿，没有曲线。锁定、索引 0、选区、一步撤销与 `fillRect` 相同。

CLI：`pixelforge polygon --point x,y`，至少重复三次。`--fill` 对应 `fill: true`。

错误：`OutOfBounds`、`PaletteIndex`、`LayerIndex`。顶点对象缺 `x` 或 `y`、多了字段、或不是对象，是 `InvalidParams`。

### `fill` { #fill }

`fill(x, y, colorIndex, connectivity?, layer?, reportDirty?)`

`params`：`x`、`y`（种子格）、`colorIndex`。可选 `connectivity`（`4` 或 `8`，缺省 `4`）、可选 `layer`。

把种子格所在的同色连通区换成 `colorIndex`。种子上现在的索引（包括 0）决定哪些格子属于这一区。`4` 只走上下左右。`8` 也走斜角。选区外的格子是墙：不写，也不穿过去。种子出画布是 `OutOfBounds`，一格都不写。种子在选区外时成功但不改格子。新颜色和种子已经相同则不进撤销栈。锁定的调色板不阻止。`colorIndex` 为 0 或越界是 `PaletteIndex`。改变的格子是一步撤销。

CLI：`pixelforge fill --x --y --color-index N`。`--connectivity 4|8`，缺省 4。

错误：`OutOfBounds`、`PaletteIndex`、`LayerIndex`。`connectivity` 不是 4 或 8 是 `InvalidParams`。

### `erase` { #erase }

`erase(x, y, layer?, reportDirty?, mode?)`

`params`：`x`、`y`。可选 `layer`。可选 `mode`：`transparent`（缺省）或 `pass`。

在 `default` 层上，两种模式都把该格设回索引 0。在 `polish` 层上，`transparent` 写下显式擦除：索引仍是 0，合成时打穿下层；`pass` 清回未修改，下层重新露出来。只改这一层。已经处于目标状态的格子不记入撤销。

错误：`OutOfBounds`、`LayerIndex`。`mode` 不是那两个字符串时是 `InvalidParams`。

### `eraseRect` { #eraserect }

`eraseRect(x, y, width, height, layer?, reportDirty?, mode?)`

`params`：`x`、`y`（矩形左上角，含这一格）、`width`、`height`（`1..=65535`）。

在当前帧上把这块轴对齐矩形设回索引 0。可选 `mode` 与 `erase` 相同：`polish` 层上缺省是显式擦除，`pass` 清回未修改。边界和 `fillRect` 相同：只要有一格出界，整次失败，一格都不写。已经处于目标状态的格子不记入撤销。整块是一步撤销；未结束的 gesture 会收进这些格子。锁定的调色板不阻止这次擦除。改剪影时用它，不要逐格 `erase`。

错误：`OutOfBounds`、`LayerIndex`。`width` 或 `height` 为 0 是 `InvalidParams`。可选 `layer` 与 `paint` 相同。

### `copyRect` { #copyrect }

`copyRect(srcX, srcY, width, height, dstX, dstY, fromFrame?, toFrame?, layer?, reportDirty?)`

`params`：`srcX`、`srcY`、`width`、`height`、`dstX`、`dstY`。宽高是 `1..=65535`。坐标是源矩形和目标矩形的左上角。

在一帧上把源矩形的调色板索引原样抄到另一帧的目标矩形，包括索引 0。源矩形保持不动。可选 `fromFrame` 是源帧，可选 `toFrame` 是目标帧；省略时都是当前帧。这次调用不改变 `activeFrame`。源或目标只要有一格出界，整次失败，一格都不写。同一帧上重叠时先读源，再写目标。改变了的格子是一步撤销；未结束的 gesture 会收进这些格子。没有变化不进栈。锁定的调色板不阻止这次复制。

错误：`OutOfBounds`、`FrameIndex`、`LayerIndex`。可选 `layer`：源和目标都用这一层，不在层与层之间抄像素，也不改变 `activeLayer`。

### `moveRect` { #moverect }

`moveRect(srcX, srcY, width, height, dstX, dstY, fromFrame?, toFrame?, layer?, reportDirty?)`

`params` 与 `copyRect` 相同，包括可选的 `fromFrame` 和 `toFrame`。

把源矩形的索引搬到目标矩形，包括索引 0。不在目标里的源格子变成索引 0。跨帧时目标在另一帧上，所以整个源矩形都会清掉。源和目标都要整块落在画布内，否则一格都不写。同一帧上重叠时以搬动前的源为准，落在目标里的源格子保留搬过去的索引，不会被清掉。整次移动是一步撤销。挪到自己身上等于没改，不进栈。不改变 `activeFrame`。

错误：`OutOfBounds`、`FrameIndex`、`LayerIndex`、`SelectionProtected`。可选 `layer` 与 `copyRect` 相同。有选区时，源矩形和目标矩形必须都整块落在选区内，否则整次失败。

### `setSelection` { #setselection }

`setSelection(x, y, width, height)`

`params`：`x`、`y`、`width`、`height`。宽高至少为 1，整块必须在画布内。

写入矩形选区。不改像素，不改 `activeLayer`。同一个矩形不进撤销栈。`result.selection` 是这块矩形。详见上面的「选区」。

错误：`OutOfBounds`。宽或高为 0 是 `InvalidParams`。

### `selectConnected` { #selectconnected }

`selectConnected(x, y, tolerance?, connectivity?, layer?, frame?)`

`params`：`x`、`y`（种子格，须在画布内）。可选 `tolerance`（`0..=255`，缺省 `0`）、`connectivity`（`4` 或 `8`，缺省 `4`）、`layer`、`frame`。

把种子格所在的索引色连通域写成 `selection`。不改像素，不改 `activeLayer`，不改 `activeFrame`。采样的是这一层存着的索引，不看 `visible`，也不乘 `opacity`。已有选区被替换，不当成墙；`fill` 才把选区外当墙。连通和 `fill` 用同一套邻域：`4` 只走上下左右，`8` 也走斜角。

`tolerance` 为 `0` 时只收和种子同一个索引的格子。另一个索引即使 RGBA 相同也不收。`tolerance` 为 `1..=255` 时，格子的调色板颜色和种子颜色在 `r`、`g`、`b`、`a` 上各自的差都不超过这个数。种子到自己的距离是 0，所以种子格一定在里面。`255` 匹配任何颜色。比较的是调色板项，不把新的 RGB 写回格子。

连通域收成和 `combineMask` 相同的互不相交矩形。实心矩形仍是一块。不规则轮廓是多块。结果不会是空蒙版，因为种子格匹配自己。同一份蒙版不进撤销栈。变了就是一步撤销。之后的 `paint`、`erase`、`fill`、`combineMask`、`previewFrame` 和 `diffFrames` 都认这份蒙版。

`result`：`selection`（形状与「选区」相同）、`cells`（格子数）、`index`（种子上的索引）、`tolerance`、`connectivity`、`frame`、`layer`。

CLI：`pixelforge selection wand --x --y`。`--tolerance N`，`--connectivity 4|8`，`--layer`，`--frame`。后两个不选中那一层或那一帧。

错误：`OutOfBounds`、`LayerIndex`、`FrameIndex`。`connectivity` 不是 4 或 8、`tolerance` 越出 `0..=255`、或带了 `reportDirty`，是 `InvalidParams`。文档不变。

### `clearSelection` { #clearselection }

`clearSelection()`

参数：无。

清掉选区。`result.selection` 是 `null`。已经没有选区时不进撤销栈。空蒙版不是「没有选区」：要让整幅又可写，用这一条，不要把 `{ "rects": [] }` 留在文件里。

### `combineMask` { #combinemask }

`combineMask(op, x?, y?, width?, height?, baseX?, baseY?, baseWidth?, baseHeight?, mask?, base?)`

`params`：`op` 是 `union`、`intersect` 或 `subtract`。操作数是 `x`、`y`、`width`、`height` 四块整数，或 `mask`（矩形数组，至少一块）。左操作数省略时用当前选区。要换左操作数，传 `base`（一块矩形对象，或矩形数组），或传 `baseX`、`baseY`、`baseWidth`、`baseHeight`。`base` 和 `baseX` 这组不能同时出现。矩形四边和 `mask` 也不能同时出现。

不改像素，不改 `activeLayer`。结果写进 `selection`，形状与「选区」相同。一步撤销。结果和当前选区相同则不进栈。没有选区时，`union` 把操作数存下来；`intersect` 和 `subtract` 是 `MaskMissing`，文档不变。空结果仍是一份选区：`cells` 为 0，之后的绘制整幅跳过。`reportDirty` 是未知字段，`InvalidParams`。

`result`：`{ "op", "selection", "cells" }`。`cells` 是蒙版盖住的格子数。

CLI：`pixelforge mask union|intersect|subtract`。矩形用 `--x --y --width --height`，或多段 `--mask x,y,width,height`。左操作数用 `--base-x --base-y --base-width --base-height`，或多段 `--base-mask`。省略左边时用工程里的选区。工具拒绝时退出码 `1`。

错误：`OutOfBounds`、`MaskMissing`。`op` 不对、矩形和 `mask` 同时出现或都缺、`mask` 是空数组，是 `InvalidParams`。

### `replaceIndex` { #replaceindex }

`replaceIndex(fromIndex, toIndex, layer?)`

`params`：`fromIndex`、`toIndex`（都是 `0..=255`）。可选 `layer`。

在当前帧上，把等于 `fromIndex` 的格子写成 `toIndex`。有选区时只改选区内。`toIndex` 为 0 时清成透明。其它 `toIndex` 按 `paint` 解析，包括 `clamp`。`fromIndex` 必须已经在调色板里，包括 0。不改变 `activeLayer`，也不改其它帧。`result` 是 `fromIndex`、`toIndex`（写进去的索引）、`changed`。只有发生钳制时才另加 `indexPolicy` 和 `requestedIndex`。一格都没变不进栈。

错误：`PaletteIndex`、`LayerIndex`。

### `setPaletteColor` { #setpalettecolor }

`setPaletteColor(index, r, g, b)`

`params`：`index`、`r`、`g`、`b`，都是 `0..=255`。索引 0 不能改。格子仍保存原来的索引，所以每一帧用到该项的地方都会变成新颜色。

错误：`PaletteLocked`、`TransparentSlot`、`PaletteIndex`。

### `addPaletteColor` { #addpalettecolor }

`addPaletteColor(r, g, b)`

`params`：`r`、`g`、`b`。在末尾追加一个不透明色。

`result`：`{ "index": <新索引> }`。

错误：`PaletteLocked`、`PaletteTooLarge`。

### `importPalette` { #importpalette }

`importPalette(text, mode)`

`params`：`text`（文件原文）、`mode`（`replace` 或 `merge`）。

格式与 `Document::import_palette` 相同，见 [dev-setup.md](dev-setup.md)。`replace` 用文件颜色换掉可绘制项，并把不再存在的索引清成透明。`merge` 只追加还没有的不透明颜色。索引 0 保持透明。一次导入是一步撤销。

错误：`PaletteLocked`、`PaletteImportMode`、`PaletteFile`、`EmptyPalette`、`PaletteTooLarge`。

### `setPaletteLocked` { #setpalettelocked }

`setPaletteLocked(locked)`

`params`：`locked`（布尔）。`true` 锁定，`false` 解锁。画笔不受影响。同一个值不进撤销栈。这个标记会写入工程文件。锁定还拒绝 `remapPalette`。它不拒绝 `applyPng`，也不拒绝 `setIndexPolicy`。

### `setIndexPolicy` { #setindexpolicy }

`setIndexPolicy(mode)`

`params`：`mode`（`reject` 或 `clamp`）。

`reject` 是默认。越界的 `paint` / `fillRect` 索引返回 `PaletteIndex`。`clamp` 把大于等于调色板长度的索引写成最后一项不透明色；调色板只有透明槽时仍然是 `PaletteIndex`。索引 0 不是钳制对象。同一个值不进撤销栈。只有 `clamp` 写入工程文件的 `indexPolicy`；缺这个字段的旧文件按 `reject` 读入。

### `remapPalette` { #remappalette }

`remapPalette(text?, maxColors?)`

`params`：`text` 或 `maxColors`，必须二选一。

`text` 是 `.gpl` 或十六进制列表，格式与 `importPalette` 相同。调色板换成「透明 + 文件颜色」，每一格按平方 RGB 距离（`dr²+dg²+db²`）映射到最近的目标色。距离相同取较小的目标索引。不发明新的 RGB。

`maxColors` 是 `1..=255`，只数不透明色，不含索引 0。先从最高索引丢掉没有像素的颜色，再把最近的一对并进像素更多的那一项（一样多则保留较小索引）。存活颜色的 RGB 保持原样，不做平均。

一次调用是一步撤销。调色板和索引都没变时不进栈。锁定时拒绝，文档不变。这里没有 Lospec 在线浏览器；调色板文本由调用方提供。

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

CLI：`pixelforge palette remap --max-colors 8` 或 `--file colors.hex`。

错误：`PaletteLocked`、`PaletteFile`、`EmptyPalette`、`PaletteTooLarge`。两个参数一起给，或一个都不给，或 `maxColors` 越界，是 `InvalidParams`。

### `applyPng` { #applypng }

`applyPng(png, frame?, layer?)`

`params`：`png`（标准 Base64，可以含空白）。可选 `frame`、`layer`，省略时是当前帧和当前层。这次调用不改变 `activeFrame` 或 `activeLayer`。

把这一张 PNG 压进当前调色板，不追加颜色，不缩放，不切精灵表。宽高必须等于画布，否则 `PngSize`。alpha 小于 128 写成索引 0；大于等于 128 当成不透明，按平方 RGB 距离选最近的不透明索引。调色板 alpha 不参与混合。调色板锁定时仍然可以写，因为不改调色板。索引变了才进撤销栈，一步撤销。

只读第一帧。接受展开后的 RGBA、RGB、灰度和带调色板的 PNG。大于画布上限（256）的图不展开像素。

`result` 在 `remapPalette` 的 `pixelsChanged`、`misses`、`merges`、`dropped` 之外还有 `frame`、`layer`、`width`、`height`。PNG 没有源调色板索引，所以 `merges` 不带 `fromIndex`，并且和 `misses` 是同一批未精确命中的颜色。`dropped` 恒为 `[]`。

CLI：`pixelforge palette apply-png --file sprite.png`。可选 `--frame` 和 `--layer`。

错误：`PngFile`、`PngSize`、`NoPaintColor`、`FrameIndex`、`LayerIndex`。`png` 不是 Base64 时是 `InvalidParams`。

### `listEngines` { #listengines }

`listEngines()`

参数：无。不改文档。

`result.default` 是没写过 `engine set` 时的 id，固定 `center-sample`。`result.engine` 是这份工程当前会用的 id。`result.engines` 每条有 `id`、`version`、`status`（`available` 或 `reserved`）、`available`、`licensed`、`summary`。

开源构建内置五个 available 引擎，顺序固定：`center-sample`、`dominant`、`kmeans`、`contrast-outline`、`grid-snap`。`pixelforge-native` 是 `reserved`，`available: false`，`licensed: false`。没有运行时下载。

CLI：`pixelforge engine list`。可选 `--project`，那时 `engine` 读的是该文件。

### `setTransformEngine` { #settransformengine }

`setTransformEngine(id)`

`params`：`id`。把默认引擎写进工程文件的 `engine`。省略该字段时，下一次 `convertPng` 用 `center-sample`。同一个 id 不进撤销栈。未知 id 是 `EngineUnknown`。`pixelforge-native` 是 `EngineUnavailable`，文件不动。

### `convertPng` { #convertpng }

`convertPng(png, width?, height?, engine?, features?, frame?, layer?, palette?, params?)`

`palette.mode = source` 遇到超过 255 个采样色时，先保留使用最多的 255 色（同频取较小 RGB，存活项保持原顺序），再压到 `maxColors`；不把任意采样色序号当成 8 位工程索引。

把一张任意尺寸的 PNG 收成一帧索引色。上层只认这一份参数。换引擎不改这些字段。

| 字段 | 含义 |
| --- | --- |
| `png` | 标准 Base64。空白可忽略。源图每边最大 4096 |
| `width`, `height` | 目标边长，`1..=256`。一起给，或一起省略（用当前画布） |
| `engine` | 引擎 id。省略时用工程里的默认引擎 |
| `palette` | `{ "strategy"?: "source" \| "limit" \| "project", "mode"?: "project" \| "source" \| "provided", "text"?, "maxColors"? }`。省略整个对象等于 `mode: project`。`strategy` 和 `mode` 只能给一个 |
| `features` | 字符串数组，最多 32 条。保存到快照，不参与转换 |
| `params` | 引擎私有对象。上层不解析。未知字段是 `EngineParams` |
| `frame`, `layer` | 写入的帧和层。省略是当前帧、当前层，并且不切换它们 |

`palette.strategy` 是三种注册策略。未知 id 是 `PaletteStrategyUnknown`，不回退到另一种，也不改文件。不能同时给 `mode` 或 `text`。

| id | 行为 |
| --- | --- |
| `source` | 沿用原图色板。索引色 PNG 取 PLTE 里 alpha ≥ 128 的项，按文件顺序，采样没用到的也留下。其它 PNG 按行优先收集每个不透明 RGB。不透明色超过 255 是 `PaletteTooLarge`，不改走 `limit`。不能带 `maxColors`。别的帧或层还有墨水、而且新色板和当前色板不同，是 `PaletteInUse`，一格都不写 |
| `limit` | 自动限制色数。索引色且不透明项不超过 255 时从那盘开始，否则从采样后的不透明颜色开始，收到 `maxColors`（默认 16，`1..=255`）。合并规则与 `remapPalette` 相同，不平均出新 RGB。超过 255 色时先留采样里出现最多的 255 个，平局留较小 RGB。和 `source` 一样，不能在还有别的墨水时换掉共用色板 |
| `project` | 映射到当前工程里已经选好的调色板，不追加、不删除。不能带 `maxColors`。调色板仍是内置默认色板（包括 `--output` 新建的那份）是 `PaletteDefault`，不写文件。锁定时，色板已经选过仍可写 |

距离是平方 RGB（`dr² + dg² + db²`）。alpha 小于 128 写成索引 0，不计入 RGB 误差。源图没有不透明色时不换调色板，`paletteReport.keptExisting` 为 true。这不是换成另一个策略 id。

`result.paletteReport` 只在给了 `strategy` 时出现。颜色误差（`inexactPixels`、`maxDistance`、`totalDistance`）是这一张采样图。`frames` 列出每一个索引变过的帧（`frame`、`pixelsChanged`）。`otherCellsChanged` 是转换层以外、索引变了的格子数；`source` 和 `limit` 在这个数会大于 0 时直接拒绝，所以成功时它是 0。其余字段：`strategy`、`version`（`"1"`）、`metric`（`"squared-rgb"`）、`origin`（`indexed`、`colors`、`sampled`、`project`、`existing`）、`keptExisting`、`sourceColors`、`paletteColors`、`mergedColors`、`droppedColors`、`opaquePixels`、`transparentPixels`、`exactPixels`、`merges`、`dropped`。`merges` 每一项是一个没精确命中的采样色：`from`、`intoIndex`、`into`、`pixels`、`distance`、`exact: false`，索引色来源另有 `sourceIndex`。`dropped` 是因为没人用、又超过上限而删掉的颜色，带 `pixels: 0`。`totalDistance` 是不精确像素的平方距离之和。同一张图、同一个策略、同一组参数，调色板、像素和这份报告都一样。

`palette.mode` 仍是原来的三种，不产生 `paletteReport`：

- `project`：压进当前调色板，不追加颜色。不能带 `text` 或 `maxColors`。锁定时仍可写。
- `source`：从采样结果收一盘颜色，不透明色不超过 `maxColors`（默认 16，`1..=255`）。这是封顶，不是策略 `source`。锁定且调色板会变时拒绝。
- `provided`：`text` 是 `.gpl` 或十六进制列表，压进这盘颜色。文本不在快照里再存一份。锁定且调色板会变时拒绝。

采样先得到目标尺寸的 RGBA。alpha 小于 128 的格子是透明。不透明格只取源图里已经有的颜色，不混合出新色。然后用和 `applyPng` 相同的平方 RGB 最近索引写入。平局取较小索引。选区不裁这块写入。

五个引擎，版本都是 `"1"`：

| id | 行为 |
| --- | --- |
| `center-sample` | 每格取该格中心覆盖到的源像素。放大是最近邻，不补细节。无私有参数 |
| `dominant` | 块内多数不透明 RGB。票数相同取较小 RGB。不透明像素不超过一半则整格透明。无私有参数 |
| `kmeans` | 不透明源色做固定 8 轮、无随机的聚类。中心最后吸到一个真实源色。块内再按簇投票。`k` 默认 16，`1..=255` |
| `contrast-outline` | 亮度差达到 `contrast`（默认 48）或邻接透明的像素是轮廓。`expand`（默认 1，`0..=4`）只在不透明格子上加粗。块内优先轮廓像素，取最接近亮度中位数的那一个；偶数个取较低的中位 |
| `grid-snap` | 用边缘投票找重复周期，允许 `slack`（默认 1，`0..=4`）格的弹性。每格取众数色，再中心采样到目标尺寸。找不到边缘时 `report.detected` 为 false，并改用中心采样。这是这个引擎自己的结果，不是换成另一个 id。`maxCell` 默认 32，`2..=64` |

`kmeans` 的均值只用来选源色，写进调色板的仍是源色。`mode: source` 若引擎给出了建议色，先用建议色再按 `maxColors` 封顶。策略 `source` 和 `limit` 不读这份建议色，它们用原图色板或采样颜色。

目标尺寸和当前画布不同时，其它帧、选区、部件按 `resizeCanvas` 的居中规则挪一次，然后所选层整层换成采样索引。图块集工程的目标必须等于图块尺寸，否则 `TileAlign`。一步撤销。失败不写。

`--engine` 只影响这一次，不改 `engine set` 存下的默认 id。成功后工程文件写入 `transform`：`id`、`version`、`width`、`height`、`palette`、`params`，以及非空的 `features`。`palette.mode` 在没写策略时仍是 `project`、`source`（封顶）或 `provided`。写了策略时，`mode` 不把限色或沿用原图色板记成这个 `source`：策略 `source` 记 `mode: keep`，策略 `limit` 记 `mode: limit` 和 `maxColors`，策略 `project` 记 `mode: project`。三种都另有 `strategy`。以前把策略 `source` / `limit` 写成 `mode: source` 的文件仍能打开，再次保存时改成 `keep` 或 `limit`。同一张 PNG、同一个引擎、同一个策略、同一组参数，输出一致。`features` 出现在 `result.featuresIgnored` 里，表示记下来了但没有参与采样。

`result` 含 `applyPng` 的 `pixelsChanged`、`misses`、`merges`、`dropped`，另有 `frame`、`layer`、`width`、`height`、`engine`（`id`、`version`、`status`）、`palette`、`paletteMode`、`transform`、`featuresIgnored`、`report`。给了 `strategy` 时还有 `paletteReport`。

```json
{ "tool": "listEngines" }
{ "tool": "setTransformEngine", "params": { "id": "center-sample" } }
{ "tool": "convertPng", "params": { "png": "<base64>", "width": 64, "height": 64, "engine": "kmeans", "palette": { "strategy": "limit", "maxColors": 16 }, "params": { "k": 16 } } }
{ "tool": "convertPng", "params": { "png": "<base64>", "palette": { "strategy": "source" } } }
{ "tool": "convertPng", "params": { "png": "<base64>", "palette": { "strategy": "project" } } }
```

CLI：

```bash
pixelforge engine list
pixelforge engine set kmeans --project hero.json
pixelforge convert --file ref.png --width 64 --height 64 --engine center-sample --output hero.json
pixelforge convert --file ref.png --output hero.json --width 64 --height 64 --palette-strategy source
pixelforge convert --file ref.png --project hero.json --palette-strategy limit --max-colors 8
pixelforge convert --file ref.png --project hero.json --palette-strategy project --checkpoint hero.ckpt.json
```

`--output` 新建工程，没写策略时默认 `palette` 是 `mode: source`（采样色封顶，默认 16）。`--project` 没写策略时默认 `mode: project`。`--palette-strategy project` 不能用在 `--output` 上，也不能用在仍是内置默认色板的工程上，那是 `PaletteDefault`，不建文件。`--palette-strategy source` 和 `limit` 在别的帧或层还有墨水时是 `PaletteInUse`，不改那些格子。`--palette-strategy` 不能和 `--palette` 或 `--palette-file` 一起用。`--output` 必须带 `--width` 和 `--height`。`--project` 省略尺寸时保持画布。`--max-colors` 配 `--palette source` 或 `--palette-strategy limit`。`--params` 是 JSON 对象。`--feature` 可重复。

错误：`EngineUnknown`、`EngineUnavailable`、`EngineParams`、`PaletteStrategyUnknown`、`PaletteInUse`、`PaletteDefault`、`ConvertFeature`、`ConvertSource`、`PngFile`、`NoPaintColor`、`PaletteLocked`、`PaletteFile`、`PaletteTooLarge`、`TileAlign`、`InvalidSize`、`FrameIndex`、`LayerIndex`。尺寸只给了一边、`strategy` 和 `mode` 一起给、策略带了 `text`、`source` 或 `project` 策略带了 `maxColors`、`png` 不是 Base64，是 `InvalidParams`。

### `setActiveFrame` { #setactiveframe }

`setActiveFrame(index)`

`params`：`index`（`0..=4294967295`，还须是已有帧）。选中当前帧。相同的下标不进撤销栈。

错误：`FrameIndex`。

### `addFrame` { #addframe }

`addFrame()`

参数：无。在末尾追加一帧空白帧，时长 100 毫秒。当前帧不变。

`result`：`{ "index": <新帧> }`。

### `deleteFrame` { #deleteframe }

`deleteFrame(index)`

`params`：`index`。至少保留一帧。

错误：`FrameIndex`、`LastFrame`。只剩一帧时，任何删除都是 `LastFrame`，包括越界下标。

### `duplicateFrame` { #duplicateframe }

`duplicateFrame(index)`

`params`：`index`。把该帧（像素和时长）插到它后面。

`result`：`{ "index": <新帧> }`。

**不会把当前帧改成这份拷贝。** 返回的 `index` 才是新帧。要画拷贝，接着调用 `setActiveFrame`（CLI 是 `frame select`）。当前帧若在插入点或它后面，下标会加一，画面仍是原来那一帧；插在当前帧前面时，当前下标不变。网页里的「复制」按钮会在同一次 gesture 里再 `setActiveFrame` 到新下标。CLI 的 `frame duplicate` 和这个工具不会。

错误：`FrameIndex`。

### `setFrameDelay` { #setframedelay }

`setFrameDelay(index, delayMs)`

`params`：`index`、`delayMs`。`delayMs` 先要能放进 `u32`，内核再要求 `1..=65535`。

错误：`FrameIndex`、`FrameDelay`。

### 动画标签 { #animation-tags }

标签是名字、闭区间帧范围，和循环模式。它写在工程文件的 `tags` 里；没有标签时省略这个字段。旧文件没有该字段时按空列表读入。撤销栈仍然不进文件。

这不是 Aseprite 的 cel 链接。每一帧仍有自己的像素。`duplicateFrame` 仍然复制索引，不让两帧共用一块缓冲。文档的 `tick` 仍按整条时间轴循环；`loop` 只存下来，并写进 atlas，不另开一个播放时钟。没有重复次数，没有 ping-pong reverse，也没有标签颜色。

| 值 | 含义 |
| --- | --- |
| `forward` | 从 `from` 播到 `to`（两端都含），然后重复。缺省是这个 |
| `reverse` | 从 `to` 播到 `from`，然后重复 |
| `pingpong` | 从 `from` 播到 `to` 再回到 `from`，然后重复。两端不多停一帧 |
| `once` | 从 `from` 播到 `to` 一次 |

区间可以重叠。名字唯一，去掉首尾空白后是 1 到 64 个字符，不能含控制字符。

插帧和删帧会在同一步撤销里挪标签。在下标 `i` 插入一帧时：起点 `>= i` 的标签整体右移，新帧不在里面；起点更早、且已经盖住 `i` 的标签，`to` 加一。删掉下标 `i` 时：整段在它后面的标签左移；盖住它的标签缩短；只盖住这一帧的标签被删掉。在末尾 `addFrame` 不拉长已有标签。复制某一帧是在它后面插入，所以复制标签的最后一帧时，拷贝不进这个标签。

### `addTag` { #addtag }

`addTag(name, from, to, loop?)`

`params`：`name`、`from`、`to`，可选 `loop`（缺省 `forward`）。`from` 和 `to` 都是帧下标，两端都包含。

`result`：`{ "index", "name", "from", "to", "loop" }`。`name` 是去掉空白后的名字。`index` 是列表末尾。

错误：`TagName`、`TagExists`、`TagRange`。`loop` 不是那四个字符串时是 `InvalidParams`。

### `setTag` { #settag }

`setTag(name, from?, to?, loop?, rename?)`

`params`：`name`，外加 `from`、`to`、`loop`、`rename` 里的至少一个。没写的字段保持原样。

`result`：改完之后的 `{ "index", "name", "from", "to", "loop" }`。结果和原来一样时不进撤销栈。

错误：`TagName`、`TagExists`、`TagMissing`、`TagRange`。四个可选字段都缺时是 `InvalidParams`。

### `removeTag` { #removetag }

`removeTag(name)`

`params`：`name`。帧还在，只删标签。

`result`：`{ "index", "name" }`。`index` 是删掉之前的位置。`name` 是去掉空白后的名字。

错误：`TagName`、`TagMissing`。

### 方向布局 { #direction-layout }

方向布局定义各朝向在精灵表中的行序。它写在工程文件的 `directionLayout` 里：`{ "count", "columns" }`。没有布局时省略。旧文件按没有布局读入。版本仍是 1。

`count` 是 `4` 或 `8`。`4` 是正式的 N/E/S/W。行序跟精灵表一样，不是从北顺时针排：

| 行 | 罗盘 | 标签后缀 |
| --- | --- | --- |
| 0 | S | `down` |
| 1 | W | `left` |
| 2 | E | `right` |
| 3 | N | `up` |
| 4 | SW | `downleft` |
| 5 | SE | `downright` |
| 6 | NW | `upleft` |
| 7 | NE | `upright` |

`8` 把四行对角接在这四行后面，不挪前四行。`columns` 是每一行的帧数，至少 1。`count * columns` 不能超过 4096。

标签仍是原来的 `{动作}_{朝向}`。朝向可以写画面名，也可以写罗盘别名（`south`、`s`、`sw`、`southwest`）。自动起的名字用画面后缀，所以已有的 `idle_down` 不用改名。对角是一个词：`downleft` 或 `sw`，不是 `down_left`。

idle / walk 的切分和精灵表一样：有 idle 时它只占该行第一格，walk 占剩下的格子；这一行没有 idle 时，整行可以只打 `walk_{朝向}`；只有一格时不该再有 walk。其它已经出现的 `{动作}_{朝向}`（例如 `attack_down`）必须在布局里的每一个朝向都有，而且相对该行的列区间相同。对不上是硬错误。

`validateDirections` 的 `result.errors` 里，`code` 是 `layout`、`frames`、`missingDirection` 或 `segment`。`errorCount` 大于 0 就是没过。没有 loose。这条命令不改文件。通过只说明帧段齐，不说明走起来自然，也不说明好看。

`applyDirectionTags` 按上面的标准切分写下 idle/walk。一格的行只写 idle。更长的行把 idle 放在第一格、walk 放在其余格。已经用别名的标签留着原名，只改区间。同一个动作在同一个朝向有两个名字时整次不写。不画像素。`setDirectionLayout` 的 `apply: true` 把存布局和写标签收成一步撤销。

`fillDirections` 才搬索引。它要求已经有布局，而且帧数正好是 `count * columns`。源朝向必须已经有合法的 idle/walk 帧段；布局里任一基本朝向出现过的其它 `{动作}_{朝向}`，源朝向也必须有，而且落在该行里面。缺了整次不写。写入时按这些帧段的相对列对齐：源的第 0 列写到目标朝向的第 0 列。`mirror` 只把 left 水平翻到 right，或把 right 翻到 left；北和南没有镜像目标，用 `copy`。`copy` 把源帧的索引导进其余三个基本朝向，不翻转。八向布局的四行对角不写，`result.diagonals` 是 `skipped`；四向是 `none`。目标帧任一层已有非 0 索引或显式擦除时默认跳过整帧，`overwrite: true` 才盖掉。像素和时长都没变的帧不进撤销栈。有变化时，这一次填充是一步撤销。不改标签、部件、选区、当前帧。水平翻转和矩形 `flipHorizontal` 用同一条规则：`x` 收到 `width - 1 - x`，只搬索引，不改颜色，也不搬部件。这不是新画，不是自然步态，不修高光，也不是自动生成的四向原画。

### `setDirectionLayout` { #setdirectionlayout }

`setDirectionLayout(count, columns, apply?)`

`params`：`count`（`4` 或 `8`）、`columns`（至少 1）。可选 `apply`（缺省 false）。

`result`：`{ "count", "columns", "wrote", "apply" }`。`apply` 为 true 时还有 `report`，形状与 `validateDirections` 相同。布局和标签都没变时 `wrote` 是 false，不进撤销栈。`apply` 时帧数对不上，或同一段有两个名字，整次不写，布局也不留下。

CLI：`pixelforge direction set --count 4 --columns 4`。加上 `--apply` 同时写标签。

错误：`DirectionLayout`、`DirectionFrames`、`DirectionSegment`。

### `clearDirectionLayout` { #cleardirectionlayout }

`clearDirectionLayout()`

参数：无。标签还在。本来就没有布局时不进撤销栈。

### `applyDirectionTags` { #applydirectiontags }

`applyDirectionTags()`

参数：无。需要已经存下的布局。

`result`：和 `validateDirections` 一样，再加 `wrote`。

错误：`DirectionMissing`、`DirectionFrames`、`DirectionSegment`。

### `validateDirections` { #validatedirections }

`validateDirections()`

参数：无。只读。不进撤销栈。

`result`：`{ "count", "columns", "errorCount", "errors", "facings" }`。没有布局时 `count` 和 `columns` 是 null，`errors` 里有一条 `layout`。`facings` 的每一项是 `{ "row", "compass", "screen", "from", "to", "present", "segments" }`。`segments` 的每一项是 `{ "action", "name", "from", "to", "loop" }`。

CLI：`pixelforge direction validate`。`errorCount` 大于 0 时退出码是 1。工具响应本身仍是 `ok: true`，因为检查跑完了。

### `listDirections` { #listdirections }

`listDirections()`

参数和 `result` 与 `validateDirections` 相同。只读。CLI `pixelforge direction list` 只要检查跑完就退出码 0，错误留在 `errors` 里。

### `exportDirection` { #exportdirection }

`exportDirection(facing, scale?)`

`params`：`facing`。可选 `scale`（缺省 1）。`facing` 接受 `N`/`E`/`S`/`W`、`down`/`left`/`right`/`up`，布局是 8 时再接受对角。

`result`：`png`（base64）、`facing`（画面后缀）、`compass`、`row`、`from`、`to`、`columns`、`rows`（恒为 1）、`sheetWidth`、`sheetHeight`、`scale`。不写文件，不改工程。这一行不是另一份 atlas。

CLI：`pixelforge direction export --facing south --output south.png`。文件由 CLI 写到 `--output`。

错误：`DirectionMissing`、`DirectionFacing`、`DirectionFrames`。`scale` 不是至少 1 的整数时是 `InvalidParams` 或 `ExportScale`。

### `fillDirections` { #filldirections }

`fillDirections(from, mode, overwrite?)`

`params`：`from`、`mode`（`mirror` 或 `copy`）。可选 `overwrite`（缺省 false）。`from` 接受 `N`/`E`/`S`/`W`、`down`/`left`/`right`/`up`，和 `direction` 其它命令一样。对角名在八向布局里也认，但不使用它们当源，返回 `DirectionFill`。

`mirror` 的源只能是 left 或 right。`copy` 的源是四个基本朝向之一。北、南、以及八向对角都不能 `mirror`。

`result`：`{ "mode", "from", "compass", "overwrite", "wrote", "framesWritten", "framesSkipped", "diagonals", "targets" }`。`from` 是画面后缀。`diagonals` 在四向是 `none`，在八向是 `skipped`。`targets` 的每一项是 `{ "facing", "compass", "hflip", "frames" }`。`frames` 的每一项是 `{ "frame", "source", "action", "status" }`。`status` 是 `written`、`skipped` 或 `unchanged`。`action` 是盖住这一列的源帧段，多段时用逗号接在一起，例如 `idle,attack`。`wrote` 为 false 时不进撤销栈。

每一层都拷，包括 `polish` 上的显式擦除。索引 0 仍是透明。非 0 索引落到目标格时，那一格的擦除标记清成 0。源帧的 `delayMs` 一起写到实际写入的目标帧；被跳过的帧时长不动。选区不裁这一次填充。部件原点不动。之后的 `bake` 仍会从静止帧重建被曲线或约束管着的层；没被管着的层留下这次写上的索引。`flattenLayers` 仍按原来的规则收成索引。

CLI：`pixelforge direction fill --from left --mode mirror`。占位用 `pixelforge direction fill --from down --mode copy`。`--overwrite` 盖掉已有墨水。先 `direction set --apply` 或自己打好源朝向的 `{动作}_{朝向}` 标签，再填充。填充不写标签。

错误：`DirectionMissing`、`DirectionFacing`、`DirectionFrames`、`DirectionSegment`、`DirectionFill`。`mode` 不是 `mirror` 或 `copy` 时是 `InvalidParams`。

### 部件与锚点 { #parts-and-anchors }

部件是名字、画布上的静止原点，可选的层下标，和一组命名锚点。它写在工程文件的 `parts` 里；没有部件时省略这个字段。旧文件没有该字段时按空列表读入。撤销栈仍然不进文件。

部件**不是**层，也不是另一块像素缓冲。层仍然是每一帧共用的绘制栈。一个部件可以用 `layer` 指向像素所在的那一层（枪在 `gear` 上），也可以不挂层（肩、肘只是一个点）。多个部件可以指向同一层。`layer` 不切换 `activeLayer`。

原点 `x`、`y` 是画布上的一格，也是这个部件本地坐标的 `(0, 0)`。锚点的 `x`、`y` 是相对原点的像素格，范围 `-256..=256`，不是画布坐标。静止时，锚点的画布位置是原点加上这个偏移。握持点因此可以记在枪上，绕它旋转时不必把每一格 `fillRect` 再画一遍。部件记录本身不存旋转角。参数曲线在 `curves`。约束在 `constraints`。`bake` 写下的是像素和 `delayMs`，文件里没有 `bake` 字段。版本仍是 1。

矩形 `flipHorizontal` / `flipVertical` / `rotate` 只搬索引，不搬部件，也不搬锚点。部件自己的平移、旋转、翻转见 `translatePart`、`rotatePart`、`flipPart`：它们改这一帧挂接层上的索引，并在同一步撤销里改原点和锚点。`resizeCanvas` 把原点按同一偏移挪走；原点会离开新画布时，这个部件连同锚点一起去掉，和选区在同一撤销步里。锚点的本地偏移不变。

插一层时，挂在该下标或更上面的部件，`layer` 加一。删一层时，挂在那一层上的部件保留名字和锚点，只是去掉 `layer`；更上面的部件下标减一。重排层时，`layer` 跟着那一层走。`flattenLayers` 把已经挂了层的部件改指到层 0，没挂层的不动。这些调整和层操作在同一步撤销里。

### `listParts` { #listparts }

`listParts()`

参数：无。只读。不进撤销栈。

`result`：`{ "parts": [ ... ] }`。每一项与 `inspect` 的 `parts` 相同。没有部件时 `parts` 是 `[]`。

### `addPart` { #addpart }

`addPart(name, x, y, layer?)`

`params`：`name`、`x`、`y`，可选 `layer`。`x`、`y` 是原点所在的画布格。`layer` 是像素所在的层，省略表示不挂层。

`result`：`{ "index", "name", "x", "y", "layer", "anchors" }`。`name` 是去掉空白后的名字。`layer` 没有挂层时是 `null`。`anchors` 这时是 `[]`。`index` 是列表末尾。

错误：`PartName`、`PartExists`、`OutOfBounds`（原点不在画布上）、`LayerIndex`。

### `setPart` { #setpart }

`setPart(name, x?, y?, layer?, clearLayer?, rename?)`

`params`：`name`，外加 `x`、`y`、`layer`、`clearLayer`、`rename` 里的至少一个。没写的字段保持原样。`clearLayer: true` 去掉层挂接，不能和 `layer` 一起写。锚点的本地坐标不动。

`result`：改完之后的部件对象，形状与 `addPart` 相同。结果和原来一样时不进撤销栈。

错误：`PartName`、`PartExists`、`PartMissing`、`OutOfBounds`、`LayerIndex`。可选字段都缺，或 `layer` 与 `clearLayer` 同时出现时，是 `InvalidParams`。

### `removePart` { #removepart }

`removePart(name)`

`params`：`name`。像素还在，部件和它的锚点一起删掉。

`result`：`{ "index", "name" }`。`index` 是删掉之前的位置。`name` 是去掉空白后的名字。

错误：`PartName`、`PartMissing`。

### `setAnchor` { #setanchor }

`setAnchor(part, name, x, y)`

`params`：`part`（部件名）、`name`、`x`、`y`。`x`、`y` 是本地像素格，整数，范围 `-256..=256`。同名锚点已存在时改偏移；不存在时追加。

`result`：`{ "part", "partIndex", "index", "name", "x", "y" }`。`part` 和 `name` 是去掉空白后的名字。同一个偏移不进撤销栈。

错误：`PartName`、`PartMissing`、`AnchorName`、`AnchorCoord`。`x` 或 `y` 不是整数时是 `InvalidParams`。

### `removeAnchor` { #removeanchor }

`removeAnchor(part, name)`

`params`：`part`、`name`。部件还在，只删这个锚点。

`result`：`{ "part", "partIndex", "index", "name" }`。`index` 是删掉之前的位置。

错误：`PartName`、`PartMissing`、`AnchorName`、`AnchorMissing`。

### `translatePart` { #translatepart }

`translatePart(name, dx, dy, frame?, reportDirty?)`

`params`：`name`、`dx`、`dy`（整数，可负）。可选 `frame`、`reportDirty`。

把部件原点平移 `(dx, dy)` 格。锚点的本地坐标不变，所以握持点的画布位置跟着原点走。挂了层时，这一帧该层上每一个非 0 索引按同样的位移搬走，源格清成 0；没挂层时只改元数据。不改调色板，不发明新索引。不切换 `activeFrame` 或 `activeLayer`。别的帧的像素不动，部件列表仍是整份文档一份。其它部件的原点不动；和它挂在同一层上的墨水会一起被搬走，所以枪应独占一层。

位移为 0 时不进撤销栈。原点或任一非 0 格的落点出画布时是 `PartOffCanvas`，一格都不写。有选区时，每个非 0 格和它的落点都必须在选区内，否则 `SelectionProtected`。

`result`：改完之后的部件对象，外加 `frame`、`dx`、`dy`、`alignment`（`translate`）、`sampling`（`exact`）、`dropped`（平移恒为 0）、`pivot`（`{x, y, anchor}`，平移时 `anchor` 为 `null`，`x,y` 是平移前的原点）。

CLI：`pixelforge part translate --name gun --dx 2 --dy -1`。`--frame` 指定帧。`--dirty` 即 `reportDirty: true`。

错误：`PartName`、`PartMissing`、`FrameIndex`、`PartOffCanvas`、`SelectionProtected`、`LayerIndex`。`dx` 或 `dy` 不是整数时是 `InvalidParams`。

### `rotatePart` { #rotatepart }

`rotatePart(name, degrees, anchor?, frame?, reportDirty?)`

`params`：`name`、`degrees`（有限数，绝对值不超过 1000000）。可选 `anchor`、`frame`、`reportDirty`。

绕枢轴顺时针转。`anchor` 给出时，枢轴是这个锚点当前的画布格；省略时枢轴是部件原点格。枢轴是这一格的中心（`alignment` 为 `center-on-anchor` 或 `center-on-origin`）。屏幕坐标 `y` 向下。

角度规则：

1. 先对 360° 取模，收进 `(-180, 180]`。`-180` 收成 `180`。
2. 再量化到最近的 0.01°。正好差一半（第三位小数是 5 且后面没有更多小数）时远离 0。例如 `90.004` 和 `89.995` 收成 `90.00`，`90.005` 收成 `90.01`，`20` 和 `20.004` 都是 `20.00`，`20.005` 是 `20.01`，`0.004` 是 `0`，`0.005` 是 `0.01`。
3. 量化后是 90° 的整数倍时，`grid` 为 `quarter`，`sampling` 为 `exact`：用整数映射，不插值。90° 顺时针把相对枢轴的 `(dx, dy)` 送到 `(-dy, dx)`。成功的 90° 再转三次回到原样；`90` 再接 `-90` 也回到原样。`degreesCenti` 为 `0`、`9000`、`18000` 或 `-9000`。
4. 其它角度 `grid` 为 `nearest`，`sampling` 为 `nearest`。每个非 0 格用它的格心绕枢轴格心旋转，再落到最近的格。正好落在两格正中（相对枢轴的坐标是 `n+0.5`）时远离枢轴；和半整数相差小于 `1e-9` 的值先当成那个半整数，免得级数误差翻掉平局。正弦和余弦是内核里的泰勒级数，不是系统 `libm`。`degreesCenti` 是量化后的百分之一度。

两个非 0 格量化到同一格时，行优先较后的那一格留下，较早的索引丢掉，源格仍清成 0。`dropped` 是丢掉的源格数。`conflicts` 是被两个及以上源格抢到的目标格数。精确的直角、平移和翻转不会重叠，这两个数都是 0。

最近邻之后做**覆盖补洞**。空格要同时满足才补上：和正向映射已经写下的墨水四邻接；格内 4×4 个采样点（相对格心 `-0.375,-0.125,0.125,0.375`）至少一半，沿整条变换的逆映射落回源墨水。颜色取这些命中里的多数索引，平局取较小索引。补上的格数是 `sealed`，并且不会超过 `dropped`，所以没有碰撞时不会把细线涂粗。选区没盖住的空格不补，也不因此让整次失败。直角、平移、翻转不补洞。

`breaks` 是结果里只靠斜角相接的像素对：两格切比雪夫距离为 1、正交不相邻，而且两条正交桥都是空的。实心块补洞之后可以是 0。1px 斜线通常大于 0，那是阶梯，不是漏点。

这些数在 `rotatePart`、`translatePart`、`flipPart`、`solveConstraints` 和 `bake` 里都有。`dropped` 不会因为补洞变小：碰撞仍算丢掉。7×3 实心块绕中心转 20° 时，正向映射是 19 格、`dropped` 为 2；补洞后是 21 格、`sealed` 为 2、`dropped` 仍是 2。2×12 的细条在 20° 会补回 1 格，`breaks` 从大于 0 变成 0。1px 线在 20° 不补洞，`breaks` 仍大于 0；靠近 45° 时中心叠在一起，线变短，`sealed` 可以仍是 0。

旋转不能代替手修。缺口、斜接和变短的枪管要自己改像素。手指一类的修正画在 `polish` 层，下一次 `bake` 不会把它写掉。非直角仍可以留下空洞：没有墨水落上去、又不是被搬走的源格，就保持原索引。

只改 `frame`（缺省为当前帧）上挂接层的索引，并改这一份原点与锚点。锚点先变成画布坐标再映射，然后重新写成相对新原点的本地格，这样握持点在整数变换里仍停在原来的画布格上。本地结果超出 `-256..=256` 时是 `AnchorCoord`，一格都不写。落点出画布是 `PartOffCanvas`。选区规则与 `translatePart` 相同。量化成 0° 时不进撤销栈。

按帧采样变换参数，见[部件曲线](#part-curves)；求解部件位置，见[约束](#constraints)；写入动画帧，使用 `bake`。

错误：`PartName`、`PartMissing`、`AnchorName`、`AnchorMissing`、`AnchorCoord`、`PartAngle`、`PartOffCanvas`、`SelectionProtected`、`FrameIndex`、`LayerIndex`。`degrees` 不是有限数时是 `InvalidParams`。

### `flipPart` { #flippart }

`flipPart(name, direction, anchor?, frame?, reportDirty?)`

`params`：`name`、`direction`（`horizontal` 或 `vertical`）。可选 `anchor`、`frame`、`reportDirty`。

绕枢轴做镜像。水平和 `flipHorizontal` 一样只搬索引、不改颜色：格 `x` 收到 `2 * pivotX - x`，`y` 不变。垂直则 `y` 收到 `2 * pivotY - y`。枢轴格自己不动。这是绕**一格的中心**，不是矩形 `flipHorizontal` 那种绕矩形中心（宽为偶数时中心在两格之间）。因此整帧 `hflip` 一般不等于绕原点的 `flipPart`。矩形翻转仍然不搬部件。

`sampling` 为 `exact`，`alignment` 同旋转。同一方向再翻一次，墨水还在画布上时回到原样。选区、出界、单帧、一步撤销，与 `translatePart` 相同。结果和原来一样时不进撤销栈。

`result` 另有 `direction`。

CLI：`pixelforge part flip --horizontal --name gun --anchor grip`。必须恰好写 `--horizontal` 或 `--vertical` 之一，否则退出码 `2`。

错误：`PartName`、`PartMissing`、`AnchorName`、`AnchorMissing`、`AnchorCoord`、`PartOffCanvas`、`SelectionProtected`、`FrameIndex`、`LayerIndex`。`direction` 不是那两个字符串时是 `InvalidParams`。

### 部件曲线 { #part-curves }

曲线是部件变换参数在帧下标上的轨道，和标签、`delayMs` 共用这一条时间轴。采样不读 `delayMs`，也不推动播放时钟。`frame` 为整数时是某一帧；为小数时落在两帧之间。`2` 是第 2 帧，`2.5` 是第 2 帧到第 3 帧的中点（按帧下标，不按毫秒）。结果是相对静止姿势的参数。它不写像素，也不调用 `rotatePart` / `translatePart` / `flipPart`。`bake` 从静止姿势读这些采样并搬走墨水。已经变换过的帧再用 `rotatePart` 施加一次会叠加上去。

一个部件的一个通道一条曲线。通道：

| `channel` | `value` | 缺省 `ease` |
| --- | --- | --- |
| `rotation` | 顺时针角度。和 `rotatePart` 一样收到 `(-180, 180]`，量化到 0.01°，平局远离 0 | `linear` |
| `translateX`, `translateY` | 相对静止原点的整格。必须是整数 | `linear` |
| `flipHorizontal`, `flipVertical` | `0` 或 `1` | `hold` |

关键帧上的 `ease` 是离开这一帧、走向下一帧。最后一帧的 `ease` 会存下来，只有后面再出现关键帧才参与采样。`t = (frame - left) / (right - left)`，`u` 是权重，采样值是 `left + (right - left) * u`：

- `linear`：`u = t`
- `easeInOut`：`u = t² (3 − 2t)`（Hermite smoothstep）。`t = 1/2` 时 `u = 1/2`，中点和线性相同。`t = 1/4` 时 `u = 5/32`
- `hold`：`u = 0`，停在左关键帧，直到下一关键帧的下标。翻转只能用这个

旋转在存好的百分之一度上混合，不走 ±180° 的短弧。关键帧区间外夹住端点（`interpolation` 为 `clamp`）。正好落在关键帧上返回该帧（`interpolation` 为 `key`），不混合。

插帧、删帧在同一步撤销里挪关键帧下标。删掉的那一帧上的关键帧被去掉；曲线空了就去掉。部件改名会带着曲线走。删掉部件，或 `resizeCanvas` 把原点裁出画布，曲线一起去掉。工程文件版本仍是 1。没有曲线时不写 `curves`。缺这个字段的旧文件按没有曲线读入。未知字段仍然拒绝。文件里的 `ease` 缺省时不写。

### `setCurveKey` { #setcurvekey }

`setCurveKey(part, channel, frame, value, ease?)`

`params`：`part`、`channel`、`frame`、`value`。可选 `ease`。

`frame` 必须是已有帧的下标。同一帧再设一次会替换值和缓动。结果和已存的关键帧相同时不进撤销栈。

`result`：`index`、`keyIndex`、`part`、`channel`、`frame`、`value`（量化后的数）、`ease`。

CLI：`pixelforge curve set --part gun --channel rotation --frame 0 --value 0 --ease ease-in-out`。`--ease ease-in-out` 和 `easeInOut` 相同。`--channel translate-x` 和 `translateX` 相同。

错误：`PartName`、`PartMissing`、`FrameIndex`、`CurveValue`、`CurveEase`。`channel` 或 `ease` 不是已知名字时是 `InvalidParams`。

### `removeCurveKey` { #removecurvekey }

`removeCurveKey(part, channel, frame)`

删掉这一帧上的关键帧。最后一帧被删掉时，整条曲线去掉，`removedCurve` 为 true。像素不动。

错误：`PartName`、`PartMissing`、`CurveMissing`、`CurveKeyMissing`。

### `clearCurve` { #clearcurve }

`clearCurve(part, channel)`

删掉这一条曲线。

错误：`PartName`、`PartMissing`、`CurveMissing`。

### `listCurves` { #listcurves }

`listCurves(part?)`

`params`：无，或可选 `part`。只读。

`result`：`{ "curves": [ { "index", "part", "channel", "keys": [ { "frame", "value", "ease" } ] } ] }`。响应里的 `ease` 总是写出，包括缺省值。指定了 `part` 但部件不存在时是 `PartMissing`，不是空数组。

### `sampleCurve` { #samplecurve }

`sampleCurve(part, channel, frame)`

`params`：`part`、`channel`、`frame`。`frame` 是有限数，可以是小数。只读。不写像素。

`result`：`part`、`channel`、`frame`、`value`、`interpolation`（`key`、`clamp`、`linear`、`easeInOut` 或 `hold`）、`leftFrame`、`rightFrame`（夹住或正好在关键帧上时是 `null`）。

从 0° 到 90°，`easeInOut` 在中点是 45，在四分点是 14.0625。线性的四分点是 22.5。

错误：`PartName`、`PartMissing`、`CurveMissing`、`CurveFrame`。

### `samplePart` { #samplepart }

`samplePart(part, frame)`

`params`：`part`、`frame`。只读。不写像素。

`result`：`part`、`frame`，再加上已经有曲线的通道。没有曲线的通道省略，不是 0。字段名就是通道名：`rotation`、`translateX`、`translateY`、`flipHorizontal`、`flipVertical`。

CLI：`pixelforge curve sample --part gun --frame 2`。不写 `--channel` 就是这一条。

错误：`PartName`、`PartMissing`、`CurveFrame`。

### 约束 { #constraints }

约束写在工程文件的 `constraints` 里。没有约束时省略。旧文件没有该字段时按空列表读入。版本仍是 1。它是元数据：名字、种类、从动部件，以及目标锚点或钉点。像素仍在层上。

三种：

| `kind` | 做什么 | 字段 |
| --- | --- | --- |
| `follow` | 平移 `part`，使 `anchor`（省略则原点）落到 `target` 的 `targetAnchor` | 要 `target`、`targetAnchor`。不能有 `x`、`y` |
| `attach` | 同上，两边锚点都要。枪托贴肩 | 要 `anchor`、`target`、`targetAnchor`。不能有 `x`、`y` |
| `pin` | 平移 `part`，使 `anchor`（省略则原点）落到画布格 `(x, y)` | 要 `x`、`y`，都在画布内。不能有 `target`、`targetAnchor` |
| `ik` | 旋转 2 或 3 节链，使末端锚点（省略则末端原点）正好落到目标格 | `chain` 从根到末端，最后一节等于 `part`。目标是 `x`,`y`，或 `target`+`targetAnchor`，或 `contacts`。够不着是 `IkUnreachable` |
| `plant` | 在脚印区间内平移 `part`，把锚点钉在那一格。区间外不钉，曲线照旧 | `contacts`：`{frame,to,x,y}`。`to` 可省略，等于 `frame`。不能有链、目标和钉点 |

Tool API 和 CLI 接受 `stick`，存成 `attach`。文件里的 `kind` 接受 `follow`、`attach`、`pin`、`ik`、`plant`。

`ik` 的骨长是静止姿势里相邻原点的整格差，最后一节再加末端锚点的本地偏移。求解从静止朝向出发，根部件保留曲线平移，链上其它部件在这一帧的旋转和翻转曲线不参与。`bend` 为 `1` 时第一关节在根指向目标的顺时针一侧，`-1` 是另一侧。省略时沿用静止姿势的那一侧；静止是直的则两侧都可以，取离静止角最近的一解。另一侧够得着而指定的一侧不够，是 `IkUnreachable`，不会悄悄换边。末端格必须正好是目标格。

`plant` 和带 `contacts` 的 `ik` 用同一份脚印。采样帧 `f` 落在 `frame <= f <= to` 里就钉那一格。区间可以跨多帧，脚底格不变，所以不会顺着曲线滑。区间外的帧不钉。省略 `solveConstraints` 的 `frame` 时，脚印按 `pixelFrame`（缺省当前帧）取样。脚印的帧下标必须已经存在；插帧、删帧会像曲线关键帧一样挪这些下标。一条 `plant` 的脚印被删光就去掉这条约束。

一条 `ik` 驱动链上的每一节，这些部件不能再被别的约束驱动。`--part` 只点了末端时，`bake` 会把整条链一起摆。每个会动的关节仍要单独一层。

`rotatePart` 和 `setCurveKey` 不求解。求解是 `solveConstraints`。`frame` 给出时，先从**当前存着的姿势**采样该部件的曲线，再解约束。通道顺序：绕存着的原点旋转、水平翻转、垂直翻转，然后整格平移。平移小数取最近格，一半远离 0。曲线枢轴是原点。省略 `frame` 则不读曲线。

求解顺序等被驱动的目标先完成。互不依赖的保持文件顺序。自指或环是 `ConstraintCycle`。同一部件两条驱动是 `ConstraintConflict`。添加时就会拒绝，名单不变。两个会动的部件共用一层是 `ConstraintLayer`。一个会动的部件可以和同层不动的部件共存，该层每一个非 0 格跟着走。

`apply` 缺省 false：做完检查，返回报告，不进撤销栈。`apply` true：写到 `pixelFrame`（缺省当前帧）。挂接层非 0 墨水按「旋转 / 翻转 + 一次平移」搬走，`ink` 为 `moved`。没挂层只改原点，`ink` 为 `metadata`。没有位移是 `unchanged`。部件或像素有变化才是一步撤销。再调用一次叠在存下的姿势上。不新建帧，不沿时间轴逐帧写。整段帧是 `bake`。

失败整次不写。删锚点不删约束，下次求解是 `AnchorMissing`。原点或最终墨水出画布是 `PartOffCanvas`。改名改写 `part` 和 `target`。删部件丢掉提到它的约束。`resizeCanvas` 平移钉点，钉点出画布或部件被裁掉时丢掉该约束。未知字段拒绝。读入时锚点可以暂时不在，求解时再查。

### `addConstraint` { #addconstraint }

`addConstraint(name, kind, part, anchor?, target?, targetAnchor?, x?, y?, chain?, bend?, contacts?)`

`params`：`name`、`kind`（`follow`、`attach`、`stick`、`pin`、`ik`、`plant`）、`part`。按种类再给 `anchor`、`target`、`targetAnchor`、`x`、`y`、`chain`、`bend`、`contacts`。

不移动像素。环或第二条驱动会被拒绝，名单不变。钉点或脚印出画布是 `PartOffCanvas`。一步撤销。

`result`：`index`、`name`、`kind`（`stick` 写成 `attach`）、`part`、`anchor`、`target`、`targetAnchor`、`x`、`y`、`chain`、`bend`、`contacts`。用不到的字段是 `null` 或空数组。

错误：`ConstraintName`、`ConstraintExists`、`ConstraintFields`、`ConstraintCycle`、`ConstraintConflict`、`PartName`、`PartMissing`、`AnchorName`、`AnchorMissing`、`PartOffCanvas`。`kind` 不是那四个字符串时是 `InvalidParams`。

### `removeConstraint` { #removeconstraint }

`removeConstraint(name)`

`params`：`name`。部件和像素不动。

`result`：`{ "index", "name" }`。`name` 是去掉空白后的名字。

错误：`ConstraintName`、`ConstraintMissing`。

### `listConstraints` { #listconstraints }

`listConstraints()`

参数：无。只读。

`result`：`{ "constraints": [ ... ] }`。每一项与 `inspect` 的 `constraints` 相同。没有约束时是 `[]`。

### `solveConstraints` { #solveconstraints }

`solveConstraints(frame?, apply?, pixelFrame?)`

`params`：可选 `frame`（有限数，可以是小数）、`apply`（布尔，缺省 false）、`pixelFrame`。

| 字段 Field | 含义 Meaning |
| --- | --- |
| `frame` | 传入的曲线帧，省略时是 `null` |
| `pixelFrame` | 写入的像素帧 |
| `apply`, `wrote` | 是否要求写入；这一次是否真的改了部件或像素 |
| `dropped` | 各层最近邻碰撞丢掉的源格数之和。直角、平移为 0。补洞不把它清成 0 |
| `conflicts` | 各层被两个及以上源格抢到的目标格数之和 |
| `sealed` | 各层覆盖补洞写上的格数之和。不超过该层的 `dropped` |
| `breaks` | 各层结果里只靠斜角相接的像素对之和 |
| `order` | 求解顺序上的约束名 |
| `constraints` | 按文件下标。含 `dx`、`dy`（末端或从动部件这一条加上的整格）、`satisfied`、`planted`（这一帧是否落在脚印区间里）、`from`、`to` |
| `parts` | 解完的姿势。`anchors` 里有本地格和 `canvasX`、`canvasY`。`ink` 是 `unchanged`、`metadata` 或 `moved`。这一层被搬走时，`dropped`、`conflicts`、`sealed`、`breaks` 是该层的数；否则是 0 |

CLI：`pixelforge constraint solve --frame 4` 只读。`pixelforge constraint solve --apply` 用存下的姿势写入当前帧。`pixelforge constraint solve --frame 4 --apply --pixel-frame 0` 先采样再写入第 0 帧。

错误：`CurveFrame`、`CurveValue`、`AnchorMissing`、`PartMissing`、`PartOffCanvas`、`SelectionProtected`、`ConstraintFields`、`ConstraintCycle`、`ConstraintConflict`、`ConstraintLayer`、`IkUnreachable`、`FrameIndex`。`frame` 不是有限数或 `apply` 不是布尔时是 `InvalidParams`。

### `bake` { #bake }

`bake(from?, to?, source?, fps?, parts?, layers?, constraints?)`

`params` 全是可选的：

| 字段 Field | 含义 Meaning |
| --- | --- |
| `from` | 闭区间第一帧。缺省 0。可以等于现有帧数，表示只追加 |
| `to` | 闭区间最后一帧。缺省最后一帧。超过现有帧数则追加。必须小于 4096 |
| `source` | 静止墨水所在帧。缺省 0。这一帧的像素不改 |
| `fps` | 大于 0 的有限数。区间里每一帧（含源帧）的 `delayMs` 变成 `round(1000/fps)`。12 是 83 |
| `parts` | 只摆这些部件。省略则摆每一个。空数组是 `InvalidParams` |
| `layers` | 只写这些层。省略则写受曲线或约束管理的挂接层，变换全为 0 也从源帧重写；给了 `parts` 又省略 `layers` 时，写这些部件挂着的层。`polish` 层即使被点名也不写 |
| `constraints` | 缺省 true。false 只采样曲线，不解约束 |

每一非源帧都从源帧的墨水摆姿势，不从该帧已经有的像素接着摆，所以再烤一次不叠加。通道顺序与 `solveConstraints` 带 `frame` 时相同。存着的原点不改。曲线和约束不删。报告里的 `x`、`y` 是摆好的原点。

已有帧只替换要写的层，其它层保持原样。先复制静止帧，没挂部件的身体才会留在这些帧上。追加的帧先整帧复制源帧，再盖上摆好的层。`from` 大于现有帧数是 `BakeRange`。

`parts` 给定时，只摆名单里的部件，从动部件只跟同样被选中的目标。`layers` 再把写入收成交集。两个都省略时，写这一次采样或求解管到的挂接层，不看当前位移是不是 0。曲线回到 0、约束增量也是 0 时，这些层从源帧恢复身份姿势。没挂曲线、也没被约束驱动的层不动。同一层上两个会动、且这一层会被写入的部件是 `ConstraintLayer`。挂接层上每一个非 0 格都搬走。

`role` 为 `polish` 的层是精修覆盖层，不从挂接层的源墨水重建。`--layer` 或 `parts` 点到它时，不会把它当成要重建的挂接层。曲线或约束管着的部件挂在这层上是 `PolishLayer`，整次不写。手指修正画在覆盖层。没绑定的覆盖层留在画下的画布格，`bake` 不搬它。`setLayerBind` 绑到部件之后，这一次如果摆了那个部件（有曲线、约束，或这一帧有位移），就把源帧上的修正和显式擦除按同一套变换写到目标帧。身份姿势也会从源帧盖回去，所以改回 0 不会留下上一次的错位。部件没被这次烘焙摆到时，绑定层不动。追加帧仍先复制源帧的覆盖层，再盖上摆好的绑定层；只存在于别的帧上的未绑定精修不会被复制。

选区必须盖住搬走的源格和落点；追加帧上新写入的非 0 格，以及显式擦除格，也必须在选区内。否则 `SelectionProtected`，一格都不写。没绑定的精修不参与这次写入，所以选区没盖住的未绑定精修格不会让烘焙失败。绑定层在选区里，和挂接层一样。出画布、缺锚点、环、第二条驱动同样整次不写。

像素或时长有变化才是一步撤销。源帧只可能改时长。当前帧不因这次调用而改变。工程文件仍是 version 1，没有 `bake` 字段。

| 字段 Field | 含义 Meaning |
| --- | --- |
| `from`, `to`, `source` | 实际使用的区间和源帧 |
| `fps` | 传入的帧率，省略时是 `null` |
| `delayMs` | 写进区间的时长，没给 `fps` 时是 `null` |
| `constraints` | 这次是否求解 |
| `appended` | 新追加的帧数 |
| `wrote` | 像素或时长是否真的变了 |
| `dropped` | 各帧最近邻碰撞丢掉的格数之和。补洞不把它清成 0 |
| `conflicts` | 各帧被抢到的目标格数之和 |
| `sealed` | 各帧覆盖补洞写上的格数之和 |
| `breaks` | 各帧只靠斜角相接的像素对之和 |
| `polish` | 精修覆盖层的下标，从底到顶。没有覆盖层时是 `[]`。未绑定的不写；绑定的见每帧 `followed` |
| `frames` | 区间里每一帧。`rest` 为真的是源帧，`layers` 和 `followed` 为空，四个质量数都是 0。其它帧带 `dropped`、`conflicts`、`sealed`、`breaks`。`parts` 与 `solveConstraints` 的 `parts` 相同；`ink` 是 `moved` 或 `unchanged`。没挂层、或这一层没写时是 `unchanged`，因为原点不写进文件。`layers` 不含 `polish`。`followed` 是这次按部件变换盖上去的绑定精修层 |

错误：`BakeRange`、`BakeFps`、`FrameDelay`、`FrameIndex`、`LayerIndex`、`PartName`、`PartMissing`、`CurveValue`、`AnchorMissing`、`PartOffCanvas`、`SelectionProtected`、`ConstraintCycle`、`ConstraintConflict`、`ConstraintLayer`、`PolishLayer`、`IkUnreachable`。`fps` 不是有限数、`constraints` 不是布尔、`parts` 或 `layers` 是空数组或类型不对时是 `InvalidParams`。选了 `ik` 的末端时，链上其它关节会一起摆。

### `beginGesture` / `endGesture` { #begingesture-endgesture }

`beginGesture()` · `endGesture()`

参数：无。把之后的成功修改收成一步撤销，直到 `endGesture`。嵌套的 `beginGesture` 返回 `OpenGesture`，已打开的 gesture 保持打开。没有打开的 `endGesture` 不报错，也不进栈。空的 gesture 不进栈。

### `undo` / `redo` { #undo-redo }

`undo()` · `redo()`

参数：无。撤销或重做最近一步内核操作。打开且已经有改动的 gesture 会先封口，再被一次 `undo` 整段撤销。

错误：`NothingToUndo`、`NothingToRedo`、`OpenGesture`（重做时 gesture 里已经有改动）。

CLI 的 `--checkpoint FILE` 不把栈写进工程文件。检查点一步是一次改过工程的进程。这一进程里内核栈还有操作时，`undo` 先撤回内核操作；内核栈空了才撤回检查点里的上一步。不带 `--checkpoint` 的新进程仍是 `NothingToUndo`。检查点和当前工程对不上时，命令在调用工具之前失败，码是 `CheckpointMismatch`，退出码 2，文件不动。

### `saveProject` { #saveproject }

`saveProject()`

参数：无。

`result`：`{ "json": "<pixelforge-project version 1>" }`。`json` 是内核写出的工程文本，含结尾换行。撤销栈和播放时钟不在文件里。

### `loadProject` { #loadproject }

`loadProject(json)`

`params`：`json`（工程文件文本，不是对象）。解析成功后用 [`Document::load_project`] 换掉当前文档。新文档的撤销栈是空的，播放是停着的。

`result`：与 `inspect` 相同。

错误：`ProjectFile`。解析失败时原来的文档还在。

### `importAseprite` { #importaseprite }

`importAseprite(bytes)`

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

错误：`AsepriteFile`、`AsepriteSize`、`AsepriteColorMode`、`AsepritePalette`、`AsepriteTilemap`、`AsepritePixel`、`AsepriteLayers`。`bytes` 不是 Base64 是 `InvalidParams`。

CLI：`pixelforge import-aseprite --file hero.aseprite --output hero.json` 新建工程，目标已存在则拒绝。`--project hero.json` 在导入成功后替换已有工程。两个目标要且只要一个。

### `exportPngSequence` { #exportpngsequence }

`exportPngSequence(scale?)`

`params`：可选 `scale`（整数 `1..=4294967295`，缺省为 1）。不改文档，不进撤销栈。

`scale` 在内核里做最近邻放大：每个调色板索引先重复成 `scale × scale` 的块，再解析成 RGBA，没有插值，也没有抗锯齿。`scale` 为 1 时和以前的字节相同。

`result`：`{ "frames": ["<base64>", ...], "scale", "width", "height" }`。每一项是时间轴上的一帧 RGBA PNG。`width` 和 `height` 是放大后的像素尺寸。Base64 是标准字母表，带 `=` 填充，没有换行，也没有 `data:` 前缀。

`scale` 为 0 或不是该范围的整数时是 `InvalidParams`。放大后放不进 PNG 时是 `ExportScale`。

### `exportSpriteSheet` { #exportspritesheet }

`exportSpriteSheet(scale?, columns?, atlas?)`

`params`：可选 `scale`，规则与 `exportPngSequence` 相同。不改文档，不进撤销栈。

| 字段 Field | 含义 Meaning |
| --- | --- |
| `png` | 网格 sprite sheet 的 Base64 PNG |
| `columns`, `rows` | 未传 `columns` 时是 `ceil(sqrt(帧数))` 和 `ceil(帧数 / 列数)`。传入 `columns` 时用那个列数，行数是 `ceil(帧数 / columns)`。`scale` 不改变这两格 |
| `sheetWidth`, `sheetHeight` | 整张图的像素尺寸，已乘 `scale` |
| `scale` | 使用的倍数，缺省请求时为 1 |

可选 `columns`（整数 `1..=4294967295`）。缺省时仍是接近正方形的网格。走路或四方向可以传帧数，让一行就是一个方向。列数大于帧数时，多出来的格子是透明空位。`columns` 为 0 是 `InvalidParams`。列数让整张图放不进 PNG 时是 `SheetColumns`。

每一格帧变成 `帧宽 × scale` 乘 `帧高 × scale`。末尾的透明空位同样放大。排布和 Godot / Unity 约定见 [export.md](export.md)。

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

`atlas` 不是布尔时是 `InvalidParams`。

`pixelforge validate-atlas` 读取图集 JSON 及其 `image` 指向的 PNG。根对象或帧可提供 `pivot`；提供时会检查原点。校验规则和退出码见[导出指南](export.md)。

### `exportApng` { #exportapng }

`exportApng(scale?, tag?, from?, to?, loop?)`

`params`：可选 `scale`（规则与 `exportPngSequence` 相同，缺省为 1）。可选 `tag`（标签名）。不写 `tag` 时，可选 `from`、`to`（闭区间帧下标，缺省是整条时间轴）和 `loop`（`forward`、`reverse`、`pingpong`、`once`，缺省 `forward`）。`tag` 与 `from` / `to` / `loop` 不能同时出现，否则是 `InvalidParams`。不改文档，不进撤销栈。

每一帧是和 `exportPngSequence` 相同的 RGBA 合成：隐藏层和 opacity 0 跳过，opacity 255 用调色板颜色盖住，1 到 254 做直通 alpha 混合。`--scale` 在混合之后最近邻放大。文件是 APNG，不是 GIF，也不做调色板量化或抖动。理由和播放序见 [export.md](export.md)。

| 字段 Field | 含义 Meaning |
| --- | --- |
| `apng` | 这一张 APNG 的标准 Base64。没有 `data:` 前缀 |
| `format` | `"apng"` |
| `scale`, `width`, `height` | 倍数和放大后的像素尺寸 |
| `loop` | 实际使用的循环模式 |
| `plays` | APNG `num_plays`。`0` 无限循环，`1` 只播一遍（`once`） |
| `frameCount` | 播放序的帧数。`pingpong` 会把中间帧再列一次，所以可以大于区间长度 |
| `frames` | 播放序。每项是 `{ "index", "delayMs" }`。`index` 是文档帧下标 |
| `tag` | 用了标签时才有，值是规范化后的名字 |

CLI：`pixelforge export apng --project hero.json --output walk.apng --tag walk_down`。stdout 的 `result` 没有 `apng` 字段，改为 `path` 和 `bytes`。不传 `--tag` 时可以写 `--from`、`--to`、`--loop`。

未知标签是 `TagMissing`。名字空或含控制字符是 `TagName`。`from` 大于 `to`，或区间超出帧列表，是 `PreviewRange`。`scale` 为 0 是 `InvalidParams`。放大后放不进 PNG 是 `ExportScale`。

### `previewFrame` { #previewframe }

`previewFrame(frame?, scale?, layer?, compare?, selection?, x?, y?, width?, height?, mask?)`

`params`：可选 `frame`（缺省为当前帧）、可选 `scale`（规则与 `exportPngSequence` 相同，缺省为 1）、可选 `layer`。不写 `layer` 时画合成图；写了就只画那一层，并且不改变 `activeLayer`。可选 `compare: true` 改成并排图，见下面。`compare: false` 和省略一样。

范围三选一，都省略就是整幅，结果里不出现 `scope`、`mask`、`protected`：

| 参数 | `scope` |
| --- | --- |
| `selection: true` | `"selection"`。用工程里的蒙版。没有选区是 `MaskMissing` |
| `x`、`y`、`width`、`height` | `"rect"`。四个整数要齐 |
| `mask` | `"mask"`。矩形数组，至少一块。`call` 用这个；聊天目录用上面的矩形 |

同时给两种范围是 `InvalidParams`（`pass only one of selection, x/y/width/height, or mask`）。`selection` 不是布尔 `true` 也是 `InvalidParams`。

只读。不改 `activeFrame`，不进撤销栈。`result`：

| 字段 Field | 含义 Meaning |
| --- | --- |
| `frame` | 画出的帧 |
| `activeFrame` | 文档当前帧，保持原值 |
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
| `scope` | 只在限定范围时出现：`selection`、`rect` 或 `mask` |
| `mask` | 只在限定范围时出现。形状与 `selection` 相同 |
| `protected` | 只在限定范围时出现。`width * height` 减去蒙版格子数 |

`pixels` 仍是整幅索引，行优先，不是 RGB。限定范围时，`ink`、`bounds`、`indices` 只数蒙版内。蒙版外的 ASCII 是 `#`，PNG 上是只用于查看的 RGBA `[80, 64, 112, 255]`。这个颜色不是调色板项，也不写回格子。字形 `*` 会把 62 及以上的索引折成同一个字符，读索引用 `pixels` 或 `indices`。

CLI 的 `pixelforge preview` 另外把 ASCII 行放进 `result.rows`，并把 PNG 写到 `result.path`（默认是工程旁边的 `<stem>.preview.png`），stdout 里不带 base64。`ink`、`bounds`、`indices`、`pixels` 仍在这条 JSON 里。`--selection`、`--x --y --width --height`，或重复的 `--mask x,y,width,height`，对应上面三种范围。内核拒绝时退出码 `2`。

`compare: true` 时 PNG 是左右两格，中间没有间隔，宽是两倍。左格是未量化的 RGBA 合成，和彩色 preview、`export png`、`frame_rgba` 相同。右格是 `flattenLayers` 会为这一帧写下的索引，再按调色板着色，和压平之后的彩色 preview 相同。命令不调用 flatten，不改文档，不进撤销栈。工程可以仍是多层：要看半透明和收成索引的差，就在压平之前调用。已经压平、结果层又不透明时，左右相同。`pixels`、`ink`、`indices` 仍是盖住索引。右格的索引在 `flattenPixels`，ASCII 在 `flattenRows`。`indexChanges` 是盖住索引和 `flattenPixels` 不同的格子数。`colorChanges` 是左格 RGBA 和右格颜色不同的格子数。`panels` 是 `["rgba", "indexed"]`。同时给 `layer` 或任一范围是 `InvalidParams`（`compare draws the full composite; omit layer and any mask`）。CLI 是 `pixelforge preview --compare`。不写 `--output` 时文件是 `<stem>.compare.png`。stderr 仍先打盖住索引，再打 `flattenRows`。

错误：`FrameIndex`、`LayerIndex`、`ExportScale`、`MaskMissing`、`OutOfBounds`。`scale` 为 0，或同时给了两种范围，是 `InvalidParams`。

### `diffFrames` { #diffframes }

`diffFrames(frame, with, scale?, layer?, selection?, x?, y?, width?, height?, mask?)`

`params`：`frame`、`with`（都必填），可选 `scale`（缺省为 1），可选 `layer`。不写 `layer` 时比较两帧的合成索引；写了就只比较那一层。范围与 `previewFrame` 相同：`selection: true`、一块矩形，或 `mask` 数组，三选一。省略就是整幅，结果里没有 `scope`。

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
| `diffRows` | 一行一个字符串。`.` 表示索引相同；`!` 表示两边都非 0 且不同；否则是那个非 0 索引的字形（和 `preview` 相同：`1`–`9`、`A`–`Z`、`a`–`z`、`*`）。蒙版外是 `#` |
| `panels` | `["frame", "with", "onion"]` |
| `pngWidth`, `pngHeight` | PNG 像素尺寸。宽是三格并排 |
| `png` | Base64 RGBA PNG。左是 `frame`，中是 `with`，右是洋葱皮 |
| `scope`, `mask`, `protected` | 与 `previewFrame` 相同。只在限定范围时出现 |

限定范围时，`changed`、`bounds`、`kinds`、`changes` 只数蒙版内。蒙版外不是 `same`，也不是一处变化。三格 PNG 在蒙版外都涂只用于查看的 RGBA `[80, 64, 112, 255]`。

洋葱皮只存在于这张视图里：索引相同就用那一个调色板颜色；只有一边非 0 就用那边的颜色；两边都非 0 且不同时，把两个调色板颜色各通道取平均。平均色不写回工程，也不进调色板。保护色同样只在视图里。

`!` 不带两边的索引。要索引就读 `changes`。

CLI 的 `pixelforge diff` 把 PNG 写到 `result.path`（默认 `<stem>.diff.png`），并在 stdout 里附上两帧的 ASCII `rows` / `withRows`。`bounds`、`kinds`、`changes` 仍在这条 JSON 里。范围旗标与 `preview` 相同。stderr 在限定范围时会写 `# = protect`。内核拒绝时退出码 `2`。

错误：`FrameIndex`、`LayerIndex`、`ExportScale`、`MaskMissing`、`OutOfBounds`。缺 `frame` 或 `with`，或同时给了两种范围，是 `InvalidParams`。

### `flipHorizontal` { #fliphorizontal }

`flipHorizontal(frame?, x?, y?, width?, height?, layer?, reportDirty?)`

`params`：可选 `frame`（缺省为当前帧）。可选 `x`、`y`、`width`、`height`：四个一起给出就是那块矩形，四个都省略就是整帧。只给一部分是 `InvalidParams`。

把矩形里的索引左右对调。格子 `x` 收到镜像列上的索引：整帧宽 64 时就是 `63 - x`。不改调色板，也不改高光颜色。一步撤销。对称的矩形不进栈。不改变 `activeFrame`。出界则一格都不写。有选区时，只有格子和它的镜像伙伴都在选区内才交换。

错误：`OutOfBounds`、`FrameIndex`、`LayerIndex`。可选 `layer`：只镜像这一层。

### `flipVertical` { #flipvertical }

`flipVertical(frame?, x?, y?, width?, height?, layer?, reportDirty?)`

`params`：与 `flipHorizontal` 相同。可选 `frame`、`layer`、矩形四边，以及 `reportDirty`。

把矩形里的索引上下对调。格子 `(x, y)` 收到 `(x, originY + height - 1 - (y - originY))` 上的索引。不改调色板。一步撤销。整块对称时不进栈。不改变 `activeFrame`。出界则一格都不写。有选区时，只有目标格和它读到的源格都在选区内才写入。

错误：`OutOfBounds`、`FrameIndex`、`LayerIndex`。可选 `layer`：只镜像这一层。

CLI：`pixelforge vflip`，或 `pixelforge flip --vertical`。

### `rotate` { #rotate }

`rotate(degrees, frame?, x?, y?, width?, height?, layer?, reportDirty?)`

`params`：必填 `degrees`，只能是 `90`、`180` 或 `270`（顺时针）。其余与 `flipHorizontal` 相同：可选 `frame`、`layer`、矩形四边、`reportDirty`。其它角度是 `InvalidParams`，文档不变。

在矩形内部重排索引。不改调色板，也不改变画布尺寸，所以其它帧的坐标仍然有效。180° 接受任意矩形。90° 和 270° 要求矩形是正方形，否则 `RotateSquare`，一格都不写。整帧转 90° 或 270° 因此需要正方形画布；要先改边长，用 `resizeCanvas`。

目标格在矩形内的局部坐标是 `(x, y)`，矩形宽 `w`、高 `h`。它读到的源格是：

| 角度 | 源格 |
| --- | --- |
| 90 | `(y, h - 1 - x)` |
| 180 | `(w - 1 - x, h - 1 - y)` |
| 270 | `(w - 1 - y, x)` |

一步撤销。转完和原来相同则不进栈。只改指定的一层（缺省为当前层），不改变 `activeLayer`。有选区时，目标格和源格都在选区内才写入。

错误：`OutOfBounds`、`FrameIndex`、`LayerIndex`、`RotateSquare`。`degrees` 不是 90、180、270 是 `InvalidParams`。

CLI：`pixelforge rotate --degrees 90`。矩形四个边要齐，或全部省略。

### `resizeCanvas` { #resizecanvas }

`resizeCanvas(width, height, offsetX?, offsetY?, reportDirty?)`

`params`：`width`、`height`（`1..=256`）。可选 `offsetX`、`offsetY`（`i32`，可负）。两个偏移都省略时居中：`offset = (new - old) / 2`，向 0 取整。只给一个是 `InvalidParams`。

把每一帧的索引拷进新画布。旧原点落在 `(offsetX, offsetY)`。没有源的格子是索引 0。落在新画布外的旧格子被裁掉。时长、调色板、锁定和当前帧不变。工程仍是 version 1，只是 `width`、`height` 和各帧 `pixels` 变了。一步撤销能回到原来的缓冲。尺寸和索引都没变时不进栈。选区的每一块矩形跟着同一个偏移平移；只要有一块会离开新画布，整份蒙版就在这一步里清掉。像素本身不按选区裁切。

`result`：`{ "width", "height", "offsetX", "offsetY" }`，偏移是实际使用的值。

错误：`InvalidSize`（内核拒绝的边长）。`width` 超出 `1..=256` 是 `InvalidParams`。每一层都跟着缩放。

## 图层 { #layers }

层是文档级的栈，每一帧各有一块同样尺寸的索引缓冲。下标 0 在最下面，最后一层画在最上面。新建工程和旧的单层文件都是一层，名字是 `Layer 1`，`activeLayer` 是 0。这种文件保存时仍不写 `layers` / `activeLayer`，和以前的 version 1 工程相同。多了层、改了名字，或当前层不是 0 时，文件里才出现这些字段。帧上多于一层时，像素写在 `frames[].layers[].pixels`，不再写旧的 `frames[].pixels`。

合成分两条，都从底往上。`visible: false` 或 `opacity: 0` 的层跳过。索引 0 不覆盖。`polish` 层上的显式擦除是例外：索引仍是 0，但擦除标记把合成格打成透明，下层露不出来。没有标记的 0 仍是「这一格没改」。

- **索引合成**（`composite`、`inspect.composite`、ASCII `preview` / `diff`）：其余层上任何非 0 索引整格盖住下层。`opacity` 为 1..=254 也不改这里的索引。这是压平之前的 ASCII，不是彩色 preview。
- **压平**（`flattenLayers`）：把下面的 RGBA 合成收成调色板索引，规则和 `applyPng` 相同。alpha 小于 128 写成索引 0。否则按平方 RGB 距离取最近的非 0 项，平局取较小索引，alpha 不计入距离。这一格的 RGBA 已经等于盖住索引的调色板色时，留下那个索引，不因为平局改号。结果层可见且不透明，所以只由层透明度带来的半透明不会留在格子里。彩色 preview 在压平前仍是未量化的 RGB。对照：`preview --output before.png`，再 `layer flatten`，再 `preview --output after.png`。
- **RGBA 合成**（`frame_rgba`、preview 彩色 PNG、`export png`、`export sheet`，以及这份表描述的图）：`opacity` 为 255 时，非 0 索引用调色板 RGBA **替换**下层。调色板 alpha 不与下层混合，和以前的单层导出一样。`opacity` 为 1..=254 时做直通 alpha 的 source-over。源 alpha 是 `round(palette.a * opacity / 255)`，四舍五入，正好一半时远离 0（先加 127 再除以 255）。通道混合同样先加分母的一半再除。混合不写回格子。最近邻放大发生在混合之后，把已经混合的格子重复成块。

`preview`、`diff`、`export png`、`export sheet` 默认看合成。`diff` 比的是索引，所以半透明不会让两边的索引变成 `!`。`previewFrame` / `diffFrames` 以及 CLI 的 `--layer N` 只看那一层存着的索引，不看 `visible`，也不乘 `opacity`，这样隐藏层还能检查。绘制工具的 `layer` 只决定改哪一层，隐藏层仍然能画。

可见性和透明度是层的属性，每一帧共用，不是逐 cel。文件里 `visible` 缺省 true，只在 false 时写出；`opacity` 缺省 255，只在不是 255 时写出。单层、名字仍是 `Layer 1`、当前层是 0、角色是 `default`、可见、不透明时，仍然不写 `layers`。旧文件没有这两个字段，读成可见且不透明。版本仍是 1。没有 multiply / screen 等混合模式。

每一层有 `role`。缺省 `default`：普通像素，`bake` 可以在曲线或约束管到这一层时从源帧整层重建。`polish` 是精修覆盖层。文件里还可以有 `part` 和可选的 `anchor`，只在绑定到部件时写出。本地 `(0, 0)` 是该锚点在静止姿势的画布格，省略 `anchor` 时是部件原点。格子仍存在帧缓冲的画布坐标上；绑定只决定变换时跟谁走。改锚点偏移不搬已经画下的格子。删掉部件则绑定去掉，格子留在画布上。文件里只有 `polish` 才写 `role`。显式擦除写在该层像素旁的 `erase` 数组里，全 0 时省略；`1` 只能落在索引 0 上。`layer flatten` 把索引合成写进 `Layer 1`，角色回到 `default`，擦除打出的洞变成普通的索引 0，结果层可见且不透明。

### `listLayers` { #listlayers }

`listLayers()`

参数：无。只读。

`result`：`{ "layerCount", "activeLayer", "layers": [{ "index", "name", "role", "part", "anchor", "visible", "opacity" }] }`。`role` 是 `default` 或 `polish`。`part` 和 `anchor` 没有绑定时是 `null`。`opacity` 是 `0..=255`。

### `addLayer` { #addlayer }

`addLayer(name?, role?)`

`params`：可选 `name`、可选 `role`（`default` 或 `polish`，缺省 `default`）。省略 `name` 时用还没被占用的 `Layer N`，从「当前层数 + 1」起找。名字会去掉首尾空白，长度 1 到 64。新层插在当前层上面，每一帧都是透明索引，并成为当前层。一步撤销。

错误：`LayerName`。`role` 不是那两个字符串时是 `InvalidParams`。

### `setLayerRole` { #setlayerrole }

`setLayerRole(role, index?)`

`params`：`role`（`default` 或 `polish`），可选 `index`（省略时是当前层）。同一个角色不进撤销栈。像素不动。

`result`：`{ "index", "role", "part", "anchor" }`。改成 `default` 时清掉绑定。

### `setLayerBind` { #setlayerbind }

`setLayerBind(part, index?, anchor?)`

`params`：`part`（字符串，或 `null` 表示解开），可选 `anchor`（字符串或 `null`），可选 `index`（省略时是当前层）。层必须已经是 `polish`。`part` 为 `null` 时不能再给 `anchor`。同一个绑定不进撤销栈。像素不动。

错误：`LayerIndex`、`PolishBind`（不是精修层，或解开时还带锚点）、`PartName`、`PartMissing`、`AnchorName`、`AnchorMissing`。`part` 缺失或类型不对是 `InvalidParams`。工程文件里的绑定对不上部件或角色是 `ProjectFile`。

错误：`LayerIndex`。`role` 缺失或不是那两个字符串时是 `InvalidParams`。

### `setLayerVisible` { #setlayervisible }

`setLayerVisible(visible, index?)`

`params`：`visible`（布尔），可选 `index`（省略时是当前层）。同一个值不进撤销栈。像素不动。隐藏层仍可绘制。合成、导出和压平跳过它。`--layer` 预览仍能看见它。

CLI：`pixelforge layer show`、`pixelforge layer hide`。可加 `--index`。

错误：`LayerIndex`。`visible` 缺失或不是布尔时是 `InvalidParams`。

### `setLayerOpacity` { #setlayeropacity }

`setLayerOpacity(opacity, index?)`

`params`：`opacity`（整数 `0..=255`），可选 `index`（省略时是当前层）。同一个值不进撤销栈。像素不动。

`255` 完全不透明，合成时用调色板颜色替换下层。`0` 跳过这一层，`visible` 可以仍是 `true`。`1..=254` 在索引合成里整格覆盖，在 RGBA 导出里按上面的 source-over 混合。这一步不写回索引；要收成最近色用 `flattenLayers`。`128` 是最接近 50% 的一档。

错误：`LayerIndex`。`opacity` 缺失、不是整数、或超出 `0..=255` 时是 `InvalidParams`。工程文件里的 `opacity` 超出这个范围是 `ProjectFile`（`layer N opacity must be an integer from 0 to 255`）。

### `deleteLayer` { #deletelayer }

`deleteLayer(index?)`

`params`：可选 `index`，省略时删当前层。至少留一层。删掉当前层时，滑到这个下标上的那一层保持选中；删的是最上面一层时，选中新的最上面一层。

`result`：`{ "index", "activeLayer", "layerCount" }`。`index` 是被删掉的那一层。

错误：`LayerIndex`、`LastLayer`。只剩一层时，任何删除都是 `LastLayer`，包括越界下标。

### `renameLayer` { #renamelayer }

`renameLayer(name, index?)`

`params`：`name`，可选 `index`（省略时是当前层）。同一个名字不进撤销栈。

`result`：`{ "index", "name" }`，`name` 是去掉空白之后的名字。

错误：`LayerIndex`、`LayerName`。

### `setActiveLayer` { #setactivelayer }

`setActiveLayer(index)`

`params`：`index`。相同下标不进撤销栈。

错误：`LayerIndex`。

### `reorderLayer` { #reorderlayer }

`reorderLayer(from, to)`

`params`：`from`、`to`。`to` 是移动之后的下标。相同下标不进栈。当前层跟着被移动的那一层走，夹在中间的层下标加减一。0 仍然是最下面。

错误：`LayerIndex`。

### `flattenLayers` { #flattenlayers }

`flattenLayers()`

参数：无。每一帧按上面的 **RGBA 合成**收成一层，名字改回 `Layer 1`，`activeLayer` 回到 0，可见，透明度 255。隐藏层和 opacity 0 不进入。alpha 小于 128 写成索引 0，否则平方 RGB 最近的非 0 项，平局取较小索引。盖住索引的 RGBA 已经等于混合结果时留下那个索引。调色板里没有可画的颜色、又有一格需要最近色时，是 `NoPaintColor`，整次不写。保存出来就是旧的单层工程。已经是这一层默认状态时不进栈。一步撤销能回到原来的栈，包括可见性和透明度。

CLI：`layer list`、`layer add --name gear`、`layer add --name polish --role polish`、`layer role --role polish`、`layer show`、`layer hide`、`layer opacity --opacity 128`、`layer remove`（别名 `delete`）、`layer rename`、`layer select`、`layer reorder --from --to`、`layer flatten`。

## 图块与地图 { #tileset-autotile-tilemap }

图块集使用 `pixelforge-project`：画布等于图块（宽和高各自是 16 或 32），每一帧是一个图块，帧下标就是 tile id。`paint`、`fillRect`、调色板、`preview`、`diff` 照旧。`tileset` 字段省略时，工程不是图块集。

地图是另一份文件，`format` 为 `pixelforge-tilemap`，version 1。宽高是格子数，不是像素。每一层的 `cells` 行优先，长度是 `width * height`。空格是 `null`。没有翻转时格子是 tile id 整数。有翻转时是 `{ "tile", "flipH", "flipV" }`。层 0 在最下面。文件不保存撤销栈。`pixelforge call --project map.json` 发现这份 format 之后走地图工具，不走像素文档。`--checkpoint` 不能用在地图上。

四邻接 autotile 的掩码是 0 到 15。北 = 1，东 = 2，南 = 4，西 = 8。地图外的邻居算空。占格是「这一格不是 null」，不看 tile id。规则文件是 `pixelforge-autotile` version 1，`neighbors` 必须是 4，`rules` 必须正好覆盖 16 个 mask 各一次。同一次输入写出同一张图。8 邻接返回 `AutotileNeighbors`，不写格子。

Tiled `.tmj` 是正交 JSON 地图，version `"1.10"`。`firstgid` 是 1，所以 tile id 0 的 GID 是 1，空格是 0。水平翻转置 `0x80000000`，垂直翻转置 `0x40000000`。不写对角翻转。`export tileset` 和 `export tilemap` 要用同一套 `--cols` 和 `--scale`，`.tmj` 里的 `image` 是图块 PNG 的文件名。

### `setTileset` / `clearTileset` / `validateTileset` { #settileset-cleartileset-validatetileset }

`setTileset(tileWidth, tileHeight)` · `clearTileset()` · `validateTileset()`

`setTileset` 的 `params`：`tileWidth`、`tileHeight`。画布必须已经等于这个尺寸，边长只能是 16 或 32。相同的值不进撤销栈。

`clearTileset` 和 `validateTileset` 没有参数。还不是图块集时，`clearTileset` 不进栈；`validateTileset` 返回 `TilesetMissing`。

`validateTileset` 的 `result`：`tileWidth`、`tileHeight`、`tileCount`（帧数）。

错误：`TileAlign`、`TilesetMissing`。

### `exportTileset` { #exporttileset }

`exportTileset(scale?, columns?, image?)`

`params`：可选 `scale`（默认 1）、`columns`（默认一行，列数等于帧数）、`image`（默认 `tiles.png`，必须是相对 `.png` 文件名）。

`result`：`png`（base64）、`meta`（`pixelforge-tileset` version 1）、`columns`、`rows`、`scale`、`imageWidth`、`imageHeight`、`tileWidth`、`tileHeight`（放大后的像素）、`sourceTileWidth`、`sourceTileHeight`、`tileCount`、`image`。

不改文档。CLI `export tileset` 把 PNG 和 meta 写到磁盘，stdout 里是路径，没有 base64。

错误：`TilesetMissing`、`TileAlign`、`TileImage`、`ExportScale`、`SheetColumns`。

### 地图工具 { #tilemap-tools }

这些名字在 `pixelforge tools` 的 `tilemapTools` 里，不在像素文档的 `tools` 里。`TileMap::call_tool` 用同一份信封。

| tool | params | 作用 |
| --- | --- | --- |
| `inspectTilemap` | 无 | 宽、高、图块尺寸、`tileCount`、层名和已填格数 |
| `getTile` | `x`、`y`，可选 `layer` 或 `name` | 读一格。空格的 `tile` 是 null |
| `setTile` | 上面的，再加 `tile`（null 清空）、可选 `flipH`、`flipV` | 写一格。相同的值不进撤销栈 |
| `fillTiles` | `x`、`y`、`width`、`height`、`tile`，可选翻转和层 | 矩形必须整块在地图内 |
| `clearTiles` | `x`、`y`、`width`、`height`，可选层 | 清成空格 |
| `addTileLayer` | `name` | 在最上面加一层空层。最多 32 层 |
| `autotile` | `rules`（对象或 JSON 字符串），可选层 | 用占格的四邻接换块。空格不动 |
| `validateTilemap` | 可选 `rules` | 查越界 id；给了规则再查规则里的 tile |
| `exportTilemap` | `tilesetProject`（工程 JSON 字符串），可选 `image`、`columns`、`scale` | `png` 是 base64 预览，`tmj` 是 Tiled 对象 |
| `undo` / `redo` | 无 | 只撤销这一进程里的地图编辑 |

`validate-tiles` 可以同时收 `--project`、`--map`、`--rules`。至少给一个。对不齐或越界是退出码 1。文件打不开是退出码 2。

在[图块地图案例](trials/111-lighthouse-tilemap.md)中，地面使用图块 0，围墙按四邻接规则选择图块。地图以 `base.tmj` 和相邻的 `tiles.png` 交付。

## 项目清单 { #project-manifest }

版本 1 的 `pixelforge-manifest` 记录画布规格、视角、目录、色板路径和素材依赖。像素数据保存在各自的 `pixelforge-project` 文件中。[项目清单字段与验证方法](project-manifest.md)

这些名字在 `pixelforge tools` 的 `manifestTools` 里。`ArtManifest::call_tool` 用同一份信封。文件没有撤销栈。`pixelforge call --project manifest.json` 发现这份 format 之后走清单工具，不走像素文档，也不走地图。`--checkpoint` 和 `--preview` 不能用。

| tool | params | 作用 |
| --- | --- | --- |
| `inspectManifest` | 无 | 返回清单对象。空登记不写 `assets` |
| `validateManifest` | `rootExists`，可选 `files` | 用调用方已经看到的磁盘对照约定和登记。不自己读盘 |
| `validateAssets` | `rootExists`，可选 `files` | 只查登记：缺失路径、悬空 id、owner、`produces` 对不上、环 |
| `characterMatrix` | `id`，`project`（`frameCount`、`directionLayout`、`tags`、`frames`），可选 `missingPaths` | 朝向 × 动作矩阵。调用方打开像素工程填探针。不改文件 |
| `addAsset` | `id`，`kind`，`path`，可选 `owner`，`dependsOn`，`produces`，`tags` | 追加一条。重复 id 拒绝 |
| `setAsset` | `id`，加上要改的字段 | 不改 id。`owner: null` 或列表 `[]` 表示清空 |
| `removeAsset` | `id` | 仍被引用时拒绝 |
| `listAssets` | 可选 `kind`，`owner`，`tag` | 过滤后的记录 |
| `showAsset` | `id` | 记录，加上算出的 `owns` 和 `dependents` |
| `graphAsset` | `id` | 一跳的 `nodes`、`edges`、`missing` |
| `dependentsAsset` | `id` | `dependsOn` 含有这个 id 的记录 |

`files` 的每一项是 `{ "path", "kind" }`，`kind` 为 `file`、`dir` 或 `missing`。文件还可以带 `format`、成对的 `width` / `height`，以及 `unreadable`。漏探针是 `probeMissing`。尺寸不符是 `canvasSize`。空目录是 warning `layoutEmpty`。`result.checks.layout[].projects` 是数量，不是文件名单。

CLI：`project init`、`project show`、`project validate`，以及 `asset add`、`asset set`、`asset remove`、`asset list`、`asset show`、`asset graph`、`asset dependents`、`asset validate`，还有 `character matrix` / `character check`。`validate` 与 `character check` 在 `errorCount` 大于 0 时退出码 1。工具响应本身仍是 `ok: true`。文件打不开是退出码 2，`ManifestFile`。`characterMatrix` 的 `project.frames[].hash` / `hashHflip` 可以是整数或 `0x` 十六进制字符串（避免 JSON 丢精度）。单元格状态是 `present` / `missing` / `empty` / `placeholder`。占位是 warning。绿色矩阵不是步态或审美通过。

## 不变量 { #invariants }

- 改像素、调色板、帧和时长只通过 `Document` 的公开方法。Tool API 不写第二份缓冲。
- 这些修改记在同一条撤销栈上。界面随后 `undo`，撤销的是 Agent 的那一步。
- 非法下标、锁定的调色板、坏的工程文件都返回上表里的 `error`，并且不留下半次修改。
- 洋葱皮只是视图。`diffFrames` 会返回一张，但不会把它写回像素。这个接口没有可编辑的洋葱层。限定范围时，蒙版外不参与比较，视图里标成保护色。
