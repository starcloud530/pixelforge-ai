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
        <p className="kernel-tag">内核桩 · JavaScript</p>
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
          视图把内核里的索引像素按最近邻放大。撤销、色板编辑、多帧界面和工程保存还在
          issue #3–#6，这一版没有做完。
        </p>
      </footer>
    </div>
  );
}
