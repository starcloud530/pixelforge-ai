# PixelForge

Follow `AGENTS.md` and `.cursor/skills/pixelforge/SKILL.md`.

Edit pixels only through the `pixelforge` CLI and the kernel Tool API. The in-app AI panel is a one-turn demo. Do not build an embedded multi-turn chat (#14), and do not add an MCP server. Cells store palette indices. A frame can hold several layers of those indices. Draw the active layer; `preview` and `export` show the composite. Do not invent RGB past the kernel. Recipes: `docs/agent-skill.md`.
