# 依赖许可证 Dependency licenses

本仓库自身以 MIT 许可证发布，全文在根目录 [`LICENSE`](../LICENSE)。版权所有 © 2026 starcloud530。

*This repository is MIT-licensed. The text is [`LICENSE`](../LICENSE). Copyright © 2026 starcloud530.*

## 结论

没有与 MIT 再分发冲突的依赖。核对范围内没有 GPL、AGPL、LGPL、MPL、SSPL 或其它 copyleft。出现过的许可证都是宽松许可：MIT、Apache-2.0、二者择一、0BSD、BSD-3-Clause、ISC、BlueOak-1.0.0、Zlib、Unlicense，以及下面单独点名的 Unicode-3.0 和 CC-BY-4.0。这些都不要求把本仓库改成 copyleft。

*No dependency in the checked set conflicts with MIT redistribution. There is no GPL, AGPL, LGPL, MPL, SSPL, or other copyleft. Every license found is permissive.*

核对日期：2026-10-27。版本取自当时的 `Cargo.lock` 与 `pnpm-lock.yaml`。Rust crate 的许可证字符串来自 crates.io 上该版本的 `license` 字段。npm 包来自 registry 上该版本的 `license` 字段。工作区自己的 crate（`pixelforge-kernel`、`pixelforge-kernel-wasm`、`pixelforge-cli`）不计入第三方。

*Checked on 2026-10-27 against `Cargo.lock` and `pnpm-lock.yaml`. License strings are the registry fields for those locked versions. Workspace crates are not third-party.*

## 直接依赖

`Cargo.toml` 里声明、并由 lockfile 解析的第三方 crate：

| Crate | 锁定版本 | 许可证 |
| --- | --- | --- |
| base64 | 0.22.1 | MIT OR Apache-2.0 |
| clap | 4.5.23 | MIT OR Apache-2.0 |
| console_error_panic_hook | 0.1.7 | Apache-2.0/MIT |
| flate2 | 1.1.10 | MIT OR Apache-2.0 |
| png | 0.17.16 | MIT OR Apache-2.0 |
| serde | 1.0.229 | MIT OR Apache-2.0 |
| serde_json | 1.0.154 | MIT OR Apache-2.0 |
| wasm-bindgen | 0.2.129 | MIT OR Apache-2.0 |

`flate2` 在清单里写的是 `1.0.35`，lockfile 解析到 `1.1.10`。上表用的是实际锁定版本。`Apache-2.0/MIT` 与 `MIT OR Apache-2.0` 是同一种双许可的不同写法。

根目录 `package.json` 与 `apps/web/package.json` 的直接依赖：

| 包 | 锁定版本 | 许可证 |
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

## 传递依赖

`Cargo.lock` 里其余第三方 crate 的许可证同样是 MIT、Apache-2.0，或 `MIT OR Apache-2.0`（`bitflags` 1.3.2 的登记写法是 `MIT/Apache-2.0`）。另外这几条仍是宽松许可，不是冲突：

| Crate | 锁定版本 | 许可证 |
| --- | --- | --- |
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 |
| memchr | 2.8.3 | Unlicense OR MIT |
| miniz_oxide | 0.8.9、0.9.1 | MIT OR Zlib OR Apache-2.0 |
| simd-adler32 | 0.3.10 | MIT |
| strsim | 0.11.1 | MIT |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| zlib-rs | 0.6.8 | Zlib |
| zmij | 1.0.23 | MIT |

`unicode-ident` 在 MIT 或 Apache-2.0 之外还要遵守 Unicode-3.0。那是 Unicode 数据的宽松许可，只约束这份数据，不把整个仓库变成 copyleft。

`pnpm-lock.yaml` 的 `packages` 段共 156 个第三方包条目。其中 141 个是 MIT，2 个是 Apache-2.0（`typescript` 5.9.3、`baseline-browser-mapping` 2.11.29），1 个是 `MIT OR Apache-2.0`（`wasm-pack` 0.15.0）。其余：

| 包 | 锁定版本 | 许可证 |
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

`caniuse-lite` 是 browserslist 用的数据，经 Vite 工具链进 lockfile，不是内核或 CLI 的运行时依赖。CC-BY-4.0 要求保留署名，不要求本仓库改用 CC，也不与 MIT 再分发冲突。BlueOak-1.0.0、ISC、BSD-3-Clause 都是宽松软件许可。

锁文件升级后应重跑这次核对。本页只记录当时登记的许可证字段，不是律师意见。
