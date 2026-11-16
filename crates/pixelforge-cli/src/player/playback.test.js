"use strict";

const assert = require("node:assert/strict");
const { test } = require("node:test");
const {
  SCALES,
  SPEEDS,
  createPlayback,
  paintFrame,
  pixelSize,
  backgroundPaint,
} = require("./playback.js");

function clip(actions, frames) {
  return {
    ok: true,
    revision: "aaaaaaaaaaaaaaaa",
    fileName: "clip.json",
    width: 4,
    height: 2,
    readOnly: true,
    frames:
      frames ||
      [0, 1, 2, 3].map(function (index) {
        return { index: index, delayMs: [40, 200, 80, 120][index], png: "png-" + index };
      }),
    actions: actions,
  };
}

function action(id, loop, plays, steps) {
  return {
    id: id,
    name: id === "timeline" ? "全部帧" : id,
    kind: id === "timeline" ? "timeline" : "tag",
    loop: loop,
    plays: plays,
    steps: steps,
  };
}

function steps() {
  return [
    { frame: 0, delayMs: 40 },
    { frame: 1, delayMs: 200 },
    { frame: 2, delayMs: 80 },
  ];
}

function playerOf(actions) {
  return createPlayback(clip(actions));
}

test("integer scale is nearest-neighbor and rejects a fractional size", function () {
  assert.deepEqual(pixelSize(5, 7, 4), { width: 20, height: 28 });
  assert.throws(function () {
    pixelSize(5, 7, 1.5);
  });
  const ctx = {
    canvas: { width: 0, height: 0 },
    imageSmoothingEnabled: true,
    fillStyle: "",
    ops: [],
    fillRect: function (x, y, w, h) {
      this.ops.push(["fill", x, y, w, h, this.fillStyle]);
    },
    drawImage: function (image, x, y, w, h) {
      this.ops.push(["draw", x, y, w, h, this.imageSmoothingEnabled, image]);
    },
  };
  paintFrame(ctx, "sprite", 2, 1, 3, "checker");
  assert.equal(ctx.canvas.width, 6);
  assert.equal(ctx.canvas.height, 3);
  const draw = ctx.ops.find(function (op) {
    return op[0] === "draw";
  });
  assert.deepEqual(draw.slice(0, 6), ["draw", 0, 0, 6, 3, false]);
  assert.ok(
    ctx.ops.every(function (op) {
      return Number.isInteger(op[1]) && Number.isInteger(op[2]);
    })
  );
  ctx.ops = [];
  paintFrame(ctx, null, 2, 1, 2, backgroundPaint("black"));
  assert.equal(ctx.ops[0][5], "#111111");
  assert.equal(ctx.ops.some(function (op) { return op[0] === "draw"; }), false);
});

test("unequal delays advance on the project clock", function () {
  const player = playerOf([action("go", "forward", 0, steps())]);
  player.play(0);
  player.tick(39);
  assert.equal(player.state().frame, 0);
  player.tick(40);
  assert.equal(player.state().frame, 1);
  player.tick(239);
  assert.equal(player.state().frame, 1);
  player.tick(240);
  assert.equal(player.state().frame, 2);
  player.tick(319);
  assert.equal(player.state().frame, 2);
  player.tick(320);
  assert.equal(player.state().frame, 0);
  assert.equal(player.state().playing, true);
});

test("four playback modes walk the server order and do not reorder it", function () {
  const forward = steps();
  const reverse = [
    { frame: 2, delayMs: 80 },
    { frame: 1, delayMs: 200 },
    { frame: 0, delayMs: 40 },
  ];
  const ping = [
    { frame: 0, delayMs: 40 },
    { frame: 1, delayMs: 200 },
    { frame: 2, delayMs: 80 },
    { frame: 1, delayMs: 200 },
  ];
  const once = [
    { frame: 1, delayMs: 200 },
    { frame: 2, delayMs: 80 },
  ];
  const player = playerOf([
    action("go", "forward", 0, forward),
    action("back", "reverse", 0, reverse),
    action("bounce", "pingpong", 0, ping),
    action("hit", "once", 1, once),
  ]);
  player.selectAction("back", 0);
  player.play(0);
  assert.equal(player.state().frame, 2);
  player.tick(80);
  assert.equal(player.state().frame, 1);
  player.tick(280);
  assert.equal(player.state().frame, 0);
  player.selectAction("bounce", 300);
  player.play(300);
  const seen = [player.state().frame];
  let now = 300;
  for (let index = 0; index < ping.length; index += 1) {
    now += ping[index].delayMs;
    player.tick(now);
    seen.push(player.state().frame);
  }
  assert.deepEqual(seen, [0, 1, 2, 1, 0]);
  player.selectAction("hit", now);
  player.play(now);
  player.tick(now + 279);
  assert.equal(player.state().finished, false);
  assert.equal(player.state().frame, 2);
  player.tick(now + 280);
  assert.equal(player.state().finished, true);
  assert.equal(player.state().playing, false);
  assert.equal(player.state().frame, 2);
});

