//! Arbitrary-size PNG → one indexed frame.
//!
//! Callers pass one [`ConvertRequest`] and read one result. The engine id
//! selects the sampler. Unknown ids error. `pixelforge-native` is reserved:
//! choosing it is [`KernelError::EngineUnavailable`] and does not fall back.
//!
//! A sampler writes RGBA at the target size. It does not invent a stored
//! palette color: every opaque sample is a source pixel, or a source color a
//! cluster snapped back onto. Alpha below [`INDEX_ALPHA_CUTOFF`] is
//! transparent. The document then crushes that buffer with the same nearest
//! opaque index `applyPng` uses.
//!
//! This is not the palette-extraction product (#126) and not a quality
//! benchmark (#127). `source` mode only caps the sampled colors with the
//! existing nearest-color merge so the frame has a palette.

use serde_json::{Map, Value};

use crate::error::KernelError;
use crate::palette_map::{self, INDEX_ALPHA_CUTOFF};
use crate::Rgba;

/// Engine used when a project has not stored one.
pub(crate) const DEFAULT_ENGINE_ID: &str = "center-sample";

/// Algorithm version stored in the snapshot. Bump the engine that changes.
pub(crate) const ENGINE_VERSION: &str = "1";

/// Paid engine id. The open-source build has no weights and does not run it.
pub(crate) const RESERVED_ENGINE_ID: &str = "pixelforge-native";

/// Largest source edge [`crate::Document::convert_png`] will expand.
pub(crate) const MAX_CONVERT_SOURCE_EDGE: u32 = 4096;

/// Default opaque cap for `palette.mode = source`. Not a #126 strategy.
pub(crate) const DEFAULT_MAX_COLORS: u8 = 16;

const DEFAULT_K: u8 = 16;
const KMEANS_ITERATIONS: usize = 8;
const DEFAULT_CONTRAST: u8 = 48;
const DEFAULT_EXPAND: u8 = 1;
const DEFAULT_MAX_CELL: u8 = 32;
const DEFAULT_SLACK: u8 = 1;
const MAX_FEATURES: usize = 32;
const MAX_FEATURE_CHARS: usize = 64;

/// How the sampled RGBA becomes palette indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PaletteMode {
    /// Crush onto the document palette. Does not add colors.
    Project,
    /// Cap the sampled opaque colors. Prerequisite for a later extractor.
    Source,
    /// Crush onto caller-supplied palette text. The text is not stored twice.
    Provided,
}

impl PaletteMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Source => "source",
            Self::Provided => "provided",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text {
            "project" => Some(Self::Project),
            "source" => Some(Self::Source),
            "provided" => Some(Self::Provided),
            _ => None,
        }
    }
}

/// Input snapshot written into the project after a successful convert.
///
/// Re-running the same PNG, engine, target, palette mode, features, and
/// `params` reproduces the indices. `features` are recorded and not applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TransformSnapshot {
    pub id: String,
    pub version: String,
    pub width: u16,
    pub height: u16,
    pub palette_mode: PaletteMode,
    pub max_colors: Option<u8>,
    pub features: Vec<String>,
    pub params: Map<String, Value>,
}

/// One convert, after the Tool API has checked the JSON object.
pub(crate) struct ConvertRequest {
    pub png: Vec<u8>,
    pub width: Option<u16>,
    pub height: Option<u16>,
    pub engine: Option<String>,
    pub palette: ConvertPalette,
    pub features: Vec<String>,
    pub params: Map<String, Value>,
    pub frame: Option<usize>,
    pub layer: Option<usize>,
}

pub(crate) enum ConvertPalette {
    Project,
    Source { max_colors: u8 },
    Provided { text: String },
}

impl ConvertPalette {
    pub(crate) fn mode(&self) -> PaletteMode {
        match self {
            Self::Project => PaletteMode::Project,
            Self::Source { .. } => PaletteMode::Source,
            Self::Provided { .. } => PaletteMode::Provided,
        }
    }

    pub(crate) fn max_colors(&self) -> Option<u8> {
        match self {
            Self::Source { max_colors } => Some(*max_colors),
            _ => None,
        }
    }
}

/// What one successful convert wrote, for the Tool API result.
pub(crate) struct ConvertOutcome {
    pub frame: usize,
    pub layer: usize,
    pub width: u16,
    pub height: u16,
    pub engine: String,
    pub version: String,
    pub snapshot: TransformSnapshot,
    pub report: Value,
    pub map: crate::palette_map::MapReport,
    pub features_ignored: Vec<String>,
}

/// Target-sized RGBA plus the engine's resolved parameters.
#[derive(Debug)]
pub(crate) struct Sampled {
    pub rgba: Vec<u8>,
    /// Opaque colors the engine already chose. `source` mode uses these
    /// instead of listing every sampled RGB. `None` lists the sample.
    pub suggested: Option<Vec<(Rgba, u32)>>,
    pub params: Map<String, Value>,
    pub version: &'static str,
    pub report: Value,
}

struct EngineSpec {
    id: &'static str,
    version: &'static str,
    reserved: bool,
    summary: &'static str,
}

