# 技术栈 Tech Stack

**状态：已锁定。** 本文是 PixelForge AI 的技术选型权威说明。早期规划里「Godot 编辑器 + Python 推理服务」的写法作废；里程碑简介或 issue 标题若仍出现这些词，实现以本文为准。

*Status: locked. This document is the authoritative tech-stack decision for PixelForge AI. Earlier planning text that named a Godot editor plus a Python inference service is superseded, including milestone blurbs or issue titles that still use those words.*

## 产品前提 Product premise

PixelForge AI 是自建的像素工作室平台与工具：画布像素、调色板、动画时间轴、导出。

文档模型是离散像素网格和索引色。工作室交付工具和平台，不交付一个训练好的模型，也不交付游戏引擎。

*PixelForge AI is a self-built pixel studio platform and its tools: canvas pixels, palette, animation timeline, and export.*

*The document model is a discrete pixel grid and an indexed palette. The studio ships tools and a platform. It does not ship a trained model, and it does not ship a game engine.*

## 选型 Choice

| 层 Layer | 技术 Technology | 职责 Responsibility |
| --- | --- | --- |
| 可视化界面 / 桌面与网页外壳 | **TypeScript** | 画布视图、调色板面板、时间轴与播放预览、撤销交互、网页或桌面外壳 |
| 核心内核 | **Rust** | 像素网格、调色板、帧与时间轴数据、工程文件、导出、Tool API |
| 本地 Agent 入口 | **`pixelforge` CLI**（Rust） | 打开磁盘上的 `pixelforge-project`，调用 Tool API，向 stdout 打印 JSON。对话留在仓库外的 Agent |
| 桌面封装（可选） | **Tauri** | 当桌面外壳需要包住 Rust 内核时使用；打包方式，不是第二套核心 |
| 智能 | **外部 AI**（例如 GPT-6 Astra） | 通过 Tool API 驱动内核；模型留在提供方 |

明确排除：

- Godot、GDScript、C# 不进入编辑器或内核。
- Python AI 服务不是主路径。不自建推理服务来夹在模型和编辑器之间。
- Agent 入口是 `pixelforge` CLI，不是应用内聊天，也不是 MCP 服务器。

*Out of the stack: Godot, GDScript, and C# do not enter the editor or the kernel. A Python AI service is not the primary path. We do not stand up our own inference service between the model and the editor. The agent entry is the `pixelforge` CLI, not an in-app chat and not an MCP server.*

## 为什么 Why

### Rust 内核

像素网格、索引色、帧、时间轴和导出是确定性的数据与算法。内核用 Rust，界面上的手绘和 Agent 的工具调用执行同一份实现。

索引帧缓冲、导出和洋葱皮共用的 RGBA 解析，以及 sprite sheet 排布留在内核里。洋葱皮的半透明叠加是外壳上的视图，不写回这些缓冲。工程文件的读写和 Tool API 都从这份内核长出来，界面和自动化共用一个数据源。

*The pixel grid, indexed color, frames, timeline, and export are deterministic data and algorithms. A Rust kernel runs both manual edits and agent tool calls.*

*Indexed frame buffers, the RGBA resolution shared by export and onion skin, and sprite-sheet packing stay in the kernel. The semi-transparent onion composite is a shell view and is not written back into those buffers. Project I/O and the Tool API grow from that same kernel, so the UI and automation share one source of truth.*

### TypeScript 外壳

画布视图、调色板、时间轴 scrub、循环预览和撤销交互在 TypeScript 里迭代。预览用最近邻缩放把格子画清楚；像素的增删改交给 Rust。

同一套界面可以跑在网页里，也可以放进桌面外壳。

*Canvas view, palette, timeline scrubbing, loop preview, and undo interaction iterate in TypeScript. Preview uses nearest-neighbor scaling so cells stay crisp; pixel mutations go to Rust.*

*The same UI can run on the web or inside a desktop shell.*

### 可选 Tauri

M9 若要可安装的桌面演示，用 Tauri 把 TypeScript 外壳和 Rust 内核包在一起。Tauri 不改变文档模型，也不取代内核。没有桌面壳时，网页外壳仍然是同一套界面。

*If M9 needs an installable desktop demo, Tauri wraps the TypeScript shell and the Rust kernel. Tauri does not change the document model and does not replace the kernel. Without a desktop shell, the web shell is still the same UI.*

### 外部 AI

