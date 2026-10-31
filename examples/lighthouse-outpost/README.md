# 灯塔 + 据点小场景（#111）

用现在的 CLI 画一座 32×64 灯塔，再把 `examples/base-tiles` 已经导出的 `base.tmj` 和 `tiles.png` 放进一个能跑的页面。不改内核，也不补 8 邻接或地图 GUI。测量记在 [docs/trials/111-lighthouse-tilemap.md](../../docs/trials/111-lighthouse-tilemap.md) 和 `metrics.json`。

## 文件

| 路径 | 内容 |
| --- | --- |
| `build.py` | 用 line / ellipse / polygon / fill / stroke-rect、层和 opacity 128 的光束画灯塔，再压平 |
| `lighthouse.json` | 三层工程。`beam` 的 opacity 是 128 |
| `lighthouse-before.png` | 压平前的 RGBA 导出。光束在空地上仍是半透明 |
| `lighthouse.png` | `layer flatten` 之后的导出。游戏默认放这一张 |
| `lighthouse.preview.png` | 压平结果的 4 倍预览 |
| `lighthouse-before.preview.png` | 压平前的 4 倍预览 |
| `play.html` / `play.js` | 读取 `../base-tiles/base.tmj` 和旁边的 `tiles.png`，把灯塔画在地图西侧 |
| `validate.py` | 核对已提交的文件，并重跑 `build.py` 看是否一致 |

页面不读 `map.png`。那张是整图预览，不是这个场景用的数据。

## 重画

在仓库根目录：

```bash
cargo build -p pixelforge-cli
python3 examples/lighthouse-outpost/build.py
python3 examples/lighthouse-outpost/validate.py
```

并排看压平差，不改 `lighthouse.json`。左是未量化 RGBA（`lighthouse-before.png`），右是压平会写下的索引色（`lighthouse.png`）：

```bash
pixelforge preview --project examples/lighthouse-outpost/lighthouse.json --compare --scale 4
```

*`preview --compare` does not edit `lighthouse.json`. Left matches `lighthouse-before.png`. Right matches `lighthouse.png`.*

## 打开场景

`fetch` 读不到 `file://`。在 `examples/` 起服务：

```bash
python3 -m http.server -d examples 8765
```

浏览器打开 `http://127.0.0.1:8765/lighthouse-outpost/play.html`。方向键或 WASD 在围墙里走动。`?sprite=before` 换成压平前的精灵。
