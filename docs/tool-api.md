# Tool API { #tool-api }

Use version 1 of the Tool API to inspect projects, edit indexed pixels and export frames. Call it from the CLI with `pixelforge call`, from Rust with `Document::call_tool`, or from WASM with `callTool`. Each entry below documents parameters, results and errors.

Omitting `apiVersion` or setting it to null selects version 1. Every response includes `"apiVersion": 1`. Requests in one CLI process share undo history. To undo between processes, supply the same checkpoint file to each modifying command. [CLI workflows](agent-skill.md)

## Request and response envelope { #envelope }

Rust and WASM return a compact JSON string. The CLI writes newline-delimited JSON and includes a `revision` for calls on a local pixel project. An optional string or numeric `id` associates a response with its request. In the tool signatures, `?` marks an optional key. Individual tools describe any conditional requirements.

```json
{ "apiVersion": 1, "id": "step-1", "tool": "paint", "params": { "x": 1, "y": 2, "colorIndex": 3 } }
```

CLI response on a new 16×16 project:

```json
{ "ok": true, "apiVersion": 1, "id": "step-1", "result": { "revision": "535a25d889238788" }, "revision": "535a25d889238788" }
```

For the same request with `x` set to 16 on a 16×16 canvas, the CLI returns:

```json
{
  "ok": false,
  "apiVersion": 1,
  "id": "step-1",
  "error": { "code": "OutOfBounds", "message": "cell 16,2 is outside 16x16" },
  "revision": "535a25d889238788"
}
```

Omit `params`, or send `null`, and both mean `{}`. It must be an object. Unknown fields are rejected. Integers are JSON numbers, not strings. Branch on `error.code`. `message` is for humans. On failure the document stays as it was before the call. `loadProject` replaces the document only after the project file parses.

## Error codes { #errors }

Protocol errors (the document is unchanged):

| code | When it occurs |
| --- | --- |
| `InvalidRequest` | The request must be a JSON object with a string `tool`, correctly typed `id` and `apiVersion`, and no unknown envelope fields. |
| `UnsupportedVersion` | `apiVersion` is not 1. |
| `UnknownTool` | Unknown `tool` name. |
| `InvalidParams` | A required parameter is absent, wrongly typed or unknown. Example: `paint: missing colorIndex`. |

Kernel errors pass through. `code` is `KernelError::code()` and `message` is its `Display`:

| code | Meaning |
| --- | --- |
| `OutOfBounds` | A cell or shape lies outside the canvas. The operation writes nothing. |
| `PolygonPoints` | Polygon has fewer than three vertices. The Tool API rejects this as `InvalidParams` before kernel dispatch. |
| `PaletteIndex` | Brush index is 0 or outside the palette. |
| `TransparentSlot` | Attempt to edit palette index 0. |
| `PaletteLocked` | Recolor, append or import attempted on a locked palette. |
| `PaletteFile` | Invalid GPL or hexadecimal color list. |
| `PaletteImportMode` | Import mode is neither `replace` nor `merge`. |
| `EmptyPalette` | Imported file contains no colors. |
| `PaletteTooLarge` | The palette would exceed 256 entries. |
| `NoPaintColor` | No opaque palette entry is available for mapping. |
| `PngFile` | Image bytes supplied to `applyPng` cannot be decoded. |
| `PngSize` | PNG dimensions differ from the canvas. Palette mapping preserves dimensions. |
| `EngineUnknown` | Unregistered conversion engine ID. |
| `PaletteStrategyUnknown` | Unknown `palette.strategy` in `convertPng`. |
| `PaletteInUse` | Replacing the shared palette under `source` or `limit` would affect ink on other frames or layers. |
| `PaletteDefault` | `project` requires an explicitly chosen palette. The built-in default is rejected, including when creating an output file. |
| `EngineUnavailable` | Registered engine has no implementation in this build, including `pixelforge-native`. |
| `EngineParams` | Missing, wrongly typed or unknown engine-specific parameter. |
| `ConvertFeature` | `features` exceeds 32 entries or contains an empty entry, control character or entry over 64 characters. |
| `ConvertSource` | Source PNG exceeds 4096 px on either side. Pixels are not expanded. |
| `FrameIndex` | Frame index out of range. |
| `LastFrame` | Attempt to delete the only frame. |
| `LayerIndex` | Layer index out of range. |
| `LastLayer` | Attempt to delete the only layer. |
| `LayerName` | Layer name is blank, contains control characters or exceeds 64 characters. |
| `FrameDelay` | `delayMs` is outside `1..=65535` |
| `TagName` | Tag name is blank, contains control characters or exceeds 64 characters. |
| `TagExists` | Duplicate tag name. |
| `TagMissing` | Unknown tag name. |
| `TagRange` | `from` exceeds `to` or the inclusive range extends beyond the frame list. |
| `PartName` | Part name is blank, contains control characters or exceeds 64 characters. |
| `PartExists` | Duplicate part name. |
| `PartMissing` | Unknown part name. |
| `AnchorName` | Anchor name is blank, contains control characters or exceeds 64 characters. |
| `AnchorMissing` | The part has no anchor with this name. |
| `AnchorCoord` | Local anchor coordinate outside `-256..=256`. |
| `PartAngle` | Angle is nonfinite or has a magnitude above 1,000,000. |
| `PartOffCanvas` | The transform would move the origin or an occupied cell off the canvas. |
| `CurveValue` | Invalid channel value. Rotation must be finite with magnitude at most 1,000,000. Translation uses integer `i32` cells and flips use 0 or 1. |
| `CurveEase` | Unsupported easing for this channel. Flips require `hold`. |
| `CurveMissing` | The part has no curve on that channel. |
| `CurveKeyMissing` | No key at the specified frame. |
| `CurveFrame` | Nonfinite sample position. |
| `ConstraintName` | Constraint name is blank, contains control characters or exceeds 64 characters. |
| `ConstraintExists` | Duplicate constraint name. |
| `ConstraintMissing` | Unknown constraint name. |
| `ConstraintFields` | Fields disagree with `follow`, `attach`, `pin`, `ik` or `plant`. Covers invalid chain lengths, overlapping contacts or conflicting targets. |
| `ConstraintCycle` | Self-reference or constraint cycle. |
| `ConstraintConflict` | Two constraints drive the same part. An IK constraint drives every link in its chain. |
| `ConstraintLayer` | Two moving parts share a layer. Give each its own layer. |
| `PolishLayer` | Baking would rebuild a `polish` layer controlled by a curve or constraint. Move the driven part to a `default` layer. |
| `IkUnreachable` | Short-chain IK cannot place the tip exactly on the target grid cell. |
| `BakeRange` | Invalid bake range. `from` exceeds `to` or the frame count, or `to` is at least 4096. |
| `BakeFps` | Bake `fps` must be positive and finite. |
| `NothingToUndo` | Undo stack is empty. |
| `NothingToRedo` | Redo stack is empty. |
| `OpenGesture` | A gesture is already open, or redo was requested during a modified gesture. |
| `MaskMissing` | No stored selection for intersect, subtract or `selection: true` on preview or diff. |
| `SelectionProtected` | A move or part transformation would take a protected cell outside the selection. |
| `ExportScale` | Scale is zero or the scaled image exceeds PNG dimensions. |
| `SheetColumns` | Sheet has zero columns or is too wide to encode as PNG. |
| `ProjectFile` | Invalid version 1 project, including malformed selection masks, tags, parts, anchors, curves or constraints. |
| `AsepriteFile` | Truncated or malformed Aseprite data, invalid magic, unreadable layers/cels or a compressed cel that cannot be decoded. |
| `AsepriteSize` | Canvas side outside `1..=256`. |
| `AsepriteColorMode` | Color depth is not 8-bit indexed. |
| `AsepritePalette` | No usable indexed palette. |
| `AsepriteTilemap` | A tilemap layer or cel prevents import of the file. |
| `AsepritePixel` | Pixel index exceeds palette length. |
| `AsepriteLayers` | No image layer remains after excluding groups and reference layers. |
| `RotateSquare` | A rectangle for 90° or 270° rotation must be square. |
| `TileAlign` | Tile side must be 16 or 32. The canvas must equal one tile and map cell size must match the tileset. |
| `TilesetMissing` | The project has no tileset dimensions. Pixel editing still works. |
| `TileId` | Tile ID outside `0..tileCount`. |
| `TileCount` | Tile count differs from the number of tileset frames or lies outside `1..=65536`. |
| `TileBounds` | Map cell or rectangle is outside map bounds. |
| `MapSize` | Map width or height lies outside `1..=256`. |
| `TileLayer` | Invalid, duplicate or missing map layer name. |
| `TileImage` | Tileset image name must be a relative PNG filename. |
| `TileMapFile` | Expected version 1 `pixelforge-tilemap`. |
| `AutotileFile` | Expected version 1 `pixelforge-autotile`. |
| `AutotileNeighbors` | Only four-neighbor rules are supported. |
| `AutotileRule` | Missing or duplicated mask among the 16 combinations, or bit order differs from `n,e,s,w`. |
| `DirectionMissing` | The direction command requires a saved layout. |
| `DirectionFacing` | Facing is absent from the layout. Four-way layouts exclude diagonals. |
| `DirectionFrames` | Frame count differs from `count * columns`. |
| `DirectionSegment` | Two names represent the same action for one facing. |
| `DirectionFill` | The direction cannot be filled. Diagonals are skipped. Mirror pairs are left/right, and source ranges must lie within their row. |

`InvalidRequest` messages include `request is not JSON`, `request must be a JSON object`, `missing tool`, `tool must be a string` and unknown-field errors.

`UnknownTool` message: `unknown tool "<name>"`.

`UnsupportedVersion` message: `tool api version <n> is not supported`.

## Tools { #tools }

`result` is `{}` when a tool does not list fields. Read-only tools (`inspect`, `scene`, `saveProject`, both exports, `previewFrame`, `diffFrames`, `listLayers`, `listParts`, `listCurves`, `sampleCurve`, `samplePart`, and `listConstraints`) do not push undo entries. A paint, rectangle fill, rectangle erase, rectangle copy or move, horizontal flip, vertical flip, rectangle rotation, canvas resize, recolor, delay, lock, index policy, layer rename, layer role, layer visibility, layer opacity, tag edit, part edit, anchor edit, part translate / rotate / flip, curve-key edit, or `solveConstraints` that does not change the current value, a `bake` that changes neither pixels nor delays, a `combineMask` or `selectConnected` that stores the same mask, and an import, remap, or PNG crush that does not change the document, are not recorded either. `importAseprite`, like `loadProject`, replaces the document and starts with an empty undo stack.

`paint`, `erase`, `fillRect`, `strokeRect`, `line`, `ellipse`, `polygon`, `fill`, `eraseRect`, `copyRect`, `moveRect`, `flipHorizontal`, `flipVertical`, `rotate`, `resizeCanvas`, `translatePart`, `rotatePart`, and `flipPart` take an optional boolean `reportDirty`. Omit it, or send `false`, and those results stay as they were. See Dirty region below. This is not a command log.

The undo stack keeps `MAX_UNDO` (100) entries. One `fillRect`, `strokeRect`, `line`, `ellipse`, `polygon`, `fill`, `eraseRect`, `copyRect`, `moveRect`, `flipHorizontal`, `flipVertical`, `rotate`, `resizeCanvas`, `translatePart`, `rotatePart`, `flipPart`, `solveConstraints`, `bake`, or `fillDirections` is already one undo step. Wrap several paints or several rectangles in `beginGesture` … `endGesture` when they should undo together.

### `inspect` { #inspect }

`inspect()`

Params: none.

`result`:

| Field | Meaning |
| --- | --- |
| `width`, `height` | Canvas dimensions in pixels |
| `palette` | Array of `{ "r", "g", "b", "a" }`. Index 0 is transparent. |
| `paletteHash` | Palette FNV-1a 64-bit hash in 16 lowercase hex digits. See `scene`. |
| `paletteLocked` | A lock prevents recoloring, appending, importing and remapping. |
| `indexPolicy` | `reject` (default) or `clamp`. See `setIndexPolicy`. |
| `frameCount` | Number of frames, at least one |
| `activeFrame` | Zero-based active frame |
| `delayMs` | Active frame delay in milliseconds |
| `frameDelays` | All frame delays in timeline order |
| `pixels` | Active-layer indices on the active frame, row-major, length `width * height`. Zero is empty. This is the buffer targeted by drawing. |
| `composite` | Active-frame indices composited in layer order |
| `layerCount` | Number of layers, at least one, shared across every frame |
| `activeLayer` | Zero-based active layer, bottom first |
| `selection` | A rectangle `{ "x", "y", "width", "height" }`, a mask `{ "rects": [...] }` or null |
| `tags` | `{ "index", "name", "from", "to", "loop" }` entries in insertion order, empty for no tags |
| `parts` | `{ "index", "name", "x", "y", "layer", "anchors" }` entries in insertion order, empty for no parts |
| `curves` | `{ "index", "part", "channel", "keys" }` entries in insertion order, empty for no curves |
| `constraints` | Constraint entries in insertion order, with index, name, kind, part, anchors, target, position, chain, bend and contacts as applicable |
| `layers` | `{ "index", "name", "role", "part", "anchor", "visible", "opacity" }` from bottom to top. Opacity ranges from 0 to 255. |
| `canUndo`, `canRedo` | Whether undo or redo is available |
| `gestureOpen` | Whether a gesture is open |
| `directionLayout` | Saved direction layout or null |
| `tileset` | Declared tile dimensions or null |
| `engine` | Current PNG conversion engine ID |
| `transform` | Most recent conversion snapshot or null |

