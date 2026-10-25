/**
 * Typed caller for the kernel Tool API.
 *
 * The Rust document owns the tools. This helper only builds the JSON request
 * and reads the JSON response. Names match `TOOL_NAMES` in the kernel and
 * the catalog in `docs/tool-api.md`.
 */

/** Version the kernel writes on every response. An omitted request version means this. */
export const TOOL_API_VERSION = 1;

/** Stable tool names. Keep this list aligned with the kernel `TOOL_NAMES`. */
export const TOOL_NAMES = [
  "inspect",
  "scene",
  "paint",
  "fillRect",
  "erase",
  "eraseRect",
  "copyRect",
  "moveRect",
  "setSelection",
  "clearSelection",
  "replaceIndex",
  "setPaletteColor",
  "addPaletteColor",
  "importPalette",
  "setPaletteLocked",
  "setIndexPolicy",
  "remapPalette",
  "applyPng",
  "setActiveFrame",
  "addFrame",
  "deleteFrame",
  "duplicateFrame",
  "setFrameDelay",
  "addTag",
  "setTag",
  "removeTag",
  "listParts",
  "addPart",
  "setPart",
  "removePart",
  "setAnchor",
  "removeAnchor",
  "translatePart",
  "rotatePart",
  "flipPart",
  "setCurveKey",
  "removeCurveKey",
  "clearCurve",
  "listCurves",
  "sampleCurve",
  "samplePart",
  "addConstraint",
  "removeConstraint",
  "listConstraints",
  "solveConstraints",
  "bake",
  "beginGesture",
  "endGesture",
  "undo",
  "redo",
  "saveProject",
  "loadProject",
  "importAseprite",
  "exportPngSequence",
  "exportSpriteSheet",
  "previewFrame",
  "diffFrames",
  "flipHorizontal",
  "flipVertical",
  "rotate",
  "resizeCanvas",
  "listLayers",
  "addLayer",
  "deleteLayer",
  "renameLayer",
  "setActiveLayer",
  "reorderLayer",
  "flattenLayers",
] as const;

export type ToolName = (typeof TOOL_NAMES)[number];

export interface ToolErrorBody {
  code: string;
  message: string;
}

export interface ToolSuccess {
  ok: true;
  apiVersion: number;
  id?: string | number;
  result: Record<string, unknown>;
}

export interface ToolFailureResponse {
  ok: false;
  apiVersion: number;
  id?: string | number;
  error: ToolErrorBody;
}

export type ToolResponse = ToolSuccess | ToolFailureResponse;

/** Anything that can run one Tool API request. `PixelKernel` does. */
export interface ToolCaller {
  callTool(request: string): string;
}

/**
 * Sends one tool call and parses the JSON body.
 * Throws when the body is not a Tool API object. A refused tool returns
 * `{ ok: false }` and does not throw.
 */
export function callDocumentTool(
  kernel: ToolCaller,
  tool: ToolName,
  params: Record<string, unknown> = {},
  id?: string | number,
): ToolResponse {
  const request: Record<string, unknown> = {
    apiVersion: TOOL_API_VERSION,
    tool,
    params,
  };
  if (id !== undefined) {
    request.id = id;
  }
  return parseToolResponse(kernel.callTool(JSON.stringify(request)));
}

/** Reads a response produced by the kernel. */
export function parseToolResponse(raw: string): ToolResponse {
  const value: unknown = JSON.parse(raw);
  if (!isRecord(value) || typeof value.ok !== "boolean" || typeof value.apiVersion !== "number") {
    throw new Error("tool response is not a Tool API object");
  }
  const id = readId(value.id);
  if (value.ok) {
    if (!isRecord(value.result)) {
      throw new Error("tool success is missing result");
    }
    return id === undefined
      ? { ok: true, apiVersion: value.apiVersion, result: value.result }
      : { ok: true, apiVersion: value.apiVersion, id, result: value.result };
  }
  if (
    !isRecord(value.error) ||
    typeof value.error.code !== "string" ||
    typeof value.error.message !== "string"
  ) {
    throw new Error("tool failure is missing error");
  }
  const error = { code: value.error.code, message: value.error.message };
  return id === undefined
    ? { ok: false, apiVersion: value.apiVersion, error }
    : { ok: false, apiVersion: value.apiVersion, id, error };
}

function readId(id: unknown): string | number | undefined {
  if (id === undefined) {
    return undefined;
  }
  if (typeof id === "string" || typeof id === "number") {
    return id;
  }
  throw new Error("tool response id is not a string or number");
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