const ENGINES: &[EngineSpec] = &[
    EngineSpec {
        id: "center-sample",
        version: ENGINE_VERSION,
        reserved: false,
        summary: "Sample the source pixel at the center of each target cell, then map it onto the palette.",
    },
    EngineSpec {
        id: "dominant",
        version: ENGINE_VERSION,
        reserved: false,
        summary: "Pick the majority source color in each target cell. Ties keep the lower RGB. No mixed color.",
    },
    EngineSpec {
        id: "kmeans",
        version: ENGINE_VERSION,
        reserved: false,
        summary: "Cluster opaque source colors, snap each center to a real source color, then vote inside the cell.",
    },
    EngineSpec {
        id: "contrast-outline",
        version: ENGINE_VERSION,
        reserved: false,
        summary: "Thicken contrast edges over opaque pixels, then keep the outline pixel nearest the block's lower median luminance.",
    },
    EngineSpec {
        id: "grid-snap",
        version: ENGINE_VERSION,
        reserved: false,
        summary: "Detect a repeated edge period, sample one color per cell, then center-sample that grid onto the target.",
    },
    EngineSpec {
        id: RESERVED_ENGINE_ID,
        version: "0",
        reserved: true,
        summary: "Reserved paid engine. Not installed in this build.",
    },
];

pub(crate) fn is_available(id: &str) -> bool {
    ENGINES
        .iter()
        .any(|engine| engine.id == id && !engine.reserved)
}

pub(crate) fn require_available(id: &str) -> Result<&'static str, KernelError> {
    let spec = find_engine(id)?;
    if spec.reserved {
        return Err(KernelError::EngineUnavailable {
            id: spec.id.to_string(),
        });
    }
    Ok(spec.id)
}

fn find_engine(id: &str) -> Result<&'static EngineSpec, KernelError> {
    if id.is_empty() || id.len() > 64 || id.chars().any(|ch| ch.is_control()) {
        return Err(KernelError::EngineUnknown {
            id: id.chars().take(64).collect(),
        });
    }
    ENGINES
        .iter()
        .find(|engine| engine.id == id)
        .ok_or_else(|| KernelError::EngineUnknown { id: id.to_string() })
}

/// Registry records for `listEngines` / `engine list`.
pub(crate) fn engines_json(selected: &str) -> Value {
    let engines = ENGINES
        .iter()
        .map(|engine| {
            serde_json::json!({
                "id": engine.id,
                "version": engine.version,
                "status": if engine.reserved { "reserved" } else { "available" },
                "available": !engine.reserved,
                "licensed": !engine.reserved,
                "summary": engine.summary,
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "default": DEFAULT_ENGINE_ID,
        "engine": selected,
        "engines": engines,
    })
}

pub(crate) fn normalize_features(features: &[String]) -> Result<Vec<String>, KernelError> {
    if features.len() > MAX_FEATURES {
        return Err(KernelError::ConvertFeature);
    }
    let mut out = Vec::with_capacity(features.len());
    for feature in features {
        let name = feature.trim();
        if name.is_empty()
            || name.chars().count() > MAX_FEATURE_CHARS
            || name.chars().any(|ch| ch.is_control())
        {
            return Err(KernelError::ConvertFeature);
        }
        out.push(name.to_string());
    }
    Ok(out)
}

/// Resolve engine parameters the same way a convert does, without an image.
pub(crate) fn normalize_params(
    id: &str,
    params: &Map<String, Value>,
) -> Result<Map<String, Value>, KernelError> {
    let id = require_available(id)?;
    match id {
        "center-sample" | "dominant" => {
            reject_unknown(params, &[])?;
            Ok(Map::new())
        }
        "kmeans" => {
            reject_unknown(params, &["k"])?;
            let k = optional_u8(params, "k", DEFAULT_K, 1, 255)?;
            Ok(one_u8("k", k))
        }
        "contrast-outline" => {
            reject_unknown(params, &["contrast", "expand"])?;
            let contrast = optional_u8(params, "contrast", DEFAULT_CONTRAST, 0, 255)?;
            let expand = optional_u8(params, "expand", DEFAULT_EXPAND, 0, 4)?;
            let mut map = Map::new();
            map.insert("contrast".to_string(), serde_json::json!(contrast));
            map.insert("expand".to_string(), serde_json::json!(expand));
            Ok(map)
        }
        "grid-snap" => {
            reject_unknown(params, &["maxCell", "slack"])?;
            let max_cell = optional_u8(params, "maxCell", DEFAULT_MAX_CELL, 2, 64)?;
            let slack = optional_u8(params, "slack", DEFAULT_SLACK, 0, 4)?;
            let mut map = Map::new();
            map.insert("maxCell".to_string(), serde_json::json!(max_cell));
            map.insert("slack".to_string(), serde_json::json!(slack));
            Ok(map)
        }
        _ => Err(KernelError::EngineUnknown { id: id.to_string() }),
    }
}

pub(crate) fn sample(
    id: &str,
    width: u32,
    height: u32,
    rgba: &[u8],
    target_width: u16,
    target_height: u16,
    params: &Map<String, Value>,
) -> Result<Sampled, KernelError> {
    let id = require_available(id)?;
    let params = normalize_params(id, params)?;
    let target_width = u32::from(target_width);
    let target_height = u32::from(target_height);
    let (rgba, suggested, report) = match id {
        "center-sample" => {
            let rgba = sample_center(rgba, width, height, target_width, target_height);
            (rgba, None, serde_json::json!({ "sampled": "center" }))
        }
        "dominant" => {
            let rgba = sample_dominant(rgba, width, height, target_width, target_height);
            (rgba, None, serde_json::json!({ "sampled": "majority" }))
        }
        "kmeans" => {
            let k = params["k"].as_u64().unwrap_or(u64::from(DEFAULT_K)) as u8;
            let (rgba, suggested, iterations) =
                sample_kmeans(rgba, width, height, target_width, target_height, k);
            (
                rgba,
                Some(suggested),
                serde_json::json!({
                    "sampled": "vote",
                    "k": k,
                    "iterations": iterations,
                }),
            )
        }
        "contrast-outline" => {
            let contrast = params["contrast"]
                .as_u64()
                .unwrap_or(u64::from(DEFAULT_CONTRAST)) as u8;
            let expand = params["expand"]
                .as_u64()
                .unwrap_or(u64::from(DEFAULT_EXPAND)) as u8;
            let rgba = sample_contrast(
                rgba,
                width,
                height,
                target_width,
                target_height,
                contrast,
                expand,
            );
            (
                rgba,
                None,
                serde_json::json!({
                    "sampled": "outline",
                    "contrast": contrast,
                    "expand": expand,
                }),
            )
        }
        "grid-snap" => {
            let max_cell = params["maxCell"]
                .as_u64()
                .unwrap_or(u64::from(DEFAULT_MAX_CELL)) as u8;
            let slack = params["slack"].as_u64().unwrap_or(u64::from(DEFAULT_SLACK)) as u8;
            let (rgba, report) = sample_grid(
                rgba,
                width,
                height,
                target_width,
                target_height,
                u32::from(max_cell),
                u32::from(slack),
            );
            (rgba, None, report)
        }
        _ => {
            return Err(KernelError::EngineUnknown { id: id.to_string() });
        }
    };
    Ok(Sampled {
        rgba,
        suggested,
        params,
        version: ENGINE_VERSION,
        report,
    })
}

/// Cap opaque colors with the existing non-averaging merge.
///
/// `colors` does not include the transparent slot. Empty input stays a
/// transparent-only palette so the caller can keep the document palette.
pub(crate) fn cap_opaque(colors: &[(Rgba, u32)], max_colors: usize) -> Vec<Rgba> {
    if colors.is_empty() || max_colors == 0 {
        return vec![Rgba::TRANSPARENT];
    }
    let mut palette = Vec::with_capacity(colors.len() + 1);
    let mut counts = Vec::with_capacity(colors.len() + 1);
    palette.push(Rgba::TRANSPARENT);
    counts.push(0);
    for (color, count) in colors {
        palette.push(*color);
        counts.push(*count);
    }
    palette_map::plan_max_colors(&palette, &counts, max_colors).colors
}

pub(crate) fn unique_opaque(rgba: &[u8]) -> Vec<(Rgba, u32)> {
    let mut counts = std::collections::BTreeMap::<(u8, u8, u8), u32>::new();
    for pixel in rgba.chunks_exact(4) {
        if pixel[3] < INDEX_ALPHA_CUTOFF {
            continue;
        }
        let entry = counts.entry((pixel[0], pixel[1], pixel[2])).or_insert(0);
        *entry = entry.saturating_add(1);
    }
    counts
        .into_iter()
        .map(|((r, g, b), count)| (Rgba::rgb(r, g, b), count))
        .collect()
}

pub(crate) fn parse_mode(text: &str) -> Option<PaletteMode> {
    PaletteMode::parse(text)
}

fn one_u8(key: &str, value: u8) -> Map<String, Value> {
    let mut map = Map::new();
    map.insert(key.to_string(), serde_json::json!(value));
    map
}

fn reject_unknown(params: &Map<String, Value>, allowed: &[&str]) -> Result<(), KernelError> {
    if params.len() > 16 {
        return Err(KernelError::EngineParams {
            detail: "too many engine parameters".to_string(),
        });
    }
    for key in params.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(KernelError::EngineParams {
                detail: format!("unknown field {key}"),
            });
        }
    }
    Ok(())
}

