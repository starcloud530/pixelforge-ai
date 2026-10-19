# 本地开发 Dev setup

像素工作室的第一版骨架：Rust 内核保存索引像素，TypeScript 外壳把这份缓冲用最近邻放大画出来。选型以 [tech-stack.md](tech-stack.md) 为准。

*First skeleton of the pixel studio. The Rust kernel stores indexed pixels. The TypeScript shell scales that buffer with nearest-neighbor sampling. [tech-stack.md](tech-stack.md) is the stack decision.*

## 前提 Prerequisites

- Rust stable（`cargo` 1.83 或更新），并且装了 `rustup`，用来补上 `wasm32-unknown-unknown`
- Node.js 20 或更新
- pnpm 10

`wasm-pack` 0.15 是仓库根目录的 devDependency，`pnpm install` 会装上。`pnpm-workspace.yaml` 允许 `wasm-pack` 和 `esbuild` 的安装脚本；pnpm 10 默认会跳过它们，跳过之后 Vite 和 wasm 构建都会缺二进制。第一次 `pnpm build:wasm` 还要联网：`rustup` 安装 wasm 目标，`wasm-pack` 下载与 crate 锁定版本匹配的 `wasm-bindgen` CLI。

仓库根目录同时有 Cargo workspace 和 pnpm workspace。两边各管一层。浏览器里的内核是 Rust 编出来的 WebAssembly。

*Rust stable (`cargo` 1.83 or newer) with `rustup`, so the wasm32 target can be installed. Node.js 20 or newer. pnpm 10.*

*`wasm-pack` 0.15 is a root devDependency installed by `pnpm install`. `pnpm-workspace.yaml` allows the `wasm-pack` and `esbuild` install scripts. pnpm 10 skips those scripts unless they are listed, and Vite and the wasm build both need the binaries. The first `pnpm build:wasm` needs network: `rustup` adds the wasm target, and `wasm-pack` downloads the `wasm-bindgen` CLI that matches the pinned crate.*

*The repo root holds a Cargo workspace and a pnpm workspace. Each one owns its layer. The kernel in the browser is WebAssembly built from Rust.*

## 布局 Layout

```text
Cargo.toml                 Cargo workspace
crates/pixelforge-kernel   索引像素、调色板、帧、画笔/橡皮、撤销栈、工程文件
crates/pixelforge-kernel-wasm
                           wasm-bindgen 桥，生成的 pkg/ 不入库
package.json               pnpm 脚本，含 build:wasm
pnpm-workspace.yaml
apps/web                   Vite + React 外壳
docs/tech-stack.md         已锁定的技术栈
docs/dev-setup.md          本文件
docs/export.md             PNG 序列、sprite sheet，以及 Godot / Unity 导入约定
```

## Rust 内核

在仓库根目录：

```bash
cargo test --workspace
cargo check --workspace
```

`pixelforge-kernel` 目前提供：

