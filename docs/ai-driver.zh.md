# 网页编辑器的模型适配器 { #ai-ai-driver }

网页编辑器可将提示和当前文档快照发送给兼容 OpenAI 接口的服务，再将返回的工具调用应用到已打开的工程。这项集成需要运行带模型代理的本地 Vite 开发或预览服务。需要编写脚本读写磁盘工程时，可直接使用 [CLI 工作流](agent-skill.md)。

## 配置模型服务 { #key-and-endpoint }

1. 按[开发环境指南](dev-setup.md)准备网页开发环境。
2. 将 [apps/web/.env.example](https://github.com/starcloud530/pixelforge-ai/blob/main/apps/web/.env.example) 复制为 `apps/web/.env.local`，填写服务地址、API 密钥和服务方提供的模型标识。
3. 重新启动 `pnpm dev`，打开画布上方的模型面板。Vite 服务会读取凭据并转发请求。请将本地环境文件排除在版本控制之外。

| 环境变量 | 备用变量 | 取值 |
| --- | --- | --- |
| `PIXELFORGE_AI_API_KEY` | `OPENAI_API_KEY` | 服务方颁发的密钥 |
| `PIXELFORGE_AI_BASE_URL` | `OPENAI_BASE_URL` | API 根地址，默认 `https://api.openai.com/v1` |
| `PIXELFORGE_AI_MODEL` | `OPENAI_MODEL` | 模型标识，默认 `gpt-6-astra` |

运行环境中已有的非空值优先于文件配置。每项设置优先使用 `PIXELFORGE_AI_*`，其次使用对应的备用变量。

<span id="non-goals"></span>

## 请求与数据流 { #provider }

浏览器向同源路径 `/api/ai/chat/completions` 发起请求。代理在配置的根地址后追加 `/chat/completions`，并在 Authorization 请求头中附加密钥。模型服务会收到提示、工具定义和 `inspect` 的结果，包括当前像素索引、色板和帧信息。请按当前工程的数据要求选择服务方。

## 执行并检查一轮操作 { #run-a-turn }

1. 适配器先调用 `inspect`。如果存在尚未结束的绘制手势，请结束手势后重试。
2. 模型收到一次补全请求。适配器按返回顺序在同一份文档上执行工具调用。
3. 面板显示各次调用的结果并刷新画布。某次调用出错后，后续调用会停止。重试前请检查已成功的修改，按需撤销。

默认情况下，适配器把修改合并为一个撤销步骤；模型显式管理手势时，沿用模型的分组。每次提交都会发出新请求，一轮中的工具结果保留在本地面板。

## 实现位置 { #where-it-lives }

[适配器源码](https://github.com/starcloud530/pixelforge-ai/tree/main/apps/web/src/ai)分别实现工具定义（`catalog.ts`）、HTTP 客户端（`provider.ts`）、执行循环（`turn.ts`）与服务端代理（`proxy.ts`）。模型返回各工具的参数，适配器补充 `apiVersion`、`tool` 和 `id` 后调用 [Tool API](tool-api.md)。

## 验证集成 { #tests }

在仓库根目录运行 `pnpm test`。测试使用模拟服务响应，在 Node WASM 中验证工具写入、遇错中止和撤销。
