import type { PixelKernel } from "./types";

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

/** Two poses so loop playback has something to cycle. Delays are kernel defaults until adjusted. */
export function seedDemo(kernel: PixelKernel): void {
  paintPose(kernel, 0);
  const next = kernel.addFrame();
  kernel.setActiveFrame(next);
  paintPose(kernel, 1);
  kernel.setFrameDelay(0, 180);
  kernel.setFrameDelay(1, 180);
  kernel.setActiveFrame(0);
}

function paintPose(kernel: PixelKernel, yOffset: number): void {
  DEMO_ROWS.forEach((row, y) => {
    if (row.length !== 16) {
      throw new Error(`demo row ${y} has length ${row.length}`);
    }
    const targetY = y + yOffset;
    if (targetY < 0 || targetY >= 16) {
      return;
    }
    for (let x = 0; x < row.length; x += 1) {
      const cell = row[x];
      if (cell === ".") {
        continue;
      }
      kernel.paint(x, targetY, Number(cell));
    }
  });
}
