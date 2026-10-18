import { useEffect, useMemo, useRef, useState } from "react";
import { PixelCanvas } from "./canvas/PixelCanvas";
import { createKernel } from "./kernel/client";
import type { PixelKernel, Rgba } from "./kernel/types";

function swatchInk(color: Rgba): string {
  const luminance = (0.299 * color.r + 0.587 * color.g + 0.114 * color.b) / 255;
  return luminance > 0.62 ? "#14161a" : "#f4f1de";
}

function hexFromRgb(color: Rgba): string {
  const channel = (value: number) => value.toString(16).padStart(2, "0");
  return `#${channel(color.r)}${channel(color.g)}${channel(color.b)}`;
}

function rgbFromHex(hex: string): { r: number; g: number; b: number } | null {
  const match = /^#([0-9a-f]{6})$/i.exec(hex);
  if (!match?.[1]) {
    return null;
  }
  const value = Number.parseInt(match[1], 16);
  return {
    r: (value >> 16) & 255,
    g: (value >> 8) & 255,
    b: value & 255,
  };
}

type Tool = "paint" | "erase";

const SCALES = [8, 16, 24] as const;

export function App() {
  const [kernel, setKernel] = useState<PixelKernel>(() => createKernel());
  const [revision, setRevision] = useState(0);
  const [tool, setTool] = useState<Tool>("paint");
  const [colorIndex, setColorIndex] = useState(7);
  const [scale, setScale] = useState<number>(16);
  const [showGrid, setShowGrid] = useState(true);

  const snapshot = useMemo(() => kernel.snapshot(), [kernel, revision]);
  const canUndo = kernel.canUndo();
  const canRedo = kernel.canRedo();
  const paletteCount = snapshot.palette.length;
  const paintColor = colorIndex > 0 ? snapshot.palette[colorIndex] : undefined;

  useEffect(() => {
    if (paletteCount > 1 && colorIndex >= paletteCount) {
      setColorIndex(paletteCount - 1);
    }
  }, [colorIndex, paletteCount]);

  function touch() {
    setRevision((value) => value + 1);
  }

  function undo() {
    if (!kernel.canUndo()) {
      return;
    }
    kernel.undo();
    touch();
  }

  function redo() {
    if (!kernel.canRedo()) {
      return;
    }
    kernel.redo();
    touch();
  }

  function closePaletteGesture() {
    if (kernel.gestureOpen()) {
      kernel.endGesture();
      touch();
    }
  }

  function editPaletteColor(value: string) {
    const next = rgbFromHex(value);
    const current = snapshot.palette[colorIndex];
    if (!next || !current || colorIndex === 0) {
      return;
    }
    if (next.r === current.r && next.g === current.g && next.b === current.b) {
      return;
    }
    if (!kernel.gestureOpen()) {
      kernel.beginGesture();
    }
    kernel.setPaletteColor(colorIndex, next.r, next.g, next.b);
    touch();
  }

  function edit(run: () => void) {
    if (kernel.gestureOpen()) {
      kernel.endGesture();
    }
    kernel.beginGesture();
    try {
      run();
    } finally {
      kernel.endGesture();
      touch();
    }
  }

  const undoRef = useRef(undo);
  const redoRef = useRef(redo);
  undoRef.current = undo;
  redoRef.current = redo;

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      const target = event.target;
      if (
        target instanceof HTMLTextAreaElement ||
        (target instanceof HTMLElement && target.isContentEditable) ||
        (target instanceof HTMLInputElement && target.type !== "color")
      ) {
        return;
      }
      const shortcut = event.ctrlKey || event.metaKey;
      if (!shortcut || event.altKey) {
        return;
      }
      const redoKey = (event.code === "KeyZ" && event.shiftKey) || event.code === "KeyY";
      const undoKey = event.code === "KeyZ" && !event.shiftKey;
      if (!undoKey && !redoKey) {
        return;
      }
      event.preventDefault();
      if (undoKey) {
        undoRef.current();
      } else {
        redoRef.current();
      }
    }

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  return (
    <div className="app">
      <header className="topbar">
        <div>
          <p className="eyebrow">PixelForge AI</p>
          <h1>像素画布骨架</h1>
        </div>
        <p className="kernel-tag">内核 · WebAssembly</p>
      </header>

      <div className="workspace">
        <aside className="panel">
          <section>
            <h2>工具</h2>
            <div className="tool-row" role="group" aria-label="绘制工具">
              <button
                type="button"
                className={tool === "paint" ? "is-active" : ""}
                aria-pressed={tool === "paint"}
                onClick={() => setTool("paint")}
              >
                画笔
              </button>
              <button
                type="button"
                className={tool === "erase" ? "is-active" : ""}
                aria-pressed={tool === "erase"}
                onClick={() => setTool("erase")}
              >
                橡皮
              </button>
            </div>
            <div className="tool-row" role="group" aria-label="撤销">
              <button
                type="button"
                disabled={!canUndo}
                title="Ctrl/⌘+Z"
                aria-keyshortcuts="Control+Z Meta+Z"
                onClick={undo}
              >
                撤销
              </button>
              <button
                type="button"
                disabled={!canRedo}
                title="Ctrl/⌘+Shift+Z"
                aria-keyshortcuts="Control+Shift+Z Meta+Shift+Z Control+Y"
                onClick={redo}
              >
                重做
              </button>
            </div>
            <p className="hint">左键使用当前工具，右键擦除。一次拖动是一步撤销。</p>
          </section>

          <section>
            <h2>索引色板</h2>
            <div className="swatches" role="listbox" aria-label="调色板">
              {snapshot.palette.map((color, index) => {
                if (index === 0) {
                  return (
                    <span key={index} className="swatch swatch-empty" title="索引 0 · 透明">
                      0
                    </span>
                  );
                }
                const selected = tool === "paint" && colorIndex === index;
                return (
                  <button
                    key={index}
                    type="button"
                    role="option"
                    aria-selected={selected}
                    className={selected ? "swatch is-selected" : "swatch"}
                    title={`索引 ${index}`}
                    style={{
                      backgroundColor: `rgb(${color.r}, ${color.g}, ${color.b})`,
                      color: swatchInk(color),
                      textShadow: "none",
                    }}
                    onClick={() => {
                      closePaletteGesture();
                      setColorIndex(index);
                      setTool("paint");
                    }}
                  >
                    {index}
                  </button>
                );
              })}
            </div>
            {paintColor ? (
              <div className="color-editor">
                <label className="color-field">
                  <span>编辑索引 {colorIndex}</span>
                  <input
                    type="color"
                    aria-label={`编辑索引 ${colorIndex} 的颜色`}
                    value={hexFromRgb(paintColor)}
                    onChange={(event) => editPaletteColor(event.target.value)}
                    onBlur={closePaletteGesture}
                  />
                  <code>{hexFromRgb(paintColor)}</code>
                </label>
                <button
                  type="button"
                  disabled={snapshot.palette.length >= 256}
                  title={
                    snapshot.palette.length >= 256 ? "调色板最多 256 色" : "在色板末尾加一个颜色"
                  }
                  onClick={() => {
                    edit(() => {
                      const index = kernel.addPaletteColor(255, 255, 255);
                      setColorIndex(index);
                      setTool("paint");
                    });
                  }}
                >
                  添加颜色
                </button>
              </div>
            ) : null}
            <p className="hint">
              点一个索引，画笔就用它。改这个索引的颜色会立刻重绘所有用到它的格子，撤销可以退回。索引
              0 不能改，画笔也不能写入它。
            </p>
          </section>

          <section>
            <h2>帧</h2>
            <div className="frame-row" role="listbox" aria-label="帧列表">
              {Array.from({ length: snapshot.frameCount }, (_, index) => {
                const selected = index === snapshot.activeFrame;
                return (
                  <button
                    key={index}
                    type="button"
                    role="option"
                    aria-selected={selected}
                    aria-label={`帧 ${index + 1}`}
                    className={selected ? "is-active" : ""}
                    onClick={() => {
                      closePaletteGesture();
                      kernel.setActiveFrame(index);
                      touch();
                    }}
                  >
                    {index + 1}
                  </button>
                );
              })}
            </div>
            <div className="tool-row">
              <button
                type="button"
                onClick={() => {
                  edit(() => {
                    const index = kernel.addFrame();
                    kernel.setActiveFrame(index);
                  });
                }}
              >
                新增
              </button>
              <button
                type="button"
                onClick={() => {
                  edit(() => {
                    const index = kernel.duplicateFrame(snapshot.activeFrame);
                    kernel.setActiveFrame(index);
                  });
                }}
              >
                复制
              </button>
              <button
                type="button"
                disabled={snapshot.frameCount === 1}
                title={snapshot.frameCount === 1 ? "至少保留一帧" : "删除当前帧"}
                onClick={() => {
                  closePaletteGesture();
                  kernel.deleteFrame(snapshot.activeFrame);
                  touch();
                }}
              >
                删除
              </button>
            </div>
            <p className="hint">增删、复制和切换当前帧记在内核的撤销栈上。最后一帧不能删。</p>
          </section>

          <section>
            <h2>视图</h2>
            <div className="tool-row" role="group" aria-label="放大倍数">
              {SCALES.map((value) => (
                <button
                  key={value}
                  type="button"
                  className={scale === value ? "is-active" : ""}
                  aria-pressed={scale === value}
                  onClick={() => setScale(value)}
                >
                  {value}×
                </button>
              ))}
            </div>
            <label className="check">
              <input
                type="checkbox"
                checked={showGrid}
                onChange={(event) => setShowGrid(event.target.checked)}
              />
              显示格子线
            </label>
            <button
              type="button"
              className="reset"
              onClick={() => {
                setKernel(createKernel());
                touch();
              }}
            >
              重置示例
            </button>
          </section>
        </aside>

        <PixelCanvas
          snapshot={snapshot}
          scale={scale}
          showGrid={showGrid}
          onStrokeStart={() => {
            if (kernel.gestureOpen()) {
              kernel.endGesture();
            }
            kernel.beginGesture();
          }}
          onStrokeEnd={() => kernel.endGesture()}
          onPrimary={(x, y) => {
            if (tool === "erase") {
              kernel.erase(x, y);
            } else {
              kernel.paint(x, y, colorIndex);
            }
            touch();
          }}
          onSecondary={(x, y) => {
            kernel.erase(x, y);
            touch();
          }}
        />
      </div>

      <footer className="footnote">
        <p>
          视图把 Rust 内核里的索引像素按最近邻放大。画笔、橡皮、调色板、帧列表和撤销都进同一份
          WebAssembly 文档。导入色板、锁定色板和工程保存还在后面的 issue 里。
        </p>
      </footer>
    </div>
  );
}