- 画布边长 `1..=256`（M1 验收尺寸 16 与 32 可以建出来）
- 索引像素缓冲，0 号为透明
- 一份内置调色板（`Palette::studio_default`）。可以改某个索引的 RGB，也可以在末尾追加颜色。索引 0 保持透明。格子里存的仍是索引，所以改色会作用到每一帧里用到该索引的格子。可以锁定色板：锁定后不能改色、追加或导入，画笔仍写入已有索引。`paletteLocked` 写进工程文件；旧文件没有这个字段时按未锁定读入
- 导入调色板。文本是 GIMP 调色板（`.gpl`，第一行 `GIMP Palette`）或十六进制列表（`.hex` / `.txt`，每行 `RRGGBB` 或 `#RRGGBB`）。`PaletteImportMode::Replace` 用文件里的颜色换掉可绘制项，并把不再存在的索引清成透明；`PaletteImportMode::Merge` 只追加色板里还没有的不透明颜色。文件里的颜色都当成不透明，从索引 1 起放，索引 0 保持透明。一次导入是一步撤销。格式见下文
- `paint` / `erase`。画笔只能写入当前调色板里的不透明索引，格子里不会出现原始 RGB
- 帧列表的增、复制、删，以及切换当前帧（外壳通过 wasm 桥调用）
- 每帧时长（毫秒）。新建帧和旧工程里缺 `delayMs` 的帧用 `DEFAULT_FRAME_DELAY_MS`（100）。合法范围是 `1..=65535`
- 循环播放。`play` / `stop` / `tick` 按每帧时长推进当前帧，这一步不进撤销栈。`scrub` 按时间轴比例选帧，拖动应包在一次 gesture 里
- 撤销 / 重做。栈上记的是内核操作，上限 `MAX_UNDO`（100），包括改色、追加颜色、导入色板、锁定色板、画笔、橡皮、帧编辑和改时长。一次 `begin_gesture` … `end_gesture` 合成一步，所以拖动一笔、拖动取色器、拖动时间轴或连续改时长只占一条记录。没有改动的格子、颜色和时长不入栈。锁定只挡住新的改色、追加和导入；已经在栈上的操作仍可撤销和重做
- 工程文件 `pixelforge-project` version 1。`Document::save_project` / `Document::load_project` 往返画布尺寸、调色板、色板锁定、每一帧的索引和时长、以及当前帧。撤销栈和播放时钟不进文件。改过或追加的颜色会写进文件。缺 `paletteLocked` 的旧文件按未锁定读入
- PNG 导出。`Document::export_png_sequence` 按时间轴给每一帧一张透明 RGBA PNG。`Document::export_sprite_sheet` 把这些帧打进一张网格图：列数是 `ceil(sqrt(帧数))`，行数是 `ceil(帧数 / 列数)`，从左到右再换行，末尾空位透明。索引 0 导出为透明。导出不进撤销栈。Godot / Unity 的导入约定见 [export.md](export.md)。

Tool API 还不在这个 crate 里。

*`pixelforge-kernel` creates canvases with an edge from 1 to 256 (including the M1 sizes 16 and 32), stores indexed pixels with 0 as transparent, and ships `Palette::studio_default`. Paint entries can be recolored, extended, or imported; index 0 stays transparent. Cells keep their indices, so a recolor updates every frame that uses that entry. A locked palette refuses recolor, append, and import, while paint still writes an existing index. `paletteLocked` is stored in the project file and defaults to unlocked when an older file omits it. `import_palette` reads a GIMP palette (`.gpl`) or a hex list (`RRGGBB` / `#RRGGBB` per line): `Replace` swaps the paint colors and clears indices that no longer exist, and `Merge` appends opaque colors the palette does not already have. One import is one undo step. `paint` / `erase` only write opaque indices that exist, never raw RGB. Each frame has a delay in milliseconds (`DEFAULT_FRAME_DELAY_MS` is 100, including when an older file omits `delayMs`; the accepted range is `1..=65535`). `play` / `stop` / `tick` loop the active frame from those delays and do not push undo entries. `scrub` selects the frame under a timeline ratio; a drag should be one gesture. Those edits, plus palette import, the palette lock, frame add / duplicate / delete, the active-frame change, and delay edits, push kernel operations onto an undo stack capped at `MAX_UNDO` (100). The lock refuses new palette edits; operations already on the stack still undo and redo. `begin_gesture` … `end_gesture` collapses one drag — a stroke, a color-picker drag, a timeline scrub, or a run of delay edits — into one step. Cells, colors, and delays that do not change are not recorded. `Document::save_project` / `Document::load_project` round-trip canvas size, the palette (including recolored and appended entries), the palette lock, every frame's indices and delay, and the active frame as `pixelforge-project` version 1 JSON. A missing `paletteLocked` loads unlocked. The undo stack and the playback clock are not in the file. The web shell calls those operations through the wasm bridge. `Document::export_png_sequence` writes one transparent RGBA PNG per frame in timeline order. `Document::export_sprite_sheet` packs those frames into one grid: columns are `ceil(sqrt(frame_count))`, rows are `ceil(frame_count / columns)`, left to right then wrapping, with transparent padding cells. Index 0 exports as transparent. Export does not push an undo entry. Godot / Unity import conventions are in [export.md](export.md). The Tool API is not in this crate.*

