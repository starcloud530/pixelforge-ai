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
 * The Rust crate owns the rules, including the undo stack. The web app calls
 * them through the wasm-bindgen bridge. See `docs/dev-setup.md`.
 */
export interface PixelKernel {
  snapshot(): KernelSnapshot;
  paint(x: number, y: number, colorIndex: number): void;
  erase(x: number, y: number): void;
  /** Replaces the RGB of a paint entry. Index 0 cannot be changed. */
  setPaletteColor(index: number, r: number, g: number, b: number): void;
  /** Appends an opaque color and returns its index. */
  addPaletteColor(r: number, g: number, b: number): number;
  addFrame(): number;
  duplicateFrame(index: number): number;
  deleteFrame(index: number): void;
  setActiveFrame(index: number): void;
  beginGesture(): void;
  endGesture(): void;
  gestureOpen(): boolean;
  undo(): void;
  redo(): void;
  canUndo(): boolean;
  canRedo(): boolean;
}

export type KernelErrorCode =
  | "InvalidSize"
  | "OutOfBounds"
  | "PaletteIndex"
  | "EmptyPalette"
  | "PaletteTooLarge"
  | "TransparentSlot"
  | "FrameIndex"
  | "LastFrame"
  | "NothingToUndo"
  | "NothingToRedo"
  | "OpenGesture";

const KERNEL_ERROR_CODES: readonly KernelErrorCode[] = [
  "InvalidSize",
  "OutOfBounds",
  "PaletteIndex",
  "EmptyPalette",
  "PaletteTooLarge",
  "TransparentSlot",
  "FrameIndex",
  "LastFrame",
  "NothingToUndo",
  "NothingToRedo",
  "OpenGesture",
];

export class KernelError extends Error {
  readonly code: KernelErrorCode;

  constructor(code: KernelErrorCode, message: string) {
    super(message);
    this.name = "KernelError";
    this.code = code;
  }
}

/** Maps a `Code: message` failure from the wasm bridge onto `KernelError`. */
export function kernelErrorFromUnknown(error: unknown): KernelError | null {
  const message =
    error instanceof Error ? error.message : typeof error === "string" ? error : null;
  if (message === null) {
    return null;
  }
  const splitAt = message.indexOf(": ");
  if (splitAt === -1) {
    return null;
  }
  const code = message.slice(0, splitAt);
  if (!isKernelErrorCode(code)) {
    return null;
  }
  return new KernelError(code, message.slice(splitAt + 2));
}

function isKernelErrorCode(code: string): code is KernelErrorCode {
  return (KERNEL_ERROR_CODES as readonly string[]).includes(code);
}
