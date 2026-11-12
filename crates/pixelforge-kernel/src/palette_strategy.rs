//! Palette strategies for [`crate::Document::convert_png`].
//!
//! Three ids are registered. An unknown id is
//! [`KernelError::PaletteStrategyUnknown`] and does not run another strategy.
//!
//! - `source` keeps the source image's palette. An indexed PNG contributes
//!   each PLTE entry whose alpha is at least [`INDEX_ALPHA_CUTOFF`], in file
//!   order, including entries the sample never uses. Any other PNG contributes
//!   each distinct opaque RGB in first-seen order. More than 255 opaque colors
//!   is [`KernelError::PaletteTooLarge`]. Nothing is merged and no new RGB is
//!   invented.
//! - `limit` reduces that set to `max_colors` with the same non-averaging
//!   merge as `remapPalette`. An indexed palette that fits is the starting
//!   set. Otherwise the distinct opaque colors of the sampled frame are.
//!   More than 255 of those keep the 255 most frequent, and a tie keeps the
//!   lower RGB. The survivor's RGB is not averaged.
//! - `project` maps every opaque sample onto the document palette. It does
//!   not add or remove a color.
//!
//! [`PaletteReport`] is the merge and error record. Distance is squared RGB.
//! Alpha below the cutoff is index 0 and is not an RGB error. A source with
//! no opaque color leaves the document palette in place and says so with
//! `kept_existing`; that is not a switch to another strategy id.
//!
//! This is not a quality score. The same image, strategy, and parameters
//! write the same palette and the same indices.

use std::collections::BTreeMap;
use std::io::Cursor;

use serde_json::{json, Value};

use crate::error::KernelError;
use crate::palette::Palette;
use crate::palette_map::{self, INDEX_ALPHA_CUTOFF};
use crate::Rgba;

/// Algorithm version stored on [`PaletteReport`].
pub(crate) const STRATEGY_VERSION: &str = "1";

/// Registered convert palette strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PaletteStrategy {
    /// Keep the source image palette.
    Source,
    /// Reduce the source colors to a fixed opaque cap.
    Limit,
    /// Map onto the document palette.
    Project,
}

impl PaletteStrategy {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Limit => "limit",
            Self::Project => "project",
        }
    }

    pub(crate) fn parse(text: &str) -> Option<Self> {
        match text {
            "source" => Some(Self::Source),
            "limit" => Some(Self::Limit),
            "project" => Some(Self::Project),
            _ => None,
        }
    }
}

/// Strategy selector after the Tool API has read the JSON string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StrategyRequest {
    Source,
    Limit {
        max_colors: u8,
    },
    Project,
    /// Not one of the three ids. Convert must not substitute another.
    Unknown {
        id: String,
    },
}

/// A request whose id is registered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KnownStrategy {
    Source,
    Limit { max_colors: u8 },
    Project,
}

impl KnownStrategy {
    pub(crate) fn id(self) -> PaletteStrategy {
        match self {
            Self::Source => PaletteStrategy::Source,
            Self::Limit { .. } => PaletteStrategy::Limit,
            Self::Project => PaletteStrategy::Project,
        }
    }
}

/// Resolve a strategy id. Unknown ids error before any pixel is written.
pub(crate) fn require_known(request: &StrategyRequest) -> Result<KnownStrategy, KernelError> {
    match request {
        StrategyRequest::Source => Ok(KnownStrategy::Source),
        StrategyRequest::Limit { max_colors } => Ok(KnownStrategy::Limit {
            max_colors: *max_colors,
        }),
        StrategyRequest::Project => Ok(KnownStrategy::Project),
        StrategyRequest::Unknown { id } => {
            Err(KernelError::PaletteStrategyUnknown { id: id.clone() })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Candidate {
    source_index: u32,
    /// False when the color was not in the extracted source palette.
    in_source: bool,
    color: Rgba,
    pixels: u32,
}

/// Palette a strategy will install, plus the colors it started from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedPalette {
    pub strategy: PaletteStrategy,
    /// `indexed`, `colors`, `sampled`, `project`, or `existing`.
    pub origin: &'static str,
    /// True when the source had no opaque color and the document palette stayed.
    pub kept_existing: bool,
    pub colors: Vec<Rgba>,
    candidates: Vec<Candidate>,
}

/// One sampled color that did not land on an equal RGB.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MergeRow {
    source_index: Option<u32>,
    from: Rgba,
    into_index: u8,
    into: Rgba,
    pixels: u32,
    distance: u32,
}

/// An unused source color that the cap removed.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DroppedRow {
    source_index: Option<u32>,
    color: Rgba,
    pixels: u32,
}

