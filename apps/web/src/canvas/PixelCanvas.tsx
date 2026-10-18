import { useLayoutEffect, useRef, useState, type PointerEvent } from "react";
import type { KernelSnapshot } from "../kernel/types";

interface Cell {
  x: number;
  y: number;
}

interface PixelCanvasProps {
  snapshot: KernelSnapshot;
  scale: number;
  showGrid: boolean;
  onPrimary: (x: number, y: number) => void;
  onSecondary: (x: number, y: number) => void;
}

/**
 * Draws the kernel's indexed buffer at 1:1, then scales it with
 * nearest-neighbor sampling. The grid lines are a view aid. They are not
 * the document.
 */
export function PixelCanvas({
  snapshot,
  scale,
  showGrid,
  onPrimary,
  onSecondary,
}: PixelCanvasProps) {
  const viewRef = useRef<HTMLCanvasElement>(null);
  const stroke = useRef<"primary" | "secondary" | null>(null);
  const [hover, setHover] = useState<Cell | null>(null);

  useLayoutEffect(() => {
    const view = viewRef.current;
    if (!view) {
      return;
    }
    paintView(view, snapshot, scale, showGrid, hover);
  }, [snapshot, scale, showGrid, hover]);

  function cellFromEvent(event: PointerEvent<HTMLCanvasElement>): Cell | null {
    const rect = event.currentTarget.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) {
      return null;
    }
    const x = Math.floor(((event.clientX - rect.left) / rect.width) * snapshot.width);
    const y = Math.floor(((event.clientY - rect.top) / rect.height) * snapshot.height);
    if (x < 0 || y < 0 || x >= snapshot.width || y >= snapshot.height) {
      return null;
    }
    return { x, y };
  }

  function apply(cell: Cell, mode: "primary" | "secondary") {
    if (mode === "secondary") {
      onSecondary(cell.x, cell.y);
    } else {
      onPrimary(cell.x, cell.y);
    }
  }

  function onPointerDown(event: PointerEvent<HTMLCanvasElement>) {
    const mode = event.button === 2 ? "secondary" : event.button === 0 ? "primary" : null;
    if (!mode) {
      return;
    }
    const cell = cellFromEvent(event);
    if (!cell) {
      return;
    }
    event.currentTarget.setPointerCapture(event.pointerId);
    stroke.current = mode;
    apply(cell, mode);
  }

  function onPointerMove(event: PointerEvent<HTMLCanvasElement>) {
    const cell = cellFromEvent(event);
    setHover(cell);
    if (!stroke.current || !cell) {
      return;
    }
    const buttons = event.buttons;
    if ((buttons & 1) === 0 && (buttons & 2) === 0) {
      stroke.current = null;
      return;
    }
    apply(cell, stroke.current);
  }

  function endStroke(event: PointerEvent<HTMLCanvasElement>) {
    stroke.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  }

  const hoverIndex =
    hover === null ? null : snapshot.pixels[hover.y * snapshot.width + hover.x];

  return (
    <div className="canvas-column">
      <div className="stage">
        <canvas
          ref={viewRef}
          className="pixel-view"
          aria-label="像素画布"
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={endStroke}
          onPointerCancel={endStroke}
          onPointerLeave={() => setHover(null)}
          onContextMenu={(event) => event.preventDefault()}
        />
      </div>
      <p className="readout">
        {snapshot.width}×{snapshot.height}
        {" · "}
        帧 {snapshot.activeFrame + 1}/{snapshot.frameCount}
        {" · "}
        {hover && hoverIndex !== null
          ? `指针 ${hover.x},${hover.y} · 索引 ${hoverIndex}`
          : "指针在画布外"}
      </p>
    </div>
  );
}

function paintView(
  view: HTMLCanvasElement,
  snapshot: KernelSnapshot,
  scale: number,
  showGrid: boolean,
  hover: Cell | null,
) {
  const cssWidth = snapshot.width * scale;
  const cssHeight = snapshot.height * scale;
  const dpr = window.devicePixelRatio || 1;
  view.width = Math.max(1, Math.round(cssWidth * dpr));
  view.height = Math.max(1, Math.round(cssHeight * dpr));
  view.style.width = `${cssWidth}px`;
  view.style.height = `${cssHeight}px`;

  const ctx = view.getContext("2d");
  if (!ctx) {
    return;
  }

  const source = document.createElement("canvas");
  source.width = snapshot.width;
  source.height = snapshot.height;
  const sourceCtx = source.getContext("2d");
  if (!sourceCtx) {
    return;
  }
  const image = sourceCtx.createImageData(snapshot.width, snapshot.height);
  for (let i = 0; i < snapshot.pixels.length; i += 1) {
    const color = snapshot.palette[snapshot.pixels[i]] ?? { r: 0, g: 0, b: 0, a: 0 };
    const offset = i * 4;
    image.data[offset] = color.r;
    image.data[offset + 1] = color.g;
    image.data[offset + 2] = color.b;
    image.data[offset + 3] = color.a;
  }
  sourceCtx.putImageData(image, 0, 0);

  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.imageSmoothingEnabled = false;
  ctx.clearRect(0, 0, cssWidth, cssHeight);
  ctx.drawImage(source, 0, 0, cssWidth, cssHeight);

  if (showGrid && scale >= 8) {
    ctx.strokeStyle = "rgba(20, 22, 26, 0.4)";
    ctx.lineWidth = 1;
    ctx.beginPath();
    for (let x = 0; x <= snapshot.width; x += 1) {
      const px = x * scale + 0.5;
      ctx.moveTo(px, 0);
      ctx.lineTo(px, cssHeight);
    }
    for (let y = 0; y <= snapshot.height; y += 1) {
      const py = y * scale + 0.5;
      ctx.moveTo(0, py);
      ctx.lineTo(cssWidth, py);
    }
    ctx.stroke();
  }

  if (hover) {
    ctx.strokeStyle = "#f4f1de";
    ctx.lineWidth = 1;
    ctx.strokeRect(hover.x * scale + 1.5, hover.y * scale + 1.5, scale - 3, scale - 3);
  }
}
