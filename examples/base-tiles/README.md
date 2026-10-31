# 据点地面 + 围墙（#106）

16×16 索引图块。地面是 tile 0。围墙是四邻接 16 掩码，tile id 是 `mask + 1`。北 = 1，东 = 2，南 = 4，西 = 8。地图是 10×8 格：整张地面，中间一圈围墙，房间内部留空。`(0,0)` 的地面格水平翻转，用来确认 Tiled GID 的翻转位。

没有地图编辑器，也没有 8 邻接。Tiled 打开 `base.tmj` 时，旁边要有 `tiles.png`。

## 文件

| 路径 | 内容 |
| --- | --- |
| `tileset.json` | 17 帧工程。帧 0 是地面，帧 1–16 是围墙 |
| `wall-rules.json` | `pixelforge-autotile`。16 个 mask 各一块 |
| `map.json` | `pixelforge-tilemap`。层 `ground`、`wall` |
| `tiles.png` | 272×16，一行 17 块 |
| `tileset.meta.json` | `pixelforge-tileset` |
| `map.png` | 160×128 预览 |
| `base.tmj` | Tiled 正交 JSON 地图 |

## 重做

在仓库根目录：

```bash
cargo build -p pixelforge-cli
python3 examples/base-tiles/build.py
python3 examples/base-tiles/validate.py
```

`validate.py` 会拒绝缺了 mask 9 的规则，退出码 1，`error.code` 是 `AutotileRule`。

把这里的 `base.tmj` 和 `tiles.png` 放进一个可跑的页面，见 [examples/lighthouse-outpost](../lighthouse-outpost/README.md)。
