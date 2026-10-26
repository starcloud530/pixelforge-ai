---
name: pixelforge-sprite-4dir
description: >-
  Build a PixelForge four-direction idle and walk sprite sheet (四向走, 待机,
  idle, walk, sprite sheet), including 96 and 128 cell sizes. Use when laying
  out rows as facings and columns as idle plus walk frames, exporting with
  export sheet --cols, tagging ranges as idle_down / walk_down, writing
  --atlas JSON, running validate-atlas after export and before game
  import, copying a rectangle across frames, checking foot slide
  (脚滑) with diff, or mirroring a side view with hflip and then repairing
  highlights. For a later frame of the same pose, duplicate it and edit
  the diff, or copyRect into a slot that already exists; merge scanline
  fillRects into one taller rect. Follow the pixelforge
  skill's index-color invariants first. Draw a new body with pixelforge-draw's
  silhouette, flats, and details. Do not use this for a single standing
  portrait. Separate equipment layers are pixelforge-layers. A read-only
  review is pixelforge-critique.
---

# 四向走 + 闲

先读 `pixelforge`。新的身体按 `pixelforge-draw` 的三阶段画第一帧，再在这里排朝向和走路帧。本 skill 不把一张表排完当成好动画。脚不滑、朝向读得出，仍然可能僵。流程补的是检查，不是画师的节奏感。

*Follow pixelforge invariants and pixelforge-draw for the first pose. This skill is the sheet layout and the foot-slide check.*

## When to use

交付物是四向（下、左、右、上）的待机加走路，并导出一张精灵表。单张站立立绘或变身故事走 `pixelforge-draw`，不要按这张表拆成四行。

## Size

用 `pixelforge-draw` 的尺寸表，按交付物选行：

- 俯视小体型、小怪：16–32。
- 常见四向全身游戏精灵：大约 32–64。
- 更高细节的俯视或全身精灵：正式档 **96×96** 或 **128×128**。需要拉高时用 **96×128** 或 **64×128**。边长仍是内核的 `1..=256`。`new` 和 `resize` 接受这些尺寸，不另设上限，也不会把格子裁小。
- 站立立绘优先 64×128 或 96×128。16 帧走表不要默认用立绘那么高，除非这一朝向确实要那么多行。

更大的边长只是格子更多，不会自动更好看，观改也更贵。96 和 128 的 ASCII 是整帧、一格一字，没有裁行，但整张 stderr 很难一眼读完。阶段检查看 `preview --scale` 的彩色 PNG，ASCII 只抽局部（接触地面的几行、脚、头）。`resize` 只搬已有索引。

俯视小体型的朝向靠外轮廓和一格「鼻子」或眼睛，不靠一张脸。接触地面的那几格在整个循环里留在相同的格子上，动的是上面的身体。小图上滑一格就像在溜冰。

较大的身体让每一朝向的脚落在同一条地面行上。破损、衣服、金属面罩用不同索引，不要和皮肤糊成一个索引。

*96×96 and 128×128 are game-sprite sizes inside `1..=256`, not a separate command. A larger canvas is more cells. On those sizes, read the scaled PNG and a local slice of the ASCII grid.*

## Layout

内核不管朝向名字。约定由帧序和 `--cols` 保住：一行一个朝向，一列一种姿势。

| 行 | 朝向 | 列 |
| --- | --- | --- |
| 0 | 下 down | idle，walk A，walk B，walk C |
| 1 | 左 left | 同上 |
| 2 | 右 right | 同上 |
| 3 | 上 up | 同上 |

帧下标 = `行 * 每行帧数 + 列`。常见是 1 个 idle 加 3 个 walk，每行 4 帧，共 16 帧。列数不同也可以，整张表的每一行必须一样长。

```bash
pixelforge export sheet --project hero.json --output hero-sheet.png --cols 4 --scale 2
```

`--cols` 等于每行帧数，这样一行还是一个朝向。不传 `--cols` 时列数是 `ceil(sqrt(帧数))`，一个朝向会被拆到两行。`scale` 不改变列数。空位是透明，不是帧。约定也写在 `docs/export.md`。

走路帧时长常用 120 毫秒，idle 可以更长。内核只接受 `1..=65535`，没有单独的「走路时长」。用 `frame delay` 或 `setFrameDelay`。

