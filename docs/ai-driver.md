# Web editor model adapter { #ai-ai-driver }

The web editor can send a prompt and the current document snapshot to an OpenAI-compatible provider, then apply the returned tool calls to the open document. This integration requires a local Vite development or preview server with the provider proxy. For file-based automation, use the [CLI workflow](agent-skill.md).

## Configure the provider { #key-and-endpoint }

1. Set up the [web development environment](dev-setup.md).
2. Copy [apps/web/.env.example](https://github.com/starcloud530/pixelforge-ai/blob/main/apps/web/.env.example) to `apps/web/.env.local`. Set your endpoint, API key and a model ID available from your provider.
3. Restart `pnpm dev` and open the panel above the canvas. The Vite server reads the credentials and forwards requests. Keep the local environment file out of version control.

| Variable | Fallback | Value |
| --- | --- | --- |
| `PIXELFORGE_AI_API_KEY` | `OPENAI_API_KEY` | Provider credential |
| `PIXELFORGE_AI_BASE_URL` | `OPENAI_BASE_URL` | API root, default `https://api.openai.com/v1` |
| `PIXELFORGE_AI_MODEL` | `OPENAI_MODEL` | Model identifier, default `gpt-6-astra` |

A nonempty value in the running environment takes precedence over the file. For each setting the `PIXELFORGE_AI_*` name takes precedence over its fallback.

<span id="non-goals"></span>

## Request and data flow { #provider }

The browser posts to the same-origin path `/api/ai/chat/completions`. The proxy appends `/chat/completions` to the configured API root and sends the key in the Authorization header. The provider receives the prompt, tool definitions and `inspect` output, including current pixel indices, palette and frame metadata. Choose a provider appropriate for the artwork in the open project.

## Execute and inspect a turn { #run-a-turn }

1. The adapter reads `inspect`. If a gesture is still open, finish it and retry.
2. The configured model receives one completion request. The adapter executes any returned tool calls in order against the same document.
3. The panel displays each result and redraws the canvas. If a call fails, subsequent calls are skipped. Review earlier successful edits and undo unwanted changes before retrying.

The adapter groups edits for undo unless the model explicitly manages gestures. Each submission sends a new request. The model does not receive a second request with tool results from the same turn.

## Implementation { #where-it-lives }

The [adapter source](https://github.com/starcloud530/pixelforge-ai/tree/main/apps/web/src/ai) separates the tool schema (`catalog.ts`), HTTP client (`provider.ts`), execution loop (`turn.ts`) and server proxy (`proxy.ts`). A tool call supplies its own parameters. The adapter adds `apiVersion`, `tool` and `id` before calling the [Tool API](tool-api.md).

## Test the integration { #tests }

Run `pnpm test` from the repository root. The Node WASM tests exercise successful edits, interruption after a failed call, and undo using a simulated provider response.
