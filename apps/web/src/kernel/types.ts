/** One palette entry. Index 0 is transparent, matching `Rgba` in the Rust kernel. */
export interface Rgba {
  r: number;
  g: number;
  b: number;
  a: number;
}

/** Matches `DEFAULT_FRAME_DELAY_MS` in the Rust kernel. */
export const DEFAULT_FRAME_DELAY_MS = 100;
/** Matches `MIN_FRAME_DELAY_MS` in the Rust kernel. */
export const MIN_FRAME_DELAY_MS = 1;
/** Matches `MAX_FRAME_DELAY_MS` in the Rust kernel. */
export const MAX_FRAME_DELAY_MS = 65_535;
/**
 * Matches `TIMELINE_RATIO_PPM` in the Rust kernel.
 * `0` is the start of the timeline and this value is the end of the last frame.
 */
export const TIMELINE_RATIO_PPM = 1_000_000;

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
  /** Per-frame delay in milliseconds, timeline order. */
  frameDelays: readonly number[];
  /** True while loop playback is advancing the active frame. */
  playing: boolean;
  /**
   * True when recolor, append, and palette import are refused.
   * Paint still writes existing indices. Stored in the project file.
   */
  paletteLocked: boolean;
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
  /**
   * Locks or unlocks palette edits. Painting still uses current indices.
   * The same value is not recorded. The flag is stored in the project file.
   */
  setPaletteLocked(locked: boolean): void;
  /**
   * Imports a GIMP palette (`.gpl`) or a hex list (`RRGGBB` / `#RRGGBB` per line).
   * `replace` swaps the paint colors and clears indices that no longer exist.
   * `merge` appends colors the palette does not already have.
   * Index 0 stays transparent. Either change is one undo step.
   * A locked palette rejects the import.
   */
  importPalette(text: string, mode: PaletteImportMode): void;
  addFrame(): number;
  duplicateFrame(index: number): number;
  deleteFrame(index: number): void;
  setActiveFrame(index: number): void;
  /** Sets one frame's on-screen duration in milliseconds. */
  setFrameDelay(index: number, delayMs: number): void;
  /** Starts loop playback at the beginning of the active frame. */
  play(): void;
  /** Stops loop playback. */
  stop(): void;
  playing(): boolean;
  /**
   * Advances the kernel playback clock by `deltaMs`.
   * Returns whether the active frame changed. The change is not an undo entry.
   */
  tick(deltaMs: number): boolean;
  /**
   * Stops playback and selects the frame under `ratioPpm` (0..=TIMELINE_RATIO_PPM).
   * A drag should be one gesture so the selection is one undo step.
   */
  scrub(ratioPpm: number): void;
  beginGesture(): void;
  endGesture(): void;
  gestureOpen(): boolean;
  undo(): void;
  redo(): void;
  canUndo(): boolean;
  canRedo(): boolean;
  /** Version 1 `pixelforge-project` JSON. The kernel owns the bytes. */
  saveProject(): string;
  /**
   * Packed RGBA for one frame, row-major, length `width * height * 4`.
   * Same colors as [`PixelKernel.exportFramePng`]. The shell composites onion
   * skin from this buffer. Reading it does not change pixels, the project, or export.
   */
  frameRgba(index: number): Uint8Array;
  /**
   * RGBA PNG for one frame, timeline order. Index 0 is transparent.
   * The kernel owns the encoding. The shell only downloads the bytes.
   */
  exportFramePng(index: number): Uint8Array;
  /** RGBA PNG of the grid sprite sheet. The kernel owns the packing. */
  exportSpriteSheet(): Uint8Array;
  /** Columns in the sprite sheet: `ceil(sqrt(frameCount))`. */
  spriteSheetColumns(): number;
  /** Rows in the sprite sheet: `ceil(frameCount / columns)`. */
  spriteSheetRows(): number;
  /**
   * One Tool API call on this same document.
   * `request` and the return value are JSON. A refused tool stays in the body
   * (`ok: false`) and is not thrown. See `docs/tool-api.md`.
   */
  callTool(request: string): string;
}

/** `replace` swaps paint colors. `merge` appends colors that are not already present. */
export type PaletteImportMode = "replace" | "merge";

export type KernelErrorCode =
  | "InvalidSize"
  | "OutOfBounds"
  | "PaletteIndex"
  | "EmptyPalette"
  | "PaletteTooLarge"
  | "TransparentSlot"
  | "PaletteLocked"
  | "PaletteFile"
  | "PaletteImportMode"
  | "FrameIndex"
  | "LastFrame"
  | "FrameDelay"
  | "NothingToUndo"
  | "NothingToRedo"
  | "OpenGesture"
  | "ProjectFile";

const KERNEL_ERROR_CODES: readonly KernelErrorCode[] = [
  "InvalidSize",
  "OutOfBounds",
  "PaletteIndex",
  "EmptyPalette",
  "PaletteTooLarge",
  "TransparentSlot",
  "PaletteLocked",
  "PaletteFile",
  "PaletteImportMode",
  "FrameIndex",
  "LastFrame",
  "FrameDelay",
  "NothingToUndo",
  "NothingToRedo",
  "OpenGesture",
  "ProjectFile",
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
