/**
 * Endpoint settings for an OpenAI-compatible chat completions API.
 * The model name is only a label: Astra and other GPT-class hosts swap in
 * by changing the base URL and the model id. No key is stored in the repo.
 */

export const DEFAULT_AI_BASE_URL = "https://api.openai.com/v1";
/** Default model id. Replace it when the host uses another GPT-class name. */
export const DEFAULT_AI_MODEL = "gpt-6-astra";

export interface AiEndpoint {
  apiKey: string;
  /** API root, with no `/chat/completions` suffix and no trailing slash. */
  baseUrl: string;
  model: string;
}

export type EnvMap = Record<string, string | undefined>;

/**
 * Reads the endpoint. `PIXELFORGE_AI_*` wins over `OPENAI_*`.
 * Returns null when neither API key is set. An empty string counts as unset.
 */
export function readAiEndpoint(env: EnvMap): AiEndpoint | null {
  const apiKey = firstNonEmpty(env, ["PIXELFORGE_AI_API_KEY", "OPENAI_API_KEY"]);
  if (!apiKey) {
    return null;
  }
  const baseUrl = stripTrailingSlash(
    firstNonEmpty(env, ["PIXELFORGE_AI_BASE_URL", "OPENAI_BASE_URL"]) ?? DEFAULT_AI_BASE_URL,
  );
  const model = firstNonEmpty(env, ["PIXELFORGE_AI_MODEL", "OPENAI_MODEL"]) ?? DEFAULT_AI_MODEL;
  return { apiKey, baseUrl, model };
}

function firstNonEmpty(env: EnvMap, names: readonly string[]): string | null {
  for (const name of names) {
    const value = env[name];
    if (typeof value === "string" && value.trim() !== "") {
      return value.trim();
    }
  }
  return null;
}

function stripTrailingSlash(url: string): string {
  return url.endsWith("/") ? url.slice(0, -1) : url;
}
