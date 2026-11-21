# 编辑与导出边界 { #known-limits }

编辑被拒绝、烘焙后的姿势与原稿有差异，或通过校验的图集在游戏中显示异常时，可按下面的操作查找原因与处理方法。

## 多个进程编辑同一文件 { #same-project-edits-wait-they-are-not-merged }

编辑命令先获取排他锁，再读取工程、图块地图或清单。默认最多等待 30 秒，`PIXELFORGE_LOCK_TIMEOUT_MS` 可调整等待时间，设为 `0` 表示只尝试一次。超时返回 `LockTimeout`，退出码为 2，此时尚未执行任何请求。等待另一进程完成后，重新读取当前状态再编辑。不同文件可以同时编辑。

保留工程旁的 `<filename>.pixelforge.lock` 文件。进程退出时操作系统会释放锁。手动删除锁文件可能导致两个写入进程各自获得不同的锁。编辑期间，`scene`、`preview` 和 `export` 等只读命令仍可读取完整快照。

本地保存会原子替换单个文件。指向已有工程的符号链接会解析到目标文件。悬空链接、符号链接形式的锁文件或具有多个硬链接的工程会返回 `UnsupportedPath`。请在本地普通文件上编辑。网络盘、云同步目录以及绕过锁直接写入的程序具有各自的一致性行为。

替换前中断时，原文件仍完整。确认无进程写入后再清理残留临时文件。Unix 上的 `DirectorySync` 发生在替换之后，请先读取原路径上的工程确认状态。Windows 替换文件时不会复制旧文件的 ACL。需要恢复或保留特殊权限时，请另外保管备份与权限配置。

工程和 `--checkpoint` 分开写入。工程已写入而检查点保存中断时，应先检查工程。随后出现的 `CheckpointMismatch` 会保留两个文件的现状。保留旧检查点用于排查，并从确认后的工程状态开始记录新的检查点。批量调用中，首个失败请求之前的编辑会保存。

## 防止旧版本覆盖新编辑 { #_1 }

用 `scene` 或 `inspect` 读取 `result.revision`，基于该快照编辑时将原值传给 `--expected-revision`。这个令牌是规范化工程内容的 16 位小写十六进制指纹。CLI 在加锁后、执行第一项操作前核对一次。只读操作和未改变工程内容的编辑都会保留原修订号。

| 返回值 | 处理方式 |
| --- | --- |
| `RevisionConflict` | 文件已改变。重新读取姿势或元数据，按当前修订号发起编辑。 |
| `RevisionInvalid` | 从响应中复制完整修订号，包括开头的零。 |
| `RevisionUnsupported` | `new`、`import-aseprite --output`、`convert --output` 等新建工程命令没有已有版本，移除修订号参数。 |

要按修订号编辑地图或清单，使用 `pixelforge call --project FILE --expected-revision REV`。`tilemap` 和 `asset` 子命令不接受此参数。

## 让精修随部件烘焙 { #unbound-polish-stays-on-canvas-cells }

未绑定的 `polish` 层会留在原来的画布坐标。将精修层绑定到一个部件后，修正像素和显式擦除会随部件一起烘焙、平移、旋转和翻转。局部 (0, 0) 默认是部件原点，也可用 `layer bind --anchor` 指定锚点。使用 `erase-rect --unmodified` 清除显式擦除，重新显示下方像素。烘焙会从源帧重建生成层，手工修正请放在精修层上。

## 检查旋转后的边缘 { #rotation-sealing-is-lossy }

非直角旋转需要将像素重新采样到网格。补洞后，单像素细线仍可能断开，在接近 45° 时也可能缩短。结合 `dropped`、`breaks` 和预览中的轮廓，在目标显示尺寸下修正断点。绑定的精修层也受同样影响。

## 设置关节与落脚点 { #ik-places-joints-it-does-not-author-gait }

逆向运动学支持由两到三个部件组成的链。可用 `pin` 或 `plant` 把接触点固定在网格上。通过关键姿势设计重心转移和帧时长，再进行烘焙。播放时观察脚掌是否踩实、髋部高度是否连贯，以及下一步的衔接。

## 压平前检查半透明图层 { #translucent-flatten-snaps-to-nearest-index }

`preview --compare` 并排显示 RGBA 合成结果和压平后将存入的索引色。压平时，alpha 小于 128 的像素变为透明索引 0。合成色若与覆盖像素的色板颜色完全相同，保留原索引；否则按 RGB 距离平方选取最近的非零色板项，距离相同取较小索引。输出图层的不透明度为 255，仅依靠层透明度产生的半透明效果会消失。需要保留该效果时请保存分层源稿。[导出与引擎导入](export.md#side-by-side)

## 检查方向填充生成的帧 { #direction-fill-is-not-a-natural-gait }

`direction set` 和 `apply` 整理帧区间与动作标签。`direction fill` 可以复制基本朝向，或在左右两个朝向之间镜像。已有像素和显式擦除默认保留，指定 `--overwrite` 才会覆盖。八向布局的斜向仍需单独绘制。运行 `direction validate` 核对区间，再逐向播放检查光照、侧面轮廓和落脚。

## 验证资产登记与动作覆盖 { #project-manifest-is-memory-not-a-canvas }

`project validate` 检查路径、画布约定和登记关系。`asset validate` 检查资产记录。要核对角色的动作覆盖情况，运行 `character matrix` 或 `character check`，区分已存在、缺失、空帧和占位区间。编辑或播放时打开角色工程。[项目清单参考](project-manifest.md)

## 在目标引擎中验收图集 { #export-validate-is-not-aesthetic-qa }

`validate-atlas` 检查 PNG 与图集 JSON。退出码为 0 时仍应查看告警。在 Godot 或 Unity 中核对实际的过滤、切片、轴心、帧时长和标签映射，并放在游戏背景上播放。APNG 适合审阅节奏，导入游戏使用精灵表 PNG 与图集 JSON。[图集校验项](export.md#validate-atlas)

## 评审 PNG 转换结果 { #png-convert-is-a-sampler-not-a-quality-score }

`convert` 将输入 PNG 采样到目标网格，并按选定策略处理色板。在目标尺寸下检查剪影、边缘和色板报告。较低的颜色误差表示结果接近采样颜色，细节是否清晰仍需看图并修整。用 `pixelforge engine list` 查看可用引擎。参数及其取值见 [convertPng](tool-api.md#convertpng)。
