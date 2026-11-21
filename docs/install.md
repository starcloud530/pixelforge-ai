# Install PixelForge and create a project { #install-quick-start }

Install the command-line tool for your operating system, draw a small lantern and export it as a transparent PNG. The examples use the v0.1.0-rc.6 release candidate.

## Install the CLI { #installer }

**macOS or Linux**

~~~bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/starcloud530/pixelforge-ai/releases/download/v0.1.0-rc.6/pixelforge-cli-installer.sh | sh
~~~

**Windows PowerShell**

~~~powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/starcloud530/pixelforge-ai/releases/download/v0.1.0-rc.6/pixelforge-cli-installer.ps1 | iex"
~~~

Open a new terminal and run `pixelforge --version`. For this release, the expected output is `pixelforge 0.1.0-rc.6`. If your shell cannot find the command, add the installation directory shown by the installer to PATH.

For manual installation, download the archive and its SHA-256 checksum from the [release files](https://github.com/starcloud530/pixelforge-ai/releases/tag/v0.1.0-rc.6). Verify the archive against its SHA-256 checksum before extracting the binary into a directory on PATH.

| Platform | Archive |
| --- | --- |
| macOS · Apple silicon | `pixelforge-cli-aarch64-apple-darwin.tar.gz` |
| macOS · Intel | `pixelforge-cli-x86_64-apple-darwin.tar.gz` |
| Windows x64 | `pixelforge-cli-x86_64-pc-windows-msvc.zip` |
| Linux x64 · glibc 2.35+ | `pixelforge-cli-x86_64-unknown-linux-gnu.tar.gz` |

## Draw and preview a lantern { #your-first-project }

Run the commands in an empty working directory. The project stores the editable 16 × 16 canvas, palette and frame. The outlines and light use three entries from the built-in palette.

~~~bash
pixelforge new --width 16 --height 16 --output lantern.json
pixelforge stroke-rect --project lantern.json --x 6 --y 1 --width 4 --height 4 --color-index 1
pixelforge fill-rect --project lantern.json --x 4 --y 4 --width 8 --height 9 --color-index 1
pixelforge fill-rect --project lantern.json --x 5 --y 5 --width 6 --height 6 --color-index 4
pixelforge fill-rect --project lantern.json --x 7 --y 6 --width 2 --height 4 --color-index 5
pixelforge preview --project lantern.json --output lantern-preview.png --scale 4
pixelforge export png --project lantern.json --out-dir lantern-frames
~~~

Open `lantern-preview.png` for a 64 × 64 enlarged view. The game-size 16 × 16 frame is `lantern-frames/pixelforge-000.png`. Reopen `lantern.json` to continue editing.

## Continue with an agent or an animation { #agents-agent }

- [Create pixel art with an agent](agent-skill.md) and give it access to your project directory.
- [Open a sample character](showcase.md#port-keeper) to play a four-direction walk cycle and verify the exported atlas.

## Build from source { #source-build }

If you are modifying PixelForge or building for another platform, follow [development setup](dev-setup.md) for the Rust toolchain and platform dependencies.

<span id="docs-site-local"></span>

[PixelForge Documentation](https://starcloud530.github.io/pixelforge-ai/)
