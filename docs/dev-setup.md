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
- 一份内置调色板（`Palette::studio_default`）。可以改某个索引的 RGB，也可以在末尾追加颜色。索引 0 保持透明。格子里存的仍是索引，所以改色会作用到每一帧里用到该索引的格子
- `paint` / `erase`。画笔只能写入当前调色板里的不透明索引
- 帧列表的增、复制、删，以及切换当前帧（外壳通过 wasm 桥调用）
- 撤销 / 重做。栈上记的是内核操作，上限 `MAX_UNDO`（100），包括改色、追加颜色、画笔、橡皮和帧编辑。一次 `begin_gesture` … `end_gesture` 合成一步，所以拖动一笔或拖动取色器只占一条记录。没有改动的格子和没有改动的颜色不入栈。
- 工程文件 `pixelforge-project` version 1。`Document::save_project` / `Document::load_project` 往返画布尺寸、调色板、每一帧的索引和当前帧。撤销栈不进文件。改过或追加的颜色会写进文件。

PNG / sprite sheet 和 Tool API 还不在这个 crate 里。

*`pixelforge-kernel` creates canvases with an edge from 1 to 256 (including the M1 sizes 16 and 32), stores indexed pixels with 0 as transparent, and ships `Palette::studio_default`. Paint entries can be recolored or extended; index 0 stays transparent. Cells keep their indices, so a recolor updates every frame that uses that entry. `paint` / `erase` only write opaque indices that exist. Those edits, plus frame add / duplicate / delete and the active-frame change, push kernel operations onto an undo stack capped at `MAX_UNDO` (100). `begin_gesture` … `end_gesture` collapses one drag — a stroke or a color-picker drag — into one step. Cells and colors that do not change are not recorded. `Document::save_project` / `Document::load_project` round-trip canvas size, the palette (including recolored and appended entries), every frame's indices, and the active frame as `pixelforge-project` version 1 JSON. The undo stack is not in the file. The web shell calls those operations through the wasm bridge. PNG / sprite-sheet export and the Tool API are not in this crate.*

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
4. 指针按下调用 `beginGesture`，画笔、橡皮、增帧、复制、删除、切换当前帧都调用内核，松开调用 `endGesture`，然后重新读取 snapshot。
5. 色板从 `snapshot().palette` 画出。点一个索引就把它设为画笔颜色。取色器调用 `setPaletteColor`，追加颜色调用 `addPaletteColor`。取色器拖动包在同一次 gesture 里，所以一步撤销退回整次改色。
6. 撤销 / 重做按钮，以及 Ctrl/⌘+Z、Ctrl/⌘+Shift+Z、Ctrl/⌘+Y，调用同一份文档的 `undo` / `redo`。示例图在种下去之后清掉历史，撤销停在示例上。
7. 「保存」和 Ctrl/⌘+S 调用 `saveProject()`，下载 `pixelforge-project.json`。「打开」和 Ctrl/⌘+O 读入文本，交给 `loadProject()`，用返回的文档换掉当前画布。打不开时画布保持原样。改过的颜色随调色板一起保存。

左键走当前工具，右键擦除。放大倍数只改视图，不改文档尺寸。

*Open <http://localhost:5173>. `pnpm dev`, `pnpm build`, and `pnpm typecheck` first run `pnpm build:wasm` at the repo root (`wasm-pack build crates/pixelforge-kernel-wasm --target web`). Output lands in `crates/pixelforge-kernel-wasm/pkg/`, which is gitignored. `pnpm build` then typechecks and runs the Vite production build. `pnpm typecheck` is wasm plus types only.*

*The canvas awaits `initKernel()`, asks `createKernel()` for a Rust document, reads palette indices from `snapshot().pixels`, and reads palette bytes from the kernel too. It blits them into a 1:1 bitmap and scales that bitmap with `imageSmoothingEnabled = false`. A pointer-down starts a gesture; paint, erase, and frame add / duplicate / delete / select call the kernel; pointer-up ends the gesture. The palette swatches come from `snapshot().palette`. Choosing a swatch selects the paint index. The color input calls `setPaletteColor`, and adding a color calls `addPaletteColor`. A color-picker drag stays inside one gesture, so one undo restores that whole edit. Undo and redo — buttons, Ctrl/⌘+Z, Ctrl/⌘+Shift+Z, and Ctrl/⌘+Y — call that same document. The demo sprite clears history after seeding, so undo stops at that sprite. Save and Ctrl/⌘+S call `saveProject()` and download `pixelforge-project.json`. Open and Ctrl/⌘+O pass the file text to `loadProject()` and replace the canvas with that document. A rejected file leaves the canvas alone. Recolored and appended palette entries travel with the file. Scale changes the view, not the document size.*

## 内核桥 Kernel bridge

