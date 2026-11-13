# Install & quick start

No Rust toolchain needed for the prebuilt CLI. Current tag: **[v0.1.0-rc.3](https://github.com/starcloud530/pixelforge-ai/releases/tag/v0.1.0-rc.3)** (prerelease). Prefer the installer scripts below; do not use `releases/latest` until a non-prerelease ships.

预编译 CLI 不需要 Rust。当前标签：**v0.1.0-rc.3**（预发布）。用下面的安装脚本；在正式版之前不要用 `releases/latest`。

## Installer / 安装脚本

macOS / Linux:

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/starcloud530/pixelforge-ai/releases/download/v0.1.0-rc.3/pixelforge-cli-installer.sh | sh
```

Windows (PowerShell):

```powershell
powershell -ExecutionPolicy Bypass -c \
  "irm https://github.com/starcloud530/pixelforge-ai/releases/download/v0.1.0-rc.3/pixelforge-cli-installer.ps1 | iex"
```

The installer puts `pixelforge` in `~/.pixelforge/bin` (and updates PATH when possible). Archives are also on the [Releases](https://github.com/starcloud530/pixelforge-ai/releases/tag/v0.1.0-rc.3) page if you prefer a manual unpack.

安装脚本把 `pixelforge` 放到 `~/.pixelforge/bin`。也可在 Releases 页手动下载对应平台压缩包。

| Platform | Archive |
| --- | --- |
| Windows x64 | `pixelforge-cli-x86_64-pc-windows-msvc.zip` |
| macOS Intel | `pixelforge-cli-x86_64-apple-darwin.tar.gz` |
| macOS Apple silicon | `pixelforge-cli-aarch64-apple-darwin.tar.gz` |
| Linux x64 | `pixelforge-cli-x86_64-unknown-linux-gnu.tar.gz` |

Linux builds need glibc 2.35+ (Ubuntu 22.04-class). Windows prebuilts link the C runtime statically.

## 30-second smoke / 三十秒冒烟

```bash
pixelforge --version
pixelforge new --width 16 --height 16 --output hero.json
pixelforge preview --project hero.json --scale 4
```

PNG sequence / APNG preview:

```bash
pixelforge export png --project hero.json --out-dir ./hero-frames --scale 2
pixelforge export apng --project hero.json --output walk.apng --tag walk_down
```

`export apng` is a looping preview, not the sprite sheet a game imports. Sheets and atlas JSON: [export](export.md).

`export apng` 是可循环预览，不是游戏用的 sprite sheet。图集与约定见 [export](export.md)。

## Agents / Agent

External agents (Cursor, Codex, Claude Code, …) drive the same local binary. Skills live under `.cursor/skills/`. Command reference: [agent-skill](agent-skill.md). Tool catalog: [tool-api](tool-api.md).

外部 Agent 调用同一支本地二进制。技能在 `.cursor/skills/`。命令速查：[agent-skill](agent-skill.md)。工具目录：[tool-api](tool-api.md)。

## Source build / 源码构建

Only when changing the kernel or not using a prebuilt. Needs Rust stable (`cargo` 1.83+). Full steps: [dev-setup](dev-setup.md).

```bash
cargo install --path crates/pixelforge-cli
```

Windows source builds also need Visual Studio Build Tools (Desktop development with C++) and a Windows 10/11 SDK. That toolchain is **not** required for the prebuilt `.exe`.

## Docs site (local) / 本地文档站

```bash
pip install -r requirements-docs.txt
mkdocs serve
```

Production deploy is GitHub Actions → Pages from `main` (workflow `pages.yml`): marketing landing at the site root, this MkDocs tree under `/docs/`.  
Expected docs URL: https://starcloud530.github.io/pixelforge-ai/docs/  
Marketing: https://starcloud530.github.io/pixelforge-ai/  
If the site 404s after merge, enable **Settings → Pages → Build and deployment → Source = GitHub Actions** once.
