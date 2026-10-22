---
name: pixelforge-draw
description: >-
  Draw a PixelForge character, standing portrait (立绘), bust, or silhouette
  from scratch. Use when the user says 从零画, 画角色, 立绘, 剪影, 全身, or
  胸像, or when choosing a canvas size, blocking a single-color silhouette,
  laying color flats, or painting eyes, teeth, and cloth damage. Follow the
  staged loop silhouette then flats then details, and run preview --scale plus
  the ASCII grid after every stage. Prefer fillRect and eraseRect; do not stamp
  one pixel at a time for a flat area. Follow the pixelforge skill's
  index-color invariants first. A four-direction walk sheet is
  pixelforge-sprite-4dir. Separate body and gear layers are pixelforge-layers.
  A review that must not change the file is pixelforge-critique.
---

# 从零画角色

先读 `pixelforge`。格子仍是调色板索引，颜色只通过 CLI 进内核。本 skill 规定观改顺序和画布尺寸。它补的是流程，不是专业像素画师的审美：剪影读得清、色阶分开，图仍然可能不好看。不要把阶段走完说成成稿。

*Follow the pixelforge invariants. This skill is a staging loop, not a substitute for a pixel artist's eye. Finishing the stages is not a finished picture.*

## When to use

从空工程或一张还没成形的帧画角色、站立立绘、胸像、剪影。变身或第二套剪影（例如人变成丧尸）也走这里：高画布、复制一帧、再选中新帧。

四向走加待机的整张表走 `pixelforge-sprite-4dir`。第一帧的身体仍然按下面的三阶段来。装备和身体要分开改时，同时读 `pixelforge-layers`，画在对应的层上。

## Canvas size

在 `pixelforge new` 里定尺寸。全身挤在精灵槽里会变成示意图。画小了用 `resize`：每一帧的索引导进更大的画布，默认居中，或成对的 `--offset-x` / `--offset-y`。放大只搬已有索引，不会长出细节。

| 题材 | 尺寸（宽 × 高） |
| --- | --- |
| 俯视小怪、图标、地块 | 16–32 |
| 全身游戏精灵 | 大约 32–64 |
| 站立立绘 / 变身故事 | 至少 64×96，优先 **64×128** 或 **96×128** |
| 胸像、脸部特写 | 至少 64×64 |

**站立立绘做成 48×64 会显得示意图化**：头只有几格，姿势读不出来，下一帧也放不下破损、衣服或第二套剪影。站立立绘不要从 16×16 或 32×32 起。俯视小体型用 16–32，那一行不是立绘。四向游戏精灵用「大约 32–64」，不要把 64×128 的立绘尺寸套到一张多帧走表上，除非用户要的就是高画布循环。

*Pick the row that matches the deliverable. A standing portrait at 48×64 looks schematic. A four-direction game sprite stays near 32–64. `resize` does not invent detail.*

## Three stages

每一阶段结束都先看再改。不要蒙着把细节画完。

每一阶段都跑 `preview` 子命令，并同时看两样东西：

1. ASCII（stderr 或 `result.rows`）：破洞、外轮廓、头和身体各占多少行。
2. `result.path` 的彩色 PNG（`--scale 4`，最近邻在内核里完成）。

会改像素的命令上的 `--preview` 只有 ASCII，不写彩色 PNG。阶段检查用 `preview` 子命令。字形对照在 `pixelforge`：`.` 是透明。

```bash
pixelforge preview --project hero.json --scale 4
pixelforge preview --project hero.json --frame 1 --scale 4
```

### 1. 剪影（单色）

整块身体只用一个已有索引。用 `fill-rect` 铺头、躯干、腿，用 `erase-rect` 切出外轮廓。这一阶段不加第二种颜色。

禁止为一块平涂逐格 `paint` 或 `erase`。一块纯色是一次 `fillRect` 或 `eraseRect`。一次 `call` 里可以放多个矩形，并用 `beginGesture` / `endGesture` 合成一步撤销。

看 ASCII：身体内部不该有无意的 `.`。外轮廓要能单独读出是个人、是侧是正。读不出来就继续改剪影，不要上色。

### 2. 色块

皮肤、衣服、金属分成不同索引的大矩形。相邻色阶在彩色 PNG 里要分开。眼睛、牙、破布边缘还不要画。新颜色先 `palette add`，再画返回的索引。

### 3. 细节

只有不是矩形的小东西才用 `paint`：眼睛、牙、布的裂口、一小块高光。手臂或掉落的道具用 `move-rect` 或 `copy-rect`，然后再 `preview`。

*Silhouette is one index, cut with rectangles. Flats separate materials. `paint` is for details. After each stage, open `result.path` and read the ASCII grid.*

## Read the picture before you praise it

打开彩色 PNG 和 ASCII 时核对这三件事，再决定进不进下一阶段：

- **破洞**：身体内部的 `.` 是不是故意留的缝（手臂和躯干之间可以是缝）。
- **头身**：头占几行，身体占几行。全身的头只有几格，就是尺寸表里的示意图，不是「小而精」。
- **体积断点**：腿或手臂从粗到细再断成空格。断点用 `fill-rect` / `erase-rect` 接上，不要用散点糊住。

完成度只写阶段名：剪影、色块、或细节。不要写成高完成度、成稿、或「已经很好」。阶段走完不等于好看。要挑错时读 `pixelforge-critique`，那是只读清单。

*Name the stage. Do not claim a high level of finish. Check holes, head-to-body rows, and breaks in volume.*

## Pose changes

第二套姿势（变身、受击、另一套剪影）用 `duplicateFrame`，然后 `frame select` / `setActiveFrame` 到返回的下标。复制不会切到新帧。`preview --frame N` 对比另一帧，不选中它。两帧要对齐时用 `diff --frame A --with B --scale 4`。

```bash
pixelforge new --width 64 --height 128 --output hero.json
pixelforge call --project hero.json \
  --request '{"tool":"beginGesture"}' \
  --request '{"tool":"fillRect","params":{"x":24,"y":8,"width":16,"height":16,"colorIndex":1}}' \
  --request '{"tool":"fillRect","params":{"x":20,"y":24,"width":24,"height":40,"colorIndex":1}}' \
  --request '{"tool":"fillRect","params":{"x":20,"y":64,"width":10,"height":48,"colorIndex":1}}' \
  --request '{"tool":"fillRect","params":{"x":34,"y":64,"width":10,"height":48,"colorIndex":1}}' \
  --request '{"tool":"endGesture"}'
pixelforge preview --project hero.json --scale 4
```

上面只是单色大块的形状，不是一张能用的人。接着 `erase-rect` 收剪影，再 `preview`。衣服和皮肤仍是矩形。然后才是 `paint`。

*A standing portrait starts at 64×128 or 96×128. `duplicateFrame` does not select the copy.*
