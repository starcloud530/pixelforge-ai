/**
 * Same-origin forwarder for the web shell.
 * The page posts to `/api/ai/chat/completions`. This function adds the bearer
 * key and calls the upstream host. The key stays out of the browser bundle.
 */

import { DEFAULT_AI_MODEL, readAiEndpoint, type EnvMap } from "./config";
import { completionsUrl } from "./provider";

export interface ProxyResult {
  status: number;
  contentType: string;
  body: string;
}

export interface AiProxyStatus {
  configured: boolean;
  model: string;
  /** Upstream API root. Null when no key is configured. */
  baseUrl: string | null;
}

/** What `GET /api/ai/status` returns. No API key. */
export function proxyStatus(env: EnvMap): AiProxyStatus {
  const endpoint = readAiEndpoint(env);
  if (!endpoint) {
    return { configured: false, model: DEFAULT_AI_MODEL, baseUrl: null };
  }
  return { configured: true, model: endpoint.model, baseUrl: endpoint.baseUrl };
}

/**
 * Forwards one chat-completion body. The upstream `model` field is the
 * configured model id, so the page cannot point the key at a different host.
 */
export async function forwardChatCompletion(input: {
  body: string;
  env: EnvMap;
  fetchImpl?: typeof fetch;
}): Promise<ProxyResult> {
  const endpoint = readAiEndpoint(input.env);
  if (!endpoint) {
    return jsonResult(400, { error: { message: "未设置 PIXELFORGE_AI_API_KEY 或 OPENAI_API_KEY。" } });
  }
  let url: string;
  try {
    url = completionsUrl(endpoint.baseUrl);
  } catch (error) {
    const message = error instanceof Error ? error.message : "模型接口地址无效。";
    return jsonResult(400, { error: { message } });
  }
  let body: string;
  try {
    body = withModel(input.body, endpoint.model);
  } catch (error) {
    const message = error instanceof Error ? error.message : "请求不是 JSON。";
    return jsonResult(400, { error: { message } });
  }
  const fetchImpl = input.fetchImpl ?? ((request, init) => fetch(request, init));
  try {
    const response = await fetchImpl(url, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${endpoint.apiKey}`,
        "Content-Type": "application/json",
      },
      body,
    });
    return {
      status: response.status,
      contentType: response.headers.get("content-type") ?? "application/json",
      body: await response.text(),
    };
  } catch (error) {
    const detail = error instanceof Error ? error.message : "网络请求失败";
    return jsonResult(502, { error: { message: `无法连接模型接口：${detail}` } });
  }
}

function withModel(raw: string, model: string): string {
  const parsed: unknown = JSON.parse(raw);
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    throw new Error("请求必须是 JSON 对象。");
  }
  return JSON.stringify({ ...parsed, model });
}

function jsonResult(status: number, body: unknown): ProxyResult {
  return {
    status,
    contentType: "application/json",
    body: JSON.stringify(body),
  };
}
