<p align="center">
  <strong>PixelForge</strong>
</p>

<h3 align="center">AI 原生的真像素 CLI / 工作室</h3>

<p align="center">
  提示或参考 → 真像素 → 动画 → 引擎可导出。<br/>
  外部 Agent 驱动本地 <code>pixelforge</code>，在真实索引网格上作画——不是扩散假像素。
</p>

<p align="center">
  <a href="https://github.com/starcloud530/pixelforge-ai/releases/tag/v0.1.0-rc.4"><img alt="Release" src="https://img.shields.io/badge/release-v0.1.0--rc.4-blue" /></a>
  <a href="https://starcloud530.github.io/pixelforge-ai/"><img alt="Site" src="https://img.shields.io/badge/site-GitHub%20Pages-teal" /></a>
  <a href="https://starcloud530.github.io/pixelforge-ai/"><img alt="Docs" src="https://img.shields.io/badge/docs-MkDocs-blue" /></a>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-green" /></a>
  <a href="https://github.com/starcloud530/pixelforge-ai/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/starcloud530/pixelforge-ai/actions/workflows/ci.yml/badge.svg" /></a>
</p>

<p align="center">
  <a href="README.md">English</a> | <b>中文</b>
</p>

<p align="center">
  <img alt="丧尸四向行走" src="https://resources.pixelforge.torzsi.com/demo-zombie/v1/a/walk-four-directions.gif" width="420" />
</p>

## 为什么是 PixelForge

- **真像素** — 索引格 + 调色板，Agent 与人改的是同一份格子
- **Agent 优先** — Cursor / Codex / Claude Code 等调用本地 `pixelforge`；网页里的一轮面板只是演示
- **引擎可导出** — PNG 序列、精灵表、图集 JSON、校验门槛
- **不是**完整 Aseprite 克隆，**不是**假像素扩散，**不是**完整游戏引擎

状态：早期 / WIP。预编译 CLI **[v0.1.0-rc.4](https://github.com/starcloud530/pixelforge-ai/releases/tag/v0.1.0-rc.4)** 为预发布。愿景与 L4 长文：[docs/vision.md](docs/vision.md)。能力边界：[docs/known-limits.md](docs/known-limits.md)。

## 快速开始

安装脚本把 `pixelforge` 放到 `~/.pixelforge/bin`。正式版之前请用 **rc.4** 标签（不要用 `latest`）。

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/starcloud530/pixelforge-ai/releases/download/v0.1.0-rc.4/pixelforge-cli-installer.sh | sh
```

```powershell
powershell -ExecutionPolicy Bypass -c \
  "irm https://github.com/starcloud530/pixelforge-ai/releases/download/v0.1.0-rc.4/pixelforge-cli-installer.ps1 | iex"
```

```bash
pixelforge --version
pixelforge new --width 16 --height 16 --output hero.json
pixelforge preview --project hero.json --scale 4
```

手动压缩包、平台说明与源码构建：**[安装文档](https://starcloud530.github.io/pixelforge-ai/install/)** · [Releases](https://github.com/starcloud530/pixelforge-ai/releases/tag/v0.1.0-rc.4)

## 站点与文档

宣传落地页：

**https://starcloud530.github.io/pixelforge-demos/**

产品文档（本仓 Pages · MkDocs Material）：

**https://starcloud530.github.io/pixelforge-ai/**

| 主题 | 链接 |
| --- | --- |
| 安装与快速开始 | [install](https://starcloud530.github.io/pixelforge-ai/install/) |
| Agent skill / CLI 配方 | [agent-skill](https://starcloud530.github.io/pixelforge-ai/agent-skill/) |
| Tool API 目录 | [tool-api](https://starcloud530.github.io/pixelforge-ai/tool-api/) |
| 导出与引擎导入 | [export](https://starcloud530.github.io/pixelforge-ai/export/) |
| 项目清单 | [project-manifest](https://starcloud530.github.io/pixelforge-ai/project-manifest/) |
| 技术栈 | [tech-stack](https://starcloud530.github.io/pixelforge-ai/tech-stack/) |
| 已知限制 | [known-limits](https://starcloud530.github.io/pixelforge-ai/known-limits/) |
| 许可证 | [licensing](https://starcloud530.github.io/pixelforge-ai/licensing/) |

正文源文件在 [`docs/`](docs/)。本地预览：

```bash
pip install -r requirements-docs.txt
mkdocs serve
```

若合入 `main` 后文档 URL 仍 404，请一次性启用：**Settings → Pages → Build and deployment → Source = GitHub Actions**。

## Agent 技能

Cursor 按 `description` 加载 `.cursor/skills/<name>/SKILL.md`。创作类 skill 先遵守 `pixelforge` 的索引色不变量。已安装的 `pixelforge` 可用 `pixelforge skills list` 和 `pixelforge skills read <name>` 列出并读取同一套打包 skill。命令速查：[agent-skill](https://starcloud530.github.io/pixelforge-ai/agent-skill/)。

| Skill | 作用 |
| --- | --- |
| `pixelforge` | 核心 CLI、Tool API、最少编辑路径 |
| `pixelforge-draw` | 剪影 → 色块 → 细节 |
| `pixelforge-sprite-4dir` | 四向闲 + 走精灵表 |
| `pixelforge-layers` | body / gear / limbs |
| `pixelforge-critique` | 只读列出缺陷 |
| `pixel-art-craft` | 走路、机位、运动轨迹工艺（公开教程链接） |

## 展示媒体

CDN（大媒体不要提交进 git）：

- [四向行走 GIF](https://resources.pixelforge.torzsi.com/demo-zombie/v1/a/walk-four-directions.gif)
- [Hero 前后对比](https://resources.pixelforge.torzsi.com/demo-zombie/v1/storyboard/hero-before-after.png)
- [七阶段时间线](https://resources.pixelforge.torzsi.com/demo-zombie/v1/storyboard/timeline.png)
- [Sheet 组装短片](https://resources.pixelforge.torzsi.com/demo-zombie/v1/videos/01-sheet-assembly.mp4)

宣传站：[pixelforge-demos Pages](https://starcloud530.github.io/pixelforge-demos/)。案例与评测：[pixelforge-demos](https://github.com/starcloud530/pixelforge-demos)。**不要**在本仓再加 `examples/`。

## 开发

只用预编译的人可跳过。源码 / 网页外壳 / CI：[dev-setup](https://starcloud530.github.io/pixelforge-ai/dev-setup/)。

```bash
cargo test -p pixelforge-kernel
cargo test -p pixelforge-cli
```

## 许可证

代码：**MIT OR Apache-2.0**（[LICENSE](LICENSE)、[LICENSE-MIT](LICENSE-MIT)、[LICENSE-APACHE](LICENSE-APACHE)）。  
依赖核对：[docs/licensing.md](docs/licensing.md)。
