---
name: pixelforge-critique
description: >-
  Read-only critique of a PixelForge figure or sprite. Use when the user asks
  to review, critique, or check an existing frame (检查, 评审, 看起来怎么样,
  剪影, 头身比, 色阶), including whether a preview looks finished. Report
  defects with evidence from preview and diff. Do not paint, fill, erase, or
  change the file. Do not answer with praise alone or with 看起来不错. Follow
  the pixelforge skill for preview and diff. Drawing or fixing pixels is
  pixelforge-draw or pixelforge-sprite-4dir, not this skill. This checklist
  does not replace a pixel artist's judgment.
---

# 只读检查

先读 `pixelforge` 里 `preview` 和 `diff` 的语义。本 skill 不改工程。不调用 `paint`、`erase`、`fill-rect`、`erase-rect`、`copy-rect`、`move-rect`、`hflip`、`resize`、`palette`、`layer flatten`，也不手改 JSON。

清单补的是观察步骤。通过清单不等于好看，也不能代替专业画师。不要把「各项都写了」说成成稿。

*Read-only. Use preview and diff. Passing the list is not a statement that the picture looks good.*

## When to use

用户要检查、评审、或问一张已有的图好不好、完成了没有。用户要改像素时改用 `pixelforge-draw` 或 `pixelforge-sprite-4dir`，不要在这次只读里顺手画。

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

先 `pixelforge scene --project hero.json`。它不改文件。用 `result.width` / `height`、`activeFrame`、`activeLayer`、`layers`、`paletteHash`、`frameCount`、`frames[].delayMs` 确认你在看哪一帧、哪一层、哪套颜色。`visible` 恒为 true，`opacity` 恒为 null：内核没有逐层隐藏，也没有逐层透明度。`scene` 不带像素；格子在 `preview` 和 `diff` 里。

`preview` 的 `result.bounds` 是有墨格子的外接矩形，`result.ink` 是非 0 格数，`result.indices` 是每个索引用了几次，`result.pixels` 是这一张视图的索引。头身和脚的行号可以从 `bounds` 和 `pixels` 数，不必只靠 ASCII。字形 `*` 把 62 及以上折成同一个字符。

`diff` 的 `result.changed` 是索引不同的格数，`result.bounds` 是这些格子的外接矩形，`result.kinds.differ` 是两边都有墨且不同，`result.changes` 里的 `left` / `right` 才是索引。ASCII 的 `!` 不包含索引。脚滑要看地面那一行的 `changes`，不要只看有没有 `!`。

本 skill 不调用 `--dirty`。那个旗标写在 `paint` 一类命令上，会改文件。色阶仍要打开彩色 PNG；哈希相同只说明调色板没变。

*Read `scene` first. It does not change the file. `preview` adds `ink`, `bounds`, `indices`, and `pixels`. `diff` adds `bounds`, `kinds`, and `changes` with the real indices. Do not pass `--dirty` from this skill. A matching `paletteHash` only says the palette did not change.*

## Checklist

每一条都要写进输出。通过的条也写证据（数过的行、帧号、你看到的字形）。不要合并成一句总评。

1. **剪影**：只看 ASCII 的外轮廓，能不能认出朝向和是不是一个人。身体内部的 `.` 是破洞还是故意的缝（手臂和躯干之间可以是缝）。
2. **头身**：头占几行，身体占几行。站立立绘若是 48×64 或更小，写成示意图，不要写成小而精。16×16 和 32×32 不是站立全身。尺寸对照在 `pixelforge-draw`：常见四向游戏精灵大约 32–64；更高细节的俯视或全身可以是 96×96 或 128×128，需要拉高时是 96×128 或 64×128。用行数说明读不读得出。96 和 128 的整张 ASCII 很难一眼读完，外轮廓和头身改看 `preview --scale` 的彩色 PNG，再抽 `result.rows` 里头、身体、脚那几行。格子多不等于画完。立绘的 64×128 标准不要套到一张 32–64 的走表上。
3. **侧向深度**：侧向要有重叠，或远侧一块更深的索引。一张正面被镜像、没有重叠的「侧面」记为缺陷。
4. **腿体积连续**：大腿到小腿到脚在 ASCII 里是一块相连的质量。中间出现 `.`，或收到只剩一格又突然变粗，记为断点。
5. **脚接触**：同一朝向的 idle 和 walk，最低的非透明格是否在同一地面行。`diff` 里地面行出现横移的 `!` 或单独字形，记为脚滑。俯视小体型看身体最下面的接触格，滑一格也算。
6. **色阶**：皮肤、衣服、金属是不是不同索引，彩色 PNG 里能不能分开。相邻区域同一索引，或颜色接近、边界分不清，记为糊。没看到 PNG 就写未核。

四向表再核对行是不是一个朝向、`export sheet` 是否会用 `--cols` 保住这个行数。那是布局，不是审美加分。

## Output

按这个结构回复。有问题的条目只写在「缺陷」。通过的条目只写在「未发现」，并附上证据。不要把同一条写进两边。不要用「看起来不错」「基本完成」「完成度很高」代替列表。完成度只允许阶段词：剪影、色块、细节、或未核。下面的数字只示范格式，换成这次数到的行和帧。

```text
## 缺陷
- [头身] 站立立绘 48×64，头 6 行，身体 40 行，读成示意图
- [脚接触] 帧 0 与帧 1 的地面行 y=46 上有横移的 !

## 未发现
- [剪影] 帧 0 的 ASCII 外轮廓能分开头和躯干，身体内部没有多余的 .

## 未核
- [色阶] 彩色 PNG 打不开

## 阶段
色块
```

阶段词不是分数。清单通过之后仍然写一句：这不判断好不好看。
