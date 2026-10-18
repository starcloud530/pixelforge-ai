import { MAX_CANVAS_EDGE, STUDIO_PALETTE, TRANSPARENT_INDEX } from "./palette";
import { KernelError, type KernelSnapshot, type PixelKernel } from "./types";

/**
 * In-memory stand-in for `pixelforge_kernel::Document`.
 * Single frame only. Frame add / duplicate / delete already exist on the
 * Rust type and are intentionally not mirrored here.
 */
class StubKernel implements PixelKernel {
  readonly width: number;
  readonly height: number;
  private readonly pixels: Uint8Array;

  constructor(width: number, height: number) {
    if (
      !Number.isInteger(width) ||
      !Number.isInteger(height) ||
      width < 1 ||
      height < 1 ||
      width > MAX_CANVAS_EDGE ||
      height > MAX_CANVAS_EDGE
    ) {
      throw new KernelError(
        "InvalidSize",
        `canvas size ${width}x${height} is outside 1..=${MAX_CANVAS_EDGE}`,
      );
    }
    this.width = width;
    this.height = height;
    this.pixels = new Uint8Array(width * height);
  }

  snapshot(): KernelSnapshot {
    return {
      width: this.width,
      height: this.height,
      palette: STUDIO_PALETTE,
      frameCount: 1,
      activeFrame: 0,
      pixels: this.pixels.slice(),
    };
  }

  paint(x: number, y: number, colorIndex: number): void {
    const offset = this.offset(x, y);
    if (
      !Number.isInteger(colorIndex) ||
      colorIndex === TRANSPARENT_INDEX ||
      colorIndex < 0 ||
      colorIndex >= STUDIO_PALETTE.length
    ) {
      throw new KernelError("PaletteIndex", `palette index ${colorIndex} cannot be painted`);
    }
    this.pixels[offset] = colorIndex;
  }

  erase(x: number, y: number): void {
    this.pixels[this.offset(x, y)] = TRANSPARENT_INDEX;
  }

  private offset(x: number, y: number): number {
    if (
      !Number.isInteger(x) ||
      !Number.isInteger(y) ||
      x < 0 ||
      y < 0 ||
      x >= this.width ||
      y >= this.height
    ) {
      throw new KernelError("OutOfBounds", `cell ${x},${y} is outside ${this.width}x${this.height}`);
    }
    return y * this.width + x;
  }
}

/** 16×16 demo painted through the kernel API, not drawn as vectors. */
const DEMO_ROWS = [
  "................",
  "......1111......",
  "....11222211....",
  "...1222222221...",
  "...1221221221...",
  "...1222222221...",
  "...1221221221...",
  "....11222211....",
  "......1111......",
  "....11333311....",
  "...1333333331...",
  "...1333333331...",
  "....11333311....",
  ".....11..11.....",
  ".....11..11.....",
  "................",
];

export function createStubKernel(width = 16, height = 16): PixelKernel {
  const kernel = new StubKernel(width, height);
  if (width === 16 && height === 16) {
    seedDemo(kernel);
  }
  return kernel;
}

function seedDemo(kernel: StubKernel): void {
  DEMO_ROWS.forEach((row, y) => {
    if (row.length !== 16) {
      throw new Error(`demo row ${y} has length ${row.length}`);
    }
    for (let x = 0; x < row.length; x += 1) {
      const cell = row[x];
      if (cell === ".") {
        continue;
      }
      kernel.paint(x, y, Number(cell));
    }
  });
}