## 网页外壳

在仓库根目录：

```bash
pnpm install
pnpm dev
```

`pnpm dev`、`pnpm build`、`pnpm typecheck` 会先跑根目录的 `pnpm build:wasm`：

```bash
rustup target add wasm32-unknown-unknown
wasm-pack build crates/pixelforge-kernel-wasm --target web --out-dir pkg
```

产物在 `crates/pixelforge-kernel-wasm/pkg/`，已 gitignore。浏览器打开 <http://localhost:5173>。

`pnpm build` 在 wasm 编完后做 `tsc --noEmit`，再跑 Vite 生产构建。`pnpm typecheck` 同样先编 wasm，然后只做类型检查。

画布路径：

1. `initKernel()` 加载 wasm 模块。`createKernel()`（`apps/web/src/kernel/client.ts`）返回这份 Rust `Document`。
2. 外壳只读 `snapshot().pixels`，那是调色板索引，不是直接在界面里涂的颜色缓冲。调色板字节也从内核读。
3. 视图先画到一张 1:1 的位图，再用 `imageSmoothingEnabled = false` 放大，所以格子保持最近邻、没有抗锯齿糊边。
4. 指针按下调用 `beginGesture`，画笔、橡皮、增帧、复制、删除、切换当前帧、时间轴拖动和改时长都调用内核，松开调用 `endGesture`，然后重新读取 snapshot。播放调用 `play` / `tick` / `stop`，时钟在内核里。
5. 色板从 `snapshot().palette` 画出。点一个索引就把它设为画笔颜色。取色器调用 `setPaletteColor`，追加颜色调用 `addPaletteColor`。取色器拖动包在同一次 gesture 里，所以一步撤销退回整次改色。「锁定色板」调用 `setPaletteLocked`。锁定后取色器、追加和导入都不可用，点索引和画笔仍然可用。「替换 / 合并」加上「导入」调用 `importPalette`，读 `.gpl` 或十六进制列表。一次导入是一步撤销。
6. 撤销 / 重做按钮，以及 Ctrl/⌘+Z、Ctrl/⌘+Shift+Z、Ctrl/⌘+Y，调用同一份文档的 `undo` / `redo`。示例图在种下去之后清掉历史，撤销停在示例上。
7. 「保存」和 Ctrl/⌘+S 调用 `saveProject()`，下载 `pixelforge-project.json`。「打开」和 Ctrl/⌘+O 读入文本，交给 `loadProject()`，用返回的文档换掉当前画布。打不开时画布保持原样。改过的颜色随调色板一起保存。
8. 「PNG 序列」对每一帧调用 `exportFramePng()`，下载 `pixelforge-000.png` 起的文件。「Sprite sheet」调用 `exportSpriteSheet()`，下载 `pixelforge-sheet-{列}x{行}.png`。网格的列数和行数来自内核。外壳不自己编码 PNG。
9. 「洋葱皮」默认打开，不透明度 40%。外壳用 `frameRgba()` 读取前一帧和后一帧（与 PNG 同一套 RGBA），半透明画在当前帧下面。可以分别关掉前一帧或后一帧。循环播放时这一层先藏起，停下后再显示。切换、绘制、保存、打开和导出都不写入这层。

左键走当前工具，右键擦除。放大倍数和洋葱皮只改视图，不改文档尺寸。

*Open <http://localhost:5173>. `pnpm dev`, `pnpm build`, and `pnpm typecheck` first run `pnpm build:wasm` at the repo root (`wasm-pack build crates/pixelforge-kernel-wasm --target web`). Output lands in `crates/pixelforge-kernel-wasm/pkg/`, which is gitignored. `pnpm build` then typechecks and runs the Vite production build. `pnpm typecheck` is wasm plus types only.*

