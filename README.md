# PixelForge AI

**AI 原生的真像素美术工作室。**  
提示或参考 → 真像素 → 动画 → 引擎可导出，常规素材尽量少看管。

*An AI-native true-pixel art studio.*  
*Prompt or reference → true pixels → animation → engine-ready export, with minimal babysitting.*

## 愿景 Vision

让任何有游戏想法的人，都能做出专业级的真像素美术与动画。AI 负责自动驾驶管线，创作者把精力放在品味与方向上。

Make professional true-pixel art and animation accessible to anyone with a game idea. AI handles the autopilot pipeline so creators focus on taste and direction.

## 使命 Mission

做一个 AI 原生的像素工作室：在真实像素网格与调色板约束上，设计、动画并导出引擎可用的精灵；对常规素材达到 L4 级自主——端到端完成，少人工看管。

Build an AI-native pixel studio that designs, animates, and exports engine-ready sprites with L4-level autonomy (end-to-end for routine assets), grounded in real pixel grids and palettes.

## 我们在做什么 What we build

PixelForge AI 走的是 Aseprite 这一类的像素工作流，目标是像素素材的 **L4 自动驾驶**：

- **真像素设计** — 在真实像素网格和调色板约束里作画。
- **动画** — 为精灵做动画。
- **导出** — 产出引擎可直接使用的精灵资源。
- **L4 自动驾驶** — 提示或参考进去，常规素材端到端走完：真像素、动画、引擎就绪导出，尽量少看管。

技术方向（不是搭建说明）：自建像素工作室。TypeScript 做可视化界面和桌面/网页外壳，Rust 做核心内核（像素网格、调色板、帧与时间轴数据、导出）；需要桌面包一层时可选 Tauri。不自训模型。外部 Agent（Codex、Claude Code，或别的能跑命令的模型）通过本地 `pixelforge` CLI 调用 Tool API 驱动这些工具。网页里的一轮面板只是演示。边界与非目标见 [docs/tech-stack.md](docs/tech-stack.md)。像素是网格和调色板上的真像素，不是扩散模型叠出来的假像素。

### 定位

Google Playground 与 Unity Spark 瞄准「文本 → 完整游戏」。我们瞄准更窄的一条：文本或参考 → 真像素精灵 → 动画 → 导出。信念相同——AI 会把创作交到更多人手里；切口更窄，也更能先交付。

工作室是同时做游戏和工具的独立小团队。

PixelForge AI follows an Aseprite-class pixel workflow, aimed at **L4 autopilot** for pixel assets:

- **True-pixel design** on a real pixel grid and palette.
- **Animation** for sprites.
- **Export** of engine-ready sprite assets.
- **L4 autopilot** — prompt or reference in; routine assets run end to end (true pixels, animation, engine-ready export) with minimal babysitting.

Stack direction, not a build guide: a self-built pixel studio. TypeScript is the visualization UI and desktop/web shell; Rust is the core kernel (pixel grid, palette, frame and timeline data, export). Tauri is optional when a desktop shell should wrap that kernel. We do not train models. An external agent (Codex, Claude Code, or any model that can run a command) drives these tools through the local `pixelforge` CLI and the Tool API. The one-turn panel in the web shell is a demo. Boundaries and non-goals: [docs/tech-stack.md](docs/tech-stack.md). Pixels are real grid-and-palette pixels, not diffusion fake-pixel.

Google Playground and Unity Spark aim at text → full games. We aim at text or reference → true-pixel sprites → animation → export. Same belief that AI puts creation in more hands; a narrower wedge we can ship. An indie / small team building games and tools.

## 我们不是什么 What we are not

- **不是** 和 Aseprite 比拼完整界面的克隆竞赛。
- **不是** 只做「看起来像像素」的扩散图。
- **不是** 完整游戏引擎，也不以一句话生成整款游戏为目标。

- Not a race to clone the full Aseprite UI.
- Not fake-pixel diffusion only.
- Not a full game engine, and not text → a whole game.

## 状态 Status

