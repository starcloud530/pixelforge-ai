<p align="center">
  <strong>PixelForge</strong>
</p>

<h3 align="center">AI 原生的真像素 CLI / 工作室</h3>

<p align="center">
  用 Agent 或脚本在索引色网格上作画，编辑图层和动画帧，再导出精灵表与图集 JSON。
</p>

<p align="center">
  <a href="https://starcloud530.github.io/pixelforge-ai/install/"><img alt="Download PixelForge" src="https://img.shields.io/badge/download-PixelForge-blue" /></a>
  <a href="https://starcloud530.github.io/pixelforge-demos/"><img alt="PixelForge Studio" src="https://img.shields.io/badge/studio-PixelForge-teal" /></a>
  <a href="https://starcloud530.github.io/pixelforge-ai/"><img alt="Docs" src="https://img.shields.io/badge/docs-MkDocs-blue" /></a>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-green" /></a>
  <a href="https://github.com/starcloud530/pixelforge-ai/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/starcloud530/pixelforge-ai/actions/workflows/ci.yml/badge.svg" /></a>
</p>

<p align="center">
  <a href="README.md">English</a> | <b>中文</b>
</p>

<p align="center">
  <a href="https://starcloud530.github.io/pixelforge-demos/gallery/#tidal-glasshouse"><img alt="潮汐温室，跨过木桥可见水岸上的玻璃温室与门灯" src="https://resources.pixelforge.torzsi.com/studio/rc6/1bacf12e29c69944/tidal-glasshouse.png?rev=fb2d394c870ececb" width="416" /></a>
</p>

## 用 PixelForge 制作的作品

这六套作品用本地 CLI 绘制。打开作品可切换视图、逐帧查看动画，或下载可编辑的源工程。

| 作品 | 可以检查什么 |
| --- | --- |
| [潮汐温室](https://starcloud530.github.io/pixelforge-demos/gallery/#tidal-glasshouse) | 51 个图块组成的五层地图 |
| [雪岭邮差](https://starcloud530.github.io/pixelforge-demos/gallery/#alpine-courier) | 四个朝向、20 帧 |
| [星芒风候仪](https://starcloud530.github.io/pixelforge-demos/gallery/#observatory-machine) | 启动、采样与待机 |
| [潮汐科考道具](https://starcloud530.github.io/pixelforge-demos/gallery/#tide-survey-icons) | 八枚背包图标，两套配色 |
| [港口守灯人](https://starcloud530.github.io/pixelforge-demos/gallery/#port-keeper) | 四向行走与分层换装 |
| [哪吒逐帧动作](https://starcloud530.github.io/pixelforge-demos/gallery/#nezha) | 12 帧动作分解 |

## 快速开始

安装 macOS、Windows 或 Linux 版本。安装脚本把 `pixelforge` 放到 `~/.pixelforge/bin`。

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/starcloud530/pixelforge-ai/releases/download/v0.1.0-rc.6/pixelforge-cli-installer.sh | sh
```

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/starcloud530/pixelforge-ai/releases/download/v0.1.0-rc.6/pixelforge-cli-installer.ps1 | iex"
```

```bash
pixelforge --version
pixelforge new --width 16 --height 16 --output hero.json
pixelforge preview --project hero.json --scale 4
```

手动压缩包、平台说明与源码构建：**[安装文档](https://starcloud530.github.io/pixelforge-ai/install/)** · [Releases](https://github.com/starcloud530/pixelforge-ai/releases/tag/v0.1.0-rc.6)

## 站点与文档

[像素画室](https://starcloud530.github.io/pixelforge-demos/) · [技术文档](https://starcloud530.github.io/pixelforge-ai/) · [示例工程验证](https://starcloud530.github.io/pixelforge-ai/showcase/)

| 主题 | 链接 |
| --- | --- |
| 安装与快速开始 | [install](https://starcloud530.github.io/pixelforge-ai/install/) |
| Agent skill / CLI 配方 | [agent-skill](https://starcloud530.github.io/pixelforge-ai/agent-skill/) |
| Tool API 目录 | [tool-api](https://starcloud530.github.io/pixelforge-ai/tool-api/) |
| 导出与引擎导入 | [export](https://starcloud530.github.io/pixelforge-ai/export/) |
| 项目清单 | [project-manifest](https://starcloud530.github.io/pixelforge-ai/project-manifest/) |
| 技术栈 | [tech-stack](https://starcloud530.github.io/pixelforge-ai/tech-stack/) |
| 使用说明 | [known-limits](https://starcloud530.github.io/pixelforge-ai/known-limits/) |
| 许可证 | [licensing](https://starcloud530.github.io/pixelforge-ai/licensing/) |

正文源文件在 [`docs/`](docs/)。本地预览：

```bash
pip install -r requirements-docs.txt
mkdocs serve
```

若合入 `main` 后文档 URL 仍 404，请一次性启用：**Settings → Pages → Build and deployment → Source = GitHub Actions**。

## Agent 技能

产品 skill 在 `skills/<name>/SKILL.md`。创作类 skill 先遵守 `pixelforge` 的索引色不变量。已安装的 `pixelforge` 可用 `pixelforge skills list` 和 `pixelforge skills read <name>` 列出并读取同一套打包 skill。命令速查：[agent-skill](https://starcloud530.github.io/pixelforge-ai/agent-skill/)。

| Skill | 作用 |
| --- | --- |
| `pixelforge` | 核心 CLI、Tool API、最少编辑路径 |
| `pixelforge-draw` | 剪影 → 色块 → 细节 |
| `pixelforge-sprite-4dir` | 四向闲 + 走精灵表 |
| `pixelforge-layers` | body / gear / limbs |
| `pixelforge-critique` | 只读列出缺陷 |
| `pixel-art-craft` | 走路、机位、运动轨迹工艺（公开教程链接） |

## 港口守灯人制作记录

<p><a href="https://starcloud530.github.io/pixelforge-demos/gallery/#port-keeper"><img alt="港口守灯人四向行走动画" src="https://resources.pixelforge.torzsi.com/port-keeper/v2/a/walk-four-directions.gif" width="416" /></a></p>

- [Hero 前后对比](https://resources.pixelforge.torzsi.com/port-keeper/v2/storyboard/hero-before-after.png)
- [七阶段时间线](https://resources.pixelforge.torzsi.com/port-keeper/v2/storyboard/timeline.png)
- [Sheet 组装短片](https://resources.pixelforge.torzsi.com/port-keeper/v2/videos/01-sheet-assembly.mp4)

[守灯人制作脚本与重放记录](https://github.com/starcloud530/pixelforge-demos/tree/main/demos/demo-zombie) · [全部案例源码](https://github.com/starcloud530/pixelforge-demos/tree/main/demos)

## 开发

只用预编译的人可跳过。源码 / 网页外壳 / CI：[dev-setup](https://starcloud530.github.io/pixelforge-ai/dev-setup/)。

```bash
cargo test -p pixelforge-kernel
cargo test -p pixelforge-cli
```

## 许可证

代码：**MIT OR Apache-2.0**（[LICENSE](LICENSE)、[LICENSE-MIT](LICENSE-MIT)、[LICENSE-APACHE](LICENSE-APACHE)）。  
示例美术采用 [CC BY 4.0](https://github.com/starcloud530/pixelforge-demos/blob/main/LICENSE-ASSETS.md)。

依赖核对：[docs/licensing.md](docs/licensing.md)。