`inspect` includes active-layer `pixels` and composited indices. Use `scene` for a lighter summary of dimensions, palette, layers and timing.

### `scene` { #scene }

`scene()`

Params: none. Read-only. Does not push undo. Does not return a pixel buffer.

`result`:

| Field | Meaning |
| --- | --- |
| `width`, `height` | Canvas dimensions in cells |
| `activeFrame`, `activeLayer` | Zero-based active frame and layer |
| `selection` | Same as `inspect`. Null when there is no selection. |
| `layerCount` | Number of layers, at least one |
| `layers` | `{ "index", "name", "role", "part", "anchor", "visible", "opacity" }` bottom to top. Opacity 255 is fully opaque. |
| `paletteLocked` | Whether the palette is locked |
| `indexPolicy` | `reject` or `clamp`. Omitted project fields default to `reject`. |
| `paletteCount` | Number of palette entries, including index 0 |
| `paletteHash` | See below |
| `palette` | Same RGBA entries as `inspect` |
| `frameCount` | Number of frames |
| `delayMs` | Current frame delay in milliseconds |
| `frames` | `{ "index", "delayMs" }` in timeline order |
| `tags` | Tags as in `inspect`, or an empty array |
| `parts` | Parts as in `inspect`, or an empty array |
| `curves` | Curves as in `inspect`, or an empty array |
| `constraints` | Constraints as in `inspect`, or an empty array |
| `directionLayout` | Saved direction layout or null |
| `tileset` | Declared tile dimensions or null |
| `engine` | Current PNG conversion engine ID |
| `transform` | Most recent conversion snapshot or null |

CLI: `pixelforge scene --project hero.json`.

`visible` is a boolean and defaults to true. `opacity` is an integer `0..=255` and defaults to 255. It is not `null`. `false` or `0` skips that layer in the composite. `1..=254` still covers in the index composite and blends in RGBA export. `flattenLayers` snaps that blend onto the nearest palette index. One layer has one flag and one opacity for every frame. An older file without the fields loads visible and fully opaque. `paletteHash` is FNV-1a 64-bit: offset basis `14695981039346656037`, prime `1099511628211`, each entry's `r`, `g`, `b`, `a` in index order, written as 16 lowercase hex digits. The same colors in the same order hash the same. It identifies the palette. It is not a secret.

### Dirty regions { #dirty-region }

`paint`, `erase`, `fillRect`, `strokeRect`, `line`, `ellipse`, `polygon`, `fill`, `eraseRect`, `copyRect`, `moveRect`, `flipHorizontal`, `flipVertical`, `rotate`, `resizeCanvas`, `translatePart`, `rotatePart` and `flipPart` accept the optional boolean `reportDirty`. Other tools reject it as `InvalidParams`.

When true, the result also includes:

| Field | Meaning |
| --- | --- |
| `changed` | Number of cells whose index changed |
| `dirty` | Bounding box `{ "x", "y", "width", "height" }` or null if unchanged |
| `regions` | One entry per modified frame and layer, with `frame`, `layer`, `changed`, `x`, `y`, `width` and `height` |
| `clipped` | Only `resizeCanvas` reports how many previously occupied cells fall outside the resized canvas. |

The box covers cells whose stored index changed, not the whole rectangle the tool was asked to touch. A new cell that stays 0 is not a change. Across frames, `dirty` is the union of `regions`. On `resizeCanvas`, a cell that still has a coordinate and holds a different index counts as `changed`, not `clipped`. The CLI flag `--dirty` sends `reportDirty: true`. Without it, these fields are absent.

### Selection { #selection }

A selection is a stored grid mask. Use `setSelection` for a rectangle, `selectConnected` for a connected color region, `combineMask` for union/intersection/subtraction and `clearSelection` to remove it. An empty stored mask protects every cell. Removing the mask restores unrestricted editing.

Selections persist in the project. A single rectangle uses `{ "x", "y", "width", "height" }`. Multiple or zero rectangles use `{ "rects": [...] }`. Clearing a selection omits the field. Older files can omit it.

All frames share one mask in canvas coordinates. It is independent of the active layer. To edit a helmet, select its bounds and target the gear layer with `layer` or `--layer`.

With an active selection:

| Tool | Outside the selection |
| --- | --- |
| `paint`, `erase`, `fillRect`, `strokeRect`, `line`, `ellipse`, `polygon`, `fill`, `eraseRect` | Skip protected cells. Geometry must still lie fully inside the canvas or the operation returns `OutOfBounds`. Flood fill treats the selection boundary as a wall. |
| `copyRect` | Skip only protected destinations. The source can be outside the selection and stays unchanged. |
| `flipHorizontal`, `flipVertical`, `rotate` | Both destination and sampled source must lie inside the selection. |
| `replaceIndex` | Replace only selected cells. |
| `moveRect` | Both rectangles must lie fully inside the selection or the operation returns `SelectionProtected` without writing. |
| `translatePart`, `rotatePart`, `flipPart` | Every occupied cell of an attached layer and its destination must be selected or the transform returns `SelectionProtected`. Metadata-only transforms ignore the selection. |

CLI: `pixelforge selection set`, `pixelforge selection wand`, `pixelforge selection clear`, `pixelforge mask union|intersect|subtract`, `pixelforge replace-index`.

The selection is document state. `setSelection` stores one rectangle. `selectConnected` stores the seed cell's indexed connected component as that same mask. `combineMask` turns rectangles into a mask with union, intersect, or subtract. `clearSelection` removes it. With no selection, every cell is writable. An empty mask is `{ "rects": [] }` and protects every cell, so the next fill does not paint the canvas.

The project file stores one rectangle as `{ "x", "y", "width", "height" }` and several rectangles, or an empty mask, as `{ "rects": [...] }`. It omits the field when the selection is clear. A file that has no field loads with no mask. Adjacent rectangles are merged, so the partition is stable. The undo stack is still not stored: `undo` in the same process restores `setSelection`, `selectConnected`, and `combineMask`, and reopening the file does not. This matches `activeLayer`: the next CLI process needs it.

The mask is in canvas coordinates and applies to every frame. It does not belong to a layer and it does not change `activeLayer`. To edit a helmet, set the rectangle on the helmet and pass `layer` for the gear layer. Only that layer is written, and only inside the mask.

`paint`, `erase`, `fillRect`, `strokeRect`, `line`, `ellipse`, `polygon`, `fill`, and `eraseRect` skip cells outside the selection. Endpoints, vertices, and the requested rectangle must still lie on the canvas. `fill` treats cells outside the mask as barriers and does not flood through them. `copyRect` skips destination cells outside it and does not change the source. `flipHorizontal`, `flipVertical`, and `rotate` write a cell only when that cell and the cell it reads are both inside. `replaceIndex` replaces matching cells inside the mask, or on the whole targeted layer of the active frame when no selection is set. `toIndex` 0 clears those cells. `moveRect` returns `SelectionProtected` and writes nothing unless both rectangles lie inside the selection. `translatePart`, `rotatePart`, and `flipPart` do the same when any ink cell on the linked layer, or the cell it would land on, lies outside the selection.

`remapPalette`, `applyPng`, palette edits, frame structure, layer structure, and `flattenLayers` ignore the selection. `resizeCanvas` translates every rectangle by the same offset and clears the whole mask, in that same undo step, when any rectangle would leave the new canvas.

### `paint` { #paint }

`paint(x, y, colorIndex, layer?, reportDirty?)`

Writes an index, not RGB. A locked palette does not block paint. Optional `layer` chooses the layer and does not change `activeLayer`. Other layers stay put. The default `indexPolicy` is `reject`: an unknown `colorIndex` is `PaletteIndex` and the document stays put. `clamp` writes an index past the palette as the last opaque entry. Index 0 is refused in both modes. `indexPolicy`, `requestedIndex`, and `colorIndex` appear on the result only when the index was clamped. `importPalette` `replace` still clears orphaned indices to 0 and does not read `indexPolicy`.

### `fillRect` { #fillrect }

`fillRect(x, y, width, height, colorIndex, layer?, reportDirty?)`

Fills an axis-aligned rectangle on the active frame with one palette index. `x` and `y` are the inclusive top-left cell. Writes an index, not RGB. A locked palette does not block the fill. Cells that already hold the index are not recorded. The rectangle is one undo step; an open gesture absorbs those cells. If any cell would fall outside the canvas, the call fails and writes nothing. A `width` or `height` of 0, a missing field, or a number outside `1..=65535` is `InvalidParams` (for example `fillRect: width must be an integer from 1 to 65535`).

### `strokeRect` { #strokerect }

`strokeRect(x, y, width, height, colorIndex, layer?, reportDirty?)`

Draws the border of an axis-aligned rectangle. The interior stays. A width or height of 1 draws that whole line. Bounds, a locked palette, index 0, the selection, and the single undo step match `fillRect`.

CLI: `pixelforge stroke-rect`.

### `line` { #line }

`line(x0, y0, x1, y1, colorIndex, pixelPerfect?, layer?, reportDirty?)`

Draws a 1px line on the active layer. The default is a continuous Bresenham line: consecutive cells share an edge or a corner, both endpoints are included, and there is no gap. `pixelPerfect: true` plots one cell per step along the longer axis and stays 8-connected. There is no anti-aliasing. A locked palette does not block the write. Index 0 is refused. One endpoint outside the canvas writes nothing. Cells outside the selection are skipped. The cells that change are one undo step.

### `ellipse` { #ellipse }

`ellipse(x, y, width, height, colorIndex, fill?, layer?, reportDirty?)`

Inscribes an ellipse in the rectangle. Omit `fill` to draw the outline. `fill: true` paints the interior and includes every outline cell. A width or height of 1 is that line for both. The midpoint ellipse is integer and has no anti-aliasing. The same input yields the same cells. Lock, index 0, bounds, selection, and the undo step match `fillRect`.

### `polygon` { #polygon }

`polygon(points, colorIndex, fill?, layer?, reportDirty?)`

Closes a polygon. Edges use the same continuous line as `line`, including the edge from the last vertex back to the first. `fill: true` adds the even-odd interior, so the border matches `line`. Every vertex must lie on the canvas. Fewer than 3 vertices is `InvalidParams` and does not call the kernel. A self-intersection uses the even-odd rule. There is no anti-aliasing and no curve. Lock, index 0, selection, and the undo step match `fillRect`.

### `fill` { #fill }

`fill(x, y, colorIndex, connectivity?, layer?, reportDirty?)`

Replaces the connected region of the seed cell's current index, including index 0, with `colorIndex`. `connectivity` 4 walks orthogonal neighbors. 8 also crosses corners. Cells outside the selection are barriers: they are not written and the fill does not travel through them. A seed outside the canvas is `OutOfBounds` and writes nothing. A seed outside the selection succeeds and changes nothing. The same index is not recorded. A locked palette does not block the write. Index 0 as the new color is `PaletteIndex`. The cells that change are one undo step.

### `erase` { #erase }

`erase(x, y, layer?, reportDirty?, mode?)`

Parameters: `x`, `y`, optional `layer` and `mode` (`transparent` by default or `pass`).

Both modes clear a default-layer cell to index 0. On polish layers, `transparent` explicitly erases through the lower layers. `pass` removes the override and reveals them again. Unchanged cells add no undo entry.

Errors: `OutOfBounds`, `LayerIndex`, or `InvalidParams` for an unknown mode.

### `eraseRect` { #eraserect }

`eraseRect(x, y, width, height, layer?, reportDirty?, mode?)`

Clears an axis-aligned rectangle on the active frame to index 0. `mode` matches `erase`: on a polish layer the default is an explicit erase, and `pass` returns the cell to unmodified. Bounds match `fillRect`: if any cell would fall outside the canvas, the call fails and writes nothing. Cells that are already in the target state are not recorded. The rectangle is one undo step; an open gesture absorbs those cells. A locked palette does not block the erase. Prefer this over erasing cell by cell when reshaping a silhouette.

### `copyRect` { #copyrect }

`copyRect(srcX, srcY, width, height, dstX, dstY, fromFrame?, toFrame?, layer?, reportDirty?)`

Copies palette indices, including 0, from one rectangle to another. Optional `fromFrame` is the source frame and optional `toFrame` is the destination; omit either one and it is the active frame. The call does not change `activeFrame`. The source stays. If the source or the destination would fall outside the canvas, the call fails and writes nothing. Overlap on the same frame reads the source before writing the destination. Changed cells are one undo step; an open gesture absorbs them. A copy that changes nothing is not recorded. A locked palette does not block the copy.