*The canvas awaits `initKernel()`, asks `createKernel()` for a Rust document, reads palette indices from `snapshot().pixels`, and reads palette bytes from the kernel too. It blits them into a 1:1 bitmap and scales that bitmap with `imageSmoothingEnabled = false`. A pointer-down starts a gesture; paint, erase, frame add / duplicate / delete / select, timeline scrub, and delay edits call the kernel; pointer-up ends the gesture. Playback calls `play` / `tick` / `stop`, and the clock stays in the kernel. The palette swatches come from `snapshot().palette`. Choosing a swatch selects the paint index. The color input calls `setPaletteColor`, and adding a color calls `addPaletteColor`. Lock calls `setPaletteLocked`. Import calls `importPalette` with `replace` or `merge` for a `.gpl` file or a hex list, and that import is one undo step. While locked, the color input, add button, and import button are disabled; swatches and paint stay available. A color-picker drag, a timeline scrub, and a delay edit each stay inside one gesture, so one undo restores that whole edit. Undo and redo — buttons, Ctrl/⌘+Z, Ctrl/⌘+Shift+Z, and Ctrl/⌘+Y — call that same document. The demo sprite clears history after seeding, so undo stops at that sprite. Save and Ctrl/⌘+S call `saveProject()` and download `pixelforge-project.json`. Open and Ctrl/⌘+O pass the file text to `loadProject()` and replace the canvas with that document. A rejected file leaves the canvas alone. Recolored and appended palette entries, and per-frame delays, travel with the file. PNG sequence calls `exportFramePng()` once per frame and downloads `pixelforge-000.png` onward. Sprite sheet calls `exportSpriteSheet()` and downloads `pixelforge-sheet-{columns}x{rows}.png`. Column and row counts come from the kernel. The shell does not encode PNG itself. Onion skin is on by default at 40% opacity. The shell calls `frameRgba()` for the previous and next frames — the same RGBA a PNG would encode — and draws them under the current frame. Each side can be turned off. Loop playback hides the overlay and shows it again when playback stops. Scale and onion skin change the view, not the document.*

## 内核桥 Kernel bridge

`crates/pixelforge-kernel-wasm` 依赖 `pixelforge-kernel`，用 `wasm-bindgen` 暴露 `Document`：尺寸、调色板 RGBA、`setPaletteColor`、`addPaletteColor`、`paletteLocked` / `setPaletteLocked`、`importPalette`、当前帧像素、`paint`、`erase`、帧的增、复制、删、切换、每帧时长、`scrub`、`play` / `tick` / `stop`、`undo` / `redo`、`saveProject` / `loadProject`、`frameRgba`，以及 `exportFramePng` / `exportSpriteSheet`。网页里没有第二份撤销栈，也没有第二份调色板、播放时钟、工程文件编码或 PNG 编码器。洋葱皮只在外壳里合成 `frameRgba` 返回的缓冲。

`apps/web/src/kernel/wasm.ts` 加载 `pkg/` 里的胶水模块。`initKernel()` 只初始化一次。`createKernel()` 返回 wasm 文档。视图继续只认 `PixelKernel` 和 `snapshot()`。

JavaScript 内核桩已经去掉。16×16 示例图仍然由外壳调用 `paint` 种进去，像素落在 Rust 文档里。界面和以后的外部模型都应该改这份内核状态，而不是另写一块像素缓冲。

*`crates/pixelforge-kernel-wasm` depends on `pixelforge-kernel` and exposes `Document` through wasm-bindgen: size, palette RGBA, `setPaletteColor`, `addPaletteColor`, `paletteLocked` / `setPaletteLocked`, `importPalette`, the active frame's pixels, `paint`, `erase`, frame add / duplicate / delete / select, per-frame delay, `scrub`, `play` / `tick` / `stop`, `undo` / `redo`, `saveProject` / `loadProject`, `frameRgba`, and `exportFramePng` / `exportSpriteSheet`. The page does not keep a second undo stack, a second palette, a second playback clock, a second project-file codec, or a second PNG encoder. Onion skin only composites the buffer `frameRgba` returns.*

*`apps/web/src/kernel/wasm.ts` loads the glue in `pkg/`. `initKernel()` initializes the module once. `createKernel()` returns the wasm document. The view stays on `PixelKernel` and `snapshot()`.*

