# 任务：`<tier>-<slug>` Task：`<tier>-<slug>`

<!--
  给 Agent 看的题面。复制本目录为 eval/tasks/<tier>-<slug>/ 后改写。
  不要写评分细则、预期失败点、或 check.py 实现。那些只放在 eval.md。
  命令示例必须来自 docs/agent-skill.md / docs/tool-api.md，不要编造子命令。
-->

## 目标 Goal

<!-- 一句话说清要交付什么。例如：把外套从索引 4 改成索引 6，其它像素不动。 -->

（填写：完成什么、成功长什么样。）

*State what to deliver and what “done” looks like.*

## 输入 Inputs

<!-- 说明 start/ 里有什么；没有则写「无，自行 new」。路径相对本任务目录。 -->

本任务的 `start/` 提供：

- （例）`start/hero.json` — 初始 `pixelforge-project`
- （例）`start/ref.png` — 参考图（只读，不要写回工程）
- （若无初始工程）无；请用 `pixelforge new --width W --height H --output …` 创建

*List every file under `start/`. If empty, say so and require `pixelforge new`.*

## 约束 Constraints

- **画布**：宽 × 高（例：`64` × `64`）。不要 `resize`，除非目标明确要求。
- **色板**：只用工程里已有索引；需要新色时先 `palette add` / `palette set`，禁止手改 JSON 塞 RGB。索引 0 为透明。
- **不可改区域**：（例：头部矩形 `(x,y,w,h)`；或「除 gear 层外」；或「无」。）用 `selection` / `--layer` 等方式自限；评审查保护区。
- **可用工具**：默认**仅**本地 `pixelforge` CLI，以及仓库 skill（`.cursor/skills/`，见根目录 `AGENTS.md` / `CLAUDE.md`）。不要改引擎源码，不要用手写脚本绕过 CLI 改像素（题面另有说明除外）。
- **其它**：（例：必须保留标签 `idle_down`；禁止 `direction fill --mode copy` 当最终步态；等。）

*Canvas, palette, protected regions, and tool policy. Default tools: pixelforge CLI + repo skills only.*

常用命令（按需选用，完整列表见 [docs/agent-skill.md](../../../docs/agent-skill.md)）：

```bash
pixelforge inspect --project hero.json
pixelforge preview --project hero.json --scale 4
pixelforge fill-rect --project hero.json --x 4 --y 8 --width 16 --height 40 --color-index 3
pixelforge selection set --project hero.json --x 10 --y 4 --width 12 --height 10
pixelforge replace-index --project hero.json --from-index 4 --to-index 6
pixelforge frame duplicate --project hero.json --index 0
pixelforge tag add --project hero.json --name walk_down --from 1 --to 3 --loop forward
pixelforge export sheet --project hero.json --output hero-sheet.png --cols 4 --atlas hero-sheet.json
pixelforge validate-atlas --atlas hero-sheet.json --sheet hero-sheet.png
```

## 交付物 Deliverables

交到运行目录的 `output/`（或评审指定的等价位置），文件名与相对路径如下：

| 文件 | 说明 |
| --- | --- |
| `output/hero.json` | （例）最终 `pixelforge-project` |
| `output/hero-sheet.png` | （例）若要求 sheet |
| `output/hero-sheet.json` | （例）若要求 `--atlas` |

<!-- 删掉用不到的行；路径与题面一致。 -->

*Name exact paths under `output/`. Omit rows you do not need.*
