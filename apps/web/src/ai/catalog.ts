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
  type: "integer" | "number" | "boolean" | "string" | "array";
  description: string;
  minimum?: number;
  maximum?: number;
  enum?: readonly string[];
  items?: {
    type: "integer" | "string" | "object";
    description?: string;
    minimum?: number;
    maximum?: number;
  };
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
    "Read a compact summary: canvas size, active frame and layer, layer names, visibility, opacity, palette hash, and frame delays. Does not return pixels. visible is a boolean. opacity is 0 to 255; 255 is fully opaque. Changes nothing.",
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
  strokeRect: tool(
    "strokeRect",
    "Draw the border of an axis-aligned rectangle on the active frame. The interior stays. One undo step. A width or height of 1 draws that whole line. A locked palette still allows this. The rectangle must lie entirely inside the canvas. Index 0 is refused. Cells outside the stored selection are skipped.",
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
  line: tool(
    "line",
    "Draw a 1px continuous Bresenham line. Consecutive cells share an edge or a corner, and both endpoints are included. pixelPerfect walks one cell per step on the longer axis and stays 8-connected. No anti-aliasing. One undo step. Both ends must lie on the canvas. A locked palette still allows an existing index. Index 0 is refused. Cells outside the stored selection are skipped.",
    {
      x0: int("Start column, 0-based.", 0, 65535),
      y0: int("Start row, 0-based.", 0, 65535),
      x1: int("End column, 0-based.", 0, 65535),
      y1: int("End row, 0-based.", 0, 65535),
      colorIndex: int("Opaque palette index that already exists.", 1, 255),
      pixelPerfect: {
        type: "boolean",
        description: "One cell per step on the longer axis. Omit for the continuous Bresenham line.",
      },
      layer: int("Layer to edit. Omit for the active layer. Does not select it.", 0, 4294967295),
      reportDirty: REPORT_DIRTY,
    },
    ["x0", "y0", "x1", "y1", "colorIndex"],
  ),
  ellipse: tool(
    "ellipse",
    "Draw an ellipse inscribed in an axis-aligned rectangle. fill paints the interior and includes the outline. A width or height of 1 is that line. No anti-aliasing. One undo step. The rectangle must lie entirely inside the canvas. Index 0 is refused. Cells outside the stored selection are skipped.",
    {
      x: int("Left column of the bounding rectangle, 0-based.", 0, 65535),
      y: int("Top row of the bounding rectangle, 0-based.", 0, 65535),
      width: int("Width in cells, at least 1.", 1, 65535),
      height: int("Height in cells, at least 1.", 1, 65535),
      colorIndex: int("Opaque palette index that already exists.", 1, 255),
      fill: {
        type: "boolean",
        description: "Paint the interior as well as the outline. Omit for the outline only.",
      },
      layer: int("Layer to edit. Omit for the active layer. Does not select it.", 0, 4294967295),
      reportDirty: REPORT_DIRTY,
    },
    ["x", "y", "width", "height", "colorIndex"],
  ),
  polygon: tool(
    "polygon",
    "Draw a closed polygon through points in order, at least 3 vertices. Edges use the same continuous line as line. fill adds the even-odd interior. No curves and no anti-aliasing. One undo step. Every vertex must lie on the canvas. Index 0 is refused. Cells outside the stored selection are skipped.",
    {
      points: {
        type: "array",
        description: "Vertices in order. At least 3 and at most 4096. Each item is {x, y}.",
        items: {
          type: "object",
          description: "One vertex. x and y are integers from 0 to 65535.",
        },
      },
      colorIndex: int("Opaque palette index that already exists.", 1, 255),
      fill: {
        type: "boolean",
        description: "Paint the even-odd interior as well as the outline.",
      },
      layer: int("Layer to edit. Omit for the active layer. Does not select it.", 0, 4294967295),
      reportDirty: REPORT_DIRTY,
    },
    ["points", "colorIndex"],
  ),
  fill: tool(
    "fill",
    "Replace the connected region of the seed cell's current index with colorIndex. connectivity 4 walks orthogonal neighbors. 8 also crosses corners. Cells outside the selection are barriers. One undo step. The seed must lie on the canvas. Index 0 is refused as the new color. A locked palette still allows an existing index.",
    {
      x: int("Seed column, 0-based.", 0, 65535),
      y: int("Seed row, 0-based.", 0, 65535),
      colorIndex: int("Opaque palette index that already exists.", 1, 255),
      connectivity: int("4 or 8. Omit for 4.", 4, 8),
      layer: int("Layer to edit. Omit for the active layer. Does not select it.", 0, 4294967295),
      reportDirty: REPORT_DIRTY,
    },
    ["x", "y", "colorIndex"],
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
    "Store one rectangle in the project file. Later paint, erase, fillRect, strokeRect, line, ellipse, polygon, fill, eraseRect, copyRect, flipHorizontal, flipVertical, rotate, and replaceIndex skip cells outside it. fill does not travel through cells outside the mask. flipVertical and rotate also require the source cell to sit inside. moveRect requires both rectangles inside it. combineMask can turn this rectangle into a mask. Does not change pixels or the active layer. There is no lasso.",
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
    "Remove the mask so the whole canvas is writable. The project file omits selection. Does not change pixels. Already clear is not recorded. An empty mask is not the same as no selection.",
  ),
  combineMask: tool(
    "combineMask",
    "Combine axis-aligned rectangles with union, intersect, or subtract and store the mask. Pass x, y, width, and height for one rectangle. Omit baseX, baseY, baseWidth, and baseHeight to use the stored selection. Union with no selection stores the operand. Intersect or subtract with no selection is MaskMissing. An empty result protects every cell. Does not change pixels or the active layer. There is no lasso.",
    {
      op: {
        type: "string",
        description: "union, intersect, or subtract.",
        enum: ["union", "intersect", "subtract"],
      },
      x: int("Left column of the operand.", 0, 65535),
      y: int("Top row of the operand.", 0, 65535),
      width: int("Operand width in cells, at least 1.", 1, 65535),
      height: int("Operand height in cells, at least 1.", 1, 65535),
      baseX: int("Left column of the other mask. Omit all four base edges to use the selection.", 0, 65535),
      baseY: int("Top row of the other mask.", 0, 65535),
      baseWidth: int("Width of the other mask, at least 1.", 1, 65535),
      baseHeight: int("Height of the other mask, at least 1.", 1, 65535),
    },
    ["op"],
  ),
  selectConnected: tool(
    "selectConnected",
    "Store the seed cell's indexed connected component as the selection mask. tolerance 0 matches that palette index. tolerance 1 to 255 is the max distance on r, g, b, and a. connectivity 4 or 8 matches fill. Optional layer and frame sample stored indices and do not select them. Replaces any current mask. Does not change pixels. There is no lasso.",
    {
      x: int("Seed column, 0-based.", 0, 65535),
      y: int("Seed row, 0-based.", 0, 65535),
      tolerance: int("0 matches the seed index. 1 to 255 is the max channel distance.", 0, 255),
      connectivity: int("4 orthogonal, or 8 including corners. Default 4.", 4, 8),
      layer: int("Layer to sample. Omit for the active layer. Does not select it.", 0, 4294967295),
      frame: int("Frame to sample. Omit for the active frame. Does not select it.", 0, 4294967295),
    },
    ["x", "y"],
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
  listEngines: tool(
    "listEngines",
    "List convert engines. center-sample, dominant, kmeans, contrast-outline, and grid-snap are available. pixelforge-native is reserved and is not installed. Changes nothing.",
  ),
  setTransformEngine: tool(
    "setTransformEngine",
    "Store the default convert engine id on the project. The same id is not recorded. pixelforge-native is refused and does not fall back.",
    {
      id: { type: "string", description: "Engine id from listEngines." },
    },
    ["id"],
  ),
  convertPng: tool(
    "convertPng",
    "Sample an arbitrary PNG into one indexed frame at width and height. Alpha below 128 becomes index 0. palette.strategy is source (keep the image palette), limit (cap with maxColors, default 16), or project (the current palette). An unknown strategy errors and does not fall back. The result paletteReport lists merges and squared-RGB error. palette.mode remains project, source, or provided and does not emit that report. Pass strategy or mode, not both. features are stored and not applied. Engine params are an opaque object. One undo step.",
    {
      png: { type: "string", description: "Standard base64 PNG. Whitespace is ignored. Each edge must be at most 4096." },
      width: int("Target width, 1 to 256. Pass with height, or omit both.", 1, 256),
      height: int("Target height, 1 to 256. Pass with width, or omit both.", 1, 256),
      engine: { type: "string", description: "Engine id. Omit for the project's stored engine." },
      features: {
        type: "array",
        description: "Feature names to record. This version does not apply them.",
        items: { type: "string", description: "One feature name, 1 to 64 characters." },
      },
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
  addTag: tool(
    "addTag",
    "Name an inclusive frame range. loop defaults to forward and is one of forward, reverse, pingpong, or once. Names are unique. This is not a linked cel: each frame keeps its own pixels. One undo step.",
    {
      name: { type: "string", description: "Tag name, trimmed, 1 to 64 characters." },
      from: int("First frame, inclusive.", 0, 4294967295),
      to: int("Last frame, inclusive.", 0, 4294967295),
      loop: { type: "string", description: "forward, reverse, pingpong, or once. Default forward." },
    },
    ["name", "from", "to"],
  ),
  setTag: tool(
    "setTag",
    "Change from, to, loop, or the name of an existing tag. Pass at least one of those. The same resulting tag is not recorded. One undo step.",
    {
      name: { type: "string", description: "Current tag name." },
      from: int("New first frame, inclusive. Omit to keep it.", 0, 4294967295),
      to: int("New last frame, inclusive. Omit to keep it.", 0, 4294967295),
      loop: { type: "string", description: "forward, reverse, pingpong, or once." },
      rename: { type: "string", description: "New unique name. Omit to keep the current name." },
    },
    ["name"],
  ),
  removeTag: tool(
    "removeTag",
    "Remove a tag by name. Frames stay. One undo step.",
    {
      name: { type: "string", description: "Tag name to remove." },
    },
    ["name"],
  ),
  setDirectionLayout: tool(
    "setDirectionLayout",
    "Store a 4-direction (N/E/S/W) or 8-direction grid. Rows stay down, left, right, up, then the diagonals. apply true writes idle/walk tags with the existing tag tools. It does not paint a gait. The same layout is not recorded.",
    {
      count: int("4 for cardinals, or 8 to append diagonals.", 4, 8),
      columns: int("Frames in each facing row.", 1, 4096),
      apply: { type: "boolean", description: "When true, also write idle on column 0 and walk on the rest." },
    },
    ["count", "columns"],
  ),
  clearDirectionLayout: tool(
    "clearDirectionLayout",
    "Remove the direction layout. Tags stay. A missing layout is not recorded.",
  ),
  applyDirectionTags: tool(
    "applyDirectionTags",
    "Write idle/walk tags for the stored layout. One-frame rows get idle only. Longer rows get idle on the first frame and walk on the rest. Alias names such as idle_south are kept. Does not paint. Two names for one segment fails and writes nothing.",
  ),
  validateDirections: tool(
    "validateDirections",
    "Hard checklist for missing facings and missing idle/walk spans. errorCount above zero is a failed gate. Does not write. A pass is not a natural gait or an aesthetic pass.",
  ),
  listDirections: tool(
    "listDirections",
    "List each facing row, its compass id, and the tags on that row. Does not write.",
  ),
  exportDirection: tool(
    "exportDirection",
    "Export one facing as a one-row PNG. facing accepts N/E/S/W, down/left/right/up, or a diagonal when count is 8. Does not write the project. The PNG is returned as base64.",
    {
      facing: { type: "string", description: "N, E, S, W, a diagonal, or down, left, right, up." },
      scale: int("Nearest-neighbor scale, at least 1. Omit for 1.", 1, 4294967295),
    },
    ["facing"],
  ),
  fillDirections: tool(
    "fillDirections",
    "Copy one cardinal facing's segment frames onto the other cardinals, or horizontally mirror left and right. mode is mirror or copy. Does not fill diagonal rows. Skips a target frame that already has ink unless overwrite is true. Does not write tags, move parts, recolor, or repair highlights. Not a natural gait and not a generated four-direction drawing. One undo step when pixels or delays change.",
    {
      from: { type: "string", description: "Source facing: N, E, S, W, or down, left, right, up. mirror accepts only left or right." },
      mode: { type: "string", description: "mirror flips left and right. copy repeats the source on the other cardinals and does not flip." },
      overwrite: { type: "boolean", description: "When true, replace target frames that already hold ink or an explicit erase. Omit to skip them." },
    },
    ["from", "mode"],
  ),
  listParts: tool(
    "listParts",
    "Read named parts and their anchors. A part is not a layer. Does not change the document.",
  ),
  addPart: tool(
    "addPart",
    "Add a named part. x and y are the canvas cell that is local (0, 0). Optional layer attaches the part to that paint layer and does not select it. Omit layer to leave the part unattached. Names are unique. One undo step. Does not write pixels.",
    {
      name: { type: "string", description: "Part name, trimmed, 1 to 64 characters." },
      x: int("Origin column. Must lie on the canvas.", 0, 65535),
      y: int("Origin row. Must lie on the canvas.", 0, 65535),
      layer: int("Paint layer this part names. Omit to leave it unattached.", 0, 4294967295),
    },
    ["name", "x", "y"],
  ),
  setPart: tool(
    "setPart",
    "Change a part origin, layer attachment, or name. Pass at least one of x, y, layer, clearLayer, or rename. clearLayer true drops the attachment and cannot be combined with layer. The same resulting part is not recorded. Anchors stay in local cells. One undo step.",
    {
      name: { type: "string", description: "Current part name." },
      x: int("New origin column. Omit to keep it.", 0, 65535),
      y: int("New origin row. Omit to keep it.", 0, 65535),
      layer: int("Attach the part to this layer. Omit to keep the attachment.", 0, 4294967295),
      clearLayer: { type: "boolean", description: "When true, drop the layer attachment. Do not pass layer as well." },
      rename: { type: "string", description: "New unique name. Omit to keep the current name." },
    },
    ["name"],
  ),
  removePart: tool(
    "removePart",
    "Remove a part and its anchors. Pixels stay. One undo step.",
    {
      name: { type: "string", description: "Part name to remove." },
    },
    ["name"],
  ),
  setAnchor: tool(
    "setAnchor",
    "Create or replace a named anchor on a part. x and y are local pixel offsets from the part origin, from -256 to 256, not canvas coordinates. The same offset is not recorded. One undo step. Does not rotate or write pixels.",
    {
      part: { type: "string", description: "Part name." },
      name: { type: "string", description: "Anchor name, trimmed, 1 to 64 characters. Unique on this part." },
      x: int("Local column, from -256 to 256.", -256, 256),
      y: int("Local row, from -256 to 256.", -256, 256),
    },
    ["part", "name", "x", "y"],
  ),
  removeAnchor: tool(
    "removeAnchor",
    "Remove one anchor from a part. The part stays. One undo step.",
    {
      part: { type: "string", description: "Part name." },
      name: { type: "string", description: "Anchor name to remove." },
    },
    ["part", "name"],
  ),
  translatePart: tool(
    "translatePart",
    "Move a part by whole cells. Local anchors stay. Non-zero indices on the linked layer of one frame move with the origin, in one undo step. An unattached part only moves metadata. Does not invent colors. A selection must cover every ink cell and its destination.",
    {
      name: { type: "string", description: "Part name." },
      dx: int("Cells added to x. May be negative.", -2147483648, 2147483647),
      dy: int("Cells added to y. May be negative.", -2147483648, 2147483647),
      frame: int("Frame to edit. Omit for the active frame. Does not select it.", 0, 4294967295),
      reportDirty: REPORT_DIRTY,
    },
    ["name", "dx", "dy"],
  ),
  rotatePart: tool(
    "rotatePart",
    "Rotate a part clockwise around its origin, or around a named anchor cell center. A multiple of 90 within 0.01 degree is an exact reversible quarter turn. Any other angle quantizes to 0.01 degree, then each ink cell snaps to the nearest cell. One frame and the part metadata, one undo step. Does not invent colors.",
    {
      name: { type: "string", description: "Part name." },
      degrees: {
        type: "number",
        description:
          "Clockwise degrees, finite, absolute value at most 1000000. 90.004 snaps to 90. 90.005 stays off the quarter-turn path.",
      },
      anchor: {
        type: "string",
        description: "Pivot anchor. Omit to pivot on the part origin.",
      },
      frame: int("Frame to edit. Omit for the active frame. Does not select it.", 0, 4294967295),
      reportDirty: REPORT_DIRTY,
    },
    ["name", "degrees"],
  ),
  flipPart: tool(
    "flipPart",
    "Mirror a part through its origin cell or a named anchor cell. direction is horizontal or vertical. Indices move and colors stay. Same direction twice restores a part that stayed on the canvas. Rectangle flipHorizontal does not move parts. One undo step.",
    {
      name: { type: "string", description: "Part name." },
      direction: {
        type: "string",
        description: "horizontal or vertical.",
        enum: ["horizontal", "vertical"],
      },
      anchor: {
        type: "string",
        description: "Pivot anchor. Omit to pivot on the part origin.",
      },
      frame: int("Frame to edit. Omit for the active frame. Does not select it.", 0, 4294967295),
      reportDirty: REPORT_DIRTY,
    },
    ["name", "direction"],
  ),
  setCurveKey: tool(
    "setCurveKey",
    "Set one key on a part channel. frame is an integer frame index, the same axis as tags. value is clockwise degrees for rotation, whole cells for translateX or translateY, or 0 or 1 for flipHorizontal or flipVertical. ease is how this key leaves toward the next key: linear, easeInOut (smoothstep u = t*t*(3-2*t)), or hold. Omit ease for linear, or hold on a flip channel. Flip rejects any other ease. The same stored key is not recorded. Does not write pixels. One undo step.",
    {
      part: { type: "string", description: "Part name." },
      channel: {
        type: "string",
        description: "rotation, translateX, translateY, flipHorizontal, or flipVertical.",
        enum: ["rotation", "translateX", "translateY", "flipHorizontal", "flipVertical"],
      },
      frame: int("Frame index, 0-based. Must exist.", 0, 4294967295),
      value: {
        type: "number",
        description:
          "Degrees, whole cells, or 0/1. Rotation quantizes to 0.01 degree. Translate must be an integer.",
      },
      ease: {
        type: "string",
        description: "linear, easeInOut, or hold. Omit for the channel default.",
        enum: ["linear", "easeInOut", "hold"],
      },
    },
    ["part", "channel", "frame", "value"],
  ),
  removeCurveKey: tool(
    "removeCurveKey",
    "Remove one key. The last key drops the curve. Frames and pixels stay. One undo step.",
    {
      part: { type: "string", description: "Part name." },
      channel: {
        type: "string",
        description: "rotation, translateX, translateY, flipHorizontal, or flipVertical.",
        enum: ["rotation", "translateX", "translateY", "flipHorizontal", "flipVertical"],
      },
      frame: int("Frame index of the key.", 0, 4294967295),
    },
    ["part", "channel", "frame"],
  ),
  clearCurve: tool(
    "clearCurve",
    "Remove every key of one part channel. One undo step. Does not write pixels.",
    {
      part: { type: "string", description: "Part name." },
      channel: {
        type: "string",
        description: "rotation, translateX, translateY, flipHorizontal, or flipVertical.",
        enum: ["rotation", "translateX", "translateY", "flipHorizontal", "flipVertical"],
      },
    },
    ["part", "channel"],
  ),
  listCurves: tool(
    "listCurves",
    "Read part-parameter curves. Pass part to list one part. Does not change the document.",
    {
      part: { type: "string", description: "Part name. Omit to list every curve." },
    },
  ),
  sampleCurve: tool(
    "sampleCurve",
    "Interpolate one channel at a frame position. frame may be fractional: 2 is frame 2, and 2.5 is halfway to frame 3 in frame-index space, not weighted by delayMs. easeInOut is t*t*(3-2*t); at t=0.5 it matches linear. Does not write pixels.",
    {
      part: { type: "string", description: "Part name." },
      channel: {
        type: "string",
        description: "rotation, translateX, translateY, flipHorizontal, or flipVertical.",
        enum: ["rotation", "translateX", "translateY", "flipHorizontal", "flipVertical"],
      },
      frame: {
        type: "number",
        description: "Frame position. May be fractional. Not weighted by delayMs.",
      },
    },
    ["part", "channel", "frame"],
  ),
  samplePart: tool(
    "samplePart",
    "Interpolate every keyed channel of one part. Missing channels are omitted, not zero. Values are the pose relative to the rest origin. Does not call rotatePart, translatePart, or flipPart, and does not write pixels.",
    {
      part: { type: "string", description: "Part name." },
      frame: {
        type: "number",
        description: "Frame position. May be fractional. Not weighted by delayMs.",
      },
    },
    ["part", "frame"],
  ),
  addConstraint: tool(
    "addConstraint",
    "Store a follow, attach, pin, ik, or plant constraint. stick is stored as attach. follow moves part so its anchor, or its origin, lands on targetAnchor. attach requires both anchors. pin stores one canvas cell. ik rotates a chain of two or three parts so the end cell is exactly the target. plant pins an anchor on footprint spans. Does not move pixels. One undo step.",
    {
      name: { type: "string", description: "Constraint name, 1 to 64 characters. Unique in the file." },
      kind: {
        type: "string",
        description: "follow, attach, stick, pin, ik, or plant. stick is stored as attach.",
        enum: ["follow", "attach", "stick", "pin", "ik", "plant"],
      },
      part: { type: "string", description: "Part this constraint moves. For ik, the end of the chain." },
      anchor: { type: "string", description: "Follower anchor. Omit on follow, pin, or plant to use the origin. On ik, the end effector. Omit it to aim the end origin." },
      target: { type: "string", description: "Part that follow, attach, or ik tracks." },
      targetAnchor: { type: "string", description: "Anchor on the target. Required for follow, attach, and an ik part target." },
      x: int("Pin or ik column. Required with y for pin. Must lie on the canvas.", -2147483648, 2147483647),
      y: int("Pin or ik row. Required with x for pin. Must lie on the canvas.", -2147483648, 2147483647),
      chain: {
        type: "array",
        description: "IK joint names, root first, two or three. The last name is part.",
        items: { type: "string" },
      },
      bend: int("1 clockwise of root-to-target, or -1 the other side. Omit to keep the rest bend.", -1, 1),
      contacts: {
        type: "array",
        description: "Footprint spans. Each item is {frame, to, x, y}. to defaults to frame. A sample inside the span uses that cell.",
        items: { type: "object" },
      },
    },
    ["name", "kind", "part"],
  ),
  removeConstraint: tool(
    "removeConstraint",
    "Remove one constraint by name. Parts and pixels stay. One undo step.",
    {
      name: { type: "string", description: "Constraint name." },
    },
    ["name"],
  ),
  listConstraints: tool(
    "listConstraints",
    "Read stored constraints in file order. Does not change the document.",
  ),
  solveConstraints: tool(
    "solveConstraints",
    "Sample curves when frame is set, then solve follow, attach, pin, ik, and plant. ik must land on the target cell or the call is IkUnreachable and writes nothing. A footprint span pins that cell; frames outside every span are not pinned. Omit frame to use the stored pose and to sample footprints at pixelFrame. Omit apply, or send false, and nothing is written. apply true writes one pixelFrame (the active frame when omitted) as one undo step when the pose changed. A linked layer's non-zero ink moves. An unattached part moves its origin only. A second call stacks. This poses one frame. bake walks the timeline.",
    {
      frame: {
        type: "number",
        description: "Curve frame. May be fractional. Omit to skip curves.",
      },
      apply: {
        type: "boolean",
        description: "Write the pose when true. Omit or false to leave the document unchanged.",
      },
      pixelFrame: int("Pixel frame to write. Omit for the active frame.", 0, 4294967295),
    },
  ),
  bake: tool(
    "bake",
    "Sample part curves, then solve constraints, and write ordinary editable pixel frames over an inclusive index range. The source frame's pixels stay the rest pose, so a second bake does not stack. Stored part origins, curves, and constraints stay. fps sets delayMs to round(1000/fps) on every frame in the range, including the source. Omit parts to pose every part. Omit layers to write layers managed by the sampled curves and solved constraints, including an identity pose, or the linked layers of parts when that filter is set. A polish layer is never written. A managed part on a polish layer refuses the bake. constraints false samples curves and skips the solver. from past the last frame appends posed copies of the source. One bake is one undo step when pixels or delays change. There is no script runtime.",
    {
      from: int("First frame index, inclusive. Default 0. May equal the frame count to append only.", 0, 4095),
      to: int("Last frame index, inclusive. Default is the last existing frame. Past the end appends frames. Must be below 4096.", 0, 4095),
      source: int("Rest-pose frame. Default 0. Its pixels are not rewritten.", 0, 4294967295),
      fps: {
        type: "number",
        description: "Playback rate. Sets delayMs to round(1000/fps) on every frame in the range. Must be finite and greater than 0.",
        minimum: 0,
      },
      parts: names("Pose only these parts. Omit to pose every part. An empty array is refused."),
      layers: indices("Write only these layers. Omit to write layers a curve or constraint manages. A polish layer is skipped."),
      constraints: {
        type: "boolean",
        description: "Solve constraints after sampling curves. Omit or true solves them. false samples curves only.",
      },
    },
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
  importAseprite: tool(
    "importAseprite",
    "Replace the document with an indexed .aseprite file. bytes is standard base64 of the file. Reads canvas size, the indexed palette, image-layer pixels, and frame durations. Groups are not stored; their child image layers become a flat stack. Reference layers are skipped. Hidden layers become visible. Blend, opacity, and cel z-index are not applied. Tags, slices, tilemaps, and color profiles are not imported. Index 0 becomes the transparent slot, swapping with the file's transparent index when that index is not 0. Does not write a .aseprite file. The undo stack of the new document is empty. There is no plugin API.",
    {
      bytes: { type: "string", description: "Standard base64 of an indexed .aseprite or .ase file. No data: prefix." },
    },
    ["bytes"],
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
    "Return one grid sprite sheet as a base64 PNG, plus columns, rows, and sheet size. Optional scale (integer >= 1, default 1) nearest-neighbor enlarges each cell. Optional columns sets the frame grid; omit it for ceil(sqrt(frame count)). Optional atlas true adds a pixelforge-atlas object with frame rects, delayMs, columns, and tags. Omit atlas and the result stays the PNG fields. Does not change the document or the undo stack.",
    {
      scale: int("Nearest-neighbor scale, integer >= 1. Default 1.", 1, 4294967295),
      columns: int("Frame columns. Omit for a nearly square grid.", 1, 4294967295),
      atlas: { type: "boolean", description: "When true, add frame rects, delays, columns, and tags. Omit to keep the PNG fields only." },
    },
  ),
  exportApng: tool(
    "exportApng",
    "Return one looping APNG preview as standard base64. Uses the same RGBA composite as exportPngSequence: a hidden layer or opacity 0 is skipped, and opacity 1 to 254 blends. Optional tag plays that tag's inclusive range and loop. Omit tag to play from and to, defaulting to the whole timeline, with loop forward, reverse, pingpong, or once. forward, reverse, and pingpong loop forever (plays 0). once plays a single time (plays 1). Each frame keeps its delayMs. Optional scale is an integer >= 1. This is a preview, not a sprite sheet. It does not quantize or dither. Do not pass tag together with from, to, or loop. Does not change the document or the undo stack.",
    {
      scale: int("Nearest-neighbor scale, integer >= 1. Default 1.", 1, 4294967295),
      tag: { type: "string", description: "Tag name. Uses that range and loop. Omit to use from, to, and loop." },
      from: int("First frame, inclusive. Omit for 0. Do not pass with tag.", 0, 4294967295),
      to: int("Last frame, inclusive. Omit for the last frame. Do not pass with tag.", 0, 4294967295),
      loop: {
        type: "string",
        enum: ["forward", "reverse", "pingpong", "once"],
        description: "Playback when tag is omitted. Default forward.",
      },
    },
  ),
  previewFrame: tool(
    "previewFrame",
    "Read-only view of one frame. Returns ink, bounds, index counts, the row-major index buffer, and a scaled color PNG as standard base64. Optional frame (default the active frame) does not select it. Optional scale (integer >= 1, default 1) nearest-neighbor repeats each index. Pass selection true to count only the stored mask, or x, y, width, and height for one rectangle. Omit both for the whole canvas. Pass only one scope. Cells outside are protected in the view and are not ink. compare true writes two panels and no gutter: left is the unquantized RGBA composite, right is the palette color of the indices flattenLayers would write. pixels stays the covering index; flattenPixels is the right panel. Omit layer and any mask when compare is true. Does not change the document or the undo stack.",
    {
      frame: int("Frame index to draw. Omit for the active frame.", 0, 4294967295),
      scale: int("Nearest-neighbor scale, integer >= 1. Default 1.", 1, 4294967295),
      layer: int("Draw this layer only. Omit for the composite. Do not pass with compare.", 0, 4294967295),
      compare: {
        type: "boolean",
        description:
          "When true, PNG is unquantized RGBA beside the indices flatten would write. Does not flatten. Omit layer and any mask.",
      },
      selection: {
        type: "boolean",
        description: "When true, count only the stored selection. Missing selection is MaskMissing.",
      },
      x: int("Left column of an explicit rectangle. Pass y, width, and height too.", 0, 65535),
      y: int("Top row of an explicit rectangle.", 0, 65535),
      width: int("Width of an explicit rectangle, at least 1.", 1, 65535),
      height: int("Height of an explicit rectangle, at least 1.", 1, 65535),
    },
  ),
  diffFrames: tool(
    "diffFrames",
    "Read-only comparison of two frames. Returns changed, bounds, kinds, changes (left and right indices), diff rows, and a base64 PNG with three panels: frame, with, and an onion blend. Shared indices stay one palette color. Differing indices are averaged only in the view. Pass selection true to compare only the stored mask, or x, y, width, and height for one rectangle. Omit both for the whole canvas. Pass only one scope. Cells outside are not changes. Does not change the document.",
    {
      frame: int("First frame, left panel.", 0, 4294967295),
      with: int("Second frame, middle panel.", 0, 4294967295),
      scale: int("Nearest-neighbor scale, integer >= 1. Default 1.", 1, 4294967295),
      layer: int("Compare this layer only. Omit for the composite.", 0, 4294967295),
      selection: {
        type: "boolean",
        description: "When true, compare only the stored selection. Missing selection is MaskMissing.",
      },
      x: int("Left column of an explicit rectangle. Pass y, width, and height too.", 0, 65535),
      y: int("Top row of an explicit rectangle.", 0, 65535),
      width: int("Width of an explicit rectangle, at least 1.", 1, 65535),
      height: int("Height of an explicit rectangle, at least 1.", 1, 65535),
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
  flipVertical: tool(
    "flipVertical",
    "Mirror palette indices top to bottom inside a rectangle or the whole frame. Does not recolor. One undo step. Optional frame does not change the active frame. Omit x, y, width, and height together to flip the whole frame. A cell is written only when it and the cell it reads are both inside the stored selection.",
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
  rotate: tool(
    "rotate",
    "Turn palette indices clockwise by 90, 180, or 270 degrees inside a rectangle or the whole frame. 90 and 270 need a square; the canvas size stays. Does not recolor. One undo step. Optional frame does not change the active frame. A cell is written only when it and the cell it reads are both inside the stored selection.",
    {
      degrees: int("Clockwise degrees: 90, 180, or 270.", 90, 270),
      frame: int("Frame to turn. Omit for the active frame.", 0, 4294967295),
      x: int("Left column. Omit with the other edges to turn the whole frame.", 0, 65535),
      y: int("Top row.", 0, 65535),
      width: int("Width in cells, at least 1.", 1, 65535),
      height: int("Height in cells, at least 1.", 1, 65535),
      layer: int("Layer to turn. Omit for the active layer. Does not select it.", 0, 4294967295),
      reportDirty: REPORT_DIRTY,
    },
    ["degrees"],
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
    "Read the layer stack. Index 0 is the bottom. Each layer has role, visible, and opacity (0 to 255, 255 fully opaque). Does not change the document.",
  ),
  addLayer: tool(
    "addLayer",
    "Insert a transparent layer above the active one on every frame and select it. Omit name for the next free Layer N. role polish is an overlay bake never writes. Omit role for default. One undo step.",
    {
      name: { type: "string", description: "Layer name, 1 to 64 characters. Omit for Layer N." },
      role: {
        type: "string",
        enum: ["default", "polish"],
        description: "default lets bake rebuild this layer. polish keeps hand-tuned cells. Omit for default.",
      },
    },
  ),
  setLayerRole: tool(
    "setLayerRole",
    "Set whether bake may rebuild this layer. polish is an overlay bake never writes. default is an ordinary layer. Pixels stay. The same role is not recorded. Omit index for the active layer.",
    {
      index: int("Layer index, 0-based. Omit for the active layer.", 0, 4294967295),
      role: {
        type: "string",
        enum: ["default", "polish"],
        description: "default or polish.",
      },
    },
    ["role"],
  ),
  setLayerVisible: tool(
    "setLayerVisible",
    "Show or hide one layer in the composite. Hidden layers are skipped by preview, diff, export, and flatten. Stored indices stay, and drawing that layer still writes them. previewFrame with layer set still shows it. The same value is not recorded. Omit index for the active layer. One undo step when the flag changes.",
    {
      index: int("Layer index, 0-based. Omit for the active layer.", 0, 4294967295),
      visible: { type: "boolean", description: "true shows the layer. false skips it in the composite." },
    },
    ["visible"],
  ),
  setLayerOpacity: tool(
    "setLayerOpacity",
    "Set composite opacity from 0 to 255. 255 covers with the palette color and does not blend palette alpha into the layer underneath. 0 skips the layer. 1 to 254 still covers in the index composite and blends in RGBA export with straight-alpha source-over. This call does not snap the blend. flattenLayers snaps that RGBA onto the nearest palette index. The same value is not recorded. Omit index for the active layer.",
    {
      index: int("Layer index, 0-based. Omit for the active layer.", 0, 4294967295),
      opacity: int("0 to 255. 255 is fully opaque. 128 is the nearest step to 50%.", 0, 255),
    },
    ["opacity"],
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
    "Composite every frame into one layer named Layer 1. Hidden layers and opacity 0 are omitted. Each cell of the straight-alpha blend snaps onto the palette: alpha below 128 is index 0, otherwise the nearest opaque entry by squared RGB, and a tie keeps the lower index. A cell whose RGBA already equals the covering index keeps that index. The result is visible and fully opaque, so layer-opacity translucency is not stored as alpha. The color preview stays unquantized until this call. One undo step. A document that is already that one default layer is not recorded.",
  ),
  setTileset: tool(
    "setTileset",
    "Declare this project a tileset. tileWidth and tileHeight are each 16 or 32, and the canvas must already equal that tile. Each frame is one tile. The same size is not recorded.",
    {
      tileWidth: int("Tile width, 16 or 32.", 1, 65535),
      tileHeight: int("Tile height, 16 or 32.", 1, 65535),
    },
    ["tileWidth", "tileHeight"],
  ),
  clearTileset: tool(
    "clearTileset",
    "Remove the tileset size. Pixels stay. A project that is not a tileset is not recorded.",
  ),
  validateTileset: tool(
    "validateTileset",
    "Check that the canvas is exactly the declared 16 or 32 tile. Does not write. tileCount is the frame count.",
  ),
  exportTileset: tool(
    "exportTileset",
    "Export a tileset PNG and pixelforge-tileset metadata. columns defaults to one row. image is a relative .png file name. Does not write a file and does not change the document.",
    {
      scale: int("Nearest-neighbor scale, at least 1. Omit for 1.", 1, 4294967295),
      columns: int("Tile-sheet columns. Omit for one row.", 1, 4294967295),
      image: { type: "string", description: "Relative .png file name stored in the metadata. Omit for tiles.png." },
    },
  ),
} satisfies Record<ToolName, ChatTool>;

/** Catalog in `TOOL_NAMES` order. One entry per kernel tool. */
export const TOOL_CATALOG: readonly ChatTool[] = TOOL_NAMES.map((name) => BY_NAME[name]);

/** Tools that never push an undo entry. */
export const READ_ONLY_TOOLS: readonly ToolName[] = [
  "inspect",
  "scene",
  "listEngines",
  "saveProject",
  "exportPngSequence",
  "exportSpriteSheet",
  "exportApng",
  "validateTileset",
  "exportTileset",
  "previewFrame",
  "diffFrames",
  "listLayers",
  "listParts",
  "listCurves",
  "sampleCurve",
  "samplePart",
  "listConstraints",
  "validateDirections",
  "listDirections",
  "exportDirection",
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

function names(description: string): SchemaProperty {
  return { type: "array", description, items: { type: "string" } };
}

function indices(description: string): SchemaProperty {
  return {
    type: "array",
    description,
    items: { type: "integer", minimum: 0, maximum: 4294967295 },
  };
}
