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

const REPORT_DIRTY: SchemaProperty = {
  type: "boolean",
  description:
    "When true, add changed, dirty bbox, and regions. Omit it and those fields stay absent.",
};

const BY_NAME = {
  inspect: tool(
    "inspect",
    "Read the document: size, palette, lock, frames, delays, active-frame indices, undo flags. Changes nothing. Includes paletteHash.",
  ),
  scene: tool(
    "scene",
    "Read a compact summary: canvas size, active frame and layer, layer names, palette hash, and frame delays. Does not return pixels. visible is always true and opacity is null. Changes nothing.",
  ),
  paint: tool(
    "paint",
    "Write a palette index into one cell. Index 0 is transparent and cannot be painted. A locked palette still allows paint. A cell outside the stored selection is left unchanged.",
    {
      x: int("Column, 0-based.", 0, 65535),
      y: int("Row, 0-based.", 0, 65535),
      colorIndex: int("Opaque palette index that already exists.", 1, 255),
      layer: int("Layer to edit. Omit for the active layer. Does not select it.", 0, 4294967295),
      reportDirty: REPORT_DIRTY,
    },
    ["x", "y", "colorIndex"],
  ),
  fillRect: tool(
    "fillRect",
    "Fill an axis-aligned rectangle on the active frame with one existing palette index. One undo step. A locked palette still allows this. The rectangle must lie entirely inside the canvas. Index 0 is refused. Cells outside the stored selection are skipped.",
    {
      x: int("Left column, 0-based.", 0, 65535),
      y: int("Top row, 0-based.", 0, 65535),
      width: int("Width in cells, at least 1.", 1, 65535),
      height: int("Height in cells, at least 1.", 1, 65535),
      colorIndex: int("Opaque palette index that already exists.", 1, 255),
      layer: int("Layer to edit. Omit for the active layer. Does not select it.", 0, 4294967295),
      reportDirty: REPORT_DIRTY,
    },
    ["x", "y", "width", "height", "colorIndex"],
  ),
  erase: tool(
    "erase",
    "Set one cell back to transparent index 0. A cell outside the stored selection is left unchanged.",
    {
      x: int("Column, 0-based.", 0, 65535),
      y: int("Row, 0-based.", 0, 65535),
      layer: int("Layer to edit. Omit for the active layer. Does not select it.", 0, 4294967295),
      reportDirty: REPORT_DIRTY,
    },
    ["x", "y"],
  ),
  eraseRect: tool(
    "eraseRect",
    "Clear an axis-aligned rectangle on the active frame to transparent index 0. One undo step. The rectangle must lie entirely inside the canvas. A locked palette still allows this. Prefer this over erasing cell by cell. Cells outside the stored selection are skipped.",
    {
      x: int("Left column, 0-based.", 0, 65535),
      y: int("Top row, 0-based.", 0, 65535),
      width: int("Width in cells, at least 1.", 1, 65535),
      height: int("Height in cells, at least 1.", 1, 65535),
      layer: int("Layer to edit. Omit for the active layer. Does not select it.", 0, 4294967295),
      reportDirty: REPORT_DIRTY,
    },
    ["x", "y", "width", "height"],
  ),
  copyRect: tool(
    "copyRect",
    "Copy palette indices, including 0, from one rectangle to another. One undo step. Source and destination must both lie inside the canvas. The source stays. Optional fromFrame and toFrame copy onto another frame and do not change the active frame. Omit both to stay on the active frame. Destination cells outside the stored selection are left unchanged.",
    {
      srcX: int("Source left column, 0-based.", 0, 65535),
      srcY: int("Source top row, 0-based.", 0, 65535),
      width: int("Width in cells, at least 1.", 1, 65535),
      height: int("Height in cells, at least 1.", 1, 65535),
      dstX: int("Destination left column, 0-based.", 0, 65535),
      dstY: int("Destination top row, 0-based.", 0, 65535),
      fromFrame: int("Source frame. Omit for the active frame.", 0, 4294967295),
      toFrame: int("Destination frame. Omit for the active frame.", 0, 4294967295),
      layer: int("Layer to read and write. Omit for the active layer. Does not select it.", 0, 4294967295),
      reportDirty: REPORT_DIRTY,
    },
    ["srcX", "srcY", "width", "height", "dstX", "dstY"],
  ),
  moveRect: tool(
    "moveRect",
    "Move palette indices: destination receives the source, including 0, and source cells outside the destination become index 0. One undo step. Optional fromFrame and toFrame move onto another frame and clear the source there. Omit both to stay on the active frame. When a selection is set, both rectangles must lie inside it or the call returns SelectionProtected and writes nothing.",
    {
      srcX: int("Source left column, 0-based.", 0, 65535),
      srcY: int("Source top row, 0-based.", 0, 65535),
      width: int("Width in cells, at least 1.", 1, 65535),
      height: int("Height in cells, at least 1.", 1, 65535),
      dstX: int("Destination left column, 0-based.", 0, 65535),
      dstY: int("Destination top row, 0-based.", 0, 65535),
      fromFrame: int("Source frame. Omit for the active frame.", 0, 4294967295),
      toFrame: int("Destination frame. Omit for the active frame.", 0, 4294967295),
      layer: int("Layer to read and write. Omit for the active layer. Does not select it.", 0, 4294967295),
      reportDirty: REPORT_DIRTY,
    },
    ["srcX", "srcY", "width", "height", "dstX", "dstY"],
  ),
  setSelection: tool(
    "setSelection",
    "Store one rectangle in the project file. Later paint, erase, fillRect, eraseRect, copyRect, flipHorizontal, and replaceIndex skip cells outside it. moveRect requires both rectangles inside it. Does not change pixels or the active layer. There is no lasso.",
    {
      x: int("Left column, 0-based.", 0, 65535),
      y: int("Top row, 0-based.", 0, 65535),
      width: int("Width in cells, at least 1.", 1, 65535),
      height: int("Height in cells, at least 1.", 1, 65535),
    },
    ["x", "y", "width", "height"],
  ),
  clearSelection: tool(
    "clearSelection",
    "Remove the rectangle so the whole canvas is writable. The project file omits selection. Does not change pixels. Already clear is not recorded.",
  ),
  replaceIndex: tool(
    "replaceIndex",
    "Replace one stored palette index on the active frame. A selection limits the replace to that rectangle. toIndex 0 clears those cells. Optional layer does not select it. Does not change other frames.",
    {
      fromIndex: int("Stored index to find, including 0.", 0, 255),
      toIndex: int("Index to write. 0 clears. Any other index must exist unless clamp is set.", 0, 255),
      layer: int("Layer to edit. Omit for the active layer. Does not select it.", 0, 4294967295),
    },
    ["fromIndex", "toIndex"],
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
    "Lock or unlock palette edits. Paint still uses existing indices. The flag is stored in the project file. Lock also refuses remapPalette. It does not refuse applyPng or setIndexPolicy.",
    {
      locked: { type: "boolean", description: "True locks recolor, append, import, and remap." },
    },
    ["locked"],
  ),
  setIndexPolicy: tool(
    "setIndexPolicy",
    "Choose how paint and fillRect treat an index past the palette. reject (default) returns PaletteIndex. clamp writes the last opaque index. Index 0 stays refused. Only clamp is stored in the project file.",
    {
      mode: {
        type: "string",
        description: "reject or clamp.",
        enum: ["reject", "clamp"],
      },
    },
    ["mode"],
  ),
  remapPalette: tool(
    "remapPalette",
    "Quantize or retarget the palette. Pass maxColors (1–255 opaque colors) or text (a .gpl or hex list), not both. Nearest squared-RGB match, lower index wins a tie, survivor RGB is not averaged. Result lists misses, merges, dropped, pixelsChanged, and paletteLength. One undo step. Refused while locked. No Lospec browser.",
    {
      text: { type: "string", description: "Target .gpl or hex list. Omit when passing maxColors." },
      maxColors: int("Maximum opaque colors to keep. Omit when passing text.", 1, 255),
    },
  ),
  applyPng: tool(
    "applyPng",
    "Crush one base64 PNG onto the current palette of one frame and layer. The PNG must match the canvas; there is no scale and no sheet slice. Alpha below 128 becomes index 0. Does not add colors. A locked palette still allows this. misses and merges list colors that were not an exact RGB hit. No Lospec browser.",
    {
      png: { type: "string", description: "Standard base64 PNG. Whitespace is ignored." },
      frame: int("Frame to write. Omit for the active frame. Does not select it.", 0, 4294967295),
      layer: int("Layer to write. Omit for the active layer. Does not select it.", 0, 4294967295),
    },
    ["png"],
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
    "Insert a copy of a frame (pixels and delay) immediately after it. Returns the new index. Does not select that frame. Call setActiveFrame with the returned index before painting the copy. The web duplicate button selects it; this tool does not.",
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
    "Return each frame as a standard base64 RGBA PNG. Optional scale (integer >= 1, default 1) nearest-neighbor repeats each palette index before resolving color. Does not change the document or the undo stack.",
    {
      scale: int("Nearest-neighbor scale, integer >= 1. Default 1.", 1, 4294967295),
    },
  ),
  exportSpriteSheet: tool(
    "exportSpriteSheet",
    "Return one grid sprite sheet as a base64 PNG, plus columns, rows, and sheet size. Optional scale (integer >= 1, default 1) nearest-neighbor enlarges each cell. Optional columns sets the frame grid; omit it for ceil(sqrt(frame count)). Does not change the document or the undo stack.",
    {
      scale: int("Nearest-neighbor scale, integer >= 1. Default 1.", 1, 4294967295),
      columns: int("Frame columns. Omit for a nearly square grid.", 1, 4294967295),
    },
  ),
  previewFrame: tool(
    "previewFrame",
    "Read-only view of one frame. Returns ink, bounds, index counts, the row-major index buffer, and a scaled color PNG as standard base64. Optional frame (default the active frame) does not select it. Optional scale (integer >= 1, default 1) nearest-neighbor repeats each index. Does not change the document or the undo stack.",
    {
      frame: int("Frame index to draw. Omit for the active frame.", 0, 4294967295),
      scale: int("Nearest-neighbor scale, integer >= 1. Default 1.", 1, 4294967295),
      layer: int("Draw this layer only. Omit for the composite.", 0, 4294967295),
    },
  ),
  diffFrames: tool(
    "diffFrames",
    "Read-only comparison of two frames. Returns changed, bounds, kinds, changes (left and right indices), diff rows, and a base64 PNG with three panels: frame, with, and an onion blend. Shared indices stay one palette color. Differing indices are averaged only in the view. Does not change the document.",
    {
      frame: int("First frame, left panel.", 0, 4294967295),
      with: int("Second frame, middle panel.", 0, 4294967295),
      scale: int("Nearest-neighbor scale, integer >= 1. Default 1.", 1, 4294967295),
      layer: int("Compare this layer only. Omit for the composite.", 0, 4294967295),
    },
    ["frame", "with"],
  ),
  flipHorizontal: tool(
    "flipHorizontal",
    "Mirror palette indices left to right inside a rectangle or the whole frame. Cell x receives the index from the mirrored column. Does not recolor. One undo step. Optional frame does not change the active frame. Omit x, y, width, and height together to flip the whole frame. A cell is written only when it and its mirror partner are both inside the stored selection.",
    {
      frame: int("Frame to mirror. Omit for the active frame.", 0, 4294967295),
      x: int("Left column. Omit with the other edges to flip the whole frame.", 0, 65535),
      y: int("Top row.", 0, 65535),
      width: int("Width in cells, at least 1.", 1, 65535),
      height: int("Height in cells, at least 1.", 1, 65535),
      layer: int("Layer to mirror. Omit for the active layer. Does not select it.", 0, 4294967295),
      reportDirty: REPORT_DIRTY,
    },
  ),
  resizeCanvas: tool(
    "resizeCanvas",
    "Copy every frame's indices onto a new canvas from 1 to 256. Omit offsetX and offsetY to center the old pixels; pass both to place the old origin. Cells outside stay transparent. One undo step. The project file stays version 1. Does not recolor.",
    {
      width: int("New width, 1 to 256.", 1, 256),
      height: int("New height, 1 to 256.", 1, 256),
      offsetX: int("Where the old left edge lands. Negative clips.", -2147483648, 2147483647),
      offsetY: int("Where the old top edge lands. Negative clips.", -2147483648, 2147483647),
      reportDirty: REPORT_DIRTY,
    },
    ["width", "height"],
  ),
  listLayers: tool(
    "listLayers",
    "Read the layer stack. Index 0 is the bottom. Does not change the document.",
  ),
  addLayer: tool(
    "addLayer",
    "Insert a transparent layer above the active one on every frame and select it. Omit name for the next free Layer N. One undo step.",
    {
      name: { type: "string", description: "Layer name, 1 to 64 characters. Omit for Layer N." },
    },
  ),
  deleteLayer: tool(
    "deleteLayer",
    "Remove a layer from every frame. At least one layer must remain. Omit index to remove the active layer.",
    {
      index: int("Layer index, 0-based. Omit for the active layer.", 0, 4294967295),
    },
  ),
  renameLayer: tool(
    "renameLayer",
    "Rename a layer. The stored name is trimmed. Omit index to rename the active layer. The same name is not recorded.",
    {
      index: int("Layer index, 0-based. Omit for the active layer.", 0, 4294967295),
      name: { type: "string", description: "New name, 1 to 64 characters." },
    },
    ["name"],
  ),
  setActiveLayer: tool(
    "setActiveLayer",
    "Select the layer the next paint writes. The same index is not recorded.",
    {
      index: int("Layer index, 0-based. 0 is the bottom.", 0, 4294967295),
    },
    ["index"],
  ),
  reorderLayer: tool(
    "reorderLayer",
    "Move a layer so it ends at `to`. Index 0 stays the bottom. The active layer follows the move. One undo step.",
    {
      from: int("Current layer index.", 0, 4294967295),
      to: int("Index after the move.", 0, 4294967295),
    },
    ["from", "to"],
  ),
  flattenLayers: tool(
    "flattenLayers",
    "Composite every frame into one layer named Layer 1. Index 0 shows through; any other index covers the layer under it. The saved file is a single-layer project. One undo step. A document that is already that one layer is not recorded.",
  ),
} satisfies Record<ToolName, ChatTool>;

/** Catalog in `TOOL_NAMES` order. One entry per kernel tool. */
export const TOOL_CATALOG: readonly ChatTool[] = TOOL_NAMES.map((name) => BY_NAME[name]);

/** Tools that never push an undo entry. */
export const READ_ONLY_TOOLS: readonly ToolName[] = [
  "inspect",
  "scene",
  "saveProject",
  "exportPngSequence",
  "exportSpriteSheet",
  "previewFrame",
  "diffFrames",
  "listLayers",
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