fn optional_u8(
    params: &Map<String, Value>,
    key: &str,
    default: u8,
    min: u8,
    max: u8,
) -> Result<u8, KernelError> {
    let Some(value) = params.get(key) else {
        return Ok(default);
    };
    let Some(number) = value.as_u64() else {
        return Err(KernelError::EngineParams {
            detail: format!("{key} must be an integer from {min} to {max}"),
        });
    };
    if number < u64::from(min) || number > u64::from(max) {
        return Err(KernelError::EngineParams {
            detail: format!("{key} must be an integer from {min} to {max}"),
        });
    }
    Ok(u8::try_from(number).unwrap_or(max))
}

fn sample_center(rgba: &[u8], width: u32, height: u32, target_w: u32, target_h: u32) -> Vec<u8> {
    let mut out = vec![0u8; target_len(target_w, target_h)];
    for y in 0..target_h {
        let sy = center_index(y, height, target_h);
        for x in 0..target_w {
            let sx = center_index(x, width, target_w);
            copy_pixel(rgba, width, sx, sy, &mut out, target_w, x, y);
        }
    }
    out
}

fn sample_dominant(rgba: &[u8], width: u32, height: u32, target_w: u32, target_h: u32) -> Vec<u8> {
    let mut out = vec![0u8; target_len(target_w, target_h)];
    for y in 0..target_h {
        let (y0, y1) = block_span(y, height, target_h);
        for x in 0..target_w {
            let (x0, x1) = block_span(x, width, target_w);
            if let Some((r, g, b)) = majority(rgba, width, x0, x1, y0, y1) {
                write_opaque(&mut out, target_w, x, y, r, g, b);
            }
        }
    }
    out
}

