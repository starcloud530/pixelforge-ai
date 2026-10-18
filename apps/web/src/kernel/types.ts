/** One palette entry. Index 0 is transparent, matching `Rgba` in the Rust kernel. */
export interface Rgba {
  r: number;
  g: number;
  b: number;
  a: number;
}

/**
 * Read-only view of the kernel document.
 * `pixels` are palette indices, row-major, length `width * height`.
 */
export interface KernelSnapshot {
  width: number;
  height: number;
  palette: readonly Rgba[];
  frameCount: number;
  activeFrame: number;
  pixels: Uint8Array;
}

/**
 * Operations the shell is allowed to ask of the kernel.
 * The Rust crate owns the real rules. The web app currently calls a JS
 * stand-in with the same paint/erase checks. See `docs/dev-setup.md`.
 */
export interface PixelKernel {
  snapshot(): KernelSnapshot;
  paint(x: number, y: number, colorIndex: number): void;
  erase(x: number, y: number): void;
}

export class KernelError extends Error {
  readonly code: "InvalidSize" | "OutOfBounds" | "PaletteIndex";

  constructor(code: KernelError["code"], message: string) {
    super(message);
    this.name = "KernelError";
    this.code = code;
  }
}
