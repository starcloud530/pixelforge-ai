# Project manifest reference { #project-manifest }

The version 1 `pixelforge-manifest` format records an asset collection’s canvas sizes, perspective, shared palette, directory layout and registered dependencies. Use it to validate a delivery or find the source clips that depend on a character. The editable frames and layers stay in their `pixelforge-project` files.

## Choose an asset root { #out-of-scope }

Place the manifest in or above the directory that contains the collection. All registered files and the shared palette use paths relative to `assetRoot`. Related records may point to one file, for example a character and its walk clip. Each registration has a separate, stable ID. [Port Keeper manifest](https://github.com/starcloud530/pixelforge-demos/blob/main/demos/demo-zombie/pixelforge.manifest.json)

## Manifest fields { #format }

`format` must be `pixelforge-manifest` and `version` must be 1. Unknown fields cause a `ManifestFile` error and exit code 2.

| Field | Requirement |
| --- | --- |
| `name` | Project name, 1–64 characters after trimming |
| `note` | Style or workflow note, 1–400 characters. Control characters are invalid in both text fields. |
| `perspective` | `top`, `side`, `front` or `oblique` |
| `assetRoot` | Directory relative to the manifest. Use `.` for the containing directory. |
| `palette` | Required relative `path` and optional display label `ref`. JSON must be a PixelForge pixel project. For GPL, HEX or TXT, validation checks file presence. |
| `canvases` | 1–32 named presets with integer `width` and `height` between 1 and 256 |
| `layout` | 1–64 directory rules, each with a unique `name` and `dir`, a `kind` and an optional `canvas` preset name |
| `assets` | Optional, up to 256 registry entries. An omitted array and an empty array are equivalent. |

Directory kinds are `character`, `prop`, `tile`, `portrait`, `ui` and `effect`. A rule with a canvas preset validates pixel projects directly inside its directory. Atlas JSON and unrelated formats are skipped. Paths use forward slashes. Except for `assetRoot: "."`, avoid absolute paths, `.`, `..`, empty segments, colons and backslashes. Symlinks are resolved by the filesystem.

## Asset registry fields { #asset-registry }

| Field | Meaning |
| --- | --- |
| `id` | Unique lookup key, 1–64 characters. Begin with A–Z or a–z, then use ASCII letters or digits, dots, underscores or hyphens. Dotted segments must be nonempty. |
| `kind` | `character`, `group`, `gear`, `prop`, `tile`, `clip` or `sheet` |
| `path` | Asset file relative to the asset root. Multiple IDs can share a file. |
| `owner` | Optional parent ID. Must be a registered character or group. Ownership chains must be acyclic. |
| `dependsOn` | Optional dependency IDs. All must exist when validating. Cycles are rejected. |
| `produces` | Optional output IDs. Each target must list this source in `dependsOn`. |
| `tags` | Optional labels for filtering. Each label is unique after trimming, 1–64 characters and free of control characters. |

To change a record, use `asset set`. Use `--clear-owner`, `--clear-depends-on`, `--clear-produces` or `--clear-tags` to remove optional values. Before removing a record, update any referring owners, dependencies or outputs. Manifest edits have no undo stack.

## Create and validate a registry { #commands }

Extract the [Port Keeper download](showcase.md#port-keeper). Run the commands from the directory containing `a/keeper.pxf.json`. They create a new manifest named `keeper-registry.json` and register two clips against the existing character.

~~~bash
pixelforge project init --name harbor --perspective top --note "64x64 character, four directions" --canvas sprite:64x64 --palette a/keeper.pxf.json --palette-ref keeper --layout characters:a:character:sprite --output keeper-registry.json
pixelforge asset add --manifest keeper-registry.json --id keeper --kind character --path a/keeper.pxf.json
pixelforge asset add --manifest keeper-registry.json --id keeper.idle --kind clip --path a/keeper.pxf.json --owner keeper --depends-on keeper --tag idle_down --tag idle_left --tag idle_right --tag idle_up
pixelforge asset add --manifest keeper-registry.json --id keeper.walk --kind clip --path a/keeper.pxf.json --owner keeper --depends-on keeper --tag walk_down --tag walk_left --tag walk_right --tag walk_up
pixelforge project show --manifest keeper-registry.json
pixelforge asset list --manifest keeper-registry.json --kind clip
pixelforge asset graph --manifest keeper-registry.json keeper.walk
pixelforge asset dependents --manifest keeper-registry.json keeper
pixelforge project validate --manifest keeper-registry.json
pixelforge asset validate --manifest keeper-registry.json
pixelforge character matrix --manifest keeper-registry.json keeper
pixelforge character check --manifest keeper-registry.json keeper
~~~

`project validate` checks project conventions and the asset registry. `asset validate` limits the check to registered files and relationships. Both return a JSON report. Exit 0 permits warnings, exit 1 means there are validation errors and exit 2 means the input could not be read or parsed. Inspect `errorCount` even when the report itself has `ok: true`.

`asset graph` shows one hop of relationships. `asset dependents` returns the records whose `dependsOn` names the selected ID. `character matrix` opens the character and compares direction layout, registered clip tags and authored frames. It reports present, missing, empty or placeholder segments. `character check` exits 1 if the matrix contains errors. Play the resulting clips to inspect motion.

| Finding | Check first |
| --- | --- |
| `assetRoot`, `layoutMissing`, `layoutKind`, `layoutEmpty` | Root and layout directories exist, have the correct type and contain the expected files. An empty layout is a warning. |
| `paletteMissing`, `paletteFormat`, `canvasSize`, `canvasUnread` | Palette file format and project dimensions match the manifest. |
| `assetMissing`, `assetPath`, `assetRef`, `assetOwner` | Registered paths point to files, referenced IDs exist, and owners are eligible. |
| `assetProduces`, `assetCycle` | Products list their source dependencies, and the dependency graph is acyclic. |

## Programmatic access { #tool-api }

`pixelforge tools` lists manifest operations under `manifestTools`. Using a manifest as `call --project FILE` selects this interface. It supports `inspectManifest`, `validateManifest`, `validateAssets`, `characterMatrix`, `addAsset`, `setAsset`, `removeAsset`, `listAssets`, `showAsset`, `graphAsset` and `dependentsAsset`. Manifest calls reject `--checkpoint` and `--preview`. [Tool API](tool-api.md)

Raw validation tools do not access the filesystem. Supply `rootExists` and `files` with observations for each checked path. Probe kinds are `file`, `dir` and `missing`. Give `width` and `height` together for a pixel project. A missing observation returns `probeMissing`. The CLI validation commands collect these observations for you.
