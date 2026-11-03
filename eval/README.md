# Agent 评测约定 Eval harness conventions

用真实任务评估外部 Agent（Codex、Claude Code，或其它能跑本地命令的模型）在调用 `pixelforge` CLI 时的**轨迹**与**最终输出**是否达到预期，并探测能力边界。本目录当前只约定任务写法、产物格式与评分口径；**运行器后续补，当前仅约定，不提供执行脚本。**

*Evaluate external agents (Codex, Claude Code, or any model that can run local commands) on real `pixelforge` CLI tasks: trajectory quality and whether the final output meets the brief, plus where capability ends. This tree documents task layout, run artifacts, and scoring. A runner comes later; there is no harness script in this change.*

产品路径不变：仓库外的 Agent → 本地 `pixelforge` CLI → Rust 内核 Tool API。命令语义以 [docs/agent-skill.md](../docs/agent-skill.md) 与 [docs/tool-api.md](../docs/tool-api.md) 为准。Skill 在 `.cursor/skills/`（见 [AGENTS.md](../AGENTS.md)）。不要手改工程 JSON 塞颜色，不要发明 CLI 子命令。

*Same product path: agent outside the repo → local `pixelforge` CLI → Tool API. Command semantics: [docs/agent-skill.md](../docs/agent-skill.md) and [docs/tool-api.md](../docs/tool-api.md). Skills live under `.cursor/skills/` (see [AGENTS.md](../AGENTS.md)). Do not hand-edit project JSON to plant colors, and do not invent CLI subcommands.*

本目录不改引擎、不加评测运行脚本。`check.py` 只作为某道题在 `eval.md` 里声明的可选机判入口；模板目录不附带可执行检查脚本。

*This tree does not change the engine and does not add a runner. An optional `check.py` is only the machine-check entry a task's `eval.md` may name; the template does not ship an executable checker.*

## 目录约定 Layout

```
eval/
  README.md
  tasks/
    <tier>-<slug>/   # 如 t1-recolor-coat
      task.md        # 必需：给 Agent 看，不含评分细节
      eval.md        # 必需：给评审看的验收标准
      start/         # 可选：初始工程 / 色板 / 参考图
      check.py       # 可选：eval.md 中可机判部分
  runs/              # 不进 Git：trajectory.jsonl、output/、score.json
```

- `tasks/<tier>-<slug>/`：一道题一个目录。`<tier>` 是下面的分档前缀；`<slug>` 用小写短横线，例如 `t1-recolor-coat`。
- `task.md`：只写 Agent 需要完成的目标、输入、约束、交付物。**不要**写如何打分、预期失败点、或机判实现细节。
- `eval.md`：给评审 / 机判用。含必须满足、加分项、人工分、预期边界、机判方式。Agent 运行时应看不到这份文件（或同等隔离）。
- `start/`：可选。初始 `pixelforge-project`、锁定色板、参考 PNG 等。没有初始工程时目录可空（模板用 `.gitkeep` 占位）。
- `check.py`：可选。只实现 `eval.md`「必须满足」里声明为可机判的条目；不负责观感分。
- `runs/`：已在根 [.gitignore](../.gitignore) 里忽略，**不要提交**。每次评测一次运行写一个 `runs/<run-id>/`。

*One directory per task. `task.md` is agent-facing and must not leak scoring. `eval.md` is reviewer-facing. `runs/` is gitignored.*

新题从 [tasks/_template/](tasks/_template/) 复制目录，改名成 `<tier>-<slug>`，再填 `task.md` / `eval.md`，按需往 `start/` 放文件。`_template` 本身不是可跑的题。

*Copy [tasks/_template/](tasks/_template/), rename to `<tier>-<slug>`, fill the markdown, and add `start/` assets as needed. `_template` is not a runnable task.*

## 分档 Tiers

