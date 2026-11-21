# Software and artwork licenses { #dependency-licenses }

| What you are using | License |
| --- | --- |
| PixelForge software, including the CLI and kernel | Choose MIT or Apache-2.0. See the [software license](https://github.com/starcloud530/pixelforge-ai/blob/main/LICENSE), [MIT text](https://github.com/starcloud530/pixelforge-ai/blob/main/LICENSE-MIT) and [Apache-2.0 text](https://github.com/starcloud530/pixelforge-ai/blob/main/LICENSE-APACHE). |
| Published example pixel projects, sheets, tiles and previews | CC BY 4.0. Credit PixelForge / starcloud530. Scripts shipped with those assets use the software license. [Example artwork license](https://github.com/starcloud530/pixelforge-demos/blob/main/LICENSE-ASSETS.md) |

## Credit example artwork { #_1 }

When sharing or adapting sample artwork, credit the creator, link to the source and license, and indicate changes you made. For example, a game that uses an edited Port Keeper sprite could include this entry in its credits. [CC BY 4.0 terms](https://creativecommons.org/licenses/by/4.0/)

> [Port Keeper](https://starcloud530.github.io/pixelforge-demos/gallery/#port-keeper) by PixelForge / starcloud530. AI-assisted artwork, licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Cape palette adapted for this game.

## Direct dependency licenses { #_2 }

For teams redistributing the software, the tables below record the license fields of the dependency versions reviewed on October 27, 2026. The Rust file-lock dependencies were added to this review on November 14, 2026. Check the lockfiles for the release you distribute if its dependency versions differ. [Cargo lockfile](https://github.com/starcloud530/pixelforge-ai/blob/main/Cargo.lock) · [pnpm lockfile](https://github.com/starcloud530/pixelforge-ai/blob/main/pnpm-lock.yaml)

### Rust packages { #rust-packages }

| Crate | Locked version | License |
| --- | --- | --- |
| base64 | 0.22.1 | MIT OR Apache-2.0 |
| clap | 4.5.23 | MIT OR Apache-2.0 |
| fs4 | 1.1.0 | MIT OR Apache-2.0 |
| libc | 0.2.169 | MIT OR Apache-2.0 |
| console_error_panic_hook | 0.1.7 | Apache-2.0/MIT |
| flate2 | 1.1.10 | MIT OR Apache-2.0 |
| png | 0.17.16 | MIT OR Apache-2.0 |
| serde | 1.0.229 | MIT OR Apache-2.0 |
| serde_json | 1.0.154 | MIT OR Apache-2.0 |
| wasm-bindgen | 0.2.129 | MIT OR Apache-2.0 |

### Web and build tools { #web-and-build-tools }

| Package | Locked version | License |
| --- | --- | --- |
| wasm-pack | 0.15.0 | MIT OR Apache-2.0 |
| react | 19.3.1 | MIT |
| react-dom | 19.3.1 | MIT |
| @types/node | 26.6.5 | MIT |
| @types/react | 19.3.0 | MIT |
| @types/react-dom | 19.3.0 | MIT |
| @vitejs/plugin-react | 4.7.0 | MIT |
| tsx | 4.23.15 | MIT |
| typescript | 5.9.3 | Apache-2.0 |
| vite | 6.4.4 | MIT |

## Transitive dependency licenses { #_3 }

The following Rust packages have additional license options, notices or platform-specific versions relevant to the reviewed dependency tree.

| Crate | Locked version | License |
| --- | --- | --- |
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 |
| bitflags | 2.13.2 | MIT OR Apache-2.0 |
| errno | 0.3.15 | MIT OR Apache-2.0 |
| linux-raw-sys | 0.9.4 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| rustix | 1.0.8 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| windows-link | 0.2.1 | MIT OR Apache-2.0 |
| windows-sys | 0.60.2, 0.61.2 | MIT OR Apache-2.0 |
| memchr | 2.8.3 | Unlicense OR MIT |
| miniz_oxide | 0.8.9, 0.9.1 | MIT OR Zlib OR Apache-2.0 |
| simd-adler32 | 0.3.10 | MIT |
| strsim | 0.11.1 | MIT |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| zlib-rs | 0.6.8 | Zlib |
| zmij | 1.0.23 | MIT |

`unicode-ident` also includes Unicode-3.0 terms for its Unicode data.

The audited pnpm lockfile contains 156 third-party package entries. Of these, 141 report MIT, two report Apache-2.0 and one reports MIT OR Apache-2.0. The remaining entries are listed below.

| Package | Locked version | License |
| --- | --- | --- |
| @isaacs/fs-minipass | 4.0.1 | ISC |
| electron-to-chromium | 1.5.459 | ISC |
| lru-cache | 5.1.1 | ISC |
| picocolors | 1.1.1 | ISC |
| semver | 6.3.1 | ISC |
| yallist | 3.1.1 | ISC |
| chownr | 3.0.0 | BlueOak-1.0.0 |
| minipass | 7.1.3 | BlueOak-1.0.0 |
| tar | 7.5.22 | BlueOak-1.0.0 |
| yallist | 5.0.0 | BlueOak-1.0.0 |
| source-map-js | 1.2.2 | BSD-3-Clause |
| caniuse-lite | 1.0.30001816 | CC-BY-4.0 |

`caniuse-lite` supplies browser compatibility data to the web build toolchain. Retain its applicable attribution when redistributing that data.
