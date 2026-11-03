# 一侧填到其余基本朝向

8×4 的接线例子，不是一张能走路的角色。它演示：只在 left 画一格，`direction fill` 把 right 水平镜像出来，再把 down 和 up 原样占位，然后 `direction validate` 通过。

通过只说明四向 idle/walk 帧段还在，而且像素按规则搬过。镜像不修高光。`copy` 不是新的正面或背面，也不是自然步态。这不是 PixelLab 自动画四向。八向的对角行这一版不填。

丧尸包里已经交出去的右朝向是镜像之后又修过高光的。不要对 `examples/game-pack-zombie/zombie/zombie.json` 执行 `direction fill --overwrite`。

## 命令

在仓库根目录。先有 `pixelforge`（`cargo build -p pixelforge-cli`，下面用 `cargo run -p pixelforge-cli --` 也行）。

```bash
pixelforge new --width 8 --height 4 --output hero.json
# 4 个朝向 × 4 列 = 16 帧。new 已经有第 0 帧，再追加 15 帧。
for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15; do
  pixelforge frame add --project hero.json
done
pixelforge direction set --project hero.json --count 4 --columns 4 --apply
pixelforge frame select --project hero.json --index 4
pixelforge paint --project hero.json --x 1 --y 0 --color-index 1
pixelforge direction fill --project hero.json --from left --mode mirror
pixelforge direction fill --project hero.json --from left --mode copy
pixelforge direction validate --project hero.json
```

left 的 idle 是第 4 帧，格子 `(1, 0)` 是索引 1。`mirror` 把它写到 right 的 idle（第 8 帧）的 `(6, 0)`。接着的 `copy` 跳过已经有墨的 right，把同一格原样写到 down（第 0 帧）和 up（第 12 帧）的 `(1, 0)`。`direction validate` 退出码 0。

`--from down --mode mirror` 会拒绝：北和南没有左右对。占位用 `--mode copy`。目标帧已经有墨时默认跳过，`--overwrite` 才盖掉。

同一条路径的测试是 `cargo test -p pixelforge-cli direction_fill_from_one_side`。

*An 8×4 wiring example, not a walk cycle. Paint one cell on left, mirror it to right, copy it onto down and up, then `direction validate`. Exit 0 means the idle/walk spans are present and the indices moved by that rule. The mirror does not repair highlights. The copy is not a new front or back and not a natural gait. It is not a PixelLab four-direction draw. Diagonal rows are not filled. Do not `--overwrite` the committed zombie sheet; that right row is a mirror plus a highlight pass.*
