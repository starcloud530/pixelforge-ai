/**
 * One canned model reply, mocked HTTP, and the real wasm Document.
 * Nothing in this file opens a socket.
 */

import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { TOOL_CATALOG } from "./catalog.js";
import { DEFAULT_AI_BASE_URL, DEFAULT_AI_MODEL, readAiEndpoint } from "./config.js";
import { OpenAiCompatibleProvider, ProviderError, completionsUrl } from "./provider.js";
import { AGENT_SYSTEM_PROMPT, runAgentTurn, type AgentTurnResult } from "./turn.js";
import {
  TOOL_NAMES,
  callDocumentTool,
  type ToolCaller,
  type ToolResponse,
} from "../kernel/toolApi.js";

const require = createRequire(import.meta.url);
const wasmPath = join(
  dirname(fileURLToPath(import.meta.url)),
  "../../../../crates/pixelforge-kernel-wasm/pkg-node/pixelforge_kernel_wasm.js",
);

interface KernelDocument {
  callTool(request: string): string;
  pixels(): Uint8Array;
  free(): void;
  beginGesture(): void;
}

const { PixelDocument } = require(wasmPath) as {
  PixelDocument: new (width: number, height: number) => KernelDocument;
};

const API_KEY = "test-key-not-real";
const BASE_URL = "https://models.example.test/v1";

interface SeenRequest {
  url: string;
  init: RequestInit | undefined;
}

function openDocument(width = 4, height = 4): {
  doc: KernelDocument;
  kernel: ToolCaller;
  calls: string[];
} {
  const doc = new PixelDocument(width, height);
  const calls: string[] = [];
  const kernel: ToolCaller = {
    callTool(request: string): string {
      calls.push(request);
      return doc.callTool(request);
    },
  };
  return { doc, kernel, calls };
}

function toolNames(calls: readonly string[]): string[] {
  return calls.map((raw) => {
    const body = JSON.parse(raw) as { tool?: string };
    assert.equal(typeof body.tool, "string");
    return body.tool as string;
  });
}

function okResult(response: ToolResponse): Record<string, unknown> {
  if (!response.ok) {
    throw new Error(response.error.message);
  }
  return response.result;
}

interface CannedCall {
  id: string;
  name: string;
  arguments: string | Record<string, unknown>;
}

function completionBody(calls: readonly CannedCall[], text: string | null = null): unknown {
  return {
    id: "chatcmpl-test",
    object: "chat.completion",
    model: "gpt-6-astra",
    choices: [
      {
        index: 0,
        message: {
          role: "assistant",
          content: text,
          tool_calls: calls.map((call) => ({
            id: call.id,
            type: "function",
            function: { name: call.name, arguments: call.arguments },
          })),
        },
        finish_reason: calls.length > 0 ? "tool_calls" : "stop",
      },
    ],
  };
}

function mockProvider(payload: unknown, status = 200): {
  seen: SeenRequest[];
  model: OpenAiCompatibleProvider;
} {
  const seen: SeenRequest[] = [];
  const fetchImpl: typeof fetch = (input, init) => {
    const url =
      typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
    seen.push({ url, init });
    const body = typeof payload === "string" ? payload : JSON.stringify(payload);
    return Promise.resolve(
      new Response(body, {
        status,
        headers: { "Content-Type": "application/json" },
      }),
    );
  };
  return {
    seen,
    model: new OpenAiCompatibleProvider({
      apiKey: API_KEY,
      baseUrl: BASE_URL,
      fetchImpl,
    }),
  };
}

function requestJson(seen: readonly SeenRequest[]): {
  model: string;
  messages: { role: string; content: string }[];
  tools: { type: string; function: { name: string } }[];
  tool_choice: string;
  parallel_tool_calls: boolean;
} {
  assert.equal(seen.length, 1);
  const first = seen[0];
  assert.ok(first);
  assert.equal(first.url, `${BASE_URL}/chat/completions`);
  const headers = new Headers(first.init?.headers);
  assert.equal(headers.get("authorization"), `Bearer ${API_KEY}`);
  const raw = first.init?.body;
  assert.equal(typeof raw, "string");
  assert.equal(String(raw).includes(API_KEY), false);
  return JSON.parse(String(raw)) as ReturnType<typeof requestJson>;
}

