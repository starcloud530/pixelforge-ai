# 使用 Agent 创作像素素材 { #agent-skill-agent-skill }

能运行终端命令的 Agent 可以编辑本地 PixelForge 工程、预览改动并导出成品。开始前告诉它工程路径、画布尺寸和交付要求，例如一套有四个朝向的行走动画。

## 接入本地 CLI { #install-and-run }

[安装 CLI](install.md) 后，在 Agent 使用的工作目录和终端环境中运行以下命令。安装包内附与该版本匹配的创作技能。

~~~bash
pixelforge --version
pixelforge skills list
pixelforge skills read pixelforge
~~~

下载并解压[港口守灯人工程](/pixelforge-demos/zh/gallery/#port-keeper)，在解压目录中向 Agent 提出以下要求。

> 打开 `b/keeper-storm-cape.pxf.json`，读取 PixelForge 核心技能和分层技能。列出图层和色板，只修改 `storm-cape` 图层，保留每帧时长，交付修改前后的预览图和可编辑工程。

## 按任务选择技能 { #skills }

先读核心技能，再按任务补充对应的技能。

| 技能 | 适用任务 |
| --- | --- |
| `pixelforge` | 读取工程、管理索引色、执行命令、处理错误和导出 |
| `pixelforge-draw` | 选择画布，从剪影、色块逐步画到细节 |
| `pixelforge-sprite-4dir` | 安排四向站姿、行走帧和动作标签 |
| `pixelforge-layers` | 分开修改身体、服装、装备和绑定的精修层 |
| `pixelforge-critique` | 评审剪影、明暗、落脚和帧时长 |
| `pixel-art-craft` | 设计关键姿势、重心、机位及附属物运动 |

~~~bash
pixelforge skills read pixelforge-layers
pixelforge skills list pixel-art-craft
pixelforge skills read pixel-art-craft references/action-motion.md
~~~

## 读取命令结果 { #output }

从标准输出读取 JSON。预览命令另存一张 PNG，并在标准错误流打印字符画。用彩色预览检查混色与透明度。脚本先核对退出码，再读取 `result`。命令失败时查看 `error.code` 和 `error.message`。

## 索引色、图层与撤销 { #invariants }

索引 0 表示透明。绘制时使用色板中已有的不透明索引。色板锁定后仍可用已有颜色绘制。修改某个色板项会影响所有使用该项的帧。绘制默认作用于当前层，可用 `--layer` 指定其他层。默认预览和导出会合成可见图层。

如需跨命令撤销，在连续的每条编辑命令上指定同一份 `--checkpoint FILE`。批量调用中，首个失败请求之前的成功改动会保存。重试前重新读取工程状态。

## 用终端编辑一个工程 { #command-reference }

在新目录中依次运行下面的角色示例。画布为 64 × 64，共有两帧，坐标从 0 开始。

### 创建画布 { #blank-project }

~~~bash
pixelforge new --width 64 --height 64 --output hero.json
pixelforge scene --project hero.json
~~~

### 绘制第一帧的色块 { #paint-fill-erase-copy-move }

在第 0 层画出外套，将左靴复制到另一侧，再新建图层画头盔。后续可以单独调整头盔的颜色。

~~~bash
pixelforge fill-rect --project hero.json --layer 0 --x 22 --y 24 --width 20 --height 21 --color-index 3
pixelforge fill-rect --project hero.json --layer 0 --x 22 --y 45 --width 6 --height 6 --color-index 1
pixelforge copy-rect --project hero.json --layer 0 --src-x 22 --src-y 45 --width 6 --height 6 --dst-x 36 --dst-y 45
pixelforge layer add --project hero.json --name gear
pixelforge fill-rect --project hero.json --layer 1 --x 23 --y 14 --width 18 --height 10 --color-index 4
~~~

### 只修改头盔 { #edit-only-the-helmet }

复制姿势后切换到副本，再给头盔换色。复制命令会保留原先选中的帧，因此需要显式选中第 1 帧。选区在帧之间共享，并随工程保存。

~~~bash
pixelforge frame duplicate --project hero.json --index 0
pixelforge frame select --project hero.json --index 1
pixelforge selection set --project hero.json --x 23 --y 14 --width 18 --height 10
pixelforge replace-index --project hero.json --layer 1 --from-index 4 --to-index 6
~~~

### 对比选区内的改动 { #diff-diff-only-the-helmet }

~~~bash
pixelforge diff --project hero.json --frame 0 --with 1 --layer 1 --selection --scale 4
pixelforge selection clear --project hero.json
~~~

对比结果会报告装备层内有 180 个格子发生变化。打开输出 PNG，可并排查看两帧及洋葱皮叠图。

### 换色时保护靴子 { #mask-fill-subtract-the-feet-then-fill }

~~~bash
pixelforge selection set --project hero.json --x 22 --y 24 --width 20 --height 27
pixelforge mask subtract --project hero.json --x 22 --y 45 --width 20 --height 6
pixelforge replace-index --project hero.json --layer 0 --from-index 3 --to-index 5
pixelforge selection clear --project hero.json
~~~

### 撤销一次编辑 { #a-stroke-as-one-undo }

~~~bash
pixelforge paint --project hero.json --checkpoint hero.ckpt.json --layer 1 --x 32 --y 16 --color-index 2
pixelforge undo --project hero.json --checkpoint hero.ckpt.json
~~~

多个工具请求组成一笔时，在同一次 `call` 里用 `beginGesture` 和 `endGesture` 包住绘制请求。参见[工具接口](tool-api.md#begingesture-endgesture)。

### 设置时长并导出 { #frames-and-export }

~~~bash
pixelforge frame delay --project hero.json --index 1 --delay-ms 160
pixelforge tag add --project hero.json --name color_cycle --from 0 --to 1 --loop forward
pixelforge export sheet --project hero.json --cols 2 --output hero-sheet.png --atlas hero-sheet.json
pixelforge validate-atlas --atlas hero-sheet.json
pixelforge export apng --project hero.json --output hero.apng --tag color_cycle
~~~

将精灵表 PNG 和图集 JSON 一起导入引擎。打开 `hero.apng`，按实际时长检查两个姿势。参见[引擎导入设置](export.md)。

## 常用创作操作 { #workflow-skills }

### 使用部件与锚点 { #parts-and-anchors }

部件在画布上定义一个带名称的原点，也可以绑定到图层。锚点标记铰链、握持点等局部位置。可按帧为旋转、平移或翻转设置曲线，再用 `bake` 写入各帧姿势。

~~~bash
pixelforge part add --project hero.json --name helmet --layer 1 --x 32 --y 24
pixelforge part anchor set --project hero.json --part helmet --name hinge --x 0 --y 0
pixelforge part list --project hero.json
~~~

### 管理色板 { #palette }

使用 `palette add` 获取新颜色的索引。`palette import` 接受 GPL 或十六进制颜色列表，支持 `--mode merge` 和 `--mode replace`。整份工程压色使用 `palette remap`，局部选区换色使用 `replace-index`。同尺寸图片可用 `palette apply-png` 映射到现有色板。

### 转换参考 PNG { #png-convert-a-png }

以下命令以上一步的精灵表为输入，将其采样到新的 64 × 32 网格，并将不透明颜色限制在八种以内。

~~~bash
pixelforge engine list
pixelforge convert --file hero-sheet.png --width 64 --height 32 --engine center-sample --palette-strategy limit --max-colors 8 --output study.json
~~~

### 读取工程并调用工具 { #inspect-and-raw-calls }

~~~bash
pixelforge scene --project hero.json
pixelforge inspect --project hero.json
pixelforge tools
pixelforge call --project hero.json --request '{"tool":"scene"}'
~~~

`scene` 返回尺寸、色板、图层、帧时长和用于防止覆盖旧版本的修订号。`inspect` 还返回像素索引。请求和返回格式见[工具接口](tool-api.md)。

### 拼装图块地图 { #tileset-autotile-tilemap }

每帧存放一个图块，宽、高均可取 16 或 32 像素，尺寸须与工程画布一致，帧下标就是图块 ID。创建地图前先声明图块尺寸。下面的命令会创建一个独立的单图块工程。

~~~bash
pixelforge new --width 16 --height 16 --output tiles.json
pixelforge tileset set --project tiles.json --tile-width 16 --tile-height 16
pixelforge fill-rect --project tiles.json --x 0 --y 0 --width 16 --height 16 --color-index 3
pixelforge tilemap new --width 8 --height 6 --tile-width 16 --tile-height 16 --tileset tiles.json --output map.json
pixelforge tilemap fill --map map.json --name ground --x 0 --y 0 --width 8 --height 6 --tile 0
pixelforge validate-tiles --project tiles.json --map map.json
pixelforge export tileset --project tiles.json --output tiles.png --meta tileset.meta.json --image tiles.png
pixelforge export tilemap --map map.json --tileset tiles.json --output map.png --tmj base.tmj --image tiles.png
~~~

### 登记关联素材 { #project-manifest }

多件素材共用色板、画布尺寸或动作约定时，可在[项目清单](project-manifest.md)登记路径和依赖关系。用 `character matrix` 查看已有的动作与朝向组合。

### 绘制剪影 { #pixelforge-draw }

先用一种不透明色勾出剪影，按游戏中的实际尺寸检查轮廓。先补服装、皮肤或机械部件的大色块，再画细节。每一阶段用 `preview --scale 4` 对照。

### 复用姿势 { #shortest-edit-path }

复制最接近的姿势，改动需要运动的部位。目标帧已存在时可用 `copy-rect --from-frame … --to-frame …`。用 `diff` 确认静止细节没有移位。

### 制作四向行走 { #pixelforge-sprite-4dir }

按下、左、右、上排列四行，每行第一格是站姿，其余是行走帧。导出时 `--cols` 取每行帧数。按游戏中的实际大小播放，检查落脚时是否滑动。[四向角色实例](showcase.md#port-keeper)

### 单独编辑装备 { #pixelforge-layers }

服装与装备放在身体上方的独立图层。用 `preview --layer` 检查单层，再看合成预览的遮挡关系。需要让精修随部件烘焙移动时，将精修层绑定到对应部件。

### 评审动画 { #pixelforge-critique }

要求评审结果指出具体帧或动作标签，说明哪个部位的运动有问题，并给出一项修改建议。按原始速度播放，检查重心、落脚和附属物的跟随运动。

### 检查动作节奏 { #pixel-art-craft }

阅读创作技能内的 `references/action-motion.md`，设计预备、接触、回收及跟随动作。可打开[哪吒动作工程](showcase.md#nezha)逐帧查看这些阶段。

## 处理编辑冲突 { #known-limitations }

如果另一进程已修改工程，带修订号的编辑会返回 `RevisionConflict`。重新读取 `scene`，根据最新修订号和画面决定下一步。检查点不匹配、写入冲突和文件系统限制见[编辑与导出边界](known-limits.md)。
