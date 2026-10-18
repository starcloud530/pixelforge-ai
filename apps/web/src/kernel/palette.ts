import type { Rgba } from "./types";

/**
 * Bytes copied from `Palette::studio_default` in `crates/pixelforge-kernel`.
 * Index 0 stays transparent. Replace this module when the wasm bridge loads
 * the palette from Rust instead of duplicating it.
 */
export const STUDIO_PALETTE: readonly Rgba[] = [
  { r: 0, g: 0, b: 0, a: 0 },
  { r: 26, g: 28, b: 36, a: 255 },
  { r: 244, g: 241, b: 222, a: 255 },
  { r: 232, g: 93, b: 76, a: 255 },
  { r: 244, g: 162, b: 89, a: 255 },
  { r: 242, g: 204, b: 102, a: 255 },
  { r: 106, g: 176, b: 106, a: 255 },
  { r: 74, g: 140, b: 196, a: 255 },
  { r: 140, g: 110, b: 184, a: 255 },
];

export const TRANSPARENT_INDEX = 0;
export const MAX_CANVAS_EDGE = 256;