test("catalog lists every kernel tool once, in catalog order", () => {
  assert.deepEqual(
    TOOL_CATALOG.map((entry) => entry.function.name),
    [...TOOL_NAMES],
  );
  for (const entry of TOOL_CATALOG) {
    assert.equal(entry.type, "function");
    assert.equal(entry.function.parameters.additionalProperties, false);
    assert.ok(Array.isArray(entry.function.parameters.required));
  }
  const paint = TOOL_CATALOG.find((entry) => entry.function.name === "paint");
  assert.ok(paint);
  assert.deepEqual(paint.function.parameters.required, ["x", "y", "colorIndex"]);
  assert.match(AGENT_SYSTEM_PROMPT, /Tool API/);
  assert.match(AGENT_SYSTEM_PROMPT, /Do not invent a pixel buffer/);
});

test("endpoint config prefers PIXELFORGE_* and never invents a key", () => {
  assert.equal(readAiEndpoint({}), null);
  assert.equal(readAiEndpoint({ PIXELFORGE_AI_API_KEY: "  ", OPENAI_API_KEY: "" }), null);
  const fallback = readAiEndpoint({ OPENAI_API_KEY: " sk " });
  assert.deepEqual(fallback, {
    apiKey: "sk",
    baseUrl: DEFAULT_AI_BASE_URL,
    model: DEFAULT_AI_MODEL,
  });
  const preferred = readAiEndpoint({
    PIXELFORGE_AI_API_KEY: "pixel",
    OPENAI_API_KEY: "other",
    PIXELFORGE_AI_BASE_URL: "https://astra.example/v1/",
    OPENAI_BASE_URL: "https://openai.example/v1",
    PIXELFORGE_AI_MODEL: "gpt-6-astra",
    OPENAI_MODEL: "gpt-4.1",
  });
  assert.deepEqual(preferred, {
    apiKey: "pixel",
    baseUrl: "https://astra.example/v1",
    model: "gpt-6-astra",
  });
  assert.equal(completionsUrl("https://astra.example/v1"), "https://astra.example/v1/chat/completions");
  assert.equal(completionsUrl("https://astra.example/v1/"), "https://astra.example/v1/chat/completions");
  assert.equal(completionsUrl("/api/ai"), "/api/ai/chat/completions");
  assert.equal(completionsUrl("/api/ai/"), "/api/ai/chat/completions");
  assert.throws(() => completionsUrl("not a url"), (error: unknown) => {
    return error instanceof ProviderError && error.code === "BadUrl";
  });
});

test("a canned tool_call sequence mutates the document and undoes as one step", async () => {
  const { doc, kernel, calls } = openDocument();
  try {
    const { seen, model } = mockProvider(
      completionBody(
        [
          { id: "call-paint-a", name: "paint", arguments: { x: 0, y: 0, colorIndex: 1 } },
          { id: "call-paint-b", name: "paint", arguments: '{"x":1,"y":0,"colorIndex":2}' },
          { id: "call-frame", name: "addFrame", arguments: "{}" },
          { id: "call-delay", name: "setFrameDelay", arguments: '{"index":1,"delayMs":80}' },
        ],
        "画了两格",
      ),
    );
    const turn = await runAgentTurn({
      prompt: "画两格并加一帧",
      kernel,
      model,
      modelName: "gpt-6-astra",
    });
    const during = calls.slice();

    assert.equal(turn.status, "completed");
    assert.equal(turn.wrote, true);
    assert.equal(turn.assistantText, "画了两格");
    assert.equal(turn.model, "gpt-6-astra");
    const body = requestJson(seen);
    assert.equal(body.model, "gpt-6-astra");
    assert.equal(body.tool_choice, "auto");
    assert.equal(body.parallel_tool_calls, false);
    assert.deepEqual(
      body.tools.map((entry) => entry.function.name),
      [...TOOL_NAMES],
    );
    assert.equal(body.messages[0]?.role, "system");
    assert.match(body.messages[1]?.content ?? "", /画两格并加一帧/);
    assert.match(body.messages[1]?.content ?? "", /"width":4/);
    assert.deepEqual(toolNames(during), [
      "inspect",
      "beginGesture",
      "paint",
      "paint",
      "addFrame",
      "setFrameDelay",
      "inspect",
      "endGesture",
    ]);
    const firstPaint = JSON.parse(during[2] ?? "{}") as { id?: string; params?: unknown };
    assert.equal(firstPaint.id, "call-paint-a");
    assert.deepEqual(firstPaint.params, { x: 0, y: 0, colorIndex: 1 });
    const paintStep = turn.steps.find((entry) => entry.id === "call-paint-a");
    assert.equal(paintStep?.response?.ok, true);
    assert.equal(paintStep?.response?.id, "call-paint-a");

    const pixels = doc.pixels();
    assert.equal(pixels.length, 16);
    assert.equal(pixels[0], 1);
    assert.equal(pixels[1], 2);
    const after = okResult(callDocumentTool(kernel, "inspect"));
    assert.equal(after.frameCount, 2);
    assert.deepEqual(after.frameDelays, [100, 80]);
    assert.equal(after.gestureOpen, false);
    assert.equal(after.canUndo, true);

    assert.equal(callDocumentTool(kernel, "undo").ok, true);
    assert.equal(doc.pixels()[0], 0);
    assert.equal(doc.pixels()[1], 0);
    const restored = okResult(callDocumentTool(kernel, "inspect"));
    assert.equal(restored.frameCount, 1);
    assert.deepEqual(restored.frameDelays, [100]);
    assert.equal(restored.canUndo, false);
  } finally {
    doc.free();
  }
});

