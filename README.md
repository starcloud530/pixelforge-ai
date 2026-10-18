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

技术方向（不是搭建说明）：自建像素工作室。TypeScript 做可视化界面和桌面/网页外壳，Rust 做核心内核（像素网格、调色板、帧与时间轴数据、导出）；需要桌面包一层时可选 Tauri。不自训模型，外部 AI（例如 GPT-6 Astra）通过 Tool API 驱动这些工具。边界与非目标见 [docs/tech-stack.md](docs/tech-stack.md)。像素是网格和调色板上的真像素，不是扩散模型叠出来的假像素。

### 定位

Google Playground 与 Unity Spark 瞄准「文本 → 完整游戏」。我们瞄准更窄的一条：文本或参考 → 真像素精灵 → 动画 → 导出。信念相同——AI 会把创作交到更多人手里；切口更窄，也更能先交付。

工作室是同时做游戏和工具的独立小团队。

PixelForge AI follows an Aseprite-class pixel workflow, aimed at **L4 autopilot** for pixel assets:

- **True-pixel design** on a real pixel grid and palette.
- **Animation** for sprites.
- **Export** of engine-ready sprite assets.
- **L4 autopilot** — prompt or reference in; routine assets run end to end (true pixels, animation, engine-ready export) with minimal babysitting.

Stack direction, not a build guide: a self-built pixel studio. TypeScript is the visualization UI and desktop/web shell; Rust is the core kernel (pixel grid, palette, frame and timeline data, export). Tauri is optional when a desktop shell should wrap that kernel. We do not train models; external AI (for example GPT-6 Astra) drives these tools through Tool APIs. Boundaries and non-goals: [docs/tech-stack.md](docs/tech-stack.md). Pixels are real grid-and-palette pixels, not diffusion fake-pixel.

Google Playground and Unity Spark aim at text → full games. We aim at text or reference → true-pixel sprites → animation → export. Same belief that AI puts creation in more hands; a narrower wedge we can ship. An indie / small team building games and tools.

## 我们不是什么 What we are not

- **不是** 和 Aseprite 比拼完整界面的克隆竞赛。
- **不是** 只做「看起来像像素」的扩散图。
- **不是** 完整游戏引擎，也不以一句话生成整款游戏为目标。

- Not a race to clone the full Aseprite UI.
- Not fake-pixel diffusion only.
- Not a full game engine, and not text → a whole game.

## 状态 Status

早期，开发中（WIP）。

Early / work in progress.

## 开发 Development

本地骨架分成两层：Rust 内核在 `crates/pixelforge-kernel`（画布尺寸、索引像素、调色板、画笔/橡皮、帧，以及记录这些操作的撤销栈），TypeScript 外壳在 `apps/web`（Vite + React，最近邻显示索引格子）。`crates/pixelforge-kernel-wasm` 用 wasm-bindgen 把这份内核编进浏览器。构建和运行见 [docs/dev-setup.md](docs/dev-setup.md)。

外壳的画笔、橡皮、帧列表和撤销/重做都调用 wasm 里的同一份 Rust `Document`。色板编辑、每帧时长与洋葱皮、工程保存还没做。

Local skeleton: a Rust kernel in `crates/pixelforge-kernel` (canvas size, indexed pixels, palette, paint/erase, frames, and an undo stack of those operations) and a TypeScript shell in `apps/web` (Vite + React, nearest-neighbor view of the indexed grid). `crates/pixelforge-kernel-wasm` compiles that kernel into the browser with wasm-bindgen. Build and run: [docs/dev-setup.md](docs/dev-setup.md).

Paint, erase, the frame list, and undo/redo in the shell call the same Rust `Document` inside wasm. Palette editing, per-frame delay and onion skin, and project save are still open.

## 许可证 License

尚未选定（TBD）。本仓库目前不声明任何开源许可证。

Not chosen yet (TBD). This repository does not claim a license.