## Tags

帧排好之后再打标签。标签是闭区间：`from` 和 `to` 都算在里面。四格不相邻的 idle 不能收成一个名叫 `idle` 的标签，所以每一朝向各写一个。

常见 1 个 idle 加 3 个 walk、每行 4 帧时：

| 标签 | 帧 | `loop` |
| --- | --- | --- |
| `idle_down` | 0–0 | `forward` |
| `walk_down` | 1–3 | `forward` |
| `idle_left` | 4–4 | `forward` |
| `walk_left` | 5–7 | `forward` |
| `idle_right` | 8–8 | `forward` |
| `walk_right` | 9–11 | `forward` |
| `idle_up` | 12–12 | `forward` |
| `walk_up` | 13–15 | `forward` |

名字是 `{动作}_{朝向}`。动作是 `idle` 或 `walk`。朝向按行序是 `down`、`left`、`right`、`up`。每行帧数不是 4 时，idle 仍是该行第一格，walk 是该行剩下的格子：`from = 行 * 每行帧数`，idle 的 `to` 等于 `from`，walk 的 `from` 是下一格，`to` 是该行最后一格。这一行没有单独的 idle 时，整行只打 `walk_{朝向}`，不要再造一个空的 idle。

`loop` 用 `forward` 表示走和待机都循环。`once` 留给只播一遍的动作。`reverse` 和 `pingpong` 也可以写。文档自己的循环预览仍走整条时间轴，不按标签播。标签不是 cel 链接：`frame duplicate` 仍是一份新的像素。

复制某个标签的最后一帧时，新帧不会自动进这个标签。走路帧还在加的时候先别打标签；这一行的帧齐了再 `tag add`。

```bash
pixelforge tag add --project hero.json --name idle_down --from 0 --to 0 --loop forward
pixelforge tag add --project hero.json --name walk_down --from 1 --to 3 --loop forward
pixelforge export sheet --project hero.json --output hero-sheet.png --cols 4 --scale 2 --atlas hero-sheet.json
```

`--cols` 仍等于每行帧数。`--atlas` 写出 `pixelforge-atlas` JSON：每一帧的矩形和 `delayMs`，加上上面的标签和 `columns`。不传 `--atlas` 时只出 PNG，和以前一样。Tool API 是 `addTag` / `setTag` / `removeTag`，以及 `exportSpriteSheet` 的 `atlas: true`。

导出之后、进游戏之前先跑 `pixelforge validate-atlas --atlas hero-sheet.json --sheet hero-sheet.png`；有 error 时退出码不是 0，不要把这份表交进游戏。

*Tag after the frames exist. A tag is an inclusive range, so each facing gets its own `idle_*` and `walk_*` tag. The pattern is `{action}_{facing}` with facings `down`, `left`, `right`, `up` in row order. `loop` `forward` repeats. Document playback still walks the whole timeline. Tags are not cel links. Duplicating the last frame of a tag leaves the copy untagged, so tag the row once the cycle is built. `--atlas` writes frame rects, `delayMs`, `columns`, and tags. Omit it and the sheet export stays PNG-only. After export, run `validate-atlas` before game import. An error is a non-zero exit; do not hand that sheet to the game.*

## Build a facing

1. 画好该朝向的 idle（新身体走剪影 → 色块 → 细节，每阶段 `preview --scale 4`）。
2. `frame duplicate`，然后 `frame select` 到返回的下标。复制不选中新帧。
3. 新帧来自 `frame duplicate` 时，身体已经在上面，`move-rect` 只挪腿或手臂。目标槽已经是空白、不能再插入时，才 `copy-rect --from-frame` 把身体拷进去。不要逐格重画一整个人。见「最少编辑路径」。
4. `diff` 对 idle 和这一步 walk，看脚，再画下一帧。

新列可以插在 idle 后面时，复制之后身体已经在新帧上，接着只改差，再 `diff`：

```bash
pixelforge frame duplicate --project hero.json --index 0
pixelforge frame select --project hero.json --index 1
pixelforge move-rect --project hero.json --src-x 8 --src-y 28 --width 8 --height 12 --dst-x 10 --dst-y 30
pixelforge diff --project hero.json --frame 0 --with 1 --scale 4
```

