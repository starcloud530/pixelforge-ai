# 软件与素材许可 { #dependency-licenses }

| 使用内容 | 适用许可 |
| --- | --- |
| PixelForge 软件，包括 CLI 和内核 | 可选择 MIT 或 Apache-2.0。参见[软件许可](https://github.com/starcloud530/pixelforge-ai/blob/main/LICENSE)、[MIT 全文](https://github.com/starcloud530/pixelforge-ai/blob/main/LICENSE-MIT)和 [Apache-2.0 全文](https://github.com/starcloud530/pixelforge-ai/blob/main/LICENSE-APACHE)。 |
| 已发布的示例像素工程、精灵表、图块和预览图 | CC BY 4.0，署名 PixelForge / starcloud530。随附制作脚本使用软件许可。[示例素材许可](https://github.com/starcloud530/pixelforge-demos/blob/main/LICENSE-ASSETS.md) |

## 给示例素材署名 { #_1 }

分发或改编示例美术时，请标明创作者、来源和许可，并说明所作修改。例如，在游戏中使用了改色后的港口守灯人，可在制作人员名单中写入以下条目。[CC BY 4.0 条款](https://creativecommons.org/licenses/by/4.0/deed.zh-hans)

> [港口守灯人](https://starcloud530.github.io/pixelforge-demos/zh/gallery/#port-keeper)，PixelForge / starcloud530 创作，AI 辅助，采用 [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/deed.zh-hans) 许可。本游戏调整了披风配色。

## 直接依赖许可 { #_2 }

供重新分发软件的团队参考。下表记录 2026 年 10 月 27 日核对的依赖版本与许可字段；Rust 文件锁相关依赖于 2026 年 11 月 14 日补充核对。若分发版本的依赖已变化，应按对应版本的锁文件检查。[Cargo 锁文件](https://github.com/starcloud530/pixelforge-ai/blob/main/Cargo.lock) · [pnpm 锁文件](https://github.com/starcloud530/pixelforge-ai/blob/main/pnpm-lock.yaml)

### Rust 包 { #rust-packages }

| Rust 包 | 锁定版本 | 许可证 |
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

### 网页与构建工具 { #web-and-build-tools }

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

## 间接依赖许可 { #_3 }

以下 Rust 间接依赖在所核对的依赖树中包含额外许可选项、声明或特定平台版本。

| Rust 包 | 锁定版本 | 许可证 |
| --- | --- | --- |
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 |
| bitflags | 2.13.2 | MIT OR Apache-2.0 |
| errno | 0.3.15 | MIT OR Apache-2.0 |
| linux-raw-sys | 0.9.4 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| rustix | 1.0.8 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| windows-link | 0.2.1 | MIT OR Apache-2.0 |
| windows-sys | 0.60.2、0.61.2 | MIT OR Apache-2.0 |
| memchr | 2.8.3 | Unlicense OR MIT |
| miniz_oxide | 0.8.9、0.9.1 | MIT OR Zlib OR Apache-2.0 |
| simd-adler32 | 0.3.10 | MIT |
| strsim | 0.11.1 | MIT |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| zlib-rs | 0.6.8 | Zlib |
| zmij | 1.0.23 | MIT |

`unicode-ident` 的 Unicode 数据还适用 Unicode-3.0 条款。

已核对的 pnpm 锁文件包含 156 个第三方包条目，其中 141 项登记为 MIT、2 项登记为 Apache-2.0、1 项登记为 MIT OR Apache-2.0。其余条目如下。

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

`caniuse-lite` 为网页构建工具提供浏览器兼容性数据。重新分发该数据时请保留相应署名。
