<p align="center">
  <strong>PixelForge</strong>
</p>

<h3 align="center">AI-native true-pixel CLI &amp; studio</h3>

<p align="center">
  Use an AI agent or script to draw on an indexed pixel grid, edit layers and animation frames, and export sprite sheets with atlas JSON.
</p>

<p align="center">
  <a href="https://starcloud530.github.io/pixelforge-ai/install/"><img alt="Download PixelForge" src="https://img.shields.io/badge/download-PixelForge-blue" /></a>
  <a href="https://starcloud530.github.io/pixelforge-demos/"><img alt="PixelForge Studio" src="https://img.shields.io/badge/studio-PixelForge-teal" /></a>
  <a href="https://starcloud530.github.io/pixelforge-ai/"><img alt="Docs" src="https://img.shields.io/badge/docs-MkDocs-blue" /></a>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-green" /></a>
  <a href="https://github.com/starcloud530/pixelforge-ai/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/starcloud530/pixelforge-ai/actions/workflows/ci.yml/badge.svg" /></a>
</p>

<p align="center">
  <b>English</b> | <a href="README.zh_CN.md">中文</a>
</p>

<p align="center">
  <a href="https://starcloud530.github.io/pixelforge-demos/gallery/#tidal-glasshouse"><img alt="Tidal Glasshouse, a glass conservatory and a lantern at the end of a wooden bridge" src="https://resources.pixelforge.torzsi.com/studio/rc6/1bacf12e29c69944/tidal-glasshouse.png?rev=fb2d394c870ececb" width="416" /></a>
</p>

## Made with PixelForge

These six projects were drawn with the local CLI. Open a project to inspect its tiles, frames or palettes and download the editable source.

| Project | What to inspect |
| --- | --- |
| [Tidal Glasshouse](https://starcloud530.github.io/pixelforge-demos/gallery/#tidal-glasshouse) | 51 tiles across five map layers |
| [Alpine Post Courier](https://starcloud530.github.io/pixelforge-demos/gallery/#alpine-courier) | four directions and 20 frames |
| [Aster Wind Observatory](https://starcloud530.github.io/pixelforge-demos/gallery/#observatory-machine) | startup, sampling and standby |
| [Tide Survey Kit](https://starcloud530.github.io/pixelforge-demos/gallery/#tide-survey-icons) | eight inventory icons in two palettes |
| [Port Keeper](https://starcloud530.github.io/pixelforge-demos/gallery/#port-keeper) | a four-direction walk with a layered outfit |
| [Nezha](https://starcloud530.github.io/pixelforge-demos/gallery/#nezha) | a 12-frame action study |

## Quick start

Install PixelForge for macOS, Windows, or Linux. Installers put `pixelforge` in `~/.pixelforge/bin`.

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
pixelforge play --project hero.json
```

Manual archives, platform notes, and source builds: **[Install docs](https://starcloud530.github.io/pixelforge-ai/install/)** · [Releases](https://github.com/starcloud530/pixelforge-ai/releases/tag/v0.1.0-rc.6)

## Site & documentation

[Pixel art studio](https://starcloud530.github.io/pixelforge-demos/) · [Technical documentation](https://starcloud530.github.io/pixelforge-ai/) · [Verify the example projects](https://starcloud530.github.io/pixelforge-ai/showcase/)

| Topic | Link |
| --- | --- |
| Install & quick start | [install](https://starcloud530.github.io/pixelforge-ai/install/) |
| Agent skill / CLI recipes | [agent-skill](https://starcloud530.github.io/pixelforge-ai/agent-skill/) |
| Tool API catalog | [tool-api](https://starcloud530.github.io/pixelforge-ai/tool-api/) |
| Export & engine import | [export](https://starcloud530.github.io/pixelforge-ai/export/) |
| Project manifest | [project-manifest](https://starcloud530.github.io/pixelforge-ai/project-manifest/) |
| Tech stack | [tech-stack](https://starcloud530.github.io/pixelforge-ai/tech-stack/) |
| Usage notes | [known-limits](https://starcloud530.github.io/pixelforge-ai/known-limits/) |
| Licensing | [licensing](https://starcloud530.github.io/pixelforge-ai/licensing/) |

Markdown sources live under [`docs/`](docs/). Local preview:

```bash
pip install -r requirements-docs.txt
mkdocs serve
```

If the docs URL 404s after merge to `main`, enable Pages once: **Settings → Pages → Build and deployment → Source = GitHub Actions**.

## Agent skills

Product skills live in `skills/<name>/SKILL.md`. Creative skills follow the `pixelforge` index-color invariants first. A packaged `pixelforge` binary lists and reads those skills with `pixelforge skills list` and `pixelforge skills read <name>`. Command reference: [agent-skill](https://starcloud530.github.io/pixelforge-ai/agent-skill/).

| Skill | Role |
| --- | --- |
| `pixelforge` | Core CLI, Tool API, shortest edit path |
| `pixelforge-draw` | Silhouette → flats → detail |
| `pixelforge-sprite-4dir` | Four-direction idle + walk sheets |
| `pixelforge-layers` | body / gear / limbs |
| `pixelforge-critique` | Read-only defect list |
| `pixel-art-craft` | Walk, camera, and motion craft (public tutorial links) |

## Port Keeper production notes

<p><a href="https://starcloud530.github.io/pixelforge-demos/gallery/#port-keeper"><img alt="Port Keeper walking in four directions" src="https://resources.pixelforge.torzsi.com/port-keeper/v2/a/walk-four-directions.gif" width="416" /></a></p>

- [Hero before/after](https://resources.pixelforge.torzsi.com/port-keeper/v2/storyboard/hero-before-after.png)
- [Seven-stage timeline](https://resources.pixelforge.torzsi.com/port-keeper/v2/storyboard/timeline.png)
- [Sheet assembly video](https://resources.pixelforge.torzsi.com/port-keeper/v2/videos/01-sheet-assembly.mp4)

[Port Keeper workflow and replay scripts](https://github.com/starcloud530/pixelforge-demos/tree/main/demos/demo-zombie) · [All example sources](https://github.com/starcloud530/pixelforge-demos/tree/main/demos)

## Development

Prebuilt users can skip this. Source / web shell / CI: [dev-setup](https://starcloud530.github.io/pixelforge-ai/dev-setup/).

```bash
cargo test -p pixelforge-kernel
cargo test -p pixelforge-cli
```

## License

Code: **MIT OR Apache-2.0** ([LICENSE](LICENSE), [LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)).  
Example artwork is licensed under [CC BY 4.0](https://github.com/starcloud530/pixelforge-demos/blob/main/LICENSE-ASSETS.md).

Dependency notes: [docs/licensing.md](docs/licensing.md).