fn sample_kmeans(
    rgba: &[u8],
    width: u32,
    height: u32,
    target_w: u32,
    target_h: u32,
    k: u8,
) -> (Vec<u8>, Vec<(Rgba, u32)>, u8) {
    let colors = unique_opaque(rgba);
    if colors.is_empty() {
        return (vec![0u8; target_len(target_w, target_h)], Vec::new(), 0);
    }
    let palette_colors: Vec<((u8, u8, u8), u32)> = colors
        .iter()
        .map(|(color, count)| ((color.r, color.g, color.b), *count))
        .collect();
    let k = usize::from(k).min(palette_colors.len()).max(1);
    let mut centers = init_centers(&palette_colors, k);
    let mut assignment = vec![0usize; palette_colors.len()];
    let mut iterations = 0u8;
    for step in 0..KMEANS_ITERATIONS {
        iterations = u8::try_from(step + 1).unwrap_or(u8::MAX);
        assign_centers(&palette_colors, &centers, &mut assignment);
        let next = snap_centers(&palette_colors, &assignment, &centers);
        let stable = next == centers;
        centers = next;
        if stable {
            break;
        }
    }
    assign_centers(&palette_colors, &centers, &mut assignment);
    let mut cluster_of = std::collections::BTreeMap::<(u8, u8, u8), usize>::new();
    for (index, (color, _)) in palette_colors.iter().enumerate() {
        cluster_of.insert(*color, assignment[index]);
    }
    let mut out = vec![0u8; target_len(target_w, target_h)];
    let mut used = vec![0u32; centers.len()];
    for y in 0..target_h {
        let (y0, y1) = block_span(y, height, target_h);
        for x in 0..target_w {
            let (x0, x1) = block_span(x, width, target_w);
            if let Some(cluster) = majority_cluster(rgba, width, x0, x1, y0, y1, &cluster_of) {
                let (r, g, b) = centers[cluster];
                write_opaque(&mut out, target_w, x, y, r, g, b);
                used[cluster] = used[cluster].saturating_add(1);
            }
        }
    }
    let mut suggested = Vec::new();
    let mut seen = Vec::new();
    for (index, color) in centers.into_iter().enumerate() {
        if used[index] == 0 || seen.contains(&color) {
            continue;
        }
        seen.push(color);
        suggested.push((Rgba::rgb(color.0, color.1, color.2), used[index]));
    }
    (out, suggested, iterations)
}

fn sample_contrast(
    rgba: &[u8],
    width: u32,
    height: u32,
    target_w: u32,
    target_h: u32,
    contrast: u8,
    expand: u8,
) -> Vec<u8> {
    let mut image = rgba.to_vec();
    for pixel in image.chunks_exact_mut(4) {
        if pixel[3] < INDEX_ALPHA_CUTOFF {
            pixel.fill(0);
        } else {
            pixel[3] = 255;
        }
    }
    let mut outline = outline_mask(&image, width, height, u16::from(contrast));
    for _ in 0..expand {
        expand_outline(&mut image, &mut outline, width, height);
    }
    let mut out = vec![0u8; target_len(target_w, target_h)];
    for y in 0..target_h {
        let (y0, y1) = block_span(y, height, target_h);
        for x in 0..target_w {
            let (x0, x1) = block_span(x, width, target_w);
            if let Some((r, g, b)) = representative(&image, &outline, width, x0, x1, y0, y1) {
                write_opaque(&mut out, target_w, x, y, r, g, b);
            }
        }
    }
    out
}

fn sample_grid(
    rgba: &[u8],
    width: u32,
    height: u32,
    target_w: u32,
    target_h: u32,
    max_cell: u32,
    slack: u32,
) -> (Vec<u8>, Value) {
    let (vertical, horizontal) = edge_votes(rgba, width, height);
    let x_grid = best_period(&vertical, max_cell);
    let y_grid = best_period(&horizontal, max_cell);
    let Some((period_x, origin_x, score_x)) = x_grid else {
        return undetected(rgba, width, height, target_w, target_h);
    };
    let Some((period_y, origin_y, score_y)) = y_grid else {
        return undetected(rgba, width, height, target_w, target_h);
    };
    if score_x == 0 || score_y == 0 {
        return undetected(rgba, width, height, target_w, target_h);
    }
    let cols = snap_lines(&vertical, period_x, origin_x, slack);
    let rows = snap_lines(&horizontal, period_y, origin_y, slack);
    if cols.len() < 2 || rows.len() < 2 {
        return undetected(rgba, width, height, target_w, target_h);
    }
    let grid_w = (cols.len() - 1) as u32;
    let grid_h = (rows.len() - 1) as u32;
    let mut grid = vec![0u8; target_len(grid_w, grid_h)];
    for (gy, pair_y) in rows.windows(2).enumerate() {
        for (gx, pair_x) in cols.windows(2).enumerate() {
            if let Some((r, g, b)) =
                majority(rgba, width, pair_x[0], pair_x[1], pair_y[0], pair_y[1])
            {
                write_opaque(
                    &mut grid,
                    grid_w,
                    u32::try_from(gx).unwrap_or(0),
                    u32::try_from(gy).unwrap_or(0),
                    r,
                    g,
                    b,
                );
            }
        }
    }
    let out = sample_center(&grid, grid_w, grid_h, target_w, target_h);
    let report = serde_json::json!({
        "sampled": "grid",
        "detected": true,
        "periodX": period_x,
        "periodY": period_y,
        "originX": origin_x,
        "originY": origin_y,
        "columns": grid_w,
        "rows": grid_h,
    });
    (out, report)
}

fn undetected(
    rgba: &[u8],
    width: u32,
    height: u32,
    target_w: u32,
    target_h: u32,
) -> (Vec<u8>, Value) {
    (
        sample_center(rgba, width, height, target_w, target_h),
        serde_json::json!({
            "sampled": "grid",
            "detected": false,
        }),
    )
}

fn center_index(index: u32, source: u32, target: u32) -> u32 {
    let numer = (u64::from(index) * 2 + 1) * u64::from(source);
    let denom = u64::from(target) * 2;
    let at = u32::try_from(numer / denom).unwrap_or(0);
    at.min(source.saturating_sub(1))
}

fn block_span(index: u32, source: u32, target: u32) -> (u32, u32) {
    let start = index.saturating_mul(source) / target;
    let end = (index + 1).saturating_mul(source) / target;
    if end <= start {
        let at = start.min(source.saturating_sub(1));
        return (at, at + 1);
    }
    let start = start.min(source.saturating_sub(1));
    let end = end.min(source);
    if end <= start {
        (start, start + 1)
    } else {
        (start, end)
    }
}

