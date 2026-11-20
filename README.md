<p align="center">
  <strong>PixelForge</strong>
</p>

<h3 align="center">AI-native true-pixel CLI &amp; studio</h3>

<p align="center">
  Use an AI agent or script to draw on an indexed pixel grid, edit layers and animation frames, and export sprite sheets with atlas JSON.
</p>

[Demos](https://starcloud530.github.io/pixelforge-demos/) · [Docs](https://starcloud530.github.io/pixelforge-ai/)

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

## Development

Run the kernel and CLI tests from the repository root.

```bash
cargo test -p pixelforge-kernel
cargo test -p pixelforge-cli
```

## License

Code is dual-licensed under MIT or Apache-2.0. See `LICENSE-MIT` and `LICENSE-APACHE`. Example artwork is licensed under CC BY 4.0.
