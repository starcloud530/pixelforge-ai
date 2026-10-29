---
name: pixelforge-layers
description: >-
  Split a PixelForge frame into body, gear, and limb layers. Use when the user
  says 分层, 装备, 身体, body, gear, or limbs, or when adding or selecting
  layers, drawing with --layer without switching the active layer, showing or
  hiding a layer, setting layer opacity, previewing the composite versus one
  layer, reordering the stack, or flattening back to a single layer. Follow
  the pixelforge skill's index-color invariants first.
  Shape on a layer still follows pixelforge-draw. A four-direction sheet layout
  is pixelforge-sprite-4dir. A read-only review is pixelforge-critique.
---

# 分层

先读 `pixelforge`。层上的造型仍按 `pixelforge-draw`：剪影、色块、细节，每阶段 `preview`。本 skill 只规定块放在哪一层。多一层不会让图画得更好。

*Follow pixelforge invariants. Draw each layer with the pixelforge-draw stages. This skill places parts on the stack.*

## When to use

身体、装备、四肢要分开改：换衣服、换武器、挪手臂时不想重画躯干。所有帧共用同一个层栈。下标 0 在最下面。

## Stack

从下到上常用这三层。四肢不一定要独立成层。

| 层 | 名字 | 放什么 |
| --- | --- | --- |
| 0 | `body` | 剪影、皮肤、衣服里属于躯干的部分 |
| 1 | `limbs` | 只有手臂或腿必须单独挪、且会挡住躯干时才加 |
| 最上面 | `gear` | 武器、面具、包、手机 |

手臂不挡着躯干、也不单独循环时，留在 `body` 上，用 `move-rect` 挪。不要为了「整齐」多加一层。

同一格不要在两层上各画一个非 0 索引。上层的非 0 在 `opacity` 为 255 时盖住下层，调色板 alpha 不与下层混合。上层的索引 0 露出下面的层。`layer hide` 让这一层退出合成，像素还在，`preview --layer` 仍能看见。`layer opacity --opacity 128` 让 RGBA 导出按直通 alpha 混合；ASCII 和 `flatten` 仍是整格覆盖，不收成最近色。细节在 `pixelforge` skill。

## Draw

新工程和旧的单层文件都是一层，名字 `Layer 1`。先改名再画。`layer add` 插在当前层上面，并选中新层。不写 `--name` 时名字是下一个空闲的 `Layer N`。

```bash
pixelforge new --width 32 --height 48 --output hero.json
pixelforge layer rename --project hero.json --name body
pixelforge fill-rect --project hero.json --x 8 --y 10 --width 16 --height 24 --color-index 3
pixelforge layer add --project hero.json --name gear
pixelforge fill-rect --project hero.json --x 10 --y 12 --width 12 --height 10 --color-index 5
pixelforge preview --project hero.json --scale 4
```

`fill-rect --layer 0` 画身体，不切换当前层。`preview` 不带 `--layer` 看合成（ASCII 加 `result.path` 彩色 PNG）。`preview --layer 0` 只看身体，不选中它。`export png` 和 `export sheet` 写合成。

```bash
pixelforge fill-rect --project hero.json --layer 0 --x 8 --y 10 --width 16 --height 24 --color-index 4
pixelforge preview --project hero.json --layer 0 --scale 4
pixelforge preview --project hero.json --scale 4
pixelforge layer list --project hero.json
pixelforge layer select --project hero.json --index 0
```

改装备时当前层留在 `gear`，或显式 `--layer` 指向装备那一层。不要在 `body` 上把装备再画一遍。

只改头盔：`selection set` 框住头盔，再 `--layer` 指向 gear。选区外的格子不写。选区不绑定层，所以漏写 `--layer` 时，选区内的 body 仍会被改。没有套索。步骤见 `pixelforge` skill 的「只改头盔」。