test("once holds the last frame and replay starts at the playback start", function () {
  const player = playerOf([
    action("hit", "once", 1, [
      { frame: 2, delayMs: 80 },
      { frame: 0, delayMs: 40 },
    ]),
  ]);
  player.play(0);
  player.tick(120);
  assert.equal(player.state().finished, true);
  assert.equal(player.state().frame, 0);
  assert.equal(player.state().step, 1);
  player.play(200);
  assert.equal(player.state().finished, false);
  assert.equal(player.state().playing, true);
  assert.equal(player.state().step, 0);
  assert.equal(player.state().frame, 2);
});

test("pause, step, and resume do not restart a once action", function () {
  const player = playerOf([action("go", "forward", 0, steps())]);
  player.play(0);
  player.tick(40);
  player.pause();
  assert.equal(player.state().playing, false);
  assert.equal(player.state().frame, 1);
  player.tick(5000);
  assert.equal(player.state().frame, 1);
  player.stepBy(1, 5000);
  assert.equal(player.state().frame, 2);
  player.stepBy(-1, 5000);
  assert.equal(player.state().frame, 1);
  player.stepBy(-1, 5000);
  assert.equal(player.state().frame, 0);
  player.stepBy(-1, 5000);
  assert.equal(player.state().frame, 2);
  player.play(5000);
  player.tick(5079);
  assert.equal(player.state().frame, 2);
  player.tick(5080);
  assert.equal(player.state().frame, 0);
});

test("speed changes the preview clock and leaves frame delays alone", function () {
  const player = playerOf([action("go", "forward", 0, steps())]);
  const before = player.state().steps.map(function (item) {
    return item.delayMs;
  });
  player.setSpeed(4, 0);
  player.play(0);
  assert.deepEqual(
    player.state().steps.map(function (item) {
      return item.delayMs;
    }),
    before
  );
  assert.equal(player.state().delayMs, 40);
  assert.equal(player.state().previewMs, 10);
  player.tick(9);
  assert.equal(player.state().frame, 0);
  player.tick(10);
  assert.equal(player.state().frame, 1);
  assert.equal(player.state().delayMs, 200);
  assert.equal(player.state().previewMs, 50);
  player.setSpeed(0.25, 10);
  assert.equal(player.state().delayMs, 200);
  assert.equal(player.state().previewMs, 800);
  player.setSpeed(7, 10);
  assert.equal(player.state().speed, 0.25);
  assert.ok(SPEEDS.indexOf(player.state().speed) >= 0);
});

test("a single frame and a timeline without tags still play", function () {
  const once = createPlayback(
    clip(
      [action("still", "once", 1, [{ frame: 0, delayMs: 100 }])],
      [{ index: 0, delayMs: 100, png: "only" }]
    )
  );
  once.play(0);
  once.tick(100);
  assert.equal(once.state().finished, true);
  assert.equal(once.state().frame, 0);
  assert.equal(once.state().png, "only");
  once.stepBy(1, 100);
  assert.equal(once.state().frame, 0);
  once.play(200);
  assert.equal(once.state().step, 0);
  assert.equal(once.state().playing, true);

  const timeline = createPlayback(
    clip([
      action("timeline", "forward", 0, [
        { frame: 0, delayMs: 40 },
        { frame: 1, delayMs: 200 },
      ]),
    ])
  );
  assert.equal(timeline.state().kind, "timeline");
  assert.equal(timeline.state().actionName, "全部帧");
  timeline.play(0);
  timeline.tick(40);
  assert.equal(timeline.state().frame, 1);
});

