# 认识 PixelForge 工程 { #vision-positioning }

一份 PixelForge 工程保存精灵或图块集的可编辑原稿。打开工程可以查看或修改像素，再按游戏需要导出相应文件。

## 画布与调色板 { #vision }

画布上的每一格保存一个色板索引，索引 0 表示透明。修改一个色板条目，所有帧中使用该索引的像素都会变色。需要保持用色一致时，可以先锁定色板再绘制。

## 图层与动画帧 { #mission }

把后续可能单独修改的元素分层，例如把角色和披风放到不同图层。每帧都有以毫秒为单位的时长。给一段连续的帧设置 `walk_down` 等动作标签，就能在播放器里单独查看这段动作。

## 选择一个工程 { #what-we-build }

- [港口守灯人](/pixelforge-demos/zh/gallery/#port-keeper)展示同一角色的换装和四向行走。
- [潮汐温室](/pixelforge-demos/zh/gallery/#tidal-glasshouse)展示独立图块怎样组成分层地图。
- [潮汐科考道具](/pixelforge-demos/zh/gallery/#tide-survey-icons)可以切换两套色板，观察八枚背包图标的变化。

## 选择交付格式 { #what-we-are-not }

| 文件 | 用途 |
| --- | --- |
| 工程 JSON | 保留像素、色板、图层和时长，供后续修改 |
| APNG | 在浏览器中审阅动画和节奏 |
| PNG 精灵表与图集 JSON | 将帧、动作标签和时长导入游戏 |
| Tiled TMJ 与图块集图片 | 加载正交图块地图；游戏对象由引擎放置 |

## 开始第一次编辑 { #status }

可以[创建空白工程](install.md)，也可以[打开一个角色示例并重新导出](showcase.md)。要通过编程 Agent 修改素材，请从[使用 Agent 创作](agent-skill.md)开始。