/// Structured merge and error report for one strategy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PaletteReport {
    pub strategy: PaletteStrategy,
    pub origin: &'static str,
    pub kept_existing: bool,
    pub source_colors: u32,
    pub palette_colors: u32,
    pub merged_colors: u32,
    pub dropped_colors: u32,
    pub opaque_pixels: u32,
    pub transparent_pixels: u32,
    pub exact_pixels: u32,
    pub inexact_pixels: u32,
    pub max_distance: u32,
    pub total_distance: u64,
    merges: Vec<MergeRow>,
    dropped: Vec<DroppedRow>,
}

impl PaletteReport {
    /// JSON object a UI or agent can read without parsing logs.
    pub(crate) fn to_json(&self) -> Value {
        json!({
            "strategy": self.strategy.as_str(),
            "version": STRATEGY_VERSION,
            "metric": "squared-rgb",
            "origin": self.origin,
            "keptExisting": self.kept_existing,
            "sourceColors": self.source_colors,
            "paletteColors": self.palette_colors,
            "mergedColors": self.merged_colors,
            "droppedColors": self.dropped_colors,
            "opaquePixels": self.opaque_pixels,
            "transparentPixels": self.transparent_pixels,
            "exactPixels": self.exact_pixels,
            "inexactPixels": self.inexact_pixels,
            "maxDistance": self.max_distance,
            "totalDistance": self.total_distance,
            "merges": self.merges.iter().map(merge_json).collect::<Vec<_>>(),
            "dropped": self.dropped.iter().map(dropped_json).collect::<Vec<_>>(),
        })
    }
}

/// Build the destination palette for one known strategy.
///
/// `source_rgba` is the decoded PNG. `sampled_rgba` is the engine output at
/// the target size. Neither buffer is resized here.
pub(crate) fn prepare(
    strategy: KnownStrategy,
    png: &[u8],
    source_rgba: &[u8],
    sampled_rgba: &[u8],
    project: &[Rgba],
) -> Result<PreparedPalette, KernelError> {
    match strategy {
        KnownStrategy::Project => Ok(PreparedPalette {
            strategy: PaletteStrategy::Project,
            origin: "project",
            kept_existing: false,
            colors: project.to_vec(),
            candidates: histogram(sampled_rgba),
        }),
        KnownStrategy::Source => prepare_source(png, source_rgba, sampled_rgba, project),
        KnownStrategy::Limit { max_colors } => {
            prepare_limit(png, sampled_rgba, project, max_colors)
        }
    }
}

/// Fill the error totals from the indices crush wrote.
pub(crate) fn finish_report(
    prepared: &PreparedPalette,
    sampled_rgba: &[u8],
    indices: &[u8],
) -> PaletteReport {
    let mut groups: BTreeMap<(u32, u8, u8, u8), MergeRow> = BTreeMap::new();
    let mut opaque_pixels = 0u32;
    let mut transparent_pixels = 0u32;
    let mut exact_pixels = 0u32;
    let mut max_distance = 0u32;
    let mut total_distance = 0u64;
    let samples = sampled_rgba.len() / 4;
    for cell in 0..samples.min(indices.len()) {
        let offset = cell * 4;
        let pixel = &sampled_rgba[offset..offset + 4];
        if pixel[3] < INDEX_ALPHA_CUTOFF {
            transparent_pixels = transparent_pixels.saturating_add(1);
            continue;
        }
        opaque_pixels = opaque_pixels.saturating_add(1);
        let into_index = indices[cell];
        let into = prepared
            .colors
            .get(usize::from(into_index))
            .copied()
            .unwrap_or(Rgba::TRANSPARENT);
        let from = Rgba::rgb(pixel[0], pixel[1], pixel[2]);
        let distance = distance_sq((from.r, from.g, from.b), into);
        if distance == 0 {
            exact_pixels = exact_pixels.saturating_add(1);
            continue;
        }
        max_distance = max_distance.max(distance);
        total_distance = total_distance.saturating_add(u64::from(distance));
        let source_index = source_index_of(&prepared.candidates, from);
        let key = (source_index.unwrap_or(u32::MAX), from.r, from.g, from.b);
        let row = groups.entry(key).or_insert(MergeRow {
            source_index,
            from,
            into_index,
            into,
            pixels: 0,
            distance,
        });
        row.pixels = row.pixels.saturating_add(1);
    }
    let merges: Vec<MergeRow> = groups.into_values().collect();
    let dropped = unused_removed(&prepared.candidates, &prepared.colors);
    let inexact_pixels = opaque_pixels.saturating_sub(exact_pixels);
    PaletteReport {
        strategy: prepared.strategy,
        origin: prepared.origin,
        kept_existing: prepared.kept_existing,
        source_colors: u32::try_from(prepared.candidates.len()).unwrap_or(u32::MAX),
        palette_colors: opaque_len(&prepared.colors),
        merged_colors: u32::try_from(merges.len()).unwrap_or(u32::MAX),
        dropped_colors: u32::try_from(dropped.len()).unwrap_or(u32::MAX),
        opaque_pixels,
        transparent_pixels,
        exact_pixels,
        inexact_pixels,
        max_distance,
        total_distance,
        merges,
        dropped,
    }
}