`crates/pixelforge-kernel-wasm` 依赖 `pixelforge-kernel`，用 `wasm-bindgen` 暴露 `Document`：尺寸、调色板 RGBA、`setPaletteColor`、`addPaletteColor`、当前帧像素、`paint`、`erase`、帧的增、复制、删、切换、`undo` / `redo`，以及 `saveProject` / `loadProject`。网页里没有第二份撤销栈，也没有第二份调色板或工程文件编码。

`apps/web/src/kernel/wasm.ts` 加载 `pkg/` 里的胶水模块。`initKernel()` 只初始化一次。`createKernel()` 返回 wasm 文档。视图继续只认 `PixelKernel` 和 `snapshot()`。

JavaScript 内核桩已经去掉。16×16 示例图仍然由外壳调用 `paint` 种进去，像素落在 Rust 文档里。界面和以后的外部模型都应该改这份内核状态，而不是另写一块像素缓冲。

*`crates/pixelforge-kernel-wasm` depends on `pixelforge-kernel` and exposes `Document` through wasm-bindgen: size, palette RGBA, `setPaletteColor`, `addPaletteColor`, the active frame's pixels, `paint`, `erase`, frame add / duplicate / delete / select, `undo` / `redo`, and `saveProject` / `loadProject`. The page does not keep a second undo stack, a second palette, or a second project-file codec.*

*`apps/web/src/kernel/wasm.ts` loads the glue in `pkg/`. `initKernel()` initializes the module once. `createKernel()` returns the wasm document. The view stays on `PixelKernel` and `snapshot()`.*

*The JavaScript kernel stub is gone. The 16×16 demo is still seeded by the shell calling `paint`, so those pixels land in the Rust document. The UI and, later, external models change that kernel state. They do not keep another pixel buffer.*

## 和 issue #2–#6 的关系

这一版仍不关闭 #2、#4、#5。#3 的撤销和 #6 的工程打开/保存已经接到内核文档和 wasm 外壳上。

| Issue | 骨架里有什么 | 还没做 |
| --- | --- | --- |
| [#2](https://github.com/starcloud530/pixelforge-ai/issues/2) 编辑器底座 | 内核 crate + 网页外壳能显示并改格子 | 手搓精灵并保存的整条验收 |
| [#3](https://github.com/starcloud530/pixelforge-ai/issues/3) 像素画布 | Rust `paint`/`erase`；外壳经 wasm 调用画笔、橡皮、整数倍缩放；撤销 / 重做走内核栈 | — |
| [#4](https://github.com/starcloud530/pixelforge-ai/issues/4) 调色板 | 索引色；可改已有颜色、追加颜色；画笔只用这些索引。改色记在撤销栈上。外壳经 wasm 点选当前颜色并编辑 | 导入、锁定色板 |
| [#5](https://github.com/starcloud530/pixelforge-ai/issues/5) 多帧 | Rust 增帧、复制、删除；外壳经 wasm 切换、新增、复制、删除当前帧 | 每帧时长、洋葱皮、循环预览 |
| [#6](https://github.com/starcloud530/pixelforge-ai/issues/6) 工程文件 | version 1 JSON：尺寸、调色板、全部帧、当前帧。外壳经 wasm 下载和打开 | 撤销栈不写入文件 |

*This slice does not close issues #2, #4, and #5. Undo/redo (#3), palette recolor and append, and project open/save (#6) go through the wasm kernel. Palette import and locking, per-frame delay, onion skin, and loop preview are still open.*

## 工程文件 Project file

格式由 `pixelforge-kernel` 拥有。`Document::save_project` 写出带缩进的 JSON，`Document::load_project` 读入。网页只通过 wasm 的 `saveProject` / `loadProject` 触发下载和打开。未知字段、缺字段和其他 `version` 都会拒绝。下面是字段，不是字节级样例；写入器会把数组和对象拆成多行。

*The `pixelforge-kernel` crate owns the format. `Document::save_project` writes pretty-printed JSON and `Document::load_project` reads it. The page only triggers download and open through the wasm `saveProject` / `loadProject` methods. Unknown fields, missing fields, and any other `version` are rejected. The object below is the field list, not a byte-for-byte sample; the writer breaks arrays and objects across lines.*

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
  "frames": [
    { "pixels": [0, 1] },
    { "pixels": [1, 0] }
  ]
}
```

`pixels` 是行优先的调色板索引，长度等于 `width * height`。索引 0 表示空格子。`activeFrame` 是当前帧，从 0 计。调色板第 0 项必须是透明色。撤销和重做不在文件里；打开之后，撤销停在文件里的像素上。

*`pixels` are row-major palette indices and the length is `width * height`. Index 0 is an empty cell. `activeFrame` is the current frame, starting at 0. Palette entry 0 must be transparent. Undo and redo are not stored; after open, undo stops at the pixels in the file.*

## 这一版明确不包含

Godot、GDScript、C#、Python 推理、模型训练都不在仓库里。外部 AI 以后通过 Tool API 调用内核，见 [tech-stack.md](tech-stack.md)。

*Godot, GDScript, C#, a Python inference service, and model training are not in this repo. External AI will call the kernel through the Tool API later.*
