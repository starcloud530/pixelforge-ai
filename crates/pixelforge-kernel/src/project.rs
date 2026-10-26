//! Version 1 `pixelforge-project` JSON.
//!
//! The kernel owns this format. The shell and, later, the Tool API call
//! [`Document::save_project`] and [`Document::load_project`] instead of
//! writing a second encoder. The file stores canvas size, the palette, the
//! palette lock, every frame's indices and delay, the active frame, and, when
//! the document is more than the original single layer, the layer stack, and,
//! when any are set, named animation tags, and, when any are set, named parts
//! with anchors, and, when any are set, part-parameter curves, and, when any
//! are set, follow, attach, and pin constraints.
//! The undo stack and the playback clock are not part of the file, so a loaded
//! document undoes back to the opened pixels and is not playing. A frame
//! written before delays existed omits `delayMs` and loads as
//! [`crate::DEFAULT_FRAME_DELAY_MS`]. A file written before the palette lock
//! existed omits `paletteLocked` and loads unlocked. A missing `indexPolicy`
//! loads as `reject`; only `clamp` is written back. A file written before
//! layers existed omits `layers` and `activeLayer` and loads as one layer
//! named [`crate::DEFAULT_LAYER_NAME`]. A single default layer is still saved
//! in that older shape. A file written before animation tags existed omits
//! `tags` and loads with none. An empty tag list is still saved that way. A file
//! written before parts existed omits `parts` and loads with none. An empty
//! part list is still saved that way. A file written before part curves existed
//! omits `curves` and loads with none. An empty curve list is still saved that
//! way. A file written before constraints existed omits `constraints` and loads
//! with none. An empty constraint list is still saved that way. `ik` and
//! `plant` are kinds on that same list. A missing
//! `selection` loads as no mask. One rectangle is written as
//! `{x,y,width,height}`. Several rectangles, or a mask that covers nothing,
//! are written as `{rects:[...]}`. The field is omitted when the selection is
//! clear. The undo stack is still omitted.

use serde::{Deserialize, Serialize};

use crate::constraint::{solve_order, Constraint, ConstraintKind, Contact};
use crate::curve::{Curve, CurveChannel, CurveKey, Ease};
use crate::document::Document;
use crate::error::{KernelError, ProjectFileError};
use crate::frame::Frame;
use crate::palette::{IndexPolicy, Palette};
use crate::part::{anchor_offset_i64, normalize_part_name, Anchor, Part};
use crate::selection::{Mask, Selection};
use crate::tag::{normalize_tag_name, LoopMode, Tag};
use crate::Rgba;
use crate::DEFAULT_FRAME_DELAY_MS;
use crate::DEFAULT_LAYER_NAME;
use crate::MAX_CANVAS_EDGE;
use crate::MAX_FRAME_DELAY_MS;
use crate::MAX_LAYER_NAME_CHARS;
use crate::MIN_FRAME_DELAY_MS;

/// `format` field written by [`Document::save_project`].
pub const PROJECT_FORMAT: &str = "pixelforge-project";

/// Only version [`Document::load_project`] accepts.
///
/// Per-frame `delayMs` was added without a version bump. Files that omit it
/// still load: each missing delay becomes [`crate::DEFAULT_FRAME_DELAY_MS`].
pub const PROJECT_VERSION: u32 = 1;