fn majority(rgba: &[u8], width: u32, x0: u32, x1: u32, y0: u32, y1: u32) -> Option<(u8, u8, u8)> {
    let mut total = 0u32;
    let mut opaque = 0u32;
    let mut counts = std::collections::BTreeMap::<(u8, u8, u8), u32>::new();
    for y in y0..y1 {
        for x in x0..x1 {
            total = total.saturating_add(1);
            let pixel = pixel_at(rgba, width, x, y);
            if pixel[3] < INDEX_ALPHA_CUTOFF {
                continue;
            }
            opaque = opaque.saturating_add(1);
            let entry = counts.entry((pixel[0], pixel[1], pixel[2])).or_insert(0);
            *entry = entry.saturating_add(1);
        }
    }
    if total == 0 || opaque.saturating_mul(2) <= total {
        return None;
    }
    let mut best: Option<((u8, u8, u8), u32)> = None;
    for (color, count) in counts {
        let replace = best.is_none_or(|(current, current_count)| {
            count > current_count || (count == current_count && color < current)
        });
        if replace {
            best = Some((color, count));
        }
    }
    best.map(|(color, _)| color)
}

fn majority_cluster(
    rgba: &[u8],
    width: u32,
    x0: u32,
    x1: u32,
    y0: u32,
    y1: u32,
    cluster_of: &std::collections::BTreeMap<(u8, u8, u8), usize>,
) -> Option<usize> {
    let mut total = 0u32;
    let mut opaque = 0u32;
    let mut votes = std::collections::BTreeMap::<usize, u32>::new();
    for y in y0..y1 {
        for x in x0..x1 {
            total = total.saturating_add(1);
            let pixel = pixel_at(rgba, width, x, y);
            if pixel[3] < INDEX_ALPHA_CUTOFF {
                continue;
            }
            opaque = opaque.saturating_add(1);
            let Some(cluster) = cluster_of.get(&(pixel[0], pixel[1], pixel[2])).copied() else {
                continue;
            };
            let entry = votes.entry(cluster).or_insert(0);
            *entry = entry.saturating_add(1);
        }
    }
    if total == 0 || opaque.saturating_mul(2) <= total {
        return None;
    }
    let mut best: Option<(usize, u32)> = None;
    for (cluster, count) in votes {
        let replace = best.is_none_or(|(current, current_count)| {
            count > current_count || (count == current_count && cluster < current)
        });
        if replace {
            best = Some((cluster, count));
        }
    }
    best.map(|(cluster, _)| cluster)
}

fn init_centers(colors: &[((u8, u8, u8), u32)], k: usize) -> Vec<(u8, u8, u8)> {
    let mut order: Vec<usize> = (0..colors.len()).collect();
    order.sort_by(|&left, &right| {
        colors[right]
            .1
            .cmp(&colors[left].1)
            .then(colors[left].0.cmp(&colors[right].0))
    });
    let mut centers = vec![colors[order[0]].0];
    while centers.len() < k {
        let mut found = false;
        let mut best_color = colors[order[0]].0;
        let mut best_dist = 0u32;
        let mut best_count = 0u32;
        for &index in &order {
            let color = colors[index].0;
            if centers.contains(&color) {
                continue;
            }
            let dist = centers
                .iter()
                .map(|center| dist_sq(color, *center))
                .min()
                .unwrap_or(0);
            let count = colors[index].1;
            let better = !found
                || dist > best_dist
                || (dist == best_dist && count > best_count)
                || (dist == best_dist && count == best_count && color < best_color);
            if better {
                found = true;
                best_color = color;
                best_dist = dist;
                best_count = count;
            }
        }
        if !found {
            break;
        }
        centers.push(best_color);
    }
    centers
}

fn assign_centers(colors: &[((u8, u8, u8), u32)], centers: &[(u8, u8, u8)], into: &mut [usize]) {
    for (index, (color, _)) in colors.iter().enumerate() {
        let mut best = 0usize;
        let mut best_dist = u32::MAX;
        for (center_index, center) in centers.iter().enumerate() {
            let dist = dist_sq(*color, *center);
            if dist < best_dist || (dist == best_dist && center_index < best) {
                best_dist = dist;
                best = center_index;
            }
        }
        into[index] = best;
    }
}

fn snap_centers(
    colors: &[((u8, u8, u8), u32)],
    assignment: &[usize],
    centers: &[(u8, u8, u8)],
) -> Vec<(u8, u8, u8)> {
    let mut next = centers.to_vec();
    for (center_index, center) in centers.iter().enumerate() {
        let mut sum_r = 0u64;
        let mut sum_g = 0u64;
        let mut sum_b = 0u64;
        let mut weight = 0u64;
        for (index, (color, count)) in colors.iter().enumerate() {
            if assignment[index] != center_index {
                continue;
            }
            let count = u64::from(*count);
            sum_r += u64::from(color.0) * count;
            sum_g += u64::from(color.1) * count;
            sum_b += u64::from(color.2) * count;
            weight += count;
        }
        if weight == 0 {
            next[center_index] = *center;
            continue;
        }
        let mean = (
            u8::try_from(sum_r / weight).unwrap_or(255),
            u8::try_from(sum_g / weight).unwrap_or(255),
            u8::try_from(sum_b / weight).unwrap_or(255),
        );
        let mut best = *center;
        let mut best_dist = u32::MAX;
        let mut picked = false;
        for (index, (color, _)) in colors.iter().enumerate() {
            if assignment[index] != center_index {
                continue;
            }
            let dist = dist_sq(*color, mean);
            if !picked || dist < best_dist || (dist == best_dist && *color < best) {
                picked = true;
                best_dist = dist;
                best = *color;
            }
        }
        next[center_index] = best;
    }
    next
}

