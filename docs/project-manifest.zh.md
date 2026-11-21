# 项目清单参考 { #project-manifest }

版本 1 的 `pixelforge-manifest` 记录一组素材的画布尺寸、视角、共享色板、目录约定及资产依赖。交付前可用它核对文件结构，也能据此查找依赖某个角色的动作片段。可编辑的帧和图层保存在各自的 `pixelforge-project` 文件中。

## 选择素材根目录 { #out-of-scope }

将清单放在素材集合所在目录或其上级目录。登记文件与共享色板均以 `assetRoot` 为基准填写相对路径。角色和它的行走片段可以指向同一个工程，各自拥有稳定 ID。[港口守灯人完整清单](https://github.com/starcloud530/pixelforge-demos/blob/main/demos/demo-zombie/pixelforge.manifest.json)

## 清单字段 { #format }

`format` 必须为 `pixelforge-manifest`，`version` 必须为 1。出现未知字段时返回 `ManifestFile`，退出码为 2。

| 字段 | 要求 |
| --- | --- |
| `name` | 项目名称，去除首尾空白后为 1 至 64 个字符 |
| `note` | 风格或流程说明，1 至 400 个字符。名称和说明均不能含控制字符 |
| `perspective` | `top`（俯视）、`side`（侧面）、`front`（正面）或 `oblique`（斜视） |
| `assetRoot` | 相对清单的素材目录，`.` 表示清单本身所在的目录 |
| `palette` | 必填相对路径 `path`，可选名称 `ref`。JSON 必须是 PixelForge 像素工程，GPL、HEX 和 TXT 仅检查文件是否存在 |
| `canvases` | 1 至 32 个具名画布档位，`width` 和 `height` 都是 1 至 256 的整数 |
| `layout` | 1 至 64 条目录规则，每条有唯一的 `name` 和 `dir`、一个 `kind` 及可选的画布档位名 `canvas` |
| `assets` | 可选，最多 256 条登记记录，省略与空数组等价 |

目录类型有 `character`、`prop`、`tile`、`portrait`、`ui` 和 `effect`。指定画布档位后，会检查该目录直属的像素工程，跳过图集 JSON 等其他格式。路径使用正斜杠。除 `assetRoot: "."` 外，不能使用绝对路径、`.`、`..`、空路径段、冒号和反斜杠。符号链接由文件系统解析。

## 资产登记字段 { #asset-registry }

| 字段 | 含义 |
| --- | --- |
| `id` | 唯一查询键，1 至 64 个字符。首字符为 A–Z 或 a–z，后续可用这些字母、数字 0–9、点、下划线或连字符，点号之间不能出现空段 |
| `kind` | `character`、`group`、`gear`、`prop`、`tile`、`clip` 或 `sheet` |
| `path` | 素材文件相对根目录的路径，多个 ID 可以共用文件 |
| `owner` | 可选。所属角色或分组的已登记 ID，归属链中不能出现环 |
| `dependsOn` | 可选。依赖的资产 ID，校验时都应存在，不能形成依赖环 |
| `produces` | 可选。以该记录为来源的产物 ID，每个产物的 `dependsOn` 都要包含该来源 |
| `tags` | 可选。用于筛选的标签，去除首尾空白后应唯一，长 1 至 64 个字符，不含控制字符 |

修改登记内容使用 `asset set`。使用 `--clear-owner`、`--clear-depends-on`、`--clear-produces` 或 `--clear-tags` 可清除对应的可选字段。删除记录前先解除其他记录指向它的归属、依赖和产物关系。清单编辑不支持撤销。

## 创建并验证清单 { #commands }

解压[港口守灯人工程](showcase.md#port-keeper)，在含有 `a/keeper.pxf.json` 的目录中执行。以下命令新建 `keeper-registry.json`，并将两个动作片段登记到已有角色下。

~~~bash
pixelforge project init --name harbor --perspective top --note "64x64 character, four directions" --canvas sprite:64x64 --palette a/keeper.pxf.json --palette-ref keeper --layout characters:a:character:sprite --output keeper-registry.json
pixelforge asset add --manifest keeper-registry.json --id keeper --kind character --path a/keeper.pxf.json
pixelforge asset add --manifest keeper-registry.json --id keeper.idle --kind clip --path a/keeper.pxf.json --owner keeper --depends-on keeper --tag idle_down --tag idle_left --tag idle_right --tag idle_up
pixelforge asset add --manifest keeper-registry.json --id keeper.walk --kind clip --path a/keeper.pxf.json --owner keeper --depends-on keeper --tag walk_down --tag walk_left --tag walk_right --tag walk_up
pixelforge project show --manifest keeper-registry.json
pixelforge asset list --manifest keeper-registry.json --kind clip
pixelforge asset graph --manifest keeper-registry.json keeper.walk
pixelforge asset dependents --manifest keeper-registry.json keeper
pixelforge project validate --manifest keeper-registry.json
pixelforge asset validate --manifest keeper-registry.json
pixelforge character matrix --manifest keeper-registry.json keeper
pixelforge character check --manifest keeper-registry.json keeper
~~~

`project validate` 检查工程约定和资产登记；`asset validate` 仅检查登记文件与关系。两者都会输出 JSON 报告。退出码 0 表示没有错误，可能仍有告警；退出码 1 表示校验发现错误；退出码 2 表示输入文件无法打开或解析。即使报告为 `ok: true`，也应核对 `errorCount`。

`asset graph` 展示一跳范围内的关系，`asset dependents` 返回 `dependsOn` 包含指定 ID 的记录。`character matrix` 打开角色工程，对照朝向布局、登记标签和实际帧，报告存在、缺失、空帧或占位的区间。矩阵存在错误时 `character check` 返回退出码 1。需要检查运动效果时再播放对应片段。

| 诊断项 | 优先检查 |
| --- | --- |
| `assetRoot`、`layoutMissing`、`layoutKind`、`layoutEmpty` | 根目录、布局目录是否存在，类型和内容是否正确；空布局目录会产生告警 |
| `paletteMissing`、`paletteFormat`、`canvasSize`、`canvasUnread` | 色板文件格式、画布尺寸与清单是否一致 |
| `assetMissing`、`assetPath`、`assetRef`、`assetOwner` | 登记路径是否为文件，被引用 ID 是否存在，所属记录是否为合法角色或分组 |
| `assetProduces`、`assetCycle` | 产物是否依赖对应来源，依赖关系是否成环 |

## 通过工具接口访问 { #tool-api }

`pixelforge tools` 会在 `manifestTools` 中列出清单操作。使用清单作为 `call --project FILE` 的输入即可调用 `inspectManifest`、`validateManifest`、`validateAssets`、`characterMatrix`、`addAsset`、`setAsset`、`removeAsset`、`listAssets`、`showAsset`、`graphAsset`、`dependentsAsset`。清单调用不接受 `--checkpoint` 和 `--preview`。[工具接口](tool-api.md)

直接调用校验工具时，调用方应提供 `rootExists` 和 `files`，为需要核查的每个路径附上实际探测结果。`kind` 可取 `file`、`dir` 或 `missing`，像素工程的 `width` 与 `height` 需要成对提供。缺少探测项会返回 `probeMissing`。使用 CLI 校验命令时会自动采集这些信息。
