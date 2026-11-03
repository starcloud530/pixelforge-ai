<p align="center">
  <strong>PixelForge</strong>
</p>

<h3 align="center">AI-native true-pixel CLI &amp; studio</h3>

<p align="center">
  Prompt or reference → true pixels → animation → engine-ready export.<br/>
  Agents drive a local <code>pixelforge</code> CLI on a real indexed grid — not fake-pixel diffusion.
</p>

<p align="center">
  <a href="https://github.com/starcloud530/pixelforge-ai/releases/tag/v0.1.0-rc.2"><img alt="Release" src="https://img.shields.io/badge/release-v0.1.0--rc.2-blue" /></a>
  <a href="https://starcloud530.github.io/pixelforge-ai/"><img alt="Docs" src="https://img.shields.io/badge/docs-GitHub%20Pages-teal" /></a>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-green" /></a>
  <a href="https://github.com/starcloud530/pixelforge-ai/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/starcloud530/pixelforge-ai/actions/workflows/ci.yml/badge.svg" /></a>
</p>

<p align="center">
  <b>English</b> | <a href="README.zh_CN.md">中文</a>
</p>

<p align="center">
  <img alt="Four-direction zombie walk" src="https://resources.pixelforge.torzsi.com/demo-zombie/v1/a/walk-four-directions.gif" width="420" />
</p>

## Why PixelForge

- **True pixels** — indexed grid + palette, the same cells agents and humans edit
- **Agent-first** — external agents (Cursor, Codex, Claude Code, …) call local `pixelforge`; the in-app one-turn panel is only a demo
- **Engine-ready export** — PNG sequences, sprite sheets, atlas JSON, validation gates
- **Not** a full Aseprite clone, **not** diffusion fake-pixel, **not** a full game engine

Status: early / WIP. Prebuilt CLI **[v0.1.0-rc.2](https://github.com/starcloud530/pixelforge-ai/releases/tag/v0.1.0-rc.2)** is a prerelease. Longer vision & L4 framing: [docs/vision.md](docs/vision.md). Honest bounds: [docs/known-limits.md](docs/known-limits.md).

## Quick start

Installers put `pixelforge` in `~/.pixelforge/bin`. Use the **rc.2** tag (not `latest`) until a stable release ships.

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/starcloud530/pixelforge-ai/releases/download/v0.1.0-rc.2/pixelforge-cli-installer.sh | sh
```

```powershell
powershell -ExecutionPolicy Bypass -c \
  "irm https://github.com/starcloud530/pixelforge-ai/releases/download/v0.1.0-rc.2/pixelforge-cli-installer.ps1 | iex"
```

```bash
pixelforge --version
pixelforge new --width 16 --height 16 --output hero.json
pixelforge preview --project hero.json --scale 4
```

Manual archives, platform notes, and source builds: **[Install docs](https://starcloud530.github.io/pixelforge-ai/install/)** · [Releases](https://github.com/starcloud530/pixelforge-ai/releases/tag/v0.1.0-rc.2)

## Documentation

Official docs (MkDocs Material → GitHub Pages):

**https://starcloud530.github.io/pixelforge-ai/**

| Topic | Link |
| --- | --- |
| Install & quick start | [install](https://starcloud530.github.io/pixelforge-ai/install/) |
| Agent skill / CLI recipes | [agent-skill](https://starcloud530.github.io/pixelforge-ai/agent-skill/) |
| Tool API catalog | [tool-api](https://starcloud530.github.io/pixelforge-ai/tool-api/) |
| Export & engine import | [export](https://starcloud530.github.io/pixelforge-ai/export/) |
| Project manifest | [project-manifest](https://starcloud530.github.io/pixelforge-ai/project-manifest/) |
| Tech stack | [tech-stack](https://starcloud530.github.io/pixelforge-ai/tech-stack/) |
| Known limits | [known-limits](https://starcloud530.github.io/pixelforge-ai/known-limits/) |
| Licensing | [licensing](https://starcloud530.github.io/pixelforge-ai/licensing/) |

Markdown sources live under [`docs/`](docs/). Local preview:

```bash
pip install -r requirements-docs.txt
mkdocs serve
```

If the docs URL 404s after merge to `main`, enable Pages once: **Settings → Pages → Build and deployment → Source = GitHub Actions**.

## Agent skills

Cursor loads `.cursor/skills/<name>/SKILL.md` by `description`. Creative skills follow the `pixelforge` index-color invariants first. Command reference: [agent-skill](https://starcloud530.github.io/pixelforge-ai/agent-skill/).

| Skill | Role |
| --- | --- |
| `pixelforge` | Core CLI, Tool API, shortest edit path |
| `pixelforge-draw` | Silhouette → flats → detail |
| `pixelforge-sprite-4dir` | Four-direction idle + walk sheets |
| `pixelforge-layers` | body / gear / limbs |
| `pixelforge-critique` | Read-only defect list |

## Showcase media

CDN (do not commit large media into git):

- [Four-direction walk GIF](https://resources.pixelforge.torzsi.com/demo-zombie/v1/a/walk-four-directions.gif)
- [Hero before/after](https://resources.pixelforge.torzsi.com/demo-zombie/v1/storyboard/hero-before-after.png)
- [Seven-stage timeline](https://resources.pixelforge.torzsi.com/demo-zombie/v1/storyboard/timeline.png)
- [Sheet assembly video](https://resources.pixelforge.torzsi.com/demo-zombie/v1/videos/01-sheet-assembly.mp4)

In-repo light examples: [`examples/game-pack-zombie/`](examples/game-pack-zombie/), [`examples/lighthouse-outpost/`](examples/lighthouse-outpost/).  
Companion demos / eval packs: [pixelforge-demos](https://github.com/starcloud530/pixelforge-demos) (may still be private — use when you have access).

## Development

Prebuilt users can skip this. Source / web shell / CI: [dev-setup](https://starcloud530.github.io/pixelforge-ai/dev-setup/).

```bash
cargo test -p pixelforge-kernel
cargo test -p pixelforge-cli
python3 examples/game-pack-zombie/validate.py
```

## License

Code: **MIT OR Apache-2.0** ([LICENSE](LICENSE), [LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)).  
Example art under `examples/`: [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/) — [examples/LICENSE-ASSETS.md](examples/LICENSE-ASSETS.md).  
Dependency notes: [docs/licensing.md](docs/licensing.md).