fn outline_mask(rgba: &[u8], width: u32, height: u32, contrast: u16) -> Vec<bool> {
    let mut mask = vec![false; (width as usize) * (height as usize)];
    for y in 0..height {
        for x in 0..width {
            let here = luma_at(rgba, width, x, y);
            let Some(here) = here else {
                continue;
            };
            let mut edge = false;
            for (nx, ny) in neighbors(width, height, x, y) {
                match luma_at(rgba, width, nx, ny) {
                    None => edge = true,
                    Some(other) if here.abs_diff(other) >= contrast => edge = true,
                    Some(_) => {}
                }
            }
            if edge {
                mask[(y as usize) * (width as usize) + (x as usize)] = true;
            }
        }
    }
    mask
}

fn expand_outline(rgba: &mut [u8], outline: &mut [bool], width: u32, height: u32) {
    let source = rgba.to_vec();
    let source_outline = outline.to_vec();
    for y in 0..height {
        for x in 0..width {
            let offset = ((y * width + x) * 4) as usize;
            if source[offset + 3] < INDEX_ALPHA_CUTOFF {
                continue;
            }
            let index = (y as usize) * (width as usize) + (x as usize);
            if source_outline[index] {
                continue;
            }
            let here = luma(source[offset], source[offset + 1], source[offset + 2]);
            let mut best: Option<(u16, (u8, u8, u8))> = None;
            for (nx, ny) in neighbors(width, height, x, y) {
                let n_index = (ny as usize) * (width as usize) + (nx as usize);
                if !source_outline[n_index] {
                    continue;
                }
                let n_off = ((ny * width + nx) * 4) as usize;
                if source[n_off + 3] < INDEX_ALPHA_CUTOFF {
                    continue;
                }
                let color = (source[n_off], source[n_off + 1], source[n_off + 2]);
                let diff = here.abs_diff(luma(color.0, color.1, color.2));
                let replace = best.is_none_or(|(best_diff, best_color)| {
                    diff > best_diff || (diff == best_diff && color < best_color)
                });
                if replace {
                    best = Some((diff, color));
                }
            }
            if let Some((_, color)) = best {
                rgba[offset] = color.0;
                rgba[offset + 1] = color.1;
                rgba[offset + 2] = color.2;
                rgba[offset + 3] = 255;
                outline[index] = true;
            }
        }
    }
}

fn representative(
    rgba: &[u8],
    outline: &[bool],
    width: u32,
    x0: u32,
    x1: u32,
    y0: u32,
    y1: u32,
) -> Option<(u8, u8, u8)> {
    let mut total = 0u32;
    let mut opaque = 0u32;
    let mut colors = Vec::new();
    for y in y0..y1 {
        for x in x0..x1 {
            total = total.saturating_add(1);
            let pixel = pixel_at(rgba, width, x, y);
            if pixel[3] < INDEX_ALPHA_CUTOFF {
                continue;
            }
            opaque = opaque.saturating_add(1);
            let index = (y as usize) * (width as usize) + (x as usize);
            colors.push((
                outline[index],
                (pixel[0], pixel[1], pixel[2]),
                luma(pixel[0], pixel[1], pixel[2]),
            ));
        }
    }
    if total == 0 || opaque.saturating_mul(2) <= total || colors.is_empty() {
        return None;
    }
    let outlined = colors.iter().any(|(edge, _, _)| *edge);
    let pool: Vec<(u8, u8, u8, u16)> = colors
        .into_iter()
        .filter(|(edge, _, _)| !outlined || *edge)
        .map(|(_, color, luma)| (color.0, color.1, color.2, luma))
        .collect();
    if pool.is_empty() {
        return None;
    }
    let mut lumas: Vec<u16> = pool.iter().map(|(_, _, _, luma)| *luma).collect();
    lumas.sort_unstable();
    let median = lumas[(lumas.len() - 1) / 2];
    let mut best = pool[0];
    for candidate in pool.iter().copied().skip(1) {
        let best_diff = best.3.abs_diff(median);
        let next_diff = candidate.3.abs_diff(median);
        let better = next_diff < best_diff
            || (next_diff == best_diff
                && (candidate.0, candidate.1, candidate.2) < (best.0, best.1, best.2));
        if better {
            best = candidate;
        }
    }
    Some((best.0, best.1, best.2))
}

fn edge_votes(rgba: &[u8], width: u32, height: u32) -> (Vec<u32>, Vec<u32>) {
    let mut vertical = vec![0u32; width as usize + 1];
    let mut horizontal = vec![0u32; height as usize + 1];
    for y in 0..height {
        for x in 0..width {
            let here = luma_at(rgba, width, x, y);
            if x + 1 < width {
                let right = luma_at(rgba, width, x + 1, y);
                let strength = edge_strength(here, right);
                vertical[(x + 1) as usize] = vertical[(x + 1) as usize].saturating_add(strength);
            }
            if y + 1 < height {
                let below = luma_at(rgba, width, x, y + 1);
                let strength = edge_strength(here, below);
                horizontal[(y + 1) as usize] =
                    horizontal[(y + 1) as usize].saturating_add(strength);
            }
        }
    }
    // The image edge is a grid line. Seeding it with the strongest interior
    // vote keeps a short period from losing to a sparser lattice that skips
    // the zero borders, and a flat image stays undetected.
    seal_border(&mut vertical);
    seal_border(&mut horizontal);
    (vertical, horizontal)
}

