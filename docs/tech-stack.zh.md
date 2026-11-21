# 运行时架构 { #tech-stack }

PixelForge 的命令行工具和网页编辑器共用 Rust 文档模型。本页面向集成开发者，说明读取、修改和导出工程时各组件的职责。

<span id="product-premise"></span><span id="why"></span><span id="tauri"></span><span id="9-roadmap"></span>

## 选择调用入口 { #choice }

| 环境 | 入口 | 交换的数据 |
| --- | --- | --- |
| Shell 或 Agent | `pixelforge` CLI | 本地工程文件、标准输出中的 JSON |
| Rust 应用 | `Document::call_tool` | 带版本的 JSON 请求与响应 |
| 网页编辑器 | TypeScript 调用 WASM 绑定 `callTool` | 针对内存中 Rust 文档的请求 |

参数、结果与错误码见 [Tool API 参考](tool-api.md)。

<span id="in-one-line"></span><span id="_1"></span>

## 一次编辑如何写入工程 { #boundaries }

1. 调用方指定工程、帧和图层。CLI 从磁盘打开文件，浏览器使用已加载至内存的文档。
2. 内核校验操作，并按索引色规则处理色板锁定、选区蒙版等约束。
3. 调用方读取结构化结果。CLI 将修改写回磁盘。网页编辑器根据更新后的文档重绘视图。
4. 导出时，内核合成可见图层并编码输出。游戏可独立读取这些导出文件。

<span id="rust_1"></span>

## 工程状态与持久化 { #rust }

工程 JSON 保存画布尺寸、色板、图层、帧和时长。内核还负责当前编辑状态和撤销记录。常规保存会省略撤销栈；CLI 工作流可通过检查点在后续进程中撤销修改。详见[编辑与导出限制](known-limits.md)。

<span id="typescript_1"></span>

## 渲染与交互 { #typescript }

TypeScript 处理指针输入、时间轴控件、文件选择和最近邻显示。WASM 桥接层把 Rust 操作暴露给网页界面。构建与测试步骤见[开发环境](dev-setup.md)。

<span id="ai"></span><span id="ai_1"></span><span id="non-goals"></span>

## Agent 与游戏引擎接入 { #ai-external-ai }

Agent 可以读取工程状态，通过 CLI 提交修改，查看预览后继续操作。需要为网页编辑器接入外部模型时，参阅[网页模型适配器](ai-driver.md)。游戏引擎接入见[精灵表、图集与地图导出约定](export.md)。