fn prepare_source(
    png: &[u8],
    source_rgba: &[u8],
    sampled_rgba: &[u8],
    project: &[Rgba],
) -> Result<PreparedPalette, KernelError> {
    let indexed = read_indexed_opaque(png)?;
    let (origin, mut candidates) = if let Some(colors) = indexed {
        if colors.is_empty() {
            return Ok(kept_existing(PaletteStrategy::Source, project));
        }
        if colors.len() > 255 {
            return Err(KernelError::PaletteTooLarge {
                len: colors.len() + 1,
            });
        }
        ("indexed", colors)
    } else {
        let colors = unique_colors(source_rgba, true)?;
        if colors.is_empty() {
            return Ok(kept_existing(PaletteStrategy::Source, project));
        }
        ("colors", colors)
    };
    note_sample(&mut candidates, sampled_rgba, true)?;
    let colors = install(&candidates)?;
    Ok(PreparedPalette {
        strategy: PaletteStrategy::Source,
        origin,
        kept_existing: false,
        colors,
        candidates,
    })
}

fn prepare_limit(
    png: &[u8],
    sampled_rgba: &[u8],
    project: &[Rgba],
    max_colors: u8,
) -> Result<PreparedPalette, KernelError> {
    let indexed = read_indexed_opaque(png)?;
    let (origin, mut candidates) =
        if let Some(colors) = indexed.filter(|colors| !colors.is_empty() && colors.len() <= 255) {
            ("indexed", colors)
        } else {
            ("sampled", histogram(sampled_rgba))
        };
    if origin == "indexed" {
        note_sample(&mut candidates, sampled_rgba, false)?;
    }
    if candidates.is_empty() {
        return Ok(kept_existing(PaletteStrategy::Limit, project));
    }
    let colors = limit_colors(&candidates, max_colors)?;
    Ok(PreparedPalette {
        strategy: PaletteStrategy::Limit,
        origin,
        kept_existing: false,
        colors,
        candidates,
    })
}

fn kept_existing(strategy: PaletteStrategy, project: &[Rgba]) -> PreparedPalette {
    PreparedPalette {
        strategy,
        origin: "existing",
        kept_existing: true,
        colors: project.to_vec(),
        candidates: Vec::new(),
    }
}

fn install(candidates: &[Candidate]) -> Result<Vec<Rgba>, KernelError> {
    let mut colors = Vec::with_capacity(candidates.len() + 1);
    colors.push(Rgba::TRANSPARENT);
    colors.extend(candidates.iter().map(|candidate| candidate.color));
    Palette::new(colors.clone())?;
    Ok(colors)
}

fn limit_colors(candidates: &[Candidate], max_colors: u8) -> Result<Vec<Rgba>, KernelError> {
    let working = if candidates.len() > 255 {
        keep_most_frequent(candidates, 255)
    } else {
        candidates.to_vec()
    };
    let mut palette = Vec::with_capacity(working.len() + 1);
    let mut counts = Vec::with_capacity(working.len() + 1);
    palette.push(Rgba::TRANSPARENT);
    counts.push(0);
    for candidate in &working {
        palette.push(candidate.color);
        counts.push(candidate.pixels);
    }
    let colors = palette_map::plan_max_colors(&palette, &counts, usize::from(max_colors)).colors;
    Palette::new(colors.clone())?;
    Ok(colors)
}