早期，开发中（WIP）。M5 Tool API 已在 Rust 内核提供稳定的机器可读接口，目录见 [docs/tool-api.md](docs/tool-api.md)。产品路径是仓库外的 Agent 调用本地 `pixelforge` CLI，CLI 再把 Tool API JSON 交给内核。用法见 [docs/agent-skill.md](docs/agent-skill.md)。网页里的一轮 AI 面板是可选演示，见 [docs/ai-driver.md](docs/ai-driver.md)。嵌在应用里的多轮对话（[#14](https://github.com/starcloud530/pixelforge-ai/issues/14)）不是产品。

Early / work in progress. The M5 Tool API is a stable machine-readable surface in the Rust kernel; the catalog is [docs/tool-api.md](docs/tool-api.md). The product path is an agent outside this repository calling the local `pixelforge` CLI, which forwards Tool API JSON to the kernel. Usage: [docs/agent-skill.md](docs/agent-skill.md). The one-turn AI panel in the web shell is an optional demo; see [docs/ai-driver.md](docs/ai-driver.md). An embedded multi-turn chat ([#14](https://github.com/starcloud530/pixelforge-ai/issues/14)) is not the product.

## 预编译安装 Prebuilt install

不需要安装 Rust。下载与系统对应的压缩包，解压得到 `pixelforge`（Windows 是 `pixelforge.exe`），然后创建工程并预览。版本号就是 `pixelforge --version`，也是根目录 `Cargo.toml` 里的 `[workspace.package].version`。标签 `v0.1.0` 对应版本 `0.1.0`。

*No Rust toolchain. Download the archive for your system, unpack `pixelforge` (`pixelforge.exe` on Windows), then create a project and preview it. `pixelforge --version` is the `[workspace.package].version` in the root `Cargo.toml`. The tag `v0.1.0` is version `0.1.0`.*

仓库还是私有时，用有权限的 GitHub 账号打开 [Releases](https://github.com/starcloud530/pixelforge-ai/releases) 下载，或：

```bash
gh release download v0.1.0 --repo starcloud530/pixelforge-ai --pattern 'pixelforge-cli-*'
```

把 `v0.1.0` 换成实际标签。`v0.1.0-rc.1` 这种预发布不是 GitHub 的 `latest`，不要用 `releases/latest`，直到打出不带预发布后缀的正式版。

*While this repository is private, download from [Releases](https://github.com/starcloud530/pixelforge-ai/releases) while logged in, or use `gh release download` as above. A prerelease such as `v0.1.0-rc.1` is not GitHub `latest`.*

| 系统 | 文件 |
| --- | --- |
| Windows x64 | `pixelforge-cli-x86_64-pc-windows-msvc.zip` |
| macOS Intel | `pixelforge-cli-x86_64-apple-darwin.tar.gz` |
| macOS Apple silicon | `pixelforge-cli-aarch64-apple-darwin.tar.gz` |
| Linux x64 | `pixelforge-cli-x86_64-unknown-linux-gnu.tar.gz` |

Windows 的 zip 根目录就是 `pixelforge.exe`，旁边有 `LICENSE` 和 `README.md`。macOS / Linux 的 tar.gz 先有一层同名目录，二进制在里面。每个压缩包有同名 `.sha256`，Release 上还有一份汇总 `sha256.sum`。Linux 包在 GitHub `ubuntu-22.04` 上构建，需要 glibc 2.35 或更新（Ubuntu 22.04 这一代）。Windows 包静态链接了 C 运行库，不用装 Visual C++ Redistributable，也不用装 MSVC。

*The Windows zip has `pixelforge.exe` at its root, plus `LICENSE` and `README.md`. macOS and Linux tarballs nest the binary under one directory of the same name. Each archive has a `.sha256` sidecar, and the Release also has `sha256.sum`. The Linux build runs on GitHub `ubuntu-22.04` and needs glibc 2.35 or newer. The Windows build links the C runtime statically.*

```powershell
# Windows，在解压目录
.\pixelforge.exe new --width 16 --height 16 --output hero.json
.\pixelforge.exe preview --project hero.json --scale 4
```

```bash
# macOS / Linux，在解压出来的目录里
./pixelforge new --width 16 --height 16 --output hero.json
./pixelforge preview --project hero.json --scale 4
```

要 PNG 序列而不是预览图：

```bash
./pixelforge export png --project hero.json --out-dir ./hero-frames --scale 2
```

要看一段动画循环：

```bash
./pixelforge export apng --project hero.json --output walk.apng --tag walk_down
```

这张 APNG 是预览，不是进游戏的 sprite sheet。

*A PNG sequence is `export png`. A looping look at one tag is `export apng`. That APNG is a preview, not the sprite sheet a game imports.*

仓库公开之后，可以用一行安装脚本，把 `pixelforge` 装进 `~/.pixelforge/bin` 并写进 PATH。正式版：

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/starcloud530/pixelforge-ai/releases/latest/download/pixelforge-cli-installer.sh | sh
```

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/starcloud530/pixelforge-ai/releases/latest/download/pixelforge-cli-installer.ps1 | iex"
```

私有仓库上这条直链会 404。那是安装脚本的限制，不是 Release 没有附件。公开后同一条工作流不用改。预发布把 URL 里的 `latest` 换成标签，例如 `v0.1.0-rc.1`。

*After the repository is public, the installer scripts above install `pixelforge` to `~/.pixelforge/bin`. On a private repository those direct download URLs return 404; use the archives from the Releases page instead. The workflow does not change when the repository becomes public. For a prerelease, replace `latest` with the tag.*

发版步骤（改版本、打 `v*` 标签、Actions 上传到 Releases）在 [docs/dev-setup.md](docs/dev-setup.md) 的「预编译与发版」。

*How to cut a tag and publish a Release: [docs/dev-setup.md](docs/dev-setup.md).*

## 已知限制 Known limitations

装好并跑通上面的 `new` / `preview` 之后，先看这五条。它们是当前 CLI 和内核的能力边界。测试通过、示例能跑、`validate-atlas` 或游戏包校验退出码是 0，仍然落在这些边界里。

*After `new` and `preview` above, read these five bounds. They are the CLI and kernel as they ship. A green test, a working example, or exit code 0 from `validate-atlas` or the game-pack check stays inside them.*

- **未绑定的精修仍钉在画布坐标。** 绑定到部件的 `polish` 层按部件局部坐标理解：本地 `(0, 0)` 是部件原点，或 `layer bind --anchor` 点名的锚点。`bake` 以及 `part translate` / `rotate` / `flip` 会把这一层的修正连同显式擦除一起搬走。没绑定的覆盖层仍是 [#72](https://github.com/starcloud530/pixelforge-ai/issues/72) 的行为：`bake` 不写它，格子留在画下的那一格。显式擦除是覆盖层上的透明格，和「这一格没改过」不是一回事；`erase-rect --unmodified`（Tool API `mode: pass`）才清回未修改。曲线或约束管着的生成层仍从源帧整层重建，修正不要画在那一层上。旋转补洞仍然有损，绑在部件上的精修跟着转时一样。一层只绑一个部件。见 [#99](https://github.com/starcloud530/pixelforge-ai/issues/99)。烘焙命令见 [docs/agent-skill.md](docs/agent-skill.md)。

- **旋转补洞不是无损。** 非直角旋转在最近邻之后做覆盖补洞。缺口比只做正向映射少，质量有改进，仍然有损。`dropped` 不会因为补上格子变成 0。单像素细线仍可能断开（`breaks` 大于 0），靠近 45° 时还会变短。墨水数量回到源格数不等于修好。见 [#73](https://github.com/starcloud530/pixelforge-ai/issues/73)。旋转不能代替手修。

- **IK 解的是关节位置。** 短链是 2 或 3 节，加上 `pin` 和 `plant` 把脚钉在接触点上。它不自动编写自然步态，也不从一条静止姿势生成走路循环。没有整棵 IK 树，没有步态生成器，也没有物理。见 [#68](https://github.com/starcloud530/pixelforge-ai/issues/68)。`examples/soldier-walk.py` 钉的是接触帧。走路帧仍要自己关键帧，或同姿复制后改差，再 `bake`。

- **层透明度不写回索引。** `opacity` 是 `0..=255`。隐藏层和 0 不进入合成。255 用调色板颜色整格覆盖，不把调色板 alpha 混进下层。1 到 254 只在 RGBA 导出、preview 彩色 PNG 和 `frame_rgba` 里做直通 alpha 混合，不量化成最近的调色板色。ASCII、`diff` 和 `layer flatten` 仍是盖住的索引，所以压平之后半透明会变成不透明的那一格。没有 multiply / screen。见 [#98](https://github.com/starcloud530/pixelforge-ai/issues/98)。

- **导出和校验通过，不是审美验收，也不是进游戏验收。** `export` 写出文件、`validate-atlas` 退出码 0、或 `examples/game-pack-zombie/validate.py` 通过，只说明清单里没有 error。warning 仍可能在。剪影、头身、色阶、动作好不好看不在这份清单里。Godot / Unity 的过滤、轴心和 `idle_*` / `walk_*` 仍要在引擎里接。`export apng` 是可循环播放的预览，不是引擎用的 sprite sheet，`validate-atlas` 不读它。像素和 PNG 导出是同一份直通 RGBA，不把调色板收成 GIF 的索引色，也不抖动；调色板 alpha 和 1 到 254 的层透明度都留着。时长按毫秒写入（分母 1000），不收成百分之一秒。见 [docs/export.md](docs/export.md) 和 [examples/game-pack-zombie/FAILURES.md](examples/game-pack-zombie/FAILURES.md)。

*An unbound `polish` overlay stays on the canvas cell where it was painted. That is still the [#72](https://github.com/starcloud530/pixelforge-ai/issues/72) behavior: `bake` does not write it. A polish layer bound to a part stores the correction in that part's local frame. Local `(0, 0)` is the part origin, or the anchor named by `layer bind --anchor`. `bake` and `part translate` / `rotate` / `flip` move those cells, including an explicit erase. An explicit erase is a transparent overlay cell, distinct from a cell that was never touched. `erase-rect --unmodified` (Tool API `mode: pass`) clears it back to pass-through. A layer a curve or constraint manages is still rebuilt from the source frame, so the fix does not belong on that generated layer. Rotation sealing stays lossy when the bound overlay turns with the part. One polish layer binds to one part. See [#99](https://github.com/starcloud530/pixelforge-ai/issues/99). Bake commands: [docs/agent-skill.md](docs/agent-skill.md).*

*Rotation past a right angle is nearest-neighbor rasterization plus a coverage seal. The seal closes more notches than forward mapping alone. Quality is better, and it is still lossy. Sealing does not clear `dropped`. A one-pixel line can still break (`breaks` above 0) or shorten near 45°. Ink count matching the source is not a finished edge. See [#73](https://github.com/starcloud530/pixelforge-ai/issues/73). Rotation does not replace a hand pass.*

*Layer opacity is `0..=255` and is not written back as an index. A hidden layer or 0 is skipped. 255 covers with the palette color and does not blend palette alpha. 1..=254 blends only in RGBA export, the preview PNG, and `frame_rgba`. It is not snapped to a nearest palette color. ASCII, `diff`, and `layer flatten` keep the covering index, so a flatten turns a partial layer opaque. There is no multiply or screen. See [#98](https://github.com/starcloud530/pixelforge-ai/issues/98).*

*Short-chain IK is two or three parts. `pin` and `plant` hold a foot on a contact. The solve places joints. It does not author a natural gait, and it does not turn one rest pose into a walk cycle. There is no full IK tree, no gait generator, and no physics. See [#68](https://github.com/starcloud530/pixelforge-ai/issues/68). `examples/soldier-walk.py` plants contact frames. Walk frames are still keyed, or duplicated and edited, then baked.*

*`export` writes the files. Exit 0 from `validate-atlas`, and a pass from `examples/game-pack-zombie/validate.py`, mean the checklist has no error. Warnings may remain. Silhouette, proportion, palette, and motion are outside that checklist. Filter, pivot, and `idle_*` / `walk_*` are still wired in Godot or Unity. `export apng` is a looping preview, not the engine sprite sheet, and `validate-atlas` does not read it. The pixels are the same straight RGBA as PNG export: no GIF palette quantization and no dither, so palette alpha and opacity 1..=254 stay. Delays are stored in milliseconds (denominator 1000), not centiseconds. See [docs/export.md](docs/export.md) and [examples/game-pack-zombie/FAILURES.md](examples/game-pack-zombie/FAILURES.md).*

## 源码构建 Source build

这是另一条路。预编译不够用、或要改内核时才走这里。需要 Rust stable（`cargo` 1.83 或更新）。

*This is a separate path, for changing the kernel or not using a prebuilt binary. Rust stable (`cargo` 1.83 or newer).*

```bash
cargo install --path crates/pixelforge-cli
```

Windows 上默认目标是 `x86_64-pc-windows-msvc`。除了 rustup，还要安装 Visual Studio Build Tools，勾选「使用 C++ 的桌面开发」，并包含 Windows 10/11 SDK。只装了 Rust、没有 SDK 时，链接器报找不到 `kernel32.lib`。这份工具链只用于源码构建。上面的 `pixelforge.exe` 不需要它。

Linux 源码构建需要系统链接器（`build-essential` 或等价的 gcc）。macOS 需要 Xcode Command Line Tools。网页外壳和 wasm 另需 Node.js 与 pnpm，见 [docs/dev-setup.md](docs/dev-setup.md)。

*On Windows the default target is `x86_64-pc-windows-msvc`. Install Visual Studio Build Tools with the Desktop development with C++ workload and a Windows 10/11 SDK. Without the SDK the linker reports that it cannot find `kernel32.lib`. That toolchain is only for source builds. Linux source builds need a system linker. macOS source builds need the Xcode Command Line Tools. The web shell and wasm also need Node.js and pnpm: [docs/dev-setup.md](docs/dev-setup.md).*

## Agent Skills

Cursor 进仓库后按 `description` 加载 `.cursor/skills/<name>/SKILL.md`。创作类 skill 先遵守 `pixelforge` 的索引色不变量。Skill 补观改流程，不代替专业画师的审美。命令速查在 [docs/agent-skill.md](docs/agent-skill.md)。本仓库不把同一份正文复制到 `.agents/skills/` 或 `.claude/skills/`；Codex 与 Claude Code 从 `AGENTS.md` / `CLAUDE.md` 打开这些文件。

*Cursor matches `description` under `.cursor/skills/<name>/SKILL.md`. Creative skills follow the `pixelforge` invariants first. Skills add a look-then-revise loop; they do not replace a pixel artist's judgment. Command reference: [docs/agent-skill.md](docs/agent-skill.md). There is no second copy under `.agents/skills/` or `.claude/skills/`.*

| Skill | 说明 |
| --- | --- |
| `pixelforge` | 核心。安装与运行、索引色不变量、退出码、`inspect` / `tools` / `call`。线段、椭圆、多边形、洪水填充和矩形轮廓。最少编辑路径：同姿复制后改差，合并相邻同色 `fillRect`，一次批量 `call` 仍按请求计数。其它创作 skill 先遵守这里。 |
| `pixelforge-draw` | 从零画角色、立绘、剪影。剪影（单色）→ 色块 → 细节，每阶段 `preview --scale` 与 ASCII。禁止一像素刷屏，读图要查破洞、头身和体积断点。 |
| `pixelforge-sprite-4dir` | 四向走 + 闲精灵表。行是朝向，列是 idle 加 walk。`export sheet --cols`、同姿列复制后改差、跨帧 `copy-rect`、`diff` 查脚滑、`hflip` 后修高光。 |
| `pixelforge-layers` | 身体、装备、四肢分层。合成预览、绘制 `--layer`、`layer hide` / `opacity`、`layer flatten`。 |
| `pixelforge-critique` | 只读检查：剪影、头身、侧向深度、腿体积、脚接触、色阶。有编辑记录或多帧时再查可合并的 `fillRect` 和未复制的重复帧。必须列出缺陷，不能只说看起来不错。 |

## 开发 Development

预编译 CLI 不需要这一节的 Rust / Node 工具链，见上面的「预编译安装」。源码构建的 Windows MSVC 要求见「源码构建」。

*The prebuilt CLI does not need the Rust or Node toolchain in this section. See Prebuilt install. Windows MSVC requirements for a source build are in Source build.*

本地骨架分成三层：Rust 内核在 `crates/pixelforge-kernel`（画布尺寸、索引像素、调色板、画笔/橡皮/矩形填充、矩形擦除、矩形复制与移动、帧的增删复制，以及记录这些操作的撤销栈），本地 Agent 入口是 `crates/pixelforge-cli` 的 `pixelforge` 命令，TypeScript 外壳在 `apps/web`（Vite + React，最近邻显示索引格子）。`crates/pixelforge-kernel-wasm` 用 wasm-bindgen 把这份内核编进浏览器。构建和运行见 [docs/dev-setup.md](docs/dev-setup.md)。CLI 的配方见 [docs/agent-skill.md](docs/agent-skill.md)。

外壳的画笔、橡皮、调色板编辑、帧列表（新增空白帧、复制当前帧、删除当前帧且至少留一帧）、帧时间轴（拖动选帧、每帧时长、循环播放）、撤销/重做、工程打开/保存，以及透明 PNG 序列和网格 sprite sheet 导出，都调用 wasm 里的同一份 Rust `Document`。画笔只写入调色板索引。色板可以锁定（写入工程文件），也可以从 GIMP 调色板（`.gpl`）或十六进制列表导入，替换或合并进当前色板，并记在撤销栈上。Agent 不走这些手势：首选是 `pixelforge` 命令，它打开磁盘上的工程文件并调用 `Document::call_tool`。网页里的 `callTool` 是同一入口。目录见 [docs/tool-api.md](docs/tool-api.md)。洋葱皮把前一帧和后一帧半透明叠在当前帧下面，方便补间；循环播放时先藏起，停下后再显示。它只存在于视图，不写入像素、工程文件或 PNG。导入 Godot / Unity 的约定见 [docs/export.md](docs/export.md)。格式说明见 [docs/dev-setup.md](docs/dev-setup.md)。

Local skeleton: a Rust kernel in `crates/pixelforge-kernel` (canvas size, indexed pixels, palette, paint/erase/rectangle fill/rectangle erase/rectangle copy and move, frame add/delete/duplicate, per-frame delay, and an undo stack of those operations), the local agent entry `pixelforge` in `crates/pixelforge-cli`, and a TypeScript shell in `apps/web` (Vite + React, nearest-neighbor view of the indexed grid). `crates/pixelforge-kernel-wasm` compiles that kernel into the browser with wasm-bindgen. Build and run: [docs/dev-setup.md](docs/dev-setup.md). CLI recipes: [docs/agent-skill.md](docs/agent-skill.md).

Paint, erase, palette edits, the frame list (add a blank frame, duplicate the current frame, delete the current frame but never below one), the frame timeline (scrub, per-frame delay, and loop playback), undo/redo, project open/save, and transparent PNG-sequence plus grid sprite-sheet export in the shell call the same Rust `Document` inside wasm. Paint writes palette indices. The palette can be locked (the flag is stored in the project file) and imported from a GIMP palette (`.gpl`) or a hex list, replacing or merging into the document palette as one undo step. An agent does not go through those gestures. The preferred entry is the `pixelforge` command: it opens the project file from disk and calls `Document::call_tool`. Wasm `callTool` is the same entry. Catalog: [docs/tool-api.md](docs/tool-api.md). Onion skin composites the previous and next frames under the current frame at partial opacity. Loop playback hides that overlay and shows it again when playback stops. The overlay is view-only: it is not written into pixels, the project file, or PNG export. Godot / Unity import conventions: [docs/export.md](docs/export.md). Palette file formats: [docs/dev-setup.md](docs/dev-setup.md).

## 检查 Checks

本地和 CI 跑同一套，见 [docs/dev-setup.md](docs/dev-setup.md) 的 CI 一节：`cargo test -p pixelforge-kernel`、`cargo test -p pixelforge-cli`，以及 `python3 examples/game-pack-zombie/validate.py`（对已提交的表跑 `validate-atlas --strict`）。新功能要收进这些测试或门槛再合入。这条工作流不部署服务。预编译包是另一条只在版本标签上跑的 Release 工作流，不替代这些测试。

*Local commands match CI. See the CI section in [docs/dev-setup.md](docs/dev-setup.md): `cargo test -p pixelforge-kernel`, `cargo test -p pixelforge-cli`, and `python3 examples/game-pack-zombie/validate.py` (`validate-atlas --strict` on the committed sheets). A new feature needs a test or gate that this workflow runs. The workflow does not deploy a service. Prebuilt packages are a separate Release workflow on version tags. They do not replace these tests.*

## 许可证 License

本仓库以 [MIT 许可证](LICENSE) 发布。版权所有 © 2026 starcloud530。

第三方依赖已核对，许可证与本仓库的 MIT 再分发兼容，没有 copyleft 冲突。记录见 [docs/licensing.md](docs/licensing.md)。

*This repository is released under the [MIT License](LICENSE). Copyright © 2026 starcloud530.*

*Third-party dependencies were checked. Their licenses are compatible with redistributing this repository under MIT, and there is no copyleft conflict. Notes: [docs/licensing.md](docs/licensing.md).*
