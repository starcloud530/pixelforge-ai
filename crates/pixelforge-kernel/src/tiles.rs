//! Tileset size, 4-neighbor autotile, tilemap cells, and Tiled `.tmj` export.
//!
//! A tileset is a [`crate::Document`] whose canvas is one tile (16 or 32 on
//! each edge) and whose frames are the tiles, in id order. Paint, palette,
//! preview, and diff stay the pixel tools. A tilemap is a separate
//! `pixelforge-tilemap` file: each cell stores a tile id plus an optional
//! horizontal or vertical flip. It is not a second pixel buffer.
//!
//! Autotile reads a layer's occupied cells (any tile id counts, an empty cell
//! does not). The four neighbors are north, east, south, and west, bits 0..3
//! with values 1, 2, 4, and 8. A cell outside the map is empty. The rule for
//! that mask replaces the cell. Rules are a `pixelforge-autotile` JSON file
//! with all 16 masks. The same input writes the same cells.
//!
//! Eight-neighbor blob autotile is refused. There is no map editor in this
//! crate.

use serde_json::{Map, Number, Value};

use crate::document::Document;
use crate::error::KernelError;
use crate::MAX_LAYER_NAME_CHARS;
use crate::MAX_TILEMAP_EDGE;
use crate::MAX_UNDO;
use crate::TOOL_API_VERSION;

/// `format` of a tilemap file.
pub const TILEMAP_FORMAT: &str = "pixelforge-tilemap";

/// `format` of tileset metadata written next to the tileset PNG.
pub const TILESET_FORMAT: &str = "pixelforge-tileset";

/// `format` of a 4-neighbor autotile rule file.
pub const AUTOTILE_FORMAT: &str = "pixelforge-autotile";

/// North neighbor. Bit 0.
pub const AUTOTILE_NORTH: u8 = 1;
/// East neighbor. Bit 1.
pub const AUTOTILE_EAST: u8 = 2;
/// South neighbor. Bit 2.
pub const AUTOTILE_SOUTH: u8 = 4;
/// West neighbor. Bit 3.
pub const AUTOTILE_WEST: u8 = 8;

/// Tiled GID flag. The low 29 bits stay the tile id plus `firstgid`.
pub const FLIPPED_HORIZONTALLY_FLAG: u32 = 0x8000_0000;
/// Tiled GID flag. Combined with the horizontal flag, this is a 180° turn.
pub const FLIPPED_VERTICALLY_FLAG: u32 = 0x4000_0000;

/// Tool names [`TileMap::call_tool`] accepts. They are not pixel-document tools.
pub const TILEMAP_TOOL_NAMES: &[&str] = &[
    "inspectTilemap",
    "getTile",
    "setTile",
    "fillTiles",
    "clearTiles",
    "addTileLayer",
    "autotile",
    "validateTilemap",
    "exportTilemap",
    "undo",
    "redo",
];

const TILEMAP_VERSION: u64 = 1;
const AUTOTILE_VERSION: u64 = 1;
const TILESET_VERSION: u64 = 1;
const MAX_TILE_COUNT: u32 = 65_536;
const MAX_TILE_LAYERS: usize = 32;
const MAX_TILESET_PATH: usize = 256;

/// Declared tile size stored on a pixel project. Omitted from the file when unset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TilesetDecl {
    pub tile_width: u16,
    pub tile_height: u16,
}

impl TilesetDecl {
    /// 16 or 32 on both edges, and the canvas is exactly one tile.
    pub fn aligned(
        tile_width: u16,
        tile_height: u16,
        width: u16,
        height: u16,
    ) -> Result<Self, KernelError> {
        if !tile_edge(tile_width)
            || !tile_edge(tile_height)
            || tile_width != width
            || tile_height != height
        {
            return Err(KernelError::TileAlign {
                tile_width,
                tile_height,
                width,
                height,
            });
        }
        Ok(Self {
            tile_width,
            tile_height,
        })
    }

    /// 16 or 32 on both edges. A tilemap cell uses this, not a pixel canvas.
    pub fn edges(tile_width: u16, tile_height: u16) -> Result<Self, KernelError> {
        Self::aligned(tile_width, tile_height, tile_width, tile_height)
    }
}

fn tile_edge(edge: u16) -> bool {
    edge == 16 || edge == 32
}

/// What [`crate::Document::validate_tileset`] reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TilesetReport {
    pub tile_width: u16,
    pub tile_height: u16,
    pub tile_count: u32,
}

/// Tileset PNG plus `pixelforge-tileset` metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct TilesetExport {
    pub png: Vec<u8>,
    pub meta: Value,
    pub columns: u32,
    pub rows: u32,
    pub scale: u32,
    pub image_width: u32,
    pub image_height: u32,
    pub tile_width: u32,
    pub tile_height: u32,
    pub source_tile_width: u16,
    pub source_tile_height: u16,
    pub tile_count: u32,
    pub image: String,
}

/// Map preview PNG plus a Tiled JSON map.
#[derive(Debug, Clone, PartialEq)]
pub struct TilemapExport {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub tmj: Value,
}

impl TilemapExport {
    /// Pretty `.tmj` text, including a trailing newline.
    pub fn tmj_pretty(&self) -> String {
        let mut json = serde_json::to_string_pretty(&self.tmj).expect("tmj is JSON");
        json.push('\n');
        json
    }
}

/// One map cell. `tile` is a 0-based tileset frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileCell {
    pub tile: u32,
    pub flip_h: bool,
    pub flip_v: bool,
}

impl TileCell {
    pub fn plain(tile: u32) -> Self {
        Self {
            tile,
            flip_h: false,
            flip_v: false,
        }
    }

    /// Tiled GID with `firstgid` 1. Empty cells are 0 and are not a [`TileCell`].
    pub fn tiled_gid(self) -> u32 {
        let mut gid = self.tile.saturating_add(1);
        if self.flip_h {
            gid |= FLIPPED_HORIZONTALLY_FLAG;
        }
        if self.flip_v {
            gid |= FLIPPED_VERTICALLY_FLAG;
        }
        gid
    }
}

/// One rule in a 16-entry table. The index is the neighbor mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutotileRule {
    pub tile: u32,
    pub flip_h: bool,
    pub flip_v: bool,
}

/// All 16 four-neighbor masks. Index `mask` is that combination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutotileRules {
    rules: [AutotileRule; 16],
}

impl AutotileRules {
    pub fn rule(&self, mask: u8) -> AutotileRule {
        self.rules[usize::from(mask & 15)]
    }

    pub fn rules(&self) -> &[AutotileRule; 16] {
        &self.rules
    }

    /// Every rule tile is inside `0..tile_count`.
    pub fn check_tiles(&self, tile_count: u32) -> Result<(), KernelError> {
        for rule in &self.rules {
            if rule.tile >= tile_count {
                return Err(KernelError::TileId {
                    tile: rule.tile,
                    tile_count,
                });
            }
        }
        Ok(())
    }
}

/// 4-neighbor mask. Bits are north, east, south, west. Outside the map is empty.
pub fn neighbor_mask(occupied: &[bool], width: u32, height: u32, x: u32, y: u32) -> u8 {
    let mut mask = 0u8;
    if y > 0 && occupied_at(occupied, width, x, y - 1) {
        mask |= AUTOTILE_NORTH;
    }
    if x + 1 < width && occupied_at(occupied, width, x + 1, y) {
        mask |= AUTOTILE_EAST;
    }
    if y + 1 < height && occupied_at(occupied, width, x, y + 1) {
        mask |= AUTOTILE_SOUTH;
    }
    if x > 0 && occupied_at(occupied, width, x - 1, y) {
        mask |= AUTOTILE_WEST;
    }
    mask
}

fn occupied_at(occupied: &[bool], width: u32, x: u32, y: u32) -> bool {
    occupied
        .get(y as usize * width as usize + x as usize)
        .copied()
        .unwrap_or(false)
}

