import { useMemo, useState } from "react";
import { PixelCanvas } from "./canvas/PixelCanvas";
import { createKernel } from "./kernel/client";
import type { PixelKernel, Rgba } from "./kernel/types";

function swatchInk(color: Rgba): string {
  const luminance = (0.299 * color.r + 0.587 * color.g + 0.114 * color.b) / 255;
  return luminance > 0.62 ? "#14161a" : "#f4f1de";
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

  function touch() {
    setRevision((value) => value + 1);
  }

  function paintAt(x: number, y: number) {
    kernel.paint(x, y, colorIndex);
    touch();
  }

  function eraseAt(x: number, y: number) {
    kernel.erase(x, y);
    touch();
  }

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
            <p className="hint">左键使用当前工具，右键擦除。拖动可连续画。</p>
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
                      setColorIndex(index);
                      setTool("paint");
                    }}
                  >
                    {index}
                  </button>
                );
              })}
            </div>
            <p className="hint">索引 0 只表示空格子。画笔不能写入它。</p>
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
                  const index = kernel.addFrame();
                  kernel.setActiveFrame(index);
                  touch();
                }}
              >
                新增
              </button>
              <button
                type="button"
                onClick={() => {
                  const index = kernel.duplicateFrame(snapshot.activeFrame);
                  kernel.setActiveFrame(index);
                  touch();
                }}
              >
                复制
              </button>
              <button
                type="button"
                disabled={snapshot.frameCount === 1}
                title={snapshot.frameCount === 1 ? "至少保留一帧" : "删除当前帧"}
                onClick={() => {
                  kernel.deleteFrame(snapshot.activeFrame);
                  touch();
                }}
              >
                删除
              </button>
            </div>
            <p className="hint">增、复制、删除都写在内核里。最后一帧不能删。</p>
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
          onPrimary={(x, y) => {
            if (tool === "erase") {
              eraseAt(x, y);
            } else {
              paintAt(x, y);
            }
          }}
          onSecondary={eraseAt}
        />
      </div>

      <footer className="footnote">
        <p>
          视图把 Rust 内核里的索引像素按最近邻放大。画笔、橡皮和帧列表都进 WebAssembly。
          撤销、色板编辑和工程保存还在 issue #3–#6。
        </p>
      </footer>
    </div>
  );
}
