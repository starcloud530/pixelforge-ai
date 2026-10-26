//! Read-only check of a `pixelforge-atlas` file against its sprite sheet.
//!
//! [`validate_atlas`] does not change pixels and does not write a second atlas
//! format. It reads the object `export sheet --atlas` already writes, plus the
//! optional `image` file name the CLI adds and an optional `pivot` when one is
//! already present. A rules file can name the engine origin the atlas itself
//! does not store.
//!
//! Exit policy for the CLI: any finding in `errors` is a failed check. Findings
//! in `warnings` do not fail, in `--strict` or in `--loose`. `--loose` only
//! moves convention misses (canvas tier, four-direction names, an empty tag
//! list, image file name, delay range, pivot drift) into `warnings`. Overlap,
//! out-of-bounds frames, a broken grid, and opaque padding stay errors.

use std::fmt;

use png::{ColorType, Decoder, Transformations};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::{MAX_FRAME_DELAY_MS, MAX_TAG_NAME_CHARS, MIN_FRAME_DELAY_MS};

/// Canvas edges treated as the formal game-sprite tiers.
///
/// The check compares `frameWidth / scale` and `frameHeight / scale`, the
/// unscaled canvas, not the sheet cell after `scale`. Both edges must be in
/// the allowed set, so 64×128 passes and 32×64 does not. An empty allow-list
/// turns the check off.
pub const FORMAL_FRAME_EDGES: [u32; 3] = [64, 96, 128];

const MAX_SHEET_EDGE: u32 = 8192;
const MAX_SHEET_PIXELS: usize = 16_777_216;
const MAX_ATLAS_FRAMES: usize = 65_536;
const MAX_ATLAS_TAGS: usize = 4_096;
const MAX_OVERLAP_REPORTS: usize = 32;
const FACINGS: [&str; 4] = ["down", "left", "right", "up"];
const LOOP_MODES: [&str; 4] = ["forward", "reverse", "pingpong", "once"];

/// How convention misses are reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtlasMode {
    /// Canvas tier, four-direction names, an empty tag list, image file name,
    /// delay range, and pivot drift are errors.
    Strict,
    /// Those same misses are warnings. Structural problems stay errors.
    Loose,
}

impl AtlasMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::Loose => "loose",
        }
    }
}

/// When four-direction tag names are checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FourDirection {
    /// Check when any tag is `{idle|walk}_{down|left|right|up}`.
    #[default]
    Auto,
    /// Always check, even when no tag uses those names.
    On,
    /// Never check.
    Off,
}

/// What an empty tag list means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagPolicy {
    /// No tags is an error in both modes.
    Required,
    /// No tags is not a finding. `requireTags` still applies.
    Optional,
}

/// Engine origin in frame pixels. Top-left is `(0, 0)` and `y` grows downward.
///
/// `x` and `y` may sit on the far edge (`x == width`, `y == height`), which is
/// the bottom or right edge of the cell rather than a pixel center.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pivot {
    pub x: f64,
    pub y: f64,
}

/// Rules for [`validate_atlas`].
///
/// Omitted file fields keep these defaults: strict mode, edges
/// [`FORMAL_FRAME_EDGES`], four-direction `auto`, and an empty tag list that
/// follows the mode. CLI flags replace the fields they set.
#[derive(Debug, Clone, PartialEq)]
pub struct AtlasRules {
    pub mode: AtlasMode,
    /// Allowed unscaled canvas edges. Empty means any size.
    pub sizes: Vec<u32>,
    pub require_tags: Vec<String>,
    pub four_direction: FourDirection,
    pub pivot: Option<Pivot>,
    /// `None` follows the mode: strict errors, loose warns.
    pub tag_policy: Option<TagPolicy>,
}

impl Default for AtlasRules {
    fn default() -> Self {
        Self {
            mode: AtlasMode::Strict,
            sizes: FORMAL_FRAME_EDGES.to_vec(),
            require_tags: Vec::new(),
            four_direction: FourDirection::Auto,
            pivot: None,
            tag_policy: None,
        }
    }
}

impl AtlasRules {
    /// Parse a `pixelforge-atlas-rules` object. Unknown fields are refused.
    pub fn parse(text: &str) -> Result<Self, AtlasInputError> {
        let value: Value =
            serde_json::from_str(text).map_err(|_| input("rules file is not JSON"))?;
        Self::from_value(&value)
    }

    pub fn from_value(value: &Value) -> Result<Self, AtlasInputError> {
        let map = object(value, "rules")?;
        reject_unknown(
            map,
            &[
                "format",
                "version",
                "mode",
                "sizes",
                "requireTags",
                "fourDirection",
                "pivot",
                "tagPolicy",
            ],
            "rules",
        )?;
        if let Some(format) = map.get("format") {
            let format = parse_string(format, "rules format")?;
            if format != "pixelforge-atlas-rules" {
                return Err(input(format!(
                    "rules format must be pixelforge-atlas-rules, got {format}"
                )));
            }
            match map.get("version") {
                Some(version) if parse_u32(version, "rules version")? == 1 => {}
                Some(version) => {
                    return Err(input(format!("rules version must be 1, got {}", version)));
                }
                None => return Err(input("rules version is missing")),
            }
        } else if map.contains_key("version") {
            return Err(input("rules version requires format"));
        }

        let mut rules = Self::default();
        if let Some(mode) = map.get("mode") {
            rules.mode = match parse_string(mode, "rules mode")?.as_str() {
                "strict" => AtlasMode::Strict,
                "loose" => AtlasMode::Loose,
                other => {
                    return Err(input(format!(
                        "rules mode must be strict or loose, got {other}"
                    )))
                }
            };
        }
        if let Some(sizes) = map.get("sizes") {
            let list = sizes
                .as_array()
                .ok_or_else(|| input("rules sizes must be an array"))?;
            let mut edges = Vec::with_capacity(list.len());
            for (index, edge) in list.iter().enumerate() {
                let edge = parse_u32(edge, &format!("rules sizes[{index}]"))?;
                if edge == 0 {
                    return Err(input("rules sizes must be at least 1"));
                }
                edges.push(edge);
            }
            rules.sizes = edges;
        }
        if let Some(tags) = map.get("requireTags") {
            let list = tags
                .as_array()
                .ok_or_else(|| input("rules requireTags must be an array"))?;
            let mut names = Vec::with_capacity(list.len());
            for (index, name) in list.iter().enumerate() {
                let name = parse_string(name, &format!("rules requireTags[{index}]"))?;
                if name.is_empty() {
                    return Err(input("rules requireTags names must be non-empty"));
                }
                names.push(name);
            }
            rules.require_tags = names;
        }
        if let Some(direction) = map.get("fourDirection") {
            rules.four_direction = match parse_string(direction, "rules fourDirection")?.as_str() {
                "auto" => FourDirection::Auto,
                "on" => FourDirection::On,
                "off" => FourDirection::Off,
                other => {
                    return Err(input(format!(
                        "rules fourDirection must be auto, on, or off, got {other}"
                    )))
                }
            };
        }
        if let Some(pivot) = map.get("pivot") {
            rules.pivot = Some(parse_pivot(pivot, "rules pivot")?);
        }
        if let Some(policy) = map.get("tagPolicy") {
            rules.tag_policy = Some(match parse_string(policy, "rules tagPolicy")?.as_str() {
                "required" => TagPolicy::Required,
                "optional" => TagPolicy::Optional,
                other => {
                    return Err(input(format!(
                        "rules tagPolicy must be required or optional, got {other}"
                    )))
                }
            });
        }
        Ok(rules)
    }
}