| 档 | 含义 | 典型能力 |
| --- | --- | --- |
| **t1** | 单帧编辑 | 在已有工程上改色、填矩形、换索引；`paint` / `fill-rect` / `replace-index` / `palette` |
| **t2** | 局部修改、保持不变区 | 选区或层约束下只改一块；`selection set` / `selection wand` / `--layer`；保护区像素必须不变 |
| **t3** | 多帧多向 | 多帧、`tag add`、四向布局；`frame duplicate`、`direction set` / `validate`、`export sheet`、`validate-atlas` |
| **t4** | 动画与换装 | 标签循环、部件 / 层换装、烘焙或约束相关流程；`export apng --tag`、`layer`、`part` / `bake` 等（以题面约束为准） |
| **t5** | 图块地图等综合 | 图块集、四邻接、tilemap、Tiled `.tmj`、清单；`tileset` / `tilemap` / `validate-tiles` / `export tileset` / `export tilemap`、`project validate` |

分档描述能力跨度，不是难度分数。一道题可以在 `eval.md` 写「预期边界」，专门用来探测失败点。

*Tiers describe capability span, not a numeric difficulty score. A task may document expected failure points in `eval.md` for boundary probing.*

## 运行产物 `runs/<run-id>/`

每次运行选定一个唯一 `<run-id>`（建议含模型名、题 slug、时间戳）。目录约定：

```
runs/<run-id>/
  trajectory.jsonl   # 每行一条 pixelforge 调用
  output/            # 最终工程与导出物
  score.json         # 各项 pass/fail；机判与人工分字段分开
```

### `trajectory.jsonl`

每行一条 JSON，对应一次进程级的 `pixelforge …` 调用（不是一次 `call` 里的单个 Tool API 请求）。字段约定：

| 字段 | 说明 |
| --- | --- |
| `argv` | 完整参数数组，例如 `["pixelforge","preview","--project","hero.json","--scale","4"]` |
| `exit_code` | 进程退出码（`0` / `1` / `2`，语义见 [docs/agent-skill.md](../docs/agent-skill.md)「输出」） |
| `duration_ms` | 该次调用耗时（毫秒） |

可选字段（运行器后续可补，题面不要依赖）：`cwd`、`stdout` 摘要、`stderr` 摘要、时间戳。同一条 `pixelforge call` 带多个 `--request` 仍记**一行**（一次进程）。

*One JSON object per process invocation. Multiple `--request` flags on one `call` are still one line.*

示例（字段齐全即可，值随题变化）：

```json
{"argv":["pixelforge","inspect","--project","hero.json"],"exit_code":0,"duration_ms":42}
{"argv":["pixelforge","fill-rect","--project","hero.json","--x","4","--y","8","--width","16","--height","40","--color-index","3"],"exit_code":0,"duration_ms":38}
{"argv":["pixelforge","preview","--project","hero.json","--scale","4"],"exit_code":0,"duration_ms":55}
```

命令必须是仓库里真实存在的子命令与参数，例如：

```bash
pixelforge new --width 16 --height 16 --output hero.json
pixelforge fill-rect --project hero.json --x 4 --y 8 --width 16 --height 40 --color-index 3
pixelforge selection set --project hero.json --x 10 --y 4 --width 12 --height 10
pixelforge replace-index --project hero.json --from-index 4 --to-index 6
pixelforge frame duplicate --project hero.json --index 0
pixelforge tag add --project hero.json --name walk_down --from 1 --to 3 --loop forward
pixelforge export sheet --project hero.json --output hero-sheet.png --cols 4 --atlas hero-sheet.json
pixelforge validate-atlas --atlas hero-sheet.json --sheet hero-sheet.png
pixelforge direction validate --project hero.json
pixelforge validate-tiles --project tiles.json --map map.json --rules wall.json
pixelforge project validate --manifest pixelforge.manifest.json
pixelforge preview --project hero.json --scale 4
```

完整速查：[docs/agent-skill.md](../docs/agent-skill.md)。不要编造子命令。

### `output/`

Agent 交卷时的最终文件树。至少包含题面要求的工程文件（如 `hero.json`）；若题面要求 sheet / atlas / `.tmj` / preview PNG，也放在这里（或题面写明的相对路径）。评审以该目录为准，不以 Agent 工作目录里的中间态为准。

