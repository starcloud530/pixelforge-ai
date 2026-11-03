# 111 试用：灯塔和据点小场景

2026-10-30。用仓库里已经合入的 CLI 做一次真实资产，不新加接口，也不在这次修卡点。灯塔脚本在 `pixelforge-demos/build.py`。小场景是 `play.html`，读的是 the base-tiles sample in [https://github.com/starcloud530/pixelforge-demos](https://github.com/starcloud530/pixelforge-demos) (when published) 里已经导出的 `base.tmj` 和 `tiles.png`。半透明压平按 [#112](https://github.com/starcloud530/pixelforge-ai/pull/112) 的规则测量（它关掉了 [#110](https://github.com/starcloud530/pixelforge-ai/issues/110)）。这不是 [#96](https://github.com/starcloud530/pixelforge-ai/issues/96) 的演示美术。

![压平后，游戏默认用的这一张](../../pixelforge-demos/lighthouse.preview.png)

![压平前，光束在空地上仍半透明](../../pixelforge-demos/lighthouse-before.preview.png)

## 灯塔

画布 **32×64**。调色板 11 色：工作室索引 0–8，加上石色 `(132, 124, 112)` 和暗石 `(72, 66, 62)`。三层：`masonry`、`lantern`、`beam`。光束层 opacity **128**。

画出这座塔的命令是 8 次进程：`new`、改层名、两次 `palette add`、两次 `layer add`、`layer opacity`，然后 **一次** `call` 里的 25 条绘制请求。

| 工具 | 次数 | 用在哪 |
| --- | --- | --- |
| `ellipse` | 5 | 塔顶、石基、暗部、左侧灌木 |
| `polygon` | 4 | 塔身填色和描边、石座、光束 |
| `fillRect` | 6 | 檐口、围栏、三道红纹、木门 |
| `line` | 3 | 避雷针、灯室竖档、左侧 pixel-perfect 撑杆 |
| `strokeRect` | 2 | 灯室、塔身小窗 |
| `fill` | 2 | 灯室玻璃、小窗 |
| `selectConnected` / `clearSelection` | 各 1 | 先圈住奶油色塔身，红纹才不会涂到天 |
| `paint` | 1 | 门把手 |

`metrics.json` 里这次脚本的 `cliSeconds` 是 **0.22**。同一台机器连跑两次是 0.17 和 0.177。二进制已经在的时候，CLI 本身不到一秒。冷编译 `pixelforge-cli` 另计，这次环境大约 13 秒，那不是画这张图的成本。

这次没有单独的人工绘制。脚本是一次 agent 会话里写的，中间看了两轮 `preview`。第一轮光束的多边形从塔身中部拉到右下，ASCII 上整片都是盖住索引 5，看起来像塔被涂金了。收成右侧一束之后，塔身正面 `(15, 24)` 仍是奶油色。

压平对照（opacity 128，规则是 alpha 小于 128 写成 0，否则平方 RGB 最近色）：

| | 压平前 | 压平后 |
| --- | --- | --- |
| 部分透明的格子 | 213 | 0 |
| 索引变化 | | 28 |
| 金（5）→ 石（9） | | 24。例：`(20, 14)` 混合 `(134, 116, 69)`，最近的是石色 `(132, 124, 112)`，不是金 |
| 金（5）→ 橙（4） | | 4。例：`(18, 26)` 金盖在红纹上，混合 `(237, 149, 89)`，最近的是橙 `(244, 162, 89)` |
| 空地上的光束 | alpha 128 的金 | 不透明的金。索引往往不变，透明没了 |

把同一张图的光束改成 opacity **127** 再压平：那 213 个半透明格全部变成索引 0。`(255 × 127 + 127) / 255 = 127`，刚好低于 128。128 留下实心光，127 把光擦掉。

## 小场景

`play.js` 193 行，`play.html` 39 行。页面 `fetch` `../base-tiles/base.tmj`，再用 `.tmj` 里的 `image` 去加载旁边的 `tiles.png`，然后把 `lighthouse.png` 底边对齐地图底边、贴在西侧。不读 `map.png`。方向键或 WASD 移动看守；围墙层 GID 非 0 的格子走不进。`?sprite=before` 换成压平前的 PNG。

打开方式写在 `pixelforge-demos/README.md`。`file://` 读不了这些文件。

相对手写一张 tile 表，这次省下的是 [#107](https://github.com/starcloud530/pixelforge-ai/issues/107) 已经做好的 17 块和 10×8 房间，包括 `(0,0)` 那一格的水平翻转位 `0x80000000`。加载器仍要自己解 GID、把 `tiles.png` 相对 `.tmj` 解析、关掉平滑。碰撞和灯塔坐标是页面里写死的。`validate-tiles` 能拦住缺 mask 的规则，这个页面不会去跑它，坏 GID 也会照画。

## 三个问题

1. **灯塔级资产要多久、卡在哪。** 可复现部分是 25 条绘制请求、一次批量 `call`，CLI 大约 0.2 秒。时间花在看图：ASCII 是盖住索引，看不见半透明；椭圆是外接矩形里的内切形；红纹必须先 `selectConnected` 再填，否则会涂出塔身。压平按 #112 工作，但最近色会换索引（金盖墨线变成石色，金盖红纹变成橙色），alpha 127 会把空地上的光整段擦掉。

2. **tileset / tilemap 进游戏是否只停在 validate。** 没有。一个不依赖引擎的页面能把导出的正交 16×16 地面和围墙画出来，并挡住人。省下的是重画图块和手写那张 GID 表。没省掉的是对象摆放、碰撞、翻转位，以及任何 Godot / Unity 工程。这比「只跑 validate-tiles」多了一步能看的场景，离关卡编辑还很远。

3. **下一步是修压平，还是做薄预览，而不是再堆 API。** #110 的「压平丢掉混合、留下盖住索引」已经由 #112 收成最近色。这次测量和那条规则一致，不需要为灯塔再改压平。缺的是一张并排的预览：左边未量化的 RGBA，右边压平后的索引。ASCII 看不出石色替换，也看不出 127 和 128 的断崖。地图侧的卡点保持列在下面，不在这次做地图 GUI，也不加 8 邻接。

## 卡点（这次不修）

- 没有 8 邻接，也没有 47 块 blob。矩形房间只用四邻接 16 掩码。
- 没有地图 GUI。改房间要重跑 CLI 或 `pixelforge-demos/build.py` 再导出。
- `.tmj` 没有对象层。灯塔坐标写在 `play.js` 里。
- 导出没有对角翻转。加载器认 `0x20000000`，但只实现水平、垂直翻转。
- `base.tmj` 的围墙是封闭的一圈，没有门。看守出不去。这是导出内容，不是加载器坏了。
- 页面不调用 `validate-tiles`。校验和显示是分开的。
- 直接打开 HTML 文件时 `fetch` 失败，需要静态 HTTP。
- `export png` 写到目录里的 `pixelforge-000.png`，不能指定单个文件名。
- 彩色 preview 在压平前是未量化 RGB；ASCII 和 `pixels` 仍是盖住索引。
- 压平结果层完全不透明。调色板里没有「金盖红」这一色，混合色只能落到已有索引。
- 只有一张图块集、正交、16 或 32。这个页面拒绝不是 16×16 的图块。
- 层透明度不是逐格的，灯塔没有做闪烁帧。
- 仓库里没有 Godot / Unity 工程。这次的接入成本是这个 HTML 加载器，不是引擎导入。

## Answers

1. A 32×64 lighthouse is 25 drawing requests in one `call`, about 0.2s of CLI once `pixelforge` is built. The slow part is reading the picture: ASCII shows the covering index, so a translucent beam looks solid, and flatten then swaps some of those cells to a nearer palette color.
2. The exported orthogonal tileset and `.tmj` do load in a small page. That is more than `validate-tiles`. It does not place sprites, cut a door, or import into an engine.
3. #112 already snaps the blend. The useful next step is a thin before/after preview, not another drawing API and not a map GUI.
