# Case study · Load a lighthouse into a tilemap { #111 }

This worked example places a 32×64 lighthouse on an exported orthogonal tilemap. Use it to inspect two integration details: how translucent layers flatten to a palette and how a Tiled map resolves its tileset. The [archived example source](https://github.com/starcloud530/pixelforge-ai/tree/8fe8c02/examples/lighthouse-outpost) includes the drawing script, player and exported images.

<span id="112-already-snaps-the-blend-the-useful-next-step-is-a-thin-beforeafter-preview-not-another-drawing-api-and-not-a-map-gui"></span>

## Compare the beam before and after flattening { #_1 }

The source has three layers: masonry, lantern and beam. Its 11-color palette includes two stone colors. The beam layer has opacity 128.

![Lighthouse before flattening, with the beam translucent over empty ground](https://raw.githubusercontent.com/starcloud530/pixelforge-ai/8fe8c02/examples/lighthouse-outpost/lighthouse-before.preview.png)

![Lighthouse after flattening, with the beam mapped to solid palette colors](https://raw.githubusercontent.com/starcloud530/pixelforge-ai/8fe8c02/examples/lighthouse-outpost/lighthouse.preview.png)

| Measurement at opacity 128 | Result after flattening |
| --- | --- |
| 213 partially transparent cells | 0 remain |
| 28 cells change palette index | 24 map from gold to stone, 4 from gold to orange |
| Gold over empty ground | Opaque gold |

At opacity 127 the same 213 translucent cells become index 0. To see this threshold in your own project, run `pixelforge preview --project FILE --compare` before flattening.

## Open the scene { #_2 }

Download the [source snapshot](https://github.com/starcloud530/pixelforge-ai/archive/8fe8c02.tar.gz) and extract it. In the directory containing `examples/`, run:

```bash
python3 -m http.server 8000 --bind 127.0.0.1
```

Open `http://127.0.0.1:8000/examples/lighthouse-outpost/play.html`. Use the arrow keys or WASD to move the keeper. Append `?sprite=before` to display the unflattened lighthouse.

<span id="answers"></span>

## How the loader places tiles { #_3 }

The loader reads `base.tmj`, resolves the tileset image relative to the map and disables image smoothing. The 10×8 room uses 17 tiles. Occupied wall cells block movement. Sprite coordinates, collision rules and flip handling belong to the loader. The [base tiles](https://github.com/starcloud530/pixelforge-ai/tree/8fe8c02/examples/base-tiles) and [loader code](https://github.com/starcloud530/pixelforge-ai/blob/8fe8c02/examples/lighthouse-outpost/play.js) can be inspected independently.

## Drawing cost and scope { #_4 }

The archived build draws the tower with 25 requests in one CLI batch. The recorded 0.22 seconds is the total for 22 CLI commands after compilation, including that batch. It excludes authoring and visual review. The loader supports one orthogonal 16×16 tileset with horizontal and vertical flips. For a game-engine project, continue with [engine import](../export.md).
