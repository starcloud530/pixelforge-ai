/**
 * OpenAI-compatible tool definitions for one chat completion.
 *
 * Names and fields follow `docs/tool-api.md` and `TOOL_NAMES`. The model
 * sends each tool's `params` object. The driver adds the Tool API envelope.
 */

import { TOOL_NAMES, type ToolName } from "../kernel/toolApi";

export interface ChatTool {
  type: "function";
  function: {
    name: ToolName;
    description: string;
    parameters: {
      type: "object";
      properties: Record<string, SchemaProperty>;
      required: string[];
      additionalProperties: false;
    };
  };
}

interface SchemaProperty {
  type: "integer" | "boolean" | "string";
  description: string;
  minimum?: number;
  maximum?: number;
  enum?: readonly string[];
}

const BY_NAME = {
  inspect: tool(
    "inspect",
    "Read the document: size, palette, lock, frames, delays, active-frame indices, undo flags. Changes nothing.",
  ),
  paint: tool(
    "paint",
    "Write a palette index into one cell. Index 0 is transparent and cannot be painted. A locked palette still allows paint.",
    {
      x: int("Column, 0-based.", 0, 65535),
      y: int("Row, 0-based.", 0, 65535),
      colorIndex: int("Opaque palette index that already exists.", 1, 255),
    },
    ["x", "y", "colorIndex"],
  ),
  fillRect: tool(
    "fillRect",
    "Fill an axis-aligned rectangle on the active frame with one existing palette index. One undo step. A locked palette still allows this. The rectangle must lie entirely inside the canvas. Index 0 is refused.",
    {
      x: int("Left column, 0-based.", 0, 65535),
      y: int("Top row, 0-based.", 0, 65535),
      width: int("Width in cells, at least 1.", 1, 65535),
      height: int("Height in cells, at least 1.", 1, 65535),
      colorIndex: int("Opaque palette index that already exists.", 1, 255),
    },
    ["x", "y", "width", "height", "colorIndex"],
  ),
  erase: tool(
    "erase",
    "Set one cell back to transparent index 0.",
    {
      x: int("Column, 0-based.", 0, 65535),
      y: int("Row, 0-based.", 0, 65535),
    },
    ["x", "y"],
  ),
  setPaletteColor: tool(
    "setPaletteColor",
    "Replace the RGB of an existing opaque palette entry. Every frame that uses the index updates. Refused while the palette is locked. Index 0 stays transparent.",
    {
      index: int("Palette index to recolor. Not 0.", 1, 255),
      r: int("Red, 0–255.", 0, 255),
      g: int("Green, 0–255.", 0, 255),
      b: int("Blue, 0–255.", 0, 255),
    },
    ["index", "r", "g", "b"],
  ),
  addPaletteColor: tool(
    "addPaletteColor",
    "Append one opaque color. Returns the new index. Refused while the palette is locked or when the palette would pass 256 entries.",
    {
      r: int("Red, 0–255.", 0, 255),
      g: int("Green, 0–255.", 0, 255),
      b: int("Blue, 0–255.", 0, 255),
    },
    ["r", "g", "b"],
  ),
  importPalette: tool(
    "importPalette",
    "Import a GIMP .gpl palette or a hex list (RRGGBB or #RRGGBB per line). replace swaps paint colors and clears dropped indices. merge appends colors that are not already present. One undo step. Refused while locked.",
    {
      text: { type: "string", description: "Raw palette file text." },
      mode: {
        type: "string",
        description: "replace or merge.",
        enum: ["replace", "merge"],
      },
    },
    ["text", "mode"],
  ),
  setPaletteLocked: tool(
    "setPaletteLocked",
    "Lock or unlock palette edits. Paint still uses existing indices. The flag is stored in the project file.",
    {
      locked: { type: "boolean", description: "True locks recolor, append, and import." },
    },
    ["locked"],
  ),
  setActiveFrame: tool(
    "setActiveFrame",
    "Select the frame the next paint and inspect pixels refer to.",
    {
      index: int("Frame index, 0-based.", 0, 4294967295),
    },
    ["index"],
  ),
  addFrame: tool(
    "addFrame",
    "Append a blank frame with a 100ms delay. The active frame stays put. Returns the new index.",
  ),
  deleteFrame: tool(
    "deleteFrame",
    "Delete a frame. At least one frame must remain.",
    {
      index: int("Frame index, 0-based.", 0, 4294967295),
    },
    ["index"],
  ),
  duplicateFrame: tool(
    "duplicateFrame",
    "Insert a copy of a frame (pixels and delay) immediately after it. Returns the new index.",
    {
      index: int("Source frame index, 0-based.", 0, 4294967295),
    },
    ["index"],
  ),
  setFrameDelay: tool(
    "setFrameDelay",
    "Set one frame's duration in milliseconds.",
    {
      index: int("Frame index, 0-based.", 0, 4294967295),
      delayMs: int("Duration in milliseconds.", 1, 65535),
    },
    ["index", "delayMs"],
  ),
  beginGesture: tool(
    "beginGesture",
    "Start grouping later successful edits into one undo step. Fails if a gesture is already open.",
  ),
  endGesture: tool(
    "endGesture",
    "Seal the open gesture. Does nothing when none is open. An empty gesture is not recorded.",
  ),
  undo: tool(
    "undo",
    "Undo the latest kernel edit. An open gesture that already changed the document is sealed and undone as one step.",
  ),
  redo: tool(
    "redo",
    "Redo the latest undone kernel edit. Fails when an open gesture already has changes.",
  ),
  saveProject: tool(
    "saveProject",
    "Return the pixelforge-project version 1 JSON text. Does not change the document or the undo stack.",
  ),
  loadProject: tool(
    "loadProject",
    "Replace the document with a pixelforge-project version 1 file. The text must be the file, not an object. On success the undo stack is empty. A parse error leaves the current document in place.",
    {
      json: { type: "string", description: "pixelforge-project version 1 text." },
    },
    ["json"],
  ),
  exportPngSequence: tool(
    "exportPngSequence",
    "Return each frame as a standard base64 RGBA PNG. Does not change the document or the undo stack.",
  ),
  exportSpriteSheet: tool(
    "exportSpriteSheet",
    "Return one grid sprite sheet as a base64 PNG, plus columns, rows, and sheet size. Does not change the document or the undo stack.",
  ),
} satisfies Record<ToolName, ChatTool>;

/** Catalog in `TOOL_NAMES` order. One entry per kernel tool. */
export const TOOL_CATALOG: readonly ChatTool[] = TOOL_NAMES.map((name) => BY_NAME[name]);

/** Tools that never push an undo entry. */
export const READ_ONLY_TOOLS: readonly ToolName[] = [
  "inspect",
  "saveProject",
  "exportPngSequence",
  "exportSpriteSheet",
];

function tool(
  name: ToolName,
  description: string,
  properties: Record<string, SchemaProperty> = {},
  required: string[] = [],
): ChatTool {
  return {
    type: "function",
    function: {
      name,
      description,
      parameters: {
        type: "object",
        properties,
        required,
        additionalProperties: false,
      },
    },
  };
}

function int(description: string, minimum: number, maximum: number): SchemaProperty {
  return { type: "integer", description, minimum, maximum };
}
