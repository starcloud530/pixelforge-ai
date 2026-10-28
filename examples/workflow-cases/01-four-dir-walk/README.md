# 案例 1：四向行走

64×64 港湾信使。一行一个朝向（down、left、right、up），一列一种姿势（idle、walk A、walk B、walk C）。身体在 `body` 层，外套和灯笼在 `gear` 层。

*A 64×64 harbor courier, four facings by idle plus three walk frames. Body and gear are layers. This is a CLI script, not an image model and not unsupervised L4.*

可复现的是 `build.py`。它通过 `pixelforge` CLI 写调色板索引。提示词不会单独生成这张表。

## 记录

| 项 | 值 |
| --- | --- |
| 提示词 / 指令摘要 | 画一个 64×64 的港湾信使：暖皮肤、深色头发、青绿外套、胸口一盏黄铜灯笼、深色裤子和靴子。四向，行序 down、left、right、up，每行 idle 加三帧 walk。右朝向从左朝向 `hflip`，再把高光画回画面左上。每帧最低不透明行固定在 y=56。不要用旋转生成侧视，不要用 IK 生成步态，不要调用图像生成。导出 `--cols 4 --atlas`，用 strict 规则跑 `validate-atlas`。 |
| 耗时 | 外部 Agent 一次会话写成脚本。没有另计画师工时，也没有图像模型排队。三条 `build.py` 在编写用的机器上用 debug 版 `pixelforge` 连着重跑约 9 秒。 |
| 外部模型成本 | $0。没有图像生成或扩散接口。颜色只经过 `palette set` / `palette add` 进内核。 |
| 人工修订次数 | 0。没有画师在脚本之外改像素。同一次会话里脚本改过三处，见失败点，那是 Agent 看预览之后改矩形，不是第二次人工绘制。 |
| 失败点 | 见下。 |

## 复现

预编译 `pixelforge` 在 `PATH` 上，或设置 `PIXELFORGE`。在仓库根目录：

```bash
python3 examples/workflow-cases/01-four-dir-walk/build.py
pixelforge validate-atlas \
  --atlas examples/workflow-cases/01-four-dir-walk/courier-sheet.json \
  --rules examples/workflow-cases/01-four-dir-walk/courier-rules.json \
  --strict
```

`build.py` 会重写本目录的工程、表和规则，并自己跑一遍 strict。从源码编 CLI 时先 `cargo build -p pixelforge-cli`，脚本会找到 `target/debug/pixelforge`。

帧：idle 200 毫秒，walk 120 毫秒。标签是 `idle_down` / `walk_down` 这一组，`loop` 为 `forward`。规则里的原点是 `(32, 57)`，在脚底下一格。每帧最低不透明行是 y=56，所以相对原点的偏移都是 -1。

右朝向是左朝向两个层分别 `hflip`，再改高光。不要整帧重画。走路帧从本行 idle 复制之后只挪四肢。

## 失败点

写脚本时实际碰到的，和故意没做的：

1. **抬腿上移会和外套叠在同一格。** 大腿若连同靴子一起上移，身体层会画进装备层已经有墨的外套里。脚本改成大腿只水平挪，靴子单独上移并盖住小腿（同一层）。着地的那只脚留在 y=56。`build.py` 会检查这件事，滑了就退出。
2. **袖子移开后和肩断开。** 水平 `moveRect` 之后用一小块 `fillRect` 把肩和袖接上，没有把整条手臂按扫描行重画。
3. **侧视第一版鼻子太长，读成兽吻。** `validate-atlas` 不看这个。缩短鼻子是看了 `preview` 之后改的。头相对 64 格仍然偏大，这是游戏精灵档，不是立绘。
4. **`hflip` 不改颜色。** 右侧只把脸上和外套上的一小块高光填回画面左上。袖子和靴子没有逐帧重打光。
5. **侧视不是旋转出来的，走路不是 IK。** 旋转补洞不是无损。短链 IK 只解关节，不生成步态。见仓库 README 的「已知限制」。

`validate-atlas` 退出码 0 只说明清单里没有 error。头身、袖子是不是方块、好不好看，都不在这份清单里。

## 文件

| 路径 | 内容 |
| --- | --- |
| `courier.json` | 16 帧，两层：`body`、`gear` |
| `courier-sheet.png` | 256×256，`--cols 4`，`--scale 1` |
| `courier-sheet.json` | `pixelforge-atlas` |
| `courier-rules.json` | strict，边长 64，`fourDirection: on`，原点 `(32, 57)` |
| `build.py` | 重画并导出。绘制在上一级的 `courier_kit.py` |