test("a refused tool stops the sequence and one undo restores the document", async () => {
  const { doc, kernel, calls } = openDocument();
  try {
    const { seen, model } = mockProvider(
      completionBody([
        { id: "call-ok", name: "paint", arguments: '{"x":0,"y":0,"colorIndex":1}' },
        { id: "call-oob", name: "paint", arguments: '{"x":9,"y":0,"colorIndex":1}' },
        { id: "call-frame", name: "addFrame", arguments: "{}" },
      ]),
    );
    const turn = await runAgentTurn({
      prompt: "涂一格，再涂到画布外，然后加帧",
      kernel,
      model,
      modelName: "gpt-6-astra",
    });
    assert.equal(seen.length, 1);
    assert.equal(turn.status, "partial");
    assert.equal(turn.wrote, true);
    assert.match(turn.message ?? "", /OutOfBounds/);
    assert.deepEqual(toolNames(calls), [
      "inspect",
      "beginGesture",
      "paint",
      "paint",
      "inspect",
      "endGesture",
    ]);
    assert.equal(turn.steps.some((entry) => entry.id === "call-frame"), false);
    assert.equal(doc.pixels()[0], 1);
    assert.equal(okResult(callDocumentTool(kernel, "inspect")).frameCount, 1);

    assert.equal(callDocumentTool(kernel, "undo").ok, true);
    assert.equal(doc.pixels()[0], 0);
    assert.equal(okResult(callDocumentTool(kernel, "inspect")).canUndo, false);
  } finally {
    doc.free();
  }
});

test("invalid arguments never reach the kernel and do not apply later tools", async () => {
  const { doc, kernel, calls } = openDocument();
  try {
    const { model } = mockProvider(
      completionBody([
        { id: "call-ok", name: "paint", arguments: '{"x":2,"y":2,"colorIndex":3}' },
        { id: "call-bad", name: "addFrame", arguments: "{" },
        { id: "call-later", name: "erase", arguments: '{"x":2,"y":2}' },
      ]),
    );
    const turn = await runAgentTurn({
      prompt: "先画再给一段坏参数",
      kernel,
      model,
      modelName: "gpt-6-astra",
    });
    assert.equal(turn.status, "partial");
    assert.equal(turn.steps.find((entry) => entry.id === "call-bad")?.driverError?.code, "InvalidArguments");
    assert.equal(turn.steps.some((entry) => entry.id === "call-later"), false);
    assert.deepEqual(toolNames(calls), ["inspect", "beginGesture", "paint", "inspect", "endGesture"]);
    assert.equal(doc.pixels()[2 * 4 + 2], 3);
    assert.equal(callDocumentTool(kernel, "undo").ok, true);
    assert.equal(doc.pixels()[2 * 4 + 2], 0);
  } finally {
    doc.free();
  }
});

