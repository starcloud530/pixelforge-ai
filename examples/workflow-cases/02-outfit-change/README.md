# 案例 2：同角色换装

还是案例 1 的港湾信使。先画青绿外套，锁调色板，确认不能再追加颜色，再把装备层换成靛色斗篷。身体层的脸、头发、手、裤子和靴子不重画。

*Same courier as case 1. The palette is locked before the gear layer is recolored with indices that were already reserved. The body layer is compared before and after. Not an image model and not unsupervised L4.*

可复现的是 `build.py`。它不读取案例 1 的文件，而是再画一遍同一套外套，然后换装，所以单独跑这一条也能得到斗篷工程。案例 1 的工程是换装前的外套。

## 记录

| 项 | 值 |
| --- | --- |
| 提示词 / 指令摘要 | 在同一个信使上换装。先锁调色板。身体层（脸、头发、皮肤、手、裤子、靴子）不许改。斗篷只能用色板里已经有的靛色索引，不能 `palette add`。灯笼从装备层去掉，并在身体层空着的格子上加帽檐。换完后 `paletteHash` 和身体层像素必须与换装前一致。再导出四向走表并 strict 校验。 |
| 耗时 | 与案例 1 同一次 Agent 会话。没有另计画师工时。三条 `build.py` 连着重跑约 9 秒（debug 版 `pixelforge`）。 |
| 外部模型成本 | $0。没有图像生成接口。换装是 `replaceIndex` 和几块 `fillRect`，不是新的 RGB。 |
| 人工修订次数 | 0。没有画师在脚本之外改像素。 |
| 失败点 | 见下。机器可读的核对在 `identity.json`。 |

`identity.json` 是脚本写的：`paletteHash` 前后都是 `eee61aba3bbf619c`，`lockRefusal.code` 是 `PaletteLocked`，`bodyLayerUnchanged` 为 true，装备层改了 5944 格。`validate.py` 会把这份记录和工程里的 `scene` 对上。

## 复现

```bash
python3 examples/workflow-cases/02-outfit-change/build.py
pixelforge validate-atlas \
  --atlas examples/workflow-cases/02-outfit-change/courier-cloak-sheet.json \
  --rules examples/workflow-cases/02-outfit-change/courier-cloak-rules.json \
  --strict
pixelforge scene --project examples/workflow-cases/02-outfit-change/courier-cloak.json
```

`scene` 的 `paletteLocked` 应为 true，`paletteHash` 应与案例 1、案例 3 相同。

换装步骤，脚本里的顺序就是这些：

1. 画完外套行走（和案例 1 同一个函数）。
2. 记下 `paletteHash` 和每一帧身体层像素。
3. `setPaletteLocked`。
4. `palette add` 必须失败，退出码 1，`error.code` 为 `PaletteLocked`。失败的调用不改文档，哈希仍是原来的。
5. 在装备层把外套索引换成斗篷索引（7 和 8，建色板时就留好了），把灯笼矩形填成斗篷色，再在身体层透明的格子上填帽檐。
6. 再比身体层和哈希。不一致就退出，不导出。

斗篷没有新的 RGB。锁着的色板不能 `palette set`、不能追加、不能 `remap`。已有索引仍然可以 `fillRect` 和 `replaceIndex`。

## 失败点

1. **锁上之后不能为斗篷现加一种紫。** 第一次如果没把靛色留在色板里，换装会停在 `PaletteLocked`。所以索引 7、8 在画外套时就占着，外套本身不用它们。
2. **帽檐不能压住身体层的脸。** 脚本只在该帧身体层为 0 的矩形上填帽檐。压到脸上的矩形会被丢掉并写进 `hoodRectsSkipped`。当前这份是空的。头发边缘被帽檐盖住时，身体层那一格仍是头发，合成图才显示斗篷。
3. **下摆没有变长。** 裤子从 y=40 开始。把斗篷画到脚底会改着地行，或者盖住身体层。换装改的是颜色、灯笼和帽檐，不是身长。
4. **走路姿势留在原来的装备层像素上。** `replaceIndex` 不挪袖子。如果擦掉整层再按待机重画，走路帧的袖子会回到闲置位置，手还在摆。那是失败过的一条路，脚本没有走。
5. 案例 1 里的着地、`hflip` 高光、以及「校验不是审美」在这里同样成立。斗篷表过 strict，不表示这套衣服已经能进游戏。

## 文件

| 路径 | 内容 |
| --- | --- |
| `courier-cloak.json` | 换装之后的 16 帧。调色板已锁。层仍是 `body` / `gear` |
| `courier-cloak-sheet.png` | 256×256，`--cols 4` |
| `courier-cloak-sheet.json` | 与案例 1 同一组 `idle_*` / `walk_*` 标签 |
| `courier-cloak-rules.json` | strict，`fourDirection: on`，原点 `(32, 57)` |
| `identity.json` | 哈希、锁拒绝、身体层未改、装备层改动格数 |
| `build.py` | 重画外套、换装、导出 |
