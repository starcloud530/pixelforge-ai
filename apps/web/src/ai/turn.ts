/**
 * One agent turn: a single chat completion, then its tool calls in order.
 *
 * Writes go through `callTool` on the same document the canvas uses. This
 * turn does not send tool results back to the model. A later multi-turn loop
 * is issue #14.
 */

import { READ_ONLY_TOOLS, TOOL_CATALOG, type ChatTool } from "./catalog";
import {
  TOOL_API_VERSION,
  parseToolResponse,
  type ToolCaller,
  type ToolResponse,
} from "../kernel/toolApi";

export interface ModelMessage {
  role: "system" | "user";
  content: string;
}

export interface ModelToolCall {
  id: string;
  name: string;
  /** JSON object text. The provider accepts an object and stringifies it. */
  arguments: string;
}

export interface ModelCompletion {
  model: string;
  text: string | null;
  toolCalls: ModelToolCall[];
}

/** Anything that can answer one completion. The HTTP provider implements this. */
export interface LanguageModel {
  complete(input: {
    model: string;
    messages: readonly ModelMessage[];
    tools: readonly ChatTool[];
  }): Promise<ModelCompletion>;
}

export interface DriverFault {
  code: "InvalidArguments";
  message: string;
}

export interface AgentStep {
  source: "model" | "driver";
  id: string;
  tool: string;
  params: Record<string, unknown> | null;
  response: ToolResponse | null;
  driverError?: DriverFault;
}

export interface AgentTurnResult {
  /** `partial` means a later tool call was not sent. */
  status: "completed" | "partial" | "refused" | "provider_error";
  model: string | null;
  assistantText: string | null;
  message: string | null;
  steps: AgentStep[];
  /** True when a successful tool changed pixels, palette, frames, delay, or history. */
  wrote: boolean;
}

export const AGENT_SYSTEM_PROMPT = [
  "You edit a PixelForge document only by calling the provided tools.",
  "The Rust kernel owns pixels, the palette, frames, delays, undo, project files, and export.",
  "Do not invent a pixel buffer, an image file, or a second canvas.",
  "The tools are Tool API version 1. Arguments are one JSON object of that tool's params.",
  "Integers are numbers, not strings. Unknown fields are rejected.",
  "Index 0 is transparent. It cannot be painted or recolored.",
  "The default indexPolicy is reject. clamp writes an out-of-range paint index as the last opaque color.",
  "remapPalette and applyPng report misses, merges, and pixelsChanged. There is no Lospec browser.",
  "The user message includes the current document from inspect.",
  "You may return several tool calls. They run in order on that same document.",
  "If one call fails, later calls in this reply are not run.",
  "Writes that succeeded stay on the kernel undo stack, so a person can undo them.",
  "If you do not call beginGesture, the shell groups this whole reply into one undo step.",
  "If you do call beginGesture, do not call it twice, and the shell will not wrap another gesture around yours.",
  "A gesture left open at the end of the reply is closed so the editor is not stuck.",
  "Read-only tools are inspect, scene, saveProject, exportPngSequence, exportSpriteSheet, previewFrame, diffFrames, and listLayers.",
  "Paint writes the active layer. Preview and export show the composite: index 0 shows the layer under it.",
  "This is one reply. Do not wait for tool results to come back.",
].join("\n");

const READ_ONLY = new Set<string>(READ_ONLY_TOOLS);
const GESTURE = new Set<string>(["beginGesture", "endGesture"]);

/**
 * Runs one user request against `kernel`.
 * A provider failure happens before any model tool call, so the document
 * stays as it was. A refused tool does not apply that call or anything after it.
 */