/// Decoded sheet. Coordinates are PNG pixels, origin top-left.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// The sheet or the atlas could not be read. The checklist did not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtlasInputError {
    message: String,
}

impl AtlasInputError {
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for AtlasInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AtlasInputError {}

/// One checklist row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AtlasFinding {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
}

/// What the checker actually applied, so a later game import can see the gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AtlasChecks {
    pub mode: String,
    pub four_direction: bool,
    /// `checked` when a pivot was in the atlas or the rules, otherwise `skipped`.
    pub pivot: String,
    pub any_size: bool,
    pub sizes: Vec<u32>,
    /// How an empty tag list is treated: `error`, `warning`, or `off`.
    pub empty_tags: String,
}

/// Checklist for one sheet. `errors` fail the import gate. `warnings` do not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AtlasReport {
    pub errors: Vec<AtlasFinding>,
    pub warnings: Vec<AtlasFinding>,
    pub error_count: usize,
    pub warning_count: usize,
    pub checks: AtlasChecks,
}

impl AtlasReport {
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Read an 8-bit PNG of any size the sheet cap allows.
///
/// This is not [`crate`] canvas decode: a sprite sheet is wider than one frame,
/// so the canvas edge of 256 does not apply. Edges above 8192, or more than
/// 16_777_216 pixels, are refused.
pub fn decode_sheet_png(bytes: &[u8]) -> Result<SheetImage, AtlasInputError> {
    let mut decoder = Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(Transformations::EXPAND | Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|_| input("sheet is not a PNG"))?;
    let (width, height) = {
        let info = reader.info();
        (info.width, info.height)
    };
    if width == 0 || height == 0 {
        return Err(input("sheet PNG has a zero edge"));
    }
    if width > MAX_SHEET_EDGE || height > MAX_SHEET_EDGE {
        return Err(input(format!(
            "sheet PNG is {width}x{height}; the edge cap is {MAX_SHEET_EDGE}"
        )));
    }
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| input("sheet PNG is too large"))?;
    if pixels > MAX_SHEET_PIXELS {
        return Err(input(format!(
            "sheet PNG has {pixels} pixels; the cap is {MAX_SHEET_PIXELS}"
        )));
    }
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|_| input("sheet is not a PNG"))?;
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        ColorType::Rgba => buf,
        ColorType::Rgb => expand_rgb(&buf)?,
        ColorType::Grayscale => expand_gray(&buf, false)?,
        ColorType::GrayscaleAlpha => expand_gray(&buf, true)?,
        _ => return Err(input("sheet PNG is not RGBA")),
    };
    if rgba.len() != pixels * 4 {
        return Err(input("sheet PNG size does not match its pixels"));
    }
    Ok(SheetImage {
        width,
        height,
        rgba,
    })
}

/// Check `atlas` against `sheet`.
///
/// `sheet_file_name` is the PNG's file name, compared to atlas `image` when
/// that field is present. Pass `None` to skip that comparison.
///
/// Returns [`AtlasInputError`] when the JSON is not a `pixelforge-atlas`
/// version 1 object. A well-formed atlas with bad rectangles still returns a
/// report.
pub fn validate_atlas(
    atlas: &Value,
    sheet: &SheetImage,
    sheet_file_name: Option<&str>,
    rules: &AtlasRules,
) -> Result<AtlasReport, AtlasInputError> {
    let parsed = parse_atlas(atlas)?;
    let mut drafts = Vec::new();
    check_grid(&parsed, &mut drafts);
    check_frame_geometry(&parsed, &mut drafts);
    check_tags(&parsed, rules, &mut drafts);
    let four_direction = four_direction_active(&parsed, rules);
    if four_direction {
        check_four_direction(&parsed, &mut drafts);
    }
    check_size(&parsed, rules, &mut drafts);
    check_image(&parsed, sheet_file_name, &mut drafts);
    let pivot_checked = pivot_is_present(&parsed, rules);
    check_pixels(&parsed, sheet, rules, pivot_checked, &mut drafts);

    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    for draft in drafts {
        let finding = AtlasFinding {
            code: draft.code.to_string(),
            message: draft.message,
            frame: draft.frame,
            tag: draft.tag,
        };
        match severity(draft.level, rules.mode) {
            Some(Severity::Error) => errors.push(finding),
            Some(Severity::Warning) => warnings.push(finding),
            None => {}
        }
    }
    let error_count = errors.len();
    let warning_count = warnings.len();
    Ok(AtlasReport {
        errors,
        warnings,
        error_count,
        warning_count,
        checks: AtlasChecks {
            mode: rules.mode.as_str().to_string(),
            four_direction,
            pivot: if pivot_checked {
                "checked".to_string()
            } else {
                "skipped".to_string()
            },
            any_size: rules.sizes.is_empty(),
            sizes: rules.sizes.clone(),
            empty_tags: empty_tags_label(rules).to_string(),
        },
    })
}

#[derive(Clone, Copy)]
enum Level {
    Always,
    Convention,
    Warn,
    Off,
}

enum Severity {
    Error,
    Warning,
}

struct Draft {
    level: Level,
    code: &'static str,
    message: String,
    frame: Option<u32>,
    tag: Option<String>,
}

struct ParsedAtlas {
    columns: u32,
    rows: u32,
    scale: u32,
    frame_width: u32,
    frame_height: u32,
    sheet_width: u32,
    sheet_height: u32,
    image: Option<String>,
    root_pivot: Option<Pivot>,
    frames: Vec<ParsedFrame>,
    tags: Vec<ParsedTag>,
}

struct ParsedFrame {
    index: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    delay_ms: u32,
    pivot: Option<Pivot>,
}

struct ParsedTag {
    index: u32,
    name: String,
    from: u32,
    to: u32,
    loop_mode: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FourAction {
    Idle,
    Walk,
}

fn severity(level: Level, mode: AtlasMode) -> Option<Severity> {
    match level {
        Level::Always => Some(Severity::Error),
        Level::Warn => Some(Severity::Warning),
        Level::Off => None,
        Level::Convention => Some(match mode {
            AtlasMode::Strict => Severity::Error,
            AtlasMode::Loose => Severity::Warning,
        }),
    }
}

fn empty_tags_label(rules: &AtlasRules) -> &'static str {
    match rules.tag_policy {
        Some(TagPolicy::Optional) => "off",
        Some(TagPolicy::Required) => "error",
        None => match rules.mode {
            AtlasMode::Strict => "error",
            AtlasMode::Loose => "warning",
        },
    }
}

