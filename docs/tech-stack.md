# Runtime architecture { #tech-stack }

PixelForge shares one Rust document model across the command-line interface and the web editor. This page describes the components involved when an integration reads, edits or exports a project.

<span id="product-premise"></span><span id="why"></span><span id="tauri"></span><span id="9-roadmap"></span>

## Select an entry point { #choice }

| Environment | Entry point | Data exchanged |
| --- | --- | --- |
| Shell or agent | `pixelforge` CLI | Local project files, JSON on stdout |
| Rust application | `Document::call_tool` | Versioned JSON request and response |
| Web editor | TypeScript with the WASM binding `callTool` | Requests to an in-memory Rust document |

For accepted fields, results and error codes, see the [Tool API reference](tool-api.md).

<span id="in-one-line"></span><span id="_1"></span>

## Follow an edit { #boundaries }

1. The caller selects the project, frame and layer. The CLI opens a file on disk. The browser works with the document already loaded in memory.
2. The kernel validates the requested operation and applies indexed-color rules, including palette locks and selection masks.
3. The caller reads the result. The CLI saves modifying operations to disk. The web editor redraws the current view from the updated document.
4. During export, the kernel composites the visible layers and encodes the output. A game can consume the exported files independently of the editor.

<span id="rust_1"></span>

## Document state and persistence { #rust }

Project JSON stores canvas dimensions, palette, layers, frames and delays. The kernel also owns the active edit state and undo history. Regular project saves omit the undo stack. A CLI workflow can use a checkpoint for undo across processes. See [editing limits](known-limits.md).

<span id="typescript_1"></span>

## Rendering and interaction { #typescript }

TypeScript handles pointer input, timeline controls, file dialogs and nearest-neighbor display. The WASM bridge exposes Rust operations to the interface. Build and test the bridge using the [development setup](dev-setup.md).

<span id="ai"></span><span id="ai_1"></span><span id="non-goals"></span>

## Agent and engine integrations { #ai-external-ai }

An agent can read project state, submit edits to the CLI and inspect a preview before continuing. An application embedding the browser provider can use [the web editor’s model adapter](ai-driver.md). For engine imports, use the [sprite sheet, atlas and tilemap conventions](export.md).
