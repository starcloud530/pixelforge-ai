# Testing and benchmarks { #_1 }

## Run the checks { #_2 }

Follow [development setup](dev-setup.md) for the toolchain. Local checks require Python 3.9+ and CI uses 3.12. Run `pnpm install --frozen-lockfile` using the pinned pnpm 10 version.

| Command | Coverage |
| --- | --- |
| `pnpm lint` | Rust formatting, strict Clippy, TypeScript for web and landing pages, offline documentation checks |
| `pnpm quick` | CLI build, eight basic calls, player JavaScript tests and documentation consistency |
| `pnpm smoke` | CLI build, 25 calls including failure cases and unchanged-file checks, map/atlas/registry fixtures |
| `cargo test --workspace --locked --no-fail-fast` | Rust workspace, concurrent edits, revisions, conversion, read-only player and output protection |
| `pnpm test` | Node WASM build and tests for one agent turn and its proxy using a mock provider |
| `pnpm build` / `pnpm build:landing` | Web/WASM and landing builds with TypeScript checking |
| `pnpm test:browser` | Chromium on desktop and narrow screens, keyboard use, error recovery, read-only mode and polling |
| `pnpm check:docs` / `mkdocs build --strict` | Local links, tool coverage, site entry points and documentation builds |

On Windows, invoke `python scripts/checks.py` with `lint`, `quick` or `smoke` if `python3` is unavailable. The `--no-build` option expects a CLI built from the current sources. Keep per-call JSON with `--artifacts DIR`.

Before browser tests, run `cargo build -p pixelforge-cli --locked` and `pnpm exec playwright install chromium`. Linux CI uses `--with-deps`. Set `PIXELFORGE_BROWSER_EXECUTABLE` for an existing Chrome installation or `PIXELFORGE_TEST_ARTIFACTS` to keep screenshots. Test projects are created in temporary directories and drawn through the CLI.

The CI check named `test and export gate` combines Rust tests on Linux, macOS and Windows, Rust 1.83, Node 20/22, web/WASM, browser and docs jobs. A failed, skipped or canceled dependency fails the gate.

## Compatibility checks { #_3 }

The Rust workspace supports Rust 1.83 and later. CI runs Clippy and exercises CLI startup, command execution and concurrent editing on Windows. The Windows CLI reserves an 8 MiB main-thread stack.

## Output and failure behavior { #_4 }

Preview, diff and export preflight all destination paths. A destination cannot alias inputs, checkpoints, locks or another output. Destination symlinks and hard-link aliases are rejected with `OutputConflict` and exit 2 before writing the first file.

Lock identity uses the canonical parent and stable filename. This avoids relying on deleted NTFS handles during atomic replacement. Windows CI repeats concurrent-edit regression tests 20 times.

Each file uses a temporary file and atomic replacement. Multi-file products such as a PNG and atlas, map and TMJ, PNG sequences or project and checkpoint publish individually. A later failure can leave an earlier file published.

## Measure performance { #_5 }

```bash
pnpm perf --output /tmp/pixelforge-performance.json
pnpm perf --source /path/to/reference.png --output /tmp/pixelforge-performance.json
```

The default release build measures 32×32/8-frame, 64×64/16-frame and 256×256/32-frame fixtures. It times scene, preview, sheet and APNG operations, tracks child peak RSS and measures player startup and edit-to-refresh latency. An optional high-color image exercises five conversion engines.

The default is three runs per measurement with one warm-up each for scene, preview, sheet and APNG. Results include raw samples, median and nearest-rank p95. RSS units are normalized across macOS and Linux. Systems without `wait4` report null. Player refresh includes edit and polling latency. Peak RSS for the long-running player is not collected.

For comparisons, hold sources, build profile, input hashes, hardware and sample count constant. Absolute time is machine-specific. High-color images, long animations, many layers and extreme files need separate measurements.