/// Keep `limit` entries. Higher sample counts win. A tie keeps the lower RGB,
/// then the earlier source index. Survivors stay in their previous order.
fn keep_most_frequent(candidates: &[Candidate], limit: usize) -> Vec<Candidate> {
    let mut ranked: Vec<usize> = (0..candidates.len()).collect();
    ranked.sort_by(|&left, &right| {
        let a = &candidates[left];
        let b = &candidates[right];
        b.pixels
            .cmp(&a.pixels)
            .then(a.color.r.cmp(&b.color.r))
            .then(a.color.g.cmp(&b.color.g))
            .then(a.color.b.cmp(&b.color.b))
            .then(a.source_index.cmp(&b.source_index))
            .then(left.cmp(&right))
    });
    let mut keep = vec![false; candidates.len()];
    for index in ranked.into_iter().take(limit) {
        keep[index] = true;
    }
    candidates
        .iter()
        .enumerate()
        .filter(|(index, _)| keep[*index])
        .map(|(_, candidate)| candidate.clone())
        .collect()
}

fn unused_removed(candidates: &[Candidate], palette: &[Rgba]) -> Vec<DroppedRow> {
    let mut kept = BTreeMap::<(u8, u8, u8), ()>::new();
    for color in palette.iter().skip(1) {
        kept.insert((color.r, color.g, color.b), ());
    }
    let mut dropped = Vec::new();
    for candidate in candidates {
        let key = (candidate.color.r, candidate.color.g, candidate.color.b);
        if candidate.pixels == 0 && !kept.contains_key(&key) {
            dropped.push(DroppedRow {
                source_index: candidate.in_source.then_some(candidate.source_index),
                color: candidate.color,
                pixels: candidate.pixels,
            });
        }
    }
    dropped.sort_by(|left, right| {
        left.source_index
            .cmp(&right.source_index)
            .then(left.color.r.cmp(&right.color.r))
            .then(left.color.g.cmp(&right.color.g))
            .then(left.color.b.cmp(&right.color.b))
    });
    dropped
}

fn source_index_of(candidates: &[Candidate], color: Rgba) -> Option<u32> {
    candidates.iter().find_map(|candidate| {
        if candidate.in_source && rgb_eq(candidate.color, color) {
            Some(candidate.source_index)
        } else {
            None
        }
    })
}

fn note_sample(
    candidates: &mut Vec<Candidate>,
    sampled: &[u8],
    reject_over_255: bool,
) -> Result<(), KernelError> {
    for candidate in candidates.iter_mut() {
        candidate.pixels = 0;
    }
    let mut first: BTreeMap<(u8, u8, u8), usize> = BTreeMap::new();
    for (index, candidate) in candidates.iter().enumerate() {
        first
            .entry((candidate.color.r, candidate.color.g, candidate.color.b))
            .or_insert(index);
    }
    for pixel in sampled.chunks_exact(4) {
        if pixel[3] < INDEX_ALPHA_CUTOFF {
            continue;
        }
        let key = (pixel[0], pixel[1], pixel[2]);
        if let Some(&index) = first.get(&key) {
            candidates[index].pixels = candidates[index].pixels.saturating_add(1);
            continue;
        }
        if reject_over_255 && candidates.len() >= 255 {
            return Err(KernelError::PaletteTooLarge { len: 257 });
        }
        let index = candidates.len();
        first.insert(key, index);
        candidates.push(Candidate {
            source_index: u32::try_from(index).unwrap_or(u32::MAX),
            in_source: false,
            color: Rgba::rgb(key.0, key.1, key.2),
            pixels: 1,
        });
    }
    Ok(())
}

fn histogram(rgba: &[u8]) -> Vec<Candidate> {
    let mut colors: Vec<Candidate> = Vec::new();
    let mut index_of: BTreeMap<(u8, u8, u8), usize> = BTreeMap::new();
    for pixel in rgba.chunks_exact(4) {
        if pixel[3] < INDEX_ALPHA_CUTOFF {
            continue;
        }
        let key = (pixel[0], pixel[1], pixel[2]);
        if let Some(&index) = index_of.get(&key) {
            colors[index].pixels = colors[index].pixels.saturating_add(1);
            continue;
        }
        let index = colors.len();
        index_of.insert(key, index);
        colors.push(Candidate {
            source_index: u32::try_from(index).unwrap_or(u32::MAX),
            in_source: true,
            color: Rgba::rgb(key.0, key.1, key.2),
            pixels: 1,
        });
    }
    colors
}

