# 丧尸 / 据点道具包（#18）

64×64 索引像素，给自家丧尸经营原型用的两张表。这里没有游戏工程。导入约定和工具侧门槛在 [docs/export.md](../../docs/export.md) 的「对接校验」和 Godot / Unity 两节。做这套表时记下的失败模式在 [FAILURES.md](FAILURES.md)。

画布是游戏默认档 64，不是立绘。丧尸走到细节阶段：剪影、色块、眼睛和破口都在，走路是三帧挪四肢。道具是路障、木箱、发电机、医疗箱。

## 文件

| 路径 | 内容 |
| --- | --- |
| `zombie/zombie.json` | 16 帧工程。行序 down、left、right、up（S/W/E/N）；列是 idle、walk A、walk B、walk C。`directionLayout` 是 `{count: 4, columns: 4}` |
| `zombie/zombie-sheet.png` | 256×256，`--cols 4`，`--scale 1` |
| `zombie/zombie-sheet.json` | `pixelforge-atlas`。标签 `idle_*` / `walk_*`，`loop` 为 `forward` |
| `zombie/zombie-rules.json` | strict，边长 64，`fourDirection: on`，原点 `(32, 57)` |
| `props/props.json` | 4 帧：barricade、crate、generator、medkit |
| `props/props-sheet.png` | 256×64，一行四格 |
| `props/props-sheet.json` | 每个道具一个标签，`loop` 为 `once` |
| `props/props-rules.json` | strict，`fourDirection: off`，`tagPolicy: optional`，同一原点 |

idle 200 毫秒，walk 120 毫秒，道具 100 毫秒。每帧最低的不透明行是 y=56（左上为原点，y 向下）。规则里的原点 y=57 在脚底下一格，各帧到原点的距离都是 -1。

## 重画、重导、再校验

在仓库根目录。先有 `pixelforge`：

```bash
cargo build -p pixelforge-cli
```

重画工程并导出（`build.py` 结束时会自己跑一遍 strict）：

```bash
python3 examples/game-pack-zombie/build.py
```

只重导、不改像素：

```bash
pixelforge export sheet \
  --project examples/game-pack-zombie/zombie/zombie.json \
  --output examples/game-pack-zombie/zombie/zombie-sheet.png \
  --cols 4 --scale 1 \
  --atlas examples/game-pack-zombie/zombie/zombie-sheet.json
pixelforge export sheet \
  --project examples/game-pack-zombie/props/props.json \
  --output examples/game-pack-zombie/props/props-sheet.png \
  --cols 4 --scale 1 \
  --atlas examples/game-pack-zombie/props/props-sheet.json
```

门槛。两条都要退出码 0：

```bash
pixelforge validate-atlas \
  --atlas examples/game-pack-zombie/zombie/zombie-sheet.json \
  --rules examples/game-pack-zombie/zombie/zombie-rules.json \
  --strict
pixelforge validate-atlas \
  --atlas examples/game-pack-zombie/props/props-sheet.json \
  --rules examples/game-pack-zombie/props/props-rules.json \
  --strict
python3 examples/game-pack-zombie/validate.py
```

`cargo test -p pixelforge-cli game_pack_zombie` 对已提交的 PNG 和 atlas 做同一组 strict 检查，并对丧尸工程跑 `direction validate`。表坏了，或四向 idle/walk 帧段缺了，这个测试失败。`direction validate` 通过只说明朝向和帧段齐，不是自然步态，也不是审美通过。

右朝向是左朝向 `hflip` 之后把高光画回画面左上。不要整帧重画。槽位已经占死后用 `copyRect`，不要 `duplicateFrame` 插进一行中间。
