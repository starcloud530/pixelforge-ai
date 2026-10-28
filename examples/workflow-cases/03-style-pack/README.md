# 案例 3：统一风格道具包

四个码头道具：挎包、灯笼、木箱、航标。调色板和港湾信使是同一份，青绿印记和黄铜灯是角色外套上已经在用的索引。不是四向表。

*Four harbor props on the courier palette. Strict atlas with four-direction naming turned off. Not the #18 zombie prop sheet, and not an image model.*

可复现的是 `build.py`。它不调用图像生成。若案例 1 的 `courier.json` 已经在磁盘上，脚本会核对 `paletteHash`，不一致就退出。

## 记录

| 项 | 值 |
| --- | --- |
| 提示词 / 指令摘要 | 用信使同一套调色板画四个道具：挎包、灯笼、带青绿印记的木箱、红白航标。不要四向命名。strict，`fourDirection` 为 off，着地行与角色相同（最低不透明行 y=56，原点 32,57）。`paletteHash` 与角色工程一致。 |
| 耗时 | 与前两个案例同一次 Agent 会话。没有另计画师工时。三条 `build.py` 连着重跑约 9 秒（debug 版 `pixelforge`）。 |
| 外部模型成本 | $0。没有图像生成接口。 |
| 人工修订次数 | 0。没有画师在脚本之外改像素。 |
| 失败点 | 见下。 |

三份工程的 `paletteHash` 都是 `eee61aba3bbf619c`。`validate.py` 要求它们相等。

## 复现

```bash
python3 examples/workflow-cases/03-style-pack/build.py
pixelforge validate-atlas \
  --atlas examples/workflow-cases/03-style-pack/harbor-props-sheet.json \
  --rules examples/workflow-cases/03-style-pack/harbor-props-rules.json \
  --strict
```

四个标签：`satchel`、`lantern`、`crate`、`buoy`，各一帧，`loop` 为 `once`，时长 100 毫秒。规则里 `fourDirection` 是 `off`，`tagPolicy` 是 `optional`。道具不是 `idle_*` / `walk_*`，打开四向检查会被命名规则拒绝，所以规则显式关掉。

原点仍是 `(32, 57)`。每个道具的最低不透明行是 y=56，和角色脚底同一条线。这是图集原点，不是航标的吃水线。

## 失败点

1. **风格统一靠的是同一张索引色板，不是一张参考图。** 脚本把角色用的 18 个不透明色按同样顺序写入。道具可以不用满每一种，但哈希把未使用的槽也算进去，所以少写一个颜色就会和角色对不上。
2. **航标和灯笼的底都落在 y=56。** 为了和角色共用原点。校验不关心它像不像浮在水上。把航标画得更短又想保持同一条着地行，底边会空，strict 的 pivot 会报偏移。
3. **这不是 #18 的据点道具。** 丧尸包里的路障、木箱、发电机、医疗箱是另一张色板、另一套规则。不要用那张表代替这一条。
4. **strict 通过不是这四件东西已经能当游戏资产。** 挎包的提手、灯笼的笼架都是矩形。轮廓读不读得清，清单不判。

## 文件

| 路径 | 内容 |
| --- | --- |
| `harbor-props.json` | 4 帧，单层 |
| `harbor-props-sheet.png` | 256×64，一行四格 |
| `harbor-props-sheet.json` | `pixelforge-atlas` |
| `harbor-props-rules.json` | strict，`fourDirection: off`，`tagPolicy: optional`，原点 `(32, 57)` |
| `build.py` | 重画并导出。色板和角色一样，画在上一级的 `courier_kit.py` |
