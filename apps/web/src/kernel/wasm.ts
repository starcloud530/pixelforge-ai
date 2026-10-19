import init, { PixelDocument } from "../../../../crates/pixelforge-kernel-wasm/pkg/pixelforge_kernel_wasm.js";
import wasmUrl from "../../../../crates/pixelforge-kernel-wasm/pkg/pixelforge_kernel_wasm_bg.wasm?url";
import { seedDemo } from "./demo";
import { kernelErrorFromUnknown, type KernelSnapshot, type PixelKernel, type Rgba } from "./types";

let ready = false;
let starting: Promise<void> | null = null;

/**
 * Loads the wasm module once. `createKernel()` throws until this resolves.
 * Vite serves the `.wasm` asset; the glue instantiates that URL.
 */
export function initKernel(): Promise<void> {
  if (!starting) {
    starting = init({ module_or_path: wasmUrl })
      .then(() => {
        ready = true;
      })
      .catch((error: unknown) => {
        starting = null;
        throw error;
      });
  }
  return starting;
}

/** Returns a wasm document. Pass 16×16 to seed the demo through `paint`. */
export function createKernel(width = 16, height = 16): PixelKernel {
  if (!ready) {
    throw new Error("initKernel() must finish before createKernel()");
  }
  const kernel = WasmKernel.open(width, height);
  if (width === 16 && height === 16) {
    seedDemo(kernel);
    kernel.clearHistory();
  }
  return kernel;
}

/** Parses project JSON in the kernel and returns that document. Does not seed the demo. */
export function openProject(json: string): PixelKernel {
  if (!ready) {
    throw new Error("initKernel() must finish before openProject()");
  }
  return WasmKernel.fromProject(json);
}

class WasmKernel implements PixelKernel {
  private readonly doc: PixelDocument;

  private constructor(doc: PixelDocument) {
    this.doc = doc;
  }

  static open(width: number, height: number): WasmKernel {
    try {
      return new WasmKernel(new PixelDocument(width, height));
    } catch (error) {
      rethrow(error);
    }
  }

  static fromProject(json: string): WasmKernel {
    try {
      return new WasmKernel(PixelDocument.loadProject(json));
    } catch (error) {
      rethrow(error);
    }
  }

  snapshot(): KernelSnapshot {
    return {
      width: this.doc.width(),
      height: this.doc.height(),
      palette: paletteFromRgba(this.doc.paletteRgba()),
      frameCount: this.doc.frameCount(),
      activeFrame: this.doc.activeFrame(),
      frameDelays: [...this.doc.frameDelays()],
      playing: this.doc.playing(),
      pixels: this.doc.pixels(),
    };
  }

  paint(x: number, y: number, colorIndex: number): void {
    try {
      this.doc.paint(x, y, colorIndex);
    } catch (error) {
      rethrow(error);
    }
  }

  erase(x: number, y: number): void {
    try {
      this.doc.erase(x, y);
    } catch (error) {
      rethrow(error);
    }
  }

  setPaletteColor(index: number, r: number, g: number, b: number): void {
    try {
      this.doc.setPaletteColor(index, r, g, b);
    } catch (error) {
      rethrow(error);
    }
  }

  addPaletteColor(r: number, g: number, b: number): number {
    try {
      return this.doc.addPaletteColor(r, g, b);
    } catch (error) {
      rethrow(error);
    }
  }

  addFrame(): number {
    return this.doc.addFrame();
  }

  duplicateFrame(index: number): number {
    try {
      return this.doc.duplicateFrame(index);
    } catch (error) {
      rethrow(error);
    }
  }

  deleteFrame(index: number): void {
    try {
      this.doc.deleteFrame(index);
    } catch (error) {
      rethrow(error);
    }
  }

  setActiveFrame(index: number): void {
    try {
      this.doc.setActiveFrame(index);
    } catch (error) {
      rethrow(error);
    }
  }

  setFrameDelay(index: number, delayMs: number): void {
    try {
      this.doc.setFrameDelay(index, delayMs);
    } catch (error) {
      rethrow(error);
    }
  }

  play(): void {
    this.doc.play();
  }

  stop(): void {
    this.doc.stop();
  }

  playing(): boolean {
    return this.doc.playing();
  }

  tick(deltaMs: number): boolean {
    return this.doc.tick(deltaMs);
  }

  scrub(ratioPpm: number): void {
    try {
      this.doc.scrub(ratioPpm);
    } catch (error) {
      rethrow(error);
    }
  }

  beginGesture(): void {
    try {
      this.doc.beginGesture();
    } catch (error) {
      rethrow(error);
    }
  }

  endGesture(): void {
    this.doc.endGesture();
  }

  gestureOpen(): boolean {
    return this.doc.gestureOpen();
  }

  undo(): void {
    try {
      this.doc.undo();
    } catch (error) {
      rethrow(error);
    }
  }

  redo(): void {
    try {
      this.doc.redo();
    } catch (error) {
      rethrow(error);
    }
  }

  canUndo(): boolean {
    return this.doc.canUndo();
  }

  canRedo(): boolean {
    return this.doc.canRedo();
  }

  clearHistory(): void {
    this.doc.clearHistory();
  }

  saveProject(): string {
    return this.doc.saveProject();
  }

  exportFramePng(index: number): Uint8Array {
    try {
      return this.doc.exportFramePng(index);
    } catch (error) {
      rethrow(error);
    }
  }

  exportSpriteSheet(): Uint8Array {
    return this.doc.exportSpriteSheet();
  }

  spriteSheetColumns(): number {
    return this.doc.spriteSheetColumns();
  }

  spriteSheetRows(): number {
    return this.doc.spriteSheetRows();
  }
}

function paletteFromRgba(bytes: Uint8Array): Rgba[] {
  if (bytes.length % 4 !== 0) {
    throw new Error("palette byte length is not a multiple of 4");
  }
  const colors: Rgba[] = [];
  for (let index = 0; index < bytes.length; index += 4) {
    colors.push({
      r: bytes[index] ?? 0,
      g: bytes[index + 1] ?? 0,
      b: bytes[index + 2] ?? 0,
      a: bytes[index + 3] ?? 0,
    });
  }
  return colors;
}

function rethrow(error: unknown): never {
  const kernelError = kernelErrorFromUnknown(error);
  if (kernelError) {
    throw kernelError;
  }
  throw error;
}
