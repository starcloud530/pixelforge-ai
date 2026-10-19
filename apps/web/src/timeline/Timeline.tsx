import { useEffect, useRef, useState, type PointerEvent } from "react";
import {
  MAX_FRAME_DELAY_MS,
  MIN_FRAME_DELAY_MS,
  TIMELINE_RATIO_PPM,
} from "../kernel/types";

interface TimelineProps {
  frameCount: number;
  activeFrame: number;
  delays: readonly number[];
  playing: boolean;
  onTogglePlay: () => void;
  onScrubStart: () => void;
  onScrub: (ratioPpm: number) => void;
  onScrubEnd: () => void;
  onDelayFocus: () => void;
  onDelayChange: (delayMs: number) => void;
  onDelayBlur: () => void;
}

/**
 * Scrub surface for the kernel timeline. Pointer position becomes a ratio;
 * the kernel decides which frame that ratio lands on.
 */
export function Timeline({
  frameCount,
  activeFrame,
  delays,
  playing,
  onTogglePlay,
  onScrubStart,
  onScrub,
  onScrubEnd,
  onDelayFocus,
  onDelayChange,
  onDelayBlur,
}: TimelineProps) {
  const dragging = useRef(false);
  const [draft, setDraft] = useState<string | null>(null);
  const delay = delays[activeFrame] ?? MIN_FRAME_DELAY_MS;
  const total = delays.reduce((sum, value) => sum + value, 0);

  useEffect(() => {
    setDraft(null);
  }, [activeFrame]);

  function ratioPpm(event: PointerEvent<HTMLDivElement>): number {
    const track = event.currentTarget;
    const rect = track.getBoundingClientRect();
    const styles = window.getComputedStyle(track);
    const padLeft = Number.parseFloat(styles.paddingLeft) || 0;
    const padRight = Number.parseFloat(styles.paddingRight) || 0;
    const width = track.clientWidth - padLeft - padRight;
    if (width <= 0) {
      return 0;
    }
    const x = event.clientX - rect.left - track.clientLeft - padLeft + track.scrollLeft;
    const ratio = Math.min(1, Math.max(0, x / width));
    return Math.round(ratio * TIMELINE_RATIO_PPM);
  }

  function onPointerDown(event: PointerEvent<HTMLDivElement>) {
    if (event.button !== 0) {
      return;
    }
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    dragging.current = true;
    onScrubStart();
    onScrub(ratioPpm(event));
  }

  function onPointerMove(event: PointerEvent<HTMLDivElement>) {
    if (!dragging.current) {
      return;
    }
    onScrub(ratioPpm(event));
  }

  function endScrub(event: PointerEvent<HTMLDivElement>) {
    if (!dragging.current) {
      return;
    }
    dragging.current = false;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    onScrubEnd();
  }

  function onDelayInput(value: string) {
    setDraft(value);
    if (!/^\d+$/.test(value)) {
      return;
    }
    const parsed = Number(value);
    if (parsed < MIN_FRAME_DELAY_MS || parsed > MAX_FRAME_DELAY_MS) {
      return;
    }
    onDelayChange(parsed);
  }

  return (
    <div className="timeline">
      <div className="transport">
        <button
          type="button"
          aria-pressed={playing}
          aria-keyshortcuts="Space"
          title="空格"
          onClick={onTogglePlay}
        >
          {playing ? "停止" : "播放"}
        </button>
        <label className="delay-field">
          <span>帧 {activeFrame + 1} 时长</span>
          <input
            type="number"
            inputMode="numeric"
            min={MIN_FRAME_DELAY_MS}
            max={MAX_FRAME_DELAY_MS}
            step={1}
            aria-label={`帧 ${activeFrame + 1} 的时长，毫秒`}
            value={draft ?? String(delay)}
            onFocus={() => {
              setDraft(null);
              onDelayFocus();
            }}
            onChange={(event) => onDelayInput(event.target.value)}
            onBlur={() => {
              setDraft(null);
              onDelayBlur();
            }}
          />
          <span>ms</span>
        </label>
        <span className="hint">循环 {total} ms</span>
      </div>
      <div
        className={playing ? "timeline-track is-playing" : "timeline-track"}
        role="slider"
        tabIndex={0}
        aria-label="时间轴"
        aria-orientation="horizontal"
        aria-valuemin={0}
        aria-valuemax={Math.max(0, frameCount - 1)}
        aria-valuenow={activeFrame}
        aria-valuetext={`帧 ${activeFrame + 1}，${delay} 毫秒`}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={endScrub}
        onPointerCancel={endScrub}
      >
        {Array.from({ length: frameCount }, (_, index) => {
          const frameDelay = delays[index] ?? MIN_FRAME_DELAY_MS;
          const selected = index === activeFrame;
          return (
            <div
              key={index}
              className={selected ? "timeline-frame is-active" : "timeline-frame"}
              style={{ flexGrow: frameDelay, flexBasis: 0 }}
              title={`帧 ${index + 1}，${frameDelay} 毫秒`}
            >
              <span>{index + 1}</span>
              <small>{frameDelay}</small>
            </div>
          );
        })}
      </div>
      <p className="hint">拖动时间轴选帧。空格播放或停止，左右方向键逐帧。改时长可以撤销。</p>
    </div>
  );
}
