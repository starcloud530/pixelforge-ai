# 安装 PixelForge 并创建工程 { #install-quick-start }

安装适用于本机的命令行工具，绘制一盏小提灯并导出透明 PNG。以下示例使用候选版本 v0.1.0-rc.6。

## 安装 CLI { #installer }

**macOS 或 Linux**

~~~bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/starcloud530/pixelforge-ai/releases/download/v0.1.0-rc.6/pixelforge-cli-installer.sh | sh
~~~

**Windows PowerShell**

~~~powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/starcloud530/pixelforge-ai/releases/download/v0.1.0-rc.6/pixelforge-cli-installer.ps1 | iex"
~~~

打开新终端，运行 `pixelforge --version`。安装此版本后应显示 `pixelforge 0.1.0-rc.6`。如果终端找不到命令，将安装程序显示的目录加入 PATH。

手动安装时，在[发布文件](https://github.com/starcloud530/pixelforge-ai/releases/tag/v0.1.0-rc.6)中下载对应压缩包及其 SHA-256 校验和。解压前核对压缩包的 SHA-256 值，然后将可执行文件放入 PATH 中的目录。

| 系统与架构 | 压缩包 |
| --- | --- |
| macOS · Apple 芯片 | `pixelforge-cli-aarch64-apple-darwin.tar.gz` |
| macOS · Intel | `pixelforge-cli-x86_64-apple-darwin.tar.gz` |
| Windows x64 | `pixelforge-cli-x86_64-pc-windows-msvc.zip` |
| Linux x64 · glibc 2.35+ | `pixelforge-cli-x86_64-unknown-linux-gnu.tar.gz` |

## 绘制并预览一盏提灯 { #your-first-project }

在一个空的工作目录中运行以下命令。工程文件保存可编辑的 16 × 16 画布、调色板和帧。提灯轮廓和灯光使用内置调色板中的三种颜色。

~~~bash
pixelforge new --width 16 --height 16 --output lantern.json
pixelforge stroke-rect --project lantern.json --x 6 --y 1 --width 4 --height 4 --color-index 1
pixelforge fill-rect --project lantern.json --x 4 --y 4 --width 8 --height 9 --color-index 1
pixelforge fill-rect --project lantern.json --x 5 --y 5 --width 6 --height 6 --color-index 4
pixelforge fill-rect --project lantern.json --x 7 --y 6 --width 2 --height 4 --color-index 5
pixelforge preview --project lantern.json --output lantern-preview.png --scale 4
pixelforge export png --project lantern.json --out-dir lantern-frames
~~~

打开 `lantern-preview.png` 查看放大到 64 × 64 的预览。可用于游戏的原尺寸帧位于 `lantern-frames/pixelforge-000.png`。下次编辑时继续打开 `lantern.json`。

## 使用 Agent 或打开动画工程 { #agents-agent }

- [使用 Agent 创作像素素材](agent-skill.md)，在工程目录中完成选区、换色和帧编辑。
- [打开四向角色工程](showcase.md#port-keeper)，播放行走动画并验证导出的图集。

## 从源码构建 { #source-build }

修改 PixelForge 或为其他平台编译时，参见[开发环境](dev-setup.md)中的 Rust 工具链和平台依赖说明。

<span id="docs-site-local"></span>

[PixelForge 使用文档](https://starcloud530.github.io/pixelforge-ai/zh/)