`layer remove` 删当前层或 `--index N`。最后一层不能删。`layer reorder --from 1 --to 0` 把上层挪到下面，`to` 是移动之后的下标。

`copy-rect` / `move-rect` 的 `--layer` 只读写那一层，不跨层复制。

## 精修覆盖层 Polish

`bake` 会把曲线或约束管着的层从源帧整层重建。手指上改过的索引画在那一层，下一次烘焙会盖掉。局部修正放在 `role: polish` 的覆盖层，不要把会动的部件挂在这层上。

```bash
pixelforge layer add --project hero.json --name polish --role polish
pixelforge paint --project hero.json --x 26 --y 14 --color-index 7
```

`layer role --role polish` 改已经有的层。`default` 又变回普通层。压平会把覆盖层的索引合成进 `Layer 1`，角色回到 `default`，结果层可见且不透明。隐藏层和 opacity 0 不进入压平。半透明混合不会烤进索引。细节见 `pixelforge` skill 的「抬枪微调不丢修过的手指」。

## 可见性和透明度 Visibility and opacity

换装层先 `layer hide`，再 `layer show`，不必删像素。`opacity` 是 0 到 255，255 是完全不透明，128 最接近 50%。0 在合成里跳过，和隐藏的画面一样，但 `visible` 可以仍是 true。

```bash
pixelforge layer hide --project hero.json --index 1
pixelforge layer show --project hero.json --index 1
pixelforge layer opacity --project hero.json --index 1 --opacity 128
pixelforge layer list --project hero.json
pixelforge scene --project hero.json
```

`scene` 和 `layer list` 的每一层都有 `visible` 和 `opacity`。彩色 PNG 才看得到半透明。ASCII 的 `rows` 仍是盖住的索引。`--checkpoint` 能在下一个进程撤回 `hide` 和 `opacity`。

*Hide a gear layer without deleting its pixels. Opacity is 0..=255. 255 covers. 128 is the nearest step to 50% and blends only in the RGBA PNG. ASCII rows stay the covering index. `layer list` and `scene` report both fields.*

*Bake rebuilds a layer a curve or constraint manages. A finger fix on that layer is replaced. Put the fix on a `polish` overlay, and do not link a moving part to it. `layer role` changes an existing layer. Flatten writes the index composite into `Layer 1` and clears the role. Hidden layers and opacity 0 are omitted. The result is visible and fully opaque.*

## Flatten

需要旧的单层文件时：

```bash
pixelforge layer flatten --project hero.json
```

索引合成写进名为 `Layer 1` 的一层：隐藏层和 opacity 0 不进入，半透明层留下盖住的索引，RGBA 混合不烤进去。结果层可见且不透明。文件回到单层工程，不再写 `layers` 和 `activeLayer`。压平之前用 preview 的彩色 PNG 确认要留下的颜色；ASCII 和压平结果一致，和半透明 PNG 不一定一致。压平是一步撤销，但下一个进程没有这个栈；要留着分层就先复制文件，或在命令上带 `--checkpoint`。

*`--layer` does not select the layer. `preview` without `--layer` is the composite. `layer flatten` writes one layer named `Layer 1`.*

## 部件不是层 Parts are not layers

层上的身体、装备、四肢是像素。命名部件（枪、肩、肘）和锚点（握持点）不在层栈里。枪可以 `part add --layer 1` 指向 `gear`，肩可以不挂层。挪这把枪用 `part translate` / `part rotate` / `part flip`，见 `pixelforge` skill 的「士兵的枪和握持点」。压平之后，已经挂了层的部件改指到层 0，锚点还在。

*Body, gear, and limbs on the stack are pixels. A named part and its anchors are not another layer. Point the gun at `gear` with `part add --layer`, or leave a shoulder unattached. Move that gun with `part translate`, `part rotate`, or `part flip`. The soldier example is in the pixelforge skill. After flatten, an attached part names layer 0 and its anchors stay.*
