# 外部 AI 编排 AI driver

产品路径是仓库外的 Agent，不是应用里的聊天。Agent 调用本地 `pixelforge` CLI，CLI 把 Tool API JSON 交给 Rust 内核。技能表、配方和不变式见 [agent-skill.md](agent-skill.md)，目录见 [tool-api.md](tool-api.md)。产品 skill 在 `skills/`，已安装的 CLI 用 `pixelforge skills list` / `skills read` 读取。

这一页剩下的内容是网页外壳里的**可选一轮演示**。它做一轮：用户写一句提示，编排向外部模型要一次 chat completion，把回复里的 `tool_calls` 按顺序交给 Rust Tool API，然后外壳从同一份 `Document` 刷新画面。多轮记忆、规划和自然语言润色不在这个仓库里做。[#14](https://github.com/starcloud530/pixelforge-ai/issues/14) 那种嵌在应用里的多轮对话不是产品；对话留在外部 Agent。MCP 不在这一阶段，以后若要接，只做 CLI 的薄封装。

*The product path is an agent outside this repository, not an in-app chat. The agent runs the local `pixelforge` CLI, and the CLI hands Tool API JSON to the Rust kernel. The skill table, recipes, and invariants: [agent-skill.md](agent-skill.md). Catalog: [tool-api.md](tool-api.md). Product skills live in `skills/` and are read with `pixelforge skills list` / `skills read`.*

*The rest of this page is the **optional one-turn demo** in the web shell. One turn: the user writes a prompt, the driver asks an external model for one chat completion, runs that reply's `tool_calls` in order through the Rust Tool API, and the shell refreshes from the same `Document`. Multi-turn memory, planning, and revision polish are not built in this repository. An embedded multi-turn chat like [#14](https://github.com/starcloud530/pixelforge-ai/issues/14) is not the product; the conversation stays with the external agent. MCP is out of this stage. A later wrapper, if any, would be a thin layer over the CLI.*

## 它在哪 Where it lives

首选入口是 `pixelforge` CLI，见 [agent-skill.md](agent-skill.md)。下面这个演示编排在 `apps/web/src/ai`。模型不在这个仓库里，也没有 Python 推理服务。像素、调色板、帧、时长、撤销、工程文件和导出只通过 `Document::call_tool`（外壳是 `callDocumentTool`，浏览器里是 wasm `callTool`，CLI 调用的是同一个方法）。目录见 [tool-api.md](tool-api.md)。编排层不改界面上的像素缓冲。

*The preferred entry is the `pixelforge` CLI; see [agent-skill.md](agent-skill.md). The demo driver below is `apps/web/src/ai`. The model is not in this repository, and there is no Python inference service. Pixels, palette, frames, delays, undo, project files, and export go through `Document::call_tool` only (the shell helper is `callDocumentTool`; in the browser that is wasm `callTool`; the CLI calls that same method). The catalog is [tool-api.md](tool-api.md). The driver does not patch the pixel buffer the view draws.*

| 文件 File | 职责 Responsibility |
| --- | --- |
| `catalog.ts` | 发给模型的工具定义，名字和字段跟 Tool API 一致 |
| `provider.ts` | `OpenAiCompatibleProvider`：一次 `POST {base}/chat/completions` |
| `turn.ts` | `runAgentTurn`：一次补全，然后按顺序执行工具 |
| `config.ts` | 从环境变量读密钥、地址和模型名 |
| `proxy.ts` | 本地服务把 `/api/ai/chat/completions` 转到上游，并附上密钥 |
| `AiPanel.tsx` | 画布上方的提示框：粘贴提示，运行一轮，列出工具调用 |

## 提供方 Provider

`OpenAiCompatibleProvider` 说的是 HTTP 形状，不是某一家 SDK。默认模型名是 `gpt-6-astra`。换 Astra 或别的 GPT 类端点时，改基地址和模型名即可。基地址是 API 根，例如 `https://api.openai.com/v1`，不要自己拼上 `/chat/completions`。驱动会补上。

请求体是 OpenAI chat completions：`model`、`messages`、`tools`、`tool_choice: "auto"`、`parallel_tool_calls: false`。工具参数是该工具自己的 `params` 对象，不是 Tool API 信封。编排再补上 `apiVersion`、`tool`、`params` 和 `id`。密钥只放在 `Authorization: Bearer` 头里，不进消息，也不进仓库。

一轮只请求一次。回复里的多个 `tool_calls` 按数组顺序执行，不并行，也不把工具结果送回模型。

*The provider is the OpenAI chat-completions HTTP shape, not a vendor SDK. The default model id is `gpt-6-astra`. Point `baseUrl` and the model id at an Astra host or any other GPT-class host. The base URL is the API root, such as `https://api.openai.com/v1`, without `/chat/completions`; the driver appends that path.*

*The body is `model`, `messages`, `tools`, `tool_choice: "auto"`, and `parallel_tool_calls: false`. Each tool's arguments are that tool's `params` object, not the Tool API envelope. The driver adds `apiVersion`, `tool`, `params`, and `id`. The key travels only in the `Authorization: Bearer` header. It is not part of the message and it is not committed.*

*One turn is one request. Several `tool_calls` in that reply run in array order. They are not parallel, and their results are not sent back to the model.*

## 密钥与地址 Key and endpoint

| 变量 Variable | 作用 Role |
| --- | --- |
| `PIXELFORGE_AI_API_KEY`，否则 `OPENAI_API_KEY` | 密钥。两个都空着则界面不能运行 |
| `PIXELFORGE_AI_BASE_URL`，否则 `OPENAI_BASE_URL` | API 根。默认 `https://api.openai.com/v1` |
| `PIXELFORGE_AI_MODEL`，否则 `OPENAI_MODEL` | 模型名。默认 `gpt-6-astra` |

`PIXELFORGE_AI_*` 优先于 `OPENAI_*`。空字符串当成没设置。已经在环境里的非空值盖过 `.env.local`。

本地网页不把密钥编进页面。画布上的面板请求同源的 `/api/ai/chat/completions`，`pnpm dev` 和 `pnpm preview` 里的 Vite 插件读密钥，再转发到上面的基地址，并带上 `Authorization: Bearer`。把 [apps/web/.env.example](../apps/web/.env.example) 复制成 `apps/web/.env.local`，填上密钥，重新启动。不要提交 `.env.local`。

`OpenAiCompatibleProvider` 自己也能直接打上游。测试和以后的非浏览器外壳这样用：把密钥交给 provider，不经过页面。

*`PIXELFORGE_AI_*` wins over `OPENAI_*`. An empty string counts as unset. A non-empty value already in the environment overrides `.env.local`.*

*The local page does not bundle the key. The canvas panel posts to the same origin at `/api/ai/chat/completions`. The Vite plugin in `pnpm dev` and `pnpm preview` reads the key and forwards to the base URL above with `Authorization: Bearer`. Copy [apps/web/.env.example](../apps/web/.env.example) to `apps/web/.env.local`, set the key, and start again. Do not commit `.env.local`.*

*`OpenAiCompatibleProvider` can also call the upstream host itself. Tests and a future non-browser shell do that: pass the key to the provider, not through the page.*

## 跑一轮 Run a turn

打开网页后，画布上方是「外部模型」。粘贴提示，点「运行一轮」。

1. 编排先 `inspect`。手势还开着就拒绝，不请求模型，文档不变。
2. 用户消息里带上这次 `inspect` 的结果，让模型看见当前格子、色板和帧。
3. 模型回复后，编排按顺序调用 Tool API。
4. 某个调用的参数不是 JSON 对象，或内核返回 `ok: false`，这一轮停下，后面的调用不发送。失败的那一次本身不改文档（`loadProject` 只有解析成功才换文档）。
5. 外壳用内核的 snapshot 重画画布。列出的是模型发出的工具调用，不是第二份像素缓冲。

撤销：如果模型没有自己调用 `beginGesture`，编排用 `beginGesture` … `endGesture` 把这一轮收成一步，一次撤销退回整轮。模型如果自己管理手势，编排不再包一层；轮末手势还开着时，编排会 `endGesture`，避免界面停在 `OpenGesture`。空的手势不进撤销栈。只读工具（`inspect`、`scene`、`saveProject`、两个导出、`previewFrame`、`diffFrames`、`listLayers`）本来就不进栈。

运行的时候画布和快捷键先停住，避免和这一轮交错。播放也会先停下；停播放不进撤销栈。

*On the page, the panel above the canvas is 外部模型. Paste a prompt and press 运行一轮.*

1. *The driver calls `inspect` first. An open gesture refuses the turn: no model request, and the document stays as it was.*
2. *The user message includes that inspect result so the model sees the current cells, palette, and frames.*
3. *After the reply, the driver calls the Tool API in order.*
4. *If arguments are not a JSON object, or the kernel returns `ok: false`, the turn stops and later calls are not sent. The failed call itself does not change the document (`loadProject` replaces it only after the file parses).*
5. *The shell redraws from the kernel snapshot. The list is the model's tool calls, not a second pixel buffer.*

*Undo: when the model does not call `beginGesture` itself, the driver wraps the reply in `beginGesture` … `endGesture`, so one undo restores the whole turn. When the model manages the gesture, the driver does not wrap another one. A gesture still open at the end is closed with `endGesture`, so the editor is not stuck on `OpenGesture`. An empty gesture is not recorded. Read-only tools (`inspect`, `scene`, `saveProject`, both exports, `previewFrame`, `diffFrames`, and `listLayers`) are not recorded either.*

*While the turn runs, the canvas and keyboard shortcuts pause so they do not interleave. Playback stops first. Stopping playback is not an undo entry.*

## 测试 Tests

在仓库根目录：

```bash
pnpm test
```

这会把内核编成 Node 可用的 wasm（`crates/pixelforge-kernel-wasm/pkg-node`，不入库），再用假的 `fetch` 跑 `apps/web/src/ai/turn.test.ts`。测试断言：一段写好的 `tool_calls` 打进 Tool API，`Document` 的格子、帧和时长跟着变；失败的调用挡住后面的调用；一次撤销能退回。没有活的网络请求。

*`pnpm test` builds the kernel for Node (`crates/pixelforge-kernel-wasm/pkg-node`, not committed) and runs `apps/web/src/ai/turn.test.ts` with a fake `fetch`. A canned `tool_calls` sequence hits the Tool API and changes the document's cells, frames, and delays. A failed call blocks the calls after it. One undo restores the turn. There is no live network.*

## 非目标 Non-goals

- 不把工具结果送回模型，也不在应用里保存对话。外部 Agent 自己保留多轮。#14 那种嵌入式聊天不是产品。
- 不在这一阶段做 MCP 服务器。
- 不训练模型，不新增 Python 服务，不把扩散图写回工程。
- 终稿仍然只存在于内核的索引色网格上。见 [tech-stack.md](tech-stack.md)。

*Out of scope: feeding tool results back to the model, and keeping a conversation inside the app. The external agent keeps its own turns. An embedded chat like #14 is not the product. An MCP server is out of this stage. We do not train a model, add a Python service, or write a diffusion image back into the project. Finished pixels still live only on the kernel's indexed grid. See [tech-stack.md](tech-stack.md).*
