---
name: pixelforge-critique
description: >-
  Read-only critique of a PixelForge figure or sprite. Use when the user asks
  to review, critique, or check an existing frame (检查, 评审, 看起来怎么样,
  剪影, 头身比, 色阶), including whether a preview looks finished. When a
  layer is translucent, use `preview --compare`: ASCII covering indices are
  not the blend. When an
  edit log or several frames are in scope, also flag mergeable fillRects and
  poses redrawn instead of duplicated. Report defects with evidence from
  preview, diff, and that log. Do not paint, fill, erase, or change the file.
  Do not answer with praise alone or with 看起来不错. Follow the pixelforge
  skill for preview, diff, and the shortest edit path. Drawing or fixing
  pixels is pixelforge-draw or pixelforge-sprite-4dir, not this skill. This
  checklist does not replace a pixel artist's judgment.
---

# 只读检查

先读 `pixelforge` 里 `preview` 和 `diff` 的语义。本 skill 不改工程。不调用 `paint`、`erase`、`fill-rect`、`erase-rect`、`copy-rect`、`move-rect`、`hflip`、`resize`、`palette`、`layer flatten`，也不手改 JSON。

清单补的是观察步骤。通过清单不等于好看，也不能代替专业画师。不要把「各项都写了」说成成稿。

*Read-only. Use preview and diff. Passing the list is not a statement that the picture looks good.*

## When to use

用户要检查、评审、或问一张已有的图好不好、完成了没有。也用于核对编辑路径：相邻同色 `fillRect` 能否合并、同姿帧是否整帧重画。用户要改像素时改用 `pixelforge-draw` 或 `pixelforge-sprite-4dir`，不要在这次只读里顺手画。

## What to run

```bash
pixelforge inspect --project hero.json
pixelforge preview --project hero.json --scale 4
pixelforge preview --project hero.json --frame N --scale 4
pixelforge diff --project hero.json --frame A --with B --scale 4
```

有多层时再 `pixelforge layer list --project hero.json`，并分别 `preview --layer N --scale 4` 和一次不带 `--layer` 的合成。`preview` 和 `diff` 不改文件。彩色图在 `result.path`。ASCII 在 stderr 和 `result.rows`。`diff` 的 stderr：`.` 相同，`!` 两边都有墨且不同，其它字形是唯一有墨的一边。

打不开彩色 PNG 时，色阶那一条写「未核」，不要写成通过。

## 如何读状态

先 `pixelforge scene --project hero.json`。它不改文件。用 `result.width` / `height`、`activeFrame`、`activeLayer`、`layers`、`paletteHash`、`frameCount`、`frames[].delayMs` 确认你在看哪一帧、哪一层、哪套颜色。`visible` 是布尔，`opacity` 是 `0..=255`。隐藏层和 opacity 0 不在合成里。半透明的 ASCII 仍是盖住的索引，颜色以彩色 PNG 为准。层 `opacity` 在 1 到 254 时再跑 `preview --compare --scale 4`：左是未量化 RGBA，右是压平会写下的索引色，工程文件不动。不要用 ASCII 判断光束或叠色。`scene` 不带像素；格子在 `preview` 和 `diff` 里。

`preview` 的 `result.bounds` 是有墨格子的外接矩形，`result.ink` 是非 0 格数，`result.indices` 是每个索引用了几次，`result.pixels` 是这一张视图的索引。头身和脚的行号可以从 `bounds` 和 `pixels` 数，不必只靠 ASCII。字形 `*` 把 62 及以上折成同一个字符。

`diff` 的 `result.changed` 是索引不同的格数，`result.bounds` 是这些格子的外接矩形，`result.kinds.differ` 是两边都有墨且不同，`result.changes` 里的 `left` / `right` 才是索引。ASCII 的 `!` 不包含索引。脚滑要看地面那一行的 `changes`，不要只看有没有 `!`。

本 skill 不调用 `--dirty`。那个旗标写在 `paint` 一类命令上，会改文件。色阶仍要打开彩色 PNG；哈希相同只说明调色板没变。

