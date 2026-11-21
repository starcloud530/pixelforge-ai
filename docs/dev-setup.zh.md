# 开发环境 { #dev-setup }

本页面向需要构建 CLI、运行网页编辑器或参与开发的读者。直接使用预编译程序，请参阅[安装 PixelForge](install.md)。

## 准备工具链 { #prerequisites }

| 组件 | 所需工具 |
| --- | --- |
| Rust 内核和 CLI | Rust stable、Cargo 1.83 或更新版本，以及系统链接器 |
| 网页编辑器 | Node.js 20 或 22（均经过 CI 验证）、pnpm 10、rustup |
| 文档站 | Python 环境，以及 `requirements-docs.txt` 中的依赖 |

在 Windows 上从源码构建，需要安装 Visual Studio Build Tools 的 C++ 桌面开发组件和 Windows 10/11 SDK。macOS 需要 Xcode Command Line Tools。首次构建网页时会下载 WASM 目标及匹配的绑定工具。

## 源码目录 { #layout }

| 目录 | 内容 |
| --- | --- |
| `crates/pixelforge-kernel` | 索引色文档模型、编辑操作、序列化与导出 |
| `crates/pixelforge-cli` | 命令解析、文件操作与本地播放器 |
| `crates/pixelforge-kernel-wasm` | TypeScript 界面使用的绑定 |
| `apps/web` | 网页编辑器与可选的模型适配器 |
| `skills` | 随 CLI 分发的 Agent 技能 |
| `docs` | 中英文文档 |

## 构建并测试内核 { #rust }

在仓库根目录运行以下命令。

```bash
cargo check --workspace --locked
cargo test --workspace --locked
```

## 从源码运行 CLI { #cli }

```bash
cargo run -p pixelforge-cli -- --help
cargo install --path crates/pixelforge-cli --locked
```

## 启动网页编辑器 { #_2 }

```bash
pnpm install --frozen-lockfile
pnpm dev
```

在浏览器中打开 Vite 输出的地址。使用 `pnpm typecheck` 检查 TypeScript 接口，或运行 `pnpm build` 生成发布构建。

### 重新构建 WASM 桥接层 { #kernel-bridge }

网页相关命令会自动构建 WASM 桥接层。单独构建网页绑定时使用 `pnpm build:wasm`。`pnpm test` 会构建 Node 绑定并运行集成测试。

<span id="issue-26"></span>

## 运行合并前检查 { #ci }

运行 `pnpm lint` 和 `pnpm smoke`。浏览器测试需要先通过 `pnpm exec playwright install chromium` 安装 Chromium。各项检查见[测试与性能测量](quality.md)。

预览文档改动时，在仓库根目录执行以下命令。

```bash
pip install -r requirements-docs.txt
mkdocs serve
```

<span id="_1"></span>

## 发布包配置 { #prebuilt-releases }

[cargo-dist 配置](https://github.com/starcloud530/pixelforge-ai/blob/main/dist-workspace.toml)定义预编译压缩包和安装脚本。安装 cargo-dist 0.33.1 后，运行 `dist plan` 可以查看计划生成的文件。发布标签由 `v` 和根目录 Cargo 工作区版本号组成。已发布的文件及校验和见[版本发布页](https://github.com/starcloud530/pixelforge-ai/releases)。

<span id="_3"></span>

## 读取工程文件 { #project-file }

版本 1 的工程使用 `"format": "pixelforge-project"`。内核负责校验并序列化画布尺寸、色板、帧和编辑状态。旧工程的帧省略 `delayMs` 时按 100 毫秒处理；省略 `paletteLocked` 时默认解锁。可通过 [Tool API](tool-api.md) 的 `saveProject` 和 `loadProject` 保存和载入工程。

## 导入色板文件 { #palette-files }

可以使用 GIMP 色板（`.gpl`），也可以每行填写一个六位 RGB 十六进制颜色值，前缀 `#` 可选。替换模式会更新可编辑色板条目；合并模式会追加新的不透明颜色。索引 0 始终表示透明。