export async function runAgentTurn(input: {
  prompt: string;
  kernel: ToolCaller;
  model: LanguageModel;
  modelName: string;
  tools?: readonly ChatTool[];
}): Promise<AgentTurnResult> {
  const prompt = input.prompt.trim();
  if (prompt === "") {
    return emptyTurn("refused", "请输入要改的内容。");
  }

  const inspected = callKernel(input.kernel, "inspect", {}, "driver-inspect");
  if (!inspected.ok) {
    return emptyTurn("refused", inspected.error.message);
  }
  if (inspected.result.gestureOpen === true) {
    return emptyTurn("refused", "有一笔手势还没结束。松开画布后再运行这一轮。");
  }

  const tools = input.tools ?? TOOL_CATALOG;
  let completion: ModelCompletion;
  try {
    completion = await input.model.complete({
      model: input.modelName,
      messages: [
        { role: "system", content: AGENT_SYSTEM_PROMPT },
        { role: "user", content: userContent(prompt, inspected.result) },
      ],
      tools,
    });
  } catch (error) {
    const message = error instanceof Error && error.message ? error.message : "模型请求失败。";
    return {
      status: "provider_error",
      model: input.modelName,
      assistantText: null,
      message,
      steps: [],
      wrote: false,
    };
  }

  const steps: AgentStep[] = [];
  const modelManagesGesture = completion.toolCalls.some(
    (call) => call.name === "beginGesture" || call.name === "endGesture",
  );
  const shouldGroup =
    !modelManagesGesture && completion.toolCalls.some((call) => !READ_ONLY.has(call.name));
  const gestureMightStayOpen = shouldGroup || modelManagesGesture;

  try {
    if (shouldGroup) {
      const opened = callKernel(input.kernel, "beginGesture", {}, "driver-begin");
      steps.push(step("driver", "driver-begin", "beginGesture", {}, opened));
      if (!opened.ok) {
        return resultOf("partial", completion, steps, "无法开始这一轮的撤销分组。");
      }
    }

    for (const call of completion.toolCalls) {
      const parsed = parseArguments(call.arguments);
      if (!parsed.ok) {
        steps.push({
          source: "model",
          id: call.id,
          tool: call.name,
          params: null,
          response: null,
          driverError: { code: "InvalidArguments", message: parsed.message },
        });
        return resultOf("partial", completion, steps, parsed.message);
      }
      const response = callKernel(input.kernel, call.name, parsed.params, call.id);
      steps.push(step("model", call.id, call.name, parsed.params, response));
      if (!response.ok) {
        const message = `${call.name}：${response.error.code} ${response.error.message}`;
        return resultOf("partial", completion, steps, message);
      }
    }
    return resultOf("completed", completion, steps, null);
  } finally {
    if (gestureMightStayOpen) {
      closeOpenGesture(input.kernel, steps);
    }
  }
}

function userContent(prompt: string, inspect: Record<string, unknown>): string {
  return `Current document (Tool API inspect):\n${JSON.stringify(inspect)}\n\nRequest:\n${prompt}`;
}

function parseArguments(
  raw: string,
): { ok: true; params: Record<string, unknown> } | { ok: false; message: string } {
  const text = raw.trim() === "" ? "{}" : raw;
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    return { ok: false, message: "工具参数不是 JSON。" };
  }
  if (value === null) {
    return { ok: true, params: {} };
  }
  if (!isRecord(value)) {
    return { ok: false, message: "工具参数必须是 JSON 对象。" };
  }
  return { ok: true, params: value };
}

function callKernel(
  kernel: ToolCaller,
  tool: string,
  params: Record<string, unknown>,
  id: string,
): ToolResponse {
  const request: Record<string, unknown> = {
    apiVersion: TOOL_API_VERSION,
    id,
    tool,
    params,
  };
  return parseToolResponse(kernel.callTool(JSON.stringify(request)));
}

function closeOpenGesture(kernel: ToolCaller, steps: AgentStep[]): void {
  try {
    const inspected = callKernel(kernel, "inspect", {}, "driver-inspect-close");
    if (!inspected.ok || inspected.result.gestureOpen !== true) {
      return;
    }
  } catch {
    // inspect failed after a write. endGesture is a no-op when nothing is open,
    // and it seals a gesture that would otherwise stick the editor on OpenGesture.
  }
  try {
    const closed = callKernel(kernel, "endGesture", {}, "driver-end");
    steps.push(step("driver", "driver-end", "endGesture", {}, closed));
  } catch {
    // The turn result is still reported. A person can undo from the shell.
  }
}

function step(
  source: "model" | "driver",
  id: string,
  tool: string,
  params: Record<string, unknown>,
  response: ToolResponse,
): AgentStep {
  return { source, id, tool, params, response };
}

function resultOf(
  status: "completed" | "partial",
  completion: ModelCompletion,
  steps: AgentStep[],
  message: string | null,
): AgentTurnResult {
  return {
    status,
    model: completion.model,
    assistantText: completion.text,
    message,
    steps,
    wrote: steps.some(
      (entry) =>
        entry.response?.ok === true && !READ_ONLY.has(entry.tool) && !GESTURE.has(entry.tool),
    ),
  };
}

function emptyTurn(status: "refused", message: string): AgentTurnResult {
  return {
    status,
    model: null,
    assistantText: null,
    message,
    steps: [],
    wrote: false,
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