/// Reads version 1 `pixelforge-autotile` JSON. All 16 masks are required.
pub fn parse_autotile(json: &str) -> Result<AutotileRules, KernelError> {
    let json = json.strip_prefix('\u{feff}').unwrap_or(json);
    let value: Value =
        serde_json::from_str(json).map_err(|_| autotile_file("autotile file is not JSON"))?;
    let map = value
        .as_object()
        .ok_or_else(|| autotile_file("autotile file must be a JSON object"))?;
    reject_keys(
        map,
        &["format", "version", "neighbors", "bits", "rules"],
        |field| autotile_file(format!("autotile file has unknown field {field}")),
    )?;
    match map.get("format") {
        Some(Value::String(name)) if name == AUTOTILE_FORMAT => {}
        _ => {
            return Err(autotile_file(
                "autotile file format is not pixelforge-autotile",
            ))
        }
    }
    match map.get("version").and_then(Value::as_u64) {
        Some(AUTOTILE_VERSION) => {}
        _ => return Err(autotile_file("autotile file version is not 1")),
    }
    let neighbors = map
        .get("neighbors")
        .and_then(Value::as_u64)
        .ok_or_else(|| autotile_file("autotile file is missing neighbors"))?;
    if neighbors != 4 {
        let neighbors = u32::try_from(neighbors).unwrap_or(u32::MAX);
        return Err(KernelError::AutotileNeighbors { neighbors });
    }
    if let Some(bits) = map.get("bits") {
        let expected = ["n", "e", "s", "w"];
        let list = bits
            .as_array()
            .ok_or_else(|| autotile_file("autotile bits must be an array"))?;
        let ok = list.len() == 4
            && list
                .iter()
                .enumerate()
                .all(|(index, value)| value.as_str() == Some(expected[index]));
        if !ok {
            return Err(KernelError::AutotileRule {
                detail: "autotile bit order must be n, e, s, w".to_string(),
            });
        }
    }
    let rules = map
        .get("rules")
        .and_then(Value::as_array)
        .ok_or_else(|| autotile_file("autotile file is missing rules"))?;
    let mut found = [None; 16];
    for rule in rules {
        let rule = rule
            .as_object()
            .ok_or_else(|| autotile_file("autotile rule must be an object"))?;
        reject_keys(rule, &["mask", "tile", "flipH", "flipV"], |field| {
            autotile_file(format!("autotile rule has unknown field {field}"))
        })?;
        let mask = rule
            .get("mask")
            .and_then(Value::as_u64)
            .ok_or_else(|| autotile_file("autotile rule is missing mask"))?;
        if mask > 15 {
            return Err(KernelError::AutotileRule {
                detail: format!("autotile rule mask {mask} is outside 0..=15"),
            });
        }
        let mask = mask as usize;
        if found[mask].is_some() {
            return Err(KernelError::AutotileRule {
                detail: format!("autotile rules repeat mask {mask}"),
            });
        }
        let tile = rule
            .get("tile")
            .and_then(Value::as_u64)
            .and_then(|tile| u32::try_from(tile).ok())
            .ok_or_else(|| autotile_file(format!("autotile rule mask {mask} is missing tile")))?;
        let flip_h = optional_bool(rule, "flipH")?;
        let flip_v = optional_bool(rule, "flipV")?;
        found[mask] = Some(AutotileRule {
            tile,
            flip_h,
            flip_v,
        });
    }
    let mut missing = Vec::new();
    let mut rules = [AutotileRule {
        tile: 0,
        flip_h: false,
        flip_v: false,
    }; 16];
    for (mask, rule) in found.into_iter().enumerate() {
        match rule {
            Some(rule) => rules[mask] = rule,
            None => missing.push(mask as u8),
        }
    }
    if !missing.is_empty() {
        let list = missing
            .iter()
            .map(|mask| mask.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(KernelError::AutotileRule {
            detail: format!("autotile rules are missing masks {list}"),
        });
    }
    Ok(AutotileRules { rules })
}

fn optional_bool(map: &Map<String, Value>, key: &str) -> Result<bool, KernelError> {
    match map.get(key) {
        None => Ok(false),
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(autotile_file(format!("autotile {key} must be a boolean"))),
    }
}

fn autotile_file(detail: impl Into<String>) -> KernelError {
    KernelError::AutotileFile {
        detail: detail.into(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TileLayer {
    name: String,
    cells: Vec<Option<TileCell>>,
}

/// W×H×layers grid of tile ids. The file does not store undo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileMap {
    width: u16,
    height: u16,
    tile_width: u16,
    tile_height: u16,
    tile_count: u32,
    tileset: String,
    layers: Vec<TileLayer>,
    undo: Vec<Vec<TileLayer>>,
    redo: Vec<Vec<TileLayer>>,
}

impl TileMap {
    /// One empty layer. `tileset` is stored as given and is not opened here.
    pub fn new(
        width: u16,
        height: u16,
        tile_width: u16,
        tile_height: u16,
        tile_count: u32,
        tileset: &str,
        layer_name: &str,
    ) -> Result<Self, KernelError> {
        check_map_size(width, height)?;
        let decl = TilesetDecl::edges(tile_width, tile_height)?;
        check_tile_count(tile_count)?;
        let tileset = normalize_tileset_path(tileset)?;
        let name = normalize_tile_layer(layer_name)?;
        let cells = vec![None; usize::from(width) * usize::from(height)];
        Ok(Self {
            width,
            height,
            tile_width: decl.tile_width,
            tile_height: decl.tile_height,
            tile_count,
            tileset,
            layers: vec![TileLayer { name, cells }],
            undo: Vec::new(),
            redo: Vec::new(),
        })
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn tile_width(&self) -> u16 {
        self.tile_width
    }

    pub fn tile_height(&self) -> u16 {
        self.tile_height
    }

    pub fn tile_count(&self) -> u32 {
        self.tile_count
    }

    pub fn tileset(&self) -> &str {
        &self.tileset
    }

    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    pub fn layer_name(&self, index: usize) -> Option<&str> {
        self.layers.get(index).map(|layer| layer.name.as_str())
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Pretty version 1 tilemap JSON, including a trailing newline.
    pub fn save(&self) -> String {
        let layers = self
            .layers
            .iter()
            .map(|layer| {
                serde_json::json!({
                    "name": layer.name,
                    "cells": layer.cells.iter().copied().map(cell_json).collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>();
        let value = serde_json::json!({
            "format": TILEMAP_FORMAT,
            "version": TILEMAP_VERSION,
            "width": self.width,
            "height": self.height,
            "tileWidth": self.tile_width,
            "tileHeight": self.tile_height,
            "tileCount": self.tile_count,
            "tileset": self.tileset,
            "layers": layers,
        });
        let mut json = serde_json::to_string_pretty(&value).expect("tilemap is JSON");
        json.push('\n');
        json
    }

    /// Reads version 1 `pixelforge-tilemap` JSON. The undo stack is empty.
    pub fn load(json: &str) -> Result<Self, KernelError> {
        let json = json.strip_prefix('\u{feff}').unwrap_or(json);
        let value: Value =
            serde_json::from_str(json).map_err(|_| tilemap_file("tilemap file is not JSON"))?;
        let map = value
            .as_object()
            .ok_or_else(|| tilemap_file("tilemap file must be a JSON object"))?;
        reject_keys(
            map,
            &[
                "format",
                "version",
                "width",
                "height",
                "tileWidth",
                "tileHeight",
                "tileCount",
                "tileset",
                "layers",
            ],
            |field| tilemap_file(format!("tilemap file has unknown field {field}")),
        )?;
        match map.get("format") {
            Some(Value::String(name)) if name == TILEMAP_FORMAT => {}
            _ => {
                return Err(tilemap_file(
                    "tilemap file format is not pixelforge-tilemap",
                ))
            }
        }
        match map.get("version").and_then(Value::as_u64) {
            Some(TILEMAP_VERSION) => {}
            _ => return Err(tilemap_file("tilemap file version is not 1")),
        }
        let width =
            json_u16(map.get("width")).ok_or_else(|| tilemap_file("tilemap width is missing"))?;
        let height =
            json_u16(map.get("height")).ok_or_else(|| tilemap_file("tilemap height is missing"))?;
        check_map_size(width, height)?;
        let tile_width = json_u16(map.get("tileWidth"))
            .ok_or_else(|| tilemap_file("tilemap tileWidth is missing"))?;
        let tile_height = json_u16(map.get("tileHeight"))
            .ok_or_else(|| tilemap_file("tilemap tileHeight is missing"))?;
        let decl = TilesetDecl::edges(tile_width, tile_height)?;
        let tile_count = json_u32(map.get("tileCount"))
            .ok_or_else(|| tilemap_file("tilemap tileCount is missing"))?;
        check_tile_count(tile_count)?;
        let tileset = map
            .get("tileset")
            .and_then(Value::as_str)
            .ok_or_else(|| tilemap_file("tilemap tileset is missing"))?;
        let tileset = normalize_tileset_path(tileset).map_err(|_| {
            tilemap_file("tilemap tileset must be a relative path without controls")
        })?;
        let layers = map
            .get("layers")
            .and_then(Value::as_array)
            .ok_or_else(|| tilemap_file("tilemap layers are missing"))?;
        if layers.is_empty() || layers.len() > MAX_TILE_LAYERS {
            return Err(tilemap_file("tilemap needs 1 to 32 layers"));
        }
        let expect = usize::from(width) * usize::from(height);
        let mut parsed = Vec::with_capacity(layers.len());
        for (index, layer) in layers.iter().enumerate() {
            let layer = layer
                .as_object()
                .ok_or_else(|| tilemap_file(format!("tile layer {index} must be an object")))?;
            reject_keys(layer, &["name", "cells"], |field| {
                tilemap_file(format!("tile layer {index} has unknown field {field}"))
            })?;
            let name = layer
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| tilemap_file(format!("tile layer {index} is missing a name")))?;
            let name = normalize_tile_layer(name).map_err(|_| {
                tilemap_file(format!(
                    "tile layer {index} name must be 1 to {MAX_LAYER_NAME_CHARS} characters"
                ))
            })?;
            if parsed
                .iter()
                .any(|existing: &TileLayer| existing.name == name)
            {
                return Err(KernelError::TileLayer {
                    detail: "tile layer name is already used".to_string(),
                });
            }
            let cells = layer
                .get("cells")
                .and_then(Value::as_array)
                .ok_or_else(|| tilemap_file(format!("tile layer {index} is missing cells")))?;
            if cells.len() != expect {
                return Err(tilemap_file(format!(
                    "tile layer {index} has {} cells, expected {expect}",
                    cells.len()
                )));
            }
            let mut stored = Vec::with_capacity(expect);
            for cell in cells {
                stored.push(parse_cell(cell, tile_count)?);
            }
            parsed.push(TileLayer {
                name,
                cells: stored,
            });
        }
        Ok(Self {
            width,
            height,
            tile_width: decl.tile_width,
            tile_height: decl.tile_height,
            tile_count,
            tileset,
            layers: parsed,
            undo: Vec::new(),
            redo: Vec::new(),
        })
    }

    pub fn get(&self, layer: usize, x: u16, y: u16) -> Result<Option<TileCell>, KernelError> {
        self.check_layer(layer)?;
        self.check_cell(x, y)?;
        Ok(self.layers[layer].cells[self.offset(x, y)])
    }

    /// Writes one cell. `None` clears it. The same value is not recorded.
    pub fn set(
        &mut self,
        layer: usize,
        x: u16,
        y: u16,
        cell: Option<TileCell>,
    ) -> Result<(), KernelError> {
        self.check_layer(layer)?;
        self.check_cell(x, y)?;
        if let Some(cell) = cell {
            self.check_tile(cell.tile)?;
        }
        let offset = self.offset(x, y);
        if self.layers[layer].cells[offset] == cell {
            return Ok(());
        }
        let mut layers = self.layers.clone();
        layers[layer].cells[offset] = cell;
        self.commit(layers);
        Ok(())
    }

    /// Fills a rectangle that lies entirely inside the map. One undo step.
    pub fn fill(
        &mut self,
        layer: usize,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        cell: Option<TileCell>,
    ) -> Result<u32, KernelError> {
        self.check_layer(layer)?;
        self.check_rect(x, y, width, height)?;
        if let Some(cell) = cell {
            self.check_tile(cell.tile)?;
        }
        let mut layers = self.layers.clone();
        let mut changed = 0u32;
        for row in y..y.saturating_add(height) {
            for column in x..x.saturating_add(width) {
                let offset = self.offset(column, row);
                if layers[layer].cells[offset] != cell {
                    layers[layer].cells[offset] = cell;
                    changed += 1;
                }
            }
        }
        if changed == 0 {
            return Ok(0);
        }
        self.commit(layers);
        Ok(changed)
    }

    pub fn add_layer(&mut self, name: &str) -> Result<usize, KernelError> {
        if self.layers.len() >= MAX_TILE_LAYERS {
            return Err(KernelError::TileLayer {
                detail: "tilemap already has 32 layers".to_string(),
            });
        }
        let name = normalize_tile_layer(name)?;
        if self.layers.iter().any(|layer| layer.name == name) {
            return Err(KernelError::TileLayer {
                detail: "tile layer name is already used".to_string(),
            });
        }
        let mut layers = self.layers.clone();
        layers.push(TileLayer {
            name,
            cells: vec![None; self.cell_count()],
        });
        let index = layers.len() - 1;
        self.commit(layers);
        Ok(index)
    }

    /// `index` or `name`. Neither means layer 0. Both is refused.
    pub fn resolve_layer(
        &self,
        index: Option<usize>,
        name: Option<&str>,
    ) -> Result<usize, KernelError> {
        match (index, name) {
            (Some(_), Some(_)) => Err(KernelError::TileLayer {
                detail: "pass layer or name, not both".to_string(),
            }),
            (Some(index), None) => {
                self.check_layer(index)?;
                Ok(index)
            }
            (None, Some(name)) => {
                let name = normalize_tile_layer(name)?;
                self.layers
                    .iter()
                    .position(|layer| layer.name == name)
                    .ok_or_else(|| KernelError::TileLayer {
                        detail: "tile layer is missing".to_string(),
                    })
            }
            (None, None) => {
                self.check_layer(0)?;
                Ok(0)
            }
        }
    }

    /// Replaces occupied cells from the 4-neighbor rules. Empty cells stay empty.
    ///
    /// Occupancy is the layer before this call, so the output does not depend
    /// on write order. A tile id outside the tileset writes nothing.
    pub fn autotile(&mut self, layer: usize, rules: &AutotileRules) -> Result<u32, KernelError> {
        self.check_layer(layer)?;
        rules.check_tiles(self.tile_count)?;
        let occupied: Vec<bool> = self.layers[layer]
            .cells
            .iter()
            .map(|cell| cell.is_some())
            .collect();
        let width = u32::from(self.width);
        let height = u32::from(self.height);
        let mut layers = self.layers.clone();
        let mut changed = 0u32;
        for y in 0..height {
            for x in 0..width {
                let offset = y as usize * width as usize + x as usize;
                if !occupied[offset] {
                    continue;
                }
                let mask = neighbor_mask(&occupied, width, height, x, y);
                let rule = rules.rule(mask);
                let next = TileCell {
                    tile: rule.tile,
                    flip_h: rule.flip_h,
                    flip_v: rule.flip_v,
                };
                if layers[layer].cells[offset] != Some(next) {
                    layers[layer].cells[offset] = Some(next);
                    changed += 1;
                }
            }
        }
        if changed == 0 {
            return Ok(0);
        }
        self.commit(layers);
        Ok(changed)
    }

    /// Ids, sizes, and layer lengths. Does not open the tileset project.
    pub fn validate(&self) -> Result<(), KernelError> {
        check_map_size(self.width, self.height)?;
        TilesetDecl::edges(self.tile_width, self.tile_height)?;
        check_tile_count(self.tile_count)?;
        let expect = self.cell_count();
        if self.layers.is_empty() {
            return Err(tilemap_file("tilemap needs 1 to 32 layers"));
        }
        for layer in &self.layers {
            if layer.cells.len() != expect {
                return Err(tilemap_file("tile layer cell count does not match the map"));
            }
            for cell in &layer.cells {
                if let Some(cell) = cell {
                    self.check_tile(cell.tile)?;
                }
            }
        }
        Ok(())
    }

    /// The map's tile size and `tileCount` match this tileset.
    pub fn validate_tileset(&self, report: &TilesetReport) -> Result<(), KernelError> {
        self.validate()?;
        if self.tile_width != report.tile_width || self.tile_height != report.tile_height {
            return Err(KernelError::TileAlign {
                tile_width: report.tile_width,
                tile_height: report.tile_height,
                width: self.tile_width,
                height: self.tile_height,
            });
        }
        if self.tile_count != report.tile_count {
            return Err(KernelError::TileCount {
                map: self.tile_count,
                tileset: report.tile_count,
            });
        }
        Ok(())
    }

    pub fn undo(&mut self) -> Result<(), KernelError> {
        let previous = self.undo.pop().ok_or(KernelError::NothingToUndo)?;
        let current = std::mem::replace(&mut self.layers, previous);
        self.redo.push(current);
        Ok(())
    }

    pub fn redo(&mut self) -> Result<(), KernelError> {
        let next = self.redo.pop().ok_or(KernelError::NothingToRedo)?;
        let current = std::mem::replace(&mut self.layers, next);
        self.undo.push(current);
        Ok(())
    }

    /// RGBA preview. `tile_rgba[id]` is that frame's unscaled composite.
    ///
    /// Index 0 in the source is transparent and shows the layer underneath.
    /// An opaque pixel covers it. `scale` repeats the composited cell.
    pub fn render_png(&self, tile_rgba: &[Vec<u8>], scale: u32) -> Result<Vec<u8>, KernelError> {
        if scale == 0 {
            return Err(KernelError::ExportScale { scale });
        }
        if tile_rgba.len() != usize::try_from(self.tile_count).unwrap_or(usize::MAX) {
            return Err(KernelError::TileCount {
                map: self.tile_count,
                tileset: u32::try_from(tile_rgba.len()).unwrap_or(u32::MAX),
            });
        }
        let tile_w = u32::from(self.tile_width);
        let tile_h = u32::from(self.tile_height);
        let expect = (tile_w as usize)
            .checked_mul(tile_h as usize)
            .and_then(|cells| cells.checked_mul(4))
            .ok_or(KernelError::ExportScale { scale })?;
        for buffer in tile_rgba {
            if buffer.len() != expect {
                return Err(KernelError::TileAlign {
                    tile_width: self.tile_width,
                    tile_height: self.tile_height,
                    width: self.tile_width,
                    height: self.tile_height,
                });
            }
        }
        let raw_w = u32::from(self.width)
            .checked_mul(tile_w)
            .ok_or(KernelError::ExportScale { scale })?;
        let raw_h = u32::from(self.height)
            .checked_mul(tile_h)
            .ok_or(KernelError::ExportScale { scale })?;
        let mut raw = vec![0u8; raw_w as usize * raw_h as usize * 4];
        for layer in &self.layers {
            for y in 0..u32::from(self.height) {
                for x in 0..u32::from(self.width) {
                    let Some(cell) = layer.cells[y as usize * usize::from(self.width) + x as usize]
                    else {
                        continue;
                    };
                    let src = &tile_rgba[cell.tile as usize];
                    blit_tile(
                        &mut raw,
                        raw_w,
                        x * tile_w,
                        y * tile_h,
                        tile_w,
                        tile_h,
                        src,
                        cell.flip_h,
                        cell.flip_v,
                    );
                }
            }
        }
        let (width, height, rgba) = scale_rgba(&raw, raw_w, raw_h, scale)?;
        Ok(encode_rgba_png(width, height, &rgba))
    }

    /// Preview PNG and a Tiled `.tmj` value. `columns` defaults to one row.
    ///
    /// The image name, column count, and scale must match `export_tileset`
    /// for the same project, or Tiled will slice a different grid.
    pub fn export_with_document(
        &self,
        document: &Document,
        image: &str,
        columns: Option<u32>,
        scale: u32,
    ) -> Result<TilemapExport, KernelError> {
        let report = document.validate_tileset()?;
        self.validate_tileset(&report)?;
        let columns = columns.unwrap_or(report.tile_count.max(1));
        let (image_width, image_height) = document.scaled_sheet_size_with(scale, Some(columns))?;
        let layout = document.sprite_sheet_layout_with(Some(columns))?;
        let tile_width = u32::from(report.tile_width)
            .checked_mul(scale)
            .ok_or(KernelError::ExportScale { scale })?;
        let tile_height = u32::from(report.tile_height)
            .checked_mul(scale)
            .ok_or(KernelError::ExportScale { scale })?;
        let spec = TilesetImage {
            image: check_image_name(image)?,
            name: image_stem(image),
            columns: layout.columns,
            rows: layout.rows,
            scale,
            image_width,
            image_height,
            tile_width,
            tile_height,
            source_tile_width: report.tile_width,
            source_tile_height: report.tile_height,
            tile_count: report.tile_count,
        };
        check_sheet(&spec)?;
        let mut frames = Vec::with_capacity(document.frame_count());
        for index in 0..document.frame_count() {
            frames.push(document.frame_rgba(index)?);
        }
        let png = self.render_png(&frames, scale)?;
        let (width, height) = scaled_map_size(self, scale)?;
        let tmj = tmj_value(self, &spec)?;
        Ok(TilemapExport {
            png,
            width,
            height,
            tmj,
        })
    }

    /// Runs one tilemap Tool API request. The envelope matches `Document::call_tool`.
    pub fn call_tool(&mut self, request: &str) -> String {
        let request = request.strip_prefix('\u{feff}').unwrap_or(request);
        let (id, outcome) = match parse_tile_request(request) {
            Ok(call) => {
                let id = call.id.clone();
                (id, dispatch_tile(self, &call))
            }
            Err(error) => (error.id, Err(error.failure)),
        };
        match outcome {
            Ok(result) => encode_ok(id, result),
            Err(failure) => encode_err(id, failure),
        }
    }

    fn commit(&mut self, layers: Vec<TileLayer>) {
        if layers == self.layers {
            return;
        }
        self.undo.push(std::mem::replace(&mut self.layers, layers));
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn cell_count(&self) -> usize {
        usize::from(self.width) * usize::from(self.height)
    }

    fn offset(&self, x: u16, y: u16) -> usize {
        usize::from(y) * usize::from(self.width) + usize::from(x)
    }

    fn check_layer(&self, layer: usize) -> Result<(), KernelError> {
        if layer >= self.layers.len() {
            Err(KernelError::TileLayer {
                detail: format!("tile layer index {layer} is outside {}", self.layers.len()),
            })
        } else {
            Ok(())
        }
    }

    fn check_cell(&self, x: u16, y: u16) -> Result<(), KernelError> {
        if x >= self.width || y >= self.height {
            Err(KernelError::TileBounds {
                x: u32::from(x),
                y: u32::from(y),
                width: u32::from(self.width),
                height: u32::from(self.height),
            })
        } else {
            Ok(())
        }
    }

    fn check_rect(&self, x: u16, y: u16, width: u16, height: u16) -> Result<(), KernelError> {
        let right = u32::from(x).checked_add(u32::from(width));
        let bottom = u32::from(y).checked_add(u32::from(height));
        if width == 0
            || height == 0
            || right.is_none_or(|right| right > u32::from(self.width))
            || bottom.is_none_or(|bottom| bottom > u32::from(self.height))
        {
            return Err(KernelError::TileBounds {
                x: u32::from(x),
                y: u32::from(y),
                width: u32::from(self.width),
                height: u32::from(self.height),
            });
        }
        Ok(())
    }

    fn check_tile(&self, tile: u32) -> Result<(), KernelError> {
        if tile >= self.tile_count {
            Err(KernelError::TileId {
                tile,
                tile_count: self.tile_count,
            })
        } else {
            Ok(())
        }
    }

    fn filled(&self, layer: usize) -> u32 {
        self.layers[layer]
            .cells
            .iter()
            .filter(|cell| cell.is_some())
            .count() as u32
    }
}

fn check_map_size(width: u16, height: u16) -> Result<(), KernelError> {
    if width == 0 || height == 0 || width > MAX_TILEMAP_EDGE || height > MAX_TILEMAP_EDGE {
        Err(KernelError::MapSize {
            width: u32::from(width),
            height: u32::from(height),
        })
    } else {
        Ok(())
    }
}

fn check_tile_count(tile_count: u32) -> Result<(), KernelError> {
    if tile_count == 0 || tile_count > MAX_TILE_COUNT {
        Err(KernelError::TileCount {
            map: tile_count,
            tileset: 0,
        })
    } else {
        Ok(())
    }
}

fn normalize_tile_layer(name: &str) -> Result<String, KernelError> {
    let trimmed = name.trim();
    let chars = trimmed.chars().count();
    if trimmed.is_empty() || chars > MAX_LAYER_NAME_CHARS || trimmed.chars().any(char::is_control) {
        Err(KernelError::TileLayer {
            detail: format!("tile layer name must be 1 to {MAX_LAYER_NAME_CHARS} characters"),
        })
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_tileset_path(path: &str) -> Result<String, KernelError> {
    let trimmed = path.trim();
    if trimmed.is_empty()
        || trimmed.chars().count() > MAX_TILESET_PATH
        || trimmed.chars().any(char::is_control)
        || trimmed.starts_with('/')
        || trimmed.contains('\\')
        || trimmed
            .split('/')
            .any(|part| part.is_empty() || part == "..")
    {
        Err(tilemap_file(
            "tilemap tileset must be a relative path without controls",
        ))
    } else {
        Ok(trimmed.to_string())
    }
}

fn tilemap_file(detail: impl Into<String>) -> KernelError {
    KernelError::TileMapFile {
        detail: detail.into(),
    }
}

fn cell_json(cell: Option<TileCell>) -> Value {
    match cell {
        None => Value::Null,
        Some(cell) if !cell.flip_h && !cell.flip_v => Value::Number(cell.tile.into()),
        Some(cell) => serde_json::json!({
            "tile": cell.tile,
            "flipH": cell.flip_h,
            "flipV": cell.flip_v,
        }),
    }
}

fn parse_cell(value: &Value, tile_count: u32) -> Result<Option<TileCell>, KernelError> {
    match value {
        Value::Null => Ok(None),
        Value::Number(number) => {
            let tile = number
                .as_u64()
                .and_then(|tile| u32::try_from(tile).ok())
                .ok_or_else(|| tilemap_file("tile id must be an integer"))?;
            if tile >= tile_count {
                return Err(KernelError::TileId { tile, tile_count });
            }
            Ok(Some(TileCell::plain(tile)))
        }
        Value::Object(map) => {
            reject_keys(map, &["tile", "flipH", "flipV"], |field| {
                tilemap_file(format!("tile cell has unknown field {field}"))
            })?;
            let tile = map
                .get("tile")
                .and_then(Value::as_u64)
                .and_then(|tile| u32::try_from(tile).ok())
                .ok_or_else(|| tilemap_file("tile cell is missing tile"))?;
            if tile >= tile_count {
                return Err(KernelError::TileId { tile, tile_count });
            }
            let flip_h = match map.get("flipH") {
                None => false,
                Some(Value::Bool(value)) => *value,
                Some(_) => return Err(tilemap_file("tile flipH must be a boolean")),
            };
            let flip_v = match map.get("flipV") {
                None => false,
                Some(Value::Bool(value)) => *value,
                Some(_) => return Err(tilemap_file("tile flipV must be a boolean")),
            };
            Ok(Some(TileCell {
                tile,
                flip_h,
                flip_v,
            }))
        }
        _ => Err(tilemap_file(
            "tile cell must be null, an integer, or an object",
        )),
    }
}

fn json_u16(value: Option<&Value>) -> Option<u16> {
    value
        .and_then(Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
}

fn json_u32(value: Option<&Value>) -> Option<u32> {
    value
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}

struct TilesetImage {
    image: String,
    name: String,
    columns: u32,
    rows: u32,
    scale: u32,
    image_width: u32,
    image_height: u32,
    tile_width: u32,
    tile_height: u32,
    source_tile_width: u16,
    source_tile_height: u16,
    tile_count: u32,
}

/// `pixelforge-tileset` object for a sheet that was just exported.
pub fn tileset_meta(export: &TilesetExport) -> Value {
    let mut tiles = Vec::with_capacity(export.tile_count as usize);
    for id in 0..export.tile_count {
        let column = id % export.columns;
        let row = id / export.columns;
        tiles.push(serde_json::json!({
            "id": id,
            "x": column * export.tile_width,
            "y": row * export.tile_height,
            "width": export.tile_width,
            "height": export.tile_height,
        }));
    }
    serde_json::json!({
        "format": TILESET_FORMAT,
        "version": TILESET_VERSION,
        "image": export.image,
        "tileWidth": export.tile_width,
        "tileHeight": export.tile_height,
        "sourceTileWidth": export.source_tile_width,
        "sourceTileHeight": export.source_tile_height,
        "scale": export.scale,
        "columns": export.columns,
        "rows": export.rows,
        "tileCount": export.tile_count,
        "imageWidth": export.image_width,
        "imageHeight": export.image_height,
        "margin": 0,
        "spacing": 0,
        "tiles": tiles,
    })
}

pub fn check_image_name(name: &str) -> Result<String, KernelError> {
    if name.is_empty()
        || name.len() > 128
        || name.chars().any(char::is_control)
        || name.starts_with('/')
        || name.contains('\\')
        || name.contains(':')
    {
        return Err(KernelError::TileImage);
    }
    let parts: Vec<&str> = name.split('/').collect();
    if parts
        .iter()
        .any(|part| part.is_empty() || *part == "." || *part == "..")
    {
        return Err(KernelError::TileImage);
    }
    let file = parts.last().copied().unwrap_or("");
    if file == ".png" || !file.ends_with(".png") {
        return Err(KernelError::TileImage);
    }
    Ok(name.to_string())
}

fn image_stem(image: &str) -> String {
    let file = image.rsplit('/').next().unwrap_or(image);
    file.trim_end_matches(".png").to_string()
}

fn check_sheet(spec: &TilesetImage) -> Result<(), KernelError> {
    if spec.columns == 0 {
        return Err(KernelError::SheetColumns { columns: 0 });
    }
    let source_w = u32::from(spec.source_tile_width)
        .checked_mul(spec.scale)
        .ok_or(KernelError::ExportScale { scale: spec.scale })?;
    let source_h = u32::from(spec.source_tile_height)
        .checked_mul(spec.scale)
        .ok_or(KernelError::ExportScale { scale: spec.scale })?;
    if source_w != spec.tile_width || source_h != spec.tile_height {
        return Err(tilemap_file(format!(
            "scaled tile {source_w}x{source_h} does not match image cell {}x{}",
            spec.tile_width, spec.tile_height
        )));
    }
    let rows = spec.tile_count.div_ceil(spec.columns);
    let width = spec
        .columns
        .checked_mul(spec.tile_width)
        .ok_or(KernelError::ExportScale { scale: spec.scale })?;
    let height = rows
        .checked_mul(spec.tile_height)
        .ok_or(KernelError::ExportScale { scale: spec.scale })?;
    if spec.image_width != width || spec.image_height != height || spec.rows != rows {
        return Err(tilemap_file(format!(
            "tileset image is {}x{}, expected {width}x{height}",
            spec.image_width, spec.image_height
        )));
    }
    Ok(())
}

fn tmj_value(map: &TileMap, spec: &TilesetImage) -> Result<Value, KernelError> {
    check_sheet(spec)?;
    check_image_name(&spec.image)?;
    let mut layers = Vec::with_capacity(map.layers.len());
    for (index, layer) in map.layers.iter().enumerate() {
        let mut data = Vec::with_capacity(layer.cells.len());
        for cell in &layer.cells {
            let gid = cell.map(TileCell::tiled_gid).unwrap_or(0);
            data.push(Value::Number(Number::from(u64::from(gid))));
        }
        layers.push(serde_json::json!({
            "data": data,
            "height": map.height,
            "id": index + 1,
            "name": layer.name,
            "opacity": 1,
            "type": "tilelayer",
            "visible": true,
            "width": map.width,
            "x": 0,
            "y": 0,
        }));
    }
    Ok(serde_json::json!({
        "compressionlevel": -1,
        "height": map.height,
        "infinite": false,
        "layers": layers,
        "nextlayerid": map.layers.len() + 1,
        "nextobjectid": 1,
        "orientation": "orthogonal",
        "renderorder": "right-down",
        "tiledversion": "1.10.2",
        "tileheight": spec.tile_height,
        "tilesets": [{
            "columns": spec.columns,
            "firstgid": 1,
            "image": spec.image,
            "imageheight": spec.image_height,
            "imagewidth": spec.image_width,
            "margin": 0,
            "name": spec.name,
            "spacing": 0,
            "tilecount": spec.tile_count,
            "tileheight": spec.tile_height,
            "tilewidth": spec.tile_width,
        }],
        "tilewidth": spec.tile_width,
        "type": "map",
        "version": "1.10",
        "width": map.width,
    }))
}

fn scaled_map_size(map: &TileMap, scale: u32) -> Result<(u32, u32), KernelError> {
    let tile_w = u32::from(map.tile_width)
        .checked_mul(scale)
        .ok_or(KernelError::ExportScale { scale })?;
    let tile_h = u32::from(map.tile_height)
        .checked_mul(scale)
        .ok_or(KernelError::ExportScale { scale })?;
    let width = u32::from(map.width)
        .checked_mul(tile_w)
        .ok_or(KernelError::ExportScale { scale })?;
    let height = u32::from(map.height)
        .checked_mul(tile_h)
        .ok_or(KernelError::ExportScale { scale })?;
    Ok((width, height))
}

fn blit_tile(
    dst: &mut [u8],
    dst_width: u32,
    origin_x: u32,
    origin_y: u32,
    tile_w: u32,
    tile_h: u32,
    src: &[u8],
    flip_h: bool,
    flip_v: bool,
) {
    let stride = dst_width as usize;
    for y in 0..tile_h {
        let src_y = if flip_v { tile_h - 1 - y } else { y };
        for x in 0..tile_w {
            let src_x = if flip_h { tile_w - 1 - x } else { x };
            let src_i = (src_y as usize * tile_w as usize + src_x as usize) * 4;
            let dst_i =
                ((origin_y as usize + y as usize) * stride + origin_x as usize + x as usize) * 4;
            over(&mut dst[dst_i..dst_i + 4], &src[src_i..src_i + 4]);
        }
    }
}

fn over(dst: &mut [u8], src: &[u8]) {
    let sa = src[3];
    if sa == 0 {
        return;
    }
    if sa == 255 {
        dst.copy_from_slice(src);
        return;
    }
    let da = dst[3] as u32;
    let sa_u = sa as u32;
    let inv = 255 - sa_u;
    let out_a = sa_u + da * inv / 255;
    if out_a == 0 {
        dst.fill(0);
        return;
    }
    for channel in 0..3 {
        let mixed = src[channel] as u32 * sa_u + dst[channel] as u32 * da * inv / 255;
        dst[channel] = (mixed / out_a) as u8;
    }
    dst[3] = out_a as u8;
}

fn scale_rgba(
    src: &[u8],
    width: u32,
    height: u32,
    scale: u32,
) -> Result<(u32, u32, Vec<u8>), KernelError> {
    if scale == 1 {
        return Ok((width, height, src.to_vec()));
    }
    let out_w = width
        .checked_mul(scale)
        .ok_or(KernelError::ExportScale { scale })?;
    let out_h = height
        .checked_mul(scale)
        .ok_or(KernelError::ExportScale { scale })?;
    let mut dst = vec![0u8; out_w as usize * out_h as usize * 4];
    let scale_us = scale as usize;
    for y in 0..height as usize {
        for x in 0..width as usize {
            let pixel = &src[(y * width as usize + x) * 4..(y * width as usize + x) * 4 + 4];
            for dy in 0..scale_us {
                for dx in 0..scale_us {
                    let ox = x * scale_us + dx;
                    let oy = y * scale_us + dy;
                    let at = (oy * out_w as usize + ox) * 4;
                    dst[at..at + 4].copy_from_slice(pixel);
                }
            }
        }
    }
    Ok((out_w, out_h, dst))
}

fn encode_rgba_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    use png::{BitDepth, ColorType, Encoder};
    let mut bytes = Vec::new();
    {
        let mut encoder = Encoder::new(&mut bytes, width, height);
        encoder.set_color(ColorType::Rgba);
        encoder.set_depth(BitDepth::Eight);
        encoder
            .write_header()
            .expect("png header fits in memory")
            .write_image_data(rgba)
            .expect("png image data fits in memory");
    }
    bytes
}

fn reject_keys<E>(
    map: &Map<String, Value>,
    allowed: &[&str],
    fail: impl Fn(String) -> E,
) -> Result<(), E> {
    for key in map.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(fail(key.clone()));
        }
    }
    Ok(())
}

struct TileCall {
    id: Option<Value>,
    tool: String,
    params: Value,
}

struct TileRequestError {
    id: Option<Value>,
    failure: TileFail,
}

enum TileFail {
    InvalidRequest(String),
    UnsupportedVersion { version: u32 },
    UnknownTool { name: String },
    InvalidParams { message: String },
    Kernel(KernelError),
}

impl TileFail {
    fn code(&self) -> &'static str {
        match self {
            Self::InvalidRequest(_) => "InvalidRequest",
            Self::UnsupportedVersion { .. } => "UnsupportedVersion",
            Self::UnknownTool { .. } => "UnknownTool",
            Self::InvalidParams { .. } => "InvalidParams",
            Self::Kernel(error) => error.code(),
        }
    }

    fn message(&self) -> String {
        match self {
            Self::InvalidRequest(message) | Self::InvalidParams { message } => message.clone(),
            Self::UnsupportedVersion { version } => {
                format!("tool api version {version} is not supported")
            }
            Self::UnknownTool { name } => format!("unknown tool \"{name}\""),
            Self::Kernel(error) => error.to_string(),
        }
    }
}

fn encode_ok(id: Option<Value>, result: Value) -> String {
    let mut body = Map::new();
    body.insert("ok".to_string(), Value::Bool(true));
    body.insert(
        "apiVersion".to_string(),
        Value::Number(TOOL_API_VERSION.into()),
    );
    if let Some(id) = id {
        body.insert("id".to_string(), id);
    }
    body.insert("result".to_string(), result);
    serde_json::to_string(&Value::Object(body)).expect("tool response is JSON")
}

fn encode_err(id: Option<Value>, failure: TileFail) -> String {
    let mut body = Map::new();
    body.insert("ok".to_string(), Value::Bool(false));
    body.insert(
        "apiVersion".to_string(),
        Value::Number(TOOL_API_VERSION.into()),
    );
    if let Some(id) = id {
        body.insert("id".to_string(), id);
    }
    body.insert(
        "error".to_string(),
        serde_json::json!({
            "code": failure.code(),
            "message": failure.message(),
        }),
    );
    serde_json::to_string(&Value::Object(body)).expect("tool response is JSON")
}

fn parse_tile_request(request: &str) -> Result<TileCall, TileRequestError> {
    let value: Value = match serde_json::from_str(request) {
        Ok(value) => value,
        Err(_) => {
            return Err(TileRequestError {
                id: None,
                failure: TileFail::InvalidRequest("request is not JSON".to_string()),
            });
        }
    };
    let Some(map) = value.as_object() else {
        return Err(TileRequestError {
            id: None,
            failure: TileFail::InvalidRequest("request must be a JSON object".to_string()),
        });
    };
    let id = match map.get("id") {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => Some(Value::String(text.clone())),
        Some(Value::Number(number)) => Some(Value::Number(number.clone())),
        Some(_) => {
            return Err(TileRequestError {
                id: None,
                failure: TileFail::InvalidRequest("id must be a string or number".to_string()),
            });
        }
    };
    let fail = |failure| TileRequestError {
        id: id.clone(),
        failure,
    };
    if let Err(field) = reject_keys(map, &["apiVersion", "id", "tool", "params"], |field| field) {
        return Err(fail(TileFail::InvalidRequest(format!(
            "unknown field {field}"
        ))));
    }
    if let Some(version) = map.get("apiVersion") {
        match version.as_u64().and_then(|value| u32::try_from(value).ok()) {
            Some(TOOL_API_VERSION) => {}
            Some(version) => {
                return Err(fail(TileFail::UnsupportedVersion { version }));
            }
            None if version.is_null() => {}
            None => {
                return Err(fail(TileFail::InvalidRequest(
                    "apiVersion must be an integer from 0 to 4294967295".to_string(),
                )));
            }
        }
    }
    let tool = match map.get("tool") {
        Some(Value::String(name)) => name.clone(),
        None => return Err(fail(TileFail::InvalidRequest("missing tool".to_string()))),
        Some(_) => {
            return Err(fail(TileFail::InvalidRequest(
                "tool must be a string".to_string(),
            )))
        }
    };
    let params = match map.get("params") {
        None | Some(Value::Null) => Value::Object(Map::new()),
        Some(Value::Object(params)) => Value::Object(params.clone()),
        Some(_) => {
            return Err(fail(TileFail::InvalidParams {
                message: format!("{tool}: params must be an object"),
            }))
        }
    };
    Ok(TileCall { id, tool, params })
}

fn dispatch_tile(map: &mut TileMap, call: &TileCall) -> Result<Value, TileFail> {
    let tool = call.tool.as_str();
    let params = match &call.params {
        Value::Object(params) => params,
        _ => {
            return Err(TileFail::InvalidParams {
                message: format!("{tool}: params must be an object"),
            })
        }
    };
    match tool {
        "inspectTilemap" => {
            reject_params(tool, params, &[])?;
            Ok(inspect_tilemap(map))
        }
        "getTile" => {
            reject_params(tool, params, &["x", "y", "layer", "name"])?;
            let (x, y) = require_cell(tool, params)?;
            let layer = resolve_param(map, tool, params)?;
            let cell = map.get(layer, x, y).map_err(TileFail::Kernel)?;
            Ok(cell_result(x, y, layer, cell))
        }
        "setTile" => {
            reject_params(
                tool,
                params,
                &["x", "y", "layer", "name", "tile", "flipH", "flipV"],
            )?;
            let (x, y) = require_cell(tool, params)?;
            let layer = resolve_param(map, tool, params)?;
            let cell = require_tile_cell(tool, params, true)?;
            map.set(layer, x, y, cell).map_err(TileFail::Kernel)?;
            Ok(cell_result(x, y, layer, cell))
        }
        "fillTiles" => {
            reject_params(
                tool,
                params,
                &[
                    "x", "y", "width", "height", "layer", "name", "tile", "flipH", "flipV",
                ],
            )?;
            let (x, y, width, height) = require_rect(tool, params)?;
            let layer = resolve_param(map, tool, params)?;
            let cell = require_tile_cell(tool, params, false)?;
            let changed = map
                .fill(layer, x, y, width, height, cell)
                .map_err(TileFail::Kernel)?;
            let mut result = cell_result(x, y, layer, cell);
            result["width"] = Value::Number(width.into());
            result["height"] = Value::Number(height.into());
            result["changed"] = Value::Number(changed.into());
            Ok(result)
        }
        "clearTiles" => {
            reject_params(
                tool,
                params,
                &["x", "y", "width", "height", "layer", "name"],
            )?;
            let (x, y, width, height) = require_rect(tool, params)?;
            let layer = resolve_param(map, tool, params)?;
            let changed = map
                .fill(layer, x, y, width, height, None)
                .map_err(TileFail::Kernel)?;
            Ok(serde_json::json!({
                "x": x,
                "y": y,
                "width": width,
                "height": height,
                "layer": layer,
                "changed": changed,
            }))
        }
        "addTileLayer" => {
            reject_params(tool, params, &["name"])?;
            let name = require_string(tool, params, "name")?;
            let index = map.add_layer(&name).map_err(TileFail::Kernel)?;
            Ok(serde_json::json!({
                "index": index,
                "name": map.layer_name(index).unwrap_or(""),
                "layerCount": map.layer_count(),
            }))
        }
        "autotile" => {
            reject_params(tool, params, &["layer", "name", "rules"])?;
            let layer = resolve_param(map, tool, params)?;
            let rules = parse_rules_param(tool, params)?;
            let changed = map.autotile(layer, &rules).map_err(TileFail::Kernel)?;
            Ok(serde_json::json!({
                "layer": layer,
                "changed": changed,
                "masks": 16,
            }))
        }
        "validateTilemap" => {
            reject_params(tool, params, &["rules"])?;
            map.validate().map_err(TileFail::Kernel)?;
            if params.contains_key("rules") {
                let rules = parse_rules_param(tool, params)?;
                rules
                    .check_tiles(map.tile_count())
                    .map_err(TileFail::Kernel)?;
            }
            Ok(inspect_tilemap(map))
        }
        "exportTilemap" => {
            reject_params(
                tool,
                params,
                &["tilesetProject", "image", "columns", "scale"],
            )?;
            let project = require_string(tool, params, "tilesetProject")?;
            let image = match params.get("image") {
                None => "tiles.png".to_string(),
                Some(_) => require_string(tool, params, "image")?,
            };
            let columns = optional_u32(tool, params, "columns")?;
            let scale = match params.get("scale") {
                None => 1,
                Some(_) => require_u32_at_least(tool, params, "scale", 1)?,
            };
            let document = Document::load_project(&project).map_err(TileFail::Kernel)?;
            let export = map
                .export_with_document(&document, &image, columns, scale)
                .map_err(TileFail::Kernel)?;
            Ok(serde_json::json!({
                "png": encode_base64(&export.png),
                "width": export.width,
                "height": export.height,
                "tmj": export.tmj,
            }))
        }
        "undo" => {
            reject_params(tool, params, &[])?;
            map.undo().map_err(TileFail::Kernel)?;
            Ok(serde_json::json!({
                "canUndo": map.can_undo(),
                "canRedo": map.can_redo(),
            }))
        }
        "redo" => {
            reject_params(tool, params, &[])?;
            map.redo().map_err(TileFail::Kernel)?;
            Ok(serde_json::json!({
                "canUndo": map.can_undo(),
                "canRedo": map.can_redo(),
            }))
        }
        _ => Err(TileFail::UnknownTool {
            name: tool.to_string(),
        }),
    }
}

fn inspect_tilemap(map: &TileMap) -> Value {
    let layers = (0..map.layer_count())
        .map(|index| {
            serde_json::json!({
                "index": index,
                "name": map.layer_name(index).unwrap_or(""),
                "filled": map.filled(index),
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "width": map.width(),
        "height": map.height(),
        "tileWidth": map.tile_width(),
        "tileHeight": map.tile_height(),
        "tileCount": map.tile_count(),
        "tileset": map.tileset(),
        "layerCount": map.layer_count(),
        "layers": layers,
        "canUndo": map.can_undo(),
        "canRedo": map.can_redo(),
    })
}

fn cell_result(x: u16, y: u16, layer: usize, cell: Option<TileCell>) -> Value {
    match cell {
        None => serde_json::json!({
            "x": x,
            "y": y,
            "layer": layer,
            "tile": Value::Null,
            "flipH": false,
            "flipV": false,
        }),
        Some(cell) => serde_json::json!({
            "x": x,
            "y": y,
            "layer": layer,
            "tile": cell.tile,
            "flipH": cell.flip_h,
            "flipV": cell.flip_v,
        }),
    }
}

fn parse_rules_param(tool: &str, params: &Map<String, Value>) -> Result<AutotileRules, TileFail> {
    let value = params.get("rules").ok_or_else(|| TileFail::InvalidParams {
        message: format!("{tool}: missing rules"),
    })?;
    let text = match value {
        Value::String(text) => text.clone(),
        Value::Object(_) => value.to_string(),
        _ => {
            return Err(TileFail::InvalidParams {
                message: format!("{tool}: rules must be an object or a JSON string"),
            })
        }
    };
    parse_autotile(&text).map_err(TileFail::Kernel)
}

fn resolve_param(
    map: &TileMap,
    tool: &str,
    params: &Map<String, Value>,
) -> Result<usize, TileFail> {
    let index = match params.get("layer") {
        None => None,
        Some(_) => Some(require_usize(tool, params, "layer")?),
    };
    let name = match params.get("name") {
        None => None,
        Some(_) => Some(require_string(tool, params, "name")?),
    };
    map.resolve_layer(index, name.as_deref())
        .map_err(TileFail::Kernel)
}

fn require_tile_cell(
    tool: &str,
    params: &Map<String, Value>,
    allow_null: bool,
) -> Result<Option<TileCell>, TileFail> {
    match params.get("tile") {
        None => Err(TileFail::InvalidParams {
            message: format!("{tool}: missing tile"),
        }),
        Some(Value::Null) if allow_null => Ok(None),
        Some(Value::Null) => Err(TileFail::InvalidParams {
            message: format!("{tool}: tile must be an integer"),
        }),
        Some(value) => {
            let tile = value
                .as_u64()
                .and_then(|tile| u32::try_from(tile).ok())
                .ok_or_else(|| TileFail::InvalidParams {
                    message: format!("{tool}: tile must be an integer from 0 to 4294967295"),
                })?;
            let flip_h = optional_flag(tool, params, "flipH")?;
            let flip_v = optional_flag(tool, params, "flipV")?;
            Ok(Some(TileCell {
                tile,
                flip_h,
                flip_v,
            }))
        }
    }
}

fn optional_flag(tool: &str, params: &Map<String, Value>, key: &str) -> Result<bool, TileFail> {
    match params.get(key) {
        None => Ok(false),
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(TileFail::InvalidParams {
            message: format!("{tool}: {key} must be a boolean"),
        }),
    }
}

fn require_cell(tool: &str, params: &Map<String, Value>) -> Result<(u16, u16), TileFail> {
    Ok((
        require_u16(tool, params, "x")?,
        require_u16(tool, params, "y")?,
    ))
}

fn require_rect(tool: &str, params: &Map<String, Value>) -> Result<(u16, u16, u16, u16), TileFail> {
    Ok((
        require_u16(tool, params, "x")?,
        require_u16(tool, params, "y")?,
        require_u16(tool, params, "width")?,
        require_u16(tool, params, "height")?,
    ))
}

fn require_string(tool: &str, params: &Map<String, Value>, key: &str) -> Result<String, TileFail> {
    match params.get(key) {
        Some(Value::String(text)) => Ok(text.clone()),
        _ => Err(TileFail::InvalidParams {
            message: format!("{tool}: missing {key}"),
        }),
    }
}

fn require_u16(tool: &str, params: &Map<String, Value>, key: &str) -> Result<u16, TileFail> {
    match params.get(key).and_then(Value::as_u64) {
        Some(value) => u16::try_from(value).map_err(|_| TileFail::InvalidParams {
            message: format!("{tool}: {key} must be an integer from 0 to 65535"),
        }),
        None => Err(TileFail::InvalidParams {
            message: format!("{tool}: missing {key}"),
        }),
    }
}

fn require_usize(tool: &str, params: &Map<String, Value>, key: &str) -> Result<usize, TileFail> {
    match params.get(key).and_then(Value::as_u64) {
        Some(value) => usize::try_from(value).map_err(|_| TileFail::InvalidParams {
            message: format!("{tool}: {key} must be an integer from 0 to 4294967295"),
        }),
        None => Err(TileFail::InvalidParams {
            message: format!("{tool}: missing {key}"),
        }),
    }
}

fn optional_u32(
    tool: &str,
    params: &Map<String, Value>,
    key: &str,
) -> Result<Option<u32>, TileFail> {
    match params.get(key) {
        None => Ok(None),
        Some(_) => Ok(Some(require_u32_at_least(tool, params, key, 1)?)),
    }
}

fn require_u32_at_least(
    tool: &str,
    params: &Map<String, Value>,
    key: &str,
    min: u32,
) -> Result<u32, TileFail> {
    match params
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
    {
        Some(value) if value >= min => Ok(value),
        Some(_) => Err(TileFail::InvalidParams {
            message: format!("{tool}: {key} must be an integer from {min} to 4294967295"),
        }),
        None => Err(TileFail::InvalidParams {
            message: format!("{tool}: missing {key}"),
        }),
    }
}

fn reject_params(
    tool: &str,
    params: &Map<String, Value>,
    allowed: &[&str],
) -> Result<(), TileFail> {
    reject_keys(params, allowed, |field| TileFail::InvalidParams {
        message: format!("{tool}: unknown field {field}"),
    })
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut index = 0;
    while index + 3 <= bytes.len() {
        let n = u32::from(bytes[index]) << 16
            | u32::from(bytes[index + 1]) << 8
            | u32::from(bytes[index + 2]);
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(TABLE[((n >> 6) & 63) as usize] as char);
        out.push(TABLE[(n & 63) as usize] as char);
        index += 3;
    }
    let rest = bytes.len() - index;
    if rest == 1 {
        let n = u32::from(bytes[index]) << 16;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rest == 2 {
        let n = u32::from(bytes[index]) << 16 | u32::from(bytes[index + 1]) << 8;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(TABLE[((n >> 6) & 63) as usize] as char);
        out.push('=');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_bits_follow_north_east_south_west() {
        let width = 3u32;
        let height = 3u32;
        let mut occupied = vec![false; 9];
        occupied[1] = true;
        occupied[3] = true;
        occupied[4] = true;
        occupied[5] = true;
        occupied[7] = true;
        assert_eq!(neighbor_mask(&occupied, width, height, 1, 1), 15);
        assert_eq!(
            neighbor_mask(&occupied, width, height, 1, 0),
            AUTOTILE_SOUTH
        );
        assert_eq!(
            neighbor_mask(&occupied, width, height, 0, 0),
            AUTOTILE_EAST | AUTOTILE_SOUTH
        );
        assert_eq!(
            TileCell {
                tile: 0,
                flip_h: true,
                flip_v: true,
            }
            .tiled_gid(),
            1 | FLIPPED_HORIZONTALLY_FLAG | FLIPPED_VERTICALLY_FLAG
        );
    }
}