### `moveRect` { #moverect }

`moveRect(srcX, srcY, width, height, dstX, dstY, fromFrame?, toFrame?, layer?, reportDirty?)`

Moves palette indices, including 0, to the destination and clears source cells that are not part of the destination. `fromFrame` and `toFrame` follow `copyRect`. Across frames the whole source rectangle is cleared, because the destination is on the other frame. Both rectangles must lie inside the canvas. Overlap on the same frame uses the source from before the move, so a source cell that lands inside the destination keeps the moved index. The move is one undo step. A move onto the same rectangle changes nothing and is not recorded. The active frame stays put.

### `setSelection` { #setselection }

`setSelection(x, y, width, height)`

Stores the rectangle. Does not change pixels or `activeLayer`. The same rectangle is not recorded. `result.selection` is that rectangle. See Selection above.

### `selectConnected` { #selectconnected }

`selectConnected(x, y, tolerance?, connectivity?, layer?, frame?)`

Stores the seed cell's indexed connected component as `selection`. Does not change pixels, `activeLayer`, or `activeFrame`. It reads the stored indices of that layer and ignores `visible` and `opacity`. An existing mask is replaced; it is not a barrier. `fill` is the tool that treats cells outside the mask as walls. Connectivity matches `fill`: `4` is orthogonal, `8` also crosses corners.

`tolerance` 0 keeps cells whose palette index equals the seed. A different index does not match, even when the RGBA is the same. `tolerance` 1..=255 keeps a cell when each of `r`, `g`, `b`, and `a` differs from the seed color by at most that value. The seed's distance to itself is 0, so the seed is included. `255` matches every color. The comparison reads palette entries and does not write RGB back into cells.

The component is stored as the same disjoint rectangles as `combineMask`. A solid rectangle stays one rectangle. An irregular contour is several. The result is never an empty mask, because the seed matches itself. The same mask is not recorded. A change is one undo step. Later `paint`, `erase`, `fill`, `combineMask`, `previewFrame`, and `diffFrames` read that mask.

