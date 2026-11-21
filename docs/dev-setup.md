# Development setup { #dev-setup }

Use this guide to build the CLI, run the web editor or contribute to PixelForge. For a ready-to-run binary, see [Install PixelForge](install.md).

## Prerequisites { #prerequisites }

| Component | Required tools |
| --- | --- |
| Rust kernel and CLI | Stable Rust with Cargo 1.83 or newer and a system linker |
| Web editor | Node.js 20 or 22 (tested in CI), pnpm 10 and rustup |
| Documentation | Python environment with `requirements-docs.txt` installed |

Windows source builds require Visual Studio Build Tools with the C++ desktop workload and a Windows 10/11 SDK. On macOS, install the Xcode Command Line Tools. The first web build downloads the WASM target and matching bindings.

## Source layout { #layout }

| Directory | Contents |
| --- | --- |
| `crates/pixelforge-kernel` | Indexed document, edit operations, serialization and export |
| `crates/pixelforge-cli` | Command parsing, filesystem operations and local player |
| `crates/pixelforge-kernel-wasm` | Bindings used by the TypeScript shell |
| `apps/web` | Web editor and optional model adapter |
| `skills` | Bundled agent skills |
| `docs` | English and Chinese documentation |

## Build and test the kernel { #rust }

Run these commands from the repository root.

```bash
cargo check --workspace --locked
cargo test --workspace --locked
```

## Run the CLI from source { #cli }

```bash
cargo run -p pixelforge-cli -- --help
cargo install --path crates/pixelforge-cli --locked
```

## Start the web editor { #_2 }

```bash
pnpm install --frozen-lockfile
pnpm dev
```

Open the URL printed by Vite. Use `pnpm typecheck` to check the TypeScript interface or `pnpm build` to build it for distribution.

### Rebuild the WASM bridge { #kernel-bridge }

The web commands build the WASM bridge automatically. Run `pnpm build:wasm` to build the web bindings alone. `pnpm test` builds separate Node bindings and runs integration tests.

<span id="issue-26"></span>

## Run pre-merge checks { #ci }

Run `pnpm lint` and `pnpm smoke`. For browser checks, install Chromium with `pnpm exec playwright install chromium` and follow [testing and benchmarks](quality.md).

To preview documentation changes, run:

```bash
pip install -r requirements-docs.txt
mkdocs serve
```

<span id="_1"></span>

## Release packaging { #prebuilt-releases }

[cargo-dist configuration](https://github.com/starcloud530/pixelforge-ai/blob/main/dist-workspace.toml) defines the prebuilt archives and installers. Install cargo-dist 0.33.1 and run `dist plan` to inspect the planned artifacts. The release tag must match `v` followed by the root Cargo workspace version. Published files and checksums are listed under [Releases](https://github.com/starcloud530/pixelforge-ai/releases).

<span id="_3"></span>

## Read a project file { #project-file }

Version 1 uses `"format": "pixelforge-project"`. The kernel serializes and validates canvas size, palette, frame data and edit state. Legacy frames without `delayMs` load with 100 ms. A missing `paletteLocked` defaults to false. Use `saveProject` and `loadProject` through the [Tool API](tool-api.md) to round-trip the supported format.

## Import palette files { #palette-files }

Use a GIMP palette (`.gpl`) or one six-digit RGB color per line, optionally prefixed with `#`. Replace mode swaps the editable entries. Merge mode appends new opaque colors. Index 0 stays transparent.
