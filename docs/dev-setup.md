# 本地开发 Dev setup

像素工作室的第一版骨架：Rust 内核保存索引像素，TypeScript 外壳把这份缓冲用最近邻放大画出来。选型以 [tech-stack.md](tech-stack.md) 为准。

*First skeleton of the pixel studio. The Rust kernel stores indexed pixels. The TypeScript shell scales that buffer with nearest-neighbor sampling. [tech-stack.md](tech-stack.md) is the stack decision.*

## 前提 Prerequisites

- Rust stable（`cargo` 1.83 或更新）
- Node.js 20 或更新
- pnpm 10

仓库根目录同时有 Cargo workspace 和 pnpm workspace。两边各管一层。

*The repo root holds a Cargo workspace and a pnpm workspace. Each one owns its layer.*

## 布局 Layout

```text
Cargo.toml                 Cargo workspace
crates/pixelforge-kernel   索引像素、调色板、帧、画笔/橡皮
package.json               pnpm 脚本
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
- 一份内置调色板（`Palette::studio_default`）
- `paint` / `erase`
- 帧列表的增、复制、删（内核原语；外壳还没接）

工程文件、撤销栈、PNG / sprite sheet、Tool API 都不在这个 crate 里。

*`pixelforge-kernel` currently creates canvases with an edge from 1 to 256 (including the M1 sizes 16 and 32), stores indexed pixels with 0 as transparent, ships `Palette::studio_default`, and implements `paint` / `erase` plus add, duplicate, and delete on the frame list. Those frame methods are kernel primitives. The shell does not call them yet. Project files, the undo stack, PNG / sprite-sheet export, and the Tool API are not in this crate.*

## 网页外壳

在仓库根目录：

```bash
pnpm install
pnpm dev
```

浏览器打开 <http://localhost:5173>。

`pnpm build` 会先做 `tsc --noEmit`，再跑 Vite 生产构建。`pnpm typecheck` 只做类型检查。

画布路径：

1. `createKernel()`（`apps/web/src/kernel/client.ts`）拿到一份文档。
2. 外壳只读 `snapshot().pixels`，那是调色板索引，不是直接在界面里涂的颜色缓冲。
3. 视图先画到一张 1:1 的位图，再用 `imageSmoothingEnabled = false` 放大，所以格子保持最近邻、没有抗锯齿糊边。
4. 画笔和橡皮调用 `paint` / `erase`，然后重新读取 snapshot。

左键走当前工具，右键擦除。放大倍数只改视图，不改文档尺寸。

*Open <http://localhost:5173>. `pnpm build` typechecks, then runs the Vite production build. `pnpm typecheck` is types only.*

*The canvas asks `createKernel()` for a document, reads palette indices from `snapshot().pixels`, blits them into a 1:1 bitmap, and scales that bitmap with `imageSmoothingEnabled = false`. Paint and erase call the kernel and read the snapshot again. Scale changes the view, not the document size.*

## 内核桥 Kernel bridge

这一版没有把 Rust 编进浏览器。`wasm-pack` 和 `wasm32` 目标留到下一次再接，避免第一版骨架被绑定工具链挡住。

现在 `apps/web/src/kernel/stub.ts` 在内存里模仿 `Document` 的画笔/橡皮规则，调色板字节与 `Palette::studio_default` 相同。它只有一帧。Rust 侧的多帧方法没有抄到桩里。

计划中的桥：

1. 新增 `crates/pixelforge-kernel-wasm`，依赖 `pixelforge-kernel`，用 `wasm-bindgen` 暴露 `Document` 的尺寸、调色板、像素、`paint`、`erase`。
2. `apps/web` 加载生成的包。
3. 只改 `createKernel()` 的函数体，让它返回 wasm 文档。视图继续只认 `PixelKernel` 和 `snapshot()`。

界面和以后的外部模型都应该改这份内核状态，而不是另写一块像素缓冲。

*This slice does not compile Rust into the browser. `apps/web/src/kernel/stub.ts` mimics `Document` paint/erase in memory, copies `Palette::studio_default`, and keeps a single frame. The planned `crates/pixelforge-kernel-wasm` crate will wrap the Rust document with wasm-bindgen. `createKernel()` is the swap point. The view stays on `PixelKernel` and `snapshot()`.*

## 和 issue #2–#6 的关系

这一版是底座的目录和最小调用面，不关闭这些 issue。

| Issue | 骨架里有什么 | 还没做 |
| --- | --- | --- |
| [#2](https://github.com/starcloud530/pixelforge-ai/issues/2) 编辑器底座 | 内核 crate + 网页外壳能显示并改格子 | 手搓精灵并保存的整条验收 |
| [#3](https://github.com/starcloud530/pixelforge-ai/issues/3) 像素画布 | Rust `paint`/`erase`；外壳画笔、橡皮、整数倍缩放 | 撤销 / 重做 |
| [#4](https://github.com/starcloud530/pixelforge-ai/issues/4) 调色板 | 索引色和一份内置色板；画笔拒绝 0 号和越界索引 | 导入、编辑、锁定色板 |
| [#5](https://github.com/starcloud530/pixelforge-ai/issues/5) 多帧 | Rust 增帧、复制、删除；不能删掉最后一帧 | 帧列表面板。网页桩仍是单帧 |
| [#6](https://github.com/starcloud530/pixelforge-ai/issues/6) 工程文件 | 无 | 打开 / 保存。格式还没落地 |

*This slice does not close issues #2–#6. It adds the kernel crate and a shell that can show and edit cells. Undo, palette editing, the frame list UI, and project open/save are still open.*

## 计划中的工程文件 Project file

还没有读写代码。以后的文件由 Rust 内核负责，界面和 Tool API 走同一份解析。草图如下，实现时可以改，现在不要当成已定格式。

*Nothing reads or writes a project file yet. Rust will own that I/O. The sketch below is a note, not a frozen format.*

```json
{
  "format": "pixelforge-project",
  "version": 1,
  "width": 16,
  "height": 16,
  "palette": [{ "r": 0, "g": 0, "b": 0, "a": 0 }],
  "frames": [{ "pixels": [0, 1, 2] }]
}
```

`pixels` 是行优先的调色板索引，长度等于 `width * height`。索引 0 表示空格子。

## 这一版明确不包含

Godot、GDScript、C#、Python 推理、模型训练都不在仓库里。外部 AI 以后通过 Tool API 调用内核，见 [tech-stack.md](tech-stack.md)。

*Godot, GDScript, C#, a Python inference service, and model training are not in this repo. External AI will call the kernel through the Tool API later.*
