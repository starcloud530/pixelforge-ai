// Loads the exported base.tmj and tiles.png, then draws the lighthouse sprite.
// Placement is not in the map: export tilemap has no object layer.

const TILE_FLIP_H = 0x80000000;
const TILE_FLIP_V = 0x40000000;
const TILE_FLIP_D = 0x20000000;
const GID_MASK = 0x1fffffff;

const statusEl = document.querySelector("#status");
const view = document.querySelector("#view");
const viewCtx = view.getContext("2d");

const state = {
  tmj: null,
  sheet: null,
  sprite: null,
  spriteName: "lighthouse.png",
  keeper: { x: 4, y: 3 },
  note: "",
  diagonal: 0,
};

function decodeGid(raw) {
  const gid = raw >>> 0;
  return {
    id: gid & GID_MASK,
    flipH: (gid & TILE_FLIP_H) !== 0,
    flipV: (gid & TILE_FLIP_V) !== 0,
    flipD: (gid & TILE_FLIP_D) !== 0,
  };
}

function loadImage(url) {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error(`图片打不开: ${url}`));
    image.src = url;
  });
}

async function fetchJson(url) {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`${url} 返回 ${response.status}`);
  }
  return response.json();
}

function tileLayer(name) {
  const layer = state.tmj.layers.find((item) => item.name === name && item.type === "tilelayer");
  if (!layer) {
    throw new Error(`地图没有图块层 ${name}`);
  }
  return layer;
}

function blocked(x, y) {
  const map = state.tmj;
  if (x < 0 || y < 0 || x >= map.width || y >= map.height) {
    return true;
  }
  const wall = tileLayer("wall");
  return decodeGid(wall.data[y * map.width + x]).id !== 0;
}

function drawMap(ctx) {
  const map = state.tmj;
  const tileset = map.tilesets[0];
  const tw = tileset.tilewidth;
  const th = tileset.tileheight;
  ctx.clearRect(0, 0, map.width * tw, map.height * th);
  for (const layer of map.layers) {
    if (layer.type !== "tilelayer" || layer.visible === false) {
      continue;
    }
    ctx.save();
    ctx.globalAlpha = layer.opacity == null ? 1 : layer.opacity;
    layer.data.forEach((cell, index) => {
      const gid = decodeGid(cell);
      if (gid.id === 0) {
        return;
      }
      if (gid.flipD) {
        state.diagonal += 1;
      }
      const tile = gid.id - tileset.firstgid;
      const sx = (tile % tileset.columns) * tw;
      const sy = Math.floor(tile / tileset.columns) * th;
      const dx = (index % map.width) * tw;
      const dy = Math.floor(index / map.width) * th;
      ctx.save();
      ctx.translate(dx + (gid.flipH ? tw : 0), dy + (gid.flipV ? th : 0));
      ctx.scale(gid.flipH ? -1 : 1, gid.flipV ? -1 : 1);
      ctx.drawImage(state.sheet, sx, sy, tw, th, 0, 0, tw, th);
      ctx.restore();
    });
    ctx.restore();
  }
  const spriteY = map.height * th - state.sprite.height;
  ctx.drawImage(state.sprite, 0, spriteY);
  drawKeeper(ctx, tw, th);
}

function drawKeeper(ctx, tw, th) {
  const x = state.keeper.x * tw + 4;
  const y = state.keeper.y * th + 2;
  ctx.fillStyle = "#e85d4c";
  ctx.fillRect(x + 2, y, 4, 4);
  ctx.fillStyle = "#4a8cc4";
  ctx.fillRect(x, y + 4, 8, 6);
}

function paint() {
  const map = state.tmj;
  const scale = 3;
  const mapW = map.width * map.tilewidth;
  const mapH = map.height * map.tileheight;
  if (!paint.offscreen) {
    paint.offscreen = document.createElement("canvas");
    paint.offscreen.width = mapW;
    paint.offscreen.height = mapH;
    view.width = mapW * scale;
    view.height = mapH * scale;
  }
  state.diagonal = 0;
  drawMap(paint.offscreen.getContext("2d"));
  viewCtx.imageSmoothingEnabled = false;
  viewCtx.clearRect(0, 0, view.width, view.height);
  viewCtx.drawImage(paint.offscreen, 0, 0, view.width, view.height);
  const tileset = map.tilesets[0];
  const diagonal = state.diagonal
    ? ` 有 ${state.diagonal} 格对角翻转，这一版只做了水平/垂直翻转。`
    : "";
  statusEl.textContent =
    `已加载 base.tmj ${map.width}×${map.height}，tiles.png ${tileset.tilecount} 块，` +
    `${state.spriteName} ${state.sprite.width}×${state.sprite.height}。` +
    `看守在 (${state.keeper.x},${state.keeper.y})。${state.note}${diagonal}`;
}

function move(dx, dy) {
  const next = { x: state.keeper.x + dx, y: state.keeper.y + dy };
  if (blocked(next.x, next.y)) {
    state.note = "围墙挡住了。";
  } else {
    state.keeper = next;
    state.note = "";
  }
  paint();
}

const STEPS = {
  ArrowUp: [0, -1],
  ArrowDown: [0, 1],
  ArrowLeft: [-1, 0],
  ArrowRight: [1, 0],
  w: [0, -1],
  a: [-1, 0],
  s: [0, 1],
  d: [1, 0],
};

window.addEventListener("keydown", (event) => {
  const step = STEPS[event.key];
  if (!step || !state.tmj) {
    return;
  }
  event.preventDefault();
  move(step[0], step[1]);
});

async function main() {
  const params = new URLSearchParams(location.search);
  state.spriteName = params.get("sprite") === "before" ? "lighthouse-before.png" : "lighthouse.png";
  const tmjUrl = new URL("../base-tiles/base.tmj", document.baseURI);
  try {
    state.tmj = await fetchJson(tmjUrl);
    const tileset = state.tmj.tilesets[0];
    if (tileset.tilewidth !== 16 || tileset.tileheight !== 16) {
      throw new Error(`图块是 ${tileset.tilewidth}×${tileset.tileheight}，这个场景只接 16×16`);
    }
    const sheetUrl = new URL(tileset.image, tmjUrl);
    state.sheet = await loadImage(sheetUrl.href);
    state.sprite = await loadImage(new URL(state.spriteName, document.baseURI).href);
    paint();
    view.focus();
  } catch (error) {
    const hint = location.protocol === "file:" ? " 请在 examples 目录起一个 HTTP 服务，不要直接打开文件。" : "";
    statusEl.textContent = `${error.message}${hint}`;
  }
}

main();