目标槽已经是空白、下标不能再插入时，把身体拷进那个槽，不要对空帧整个人 `fillRect`：

```bash
pixelforge copy-rect --project hero.json --from-frame 0 --to-frame 5 \
  --src-x 8 --src-y 4 --width 16 --height 28 --dst-x 8 --dst-y 4
```

`--from-frame` / `--to-frame` 不改变 `activeFrame`。跨帧 `move-rect` 会清掉整个源矩形。出界则一格都不写。`frame duplicate` 已经把源帧像素放到新帧上，紧接着再 `copy-rect` 同一块是重复操作。

## 最少编辑路径

四条规则和前后对照在 `pixelforge` 的「最少编辑路径」。这张表上的短路径是：这一行的下一列从本行 idle 来，只改腿和手臂；右朝向从左朝向镜像，再把高光画回光源一侧。2026-10-25 士兵射击那种 1,133 次 `fillRect`，就是同姿重画加扫描行，不要带到走表上。

帧可以紧挨在源帧后面时，`frame duplicate` 再 `frame select` 返回的下标，然后 `move-rect` 或一小块 `fill-rect` / `erase-rect`。槽位已经占死（16 帧已经铺好，第 3 列必须留在这一行）时，用 `copy-rect --from-frame --to-frame` 把 idle 的身体拷进那个槽，再改差。`duplicateFrame` 会插入一帧并后移后面的列，不能用来填一个已经定死的下标。

之前，walk A 是 `frame add` 的空白，再把 idle 里已经有的头和躯干按扫描行 `fillRect`。一次 `call` 里排几十个 `--request` 仍是几十次 `fillRect`。`beginGesture` 只把它们收成一步撤销。

之后，从 idle 复制，只挪小腿，再 `diff`：

```bash
pixelforge frame duplicate --project hero.json --index 0
pixelforge frame select --project hero.json --index 1
pixelforge move-rect --project hero.json --src-x 8 --src-y 28 --width 8 --height 12 --dst-x 10 --dst-y 30
pixelforge diff --project hero.json --frame 0 --with 1 --scale 4
```

高度为 1、上下相接、同一 `x`、同一 `width`、同一索引的 `fillRect`，发出前合成一块更高的矩形。例如 `y=20`、`21`、`22` 三笔 `width=12`、`colorIndex=4` 合成 `y=20`、`height=3` 的一笔。左右相接则合成更宽的一笔。规则全文在 `pixelforge`。

右朝向从左朝向来：`copy-rect --from-frame` 到这一行的槽，或 `duplicateFrame` 后再 `hflip`，然后只 `paint` 高光。整帧重画右侧是更长的路径。

*Duplicate the idle and move the limbs when the new column can sit just after it. `copy-rect` into a slot that already exists. Merge scanline `fillRect`s before the call. A batched `call` still counts every request. Mirror the opposite facing and repaint highlights.*

## Feet

`diff` 只读。stderr 上 `.` 是相同索引，`!` 是两边都有墨且不同，其它字形是唯一有墨的一边。彩色 PNG 在 `result.path`（默认 `<stem>.diff.png`），三格是 frame、with、洋葱皮。洋葱皮不写回像素。

看地面那一行：最低的非 `.` 格如果在 idle 和 walk 之间横着挪了，而设计是踩在原地，就是脚滑。俯视小体型看身体最下面那一圈接触格，循环里让它们留在原格。改脚用 `fill-rect` / `erase-rect` 或一小块 `move-rect`，再 `diff` 一次。不要只看彩色缩略图就导出。

## hflip and highlights

右朝向可以从左朝向镜像，或反过来。`hflip` 不用再写 `--horizontal`。左右朝向用这一条。上下镜像是 `vflip`，顺时针转 90、180、270 是 `rotate --degrees`。那两条不代替左右朝向。

```bash
pixelforge hflip --project hero.json --frame 2
```

镜像只搬索引，不改颜色。画面左上的光源不会跟着翻：高光格子跟着身体到了另一侧。镜像之后用 `paint` 把高光画回光源那一侧，再 `preview --frame N --scale 4`。伤疤、包、破布如果设计上就该跟着翻，可以留着；光的色阶不要留在翻错的一侧。

*hflip does not recolor. Repaint highlights on the light side after the mirror. Check feet with diff before export.*
