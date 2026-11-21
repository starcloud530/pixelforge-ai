# Editing and export limits { #known-limits }

Use this reference when an edit is rejected, a baked pose looks different from its source, or a validated atlas looks wrong in a game. The sections below describe what each operation checks and how to inspect its result.

## Concurrent edits to one file { #same-project-edits-wait-they-are-not-merged }

A writer takes an exclusive lock before reading the project, tilemap or manifest. The default wait is 30 seconds. Set `PIXELFORGE_LOCK_TIMEOUT_MS` to change it, or use `0` for one immediate attempt. A timeout returns `LockTimeout` and exit code 2 before any requests are applied. Wait for the other writer to finish and read the latest state before retrying. Separate files can be edited concurrently.

Keep the `<filename>.pixelforge.lock` sidecar beside the project. The operating system releases the lock when the process exits. Deleting the sidecar can allow two writers to lock different files. Read-only commands such as `scene`, `preview` and `export` can read a complete snapshot while edits are in progress.

Local saves replace one file atomically. A symlink to an existing project resolves to its target. A dangling symlink, a symlinked lock file or a multiply linked project returns `UnsupportedPath`. Use a regular local file for editing. Network or cloud-synchronized filesystems and tools that bypass the lock have their own consistency behavior.

If an interruption occurs before replacement, the original file is intact. Remove a leftover temporary file only after confirming that no writer is using it. On Unix, a `DirectorySync` error occurs after replacement, so first read the project at its original path. Windows does not preserve a replaced file's ACL. Keep separate backups where filesystem recovery or permissions matter.

The project and its `--checkpoint` are saved separately. If the project was written and its checkpoint was interrupted, inspect the saved project. A subsequent `CheckpointMismatch` leaves both files unchanged. Preserve the old checkpoint for recovery and start a fresh checkpoint for the confirmed project state. In a batch, edits before the first failed request remain saved.

## Protect edits against stale revisions { #_1 }

Read `result.revision` from `scene` or `inspect` and pass it unchanged to `--expected-revision` when updating that snapshot. This token is a 16-digit lowercase hexadecimal fingerprint of the normalized project contents. The CLI checks it once after locking and before executing the first operation. Read-only operations and edits that leave the contents unchanged retain the revision.

| Response | What to do |
| --- | --- |
| `RevisionConflict` | The file changed. Read it again, review the new pose or metadata and issue a new edit using the current revision. |
| `RevisionInvalid` | Copy the complete revision from the response, including any leading zeros. |
| `RevisionUnsupported` | Omit the guard on commands creating a new project, such as `new`, `import-aseprite --output` or `convert --output`. |

For guarded map or manifest changes use `pixelforge call --project FILE --expected-revision REV`. The `tilemap` and `asset` subcommands do not accept this flag.

## Keep polish attached during baking { #unbound-polish-stays-on-canvas-cells }

An unbound `polish` layer stays at its painted canvas coordinates. Bind a polish layer to one part to move corrections and explicit erasures with that part during `bake`, translation, rotation or flipping. By default local (0, 0) is the part origin. Use `layer bind --anchor` to select an anchor instead. Clear an explicit erasure with `erase-rect --unmodified` to reveal the underlying pixels again. Keep manual touch-ups on a polish layer because baking rebuilds generated layers from the source frame.

## Inspect rotated edges { #rotation-sealing-is-lossy }

Rotating by a non-right angle resamples pixels onto the grid. Even after gap filling, a one-pixel line can break or shorten near 45°. Inspect `dropped`, `breaks` and the rendered silhouette. Touch up discontinuities at the sprite's intended display size. The same behavior applies to a bound polish layer.

## Place joints and foot contacts { #ik-places-joints-it-does-not-author-gait }

Inverse kinematics supports chains of two or three parts. Use `pin` or `plant` to hold a contact on the grid. Author the weight transfer and timing with key poses, then bake. Play the result to check the sole at contact, the height of the hips and the transition to the next step.

## Compare translucent layers before flattening { #translucent-flatten-snaps-to-nearest-index }

`preview --compare` shows the RGBA composite beside the indexed result that flattening would store. Flattening maps alpha below 128 to transparent index 0. An exact match to the covering pixel’s palette color retains its index. Otherwise, the nearest nonzero entry by squared RGB distance wins. Ties use the lower index. The output layer’s opacity is 255, so layer-only translucency disappears. Keep the layered source if transparency is part of the artwork. [Export and engine import](export.md#side-by-side)

## Check generated direction rows { #direction-fill-is-not-a-natural-gait }

`direction set` and `apply` arrange frame ranges and tags. `direction fill` copies cardinal rows or mirrors left and right. Existing ink and explicit erasures are preserved unless `--overwrite` is specified. Eight-direction layouts still require separately authored diagonal rows. Run `direction validate` to check the ranges, then play each walk cycle to inspect the lighting, profile and foot contact.

## Validate an asset registry { #project-manifest-is-memory-not-a-canvas }

`project validate` checks the configured paths, canvas conventions and registered relationships. `asset validate` checks registry entries. To inspect a character’s action coverage, run `character matrix` or `character check`. These distinguish present, missing, empty and placeholder segments. Open the character project to edit or play the pixels. [Project manifest reference](project-manifest.md)

## Verify an atlas in the target engine { #export-validate-is-not-aesthetic-qa }

`validate-atlas` checks the PNG and atlas JSON. Read warnings even when it exits 0. In Godot or Unity, verify the actual filter, grid slices, pivots, frame delays and tag mappings. Play the result against your game background. Use the APNG preview for timing review and the PNG sheet plus atlas JSON for import. [Atlas checks](export.md#validate-atlas)

## Inspect converted PNGs { #png-convert-is-a-sampler-not-a-quality-score }

`convert` samples an input PNG onto a target grid and applies a palette strategy. Inspect the silhouette, edge continuity and palette report at the target size. A low color error measures closeness to the sampled colors. Fine details may still need manual edits. Use `pixelforge engine list` to check engine availability. Parameters and ranges are listed under [convertPng](tool-api.md#convertpng).