*The JavaScript kernel stub is gone. The 16×16 demo is still seeded by the shell calling `paint`, so those pixels land in the Rust document. The UI and, later, external models change that kernel state. They do not keep another pixel buffer.*

## 和 issue #2–#6 的关系

这一版仍不关闭 #2、#5。#3 的撤销、#4 的色板和 #6 的工程打开/保存已经接到内核文档和 wasm 外壳上。

| Issue | 骨架里有什么 | 还没做 |
| --- | --- | --- |
| [#2](https://github.com/starcloud530/pixelforge-ai/issues/2) 编辑器底座 | 内核 crate + 网页外壳能显示并改格子 | 手搓精灵并保存的整条验收 |
| [#3](https://github.com/starcloud530/pixelforge-ai/issues/3) 像素画布 | Rust `paint`/`erase`；外壳经 wasm 调用画笔、橡皮、整数倍缩放；撤销 / 重做走内核栈 | — |
| [#4](https://github.com/starcloud530/pixelforge-ai/issues/4) 调色板 | 索引色；可改已有颜色、追加颜色；画笔只用这些索引。可锁定，可导入 `.gpl` 和十六进制列表（替换或合并）。改色、导入和锁定记在撤销栈上。外壳经 wasm 点选、编辑、锁定和导入 | — |
| [#5](https://github.com/starcloud530/pixelforge-ai/issues/5) 多帧 | Rust 增帧、复制、删除；外壳经 wasm 切换、新增、复制、删除当前帧。每帧时长和循环预览在 [#8](https://github.com/starcloud530/pixelforge-ai/issues/8)。洋葱皮在 [#9](https://github.com/starcloud530/pixelforge-ai/issues/9) | — |
| [#6](https://github.com/starcloud530/pixelforge-ai/issues/6) 工程文件 | version 1 JSON：尺寸、调色板、全部帧、当前帧。外壳经 wasm 下载和打开 | 撤销栈不写入文件 |

[#10](https://github.com/starcloud530/pixelforge-ai/issues/10) 的 PNG 序列和网格 sprite sheet 已由内核编码，外壳只下载。约定见 [export.md](export.md)。[#9](https://github.com/starcloud530/pixelforge-ai/issues/9) 洋葱皮由外壳把 `frameRgba` 半透明叠在当前帧下面；循环播放时隐藏。不进工程文件，也不进 PNG。

*This slice does not close issues #2 and #5. Undo/redo (#3), the indexed palette with lock and import (#4), project open/save (#6), the frame timeline (#8: scrub, per-frame delay, loop preview), and PNG sequence plus grid sprite-sheet export (#10) go through the wasm kernel. Onion skin (#9) reads `frameRgba` from that kernel and composites in the shell. The overlay is hidden while the timeline is playing and is not stored in the project file or in PNG export.*

## 工程文件 Project file

格式由 `pixelforge-kernel` 拥有。`Document::save_project` 写出带缩进的 JSON，`Document::load_project` 读入。网页只通过 wasm 的 `saveProject` / `loadProject` 触发下载和打开。未知字段、缺了必填字段、以及其他 `version` 都会拒绝。帧上的 `delayMs` 可以省略，省略时按 100 毫秒读入。`paletteLocked` 也可以省略，省略时按未锁定读入。下面是字段，不是字节级样例；写入器会把数组和对象拆成多行。

*The `pixelforge-kernel` crate owns the format. `Document::save_project` writes pretty-printed JSON and `Document::load_project` reads it. The page only triggers download and open through the wasm `saveProject` / `loadProject` methods. Unknown fields, missing required fields, and any other `version` are rejected. `delayMs` on a frame may be omitted and then loads as 100 milliseconds. `paletteLocked` may be omitted and then loads unlocked. The object below is the field list, not a byte-for-byte sample; the writer breaks arrays and objects across lines.*

```json
{
  "format": "pixelforge-project",
  "version": 1,
  "width": 2,
  "height": 1,
  "activeFrame": 1,
  "palette": [
    { "r": 0, "g": 0, "b": 0, "a": 0 },
    { "r": 232, "g": 93, "b": 76, "a": 255 }
  ],
  "paletteLocked": false,
  "frames": [
    { "pixels": [0, 1], "delayMs": 100 },
    { "pixels": [1, 0], "delayMs": 80 }
  ]
}
```

`pixels` 是行优先的调色板索引，长度等于 `width * height`。索引 0 表示空格子。`activeFrame` 是当前帧，从 0 计。`delayMs` 是这一帧在循环播放里停留的毫秒数，范围 `1..=65535`；缺这个字段时按 100 读。调色板第 0 项必须是透明色。`paletteLocked` 为真时，这份文档拒绝改色、追加和导入色板，画笔仍可使用已有索引；缺这个字段时按未锁定读。撤销、重做和播放时钟不在文件里；打开之后，撤销停在文件里的像素上，播放是停着的。

*`pixels` are row-major palette indices and the length is `width * height`. Index 0 is an empty cell. `activeFrame` is the current frame, starting at 0. `delayMs` is how many milliseconds that frame stays up during loop playback, in `1..=65535`; a missing field loads as 100. Palette entry 0 must be transparent. `paletteLocked` refuses recolor, append, and palette import while still allowing paint of an existing index; a missing field loads unlocked. Undo, redo, and the playback clock are not stored; after open, undo stops at the pixels in the file and playback is stopped.*

## 调色板文件 Palette files

内核的 `Document::import_palette` 接受两种文本。外壳的「导入」把文件原文交给 wasm 的 `importPalette(text, mode)`，`mode` 是 `replace` 或 `merge`。

*The kernel's `Document::import_palette` accepts two kinds of text. The shell's import button passes the file text to the wasm `importPalette(text, mode)`, where `mode` is `replace` or `merge`.*

- **GIMP 调色板（`.gpl`）**。第一个非空行必须是 `GIMP Palette`。`Name:`、`Columns:` 和以 `#` 开头的注释会跳过。颜色行是三个 `0..=255` 的整数，后面可以有名字，例如 `255 0 0 Red`。
- **十六进制列表（`.hex` 或 `.txt`）**。每行一个 `RRGGBB` 或 `#RRGGBB`，大小写都行，行尾可以有名字。空行、以 `;` 开头的行和以 `//` 开头的行是注释。

文件里的颜色都存成不透明。索引 0 始终是透明，不会被文件里的第一个颜色替换；那一个颜色会成为索引 1。所以一份从黑色开始的色板，黑色是可绘制的索引，擦除仍然是索引 0。

`replace` 用这些颜色换掉当前可绘制项。若新色板更短，所有帧里超出范围的索引会变成透明，并和色板更换记在同一步撤销里。`merge` 只追加当前色板里还没有的不透明 RGB，文件内的重复颜色只追加一次；若追加后会超过 256 色，整次导入拒绝，文档保持原样。色板锁定时两种导入都拒绝。和当前色板完全一样的文件不会记入撤销栈。

*A GIMP palette (`.gpl`) starts with a `GIMP Palette` line. `Name:`, `Columns:`, and `#` comments are skipped. Each color line is three integers in `0..=255`, optionally followed by a name. A hex list (`.hex` or `.txt`) is one `RRGGBB` or `#RRGGBB` per line. Blank lines, `;` comments, and `//` comments are skipped. A name may follow the hex token.*

*Imported colors are stored opaque. Index 0 stays transparent, so the first color in the file becomes index 1. A palette that starts with black keeps black as a paintable entry; erase still writes index 0.*

*`replace` swaps the paint entries for the file. If the new palette is shorter, every frame's out-of-range index becomes transparent in the same undo step. `merge` appends opaque RGB values the palette does not already have, once each. If that would exceed 256 entries, the import is rejected and the document stays as it was. A locked palette rejects both modes. A file that matches the current palette is not recorded.*

## 这一版明确不包含

Godot、GDScript、C#、Python 推理、模型训练都不在仓库里。外部 AI 以后通过 Tool API 调用内核，见 [tech-stack.md](tech-stack.md)。

*Godot, GDScript, C#, a Python inference service, and model training are not in this repo. External AI will call the kernel through the Tool API later.*
