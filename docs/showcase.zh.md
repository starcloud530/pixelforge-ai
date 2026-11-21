# 验证示例工程 { #_1 }

打开可编辑的工程，重新导出图集并核对帧数据。用港口守灯人检查四向角色，用哪吒观察单帧时长对整段动作的影响。[安装 CLI](install.md)

## 港口守灯人 · 验证四向角色 { #port-keeper }

在作品集中下载工程并解压，在包含 `a` 和 `b` 两个子目录的文件夹中执行以下命令。角色画布为 64 × 64，每个朝向有五帧。暴风装束在单独图层上增加了披风。[在作品集中查看港口守灯人](/pixelforge-demos/zh/gallery/#port-keeper)

### 播放源工程 { #_2 }

~~~bash
pixelforge play --project a/keeper.pxf.json
~~~

选择 `walk_down`，逐帧查看四张行走姿势，对应时长为 140、110、140、110 毫秒。与 `idle_down` 的站姿对照，观察脚底的落点。查看完毕后回到终端按 Ctrl+C 停止播放器，再运行后续命令。

### 校验图集并重新导出 { #_3 }

~~~bash
pixelforge validate-atlas --atlas a/keeper-sheet.atlas.json
pixelforge export sheet --project a/keeper.pxf.json --cols 5 --output keeper-check.png --atlas keeper-check.atlas.json
pixelforge validate-atlas --atlas keeper-check.atlas.json
~~~

第一次检查随包附带的文件，第二次检查刚导出的文件。核对 20 个帧矩形、五列排布、四个朝向和八个站立或行走标签。在 `keeper-check.atlas.json` 中，`image` 应指向 `keeper-check.png`。披风版可单独运行 `pixelforge validate-atlas --atlas b/keeper-storm-sheet.atlas.json` 检查。

### 将行走动画导入引擎 { #_4 }

使用 `keeper-check.png` 作为纹理，并按配套 JSON 中的帧矩形、动作区间、时长及已指定的轴心设置动画。在游戏缩放比例和背景下检查轮廓与落脚。[Godot 4](export.md#godot-4) · [Unity](export.md#unity)

## 哪吒 · 检查并调整动作时长 { #nezha }

下载并解压工程，在包含 `nezha-attack.pxf.json` 的目录中执行以下命令。原稿是在 128 × 128 画布上绘制的 12 帧动画，一次完整动作持续 1,070 毫秒。[在作品集中查看哪吒](/pixelforge-demos/zh/gallery/#nezha)

### 观察动作 { #_5 }

~~~bash
pixelforge play --project nezha-attack.pxf.json
~~~

通过 `ready`、`charge`、`thrust` 和 `recover` 跳转到相应阶段。暂停在蓄力末尾，逐帧观察前脚、长枪和红绫，最后用 Ctrl+C 停止播放器。

### 导出对照版本 { #_6 }

~~~bash
pixelforge export sheet --project nezha-attack.pxf.json --cols 6 --output nezha-check.png --atlas nezha-check.atlas.json
pixelforge export apng --project nezha-attack.pxf.json --output nezha-check.apng
pixelforge validate-atlas --atlas nezha-check.atlas.json
~~~

六列精灵表共两行，配套图集记录帧矩形和时间。打开 `nezha-check.apng`，按原始速度查看调整前的动画。

### 延长蓄力的最后一帧 { #_7 }

蓄力的最后一个姿势位于索引 3，也就是第 4 帧。将它从 60 毫秒延长到 120 毫秒，导出另一份预览，再用同一检查点撤销此次修改。

~~~bash
pixelforge frame delay --project nezha-attack.pxf.json --index 3 --delay-ms 120 --checkpoint timing.ckpt.json
pixelforge export apng --project nezha-attack.pxf.json --output nezha-long-charge.apng
pixelforge undo --project nezha-attack.pxf.json --checkpoint timing.ckpt.json
~~~

并排播放两份 APNG。新预览的蓄力增加了 60 毫秒，出枪阶段的时长和角色像素保持原样。撤销后仍可比较两个已导出的文件。两个示例工程均采用 CC BY 4.0 许可。[素材署名](licensing.md)
