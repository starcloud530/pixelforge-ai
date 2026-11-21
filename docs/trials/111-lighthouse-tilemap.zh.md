# 接入案例 · 将灯塔放入图块地图 { #111 }

这个案例将一座 32×64 像素的灯塔放入导出的正交图块地图，可以用来检查半透明图层压平后的颜色，以及 Tiled 地图如何查找图块集。[案例源码快照](https://github.com/starcloud530/pixelforge-ai/tree/8fe8c02/examples/lighthouse-outpost)包含绘制脚本、场景加载器和导出图像。

<span id="112-already-snaps-the-blend-the-useful-next-step-is-a-thin-beforeafter-preview-not-another-drawing-api-and-not-a-map-gui"></span>

## 对照光束压平前后的效果 { #_1 }

灯塔包含塔身、灯室和光束三个图层，11 色调色板中有两种石色。光束层的不透明度为 128。

![压平前，光束覆盖空地的部分保持半透明](https://raw.githubusercontent.com/starcloud530/pixelforge-ai/8fe8c02/examples/lighthouse-outpost/lighthouse-before.preview.png)

![压平后，光束映射为色板中的不透明颜色](https://raw.githubusercontent.com/starcloud530/pixelforge-ai/8fe8c02/examples/lighthouse-outpost/lighthouse.preview.png)

| 不透明度 128 时的测量项 | 压平后的结果 |
| --- | --- |
| 原有 213 个半透明格 | 剩余 0 个 |
| 28 个像素更换索引 | 24 格由金色变为石色，4 格由金色变为橙色 |
| 空地上的金色光束 | 变为不透明的金色 |

将不透明度调为 127 后压平，同一批 213 个格子全部变为透明索引 0。可以在自己的工程中运行 `pixelforge preview --project FILE --compare`，先对照效果再压平。

## 运行场景 { #_2 }

下载并解压[源码快照](https://github.com/starcloud530/pixelforge-ai/archive/8fe8c02.tar.gz)。在包含 `examples/` 的目录运行以下命令。

```bash
python3 -m http.server 8000 --bind 127.0.0.1
```

打开 `http://127.0.0.1:8000/examples/lighthouse-outpost/play.html`。使用方向键或 WASD 移动守灯人。在地址末尾添加 `?sprite=before` 可以切换到压平前的灯塔。

<span id="answers"></span>

## 加载器如何放置图块 { #_3 }

加载器读取 `base.tmj`，相对地图文件解析图块集图片的路径，并关闭图像平滑。10×8 的房间使用 17 个图块，围墙图层中有图块的格子会阻挡角色移动。精灵坐标、碰撞和翻转由加载器处理。可分别查看[图块与地图文件](https://github.com/starcloud530/pixelforge-ai/tree/8fe8c02/examples/base-tiles)和[加载器源码](https://github.com/starcloud530/pixelforge-ai/blob/8fe8c02/examples/lighthouse-outpost/play.js)。

## 绘制耗时与接入范围 { #_4 }

归档脚本将绘制灯塔的 25 条请求放在一次批量调用中。二进制程序已编译时，包括该批量调用在内的 22 条 CLI 命令合计耗时 0.22 秒，计时范围不含脚本编写和画面审阅。该加载器支持一个正交 16×16 图块集，以及水平和垂直翻转。接入游戏引擎时，可继续阅读[引擎导入指南](../export.md)。
