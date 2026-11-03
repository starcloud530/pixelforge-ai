# 给外部 Agent 的 skill Agent skill

产品路径是：**仓库外的 Agent → 本地 `pixelforge` CLI → Rust 内核 Tool API**。Agent 不改界面上的像素缓冲，也不在这个仓库里做多轮聊天。网页里的一轮面板是可选演示，见 [ai-driver.md](ai-driver.md)。[#14](https://github.com/starcloud530/pixelforge-ai/issues/14) 那种嵌在应用里的多轮对话不是产品。MCP 不在这一阶段；以后若要接，只做这支 CLI 的薄封装。

Skill 补观改流程，不代替专业像素画师的审美。阶段走完、清单通过，图仍然可能不好看。

*The product path is an agent outside this repository, then the local `pixelforge` CLI, then the Rust kernel Tool API. The agent does not patch the pixel buffer the view draws, and this repository does not host a multi-turn chat. The one-turn panel in the web shell is an optional demo; see [ai-driver.md](ai-driver.md). An embedded multi-turn chat like [#14](https://github.com/starcloud530/pixelforge-ai/issues/14) is not the product. MCP is out of this stage. A later wrapper, if any, would be a thin layer over this CLI. Skills add a look-then-revise loop. They do not replace a pixel artist's judgment.*

## 技能表 Skills

Cursor 按每个 `SKILL.md` 的 `description` 选择技能。正文只在 `.cursor/skills/<name>/SKILL.md`。本仓库不复制到 `.agents/skills/` 或 `.claude/skills/`。Codex、Claude Code 读 `AGENTS.md` / `CLAUDE.md` 后打开下表里的文件。某个运行时若只扫描 Agent Skills 标准目录，把 `.cursor/skills` 链过去，不要另写一份正文。

创作类 skill 先遵守 `pixelforge` 的索引色不变量。命令语义以 `pixelforge` 和本文件的速查为准；剪影、四向、分层、挑错的步骤以对应 skill 为准。

*Cursor matches `description` in `.cursor/skills/<name>/SKILL.md`. This repository does not keep a second copy under `.agents/skills/` or `.claude/skills/`. Codex and Claude Code open the files below from `AGENTS.md` and `CLAUDE.md`. Creative skills follow the `pixelforge` invariants first.*

| Skill | 路径 | 何时加载 | 说明 |
| --- | --- | --- | --- |
| `pixelforge` | [.cursor/skills/pixelforge/SKILL.md](../.cursor/skills/pixelforge/SKILL.md) | 用 pixelforge、改工程、CLI、调色板、部件锚点、部件变换、关键帧曲线、约束、烘焙、导出、validate-atlas、图块集、四邻接 autotile、tilemap、Tiled `.tmj`、项目清单 `project init` / `show` / `validate`、资产登记 `asset add` / `list` / `show` / `graph` / `dependents` / `validate`、角色矩阵 `character matrix` / `character check`、缩短编辑路径 | 核心。安装、索引色不变量、退出码、`inspect` / `tools` / `call`。最少编辑路径：同姿复制后改差、合并相邻同色 `fillRect`、一次批量 `call` 仍按请求计数。部件平移、旋转、翻转落在索引格上。曲线按帧下标采样这些参数，不写像素。`bake` 把采样写进像素帧，曲线留在文件里。图块集是画布等于 16 或 32 的工程，一帧一块；地图、autotile 和 `.tmj` 见该 skill 的「图块与地图」。项目清单是约定加资产 id，不是画布。其它创作 skill 先遵守这里。 |
| `pixelforge-draw` | [.cursor/skills/pixelforge-draw/SKILL.md](../.cursor/skills/pixelforge-draw/SKILL.md) | 从零画角色、立绘、剪影、选画布 | 强制剪影（单色）→ 色块 → 细节。每阶段 `preview --scale` 加 ASCII。禁止一像素刷屏。读图要查破洞、头身、体积断点，禁止把完成度说高。 |
| `pixelforge-sprite-4dir` | [.cursor/skills/pixelforge-sprite-4dir/SKILL.md](../.cursor/skills/pixelforge-sprite-4dir/SKILL.md) | 四向走 + 闲、精灵表、脚滑、动画标签、`direction fill` | 行是朝向，列是 idle 加 walk。`export sheet --cols`。标签名是 `idle_down` / `walk_down`。`--atlas` 写帧矩形和标签。导出后先 `validate-atlas`，再进游戏。#18 的 64×64 包在 [pixelforge-demos](https://github.com/starcloud530/pixelforge-demos) (demo-zombie / packs when published)，`validate.py` 重跑 strict。同姿列先 `duplicate` 再改差，槽位已占死时用跨帧 `copy-rect`。`diff` 查脚滑。`hflip` 或 `direction fill --mode mirror` 之后修高光。`copy` 只是占位，不是自然步态。 |
| `pixelforge-layers` | [.cursor/skills/pixelforge-layers/SKILL.md](../.cursor/skills/pixelforge-layers/SKILL.md) | 分层、装备、身体、四肢、隐藏、透明度 | body / gear / limbs。合成预览、`--layer`、`layer hide` / `opacity`、`layer flatten`。 |
| `pixelforge-critique` | [.cursor/skills/pixelforge-critique/SKILL.md](../.cursor/skills/pixelforge-critique/SKILL.md) | 检查、评审、问看起来怎么样 | 只读清单：剪影、头身、侧向深度、腿体积、脚接触、色阶。有编辑记录或多帧时再查可合并的 `fillRect` 和未复制的重复帧。输出必须列缺陷，不能只说看起来不错。 |

## 安装与运行 Install and run

最终用户用预编译的 `pixelforge`，不装 Rust。安装脚本、`new` 和 `preview` 见 [install.md](install.md)（根 [README](../README.md) 也有快速开始）。Windows 源码构建需要的 MSVC / Windows SDK 写在 [dev-setup.md](dev-setup.md) / [install.md](install.md)，和预编译路径分开。

在这个仓库里改代码，从仓库根目录：

```bash
cargo install --path crates/pixelforge-cli
pixelforge --help
```

不安装也可以：

```bash
cargo run -p pixelforge-cli -- --help
```

需要 Rust stable（`cargo` 1.83 或更新）。没有 Python 服务。

*End users run a prebuilt `pixelforge` and do not install Rust. Installer, `new`, and `preview` are in [install.md](install.md) (and the root [README](../README.md) Quick start). The MSVC / Windows SDK requirement for a source build is in [dev-setup.md](dev-setup.md) / [install.md](install.md), separate from the prebuilt path.*

*From this repository, `cargo install --path crates/pixelforge-cli` puts `pixelforge` on the path. `cargo run -p pixelforge-cli -- --help` runs it without installing. Rust stable (`cargo` 1.83 or newer) is enough. There is no Python service.*

## 已知限制 Known limitations

和 [known-limits.md](known-limits.md) 是同一份边界（精简版），放在安装和 `new` / `preview` 之后。测试通过或校验退出码 0 都停在这里。

*Same bounds as [known-limits.md](known-limits.md) (short form here), placed after install and `new` / `preview`. A green test or exit code 0 stays inside them.*

- **未绑定的精修仍钉在画布坐标。** 绑定到部件的 `polish` 层随 `bake` 和部件平移 / 旋转 / 翻转一起走，显式擦除也跟着走。没绑定的覆盖层 `bake` 不写。直接改曲线或约束管着的生成层，下一次 `bake` 仍会从源帧整层盖掉。见 [#99](https://github.com/starcloud530/pixelforge-ai/issues/99) 和 [#72](https://github.com/starcloud530/pixelforge-ai/issues/72)。
- **旋转补洞不是无损。** 覆盖补洞让缺口变少，质量有改进，仍然有损。单像素细线仍可能断开或在接近 45° 时变短。`dropped` 不会被补洞清成 0。见 [#73](https://github.com/starcloud530/pixelforge-ai/issues/73)。
- **IK 解关节位置。** 短链（2 或 3 节）加 `pin` / `plant` 钉接触点。它不编写自然步态，也不生成走路循环。见 [#68](https://github.com/starcloud530/pixelforge-ai/issues/68)。an external driver script (kept out of this repo; see companion demos when published) 只钉接触帧。
- **方向填充不是自然步态，也不是自动画四向。** `direction apply` 只写 idle/walk 标签。`direction fill --mode mirror` 把 left 和 right 水平镜像，`--mode copy` 把一个基本朝向原样拷到其余基本朝向。八向对角不填。镜像不修高光。这不是新画，也不是 PixelLab 自动画四向。`direction validate` 退出码 0 只说明帧段齐。见 [#108](https://github.com/starcloud530/pixelforge-ai/issues/108) 和 [#116](https://github.com/starcloud530/pixelforge-ai/issues/116)。
- **半透明压平收成最近索引。** 彩色 preview 是未量化的直通 alpha 混合。ASCII 在压平前仍是盖住的索引。`layer flatten` 按 `apply-png` 的规则收成索引：alpha 小于 128 写成 0，否则平方 RGB 最近的非 0 项，平局取较小索引。盖住索引的 RGBA 已经等于混合结果时留下那个索引。结果层完全不透明。不改工程的对照是 `preview --compare`：左是未量化 RGBA，右是 flatten 会写下的索引再按调色板着色。要真的收成单层再用 `preview --output before.png`，然后 `layer flatten`，再 `preview --output after.png`。没有混合模式。见 [#114](https://github.com/starcloud530/pixelforge-ai/issues/114) 和 [#110](https://github.com/starcloud530/pixelforge-ai/issues/110)。
- **校验通过不是验收。** `export`、`validate-atlas` 退出码 0、`the pack validate script in pixelforge-demos (when published)` 通过，只说明清单里没有 error。画面质量和 Godot / Unity 接入都不在这次检查里。`export apng` 是可播放预览，不是引擎 sheet，也不量化、不抖动。见 [export.md](export.md)。
- **项目清单不是画布。** `project validate` 退出码 0 只说明尺寸档、视角、目录、色板路径，以及已经登记的资产路径和关系对得上。它不存像素，不扫描目录来发明 id，也不代替画布。朝向 × 动作缺格是 `character check`；矩阵绿 ≠ 步态或观感。见 [project-manifest.md](project-manifest.md)。

*An unbound polish overlay stays on the canvas cell where it was painted. A polish layer bound to a part follows `bake` and part translate, rotate, and flip, including an explicit erase. An edit on a generated layer is still replaced when `bake` rebuilds that layer from the source. See [#99](https://github.com/starcloud530/pixelforge-ai/issues/99) and [#72](https://github.com/starcloud530/pixelforge-ai/issues/72). Rotation sealing is lossy: a one-pixel line can still break or shorten, and `dropped` stays. See [#73](https://github.com/starcloud530/pixelforge-ai/issues/73). IK places joints on a short chain plus `pin` / `plant`. It does not author a gait. See [#68](https://github.com/starcloud530/pixelforge-ai/issues/68). `direction fill` mirrors left and right or copies one cardinal onto the others. It does not fill diagonal rows, repair highlights, author a natural gait, or draw a PixelLab four-direction sheet. Exit 0 from `direction validate` only means the spans are present. See [#108](https://github.com/starcloud530/pixelforge-ai/issues/108) and [#116](https://github.com/starcloud530/pixelforge-ai/issues/116). A translucent stack flattens to the nearest palette index of the unquantized color preview. Alpha below 128 becomes index 0. `preview --compare` shows that blend on the left and the palette color of the indices flatten would write on the right, and does not change the project. See [#114](https://github.com/starcloud530/pixelforge-ai/issues/114) and [#110](https://github.com/starcloud530/pixelforge-ai/issues/110). `project validate` exit 0 means the manifest conventions and the registered asset paths and relations match; it does not store pixels, scan a directory into new ids, or replace the canvas. See [project-manifest.md](project-manifest.md). Export and `validate-atlas` exit 0, including the game-pack script, mean the checklist has no error. They do not accept the picture or the engine import. `export apng` is a playable preview, not the engine sheet, and it does not quantize or dither. See [export.md](export.md).*

## 输出 Output

stdout 是紧凑 JSON，一行一个响应，给 Agent 解析。stderr 是给人看的短句，例如 `pixelforge: wrote hero.json`。`preview` 和 `--preview` 还会把当前帧的 ASCII 图画到 stderr，那一块要读，用来看形状；其余 stderr 不要当接口解析。`preview` 的 stdout 里，`result.rows` 是同一张图，一行一个字符串。

`call` 打出的就是 Tool API 响应，和 [tool-api.md](tool-api.md) 一致。`export` 子命令把 PNG 或 APNG 写到磁盘，stdout 里是路径和尺寸，不带 base64。要用 base64，就 `call` 工具 `exportPngSequence`、`exportSpriteSheet` 或 `exportApng`，这些工具不写文件。

退出码：`0` 全部 `ok: true`。`1` 某个工具 `ok: false`，stdout 仍是那个错误对象。`2` 工具没跑起来（文件打不开、工程损坏、请求文件不是 JSON）。`preview` 和 `diff` 在内核拒绝时也是退出码 `2`，stdout 仍是那个错误对象。缺了必填参数时 clap 把说明打到 stderr，也是退出码 `2`，这时没有 JSON 行。

*Stdout is compact JSON, one response per line. Stderr is a short note such as `pixelforge: wrote hero.json`. Parse stdout. `preview` and `--preview` also write an ASCII picture of the active frame to stderr; read that picture. `preview` puts the same rows in `result.rows`. `call` prints the Tool API response from [tool-api.md](tool-api.md). The `export` subcommand writes PNG or APNG files and prints paths and sizes, without base64. Base64 comes from `call` with `exportPngSequence`, `exportSpriteSheet`, or `exportApng`, and those calls do not write files.*

*Exit `0` means every tool returned `ok: true`. Exit `1` means a tool returned `ok: false` and stdout still holds that error object. Exit `2` means the tool did not run (unreadable file, bad project, request file is not JSON). `preview` and `diff` also exit `2` when the kernel refuses, and stdout is still that error object. A missing flag is also exit `2`: clap writes the usage text to stderr and there is no JSON line.*

## 不变量 Invariants

- 格子存的是调色板索引。索引 0 是透明。`paint`、`fillRect`、`strokeRect`、`line`、`ellipse`、`polygon` 和 `fill` 写入已经存在的 `1..=255` 索引，不写 RGB。`fill` 的种子可以是 0，写入的新颜色不能是 0。`eraseRect` 写入索引 0。`copyRect` 和 `moveRect` 原样复制索引，包括 0。
- 每一帧共用同一个层栈。下标 0 在最下面。绘制写当前层，或者 `--layer` 指定的那一层，别的层不动。`--layer` 不切换当前层。`preview`、`diff`、`export png` / `export sheet` / `export apng` 看的是合成图。`visible: false` 或 `opacity: 0` 的层跳过。索引 0 露出下面的层。`opacity` 缺省 255，非 0 索引整格盖住下层，调色板 alpha 不与下层混合。1 到 254 在索引合成、ASCII 和 `diff` 里仍是整格覆盖。RGBA 导出按直通 alpha 做 source-over，源 alpha 是 `round(调色板 a × opacity / 255)`，一半时远离 0。这份混合在压平之前不写回格子。`preview --layer N` 和 `diff --layer N` 只看那一层的索引，不看可见性，也不乘透明度。没有混合模式。`visible` 缺省 true，两个默认都不写入文件。旧文件读成可见且不透明。`layer flatten` 把这份 RGBA 按 `apply-png` 的规则收成索引（alpha 小于 128 写成 0，否则平方 RGB 最近色，平局取较小索引；已经等于盖住索引的 RGBA 时留下那个索引），结果层可见且不透明。不改工程的对照是 `preview --compare`（左是未量化 RGBA，右是 flatten 会写下的索引色）。要收成单层再用 `preview --output before.png`，然后 `layer flatten`，再 `preview --output after.png`。仍然是这一层默认状态（可见、不透明、`default`、名字 `Layer 1`、当前层 0）时，文件里不写 `layers` 和 `activeLayer`。`role` 缺省 `default`。`polish` 是精修覆盖层。没绑定时 `bake` 不写它；`layer bind --part` 之后，烘焙和部件变换会把修正和显式擦除一起搬走。
- 新建工程带工作室调色板，索引 1 到 8 可以直接画。要新颜色，先 `palette add` 或 `palette set`，再用返回的索引去画。
- `palette lock` 之后仍可用 `paint`、`fillRect`、`strokeRect`、`line`、`ellipse`、`polygon` 和 `fill` 画已有索引，但不能改色、追加、导入或 `remap`。`apply-png` 只写索引，锁定时仍可用。
- 画笔默认拒绝越界索引（`indexPolicy: reject`）。`palette enforce --mode clamp` 把越界索引写成最后一项不透明色。索引 0 仍然拒绝。`import` 的 `replace` 不看这个策略。
- 不要手改工程 JSON 去塞一个颜色。越界索引会让文件打不开。不要把扩散图写回工程。
- 一次进程是一份文档。同一次 `pixelforge call` 里的多个 `--request` 共享撤销栈。工程文件不保存撤销栈。不带 `--checkpoint` 时下一个进程是空栈，和在编辑器里重新打开这份文件一样，`pixelforge undo` 得到 `NothingToUndo`。长流程在每条会改工程的命令上带同一个 `--checkpoint FILE`，进程退出后用 `pixelforge undo --project hero.json --checkpoint FILE`（或 `redo`）恢复；检查点一步是该进程对工程文件的净修改，进程内的 `undo` 仍按内核操作逐条撤回。对不上当前工程的检查点是 `CheckpointMismatch`（退出码 2），不改文件。
- 失败的那一次调用不改文档。一批请求里，失败之前的成功调用会保存，失败之后的不再执行。
- 导出在内核里把索引解析成 RGBA。不要自己给 PNG 重新上色。`diff` / `diffFrames` 画出洋葱皮视图，不把它写回像素。`flip`、`vflip`、`rotate` 只搬索引，不改高光颜色。

*Cells store palette indices. Index 0 is transparent. `paint`, `fillRect`, `strokeRect`, `line`, `ellipse`, `polygon`, and `fill` write an existing index, never RGB. `fill` may start on index 0 and still refuses to write 0. `eraseRect` writes index 0. `copyRect` and `moveRect` copy stored indices, including 0. A stored `selection` is one rectangle or a mask of rectangles. Paint tools skip cells outside it, and the file omits the field when the selection is clear. `preview --selection` and `diff --selection` count only that mask. Every frame shares one layer stack. Index 0 is the bottom. Drawing writes the active layer, or `--layer`, and leaves the other layers alone. `preview`, `diff`, and export show the composite. A hidden layer or opacity 0 is skipped. Opacity 255 covers and does not blend palette alpha. Opacity 1..=254 covers in the index composite and blends in RGBA export. `preview --layer` ignores visibility and opacity. `layer flatten` snaps that RGBA onto the palette (alpha below 128 is index 0, otherwise nearest opaque entry by squared RGB, ties keep the lower index; an exact cover stays) and writes one layer named `Layer 1`, visible and fully opaque. The color preview stays unquantized until that call. `preview --compare` shows that blend beside the indices flatten would write and does not change the project. Old files load visible and opaque. There is no blend mode. A new project ships the studio palette, indices 1 through 8. Add or recolor a palette entry before painting a new color. A locked palette still accepts paint, rectangle fill, line, ellipse, polygon, flood fill, and rectangle outline of an existing index and refuses recolor, append, import, and remap. Out-of-range paint indices are rejected unless `indexPolicy` is `clamp`. Do not hand-edit the project JSON to plant a color, and do not write a diffusion image back into the project.*

*One process is one document. Several `--request` flags on one `pixelforge call` share the undo stack. The project file does not store that stack. Without `--checkpoint`, the next process starts empty, the same as reopening the file, and `pixelforge undo` returns `NothingToUndo`. A long workflow passes the same `--checkpoint FILE` on every command that edits the project, then restores with `pixelforge undo --project hero.json --checkpoint FILE` (or `redo`). One checkpoint step is that process's net change to the project file, and `undo` inside the process still walks kernel operations. A checkpoint that does not match the project is `CheckpointMismatch` (exit 2) and writes nothing. A refused call does not change the document. Earlier calls in that batch are saved; later calls are not run. Export resolves indices in the kernel. `diff` / `diffFrames` draws an onion view and does not write it into pixels. `flip`, `vflip`, and `rotate` move indices only and do not recolor highlights.*

## 命令速查 Command reference

画布尺寸、三阶段画法和完成度用词在 [pixelforge-draw](../.cursor/skills/pixelforge-draw/SKILL.md)。四向行序在 [pixelforge-sprite-4dir](../.cursor/skills/pixelforge-sprite-4dir/SKILL.md)。层怎么摆在 [pixelforge-layers](../.cursor/skills/pixelforge-layers/SKILL.md)。只读清单在 [pixelforge-critique](../.cursor/skills/pixelforge-critique/SKILL.md)。下面是和当前 CLI 一致的命令。

*`resize` copies stored indices into a new canvas and does not invent detail. Omit both offsets to center; pass `--offset-x` and `--offset-y` together. The size table lives in pixelforge-draw.*

### 空白工程 Blank project

```bash
pixelforge new --width 16 --height 16 --output hero.json
```

stdout 是 `inspect` 的响应。文件是 version 1 `pixelforge-project`。`new` 不会覆盖已经存在的文件。16×16 只适合俯视小体型或图标，不适合站立立绘。

*Stdout is the `inspect` response. The file is version 1 `pixelforge-project`. `new` refuses to replace a file that already exists.*

### 只改头盔 Edit only the helmet

选区是工程文件里的一块矩形，或若干互不相交的矩形。不是套索。`selection wand` 把种子格的索引色连通域收成同一份蒙版。没有选区时文件不写 `selection`。多块或空蒙版写成 `{ "rects": [...] }`。下一次 `pixelforge` 进程会读到它；撤销栈仍然不保存。选区不绑定层，也不改 `activeLayer`。头盔在装备层上时，用 `--layer` 指向那一层：选区外任何层都不写，选区内只写你指定的层。

*The selection is one rectangle, or several disjoint rectangles, in the project file. It is not a lasso. `selection wand` stores the seed's connected component as that same mask. A clear selection omits `selection`. Several rectangles, or an empty mask, are `{ "rects": [...] }`. The next process loads it; the undo stack is still not stored. The mask is not tied to a layer and does not change `activeLayer`. Point `--layer` at the gear layer so only that layer is written, and only inside the helmet.*

```bash
pixelforge layer add --project hero.json --name gear
pixelforge fill-rect --project hero.json --layer 0 --x 8 --y 16 --width 16 --height 24 --color-index 3
pixelforge fill-rect --project hero.json --layer 1 --x 10 --y 4 --width 12 --height 10 --color-index 4
pixelforge selection set --project hero.json --x 10 --y 4 --width 12 --height 10
pixelforge replace-index --project hero.json --layer 1 --from-index 4 --to-index 6
pixelforge fill-rect --project hero.json --layer 1 --x 0 --y 0 --width 32 --height 48 --color-index 2
pixelforge selection clear --project hero.json
```

`selection wand --x 12 --y 6 --layer 1` 把种子格的索引色连通域写成同一份选区（`--tolerance` 缺省 0，精确索引；`--connectivity` 为 4 或 8）。

*`selection wand --x 12 --y 6 --layer 1` stores that seed's connected index as the same mask (`--tolerance` defaults to 0; `--connectivity` is 4 or 8).*

`replace-index` 只把装备层上、选区内的索引 4 换成 6。接着那次整画布 `fill-rect` 也只落在头盔矩形里。身体层的索引 3 不动。`move-rect` 的源和目标必须都整块在选区内，否则 `SelectionProtected`，一格都不写。`remap` 和 `apply-png` 不看选区。

*`replace-index` changes index 4 to 6 on the gear layer inside the rectangle. The following full-canvas `fill-rect` also lands only inside the helmet. Index 3 on the body layer stays. `move-rect` returns `SelectionProtected` and writes nothing unless both rectangles lie inside the selection. `remap` and `apply-png` ignore it.*

### 只 diff 头盔选区 Diff only the helmet

```bash
pixelforge selection set --project hero.json --x 10 --y 4 --width 12 --height 10
pixelforge diff --project hero.json --frame 0 --with 1 --layer 1 --selection --scale 4
```

`changed` 只数头盔里的格子。选区外是 `#`，不算 `same`。`result.scope` 是 `selection`。`--layer` 不改 `activeLayer`。不写 `--selection` 时没有 `scope` 字段。也可以用 `--x --y --width --height`，或重复 `--mask x,y,width,height`，三种范围只写一种。没有选区却写 `--selection` 是 `MaskMissing`，退出码 `2`。

*`--selection` counts the stored mask. Cells outside are `#` and are not `same`. Pass only one of `--selection`, the rectangle, or `--mask`.*

### mask 减去脚底再 fill Subtract the feet, then fill

```bash
pixelforge mask subtract --project hero.json \
  --base-x 8 --base-y 16 --base-width 16 --base-height 40 \
  --x 12 --y 52 --width 8 --height 4
pixelforge fill-rect --project hero.json --layer 1 \
  --x 0 --y 0 --width 64 --height 96 --color-index 3 --dirty
```

`mask subtract` 把身体减去脚底，写入 `selection`。不改像素，也不改 `activeLayer`。整画布 `fill-rect` 只落在剩下的格子上。`--dirty` 的 `changed` 等于 `cells`。脚底仍是索引 0。没有选区时 `subtract` / `intersect` 是 `MaskMissing`（退出码 `1`）；`union` 把操作数存成选区。空蒙版保护每一格，要再画整幅先 `selection clear`。`mask union` 和 `mask intersect` 用同一组旗标。

*`mask subtract` stores the body minus the feet. The fill then lands only there. `changed` matches `cells`. The feet stay index 0.*

### 画、填、擦、复制、移动 Paint, fill, erase, copy, move

```bash
pixelforge fill-rect --project hero.json --x 4 --y 8 --width 16 --height 40 --color-index 3 --preview
pixelforge erase-rect --project hero.json --x 4 --y 40 --width 16 --height 8
pixelforge copy-rect --project hero.json --src-x 8 --src-y 70 --width 6 --height 10 --dst-x 40 --dst-y 78
pixelforge move-rect --project hero.json --src-x 40 --src-y 78 --width 6 --height 10 --dst-x 44 --dst-y 80
pixelforge paint --project hero.json --x 1 --y 2 --color-index 3
pixelforge erase --project hero.json --x 1 --y 2
pixelforge preview --project hero.json --scale 4
pixelforge preview --project hero.json --frame 1 --scale 4
pixelforge diff --project hero.json --frame 0 --with 1 --scale 4
```

`fill-rect` 在当前帧上用一个调色板索引填一块轴对齐矩形。`x`、`y` 是左上角。`width` 和 `height` 至少为 1，整块必须落在画布里；只要有一格会出界，一格都不写。色板锁定时仍然可以填。`call` 里的工具名是 `fillRect`。

`stroke-rect` 只画矩形边（`strokeRect`）。`line` 画连续 Bresenham 直线（`line`：`x0`、`y0`、`x1`、`y1`），`--pixel-perfect` 沿长轴一步一格，仍然 8 连通、没有断点。`ellipse` 把椭圆内切在矩形里（`ellipse`），`--fill` 涂内部。`polygon` 按 `--point x,y` 的顺序闭合（`polygon` 的 `points`），至少三个顶点，`--fill` 用偶奇规则涂内部。`fill` 把种子格的同色连通区换成一个已有索引（`fill`），`--connectivity` 是 `4` 或 `8`，选区外的格子是墙。每条都是一步撤销。越界整次不写。没有曲线、笔刷形状或抗锯齿。

```bash
pixelforge line --project hero.json --x0 2 --y0 2 --x1 20 --y1 14 --color-index 1
pixelforge line --project hero.json --x0 2 --y0 16 --x1 20 --y1 28 --color-index 2 --pixel-perfect
pixelforge ellipse --project hero.json --x 8 --y 8 --width 20 --height 12 --color-index 4 --fill
pixelforge polygon --project hero.json --point 4,4 --point 20,6 --point 12,18 --color-index 5 --fill
pixelforge stroke-rect --project hero.json --x 4 --y 30 --width 16 --height 10 --color-index 3
pixelforge fill --project hero.json --x 8 --y 34 --color-index 6 --connectivity 4
```

*`stroke-rect` is the rectangle outline (`strokeRect`). `line` is a continuous Bresenham line; `--pixel-perfect` walks the longer axis one cell at a time and stays 8-connected. `ellipse --fill` paints the interior. `polygon` closes `--point x,y` vertices and `--fill` uses the even-odd rule. `fill` replaces the seed's connected index. `--connectivity` is 4 or 8, and cells outside the selection are barriers. Each command is one undo step. One cell outside the canvas writes nothing. There is no curve, brush shape, or anti-aliasing.*

*`fill-rect` writes one palette index into an axis-aligned rectangle on the active frame. `x` and `y` are the top-left cell. `width` and `height` are at least 1, and the whole rectangle must sit inside the canvas; if any cell would fall outside, nothing is written. A locked palette still allows the fill. The tool name inside `call` is `fillRect`.*

`erase-rect` 把同样的矩形清成索引 0（`eraseRect`）。`copy-rect` 原样复制索引，包括 0（`copyRect` 的参数是 `srcX`、`srcY`、`width`、`height`、`dstX`、`dstY`，外加可选的 `fromFrame`、`toFrame`）。`move-rect` 在同一步撤销里搬走这些索引，并把不在目标里的源格子清成 0（`moveRect`）。不写 `--from-frame` / `--to-frame` 时仍在当前帧。写了也不改变 `activeFrame`。跨帧的 `move-rect` 会把整个源矩形清掉。源或目标出界时，一格都不写。`--layer` 只读写那一层，不切换当前层。

*`erase-rect` clears that rectangle to index 0 (`eraseRect`). `copy-rect` copies indices as they are, including 0 (`copyRect`: `srcX`, `srcY`, `width`, `height`, `dstX`, `dstY`, optional `fromFrame`, optional `toFrame`). `move-rect` moves those indices and clears source cells outside the destination, as one undo step (`moveRect`). Omit `--from-frame` / `--to-frame` to stay on the active frame. Those flags do not change `activeFrame`. A cross-frame move clears the whole source rectangle. If the source or the destination would leave the canvas, nothing is written. `--layer` does not select the layer.*

```bash
pixelforge copy-rect --project hero.json --from-frame 0 --to-frame 2 \
  --src-x 16 --src-y 20 --width 32 --height 36 --dst-x 16 --dst-y 20
pixelforge hflip --project hero.json --frame 2
pixelforge flip --horizontal --project hero.json --x 0 --y 40 --width 64 --height 24
pixelforge vflip --project hero.json
pixelforge flip --vertical --project hero.json --x 0 --y 0 --width 16 --height 32
pixelforge rotate --project hero.json --degrees 180 --x 0 --y 0 --width 8 --height 8
pixelforge resize --project hero.json --width 64 --height 64
pixelforge resize --project hero.json --width 64 --height 96 --offset-x 0 --offset-y 0
```

`hflip` 就是 `flip --horizontal`，不必再写 `--horizontal`。`vflip` 就是 `flip --vertical`。`flip` 必须恰好写 `--horizontal` 或 `--vertical` 里的一个，否则退出码 `2`。两者只搬索引：水平时 `x` 收到 `origin + width - 1 - x`，垂直时 `y` 收到 `origin + height - 1 - y`。不改颜色。不写矩形就是整帧；写矩形时四个边要齐。`rotate --degrees` 只能是 `90`、`180` 或 `270`（顺时针）。180 接受任意矩形。90 和 270 要求正方形，否则 `RotateSquare`，并且不改变画布尺寸。整帧转 90 或 270 要先把画布改成正方形。有选区时，目标格和它读到的源格都在选区内才写入。`resize` 仍写出 version 1 工程。不写偏移就居中；两个偏移都写就指定旧原点。越出新画布的格子被裁掉。一步撤销回到原来的画布。

*`hflip` is `flip --horizontal`. `vflip` is `flip --vertical`. `flip` needs exactly one of those flags. Neither recolors. `rotate --degrees` is 90, 180, or 270 clockwise. 90 and 270 need a square rectangle and do not resize the canvas. A selection writes a cell only when it and the source cell are both inside. `resize` keeps the version 1 project. Omit the offsets to center. Content outside the new canvas is clipped.*

```bash
pixelforge import-aseprite --file hero.aseprite --output hero.json
pixelforge import-aseprite --file hero.aseprite --project hero.json
```

`import-aseprite` 只读索引色 8 位 `.aseprite` / `.ase`：画布、调色板、图像层像素、帧时长。层按文件从底到顶放进现有层栈，组文件夹摊平，参考层跳过。`--output` 新建工程，文件已存在则拒绝。`--project` 在成功后替换已有工程。两个目标要且只要一个。Tool API 是 `importAseprite`，`params.bytes` 为 Base64。成功时 `result` 等于 `inspect`，另加 `aseprite` 报告（对调过的透明索引、被忽略的混合和透明度、裁掉的格子）。失败不改当前文档。不写回 `.aseprite`，不加载插件。灰度和 RGBA、tilemap、画布超出 `1..=256` 都会拒绝。完整不兼容表在 [tool-api.md](tool-api.md) 的 `importAseprite`。

*`import-aseprite` reads indexed 8-bit canvas, palette, image-layer pixels, and frame durations. Layers stay a stack. `--output` creates a project and refuses an existing file. `--project` replaces one after success. Pass exactly one of them. The tool is `importAseprite` with Base64 `bytes`. It does not write `.aseprite`. Grayscale, RGBA, and tilemaps are refused. The incompatibility list is on `importAseprite` in [tool-api.md](tool-api.md).*

字形：`.` 是透明（索引 0），`1`–`9` 是这些索引，`A`–`Z` 是 10–35，`a`–`z` 是 36–61，`*` 是 62 及以上（stderr 的 legend 会写出那些索引和 `#rrggbb`）。`preview` 子命令按 `--scale` 写出彩色 PNG，路径在 `result.path`（默认工程旁边的 `<stem>.preview.png`，或 `--output`）。`--scale` 是 ≥ 1 的整数，默认 1。`preview --frame N` 不改 `activeFrame`，也不改工程文件。`result.frame` 是这张图，`result.activeFrame` 仍是文档的当前帧。会改像素的命令上的 `--preview` 只有 ASCII，不写彩色 PNG。

`diff` 只读。stderr 上 `.` 是索引相同，`!` 是两边都有墨且不同，别的字形是唯一有墨的那边。限定范围时蒙版外是 `#`。`result.path` 默认是 `<stem>.diff.png`，三格：frame、with、洋葱皮。洋葱皮不改像素。保护色只在这张图上。

`export png --scale K` 和 `export sheet --scale K` 在内核里做最近邻放大（`K` 是 ≥ 1 的整数，默认 1）。每个索引变成 `K × K` 的块，再由内核解析颜色。`export sheet` 不传 `--cols` 时列数是 `ceil(sqrt(帧数))`；`--cols N` 指定列数，行数是 `ceil(帧数 / N)`。`scale` 不改变列数。见 [export.md](export.md)。

*Glyphs: `.` is transparent, `1`–`9` and `A`–`Z` / `a`–`z` are indices, `*` is index 62 or above. `preview` writes a color PNG at `result.path`. `--preview` on a mutating command is ASCII only. `diff` is read-only. `export sheet --cols N` sets the column count; scale does not.*

平涂优先 `fillRect`，挖剪影优先 `eraseRect`，挪姿势或道具优先 `copyRect` / `moveRect`。一次 `pixelforge call` 里放多个 `--request`。一块矩形已经是一步撤销。好几块要一起撤销时，用 `beginGesture` / `endGesture` 包住。撤销步数不是内核操作次数，见下面的最少编辑路径。

```bash
pixelforge call --project hero.json \
  --request '{"tool":"beginGesture"}' \
  --request '{"tool":"fillRect","params":{"x":8,"y":4,"width":16,"height":20,"colorIndex":1}}' \
  --request '{"tool":"fillRect","params":{"x":10,"y":24,"width":12,"height":28,"colorIndex":2}}' \
  --request '{"tool":"endGesture"}' \
  --preview
```

### 一笔撤销 A stroke as one undo

手势和撤销必须在同一次进程里：

```bash
pixelforge call --project hero.json \
  --request '{"tool":"beginGesture"}' \
  --request '{"tool":"paint","params":{"x":1,"y":2,"colorIndex":3}}' \
  --request '{"tool":"paint","params":{"x":2,"y":2,"colorIndex":3}}' \
  --request '{"tool":"endGesture"}'
```

要在进程结束前撤回这一笔，把 `--request '{"tool":"undo"}'` 加在同一条命令里。长流程在每条会改工程的命令上带同一个 `--checkpoint FILE`，进程退出后用 `pixelforge undo --project hero.json --checkpoint FILE` 恢复这一进程的净修改。

```bash
pixelforge paint --project hero.json --checkpoint hero.ckpt.json --x 1 --y 2 --color-index 3
pixelforge undo --project hero.json --checkpoint hero.ckpt.json
```

*Add `--request '{"tool":"undo"}'` to that same command to revert the stroke before the process exits. A long workflow passes the same `--checkpoint FILE` on every command that edits the project, then restores that process's net change with `pixelforge undo --project hero.json --checkpoint FILE`.*

### 帧与导出 Frames and export

```bash
pixelforge call --project hero.json \
  --request '{"tool":"duplicateFrame","params":{"index":0}}' \
  --request '{"tool":"setActiveFrame","params":{"index":1}}' \
  --request '{"tool":"setFrameDelay","params":{"index":1,"delayMs":120}}'
pixelforge tag add --project hero.json --name walk_down --from 1 --to 3 --loop forward
pixelforge tag set --project hero.json --name walk_down --loop pingpong --rename walk_south
pixelforge tag remove --project hero.json --name walk_south
pixelforge export sheet --project hero.json --output hero-sheet.png --cols 4 --scale 2 --atlas hero-sheet.json
pixelforge validate-atlas --atlas hero-sheet.json --sheet hero-sheet.png
pixelforge export png --project hero.json --out-dir ./hero-frames --scale 2
pixelforge export apng --project hero.json --output walk.apng --tag walk_down
```

`duplicateFrame` 把像素和时长复制到下一格，返回的 `index` 是新帧。**它不会把当前帧改成这份拷贝。** 当前帧仍显示原来的像素：若它的下标在插入点或之后，下标会加一；否则下标不变。要画拷贝，先 `frame select` / `setActiveFrame` 到返回的下标。网页「复制」按钮会代为选中；CLI 和 `duplicateFrame` 不会。`frame add` 在末尾追加一帧 100 毫秒的空白帧，不改变当前帧。`export png` 写出 `pixelforge-000.png` 起的文件。时长范围是 `1..=65535`。`export apng --tag` 按该标签的循环和每帧 `delayMs` 写出一张可播放的 APNG，用来看这段动画。它不是引擎用的表。

导出之后、进游戏之前先跑 `validate-atlas`。有 error 时退出码是 1，不要把这份表交出去。只剩 warning 时退出码仍是 0。规则和 [#18](https://github.com/starcloud530/pixelforge-ai/issues/18) 的交接在 [export.md](export.md)。64×64 丧尸和据点道具包走 [pixelforge-demos](https://github.com/starcloud530/pixelforge-demos) (demo-zombie / packs when published)：`export sheet --cols 4 --atlas`，再 `validate-atlas --rules … --strict`，游戏侧清单在该目录的 `FAILURES.md`。

*duplicateFrame does not select the copy. `frame add` appends a blank 100 ms frame. `export png` writes `pixelforge-000.png` onward. `export apng --tag` writes a looping APNG preview of that tag. It is not the engine sheet. `export sheet --cols N` keeps one facing on one row when N is the frame count of that facing. `--atlas FILE` writes `pixelforge-atlas` JSON beside the sheet. Omit it and the export result stays the PNG fields. After export, run `validate-atlas` before game import. An error exits 1. Warnings alone stay 0. Tag names for a four-direction sheet live in pixelforge-sprite-4dir. The #18 handoff is in export.md. The 64×64 worked pack is [pixelforge-demos](https://github.com/starcloud530/pixelforge-demos) (demo-zombie / packs when published).*

`tag add` / `addTag` 给一段闭区间起名，并写上 `loop`（`forward`、`reverse`、`pingpong`、`once`，缺省 `forward`）。`tag set` 改区间、循环或改名。`tag remove` 只删标签。没有标签时工程文件不写 `tags`。`inspect` 和 `scene` 始终带 `tags`，空的时候是 `[]`。标签不是 cel 链接：复制帧仍然复制像素。整条时间轴的循环预览不改成按标签播放。插帧、删帧会在同一步撤销里挪已经存在的标签；复制某个标签的最后一帧时，新帧不自动进这个标签。四向名字在帧排好之后再打，见 [pixelforge-sprite-4dir](../.cursor/skills/pixelforge-sprite-4dir/SKILL.md)。

*`tag add` names an inclusive range. `loop` is `forward`, `reverse`, `pingpong`, or `once`, and defaults to `forward`. `tag set` changes the range, the loop, or the name. `tag remove` drops the tag and leaves the frames. A project with no tags omits `tags`. `inspect` and `scene` always include `tags`, and an empty list is `[]`. Tags are not cel links. Playback of the whole timeline is unchanged. Adding or deleting a frame moves existing tags in that same undo step. Duplicating the last frame of a tag does not pull the copy into the tag. Name a four-direction sheet after the frames exist. `direction set` stores that row grid (4, or 8 with diagonals). `direction validate` is a hard error when a facing or an idle/walk span is missing. It does not judge the gait. `direction fill` copies cardinal pixels after those tags exist.*

```bash
pixelforge direction set --project hero.json --count 4 --columns 4 --apply
pixelforge direction validate --project hero.json
pixelforge direction list --project hero.json
pixelforge direction fill --project hero.json --from left --mode mirror
pixelforge direction fill --project hero.json --from down --mode copy
pixelforge direction export --project hero.json --facing south --output south.png
```

`direction set --count 4` 是 N/E/S/W，行序仍是 down、left、right、up（S、W、E、N）。`--count 8` 在这四行后面加 downleft、downright、upleft、upright，不挪前四行。`--apply` 按现有标签规则写 idle/walk，不画像素。`direction fill --mode mirror` 把 left 水平翻到 right，或反过来；`--mode copy` 把源朝向原样拷到其余基本朝向，用来占位，不是新的走路。已有墨水的目标帧默认跳过，`--overwrite` 才盖掉。八向的对角行不填。缺源朝向的 idle/walk 或其它 `{动作}_{朝向}` 帧段时整次不写。填充不写标签，也不修高光。`direction validate` 缺方向或缺关键帧段时退出码 1。通过不是自然步态，也不是审美通过，更不是 PixelLab 自动画四向。细节在 [pixelforge-sprite-4dir](../.cursor/skills/pixelforge-sprite-4dir/SKILL.md)。小例子是 [pixelforge-demos](../pixelforge-demos/README.md)。

*`direction set --count 4` is N/E/S/W in sheet order: down, left, right, up. `--count 8` appends the diagonals and does not move those rows. `--apply` writes idle/walk tags and does not paint. `direction fill --mode mirror` flips left into right, or right into left. `--mode copy` repeats one cardinal on the others as a placeholder, not a new walk. A target frame that already has ink is skipped unless `--overwrite` is set. Diagonal rows are not filled. A missing source idle/walk segment, or any other `{action}_{facing}` the layout already uses, writes nothing. Fill does not write tags and does not repair highlights. `direction validate` exits 1 when a facing or a key span is missing. A pass is not a natural gait, not an aesthetic pass, and not a PixelLab four-direction draw. The worked commands are in [pixelforge-demos](../pixelforge-demos/README.md).*

### 部件与锚点 Parts and anchors

部件不是层。原点 `x,y` 是画布格，也是本地 `(0, 0)`。锚点是相对原点的像素格。枪的像素仍画在 `gear` 上；`layer` 只是指向那一层。没有部件时工程文件不写 `parts`。`part list` 不改文件。矩形翻转和旋转不搬锚点。`part translate` / `part rotate` / `part flip` 才把这一层的墨水和原点、锚点一起搬走。直角是精确整数映射；其它角度先量化到 0.01°，再最近邻落到格子上，枢轴是原点或 `--anchor` 那一格的中心。最近邻会做覆盖补洞，但 `dropped` 仍记下碰撞。同时看 `sealed` 和 `breaks`。1px 斜线和接近 45° 的细线仍可能断或变短。旋转不能代替手修。多帧上的抬枪用 `curve`，采样不写像素。手跟握持、枪托贴肩、脚底固定用 `constraint solve`，它写这一帧。整段帧用 `bake`。士兵示例在 [pixelforge skill](../.cursor/skills/pixelforge/SKILL.md) 的「士兵的枪和握持点」、「部件变换」、「部件曲线」、「约束」和「烘焙」，外部脚本是 [an external driver script (companion demos when published)](https://github.com/starcloud530/pixelforge-demos)。

*A part is not a layer. Its `x,y` is the canvas cell that is local `(0, 0)`. An anchor is a pixel offset from that cell. Gun pixels stay on `gear`; `layer` only names that layer. A project with no parts omits `parts`. `part list` does not write the file. Rectangle flip and rotate do not move anchors. `part translate`, `part rotate`, and `part flip` move that layer's ink together with the origin and anchors. Quarter turns are an exact integer map. Any other angle quantizes to 0.01° and then to the nearest cell, around the origin or the `--anchor` cell. A nearest rotation seals covered notches, and `dropped` still counts the collisions. Read `sealed` and `breaks` too. A 1px diagonal, and a thin line near 45°, can still break or shorten. Rotation does not replace hand cleanup. A raise across frames uses `curve`. Sampling does not write pixels. A hand, a butt, and a planted foot use `constraint solve`, which writes that one frame. `bake` writes the span. The soldier commands are in the pixelforge skill, and the external script is an external driver script (kept out of this repo; see companion demos when published).*

```bash
pixelforge part add --project soldier.json --name gun --layer 1 --x 24 --y 12
pixelforge part anchor set --project soldier.json --part gun --name grip --x 2 --y 2
pixelforge part list --project soldier.json
pixelforge part rotate --project soldier.json --name gun --degrees 90 --anchor grip
pixelforge curve set --project soldier.json --part gun --channel rotation --frame 0 --value 0 --ease ease-in-out
pixelforge curve set --project soldier.json --part gun --channel rotation --frame 4 --value 90
pixelforge curve sample --project soldier.json --part gun --channel rotation --frame 2
```

`curve` 的帧下标和标签相同。`--frame 2` 在 0 与 4 的中点上，`easeInOut`（`u = t² (3 − 2t)`）和线性一样是 45。`--frame 1` 是四分点，`easeInOut` 是 14.0625。翻转通道只 `hold`。没有曲线时工程文件不写 `curves`。采样不写像素。公式和通道见 [pixelforge skill](../.cursor/skills/pixelforge/SKILL.md) 的「部件曲线」。

*A `curve` uses the same frame index as a tag. `--frame 2` is the midpoint of 0 and 4. `easeInOut` (`u = t² (3 − 2t)`) matches linear there: 45. `--frame 1` is the quarter point, and `easeInOut` is 14.0625. Flip channels are `hold` only. A project with no curves omits `curves`. Sampling does not write pixels. The formula and the channels are in the pixelforge skill.*

```bash
pixelforge constraint add --project soldier.json --name hand-grip --kind follow --part hand --anchor palm --target gun --target-anchor grip
pixelforge constraint add --project soldier.json --name butt-shoulder --kind stick --part gun --anchor butt --target shoulder --target-anchor joint
pixelforge constraint add --project soldier.json --name plant-foot --kind pin --part foot --anchor sole --x 4 --y 11
pixelforge constraint add --project soldier.json --name left-leg --kind ik --part shin-l --anchor sole --chain thigh-l --chain shin-l --bend 1 --contact 0:1:12:12 --contact 2:14:10
pixelforge constraint add --project soldier.json --name left-sole --kind plant --part boot --anchor sole --contact 0:1:4:12
pixelforge constraint solve --project soldier.json --frame 4
pixelforge constraint solve --project soldier.json --frame 4 --apply
```

`follow` 让手掌落到枪的 `grip`。`stick` 存成 `attach`，把枪托锚点贴到肩的 `joint`。`pin` 把脚的 `sole` 钉在画布格上。`ik` 转 2 或 3 节，末端必须正好落到目标或 `--contact` 脚印；够不着是 `IkUnreachable`，不会停在旁边的格。`plant` 只在脚印区间里钉脚底，区间外不钉，所以接触帧不会顺着曲线滑。`solve` 不带 `--apply` 只打印报告。带 `--frame` 时先采样曲线再解约束；不带 `--frame` 时用 `part rotate` 已经写下的姿势，脚印按像素帧取样。环、第二条驱动、缺锚点、出画布都报错并且不写。没有约束时工程文件不写 `constraints`。走路示例是 [an external driver script (companion demos when published)](https://github.com/starcloud530/pixelforge-demos)。求解规则见 [pixelforge skill](../.cursor/skills/pixelforge/SKILL.md) 的「约束」。

*A `follow` puts the palm on the gun's `grip`. `stick` is stored as `attach` and puts the butt anchor on the shoulder `joint`. A `pin` plants the foot's `sole` on a canvas cell. `ik` turns a chain of two or three parts so the end cell is exactly the target or a `--contact` span; a miss is `IkUnreachable` and does not stop on a neighbor. `plant` pins the sole only inside a footprint span, so a contact frame cannot slide along a curve. `solve` without `--apply` prints the report. With `--frame` it samples curves and then solves. Without `--frame` it uses the pose `part rotate` already wrote, and footprint spans sample the pixel frame. A cycle, a second driver, a missing anchor, or a cell off the canvas is an error and writes nothing. A project with no constraints omits `constraints`. The walk script is an external driver script (kept out of this repo; see companion demos when published). The solve rules are in the pixelforge skill.*

```bash
pixelforge bake --project soldier.json --from 0 --to 11 --fps 12
pixelforge bake --project soldier.json --part gun --part hand --layer 1
pixelforge bake --project soldier.json --from 4 --to 4 --curves-only
```

`bake` 在 `--from` 到 `--to` 的闭区间上，从 `--source`（缺省第 0 帧）的静止墨水采样曲线并解约束，写成普通像素帧。源帧的像素不改。它重写曲线或约束管着的层，变换回到 0 也从源帧恢复。那一层上改过的格子会被盖掉。精修画在 `layer add --role polish` 的覆盖层上。没绑定的覆盖层 `bake` 不写，格子留在画下的画布格。`layer bind --part 手 --anchor 指尖` 把这一层绑到部件：本地 `(0, 0)` 是锚点，省略 `--anchor` 时是部件原点。这一次烘焙如果摆了那个部件，就把源帧上的修正和显式擦除按同一套变换盖到目标帧，报告里的 `followed` 是这些层。`layer unbind` 回到不跟随。受管理的部件挂在精修层上仍是 `PolishLayer`，整次不写；部件的挂接层保持 `default`。`--fps` 把区间里每一帧的 `delayMs` 设成 `round(1000/fps)`，12 是 83。`--part` 和 `--layer` 可重复。`--curves-only` 不解约束。曲线、约束和存着的原点都留下。像素或时长有变化才是一步撤销。先 `frame duplicate` 静止帧，身体那一层才会留在已有帧上。没有内置脚本。规则见 [pixelforge skill](../.cursor/skills/pixelforge/SKILL.md) 的「烘焙」。

*`bake` samples curves and solves constraints from the rest ink on `--source` (frame 0 by default) across the inclusive `--from`..`--to` range, and writes ordinary pixel frames. The source frame's pixels stay. It rebuilds every layer a curve or constraint manages, including a pose that returned to zero, so a retouch on that layer is replaced. Paint the fix on `layer add --role polish`. An unbound overlay is not written, so those cells stay where they were painted. `layer bind --part hand --anchor tip` binds that layer to the part: local `(0, 0)` is the anchor, or the part origin when `--anchor` is omitted. When this bake poses that part, it stamps the source frame's corrections and explicit erases through the same transform. `followed` lists those layers. `layer unbind` leaves the overlay on canvas coordinates. A managed part linked to the polish layer is still `PolishLayer` and the call writes nothing; keep the part's linked layer `default`. `--fps` sets `delayMs` on every frame in the range to `round(1000/fps)`; 12 is 83. Repeat `--part` and `--layer`. `--curves-only` skips constraints. Curves, constraints, and the stored origins stay. A change to pixels or delays is one undo step. Duplicate the rest frame first so a body layer remains on frames that already exist. There is no script runtime. The rules are in the pixelforge skill.*

握持点 `(2, 2)` 的画布格是 `(26, 14)`。肩和肘用同样的命令，可以不写 `--layer`。

*Grip `(2, 2)` is canvas cell `(26, 14)`. Shoulder and elbow use the same commands and can omit `--layer`.*

同一批修改也可以拆成子命令。子命令各自是一次进程，撤销栈不跨进程：

```bash
pixelforge frame duplicate --project hero.json --index 0
pixelforge frame select --project hero.json --index 1
pixelforge frame delay --project hero.json --index 1 --delay-ms 120
pixelforge frame delete --project hero.json --index 1
```

### 调色板 Palette

```bash
pixelforge palette add --project hero.json --r 20 --g 30 --b 40
pixelforge palette set --project hero.json --index 1 --r 10 --g 20 --b 30
pixelforge palette lock --project hero.json
pixelforge palette unlock --project hero.json
pixelforge palette import --project hero.json --mode merge --file colors.gpl
pixelforge palette enforce --project hero.json --mode clamp
pixelforge palette remap --project hero.json --max-colors 8
pixelforge palette remap --project hero.json --file colors.hex
pixelforge palette apply-png --project hero.json --file sprite.png
```

`palette add` 的结果里有新索引，画笔用那个索引。`import` 的 `mode` 是 `replace` 或 `merge`，文件是 `.gpl` 或十六进制列表。色板锁定时导入和 `remap` 会被拒绝。

`enforce --mode` 是 `reject`（默认）或 `clamp`。`clamp` 写入工程文件；旧文件没有 `indexPolicy` 时按 `reject` 读。只有索引被钳制时，`paint` / `fillRect` 的结果才多 `requestedIndex` 和 `colorIndex`。

`remap` 在 `--max-colors N` 和 `--file` 里只能给一个。`--max-colors` 是保留的不透明色上限。`--file` 用和 `import` 相同的文本做目标盘，像素按平方 RGB 距离重映射，不平均出新颜色。`apply-png` 把一张与画布同尺寸的 PNG 压进当前调色板：不缩放、不切表、不追加颜色，alpha 小于 128 写成索引 0。锁定色板时仍可压盘。

冲突报告在 `result` 里：`misses` 是没精确命中的颜色，`merges` 是并进了哪项近似色（`exact: true` 表示 RGB 相同、只是索引被压紧），`pixelsChanged` 是索引变了的格子数，`dropped` 是量化时丢掉的未使用色。没有 Lospec 在线浏览器。

```json
{ "tool": "setIndexPolicy", "params": { "mode": "reject" } }
{ "tool": "remapPalette", "params": { "maxColors": 8 } }
{ "tool": "remapPalette", "params": { "text": "ff0000\n00ff00\n" } }
{ "tool": "applyPng", "params": { "png": "<base64>", "frame": 0 } }
```

*`palette add` returns the new index. Paint that index. `import` takes `replace` or `merge` and a `.gpl` file or a hex list. A locked palette refuses import and remap. `enforce` sets `reject` (default) or `clamp`. `remap` takes `--max-colors` or `--file`. `apply-png` crushes one canvas-sized PNG onto the current palette. Read `misses`, `merges`, and `pixelsChanged`. There is no Lospec browser.*

### 查看与原始调用 Inspect and raw calls

```bash
pixelforge scene --project hero.json
pixelforge inspect --project hero.json
pixelforge paint --project hero.json --x 1 --y 2 --color-index 3 --dirty
pixelforge tools
pixelforge call --project hero.json --request-file edits.json
```

`scene` 是紧凑摘要：画布、`activeFrame` / `activeLayer`、`selection`（一块矩形、`{rects}`，或 `null`）、图层（`visible` 布尔，`opacity` 为 `0..=255`，缺省可见且 255）、`paletteHash`、帧数、`delayMs`、`tags`（没有标签时是 `[]`）和 `parts`（没有部件时是 `[]`）。不返回像素。怎么读这些字段见 [pixelforge skill](../.cursor/skills/pixelforge/SKILL.md) 的 “How to read state”。`--dirty` 只在绘制命令上多返回 `changed` 和脏区；不写它，结果和以前一样。`combineMask` 不接受 `reportDirty`。

*`scene` is the compact summary: canvas, active frame and layer, layers, `paletteHash`, frame count, delay, `tags` (an empty array when there are none), and `parts` (an empty array when there are none). It does not return pixels. `--dirty` adds `changed` and a dirty box on a drawing command. Omit it and the result stays as it was.*

`edits.json` 可以是一个 JSON 对象、一个 JSON 数组，或每行一个 JSON 值。也可以把 JSON 从 stdin 送进 `pixelforge call --project hero.json`。每个 `--request` 是一个请求对象，原样交给 `Document::call_tool`。

工具名、参数和 `error.code` 见 [tool-api.md](tool-api.md)。

*`edits.json` may be one JSON object, a JSON array, or one JSON value per line. Stdin works the same way: `pixelforge call --project hero.json`. Each `--request` is one request object and is passed to `Document::call_tool`. Names, params, and `error.code` values are in [tool-api.md](tool-api.md).*

### 图块与地图 Tileset, autotile, tilemap

图块集是画布等于图块的工程。边长是 16 或 32。一帧一块，帧下标是 tile id。`tileset set` 之后用原来的画笔和 `preview`。地图是 `pixelforge-tilemap`：格子存 tile id，可水平或垂直翻转。四邻接掩码是北 1、东 2、南 4、西 8，规则 JSON 要齐 16 个 mask。`validate-tiles` 不过就不要导出。`export tileset` 和 `export tilemap` 的 `--cols`、`--scale` 必须相同。`.tmj` 和 `tiles.png` 放在一起给 Tiled 打开。例子是 the base-tiles sample in [https://github.com/starcloud530/pixelforge-demos](https://github.com/starcloud530/pixelforge-demos) (when published)。8 邻接和地图 GUI 不在这一版。

*A tileset is a project whose canvas is the tile. Edges are 16 or 32. One frame is one tile, and the frame index is the tile id. After `tileset set`, paint and `preview` are the usual commands. A map is `pixelforge-tilemap`: a cell stores a tile id and an optional horizontal or vertical flip. The 4-neighbor mask is north 1, east 2, south 4, west 8, and the rules JSON needs all 16 masks. Do not export when `validate-tiles` fails. `export tileset` and `export tilemap` use the same `--cols` and `--scale`. Put the `.tmj` next to `tiles.png` for Tiled. The example is the base-tiles sample in [https://github.com/starcloud530/pixelforge-demos](https://github.com/starcloud530/pixelforge-demos) (when published). Eight-neighbor autotile and a map GUI are not this version.*

```bash
pixelforge tileset set --project tiles.json --tile-width 16 --tile-height 16
pixelforge tilemap fill --map map.json --name ground --x 0 --y 0 --width 8 --height 6 --tile 0
pixelforge autotile apply --map map.json --rules wall.json --name wall
pixelforge validate-tiles --project tiles.json --map map.json --rules wall.json
pixelforge export tileset --project tiles.json --output tiles.png --meta tileset.meta.json --image tiles.png
pixelforge export tilemap --map map.json --tileset tiles.json --output map.png --tmj base.tmj --image tiles.png
```

参数和 `error.code` 见 [tool-api.md](tool-api.md) 的「图块与地图」。

### 项目清单 Project manifest

`pixelforge project` 读写的是 `pixelforge-manifest`，不是画布。它记下尺寸档、视角、目录约定、默认色板路径、一段短说明、资产根和相对路径。`assets` 是你登记的稳定 id 和关系，不是目录扫描。`show` 只读清单。`validate` 核对目录里直接放着的 `pixelforge-project` 是否符合档位，并核对已经登记的路径和关系。通过只说明这些对得上，不是画完了。

`pixelforge asset` 增删改查这些 id。`list --kind`、`show`、`graph`、`dependents` 给 Agent 读关系。`validate` 报缺失路径、悬空 id、owner 错误、`produces` 没有被对方依赖、以及环。它不画像素，不查朝向 × 动作缺格，也不记草稿或已导出。格式见 [project-manifest.md](project-manifest.md)。丧尸包的例子是 the zombie pack manifest in [pixelforge-demos](https://github.com/starcloud530/pixelforge-demos)：`zombie`、`zombie.walk`、`zombie.sheet`，以及 `props` 下的四个道具。灯塔和 `base-tiles` 不在同一个资产根里。

*`pixelforge project` reads and writes a `pixelforge-manifest`, not a canvas. It stores size tiers, perspective, directory rules, the default palette path, a short note, and the asset root. `assets` is the stable ids and relations you register, not a directory scan. `show` prints the file. `validate` checks `pixelforge-project` files directly in those directories and checks registered paths and relations. A pass means those claims match. It does not mean the picture is finished.*

*`pixelforge asset` adds, edits, and queries those ids. `list --kind`, `show`, `graph`, and `dependents` are what an agent reads. `validate` reports a missing path, a dangling id, a bad owner, a `produces` edge the product does not depend on, and a cycle. It does not paint and it does not store draft or exported. Facing × action gaps are `character check`. The format is [project-manifest.md](project-manifest.md). The zombie pack example is the zombie pack manifest in [pixelforge-demos](https://github.com/starcloud530/pixelforge-demos): `zombie`, `zombie.walk`, `zombie.sheet`, and the four props under `props`. The lighthouse and `base-tiles` are not one asset root.*

```bash
pixelforge project show --manifest pixelforge-demos/pixelforge.manifest.json
pixelforge project validate --manifest pixelforge-demos/pixelforge.manifest.json
pixelforge asset show --manifest pixelforge-demos/pixelforge.manifest.json zombie.walk
pixelforge asset graph --manifest pixelforge-demos/pixelforge.manifest.json zombie
pixelforge asset dependents --manifest pixelforge-demos/pixelforge.manifest.json zombie
pixelforge asset validate --manifest pixelforge-demos/pixelforge.manifest.json
pixelforge character check --manifest pixelforge-demos/pixelforge.manifest.json zombie
```

## 创作流程放在哪 Workflow skills

流程正文不在这里重复。摘要如下，和对应 skill 一致。

*The normative steps are the skills. This list is the same contract, shortened.*

### 从零画角色 pixelforge-draw

剪影只用一个索引，用 `fill-rect` / `erase-rect` 切外形。然后才是皮肤、衣服、金属的色块。眼睛、牙、裂口才用 `paint`。每一阶段跑 `pixelforge preview --project … --scale 4`，读 ASCII，并打开 `result.path`。读图时查破洞、头身各占几行、腿和手臂有没有断掉。完成度只写阶段名（剪影、色块、细节），不要写成高完成度。

站立立绘优先 64×128 或 96×128。站立立绘做成 48×64 是示意图。俯视小体型是 16–32。常见四向游戏精灵大约 32–64。更高细节的俯视或全身精灵用 96×96 或 128×128，需要拉高时用 96×128 或 64×128。这些尺寸都在边长 `1..=256` 里，默认 `new` 仍是 16×16。更大只是格子更多，不会自动更好看。96 和 128 的 ASCII 是整帧，阶段检查看 `preview --scale` 的彩色 PNG，ASCII 只抽局部。16 帧走表不要默认用立绘的高度。`resize` 只搬已有索引。变身用 `duplicateFrame` 之后必须 `frame select` 返回的下标。

### 最少编辑路径 Shortest edit path

规范正文和前后对照在 [pixelforge](../.cursor/skills/pixelforge/SKILL.md) 的「最少编辑路径」。四向表上的用法在 [pixelforge-sprite-4dir](../.cursor/skills/pixelforge-sprite-4dir/SKILL.md)。只读挑错在 [pixelforge-critique](../.cursor/skills/pixelforge-critique/SKILL.md) 的第 7、8 条。这里是同一条约，缩短后的样子：

同姿且新帧可以紧挨源帧：`duplicateFrame`，再 `setActiveFrame` 到返回的下标，只改差集。槽位已经占死（例如第 3、4、7 帧）：`copyRect` 的 `fromFrame` / `toFrame` 把身体拷进去，再改差。不要 `addFrame` 出空白再把整个人 `fillRect` 一遍。

相邻同色、同一 `x`、同一 `width`、`y` 相接的扫描行合成一块更高的矩形。`y=20`、`21`、`22` 三笔 `height=1`、`width=12`、`colorIndex=4` 是一笔 `height=3`。左右相接、同 `y`、同 `height`、同色则合成更宽的一笔。

一次 `pixelforge call` 里每个绘制 `--request` 仍是一次内核操作。`beginGesture` / `endGesture` 只合并撤销。2026-10-25 士兵射击动画是 1,191 次工具操作，其中 1,133 次 `fillRect`，就是这三件事叠在一起。不新增内核工具。

*Duplicate a same pose and edit the diff, or `copyRect` into a slot that already exists. Merge abutting same-color scanlines into one taller `fillRect`. A batched `call` counts every drawing request. `beginGesture` groups undo only.*

### 四向走 + 闲 pixelforge-sprite-4dir

行是朝向（下、左、右、上），列是 idle 然后 walk。`export sheet --cols N` 的 `N` 等于每行帧数。下一列从本行 idle `duplicate` 后只改腿和手臂；槽位已经占死时用 `copy-rect --from-frame --to-frame`。脚是否滑看 `diff --frame A --with B --scale 4`：地面行上横移的 `!` 是脚滑。俯视小体型把接触格留在原位，动上面的身体。`hflip` 只搬索引，不改颜色。高光格子跟着身体翻到另一侧，画面上的光源还在原来的一侧，再用 `paint` 把高光画回去。扫描行 `fillRect` 先合成更高的一块。一次批量 `call` 仍按请求计数。

### 分层 pixelforge-layers

`body` 在下标 0，需要单独挪的四肢可以加 `limbs`，装备放 `gear`。手指精修放最上面的 `polish` 覆盖层（`layer add --role polish`）。要跟着部件走，再 `layer bind --part`。没绑定的覆盖层 `bake` 不写。`layer add` 选中新层。`fill-rect --layer N` 不切换当前层。`layer hide` / `layer show` 控制合成里是否出现。`layer opacity --opacity 0..255` 里 255 整格覆盖，1 到 254 在彩色 PNG 里混合。`preview` 的 ASCII 是索引合成，彩色 PNG 才是未量化的混合。`preview --layer N` 看一层，不乘透明度。`export png` 写 RGBA 合成，不收成索引。`layer flatten` 把这份混合按 `apply-png` 的规则收成名为 `Layer 1` 的单层：alpha 小于 128 写成 0，否则平方 RGB 最近色。不改工程的对照是 `preview --compare`。要收成单层再 `preview --output before.png`，然后压平，再 `preview --output after.png`。同一格不要在两层各画一个非 0 索引。

### 只读检查 pixelforge-critique

不改文件。用 `inspect`、`preview --scale 4`、`diff`。输出必须逐条写剪影、头身、侧向深度、腿体积连续、脚接触、色阶。有编辑记录时再写可合并的 `fillRect`（相邻同色扫描行应合成一块更高的矩形）。多帧时再写重复帧未复制（同姿却 `addFrame` 后整帧重画；没有记录则写未核）。有缺陷就列出来。没有缺陷的条写「未发现」并附上帧号或字形。不要只回答「看起来不错」。打不开彩色 PNG 时色阶写「未核」。一次 `call` 里的每个绘制请求各算一次。