*Read `scene` first. It does not change the file. `preview` adds `ink`, `bounds`, `indices`, and `pixels`. `diff` adds `bounds`, `kinds`, and `changes` with the real indices. Do not pass `--dirty` from this skill. A matching `paletteHash` only says the palette did not change.*

## 最少编辑路径

画图时怎么把路径缩短，规范在 `pixelforge` 的「最少编辑路径」。这里只读，用来挑已经发生的长路径。不调用 `duplicateFrame`、`fillRect` 或任何会改文件的工具。2026-10-25 的士兵射击动画是 1,191 次工具操作，其中 1,133 次是 `fillRect`：同姿帧整帧重画、扫描行没有合成更高的矩形、空白帧上重填整个人，并且一次批量 `call` 被当成了一步。

有对话里的工具记录时，逐条数绘制请求（`fillRect`、`strokeRect`、`line`、`ellipse`、`polygon`、`fill`、`eraseRect`、`paint`、`erase`、`copyRect`、`moveRect`）。一条斜线被拆成许多 `paint`，或一整块同色连通区被逐格改色，记为缺陷：短路径是一次 `line` 或一次 `fill`。平涂仍然看能不能合成更高的 `fillRect`。一次 `pixelforge call` 里的每个 `--request` 各算一次。`beginGesture` / `endGesture` 只说明这些请求会合成一步撤销，不把次数收成 1。没有记录时，可合并的 `fillRect` 写「未核」：最终像素看不出当初拆成几笔。

同姿帧用 `diff`。`changed` 为 0 只说明两帧像素相同，看不出是 `duplicateFrame` 还是重画；没有记录就写未核。记录里是 `addFrame` 再对整个人 `fillRect`，而另一帧已经有这个姿势，记为缺陷。短路径是 `duplicateFrame` 后只改差，或在下标已经占死时 `copyRect --from-frame --to-frame` 再改差。

可合并的记录长这样。之前三笔，同一索引、同一 `x`、同一 `width`，`y` 为 20、21、22，高度都是 1：

```json
{"tool":"fillRect","params":{"x":8,"y":20,"width":12,"height":1,"colorIndex":4}}
{"tool":"fillRect","params":{"x":8,"y":21,"width":12,"height":1,"colorIndex":4}}
{"tool":"fillRect","params":{"x":8,"y":22,"width":12,"height":1,"colorIndex":4}}
```

合成之后是一笔：`x=8`，`y=20`，`width=12`，`height=3`，`colorIndex=4`。左右相接、同 `y`、同 `height`、同色，合成更宽的一笔，同一条规则。L 形或中间有缝不算可合并。缺陷里写出能合成的那一块矩形，不要只写「可以更短」。

*Read-only. The normative before-and-after is in the pixelforge skill. Flag a redraw of a pose that already exists, and flag abutting same-color `fillRect`s that form one taller or wider rect. Each `--request` in one `call` counts. Without a log, the fillRect merge check is 未核.*

## Checklist

每一条都要写进输出。通过的条也写证据（数过的行、帧号、你看到的字形）。不要合并成一句总评。

