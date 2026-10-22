# PixelForge AI

**AI 原生的真像素美术工作室。**  
提示或参考 → 真像素 → 动画 → 引擎可导出，常规素材尽量少看管。

*An AI-native true-pixel art studio.*  
*Prompt or reference → true pixels → animation → engine-ready export, with minimal babysitting.*

## 愿景 Vision

让任何有游戏想法的人，都能做出专业级的真像素美术与动画。AI 负责自动驾驶管线，创作者把精力放在品味与方向上。

Make professional true-pixel art and animation accessible to anyone with a game idea. AI handles the autopilot pipeline so creators focus on taste and direction.

## 使命 Mission

做一个 AI 原生的像素工作室：在真实像素网格与调色板约束上，设计、动画并导出引擎可用的精灵；对常规素材达到 L4 级自主——端到端完成，少人工看管。

Build an AI-native pixel studio that designs, animates, and exports engine-ready sprites with L4-level autonomy (end-to-end for routine assets), grounded in real pixel grids and palettes.

## 我们在做什么 What we build

PixelForge AI 走的是 Aseprite 这一类的像素工作流，目标是像素素材的 **L4 自动驾驶**：

- **真像素设计** — 在真实像素网格和调色板约束里作画。
- **动画** — 为精灵做动画。
- **导出** — 产出引擎可直接使用的精灵资源。
- **L4 自动驾驶** — 提示或参考进去，常规素材端到端走完：真像素、动画、引擎就绪导出，尽量少看管。

技术方向（不是搭建说明）：自建像素工作室。TypeScript 做可视化界面和桌面/网页外壳，Rust 做核心内核（像素网格、调色板、帧与时间轴数据、导出）；需要桌面包一层时可选 Tauri。不自训模型。外部 Agent（Codex、Claude Code，或别的能跑命令的模型）通过本地 `pixelforge` CLI 调用 Tool API 驱动这些工具。网页里的一轮面板只是演示。边界与非目标见 [docs/tech-stack.md](docs/tech-stack.md)。像素是网格和调色板上的真像素，不是扩散模型叠出来的假像素。

### 定位

Google Playground 与 Unity Spark 瞄准「文本 → 完整游戏」。我们瞄准更窄的一条：文本或参考 → 真像素精灵 → 动画 → 导出。信念相同——AI 会把创作交到更多人手里；切口更窄，也更能先交付。

工作室是同时做游戏和工具的独立小团队。

PixelForge AI follows an Aseprite-class pixel workflow, aimed at **L4 autopilot** for pixel assets:

- **True-pixel design** on a real pixel grid and palette.
- **Animation** for sprites.
- **Export** of engine-ready sprite assets.
- **L4 autopilot** — prompt or reference in; routine assets run end to end (true pixels, animation, engine-ready export) with minimal babysitting.

Stack direction, not a build guide: a self-built pixel studio. TypeScript is the visualization UI and desktop/web shell; Rust is the core kernel (pixel grid, palette, frame and timeline data, export). Tauri is optional when a desktop shell should wrap that kernel. We do not train models. An external agent (Codex, Claude Code, or any model that can run a command) drives these tools through the local `pixelforge` CLI and the Tool API. The one-turn panel in the web shell is a demo. Boundaries and non-goals: [docs/tech-stack.md](docs/tech-stack.md). Pixels are real grid-and-palette pixels, not diffusion fake-pixel.

Google Playground and Unity Spark aim at text → full games. We aim at text or reference → true-pixel sprites → animation → export. Same belief that AI puts creation in more hands; a narrower wedge we can ship. An indie / small team building games and tools.

## 我们不是什么 What we are not

- **不是** 和 Aseprite 比拼完整界面的克隆竞赛。
- **不是** 只做「看起来像像素」的扩散图。
- **不是** 完整游戏引擎，也不以一句话生成整款游戏为目标。

- Not a race to clone the full Aseprite UI.
- Not fake-pixel diffusion only.
- Not a full game engine, and not text → a whole game.

## 状态 Status

