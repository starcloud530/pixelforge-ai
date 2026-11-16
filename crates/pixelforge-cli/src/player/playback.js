"use strict";

(function (root, factory) {
  const api = factory();
  if (typeof module === "object" && module.exports) {
    module.exports = api;
  }
  root.PixelforgePlayback = api;
})(typeof globalThis !== "undefined" ? globalThis : this, function () {
  const SCALES = [1, 2, 3, 4, 6, 8];
  const SPEEDS = [0.25, 0.5, 1, 2, 4];
  const BACKGROUNDS = {
    checker: "checker",
    black: "#111111",
    white: "#f4f1de",
    gray: "#808080",
  };

  function pixelSize(width, height, scale) {
    if (!Number.isInteger(scale) || scale < 1) {
      throw new Error("scale must be an integer >= 1");
    }
    return { width: width * scale, height: height * scale };
  }

  function fitIntegerScale(width, height, maxWidth, maxHeight, scales) {
    const choices = (scales || SCALES).filter(function (scale) {
      return Number.isInteger(scale) && scale >= 1;
    });
    if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) {
      return 1;
    }
    if (!Number.isFinite(maxWidth) || !Number.isFinite(maxHeight) || maxWidth <= 0 || maxHeight <= 0) {
      return 1;
    }
    let best = 1;
    for (let index = 0; index < choices.length; index += 1) {
      const scale = choices[index];
      if (width * scale <= maxWidth && height * scale <= maxHeight && scale > best) {
        best = scale;
      }
    }
    return best;
  }

  function backgroundPaint(name) {
    return BACKGROUNDS[name] || BACKGROUNDS.checker;
  }

  function paintFrame(ctx, image, width, height, scale, background) {
    const size = pixelSize(width, height, scale);
    if (ctx.canvas.width !== size.width) {
      ctx.canvas.width = size.width;
    }
    if (ctx.canvas.height !== size.height) {
      ctx.canvas.height = size.height;
    }
    ctx.imageSmoothingEnabled = false;
    if (background === "checker") {
      const cell = scale;
      for (let y = 0; y < size.height; y += cell) {
        for (let x = 0; x < size.width; x += cell) {
          const on = (x / cell + y / cell) % 2 === 0;
          ctx.fillStyle = on ? "#d5d0c6" : "#9a958b";
          ctx.fillRect(x, y, cell, cell);
        }
      }
    } else {
      ctx.fillStyle = background;
      ctx.fillRect(0, 0, size.width, size.height);
    }
    ctx.imageSmoothingEnabled = false;
    if (image) {
      ctx.drawImage(image, 0, 0, size.width, size.height);
    }
  }

  function copySteps(steps) {
    return steps.map(function (item) {
      return { frame: item.frame, delayMs: item.delayMs };
    });
  }

  function snapshotReady(next) {
    if (!next || next.ok !== true) {
      return false;
    }
    if (!Array.isArray(next.actions) || next.actions.length === 0) {
      return false;
    }
    if (!Array.isArray(next.frames) || next.frames.length === 0) {
      return false;
    }
    for (let actionIndex = 0; actionIndex < next.actions.length; actionIndex += 1) {
      const action = next.actions[actionIndex];
      if (!action || !Array.isArray(action.steps) || action.steps.length === 0) {
        return false;
      }
      for (let stepIndex = 0; stepIndex < action.steps.length; stepIndex += 1) {
        const step = action.steps[stepIndex];
        const found = next.frames.some(function (frame) {
          return frame.index === step.frame;
        });
        if (!found) {
          return false;
        }
      }
    }
    return true;
  }

  function createPlayback(snapshot) {
    if (!snapshotReady(snapshot)) {
      throw new Error("snapshot is not ready");
    }

    let view = snapshot;
    let actionIndex = 0;
    let step = 0;
    let playing = false;
    let finished = false;
    let speed = 1;
    let error = null;
    let shownAt = 0;
    let scale = 4;
    let background = "checker";
    let intoMs = 0;
    let lastNow = 0;

    function action() {
      return view.actions[actionIndex];
    }

    function play(now) {
      lastNow = now;
      if (error) {
        return;
      }
      if (!action().steps.length) {
        return;
      }
      if (action().plays === 1 && finished) {
        step = 0;
        intoMs = 0;
        finished = false;
        shownAt = now;
        playing = true;
        return;
      }
      const delay = action().steps[step].delayMs;
      const wait = delay / speed;
      const fraction = delay > 0 ? intoMs / delay : 0;
      shownAt = now - fraction * wait;
      playing = true;
    }

    function pause(now) {
      if (playing && !error) {
        const at = Number.isFinite(now) ? now : lastNow;
        tick(at);
        const delay = action().steps[step].delayMs;
        intoMs = finished ? delay : Math.min(delay, Math.max(0, (at - shownAt) * speed));
      }
      playing = false;
    }

    function tick(now) {
      lastNow = now;
      if (!playing || error) {
        return;
      }
      const steps = action().steps;
      const count = steps.length;
      if (count === 0) {
        playing = false;
        return;
      }
      const elapsed = now - shownAt;
      if (!(elapsed > 0)) {
        return;
      }
      const wait = steps.map(function (item) {
        return item.delayMs / speed;
      });
      if (action().plays === 1) {
        let left = elapsed;
        for (let index = step; index < count; index += 1) {
          if (left < wait[index]) {
            step = index;
            shownAt = now - left;
            return;
          }
          left -= wait[index];
        }
        step = count - 1;
        finished = true;
        playing = false;
        shownAt = now;
        return;
      }
      const cycle = wait.reduce(function (sum, value) {
        return sum + value;
      }, 0);
      if (!(cycle > 0)) {
        shownAt = now;
        return;
      }
      let origin = 0;
      for (let index = 0; index < step; index += 1) {
        origin += wait[index];
      }
      const into = (origin + elapsed) % cycle;
      let left = into;
      for (let index = 0; index < count; index += 1) {
        if (left < wait[index]) {
          step = index;
          shownAt = now - left;
          return;
        }
        left -= wait[index];
      }
      step = 0;
      shownAt = now;
    }

    function prefixMs(index) {
      const steps = action().steps;
      let sum = 0;
      const end = Math.max(0, Math.min(index, steps.length));
      for (let cursor = 0; cursor < end; cursor += 1) {
        sum += steps[cursor].delayMs;
      }
      return sum;
    }

    function durationMs() {
      return prefixMs(action().steps.length);
    }

    function sourceTimeMs(now) {
      const steps = action().steps;
      if (!steps.length) {
        return 0;
      }
      if (finished) {
        return durationMs();
      }
      const delay = steps[step].delayMs;
      if (playing) {
        const wait = delay / speed;
        const spent = Math.min(Math.max(0, now - shownAt), wait);
        const fraction = wait > 0 ? spent / wait : 0;
        return prefixMs(step) + fraction * delay;
      }
      return prefixMs(step) + intoMs;
    }

    function seekSource(ms, now) {
      const steps = action().steps;
      if (!steps.length) {
        return;
      }
      pause(now);
      const total = durationMs();
      const clamped = Math.max(0, Math.min(Number(ms) || 0, total));
      let acc = 0;
      let index = steps.length - 1;
      let into = steps[index].delayMs;
      for (let cursor = 0; cursor < steps.length; cursor += 1) {
        const next = acc + steps[cursor].delayMs;
        if (clamped < next || cursor === steps.length - 1) {
          index = cursor;
          into = Math.max(0, Math.min(steps[cursor].delayMs, clamped - acc));
          break;
        }
        acc = next;
      }
      step = index;
      intoMs = into;
      finished = action().plays === 1 && total > 0 && clamped >= total;
      const wait = steps[index].delayMs / speed;
      const fraction = steps[index].delayMs > 0 ? into / steps[index].delayMs : 0;
      shownAt = now - fraction * wait;
    }

    function stepBy(delta, now) {
      const count = action().steps.length;
      if (!count) {
        return;
      }
      pause(now);
      finished = false;
      step = Math.max(0, Math.min(count - 1, step + delta));
      intoMs = 0;
      shownAt = now;
    }

    function jumpTo(index, now) {
      const count = action().steps.length;
      if (index < 0 || index >= count) {
        return;
      }
      pause(now);
      finished = false;
      step = index;
      intoMs = 0;
      shownAt = now;
    }

    function toStart(now) {
      jumpTo(0, now);
    }

    function speedAllowed(next) {
      if (!Number.isFinite(next) || next < 0.25 || next > 4) {
        return false;
      }
      const snapped = Math.round(next * 4) / 4;
      return Math.abs(snapped - next) < 1e-6;
    }

    function setSpeed(next, now) {
      if (!speedAllowed(next)) {
        return;
      }
      if (playing && !error) {
        const oldWait = action().steps[step].delayMs / speed;
        const spent = Math.min(Math.max(0, now - shownAt), oldWait);
        const fraction = oldWait > 0 ? spent / oldWait : 0;
        speed = next;
        const nextWait = action().steps[step].delayMs / speed;
        shownAt = now - fraction * nextWait;
      } else {
        speed = next;
      }
    }

    function selectAction(id, now) {
      const found = view.actions.findIndex(function (item) {
        return item.id === id;
      });
      if (found < 0) {
        return;
      }
      actionIndex = found;
      step = 0;
      intoMs = 0;
      finished = false;
      shownAt = now;
    }

    function setScale(next) {
      if (SCALES.indexOf(next) < 0) {
        return;
      }
      scale = next;
    }

    function setBackground(next) {
      if (!Object.prototype.hasOwnProperty.call(BACKGROUNDS, next)) {
        return;
      }
      background = next;
    }

    function applySnapshot(next, now) {
      if (!next || next.ok !== true) {
        pause(now);
        error = (next && next.error) || "无法读取工程";
        return { swapped: false, error: error };
      }
      if (!snapshotReady(next)) {
        pause(now);
        error = "快照不完整";
        return { swapped: false, error: error };
      }
      const previous = { kind: action().kind, id: action().id };
      const keepStep = step;
      const wasPlaying = playing;
      intoMs = 0;
      view = {
        ok: true,
        revision: next.revision,
        fileName: next.fileName,
        width: next.width,
        height: next.height,
        readOnly: true,
        frames: next.frames,
        actions: next.actions,
      };
      error = null;
      const found = view.actions.findIndex(function (item) {
        return item.kind === previous.kind && item.id === previous.id;
      });
      if (found >= 0) {
        actionIndex = found;
        const max = view.actions[found].steps.length - 1;
        step = Math.min(keepStep, Math.max(0, max));
      } else {
        actionIndex = 0;
        step = 0;
      }
      finished = false;
      playing = wasPlaying;
      shownAt = now;
      return { swapped: true, error: null };
    }

    function currentFrame() {
      const item = action().steps[step];
      for (let index = 0; index < view.frames.length; index += 1) {
        if (view.frames[index].index === item.frame) {
          return view.frames[index];
        }
      }
      return null;
    }

    function state() {
      const frame = currentFrame();
      const item = action().steps[step];
      return {
        revision: view.revision,
        fileName: view.fileName,
        width: view.width,
        height: view.height,
        actionId: action().id,
        actionName: action().name,
        kind: action().kind,
        loop: action().loop,
        plays: action().plays,
        step: step,
        stepCount: action().steps.length,
        frame: item.frame,
        delayMs: item.delayMs,
        previewMs: item.delayMs / speed,
        speed: speed,
        playing: playing,
        finished: finished,
        error: error,
        scale: scale,
        background: background,
        png: frame ? frame.png : null,
        steps: copySteps(action().steps),
        frames: view.frames.map(function (entry) {
          return { index: entry.index, delayMs: entry.delayMs, png: entry.png };
        }),
        actions: view.actions.map(function (entry) {
          return {
            id: entry.id,
            name: entry.name,
            kind: entry.kind,
            loop: entry.loop,
            plays: entry.plays,
          };
        }),
      };
    }

    return {
      play: play,
      pause: pause,
      tick: tick,
      stepBy: stepBy,
      jumpTo: jumpTo,
      toStart: toStart,
      duration: durationMs,
      sourceTime: sourceTimeMs,
      seekSource: seekSource,
      setSpeed: setSpeed,
      selectAction: selectAction,
      setScale: setScale,
      setBackground: setBackground,
      applySnapshot: applySnapshot,
      state: state,
    };
  }

  return {
    SCALES: SCALES,
    SPEEDS: SPEEDS,
    BACKGROUNDS: BACKGROUNDS,
    createPlayback: createPlayback,
    paintFrame: paintFrame,
    pixelSize: pixelSize,
    fitIntegerScale: fitIntegerScale,
    backgroundPaint: backgroundPaint,
    snapshotReady: snapshotReady,
  };
});