*The graded tree is whatever the task asks for under `output/`, not intermediate files left in a scratch cwd.*

### `score.json`

机判结果与人工 / 模型观感分**分字段存放，不要加总成一个混分**。建议形状：

```json
{
  "task": "t1-recolor-coat",
  "run_id": "codex-t1-recolor-coat-20261103T021500Z",
  "machine": {
    "items": [
      {"id": "canvas_size", "pass": true},
      {"id": "palette_in_bounds", "pass": true},
      {"id": "validate_ok", "pass": true},
      {"id": "protected_region_unchanged", "pass": false, "detail": "3 cells differed"}
    ]
  },
  "bonus": {
    "items": [
      {"id": "few_steps", "pass": true},
      {"id": "no_spurious_undo", "pass": true},
      {"id": "no_exit_error", "pass": false}
    ]
  },
  "human": {
    "score": null,
    "scale": "1-5",
    "notes": ""
  },
  "expected_boundary": {
    "triggered": false,
    "notes": ""
  }
}
```

- `machine`：对应 `eval.md`「必须满足」里可机判的条目，每项 `pass` / `fail`（上例用布尔 `pass`）。
- `bonus`：加分项，同样逐项记录，**不计入**必须满足是否通过。
- `human`：观感 / 剪影 / 色阶等人工或模型评审分，与机判分开；未评时 `score` 可为 `null`。
- `expected_boundary`：若本题用于边界探测，记录是否按预期失败，以及现象说明。

*Keep machine checks, bonus, and human scores in separate objects. Do not fold them into one blended score.*

## 机判与人工分 Machine vs human

| 类别 | 放哪 | 例子 |
| --- | --- | --- |
| 机判 | `score.json` → `machine` | 画布宽高、色板索引不越界、标签名齐全、`pixelforge validate-atlas` / `direction validate` / `validate-tiles` / `project validate` 退出码 0、保护区 `diff` 无变化 |
| 加分 | `score.json` → `bonus` | 步数少、无多余 `undo`、轨迹里少退出码非 0 |
| 人工 / 模型 | `score.json` → `human` | 剪影、头身、脚滑观感、换装是否可读 |

`validate-atlas`、`direction validate`、`project validate` 等退出码 0 **只说明清单无 error**，不是审美验收（见 README「已知限制」与 [docs/agent-skill.md](../docs/agent-skill.md)）。观感必须走 `human`，不要因为校验通过就把人工分写成满分。

*Exit 0 from validators means the checklist has no error, not that the picture looks good. Put taste in `human`.*

## 预期失败点 Expected failure points

部分题目故意压在当前 CLI / 内核能力边界上（例如自然步态、无损旋转细线、八向对角自动填充、未绑定 polish 随 bake 移动等，见仓库「已知限制」）。这类题在 `eval.md` 的「预期边界」写明：

- Agent **可能**失败的具体点；
- 怎样算「按预期撞到边界」（例如某校验必失败、或某观感项预期低分）；
- 怎样算「意外通过」或「意外用错工具」。

`task.md` **不要**剧透这些失败点。边界探测的价值在于：轨迹是否合理尝试、是否幻觉不存在的子命令、最终是否诚实停在可交付状态。

*Boundary tasks state expected failure points only in `eval.md`. `task.md` must not spoil them.*

## 当前范围 Scope

- **有**：目录约定、模板、`runs/` 忽略规则、本说明。
- **无**：自动拉起 Codex / Claude Code 的运行器、批量打分入口、CI 里跑 eval、具体 t1–t5 题目正文（除 `_template`）。

运行器补上之后，应读取 `tasks/<id>/task.md` 与可选 `start/`，把 Agent 的命令记进 `trajectory.jsonl`，把交付物收进 `output/`，再按 `eval.md` / `check.py` 写 `score.json`。在此之前，人工按同一约定落盘即可。

*A future runner should read `task.md` and optional `start/`, log commands, collect `output/`, and write `score.json` from `eval.md` / `check.py`. Until then, humans can follow the same layout by hand.*
