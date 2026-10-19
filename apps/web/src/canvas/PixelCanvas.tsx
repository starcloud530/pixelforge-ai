import { useLayoutEffect, useRef, useState, type PointerEvent } from "react";
import type { KernelSnapshot, Rgba } from "../kernel/types";

interface Cell {
  x: number;
  y: number;
}

/**
 * One neighbor frame, already resolved to RGBA by the kernel.
 * `opacity` is 0–1 and applies only while this view is painted.
 */
export interface OnionLayer {
  rgba: Uint8Array;
  opacity: number;
}

interface PixelCanvasProps {
  snapshot: KernelSnapshot;
  scale: number;
  showGrid: boolean;
  /** Previous frame first, then the next. Drawn under the editable frame. */
  onionLayers: readonly OnionLayer[];
  /** True when the user left onion skin on. Playback passes an empty `onionLayers`. */
  onionEnabled: boolean;
  onStrokeStart: () => void;
  onStrokeEnd: () => void;
  onPrimary: (x: number, y: number) => void;
  onSecondary: (x: number, y: number) => void;
}

/**
 * Draws the kernel's indexed buffer at 1:1, then scales it with
 * nearest-neighbor sampling. Grid lines and onion-skin layers are view aids.
 * They are not the document: onion frames are composited under the current
 * frame and never written back.
 */
export function PixelCanvas({
  snapshot,
  scale,
  showGrid,
  onionLayers,
  onionEnabled,
  onStrokeStart,
  onStrokeEnd,
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
    paintView(view, snapshot, scale, showGrid, onionLayers, hover);
  }, [snapshot, scale, showGrid, onionLayers, hover]);

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
    onStrokeStart();
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
    const active = stroke.current;
    stroke.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    if (active) {
      onStrokeEnd();
    }
  }

  const hoverIndex =
    hover === null ? null : snapshot.pixels[hover.y * snapshot.width + hover.x];

  const frameDelay = snapshot.frameDelays[snapshot.activeFrame];

  return (
    <>
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
        {frameDelay !== undefined ? ` · ${frameDelay} ms` : ""}
        {snapshot.playing ? " · 播放中" : ""}
        {onionEnabled && snapshot.playing ? " · 洋葱皮已隐藏" : ""}
        {onionLayers.length > 0 ? " · 洋葱皮" : ""}
        {" · "}
        {hover && hoverIndex !== null
          ? `指针 ${hover.x},${hover.y} · 索引 ${hoverIndex}`
          : "指针在画布外"}
      </p>
    </>
  );
}

function paintView(
  view: HTMLCanvasElement,
  snapshot: KernelSnapshot,
  scale: number,
  showGrid: boolean,
  onionLayers: readonly OnionLayer[],
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

  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.imageSmoothingEnabled = false;
  ctx.clearRect(0, 0, cssWidth, cssHeight);

  for (const layer of onionLayers) {
    if (!putRgba(sourceCtx, snapshot.width, snapshot.height, layer.rgba)) {
      continue;
    }
    ctx.globalAlpha = clampOpacity(layer.opacity);
    ctx.drawImage(source, 0, 0, cssWidth, cssHeight);
  }
  ctx.globalAlpha = 1;
  putIndexed(sourceCtx, snapshot);
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

function putIndexed(
  target: CanvasRenderingContext2D,
  snapshot: KernelSnapshot,
): void {
  const image = target.createImageData(snapshot.width, snapshot.height);
  for (let i = 0; i < snapshot.pixels.length; i += 1) {
    writeColor(image.data, i * 4, snapshot.palette[snapshot.pixels[i]]);
  }
  target.putImageData(image, 0, 0);
}

/** Returns false when the buffer is not one RGBA byte per pixel. */
function putRgba(
  target: CanvasRenderingContext2D,
  width: number,
  height: number,
  rgba: Uint8Array,
): boolean {
  if (rgba.length !== width * height * 4) {
    return false;
  }
  const image = target.createImageData(width, height);
  image.data.set(rgba);
  target.putImageData(image, 0, 0);
  return true;
}

function writeColor(data: Uint8ClampedArray, offset: number, color: Rgba | undefined): void {
  data[offset] = color?.r ?? 0;
  data[offset + 1] = color?.g ?? 0;
  data[offset + 2] = color?.b ?? 0;
  data[offset + 3] = color?.a ?? 0;
}

function clampOpacity(opacity: number): number {
  if (Number.isNaN(opacity)) {
    return 0;
  }
  return Math.min(1, Math.max(0, opacity));
}
