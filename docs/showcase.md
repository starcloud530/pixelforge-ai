# Verify a sample project { #_1 }

Open an editable project, export a fresh atlas and check its frame data. Use the Port Keeper to verify a four-direction character. Use Nezha to see how changing a single frame delay affects an action. [Install the CLI](install.md)

## Port Keeper · Verify a directional character { #port-keeper }

Download the project from the gallery and unzip it. Run the commands in the folder containing the `a` and `b` directories. The character uses a 64 × 64 canvas with five frames per direction. The storm outfit adds a cape on a separate layer. [View Port Keeper in the gallery](/pixelforge-demos/gallery/#port-keeper)

### Play the source project { #_2 }

~~~bash
pixelforge play --project a/keeper.pxf.json
~~~

Choose `walk_down` and step through its four frames. Their delays are 140, 110, 140 and 110 ms. Compare the foot positions against the standing pose in `idle_down`. Return to the terminal and press Ctrl+C to stop the viewer before continuing.

### Validate the atlas and export it again { #_3 }

~~~bash
pixelforge validate-atlas --atlas a/keeper-sheet.atlas.json
pixelforge export sheet --project a/keeper.pxf.json --cols 5 --output keeper-check.png --atlas keeper-check.atlas.json
pixelforge validate-atlas --atlas keeper-check.atlas.json
~~~

The first check reads the supplied export. The second checks your new one. Look for 20 frame rectangles in five columns, four directions and eight idle/walk tags. In `keeper-check.atlas.json` the `image` field should be `keeper-check.png`. Check the cape variant separately with `pixelforge validate-atlas --atlas b/keeper-storm-sheet.atlas.json`.

### Import the walk cycle { #_4 }

Use `keeper-check.png` as the texture and its JSON for frame rectangles, pivots if specified, tag ranges and timing. Check the silhouette and foot contact at your game's zoom and against its background. [Godot 4](export.md#godot-4) · [Unity](export.md#unity)

## Nezha · Review and change action timing { #nezha }

Download and extract the project. Run these commands from the directory containing `nezha-attack.pxf.json`. The original has 12 frames on a 128 × 128 canvas. One complete attack lasts 1,070 ms. [View Nezha in the gallery](/pixelforge-demos/gallery/#nezha)

### Inspect the motion { #_5 }

~~~bash
pixelforge play --project nezha-attack.pxf.json
~~~

Use `ready`, `charge`, `thrust` and `recover` to jump between phases. Pause at the end of the charge and step forward to inspect the front foot, spear and sash. Stop the viewer with Ctrl+C.

### Export a baseline { #_6 }

~~~bash
pixelforge export sheet --project nezha-attack.pxf.json --cols 6 --output nezha-check.png --atlas nezha-check.atlas.json
pixelforge export apng --project nezha-attack.pxf.json --output nezha-check.apng
pixelforge validate-atlas --atlas nezha-check.atlas.json
~~~

The six-column PNG contains two rows. The matching atlas stores the frame rectangles and timing. Open `nezha-check.apng` to review the original animation at its intended speed.

### Lengthen the final charge frame { #_7 }

Frame index 3 (the fourth frame) is the final charge pose. Change its delay from 60 to 120 ms, export a second preview, then undo the edit using the same checkpoint.

~~~bash
pixelforge frame delay --project nezha-attack.pxf.json --index 3 --delay-ms 120 --checkpoint timing.ckpt.json
pixelforge export apng --project nezha-attack.pxf.json --output nezha-long-charge.apng
pixelforge undo --project nezha-attack.pxf.json --checkpoint timing.ckpt.json
~~~

Play the two APNGs side by side. The longer charge adds 60 ms to the preparation and leaves the thrust frames and pixel artwork intact. The two previews remain available after undo. Both source projects are licensed under CC BY 4.0. [Artwork attribution](licensing.md)
