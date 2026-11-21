# 导出精灵并导入游戏 { #export }

| 交付用途 | 所需文件 |
| --- | --- |
| 游戏里的动画角色 | 精灵表 PNG 和图集 JSON，后者提供帧矩形、时长与标签 |
| 逐帧图片 | 按帧编号排列的 PNG 序列 |
| 动画审阅 | 保留透明度和工程帧时长的 APNG |
| 图块地图 | Tiled 地图（`.tmj`）与其引用的图块集 PNG |

下面的精灵示例使用[港口守灯人工程](showcase.md#port-keeper)。请在角色包解压后的根目录运行命令，源文件为 `a/keeper.pxf.json`。

## 导出精灵表 { #sprite-sheet }

~~~bash
pixelforge export sheet --project a/keeper.pxf.json --cols 5 --output keeper-sheet.png --atlas keeper-sheet.json
~~~

这条命令把 20 帧排列为五列四行，每行一个朝向。帧先从左到右，再从上到下排列。省略 `--cols` 时，列数为总帧数开平方后向上取整。末行多出的单元格是透明留白。

导出时按顺序合成可见图层，透明索引 0 和完全隐藏的层不会产生可见像素。指定 `--scale K` 可用最近邻将每个格子放大为整数 K × K 的色块。默认为 1。保留原工程可继续编辑图层与帧。

## 读取图集 JSON { #atlas-json }

指定 `--atlas` 会在 PNG 旁写出版本 1 的 `pixelforge-atlas`。`image` 指向精灵表文件，移动资源时应保留这个文件名和相对位置。

| 图集数据 | 读取方式 |
| --- | --- |
| `columns`、`rows`、`frameWidth`、`frameHeight` | 网格行列数及单元格在精灵表上的实际像素尺寸 |
| `frames` | 每个有效帧对应一条记录，包括从 0 开始的索引、左上角像素坐标、尺寸和 `delayMs` |
| `tags` | 具名动作片段，`from` 到 `to` 均包含在内，`loop` 指定播放模式 |

## 校验导出结果 { #validate-atlas }

~~~bash
pixelforge validate-atlas --atlas keeper-sheet.json
~~~

校验器按图集所在目录查找 PNG，也可用 `--sheet FILE` 指定路径。标准输出包含 `result.errors`、`result.warnings` 和 `result.checks`。退出码 0 表示没有错误，1 表示已完成检查且发现错误，2 表示无法读取或解析输入文件。

### 图集校验项 { #18 }

默认使用严格模式。`--loose` 可将部分约定类问题降为告警。两种模式都会将无效的精灵表几何结构判为错误。

| 校验项 | 诊断码 | 严格模式 | 宽松模式 |
| --- | --- | --- | --- |
| 帧矩形越界 | `outOfBounds` | 错误 | 错误 |
| 帧矩形重叠 | `overlap` | 错误 | 错误 |
| 帧矩形偏离网格、图集尺寸与行列数不符，或行数不足 | `grid` | 错误 | 错误 |
| PNG 尺寸不是 atlas 的 `sheetWidth` / `sheetHeight` | `sheetSize` | 错误 | 错误 |
| 有效帧之外存在不透明像素，含末行留白 | `padding` | 错误 | 错误 |
| `frameWidth` 或 `frameHeight` 不能被 `scale` 整除 | `scale` | 错误 | 错误 |
| 帧下标不连续、标签下标不连续、区间越界、`loop` 不是 `forward` / `reverse` / `pingpong` / `once`、标签重名 | `frameIndex` / `tagIndex` / `tagRange` / `loop` / `tagName` | 错误 | 错误 |
| 缺少 `requireTags` 指定的标签 | `missingTags` | 错误 | 错误 |
| 原始画布边长不属于允许尺寸。按导出尺寸除以 `scale` 计算，默认允许 64、96、128。两边都要符合 | `size` | 错误 | 告警 |
| 方向标签或动作区间与布局不符。四向行序为下、左、右、上，支持罗盘方位别名。存在对角标签或八行时，检查八向布局 | `naming`、`missingTags` | 错误 | 告警 |
| 无动作标签，且未启用四向检查（默认标签策略） | `missingTags` | 错误 | 告警 |
| `image` 和 PNG 文件名不一致 | `image` | 错误 | 告警 |
| `delayMs` 不在 `1..=65535` | `delay` | 错误 | 告警 |
| 轴心在帧外，或各帧最底部不透明像素与轴心的距离不一致。轴心坐标以帧左上角为原点，允许位于远边界 | `pivot` | 两种情况均为错误 | 越界为错误，距离不一致为告警 |
| 一整格都透明 | `emptyFrame` | 告警 | 告警 |

### 配置项目规则 { #18_1 }

将以下内容保存为 `keeper-rules.json`，检查示例是否使用 64 像素画布并包含朝下站立、行走两个标签。同一项规则以命令行参数为准。

~~~json
{
  "format": "pixelforge-atlas-rules",
  "version": 1,
  "mode": "strict",
  "sizes": [64],
  "requireTags": ["idle_down", "walk_down"],
  "fourDirection": "auto",
  "tagPolicy": "required"
}
~~~

~~~bash
pixelforge validate-atlas --atlas keeper-sheet.json --rules keeper-rules.json
~~~

`sizes` 取空数组可跳过画布档位检查。可选的 `pivot` 用帧内像素坐标指定轴心。当图集与规则文件均未指定轴心时，`checks.pivot` 会跳过；导入引擎时请自行设置轴心。

## 导出 PNG 序列 { #png-png-sequence }

~~~bash
pixelforge export png --project a/keeper.pxf.json --out-dir keeper-frames
~~~

`pixelforge-000.png`、`pixelforge-001.png` 等文件按时间轴顺序生成。导入序列时，从源工程或图集中读取每帧时长。

## 导出动画预览 { #apng }

~~~bash
pixelforge export apng --project a/keeper.pxf.json --output keeper-walk.apng --tag walk_down
~~~

APNG 保留合成结果的 RGBA 像素和毫秒级时长。可通过标签选择片段，也可使用 `--from` 和 `--to` 并选填 `--loop`。循环模式有 `forward`、`reverse`、`pingpong` 和 `once`。往返播放在端点转向时不会重复停留。需要暂停或逐帧查看时，用 `pixelforge play` 打开源工程。

## 预览压平后的色彩变化 { #side-by-side }

~~~bash
pixelforge preview --project a/keeper.pxf.json --compare --scale 4
~~~

打开源文件旁的 `.compare.png`。左侧显示可见图层的 RGBA 合成，右侧显示压平时会保留的索引色。预览后仍可继续编辑源图层。此模式作用于完整合成图，使用时省略 `--layer` 和蒙版参数。[半透明图层的压平规则](known-limits.md#translucent-flatten-snaps-to-nearest-index)

## 导入 Godot 4 { #godot-4 }

1. 将 PNG 放入工程，将目标 CanvasItem 的 **Texture Filter** 设为 **Nearest**；使用整数倍像素显示时关闭纹理的 mipmaps。
2. 为 `AnimatedSprite2D` 创建 `SpriteFrames`。按图集行列数切分精灵表，从各个标签的 `from` 索引起选取其有效帧。
3. Godot 的帧时长以动画速度为基准。将动画设为 10 FPS、播放倍率设为 1 时，相对时长取 `delayMs / 100`，例如 140 ms 写为 1.4。再按动作片段设置播放顺序和循环方式。[Godot SpriteFrames 参考](https://docs.godotengine.org/en/stable/classes/class_spriteframes.html)

## 导入 Unity { #unity }

1. 设定 **Texture Type → Sprite (2D and UI)**、**Sprite Mode → Multiple**、**Mesh Type → Full Rect**、**Filter Mode → Point**、**Compression → None**。同组素材使用一致的 Pixels Per Unit。
2. 在 Sprite Editor 中按单元格尺寸切片，使用导出的 `frameWidth` 和 `frameHeight`。同组姿势采用相同的轴心，最后一个有效帧之后的补齐空格不加入动画。
3. 按图集顺序将精灵加入动画。帧时长相同时，采样率取 `1000 / delayMs`；时长不同时，按累计时间放置关键帧，并让末帧保持完整时长。[Unity 精灵导入设置](https://docs.unity3d.com/6000.0/Documentation/Manual/texture-type-sprite.html)

## 在 Tiled 中打开地图 { #tiled }

分别用 `export tileset` 和 `export tilemap` 导出图块集与地图，两条命令使用相同的 `--cols` 和 `--scale`。TMJ 按自身目录查找 `image` 指向的文件，请将对应的图块集 PNG 和 TMJ 一起移动。可用[潮汐温室](/pixelforge-demos/zh/gallery/#tidal-glasshouse)的五层地图直接练习。

导出支持一个正交图块集和 16 种四邻接自动拼接蒙版。空格使用 GID 0，图块 ID 0 对应 GID 1，横向和纵向翻转位分别为 `0x80000000` 和 `0x40000000`。当前地图格式仅支持这两种翻转和四邻接拼接。[用 CLI 创建图块地图](agent-skill.md#tileset-autotile-tilemap)
