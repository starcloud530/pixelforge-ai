# PixelForge agents

Edit pixels only through the `pixelforge` CLI (`crates/pixelforge-cli`) and the kernel Tool API.

Read `.cursor/skills/pixelforge/SKILL.md` before creating or changing a `pixelforge-project`, its frames, palette, or exports. The same recipes are in `docs/agent-skill.md`.

The in-app AI panel is a one-turn demo. Do not build an embedded multi-turn chat (#14). Do not add an MCP server in this tree; a later wrapper would only call this CLI. Cells store palette indices. A frame can hold several layers of those indices. Draw the active layer; `preview` and `export` show the composite. Do not invent RGB past the kernel.
