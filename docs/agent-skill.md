# Create pixel art with an agent { #agent-skill-agent-skill }

An agent with terminal access can edit a local PixelForge project, render a preview after each change and export the result. Give it the project path, the intended canvas size and a concrete deliverable, such as a walk cycle with four directions.

## Connect your agent { #install-and-run }

[Install the CLI](install.md), then run these commands in the same working directory and terminal environment your agent uses. The installed binary includes the skills for its release.

~~~bash
pixelforge --version
pixelforge skills list
pixelforge skills read pixelforge
~~~

Download and unzip the [Port Keeper project](/pixelforge-demos/gallery/#port-keeper). From the extracted directory, ask:

> Open `b/keeper-storm-cape.pxf.json`. Read the PixelForge and layer skills. Show me the layers and palette. Edit only the `storm-cape` layer, preserve the frame timings, and return the before/after previews and editable project.

## Choose a skill { #skills }

Read the core skill first. Load the additional skill that matches the edit.

| Skill | Use it for |
| --- | --- |
| `pixelforge` | Project structure, indexed colors, CLI calls, error handling and export |
| `pixelforge-draw` | Choosing a canvas and building a silhouette, color masses and details |
| `pixelforge-sprite-4dir` | Arranging directional idle poses, walk cycles and animation tags |
| `pixelforge-layers` | Editing body, clothing, equipment and bound polish layers independently |
| `pixelforge-critique` | Reviewing silhouette, contrast, foot contact and frame timing |
| `pixel-art-craft` | Planning key poses, weight shifts, camera angle and secondary motion |

~~~bash
pixelforge skills read pixelforge-layers
pixelforge skills list pixel-art-craft
pixelforge skills read pixel-art-craft references/action-motion.md
~~~

## Read command results { #output }

Read structured JSON from stdout. Preview commands also write a PNG and print an ASCII rendering to stderr. Use the color preview to judge blending and opacity. For a script, check the exit code before using `result`. On failure, inspect `error.code` and `error.message`.

## Palette indices, layers and undo { #invariants }

Index 0 is transparent. Paint with an existing opaque palette index. A locked palette still permits drawing with its entries. A color change at one index affects every frame that uses it. Direct writes go to the active layer unless you specify `--layer`. The default preview and export composite visible layers.

For undo across CLI invocations, pass the same `--checkpoint FILE` on every modifying command in the sequence. Within a batch, valid requests before the first failure are saved. Read the project again before retrying that batch.

## Edit a project from the terminal { #command-reference }

Run the following character examples in order in a new directory. They use a 64 × 64 canvas and two frames. All coordinates start at zero.

### Create the canvas { #blank-project }

~~~bash
pixelforge new --width 64 --height 64 --output hero.json
pixelforge scene --project hero.json
~~~

### Block in the first pose { #paint-fill-erase-copy-move }

Draw a coat on layer 0. Copy its left boot to the other side, then add a helmet on a separate layer. Keeping the helmet on its own layer makes a later recolor local to that garment.

~~~bash
pixelforge fill-rect --project hero.json --layer 0 --x 22 --y 24 --width 20 --height 21 --color-index 3
pixelforge fill-rect --project hero.json --layer 0 --x 22 --y 45 --width 6 --height 6 --color-index 1
pixelforge copy-rect --project hero.json --layer 0 --src-x 22 --src-y 45 --width 6 --height 6 --dst-x 36 --dst-y 45
pixelforge layer add --project hero.json --name gear
pixelforge fill-rect --project hero.json --layer 1 --x 23 --y 14 --width 18 --height 10 --color-index 4
~~~

### Edit only the helmet { #edit-only-the-helmet }

Duplicate the pose, select its copy, then recolor the helmet. Frame duplication keeps the original selected, so select frame 1 explicitly. The selection is shared across frames and persists in the project.

~~~bash
pixelforge frame duplicate --project hero.json --index 0
pixelforge frame select --project hero.json --index 1
pixelforge selection set --project hero.json --x 23 --y 14 --width 18 --height 10
pixelforge replace-index --project hero.json --layer 1 --from-index 4 --to-index 6
~~~

### Compare the selected area { #diff-diff-only-the-helmet }

~~~bash
pixelforge diff --project hero.json --frame 0 --with 1 --layer 1 --selection --scale 4
pixelforge selection clear --project hero.json
~~~

The comparison reports 180 changed cells on the gear layer. The output PNG shows the two frames and an onion-skin comparison.

### Protect the boots while recoloring { #mask-fill-subtract-the-feet-then-fill }

~~~bash
pixelforge selection set --project hero.json --x 22 --y 24 --width 20 --height 27
pixelforge mask subtract --project hero.json --x 22 --y 45 --width 20 --height 6
pixelforge replace-index --project hero.json --layer 0 --from-index 3 --to-index 5
pixelforge selection clear --project hero.json
~~~

### Undo one edit { #a-stroke-as-one-undo }

~~~bash
pixelforge paint --project hero.json --checkpoint hero.ckpt.json --layer 1 --x 32 --y 16 --color-index 2
pixelforge undo --project hero.json --checkpoint hero.ckpt.json
~~~

For a continuous stroke with several tool requests, send `beginGesture` and `endGesture` around the edits in the same `call`. See the [Tool API](tool-api.md#begingesture-endgesture).

### Time and export the animation { #frames-and-export }

~~~bash
pixelforge frame delay --project hero.json --index 1 --delay-ms 160
pixelforge tag add --project hero.json --name color_cycle --from 0 --to 1 --loop forward
pixelforge export sheet --project hero.json --cols 2 --output hero-sheet.png --atlas hero-sheet.json
pixelforge validate-atlas --atlas hero-sheet.json
pixelforge export apng --project hero.json --output hero.apng --tag color_cycle
~~~

Give the engine the sheet PNG and its atlas JSON. Open `hero.apng` to review the two poses at their authored delays. See [engine import settings](export.md).

## More editing operations { #workflow-skills }

### Work with parts and anchors { #parts-and-anchors }

A part defines a named origin on the canvas and can be attached to a layer. An anchor identifies a local attachment point, such as a hinge or grip. Define a curve to change rotation, translation or flip state by frame. Use `bake` to write the sampled poses.

~~~bash
pixelforge part add --project hero.json --name helmet --layer 1 --x 32 --y 24
pixelforge part anchor set --project hero.json --part helmet --name hinge --x 0 --y 0
pixelforge part list --project hero.json
~~~

### Manage a palette { #palette }

Use `palette add` to obtain a new index. `palette import` accepts GPL or hexadecimal color lists with `--mode merge` or `--mode replace`. Use `palette remap` for the whole project and `replace-index` to edit a selected region. `palette apply-png` maps a same-size image to the current palette.

### Convert a reference PNG { #png-convert-a-png }

Use the sheet from the previous example as input. The following command samples it onto a new 64 × 32 grid and limits the palette to eight opaque colors.

~~~bash
pixelforge engine list
pixelforge convert --file hero-sheet.png --width 64 --height 32 --engine center-sample --palette-strategy limit --max-colors 8 --output study.json
~~~

### Inspect project data and call tools { #inspect-and-raw-calls }

~~~bash
pixelforge scene --project hero.json
pixelforge inspect --project hero.json
pixelforge tools
pixelforge call --project hero.json --request '{"tool":"scene"}'
~~~

`scene` returns dimensions, palette, layers, frame timing and the revision used for guarded edits. `inspect` also returns pixel indices. For the JSON request and response formats, see the [Tool API](tool-api.md).

### Assemble a tilemap { #tileset-autotile-tilemap }

Each frame holds one tile. Its width and height can each be 16 or 32 pixels and must match the project canvas. Tile IDs equal frame indices. Declare the tile size before creating the map. These commands build a separate, single-tile project.

~~~bash
pixelforge new --width 16 --height 16 --output tiles.json
pixelforge tileset set --project tiles.json --tile-width 16 --tile-height 16
pixelforge fill-rect --project tiles.json --x 0 --y 0 --width 16 --height 16 --color-index 3
pixelforge tilemap new --width 8 --height 6 --tile-width 16 --tile-height 16 --tileset tiles.json --output map.json
pixelforge tilemap fill --map map.json --name ground --x 0 --y 0 --width 8 --height 6 --tile 0
pixelforge validate-tiles --project tiles.json --map map.json
pixelforge export tileset --project tiles.json --output tiles.png --meta tileset.meta.json --image tiles.png
pixelforge export tilemap --map map.json --tileset tiles.json --output map.png --tmj base.tmj --image tiles.png
~~~

### Register related assets { #project-manifest }

When several assets share a palette, canvas size or animation convention, record their paths and dependencies in a [project manifest](project-manifest.md). Use `character matrix` to see which action and facing pairs are present.

### Build a silhouette { #pixelforge-draw }

Start with one opaque color and check whether the subject reads at game scale. Add the large masses of clothing, skin or machinery before small details. Compare each pass with `preview --scale 4`.

### Reuse a pose { #shortest-edit-path }

Duplicate the closest pose and change the areas that move. For an occupied frame slot, use `copy-rect --from-frame … --to-frame …`. Use `diff` to check that stationary details stayed in place.

### Build a directional walk cycle { #pixelforge-sprite-4dir }

Arrange the rows down, left, right and up. In each row put the idle pose first and walk poses in the remaining columns. Set `--cols` to the number of frames per row when exporting. Check foot contact at game scale. [Four-direction example](showcase.md#port-keeper)

### Keep equipment editable { #pixelforge-layers }

Keep clothing and equipment on layers above the body. Use `preview --layer` to inspect a layer and a composite preview to check its overlap. Bind a polish layer to a part if corrections should travel with that part during baking.

### Review an animation { #pixelforge-critique }

Ask for the frame or tag where a problem appears, a description of what moves incorrectly and one proposed correction. Open the animation at its original speed to inspect weight shifts, foot contact and the motion of attached items.

### Study action timing { #pixel-art-craft }

Read `references/action-motion.md` in the bundled craft skill to plan anticipation, contact, recovery and follow-through. Use the [Nezha animation](showcase.md#nezha) to inspect these stages frame by frame.

## Recover from an edit conflict { #known-limitations }

If another process has changed the project, a guarded edit returns `RevisionConflict`. Read `scene` again and plan the change against the new revision. For checkpoint mismatches, write conflicts and supported filesystem behavior, see [editing and export limits](known-limits.md).