产品是工具和平台。仓库外的 Agent（Codex、Claude Code，或 GPT-6 Astra 这类模型）负责理解意图和多轮修订，通过本地 `pixelforge` CLI 调用 Tool API，改像素、换色板、加帧、导出。训练、权重和推理留在模型提供方。对话也留在 Agent 那里。网页外壳里的一轮面板是可选演示，不是这条路径。

*The product is tools and a platform. An agent outside this repository (Codex, Claude Code, or a model such as GPT-6 Astra) interprets intent and multi-turn revisions, then changes pixels, palettes, frames, and export by running the local `pixelforge` CLI against the Tool API. Training, weights, and inference stay with the model provider. The conversation stays with the agent. The one-turn panel in the web shell is an optional demo, not this path.*

## 边界 Boundaries

### TypeScript 拥有什么

- 指针、键盘、面板布局。
- 把内核状态画成像素视图、调色板和时间轴。
- 洋葱皮与播放的显示层。
- 把一次手势翻译成一次内核操作（画、擦、选色、改帧时长、撤销）。

*TypeScript owns pointer and keyboard input, panel layout, drawing kernel state as the pixel view, palette, and timeline, the onion-skin and playback display, and translating a gesture into one kernel operation (draw, erase, pick a color, edit frame delay, undo).*

### Rust 拥有什么

- 画布尺寸、索引像素、调色板、帧列表、每帧时长、循环标记。
- 绘制、橡皮、矩形填充、换色、帧增删复制、调色板重映射。
- 工程文件打开与保存。
- PNG 序列与网格 sprite sheet 导出。
- Tool API：稳定、机器可读、与 UI 操作同一数据源。

*Rust owns canvas size, indexed pixels, palette, the frame list, per-frame delay, loop flags, paint, erase, rectangle fill, rectangle erase, rectangle copy and move, recolor, frame add/delete/duplicate, palette remap, project open/save, PNG-sequence and grid sprite-sheet export (including nearest-neighbor scale), and the Tool API — stable, machine-readable, and the same data source the UI uses.*

### 共享事实

界面显示的像素必须来自内核。Agent 不得绕过内核直接改界面上的缓冲区。撤销栈记录的是内核操作。

*Pixels on screen come from the kernel. An agent does not patch a UI buffer past the kernel. The undo stack records kernel operations.*

### 外部 AI 拥有什么

意图、对话和工具调用的编排。它不拥有像素真相，也不成为仓库里的主服务。

*Intent, dialogue, and orchestration of tool calls. It does not own pixel truth, and it is not a primary service in this repository.*

## 外部 AI External AI

我们不训练模型。

外部 Agent（例如 Codex、Claude Code、GPT-6 Astra，以及日后接入的真像素或像素化模型）是 Tool API 的调用方。一次修订的闭环是：

1. Agent 读 [agent-skill.md](agent-skill.md)，按任务打开 `.cursor/skills/` 里对应的 skill，对磁盘上的工程文件运行 `pixelforge`。
2. CLI 把 Tool API JSON 交给 `Document::call_tool`。Rust 内核执行，并写回工程文件。
3. 人可以在 TypeScript 外壳里打开同一份工程，继续画，或撤销文件里还留得住的那一步。

终稿必须落在内核的索引色网格上。只是「看起来像像素」的图像不能当作终稿写回工程。

网页里的一轮 AI 面板可以演示同一次 `call_tool`，它不是 Agent 的主路径。不在应用内做多轮对话。MCP 不在这一阶段；以后若要接，只做这支 CLI 的薄封装，不另写一套像素路径。

*We do not train models.*

*An external agent (for example Codex, Claude Code, GPT-6 Astra, and later true-pixel or pixelation models) is a caller of the Tool API. One revision loop is: the agent reads [agent-skill.md](agent-skill.md), opens the matching skill under `.cursor/skills/`, and runs `pixelforge` on a project file; the CLI passes Tool API JSON to `Document::call_tool`, and the Rust kernel applies it and writes the project; a person can open that same file in the TypeScript shell, keep drawing, or undo a step the file still holds.*

*Final pixels land on the kernel's indexed grid. An image that only looks pixelated is not written back as the finished asset.*

*The one-turn AI panel in the web shell can demo the same `call_tool`. It is not the primary agent path. An in-app multi-turn chat is not the product. MCP is out of this stage. A later wrapper, if any, would be a thin layer over this CLI, not a second pixel path.*

