import { loadEnv, defineConfig, type Connect, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import type { EnvMap } from "./src/ai/config";
import { forwardChatCompletion, proxyStatus } from "./src/ai/proxy";

const AI_ENV_NAMES = [
  "PIXELFORGE_AI_API_KEY",
  "OPENAI_API_KEY",
  "PIXELFORGE_AI_BASE_URL",
  "OPENAI_BASE_URL",
  "PIXELFORGE_AI_MODEL",
  "OPENAI_MODEL",
] as const;

/** File env, with a non-empty process env winning so a shell can override `.env.local`. */
function aiEnv(mode: string, envDir: string): EnvMap {
  const merged: EnvMap = { ...loadEnv(mode, envDir, ["PIXELFORGE_", "OPENAI_"]) };
  for (const name of AI_ENV_NAMES) {
    const value = process.env[name];
    if (typeof value === "string" && value.trim() !== "") {
      merged[name] = value;
    }
  }
  return merged;
}

/**
 * The browser posts to this origin. The key is read here and attached on the
 * way out, so it is not inlined into the page.
 */
function aiProxyPlugin(): Plugin {
  const attach = (mode: string, envDir: string): Connect.NextHandleFunction => {
    const fileEnv = aiEnv(mode, envDir);
    return (req, res, next) => {
      const path = (req.url ?? "").split("?")[0] ?? "";
      if (path === "/api/ai/status" && req.method === "GET") {
        res.statusCode = 200;
        res.setHeader("Content-Type", "application/json");
        res.end(JSON.stringify(proxyStatus(fileEnv)));
        return;
      }
      if (path !== "/api/ai/chat/completions") {
        next();
        return;
      }
      if (req.method !== "POST") {
        res.statusCode = 405;
        res.end();
        return;
      }
      const chunks: Buffer[] = [];
      req.on("data", (chunk: Buffer | string) => {
        chunks.push(typeof chunk === "string" ? Buffer.from(chunk) : chunk);
      });
      req.on("end", () => {
        const raw = Buffer.concat(chunks).toString("utf8");
        void forwardChatCompletion({ body: raw, env: fileEnv })
          .then((result) => {
            res.statusCode = result.status;
            res.setHeader("Content-Type", result.contentType);
            res.end(result.body);
          })
          .catch(() => {
            res.statusCode = 502;
            res.setHeader("Content-Type", "application/json");
            res.end(JSON.stringify({ error: { message: "模型接口失败。" } }));
          });
      });
    };
  };

  return {
    name: "pixelforge-ai-proxy",
    configureServer(server) {
      server.middlewares.use(attach(server.config.mode, server.config.envDir || server.config.root));
    },
    configurePreviewServer(server) {
      server.middlewares.use(attach(server.config.mode, server.config.envDir || server.config.root));
    },
  };
}

export default defineConfig({
  plugins: [react(), aiProxyPlugin()],
  server: {
    host: "0.0.0.0",
    port: 5173,
  },
});