1. **剪影**：只看 ASCII 的外轮廓，能不能认出朝向和是不是一个人。身体内部的 `.` 是破洞还是故意的缝（手臂和躯干之间可以是缝）。
2. **头身**：头占几行，身体占几行。站立立绘若是 48×64 或更小，写成示意图，不要写成小而精。16×16 和 32×32 不是站立全身。尺寸对照在 `pixelforge-draw`：常见四向游戏精灵大约 32–64；更高细节的俯视或全身可以是 96×96 或 128×128，需要拉高时是 96×128 或 64×128。用行数说明读不读得出。96 和 128 的整张 ASCII 很难一眼读完，外轮廓和头身改看 `preview --scale` 的彩色 PNG，再抽 `result.rows` 里头、身体、脚那几行。格子多不等于画完。立绘的 64×128 标准不要套到一张 32–64 的走表上。
3. **侧向深度**：侧向要有重叠，或远侧一块更深的索引。一张正面被镜像、没有重叠的「侧面」记为缺陷。
4. **腿体积连续**：大腿到小腿到脚在 ASCII 里是一块相连的质量。中间出现 `.`，或收到只剩一格又突然变粗，记为断点。
5. **脚接触**：同一朝向的 idle 和 walk，最低的非透明格是否在同一地面行。`diff` 里地面行出现横移的 `!` 或单独字形，记为脚滑。俯视小体型看身体最下面的接触格，滑一格也算。
6. **色阶**：皮肤、衣服、金属是不是不同索引，彩色 PNG 里能不能分开。相邻区域同一索引，或颜色接近、边界分不清，记为糊。没看到 PNG 就写未核。`inspect` / `scene` 的 `indexPolicy` 默认是 `reject`；`clamp` 表示越界索引会被写成最后一项，色阶检查时要知道画上去的索引可能不是请求的那个。若对话里已经有 `palette remap` 或 `palette apply-png` 的结果，用 `misses`（没精确命中的颜色）、`merges`（并进了哪项近似色）和 `pixelsChanged`（改了多少格）当作证据。本 skill 不调用这两条，它们会改文件。没有这份报告时，仍只看彩色 PNG。
7. **可合并的 fillRect**：只在有本次编辑记录时核。同一 `colorIndex`、同一 `x`、同一 `width`，下一块的 `y` 等于上一块的 `y + height`，记为缺陷，并写出合成后的那一块（`x`、`y`、`width`、合计 `height`、索引）。同 `y`、同 `height`、左右相接、同色，写出合成后的更宽一块。一次 `call` 里的多笔仍逐笔算。没有记录就写未核，不要写成通过。
8. **重复帧未复制**：多帧时对看起来同姿的两帧跑 `diff`。记录里是 `addFrame`（或事先的空白帧）再整帧 `fillRect`，而 `diff` 的 `changed` 远小于 `preview` 的 `ink`，或另一帧已经是这个姿势，记为缺陷，并写明应 `duplicateFrame` 后只改差，或在槽位已占死时 `copyRect` 再改差。`changed` 为 0 且没有记录：写未核，像素相同证明不了路径。只有一帧且没有记录：写未核。

四向表再核对行是不是一个朝向、`export sheet` 是否会用 `--cols` 保住这个行数。那是布局，不是审美加分。第 7、8 条是编辑路径，不是审美加分。单帧且没有编辑记录时，两条都写入「未核」。

*Items 7 and 8 are the edit path. A batched call still counts each fill. Identical pixels without a log are 未核, not a pass.*

## Output

按这个结构回复。有问题的条目只写在「缺陷」。通过的条目只写在「未发现」，并附上证据。不要把同一条写进两边。不要用「看起来不错」「基本完成」「完成度很高」代替列表。完成度只允许阶段词：剪影、色块、细节、或未核。下面的数字只示范格式，换成这次数到的行和帧。

```text
## 缺陷
- [头身] 站立立绘 48×64，头 6 行，身体 40 行，读成示意图
- [脚接触] 帧 0 与帧 1 的地面行 y=46 上有横移的 !
- [可合并的 fillRect] 记录里三笔 colorIndex 4、x=8、width=12、height=1，y=20、21、22，可合成 x=8 y=20 width=12 height=3。这三笔在同一次 call 里仍是三次 fillRect
- [重复帧未复制] 帧 3 与帧 1 的 diff changed 为 40，preview ink 约 800；记录是 addFrame 后整帧 fillRect。应 duplicateFrame 帧 1 再改这 40 格，槽位已占死时用 copyRect

## 未发现
- [剪影] 帧 0 的 ASCII 外轮廓能分开头和躯干，身体内部没有多余的 .

## 未核
- [色阶] 彩色 PNG 打不开

## 阶段
色块
```

没有编辑记录、只有一帧时，第 7、8 条改写入「未核」，例如「[可合并的 fillRect] 没有本次编辑记录，最终像素看不出拆成几笔」。不要把这两条算成通过。

阶段词不是分数。清单通过之后仍然写一句：这不判断好不好看。