早期，开发中（WIP）。M5 Tool API 已在 Rust 内核提供稳定的机器可读接口，目录见 [docs/tool-api.md](docs/tool-api.md)。产品路径是仓库外的 Agent 调用本地 `pixelforge` CLI，CLI 再把 Tool API JSON 交给内核。用法见 [docs/agent-skill.md](docs/agent-skill.md)。网页里的一轮 AI 面板是可选演示，见 [docs/ai-driver.md](docs/ai-driver.md)。嵌在应用里的多轮对话（[#14](https://github.com/starcloud530/pixelforge-ai/issues/14)）不是产品。

Early / work in progress. The M5 Tool API is a stable machine-readable surface in the Rust kernel; the catalog is [docs/tool-api.md](docs/tool-api.md). The product path is an agent outside this repository calling the local `pixelforge` CLI, which forwards Tool API JSON to the kernel. Usage: [docs/agent-skill.md](docs/agent-skill.md). The one-turn AI panel in the web shell is an optional demo; see [docs/ai-driver.md](docs/ai-driver.md). An embedded multi-turn chat ([#14](https://github.com/starcloud530/pixelforge-ai/issues/14)) is not the product.

## Agent Skills

Cursor 进仓库后按 `description` 加载 `.cursor/skills/<name>/SKILL.md`。创作类 skill 先遵守 `pixelforge` 的索引色不变量。Skill 补观改流程，不代替专业画师的审美。命令速查在 [docs/agent-skill.md](docs/agent-skill.md)。本仓库不把同一份正文复制到 `.agents/skills/` 或 `.claude/skills/`；Codex 与 Claude Code 从 `AGENTS.md` / `CLAUDE.md` 打开这些文件。

*Cursor matches `description` under `.cursor/skills/<name>/SKILL.md`. Creative skills follow the `pixelforge` invariants first. Skills add a look-then-revise loop; they do not replace a pixel artist's judgment. Command reference: [docs/agent-skill.md](docs/agent-skill.md). There is no second copy under `.agents/skills/` or `.claude/skills/`.*

| Skill | 说明 |
| --- | --- |
| `pixelforge` | 核心。安装与运行、索引色不变量、退出码、`inspect` / `tools` / `call`。其它创作 skill 先遵守这里。 |
| `pixelforge-draw` | 从零画角色、立绘、剪影。剪影（单色）→ 色块 → 细节，每阶段 `preview --scale` 与 ASCII。禁止一像素刷屏，读图要查破洞、头身和体积断点。 |
| `pixelforge-sprite-4dir` | 四向走 + 闲精灵表。行是朝向，列是 idle 加 walk。`export sheet --cols`、跨帧 `copy-rect`、`diff` 查脚滑、`hflip` 后修高光。 |
| `pixelforge-layers` | 身体、装备、四肢分层。合成预览、绘制 `--layer`、`layer flatten`。 |
| `pixelforge-critique` | 只读检查：剪影、头身、侧向深度、腿体积、脚接触、色阶。必须列出缺陷，不能只说看起来不错。 |

## 开发 Development

本地骨架分成三层：Rust 内核在 `crates/pixelforge-kernel`（画布尺寸、索引像素、调色板、画笔/橡皮/矩形填充、矩形擦除、矩形复制与移动、帧的增删复制，以及记录这些操作的撤销栈），本地 Agent 入口是 `crates/pixelforge-cli` 的 `pixelforge` 命令，TypeScript 外壳在 `apps/web`（Vite + React，最近邻显示索引格子）。`crates/pixelforge-kernel-wasm` 用 wasm-bindgen 把这份内核编进浏览器。构建和运行见 [docs/dev-setup.md](docs/dev-setup.md)。CLI 的配方见 [docs/agent-skill.md](docs/agent-skill.md)。

外壳的画笔、橡皮、调色板编辑、帧列表（新增空白帧、复制当前帧、删除当前帧且至少留一帧）、帧时间轴（拖动选帧、每帧时长、循环播放）、撤销/重做、工程打开/保存，以及透明 PNG 序列和网格 sprite sheet 导出，都调用 wasm 里的同一份 Rust `Document`。画笔只写入调色板索引。色板可以锁定（写入工程文件），也可以从 GIMP 调色板（`.gpl`）或十六进制列表导入，替换或合并进当前色板，并记在撤销栈上。Agent 不走这些手势：首选是 `pixelforge` 命令，它打开磁盘上的工程文件并调用 `Document::call_tool`。网页里的 `callTool` 是同一入口。目录见 [docs/tool-api.md](docs/tool-api.md)。洋葱皮把前一帧和后一帧半透明叠在当前帧下面，方便补间；循环播放时先藏起，停下后再显示。它只存在于视图，不写入像素、工程文件或 PNG。导入 Godot / Unity 的约定见 [docs/export.md](docs/export.md)。格式说明见 [docs/dev-setup.md](docs/dev-setup.md)。

Local skeleton: a Rust kernel in `crates/pixelforge-kernel` (canvas size, indexed pixels, palette, paint/erase/rectangle fill/rectangle erase/rectangle copy and move, frame add/delete/duplicate, per-frame delay, and an undo stack of those operations), the local agent entry `pixelforge` in `crates/pixelforge-cli`, and a TypeScript shell in `apps/web` (Vite + React, nearest-neighbor view of the indexed grid). `crates/pixelforge-kernel-wasm` compiles that kernel into the browser with wasm-bindgen. Build and run: [docs/dev-setup.md](docs/dev-setup.md). CLI recipes: [docs/agent-skill.md](docs/agent-skill.md).

Paint, erase, palette edits, the frame list (add a blank frame, duplicate the current frame, delete the current frame but never below one), the frame timeline (scrub, per-frame delay, and loop playback), undo/redo, project open/save, and transparent PNG-sequence plus grid sprite-sheet export in the shell call the same Rust `Document` inside wasm. Paint writes palette indices. The palette can be locked (the flag is stored in the project file) and imported from a GIMP palette (`.gpl`) or a hex list, replacing or merging into the document palette as one undo step. An agent does not go through those gestures. The preferred entry is the `pixelforge` command: it opens the project file from disk and calls `Document::call_tool`. Wasm `callTool` is the same entry. Catalog: [docs/tool-api.md](docs/tool-api.md). Onion skin composites the previous and next frames under the current frame at partial opacity. Loop playback hides that overlay and shows it again when playback stops. The overlay is view-only: it is not written into pixels, the project file, or PNG export. Godot / Unity import conventions: [docs/export.md](docs/export.md). Palette file formats: [docs/dev-setup.md](docs/dev-setup.md).

## 许可证 License

尚未选定（TBD）。本仓库目前不声明任何开源许可证。

Not chosen yet (TBD). This repository does not claim a license.