fn seal_border(votes: &mut [u32]) {
    if votes.len() < 3 {
        return;
    }
    let peak = votes[1..votes.len() - 1].iter().copied().max().unwrap_or(0);
    if peak == 0 {
        return;
    }
    votes[0] = votes[0].max(peak);
    let last = votes.len() - 1;
    votes[last] = votes[last].max(peak);
}

fn edge_strength(left: Option<u16>, right: Option<u16>) -> u32 {
    match (left, right) {
        (Some(left), Some(right)) => u32::from(left.abs_diff(right)),
        (Some(_), None) | (None, Some(_)) => 255,
        (None, None) => 0,
    }
}

/// `(period, origin, average vote)`.
fn best_period(votes: &[u32], max_cell: u32) -> Option<(u32, u32, u64)> {
    if votes.len() < 2 {
        return None;
    }
    let last = (votes.len() - 1) as u32;
    let max_period = max_cell.min(last);
    if max_period < 2 {
        return None;
    }
    let mut best: Option<(u64, u32, u32)> = None;
    for period in 2..=max_period {
        for origin in 0..period {
            let mut score = 0u64;
            let mut lines = 0u64;
            let mut at = origin;
            while at <= last {
                score += u64::from(votes[at as usize]);
                lines += 1;
                at += period;
            }
            if lines < 2 {
                continue;
            }
            let average = score / lines;
            let replace = best.is_none_or(|(best_avg, best_period, best_origin)| {
                average > best_avg
                    || (average == best_avg
                        && (period < best_period
                            || (period == best_period && origin < best_origin)))
            });
            if replace {
                best = Some((average, period, origin));
            }
        }
    }
    best.map(|(average, period, origin)| (period, origin, average))
}

fn snap_lines(votes: &[u32], period: u32, origin: u32, slack: u32) -> Vec<u32> {
    let last = (votes.len() - 1) as u32;
    let mut lines = vec![0u32];
    let mut cursor = if origin == 0 { period } else { origin };
    while cursor < last {
        let prev = *lines.last().unwrap_or(&0);
        let lo = cursor.saturating_sub(slack).max(prev + 1);
        let hi = (cursor + slack).min(last.saturating_sub(1));
        if lo > hi {
            break;
        }
        let mut chosen = lo;
        let mut best = votes[lo as usize];
        for at in lo..=hi {
            let vote = votes[at as usize];
            if vote > best {
                best = vote;
                chosen = at;
            }
        }
        if best == 0 {
            chosen = cursor.clamp(lo, hi);
        }
        if chosen <= prev || chosen >= last {
            break;
        }
        lines.push(chosen);
        cursor = chosen.saturating_add(period);
    }
    if *lines.last().unwrap_or(&0) != last {
        lines.push(last);
    }
    lines
}

fn neighbors(width: u32, height: u32, x: u32, y: u32) -> impl Iterator<Item = (u32, u32)> {
    let mut cells = Vec::with_capacity(4);
    if x > 0 {
        cells.push((x - 1, y));
    }
    if x + 1 < width {
        cells.push((x + 1, y));
    }
    if y > 0 {
        cells.push((x, y - 1));
    }
    if y + 1 < height {
        cells.push((x, y + 1));
    }
    cells.into_iter()
}

fn luma_at(rgba: &[u8], width: u32, x: u32, y: u32) -> Option<u16> {
    let pixel = pixel_at(rgba, width, x, y);
    if pixel[3] < INDEX_ALPHA_CUTOFF {
        None
    } else {
        Some(luma(pixel[0], pixel[1], pixel[2]))
    }
}

fn luma(r: u8, g: u8, b: u8) -> u16 {
    (u16::from(r) * 30 + u16::from(g) * 59 + u16::from(b) * 11) / 100
}

fn dist_sq(left: (u8, u8, u8), right: (u8, u8, u8)) -> u32 {
    let dr = i32::from(left.0) - i32::from(right.0);
    let dg = i32::from(left.1) - i32::from(right.1);
    let db = i32::from(left.2) - i32::from(right.2);
    u32::try_from(dr * dr + dg * dg + db * db).unwrap_or(u32::MAX)
}

