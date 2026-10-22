---
name: pixelforge-sprite-4dir
description: >-
  Build a PixelForge four-direction idle and walk sprite sheet (四向走, 待机,
  idle, walk, sprite sheet). Use when laying out rows as facings and columns
  as idle plus walk frames, exporting with export sheet --cols, copying a
  rectangle across frames, checking foot slide (脚滑) with diff, or mirroring
  a side view with hflip and then repairing highlights. Follow the pixelforge
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
- 四向全身游戏精灵：大约 32–64。
- 64×128 或 96×128 是站立立绘。不要默认拿来做 16 帧的走表，除非用户要的就是高画布循环。

俯视小体型的朝向靠外轮廓和一格「鼻子」或眼睛，不靠一张脸。接触地面的那几格在整个循环里留在相同的格子上，动的是上面的身体。小图上滑一格就像在溜冰。

较大的身体（站立比例的走路，而不是立绘画布）让每一朝向的脚落在同一条地面行上。破损、衣服、金属面罩用不同索引，不要和皮肤糊成一个索引。

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

## Build a facing

1. 画好该朝向的 idle（新身体走剪影 → 色块 → 细节，每阶段 `preview --scale 4`）。
2. `frame duplicate`，然后 `frame select` 到返回的下标。复制不选中新帧。
3. `copy-rect --from-frame` 把身体拷到新帧，`move-rect` 只挪腿或手臂。不要逐格重画一整个人。
4. `diff` 对 idle 和这一步 walk，看脚，再画下一帧。

```bash
pixelforge frame duplicate --project hero.json --index 0
pixelforge frame select --project hero.json --index 1
pixelforge copy-rect --project hero.json --from-frame 0 --to-frame 1 \
  --src-x 8 --src-y 4 --width 16 --height 28 --dst-x 8 --dst-y 4
pixelforge diff --project hero.json --frame 0 --with 1 --scale 4
```

`--from-frame` / `--to-frame` 不改变 `activeFrame`。跨帧 `move-rect` 会清掉整个源矩形。出界则一格都不写。

## Feet

`diff` 只读。stderr 上 `.` 是相同索引，`!` 是两边都有墨且不同，其它字形是唯一有墨的一边。彩色 PNG 在 `result.path`（默认 `<stem>.diff.png`），三格是 frame、with、洋葱皮。洋葱皮不写回像素。

看地面那一行：最低的非 `.` 格如果在 idle 和 walk 之间横着挪了，而设计是踩在原地，就是脚滑。俯视小体型看身体最下面那一圈接触格，循环里让它们留在原格。改脚用 `fill-rect` / `erase-rect` 或一小块 `move-rect`，再 `diff` 一次。不要只看彩色缩略图就导出。

## hflip and highlights

右朝向可以从左朝向镜像，或反过来。`hflip` 不用再写 `--horizontal`。`flip` 必须写 `--horizontal`。

```bash
pixelforge hflip --project hero.json --frame 2
```

镜像只搬索引，不改颜色。画面左上的光源不会跟着翻：高光格子跟着身体到了另一侧。镜像之后用 `paint` 把高光画回光源那一侧，再 `preview --frame N --scale 4`。伤疤、包、破布如果设计上就该跟着翻，可以留着；光的色阶不要留在翻错的一侧。

*hflip does not recolor. Repaint highlights on the light side after the mirror. Check feet with diff before export.*