`result` is `selection` (the same shape as Selection above), `cells`, `index` (the seed's stored index), `tolerance`, `connectivity`, `frame`, and `layer`.

CLI: `pixelforge selection wand --x --y`. `--tolerance N`, `--connectivity 4|8`, `--layer`, and `--frame`. The last two do not select that layer or frame.

### `clearSelection` { #clearselection }

`clearSelection()`

Clears the mask. `result.selection` is `null`. Already clear is not recorded. An empty mask is not "no selection": call this to make the canvas writable again.

### `combineMask` { #combinemask }

`combineMask(op, x?, y?, width?, height?, baseX?, baseY?, baseWidth?, baseHeight?, mask?, base?)`

Use `op` (union, intersect or subtract). Supply one right-hand mask: either all four rectangle bounds or a nonempty rectangle array in `mask`. The left side defaults to the saved selection. Override it with `base` or all four `baseX`/`baseY`/`baseWidth`/`baseHeight` values. Combining both forms returns `InvalidParams`. Returns `op`, the resulting `selection` and its cell count.

Does not change pixels or `activeLayer`. The result becomes `selection`. One undo step. The same mask is not recorded. With no selection, `union` stores the operand and `intersect` / `subtract` return `MaskMissing`. An empty result still protects every cell. `reportDirty` is `InvalidParams`.

### `replaceIndex` { #replaceindex }

`replaceIndex(fromIndex, toIndex, layer?)`

Replaces `fromIndex` with `toIndex` on the active frame. With a selection, only cells covered by the stored mask are replaced. `toIndex` 0 clears those cells. Any other `toIndex` resolves like `paint`. `fromIndex` must already be in the palette, including 0. The active layer and other frames stay put. `toIndex` in the result is the stored index. `indexPolicy` and `requestedIndex` appear only when the index was clamped. No change is not recorded.

### `setPaletteColor` { #setpalettecolor }

`setPaletteColor(index, r, g, b)`

Parameters: `index`, `r`, `g` and `b` in `0..=255`. Index 0 is immutable. Every frame using this index displays the updated color.

Errors: `PaletteLocked`, `TransparentSlot`, `PaletteIndex`.

### `addPaletteColor` { #addpalettecolor }

`addPaletteColor(r, g, b)`

Parameters: `r`, `g`, `b`. Appends one opaque color.

Returns `{ "index": <new index> }`.

Errors: `PaletteLocked`, `PaletteTooLarge`.

### `importPalette` { #importpalette }

`importPalette(text, mode)`

Parameters: `text` (file contents) and `mode` (`replace` or `merge`).

Accepts the formats documented in [development setup](dev-setup.md). Replace installs the new colors and clears missing indices. Merge appends missing opaque colors. Index 0 stays transparent. Each import is a single undo step.

Errors: `PaletteLocked`, `PaletteImportMode`, `PaletteFile`, `EmptyPalette`, `PaletteTooLarge`.

### `setPaletteLocked` { #setpalettelocked }

`setPaletteLocked(locked)`

The boolean `locked` persists in the project. A lock blocks palette edits and `remapPalette`. Painting with existing indices, `applyPng` and `setIndexPolicy` still work. Setting the same value creates no undo entry.

### `setIndexPolicy` { #setindexpolicy }

`setIndexPolicy(mode)`

`result`: `{ "mode": "reject" | "clamp" }`.

Sets the document index policy. `reject` is the default and is omitted from the project file. `clamp` is stored. The same value is not recorded. A locked palette does not freeze this flag.

CLI: `pixelforge palette enforce --mode reject|clamp`.

### `remapPalette` { #remappalette }

`remapPalette(text?, maxColors?)`

Supply exactly one of `text` (GPL or hex palette) and `maxColors` (1–255 opaque colors). Text remapping uses squared RGB distance; ties use the lower target index. Reducing a palette removes unused entries before merging the nearest colors. A locked palette rejects the operation. The entire remap is one undo step.

`result`:

| Field | Meaning |
| --- | --- |
| `pixelsChanged` | Count of cells whose stored index changed, excluding color-only changes |
| `paletteLength` | Final palette length, including index 0 |
| `misses` | Source colors without an exact RGB match |
| `merges` | Merged entries, including exact colors whose indices changed during compaction |
| `dropped` | Unused colors removed to meet `maxColors`. Empty for remapping with `text`. |

One of `text` or `maxColors`. `text` installs that palette and nearest-maps every index. `maxColors` caps opaque entries. Squared RGB distance, lower index wins a tie, survivor RGB is not averaged. One undo step. Refused while locked. No Lospec browser: the caller supplies the palette text.

### `applyPng` { #applypng }

`applyPng(png, frame?, layer?)`

Supply the PNG in standard base64. The image dimensions must match the canvas. Alpha below 128 becomes transparent; other cells use the nearest opaque palette entry by squared RGB distance. Only the first image frame is read. The result includes target frame, layer, dimensions, changed pixel count and color-mapping diagnostics. Errors include `PngFile`, `PngSize` and `NoPaintColor`.

Crushes one PNG onto the current palette. The image must match the canvas; there is no scale and no sheet slice. Alpha below 128 becomes index 0. A locked palette still allows the write. `merges` and `misses` list the same near-colors. This is not a Lospec browser.

### `listEngines` { #listengines }

`listEngines()`

No parameters. `default` is `center-sample`. `engine` is the id this project will use. Reserved `pixelforge-native` is not available and not licensed. Nothing is downloaded.

### `setTransformEngine` { #settransformengine }

`setTransformEngine(id)`

`result`: `{ "engine": "<id>" }`.

Stores the default engine. The same id is not recorded. A reserved id writes nothing.

CLI: `pixelforge engine set <id> --project hero.json`.

### `convertPng` { #convertpng }

`convertPng(png, width?, height?, engine?, features?, frame?, layer?, palette?, params?)`

`params`:

| Field | Meaning |
| --- | --- |
| `png` | Standard base64. Whitespace is allowed. Maximum source dimension is 4096 px per side. |
| `width`, `height` | Target dimensions in `1..=256`. Supply both, or omit both to use the canvas size. |
| `engine` | Engine ID. Defaults to the project’s configured engine. |
| `palette` | Use either `strategy` (source, limit, project) or `mode` (project, source, provided), with optional `text` or `maxColors` as applicable. Omitting the object selects `mode: project`. |
| `features` | Up to 32 strings, stored in the snapshot and reserved for future use |
| `params` | Engine-specific object. Unknown fields return `EngineParams`. |
| `frame`, `layer` | Target frame and layer. Defaults to the active pair and keeps the current selection. |

Supply one of the three registered `palette.strategy` IDs. An unknown strategy returns `PaletteStrategyUnknown` with no changes. Omit `mode` and `text` when using a strategy.

| id | Behavior |
| --- | --- |
| `source` | Use the source palette. For indexed PNGs keep PLTE entries with alpha ≥128 in file order, including unused entries. For other images collect unique opaque colors in row-major order. Over 255 opaque colors returns `PaletteTooLarge`. Omit `maxColors`. Ink on other layers or frames prevents replacement. |
| `limit` | Limit the number of opaque colors to `maxColors` (default 16, range 1–255). Prefer the indexed palette when eligible, else use sampled colors. Reduce using the same rules as `remapPalette`, retaining real colors. If necessary, first keep the 255 most frequent sampled colors. |
| `project` | Map onto an explicitly selected project palette without modifying it. A locked palette is valid. The built-in default returns `PaletteDefault`. Omit `maxColors`. |

Mapping uses squared RGB distance. Pixels below alpha 128 become index 0 and contribute no RGB error. Fully transparent input keeps the existing palette and sets `paletteReport.keptExisting`.

`result.paletteReport` accompanies explicit strategies. It reports strategy, version, origin, palette counts, exact/inexact pixels, squared distance totals, per-color merges and unused dropped colors. `frames` reports index changes per frame. `otherCellsChanged` counts changes outside the conversion layer. A successful `source` or `limit` has zero such changes. Identical inputs and options produce identical output.

The original `palette.mode` options do not emit `paletteReport`.

`project` maps into the current palette and accepts neither `text` nor `maxColors`. `source` derives up to `maxColors` opaque sampled colors (default 16). `provided` reads GPL or hexadecimal colors from `text`. A lock prevents a palette-changing conversion.

Sample RGBA at the target size. Alpha below 128 becomes transparent. Opaque cells use existing source colors and map to the nearest project index by squared RGB distance. Ties choose the smaller index. The stored selection does not clip this operation.

Five engines are available at version 1:

| id | Behavior |
| --- | --- |
| `center-sample` | Use the source pixel at each target cell’s center. Enlargement uses nearest-neighbor sampling. No private parameters. |
| `dominant` | Use the most frequent opaque RGB in a block. Ties favor the smaller RGB value. If at most half the pixels are opaque, the cell is transparent. No private parameters. |
| `kmeans` | Cluster opaque source colors in eight deterministic rounds and snap centroids to real source colors. Vote by cluster within each block. `k` defaults to 16 (1–255). |
| `contrast-outline` | Find outlines from luminance contrast (`contrast`, default 48) and transparency boundaries. Thicken only over opaque cells (`expand`, default 1, range 0–4). Prefer outline pixels nearest the lower median luminance. |
| `grid-snap` | Detect repeated cell spacing from edge votes with `slack` (default 1, range 0–4) and `maxCell` (default 32, range 2–64). Take modal cell colors and sample to the target size. Without edges, center sampling is used and `report.detected` is false. |

One request shape for every engine. Alpha below 128 is index 0. Opaque samples stay source colors, then `applyPng` maps them. `strategy` is `source` (keep the image palette, recorded as mode `keep`), `limit` (cap, recorded as mode `limit`), or `project` (a palette this project already chose). An unknown id errors. `source` and `limit` return `PaletteInUse` and write nothing when another frame or layer still has ink and the palette would change. `project` returns `PaletteDefault` on the built-in default palette, including a new `--output` file. `paletteReport.frames` lists every frame whose indices changed, and `otherCellsChanged` is outside the converted layer. Legacy `mode: source` still only caps sampled colors, records mode `source` with no strategy, and does not emit that report. `provided` uses caller text. `pixelforge-native` errors. Features are stored and ignored. One undo step. This is not a quality score.

```json
{ "tool": "listEngines" }
{ "tool": "setTransformEngine", "params": { "id": "center-sample" } }
{ "tool": "convertPng", "params": { "png": "<base64>", "width": 64, "height": 64, "engine": "kmeans", "palette": { "strategy": "limit", "maxColors": 16 }, "params": { "k": 16 } } }
{ "tool": "convertPng", "params": { "png": "<base64>", "palette": { "strategy": "source" } } }
{ "tool": "convertPng", "params": { "png": "<base64>", "palette": { "strategy": "project" } } }
```

CLI:

```bash
pixelforge engine list
pixelforge engine set kmeans --project hero.json
pixelforge convert --file ref.png --width 64 --height 64 --engine center-sample --output hero.json
pixelforge convert --file ref.png --output hero.json --width 64 --height 64 --palette-strategy source
pixelforge convert --file ref.png --project hero.json --palette-strategy limit --max-colors 8
pixelforge convert --file ref.png --project hero.json --palette-strategy project --checkpoint hero.ckpt.json
```

`--output` creates a project, requires width and height and defaults to `mode: source`. `--project` preserves omitted dimensions and defaults to `mode: project`. Explicit project strategy requires a chosen palette. Source/limit reject changes to ink elsewhere. Use either `--palette-strategy` or legacy palette options. `--max-colors` applies to source mode or limit strategy. `--params` accepts JSON and `--feature` is repeatable.

Errors: `EngineUnknown`, `EngineUnavailable`, `EngineParams`, `PaletteStrategyUnknown`, `PaletteInUse`, `PaletteDefault`, `ConvertFeature`, `ConvertSource`, `PngFile`, `NoPaintColor`, `PaletteLocked`, `PaletteFile`, `PaletteTooLarge`, `TileAlign`, `InvalidSize`, `FrameIndex`, `LayerIndex`. The tool returns `InvalidParams` if only one dimension is supplied, if `strategy` and `mode` are combined, if a strategy includes `text`, if the `source` or `project` strategy includes `maxColors`, or if `png` is invalid Base64.

### `setActiveFrame` { #setactiveframe }

`setActiveFrame(index)`

Set `index` to an existing frame index in `0..=4294967295`. Selecting the current frame adds no undo entry.

Errors: `FrameIndex`.

### `addFrame` { #addframe }

`addFrame()`

No parameters. Append a blank frame with a delay of 100 ms. The active frame stays selected.

Returns `{ "index": <new frame index> }`.

### `deleteFrame` { #deleteframe }

`deleteFrame(index)`

Errors: `FrameIndex`, `LastFrame`. With one frame left, every delete returns `LastFrame`, including an index past the list.

### `duplicateFrame` { #duplicateframe }

`duplicateFrame(index)`

Does not select the copy. `index` in the result is the new frame. Call `setActiveFrame` (CLI: `frame select`) before painting it. If the active frame sits at or after the insertion point, its index increases by one and it still shows the same pixels. An earlier active index stays put. The web editor's duplicate button then calls `setActiveFrame` on the new index inside the same gesture. `frame duplicate` and this tool do not.

### `setFrameDelay` { #setframedelay }

`setFrameDelay(index, delayMs)`

Parameters: `index` and `delayMs`. The delay must fit in a `u32` and lie in `1..=65535`.

Errors: `FrameIndex`, `FrameDelay`.

### Animation tags { #animation-tags }

`loop`:

| Value | Meaning |
| --- | --- |
| `forward` | Play from `from` through `to`, inclusive, and repeat (default) |
| `reverse` | Play backwards from `to` through `from` and repeat |
| `pingpong` | Play forwards then backwards and repeat, without duplicating either endpoint |
| `once` | Play the inclusive range once |

A tag is a name, an inclusive frame range, and a loop mode. The project file stores `tags` and omits the field when the list is empty. A file without the field loads with no tags. The undo stack is still not stored.

This is not an Aseprite cel link. Each frame keeps its own pixels. `duplicateFrame` still copies indices. `tick` still loops the whole timeline. `loop` is stored and written into the atlas. It does not start a second clock. There is no repeat count, no ping-pong reverse, and no tag color.

`forward` plays `from` through `to` and repeats (the default). `reverse` plays `to` through `from` and repeats. `pingpong` plays to the end and back without holding either end for an extra frame. `once` plays `from` through `to` a single time.

Ranges may overlap. Names are unique after trimming, 1 to 64 characters, with no control character.

Inserting a frame at `i` shifts a tag that starts at or after `i`, and grows a tag that starts earlier and already covers `i`. Deleting frame `i` shifts later tags left, shrinks tags that covered it, and drops a tag that named only that frame. `addFrame` does not grow existing tags. Duplicating a frame inserts after it, so duplicating the last frame of a tag leaves the copy outside that tag. The frame edit and the tag adjustment are one undo step.

### `addTag` { #addtag }

`addTag(name, from, to, loop?)`

Append a named inclusive frame range. The loop mode defaults to `forward`. Returns the index, trimmed name, bounds and loop. Invalid names, duplicates or out-of-range bounds produce `TagName`, `TagExists` or `TagRange`.

CLI: `pixelforge tag add --name walk_down --from 1 --to 3 --loop forward`.

### `setTag` { #settag }

`setTag(name, from?, to?, loop?, rename?)`

Update the named tag. Supply at least one of `from`, `to`, `loop` and `rename`. Omitted properties keep their current values. Returns the updated tag; an unchanged tag adds no undo step.

CLI: `pixelforge tag set --name walk_down --loop pingpong --rename walk_south`.

### `removeTag` { #removetag }

`removeTag(name)`

Remove the tag while preserving its frames. Returns its former index and name. An absent tag returns `TagMissing`.

CLI: `pixelforge tag remove --name walk_down`.

### Direction layout { #direction-layout }

`directionLayout` stores row organization as `{ "count", "columns" }` in version 1 projects. Projects without the field have no stored layout.

`count` is 4 or 8. Four-way uses the cardinal directions in sprite-sheet row order:

| Row | Compass | Tag suffix |
| --- | --- | --- |
| 0 | S | `down` |
| 1 | W | `left` |
| 2 | E | `right` |
| 3 | N | `up` |
| 4 | SW | `downleft` |
| 5 | SE | `downright` |
| 6 | NW | `upleft` |
| 7 | NE | `upright` |

A direction layout stores how facing rows are arranged. The project file stores `directionLayout` as `{ count, columns }` and omits it when unset. Old files load with no layout. The version stays 1.

`count` is 4 or 8. 4 is the formal N/E/S/W set. Row order matches the sheet, not compass order clockwise from north: S/`down`, W/`left`, E/`right`, N/`up`. 8 appends SW/`downleft`, SE/`downright`, NW/`upleft`, NE/`upright` and does not move the first four rows. `columns` is the frame count of each row. `count * columns` must be from 1 to 4096.

Tags stay `{action}_{facing}`. A facing accepts the screen suffix or a compass alias (`south`, `s`, `sw`, `southwest`). Auto-named tags use the screen suffix, so an existing `idle_down` keeps that name. A diagonal is one token, `downleft` or `sw`, not `down_left`.

The idle/walk split matches the sheet: idle, when present, is only the first frame of the row, and walk is the rest. A row with no idle may be walk across the whole row. A one-frame row has no walk after idle. Any other `{action}_{facing}` that already exists, such as `attack_down`, must exist on every facing in the layout and cover the same columns relative to that row. A miss is a hard error.

`validateDirections` reports `layout`, `frames`, `missingDirection`, or `segment` in `result.errors`. `errorCount` above zero fails the gate. There is no loose mode. The call does not write. A pass means the spans are present. It does not mean the gait is natural or that the picture passes an aesthetic check.

`applyDirectionTags` writes that idle/walk split. A one-frame row gets idle only. A longer row puts idle on the first frame and walk on the rest. An alias tag keeps its name and its range is corrected. Two names for one action on one facing write nothing. It does not paint. `setDirectionLayout` with `apply: true` stores the layout and the tags in one undo step.

`fillDirections` is the call that moves indices. The layout must already be stored, and the frame count must be `count * columns`. The source facing must already have a valid idle/walk segment. Every other `{action}_{facing}` that appears on a cardinal in the layout must also exist on the source, inside that row. A miss writes nothing. Columns line up inside those segments: source column 0 is written to column 0 of each target. `mirror` flips left into right, or right into left. North and south have no mirror target; use `copy`. `copy` repeats the source indices on the other three cardinals and does not flip. On an eight-direction layout the four diagonal rows are not written and `result.diagonals` is `skipped`. On four directions it is `none`. A target frame that already holds a non-zero index or an explicit erase on any layer is skipped unless `overwrite` is true. A frame whose pixels and delay already match is not recorded. A change is one undo step. Tags, parts, the selection, and the active frame stay. The horizontal flip is the same rule as rectangle `flipHorizontal`: `x` receives `width - 1 - x`. Indices move. Colors and parts do not. This is not new art, not a natural gait, does not repair highlights, and is not a generated four-direction drawing.

### `setDirectionLayout` { #setdirectionlayout }

`setDirectionLayout(count, columns, apply?)`

Parameters: `count` (4 or 8), `columns` (at least 1), and optional `apply` (default false).

Returns `count`, `columns`, `wrote` and `apply`. With `apply: true`, returns the validation `report` too. An unchanged layout and tags create no undo entry.

CLI: `pixelforge direction set --count 4 --columns 4`. Add `--apply` to write tags.

Errors: `DirectionLayout`, `DirectionFrames`, `DirectionSegment`.

### `clearDirectionLayout` { #cleardirectionlayout }

`clearDirectionLayout()`

`result`: `{ "directionLayout": null }`.

CLI: `pixelforge direction clear`.

### `applyDirectionTags` { #applydirectiontags }

`applyDirectionTags()`

Use the stored layout to write the idle/walk ranges. Returns the direction validation report with a `wrote` flag. A missing layout returns `DirectionMissing`. Invalid frame counts or ranges return `DirectionFrames` or `DirectionSegment`.

CLI: `pixelforge direction apply`.

### `validateDirections` { #validatedirections }

`validateDirections()`

No parameters. Read-only and does not add an undo entry.

Returns `count`, `columns`, `errorCount`, `errors` and `facings`. Without a layout the dimensions are null and a layout issue is reported. Facings include row, compass and screen names, range, presence and action segments.

CLI: `pixelforge direction validate` exits 1 when `errorCount` is positive. A completed validation still returns `ok: true`.

### `listDirections` { #listdirections }

`listDirections()`

Same inputs and result as `validateDirections`. The read-only `pixelforge direction list` exits 0 when inspection completes. Inspect `errors` for layout issues.

### `exportDirection` { #exportdirection }

`exportDirection(facing, scale?)`

Parameters: `facing` and optional `scale` (default 1). Use cardinal compass or screen names. Eight-way layouts also accept diagonals.

Returns base64 `png`, `facing`, `compass`, `row`, `from`, `to`, `columns`, `rows` (1), `sheetWidth`, `sheetHeight` and `scale`. Exports the row without modifying the project.

CLI: `pixelforge direction export --facing south --output south.png` writes the row to disk.

Errors: `DirectionMissing`, `DirectionFacing`, `DirectionFrames`, and `InvalidParams` or `ExportScale` for invalid scale.

### `fillDirections` { #filldirections }

`fillDirections(from, mode, overwrite?)`

Copies or horizontally mirrors one cardinal row. `mode` is `mirror` or `copy`. `overwrite` defaults to false. A diagonal source is `DirectionFill` even when the layout count is 8. `mirror` accepts only left or right. `copy` accepts one of the four cardinals.

`result.diagonals` is `none` or `skipped`. Each target lists `hflip` and one entry per source segment column. `status` is `written`, `skipped`, or `unchanged`. `wrote` false is not recorded. Every layer is copied, including a polish erase mark. A non-zero index clears the destination erase mark. The source `delayMs` is copied onto a frame that is written. A skipped frame keeps its delay. The selection does not clip the fill. Part origins stay. A later `bake` still rebuilds a managed layer from the rest frame. `flattenLayers` still snaps the composite.

CLI: `pixelforge direction fill --from left --mode mirror`. A placeholder for the other cardinals is `pixelforge direction fill --from down --mode copy`. `--overwrite` replaces ink. Set the tags first with `direction set --apply`, or name the source `{action}_{facing}` segments yourself. Fill does not write tags.

### Parts and anchors { #parts-and-anchors }

`A part is a name, a rest-pose origin on the canvas, an optional layer index, and named anchors. The project file stores `parts` and omits the field when the list is empty. A file without the field loads with no parts. The undo stack is still not stored.

`layer` names the paint layer whose pixels belong to the part, and does not select that layer. Omit it for a rig point such as a shoulder. Several parts may name one layer.

`x` and `y` on the part are the canvas cell that is local `(0, 0)`. Anchor `x` and `y` are pixel offsets from that cell, in `-256..=256`, not canvas coordinates. At rest, the canvas cell of an anchor is the origin plus that offset. A grip can sit on a gun so a later rotation can turn around it. The part record does not store a rotation angle. Parameter curves live in `curves`. Constraints live in `constraints`. `bake` writes pixels and `delayMs`. The file has no `bake` field and stays version 1.

Rectangle `flipHorizontal`, `flipVertical`, and `rotate` move indices only. They do not move parts or anchors. `translatePart`, `rotatePart`, and `flipPart` move the linked layer's indices on one frame and update the origin and anchors in that same undo step. `resizeCanvas` translates each origin by the same offset and drops a part, in that same undo step, when the origin would leave the new canvas. Local anchor offsets stay put.

Inserting a layer shifts attachments at or above that index. Deleting a layer clears the attachment of a part that named it and shifts later attachments down. The part and its anchors stay. Reordering follows the layer. `flattenLayers` retargets every attached part to layer 0 and leaves an unattached part alone. The layer edit and the part adjustment are one undo step.

### `listParts` { #listparts }

`listParts()`

Returns the same `parts` array as `inspect` in insertion order. An empty project returns an empty array.

CLI: `pixelforge part list --project soldier.json`.

### `addPart` { #addpart }

`addPart(name, x, y, layer?)`

Set the part origin at canvas cell `(x, y)`. Omit `layer` for a metadata-only part. Returns the new index, name, position, nullable layer and initially empty anchor list. Duplicate names, off-canvas origins and missing layers are rejected.

CLI: `pixelforge part add --name gun --layer 1 --x 24 --y 12`.

### `setPart` { #setpart }

`setPart(name, x?, y?, layer?, clearLayer?, rename?)`

Identify the part by its existing name. Update at least one field. To detach its layer, send `clearLayer: true` and omit `layer`. Local anchor coordinates stay unchanged. Returns the updated part.

CLI: `pixelforge part set --name gun --x 26 --rename rifle`. `pixelforge part set --name gun --clear-layer`.

### `removePart` { #removepart }

`removePart(name)`

Remove the named part and its anchors while preserving the underlying pixels. Returns its previous index and name. An unknown part returns `PartMissing`.

CLI: `pixelforge part remove --name gun`.

### `setAnchor` { #setanchor }

`setAnchor(part, name, x, y)`

Add or update an anchor on the named part. Coordinates are integer offsets relative to its origin, each in `-256..=256`. Returns the part name/index and anchor index, name and position. Invalid coordinates return `AnchorCoord`.

CLI: `pixelforge part anchor set --part gun --name grip --x 2 --y 2`.

### `removeAnchor` { #removeanchor }

`removeAnchor(part, name)`

Remove the named anchor from the specified part. Returns the part name/index and the former anchor index/name. An unknown anchor returns `AnchorMissing`.

CLI: `pixelforge part anchor remove --part gun --name grip`.

### `translatePart` { #translatepart }

`translatePart(name, dx, dy, frame?, reportDirty?)`

Parameters: `name`, integer `dx`, `dy`, optional `frame` and `reportDirty`. Offsets may be negative.

Move the part origin by `(dx, dy)` cells. Anchors keep local coordinates. Occupied cells on the attached layer move with it and their old positions clear. Without a layer, only metadata changes. Frame, layer and palette selections stay intact.

Zero displacement creates no undo entry. An off-canvas origin or cell returns `PartOffCanvas`. When masked, both occupied source and destination cells must be selected or the call returns `SelectionProtected`.

Returns the updated part, frame and offsets, `alignment: translate`, `sampling: exact`, `dropped: 0` and the old origin as `pivot` with a null anchor.

CLI: `pixelforge part translate --name gun --dx 2 --dy -1`. Use `--frame` to choose a frame and `--dirty` to request the dirty region.

Errors: `PartName`, `PartMissing`, `FrameIndex`, `PartOffCanvas`, `SelectionProtected`, `LayerIndex`. Noninteger offsets return `InvalidParams`.

### `rotatePart` { #rotatepart }

`rotatePart(name, degrees, anchor?, frame?, reportDirty?)`

Rotate clockwise around the center of the named anchor cell or, when omitted, the origin cell. Positive y points down. Degrees must be finite with magnitude no larger than 1,000,000. Normalization to (−180, 180] precedes rounding to 0.01°. If an origin or occupied cell would move off-canvas the transform returns `PartOffCanvas` without writing.

Translate, rotate, and flip a part on the indexed grid. The pivot is the center of the origin cell, or of a named anchor. Quarter turns, flips, and translates are exact integer maps. Any other angle quantizes to 0.01° (ties away from zero) and then each ink cell center snaps to the nearest cell (ties away from the pivot). A collision keeps the later row-major index. `dropped` is the source cells that lost. `conflicts` is the destination cells claimed more than once. Exact maps report 0 for both.

A nearest rotation then seals notches. An empty cell is filled only when it is 4-adjacent to ink the forward map wrote, at least half of a 4×4 sample inside it inverse-maps onto source ink, and the number of new cells does not exceed `dropped`. The color is the majority index, and a tie keeps the smaller index. `sealed` is how many cells were filled. A cell outside the selection is left empty and does not fail the call. Exact maps do not seal. `breaks` counts bare diagonal joins in the result: two ink cells at Chebyshev distance 1 that are not orthogonal neighbors, with both orthogonal bridges empty.

Sealing does not clear `dropped`. A 7×3 block rotated 20° around its center cell is 19 cells with `dropped` 2 before the seal, and 21 cells with `sealed` 2 afterward; `dropped` stays 2. A 2×12 strip at 20° seals the missing cell and `breaks` falls to 0. A 1px strip at 20° is not thickened, and `breaks` stays above 0. Near 45° that 1px strip can still shorten, with `sealed` at 0. Rotation does not replace hand cleanup. Paint a finger fix on a `polish` layer so the next `bake` leaves it. Indices move; colors are not invented. One frame's linked layer moves with the origin and anchors in one undo step. Rectangle flip and rotate still leave parts where they are.

CLI: `pixelforge part rotate --name gun --degrees 90 --anchor grip`.

### `flipPart` { #flippart }

`flipPart(name, direction, anchor?, frame?, reportDirty?)`

Use `horizontal` or `vertical` for `direction`. The pivot is the origin or the named anchor. Reflection uses exact integer cell coordinates. It changes the selected frame on the attached layer, moves local anchors with the part and respects the stored selection. An out-of-bounds result fails without writing.

### Part curves { #part-curves }

A curve samples transform values by frame index. For example, 2.5 is halfway from frame 2 to frame 3 regardless of their delays. Sampling leaves playback and pixels unchanged.

Each part has at most one curve per channel:

| `channel` | `value` | Default easing |
| --- | --- | --- |
| `rotation` | Clockwise degrees normalized to (−180, 180], quantized to 0.01°. Ties round away from zero. | `linear` |
| `translateX`, `translateY` | Integer cells relative to the rest origin | `linear` |
| `flipHorizontal`, `flipVertical` | 0 or 1 | `hold` |

A curve is a track of part-transform parameters on the frame index, the same axis as tags and `delayMs`. Sampling does not read `delayMs` and does not move the playback clock. An integer `frame` is that frame. A fractional `frame` falls between frames: `2` is frame 2, and `2.5` is halfway from frame 2 to frame 3 in index space, not weighted by milliseconds. The number is the pose relative to the rest origin. It does not write pixels and it does not call `rotatePart`, `translatePart`, or `flipPart`. `bake` reads the sample and warps rest ink. Applying the sample again with `rotatePart` on a frame that was already transformed stacks.

One curve per part per channel. The ease on a key is how that key leaves toward the next one. The last key's ease is stored and used only after a later key exists. `t = (frame - left) / (right - left)` and the sample is `left + (right - left) * u`.

- *`linear`: `u = t`*
- *`easeInOut`: `u = t² (3 − 2t)` (Hermite smoothstep). At `t = 1/2`, `u = 1/2`, so the midpoint matches linear. At `t = 1/4`, `u = 5/32`*
- *`hold`: `u = 0` until the next key. Flip channels accept only this*

Rotation blends stored centi-degrees and does not take the short arc across ±180°. Outside the span, `interpolation` is `clamp`. On a key it is `key`. Inserting or deleting a frame moves key indices in that undo step. A key on a deleted frame is dropped, and an empty curve is dropped. Renaming a part renames its curves. Removing a part, or a resize that drops its origin, drops its curves. The project version stays 1. An empty list is omitted. An older file that lacks `curves` loads with none. Unknown fields are still rejected. A default ease is omitted from the file.

### `setCurveKey` { #setcurvekey }

`setCurveKey(part, channel, frame, value, ease?)`

Parameters: `part`, `channel`, `frame`, `value` and optional `ease`.

Use an existing frame index. Setting the same key again replaces value and easing. An unchanged key creates no undo entry.

Returns `index`, `keyIndex`, `part`, `channel`, `frame`, quantized `value` and `ease`.

CLI: `pixelforge curve set --part gun --channel rotation --frame 0 --value 0 --ease ease-in-out`. Kebab-case and camelCase channel/easing names are equivalent.

Errors: `PartName`, `PartMissing`, `FrameIndex`, `CurveValue`, `CurveEase`. Unknown channels or easing names return `InvalidParams`.

### `removeCurveKey` { #removecurvekey }

`removeCurveKey(part, channel, frame)`

`params`: `part`, `channel`, `frame`.

`result`: `index`, `part`, `channel`, `frame`, `removedCurve`.

CLI: `pixelforge curve remove --part gun --channel rotation --frame 0`.

### `clearCurve` { #clearcurve }

`clearCurve(part, channel)`

`params`: `part`, `channel`.

`result`: `index`, `part`, `channel`.

CLI: `pixelforge curve clear --part gun --channel rotation`.

### `listCurves` { #listcurves }

`listCurves(part?)`

Returns `curves` in insertion order, optionally filtered by part. Each track lists its part, channel and keys with frame, value and easing. An unknown part returns `PartMissing`.

CLI: `pixelforge curve list --part gun`.

### `sampleCurve` { #samplecurve }

`sampleCurve(part, channel, frame)`

Read the curve at a finite, possibly fractional frame index. The result includes value, interpolation mode and bounding keys. At a key or outside the range, `rightFrame` is null. Missing tracks return `CurveMissing`.

CLI: `pixelforge curve sample --part gun --channel rotation --frame 2`.

### `samplePart` { #samplepart }

`samplePart(part, frame)`

Read a part’s transform channels at a frame index. The result contains only the channels with curves. Sampling leaves the project unchanged.

Keyframe tracks sample part-transform parameters. They do not write pixels. `easeInOut` is Hermite smoothstep. Its midpoint matches linear. Flip holds. Apply the sample from the rest pose, or pass the frame to `solveConstraints`.

### Constraints { #constraints }

Version 1 projects store constraint metadata in `constraints`. Older projects and projects without the field have none. Pixel data remains in layers.

Constraint kinds:

| `kind` | Operation | Field |
| --- | --- | --- |
| `follow` | Translate `part` until its anchor (or origin) reaches the target anchor | Requires `target` and `targetAnchor`. Omit `x` and `y`. |
| `attach` | Align the anchors on both parts, as for a stock against a shoulder. | Requires `anchor`, `target` and `targetAnchor`. Omit `x` and `y`. |
| `pin` | Move the specified anchor (or origin) to canvas cell `(x, y)` | Requires an on-canvas `x`, `y`. Omit target fields. |
| `ik` | Rotate a two- or three-link chain to place its tip anchor or origin exactly on the target cell. | List `chain` from root to tip, ending in `part`. Specify coordinates, a target anchor, or contacts. Unreachable targets return `IkUnreachable`. |
| `plant` | Pin a part’s anchor to a footprint during contact intervals. Outside the interval, its curves continue normally. | `contacts` contains `{frame,to,x,y}` entries. Omitted `to` equals `frame`. Excludes chains, target and pin fields. |

`constraints` is omitted when empty. An older file loads with none. The version stays 1. A constraint is metadata. Pixels stay on layers.

`follow` tracks `targetAnchor` with an optional follower anchor. `attach` requires both anchors. `pin` stores a canvas cell. `ik` rotates a chain of two or three parts so the end cell is exactly the target. `plant` pins an anchor on a footprint span and leaves other frames on the curve pose. `stick` is accepted by the tool and stored as `attach`. The file spelling is `follow`, `attach`, `pin`, `ik`, or `plant`.

`ik` measures bone lengths from the rest origins, keeps the root's curve translation, and does not apply rotation or flip curves on the chain for that frame. `bend` `1` keeps the first joint on the clockwise side of root-to-target. `-1` is the other side. Omit it to keep the rest side. A straight rest accepts either side and picks the angles closest to rest. The other side is not used when the requested side misses. A miss is `IkUnreachable` and does not leave the effector on a neighbor cell.

A footprint span `{frame,to,x,y}` pins while `frame <= f <= to`. The cell does not change inside the span, so a translate curve cannot slide the sole. Frames outside every span are not pinned. Omit `to` and the span is that one frame. Omit `frame` on `solveConstraints` and the span is sampled at `pixelFrame`. Contact frames must already exist. Inserting or deleting a frame shifts them like curve keys. A `plant` with no spans left is dropped. An `ik` drives every joint in its chain. `bake --part` on the end poses the whole chain.

`solveConstraints` is explicit. With `frame`, each part's curves are sampled from the stored pose, then constraints run. Channel order is rotation, horizontal flip, vertical flip around the stored origin, then whole-cell translation. Fractional translation snaps to the nearest cell, ties away from zero. Omit `frame` and curves are not read. Solve order waits for a driven target. Independent constraints keep file order. A cycle is `ConstraintCycle`. A second driver is `ConstraintConflict` and is refused at add time. Two posed parts on one layer are `ConstraintLayer`. One moving part may share a layer with a stationary part; every non-zero cell on that layer moves.

`apply` defaults to false and returns the report without undo. `apply` true writes `pixelFrame` or the active frame. Linked ink moves and `ink` is `moved`. An unattached part is `metadata`. No motion is `unchanged`. A change is one undo step. A later call stacks. The call does not create frames and does not walk the timeline. A span of frames is `bake`.

A failure writes nothing. Removing an anchor leaves the constraint; the next solve is `AnchorMissing`. An origin or final ink cell off the canvas is `PartOffCanvas`. Rename rewrites `part` and `target`. Removing a part drops constraints that name it. `resizeCanvas` shifts pin cells and drops a constraint whose pin or part leaves the canvas. Unknown fields are rejected. Load does not require the anchor to still exist.

### `addConstraint` { #addconstraint }

`addConstraint(name, kind, part, anchor?, target?, targetAnchor?, x?, y?, chain?, bend?, contacts?)`

CLI: `pixelforge constraint add --name hand-grip --kind follow --part hand --anchor palm --target gun --target-anchor grip`.

`pixelforge constraint add --name butt-shoulder --kind stick --part gun --anchor butt --target shoulder --target-anchor joint`.

`pixelforge constraint add --name plant-foot --kind pin --part foot --anchor sole --x 4 --y 11`.

`pixelforge constraint add --name left-leg --kind ik --part shin --anchor sole --chain thigh --chain shin --bend 1 --contact 0:1:12:12`.

`pixelforge constraint add --name left-sole --kind plant --part boot --anchor sole --contact 0:1:4:12`.

### `removeConstraint` { #removeconstraint }

`removeConstraint(name)`

Remove the named constraint, preserving parts and pixels. Returns the previous index and trimmed name. An unknown constraint returns `ConstraintMissing`.

CLI: `pixelforge constraint remove --name hand-grip`.

### `listConstraints` { #listconstraints }

`listConstraints()`

Returns a read-only `constraints` array in the same format as `inspect`, or an empty array.

CLI: `pixelforge constraint list`.

### `solveConstraints` { #solveconstraints }

`solveConstraints(frame?, apply?, pixelFrame?)`

`result`:

| Field | Meaning |
| --- | --- |
| `frame` | Input curve frame or null |
| `pixelFrame` | Pixel frame written |
| `apply`, `wrote` | Whether application was requested and whether parts or pixels actually changed |
| `dropped` | Total source cells dropped by nearest-neighbor collisions. Translation and quarter turns lose none. Sealing does not reset the count. |
| `conflicts` | Destination cells claimed by multiple source cells, summed across layers |
| `sealed` | Cells filled by coverage sealing, at most the corresponding dropped count per layer |
| `breaks` | Pairs of pixels connected only diagonally in the result |
| `order` | Constraint names in solution order |
| `constraints` | Entries in file order with `dx`, `dy`, `satisfied`, `planted`, `from` and `to` |
| `parts` | Solved pose with local and canvas anchor coordinates. `ink` indicates unchanged, metadata-only or moved. Moved layers include `dropped`, `conflicts`, `sealed` and `breaks` counts. |

Solve order, cycles, a missing anchor, and a cell off the canvas are errors. The call writes nothing in those cases. `listConstraints` is read-only. `solveConstraints` with `apply` false is read-only. `apply` true is one undo step when the pose changes.

### `bake` { #bake }

`bake(from?, to?, source?, fps?, parts?, layers?, constraints?)`

All parameters are optional:

| Field | Meaning |
| --- | --- |
| `from` | First frame, inclusive. Defaults to 0. Set to current frame count for append-only baking. |
| `to` | Last frame, inclusive. Defaults to the last existing frame. Additional frames are appended. Must be below 4096. |
| `source` | Rest-pose pixel frame, default 0. Its pixels are preserved. |
| `fps` | Positive finite frame rate. Sets all delays in range to rounded `1000/fps` milliseconds, including the source. At 12 fps the delay is 83 ms. |
| `parts` | Only pose the listed parts. Omit for all parts. An empty array returns `InvalidParams`. |
| `layers` | Only write listed layers. By default, rebuild attached layers controlled by curves or constraints from the source, even for zero transforms. When parts are specified, use their attached layers. Polish layers are excluded from these generated writes. |
| `constraints` | Defaults to true. Set false to sample curves without solving constraints. |

`result`:

| Field | Meaning |
| --- | --- |
| `from`, `to`, `source` | Actual frame range and source frame |
| `fps` | Requested fps or null |
| `delayMs` | Written frame delay or null when fps is omitted |
| `constraints` | Whether constraints were solved |
| `appended` | Newly appended frames |
| `wrote` | Whether pixels or delays changed |
| `dropped` | Total source cells dropped across frames before sealing |
| `conflicts` | Total contested destination cells across frames |
| `sealed` | Total cells filled by coverage sealing |
| `breaks` | Total diagonally connected pixel pairs |
| `polish` | Polish-layer indices from bottom to top. See per-frame `followed` for bound corrections. |
| `frames` | Entries for every frame. The rest frame has empty `layers`/`followed` and zero quality counts. Other entries include pose and collision counts. `layers` excludes polish; `followed` identifies bound corrections transformed with their parts. |

CLI: `pixelforge bake --from 0 --to 11 --fps 12`. `pixelforge bake --part gun --part hand --layer 0`. `pixelforge bake --curves-only`. `pixelforge bake --source 0 --from 1 --to 4`.

Every optional field may be omitted. Each non-source frame is posed from the source frame's ink, not from pixels already on that frame, so a second bake does not stack. Channel order matches `solveConstraints` with `frame`. Stored origins stay. Curves and constraints stay. Reported `x` and `y` are the posed origin.

An existing frame replaces only the layers being written. Duplicate the rest frame first when a body with no part should remain. An appended frame copies every source layer, then overwrites the posed layers. `from` past the frame count is `BakeRange`.

A `parts` list poses only those parts, and a follower tracks a target only when that target is included. `layers` intersects that write set. With neither filter, a layer linked to a part this bake samples or that a solved constraint drives is written, including when that pose is the identity, so a curve that returns to zero restores the source ink. A layer with no curve and no driving constraint stays. Two moving parts on a layer that will be written are `ConstraintLayer`. Every non-zero cell on a written linked layer moves.

A `polish` layer is an overlay and is not rebuilt from a linked part's ink. Naming it with `layers` or `parts` does not treat it as that rebuild. A managed part linked to it is `PolishLayer` and the call writes nothing. Paint the finger fix on the overlay. An unbound overlay stays on the canvas cells where it was painted. After `setLayerBind`, a bake that poses that part stamps the source frame's corrections and explicit erases through the same transform, including an identity pose, so returning to zero does not leave the previous offset. A bound layer whose part this bake does not pose stays. An appended frame still copies the source overlay, then overwrites a bound layer that followed. It does not copy unbound polish that exists only on another frame.

A selection must cover each moving source cell and its destination. A new non-zero cell on an appended frame must lie inside it, and so must an explicit erase. Otherwise the call is `SelectionProtected` and writes nothing. Unbound polish is not part of that write, so an unbound polish cell outside the selection does not fail the bake. A bound overlay is. Off-canvas ink, a missing anchor, a cycle, or a second driver also write nothing.

A change to pixels or delays is one undo step. The source frame can change its delay only. The active frame stays. The project stays version 1 and has no `bake` field. `ink` is `moved` or `unchanged`. An unattached part, or a layer that was not written, is `unchanged`, because the origin is not stored. `polish` is the overlay indices, or `[]`. `dropped`, `conflicts`, `sealed`, and `breaks` use the same coverage-seal rules as `rotatePart`. The source frame reports zeros. A seal does not clear `dropped`. Rotation does not replace hand cleanup.

### `beginGesture` / `endGesture` { #begingesture-endgesture }

`beginGesture()` · `endGesture()`

No parameters. Group successful edits into one undo until `endGesture`. Nested begin returns `OpenGesture`. Ending without a gesture is harmless. Empty gestures add no undo entry.

### `undo` / `redo` { #undo-redo }

`undo()` · `redo()`

`result`: `{ "canUndo": <bool>, "canRedo": <bool> }`.

CLI `--checkpoint FILE` does not store the stack in the project file. One checkpoint step is one process that changed the project. While this process still has kernel operations, `undo` reverts those first; an empty kernel stack then reverts the previous checkpoint step. A fresh process without `--checkpoint` is still `NothingToUndo`. A checkpoint that does not match the project fails before the tool runs, with code `CheckpointMismatch`, exit 2, and neither file written.

### `saveProject` { #saveproject }

`saveProject()`

No parameters.

Returns `{ "json": "<pixelforge-project version 1>" }`, with a final newline. The undo stack and playback clock are excluded.

### `loadProject` { #loadproject }

`loadProject(json)`

Pass `json` as project text. A successful load replaces the document, clears undo history and stops playback.

Returns the same result as `inspect`.

Error: `ProjectFile`. A failed parse preserves the original document.

### `importAseprite` { #importaseprite }

`importAseprite(bytes)`

Pass base64 Aseprite file contents in `bytes`. This tool does not accept `reportDirty`.

Success replaces the document. Frame and layer start at 0, the palette is unlocked, index policy is reject, and undo history, selection and tags are empty. Failure preserves the original.

Returns the `inspect` fields plus the following `aseprite` report:

| Field | Meaning |
| --- | --- |
| `sourceTransparentIndex` | Transparent palette index in the file header |
| `transparentSwapped` | Whether the original transparent index was exchanged with index 0 |
| `hiddenLayers` | Hidden source layers imported as visible |
| `skippedGroups` | Skipped groups. Their child layers are flattened into file order. |
| `skippedReference` | Skipped reference layers |
| `backgroundLayers` | Background layers. The transparent entry maps to index 0. |
| `nonNormalBlend` | Layers with a non-Normal blend mode, imported without blending |
| `partialOpacity` | Layers whose opacity differs from 255 |
| `partialCelOpacity` | Cels whose opacity differs from 255 |
| `nonzeroZIndex` | Cels with nonzero z-index, imported without reordering |
| `durationDefaulted` | Zero-delay frames given a 100 ms delay because the header provided no speed |
| `clippedCells` | Off-canvas pixels discarded |
| `ignored` | Sorted skipped chunk names |

Replaces the document, the same as `loadProject`. The new undo stack is empty. Failure leaves the previous document. Indexed 8-bit only. Image layers map bottom to top so body and gear stay separate. Groups are not stored; their children join that stack in file order. Reference layers are skipped. Hidden layers become visible. Blend, layer opacity, cel opacity, and z-index are counted and not applied. Index 0 is forced transparent; a non-zero transparent index is swapped with slot 0 first. Tags, slices, user data, masks, color profiles, external files, and tilemaps are not imported. There is no `.aseprite` writer and no plugin API. The reader is a small indexed subset parser: `asefile` returns composited RGBA, and `aseprite-io` writes the format back on a newer Rust than this repo. `flate2` only inflates compressed cels.

### `exportPngSequence` { #exportpngsequence }

`exportPngSequence(scale?)`

Returns `frames` as base64 RGBA PNGs in timeline order, along with `scale`, scaled `width` and `height`. The default scale is 1. Out-of-range sizes return `InvalidParams` or `ExportScale`.

`scale` is optional and defaults to 1. The kernel repeats each palette index into a `scale` by `scale` block, then resolves RGBA. There is no filtering. Scale 1 matches the previous bytes. `width` and `height` are the scaled pixel size.

### `exportSpriteSheet` { #exportspritesheet }

`exportSpriteSheet(scale?, columns?, atlas?)`

`result`:

| Field | Meaning |
| --- | --- |
| `png` | Base64 PNG of the grid sheet |
| `columns`, `rows` | Default columns are the ceiling of the square root of frame count. Rows are the ceiling of frame count divided by columns. Explicit `columns` overrides the default. Scale leaves row and column counts unchanged. |
| `sheetWidth`, `sheetHeight` | Full sheet pixel dimensions after scaling |
| `scale` | Scale used, default 1 |

Optional `columns` (an integer `1..=4294967295`). Omit it and the grid stays nearly square. Pass the frame count to keep one direction on one row. Columns past the frame count are transparent padding. `0` is `InvalidParams`. A column count that does not fit in a PNG is `SheetColumns`.

Each frame cell becomes `frame width × scale` by `frame height × scale`. Columns and rows stay the frame grid. Padding cells scale as transparent blocks.

| Field | Meaning |
| --- | --- |
| `format`, `version` | `"pixelforge-atlas"` and `1` |
| `columns`, `rows`, `scale` | Frame grid used for the export, independent of scale |
| `frameWidth`, `frameHeight` | Scaled pixel dimensions of one frame cell |
| `sheetWidth`, `sheetHeight` | Full sheet pixel dimensions |
| `frames` | Actual frames excluding padding, with `index`, pixel `x`, `y`, `width`, `height` and millisecond `delayMs` |
| `tags` | Tags in document order with `index`, `name`, `from`, `to` and `loop`. Ranges use frame indices. |

Optional boolean `atlas`. Omit it, or send `false`, and the result stays the PNG fields with no `atlas` key. `true` adds a `pixelforge-atlas` version 1 object and does not write a file or an undo entry. `frames` are real frames in sheet pixels, including `delayMs`, and do not include padding cells. `tags` are the document tags. `columns` is the frame grid.

CLI `export sheet --atlas FILE` writes that object as pretty JSON plus `image` (the PNG file name). Stdout `result.atlas` is the JSON path. Without `--atlas`, the file is not written and the stdout result has no `atlas` field.

`pixelforge validate-atlas` checks this JSON and the PNG identified by `image`. If a root or per-frame `pivot` is present, it also checks the origin. See [atlas validation and exit codes](export.md).

### `exportApng` { #exportapng }

`exportApng(scale?, tag?, from?, to?, loop?)`

`result`:

| Field | Meaning |
| --- | --- |
| `apng` | Standard base64 of the APNG without a data URL prefix |
| `format` | `"apng"` |
| `scale`, `width`, `height` | Scale and scaled pixel dimensions |
| `loop` | Loop mode used |
| `plays` | APNG `num_plays`. Zero loops indefinitely, one plays once. |
| `frameCount` | Number of playback frames. Ping-pong may repeat interior frames. |
| `frames` | Playback order as `{ "index", "delayMs" }` using document frame indices |
| `tag` | Normalized tag name, only when a tag is used |

Optional `scale` matches `exportPngSequence`. Optional `tag` plays that tag's range and loop. Without `tag`, optional `from` and `to` are an inclusive range (the whole timeline when omitted) and `loop` is `forward`, `reverse`, `pingpong`, or `once` (default `forward`). Passing `tag` together with `from`, `to`, or `loop` is `InvalidParams`. The document and the undo stack stay unchanged. Each frame is the same RGBA composite as `exportPngSequence`. The file is APNG, not GIF, and it is not quantized or dithered. See [export.md](export.md). `frames` is the playback order; ping-pong repeats a middle frame, so `frameCount` can be longer than the source range. `plays` is 0 for a loop and 1 for `once`. CLI `export apng` writes the file and returns `path` and `bytes` instead of `apng`. A missing tag is `TagMissing`. A bad name is `TagName`. A range outside the frame list is `PreviewRange`. Scale 0 is `InvalidParams`. A scaled size that does not fit a PNG is `ExportScale`.

### `previewFrame` { #previewframe }

`previewFrame(frame?, scale?, layer?, compare?, selection?, x?, y?, width?, height?, mask?)`

Optional `frame` (default active), `scale` (default 1) and `layer`. Omit the layer for the composite. Set `compare: true` for a side-by-side comparison. The active layer stays selected.

Choose at most one scope. With no scope the full frame is used and the result omits `scope`, `mask` and `protected`.

| Parameter | `scope` |
| --- | --- |
| `selection: true` | `selection` uses the stored mask. Missing selection returns `MaskMissing`. |
| `x`, `y`, `width`, `height` | `rect` requires all four integer bounds. |
| `mask` | `mask` takes one or more rectangles in raw calls. The chat catalog accepts rectangular bounds. |

Multiple scopes or `selection` other than boolean true return `InvalidParams`.

Read-only. The active frame and undo history stay unchanged. Returns:

| Field | Meaning |
| --- | --- |
| `frame` | Rendered frame |
| `activeFrame` | Document’s active frame |
| `activeLayer` | Document’s active layer |
| `layer` | Rendered layer or null for a composite |
| `composite` | True when no layer is specified |
| `width`, `height` | Canvas dimensions in cells |
| `scale` | Scale used |
| `ink` | Number of nonzero cells |
| `bounds` | Bounding rectangle of occupied cells or null when empty |
| `indices` | Used indices and counts in increasing order, including index 0 when used |
| `pixels` | Row-major palette indices of the composite or selected layer |
| `pngWidth`, `pngHeight` | Scaled pixel dimensions |
| `png` | Base64 RGBA PNG with nearest-neighbor scaling |
| `scope` | Scope name when constrained: `selection`, `rect` or `mask` |
| `mask` | Mask in the same shape as the stored selection, only for a constrained scope |
| `protected` | Number of cells outside the mask, only for a constrained scope |

Read-only. Does not change `activeFrame` and does not push undo. Omit the scope and the result has no `scope`, `mask`, or `protected`. Inside a scope, `ink`, `bounds`, and `indices` count only masked cells. `pixels` stay the full index buffer. ASCII outside the mask is `#`. The PNG paints RGBA `[80, 64, 112, 255]` there. That color is a view. It is not a palette entry and it is not stored. The PNG is this frame after nearest-neighbor `scale`, then palette resolution. The glyph `*` collapses every index from 62 up, so read `pixels` or `indices` for the real index. The `pixelforge preview` command also puts ASCII rows in `result.rows` and writes the PNG to `result.path` (default `<stem>.preview.png` beside the project) instead of base64.

`compare: true` writes two panels with no gutter. Width is doubled. The left panel is the unquantized RGBA composite (`frame_rgba`, the color preview, `export png`). The right panel is the palette color of the indices `flattenLayers` would store for this frame, which is the color preview after flatten. The call does not flatten, does not change the document, and does not push undo. Use it before flatten, while the stack is still layered. After flatten, when the result layer is opaque, the panels match. `pixels`, `ink`, and `indices` stay the covering index. `flattenPixels` and `flattenRows` are the right panel. `indexChanges` counts cells whose covering index differs from `flattenPixels`. `colorChanges` counts cells whose left RGBA differs from the right color. `panels` is `["rgba", "indexed"]`. `layer` or any scope together with `compare` is `InvalidParams`. CLI: `pixelforge preview --compare`. The default file is `<stem>.compare.png`. Stderr prints the covering rows, then `flattenRows`.

### `diffFrames` { #diffframes }

`diffFrames(frame, with, scale?, layer?, selection?, x?, y?, width?, height?, mask?)`

Required `frame` and `with`, optional `scale` (default 1) and `layer`. Omit layer for composite indices. Supports the same selection, rectangle or mask scopes as `previewFrame`.

Read-only comparison of stored indices. The onion skin is a preview. Returns:

| Field | Meaning |
| --- | --- |
| `frame`, `with` | The two compared frame indices |
| `activeFrame` | Document’s active frame |
| `activeLayer` | Document’s active layer |
| `layer` | Compared layer or null for the composite |
| `composite` | True when no layer is specified |
| `width`, `height` | Canvas dimensions in cells |
| `scale` | Scale used |
| `changed` | Count of cells with different indices |
| `bounds` | Bounding rectangle of differences, or null if unchanged |
| `kinds` | `same`, `differ`, `onlyFrame` and `onlyWith` counts. `differ` counts unequal pairs where both entries are nonzero. |
| `changes` | Only differing cells, in row-major order: `{ "x", "y", "left", "right" }`. Left is `frame` and right is `with`. |
| `diffRows` | One string per row. A dot means equal, an exclamation mark means different nonzero indices and a single nonzero cell uses its preview glyph. A hash marks protected cells. |
| `panels` | `["frame", "with", "onion"]` |
| `pngWidth`, `pngHeight` | PNG dimensions. Width spans three side-by-side panels. |
| `png` | Base64 RGBA image showing `frame`, `with` and an onion skin |
| `scope`, `mask`, `protected` | Same as `previewFrame`, only for constrained scopes |

Read-only. Compares indices and does not write onion skin back into pixels. `diffRows`: `.` same index, `!` both non-zero and different, otherwise the glyph of the only non-zero index. `#` is outside the mask and is neither `same` nor a change. The PNG is three panels: `frame`, `with`, then the onion. The onion uses the shared palette color when indices match, the only ink when one side is transparent, and the average of the two palette colors when both differ. Cells outside the mask are RGBA `[80, 64, 112, 255]` on every panel. Those colors are a view. They are not stored and they are not palette entries.

The `!` glyph does not carry either index. Read `changes` for `left` and `right`. The CLI keeps `bounds`, `kinds`, and `changes` on the JSON line.

### `flipHorizontal` { #fliphorizontal }

`flipHorizontal(frame?, x?, y?, width?, height?, layer?, reportDirty?)`

Mirrors indices left to right. Cell `x` receives the index from the mirrored column (`63 - x` on a full frame of width 64). The palette is unchanged, including highlight colors. One undo step. A symmetric rectangle is not recorded. `frame` is not selected. An out-of-bounds rectangle writes nothing.

### `flipVertical` { #flipvertical }

`flipVertical(frame?, x?, y?, width?, height?, layer?, reportDirty?)`

Mirrors indices top to bottom. Cell `(x, y)` receives the index from `(x, originY + height - 1 - (y - originY))`. The palette is unchanged. One undo step. A symmetric rectangle is not recorded. `frame` is not selected. An out-of-bounds rectangle writes nothing. With a selection, a cell is written only when it and the cell it reads are both inside.

### `rotate` { #rotate }

`rotate(degrees, frame?, x?, y?, width?, height?, layer?, reportDirty?)`

`degrees` must be 90, 180 or 270 clockwise. Other parameters match `flipHorizontal`, including optional frame, layer, rectangle bounds and dirty reporting. Invalid angles return `InvalidParams`.

Reorder indices inside a rectangle while preserving palette and canvas dimensions. A half turn accepts any rectangle. A quarter turn requires a square or returns `RotateSquare`. Use `resizeCanvas` before rotating a nonsquare full frame.

For destination coordinates `(x, y)` inside a rectangle of width `w` and height `h`, the source cell is:

| Angle | Source cell |
| --- | --- |
| 90 | `(y, h - 1 - x)` |
| 180 | `(w - 1 - x, h - 1 - y)` |
| 270 | `(w - 1 - y, x)` |

Turns indices clockwise inside the rectangle. The palette and the canvas size stay, so other frames keep their coordinates. 180 accepts any rectangle. 90 and 270 need a square, or the call is `RotateSquare` and writes nothing. A whole-frame quarter turn therefore needs a square canvas; resize first. One undo step. One layer. A selection writes a cell only when the destination and the source cell are both inside.

### `resizeCanvas` { #resizecanvas }

`resizeCanvas(width, height, offsetX?, offsetY?, reportDirty?)`

Choose width and height in 1–256. Supply both signed offsets or omit both for centered placement (integer division toward zero). Cells outside the new canvas are cropped. The returned offsets are the ones actually used.

Copies every frame's indices onto the new canvas. The old origin lands at `(offsetX, offsetY)`. Omit both offsets to center with `offset = (new - old) / 2`, truncating toward zero. Cells without a source stay index 0. Old cells that fall outside are clipped. Delays, the palette, the lock, and the active frame stay. The file stays version 1. One undo restores the previous buffers. A resize that leaves every index in place is not recorded.

## Layers { #layers }

Composite from the bottom. A hidden layer, or opacity 0, is skipped. Index 0 does not cover. On a `polish` layer an explicit erase is still index 0, and the erase mark clears the composite cell so the layers underneath do not show. A 0 without that mark is unmodified. The index composite (ASCII preview, diff, inspect `composite`) lets any other index cover the cell. `flattenLayers` snaps each cell of the RGBA composite with the `applyPng` rule: alpha below 128 is index 0, otherwise the nearest opaque entry by squared RGB, and a tie keeps the lower index. A cell whose RGBA already equals the covering index keeps that index. The color preview stays the unquantized blend until flatten. Compare with `preview --output before.png`, then `layer flatten`, then `preview --output after.png`. The RGBA composite (`frame_rgba`, the preview PNG, `export png`, `export sheet`) replaces the cell with the palette color when opacity is 255, and palette alpha is not blended into the layer underneath. An explicit erase at opacity 255 clears the cell; a lower opacity scales the destination alpha by the remaining visibility. Opacity 1..=254 uses straight-alpha source-over for painted cells. Source alpha is `round(palette.a * opacity / 255)`, ties away from zero. The blend is not written back. Nearest-neighbor scale repeats the blended cell. `--layer` shows stored indices and ignores visibility and opacity, so an erase mark looks like index 0 on that layer alone. The flag and the opacity are shared by every frame. Defaults are omitted from the file. Older files load visible and fully opaque. There is no blend mode.

Each layer has a `role`. The default is `default`: ordinary pixels, which `bake` may rebuild from the source when a curve or constraint manages that layer. `polish` is an overlay. The file may also store `part` and an optional `anchor`, and only when the overlay is bound. Local `(0, 0)` is that anchor's rest canvas cell, or the part origin when `anchor` is omitted. Cells stay in the frame buffer at canvas coordinates; the binding decides which transform they follow. Changing an anchor offset does not move cells already painted. Removing the part clears the binding and leaves the cells on the canvas. The file writes `role` only for `polish`. An explicit erase is an `erase` array beside that layer's pixels, omitted when every cell is 0. A `1` is valid only on index 0. `layer flatten` snaps the RGBA composite into `Layer 1`, clears the role, and turns a punched hole into an ordinary index 0 when that blend's alpha is below 128. The result stays visible and fully opaque.

### `listLayers` { #listlayers }

`listLayers()`

No parameters. Read-only.

Returns `layerCount`, `activeLayer` and layer records with `index`, `name`, `role`, `part`, `anchor`, `visible` and `opacity` (0–255). Role is default or polish. Missing bindings are null.

### `addLayer` { #addlayer }

`addLayer(name?, role?)`

Insert a transparent layer above the active layer on every frame and select it. Omit `name` for an unused Layer N name. Use `default` (the default) or `polish` for the role. Names are trimmed, with 1–64 characters allowed.

`result`: `{ "index", "name", "role", "activeLayer" }`.

### `setLayerRole` { #setlayerrole }

`setLayerRole(role, index?)`

Assign `default` or `polish` to the selected or specified layer. Switching to `default` clears its part binding. Returns `index`, `role`, `part` and `anchor`. Unknown roles return `InvalidParams`.

CLI: `pixelforge layer role --role polish`. `pixelforge layer add --name polish --role polish`.

### `setLayerBind` { #setlayerbind }

`setLayerBind(part, index?, anchor?)`

Bind an existing polish layer to a part and optional anchor. Pass null for `part` to clear the binding and omit the anchor. The default target is the active layer. Invalid bindings return `PolishBind`.

`result`: `{ "index", "role", "part", "anchor" }`.

CLI: `pixelforge layer bind --part hand --anchor palm`. `pixelforge layer unbind`.

### `setLayerVisible` { #setlayervisible }

`setLayerVisible(visible, index?)`

Set a boolean visibility flag on the active or specified layer. Hiding a layer excludes it from composites, flattening and export. Its stored indices remain editable.

`result`: `{ "index", "visible" }`.

### `setLayerOpacity` { #setlayeropacity }

`setLayerOpacity(opacity, index?)`

Set the active or specified layer’s opacity to an integer from 0 to 255. Zero omits it from the composite; 255 uses its palette color. Intermediate values use source-over blending in RGBA exports. Invalid values return `InvalidParams`.

`result`: `{ "index", "opacity" }`.

CLI: `pixelforge layer opacity --opacity 128`.

### `deleteLayer` { #deletelayer }

`deleteLayer(index?)`

Optional `index`, default active layer. At least one layer must remain. Selection follows the layer at the deleted index or the new top layer.

Returns the deleted `index`, new `activeLayer` and remaining `layerCount`.

Errors: `LayerIndex`, `LastLayer`. The only remaining layer cannot be deleted, regardless of the supplied index.

### `renameLayer` { #renamelayer }

`renameLayer(name, index?)`

Pass `name` and an optional `index` (default active layer). An unchanged name adds no undo entry.

Returns `index` and the trimmed `name`.

Errors: `LayerIndex`, `LayerName`.

### `setActiveLayer` { #setactivelayer }

`setActiveLayer(index)`

Select an existing zero-based layer index. Selecting the same layer adds no undo step. An invalid index returns `LayerIndex`.

`result`: `{ "index" }`.

### `reorderLayer` { #reorderlayer }

`reorderLayer(from, to)`

Move a layer from its current index to its final index. Index 0 is the bottom. Selection follows the layer through the move and an unchanged position adds no undo step.

`result`: `{ "from", "to", "activeLayer" }`.

### `flattenLayers` { #flattenlayers }

`flattenLayers()`

`result`: `{ "layerCount": 1, "activeLayer": 0, "name": "Layer 1" }`.

Layers are a document-level stack. Every frame has one index buffer per layer, the same size as the canvas. Index 0 is the bottom. A new project and an old single-layer file are one layer named `Layer 1`. That file is still saved without `layers` or `activeLayer`. A stack, a renamed layer, or a non-zero active layer writes those fields. Frames with more than one layer store `frames[].layers[].pixels` and omit the legacy `pixels` array.

Index composite and RGBA composite are separate. See the rules above. Flatten snaps the RGBA composite into `Layer 1`.

`preview`, `diff`, and `export png` / `export sheet` / `export apng` show the composite. `previewFrame`, `diffFrames`, and CLI `--layer N` show one layer. A `layer` param on a drawing tool chooses where the indices go and does not change `activeLayer`.

`addLayer` inserts a transparent layer above the active one on every frame and selects it. Optional `role` is `default` or `polish`. `setLayerRole` changes that role and does not move pixels. `setLayerVisible` and `setLayerOpacity` change the composite and do not move pixels. The same value is not recorded. `deleteLayer` refuses the last layer. `reorderLayer`'s `to` is the index after the move. `flattenLayers` snaps the RGBA composite onto the palette and writes one layer named `Layer 1`, visible and fully opaque, and clears a polish role. Alpha below 128 is index 0. Otherwise the nearest opaque entry by squared RGB wins, and a tie keeps the lower index. An exact cover stays.

## Tilesets, autotiling and maps { #tileset-autotile-tilemap }

A tileset is not a second pixel buffer. It is a `pixelforge-project` whose canvas is one tile, 16 or 32 on each edge, and whose frames are the tiles. The frame index is the tile id. `paint`, `fillRect`, the palette, `preview`, and `diff` are unchanged. The file omits `tileset` when the project is not a tileset.

A map is a separate `pixelforge-tilemap` version 1 file. Width and height count cells, not pixels. Each layer's `cells` are row-major and `width * height` long. An empty cell is `null`. A cell with no flip is an integer tile id. A flipped cell is `{ "tile", "flipH", "flipV" }`. Layer 0 is the bottom. The file does not store undo. `pixelforge call --project map.json` dispatches the tilemap tools when the format is `pixelforge-tilemap`. `--checkpoint` is refused on a map.

Four-neighbor autotile masks are 0 through 15. North is 1, east is 2, south is 4, west is 8. A neighbor outside the map is empty. A cell is occupied when it is not null, whatever the tile id is. Rules are `pixelforge-autotile` version 1. `neighbors` must be 4, and `rules` must contain each mask once. The same input writes the same cells. Eight neighbors return `AutotileNeighbors` and write nothing.

A Tiled `.tmj` is an orthogonal JSON map, version `"1.10"`. `firstgid` is 1, so tile id 0 is GID 1 and an empty cell is 0. Horizontal flip sets `0x80000000`. Vertical flip sets `0x40000000`. There is no diagonal flip. `export tileset` and `export tilemap` must use the same `--cols` and `--scale`. The `.tmj` `image` is the tileset PNG file name.

### `setTileset` / `clearTileset` / `validateTileset` { #settileset-cleartileset-validatetileset }

`setTileset(tileWidth, tileHeight)` · `clearTileset()` · `validateTileset()`

Set both dimensions to the existing canvas dimensions. Each must be 16 or 32. Each frame becomes one tile. `clearTileset` removes the declaration while keeping pixels. `validateTileset` returns tile dimensions and the number of frames, or `TilesetMissing` for a project without a tileset.

CLI: `pixelforge tileset set --tile-width 16 --tile-height 16`, `tileset clear`, `tileset validate`.

### `exportTileset` { #exporttileset }

`exportTileset(scale?, columns?, image?)`

Optional `scale` (default 1), `columns` (default one row) and `image` (default `tiles.png`, a relative PNG filename).

Returns base64 `png`, version 1 `pixelforge-tileset` metadata, columns, rows, scale, scaled image/tile dimensions, sourceTileWidth, sourceTileHeight, tileCount and image name.

Read-only. CLI `export tileset` writes the PNG and metadata and returns paths on stdout.

Errors: `TilesetMissing`, `TileAlign`, `TileImage`, `ExportScale`, `SheetColumns`.

### Tilemap tools { #tilemap-tools }

Find these under `tilemapTools` in `pixelforge tools`. `TileMap::call_tool` accepts the same envelope.

| tool | params | Operation |
| --- | --- | --- |
| `inspectTilemap` | None | Map dimensions, tile size, tile count, layer names and filled cells |
| `getTile` | `x`, `y`, optional `layer` or `name` | Read one cell. Empty cells return a null tile. |
| `setTile` | Same coordinates plus `tile` (null to clear), optional `flipH` and `flipV` | Write one cell. Unchanged values add no undo entry. |
| `fillTiles` | `x`, `y`, `width`, `height`, `tile`, optional flips and layer | The entire rectangle must lie inside the map. |
| `clearTiles` | `x`, `y`, `width`, `height`, optional layer | Clear the rectangle |
| `addTileLayer` | `name` | Add an empty layer at the top, up to 32 layers |
| `autotile` | `rules` (object or JSON string), optional layer | Retile occupied cells from their four neighbors, leaving empty cells alone |
| `validateTilemap` | Optional `rules` | Check tile IDs and, if supplied, rule tile references |
| `exportTilemap` | `tilesetProject` as JSON text, optional `image`, `columns` and `scale` | Base64 preview `png` and a Tiled `tmj` object |
| `undo` / `redo` | None | Undo map edits in the current process only |

CLI: `tilemap new`, `tilemap inspect`, `tilemap get`, `tilemap set`, `tilemap fill`, `tilemap clear`, `tilemap layer add`, `autotile apply`, `autotile validate`, `export tilemap`, `validate-tiles`.

Tilemap tool names are `tilemapTools` from `pixelforge tools`, not the pixel-document `tools` list. `TileMap::call_tool` uses the same envelope. `validate-tiles` accepts `--project`, `--map`, and `--rules`, at least one. A kernel error exits 1. An unreadable file exits 2.

## Project manifest { #project-manifest }

A project manifest is a separate `pixelforge-manifest` version 1 file. It stores canvas tiers, perspective, directory rules, the default palette path, a short note, the asset root, and an optional asset registry. It does not store pixels and it does not replace a `pixelforge-project`. Fields, exit codes, and the out-of-scope list are in [project-manifest.md](project-manifest.md). The example is the zombie pack manifest in [pixelforge-demos](https://github.com/starcloud530/pixelforge-demos): the zombie character, its idle and walk clips, the atlas, and the four props.

| tool | params | Operation |
| --- | --- | --- |
| `inspectManifest` | None | Return the manifest. Empty registries omit `assets`. |
| `validateManifest` | `rootExists`, optional `files` | Validate conventions and registrations against the caller’s filesystem probes without reading disk. |
| `validateAssets` | `rootExists`, optional `files` | Check registered paths, dangling IDs, owners, output relationships and cycles |
| `characterMatrix` | `id`, `project` with `frameCount`, `directionLayout`, `tags`, `frames`, optional `missingPaths` | Direction/action matrix built from the caller’s project probe, read-only |
| `addAsset` | `id`, `kind`, `path`, optional `owner`, `dependsOn`, `produces`, `tags` | Append a record. Duplicate IDs are rejected. |
| `setAsset` | `id` and the fields to update | ID is immutable. Use `owner: null` or an empty list to clear a field. |
| `removeAsset` | `id` | Reject removal while references remain |
| `listAssets` | Optional `kind`, `owner`, `tag` | Matching records |
| `showAsset` | `id` | Stored record plus derived `owns` and `dependents` |
| `graphAsset` | `id` | One hop of `nodes`, `edges` and `missing` references |
| `dependentsAsset` | `id` | Records whose `dependsOn` contains this ID |

Manifest tool names are `manifestTools` from `pixelforge tools`. `ArtManifest::call_tool` uses the same envelope. The file has no undo stack. `pixelforge call --project manifest.json` dispatches these tools when the format is `pixelforge-manifest`, not the pixel document and not the tilemap. `--checkpoint` and `--preview` are refused. `validateManifest` and `validateAssets` compare the caller's disk snapshot and do not read the disk themselves. A missing probe is `probeMissing`. A canvas miss is `canvasSize`. An empty directory is a `layoutEmpty` warning. `projects` is a count, not a file list. `checks.assets` is the registry count. A missing asset file is `assetMissing`. A dangling id is `assetRef` or `assetOwner`. A `produces` edge that is not mirrored by `dependsOn` is `assetProduces`. A `dependsOn` loop is `assetCycle`. `characterMatrix` takes a project probe the caller fills after opening the pixel file; cell status is `present` / `missing` / `empty` / `placeholder`. Placeholders are warnings. The matrix checks clip coverage. Review motion using the player. `project validate`, `asset validate`, and `character check` exit 1 when `errorCount` is greater than 0. The tool response stays `ok: true`. An unreadable file exits 2 with `ManifestFile`. `addAsset` writes when the caller saves.

## Invariants { #invariants }

Pixel, palette, frame, and delay edits go through `Document`'s public methods only. The Tool API does not keep a second buffer.

Those edits share the undo stack. A later UI `undo` reverts the agent's step.

A bad index, a locked palette, and a bad project file return an `error` from the tables above and do not leave a partial edit.

Onion skin is a view. `diffFrames` returns one and does not write it back into pixels. This API has no tool that stores an onion layer. A scope leaves cells outside the mask out of the compare and marks them protected in the view.