## 非目标 Non-goals

- **Godot fork。** 不 fork、不内嵌 Godot，不用 GDScript 或 C# 做编辑器。工作室不是游戏引擎的场景编辑器。
- **SVG 当作引擎。** 矢量不是文档模型、动画模型或导出真相。若将来接受矢量，只作为参考或导入，写回前必须栅格化到索引色网格。
- **自训与 Python 主服务。** 不训练自有模型，不以 Python 推理服务作为主架构。
- **完整游戏引擎，或一句话生成整款游戏。** 与 README 中的产品边界一致。
- **应用内多轮对话。** 不在外壳里做 [#14](https://github.com/starcloud530/pixelforge-ai/issues/14) 那种嵌入式聊天。外部 Agent 自己保留对话，通过 CLI 改工程。
- **MCP 服务器。** 这一阶段不做。若以后要接，只做 `pixelforge` CLI 的薄封装。

导出物可以被 Godot、Unity 等引擎导入。导入约定写在导出文档里，那是消费端约定，不是编辑器技术栈。

*Non-goals: a Godot fork (no embedded Godot, no GDScript or C# editor); SVG as the engine (vectors are not the document, animation, or export source of truth; a future vector import would rasterize onto the indexed grid before it is stored); training our own models or a Python inference service as the primary architecture; a full game engine or text-to-a-whole-game; an in-app multi-turn chat ([#14](https://github.com/starcloud530/pixelforge-ai/issues/14) — the external agent keeps the conversation and edits the project through the CLI); an MCP server in this stage (a later wrapper would only call the `pixelforge` CLI).*

*Exported files may be imported by engines such as Godot or Unity. Those import conventions belong in export docs. They are a consumer contract, not the editor stack.*

## 与 9 个月路线图的关系 Roadmap

路线图的产品目标不变（2026-10 至 2027-07）。变的是每一阶段落在哪一层。下表是实现约束，不新开范围。

*Product goals on the roadmap stay the same (2026-10 through 2027-07). What changes is which layer carries each phase. The table below is an implementation constraint. It does not add scope.*

| 阶段 Phase | 窗口 Window | Rust 内核 | TypeScript 外壳 | AI |
| --- | --- | --- | --- | --- |
| M1–M2 编辑器底座 | 2026-10 → 2026-12 | 固定网格、调色板锁定与索引色、帧列表增删复制、工程打开/保存 | 画笔、橡皮、撤销重做、缩放视图 | 不接入。人能手搓 16×16 / 32×32 并保存 |
| M3–M4 动画与导出 | 2027-01 → 2027-02 | 每帧时长、循环标记、PNG 序列与网格 sprite sheet | 时间轴 scrub、洋葱皮、循环预览 | 不接入。走路循环可预览并导出引擎可用 sheet |
| M5–M6 AI 驾驶员 L2–L3 | 2027-03 → 2027-04 | Tool API：改像素、换色板、加帧、导出；与 UI 同一数据源 | 每步预览，失败可人工接管同一工程 | 外部模型（含 GPT-6 Astra）只发工具调用。不建 Python 推理服务，不做 Godot 桥 |
| M7–M8 L4 自动驾驶 + 游戏验证 | 2027-05 → 2027-06 | 同一 Tool API 上完成生成结果落格、调色板约束、补帧、导出 | 关键节点可人审 | 外部真像素或像素化模型作为调用方。验收是丧尸/据点道具精灵包，游戏原型消费导出物 |
| M9 打磨与交付 | 2027-07 | 内核放进安装包 | 安装与运行说明；可选 Tauri 桌面封装 | 沿用已有外部调用，本阶段不新增模型栈。许可证与对外说明另计 |

M1–M2 的验收仍然是手绘闭环，不提前做自动驾驶。M5 之后所有写像素的路径，包括模型和界面，都进 Rust Tool API。仓库外的 Agent 走 `pixelforge` CLI。网页里的一轮面板只是演示。

*M1–M2 still accepts a manual drawing loop. Autopilot waits. From M5 on, every path that writes pixels — model or UI — goes through the Rust Tool API. An agent outside the repository uses the `pixelforge` CLI. The one-turn panel in the web shell is only a demo.*

## 一句话 In one line

TypeScript 给人看，Rust 记像素，外部 Agent 通过本地 CLI 调工具。

*TypeScript shows the work, Rust stores the pixels, and an external agent calls the tools through the local CLI.*