fn default_delay_ms() -> u64 {
    u64::from(DEFAULT_FRAME_DELAY_MS)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectFile {
    format: String,
    version: u64,
    width: u64,
    height: u64,
    active_frame: u64,
    /// Missing, or 0, in a file that still has one default layer.
    #[serde(default, skip_serializing_if = "is_zero")]
    active_layer: u64,
    /// Missing when the file is still the single default layer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    layers: Vec<LayerNameFile>,
    palette: Vec<ColorFile>,
    /// Missing in files saved before the palette could be locked.
    #[serde(default)]
    palette_locked: bool,
    /// Missing, or `reject`, unless paint should clamp out-of-range indices.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    index_policy: Option<String>,
    /// Missing when every cell is writable. One rectangle, or `rects`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    selection: Option<SelectionFile>,
    frames: Vec<FrameFile>,
    /// Named inclusive frame ranges. Missing when the file has no tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tags: Vec<TagFile>,
    /// Named parts. Missing when the file has none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    parts: Vec<PartFile>,
    /// Part-parameter curves. Missing when the file has none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    curves: Vec<CurveFile>,
    /// Follow, attach, and pin constraints. Missing when the file has none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    constraints: Vec<ConstraintFile>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum SelectionFile {
    Rect(SelectionRectFile),
    Rects(SelectionRectsFile),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionRectFile {
    x: u64,
    y: u64,
    width: u64,
    height: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionRectsFile {
    rects: Vec<SelectionRectFile>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TagFile {
    name: String,
    from: u64,
    to: u64,
    /// Missing means [`LoopMode::Forward`].
    #[serde(default, rename = "loop")]
    loop_mode: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PartFile {
    name: String,
    x: i64,
    y: i64,
    /// Missing when the part is not attached to a layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    layer: Option<i64>,
    /// Missing when the part has no anchors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    anchors: Vec<AnchorFile>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AnchorFile {
    name: String,
    x: i64,
    y: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CurveFile {
    part: String,
    channel: String,
    keys: Vec<CurveKeyFile>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConstraintFile {
    name: String,
    kind: String,
    part: String,
    /// Missing when the constraint tracks the part origin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    anchor: Option<String>,
    /// Missing on a pin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target: Option<String>,
    /// Missing on a pin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target_anchor: Option<String>,
    /// Pin column. Missing on follow and attach.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    x: Option<i64>,
    /// Pin row. Missing on follow and attach.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    y: Option<i64>,
    /// Root-first IK chain. Empty for follow, attach, pin, and plant.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    chain: Vec<String>,
    /// `1` or `-1`. Missing keeps the rest bend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bend: Option<i64>,
    /// Footprint spans. Empty when this constraint has none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    contacts: Vec<ContactFile>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContactFile {
    frame: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    to: Option<u64>,
    x: i64,
    y: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CurveKeyFile {
    frame: u64,
    value: JsonNumber,
    /// Missing means the channel default: `linear`, or `hold` for flip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ease: Option<String>,
}

/// A finite JSON number. Whole values are written as integers.
#[derive(Debug, Clone, Copy)]
struct JsonNumber(f64);

impl Serialize for JsonNumber {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if self.0.is_finite() && self.0.fract() == 0.0 && self.0.abs() <= i64::MAX as f64 {
            serializer.serialize_i64(self.0 as i64)
        } else {
            serializer.serialize_f64(self.0)
        }
    }
}

impl<'de> Deserialize<'de> for JsonNumber {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = f64::deserialize(deserializer)?;
        if value.is_finite() {
            Ok(Self(value))
        } else {
            Err(serde::de::Error::custom("curve value must be finite"))
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ColorFile {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LayerNameFile {
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LayerPixelsFile {
    pixels: Vec<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FrameFile {
    /// The only layer, or omitted when [`Self::layers`] holds the stack.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pixels: Vec<u64>,
    /// Milliseconds. Missing in files saved before delays existed.
    #[serde(default = "default_delay_ms", rename = "delayMs")]
    delay_ms: u64,
    /// One buffer per document layer, bottom to top. Omitted for one layer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    layers: Vec<LayerPixelsFile>,
}

fn is_zero(value: &u64) -> bool {
    *value == 0
}

impl Document {
    /// Pretty-printed version 1 project JSON, including a trailing newline.
    ///
    /// An open gesture is saved as the pixels already on the document. Undo
    /// and redo are omitted.
    pub fn save_project(&self) -> String {
        let file = ProjectFile {
            format: PROJECT_FORMAT.to_string(),
            version: u64::from(PROJECT_VERSION),
            width: u64::from(self.width()),
            height: u64::from(self.height()),
            active_frame: u64::try_from(self.active_frame()).expect("frame index fits in u64"),
            active_layer: u64::try_from(self.active_layer()).expect("layer index fits in u64"),
            layers: self.stored_layer_names(),
            palette: self
                .palette_colors()
                .iter()
                .copied()
                .map(ColorFile::from)
                .collect(),
            palette_locked: self.palette_locked(),
            index_policy: match self.index_policy() {
                IndexPolicy::Reject => None,
                IndexPolicy::Clamp => Some("clamp".to_string()),
            },
            selection: self.selection().map(SelectionFile::from),
            frames: (0..self.frame_count())
                .map(|index| self.stored_frame(index))
                .collect(),
            tags: self.stored_tags(),
            parts: self.stored_parts(),
            curves: self.stored_curves(),
            constraints: self.stored_constraints(),
        };
        let mut json = serde_json::to_string_pretty(&file)
            .expect("project fields are JSON numbers, strings, and arrays");
        json.push('\n');
        json
    }

    /// Reads version 1 project JSON. The undo stack of the result is empty.
    pub fn load_project(json: &str) -> Result<Self, KernelError> {
        let json = json.strip_prefix('\u{feff}').unwrap_or(json);
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|_| invalid(ProjectFileError::Malformed))?;
        let Some(object) = value.as_object() else {
            return Err(invalid(ProjectFileError::Malformed));
        };
        match object.get("format") {
            Some(serde_json::Value::String(name)) if name == PROJECT_FORMAT => {}
            Some(_) => return Err(invalid(ProjectFileError::Format)),
            None => return Err(invalid(ProjectFileError::Malformed)),
        }
        match object.get("version") {
            Some(serde_json::Value::Number(number)) => match number.as_u64() {
                Some(version) if version == u64::from(PROJECT_VERSION) => {}
                Some(version) => return Err(invalid(ProjectFileError::Version { version })),
                None => return Err(invalid(ProjectFileError::Malformed)),
            },
            _ => return Err(invalid(ProjectFileError::Malformed)),
        }

        let file: ProjectFile =
            serde_json::from_value(value).map_err(|_| invalid(ProjectFileError::Malformed))?;
        document_from_file(file)
    }
}

fn document_from_file(file: ProjectFile) -> Result<Document, KernelError> {
    let (width, height) = canvas_size(file.width, file.height)?;
    let palette = palette_from_file(file.palette)?;
    if file.frames.is_empty() {
        return Err(invalid(ProjectFileError::EmptyFrames));
    }
    let expected = usize::from(width) * usize::from(height);
    let layer_names = layer_names_from_file(file.layers)?;
    let mut frames = Vec::with_capacity(file.frames.len());
    for (index, frame) in file.frames.into_iter().enumerate() {
        let buffers =
            layer_buffers_from_file(index, frame.pixels, frame.layers, expected, &palette)?;
        if layer_names.is_empty() {
            if buffers.len() != 1 {
                return Err(invalid(ProjectFileError::LayerCount {
                    frame: index,
                    expected: 1,
                    actual: buffers.len(),
                }));
            }
        } else if buffers.len() != layer_names.len() {
            return Err(invalid(ProjectFileError::LayerCount {
                frame: index,
                expected: layer_names.len(),
                actual: buffers.len(),
            }));
        }
        let delay_ms = frame_delay_from_file(index, frame.delay_ms)?;
        frames.push(Frame::from_layers(buffers, delay_ms));
    }
    if file.active_frame >= u64::try_from(frames.len()).expect("frame count fits in u64") {
        return Err(invalid(ProjectFileError::ActiveFrame {
            index: file.active_frame,
            len: frames.len(),
        }));
    }
    let active = usize::try_from(file.active_frame).expect("active frame was checked");
    let names = if layer_names.is_empty() {
        vec![DEFAULT_LAYER_NAME.to_string()]
    } else {
        layer_names
    };
    if file.active_layer >= u64::try_from(names.len()).expect("layer count fits in u64") {
        return Err(invalid(ProjectFileError::ActiveLayer {
            index: file.active_layer,
            len: names.len(),
        }));
    }
    let active_layer = usize::try_from(file.active_layer).expect("active layer was checked");
    let index_policy = index_policy_from_file(file.index_policy)?;
    let selection = selection_from_file(file.selection, width, height)?;
    let tags = tags_from_file(file.tags, frames.len())?;
    let parts = parts_from_file(file.parts, width, height, names.len())?;
    let curves = curves_from_file(file.curves, &parts, frames.len())?;
    let constraints = constraints_from_file(file.constraints, &parts, width, height, frames.len())?;
    Ok(Document::from_stored(
        width,
        height,
        palette,
        file.palette_locked,
        index_policy,
        frames,
        active,
        names,
        active_layer,
        selection,
        tags,
        parts,
        curves,
        constraints,
    ))
}

fn constraints_from_file(
    files: Vec<ConstraintFile>,
    parts: &[Part],
    width: u16,
    height: u16,
    frame_count: usize,
) -> Result<Vec<Constraint>, KernelError> {
    let mut constraints = Vec::with_capacity(files.len());
    for (index, file) in files.into_iter().enumerate() {
        let name = normalize_part_name(&file.name)
            .map_err(|()| invalid(ProjectFileError::ConstraintName { index }))?;
        if constraints
            .iter()
            .any(|existing: &Constraint| existing.name == name)
        {
            return Err(invalid(ProjectFileError::ConstraintDuplicate { index }));
        }
        let kind = ConstraintKind::parse_stored(&file.kind)
            .ok_or_else(|| invalid(ProjectFileError::ConstraintKind { index }))?;
        let part = normalize_part_name(&file.part)
            .map_err(|()| invalid(ProjectFileError::ConstraintPart { index }))?;
        if !parts.iter().any(|existing| existing.name == part) {
            return Err(invalid(ProjectFileError::ConstraintPart { index }));
        }
        let anchor = optional_constraint_anchor(index, file.anchor)?;
        let target = match file.target {
            None => None,
            Some(target) => {
                let target = normalize_part_name(&target)
                    .map_err(|()| invalid(ProjectFileError::ConstraintPart { index }))?;
                if !parts.iter().any(|existing| existing.name == target) {
                    return Err(invalid(ProjectFileError::ConstraintPart { index }));
                }
                Some(target)
            }
        };
        let target_anchor = optional_constraint_anchor(index, file.target_anchor)?;
        let pin = match (file.x, file.y) {
            (None, None) => None,
            (Some(x), Some(y)) => {
                let x = u16::try_from(x)
                    .map_err(|_| invalid(ProjectFileError::ConstraintPin { index }))?;
                let y = u16::try_from(y)
                    .map_err(|_| invalid(ProjectFileError::ConstraintPin { index }))?;
                if x >= width || y >= height {
                    return Err(invalid(ProjectFileError::ConstraintPin { index }));
                }
                Some((x, y))
            }
            _ => return Err(invalid(ProjectFileError::ConstraintFields { index })),
        };
        let bend = match file.bend {
            None => None,
            Some(1) => Some(1),
            Some(-1) => Some(-1),
            Some(_) => return Err(invalid(ProjectFileError::ConstraintFields { index })),
        };
        let mut contacts = Vec::with_capacity(file.contacts.len());
        for contact in file.contacts {
            let frame = u16::try_from(contact.frame)
                .map_err(|_| invalid(ProjectFileError::ConstraintFields { index }))?;
            let to = match contact.to {
                None => frame,
                Some(to) => u16::try_from(to)
                    .map_err(|_| invalid(ProjectFileError::ConstraintFields { index }))?,
            };
            let x = u16::try_from(contact.x)
                .map_err(|_| invalid(ProjectFileError::ConstraintPin { index }))?;
            let y = u16::try_from(contact.y)
                .map_err(|_| invalid(ProjectFileError::ConstraintPin { index }))?;
            if frame > to || usize::from(to) >= frame_count || x >= width || y >= height {
                return Err(invalid(ProjectFileError::ConstraintFields { index }));
            }
            if contacts
                .iter()
                .any(|earlier: &Contact| earlier.frame <= to && frame <= earlier.to)
            {
                return Err(invalid(ProjectFileError::ConstraintFields { index }));
            }
            contacts.push(Contact { frame, to, x, y });
        }
        let mut chain = Vec::with_capacity(file.chain.len());
        for name in file.chain {
            let name = normalize_part_name(&name)
                .map_err(|()| invalid(ProjectFileError::ConstraintPart { index }))?;
            if chain.iter().any(|existing: &String| existing == &name)
                || !parts.iter().any(|existing| existing.name == name)
            {
                return Err(invalid(ProjectFileError::ConstraintPart { index }));
            }
            chain.push(name);
        }
        match kind {
            ConstraintKind::Follow => {
                if pin.is_some()
                    || target.is_none()
                    || target_anchor.is_none()
                    || bend.is_some()
                    || !chain.is_empty()
                    || !contacts.is_empty()
                {
                    return Err(invalid(ProjectFileError::ConstraintFields { index }));
                }
            }
            ConstraintKind::Attach => {
                if pin.is_some()
                    || anchor.is_none()
                    || target.is_none()
                    || target_anchor.is_none()
                    || bend.is_some()
                    || !chain.is_empty()
                    || !contacts.is_empty()
                {
                    return Err(invalid(ProjectFileError::ConstraintFields { index }));
                }
            }
            ConstraintKind::Pin => {
                if pin.is_none()
                    || target.is_some()
                    || target_anchor.is_some()
                    || bend.is_some()
                    || !chain.is_empty()
                    || !contacts.is_empty()
                {
                    return Err(invalid(ProjectFileError::ConstraintFields { index }));
                }
            }
            ConstraintKind::Plant => {
                if pin.is_some()
                    || target.is_some()
                    || target_anchor.is_some()
                    || bend.is_some()
                    || !chain.is_empty()
                    || contacts.is_empty()
                {
                    return Err(invalid(ProjectFileError::ConstraintFields { index }));
                }
            }
            ConstraintKind::Ik => {
                if chain.len() < 2
                    || chain.len() > 3
                    || chain.last().map(String::as_str) != Some(part.as_str())
                {
                    return Err(invalid(ProjectFileError::ConstraintFields { index }));
                }
                let has_cell = pin.is_some();
                let has_target = target.is_some() || target_anchor.is_some();
                if has_cell && has_target {
                    return Err(invalid(ProjectFileError::ConstraintFields { index }));
                }
                if has_target && (target.is_none() || target_anchor.is_none()) {
                    return Err(invalid(ProjectFileError::ConstraintFields { index }));
                }
                if !has_cell && !has_target && contacts.is_empty() {
                    return Err(invalid(ProjectFileError::ConstraintFields { index }));
                }
                if let Some(target_name) = target.as_deref() {
                    if chain.iter().any(|joint| joint == target_name) {
                        return Err(invalid(ProjectFileError::ConstraintCycle { index }));
                    }
                }
                match crate::constraint::ik_bones(parts, &chain, anchor.as_deref()) {
                    Ok(_) | Err(KernelError::AnchorMissing) => {}
                    Err(KernelError::ConstraintFields) => {
                        return Err(invalid(ProjectFileError::ConstraintFields { index }));
                    }
                    Err(_) => return Err(invalid(ProjectFileError::Malformed)),
                }
            }
        }
        let (x, y) = match pin {
            None => (None, None),
            Some((x, y)) => (Some(x), Some(y)),
        };
        constraints.push(Constraint {
            name,
            kind,
            part,
            anchor,
            target,
            target_anchor,
            x,
            y,
            chain,
            bend,
            contacts,
        });
    }
    match solve_order(&constraints) {
        Ok(_) => Ok(constraints),
        Err(KernelError::ConstraintConflict { part }) => {
            let index = constraints
                .iter()
                .rposition(|constraint| constraint.part == part)
                .expect("the conflicting part is in the list");
            Err(invalid(ProjectFileError::ConstraintConflict { index }))
        }
        Err(KernelError::ConstraintCycle { name }) => {
            let index = constraints
                .iter()
                .position(|constraint| constraint.name == name)
                .expect("the cycle names a constraint in the list");
            Err(invalid(ProjectFileError::ConstraintCycle { index }))
        }
        Err(_) => Err(invalid(ProjectFileError::Malformed)),
    }
}

fn optional_constraint_anchor(
    index: usize,
    name: Option<String>,
) -> Result<Option<String>, KernelError> {
    match name {
        None => Ok(None),
        Some(name) => {
            Ok(Some(normalize_part_name(&name).map_err(|()| {
                invalid(ProjectFileError::ConstraintAnchor { index })
            })?))
        }
    }
}

fn curves_from_file(
    files: Vec<CurveFile>,
    parts: &[Part],
    frame_count: usize,
) -> Result<Vec<Curve>, KernelError> {
    let mut curves: Vec<Curve> = Vec::with_capacity(files.len());
    for (index, file) in files.into_iter().enumerate() {
        let part = normalize_part_name(&file.part)
            .map_err(|()| invalid(ProjectFileError::CurvePart { index }))?;
        if !parts.iter().any(|existing| existing.name == part) {
            return Err(invalid(ProjectFileError::CurvePart { index }));
        }
        let channel = CurveChannel::parse(&file.channel)
            .ok_or_else(|| invalid(ProjectFileError::CurveChannel { index }))?;
        if curves
            .iter()
            .any(|curve| curve.part == part && curve.channel == channel)
        {
            return Err(invalid(ProjectFileError::CurveDuplicate { index }));
        }
        if file.keys.is_empty() {
            return Err(invalid(ProjectFileError::CurveKey { index }));
        }
        let mut keys = Vec::with_capacity(file.keys.len());
        let mut previous: Option<usize> = None;
        for key in file.keys {
            let frame = usize::try_from(key.frame)
                .map_err(|_| invalid(ProjectFileError::CurveKey { index }))?;
            if frame >= frame_count || previous.is_some_and(|prev| frame <= prev) {
                return Err(invalid(ProjectFileError::CurveKey { index }));
            }
            previous = Some(frame);
            let ease = match key.ease {
                None => channel.default_ease(),
                Some(text) => Ease::parse(&text)
                    .ok_or_else(|| invalid(ProjectFileError::CurveEase { index }))?,
            };
            if !channel.allows(ease) {
                return Err(invalid(ProjectFileError::CurveEase { index }));
            }
            let stored = channel
                .store(key.value.0)
                .map_err(|_| invalid(ProjectFileError::CurveValue { index }))?;
            keys.push(CurveKey {
                frame,
                value: stored,
                ease,
            });
        }
        curves.push(Curve {
            part,
            channel,
            keys,
        });
    }
    Ok(curves)
}

fn parts_from_file(
    files: Vec<PartFile>,
    width: u16,
    height: u16,
    layer_count: usize,
) -> Result<Vec<Part>, KernelError> {
    let mut parts: Vec<Part> = Vec::with_capacity(files.len());
    for (index, file) in files.into_iter().enumerate() {
        let part = part_from_file(index, file, width, height, layer_count)?;
        if parts.iter().any(|existing| existing.name == part.name) {
            return Err(invalid(ProjectFileError::PartDuplicate { index }));
        }
        parts.push(part);
    }
    Ok(parts)
}

fn part_from_file(
    index: usize,
    file: PartFile,
    width: u16,
    height: u16,
    layer_count: usize,
) -> Result<Part, KernelError> {
    let name = normalize_part_name(&file.name)
        .map_err(|()| invalid(ProjectFileError::PartName { index }))?;
    let x = u16::try_from(file.x).map_err(|_| invalid(ProjectFileError::PartOrigin { index }))?;
    let y = u16::try_from(file.y).map_err(|_| invalid(ProjectFileError::PartOrigin { index }))?;
    if x >= width || y >= height {
        return Err(invalid(ProjectFileError::PartOrigin { index }));
    }
    let layer = match file.layer {
        None => None,
        Some(layer) => {
            let layer = usize::try_from(layer)
                .map_err(|_| invalid(ProjectFileError::PartLayer { index }))?;
            if layer >= layer_count {
                return Err(invalid(ProjectFileError::PartLayer { index }));
            }
            Some(layer)
        }
    };
    let mut anchors = Vec::with_capacity(file.anchors.len());
    for (anchor_index, anchor) in file.anchors.into_iter().enumerate() {
        let anchor_name = normalize_part_name(&anchor.name).map_err(|()| {
            invalid(ProjectFileError::AnchorName {
                part: index,
                index: anchor_index,
            })
        })?;
        if anchors
            .iter()
            .any(|existing: &Anchor| existing.name == anchor_name)
        {
            return Err(invalid(ProjectFileError::AnchorDuplicate {
                part: index,
                index: anchor_index,
            }));
        }
        let Some((anchor_x, anchor_y)) = anchor_offset_i64(anchor.x, anchor.y) else {
            return Err(invalid(ProjectFileError::AnchorCoord {
                part: index,
                index: anchor_index,
            }));
        };
        anchors.push(Anchor {
            name: anchor_name,
            x: anchor_x,
            y: anchor_y,
        });
    }
    Ok(Part {
        name,
        x,
        y,
        layer,
        anchors,
    })
}

fn tags_from_file(files: Vec<TagFile>, frame_count: usize) -> Result<Vec<Tag>, KernelError> {
    let mut tags: Vec<Tag> = Vec::with_capacity(files.len());
    for (index, file) in files.into_iter().enumerate() {
        let tag = tag_from_file(index, file, frame_count)?;
        if tags.iter().any(|existing| existing.name == tag.name) {
            return Err(invalid(ProjectFileError::TagDuplicate { index }));
        }
        tags.push(tag);
    }
    Ok(tags)
}

fn tag_from_file(index: usize, file: TagFile, frame_count: usize) -> Result<Tag, KernelError> {
    let name = normalize_tag_name(&file.name)
        .map_err(|()| invalid(ProjectFileError::TagName { index }))?;
    let from =
        usize::try_from(file.from).map_err(|_| invalid(ProjectFileError::TagRange { index }))?;
    let to = usize::try_from(file.to).map_err(|_| invalid(ProjectFileError::TagRange { index }))?;
    if from > to || to >= frame_count {
        return Err(invalid(ProjectFileError::TagRange { index }));
    }
    let loop_mode = match file.loop_mode.as_deref() {
        None | Some("forward") => LoopMode::Forward,
        Some("reverse") => LoopMode::Reverse,
        Some("pingpong") => LoopMode::PingPong,
        Some("once") => LoopMode::Once,
        Some(_) => return Err(invalid(ProjectFileError::TagLoop { index })),
    };
    Ok(Tag {
        name,
        from,
        to,
        loop_mode,
    })
}

fn selection_from_file(
    value: Option<SelectionFile>,
    width: u16,
    height: u16,
) -> Result<Option<Mask>, KernelError> {
    let Some(file) = value else {
        return Ok(None);
    };
    let rects = match file {
        SelectionFile::Rect(rect) => vec![selection_rect_from_file(rect)?],
        SelectionFile::Rects(file) => file
            .rects
            .into_iter()
            .map(selection_rect_from_file)
            .collect::<Result<Vec<_>, _>>()?,
    };
    let mask = Mask::try_from_rects(rects).map_err(|()| invalid(ProjectFileError::Selection))?;
    if mask.fits(width, height) {
        Ok(Some(mask))
    } else {
        Err(invalid(ProjectFileError::Selection))
    }
}

fn selection_rect_from_file(file: SelectionRectFile) -> Result<Selection, KernelError> {
    Ok(Selection {
        x: u16::try_from(file.x).map_err(|_| invalid(ProjectFileError::Selection))?,
        y: u16::try_from(file.y).map_err(|_| invalid(ProjectFileError::Selection))?,
        width: u16::try_from(file.width).map_err(|_| invalid(ProjectFileError::Selection))?,
        height: u16::try_from(file.height).map_err(|_| invalid(ProjectFileError::Selection))?,
    })
}

fn index_policy_from_file(value: Option<String>) -> Result<IndexPolicy, KernelError> {
    match value.as_deref() {
        None | Some("reject") => Ok(IndexPolicy::Reject),
        Some("clamp") => Ok(IndexPolicy::Clamp),
        Some(_) => Err(invalid(ProjectFileError::IndexPolicy)),
    }
}

impl Document {
    fn stored_parts(&self) -> Vec<PartFile> {
        self.parts()
            .iter()
            .map(|part| PartFile {
                name: part.name.clone(),
                x: i64::from(part.x),
                y: i64::from(part.y),
                layer: part
                    .layer
                    .map(|layer| i64::try_from(layer).expect("layer index fits in i64")),
                anchors: part
                    .anchors
                    .iter()
                    .map(|anchor| AnchorFile {
                        name: anchor.name.clone(),
                        x: i64::from(anchor.x),
                        y: i64::from(anchor.y),
                    })
                    .collect(),
            })
            .collect()
    }

    fn stored_constraints(&self) -> Vec<ConstraintFile> {
        self.constraints()
            .iter()
            .map(|constraint| ConstraintFile {
                name: constraint.name.clone(),
                kind: constraint.kind.as_str().to_string(),
                part: constraint.part.clone(),
                anchor: constraint.anchor.clone(),
                target: constraint.target.clone(),
                target_anchor: constraint.target_anchor.clone(),
                x: constraint.x.map(i64::from),
                y: constraint.y.map(i64::from),
                chain: constraint.chain.clone(),
                bend: constraint.bend.map(i64::from),
                contacts: constraint
                    .contacts
                    .iter()
                    .map(|contact| ContactFile {
                        frame: u64::from(contact.frame),
                        to: if contact.to == contact.frame {
                            None
                        } else {
                            Some(u64::from(contact.to))
                        },
                        x: i64::from(contact.x),
                        y: i64::from(contact.y),
                    })
                    .collect(),
            })
            .collect()
    }

    fn stored_curves(&self) -> Vec<CurveFile> {
        self.curves()
            .iter()
            .map(|curve| CurveFile {
                part: curve.part.clone(),
                channel: curve.channel.as_str().to_string(),
                keys: curve
                    .keys
                    .iter()
                    .map(|key| CurveKeyFile {
                        frame: u64::try_from(key.frame).expect("frame index fits in u64"),
                        value: JsonNumber(curve.channel.decode(key.value)),
                        ease: if key.ease == curve.channel.default_ease() {
                            None
                        } else {
                            Some(key.ease.as_str().to_string())
                        },
                    })
                    .collect(),
            })
            .collect()
    }

    fn stored_tags(&self) -> Vec<TagFile> {
        self.tags()
            .iter()
            .map(|tag| TagFile {
                name: tag.name.clone(),
                from: u64::try_from(tag.from).expect("frame index fits in u64"),
                to: u64::try_from(tag.to).expect("frame index fits in u64"),
                loop_mode: Some(tag.loop_mode.as_str().to_string()),
            })
            .collect()
    }

    fn stored_layer_names(&self) -> Vec<LayerNameFile> {
        let legacy = self.layer_count() == 1
            && self.layer_name(0).expect("a document has a layer") == DEFAULT_LAYER_NAME
            && self.active_layer() == 0;
        if legacy {
            Vec::new()
        } else {
            self.layer_names()
                .iter()
                .map(|name| LayerNameFile { name: name.clone() })
                .collect()
        }
    }

    fn stored_frame(&self, index: usize) -> FrameFile {
        let delay_ms = u64::from(self.frame_delay(index).expect("index walks frame_count"));
        if self.layer_count() == 1 {
            FrameFile {
                pixels: indices_to_file(
                    self.layer_pixels(index, 0)
                        .expect("index walks frame_count"),
                ),
                delay_ms,
                layers: Vec::new(),
            }
        } else {
            FrameFile {
                pixels: Vec::new(),
                delay_ms,
                layers: (0..self.layer_count())
                    .map(|layer| LayerPixelsFile {
                        pixels: indices_to_file(
                            self.layer_pixels(index, layer)
                                .expect("layer walks layer_count"),
                        ),
                    })
                    .collect(),
            }
        }
    }
}

fn indices_to_file(pixels: &[u8]) -> Vec<u64> {
    pixels.iter().copied().map(u64::from).collect()
}

fn layer_names_from_file(layers: Vec<LayerNameFile>) -> Result<Vec<String>, KernelError> {
    let mut names = Vec::with_capacity(layers.len());
    for (index, layer) in layers.into_iter().enumerate() {
        names.push(layer_name_from_file(index, &layer.name)?);
    }
    Ok(names)
}

fn layer_name_from_file(index: usize, name: &str) -> Result<String, KernelError> {
    let trimmed = name.trim();
    let chars = trimmed.chars().count();
    if trimmed.is_empty() || chars > MAX_LAYER_NAME_CHARS || trimmed.chars().any(char::is_control) {
        Err(invalid(ProjectFileError::LayerName { index }))
    } else {
        Ok(trimmed.to_string())
    }
}

fn layer_buffers_from_file(
    frame: usize,
    pixels: Vec<u64>,
    layers: Vec<LayerPixelsFile>,
    expected: usize,
    palette: &Palette,
) -> Result<Vec<Vec<u8>>, KernelError> {
    if !pixels.is_empty() && !layers.is_empty() {
        return Err(invalid(ProjectFileError::LayerPixels { frame }));
    }
    if !layers.is_empty() {
        let mut buffers = Vec::with_capacity(layers.len());
        for (layer, buffer) in layers.into_iter().enumerate() {
            if buffer.pixels.len() != expected {
                return Err(invalid(ProjectFileError::LayerLength {
                    frame,
                    layer,
                    expected,
                    actual: buffer.pixels.len(),
                }));
            }
            buffers.push(decode_indices(frame, buffer.pixels, palette)?);
        }
        return Ok(buffers);
    }
    if pixels.len() != expected {
        return Err(invalid(ProjectFileError::FrameLength {
            index: frame,
            expected,
            actual: pixels.len(),
        }));
    }
    Ok(vec![decode_indices(frame, pixels, palette)?])
}

fn decode_indices(
    frame: usize,
    pixels: Vec<u64>,
    palette: &Palette,
) -> Result<Vec<u8>, KernelError> {
    let mut decoded = Vec::with_capacity(pixels.len());
    for index_value in pixels {
        if index_value >= u64::try_from(palette.len()).expect("palette length fits in u64") {
            return Err(invalid(ProjectFileError::PixelIndex {
                frame,
                index: index_value,
                palette_len: palette.len(),
            }));
        }
        decoded.push(u8::try_from(index_value).expect("index is below the palette length"));
    }
    Ok(decoded)
}

fn frame_delay_from_file(index: usize, delay: u64) -> Result<u32, KernelError> {
    if (u64::from(MIN_FRAME_DELAY_MS)..=u64::from(MAX_FRAME_DELAY_MS)).contains(&delay) {
        Ok(u32::try_from(delay).expect("delay range fits in u32"))
    } else {
        Err(invalid(ProjectFileError::FrameDelay {
            frame: index,
            delay,
        }))
    }
}

fn canvas_size(width: u64, height: u64) -> Result<(u16, u16), KernelError> {
    let width_u16 = u16::try_from(width);
    let height_u16 = u16::try_from(height);
    match (width_u16, height_u16) {
        (Ok(width), Ok(height))
            if width > 0 && height > 0 && width <= MAX_CANVAS_EDGE && height <= MAX_CANVAS_EDGE =>
        {
            Ok((width, height))
        }
        _ => Err(invalid(ProjectFileError::Size { width, height })),
    }
}

fn palette_from_file(colors: Vec<ColorFile>) -> Result<Palette, KernelError> {
    let colors = colors.into_iter().map(Rgba::from).collect();
    Palette::new(colors).map_err(|error| match error {
        KernelError::EmptyPalette => invalid(ProjectFileError::EmptyPalette),
        KernelError::PaletteTooLarge { len } => invalid(ProjectFileError::PaletteTooLarge { len }),
        KernelError::TransparentSlot => invalid(ProjectFileError::TransparentSlot),
        other => other,
    })
}

fn invalid(error: ProjectFileError) -> KernelError {
    KernelError::ProjectFile(error)
}

impl From<&Mask> for SelectionFile {
    fn from(mask: &Mask) -> Self {
        if let Some(selection) = mask.as_rect() {
            Self::Rect(SelectionRectFile::from(selection))
        } else {
            Self::Rects(SelectionRectsFile {
                rects: mask
                    .rects()
                    .iter()
                    .copied()
                    .map(SelectionRectFile::from)
                    .collect(),
            })
        }
    }
}

impl From<Selection> for SelectionRectFile {
    fn from(selection: Selection) -> Self {
        Self {
            x: u64::from(selection.x),
            y: u64::from(selection.y),
            width: u64::from(selection.width),
            height: u64::from(selection.height),
        }
    }
}

impl From<Rgba> for ColorFile {
    fn from(color: Rgba) -> Self {
        Self {
            r: color.r,
            g: color.g,
            b: color.b,
            a: color.a,
        }
    }
}

impl From<ColorFile> for Rgba {
    fn from(color: ColorFile) -> Self {
        Self::new(color.r, color.g, color.b, color.a)
    }
}
