# #18 失败模式

这套包用当前 CLI 画完，并且 `validate-atlas --strict` 退出码是 0。下面是做的时候实际碰到的拒绝，以及退出码已经是 0、Godot / Unity 仍会做错的事。

没有单独的游戏仓库，也没有在这个仓库里打开 Godot 或 Unity。消费侧就是这份清单，加上 [docs/export.md](../../docs/export.md) 的 Godot 4 / Unity 两节。`validate-atlas` 不自动修图。

探针都在临时目录里跑，没有改已提交的表。丧尸表不带规则时 `checks.pivot` 是 `skipped`；带 `zombie-rules.json` 时是 `checked`，`fourDirection` 为 true。道具规则把 `fourDirection` 关掉，`emptyTags` 为 `off`。

## 工具侧：validate-atlas 会拒绝

这些命令的退出码是 1（文件打不开是 2）。

**空标签。** 把 atlas 的 `tags` 清成 `[]` 再 strict：

```text
error missingTags: atlas has no tags
```

四向名字还没打的时候，`fourDirection` 是 auto，空列表不会走进四向检查，而是直接记「没有标签」。行齐了再 `tag add`。`duplicateFrame` 不会把新帧收进已有标签，插帧还会把后面的区间挤开。这套表是先复制出 16 个槽，改完像素再打 `idle_*` / `walk_*`。

**画布不在 64 / 96 / 128。** 32×32 涂一格、打一个标签、导出后 strict：

```text
error size: canvas is 32x32 (sheet cell 32x32 at scale 1); allowed edges are 64, 96, 128
```

`new` 的默认仍是 16×16，同样过不了。据点道具如果要 32，得在规则 `sizes` 里写上 32。这套道具保持 64，所以不带规则的 strict 也能过。

**PNG 文件名和 atlas `image` 不一致。** `--sheet renamed.png`，atlas 里仍是 `zombie-sheet.png`：

```text
error image: atlas image is `zombie-sheet.png`; sheet file name is `renamed.png`
```

改名要两份一起改，或者导出时让 `--output` 的文件名就是要提交的名字。

**原点漂移，而且只有规则里写了 pivot 才查。** 导出不写 `pivot`。不带规则时，把第 1 帧的一只脚涂到 y=63，strict 仍是退出码 0，`checks.pivot` 为 `skipped`。带上 `zombie-rules.json` 才变成退出码 1：

```text
error pivot: opaque bottom offset from the pivot differs (frame 0 offset -1, frame 1 offset 6)
```

`--loose` 把同一句降成 warning，退出码回到 0。已提交的表每帧最低不透明行都是 y=56，规则原点是 `(32, 57)`，距离是 -1。

**道具误开四向。** `--four-direction on` 打在道具表上：

```text
error naming: tag `barricade` is not {idle|walk}_{down|left|right|up}
error missingTags: facing down has frames 0-3 and no idle_down or walk_down tag
```

`crate`、`generator`、`medkit` 同样是 `naming`。道具规则必须 `fourDirection: off`。角色表用 `on`。

## 工具侧：碰到了，但 validate-atlas 不判失败

**`--cols` 省略时，4 个道具会排成 2×2。** `ceil(sqrt(4))` 是 2。临时导出的 atlas 是 `columns: 2`、`rows: 2`、128×128。带着道具规则 strict 仍是退出码 0：标签是帧区间，不是行。已提交的表显式 `--cols 4`，一行四格，256×64。16 帧的丧尸表碰巧 `ceil(sqrt(16))` 也是 4，省略 `--cols` 看起来对，换帧数就会拆行。

**`--scale 2` 不改变「画布档」，也会放过一个对不齐的原点。** 格子变成 128，表变成 512×512，`frameWidth / scale` 仍是 64，尺寸检查通过。atlas 里没有 `pivot`。规则里的 `(32, 57)` 是按 scale 1 的格子写的；放大后各帧脚底一起下移，距离仍然彼此相等，strict 退出码 0。引擎若把 57 当成 128 格子里的脚底，人会浮起来。已提交的表是 `--scale 1`，格子就是 64。

**整帧透明只是 warning。** 64×64 上第二帧空着，标签盖住 0–1，strict 退出码 0：

```text
warning emptyFrame: 1 frames are fully transparent, starting at frame 1
```

游戏会播一帧空白。退出码 0 不能当成「没有空帧」。

**`hflip` 不改颜色。** 右朝向从左朝向整帧镜像。画面左上的光跟着身体翻到另一侧。`validate-atlas` 不看高光在哪一侧，镜像前后都是退出码 0。右朝向在镜像之后另画了画面左侧的亮色。矩形 `hflip` 也不搬部件。