fn empty_tags_level(rules: &AtlasRules) -> Level {
    match rules.tag_policy {
        Some(TagPolicy::Required) => Level::Always,
        Some(TagPolicy::Optional) => Level::Off,
        None => Level::Convention,
    }
}

fn push(
    drafts: &mut Vec<Draft>,
    level: Level,
    code: &'static str,
    message: impl Into<String>,
    frame: Option<u32>,
    tag: Option<String>,
) {
    drafts.push(Draft {
        level,
        code,
        message: message.into(),
        frame,
        tag,
    });
}

fn parse_atlas(value: &Value) -> Result<ParsedAtlas, AtlasInputError> {
    let map = object(value, "atlas")?;
    let format = parse_string(require(map, "format", "atlas")?, "atlas format")?;
    if format != "pixelforge-atlas" {
        return Err(input(format!(
            "atlas format must be pixelforge-atlas, got {format}"
        )));
    }
    let version = parse_u32(require(map, "version", "atlas")?, "atlas version")?;
    if version != 1 {
        return Err(input(format!("atlas version must be 1, got {version}")));
    }
    let frames_value = require(map, "frames", "atlas")?
        .as_array()
        .ok_or_else(|| input("atlas frames must be an array"))?;
    if frames_value.is_empty() {
        return Err(input("atlas has no frames"));
    }
    if frames_value.len() > MAX_ATLAS_FRAMES {
        return Err(input(format!(
            "atlas has {} frames; the cap is {MAX_ATLAS_FRAMES}",
            frames_value.len()
        )));
    }
    let tags_value = require(map, "tags", "atlas")?
        .as_array()
        .ok_or_else(|| input("atlas tags must be an array"))?;
    if tags_value.len() > MAX_ATLAS_TAGS {
        return Err(input(format!(
            "atlas has {} tags; the cap is {MAX_ATLAS_TAGS}",
            tags_value.len()
        )));
    }
    let image = match map.get("image") {
        None => None,
        Some(image) => {
            let image = parse_string(image, "atlas image")?;
            if image.is_empty() {
                return Err(input("atlas image must be a non-empty file name"));
            }
            Some(image)
        }
    };
    let root_pivot = match map.get("pivot") {
        None => None,
        Some(pivot) => Some(parse_pivot(pivot, "atlas pivot")?),
    };
    let mut frames = Vec::with_capacity(frames_value.len());
    for (index, frame) in frames_value.iter().enumerate() {
        frames.push(parse_frame(frame, index)?);
    }
    let mut tags = Vec::with_capacity(tags_value.len());
    for (index, tag) in tags_value.iter().enumerate() {
        tags.push(parse_tag(tag, index)?);
    }
    Ok(ParsedAtlas {
        columns: parse_positive(require(map, "columns", "atlas")?, "atlas columns")?,
        rows: parse_positive(require(map, "rows", "atlas")?, "atlas rows")?,
        scale: parse_positive(require(map, "scale", "atlas")?, "atlas scale")?,
        frame_width: parse_positive(require(map, "frameWidth", "atlas")?, "atlas frameWidth")?,
        frame_height: parse_positive(require(map, "frameHeight", "atlas")?, "atlas frameHeight")?,
        sheet_width: parse_positive(require(map, "sheetWidth", "atlas")?, "atlas sheetWidth")?,
        sheet_height: parse_positive(require(map, "sheetHeight", "atlas")?, "atlas sheetHeight")?,
        image,
        root_pivot,
        frames,
        tags,
    })
}

fn parse_frame(value: &Value, position: usize) -> Result<ParsedFrame, AtlasInputError> {
    let what = format!("frames[{position}]");
    let map = object(value, &what)?;
    let pivot = match map.get("pivot") {
        None => None,
        Some(pivot) => Some(parse_pivot(pivot, &format!("{what}.pivot"))?),
    };
    Ok(ParsedFrame {
        index: parse_u32(require(map, "index", &what)?, &format!("{what}.index"))?,
        x: parse_u32(require(map, "x", &what)?, &format!("{what}.x"))?,
        y: parse_u32(require(map, "y", &what)?, &format!("{what}.y"))?,
        width: parse_u32(require(map, "width", &what)?, &format!("{what}.width"))?,
        height: parse_u32(require(map, "height", &what)?, &format!("{what}.height"))?,
        delay_ms: parse_u32(require(map, "delayMs", &what)?, &format!("{what}.delayMs"))?,
        pivot,
    })
}

fn parse_tag(value: &Value, position: usize) -> Result<ParsedTag, AtlasInputError> {
    let what = format!("tags[{position}]");
    let map = object(value, &what)?;
    Ok(ParsedTag {
        index: parse_u32(require(map, "index", &what)?, &format!("{what}.index"))?,
        name: parse_string(require(map, "name", &what)?, &format!("{what}.name"))?,
        from: parse_u32(require(map, "from", &what)?, &format!("{what}.from"))?,
        to: parse_u32(require(map, "to", &what)?, &format!("{what}.to"))?,
        loop_mode: parse_string(require(map, "loop", &what)?, &format!("{what}.loop"))?,
    })
}

fn check_grid(atlas: &ParsedAtlas, drafts: &mut Vec<Draft>) {
    match atlas.columns.checked_mul(atlas.frame_width) {
        Some(width) if width == atlas.sheet_width => {}
        Some(width) => push(
            drafts,
            Level::Always,
            "grid",
            format!(
                "sheetWidth is {}; columns {} * frameWidth {} is {width}",
                atlas.sheet_width, atlas.columns, atlas.frame_width
            ),
            None,
            None,
        ),
        None => push(
            drafts,
            Level::Always,
            "grid",
            "columns * frameWidth overflows",
            None,
            None,
        ),
    }
    match atlas.rows.checked_mul(atlas.frame_height) {
        Some(height) if height == atlas.sheet_height => {}
        Some(height) => push(
            drafts,
            Level::Always,
            "grid",
            format!(
                "sheetHeight is {}; rows {} * frameHeight {} is {height}",
                atlas.sheet_height, atlas.rows, atlas.frame_height
            ),
            None,
            None,
        ),
        None => push(
            drafts,
            Level::Always,
            "grid",
            "rows * frameHeight overflows",
            None,
            None,
        ),
    }
    let needed = atlas.frames.len().div_ceil(atlas.columns as usize);
    if (atlas.rows as usize) < needed {
        push(
            drafts,
            Level::Always,
            "grid",
            format!(
                "rows {} cannot hold {} frames in {} columns",
                atlas.rows,
                atlas.frames.len(),
                atlas.columns
            ),
            None,
            None,
        );
    }
}

