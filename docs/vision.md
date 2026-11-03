# Vision & positioning

Long-form product framing lived here so the repository README can stay a landing page. Stack boundaries stay authoritative in [tech-stack](tech-stack.md).

愿景与定位长文放在这里，根 README 保持落地页。技术边界以 [tech-stack](tech-stack.md) 为准。

## Vision / 愿景

Make professional true-pixel art and animation accessible to anyone with a game idea. AI handles the autopilot pipeline so creators focus on taste and direction.

让任何有游戏想法的人，都能做出专业级的真像素美术与动画。AI 负责自动驾驶管线，创作者把精力放在品味与方向上。

## Mission / 使命

Build an AI-native pixel studio that designs, animates, and exports engine-ready sprites with L4-level autonomy (end-to-end for routine assets), grounded in real pixel grids and palettes.

做一个 AI 原生的像素工作室：在真实像素网格与调色板约束上，设计、动画并导出引擎可用的精灵；对常规素材达到 L4 级自主——端到端完成，少人工看管。

## What we build / 我们在做什么

PixelForge follows an Aseprite-class pixel workflow, aimed at **L4 autopilot** for pixel assets:

- **True-pixel design** on a real pixel grid and palette
- **Animation** for sprites
- **Export** of engine-ready sprite assets
- **L4 autopilot** — prompt or reference in; routine assets run end to end with minimal babysitting

Stack direction (not a build guide): TypeScript for the visualization UI and desktop/web shell; Rust for the core kernel (pixel grid, palette, frame and timeline data, export); optional Tauri when a desktop shell should wrap that kernel. We do not train models. An external agent drives tools through the local `pixelforge` CLI and the Tool API. The one-turn panel in the web shell is a demo. Pixels are real grid-and-palette pixels, not diffusion fake-pixel.

Google Playground and Unity Spark aim at text → full games. We aim at text or reference → true-pixel sprites → animation → export. Same belief that AI puts creation in more hands; a narrower wedge we can ship.

## What we are not / 我们不是什么

- Not a race to clone the full Aseprite UI
- Not fake-pixel diffusion only
- Not a full game engine, and not text → a whole game

## Status / 状态

Early / work in progress. The M5 Tool API is a stable machine-readable surface in the Rust kernel; catalog: [tool-api](tool-api.md). The product path is an agent outside this repository calling the local `pixelforge` CLI. Usage: [agent-skill](agent-skill.md). The one-turn AI panel in the web shell is optional; see [ai-driver](ai-driver.md). An embedded multi-turn chat ([#14](https://github.com/starcloud530/pixelforge-ai/issues/14)) is not the product.
