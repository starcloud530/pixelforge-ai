# 给外部 Agent 的 skill Agent skill

产品路径是：**仓库外的 Agent → 本地 `pixelforge` CLI → Rust 内核 Tool API**。Agent 不改界面上的像素缓冲，也不在这个仓库里做多轮聊天。网页里的一轮面板是可选演示，见 [ai-driver.md](ai-driver.md)。[#14](https://github.com/starcloud530/pixelforge-ai/issues/14) 那种嵌在应用里的多轮对话不是产品。MCP 不在这一阶段；以后若要接，只做这支 CLI 的薄封装。

Cursor 从 [.cursor/skills/pixelforge/SKILL.md](../.cursor/skills/pixelforge/SKILL.md) 读取同一份操作说明。Codex、Claude Code 或其他能跑命令的 Agent 按本文件做即可。

*The product path is an agent outside this repository, then the local `pixelforge` CLI, then the Rust kernel Tool API. The agent does not patch the pixel buffer the view draws, and this repository does not host a multi-turn chat. The one-turn panel in the web shell is an optional demo; see [ai-driver.md](ai-driver.md). An embedded multi-turn chat like [#14](https://github.com/starcloud530/pixelforge-ai/issues/14) is not the product. MCP is out of this stage. A later wrapper, if any, would be a thin layer over this CLI.*

*Cursor loads [.cursor/skills/pixelforge/SKILL.md](../.cursor/skills/pixelforge/SKILL.md). Codex, Claude Code, and any other agent that can run a command follow this file.*

## 安装与运行 Install and run

在仓库根目录：

```bash
cargo install --path crates/pixelforge-cli
pixelforge --help
```

不安装也可以：

```bash
cargo run -p pixelforge-cli -- --help
```

需要 Rust stable（`cargo` 1.83 或更新）。没有 Python 服务。

*From the repository root, `cargo install --path crates/pixelforge-cli` puts `pixelforge` on the path. `cargo run -p pixelforge-cli -- --help` runs it without installing. Rust stable (`cargo` 1.83 or newer) is enough. There is no Python service.*

## 输出 Output

stdout 是紧凑 JSON，一行一个响应，给 Agent 解析。stderr 是给人看的短句，例如 `pixelforge: wrote hero.json`。不要解析 stderr。

`call` 打出的就是 Tool API 响应，和 [tool-api.md](tool-api.md) 一致。`export` 子命令把 PNG 写到磁盘，stdout 里是路径和网格尺寸，不带 base64。要用 base64，就 `call` 工具 `exportPngSequence` 或 `exportSpriteSheet`，那两个工具不写文件。

退出码：`0` 全部 `ok: true`。`1` 某个工具 `ok: false`，stdout 仍是那个错误对象。`2` 工具没跑起来（文件打不开、工程损坏、请求文件不是 JSON）。缺了必填参数时 clap 把说明打到 stderr，也是退出码 `2`，这时没有 JSON 行。

*Stdout is compact JSON, one response per line. Stderr is a short note such as `pixelforge: wrote hero.json`. Parse stdout. `call` prints the Tool API response from [tool-api.md](tool-api.md). The `export` subcommand writes PNG files and prints paths and the grid size, without base64. Base64 comes from `call` with `exportPngSequence` or `exportSpriteSheet`, and those calls do not write files.*

*Exit `0` means every tool returned `ok: true`. Exit `1` means a tool returned `ok: false` and stdout still holds that error object. Exit `2` means the tool did not run (unreadable file, bad project, request file is not JSON). A missing flag is also exit `2`: clap writes the usage text to stderr and there is no JSON line.*

## 不变量 Invariants

- 格子存的是调色板索引。索引 0 是透明。`paint` 写入已经存在的 `1..=255` 索引，不写 RGB。
- 新建工程带工作室调色板，索引 1 到 8 可以直接画。要新颜色，先 `palette add` 或 `palette set`，再用返回的索引去画。
- `palette lock` 之后仍可画已有索引，但不能改色、追加或导入。
- 不要手改工程 JSON 去塞一个颜色。越界索引会让文件打不开。不要把扩散图写回工程。
- 一次进程是一份文档。同一次 `pixelforge call` 里的多个 `--request` 共享撤销栈。工程文件不保存撤销栈，下一个进程是空栈，和在编辑器里重新打开这份文件一样。对刚打开的文件执行 `pixelforge undo` 会得到 `NothingToUndo`。
- 失败的那一次调用不改文档。一批请求里，失败之前的成功调用会保存，失败之后的不再执行。
- 导出在内核里把索引解析成 RGBA。不要自己给 PNG 重新上色。洋葱皮只是视图，没有把它写回像素的工具。

*Cells store palette indices. Index 0 is transparent. `paint` writes an existing index, never RGB. A new project ships the studio palette, indices 1 through 8. Add or recolor a palette entry before painting a new color. A locked palette still accepts paint of an existing index and refuses recolor, append, and import. Do not hand-edit the project JSON to plant a color, and do not write a diffusion image back into the project.*

*One process is one document. Several `--request` flags on one `pixelforge call` share the undo stack. The project file does not store that stack, so the next process starts empty, the same as reopening the file. `pixelforge undo` on a freshly opened file returns `NothingToUndo`. A refused call does not change the document. Earlier calls in that batch are saved; later calls are not run. Export resolves indices in the kernel. Onion skin is a view and has no tool.*

## 配方 Recipes

### 空白工程 Blank project

```bash
pixelforge new --width 16 --height 16 --output hero.json
```

stdout 是 `inspect` 的响应。文件是 version 1 `pixelforge-project`。`new` 不会覆盖已经存在的文件。

*Stdout is the `inspect` response. The file is version 1 `pixelforge-project`. `new` refuses to replace a file that already exists.*

### 画一格、擦一格 Paint and erase

```bash
pixelforge paint --project hero.json --x 1 --y 2 --color-index 3
pixelforge erase --project hero.json --x 1 --y 2
```

### 一笔撤销 A stroke as one undo

手势和撤销必须在同一次进程里：

*The gesture and the undo have to be the same process:*

```bash
pixelforge call --project hero.json \
  --request '{"tool":"beginGesture"}' \
  --request '{"tool":"paint","params":{"x":1,"y":2,"colorIndex":3}}' \
  --request '{"tool":"paint","params":{"x":2,"y":2,"colorIndex":3}}' \
  --request '{"tool":"endGesture"}'
```

要在进程结束前撤回这一笔，把 `--request '{"tool":"undo"}'` 加在同一条命令里。进程结束之后栈就没了；需要回头时先复制文件（`cp hero.json hero.json.bak`）。

*Add `--request '{"tool":"undo"}'` to that same command to revert the stroke before the process exits. After it exits the stack is gone. Copy the file first when you may need the previous pixels.*

### 走路帧 Walk frames

```bash
pixelforge call --project hero.json \
  --request '{"tool":"duplicateFrame","params":{"index":0}}' \
  --request '{"tool":"setActiveFrame","params":{"index":1}}' \
  --request '{"tool":"setFrameDelay","params":{"index":1,"delayMs":120}}' \
  --request '{"tool":"paint","params":{"x":0,"y":0,"colorIndex":1}}'
pixelforge export sheet --project hero.json --output hero-sheet.png
pixelforge export png --project hero.json --out-dir ./hero-frames
```

`duplicateFrame` 把像素和时长复制到下一格。`frame add` 在末尾追加一帧 100 毫秒的空白帧，不改变当前帧。`export png` 写出 `pixelforge-000.png` 起的文件。`export sheet` 的网格是内核的 `ceil(sqrt(帧数))` 列。约定见 [export.md](export.md)。

同一批修改也可以拆成子命令。子命令各自是一次进程，撤销栈不跨进程：

*The same edits as separate commands. Each command is its own process, so the undo stack does not carry across them:*

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
```

`palette add` 的结果里有新索引，画笔用那个索引。`import` 的 `mode` 是 `replace` 或 `merge`，文件是 `.gpl` 或十六进制列表。色板锁定时导入会被拒绝。

*`palette add` returns the new index. Paint that index. `import` takes `replace` or `merge` and a `.gpl` file or a hex list. A locked palette refuses the import.*

### 查看与原始调用 Inspect and raw calls

```bash
pixelforge inspect --project hero.json
pixelforge tools
pixelforge call --project hero.json --request-file edits.json
```

`edits.json` 可以是一个 JSON 对象、一个 JSON 数组，或每行一个 JSON 值。也可以把 JSON 从 stdin 送进 `pixelforge call --project hero.json`。每个 `--request` 是一个请求对象，原样交给 `Document::call_tool`。

工具名、参数和 `error.code` 见 [tool-api.md](tool-api.md)。

*`edits.json` may be one JSON object, a JSON array, or one JSON value per line. Stdin works the same way: `pixelforge call --project hero.json`. Each `--request` is one request object and is passed to `Document::call_tool`. Names, params, and `error.code` values are in [tool-api.md](tool-api.md).*
