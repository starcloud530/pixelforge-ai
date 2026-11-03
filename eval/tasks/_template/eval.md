# 验收：`<tier>-<slug>` Eval：`<tier>-<slug>`

<!--
  给评审 / 机判看。不要发给 Agent。
  与 task.md 同目录。可机判条目应对得上可选的 check.py。
-->

## 必须满足 Must pass

每条独立判 **pass / fail**。全部 pass 才算机判门槛通过；任一条 fail 则本题机判未通过。

1. **尺寸**：`inspect`（或读工程）显示宽高为 `W` × `H`（与 `task.md` 一致）。
2. **色板不越界**：像素索引均落在 `0..=(palette.len-1)`；无非法索引导致工程打不开。
3. **帧标签齐全**：（例）存在 `idle_down` / `walk_down` / …；或「本题无标签要求 → 删掉本条」。
4. **校验通过**：（按题勾选，命令须真实存在）
   - `pixelforge validate-atlas --atlas … --sheet …` 退出码 `0`
   - `pixelforge direction validate --project …` 退出码 `0`
   - `pixelforge validate-tiles --project … --map … --rules …` 退出码 `0`
   - `pixelforge project validate --manifest …` 退出码 `0`
   - 或其它题面写明的只读校验；**退出码 0 ≠ 观感合格**
5. **受保护区域不变**：（例）相对 `start/hero.json`，保护区矩形 / 层上 `diff` 无变化；或「无保护区 → 删掉本条」。

<!-- 按题增删条目；每条必须可写成 score.json machine.items[].id。 -->

*Each item is independently pass/fail. Validator exit 0 is not an aesthetic pass.*

## 加分项 Bonus

不计入「必须满足」是否通过；记入 `score.json` → `bonus`。

- **步数**：`trajectory.jsonl` 行数不超过 N（填写）；或同效编辑更短。
- **无多余撤销**：无连续无效 `undo` / `redo`；无「改错再整段重来」的明显空转。
- **无多余报错**：轨迹中非预期的退出码 `1` / `2` 次数为 0（或 ≤ 题面允许的探测性失败）。

*Bonus only. Do not mix into the must-pass gate.*

## 人工 / 模型判定 Human / model judgment

单独打分，写入 `score.json` → `human`，**不要**与机判加总。

建议量表 `1–5`（或题面约定），关注：

- 剪影是否可读、头身 / 体积是否明显崩坏；
- 局部改色 / 换装是否仍像同一角色；
- 多帧时脚滑、闪烁、标签区间是否「看起来」在动（机判管不了的部分）。

评审可对照 `pixelforge preview --project … --scale 4` 或 `export apng --tag …` 的预览；观感结论只写在 `human.notes` / `human.score`。

*Taste score stays in `human`. Do not blend with `machine`.*

## 预期边界 Expected boundary

<!-- 若本题用于能力边界探测，写明预期失败点；否则写「无，常规达标题」。 -->

- **本题是否压边界**：（是 / 否）
- **预期失败点**：（例）Agent 可能以为 `direction fill --mode mirror` 会修好高光；或试图要八向对角自动填充；或期望未绑定 `polish` 随 `bake` 移动。——对照仓库 README / [docs/agent-skill.md](../../../docs/agent-skill.md)「已知限制」。
- **怎样算按预期撞墙**：（例）最终 `direction validate` 仍缺对角标签且题面要求八向；或保护区被改但仍声称完成。
- **不要写进 task.md**：Agent 不应提前看到本小节。

*Boundary probing only. Keep spoilers out of task.md.*

## 机判方式 How to check

- （推荐）实现同目录 `check.py`：读 `output/`（及需要时的 `start/`），对「必须满足」逐项输出 pass/fail；由运行器写入 `score.json` → `machine`。
- （当前）无运行器时：评审按本文件手工执行对应 CLI，例如：

```bash
pixelforge inspect --project output/hero.json
pixelforge preview --project output/hero.json --scale 4
pixelforge validate-atlas --atlas output/hero-sheet.json --sheet output/hero-sheet.png
pixelforge direction validate --project output/hero.json
pixelforge project validate --manifest output/pixelforge.manifest.json
```

保护区是否被改：对照 `start/` 与 `output/` 的对应工程（由 `check.py` 逐格比较，或评审人工核对）；`pixelforge diff` 只比较**同一工程**里的两帧（`--frame` / `--with`），不能直接对两个文件做保护区机判。

（删掉与本题无关的命令；参数换成题面真实路径。）

模板目录**不附带**可执行的 `check.py`；正式题目若需要机判再添加，并在本小节写「运行：`python3 check.py --output …`」。

*Point at check.py when present; otherwise document the exact manual CLI checks. Do not invent flags.*