fn check_frame_geometry(atlas: &ParsedAtlas, drafts: &mut Vec<Draft>) {
    let mut overlap_reports = 0usize;
    let mut overlap_pairs = 0usize;
    for (left_index, left) in atlas.frames.iter().enumerate() {
        if left.index as usize != left_index {
            push(
                drafts,
                Level::Always,
                "frameIndex",
                format!(
                    "frames[{left_index}].index is {}; expected {left_index}",
                    left.index
                ),
                Some(left.index),
                None,
            );
        }
        if !delay_in_range(left.delay_ms) {
            push(
                drafts,
                Level::Convention,
                "delay",
                format!(
                    "frame {} delayMs {} is outside {MIN_FRAME_DELAY_MS}..={MAX_FRAME_DELAY_MS}",
                    left.index, left.delay_ms
                ),
                Some(left.index),
                None,
            );
        }
        if !rect_inside(left, atlas.sheet_width, atlas.sheet_height) {
            push(
                drafts,
                Level::Always,
                "outOfBounds",
                format!(
                    "frame {} rect {},{} {}x{} is outside the {}x{} sheet",
                    left.index,
                    left.x,
                    left.y,
                    left.width,
                    left.height,
                    atlas.sheet_width,
                    atlas.sheet_height
                ),
                Some(left.index),
                None,
            );
        }
        if let Some((x, y, width, height)) = expected_cell(atlas, left.index) {
            if left.x != x || left.y != y || left.width != width || left.height != height {
                push(
                    drafts,
                    Level::Always,
                    "grid",
                    format!(
                        "frame {} rect {},{} {}x{} is not cell {x},{y} {width}x{height}",
                        left.index, left.x, left.y, left.width, left.height
                    ),
                    Some(left.index),
                    None,
                );
            }
        }
        for right in atlas.frames.iter().skip(left_index + 1) {
            if !rects_overlap(left, right) {
                continue;
            }
            overlap_pairs += 1;
            if overlap_reports < MAX_OVERLAP_REPORTS {
                overlap_reports += 1;
                push(
                    drafts,
                    Level::Always,
                    "overlap",
                    format!("frame {} overlaps frame {}", left.index, right.index),
                    Some(left.index),
                    None,
                );
            }
        }
    }
    if overlap_pairs > MAX_OVERLAP_REPORTS {
        push(
            drafts,
            Level::Always,
            "overlap",
            format!(
                "{overlap_pairs} overlapping pairs; the first {MAX_OVERLAP_REPORTS} are listed"
            ),
            None,
            None,
        );
    }
}

fn expected_cell(atlas: &ParsedAtlas, index: u32) -> Option<(u32, u32, u32, u32)> {
    let columns = atlas.columns as usize;
    let index = index as usize;
    let row = index / columns;
    if row >= atlas.rows as usize {
        return None;
    }
    let column = index % columns;
    let x = (column as u32).checked_mul(atlas.frame_width)?;
    let y = (row as u32).checked_mul(atlas.frame_height)?;
    Some((x, y, atlas.frame_width, atlas.frame_height))
}

fn check_tags(atlas: &ParsedAtlas, rules: &AtlasRules, drafts: &mut Vec<Draft>) {
    if atlas.tags.is_empty() && !four_direction_active(atlas, rules) {
        push(
            drafts,
            empty_tags_level(rules),
            "missingTags",
            "atlas has no tags",
            None,
            None,
        );
    }
    let mut seen = Vec::<&str>::new();
    let last = atlas.frames.len() - 1;
    for (position, tag) in atlas.tags.iter().enumerate() {
        if tag.index as usize != position {
            push(
                drafts,
                Level::Always,
                "tagIndex",
                format!(
                    "tags[{position}].index is {}; expected {position}",
                    tag.index
                ),
                None,
                Some(tag.name.clone()),
            );
        }
        if !tag_name_ok(&tag.name) {
            push(
                drafts,
                Level::Always,
                "tagName",
                format!(
                    "tag name `{}` is empty, too long, or has a control character",
                    tag.name
                ),
                None,
                Some(tag.name.clone()),
            );
        } else if seen.iter().any(|name| *name == tag.name) {
            push(
                drafts,
                Level::Always,
                "tagName",
                format!("duplicate tag `{}`", tag.name),
                None,
                Some(tag.name.clone()),
            );
        } else {
            seen.push(tag.name.as_str());
        }
        if !LOOP_MODES.contains(&tag.loop_mode.as_str()) {
            push(
                drafts,
                Level::Always,
                "loop",
                format!(
                    "tag `{}` loop `{}` is not forward, reverse, pingpong, or once",
                    tag.name, tag.loop_mode
                ),
                None,
                Some(tag.name.clone()),
            );
        }
        if tag.from > tag.to || tag.to as usize > last {
            push(
                drafts,
                Level::Always,
                "tagRange",
                format!(
                    "tag `{}` range {}-{} is outside frames 0-{last}",
                    tag.name, tag.from, tag.to
                ),
                None,
                Some(tag.name.clone()),
            );
        }
    }
    for name in &rules.require_tags {
        if !atlas.tags.iter().any(|tag| tag.name == *name) {
            push(
                drafts,
                Level::Always,
                "missingTags",
                format!("missing tag `{name}`"),
                None,
                Some(name.clone()),
            );
        }
    }
}

fn check_four_direction(atlas: &ParsedAtlas, drafts: &mut Vec<Draft>) {
    for tag in &atlas.tags {
        if parse_four_dir_name(&tag.name).is_none() {
            push(
                drafts,
                Level::Convention,
                "naming",
                format!(
                    "tag `{}` is not {{idle|walk}}_{{down|left|right|up}}",
                    tag.name
                ),
                None,
                Some(tag.name.clone()),
            );
        }
    }
    if atlas.rows > 4 {
        push(
            drafts,
            Level::Convention,
            "naming",
            format!(
                "four-direction sheet has {} rows; facings use down, left, right, up",
                atlas.rows
            ),
            None,
            None,
        );
    }
    let frame_count = atlas.frames.len() as u32;
    for (facing_index, facing) in FACINGS.iter().enumerate() {
        let row = facing_index as u32;
        let idles = tags_for(atlas, FourAction::Idle, facing_index);
        let walks = tags_for(atlas, FourAction::Walk, facing_index);
        let present = !idles.is_empty() || !walks.is_empty();
        if row >= atlas.rows {
            if present {
                push(
                    drafts,
                    Level::Convention,
                    "naming",
                    format!("facing {facing} does not fit in {} rows", atlas.rows),
                    None,
                    None,
                );
            }
            continue;
        }
        let Some(row_start) = row.checked_mul(atlas.columns) else {
            push(
                drafts,
                Level::Always,
                "grid",
                format!("facing {facing} row start overflows"),
                None,
                None,
            );
            continue;
        };
        if row_start >= frame_count {
            if present {
                push(
                    drafts,
                    Level::Convention,
                    "naming",
                    format!("facing {facing} is past the last frame"),
                    None,
                    None,
                );
            }
            continue;
        }
        let row_len = atlas.columns.min(frame_count - row_start);
        let row_end = row_start + row_len - 1;
        if row_len != atlas.columns {
            push(
                drafts,
                Level::Convention,
                "naming",
                format!(
                    "{facing} row has {row_len} frames; columns is {}",
                    atlas.columns
                ),
                None,
                None,
            );
        }
        if idles.is_empty() && walks.is_empty() {
            push(
                drafts,
                Level::Convention,
                "missingTags",
                format!(
                    "facing {facing} has frames {row_start}-{row_end} and no idle_{facing} or walk_{facing} tag"
                ),
                None,
                None,
            );
            continue;
        }
        if let Some(idle) = idles.first() {
            if idle.from != row_start || idle.to != row_start {
                push(
                    drafts,
                    Level::Convention,
                    "naming",
                    format!("idle_{facing} must cover only frame {row_start}"),
                    None,
                    Some((*idle).name.clone()),
                );
            }
        }
        if idles.is_empty() {
            if let Some(walk) = walks.first() {
                if walk.from != row_start || walk.to != row_end {
                    push(
                        drafts,
                        Level::Convention,
                        "naming",
                        format!("walk_{facing} must cover frames {row_start}-{row_end}"),
                        None,
                        Some((*walk).name.clone()),
                    );
                }
            }
        } else if row_start >= row_end {
            if let Some(walk) = walks.first() {
                push(
                    drafts,
                    Level::Convention,
                    "naming",
                    format!("walk_{facing} has no frames after idle on a one-frame row"),
                    None,
                    Some((*walk).name.clone()),
                );
            }
        } else if let Some(walk) = walks.first() {
            let expect_from = row_start + 1;
            if walk.from != expect_from || walk.to != row_end {
                push(
                    drafts,
                    Level::Convention,
                    "naming",
                    format!("walk_{facing} must cover frames {expect_from}-{row_end}"),
                    None,
                    Some((*walk).name.clone()),
                );
            }
        } else {
            push(
                drafts,
                Level::Convention,
                "missingTags",
                format!(
                    "facing {facing} has no walk_{facing} for frames {}-{row_end}",
                    row_start + 1
                ),
                None,
                None,
            );
        }
    }
}

