# Working with PixelForge projects { #vision-positioning }

A PixelForge project holds the editable artwork for a sprite or a tileset. Open a project to inspect or change its pixels, then export the files your game needs.

## Canvas and palette { #vision }

Every cell holds a palette index. Index 0 is transparent. Changing a palette color updates all cells that use it across the project. Lock the palette before editing if the set of colors must stay fixed.

## Layers and frames { #mission }

Put elements you expect to change independently on separate layers, such as a character and a cape. Each frame has a delay in milliseconds. Name a frame range to identify an action such as `walk_down`. The player can preview that action in isolation.

## Choose a project to explore { #what-we-build }

- [Port Keeper](/pixelforge-demos/gallery/#port-keeper) shows how one character uses separate outfits and four walking directions.
- [Tidal Glasshouse](/pixelforge-demos/gallery/#tidal-glasshouse) shows how individual tiles fit together into a layered map.
- [Tide Survey Kit](/pixelforge-demos/gallery/#tide-survey-icons) shows how a palette change affects eight inventory icons.

## Choose an output { #what-we-are-not }

| Output | Use it for |
| --- | --- |
| Editable project JSON | Keep the source palette, pixels, layers and timing for the next edit |
| APNG | Review the animation and its timing in a browser |
| PNG sheet and atlas JSON | Import frames, tag ranges and timing into a game |
| Tiled TMJ and tileset image | Load an orthogonal tilemap. Place game objects in your engine. |

## Start your first edit { #status }

[Create a blank project](install.md) or [open and re-export a sample character](showcase.md). To make changes through a coding agent, start with the [agent workflow](agent-skill.md).