test("a failed refresh pauses and keeps the last frames, then a good file reloads", function () {
  const player = playerOf([
    action("go", "forward", 0, steps()),
    action("hit", "once", 1, [
      { frame: 1, delayMs: 200 },
      { frame: 2, delayMs: 80 },
    ]),
  ]);
  player.selectAction("hit", 0);
  player.jumpTo(1, 0);
  player.play(0);
  const kept = player.state().frames.map(function (frame) {
    return frame.png;
  });
  const failed = player.applySnapshot({ ok: false, revision: player.state().revision, error: "project file is empty" }, 30);
  assert.equal(failed.swapped, false);
  assert.equal(player.state().playing, false);
  assert.equal(player.state().error, "project file is empty");
  assert.equal(player.state().frame, 2);
  assert.deepEqual(
    player.state().frames.map(function (frame) {
      return frame.png;
    }),
    kept
  );
  player.tick(10000);
  assert.equal(player.state().frame, 2);

  const repaired = clip(
    [
      action("go", "forward", 0, steps()),
      action("hit", "once", 1, [
        { frame: 1, delayMs: 200 },
        { frame: 2, delayMs: 80 },
      ]),
    ],
    [0, 1, 2, 3].map(function (index) {
      return { index: index, delayMs: [40, 200, 80, 120][index], png: "new-" + index };
    })
  );
  repaired.revision = "bbbbbbbbbbbbbbbb";
  const restored = player.applySnapshot(repaired, 40);
  assert.equal(restored.swapped, true);
  assert.equal(player.state().error, null);
  assert.equal(player.state().playing, false);
  assert.equal(player.state().actionId, "hit");
  assert.equal(player.state().step, 1);
  assert.equal(player.state().png, "new-2");
  assert.deepEqual(
    player.state().frames.map(function (frame) {
      return frame.png;
    }),
    repaired.frames.map(function (frame) {
      return frame.png;
    })
  );
});

test("a new snapshot replaces actions and frames together", function () {
  const player = playerOf([action("idle", "forward", 0, [{ frame: 0, delayMs: 40 }])]);
  const next = clip(
    [action("attack", "once", 1, [{ frame: 1, delayMs: 90 }])],
    [
      { index: 0, delayMs: 40, png: "gone" },
      { index: 1, delayMs: 90, png: "attack-png" },
    ]
  );
  next.revision = "cccccccccccccccc";
  player.play(0);
  player.applySnapshot(next, 15);
  const state = player.state();
  assert.deepEqual(
    state.actions.map(function (item) {
      return item.id;
    }),
    ["attack"]
  );
  assert.deepEqual(
    state.frames.map(function (frame) {
      return frame.png;
    }),
    ["gone", "attack-png"]
  );
  assert.equal(state.actionId, "attack");
  assert.equal(state.png, "attack-png");
  assert.equal(state.playing, true);
  assert.equal(
    state.frames.some(function (frame) {
      return frame.png === "png-0";
    }),
    false
  );
});

test("an incomplete snapshot is refused and the previous picture stays", function () {
  const player = playerOf([action("go", "forward", 0, steps())]);
  const broken = clip([action("go", "forward", 0, [{ frame: 9, delayMs: 40 }])]);
  const result = player.applySnapshot(broken, 5);
  assert.equal(result.swapped, false);
  assert.equal(player.state().error, "快照不完整");
  assert.equal(player.state().png, "png-0");
  assert.equal(player.state().playing, false);
});

test("scale and background stay on the listed preview values", function () {
  const player = playerOf([action("go", "forward", 0, steps())]);
  assert.equal(player.state().scale, 4);
  assert.equal(player.state().background, "checker");
  player.setScale(8);
  player.setBackground("white");
  assert.equal(player.state().scale, 8);
  assert.equal(player.state().background, "white");
  player.setScale(5);
  player.setBackground("magenta");
  assert.equal(player.state().scale, 8);
  assert.equal(player.state().background, "white");
  assert.deepEqual(SCALES, [1, 2, 3, 4, 6, 8]);
});