fn tags_for(atlas: &ParsedAtlas, action: FourAction, facing: usize) -> Vec<&ParsedTag> {
    atlas
        .tags
        .iter()
        .filter(|tag| parse_four_dir_name(&tag.name) == Some((action, facing)))
        .collect()
}

fn parse_four_dir_name(name: &str) -> Option<(FourAction, usize)> {
    let (action, facing) = name.split_once('_')?;
    if facing.contains('_') {
        return None;
    }
    let action = match action {
        "idle" => FourAction::Idle,
        "walk" => FourAction::Walk,
        _ => return None,
    };
    let facing = FACINGS.iter().position(|name| *name == facing)?;
    Some((action, facing))
}

fn four_direction_active(atlas: &ParsedAtlas, rules: &AtlasRules) -> bool {
    match rules.four_direction {
        FourDirection::Off => false,
        FourDirection::On => true,
        FourDirection::Auto => atlas
            .tags
            .iter()
            .any(|tag| parse_four_dir_name(&tag.name).is_some()),
    }
}

fn check_size(atlas: &ParsedAtlas, rules: &AtlasRules, drafts: &mut Vec<Draft>) {
    let width_ok = atlas.frame_width % atlas.scale == 0;
    let height_ok = atlas.frame_height % atlas.scale == 0;
    if !width_ok || !height_ok {
        push(
            drafts,
            Level::Always,
            "scale",
            format!(
                "frame size {}x{} is not divisible by scale {}",
                atlas.frame_width, atlas.frame_height, atlas.scale
            ),
            None,
            None,
        );
        return;
    }
    if rules.sizes.is_empty() {
        return;
    }
    let canvas_width = atlas.frame_width / atlas.scale;
    let canvas_height = atlas.frame_height / atlas.scale;
    if rules.sizes.contains(&canvas_width) && rules.sizes.contains(&canvas_height) {
        return;
    }
    let allowed = rules
        .sizes
        .iter()
        .map(|edge| edge.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    push(
        drafts,
        Level::Convention,
        "size",
        format!(
            "canvas is {canvas_width}x{canvas_height} (sheet cell {}x{} at scale {}); allowed edges are {allowed}",
            atlas.frame_width, atlas.frame_height, atlas.scale
        ),
        None,
        None,
    );
}

fn check_image(atlas: &ParsedAtlas, sheet_file_name: Option<&str>, drafts: &mut Vec<Draft>) {
    let (Some(image), Some(file_name)) = (&atlas.image, sheet_file_name) else {
        return;
    };
    if image != file_name {
        push(
            drafts,
            Level::Convention,
            "image",
            format!("atlas image is `{image}`; sheet file name is `{file_name}`"),
            None,
            None,
        );
    }
}

fn pivot_is_present(atlas: &ParsedAtlas, rules: &AtlasRules) -> bool {
    rules.pivot.is_some()
        || atlas.root_pivot.is_some()
        || atlas.frames.iter().any(|frame| frame.pivot.is_some())
}

fn check_pixels(
    atlas: &ParsedAtlas,
    sheet: &SheetImage,
    rules: &AtlasRules,
    pivot_checked: bool,
    drafts: &mut Vec<Draft>,
) {
    if sheet.width != atlas.sheet_width || sheet.height != atlas.sheet_height {
        push(
            drafts,
            Level::Always,
            "sheetSize",
            format!(
                "sheet PNG is {}x{}; atlas sheet is {}x{}",
                sheet.width, sheet.height, atlas.sheet_width, atlas.sheet_height
            ),
            None,
            None,
        );
        return;
    }
    let pixels = (sheet.width as usize).saturating_mul(sheet.height as usize);
    if sheet.rgba.len() != pixels.saturating_mul(4) {
        push(
            drafts,
            Level::Always,
            "sheetSize",
            "sheet PNG size does not match its pixels",
            None,
            None,
        );
        return;
    }
    let mut covered = vec![false; pixels];
    for frame in &atlas.frames {
        if !rect_inside(frame, sheet.width, sheet.height) {
            continue;
        }
        for y in frame.y..frame.y + frame.height {
            let row = (y as usize) * (sheet.width as usize);
            for x in frame.x..frame.x + frame.width {
                covered[row + x as usize] = true;
            }
        }
    }
    let mut opaque_padding = 0u32;
    let mut first_padding = None;
    for (index, pixel) in covered.iter().enumerate() {
        if *pixel {
            continue;
        }
        if sheet.rgba[index * 4 + 3] == 0 {
            continue;
        }
        opaque_padding += 1;
        if first_padding.is_none() {
            let x = (index % sheet.width as usize) as u32;
            let y = (index / sheet.width as usize) as u32;
            first_padding = Some((x, y));
        }
    }
    if let Some((x, y)) = first_padding {
        push(
            drafts,
            Level::Always,
            "padding",
            format!(
                "padding pixel {x},{y} is not transparent ({opaque_padding} opaque padding pixels)"
            ),
            None,
            None,
        );
    }
    if !pivot_checked {
        warn_empty_frames(atlas, sheet, drafts);
        return;
    }
    check_pivot_consistency(atlas, sheet, rules, drafts);
}

fn warn_empty_frames(atlas: &ParsedAtlas, sheet: &SheetImage, drafts: &mut Vec<Draft>) {
    let empty = empty_frame_indexes(atlas, sheet);
    if empty.is_empty() {
        return;
    }
    push(
        drafts,
        Level::Warn,
        "emptyFrame",
        format!(
            "{} frames are fully transparent, starting at frame {}",
            empty.len(),
            empty[0]
        ),
        Some(empty[0]),
        None,
    );
}

fn check_pivot_consistency(
    atlas: &ParsedAtlas,
    sheet: &SheetImage,
    rules: &AtlasRules,
    drafts: &mut Vec<Draft>,
) {
    let mut first_pivot: Option<(u32, Pivot)> = None;
    let mut bottoms: Vec<(u32, f64)> = Vec::new();
    let mut empty = Vec::new();
    for frame in &atlas.frames {
        let atlas_pivot = frame.pivot.or(atlas.root_pivot);
        if let (Some(rules_pivot), Some(atlas_pivot)) = (rules.pivot, atlas_pivot) {
            if !pivots_near(rules_pivot, atlas_pivot) {
                push(
                    drafts,
                    Level::Convention,
                    "pivot",
                    format!(
                        "frame {} pivot {},{} does not match rules pivot {},{}",
                        frame.index, atlas_pivot.x, atlas_pivot.y, rules_pivot.x, rules_pivot.y
                    ),
                    Some(frame.index),
                    None,
                );
            }
        }
        let Some(pivot) = rules.pivot.or(atlas_pivot) else {
            continue;
        };
        if !pivot_inside(pivot, frame.width, frame.height) {
            push(
                drafts,
                Level::Always,
                "pivot",
                format!(
                    "frame {} pivot {},{} is outside the {}x{} cell",
                    frame.index, pivot.x, pivot.y, frame.width, frame.height
                ),
                Some(frame.index),
                None,
            );
        }
        if let Some((first_index, first)) = first_pivot {
            if !pivots_near(first, pivot) {
                push(
                    drafts,
                    Level::Convention,
                    "pivot",
                    format!(
                        "frame {} pivot {},{} differs from frame {first_index} pivot {},{}",
                        frame.index, pivot.x, pivot.y, first.x, first.y
                    ),
                    Some(frame.index),
                    None,
                );
            }
        } else {
            first_pivot = Some((frame.index, pivot));
        }
        if !rect_inside(frame, sheet.width, sheet.height) {
            continue;
        }
        match opaque_bottom(frame, sheet) {
            None => empty.push(frame.index),
            Some(bottom) => bottoms.push((frame.index, bottom as f64 - pivot.y)),
        }
    }
    if !empty.is_empty() {
        push(
            drafts,
            Level::Warn,
            "emptyFrame",
            format!(
                "{} frames are fully transparent, starting at frame {}",
                empty.len(),
                empty[0]
            ),
            Some(empty[0]),
            None,
        );
    }
    if let Some((&(left_index, left), &(right_index, right))) = bottoms.first().zip(
        bottoms
            .iter()
            .find(|(_, offset)| !near(*offset, bottoms[0].1)),
    ) {
        push(
            drafts,
            Level::Convention,
            "pivot",
            format!(
                "opaque bottom offset from the pivot differs (frame {left_index} offset {left}, frame {right_index} offset {right})"
            ),
            Some(left_index),
            None,
        );
    }
}

fn empty_frame_indexes(atlas: &ParsedAtlas, sheet: &SheetImage) -> Vec<u32> {
    atlas
        .frames
        .iter()
        .filter(|frame| {
            rect_inside(frame, sheet.width, sheet.height) && opaque_bottom(frame, sheet).is_none()
        })
        .map(|frame| frame.index)
        .collect()
}

fn opaque_bottom(frame: &ParsedFrame, sheet: &SheetImage) -> Option<u32> {
    let mut bottom = None;
    for local_y in 0..frame.height {
        let y = frame.y + local_y;
        let row = (y as usize) * (sheet.width as usize);
        for local_x in 0..frame.width {
            let x = frame.x + local_x;
            let alpha = sheet.rgba[(row + x as usize) * 4 + 3];
            if alpha != 0 {
                bottom = Some(local_y);
            }
        }
    }
    bottom
}

fn rect_inside(frame: &ParsedFrame, sheet_width: u32, sheet_height: u32) -> bool {
    frame.width > 0
        && frame.height > 0
        && frame
            .x
            .checked_add(frame.width)
            .is_some_and(|right| right <= sheet_width)
        && frame
            .y
            .checked_add(frame.height)
            .is_some_and(|bottom| bottom <= sheet_height)
}

fn rects_overlap(left: &ParsedFrame, right: &ParsedFrame) -> bool {
    if left.width == 0 || left.height == 0 || right.width == 0 || right.height == 0 {
        return false;
    }
    let Some(left_right) = left.x.checked_add(left.width) else {
        return false;
    };
    let Some(right_right) = right.x.checked_add(right.width) else {
        return false;
    };
    let Some(left_bottom) = left.y.checked_add(left.height) else {
        return false;
    };
    let Some(right_bottom) = right.y.checked_add(right.height) else {
        return false;
    };
    left.x < right_right && right.x < left_right && left.y < right_bottom && right.y < left_bottom
}

fn delay_in_range(delay_ms: u32) -> bool {
    (MIN_FRAME_DELAY_MS..=MAX_FRAME_DELAY_MS).contains(&delay_ms)
}

fn tag_name_ok(name: &str) -> bool {
    let trimmed = name.trim();
    !trimmed.is_empty()
        && trimmed.chars().count() <= MAX_TAG_NAME_CHARS
        && !name.chars().any(|ch| ch.is_control())
        && name == trimmed
}

fn pivot_inside(pivot: Pivot, width: u32, height: u32) -> bool {
    pivot.x >= 0.0 && pivot.y >= 0.0 && pivot.x <= f64::from(width) && pivot.y <= f64::from(height)
}

fn pivots_near(left: Pivot, right: Pivot) -> bool {
    near(left.x, right.x) && near(left.y, right.y)
}

fn near(left: f64, right: f64) -> bool {
    (left - right).abs() < 0.001
}

fn input(message: impl Into<String>) -> AtlasInputError {
    AtlasInputError {
        message: message.into(),
    }
}

fn object<'a>(value: &'a Value, what: &str) -> Result<&'a Map<String, Value>, AtlasInputError> {
    value
        .as_object()
        .ok_or_else(|| input(format!("{what} must be a JSON object")))
}

