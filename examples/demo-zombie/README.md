# 港口守灯人：可重放的CLI像素案例

64×64、32个非透明调色板项、透明索引0。本例由外部Agent用本地PixelForge CLI/kernel绘制；没有调用生图模型、PNG导入、旋转冒充四向或IK自动步态。

![四向行走](https://resources.pixelforge.torzsi.com/demo-zombie/v1/a/walk-four-directions.gif)

本目录只保存工程层。展示层在CDN，`clone`后验证或重建工程不需要下载展示素材，也不需要Pillow或FFmpeg。

## 快速验证与完整重放

从仓库根目录运行，Python3只使用标准库：

```bash
cargo build -p pixelforge-cli
python3 examples/demo-zombie/validate.py
python3 examples/demo-zombie/replay.py
```

如果使用已安装的预编译CLI，直接运行后两条即可。也可通过`PIXELFORGE_BIN`指定可执行文件；未指定时优先使用仓库`target/debug/pixelforge`，不存在则从PATH查找`pixelforge`。安装方法见[仓库README](../../README.md)。

`replay.py`在新临时目录中依次运行全部制作阶段、验证manifest和资产、比较四张原尺寸sheet的SHA256。可用`--out /path/to/empty-dir`保留指定位置；非空目录会拒绝，避免覆盖已有资产。运行结束打印结果路径。重放中生成的预览和阶段快照留在输出目录，不必提交Git。

## 工程层文件

| 案例 | 源文件与引擎输出 | 说明 |
| --- | --- | --- |
| A四向行走 | `a/keeper.pxf.json`、`keeper-sheet.png`、`keeper-sheet.atlas.json`、`frame-names.csv` | 20帧，4层；每向1idle+4walk |
| B锁身份换装 | `b/keeper-storm-cape.pxf.json`、`keeper-storm-sheet.png`、atlas、`verification.json` | 只新增storm-cape层，原四层逐帧完全不变 |
| C同色板道具 | `c/harbor-props.pxf.json`、`harbor-props-sheet.png`、atlas | 包、灯、箱、浮标；完整色板与角色相同 |
| D连续局部修改 | `d/local-iterations.pxf.json`、`iteration-sheet.png`、atlas、`verification.json` | 4个版本帧，3次局部编辑 |
| 重放与校验 | `build.py`、`replay.py`、`validate.py`、`pixelforge.manifest.json` | 不依赖CDN |
| 过程记录 | `PROCESS.md`、`process.json`、`validation.json`、`logs/` | 保留请求、结构验证及重放报告；机器路径已归一化 |

原尺寸sheet是工程输出，不是需要排除的宣传大图。A/B图集320×256，C/D图集256×64。atlas中的`image`指向同目录原尺寸PNG。

## A：帧名、方向与时长

行序down、left、right、up（S/W/E/N），每行5格。每格64×64。

| idle /walk标签 | 帧范围 |
| --- | --- |
| idle_down /walk_down | 0 /1–4 |
| idle_left /walk_left | 5 /6–9 |
| idle_right /walk_right | 10 /11–14 |
| idle_up /walk_up | 15 /16–19 |

idle为400ms，walk依次140/110/140/110ms。body、legs、arms、gear四层；侧脸单独造型，右侧镜像后修光和可见颊疤。walk复制本朝向idle后修改腿、手臂和起伏。脚底最低不透明行为y=58，游戏端建议脚底下一格`(32,59)`为锚点，仍需在引擎中设置。

## B/C/D证据

B新增披风层每帧墨水为240格（正面）、251格（侧面）、545格（背面）；这是新层墨水，不是合成图新增面积。原body/legs/arms/gear层均逐格一致。

D0的支撑靴向右偏2格是**明确设置的诊断故障**，不是历史自然缺陷。三步为围巾改色、新增shield层、靴子向左归位2格。合成变化依次75、222、40格。脸部和body层不变，加盾步骤原四层不变，修脚后腿层精确恢复A的接触姿势。

每个`d/step-N/`保留说明和原生diff JSON；对应前后图和diff展示图链接至CDN。`d/local-iterations.pxf.json`包含全部版本，可自行运行原生diff：

```bash
pixelforge diff --project examples/demo-zombie/d/local-iterations.pxf.json \
  --frame 2 --with 3 --scale 4 --output /tmp/keeper-foot-diff.png
```

## 展示层（CDN，不进Git）

- [Hero前后对比](https://resources.pixelforge.torzsi.com/demo-zombie/v1/storyboard/hero-before-after.png)
- [七阶段故事板](https://resources.pixelforge.torzsi.com/demo-zombie/v1/storyboard/timeline.png)
- [换装前后](https://resources.pixelforge.torzsi.com/demo-zombie/v1/b/before-after.png)
- [道具预览](https://resources.pixelforge.torzsi.com/demo-zombie/v1/c/harbor-props-preview.png)
- [三次局部修改](https://resources.pixelforge.torzsi.com/demo-zombie/v1/d/local-iterations.png)
- [40秒sheet组装短片](https://resources.pixelforge.torzsi.com/demo-zombie/v1/videos/01-sheet-assembly.mp4)
- [60秒局部编辑短片](https://resources.pixelforge.torzsi.com/demo-zombie/v1/videos/02-local-editing.mp4)

短片编排自真实CLI导出，**非GUI或逐键终端录屏**。安装镜头对应本地已构建二进制安装与检查，不宣称全新机器下载。可选`media.py`用于本地重建展示层：先完成`replay.py --out ...`，安装`requirements-media.txt`中的媒体依赖，再将`PIXELFORGE_DEMO_OUT`指向该输出目录运行`media.py`及`media.py videos`。该脚本不改资产像素。

## 验收范围与许可

验证覆盖atlas结构、四向标签、共享色板、20帧脚底行、每向4个不同walk姿势、B身份保护和D修改边界。**`aestheticApproval`仍为false**。步态、造型和游戏引擎适配仍需人工判断，结构校验不等于审美或进游戏验收。

本包美术为AI辅助、原生CLI绘制，采用[示例素材CC BY4.0许可](../LICENSE-ASSETS.md)，署名PixelForge / starcloud530。脚本和工具遵循仓库[MIT OR Apache-2.0](../../LICENSE)双许可。
