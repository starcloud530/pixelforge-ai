# Export sprites and import them into a game { #export }

| Deliverable | Files to use |
| --- | --- |
| Animated game sprite | Sheet PNG plus atlas JSON for frame rectangles, timing and tags |
| Individual frames | A numbered PNG sequence |
| Animation review | A transparent APNG with the project’s frame delays |
| Tilemap | Tiled JSON map (`.tmj`) and the tileset PNG it references |

The sprite examples use the [Port Keeper project](showcase.md#port-keeper). Run the commands from the root of the extracted character pack. Its source is `a/keeper.pxf.json`.

## Export a sprite sheet { #sprite-sheet }

~~~bash
pixelforge export sheet --project a/keeper.pxf.json --cols 5 --output keeper-sheet.png --atlas keeper-sheet.json
~~~

This exports 20 frames into five columns and four rows. Each row holds one direction. Frames read left to right, then top to bottom. With no `--cols`, the column count is the ceiling of the square root of the frame count. Unused cells at the end of the sheet are transparent padding.

Exports composite the visible layers. Index 0 and fully hidden layers produce no ink. Set `--scale K` to enlarge each output cell into an integer K × K block using nearest-neighbor sampling. The default scale is 1. Keep the original project for later edits to layers and frames.

## Read the atlas { #atlas-json }

Pass `--atlas` to write a version 1 `pixelforge-atlas` alongside the PNG. The `image` value names the sheet. Keep the files together and preserve that name when moving them.

| Atlas data | Interpretation |
| --- | --- |
| `columns`, `rows`, `frameWidth`, `frameHeight` | Sheet grid and scaled dimensions of a single cell |
| `frames` | One record per real frame, with a zero-based index, top-left pixel position, dimensions and `delayMs` |
| `tags` | Named clips with inclusive `from` and `to` indices and a `loop` mode |

## Validate the exported files { #validate-atlas }

~~~bash
pixelforge validate-atlas --atlas keeper-sheet.json
~~~

The validator locates the PNG relative to the atlas. Use `--sheet FILE` for an explicit path. Read `result.errors`, `result.warnings` and `result.checks` in stdout. Exit 0 means that no errors were found. Exit 1 means validation completed with errors. Exit 2 means the input could not be read or parsed.

### Atlas checks { #18 }

Strict mode is the default. `--loose` downgrades specified convention checks to warnings. Both modes reject invalid sheet geometry.

| Check | Code | Strict | Loose |
| --- | --- | --- | --- |
| Frame rectangle extends beyond the sheet | `outOfBounds` | error | error |
| Frame rectangles overlap | `overlap` | error | error |
| Frame is outside its grid slot, sheet dimensions disagree with the declared grid, or there are too few rows | `grid` | error | error |
| PNG dimensions differ from `sheetWidth` or `sheetHeight` | `sheetSize` | error | error |
| Opaque pixels lie outside every frame, including unused trailing cells | `padding` | error | error |
| `frameWidth` or `frameHeight` is not divisible by `scale` | `scale` | error | error |
| Frame or tag indices are discontinuous, a range is invalid, a name is duplicated, or `loop` is outside `forward`, `reverse`, `pingpong` and `once` | `frameIndex` / `tagIndex` / `tagRange` / `loop` / `tagName` | error | error |
| A name listed in `requireTags` is missing | `missingTags` | error | error |
| Unscaled frame sides must each match an allowed size (default **64, 96, 128**). A 64×128 canvas passes; 32×64 does not. Divide the exported dimensions by `scale` before comparing. | `size` | error | warning |
| Directional idle/walk tag names and ranges must match the rows. For four directions the order is down, left, right, up (S/W/E/N). Compass aliases are accepted. Eight rows or a diagonal tag enables eight-direction checks. | `naming` or `missingTags` | error | warning |
| The atlas has no tags, four-direction checks are inactive and the default tag policy is in use | `missingTags` | error | warning |
| `image` differs from the PNG filename | `image` | error | warning |
| `delayMs` is outside `1..=65535` | `delay` | error | warning |
| An optional `pivot` is measured in local pixels from the top left, with y increasing downwards. The far edges are valid. The vertical distance from each frame’s lowest opaque row to the pivot must be consistent. | `pivot` | Out of bounds or inconsistent distance is an error | Out of bounds is an error. Inconsistent distance is a warning. |
| A frame is entirely transparent | `emptyFrame` | warning | warning |

### Set project-specific rules { #18_1 }

Save this file as `keeper-rules.json` to require a 64-pixel canvas and down-facing idle and walk tags in the sample. Command-line flags override corresponding file settings.

~~~json
{
  "format": "pixelforge-atlas-rules",
  "version": 1,
  "mode": "strict",
  "sizes": [64],
  "requireTags": ["idle_down", "walk_down"],
  "fourDirection": "auto",
  "tagPolicy": "required"
}
~~~

~~~bash
pixelforge validate-atlas --atlas keeper-sheet.json --rules keeper-rules.json
~~~

Use an empty `sizes` array to skip canvas-tier checks. An optional `pivot` sets frame-local pixel coordinates. When neither the atlas nor the rules specify a pivot, `checks.pivot` is skipped. Configure the same pivot in the engine.

## Export a PNG sequence { #png-png-sequence }

~~~bash
pixelforge export png --project a/keeper.pxf.json --out-dir keeper-frames
~~~

The files `pixelforge-000.png`, `pixelforge-001.png` and so on follow timeline order. Read delays from the source or atlas when importing the sequence.

## Export an animated preview { #apng }

~~~bash
pixelforge export apng --project a/keeper.pxf.json --output keeper-walk.apng --tag walk_down
~~~

APNG preserves the composited RGBA pixels and millisecond delays. Use a named tag or choose `--from` and `--to` with an optional `--loop`. Available loop modes are `forward`, `reverse`, `pingpong` and `once`. Ping-pong reverses direction without holding either endpoint twice. To pause and inspect the original frames, open the project with `pixelforge play`.

## Preview before flattening { #side-by-side }

~~~bash
pixelforge preview --project a/keeper.pxf.json --compare --scale 4
~~~

Open the `.compare.png` file beside the source. The left panel shows the visible RGBA composite. The right shows which palette indices a flatten operation would keep. The preview leaves the source layers editable. This mode operates on the full composite. Omit `--layer` and mask options. [Transparency and flattening](known-limits.md#translucent-flatten-snaps-to-nearest-index)

## Import into Godot 4 { #godot-4 }

1. Add the PNG to the project. Set the target CanvasItem’s **Texture Filter** to **Nearest** and disable texture mipmaps for integer-scale pixel art.
2. For `AnimatedSprite2D`, create a `SpriteFrames` resource. Add frames from the sheet using the atlas column and row counts. Select only the real frames of each tag, starting at its `from` index.
3. Godot expresses frame duration relative to animation speed. With the animation set to 10 FPS and playback speed 1, use `delayMs / 100` as the relative duration. A 140 ms frame has duration 1.4. Match the clip’s playback order and loop behavior. [Godot SpriteFrames reference](https://docs.godotengine.org/en/stable/classes/class_spriteframes.html)

## Import into Unity { #unity }

1. Set **Texture Type** to **Sprite (2D and UI)**, **Sprite Mode** to **Multiple**, **Mesh Type** to **Full Rect**, **Filter Mode** to **Point** and **Compression** to **None**. Choose a consistent Pixels Per Unit value for related assets.
2. In the Sprite Editor, slice by grid cell size using the exported `frameWidth` and `frameHeight`. Choose the same pivot for related poses. Leave out transparent padding after the last real frame.
3. Add sprites to the animation in atlas order. For equal delays, use `1000 / delayMs` samples per second. For variable delays, place keys at cumulative frame times and keep the final frame visible for its full duration. [Unity sprite import settings](https://docs.unity3d.com/6000.0/Documentation/Manual/texture-type-sprite.html)

## Open a map in Tiled { #tiled }

Export the tileset with `export tileset` and the map with `export tilemap`, using the same `--cols` and `--scale` for both. The TMJ resolves its `image` filename relative to itself. Keep that tileset PNG and the TMJ together. For a ready-to-open five-layer map, download [Tidal Glasshouse](/pixelforge-demos/gallery/#tidal-glasshouse).

The export supports one orthogonal tileset and 16 four-neighbor autotile masks. Empty cells use GID 0. Tile ID 0 maps to GID 1. Horizontal and vertical flips use GID bits `0x80000000` and `0x40000000`. Diagonal flips and eight-neighbor autotiling are outside the supported map format. [Create a tilemap with the CLI](agent-skill.md#tileset-autotile-tilemap)