fn require<'a>(
    map: &'a Map<String, Value>,
    key: &str,
    what: &str,
) -> Result<&'a Value, AtlasInputError> {
    map.get(key)
        .ok_or_else(|| input(format!("{what} is missing {key}")))
}

fn reject_unknown(
    map: &Map<String, Value>,
    allowed: &[&str],
    what: &str,
) -> Result<(), AtlasInputError> {
    if let Some(key) = map.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(input(format!("{what}: unknown field {key}")));
    }
    Ok(())
}

fn parse_u32(value: &Value, what: &str) -> Result<u32, AtlasInputError> {
    let Some(number) = value.as_u64() else {
        return Err(input(format!(
            "{what} must be an integer from 0 to 4294967295"
        )));
    };
    u32::try_from(number)
        .map_err(|_| input(format!("{what} must be an integer from 0 to 4294967295")))
}

fn parse_positive(value: &Value, what: &str) -> Result<u32, AtlasInputError> {
    let number = parse_u32(value, what)?;
    if number == 0 {
        return Err(input(format!("{what} must be at least 1")));
    }
    Ok(number)
}

fn parse_string(value: &Value, what: &str) -> Result<String, AtlasInputError> {
    value
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| input(format!("{what} must be a string")))
}

fn parse_f64(value: &Value, what: &str) -> Result<f64, AtlasInputError> {
    let number = value
        .as_f64()
        .ok_or_else(|| input(format!("{what} must be a number")))?;
    if !number.is_finite() {
        return Err(input(format!("{what} must be a finite number")));
    }
    Ok(number)
}