fn target_len(width: u32, height: u32) -> usize {
    (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(4)
}

fn pixel_at(rgba: &[u8], width: u32, x: u32, y: u32) -> &[u8] {
    let offset = ((y * width + x) * 4) as usize;
    &rgba[offset..offset + 4]
}

fn copy_pixel(
    rgba: &[u8],
    width: u32,
    sx: u32,
    sy: u32,
    out: &mut [u8],
    target_w: u32,
    x: u32,
    y: u32,
) {
    let pixel = pixel_at(rgba, width, sx, sy);
    if pixel[3] < INDEX_ALPHA_CUTOFF {
        return;
    }
    write_opaque(out, target_w, x, y, pixel[0], pixel[1], pixel[2]);
}

fn write_opaque(out: &mut [u8], width: u32, x: u32, y: u32, r: u8, g: u8, b: u8) {
    let offset = ((y * width + x) * 4) as usize;
    out[offset] = r;
    out[offset + 1] = g;
    out[offset + 2] = b;
    out[offset + 3] = 255;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for _ in 0..width * height {
            rgba.extend_from_slice(&color);
        }
        rgba
    }

    fn put(rgba: &mut [u8], width: u32, x: u32, y: u32, color: [u8; 4]) {
        let offset = ((y * width + x) * 4) as usize;
        rgba[offset..offset + 4].copy_from_slice(&color);
    }

    #[test]
    fn center_sample_picks_the_cell_center_and_keeps_identity() {
        let mut rgba = solid(4, 1, [0, 0, 0, 0]);
        put(&mut rgba, 4, 0, 0, [1, 0, 0, 255]);
        put(&mut rgba, 4, 1, 0, [2, 0, 0, 255]);
        put(&mut rgba, 4, 2, 0, [3, 0, 0, 255]);
        put(&mut rgba, 4, 3, 0, [4, 0, 0, 255]);
        let down = sample_center(&rgba, 4, 1, 2, 1);
        assert_eq!(&down[0..4], &[2, 0, 0, 255]);
        assert_eq!(&down[4..8], &[4, 0, 0, 255]);
        let same = sample_center(&rgba, 4, 1, 4, 1);
        assert_eq!(same, rgba);
        let up = sample_center(&[9, 8, 7, 255], 1, 1, 2, 1);
        assert_eq!(up, vec![9, 8, 7, 255, 9, 8, 7, 255]);
    }

    #[test]
    fn alpha_below_128_is_transparent_and_a_tie_stays_clear() {
        let rgba = vec![
            255, 0, 0, 127, 0, 255, 0, 128, 0, 0, 255, 255, 255, 255, 255, 255,
        ];
        let center = sample_center(&rgba, 2, 2, 2, 2);
        assert_eq!(&center[0..4], &[0, 0, 0, 0]);
        assert_eq!(&center[4..8], &[0, 255, 0, 255]);
        let half = vec![255, 0, 0, 255, 0, 0, 0, 0];
        assert_eq!(majority(&half, 2, 0, 2, 0, 1), None);
        let tie = vec![10, 0, 0, 255, 1, 0, 0, 255];
        assert_eq!(majority(&tie, 2, 0, 2, 0, 1), Some((1, 0, 0)));
    }

    #[test]
    fn dominant_does_not_mix_and_contrast_keeps_the_outline() {
        let rgba = vec![
            0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
        ];
        assert_eq!(sample_dominant(&rgba, 4, 1, 1, 1), vec![255, 255, 255, 255]);
        assert_eq!(sample_center(&rgba, 4, 1, 1, 1), vec![255, 255, 255, 255]);
        let outline = sample_contrast(&rgba, 4, 1, 1, 1, 48, 0);
        assert_eq!(outline, vec![0, 0, 0, 255]);
    }

    #[test]
    fn kmeans_is_deterministic_and_uses_source_colors() {
        let mut rgba = Vec::new();
        for shade in [0u8, 8, 16, 24, 200, 210, 220, 230] {
            rgba.extend_from_slice(&[shade, 0, 0, 255]);
        }
        let params = one_u8("k", 2);
        let once = sample("kmeans", 8, 1, &rgba, 8, 1, &params).unwrap();
        let twice = sample("kmeans", 8, 1, &rgba, 8, 1, &params).unwrap();
        assert_eq!(once.rgba, twice.rgba);
        assert_eq!(once.params, twice.params);
        let suggested = once.suggested.unwrap();
        assert_eq!(suggested.len(), 2);
        for (color, _) in &suggested {
            assert!(rgba.chunks_exact(4).any(|pixel| {
                pixel[0] == color.r && pixel[1] == color.g && pixel[2] == color.b
            }));
        }
        for pixel in once.rgba.chunks_exact(4) {
            if pixel[3] == 0 {
                continue;
            }
            assert!(
                suggested.iter().any(|(color, _)| color.r == pixel[0]
                    && color.g == pixel[1]
                    && color.b == pixel[2]),
                "voted color is a cluster color"
            );
        }
    }

    #[test]
    fn grid_snap_recovers_a_two_pixel_lattice() {
        let mut rgba = solid(8, 8, [0, 0, 0, 255]);
        for by in 0..4u32 {
            for bx in 0..4u32 {
                let color = [bx as u8 * 40, by as u8 * 40, 30, 255];
                for y in 0..2 {
                    for x in 0..2 {
                        put(&mut rgba, 8, bx * 2 + x, by * 2 + y, color);
                    }
                }
            }
        }
        let (out, report) = sample_grid(&rgba, 8, 8, 4, 4, 32, 1);
        assert_eq!(report["detected"], serde_json::json!(true));
        assert_eq!(report["periodX"], serde_json::json!(2));
        assert_eq!(report["columns"], serde_json::json!(4));
        assert_eq!(&out[0..4], &[0, 0, 30, 255]);
        assert_eq!(&out[12..16], &[120, 0, 30, 255]);
        let flat = solid(4, 4, [4, 5, 6, 255]);
        let (_, missed) = sample_grid(&flat, 4, 4, 2, 2, 32, 1);
        assert_eq!(missed["detected"], serde_json::json!(false));
    }

    #[test]
    fn reserved_and_unknown_engines_do_not_fall_back() {
        let err = require_available(RESERVED_ENGINE_ID).unwrap_err();
        assert!(matches!(err, KernelError::EngineUnavailable { .. }));
        assert_eq!(err.code(), "EngineUnavailable");
        let unknown = require_available("nope").unwrap_err();
        assert!(matches!(unknown, KernelError::EngineUnknown { .. }));
        let params = one_u8("k", 4);
        let rejected = sample("center-sample", 1, 1, &[0, 0, 0, 255], 1, 1, &params).unwrap_err();
        assert!(matches!(rejected, KernelError::EngineParams { .. }));
    }

    #[test]
    fn omitted_engine_parameters_normalize_to_the_defaults() {
        let params = normalize_params("kmeans", &Map::new()).unwrap();
        assert_eq!(params["k"], serde_json::json!(DEFAULT_K));
        let explicit = normalize_params("kmeans", &one_u8("k", 16)).unwrap();
        assert_eq!(params, explicit);
        let bad = normalize_params("kmeans", &one_u8("k", 0)).unwrap_err();
        assert!(matches!(bad, KernelError::EngineParams { .. }));
    }
}
