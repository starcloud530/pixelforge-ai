import { useEffect, useMemo, useRef, useState, type ChangeEvent } from "react";
import { AiPanel } from "./ai/AiPanel";
import { PixelCanvas, type OnionLayer } from "./canvas/PixelCanvas";
import { createKernel, openProject } from "./kernel/client";
import {
  kernelErrorFromUnknown,
  type PaletteImportMode,
  type PixelKernel,
  type Rgba,
} from "./kernel/types";
import { Timeline } from "./timeline/Timeline";

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
const PROJECT_FILENAME = "pixelforge-project.json";
/** Default onion-skin opacity. Strong enough to trace, light enough to keep the current frame in front. */
const DEFAULT_ONION_OPACITY = 40;
const MIN_ONION_OPACITY = 10;
const MAX_ONION_OPACITY = 80;

function framePngName(index: number): string {
  return `pixelforge-${index.toString().padStart(3, "0")}.png`;
}

function sheetPngName(columns: number, rows: number): string {
  return `pixelforge-sheet-${columns}x${rows}.png`;
}

export function App() {
  const [kernel, setKernel] = useState<PixelKernel>(() => createKernel());
  const [revision, setRevision] = useState(0);
  const [tool, setTool] = useState<Tool>("paint");
  const [colorIndex, setColorIndex] = useState(7);
  const [scale, setScale] = useState<number>(16);
  const [showGrid, setShowGrid] = useState(true);
  const [onionOn, setOnionOn] = useState(true);
  const [onionPrevious, setOnionPrevious] = useState(true);
  const [onionNext, setOnionNext] = useState(true);
  const [onionOpacity, setOnionOpacity] = useState(DEFAULT_ONION_OPACITY);
  const [projectError, setProjectError] = useState<string | null>(null);
  const [exportNote, setExportNote] = useState<string | null>(null);
  const [exportError, setExportError] = useState<string | null>(null);
  const [paletteError, setPaletteError] = useState<string | null>(null);
  const [importMode, setImportMode] = useState<PaletteImportMode>("replace");
  const [agentBusy, setAgentBusy] = useState(false);
  const fileInput = useRef<HTMLInputElement>(null);
  const paletteInput = useRef<HTMLInputElement>(null);

  const snapshot = useMemo(() => kernel.snapshot(), [kernel, revision]);
  const onionLayers = useMemo(() => {
    if (!onionOn || snapshot.playing) {
      return [];
    }
    const opacity = onionOpacity / 100;
    const layers: OnionLayer[] = [];
    if (onionPrevious && snapshot.activeFrame > 0) {
      layers.push({
        rgba: kernel.frameRgba(snapshot.activeFrame - 1),
        opacity,
      });
    }
    if (onionNext && snapshot.activeFrame + 1 < snapshot.frameCount) {
      layers.push({
        rgba: kernel.frameRgba(snapshot.activeFrame + 1),
        opacity,
      });
    }
    return layers;
  }, [kernel, snapshot, onionOn, onionPrevious, onionNext, onionOpacity]);
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
    if (!next || !current || colorIndex === 0 || snapshot.paletteLocked) {
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

  function stopPlayback() {
    if (kernel.playing()) {
      kernel.stop();
    }
  }

  function togglePlayback() {
    if (kernel.gestureOpen()) {
      kernel.endGesture();
    }
    if (kernel.playing()) {
      kernel.stop();
    } else {
      kernel.play();
    }
    touch();
  }

  function stepFrame(delta: number) {
    if (kernel.gestureOpen()) {
      kernel.endGesture();
    }
    stopPlayback();
    const shot = kernel.snapshot();
    const next = Math.min(shot.frameCount - 1, Math.max(0, shot.activeFrame + delta));
    kernel.setActiveFrame(next);
    touch();
  }

  function onScrubStart() {
    if (kernel.gestureOpen()) {
      kernel.endGesture();
    }
    stopPlayback();
    kernel.beginGesture();
  }

  function onScrub(ratioPpm: number) {
    kernel.scrub(ratioPpm);
    touch();
  }

  function onScrubEnd() {
    if (kernel.gestureOpen()) {
      kernel.endGesture();
    }
    touch();
  }

  function onDelayFocus() {
    if (kernel.gestureOpen()) {
      kernel.endGesture();
    }
    stopPlayback();
    touch();
  }

  function onDelayChange(delayMs: number) {
    stopPlayback();
    if (!kernel.gestureOpen()) {
      kernel.beginGesture();
    }
    kernel.setFrameDelay(kernel.snapshot().activeFrame, delayMs);
    touch();
  }

  function onDelayBlur() {
    closePaletteGesture();
  }

  function edit(run: () => void) {
    stopPlayback();
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

  function saveProjectFile() {
    downloadText(PROJECT_FILENAME, kernel.saveProject());
    setProjectError(null);
  }

  function exportPngSequence() {
    const count = kernel.snapshot().frameCount;
    try {
      for (let index = 0; index < count; index += 1) {
        downloadBytes(framePngName(index), kernel.exportFramePng(index), "image/png");
      }
      const last = framePngName(count - 1);
      setExportError(null);
      setExportNote(
        count === 1
          ? `已下载 ${framePngName(0)}。索引 0 是透明像素。`
          : `已下载 ${count} 个 PNG：${framePngName(0)} 到 ${last}。索引 0 是透明像素。`,
      );
    } catch (error) {
      setExportNote(null);
      setExportError(`无法导出 PNG 序列：${errorText(error)}`);
    }
  }

  function exportSpriteSheet() {
    try {
      const columns = kernel.spriteSheetColumns();
      const rows = kernel.spriteSheetRows();
      const name = sheetPngName(columns, rows);
      downloadBytes(name, kernel.exportSpriteSheet(), "image/png");
      setExportError(null);
      setExportNote(`已下载 ${name}。${columns}×${rows} 格，从左到右再换行。`);
    } catch (error) {
      setExportNote(null);
      setExportError(`无法导出 sprite sheet：${errorText(error)}`);
    }
  }

  function requestOpen() {
    fileInput.current?.click();
  }

  function requestPaletteImport() {
    paletteInput.current?.click();
  }

  function onPaletteFile(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) {
      return;
    }
    const mode = importMode;
    void file.text().then(
      (text) => {
        try {
          edit(() => {
            kernel.importPalette(text, mode);
          });
          setPaletteError(null);
        } catch (error) {
          setPaletteError(`无法导入色板：${errorText(error)}`);
        }
      },
      () => {
        setPaletteError("无法读取这个色板文件。");
      },
    );
  }

  function onProjectFile(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) {
      return;
    }
    void file.text().then(
      (json) => {
        try {
          const next = openProject(json);
          const paletteLength = next.snapshot().palette.length;
          setKernel(next);
          setColorIndex((current) =>
            current > 0 && current < paletteLength ? current : Math.min(1, paletteLength - 1),
          );
          setProjectError(null);
          setExportNote(null);
          setExportError(null);
          setPaletteError(null);
          touch();
        } catch (error) {
          setProjectError(`无法打开工程：${errorText(error)}`);
        }
      },
      () => {
        setProjectError("无法读取这个文件。");
      },
    );
  }

  const undoRef = useRef(undo);
  const redoRef = useRef(redo);
  const saveRef = useRef(saveProjectFile);
  const openRef = useRef(requestOpen);
  const toggleRef = useRef(togglePlayback);
  const stepRef = useRef(stepFrame);
  undoRef.current = undo;
  redoRef.current = redo;
  saveRef.current = saveProjectFile;
  openRef.current = requestOpen;
  toggleRef.current = togglePlayback;
  stepRef.current = stepFrame;

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (agentBusy) {
        return;
      }
      const target = event.target;
      if (
        target instanceof HTMLTextAreaElement ||
        (target instanceof HTMLElement && target.isContentEditable) ||
        (target instanceof HTMLInputElement && target.type !== "color")
      ) {
        return;
      }
      if (!event.ctrlKey && !event.metaKey && !event.altKey) {
        if (event.code === "Space" && !event.shiftKey) {
          if (target instanceof HTMLButtonElement) {
            return;
          }
          event.preventDefault();
          toggleRef.current();
          return;
        }
        if (event.code === "ArrowLeft" || event.code === "ArrowRight") {
          event.preventDefault();
          stepRef.current(event.code === "ArrowRight" ? 1 : -1);
          return;
        }
      }
      const shortcut = event.ctrlKey || event.metaKey;
      if (!shortcut || event.altKey) {
        return;
      }
      if (event.code === "KeyS" && !event.shiftKey) {
        event.preventDefault();
        saveRef.current();
        return;
      }
      if (event.code === "KeyO" && !event.shiftKey) {
        event.preventDefault();
        openRef.current();
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
  }, [agentBusy]);

  useEffect(() => {
    if (!snapshot.playing) {
      return;
    }
    let last = performance.now();
    let handle = 0;
    const step = (now: number) => {
      const delta = Math.max(0, now - last);
      last = now;
      const ms = Math.min(0xffff_ffff, Math.round(delta));
      if (ms > 0 && kernel.tick(ms)) {
        setRevision((value) => value + 1);
      }
      handle = window.requestAnimationFrame(step);
    };
    handle = window.requestAnimationFrame(step);
    return () => window.cancelAnimationFrame(handle);
  }, [kernel, snapshot.playing]);

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
            <h2>工程</h2>
            <div className="tool-row" role="group" aria-label="工程文件">
              <button
                type="button"
                title="Ctrl/⌘+S"
                aria-keyshortcuts="Control+S Meta+S"
                onClick={saveProjectFile}
              >
                保存
              </button>
              <button
                type="button"
                title="Ctrl/⌘+O"
                aria-keyshortcuts="Control+O Meta+O"
                onClick={requestOpen}
              >
                打开
              </button>
            </div>
            <input
              ref={fileInput}
              className="file-input"
              type="file"
              accept="application/json,.json"
              aria-label="打开工程文件"
              onChange={onProjectFile}
            />
            <p className="hint">
              保存会下载 {PROJECT_FILENAME}。打开换成文件里的画布，撤销从这份文件重新开始。
            </p>
            {projectError ? (
              <p className="alert" role="alert">
                {projectError}
              </p>
            ) : null}
          </section>

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
                    disabled={snapshot.paletteLocked}
                    onChange={(event) => editPaletteColor(event.target.value)}
                    onBlur={closePaletteGesture}
                  />
                  <code>{hexFromRgb(paintColor)}</code>
                </label>
                <button
                  type="button"
                  disabled={snapshot.paletteLocked || snapshot.palette.length >= 256}
                  title={
                    snapshot.paletteLocked
                      ? "色板已锁定"
                      : snapshot.palette.length >= 256
                        ? "调色板最多 256 色"
                        : "在色板末尾加一个颜色"
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
            <label className="check">
              <input
                type="checkbox"
                checked={snapshot.paletteLocked}
                onChange={(event) => {
                  closePaletteGesture();
                  stopPlayback();
                  kernel.setPaletteLocked(event.target.checked);
                  setPaletteError(null);
                  touch();
                }}
              />
              锁定色板
            </label>
            <div className="tool-row" role="group" aria-label="导入调色板">
              <button
                type="button"
                className={importMode === "replace" ? "is-active" : ""}
                aria-pressed={importMode === "replace"}
                disabled={snapshot.paletteLocked}
                onClick={() => setImportMode("replace")}
              >
                替换
              </button>
              <button
                type="button"
                className={importMode === "merge" ? "is-active" : ""}
                aria-pressed={importMode === "merge"}
                disabled={snapshot.paletteLocked}
                onClick={() => setImportMode("merge")}
              >
                合并
              </button>
              <button
                type="button"
                disabled={snapshot.paletteLocked}
                title={snapshot.paletteLocked ? "色板已锁定" : "从 .gpl 或十六进制列表导入"}
                onClick={requestPaletteImport}
              >
                导入
              </button>
            </div>
            <input
              ref={paletteInput}
              className="file-input"
              type="file"
              accept=".gpl,.hex,.txt,text/plain"
              aria-label="导入调色板文件"
              onChange={onPaletteFile}
            />
            {paletteError ? (
              <p className="alert" role="alert">
                {paletteError}
              </p>
            ) : null}
            <p className="hint">
              点一个索引，画笔就写入这个索引。改色会重绘所有用到它的格子。锁定后不能改色、追加或导入，画笔仍用当前索引，锁定会写进工程文件。
            </p>
            <p className="hint">
              导入支持 GIMP 调色板（.gpl）和十六进制列表（每行 RRGGBB 或 #RRGGBB）。索引 0
              保持透明。替换会把用到被丢掉索引的格子变回透明；合并只追加还没有的颜色。一步撤销可以退回。
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
                      stopPlayback();
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
                  stopPlayback();
                  kernel.deleteFrame(snapshot.activeFrame);
                  touch();
                }}
              >
                删除
              </button>
            </div>
            <p className="hint">
              增删、复制、切换和改时长都记在内核的撤销栈上。画布下方的时间轴按每帧时长循环播放。最后一帧不能删。
            </p>
          </section>

          <section>
            <h2>导出</h2>
            <div className="tool-row" role="group" aria-label="导出">
              <button type="button" onClick={exportPngSequence}>
                PNG 序列
              </button>
              <button type="button" onClick={exportSpriteSheet}>
                Sprite sheet
              </button>
            </div>
            <p className="hint">
              序列是一帧一个透明 PNG，按时间轴顺序，文件名从 {framePngName(0)} 起。Sprite sheet 是{" "}
              {kernel.spriteSheetColumns()}×{kernel.spriteSheetRows()} 格，从左到右再换行，空位保持透明。时长留在时间轴上，不写进
              PNG。
            </p>
            {exportNote ? <p className="hint">{exportNote}</p> : null}
            {exportError ? (
              <p className="alert" role="alert">
                {exportError}
              </p>
            ) : null}
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
            <label className="check">
              <input
                type="checkbox"
                checked={onionOn}
                onChange={(event) => setOnionOn(event.target.checked)}
              />
              洋葱皮
            </label>
            <div className={onionOn ? "onion-options" : "onion-options is-disabled"}>
              <label className="check">
                <input
                  type="checkbox"
                  checked={onionPrevious}
                  disabled={!onionOn}
                  onChange={(event) => setOnionPrevious(event.target.checked)}
                />
                前一帧
              </label>
              <label className="check">
                <input
                  type="checkbox"
                  checked={onionNext}
                  disabled={!onionOn}
                  onChange={(event) => setOnionNext(event.target.checked)}
                />
                后一帧
              </label>
              <label className="opacity-field">
                <span>不透明度 {onionOpacity}%</span>
                <input
                  type="range"
                  min={MIN_ONION_OPACITY}
                  max={MAX_ONION_OPACITY}
                  step={5}
                  value={onionOpacity}
                  disabled={!onionOn}
                  aria-label="洋葱皮不透明度"
                  onChange={(event) => setOnionOpacity(Number(event.target.value))}
                />
              </label>
            </div>
            <p className="hint">
              前后帧半透明叠在当前帧下面，方便对位补间。只改视图：不写入像素、工程文件或 PNG。循环播放时先藏起，停下后再显示。第一帧没有前一帧，最后一帧没有后一帧。
            </p>
            <button
              type="button"
              className="reset"
              onClick={() => {
                setKernel(createKernel());
                setProjectError(null);
                setExportNote(null);
                setExportError(null);
                setPaletteError(null);
                touch();
              }}
            >
              重置示例
            </button>
          </section>
        </aside>

        <div className="canvas-column">
          <AiPanel
            kernel={kernel}
            onBusy={setAgentBusy}
            onBeforeRun={() => {
              if (kernel.playing()) {
                kernel.stop();
                touch();
              }
            }}
            onApplied={touch}
          />
          <div className={agentBusy ? "is-busy" : undefined}>
            <PixelCanvas
              snapshot={snapshot}
              scale={scale}
              showGrid={showGrid}
              onionLayers={onionLayers}
              onionEnabled={onionOn}
              onStrokeStart={() => {
                stopPlayback();
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
            <Timeline
              frameCount={snapshot.frameCount}
              activeFrame={snapshot.activeFrame}
              delays={snapshot.frameDelays}
              playing={snapshot.playing}
              onTogglePlay={togglePlayback}
              onScrubStart={onScrubStart}
              onScrub={onScrub}
              onScrubEnd={onScrubEnd}
              onDelayFocus={onDelayFocus}
              onDelayChange={onDelayChange}
              onDelayBlur={onDelayBlur}
            />
          </div>
        </div>
      </div>

      <footer className="footnote">
        <p>
          视图把 Rust 内核里的索引像素按最近邻放大。画笔只写入调色板索引。锁定色板，以及从 .gpl
          或十六进制列表导入，都进同一份 WebAssembly 文档。外部模型的一轮修改也只走这份文档的 Tool
          API。洋葱皮只叠在视图上，播放时先藏起。
        </p>
      </footer>
    </div>
  );
}

function downloadText(filename: string, contents: string): void {
  downloadBlob(filename, new Blob([contents], { type: "application/json" }));
}

function downloadBytes(filename: string, bytes: Uint8Array, type: string): void {
  const copy = new Uint8Array(bytes);
  downloadBlob(filename, new Blob([copy], { type }));
}

function downloadBlob(filename: string, blob: Blob): void {
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  document.body.append(link);
  link.click();
  link.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 1500);
}

function errorText(error: unknown): string {
  const kernelError = kernelErrorFromUnknown(error);
  if (kernelError) {
    return kernelError.message;
  }
  if (error instanceof Error && error.message) {
    return error.message;
  }
  return "文件不是这个版本的工程。";
}