fn parse_pivot(value: &Value, what: &str) -> Result<Pivot, AtlasInputError> {
    let map = object(value, what)?;
    reject_unknown(map, &["x", "y"], what)?;
    Ok(Pivot {
        x: parse_f64(require(map, "x", what)?, &format!("{what}.x"))?,
        y: parse_f64(require(map, "y", what)?, &format!("{what}.y"))?,
    })
}

fn expand_rgb(bytes: &[u8]) -> Result<Vec<u8>, AtlasInputError> {
    if bytes.len() % 3 != 0 {
        return Err(input("sheet PNG size does not match its pixels"));
    }
    let mut rgba = Vec::with_capacity(bytes.len() / 3 * 4);
    for pixel in bytes.chunks_exact(3) {
        rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
    }
    Ok(rgba)
}

fn expand_gray(bytes: &[u8], alpha: bool) -> Result<Vec<u8>, AtlasInputError> {
    let width = if alpha { 2 } else { 1 };
    if bytes.len() % width != 0 {
        return Err(input("sheet PNG size does not match its pixels"));
    }
    let mut rgba = Vec::with_capacity(bytes.len() / width * 4);
    for pixel in bytes.chunks_exact(width) {
        let (gray, alpha) = if alpha {
            (pixel[0], pixel[1])
        } else {
            (pixel[0], 255)
        };
        rgba.extend_from_slice(&[gray, gray, gray, alpha]);
    }
    Ok(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Document, LoopMode};
    use serde_json::json;

    fn painted(width: u16, height: u16) -> Document {
        let mut document = Document::new(width, height).unwrap();
        document.paint(1, height - 1, 1).unwrap();
        document
    }

    fn exported(document: &Document, scale: u32, columns: Option<u32>) -> (Value, SheetImage) {
        let png = document
            .export_sprite_sheet_scaled_with(scale, columns)
            .unwrap();
        let atlas = document.sprite_sheet_atlas(scale, columns).unwrap();
        let mut value = serde_json::to_value(&atlas).unwrap();
        value["image"] = json!("hero-sheet.png");
        let sheet = decode_sheet_png(&png).unwrap();
        (value, sheet)
    }

    fn codes(findings: &[AtlasFinding]) -> Vec<&str> {
        findings
            .iter()
            .map(|finding| finding.code.as_str())
            .collect()
    }

    #[test]
    fn formal_export_passes_strict_with_no_warnings() {
        let mut document = painted(64, 64);
        document.add_tag("prop", 0, 0, LoopMode::Forward).unwrap();
        let (atlas, sheet) = exported(&document, 1, None);
        let report = validate_atlas(
            &atlas,
            &sheet,
            Some("hero-sheet.png"),
            &AtlasRules::default(),
        )
        .unwrap();
        assert!(report.ok(), "{:?}", report.errors);
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        assert_eq!(report.checks.pivot, "skipped");
        assert!(!report.checks.four_direction);
        assert_eq!(report.checks.sizes, vec![64, 96, 128]);
    }

    #[test]
    fn scaled_formal_canvas_uses_unscaled_edges() {
        let mut document = painted(64, 128);
        document.add_tag("prop", 0, 0, LoopMode::Forward).unwrap();
        let (atlas, sheet) = exported(&document, 2, Some(1));
        let report = validate_atlas(
            &atlas,
            &sheet,
            Some("hero-sheet.png"),
            &AtlasRules::default(),
        )
        .unwrap();
        assert!(report.ok(), "{:?}", report.errors);
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        assert_eq!(atlas["frameWidth"], 128);
        assert_eq!(atlas["frameHeight"], 256);
        assert_eq!(atlas["scale"], 2);
    }

    #[test]
    fn size_is_a_warning_in_loose_and_an_error_in_strict() {
        let mut document = painted(32, 32);
        document.add_tag("prop", 0, 0, LoopMode::Forward).unwrap();
        let (atlas, sheet) = exported(&document, 2, None);
        let strict = validate_atlas(
            &atlas,
            &sheet,
            Some("hero-sheet.png"),
            &AtlasRules::default(),
        )
        .unwrap();
        assert!(!strict.ok());
        assert_eq!(codes(&strict.errors), vec!["size"]);
        assert!(strict.errors[0].message.contains("32x32"));

        let mut loose = AtlasRules::default();
        loose.mode = AtlasMode::Loose;
        let report = validate_atlas(&atlas, &sheet, Some("hero-sheet.png"), &loose).unwrap();
        assert!(report.ok(), "{:?}", report.errors);
        assert_eq!(codes(&report.warnings), vec!["size"]);
        assert_eq!(report.checks.empty_tags, "warning");
    }

    #[test]
    fn overlap_and_out_of_bounds_are_errors() {
        let mut document = painted(64, 64);
        document.add_frame();
        document.set_active_frame(1).unwrap();
        document.paint(1, 63, 1).unwrap();
        document.add_tag("prop", 0, 1, LoopMode::Forward).unwrap();
        let (mut atlas, sheet) = exported(&document, 1, Some(2));

        atlas["frames"][1]["x"] = json!(0);
        let overlap = validate_atlas(
            &atlas,
            &sheet,
            Some("hero-sheet.png"),
            &AtlasRules::default(),
        )
        .unwrap();
        assert!(codes(&overlap.errors).contains(&"overlap"));
        assert!(!overlap.ok());

        atlas["frames"][1]["x"] = json!(128);
        let outside = validate_atlas(
            &atlas,
            &sheet,
            Some("hero-sheet.png"),
            &AtlasRules::default(),
        )
        .unwrap();
        assert!(codes(&outside.errors).contains(&"outOfBounds"));
        assert!(!outside.ok());
    }

    #[test]
    fn opaque_padding_is_an_error() {
        let mut document = painted(64, 64);
        document.add_frame();
        document.set_active_frame(1).unwrap();
        document.paint(1, 63, 1).unwrap();
        document.add_tag("prop", 0, 1, LoopMode::Forward).unwrap();
        let (atlas, mut sheet) = exported(&document, 1, Some(3));
        assert_eq!((sheet.width, sheet.height), (192, 64));
        let padding = ((128usize) * 4) + 3;
        assert_eq!(sheet.rgba[padding], 0);
        sheet.rgba[padding] = 255;
        let report = validate_atlas(
            &atlas,
            &sheet,
            Some("hero-sheet.png"),
            &AtlasRules::default(),
        )
        .unwrap();
        assert_eq!(codes(&report.errors), vec!["padding"]);
        assert!(report.errors[0].message.contains("128,0"));
    }

    #[test]
    fn four_direction_row_passes_and_a_bad_name_follows_the_mode() {
        let mut document = Document::new(64, 64).unwrap();
        for index in 0..4 {
            if index > 0 {
                document.add_frame();
                document.set_active_frame(index).unwrap();
            }
            document.paint(1, 63, 1).unwrap();
        }
        document
            .add_tag("idle_down", 0, 0, LoopMode::Forward)
            .unwrap();
        document
            .add_tag("walk_down", 1, 3, LoopMode::Forward)
            .unwrap();
        let (atlas, sheet) = exported(&document, 1, Some(4));
        let report = validate_atlas(
            &atlas,
            &sheet,
            Some("hero-sheet.png"),
            &AtlasRules::default(),
        )
        .unwrap();
        assert!(report.ok(), "{:?}", report.errors);
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        assert!(report.checks.four_direction);

        let mut walk_only = Document::new(64, 64).unwrap();
        walk_only.paint(1, 63, 1).unwrap();
        walk_only.add_frame();
        walk_only.set_active_frame(1).unwrap();
        walk_only.paint(2, 63, 1).unwrap();
        walk_only
            .add_tag("walk_down", 0, 1, LoopMode::Forward)
            .unwrap();
        let (walk_atlas, walk_sheet) = exported(&walk_only, 1, Some(2));
        let walk_report = validate_atlas(
            &walk_atlas,
            &walk_sheet,
            Some("hero-sheet.png"),
            &AtlasRules::default(),
        )
        .unwrap();
        assert!(walk_report.ok(), "{:?}", walk_report.errors);
        assert!(
            walk_report.warnings.is_empty(),
            "{:?}",
            walk_report.warnings
        );

        let mut renamed = atlas;
        renamed["tags"][1]["name"] = json!("walk_south");
        let strict = validate_atlas(
            &renamed,
            &sheet,
            Some("hero-sheet.png"),
            &AtlasRules::default(),
        )
        .unwrap();
        assert!(
            codes(&strict.errors).contains(&"naming"),
            "{:?}",
            strict.errors
        );
        let mut loose = AtlasRules::default();
        loose.mode = AtlasMode::Loose;
        let warning = validate_atlas(&renamed, &sheet, Some("hero-sheet.png"), &loose).unwrap();
        assert!(warning.ok(), "{:?}", warning.errors);
        assert!(codes(&warning.warnings).contains(&"naming"));
    }

    #[test]
    fn pivot_drift_is_an_error_when_the_rules_name_an_origin() {
        let mut document = Document::new(64, 64).unwrap();
        document.paint(32, 63, 1).unwrap();
        document.add_frame();
        document.set_active_frame(1).unwrap();
        document.paint(32, 50, 1).unwrap();
        document
            .add_tag("idle_down", 0, 0, LoopMode::Forward)
            .unwrap();
        document.add_tag("walk_down", 1, 1, LoopMode::Once).unwrap();
        let (mut atlas, sheet) = exported(&document, 1, Some(2));
        atlas["pivot"] = json!({"x": 32, "y": 64});
        let report = validate_atlas(
            &atlas,
            &sheet,
            Some("hero-sheet.png"),
            &AtlasRules::default(),
        )
        .unwrap();
        assert_eq!(report.checks.pivot, "checked");
        assert!(
            codes(&report.errors).contains(&"pivot"),
            "{:?}",
            report.errors
        );

        let mut rules = AtlasRules::default();
        rules.pivot = Some(Pivot { x: 32.0, y: 64.0 });
        rules.mode = AtlasMode::Loose;
        atlas.as_object_mut().unwrap().remove("pivot");
        let loose = validate_atlas(&atlas, &sheet, Some("hero-sheet.png"), &rules).unwrap();
        assert!(loose.ok(), "{:?}", loose.errors);
        assert!(codes(&loose.warnings).contains(&"pivot"));
    }

    #[test]
    fn empty_tags_follow_mode_policy_and_require_tags() {
        let document = painted(64, 64);
        let (atlas, sheet) = exported(&document, 1, None);
        let strict = validate_atlas(
            &atlas,
            &sheet,
            Some("hero-sheet.png"),
            &AtlasRules::default(),
        )
        .unwrap();
        assert_eq!(codes(&strict.errors), vec!["missingTags"]);

        let mut optional = AtlasRules::default();
        optional.tag_policy = Some(TagPolicy::Optional);
        let report = validate_atlas(&atlas, &sheet, Some("hero-sheet.png"), &optional).unwrap();
        assert!(report.ok(), "{:?}", report.errors);
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        assert_eq!(report.checks.empty_tags, "off");

        let mut loose = AtlasRules::default();
        loose.mode = AtlasMode::Loose;
        loose.require_tags = vec!["hit".to_string()];
        let report = validate_atlas(&atlas, &sheet, Some("hero-sheet.png"), &loose).unwrap();
        assert!(codes(&report.errors).contains(&"missingTags"));
        assert!(codes(&report.warnings).contains(&"missingTags"));
    }

    #[test]
    fn rules_file_rejects_unknown_fields_and_scale_must_divide() {
        let error = AtlasRules::parse(
            r#"{"format":"pixelforge-atlas-rules","version":1,"sizes":[64],"extra":true}"#,
        )
        .unwrap_err();
        assert!(error.message().contains("unknown field extra"));
        let rules = AtlasRules::parse(
            r#"{"format":"pixelforge-atlas-rules","version":1,"mode":"loose","sizes":[],"tagPolicy":"optional","fourDirection":"off"}"#,
        )
        .unwrap();
        assert_eq!(rules.mode, AtlasMode::Loose);
        assert!(rules.sizes.is_empty());
        assert_eq!(rules.tag_policy, Some(TagPolicy::Optional));

        let mut document = painted(64, 64);
        document.add_tag("prop", 0, 0, LoopMode::Forward).unwrap();
        let (mut atlas, sheet) = exported(&document, 1, None);
        atlas["scale"] = json!(3);
        let report = validate_atlas(&atlas, &sheet, Some("hero-sheet.png"), &rules).unwrap();
        assert_eq!(codes(&report.errors), vec!["scale"]);
    }

    #[test]
    fn image_mismatch_follows_the_mode_and_a_bad_format_does_not_run() {
        let mut document = painted(64, 64);
        document.add_tag("prop", 0, 0, LoopMode::Forward).unwrap();
        let (atlas, sheet) = exported(&document, 1, None);
        let strict =
            validate_atlas(&atlas, &sheet, Some("other.png"), &AtlasRules::default()).unwrap();
        assert_eq!(codes(&strict.errors), vec!["image"]);
        let mut loose = AtlasRules::default();
        loose.mode = AtlasMode::Loose;
        let warning = validate_atlas(&atlas, &sheet, Some("other.png"), &loose).unwrap();
        assert!(warning.ok());
        assert_eq!(codes(&warning.warnings), vec!["image"]);

        let error = validate_atlas(&json!({"format":"other","version":1}), &sheet, None, &loose)
            .unwrap_err();
        assert!(error.message().contains("pixelforge-atlas"));
    }
}