/// Distinct opaque colors in first-seen order.
///
/// `reject_over_255` makes the 256th color [`KernelError::PaletteTooLarge`]
/// without storing it.
fn unique_colors(rgba: &[u8], reject_over_255: bool) -> Result<Vec<Candidate>, KernelError> {
    let mut colors = Vec::new();
    let mut seen: BTreeMap<(u8, u8, u8), ()> = BTreeMap::new();
    for pixel in rgba.chunks_exact(4) {
        if pixel[3] < INDEX_ALPHA_CUTOFF {
            continue;
        }
        let key = (pixel[0], pixel[1], pixel[2]);
        if seen.contains_key(&key) {
            continue;
        }
        if reject_over_255 && colors.len() == 255 {
            return Err(KernelError::PaletteTooLarge { len: 257 });
        }
        seen.insert(key, ());
        let index = colors.len();
        colors.push(Candidate {
            source_index: u32::try_from(index).unwrap_or(u32::MAX),
            in_source: true,
            color: Rgba::rgb(key.0, key.1, key.2),
            pixels: 0,
        });
    }
    Ok(colors)
}

/// Opaque PLTE entries. `None` when the PNG is not indexed.
///
/// Alpha below [`INDEX_ALPHA_CUTOFF`] is omitted. A missing `tRNS` byte is
/// opaque. File order is kept, so an unused entry stays in place.
fn read_indexed_opaque(bytes: &[u8]) -> Result<Option<Vec<Candidate>>, KernelError> {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let reader = decoder.read_info().map_err(|_| KernelError::PngFile)?;
    let info = reader.info();
    if info.color_type != png::ColorType::Indexed {
        return Ok(None);
    }
    let Some(palette) = info.palette.as_deref() else {
        return Err(KernelError::PngFile);
    };
    if palette.is_empty() || palette.len() % 3 != 0 {
        return Err(KernelError::PngFile);
    }
    let trns = info.trns.as_deref().unwrap_or(&[]);
    let mut colors = Vec::new();
    for (index, rgb) in palette.chunks_exact(3).enumerate() {
        let alpha = trns.get(index).copied().unwrap_or(255);
        if alpha < INDEX_ALPHA_CUTOFF {
            continue;
        }
        colors.push(Candidate {
            source_index: u32::try_from(index).unwrap_or(u32::MAX),
            in_source: true,
            color: Rgba::rgb(rgb[0], rgb[1], rgb[2]),
            pixels: 0,
        });
    }
    Ok(Some(colors))
}

fn opaque_len(colors: &[Rgba]) -> u32 {
    u32::try_from(colors.len().saturating_sub(1)).unwrap_or(u32::MAX)
}

fn rgb_eq(color: Rgba, rgb: Rgba) -> bool {
    color.r == rgb.r && color.g == rgb.g && color.b == rgb.b
}

fn distance_sq(rgb: (u8, u8, u8), color: Rgba) -> u32 {
    let dr = i32::from(rgb.0) - i32::from(color.r);
    let dg = i32::from(rgb.1) - i32::from(color.g);
    let db = i32::from(rgb.2) - i32::from(color.b);
    u32::try_from(dr * dr + dg * dg + db * db).unwrap_or(u32::MAX)
}

fn merge_json(row: &MergeRow) -> Value {
    let mut value = json!({
        "from": { "r": row.from.r, "g": row.from.g, "b": row.from.b },
        "intoIndex": row.into_index,
        "into": { "r": row.into.r, "g": row.into.g, "b": row.into.b },
        "pixels": row.pixels,
        "distance": row.distance,
        "exact": false,
    });
    if let Some(index) = row.source_index {
        value["sourceIndex"] = json!(index);
    }
    value
}

fn dropped_json(row: &DroppedRow) -> Value {
    let mut value = json!({
        "r": row.color.r,
        "g": row.color.g,
        "b": row.color.b,
        "pixels": row.pixels,
    });
    if let Some(index) = row.source_index {
        value["sourceIndex"] = json!(index);
    }
    value
}
