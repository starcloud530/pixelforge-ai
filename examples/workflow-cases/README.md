# 工作流案例（#81）

三份可公开复现的产品演示。装好预编译 `pixelforge`（见仓库 README 的「预编译安装」）和 Python 3 标准库就能跟跑。不需要 Rust，除非你是从源码编 CLI。

这不是 [#18](https://github.com/starcloud530/pixelforge-ai/issues/18) 的丧尸消费包。`examples/game-pack-zombie/` 仍只验证「表能过 strict、游戏侧还要手接什么」。这里的三份是另一条证据：外部 Agent 用 CLI 走完一条工作流，并把提示、耗时、成本和失败点写下来。

这也不是无人值守的 L4（[#16](https://github.com/starcloud530/pixelforge-ai/issues/16)）。仓库没有内置生图。可复现的是每个目录里的 `build.py`：它调用 `pixelforge`，把调色板索引写进工程。提示词本身不会在没有这份脚本的机器上变出像素。矩形放在哪，是写脚本时看过 `preview` 之后定的。

*Three product demos for #81. A prebuilt `pixelforge` and the Python 3 standard library are enough. They are not the #18 zombie pack and not unsupervised L4. The reproducible artifact is each `build.py`.*

| 案例 | 目录 | 校验 |
| --- | --- | --- |
| 四向行走 | [01-four-dir-walk](01-four-dir-walk/README.md) | 角色 strict，`fourDirection: on` |
| 同角色换装，锁色板 | [02-outfit-change](02-outfit-change/README.md) | 角色 strict，`fourDirection: on` |
| 统一风格道具 | [03-style-pack](03-style-pack/README.md) | strict，`fourDirection: off` |

角色是 64×64 的港湾信使。道具用同一套调色板，`paletteHash` 必须和两份角色工程相同。已知限制（精修不跟部件、旋转有损、IK 不是步态、校验不是审美）见仓库 README，这三份都落在那些边界里。

重画三条：

```bash
python3 examples/workflow-cases/build.py
```

只检查已提交的表、哈希和换装记录：

```bash
python3 examples/workflow-cases/validate.py
```

`validate.py` 不重画。表坏了，或者斗篷工程的身体层记录对不上，退出码不是 0。`cargo test -p pixelforge-cli workflow_cases` 对三张表做同一组 strict 检查。