**没有魔棒，也没有套索。** 头的圆角和路障中间的缺口是 `eraseRect`。选区是矩形蒙版。

**`paint` 没有 `--frame`。** 它只写当前帧。探针里对第 1 帧补脚底，直接加 `--frame` 被 clap 拒绝（退出码 2）。要先 `frame select`。

**`duplicateFrame` 插在源帧后面，而且不选中新帧。** 左朝向的走路帧已经排好之后再复制 idle，会把新帧插进这一行，后面的下标和标签一起后移。这套表先把第 0 帧复制出 15 份空槽，再用 `copyRect` 填右朝向和背面。

**旋转做不出第二个朝向。** 16×3 的细条 `part rotate --degrees 20` 结果是 `dropped: 3`、`conflicts: 3`、`sealed: 3`、`breaks: 0`、`grid: nearest`。补洞不把 `dropped` 清成 0。同一块矩形 `rotate --degrees 90`：

```text
RotateSquare: 90 and 270 degree rotation needs a square rectangle, got 16x3
```

四向是四套剪影。背面是正面盖住脸，右侧是左侧镜像。没有用 `bake` 把一条腿转成走路：一条静止姿势转不出来正面、侧面和背面，最近格旋转还会丢像素。`bake` 适合同一剪影上的抬手或钉脚，不适合这张表。

## 退出码 0 之后，Godot / Unity 仍会错

引擎不读 `pixelforge-atlas`，也不读 `pixelforge-atlas-rules`。过滤、mipmap、轴心、动画名都要手接。[docs/export.md](../../docs/export.md) 的 Godot 4 和 Unity 是这三步的后两步。

| 游戏里会错的地方 | 表上有什么 | 引擎要做什么 |
| --- | --- | --- |
| 发糊 | PNG 没有采样模式 | 关 Filter / mipmap。Godot 用 Nearest。Unity Filter Mode 用 Point，Compression 用 None，打开 Alpha Is Transparency，Mesh Type 用 Full Rect |
| 格子对不上 | 丧尸 4×4，每格 64。道具 4×1，每格 64 | Godot：`hframes` / `vframes` 用 atlas 的 `columns` / `rows`。Unity 按 Cell Size 64×64 切。不要把 `--scale 2` 的 128 格当成 64 |
| 脚不在地上 | atlas 没有 pivot。规则原点是帧内 `(32, 57)`，y 向下 | Godot `centered` 时 `offset = Vector2(0, -25)`（格心在 (32, 32)，原点在它下面 25 像素）。Unity 轴心从左下角算是 `(0.5, 7/64)`，也就是 `(0.5, 0.109375)`。规则文件不会被导入 |
| 动画名对不上 | 标签是 `idle_down` 这种闭区间 | Godot `SpriteFrames`、Unity Animator 都不会读这些名字。Unity 切片叫 `zombie-sheet_0` 这种序号。按下表自己建剪辑 |
| 时长不对 | `delayMs` 在 atlas，不在 PNG | 秒 = `delayMs / 1000`。idle 0.2，walk 0.12，道具 0.1。Unity 不要用一个采样率盖住两种时长 |
| 播到透明空位 | 这两张表没有多余格子 | 帧数不是列数整数倍时，末尾空位不是帧。动画停在真实帧数 |
| 左右光反了 | 校验不看高光 | 镜像之后要看画面左上，不能只看退出码 |

### 丧尸动画

| 标签 | 帧 | 时长 |
| --- | --- | --- |
| `idle_down` | 0 | 200 ms |
| `walk_down` | 1–3 | 120 ms |
| `idle_left` | 4 | 200 ms |
| `walk_left` | 5–7 | 120 ms |
| `idle_right` | 8 | 200 ms |
| `walk_right` | 9–11 | 120 ms |
| `idle_up` | 12 | 200 ms |
| `walk_up` | 13–15 | 120 ms |

`loop` 都是 `forward`。文档自己的预览仍播整条时间轴，不按标签跳。

### 道具

| 标签 | 帧 | 说明 |
| --- | --- | --- |
| `barricade` | 0 | 路障。中间木板有缺口 |
| `crate` | 1 | 木箱 |
| `generator` | 2 | 发电机，黄黑条和表盘 |
| `medkit` | 3 | 医疗箱 |

`loop` 是 `once`。不要把四格播成动画。Godot 用四个 `Sprite2D`，或一个图集里四个 region。`hframes = 4`，`vframes = 1`。

## 这张图自己的限度

64 档能分开头、衣服和朝向，头身仍然是游戏精灵，不是 96 或 128 的立绘。走路没有把身体重心交出去，抬起来的那只脚靠矩形挪开，肩和髋用一小块填回去。侧面和背面不是同一套部件烤出来的。这些校验都不会报。