test("a model-managed gesture is not wrapped again", async () => {
  const { doc, kernel, calls } = openDocument();
  try {
    const { model } = mockProvider(
      completionBody([
        { id: "call-begin", name: "beginGesture", arguments: "{}" },
        { id: "call-a", name: "paint", arguments: '{"x":0,"y":1,"colorIndex":1}' },
        { id: "call-b", name: "paint", arguments: '{"x":1,"y":1,"colorIndex":1}' },
        { id: "call-end", name: "endGesture", arguments: "{}" },
      ]),
    );
    const turn = await runAgentTurn({
      prompt: "用一笔手势画两个格子",
      kernel,
      model,
      modelName: "gpt-6-astra",
    });
    assert.equal(turn.status, "completed");
    assert.equal(turn.steps.some((entry) => entry.id === "driver-begin"), false);
    assert.equal(turn.steps.some((entry) => entry.id === "driver-end"), false);
    assert.deepEqual(toolNames(calls), [
      "inspect",
      "beginGesture",
      "paint",
      "paint",
      "endGesture",
      "inspect",
    ]);
    assert.equal(doc.pixels()[4], 1);
    assert.equal(doc.pixels()[5], 1);
    assert.equal(callDocumentTool(kernel, "undo").ok, true);
    assert.equal(doc.pixels()[4], 0);
    assert.equal(doc.pixels()[5], 0);
    assert.equal(okResult(callDocumentTool(kernel, "inspect")).canUndo, false);
  } finally {
    doc.free();
  }
});

test("an HTTP error and a bad body leave the document untouched", async () => {
  const { doc, kernel, calls } = openDocument();
  try {
    const http = mockProvider({ error: { message: "nope" } }, 503);
    const failed = await runAgentTurn({
      prompt: "请画一笔",
      kernel,
      model: http.model,
      modelName: "gpt-6-astra",
    });
    assert.equal(failed.status, "provider_error");
    assert.match(failed.message ?? "", /503/);
    assert.match(failed.message ?? "", /nope/);
    assert.equal(failed.wrote, false);
    assert.deepEqual(toolNames(calls), ["inspect"]);
    assert.equal(http.seen.length, 1);

    const before = calls.length;
    const bad = mockProvider({ nope: true });
    const broken = await runAgentTurn({
      prompt: "再试一次",
      kernel,
      model: bad.model,
      modelName: "gpt-6-astra",
    });
    assert.equal(broken.status, "provider_error");
    assert.deepEqual(toolNames(calls.slice(before)), ["inspect"]);
    assert.ok(doc.pixels().every((index) => index === 0));
    assert.equal(okResult(callDocumentTool(kernel, "inspect")).canUndo, false);
  } finally {
    doc.free();
  }
});

test("an empty prompt or an open gesture does not call the model", async () => {
  const { doc, kernel, calls } = openDocument();
  try {
    const fetchImpl: typeof fetch = () => {
      throw new Error("fetch should not run");
    };
    const model = new OpenAiCompatibleProvider({
      apiKey: API_KEY,
      baseUrl: BASE_URL,
      fetchImpl,
    });
    const empty = await runAgentTurn({
      prompt: "   ",
      kernel,
      model,
      modelName: "gpt-6-astra",
    });
    assert.equal(empty.status, "refused");
    assert.deepEqual(calls, []);

    assert.equal(callDocumentTool(kernel, "beginGesture").ok, true);
    const blocked = await runAgentTurn({
      prompt: "现在画",
      kernel,
      model,
      modelName: "gpt-6-astra",
    });
    assert.equal(blocked.status, "refused");
    assert.match(blocked.message ?? "", /手势/);
    assert.deepEqual(toolNames(calls), ["beginGesture", "inspect"]);
    assert.equal(okResult(callDocumentTool(kernel, "inspect")).gestureOpen, true);
    assert.equal(callDocumentTool(kernel, "endGesture").ok, true);
  } finally {
    doc.free();
  }
});

test("a missing API key fails before any network call", async () => {
  const { doc, kernel, calls } = openDocument();
  try {
    const model = new OpenAiCompatibleProvider({
      apiKey: " ",
      baseUrl: BASE_URL,
      fetchImpl: () => {
        throw new Error("fetch should not run");
      },
    });
    const turn: AgentTurnResult = await runAgentTurn({
      prompt: "画",
      kernel,
      model,
      modelName: "gpt-6-astra",
    });
    assert.equal(turn.status, "provider_error");
    assert.match(turn.message ?? "", /密钥/);
    assert.deepEqual(toolNames(calls), ["inspect"]);
    assert.ok(doc.pixels().every((index) => index === 0));
  } finally {
    doc.free();
  }
});
