# PixelForge

**AI-native true-pixel CLI and studio.**  
Prompt or reference → true pixels → animation → engine-ready export, with agents driving a local `pixelforge` CLI.

PixelForge 是 **AI 原生的真像素 CLI / 工作室**：外部 Agent 驱动本地 `pixelforge`，在真实像素网格与调色板上作画、做动画、导出引擎可用资源。

## What it is / 它是什么

- **True pixels** — indexed grid + palette, not diffusion “fake pixel”
- **Agent-first** — product path is Agent → local `pixelforge` CLI → Rust Tool API
- **Studio tools** — frames, layers, parts/anchors, sheets, atlas validation, tilemaps
- **真像素** — 索引格 + 调色板，不是扩散假像素
- **Agent 优先** — 产品路径是 Agent → 本地 CLI → Rust Tool API
- **工作室工具** — 帧、层、部件/锚点、精灵表、图集校验、tilemap

## What it is not / 它不是什么

- Not a full Aseprite UI clone  
- Not fake-pixel diffusion only  
- Not a full game engine / text → whole game  
- 不是完整 Aseprite 界面克隆，不是假像素扩散，不是完整游戏引擎

## Status / 状态

Early / WIP. Current prebuilt CLI: **[v0.1.0-rc.5](https://github.com/starcloud530/pixelforge-ai/releases/tag/v0.1.0-rc.5)** (prerelease). Installer scripts: `pixelforge-cli-installer.sh` / `.ps1`.

早期开发中。当前预编译：**v0.1.0-rc.5**（预发布）。用安装脚本，不要依赖 GitHub `latest`，直到正式版。

## Start here / 从这里开始

| | |
| --- | --- |
| [Install & quick start](install.md) | curl / PowerShell installer + 30s commands |
| [Releases](https://github.com/starcloud530/pixelforge-ai/releases) | Prebuilt binaries and installers |
| [Agent skill](agent-skill.md) | How agents use the CLI |
| [Tool API](tool-api.md) | Machine-readable tool catalog |
| [Known limits](known-limits.md) | Honest capability bounds |
| [Repository](https://github.com/starcloud530/pixelforge-ai) | Source, issues, examples |

Marketing landing (Pages root):  
**https://starcloud530.github.io/pixelforge-ai/**

Docs site (this MkDocs tree under `/docs/`):  
**https://starcloud530.github.io/pixelforge-ai/docs/**

## License / 许可证

Code: **MIT OR Apache-2.0**. Example art in the companion demos repo: **CC BY 4.0**. See [licensing](licensing.md).
