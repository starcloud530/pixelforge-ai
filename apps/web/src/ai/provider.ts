/**
 * One pluggable chat provider. The HTTP shape is OpenAI chat completions,
 * so a GPT-class or Astra-class host is selected with a base URL and model id.
 * The API key stays in the request header and is never written into the body.
 */

import type { ChatTool } from "./catalog";
import type { LanguageModel, ModelCompletion, ModelMessage, ModelToolCall } from "./turn";

export class ProviderError extends Error {
  readonly code: "MissingApiKey" | "BadUrl" | "Http" | "BadResponse";

  constructor(code: ProviderError["code"], message: string) {
    super(message);
    this.name = "ProviderError";
    this.code = code;
  }
}

export interface OpenAiCompatibleOptions {
  apiKey: string;
  /** API root, for example `https://api.openai.com/v1`. */
  baseUrl: string;
  /** Test seam. Production uses the platform `fetch`. */
  fetchImpl?: typeof fetch;
}

/**
 * Posts one chat completion and returns the assistant text plus tool calls.
 * Does not talk to the kernel.
 */
export class OpenAiCompatibleProvider implements LanguageModel {
  private readonly apiKey: string;
  private readonly baseUrl: string;
  private readonly fetchImpl: typeof fetch;

  constructor(options: OpenAiCompatibleOptions) {
    this.apiKey = options.apiKey.trim();
    this.baseUrl = options.baseUrl.trim();
    // Calling `fetch` as a method drops the window receiver and throws
    // "illegal invocation" in the browser. A wrapper keeps the real fetch bound.
    this.fetchImpl = options.fetchImpl ?? ((input, init) => fetch(input, init));
  }

  async complete(input: {
    model: string;
    messages: readonly ModelMessage[];
    tools: readonly ChatTool[];
  }): Promise<ModelCompletion> {
    if (this.apiKey === "") {
      throw new ProviderError("MissingApiKey", "未设置 API 密钥。");
    }
    const url = completionsUrl(this.baseUrl);
    const body = {
      model: input.model,
      messages: input.messages,
      tools: input.tools,
      tool_choice: "auto",
      parallel_tool_calls: false,
    };
    let response: Response;
    try {
      response = await this.fetchImpl(url, {
        method: "POST",
        headers: {
          Authorization: `Bearer ${this.apiKey}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify(body),
      });
    } catch (error) {
      const detail = error instanceof Error ? error.message : "网络请求失败";
      throw new ProviderError("Http", `无法连接模型接口：${detail}`);
    }
    const text = await response.text();
    if (!response.ok) {
      throw new ProviderError("Http", httpFailure(response.status, text));
    }
    let parsed: unknown;
    try {
      parsed = JSON.parse(text);
    } catch {
      throw new ProviderError("BadResponse", "模型接口没有返回 JSON。");
    }
    return parseChatCompletion(parsed, input.model);
  }
}

/**
 * `https://host/v1` and `https://host/v1/` both become `…/v1/chat/completions`.
 * A path such as `/api/ai` stays same-origin for the local dev proxy.
 */
export function completionsUrl(baseUrl: string): string {
  const trimmed = baseUrl.trim();
  if (trimmed === "") {
    throw new ProviderError("BadUrl", "模型接口地址是空的。");
  }
  if (trimmed.startsWith("/")) {
    const path = trimmed.endsWith("/") ? trimmed.slice(0, -1) : trimmed;
    return `${path}/chat/completions`;
  }
  const withSlash = trimmed.endsWith("/") ? trimmed : `${trimmed}/`;
  try {
    return new URL("chat/completions", withSlash).href;
  } catch {
    throw new ProviderError("BadUrl", "模型接口地址不是合法 URL。");
  }
}

/**
 * Reads an OpenAI chat-completion body.
 * Tool arguments may be a JSON string or an object; both become a string
 * the driver parses before calling the kernel.
 */
export function parseChatCompletion(body: unknown, fallbackModel: string): ModelCompletion {
  if (!isRecord(body)) {
    throw new ProviderError("BadResponse", "模型响应不是 JSON 对象。");
  }
  const model = typeof body.model === "string" && body.model !== "" ? body.model : fallbackModel;
  const choices = body.choices;
  if (!Array.isArray(choices) || choices.length === 0) {
    throw new ProviderError("BadResponse", "模型响应缺少 choices。");
  }
  const first = choices[0];
  if (!isRecord(first) || !isRecord(first.message)) {
    throw new ProviderError("BadResponse", "模型响应缺少 message。");
  }
  const message = first.message;
  return {
    model,
    text: readContent(message.content),
    toolCalls: readToolCalls(message.tool_calls),
  };
}

function readContent(content: unknown): string | null {
  if (content === undefined || content === null || content === "") {
    return null;
  }
  if (typeof content === "string") {
    return content;
  }
  throw new ProviderError("BadResponse", "模型 message.content 不是字符串。");
}

function readToolCalls(value: unknown): ModelToolCall[] {
  if (value === undefined || value === null) {
    return [];
  }
  if (!Array.isArray(value)) {
    throw new ProviderError("BadResponse", "模型 tool_calls 不是数组。");
  }
  return value.map((entry, index) => readToolCall(entry, index));
}

function readToolCall(entry: unknown, index: number): ModelToolCall {
  if (!isRecord(entry)) {
    throw new ProviderError("BadResponse", `tool_calls[${index}] 不是对象。`);
  }
  if (entry.type !== undefined && entry.type !== "function") {
    throw new ProviderError("BadResponse", `tool_calls[${index}] 不是 function 调用。`);
  }
  const fn = isRecord(entry.function) ? entry.function : null;
  const name = fn && typeof fn.name === "string" ? fn.name : "";
  if (name === "") {
    throw new ProviderError("BadResponse", `tool_calls[${index}] 缺少函数名。`);
  }
  const id = typeof entry.id === "string" && entry.id !== "" ? entry.id : `call-${index}`;
  return { id, name, arguments: argumentsText(fn?.arguments, index) };
}

function argumentsText(value: unknown, index: number): string {
  if (value === undefined || value === null || value === "") {
    return "{}";
  }
  if (typeof value === "string") {
    return value;
  }
  if (isRecord(value)) {
    return JSON.stringify(value);
  }
  throw new ProviderError("BadResponse", `tool_calls[${index}] 的 arguments 不是 JSON。`);
}

function httpFailure(status: number, raw: string): string {
  const detail = errorDetail(raw);
  return detail === "" ? `模型接口返回 ${status}。` : `模型接口返回 ${status}：${detail}`;
}

function errorDetail(raw: string): string {
  const trimmed = raw.trim();
  if (trimmed === "") {
    return "";
  }
  try {
    const parsed: unknown = JSON.parse(trimmed);
    if (isRecord(parsed) && isRecord(parsed.error) && typeof parsed.error.message === "string") {
      return clip(parsed.error.message);
    }
  } catch {
    // The body was not JSON. Clip the text below.
  }
  return clip(trimmed);
}

function clip(text: string): string {
  const oneLine = text.replace(/\s+/g, " ").trim();
  return oneLine.length > 300 ? `${oneLine.slice(0, 300)}…` : oneLine;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
