<p align="center">
  <strong>PixelForge</strong>
</p>

<h3 align="center">AI 原生的真像素 CLI / 工作室</h3>

<p align="center">
  用 Agent 或脚本在索引色网格上作画，编辑图层和动画帧，再导出精灵表与图集 JSON。
</p>

[Demos](https://starcloud530.github.io/pixelforge-demos/) · [Docs](https://starcloud530.github.io/pixelforge-ai/)

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

## 开发

在仓库根目录运行内核与 CLI 测试。

```bash
cargo test -p pixelforge-kernel
cargo test -p pixelforge-cli
```

## 许可证

代码采用 MIT 或 Apache-2.0 双许可，全文见 `LICENSE-MIT` 和 `LICENSE-APACHE`。示例美术采用 CC BY 4.0。
