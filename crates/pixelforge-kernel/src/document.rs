use std::cmp::Ordering;

use crate::constraint::{
    apply_deltas, canvas_local, constraints_after_insert, constraints_after_remove,
    constraints_after_rename, constraints_after_resize, constraints_for_parts, ink_label,
    motion_steps, orient_part, place_origin, prepare_constraint, solve_order, warp_ink,
    warp_overlay, Constraint, ConstraintKind, Contact, MotionStep, PosedPart, SolveReport,
    SolvedAnchor, SolvedPart, WarpReport,
};
use crate::curve::{
    curves_after_insert, curves_after_remove, curves_after_rename, curves_for_parts, sample_keys,
    Curve, CurveChannel, CurveKey, CurveRemoval, CurveSample, CurveWrite, Ease, PartSample,
};
use crate::error::KernelError;
use crate::frame::Frame;
use crate::history::{Change, History};
use crate::palette::{IndexPolicy, Palette};
use crate::palette_file::{parse_palette_text, PaletteImportMode};
use crate::palette_map::{self, DecodedPng, MapReport, PalettePlan};
use crate::part::{
    anchor_offset_i64, normalize_part_name, parts_after_flatten, parts_after_layer_insert,
    parts_after_layer_remove, parts_after_reorder, parts_after_resize, Anchor, LayerAttach, Part,
    PartUpdate,
};
use crate::part_xform::{self, map_point, CellMap, Motion, PartPose};
use crate::raster::{self, Connectivity};
use crate::selection::{Mask, MaskOp, Selection};
use crate::tag::{
    normalize_tag_name, tags_after_insert, tags_after_remove, LoopMode, Tag, TagUpdate,
};
use crate::LayerRole;
use crate::PolishBind;
use crate::Rgba;
use crate::DEFAULT_FRAME_DELAY_MS;
use crate::DEFAULT_LAYER_NAME;
use crate::LAYER_OPACITY_OPAQUE;
use crate::MAX_BAKE_FRAMES;
use crate::MAX_CANVAS_EDGE;
use crate::MAX_FRAME_DELAY_MS;
use crate::MAX_LAYER_NAME_CHARS;
use crate::MIN_FRAME_DELAY_MS;
use crate::TIMELINE_RATIO_PPM;
use crate::TRANSPARENT_INDEX;

/// What [`Document::bake`] was asked to write.
#[derive(Debug, Clone, PartialEq)]
pub struct BakeOptions {
    /// First frame index. `None` is 0.
    pub from: Option<usize>,
    /// Last frame index, inclusive. `None` is the last existing frame.
    pub to: Option<usize>,
    /// Frame whose pixels are the rest pose. `None` is frame 0.
    ///
    /// Bake never overwrites this frame. A second bake reads the same rest
    /// ink, so it does not stack.
    pub source: Option<usize>,
    /// When set, each frame in the range gets `round(1000 / fps)` milliseconds.
    pub fps: Option<f64>,
    /// Pose only these parts. `None` poses every part. An empty list poses none.
    pub parts: Option<Vec<String>>,
    /// Write only these layers. `None` writes layers managed by the sampled
    /// curves and solved constraints, including an identity pose, or the
    /// linked layers of `parts` when that filter is set.
    pub layers: Option<Vec<usize>>,
    /// Sample curves, then solve constraints. `false` samples curves only.
    pub constraints: bool,
}

impl Default for BakeOptions {
    fn default() -> Self {
        Self {
            from: None,
            to: None,
            source: None,
            fps: None,
            parts: None,
            layers: None,
            constraints: true,
        }
    }
}

/// One frame after [`Document::bake`].
///
/// `x` and `y` on each part are the posed origin. The file keeps the rest
/// origin. `rest` is the source frame, which was not rewritten.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BakeFrame {
    pub index: usize,
    pub rest: bool,
    pub delay_ms: u32,
    pub dropped: u32,
    pub conflicts: u32,
    pub sealed: u32,
    pub breaks: u32,
    /// Layers whose ink was replaced from the rest pose. Empty on the source.
    pub layers: Vec<usize>,
    /// Bound polish layers restamped from the source pose. Empty on the source
    /// and when no overlay follows a posed part.
    pub followed: Vec<usize>,
    pub parts: Vec<SolvedPart>,
}

struct OverlayMotion {
    layer: usize,
    pixels: Vec<u8>,
    erase: Vec<u8>,
    old_pixels: Vec<u8>,
    old_erase: Vec<u8>,
    report: WarpReport,
}

struct BakeWrite {
    layer: usize,
    pixels: Vec<u8>,
    /// `Some` when this write is a bound polish overlay, including its erase mask.
    erase: Option<Vec<u8>>,
    report: WarpReport,
}

/// What [`Document::bake`] wrote. Curves and constraints are not removed.
#[derive(Debug, Clone, PartialEq)]
pub struct BakeReport {
    pub from: usize,
    pub to: usize,
    pub source: usize,
    pub fps: Option<f64>,
    pub delay_ms: Option<u32>,
    pub constraints: bool,
    pub appended: usize,
    pub wrote: bool,
    pub dropped: u32,
    pub conflicts: u32,
    pub sealed: u32,
    pub breaks: u32,
    /// Polish overlay indices, bound and unbound. Unbound layers are not written.
    /// Bound layers are restamped and listed on each frame's `followed`.
    pub polish: Vec<usize>,
    pub frames: Vec<BakeFrame>,
}

struct BakePlan {
    index: usize,
    rest: bool,
    delay_ms: u32,
    is_new: bool,
    new_frame: Option<Frame>,
    writes: Vec<BakeWrite>,
    report: WarpReport,
    layers: Vec<usize>,
    followed: Vec<usize>,
    parts: Vec<SolvedPart>,
}

fn delay_from_fps(fps: f64) -> Result<u32, KernelError> {
    if !fps.is_finite() || fps <= 0.0 {
        return Err(KernelError::BakeFps);
    }
    let rounded = (1000.0 / fps).round();
    if !rounded.is_finite() || rounded < 1.0 || rounded > f64::from(MAX_FRAME_DELAY_MS) {
        let delay_ms = if rounded.is_finite() && (0.0..=f64::from(u32::MAX)).contains(&rounded) {
            rounded as u32
        } else if rounded > f64::from(u32::MAX) {
            u32::MAX
        } else {
            0
        };
        return Err(KernelError::FrameDelay { delay_ms });
    }
    Ok(rounded as u32)
}

fn stored_bake_part(part: &crate::part::Part) -> SolvedPart {
    SolvedPart {
        name: part.name.clone(),
        x: part.x,
        y: part.y,
        layer: part.layer,
        ink: "unchanged",
        dropped: 0,
        conflicts: 0,
        sealed: 0,
        breaks: 0,
        anchors: part
            .anchors
            .iter()
            .map(|anchor| SolvedAnchor {
                name: anchor.name.clone(),
                x: anchor.x,
                y: anchor.y,
                canvas_x: i32::from(part.x) + i32::from(anchor.x),
                canvas_y: i32::from(part.y) + i32::from(anchor.y),
            })
            .collect(),
    }
}

/// A solved constraint poses this part, including an IK joint that is not the end.
fn constraint_drives(constraint: &Constraint, name: &str) -> bool {
    if constraint.part == name {
        return true;
    }
    constraint.kind == ConstraintKind::Ik && constraint.chain.iter().any(|joint| joint == name)
}

/// Layers to replace from the rest ink.
///
/// `managed[i]` is true when part `i` is sampled from a curve, or driven by a
/// constraint this bake solves. An identity sample still counts: returning a
/// curve to zero must rewrite that layer. `parts` names linked layers instead.
/// `layers` intersects either set. A part with neither a curve nor a driving
/// constraint does not pull its layer in, so a body with no part stays put.
fn bake_write_layers(
    posed: &[PosedPart],
    managed: &[bool],
    parts: Option<&[String]>,
    layers: Option<&[usize]>,
    layer_count: usize,
) -> Vec<usize> {
    let mut managed_layers = Vec::new();
    for (index, part) in posed.iter().enumerate() {
        if !managed.get(index).copied().unwrap_or(false) {
            continue;
        }
        if let Some(layer) = part.oriented.layer {
            if !managed_layers.contains(&layer) {
                managed_layers.push(layer);
            }
        }
    }
    let mut out = Vec::new();
    for layer in 0..layer_count {
        if layers.is_some_and(|layers| !layers.contains(&layer)) {
            continue;
        }
        if let Some(names) = parts {
            let linked = posed.iter().any(|part| {
                names.iter().any(|name| name == &part.oriented.name)
                    && part.oriented.layer == Some(layer)
            });
            if linked {
                out.push(layer);
            }
            continue;
        }
        if layers.is_some() || managed_layers.contains(&layer) {
            out.push(layer);
        }
    }
    out
}

/// Drops polish overlays out of a bake write set.
///
/// Naming a polish layer with `parts` or `layers` leaves its pixels. A part
/// this bake manages must not be linked to it, including when the pose is the
/// identity: that rebuild would be dropped, so the call fails and writes nothing.
fn exclude_polish_layers(
    write_layers: &[usize],
    roles: &[LayerRole],
    posed: &[PosedPart],
    managed: &[bool],
) -> Result<Vec<usize>, KernelError> {
    let mut kept = Vec::with_capacity(write_layers.len());
    for &layer in write_layers {
        if roles.get(layer).copied().unwrap_or(LayerRole::Default) != LayerRole::Polish {
            kept.push(layer);
            continue;
        }
        let managed_here = posed.iter().enumerate().any(|(index, part)| {
            managed.get(index).copied().unwrap_or(false) && part.oriented.layer == Some(layer)
        });
        if managed_here {
            return Err(KernelError::PolishLayer { layer });
        }
    }
    Ok(kept)
}

fn describe_bake_parts(
    posed: &[PosedPart],
    steps: &[Vec<MotionStep>],
    written: &[usize],
    reports: &[(usize, WarpReport)],
    width: u16,
    height: u16,
) -> Result<Vec<SolvedPart>, KernelError> {
    let mut parts = Vec::with_capacity(posed.len());
    for (index, part) in posed.iter().enumerate() {
        let moves = !steps[index].is_empty();
        let layer = part.oriented.layer;
        let written_layer = layer.is_some_and(|layer| written.contains(&layer));
        let lost = layer
            .and_then(|layer| {
                reports
                    .iter()
                    .find(|(reported_layer, _)| *reported_layer == layer)
                    .map(|(_, report)| *report)
            })
            .unwrap_or(WarpReport::ZERO);
        let lost = if moves && written_layer {
            lost
        } else {
            WarpReport::ZERO
        };
        let anchors = part
            .oriented
            .anchors
            .iter()
            .map(|anchor| {
                let (canvas_x, canvas_y) =
                    canvas_local(part, i32::from(anchor.x), i32::from(anchor.y))?;
                Ok(SolvedAnchor {
                    name: anchor.name.clone(),
                    x: anchor.x,
                    y: anchor.y,
                    canvas_x,
                    canvas_y,
                })
            })
            .collect::<Result<Vec<_>, KernelError>>()?;
        let (origin_x, origin_y) = place_origin(part, width, height)?;
        parts.push(SolvedPart {
            name: part.oriented.name.clone(),
            x: origin_x,
            y: origin_y,
            layer,
            ink: if moves && written_layer {
                "moved"
            } else {
                "unchanged"
            },
            dropped: lost.dropped,
            conflicts: lost.conflicts,
            sealed: lost.sealed,
            breaks: lost.breaks,
            anchors,
        });
    }
    Ok(parts)
}

/// Indexed pixel document: size, palette, frames, layers, delays, tags, parts, curves, constraints, and the undo stack.
///
/// UI gestures and Tool API calls meet here. Paint, erase, rectangle fill,
/// rectangle erase, rectangle copy, rectangle move, palette edits, frame
/// edits, layer edits, and delay edits record kernel operations on this
/// document. Drawing writes the active layer. When a selection mask is set,
/// paint tools skip cells outside it. Preview and export
/// composite the stack. Undo restores those operations; it does not patch a
/// buffer owned by the shell.
/// Loop playback is preview state: [`Self::tick`] moves the active frame from
/// each frame's delay and does not push undo entries. Project open and save
/// encode pixels, palette, the palette lock, frames, and delays, not the undo
/// stack or the playback clock. PNG export reads this state and does not
/// record an undo entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    width: u16,
    height: u16,
    palette: Palette,
    /// When set, recolor, append, import, and palette remap fail. Paint still
    /// writes indices. PNG crush writes indices and is not a palette edit.
    palette_locked: bool,
    /// Out-of-range `paint` / `fill_rect` indices. Default is reject.
    index_policy: IndexPolicy,
    frames: Vec<Frame>,
    active: usize,
    /// Bottom layer is index 0. The same names and order apply to every frame.
    layer_names: Vec<String>,
    /// Bake role of each layer. Same length as [`Self::layer_names`].
    ///
    /// [`LayerRole::Polish`] is an overlay. Bake does not rebuild it from a
    /// linked part. A binding restamps it from the source pose. The file omits
    /// the role when it is [`LayerRole::Default`].
    layer_roles: Vec<LayerRole>,
    /// Part binding of each polish layer. `None` keeps canvas coordinates.
    layer_binds: Vec<Option<PolishBind>>,
    /// Composite visibility. Same length as [`Self::layer_names`].
    ///
    /// `false` skips the layer in every composite. The file omits the flag
    /// when it is `true`. Drawing still writes a hidden layer.
    layer_visible: Vec<bool>,
    /// Composite opacity, `0..=255`. Same length as [`Self::layer_names`].
    ///
    /// `0` skips the layer. [`LAYER_OPACITY_OPAQUE`] covers and is omitted
    /// from the file. A value in between blends in the RGBA composite only.
    layer_opacity: Vec<u8>,
    active_layer: usize,
    /// Canvas mask shared by every frame and layer, or none.
    ///
    /// `None` means every cell is writable. `Some` with no rectangles protects
    /// every cell. Paint tools skip cells outside it. The project file stores it.
    selection: Option<Mask>,
    /// Named inclusive frame ranges. Empty when the file omits `tags`.
    /// Not a linked cel: each frame still owns its pixels.
    tags: Vec<Tag>,
    /// Named parts and local anchors. Empty when the file omits `parts`.
    /// Not a layer and not a pixel buffer.
    parts: Vec<Part>,
    /// Keyframed part-transform parameters. Empty when the file omits `curves`.
    /// Sampling reads this list and does not write pixels.
    curves: Vec<Curve>,
    /// Follow, attach, and pin definitions. Empty when the file omits
    /// `constraints`. Solving them is a separate call.
    constraints: Vec<Constraint>,
    history: History,
    playback: Playback,
}

/// Preview clock. Not stored in the project file and not part of an undo entry.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Playback {
    playing: bool,
    /// Milliseconds already shown of the active frame.
    elapsed_ms: u32,
}

impl Playback {
    fn stopped() -> Self {
        Self {
            playing: false,
            elapsed_ms: 0,
        }
    }
}

impl Document {
    /// Blank document filled with transparent indices and the studio palette.
    pub fn new(width: u16, height: u16) -> Result<Self, KernelError> {
        Self::with_palette(width, height, Palette::studio_default())
    }

    pub fn with_palette(width: u16, height: u16, palette: Palette) -> Result<Self, KernelError> {
        check_size(width, height)?;
        let len = pixel_len(width, height);
        Ok(Self {
            width,
            height,
            palette,
            palette_locked: false,
            index_policy: IndexPolicy::Reject,
            frames: vec![Frame::blank(1, len, DEFAULT_FRAME_DELAY_MS)],
            active: 0,
            layer_names: vec![DEFAULT_LAYER_NAME.to_string()],
            layer_roles: vec![LayerRole::Default],
            layer_binds: vec![None],
            layer_visible: vec![true],
            layer_opacity: vec![LAYER_OPACITY_OPAQUE],
            active_layer: 0,
            selection: None,
            tags: Vec::new(),
            parts: Vec::new(),
            curves: Vec::new(),
            constraints: Vec::new(),
            history: History::new(),
            playback: Playback::stopped(),
        })
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    pub fn palette_colors(&self) -> &[Rgba] {
        self.palette.colors()
    }

    /// True when recolor, append, import, and palette remap are refused.
    pub fn palette_locked(&self) -> bool {
        self.palette_locked
    }

    /// How `paint` and `fill_rect` treat an index past the palette.
    pub fn index_policy(&self) -> IndexPolicy {
        self.index_policy
    }

    /// Resolves a paint index under [`Self::index_policy`].
    ///
    /// An in-range opaque index is returned unchanged. Index 0 is
    /// [`KernelError::PaletteIndex`] in both modes. [`IndexPolicy::Clamp`]
    /// maps any other out-of-range index to the last opaque entry.
    pub fn resolve_paint_index(&self, color_index: u8) -> Result<u8, KernelError> {
        if self.palette.contains_paint_index(color_index) {
            return Ok(color_index);
        }
        if self.index_policy == IndexPolicy::Clamp && color_index != TRANSPARENT_INDEX {
            let last = self.palette.len().saturating_sub(1);
            if last >= 1 {
                return Ok(u8::try_from(last).expect("palette length fits in u8"));
            }
        }
        Err(KernelError::PaletteIndex { index: color_index })
    }

    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    pub fn active_frame(&self) -> usize {
        self.active
    }

    /// Palette indices for the active layer of the active frame, row-major.
    ///
    /// This is the buffer [`Self::paint`] writes. It is not the composite.
    /// [`Self::composite_pixels`] stacks every layer.
    pub fn pixels(&self) -> &[u8] {
        self.frames[self.active].layer_pixels(self.active_layer)
    }

    /// Active-layer indices for one frame. Not the composite.
    pub fn frame_pixels(&self, index: usize) -> Result<&[u8], KernelError> {
        let index = self.frame_or_err(index)?;
        Ok(self.frames[index].layer_pixels(self.active_layer))
    }

    /// Indices stored on one layer of one frame.
    pub fn layer_pixels(&self, frame: usize, layer: usize) -> Result<&[u8], KernelError> {
        let frame = self.frame_or_err(frame)?;
        let layer = self.layer_or_err(layer)?;
        Ok(self.frames[frame].layer_pixels(layer))
    }

    /// Bottom-to-top index composite of one frame.
    ///
    /// A hidden layer, or a layer at opacity 0, is skipped. Index 0 leaves the
    /// lower layer visible, unless this layer is polish and the cell is an
    /// explicit erase, which clears the composite. Any other index replaces
    /// it, including when that layer's opacity is only partial. Partial opacity
    /// is not quantized to a palette index. [`Self::frame_rgba`] blends it for
    /// PNG export.
    pub fn composite_pixels(&self, frame: usize) -> Result<Vec<u8>, KernelError> {
        let frame = self.frame_or_err(frame)?;
        Ok(self.frames[frame].composite(
            &self.layer_visible,
            &self.layer_opacity,
            &self.polish_flags(),
        ))
    }

    /// Composite, or one layer when `layer` is set.
    pub fn view_pixels(&self, frame: usize, layer: Option<usize>) -> Result<Vec<u8>, KernelError> {
        match layer {
            Some(layer) => Ok(self.layer_pixels(frame, layer)?.to_vec()),
            None => self.composite_pixels(frame),
        }
    }

    pub fn layer_count(&self) -> usize {
        self.layer_names.len()
    }

    pub fn active_layer(&self) -> usize {
        self.active_layer
    }

    /// The selection mask, or `None` when every cell is writable.
    ///
    /// One mask in canvas coordinates. It is not per layer and not per frame.
    /// The project file stores it and omits the field when it is clear.
    /// An empty mask is `Some` and protects every cell.
    pub fn selection(&self) -> Option<&Mask> {
        self.selection.as_ref()
    }

    /// The mask when it is exactly one rectangle.
    pub fn selection_rect(&self) -> Option<Selection> {
        self.selection.as_ref().and_then(Mask::as_rect)
    }

    /// Stores a rectangle that lies entirely on the canvas.
    ///
    /// The same rectangle is not recorded. This does not change
    /// [`Self::active_layer`] or any pixel. Later paint, erase, fill, copy,
    /// flip, and index replace skip cells outside it. A move whose source or
    /// destination leaves the rectangle is [`KernelError::SelectionProtected`].
    pub fn set_selection(
        &mut self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    ) -> Result<(), KernelError> {
        self.inclusive_rect(x, y, width, height)?;
        self.replace_mask(Mask::from_rect(Selection {
            x,
            y,
            width,
            height,
        }))
    }

    /// Replaces the stored selection with `base OP operand`.
    ///
    /// `base == None` uses the current selection. With no selection, `union`
    /// stores `operand` and `intersect` / `subtract` return
    /// [`KernelError::MaskMissing`]. The result is stored even when it covers
    /// no cells, so a later fill does not paint the whole canvas. The same
    /// mask is not recorded. This does not change pixels or the active layer.
    pub fn combine_mask(
        &mut self,
        op: MaskOp,
        base: Option<Mask>,
        operand: Mask,
    ) -> Result<(), KernelError> {
        if let Some(base) = &base {
            self.ensure_mask_fits(base)?;
        }
        self.ensure_mask_fits(&operand)?;
        let left = if let Some(base) = base {
            base
        } else if let Some(current) = &self.selection {
            current.clone()
        } else {
            return match op {
                MaskOp::Union => self.replace_mask(operand),
                MaskOp::Intersect | MaskOp::Subtract => Err(KernelError::MaskMissing),
            };
        };
        let result = match op {
            MaskOp::Union => left.union(&operand),
            MaskOp::Intersect => left.intersect(&operand),
            MaskOp::Subtract => left.subtract(&operand),
        };
        self.replace_mask(result)
    }

    /// Clears the mask so the whole canvas is writable again.
    ///
    /// A document that already has no selection is not recorded. This does not
    /// change pixels or the active layer. An empty mask is cleared too: empty
    /// protects every cell, and clear removes that protection.
    pub fn clear_selection(&mut self) {
        let Some(before) = self.selection.clone() else {
            return;
        };
        self.selection = None;
        self.history.record(Change::Selection {
            before: Some(before),
            after: None,
        });
    }

    fn ensure_mask_fits(&self, mask: &Mask) -> Result<(), KernelError> {
        for rect in mask.rects() {
            self.inclusive_rect(rect.x, rect.y, rect.width, rect.height)?;
        }
        Ok(())
    }

    fn replace_mask(&mut self, mask: Mask) -> Result<(), KernelError> {
        let after = Some(mask);
        if self.selection == after {
            return Ok(());
        }
        let before = self.selection.clone();
        self.selection = after.clone();
        self.history.record(Change::Selection { before, after });
        Ok(())
    }

    pub fn layer_names(&self) -> &[String] {
        &self.layer_names
    }

    pub fn layer_name(&self, index: usize) -> Result<&str, KernelError> {
        let index = self.layer_or_err(index)?;
        Ok(&self.layer_names[index])
    }

    /// Bake role of one layer. [`LayerRole::Polish`] is an overlay.
    pub fn layer_role(&self, index: usize) -> Result<LayerRole, KernelError> {
        let index = self.layer_or_err(index)?;
        Ok(self.layer_roles[index])
    }

    /// Bake role of every layer, bottom to top.
    pub fn layer_roles(&self) -> &[LayerRole] {
        &self.layer_roles
    }

    /// Part binding of one layer. `Ok(None)` is unbound canvas coordinates.
    pub fn layer_bind(&self, index: usize) -> Result<Option<&PolishBind>, KernelError> {
        let index = self.layer_or_err(index)?;
        Ok(self.layer_binds[index].as_ref())
    }

    /// Explicit-erase marks for one layer of one frame. `1` punches the composite.
    pub fn layer_erase(&self, frame: usize, layer: usize) -> Result<&[u8], KernelError> {
        let frame = self.frame_or_err(frame)?;
        let layer = self.layer_or_err(layer)?;
        Ok(self.frames[frame].layer_erase(layer))
    }

    pub(crate) fn frame_erase_buffers(&self, frame: usize) -> Result<&[Vec<u8>], KernelError> {
        let frame = self.frame_or_err(frame)?;
        Ok(self.frames[frame].erasures())
    }

    pub(crate) fn polish_flags(&self) -> Vec<bool> {
        self.layer_roles
            .iter()
            .map(|role| *role == LayerRole::Polish)
            .collect()
    }

    pub(crate) fn layer_visibility_flags(&self) -> &[bool] {
        &self.layer_visible
    }

    pub(crate) fn layer_opacity_values(&self) -> &[u8] {
        &self.layer_opacity
    }

    pub(crate) fn frame_layer_buffers(&self, frame: usize) -> Result<&[Vec<u8>], KernelError> {
        let frame = self.frame_or_err(frame)?;
        Ok(self.frames[frame].layers())
    }

    /// Whether this layer is included in the composite.
    ///
    /// `false` skips it in preview, diff, export, and flatten. The stored
    /// indices stay, and drawing this layer still writes them.
    pub fn layer_visible(&self, index: usize) -> Result<bool, KernelError> {
        let index = self.layer_or_err(index)?;
        Ok(self.layer_visible[index])
    }

    /// Composite opacity, `0..=255`. [`LAYER_OPACITY_OPAQUE`] is fully opaque.
    ///
    /// `0` skips the layer the same way a hidden layer is skipped. The visible
    /// flag stays whatever it was. `1..=254` still covers in
    /// [`Self::composite_pixels`] and blends in [`Self::frame_rgba`].
    pub fn layer_opacity(&self, index: usize) -> Result<u8, KernelError> {
        let index = self.layer_or_err(index)?;
        Ok(self.layer_opacity[index])
    }

    /// Shows or hides one layer in the composite. The same value is not recorded.
    ///
    /// Pixels stay. One undo step when the flag changes. A hidden layer is
    /// still a paint target.
    pub fn set_layer_visible(&mut self, index: usize, visible: bool) -> Result<(), KernelError> {
        let index = self.layer_or_err(index)?;
        if self.layer_visible[index] == visible {
            return Ok(());
        }
        let before = self.layer_visible[index];
        self.layer_visible[index] = visible;
        self.history.record(Change::LayerVisible {
            index,
            before,
            after: visible,
        });
        Ok(())
    }

    /// Sets one layer's composite opacity. The same value is not recorded.
    ///
    /// `opacity` is `0..=255`. Pixels stay. One undo step when the value
    /// changes. `0` skips the layer. [`LAYER_OPACITY_OPAQUE`] covers without
    /// blending. Any other value blends only in the RGBA composite.
    pub fn set_layer_opacity(&mut self, index: usize, opacity: u8) -> Result<(), KernelError> {
        let index = self.layer_or_err(index)?;
        if self.layer_opacity[index] == opacity {
            return Ok(());
        }
        let before = self.layer_opacity[index];
        self.layer_opacity[index] = opacity;
        self.history.record(Change::LayerOpacity {
            index,
            before,
            after: opacity,
        });
        Ok(())
    }

    pub fn get(&self, x: u16, y: u16) -> Result<u8, KernelError> {
        let offset = self.offset(x, y)?;
        Ok(self.pixels()[offset])
    }

    /// Writes a non-transparent palette index on the active layer. Index 0 is
    /// rejected; use [`Self::erase`].
    ///
    /// The cell stores `color_index`, not the entry's RGB. Recoloring that
    /// entry later changes how the cell is shown and exported. A cell that
    /// already holds `color_index` is left alone and is not recorded. A locked
    /// palette does not block this write. Other layers are left alone.
    pub fn paint(&mut self, x: u16, y: u16, color_index: u8) -> Result<(), KernelError> {
        self.paint_on(self.active_layer, x, y, color_index)
    }

    /// [`Self::paint`] on one layer. Does not change the active layer.
    pub fn paint_on(
        &mut self,
        layer: usize,
        x: u16,
        y: u16,
        color_index: u8,
    ) -> Result<(), KernelError> {
        let color_index = self.resolve_paint_index(color_index)?;
        let layer = self.layer_or_err(layer)?;
        let offset = self.offset(x, y)?;
        if !self.accepts_write(x, y) {
            return Ok(());
        }
        self.write_index(self.active, layer, offset, color_index);
        Ok(())
    }

    /// Replaces the RGB of a paint entry. Index 0 cannot be changed.
    ///
    /// Cells keep their indices, so every frame that uses `index` shows the
    /// new color. The same RGB is not recorded. A color-picker drag should
    /// sit inside one gesture; those writes fold into a single undo step.
    pub fn set_palette_color(&mut self, index: u8, r: u8, g: u8, b: u8) -> Result<(), KernelError> {
        self.ensure_palette_unlocked()?;
        let before = self.palette.set_rgb(index, r, g, b)?;
        let after = Rgba::rgb(r, g, b);
        if before == after {
            return Ok(());
        }
        self.history.record(Change::PaletteColor {
            index,
            before,
            after,
        });
        Ok(())
    }

    /// Appends an opaque color and returns its index. Paint can use it immediately.
    pub fn add_palette_color(&mut self, r: u8, g: u8, b: u8) -> Result<u8, KernelError> {
        self.ensure_palette_unlocked()?;
        let index = self.palette.push_rgb(r, g, b)?;
        self.history.record(Change::AddPaletteColor {
            index,
            color: Rgba::rgb(r, g, b),
        });
        Ok(index)
    }

    /// Locks or unlocks palette edits. Painting is unaffected.
    ///
    /// The same value is not recorded. The flag is stored in the project file.
    /// Undo and redo still apply palette operations that were already on the
    /// stack; the lock only refuses new recolors, appends, and imports.
    pub fn set_palette_locked(&mut self, locked: bool) {
        if self.palette_locked == locked {
            return;
        }
        let before = self.palette_locked;
        self.palette_locked = locked;
        self.history.record(Change::PaletteLocked {
            before,
            after: locked,
        });
    }

    /// Sets how out-of-range paint indices are treated.
    ///
    /// The same value is not recorded. The lock does not freeze this flag:
    /// it is not a palette color edit. `reject` is omitted from the project
    /// file. `clamp` is stored as `indexPolicy`.
    pub fn set_index_policy(&mut self, policy: IndexPolicy) {
        if self.index_policy == policy {
            return;
        }
        let before = self.index_policy;
        self.index_policy = policy;
        self.history.record(Change::IndexPolicy {
            before,
            after: policy,
        });
    }

    /// Reads a GIMP palette (`.gpl`) or a hex list into this document.
    ///
    /// See [`PaletteImportMode`]. Index 0 stays transparent, and every imported
    /// color is stored opaque. A locked palette is refused before the text is
    /// applied. A file that does not change the document is not recorded.
    /// [`PaletteImportMode::Replace`] that drops indices, and
    /// [`PaletteImportMode::Merge`] that appends several colors, are one undo
    /// step. Callers wrap this in a gesture only when it should join other edits.
    pub fn import_palette(
        &mut self,
        text: &str,
        mode: PaletteImportMode,
    ) -> Result<(), KernelError> {
        self.ensure_palette_unlocked()?;
        let imported = parse_palette_text(text)?;
        match mode {
            PaletteImportMode::Replace => self.replace_palette(imported),
            PaletteImportMode::Merge => self.merge_palette(imported),
        }
    }

    fn replace_palette(&mut self, imported: Vec<Rgba>) -> Result<(), KernelError> {
        let mut colors = Vec::with_capacity(imported.len() + 1);
        colors.push(Rgba::TRANSPARENT);
        colors.extend(imported);
        let checked = Palette::new(colors.clone())?;
        if checked.colors() == self.palette.colors() {
            return Ok(());
        }
        let mut changes = Vec::new();
        if colors.len() < self.palette.len() {
            let limit = u8::try_from(colors.len()).expect("palette length fits in u8");
            for (frame, pixels) in self.frames.iter().enumerate() {
                for (layer, buffer) in pixels.layers().iter().enumerate() {
                    for (offset, index) in buffer.iter().copied().enumerate() {
                        if index >= limit {
                            changes.push(Change::Pixel {
                                frame,
                                layer,
                                offset,
                                before: index,
                                after: TRANSPARENT_INDEX,
                            });
                        }
                    }
                }
            }
        }
        let mut cleared_marks = Vec::new();
        for change in &changes {
            let Change::Pixel {
                frame,
                layer,
                offset,
                after,
                ..
            } = change
            else {
                continue;
            };
            self.frames[*frame].set_on(*layer, *offset, *after);
            let before_mark = self.frames[*frame].layer_erase(*layer)[*offset];
            if before_mark != 0 {
                self.frames[*frame].set_erase(*layer, *offset, 0);
                cleared_marks.push(Change::EraseMark {
                    frame: *frame,
                    layer: *layer,
                    offset: *offset,
                    before: before_mark,
                    after: 0,
                });
            }
        }
        changes.extend(cleared_marks);
        let before = self.palette.colors().to_vec();
        self.palette = checked;
        changes.push(Change::PaletteReplace {
            before,
            after: colors,
        });
        self.history.record_group(changes);
        Ok(())
    }

    fn merge_palette(&mut self, imported: Vec<Rgba>) -> Result<(), KernelError> {
        let mut existing = self.palette.colors().to_vec();
        let mut additions = Vec::new();
        for color in imported {
            if existing.contains(&color) {
                continue;
            }
            existing.push(color);
            additions.push(color);
        }
        if additions.is_empty() {
            return Ok(());
        }
        if existing.len() > usize::from(u8::MAX) + 1 {
            return Err(KernelError::PaletteTooLarge {
                len: existing.len(),
            });
        }
        let mut changes = Vec::new();
        for color in additions {
            let index = self
                .palette
                .push_rgb(color.r, color.g, color.b)
                .expect("merged color fits");
            changes.push(Change::AddPaletteColor { index, color });
        }
        self.history.record_group(changes);
        Ok(())
    }

    /// Nearest-maps every stored index onto the colors in `text` and installs
    /// that palette. One undo step. A locked palette is refused.
    ///
    /// `text` is a GIMP palette or a hex list, the same formats as
    /// [`Self::import_palette`]. Unlike `replace` import, pixels are not
    /// cleared: each old color moves to the nearest target color. Index 0
    /// stays 0. Distance is squared RGB. A tie keeps the lower target index.
    /// The report lists colors that missed an exact RGB hit, merges (including
    /// exact index shifts), and how many cells changed index. The rectangular
    /// selection is not consulted.
    pub(crate) fn remap_palette_text(&mut self, text: &str) -> Result<MapReport, KernelError> {
        self.ensure_palette_unlocked()?;
        let imported = parse_palette_text(text)?;
        let counts = self.index_counts();
        let plan = palette_map::plan_target(self.palette.colors(), &counts, &imported)?;
        Ok(self.commit_palette_plan(plan))
    }

    /// Reduces the opaque palette to at most `max_colors` entries.
    ///
    /// Does not consult the rectangular selection. Use [`Self::replace_index`]
    /// to recolor one index inside the rectangle.
    ///
    /// Unused entries drop from the highest index first. Used entries then
    /// merge into the nearer existing color; that color's RGB is kept.
    /// `max_colors` does not count the transparent slot. A locked palette is
    /// refused. One undo step when the palette or any index changes.
    pub(crate) fn quantize_palette(&mut self, max_colors: u8) -> Result<MapReport, KernelError> {
        self.ensure_palette_unlocked()?;
        let counts = self.index_counts();
        let plan =
            palette_map::plan_max_colors(self.palette.colors(), &counts, usize::from(max_colors));
        Ok(self.commit_palette_plan(plan))
    }

    /// Writes one PNG into one layer by nearest color on the current palette.
    ///
    /// The PNG must be the canvas size. This does not scale, slice a sheet, or
    /// add palette colors. It does not consult the rectangular selection.
    /// Alpha below 128 becomes index 0. Any higher alpha
    /// is opaque and matched by squared RGB. A locked palette still allows
    /// the write, because the palette itself does not change. One undo step
    /// when an index changes. The active frame and layer stay put.
    pub(crate) fn apply_png(
        &mut self,
        frame: usize,
        layer: usize,
        png: &[u8],
    ) -> Result<MapReport, KernelError> {
        let frame = self.frame_or_err(frame)?;
        let layer = self.layer_or_err(layer)?;
        let (png_width, png_height, rgba) = match palette_map::decode_png(png)? {
            DecodedPng::TooBig { width, height } => {
                return Err(KernelError::PngSize {
                    png_width: width,
                    png_height: height,
                    width: self.width,
                    height: self.height,
                });
            }
            DecodedPng::Image {
                width,
                height,
                rgba,
            } => (width, height, rgba),
        };
        if png_width != u32::from(self.width) || png_height != u32::from(self.height) {
            return Err(KernelError::PngSize {
                png_width,
                png_height,
                width: self.width,
                height: self.height,
            });
        }
        let (indices, mut report) = palette_map::crush_rgba(self.palette.colors(), &rgba)?;
        let mut changes = Vec::new();
        let before = self.frames[frame].layer_pixels(layer);
        for (offset, (old, new)) in before
            .iter()
            .copied()
            .zip(indices.iter().copied())
            .enumerate()
        {
            if old != new {
                changes.push(Change::Pixel {
                    frame,
                    layer,
                    offset,
                    before: old,
                    after: new,
                });
            }
        }
        let mut cleared_marks = Vec::new();
        for change in &changes {
            let Change::Pixel {
                frame,
                layer,
                offset,
                after,
                ..
            } = change
            else {
                continue;
            };
            self.frames[*frame].set_on(*layer, *offset, *after);
            let before_mark = self.frames[*frame].layer_erase(*layer)[*offset];
            if before_mark != 0 {
                self.frames[*frame].set_erase(*layer, *offset, 0);
                cleared_marks.push(Change::EraseMark {
                    frame: *frame,
                    layer: *layer,
                    offset: *offset,
                    before: before_mark,
                    after: 0,
                });
            }
        }
        changes.extend(cleared_marks);
        report.pixels_changed = changes
            .iter()
            .filter(|change| matches!(change, Change::Pixel { .. }))
            .count();
        self.history.record_group(changes);
        Ok(report)
    }

    fn index_counts(&self) -> Vec<u32> {
        let mut counts = vec![0u32; self.palette.len()];
        for frame in &self.frames {
            for layer in frame.layers() {
                for index in layer.iter().copied() {
                    let slot = usize::from(index);
                    debug_assert!(slot < counts.len(), "stored index is outside the palette");
                    counts[slot] = counts[slot].saturating_add(1);
                }
            }
        }
        counts
    }

    fn commit_palette_plan(&mut self, plan: PalettePlan) -> MapReport {
        let mut changes = Vec::new();
        for (frame, pixels) in self.frames.iter().enumerate() {
            for (layer, buffer) in pixels.layers().iter().enumerate() {
                for (offset, index) in buffer.iter().copied().enumerate() {
                    let next = plan.map[usize::from(index)];
                    if next == index {
                        continue;
                    }
                    changes.push(Change::Pixel {
                        frame,
                        layer,
                        offset,
                        before: index,
                        after: next,
                    });
                }
            }
        }
        let mut cleared_marks = Vec::new();
        for change in &changes {
            let Change::Pixel {
                frame,
                layer,
                offset,
                after,
                ..
            } = change
            else {
                continue;
            };
            self.frames[*frame].set_on(*layer, *offset, *after);
            let before_mark = self.frames[*frame].layer_erase(*layer)[*offset];
            if before_mark != 0 {
                self.frames[*frame].set_erase(*layer, *offset, 0);
                cleared_marks.push(Change::EraseMark {
                    frame: *frame,
                    layer: *layer,
                    offset: *offset,
                    before: before_mark,
                    after: 0,
                });
            }
        }
        changes.extend(cleared_marks);
        let pixels_changed = changes
            .iter()
            .filter(|change| matches!(change, Change::Pixel { .. }))
            .count();
        if self.palette.colors() != plan.colors.as_slice() {
            let before = self.palette.colors().to_vec();
            self.palette.install(plan.colors.clone());
            changes.push(Change::PaletteReplace {
                before,
                after: plan.colors,
            });
        }
        self.history.record_group(changes);
        let mut report = plan.report;
        report.pixels_changed = pixels_changed;
        report
    }

    fn ensure_palette_unlocked(&self) -> Result<(), KernelError> {
        if self.palette_locked {
            Err(KernelError::PaletteLocked)
        } else {
            Ok(())
        }
    }

    /// Fills an axis-aligned rectangle on the active layer of the active frame
    /// with one palette index.
    ///
    /// `x` and `y` are the inclusive top-left cell. `width` and `height` are
    /// the size in cells. Every cell must lie on the canvas; a zero edge, an
    /// origin outside the canvas, or a far edge that falls outside is refused
    /// and writes nothing. The cell stores `color_index`, not RGB. Cells that
    /// already hold that index are skipped. The cells that do change are one
    /// undo step, or part of the open gesture when one is open. A fill that
    /// changes nothing is not recorded. A locked palette does not block this
    /// write. Index 0 is rejected; use [`Self::erase`] or [`Self::erase_rect`].
    pub fn fill_rect(
        &mut self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        color_index: u8,
    ) -> Result<(), KernelError> {
        self.fill_rect_on(self.active_layer, x, y, width, height, color_index)
    }

    /// [`Self::fill_rect`] on one layer. Does not change the active layer.
    pub fn fill_rect_on(
        &mut self,
        layer: usize,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        color_index: u8,
    ) -> Result<(), KernelError> {
        let color_index = self.resolve_paint_index(color_index)?;
        let layer = self.layer_or_err(layer)?;
        let (x1, y1) = self.inclusive_rect(x, y, width, height)?;
        let frame = self.active;
        let mut changes = Vec::new();
        for row in y..=y1 {
            let row_offset = pixel_len(self.width, row);
            for col in x..=x1 {
                if !self.accepts_write(col, row) {
                    continue;
                }
                let offset = row_offset + usize::from(col);
                let before = self.frames[frame].layer_pixels(layer)[offset];
                if before == color_index && self.frames[frame].layer_erase(layer)[offset] == 0 {
                    continue;
                }
                self.put_index(frame, layer, offset, color_index, &mut changes);
            }
        }
        self.history.record_group(changes);
        Ok(())
    }

    /// Sets an axis-aligned rectangle on the active frame to [`TRANSPARENT_INDEX`].
    ///
    /// Bounds match [`Self::fill_rect`]: a zero edge or any cell outside the
    /// canvas is refused and writes nothing. Cells that are already transparent
    /// are skipped. The cells that do change are one undo step, or part of the
    /// open gesture when one is open. A rectangle that changes nothing is not
    /// recorded. A locked palette does not block this write.
    pub fn erase_rect(
        &mut self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    ) -> Result<(), KernelError> {
        self.erase_rect_on(self.active_layer, x, y, width, height)
    }

    /// [`Self::erase_rect`] on one layer. Does not change the active layer.
    pub fn erase_rect_on(
        &mut self,
        layer: usize,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    ) -> Result<(), KernelError> {
        let layer = self.layer_or_err(layer)?;
        let (x1, y1) = self.inclusive_rect(x, y, width, height)?;
        let frame = self.active;
        let mut changes = Vec::new();
        for row in y..=y1 {
            let row_offset = pixel_len(self.width, row);
            for col in x..=x1 {
                if !self.accepts_write(col, row) {
                    continue;
                }
                let offset = row_offset + usize::from(col);
                self.put_erase(frame, layer, offset, &mut changes);
            }
        }
        self.history.record_group(changes);
        Ok(())
    }

    /// Draws a 1px line on the active layer of the active frame.
    ///
    /// Both endpoints must lie on the canvas. The line is Bresenham and
    /// 8-connected, so it has no gaps. `pixel_perfect` walks the longer axis
    /// one cell at a time and is still 8-connected. Cells store `color_index`,
    /// not RGB. Cells that already hold that index are skipped. The cells that
    /// change are one undo step, or part of the open gesture. A line that
    /// changes nothing is not recorded. A locked palette does not block this
    /// write. Index 0 is rejected. A selection skips cells outside the mask
    /// and still requires both endpoints on the canvas.
    pub fn line(
        &mut self,
        x0: u16,
        y0: u16,
        x1: u16,
        y1: u16,
        color_index: u8,
        pixel_perfect: bool,
    ) -> Result<(), KernelError> {
        self.line_on(
            self.active_layer,
            x0,
            y0,
            x1,
            y1,
            color_index,
            pixel_perfect,
        )
    }

    /// [`Self::line`] on one layer. Does not change the active layer.
    pub fn line_on(
        &mut self,
        layer: usize,
        x0: u16,
        y0: u16,
        x1: u16,
        y1: u16,
        color_index: u8,
        pixel_perfect: bool,
    ) -> Result<(), KernelError> {
        let color_index = self.resolve_paint_index(color_index)?;
        let layer = self.layer_or_err(layer)?;
        self.offset(x0, y0)?;
        self.offset(x1, y1)?;
        let cells = if pixel_perfect {
            raster::line_perfect(i32::from(x0), i32::from(y0), i32::from(x1), i32::from(y1))
        } else {
            raster::line_continuous(i32::from(x0), i32::from(y0), i32::from(x1), i32::from(y1))
        };
        let cells = self.canvas_cells(&cells)?;
        self.stamp_cells(layer, color_index, &cells)
    }

    /// Draws an ellipse inscribed in an axis-aligned rectangle.
    ///
    /// Bounds match [`Self::fill_rect`]. `fill` paints the interior as well as
    /// the outline. A width or height of 1 is that line, filled or not. One
    /// undo step. A locked palette does not block this write. Index 0 is
    /// rejected. A selection skips cells outside the mask.
    pub fn ellipse(
        &mut self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        color_index: u8,
        fill: bool,
    ) -> Result<(), KernelError> {
        self.ellipse_on(self.active_layer, x, y, width, height, color_index, fill)
    }

    /// [`Self::ellipse`] on one layer. Does not change the active layer.
    pub fn ellipse_on(
        &mut self,
        layer: usize,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        color_index: u8,
        fill: bool,
    ) -> Result<(), KernelError> {
        let color_index = self.resolve_paint_index(color_index)?;
        let layer = self.layer_or_err(layer)?;
        let _ = self.inclusive_rect(x, y, width, height)?;
        let cells = raster::ellipse_cells(
            i32::from(x),
            i32::from(y),
            i32::from(width),
            i32::from(height),
            fill,
        );
        let cells = self.canvas_cells(&cells)?;
        self.stamp_cells(layer, color_index, &cells)
    }

    /// Draws a closed polygon through `points` on the active layer.
    ///
    /// At least three vertices are required. Every vertex must lie on the
    /// canvas. The outline uses the same continuous line as [`Self::line`].
    /// `fill` adds the even-odd interior. One undo step. A locked palette
    /// does not block this write. Index 0 is rejected. A selection skips
    /// cells outside the mask.
    pub fn polygon(
        &mut self,
        points: &[(u16, u16)],
        color_index: u8,
        fill: bool,
    ) -> Result<(), KernelError> {
        self.polygon_on(self.active_layer, points, color_index, fill)
    }

    /// [`Self::polygon`] on one layer. Does not change the active layer.
    pub fn polygon_on(
        &mut self,
        layer: usize,
        points: &[(u16, u16)],
        color_index: u8,
        fill: bool,
    ) -> Result<(), KernelError> {
        let color_index = self.resolve_paint_index(color_index)?;
        let layer = self.layer_or_err(layer)?;
        if points.len() < 3 {
            return Err(KernelError::PolygonPoints);
        }
        for &(x, y) in points {
            self.offset(x, y)?;
        }
        let raw: Vec<(i32, i32)> = points
            .iter()
            .map(|&(x, y)| (i32::from(x), i32::from(y)))
            .collect();
        let cells = raster::polygon_cells(&raw, fill);
        let cells = self.canvas_cells(&cells)?;
        self.stamp_cells(layer, color_index, &cells)
    }

    /// Replaces the connected region of the seed cell's index.
    ///
    /// The seed must lie on the canvas. Connectivity is 4 or 8. Cells outside
    /// the selection are barriers and are not written. The same index is not
    /// recorded. One undo step. A locked palette does not block this write.
    /// Index 0 is rejected as the new color. The seed's current index, including
    /// 0, is the region that changes.
    pub fn flood_fill(
        &mut self,
        x: u16,
        y: u16,
        color_index: u8,
        connectivity: Connectivity,
    ) -> Result<(), KernelError> {
        self.flood_fill_on(self.active_layer, x, y, color_index, connectivity)
    }

    /// [`Self::flood_fill`] on one layer. Does not change the active layer.
    pub fn flood_fill_on(
        &mut self,
        layer: usize,
        x: u16,
        y: u16,
        color_index: u8,
        connectivity: Connectivity,
    ) -> Result<(), KernelError> {
        let color_index = self.resolve_paint_index(color_index)?;
        let layer = self.layer_or_err(layer)?;
        let origin = self.offset(x, y)?;
        if !self.accepts_write(x, y) {
            return Ok(());
        }
        let frame = self.active;
        let source = self.frames[frame].layer_pixels(layer)[origin];
        if source == color_index {
            return Ok(());
        }
        let count = self.pixel_count();
        let mut writable = vec![false; count];
        for row in 0..self.height {
            for col in 0..self.width {
                if self.accepts_write(col, row) {
                    writable[pixel_len(self.width, row) + usize::from(col)] = true;
                }
            }
        }
        let cells = raster::flood_cells(
            self.frames[frame].layer_pixels(layer),
            self.width,
            self.height,
            x,
            y,
            source,
            connectivity,
            &writable,
        );
        self.stamp_cells(layer, color_index, &cells)
    }

    /// Draws the border of an axis-aligned rectangle.
    ///
    /// Bounds match [`Self::fill_rect`]. A width or height of 1 is that whole
    /// rectangle. The interior stays. One undo step. A locked palette does not
    /// block this write. Index 0 is rejected. A selection skips cells outside
    /// the mask.
    pub fn stroke_rect(
        &mut self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        color_index: u8,
    ) -> Result<(), KernelError> {
        self.stroke_rect_on(self.active_layer, x, y, width, height, color_index)
    }

    /// [`Self::stroke_rect`] on one layer. Does not change the active layer.
    pub fn stroke_rect_on(
        &mut self,
        layer: usize,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        color_index: u8,
    ) -> Result<(), KernelError> {
        let color_index = self.resolve_paint_index(color_index)?;
        let layer = self.layer_or_err(layer)?;
        let _ = self.inclusive_rect(x, y, width, height)?;
        let cells = raster::stroke_rect_cells(
            i32::from(x),
            i32::from(y),
            i32::from(width),
            i32::from(height),
        );
        let cells = self.canvas_cells(&cells)?;
        self.stamp_cells(layer, color_index, &cells)
    }

    /// Copies palette indices from one rectangle to another on the active frame.
    ///
    /// `src_x` and `src_y` are the source top-left. `dst_x` and `dst_y` are the
    /// destination top-left. `width` and `height` are the size in cells. Index
    /// 0 is copied as transparent. The source stays. Both rectangles must lie
    /// entirely on the canvas; if either would fall outside, nothing is written.
    /// Overlap reads the source before any destination cell is written. The
    /// cells that change are one undo step, or part of the open gesture. A copy
    /// that changes nothing is not recorded. A locked palette does not block
    /// this write. The active frame is both the source and the destination.
    pub fn copy_rect(
        &mut self,
        src_x: u16,
        src_y: u16,
        width: u16,
        height: u16,
        dst_x: u16,
        dst_y: u16,
    ) -> Result<(), KernelError> {
        let frame = self.active;
        self.copy_rect_from_on(
            self.active_layer,
            frame,
            src_x,
            src_y,
            width,
            height,
            frame,
            dst_x,
            dst_y,
        )
    }

    /// Copies indices from `src_frame` onto `dst_frame`. Does not change the
    /// active frame. Same bounds, overlap, and undo rules as [`Self::copy_rect`].
    ///
    /// When the frames differ there is no overlap: the source rectangle is read
    /// as it is, and the destination rectangle is replaced, including index 0.
    pub fn copy_rect_from(
        &mut self,
        src_frame: usize,
        src_x: u16,
        src_y: u16,
        width: u16,
        height: u16,
        dst_frame: usize,
        dst_x: u16,
        dst_y: u16,
    ) -> Result<(), KernelError> {
        self.copy_rect_from_on(
            self.active_layer,
            src_frame,
            src_x,
            src_y,
            width,
            height,
            dst_frame,
            dst_x,
            dst_y,
        )
    }

    /// [`Self::copy_rect_from`] on one layer of both frames. Does not change
    /// the active layer. The copy does not read or write any other layer.
    pub fn copy_rect_from_on(
        &mut self,
        layer: usize,
        src_frame: usize,
        src_x: u16,
        src_y: u16,
        width: u16,
        height: u16,
        dst_frame: usize,
        dst_x: u16,
        dst_y: u16,
    ) -> Result<(), KernelError> {
        self.blit_between(
            layer, src_frame, src_x, src_y, width, height, dst_frame, dst_x, dst_y, false,
        )
    }

    /// Moves palette indices from one rectangle to another on the active frame.
    ///
    /// Same bounds and overlap rules as [`Self::copy_rect`]. Destination cells
    /// receive the source indices, including 0. Source cells that are not also
    /// in the destination become [`TRANSPARENT_INDEX`]. The whole move is one
    /// undo step, or part of the open gesture. A move onto itself changes
    /// nothing and is not recorded. A locked palette does not block this write.
    pub fn move_rect(
        &mut self,
        src_x: u16,
        src_y: u16,
        width: u16,
        height: u16,
        dst_x: u16,
        dst_y: u16,
    ) -> Result<(), KernelError> {
        let frame = self.active;
        self.move_rect_from_on(
            self.active_layer,
            frame,
            src_x,
            src_y,
            width,
            height,
            frame,
            dst_x,
            dst_y,
        )
    }

    /// Moves indices from `src_frame` onto `dst_frame`. Does not change the
    /// active frame. Source cells that are not part of the destination become
    /// [`TRANSPARENT_INDEX`]. Across frames the whole source rectangle is
    /// cleared, because the destination lives on another frame.
    pub fn move_rect_from(
        &mut self,
        src_frame: usize,
        src_x: u16,
        src_y: u16,
        width: u16,
        height: u16,
        dst_frame: usize,
        dst_x: u16,
        dst_y: u16,
    ) -> Result<(), KernelError> {
        self.move_rect_from_on(
            self.active_layer,
            src_frame,
            src_x,
            src_y,
            width,
            height,
            dst_frame,
            dst_x,
            dst_y,
        )
    }

    /// [`Self::move_rect_from`] on one layer of both frames. Does not change
    /// the active layer.
    pub fn move_rect_from_on(
        &mut self,
        layer: usize,
        src_frame: usize,
        src_x: u16,
        src_y: u16,
        width: u16,
        height: u16,
        dst_frame: usize,
        dst_x: u16,
        dst_y: u16,
    ) -> Result<(), KernelError> {
        self.blit_between(
            layer, src_frame, src_x, src_y, width, height, dst_frame, dst_x, dst_y, true,
        )
    }

    /// Mirrors palette indices left to right inside a rectangle.
    ///
    /// Cell `x` receives the index that was at `origin_x + width - 1 - (x - origin_x)`.
    /// On a full frame of width 64 that is `63 - x`. Palette colors are not
    /// changed. The rectangle must lie on the canvas. The mirror is one undo
    /// step. A symmetric rectangle is not recorded. `frame` is not selected.
    pub fn flip_horizontal(
        &mut self,
        frame: usize,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    ) -> Result<(), KernelError> {
        self.flip_horizontal_on(self.active_layer, frame, x, y, width, height)
    }

    /// [`Self::flip_horizontal`] on one layer. Does not change the active layer.
    pub fn flip_horizontal_on(
        &mut self,
        layer: usize,
        frame: usize,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    ) -> Result<(), KernelError> {
        self.transform_on(layer, frame, x, y, width, height, Turn::Horizontal)
    }

    /// Mirrors palette indices top to bottom inside a rectangle.
    ///
    /// Cell `y` receives the index that was at `origin_y + height - 1 - (y - origin_y)`.
    /// Palette colors are not changed. The rectangle must lie on the canvas.
    /// The mirror is one undo step. A symmetric rectangle is not recorded.
    /// `frame` is not selected. A cell is written only when it and the cell it
    /// reads are both inside the selection.
    pub fn flip_vertical(
        &mut self,
        frame: usize,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    ) -> Result<(), KernelError> {
        self.flip_vertical_on(self.active_layer, frame, x, y, width, height)
    }

    /// [`Self::flip_vertical`] on one layer. Does not change the active layer.
    pub fn flip_vertical_on(
        &mut self,
        layer: usize,
        frame: usize,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    ) -> Result<(), KernelError> {
        self.transform_on(layer, frame, x, y, width, height, Turn::Vertical)
    }

    /// Turns palette indices clockwise inside a rectangle.
    ///
    /// 180° accepts any rectangle. 90° and 270° need a square; the canvas size
    /// stays, and a non-square rectangle is [`KernelError::RotateSquare`] with
    /// nothing written. Palette colors are not changed. One undo step. `frame`
    /// is not selected. A cell is written only when it and the cell it reads
    /// are both inside the selection.
    pub fn rotate(
        &mut self,
        frame: usize,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        turn: QuarterTurn,
    ) -> Result<(), KernelError> {
        self.rotate_on(self.active_layer, frame, x, y, width, height, turn)
    }

    /// [`Self::rotate`] on one layer. Does not change the active layer.
    pub fn rotate_on(
        &mut self,
        layer: usize,
        frame: usize,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        turn: QuarterTurn,
    ) -> Result<(), KernelError> {
        let turn = match turn {
            QuarterTurn::Cw90 => Turn::Cw90,
            QuarterTurn::Cw180 => Turn::Cw180,
            QuarterTurn::Cw270 => Turn::Cw270,
        };
        self.transform_on(layer, frame, x, y, width, height, turn)
    }

    fn transform_on(
        &mut self,
        layer: usize,
        frame: usize,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        turn: Turn,
    ) -> Result<(), KernelError> {
        let layer = self.layer_or_err(layer)?;
        let frame = self.frame_or_err(frame)?;
        let (x1, y1) = self.inclusive_rect(x, y, width, height)?;
        if matches!(turn, Turn::Cw90 | Turn::Cw270) && width != height {
            return Err(KernelError::RotateSquare { width, height });
        }
        let mut sample = Vec::with_capacity(usize::from(width) * usize::from(height));
        {
            let pixels = self.frames[frame].layer_pixels(layer);
            for row in y..=y1 {
                let row_offset = pixel_len(self.width, row);
                for col in x..=x1 {
                    sample.push(pixels[row_offset + usize::from(col)]);
                }
            }
        }
        let mut changes = Vec::new();
        let span = usize::from(width);
        for row in y..=y1 {
            let local_y = row - y;
            let row_offset = pixel_len(self.width, row);
            for col in x..=x1 {
                let local_x = col - x;
                let (src_x, src_y) = source_local(turn, width, height, local_x, local_y);
                let read_col = x + src_x;
                let read_row = y + src_y;
                if !self.accepts_write(col, row) || !self.accepts_write(read_col, read_row) {
                    continue;
                }
                let after = sample[usize::from(src_y) * span + usize::from(src_x)];
                let offset = row_offset + usize::from(col);
                let before = self.frames[frame].layer_pixels(layer)[offset];
                if before == after && self.frames[frame].layer_erase(layer)[offset] == 0 {
                    continue;
                }
                self.put_index(frame, layer, offset, after, &mut changes);
            }
        }
        self.history.record_group(changes);
        Ok(())
    }

    /// Places every frame's indices onto a new canvas.
    ///
    /// `offset_x` and `offset_y` are where the old origin lands on the new
    /// canvas. They may be negative, which clips the old left or top edge.
    /// Cells with no source stay [`TRANSPARENT_INDEX`]. Content that falls
    /// outside the new canvas is dropped. Delays, the palette, the lock, and
    /// the active frame stay. The project file stays version 1; only `width`,
    /// `height`, and each frame's `pixels` change. One undo step restores the
    /// previous buffers. A resize that leaves every index where it was is not
    /// recorded.
    pub fn resize_canvas(
        &mut self,
        width: u16,
        height: u16,
        offset_x: i32,
        offset_y: i32,
    ) -> Result<(), KernelError> {
        check_size(width, height)?;
        let before_pixels: Vec<Vec<Vec<u8>>> = self
            .frames
            .iter()
            .map(|frame| frame.layers().to_vec())
            .collect();
        let before_erase: Vec<Vec<Vec<u8>>> = self
            .frames
            .iter()
            .map(|frame| frame.erasures().to_vec())
            .collect();
        let place = |buffers: &[Vec<u8>]| {
            buffers
                .iter()
                .map(|pixels| {
                    place_pixels(
                        pixels,
                        self.width,
                        self.height,
                        width,
                        height,
                        offset_x,
                        offset_y,
                    )
                })
                .collect::<Vec<_>>()
        };
        let after_pixels: Vec<Vec<Vec<u8>>> =
            before_pixels.iter().map(|layers| place(layers)).collect();
        let after_erase: Vec<Vec<Vec<u8>>> =
            before_erase.iter().map(|layers| place(layers)).collect();
        let before_selection = self.selection.clone();
        let after_selection = self
            .selection
            .as_ref()
            .and_then(|sel| sel.translated(offset_x, offset_y, width, height));
        let pixels_same = self.width == width
            && self.height == height
            && before_pixels == after_pixels
            && before_erase == after_erase;
        let selection_same = before_selection == after_selection;
        let before_parts = self.parts.clone();
        let before_curves = self.curves.clone();
        let before_constraints = self.constraints.clone();
        let after_parts = parts_after_resize(&before_parts, offset_x, offset_y, width, height);
        let after_curves = curves_for_parts(&before_curves, &after_parts);
        let after_constraints = constraints_after_resize(
            &before_constraints,
            &after_parts,
            offset_x,
            offset_y,
            width,
            height,
        );
        let parts_same = before_parts == after_parts;
        let curves_same = before_curves == after_curves;
        let constraints_same = before_constraints == after_constraints;
        if pixels_same && selection_same && parts_same && curves_same && constraints_same {
            return Ok(());
        }
        let before_width = self.width;
        let before_height = self.height;
        let mut changes = Vec::new();
        if !pixels_same {
            self.width = width;
            self.height = height;
            for (frame, (pixels, erase)) in self.frames.iter_mut().zip(
                after_pixels
                    .iter()
                    .cloned()
                    .zip(after_erase.iter().cloned()),
            ) {
                frame.set_stack(pixels, erase);
            }
            changes.push(Change::CanvasResize {
                before_width,
                before_height,
                after_width: width,
                after_height: height,
                before_pixels,
                after_pixels,
                before_erase,
                after_erase,
            });
        }
        if !selection_same {
            self.selection = after_selection.clone();
            changes.push(Change::Selection {
                before: before_selection,
                after: after_selection,
            });
        }
        if !parts_same {
            self.parts = after_parts.clone();
            changes.push(Change::Parts {
                before: before_parts,
                after: after_parts,
            });
        }
        if !curves_same {
            self.curves = after_curves.clone();
            changes.push(Change::Curves {
                before: before_curves,
                after: after_curves,
            });
        }
        if !constraints_same {
            self.constraints = after_constraints.clone();
            changes.push(Change::Constraints {
                before: before_constraints,
                after: after_constraints,
            });
        }
        self.history.record_group(changes);
        Ok(())
    }

    /// Copies `width` by `height` indices from one frame's rectangle to another.
    ///
    /// When `clear_source` is set, source cells outside the destination are
    /// cleared to [`TRANSPARENT_INDEX`] in the same undo step. Across frames
    /// the destination never covers the source, so the whole source rectangle
    /// is cleared. A selection skips destination cells outside it on a copy.
    /// A move is refused unless both rectangles lie inside the selection.
    fn blit_between(
        &mut self,
        layer: usize,
        src_frame: usize,
        src_x: u16,
        src_y: u16,
        width: u16,
        height: u16,
        dst_frame: usize,
        dst_x: u16,
        dst_y: u16,
        clear_source: bool,
    ) -> Result<(), KernelError> {
        let layer = self.layer_or_err(layer)?;
        let src_frame = self.frame_or_err(src_frame)?;
        let dst_frame = self.frame_or_err(dst_frame)?;
        let (src_x1, src_y1) = self.inclusive_rect(src_x, src_y, width, height)?;
        let (dst_x1, dst_y1) = self.inclusive_rect(dst_x, dst_y, width, height)?;
        if clear_source
            && (!self.rect_inside_selection(src_x, src_y, width, height)
                || !self.rect_inside_selection(dst_x, dst_y, width, height))
        {
            return Err(KernelError::SelectionProtected);
        }
        let mut sample = Vec::with_capacity(usize::from(width) * usize::from(height));
        {
            let pixels = self.frames[src_frame].layer_pixels(layer);
            for row in src_y..=src_y1 {
                let row_offset = pixel_len(self.width, row);
                for col in src_x..=src_x1 {
                    sample.push(pixels[row_offset + usize::from(col)]);
                }
            }
        }

        let count = self.pixel_count();
        let mut src_plan = vec![None; count];
        let mut dst_plan = if src_frame == dst_frame {
            None
        } else {
            Some(vec![None; count])
        };
        if clear_source {
            for row in src_y..=src_y1 {
                for col in src_x..=src_x1 {
                    if src_frame == dst_frame
                        && col >= dst_x
                        && col <= dst_x1
                        && row >= dst_y
                        && row <= dst_y1
                    {
                        continue;
                    }
                    let offset = pixel_len(self.width, row) + usize::from(col);
                    src_plan[offset] = Some(TRANSPARENT_INDEX);
                }
            }
        }
        let mut sample_at = 0;
        if src_frame == dst_frame {
            for row in dst_y..=dst_y1 {
                let row_offset = pixel_len(self.width, row);
                for col in dst_x..=dst_x1 {
                    let offset = row_offset + usize::from(col);
                    if self.accepts_write(col, row) {
                        src_plan[offset] = Some(sample[sample_at]);
                    }
                    sample_at += 1;
                }
            }
        } else {
            let plan = dst_plan.as_mut().expect("cross-frame blit has a dest plan");
            for row in dst_y..=dst_y1 {
                let row_offset = pixel_len(self.width, row);
                for col in dst_x..=dst_x1 {
                    let offset = row_offset + usize::from(col);
                    if self.accepts_write(col, row) {
                        plan[offset] = Some(sample[sample_at]);
                    }
                    sample_at += 1;
                }
            }
        }

        let mut changes = Vec::new();
        self.push_plan(src_frame, layer, &src_plan, &mut changes);
        if let Some(plan) = &dst_plan {
            self.push_plan(dst_frame, layer, plan, &mut changes);
        }
        self.history.record_group(changes);
        Ok(())
    }

    fn push_plan(
        &mut self,
        frame: usize,
        layer: usize,
        plan: &[Option<u8>],
        changes: &mut Vec<Change>,
    ) {
        for (offset, after) in plan.iter().enumerate() {
            let Some(after) = after else {
                continue;
            };
            self.put_index(frame, layer, offset, *after, changes);
        }
    }

    fn push_marks(&mut self, frame: usize, layer: usize, marks: &[u8], changes: &mut Vec<Change>) {
        for (offset, mark) in marks.iter().enumerate() {
            self.put_mark(frame, layer, offset, *mark, changes);
        }
    }

    fn layer_or_err(&self, index: usize) -> Result<usize, KernelError> {
        if index < self.layer_names.len() {
            Ok(index)
        } else {
            Err(KernelError::LayerIndex {
                index,
                len: self.layer_names.len(),
            })
        }
    }

    fn frame_or_err(&self, index: usize) -> Result<usize, KernelError> {
        if index < self.frames.len() {
            Ok(index)
        } else {
            Err(KernelError::FrameIndex {
                index,
                len: self.frames.len(),
            })
        }
    }

    /// Inclusive bottom-right of a rectangle that lies entirely on the canvas.
    ///
    /// A zero edge or a span that does not fit is [`KernelError::OutOfBounds`]
    /// and names a cell that is not on the canvas when one exists.
    pub(crate) fn inclusive_rect(
        &self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    ) -> Result<(u16, u16), KernelError> {
        let outside = |x: u16, y: u16| KernelError::OutOfBounds {
            x,
            y,
            width: self.width,
            height: self.height,
        };
        if width == 0 || height == 0 {
            return Err(outside(x, y));
        }
        let x1 = x.checked_add(width - 1);
        let y1 = y.checked_add(height - 1);
        let x_ok = x1.is_some_and(|end| x < self.width && end < self.width);
        let y_ok = y1.is_some_and(|end| y < self.height && end < self.height);
        if x_ok && y_ok {
            return Ok((
                x1.expect("x span was checked"),
                y1.expect("y span was checked"),
            ));
        }
        let bad_x = if x >= self.width { x } else { self.width };
        let bad_y = if y >= self.height {
            y
        } else if y1.is_some_and(|end| end < self.height) {
            y
        } else {
            self.height
        };
        Err(outside(bad_x, bad_y))
    }

    /// Sets one cell back to [`TRANSPARENT_INDEX`].
    ///
    /// An empty cell is left alone and is not recorded.
    pub fn erase(&mut self, x: u16, y: u16) -> Result<(), KernelError> {
        self.erase_on(self.active_layer, x, y)
    }

    /// [`Self::erase`] on one layer. Does not change the active layer.
    pub fn erase_on(&mut self, layer: usize, x: u16, y: u16) -> Result<(), KernelError> {
        let layer = self.layer_or_err(layer)?;
        let offset = self.offset(x, y)?;
        if !self.accepts_write(x, y) {
            return Ok(());
        }
        let mut changes = Vec::new();
        self.put_erase(self.active, layer, offset, &mut changes);
        self.history.record_group(changes);
        Ok(())
    }

    /// Clears one cell back to pass-through: index 0 and no explicit erase.
    ///
    /// On a default layer this is the same write as [`Self::erase_on`]. On a
    /// polish layer it removes a paint or an explicit erase and lets the
    /// layers underneath show. An already-clear cell is not recorded.
    pub fn clear_overlay_on(&mut self, layer: usize, x: u16, y: u16) -> Result<(), KernelError> {
        let layer = self.layer_or_err(layer)?;
        let offset = self.offset(x, y)?;
        if !self.accepts_write(x, y) {
            return Ok(());
        }
        let mut changes = Vec::new();
        self.put_index(self.active, layer, offset, TRANSPARENT_INDEX, &mut changes);
        self.history.record_group(changes);
        Ok(())
    }

    /// [`Self::clear_overlay_on`] for a rectangle. Cells outside the selection are skipped.
    pub fn clear_overlay_rect_on(
        &mut self,
        layer: usize,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    ) -> Result<(), KernelError> {
        let layer = self.layer_or_err(layer)?;
        let (x1, y1) = self.inclusive_rect(x, y, width, height)?;
        let frame = self.active;
        let mut changes = Vec::new();
        for row in y..=y1 {
            let row_offset = pixel_len(self.width, row);
            for col in x..=x1 {
                if !self.accepts_write(col, row) {
                    continue;
                }
                let offset = row_offset + usize::from(col);
                self.put_index(frame, layer, offset, TRANSPARENT_INDEX, &mut changes);
            }
        }
        self.history.record_group(changes);
        Ok(())
    }

    /// Replaces every `from_index` on the active layer of the active frame.
    ///
    /// A selection limits the replace to that rectangle. `to_index` may be 0,
    /// which clears those cells. Any other `to_index` is resolved like
    /// [`Self::paint`]. `from_index` must already be stored in the palette,
    /// including 0. Does not change the active layer. Cells that already hold
    /// the resolved index are skipped. The cells that change are one undo step.
    pub fn replace_index(&mut self, from_index: u8, to_index: u8) -> Result<u32, KernelError> {
        self.replace_index_on(self.active_layer, from_index, to_index)
    }

    /// [`Self::replace_index`] on one layer. Does not change the active layer.
    pub fn replace_index_on(
        &mut self,
        layer: usize,
        from_index: u8,
        to_index: u8,
    ) -> Result<u32, KernelError> {
        self.stored_index(from_index)?;
        let to_index = self.resolve_replace_index(to_index)?;
        let layer = self.layer_or_err(layer)?;
        let frame = self.active;
        let pixels = self.frames[frame].layer_pixels(layer).to_vec();
        let mut changes = Vec::new();
        for (offset, before) in pixels.into_iter().enumerate() {
            if before != from_index || before == to_index {
                continue;
            }
            let x = u16::try_from(offset % usize::from(self.width)).expect("x fits in u16");
            let y = u16::try_from(offset / usize::from(self.width)).expect("y fits in u16");
            if !self.accepts_write(x, y) {
                continue;
            }
            self.put_index(frame, layer, offset, to_index, &mut changes);
        }
        let changed = changes
            .iter()
            .filter(|change| matches!(change, Change::Pixel { .. }))
            .count();
        let changed = u32::try_from(changed).expect("changed cells fit in u32");
        self.history.record_group(changes);
        Ok(changed)
    }

    /// Appends a blank frame and returns its index. The active frame stays put.
    ///
    /// A tag that already ends before the new frame stays put, so appending
    /// does not grow a tag.
    pub fn add_frame(&mut self) -> usize {
        let index = self.frames.len();
        let before_tags = self.tags.clone();
        let before_curves = self.curves.clone();
        let before_constraints = self.constraints.clone();
        self.frames.push(self.blank_frame());
        self.tags = tags_after_insert(&before_tags, index);
        self.curves = curves_after_insert(&before_curves, index);
        self.constraints = constraints_after_insert(&before_constraints, index);
        self.record_frame_change(
            before_tags,
            before_curves,
            before_constraints,
            Change::AddFrame { index },
        );
        index
    }

    /// Inserts a copy after `index` and returns the new index.
    ///
    /// Does not select the copy. The active frame keeps showing the same
    /// pixels: an active index at or after the insertion point increases by
    /// one, and an earlier active index stays put. Call
    /// [`Self::set_active_frame`] with the returned index to edit the copy.
    ///
    /// The copy is a new pixel buffer, not a linked cel. A tag that starts
    /// before the insertion point and already covers it grows by one. A tag
    /// that ends on the source frame does not take the copy, because the copy
    /// is inserted after that end.
    pub fn duplicate_frame(&mut self, index: usize) -> Result<usize, KernelError> {
        let frame = self
            .frames
            .get(index)
            .cloned()
            .ok_or(KernelError::FrameIndex {
                index,
                len: self.frames.len(),
            })?;
        let inserted = index + 1;
        let active_before = self.active;
        let before_tags = self.tags.clone();
        let before_curves = self.curves.clone();
        let before_constraints = self.constraints.clone();
        self.frames.insert(inserted, frame.clone());
        if self.active >= inserted {
            self.active += 1;
        }
        self.tags = tags_after_insert(&before_tags, inserted);
        self.curves = curves_after_insert(&before_curves, inserted);
        self.constraints = constraints_after_insert(&before_constraints, inserted);
        self.record_frame_change(
            before_tags,
            before_curves,
            before_constraints,
            Change::InsertFrame {
                index: inserted,
                frame,
                active_before,
                active_after: self.active,
            },
        );
        Ok(inserted)
    }

    /// Removes a frame. The last remaining frame cannot be deleted.
    pub fn delete_frame(&mut self, index: usize) -> Result<(), KernelError> {
        if self.frames.len() == 1 {
            return Err(KernelError::LastFrame);
        }
        if index >= self.frames.len() {
            return Err(KernelError::FrameIndex {
                index,
                len: self.frames.len(),
            });
        }
        let active_before = self.active;
        let before_tags = self.tags.clone();
        let before_curves = self.curves.clone();
        let before_constraints = self.constraints.clone();
        let frame = self.frames.remove(index);
        self.tags = tags_after_remove(&before_tags, index);
        self.curves = curves_after_remove(&before_curves, index);
        self.constraints = constraints_after_remove(&before_constraints, index);
        self.active = match active_before.cmp(&index) {
            Ordering::Greater => active_before - 1,
            Ordering::Equal => index.min(self.frames.len() - 1),
            Ordering::Less => active_before,
        };
        self.clamp_elapsed();
        self.record_frame_change(
            before_tags,
            before_curves,
            before_constraints,
            Change::RemoveFrame {
                index,
                frame,
                active_before,
                active_after: self.active,
            },
        );
        Ok(())
    }

    /// Selects a frame. The same index is not recorded. Switching frames is an
    /// undoable kernel operation so later undos return to the frame that was
    /// being edited.
    pub fn set_active_frame(&mut self, index: usize) -> Result<(), KernelError> {
        if index >= self.frames.len() {
            return Err(KernelError::FrameIndex {
                index,
                len: self.frames.len(),
            });
        }
        if index == self.active {
            return Ok(());
        }
        let before = self.active;
        self.active = index;
        self.playback.elapsed_ms = 0;
        self.history.record(Change::ActiveFrame {
            before,
            after: index,
        });
        Ok(())
    }

    /// Inserts a transparent layer above the active one and selects it.
    ///
    /// The same layer is added to every frame. `name` is trimmed. Omit it and
    /// the name is the first free `Layer N`. The role is [`LayerRole::Default`].
    /// The insert is one undo step.
    pub fn add_layer(&mut self, name: Option<&str>) -> Result<usize, KernelError> {
        self.add_layer_with_role(name, LayerRole::Default)
    }

    /// Inserts a transparent layer with an explicit bake role.
    ///
    /// [`LayerRole::Polish`] is an overlay. Bake does not rebuild it.
    pub fn add_layer_with_role(
        &mut self,
        name: Option<&str>,
        role: LayerRole,
    ) -> Result<usize, KernelError> {
        let name = match name {
            Some(name) => normalize_layer_name(name)?,
            None => self.next_layer_name(),
        };
        let index = self.active_layer + 1;
        let active_before = self.active_layer;
        let before_parts = self.parts.clone();
        let blank = vec![TRANSPARENT_INDEX; self.pixel_count()];
        for frame in &mut self.frames {
            frame.insert_layer(index, blank.clone());
        }
        self.layer_names.insert(index, name.clone());
        self.layer_roles.insert(index, role);
        self.layer_binds.insert(index, None);
        self.layer_visible.insert(index, true);
        self.layer_opacity.insert(index, LAYER_OPACITY_OPAQUE);
        self.active_layer = index;
        self.parts = parts_after_layer_insert(&before_parts, index);
        self.record_with_parts(
            before_parts,
            Change::AddLayer {
                index,
                name,
                role,
                active_before,
                active_after: index,
            },
        );
        Ok(index)
    }

    /// Removes one layer from every frame. The last layer cannot be removed.
    ///
    /// When the active layer is removed, the layer that slides into that index
    /// stays selected, or the new top layer when the top one was removed.
    pub fn delete_layer(&mut self, index: usize) -> Result<(), KernelError> {
        if self.layer_names.len() == 1 {
            return Err(KernelError::LastLayer);
        }
        let index = self.layer_or_err(index)?;
        let active_before = self.active_layer;
        let before_parts = self.parts.clone();
        let name = self.layer_names.remove(index);
        let role = self.layer_roles.remove(index);
        let bind = self.layer_binds.remove(index);
        let visible = self.layer_visible.remove(index);
        let opacity = self.layer_opacity.remove(index);
        let mut pixels = Vec::with_capacity(self.frames.len());
        let mut erase = Vec::with_capacity(self.frames.len());
        for frame in &mut self.frames {
            let (buffer, mask) = frame.remove_layer(index);
            pixels.push(buffer);
            erase.push(mask);
        }
        self.active_layer = if active_before > index {
            active_before - 1
        } else if active_before == index {
            index.min(self.layer_names.len() - 1)
        } else {
            active_before
        };
        self.parts = parts_after_layer_remove(&before_parts, index);
        self.record_with_parts(
            before_parts,
            Change::RemoveLayer {
                index,
                name,
                role,
                visible,
                opacity,
                bind,
                pixels,
                erase,
                active_before,
                active_after: self.active_layer,
            },
        );
        Ok(())
    }

    /// Renames one layer. The same name is not recorded. The stored name is trimmed.
    pub fn rename_layer(&mut self, index: usize, name: &str) -> Result<(), KernelError> {
        let index = self.layer_or_err(index)?;
        let name = normalize_layer_name(name)?;
        if self.layer_names[index] == name {
            return Ok(());
        }
        let before = std::mem::replace(&mut self.layer_names[index], name.clone());
        self.history.record(Change::RenameLayer {
            index,
            before,
            after: name,
        });
        Ok(())
    }

    /// Sets whether bake may rebuild this layer.
    ///
    /// The same role is not recorded. Pixels stay. [`LayerRole::Polish`] is the
    /// overlay bake leaves alone.
    pub fn set_layer_role(&mut self, index: usize, role: LayerRole) -> Result<(), KernelError> {
        let index = self.layer_or_err(index)?;
        if self.layer_roles[index] == role {
            return Ok(());
        }
        let before = self.layer_roles[index];
        let before_bind = self.layer_binds[index].clone();
        self.layer_roles[index] = role;
        if role != LayerRole::Polish {
            self.layer_binds[index] = None;
        }
        let mut changes = vec![Change::LayerRole {
            index,
            before,
            after: role,
        }];
        if before_bind.is_some() && role != LayerRole::Polish {
            changes.push(Change::LayerBind {
                index,
                before: before_bind,
                after: None,
            });
        }
        self.history.record_group(changes);
        Ok(())
    }

    /// Binds a polish layer to a part, or clears the binding when `part` is `None`.
    ///
    /// `anchor` is the local origin. `None` uses the part origin. The layer must
    /// already be [`LayerRole::Polish`]. The same binding is not recorded.
    /// Pixels stay where they were painted until a part transform or bake moves them.
    pub fn set_layer_bind(
        &mut self,
        index: usize,
        part: Option<&str>,
        anchor: Option<&str>,
    ) -> Result<(), KernelError> {
        let index = self.layer_or_err(index)?;
        let next = match part {
            None => {
                if anchor.is_some() {
                    return Err(KernelError::PolishBind { layer: index });
                }
                None
            }
            Some(part) => {
                if self.layer_roles[index] != LayerRole::Polish {
                    return Err(KernelError::PolishBind { layer: index });
                }
                let part = normalize_part_name(part).map_err(|()| KernelError::PartName)?;
                let found = self
                    .parts
                    .iter()
                    .find(|candidate| candidate.name == part)
                    .ok_or(KernelError::PartMissing)?;
                let anchor = match anchor {
                    None => None,
                    Some(anchor) => {
                        let anchor =
                            normalize_part_name(anchor).map_err(|()| KernelError::AnchorName)?;
                        if found.anchors.iter().all(|item| item.name != anchor) {
                            return Err(KernelError::AnchorMissing);
                        }
                        Some(anchor)
                    }
                };
                Some(PolishBind { part, anchor })
            }
        };
        if self.layer_binds[index] == next {
            return Ok(());
        }
        let before = self.layer_binds[index].clone();
        self.layer_binds[index] = next.clone();
        self.history.record(Change::LayerBind {
            index,
            before,
            after: next,
        });
        Ok(())
    }

    /// Selects a layer. The same index is not recorded.
    pub fn set_active_layer(&mut self, index: usize) -> Result<(), KernelError> {
        let index = self.layer_or_err(index)?;
        if index == self.active_layer {
            return Ok(());
        }
        let before = self.active_layer;
        self.active_layer = index;
        self.history.record(Change::ActiveLayer {
            before,
            after: index,
        });
        Ok(())
    }

    /// Moves the layer at `from` so it ends at `to`.
    ///
    /// `to` is the index after the move. The same index is not recorded. The
    /// active layer follows the moved layer, and layers between the two ends
    /// shift by one.
    pub fn reorder_layer(&mut self, from: usize, to: usize) -> Result<(), KernelError> {
        let from = self.layer_or_err(from)?;
        let to = self.layer_or_err(to)?;
        if from == to {
            return Ok(());
        }
        let active_before = self.active_layer;
        let before_parts = self.parts.clone();
        self.move_layer(from, to);
        let active_after = remap_layer_index(active_before, from, to);
        self.active_layer = active_after;
        self.parts = parts_after_reorder(&before_parts, from, to);
        self.record_with_parts(
            before_parts,
            Change::ReorderLayer {
                from,
                to,
                active_before,
                active_after,
            },
        );
        Ok(())
    }

    /// Composites every frame into one layer named [`DEFAULT_LAYER_NAME`].
    ///
    /// The result is the index composite: hidden layers and opacity 0 are
    /// omitted, and a partial opacity still covers with its index. The RGBA
    /// blend is not baked. The new layer is visible and fully opaque. A
    /// document that is already that one default layer is not recorded.
    /// One undo restores the previous stack, including visibility and opacity.
    pub fn flatten(&mut self) -> Result<(), KernelError> {
        let marks_clear = self.frames.iter().all(|frame| {
            frame
                .erasures()
                .iter()
                .all(|mask| mask.iter().all(|mark| *mark == 0))
        });
        if self.layer_names.len() == 1
            && self.layer_names[0] == DEFAULT_LAYER_NAME
            && self.active_layer == 0
            && self.layer_roles[0] == LayerRole::Default
            && self.layer_binds[0].is_none()
            && self.layer_visible[0]
            && self.layer_opacity[0] == LAYER_OPACITY_OPAQUE
            && marks_clear
        {
            return Ok(());
        }
        let before_names = self.layer_names.clone();
        let before_roles = self.layer_roles.clone();
        let before_visible = self.layer_visible.clone();
        let before_opacity = self.layer_opacity.clone();
        let before_binds = self.layer_binds.clone();
        let before_pixels = self
            .frames
            .iter()
            .map(|frame| frame.layers().to_vec())
            .collect();
        let before_erase = self
            .frames
            .iter()
            .map(|frame| frame.erasures().to_vec())
            .collect();
        let before_active = self.active_layer;
        let before_parts = self.parts.clone();
        let polish = self.polish_flags();
        let after_pixels: Vec<Vec<u8>> = self
            .frames
            .iter()
            .map(|frame| frame.composite(&self.layer_visible, &self.layer_opacity, &polish))
            .collect();
        for (frame, pixels) in self.frames.iter_mut().zip(after_pixels.iter().cloned()) {
            frame.set_stack(vec![pixels.clone()], vec![vec![0; pixels.len()]]);
        }
        self.layer_names = vec![DEFAULT_LAYER_NAME.to_string()];
        self.layer_roles = vec![LayerRole::Default];
        self.layer_binds = vec![None];
        self.layer_visible = vec![true];
        self.layer_opacity = vec![LAYER_OPACITY_OPAQUE];
        self.active_layer = 0;
        self.parts = parts_after_flatten(&before_parts);
        self.record_with_parts(
            before_parts,
            Change::Flatten {
                before_names,
                before_roles,
                before_visible,
                before_opacity,
                before_binds,
                before_pixels,
                before_erase,
                before_active,
                after_pixels,
            },
        );
        Ok(())
    }

    /// Milliseconds the frame stays on screen during [`Self::tick`].
    pub fn frame_delay(&self, index: usize) -> Result<u32, KernelError> {
        self.frames
            .get(index)
            .map(Frame::delay_ms)
            .ok_or(KernelError::FrameIndex {
                index,
                len: self.frames.len(),
            })
    }

    /// Delay of every frame, in timeline order.
    pub fn frame_delays(&self) -> Vec<u32> {
        self.frames.iter().map(Frame::delay_ms).collect()
    }

    /// Sum of every frame delay. Loop playback wraps at this length.
    pub fn duration_ms(&self) -> u64 {
        self.frames
            .iter()
            .map(|frame| u64::from(frame.delay_ms()))
            .sum()
    }

    /// Sets one frame's delay in milliseconds.
    ///
    /// The same value is not recorded. A drag or a typing session should sit
    /// inside one gesture; those writes fold into a single undo step.
    pub fn set_frame_delay(&mut self, index: usize, delay_ms: u32) -> Result<(), KernelError> {
        if index >= self.frames.len() {
            return Err(KernelError::FrameIndex {
                index,
                len: self.frames.len(),
            });
        }
        if !(MIN_FRAME_DELAY_MS..=MAX_FRAME_DELAY_MS).contains(&delay_ms) {
            return Err(KernelError::FrameDelay { delay_ms });
        }
        let before = self.frames[index].delay_ms();
        if before == delay_ms {
            return Ok(());
        }
        self.frames[index].set_delay_ms(delay_ms);
        if index == self.active && self.playback.elapsed_ms >= delay_ms {
            self.playback.elapsed_ms = 0;
        }
        self.history.record(Change::FrameDelay {
            index,
            before,
            after: delay_ms,
        });
        Ok(())
    }

    /// Named frame ranges, in insertion order. Empty when the file has none.
    pub fn tags(&self) -> &[Tag] {
        &self.tags
    }

    /// Appends a tag. `name` is trimmed. The range is inclusive.
    ///
    /// Overlapping ranges are allowed. A second tag with the same name is
    /// [`KernelError::TagExists`]. One undo step. This does not link cels.
    pub fn add_tag(
        &mut self,
        name: &str,
        from: usize,
        to: usize,
        loop_mode: LoopMode,
    ) -> Result<usize, KernelError> {
        let name = normalize_tag_name(name).map_err(|()| KernelError::TagName)?;
        if self.tags.iter().any(|tag| tag.name == name) {
            return Err(KernelError::TagExists);
        }
        self.check_tag_range(from, to)?;
        let index = self.tags.len();
        let before = self.tags.clone();
        self.tags.push(Tag {
            name,
            from,
            to,
            loop_mode,
        });
        self.record_tags(before);
        Ok(index)
    }

    /// Changes one tag's range, loop mode, or name. Unset fields stay.
    ///
    /// The same resulting tag is not recorded. `rename` is trimmed. One undo
    /// step when the tag changes. Returns the tag's index.
    pub fn set_tag(&mut self, name: &str, update: TagUpdate) -> Result<usize, KernelError> {
        let name = normalize_tag_name(name).map_err(|()| KernelError::TagName)?;
        let index = self.tag_position(&name)?;
        let mut next = self.tags[index].clone();
        if let Some(from) = update.from {
            next.from = from;
        }
        if let Some(to) = update.to {
            next.to = to;
        }
        if let Some(loop_mode) = update.loop_mode {
            next.loop_mode = loop_mode;
        }
        if let Some(rename) = update.rename {
            let rename = normalize_tag_name(&rename).map_err(|()| KernelError::TagName)?;
            if rename != next.name && self.tags.iter().any(|tag| tag.name == rename) {
                return Err(KernelError::TagExists);
            }
            next.name = rename;
        }
        self.check_tag_range(next.from, next.to)?;
        if next == self.tags[index] {
            return Ok(index);
        }
        let before = self.tags.clone();
        self.tags[index] = next;
        self.record_tags(before);
        Ok(index)
    }

    /// Removes the tag with this name. Frames stay. Returns the removed index.
    pub fn remove_tag(&mut self, name: &str) -> Result<usize, KernelError> {
        let name = normalize_tag_name(name).map_err(|()| KernelError::TagName)?;
        let index = self.tag_position(&name)?;
        let before = self.tags.clone();
        self.tags.remove(index);
        self.record_tags(before);
        Ok(index)
    }

    /// Named parts, in insertion order. Empty when the file has none.
    pub fn parts(&self) -> &[Part] {
        &self.parts
    }

    /// Appends a part. `name` is trimmed. `x` and `y` are the canvas cell that
    /// is local `(0, 0)`. `layer` attaches the part to that paint layer, or
    /// leaves it unattached when `None`.
    ///
    /// A second part with the same name is [`KernelError::PartExists`]. The
    /// origin must lie on the canvas. One undo step. This does not write pixels.
    pub fn add_part(
        &mut self,
        name: &str,
        x: u16,
        y: u16,
        layer: Option<usize>,
    ) -> Result<usize, KernelError> {
        let name = normalize_part_name(name).map_err(|()| KernelError::PartName)?;
        if self.parts.iter().any(|part| part.name == name) {
            return Err(KernelError::PartExists);
        }
        self.check_origin(x, y)?;
        if let Some(layer) = layer {
            self.layer_or_err(layer)?;
        }
        let index = self.parts.len();
        let before = self.parts.clone();
        self.parts.push(Part {
            name,
            x,
            y,
            layer,
            anchors: Vec::new(),
        });
        self.record_parts(before);
        Ok(index)
    }

    /// Changes one part's origin, layer attachment, or name. Unset fields stay.
    ///
    /// The same resulting part is not recorded. `rename` is trimmed. One undo
    /// step when the part changes. Returns the part's index. This does not
    /// write pixels and does not move anchors: they stay in local cells.
    pub fn set_part(&mut self, name: &str, update: PartUpdate) -> Result<usize, KernelError> {
        let name = normalize_part_name(name).map_err(|()| KernelError::PartName)?;
        let index = self.part_position(&name)?;
        let mut next = self.parts[index].clone();
        if let Some(x) = update.x {
            next.x = x;
        }
        if let Some(y) = update.y {
            next.y = y;
        }
        self.check_origin(next.x, next.y)?;
        match update.layer {
            LayerAttach::Keep => {}
            LayerAttach::Clear => next.layer = None,
            LayerAttach::Index(layer) => {
                self.layer_or_err(layer)?;
                next.layer = Some(layer);
            }
        }
        if let Some(rename) = update.rename {
            let rename = normalize_part_name(&rename).map_err(|()| KernelError::PartName)?;
            if rename != next.name && self.parts.iter().any(|part| part.name == rename) {
                return Err(KernelError::PartExists);
            }
            next.name = rename;
        }
        if next == self.parts[index] {
            return Ok(index);
        }
        let previous_name = self.parts[index].name.clone();
        let before_parts = self.parts.clone();
        let before_curves = self.curves.clone();
        let before_constraints = self.constraints.clone();
        self.parts[index] = next;
        let mut bind_changes = Vec::new();
        if self.parts[index].name != previous_name {
            self.curves =
                curves_after_rename(&before_curves, &previous_name, &self.parts[index].name);
            self.constraints = constraints_after_rename(
                &before_constraints,
                &previous_name,
                &self.parts[index].name,
            );
            let renamed = self.parts[index].name.clone();
            bind_changes = self.retarget_binds(&previous_name, Some(&renamed));
        }
        self.record_rig(
            before_parts,
            before_curves,
            before_constraints,
            bind_changes,
        );
        Ok(index)
    }

    /// Removes the part with this name, including its anchors and curves. Returns the removed index.
    pub fn remove_part(&mut self, name: &str) -> Result<usize, KernelError> {
        let name = normalize_part_name(name).map_err(|()| KernelError::PartName)?;
        let index = self.part_position(&name)?;
        let before_parts = self.parts.clone();
        let before_curves = self.curves.clone();
        let before_constraints = self.constraints.clone();
        let removed = before_parts[index].name.clone();
        self.parts.remove(index);
        self.curves = curves_for_parts(&before_curves, &self.parts);
        self.constraints = constraints_for_parts(&before_constraints, &self.parts);
        let bind_changes = self.retarget_binds(&removed, None);
        self.record_rig(
            before_parts,
            before_curves,
            before_constraints,
            bind_changes,
        );
        Ok(index)
    }

    /// Sets one named anchor in the part's local pixel grid.
    ///
    /// Creates the anchor when the name is new, or replaces its offset.
    /// The same offset is not recorded. `x` and `y` are cells relative to the
    /// part origin, in `-MAX_CANVAS_EDGE..=MAX_CANVAS_EDGE`. Returns
    /// `(part index, anchor index)`. This does not write pixels.
    pub fn set_anchor(
        &mut self,
        part: &str,
        name: &str,
        x: i32,
        y: i32,
    ) -> Result<(usize, usize), KernelError> {
        let part_name = normalize_part_name(part).map_err(|()| KernelError::PartName)?;
        let anchor_name = normalize_part_name(name).map_err(|()| KernelError::AnchorName)?;
        let Some((x, y)) = anchor_offset_i64(i64::from(x), i64::from(y)) else {
            return Err(KernelError::AnchorCoord { x, y });
        };
        let part_index = self.part_position(&part_name)?;
        if let Some(anchor_index) = self.parts[part_index]
            .anchors
            .iter()
            .position(|anchor| anchor.name == anchor_name)
        {
            let current = &self.parts[part_index].anchors[anchor_index];
            if current.x == x && current.y == y {
                return Ok((part_index, anchor_index));
            }
            let before = self.parts.clone();
            let anchor = &mut self.parts[part_index].anchors[anchor_index];
            anchor.x = x;
            anchor.y = y;
            self.record_parts(before);
            return Ok((part_index, anchor_index));
        }
        let before = self.parts.clone();
        self.parts[part_index].anchors.push(crate::part::Anchor {
            name: anchor_name,
            x,
            y,
        });
        let anchor_index = self.parts[part_index].anchors.len() - 1;
        self.record_parts(before);
        Ok((part_index, anchor_index))
    }

    /// Removes one anchor from a part. The part stays. Returns `(part index, anchor index)`.
    pub fn remove_anchor(&mut self, part: &str, name: &str) -> Result<(usize, usize), KernelError> {
        let part_name = normalize_part_name(part).map_err(|()| KernelError::PartName)?;
        let anchor_name = normalize_part_name(name).map_err(|()| KernelError::AnchorName)?;
        let part_index = self.part_position(&part_name)?;
        let anchor_index = self.parts[part_index]
            .anchors
            .iter()
            .position(|anchor| anchor.name == anchor_name)
            .ok_or(KernelError::AnchorMissing)?;
        let before = self.parts.clone();
        let anchor_name = self.parts[part_index].anchors[anchor_index].name.clone();
        let part_name = self.parts[part_index].name.clone();
        self.parts[part_index].anchors.remove(anchor_index);
        let bind_changes = self.clear_anchor_binds(&part_name, &anchor_name);
        self.record_parts_with(before, bind_changes);
        Ok((part_index, anchor_index))
    }

    /// Keyframed part parameters, in insertion order. Empty when the file has none.
    pub fn curves(&self) -> &[Curve] {
        &self.curves
    }

    /// Sets one key on a part channel. Creates the curve when that channel is new.
    ///
    /// `frame` is a timeline index, the same axis as tags. `value` is degrees
    /// for rotation, whole cells for translate, or `0` / `1` for flip. `ease`
    /// is how this key leaves toward the next one. Omit it and rotation or
    /// translate use linear, while flip uses hold. The same stored key is not
    /// recorded. One undo step. This does not write pixels.
    pub fn set_curve_key(
        &mut self,
        part: &str,
        channel: CurveChannel,
        frame: usize,
        value: f64,
        ease: Option<Ease>,
    ) -> Result<CurveWrite, KernelError> {
        let part = normalize_part_name(part).map_err(|()| KernelError::PartName)?;
        self.part_position(&part)?;
        if frame >= self.frames.len() {
            return Err(KernelError::FrameIndex {
                index: frame,
                len: self.frames.len(),
            });
        }
        let ease = ease.unwrap_or_else(|| channel.default_ease());
        if !channel.allows(ease) {
            return Err(KernelError::CurveEase);
        }
        let stored = channel.store(value)?;
        let canonical = channel.decode(stored);
        if let Some(index) = self.curve_position(&part, channel) {
            if let Some(key_index) = self.curves[index]
                .keys
                .iter()
                .position(|key| key.frame == frame)
            {
                let current = &self.curves[index].keys[key_index];
                if current.value == stored && current.ease == ease {
                    return Ok(CurveWrite {
                        index,
                        key_index,
                        value: canonical,
                        ease,
                    });
                }
            }
        }
        let before = self.curves.clone();
        let (index, key_index) = if let Some(index) = self.curve_position(&part, channel) {
            let keys = &mut self.curves[index].keys;
            if let Some(key_index) = keys.iter().position(|key| key.frame == frame) {
                keys[key_index].value = stored;
                keys[key_index].ease = ease;
                (index, key_index)
            } else {
                let key_index = keys
                    .iter()
                    .position(|key| key.frame > frame)
                    .unwrap_or(keys.len());
                keys.insert(
                    key_index,
                    CurveKey {
                        frame,
                        value: stored,
                        ease,
                    },
                );
                (index, key_index)
            }
        } else {
            let index = self.curves.len();
            self.curves.push(Curve {
                part,
                channel,
                keys: vec![CurveKey {
                    frame,
                    value: stored,
                    ease,
                }],
            });
            (index, 0)
        };
        self.record_curves(before);
        Ok(CurveWrite {
            index,
            key_index,
            value: canonical,
            ease,
        })
    }

    /// Removes one key. The last key drops the curve. Frames stay.
    pub fn remove_curve_key(
        &mut self,
        part: &str,
        channel: CurveChannel,
        frame: usize,
    ) -> Result<CurveRemoval, KernelError> {
        let part = normalize_part_name(part).map_err(|()| KernelError::PartName)?;
        self.part_position(&part)?;
        let index = self
            .curve_position(&part, channel)
            .ok_or(KernelError::CurveMissing)?;
        if !self.curves[index].keys.iter().any(|key| key.frame == frame) {
            return Err(KernelError::CurveKeyMissing);
        }
        let before = self.curves.clone();
        self.curves[index].keys.retain(|key| key.frame != frame);
        let removed_curve = self.curves[index].keys.is_empty();
        if removed_curve {
            self.curves.remove(index);
        }
        self.record_curves(before);
        Ok(CurveRemoval {
            index,
            part,
            channel,
            frame,
            removed_curve,
        })
    }

    /// Removes every key of one part channel. Returns the curve index it had.
    pub fn clear_curve(&mut self, part: &str, channel: CurveChannel) -> Result<usize, KernelError> {
        let part = normalize_part_name(part).map_err(|()| KernelError::PartName)?;
        self.part_position(&part)?;
        let index = self
            .curve_position(&part, channel)
            .ok_or(KernelError::CurveMissing)?;
        let before = self.curves.clone();
        self.curves.remove(index);
        self.record_curves(before);
        Ok(index)
    }

    /// Interpolated value of one channel at `frame`.
    ///
    /// `frame` may be fractional. `2` is the key on frame 2 when one exists.
    /// `2.5` is halfway from frame 2 to frame 3 in index space, not in
    /// `delayMs`. This does not write pixels and does not push undo.
    pub fn sample_curve(
        &self,
        part: &str,
        channel: CurveChannel,
        frame: f64,
    ) -> Result<CurveSample, KernelError> {
        let part = normalize_part_name(part).map_err(|()| KernelError::PartName)?;
        self.part_position(&part)?;
        if !frame.is_finite() {
            return Err(KernelError::CurveFrame);
        }
        let curve = self
            .curves
            .iter()
            .find(|curve| curve.part == part && curve.channel == channel)
            .ok_or(KernelError::CurveMissing)?;
        Ok(sample_keys(channel, &curve.keys, frame))
    }

    /// Every keyed channel of one part at `frame`. Missing channels are not keyed.
    ///
    /// The numbers are the pose relative to the rest origin. Passing one to
    /// `rotate_part` or `translate_part` on a frame that was already
    /// transformed stacks. This call does not apply that pose.
    pub fn sample_part(&self, part: &str, frame: f64) -> Result<PartSample, KernelError> {
        let part = normalize_part_name(part).map_err(|()| KernelError::PartName)?;
        self.part_position(&part)?;
        if !frame.is_finite() {
            return Err(KernelError::CurveFrame);
        }
        let mut sample = PartSample {
            part: part.clone(),
            frame,
            rotation: None,
            translate_x: None,
            translate_y: None,
            flip_horizontal: None,
            flip_vertical: None,
        };
        for curve in self.curves.iter().filter(|curve| curve.part == part) {
            let value = sample_keys(curve.channel, &curve.keys, frame).value;
            match curve.channel {
                CurveChannel::Rotation => sample.rotation = Some(value),
                CurveChannel::TranslateX => sample.translate_x = Some(value),
                CurveChannel::TranslateY => sample.translate_y = Some(value),
                CurveChannel::FlipHorizontal => sample.flip_horizontal = Some(value),
                CurveChannel::FlipVertical => sample.flip_vertical = Some(value),
            }
        }
        Ok(sample)
    }

    /// Moves a part by whole cells.
    ///
    /// The origin and every anchor's canvas cell shift by `(dx, dy)`. Local
    /// anchor offsets stay. Ink on the linked layer of `frame` shifts with
    /// them. An unattached part only moves its metadata. One undo step when
    /// anything changes. A zero move is not recorded. `frame` is not selected.
    ///
    /// Every non-zero cell on that layer must land on the canvas. When a
    /// selection is set, every non-zero cell and its destination must lie
    /// inside it, the same all-or-nothing rule as [`Self::move_rect`]. The
    /// call writes nothing when either check fails.
    pub fn translate_part(
        &mut self,
        name: &str,
        dx: i32,
        dy: i32,
        frame: usize,
    ) -> Result<PartPose, KernelError> {
        self.transform_part(name, Motion::Translate { dx, dy }, None, frame)
    }

    /// Turns a part clockwise around its origin, or around a named anchor.
    ///
    /// The pivot is the center of that cell. See [`crate::part_xform`] for the
    /// 0.01° quantization and the nearest-neighbor rule. Quarter turns use an
    /// integer map. Any other angle snaps cell centers, then seals notches the
    /// snap left empty. `dropped` still counts collisions. One undo step on
    /// `frame` plus the part. `frame` is not selected. A multiple of 360° that
    /// quantizes to 0 is not recorded.
    pub fn rotate_part(
        &mut self,
        name: &str,
        degrees: f64,
        anchor: Option<&str>,
        frame: usize,
    ) -> Result<PartPose, KernelError> {
        self.transform_part(name, Motion::Rotate(degrees), anchor, frame)
    }

    /// Mirrors a part around its origin, or around a named anchor.
    ///
    /// `vertical` is false for a left-right mirror. The mirror fixes the pivot
    /// cell: `x` becomes `2 * pivot_x - x` (or the same for `y`). Indices
    /// move and colors do not. Applying the same flip again restores a part
    /// whose ink stayed on the canvas. One undo step. `frame` is not selected.
    pub fn flip_part(
        &mut self,
        name: &str,
        vertical: bool,
        anchor: Option<&str>,
        frame: usize,
    ) -> Result<PartPose, KernelError> {
        let motion = if vertical {
            Motion::FlipVertical
        } else {
            Motion::FlipHorizontal
        };
        self.transform_part(name, motion, anchor, frame)
    }

    /// Stored constraints, in insertion order. Empty when the file has none.
    pub fn constraints(&self) -> &[Constraint] {
        &self.constraints
    }

    /// Appends one constraint. Names are trimmed.
    ///
    /// `follow` tracks `target_anchor` with an optional follower anchor (the
    /// origin when omitted). `attach` requires both anchors. `pin` stores a
    /// canvas cell and no target. `stick` is not a stored kind; pass
    /// [`ConstraintKind::Attach`]. A second driver for the same part, or a
    /// cycle, is refused and the list is left unchanged. One undo step. This
    /// does not move pixels. Solving is [`Self::solve_constraints`].
    ///
    /// `ik` and `plant` use [`Self::add_constraint_full`].
    pub fn add_constraint(
        &mut self,
        name: &str,
        kind: ConstraintKind,
        part: &str,
        anchor: Option<&str>,
        target: Option<&str>,
        target_anchor: Option<&str>,
        x: Option<i32>,
        y: Option<i32>,
    ) -> Result<usize, KernelError> {
        self.add_constraint_full(
            name,
            kind,
            part,
            anchor,
            target,
            target_anchor,
            x,
            y,
            &[],
            None,
            &[],
        )
    }

    /// [`Self::add_constraint`] plus a short IK chain, a bend sign, and footprint spans.
    ///
    /// `chain` is root-first and has two or three parts. The last name is `part`.
    /// `bend` is `1`, `-1`, or omitted. `contacts` are inclusive frame spans on
    /// cells that already exist. One undo step. This does not move pixels.
    pub fn add_constraint_full(
        &mut self,
        name: &str,
        kind: ConstraintKind,
        part: &str,
        anchor: Option<&str>,
        target: Option<&str>,
        target_anchor: Option<&str>,
        x: Option<i32>,
        y: Option<i32>,
        chain: &[String],
        bend: Option<i8>,
        contacts: &[Contact],
    ) -> Result<usize, KernelError> {
        let constraint = prepare_constraint(
            &self.constraints,
            &self.parts,
            self.width,
            self.height,
            self.frames.len(),
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
        )?;
        let index = self.constraints.len();
        let before = self.constraints.clone();
        self.constraints.push(constraint);
        self.record_constraints(before);
        Ok(index)
    }

    /// Removes one constraint by name. Parts and pixels stay. Returns the removed index.
    pub fn remove_constraint(&mut self, name: &str) -> Result<usize, KernelError> {
        let name = normalize_part_name(name).map_err(|()| KernelError::ConstraintName)?;
        let index = self
            .constraints
            .iter()
            .position(|constraint| constraint.name == name)
            .ok_or(KernelError::ConstraintMissing)?;
        let before = self.constraints.clone();
        self.constraints.remove(index);
        self.record_constraints(before);
        Ok(index)
    }

    /// Samples curves when `frame` is set, then solves constraints.
    ///
    /// `frame` is a curve position and may be fractional. Omit it to read the
    /// stored part pose, including a pose already written by `rotate_part`.
    /// The channel order is rotation, horizontal flip, vertical flip, each
    /// around the stored origin, then translation. A fractional translation
    /// snaps to the nearest cell, ties away from zero.
    ///
    /// Solve order is dependency order: a follower waits for a driven target.
    /// Independent constraints keep file order. A cycle, a second driver, a
    /// missing anchor, an origin or ink cell off the canvas, a shared layer
    /// with two posed parts, or a selection that does not cover the ink is an
    /// error and writes nothing.
    ///
    /// `apply` false returns the pose and does not push undo. `apply` true
    /// writes that pose onto `pixel_frame` (the active frame when omitted) as
    /// one undo step when anything changed. A linked layer's non-zero ink
    /// moves. An unattached part only moves its origin and anchors. Calling
    /// this again stacks on the stored pose. It does not create frames and
    /// does not walk the timeline.
    pub fn solve_constraints(
        &mut self,
        frame: Option<f64>,
        apply: bool,
        pixel_frame: Option<usize>,
    ) -> Result<SolveReport, KernelError> {
        if frame.is_some_and(|frame| !frame.is_finite()) {
            return Err(KernelError::CurveFrame);
        }
        let pixel_frame = self.frame_or_err(pixel_frame.unwrap_or(self.active))?;
        let mut posed: Vec<PosedPart> = Vec::with_capacity(self.parts.len());
        for part in &self.parts {
            posed.push(orient_part(
                part,
                &self.curves,
                frame,
                self.width,
                self.height,
            )?);
        }
        let order = solve_order(&self.constraints)?;
        let stored = self.parts.clone();
        let solved = apply_deltas(
            &mut posed,
            &stored,
            &self.constraints,
            &order,
            self.width,
            self.height,
            frame,
            pixel_frame,
        )?;
        let order_names: Vec<String> = order
            .iter()
            .map(|index| self.constraints[*index].name.clone())
            .collect();

        let mut steps_per_part = Vec::with_capacity(posed.len());
        let mut moving_layers: Vec<(usize, usize)> = Vec::new();
        for (index, part) in posed.iter().enumerate() {
            let steps = motion_steps(part)?;
            if !steps.is_empty() {
                if let Some(layer) = part.oriented.layer {
                    if moving_layers.iter().any(|(existing, _)| *existing == layer) {
                        return Err(KernelError::ConstraintLayer { layer });
                    }
                    moving_layers.push((layer, index));
                }
            }
            steps_per_part.push(steps);
        }

        let mut layer_buffers: Vec<(usize, Vec<u8>, WarpReport)> = Vec::new();
        for (layer, index) in &moving_layers {
            let old = self.frames[pixel_frame].layer_pixels(*layer);
            let (next, report) = crate::constraint::warp_ink(
                self.width,
                self.height,
                old,
                &steps_per_part[*index],
                |x, y| self.accepts_write(x, y),
            )?;
            layer_buffers.push((*layer, next, report));
        }

        let mut solved_parts = Vec::with_capacity(posed.len());
        for (index, part) in posed.iter().enumerate() {
            let (x, y) = place_origin(part, self.width, self.height)?;
            let moves = !steps_per_part[index].is_empty();
            let report = layer_buffers
                .iter()
                .find(|(layer, _, _)| part.oriented.layer == Some(*layer) && moves)
                .map(|(_, _, report)| *report)
                .unwrap_or(WarpReport::ZERO);
            let anchors = part
                .oriented
                .anchors
                .iter()
                .map(|anchor| {
                    let (canvas_x, canvas_y) =
                        canvas_local(part, i32::from(anchor.x), i32::from(anchor.y))
                            .expect("anchor local was stored inside the offset limit");
                    SolvedAnchor {
                        name: anchor.name.clone(),
                        x: anchor.x,
                        y: anchor.y,
                        canvas_x,
                        canvas_y,
                    }
                })
                .collect();
            solved_parts.push(SolvedPart {
                name: part.oriented.name.clone(),
                x,
                y,
                layer: part.oriented.layer,
                ink: ink_label(part.oriented.layer, moves),
                dropped: report.dropped,
                conflicts: report.conflicts,
                sealed: report.sealed,
                breaks: report.breaks,
                anchors,
            });
        }

        let next_parts: Vec<Part> = solved_parts
            .iter()
            .map(|part| Part {
                name: part.name.clone(),
                x: part.x,
                y: part.y,
                layer: part.layer,
                anchors: part
                    .anchors
                    .iter()
                    .map(|anchor| Anchor {
                        name: anchor.name.clone(),
                        x: anchor.x,
                        y: anchor.y,
                    })
                    .collect(),
            })
            .collect();
        let parts_changed = next_parts != self.parts;
        let pixels_changed = layer_buffers.iter().any(|(layer, next, _)| {
            next.as_slice() != self.frames[pixel_frame].layer_pixels(*layer)
        });
        let wrote = apply && (parts_changed || pixels_changed);
        if wrote {
            let mut changes = Vec::new();
            for (layer, next, _) in &layer_buffers {
                let old = self.frames[pixel_frame].layer_pixels(*layer);
                let mut plan = vec![None; next.len()];
                for (offset, (before, after)) in old.iter().zip(next.iter()).enumerate() {
                    if before != after {
                        plan[offset] = Some(*after);
                    }
                }
                self.push_plan(pixel_frame, *layer, &plan, &mut changes);
            }
            if parts_changed {
                let before = std::mem::replace(&mut self.parts, next_parts);
                changes.push(Change::Parts {
                    before,
                    after: self.parts.clone(),
                });
            }
            self.history.record_group(changes);
        }
        let totals = solved_parts.iter().fold(WarpReport::ZERO, |total, part| {
            total.saturating_add(WarpReport {
                dropped: part.dropped,
                conflicts: part.conflicts,
                sealed: part.sealed,
                breaks: part.breaks,
            })
        });
        Ok(SolveReport {
            frame,
            pixel_frame,
            apply,
            wrote,
            dropped: totals.dropped,
            conflicts: totals.conflicts,
            sealed: totals.sealed,
            breaks: totals.breaks,
            order: order_names,
            constraints: solved,
            parts: solved_parts,
        })
    }

    /// Samples part curves into pixel frames, then solves constraints.
    ///
    /// Each target frame is posed from the rest ink on `source` (frame 0 when
    /// omitted), using the same curve order and constraint solver as
    /// [`Self::solve_constraints`]. The source frame itself is not rewritten,
    /// and the stored part origins stay at rest, so a second bake does not
    /// stack. Curves and constraints stay in the file.
    ///
    /// `from`..=`to` defaults to every existing frame. `to` past the last frame
    /// appends posed copies of the source. `from` past the frame count is
    /// [`KernelError::BakeRange`]. `fps` sets `delayMs` to `round(1000 / fps)`
    /// on every frame in the range, including the source.
    ///
    /// On an existing frame, layers managed by the sampled curves and solved
    /// constraints are replaced from the rest ink, even when that pose is the
    /// identity. `parts` or `layers` name a different write set. A layer with
    /// no curve and no driving constraint stays, so duplicate the rest frame
    /// first when a body with no part should remain. An unbound
    /// [`LayerRole::Polish`] layer is not written. A polish layer bound to a
    /// part this bake poses is restamped from the source overlay, including
    /// explicit erases. A managed part linked to a polish layer is
    /// [`KernelError::PolishLayer`] and the call writes nothing. Appended
    /// frames copy every source layer, including polish from the source, then
    /// replace the posed layers and any bound overlay that followed.
    ///
    /// A failure writes nothing. A success that changes pixels, delays, or the
    /// frame list is one undo step. The same pixels and delays are not recorded.
    pub fn bake(&mut self, options: BakeOptions) -> Result<BakeReport, KernelError> {
        let delay_ms = match options.fps {
            None => None,
            Some(fps) => Some(delay_from_fps(fps)?),
        };
        let source = self.frame_or_err(options.source.unwrap_or(0))?;
        let len = self.frames.len();
        let from = options.from.unwrap_or(0);
        let to = options.to.unwrap_or(len - 1);
        if from > to || from > len || to >= MAX_BAKE_FRAMES {
            return Err(KernelError::BakeRange { from, to, len });
        }
        let part_filter = match &options.parts {
            None => None,
            Some(names) => Some(self.normalize_bake_parts(names)?),
        };
        let layer_filter = match &options.layers {
            None => None,
            Some(layers) => Some(self.normalize_bake_layers(layers)?),
        };
        let included: Vec<bool> = self
            .parts
            .iter()
            .map(|part| match &part_filter {
                None => true,
                Some(names) => names.iter().any(|name| name == &part.name),
            })
            .collect();
        let pose_included = self.bake_included(&included, options.constraints);
        let managed = self.bake_managed(&pose_included, options.constraints);
        let polish = self.polish_layers();
        let source_layers: Vec<Vec<u8>> = (0..self.layer_names.len())
            .map(|layer| self.frames[source].layer_pixels(layer).to_vec())
            .collect();
        let source_erase: Vec<Vec<u8>> = (0..self.layer_names.len())
            .map(|layer| self.frames[source].layer_erase(layer).to_vec())
            .collect();
        let source_delay = self.frames[source].delay_ms();
        let mut plans = Vec::with_capacity(to - from + 1);
        for index in from..=to {
            if index == source {
                let delay = delay_ms.unwrap_or(source_delay);
                plans.push(BakePlan {
                    index,
                    rest: true,
                    delay_ms: delay,
                    is_new: false,
                    new_frame: None,
                    writes: Vec::new(),
                    report: WarpReport::ZERO,
                    layers: Vec::new(),
                    followed: Vec::new(),
                    parts: self.parts.iter().map(stored_bake_part).collect(),
                });
                continue;
            }
            let posed = self.pose_bake(index as f64, &included, options.constraints)?;
            let mut steps_per_part = Vec::with_capacity(posed.len());
            let mut moving: Vec<(usize, usize)> = Vec::new();
            for (part_index, part) in posed.iter().enumerate() {
                let steps = motion_steps(part)?;
                if !steps.is_empty() {
                    if let Some(layer) = part.oriented.layer {
                        moving.push((layer, part_index));
                    }
                }
                steps_per_part.push(steps);
            }
            let write_layers = bake_write_layers(
                &posed,
                &managed,
                part_filter.as_deref(),
                layer_filter.as_deref(),
                self.layer_names.len(),
            );
            for (layer, _) in &moving {
                if write_layers.contains(layer)
                    && moving.iter().filter(|(other, _)| other == layer).count() > 1
                {
                    return Err(KernelError::ConstraintLayer { layer: *layer });
                }
            }
            let write_layers =
                exclude_polish_layers(&write_layers, &self.layer_roles, &posed, &managed)?;
            let is_new = index >= len;
            let mut writes = Vec::new();
            let mut frame_report = WarpReport::ZERO;
            let mut per_layer_report = Vec::new();
            for layer in &write_layers {
                let (next, lost) = if let Some((_, part_index)) = moving
                    .iter()
                    .find(|(moving_layer, _)| moving_layer == layer)
                {
                    warp_ink(
                        self.width,
                        self.height,
                        &source_layers[*layer],
                        &steps_per_part[*part_index],
                        |x, y| self.accepts_write(x, y),
                    )?
                } else {
                    (source_layers[*layer].clone(), WarpReport::ZERO)
                };
                if is_new {
                    self.reject_ink_outside(&next)?;
                } else {
                    self.reject_diff_outside(index, *layer, &next)?;
                }
                frame_report = frame_report.saturating_add(lost);
                per_layer_report.push((*layer, lost));
                writes.push(BakeWrite {
                    layer: *layer,
                    pixels: next,
                    erase: None,
                    report: lost,
                });
            }
            let mut followed = Vec::new();
            for (layer, bind) in self.layer_binds.iter().enumerate() {
                let Some(bind) = bind else {
                    continue;
                };
                let Some(part_index) = self.parts.iter().position(|part| part.name == bind.part)
                else {
                    continue;
                };
                if !pose_included.get(part_index).copied().unwrap_or(false) {
                    continue;
                }
                let steps = &steps_per_part[part_index];
                let follows =
                    managed.get(part_index).copied().unwrap_or(false) || !steps.is_empty();
                if !follows {
                    continue;
                }
                let (next, next_erase, lost) = warp_overlay(
                    self.width,
                    self.height,
                    &source_layers[layer],
                    &source_erase[layer],
                    steps,
                    |x, y| self.accepts_write(x, y),
                )?;
                if is_new {
                    self.reject_ink_outside(&next)?;
                    self.reject_marks_outside(&next_erase)?;
                } else {
                    self.reject_diff_outside(index, layer, &next)?;
                    self.reject_mark_diff(index, layer, &next_erase)?;
                }
                frame_report = frame_report.saturating_add(lost);
                followed.push(layer);
                writes.push(BakeWrite {
                    layer,
                    pixels: next,
                    erase: Some(next_erase),
                    report: lost,
                });
            }
            let mut parts = describe_bake_parts(
                &posed,
                &steps_per_part,
                &write_layers,
                &per_layer_report,
                self.width,
                self.height,
            )?;
            for write in writes.iter().filter(|write| write.erase.is_some()) {
                if let Some(bind) = self
                    .layer_binds
                    .get(write.layer)
                    .and_then(|bind| bind.as_ref())
                {
                    if let Some(part) = parts.iter_mut().find(|part| part.name == bind.part) {
                        part.dropped = part.dropped.saturating_add(write.report.dropped);
                        part.conflicts = part.conflicts.saturating_add(write.report.conflicts);
                        part.sealed = part.sealed.saturating_add(write.report.sealed);
                        part.breaks = part.breaks.saturating_add(write.report.breaks);
                    }
                }
            }
            let delay = delay_ms.unwrap_or(if is_new {
                source_delay
            } else {
                self.frames[index].delay_ms()
            });
            let new_frame = if is_new {
                let mut layers = source_layers.clone();
                let mut erase = source_erase.clone();
                for write in &writes {
                    layers[write.layer] = write.pixels.clone();
                    if let Some(mask) = &write.erase {
                        erase[write.layer] = mask.clone();
                    }
                }
                self.reject_ink_outside_all(&layers)?;
                self.reject_marks_outside_all(&erase)?;
                Some(Frame::from_layers_with_erase(layers, erase, delay))
            } else {
                None
            };
            plans.push(BakePlan {
                index,
                rest: false,
                delay_ms: delay,
                is_new,
                new_frame,
                writes,
                report: frame_report,
                layers: write_layers,
                followed,
                parts,
            });
        }

        let active = self.active;
        let mut changes = Vec::new();
        for plan in &plans {
            if plan.is_new {
                let frame = plan
                    .new_frame
                    .clone()
                    .expect("an appended bake frame stores its pixels");
                let index = self.frames.len();
                debug_assert_eq!(index, plan.index);
                self.frames.push(frame.clone());
                changes.push(Change::InsertFrame {
                    index,
                    frame,
                    active_before: active,
                    active_after: active,
                });
                continue;
            }
            for write in &plan.writes {
                let mut pixels = vec![None; write.pixels.len()];
                for (offset, after) in write.pixels.iter().enumerate() {
                    pixels[offset] = Some(*after);
                }
                self.push_plan(plan.index, write.layer, &pixels, &mut changes);
                if let Some(erase) = &write.erase {
                    self.push_marks(plan.index, write.layer, erase, &mut changes);
                }
            }
            let before = self.frames[plan.index].delay_ms();
            if before != plan.delay_ms {
                self.frames[plan.index].set_delay_ms(plan.delay_ms);
                if plan.index == self.active && self.playback.elapsed_ms >= plan.delay_ms {
                    self.playback.elapsed_ms = 0;
                }
                changes.push(Change::FrameDelay {
                    index: plan.index,
                    before,
                    after: plan.delay_ms,
                });
            }
        }
        let wrote = !changes.is_empty();
        self.history.record_group(changes);
        let appended = plans.iter().filter(|plan| plan.is_new).count();
        let totals = plans.iter().fold(WarpReport::ZERO, |total, plan| {
            total.saturating_add(plan.report)
        });
        let frames = plans
            .iter()
            .map(|plan| BakeFrame {
                index: plan.index,
                rest: plan.rest,
                delay_ms: plan.delay_ms,
                dropped: plan.report.dropped,
                conflicts: plan.report.conflicts,
                sealed: plan.report.sealed,
                breaks: plan.report.breaks,
                layers: plan.layers.clone(),
                followed: plan.followed.clone(),
                parts: plan.parts.clone(),
            })
            .collect();
        Ok(BakeReport {
            from,
            to,
            source,
            fps: options.fps,
            delay_ms,
            constraints: options.constraints,
            appended,
            wrote,
            dropped: totals.dropped,
            conflicts: totals.conflicts,
            sealed: totals.sealed,
            breaks: totals.breaks,
            polish,
            frames,
        })
    }

    fn polish_layers(&self) -> Vec<usize> {
        self.layer_roles
            .iter()
            .enumerate()
            .filter(|(_, role)| **role == LayerRole::Polish)
            .map(|(index, _)| index)
            .collect()
    }

    fn pose_bake(
        &self,
        frame: f64,
        included: &[bool],
        solve: bool,
    ) -> Result<Vec<PosedPart>, KernelError> {
        let included = self.bake_included(included, solve);
        let mut posed = Vec::with_capacity(self.parts.len());
        for (index, part) in self.parts.iter().enumerate() {
            let sample = if included[index] { Some(frame) } else { None };
            posed.push(orient_part(
                part,
                &self.curves,
                sample,
                self.width,
                self.height,
            )?);
        }
        if solve {
            let filtered: Vec<Constraint> = self
                .constraints
                .iter()
                .filter(|constraint| {
                    self.parts
                        .iter()
                        .enumerate()
                        .any(|(index, part)| included[index] && part.name == constraint.part)
                })
                .cloned()
                .collect();
            let order = solve_order(&filtered)?;
            let pixel_frame = frame as usize;
            apply_deltas(
                &mut posed,
                &self.parts,
                &filtered,
                &order,
                self.width,
                self.height,
                Some(frame),
                pixel_frame,
            )?;
        } else {
            for part in &posed {
                place_origin(part, self.width, self.height)?;
            }
        }
        Ok(posed)
    }

    /// Parts whose linked layers a default bake rewrites, including identity.
    ///
    /// A part counts when this bake samples one of its curves, or when
    /// constraint solving drives it. Zero motion is still managed. A part that
    /// is not included, and a part with neither a curve nor a driving
    /// constraint, does not.
    fn bake_managed(&self, included: &[bool], solve: bool) -> Vec<bool> {
        self.parts
            .iter()
            .enumerate()
            .map(|(index, part)| {
                if !included.get(index).copied().unwrap_or(false) {
                    return false;
                }
                if self.curves.iter().any(|curve| curve.part == part.name) {
                    return true;
                }
                if !solve {
                    return false;
                }
                self.constraints.iter().any(|constraint| {
                    let driver_included =
                        self.parts
                            .iter()
                            .enumerate()
                            .any(|(part_index, candidate)| {
                                included[part_index] && candidate.name == constraint.part
                            });
                    driver_included && constraint_drives(constraint, &part.name)
                })
            })
            .collect()
    }

    /// An IK chain is posed together when its end part is selected.
    fn bake_included(&self, included: &[bool], solve: bool) -> Vec<bool> {
        let mut included = included.to_vec();
        if !solve {
            return included;
        }
        for constraint in &self.constraints {
            if constraint.kind != ConstraintKind::Ik {
                continue;
            }
            let end = self
                .parts
                .iter()
                .enumerate()
                .any(|(index, part)| included[index] && part.name == constraint.part);
            if !end {
                continue;
            }
            for name in &constraint.chain {
                if let Some(index) = self.parts.iter().position(|part| part.name == *name) {
                    included[index] = true;
                }
            }
        }
        included
    }

    fn normalize_bake_parts(&self, names: &[String]) -> Result<Vec<String>, KernelError> {
        let mut out = Vec::new();
        for name in names {
            let name = normalize_part_name(name).map_err(|()| KernelError::PartName)?;
            if self.parts.iter().all(|part| part.name != name) {
                return Err(KernelError::PartMissing);
            }
            if !out.iter().any(|existing| existing == &name) {
                out.push(name);
            }
        }
        Ok(out)
    }

    fn normalize_bake_layers(&self, layers: &[usize]) -> Result<Vec<usize>, KernelError> {
        let mut out = Vec::new();
        for &layer in layers {
            self.layer_or_err(layer)?;
            if !out.contains(&layer) {
                out.push(layer);
            }
        }
        Ok(out)
    }

    fn reject_diff_outside(
        &self,
        frame: usize,
        layer: usize,
        next: &[u8],
    ) -> Result<(), KernelError> {
        if self.selection.is_none() {
            return Ok(());
        }
        let current = self.frames[frame].layer_pixels(layer);
        for (offset, (before, after)) in current.iter().zip(next.iter()).enumerate() {
            if before != after && !self.accepts_offset(offset) {
                return Err(KernelError::SelectionProtected);
            }
        }
        Ok(())
    }

    fn reject_ink_outside(&self, pixels: &[u8]) -> Result<(), KernelError> {
        if self.selection.is_none() {
            return Ok(());
        }
        for (offset, index) in pixels.iter().enumerate() {
            if *index != TRANSPARENT_INDEX && !self.accepts_offset(offset) {
                return Err(KernelError::SelectionProtected);
            }
        }
        Ok(())
    }

    fn reject_ink_outside_all(&self, layers: &[Vec<u8>]) -> Result<(), KernelError> {
        for layer in layers {
            self.reject_ink_outside(layer)?;
        }
        Ok(())
    }

    fn reject_marks_outside(&self, marks: &[u8]) -> Result<(), KernelError> {
        if self.selection.is_none() {
            return Ok(());
        }
        for (offset, mark) in marks.iter().enumerate() {
            if *mark != 0 && !self.accepts_offset(offset) {
                return Err(KernelError::SelectionProtected);
            }
        }
        Ok(())
    }

    fn reject_marks_outside_all(&self, layers: &[Vec<u8>]) -> Result<(), KernelError> {
        for layer in layers {
            self.reject_marks_outside(layer)?;
        }
        Ok(())
    }

    fn reject_mark_diff(&self, frame: usize, layer: usize, next: &[u8]) -> Result<(), KernelError> {
        if self.selection.is_none() {
            return Ok(());
        }
        let current = self.frames[frame].layer_erase(layer);
        for (offset, (before, after)) in current.iter().zip(next.iter()).enumerate() {
            if before != after && !self.accepts_offset(offset) {
                return Err(KernelError::SelectionProtected);
            }
        }
        Ok(())
    }

    fn accepts_offset(&self, offset: usize) -> bool {
        let x = u16::try_from(offset % usize::from(self.width)).expect("x fits in u16");
        let y = u16::try_from(offset / usize::from(self.width)).expect("y fits in u16");
        self.accepts_write(x, y)
    }

    fn transform_part(
        &mut self,
        name: &str,
        motion: Motion,
        anchor: Option<&str>,
        frame: usize,
    ) -> Result<PartPose, KernelError> {
        let name = normalize_part_name(name).map_err(|()| KernelError::PartName)?;
        let index = self.part_position(&name)?;
        let frame = self.frame_or_err(frame)?;
        let part = self.parts[index].clone();
        let pivot_anchor = match anchor {
            None => None,
            Some(raw) => Some(normalize_part_name(raw).map_err(|()| KernelError::AnchorName)?),
        };
        let (pivot_x, pivot_y) = match &pivot_anchor {
            None => (i32::from(part.x), i32::from(part.y)),
            Some(anchor_name) => part
                .anchor_canvas(anchor_name)
                .ok_or(KernelError::AnchorMissing)?,
        };
        let resolved = part_xform::resolve_motion(motion)?;
        let (origin_x, origin_y) = self.mapped_canvas_cell(
            i32::from(part.x),
            i32::from(part.y),
            pivot_x,
            pivot_y,
            &resolved.map,
        )?;
        let anchors =
            self.mapped_anchors(&part, origin_x, origin_y, pivot_x, pivot_y, &resolved.map)?;
        let (planned, mut report) =
            self.plan_part_ink(&part, frame, pivot_x, pivot_y, &resolved.map)?;
        let overlays = self.plan_bound_overlays(&name, frame, pivot_x, pivot_y, &resolved.map)?;
        for overlay in &overlays {
            if part.layer == Some(overlay.layer) {
                report = overlay.report;
            } else {
                report = report.saturating_add(overlay.report);
            }
        }
        let mut next_parts = self.parts.clone();
        next_parts[index] = Part {
            x: origin_x,
            y: origin_y,
            anchors,
            ..part
        };
        let parts_changed = next_parts != self.parts;
        let pixels_changed = planned.as_ref().is_some_and(|(_, next, old)| next != old)
            || overlays.iter().any(|overlay| {
                overlay.pixels != overlay.old_pixels || overlay.erase != overlay.old_erase
            });
        let alignment = if resolved.dx.is_some() {
            "translate"
        } else if pivot_anchor.is_some() {
            "center-on-anchor"
        } else {
            "center-on-origin"
        };
        if parts_changed || pixels_changed {
            let mut changes = Vec::new();
            let covered: Vec<usize> = overlays.iter().map(|overlay| overlay.layer).collect();
            if let Some((layer, next, old)) = planned {
                if !covered.contains(&layer) {
                    let mut plan = vec![None; next.len()];
                    for (offset, (before, after)) in old.iter().zip(next.iter()).enumerate() {
                        if before != after {
                            plan[offset] = Some(*after);
                        }
                    }
                    self.push_plan(frame, layer, &plan, &mut changes);
                }
            }
            for overlay in &overlays {
                let mut plan = vec![None; overlay.pixels.len()];
                for (offset, after) in overlay.pixels.iter().enumerate() {
                    plan[offset] = Some(*after);
                }
                self.push_plan(frame, overlay.layer, &plan, &mut changes);
                self.push_marks(frame, overlay.layer, &overlay.erase, &mut changes);
            }
            if parts_changed {
                let before = std::mem::replace(&mut self.parts, next_parts);
                changes.push(Change::Parts {
                    before,
                    after: self.parts.clone(),
                });
            }
            self.history.record_group(changes);
        }
        Ok(PartPose {
            index,
            frame,
            pivot_x,
            pivot_y,
            pivot_anchor,
            alignment,
            sampling: resolved.sampling,
            grid: resolved.grid,
            degrees_centi: resolved.degrees_centi,
            dx: resolved.dx,
            dy: resolved.dy,
            direction: resolved.direction,
            dropped: report.dropped,
            conflicts: report.conflicts,
            sealed: report.sealed,
            breaks: report.breaks,
        })
    }

    fn mapped_canvas_cell(
        &self,
        x: i32,
        y: i32,
        pivot_x: i32,
        pivot_y: i32,
        map: &CellMap,
    ) -> Result<(u16, u16), KernelError> {
        let (x, y) = map_point(map, x, y, pivot_x, pivot_y)
            .map_err(|(x, y)| KernelError::PartOffCanvas { x, y })?;
        self.require_canvas_cell(x, y)
    }

    fn mapped_anchors(
        &self,
        part: &Part,
        origin_x: u16,
        origin_y: u16,
        pivot_x: i32,
        pivot_y: i32,
        map: &CellMap,
    ) -> Result<Vec<Anchor>, KernelError> {
        let mut anchors = Vec::with_capacity(part.anchors.len());
        for anchor in &part.anchors {
            let canvas_x = i32::from(part.x).saturating_add(i32::from(anchor.x));
            let canvas_y = i32::from(part.y).saturating_add(i32::from(anchor.y));
            let (mapped_x, mapped_y) = map_point(map, canvas_x, canvas_y, pivot_x, pivot_y)
                .map_err(|(x, y)| KernelError::PartOffCanvas { x, y })?;
            let local_x = i64::from(mapped_x) - i64::from(origin_x);
            let local_y = i64::from(mapped_y) - i64::from(origin_y);
            let Some((local_x, local_y)) = anchor_offset_i64(local_x, local_y) else {
                return Err(KernelError::AnchorCoord {
                    x: i32_from_i64(local_x),
                    y: i32_from_i64(local_y),
                });
            };
            anchors.push(Anchor {
                name: anchor.name.clone(),
                x: local_x,
                y: local_y,
            });
        }
        Ok(anchors)
    }

    /// New ink buffer for the part's layer, or `None` when the part is unattached.
    ///
    /// The returned old buffer is the layer before the transform. The report
    /// counts collisions, sealed notches, and bare diagonal joins. An
    /// unattached part reports zeros.
    fn plan_part_ink(
        &self,
        part: &Part,
        frame: usize,
        pivot_x: i32,
        pivot_y: i32,
        map: &CellMap,
    ) -> Result<(Option<(usize, Vec<u8>, Vec<u8>)>, WarpReport), KernelError> {
        let Some(layer) = part.layer else {
            return Ok((None, WarpReport::ZERO));
        };
        let layer = self.layer_or_err(layer)?;
        let old = self.frames[frame].layer_pixels(layer).to_vec();
        let steps = [MotionStep {
            map: *map,
            pivot_x,
            pivot_y,
        }];
        let (next, report) = warp_ink(self.width, self.height, &old, &steps, |x, y| {
            self.accepts_write(x, y)
        })?;
        Ok((Some((layer, next, old)), report))
    }

    /// Bound polish overlays that should move with `part_name` on `frame`.
    fn plan_bound_overlays(
        &self,
        part_name: &str,
        frame: usize,
        pivot_x: i32,
        pivot_y: i32,
        map: &CellMap,
    ) -> Result<Vec<OverlayMotion>, KernelError> {
        let steps = [MotionStep {
            map: *map,
            pivot_x,
            pivot_y,
        }];
        let mut plans = Vec::new();
        for (layer, bind) in self.layer_binds.iter().enumerate() {
            let Some(bind) = bind else {
                continue;
            };
            if bind.part != part_name {
                continue;
            }
            let old_pixels = self.frames[frame].layer_pixels(layer).to_vec();
            let old_erase = self.frames[frame].layer_erase(layer).to_vec();
            let (pixels, erase, report) = warp_overlay(
                self.width,
                self.height,
                &old_pixels,
                &old_erase,
                &steps,
                |x, y| self.accepts_write(x, y),
            )?;
            plans.push(OverlayMotion {
                layer,
                pixels,
                erase,
                old_pixels,
                old_erase,
                report,
            });
        }
        Ok(plans)
    }

    fn require_canvas_cell(&self, x: i32, y: i32) -> Result<(u16, u16), KernelError> {
        match (u16::try_from(x), u16::try_from(y)) {
            (Ok(x), Ok(y)) if x < self.width && y < self.height => Ok((x, y)),
            _ => Err(KernelError::PartOffCanvas { x, y }),
        }
    }

    /// True while [`Self::tick`] advances frames.
    pub fn playing(&self) -> bool {
        self.playback.playing
    }

    /// Starts loop playback at the beginning of the active frame.
    ///
    /// A second call while already playing leaves the clock where it is.
    pub fn play(&mut self) {
        if self.playback.playing {
            return;
        }
        self.playback.elapsed_ms = 0;
        self.playback.playing = true;
    }

    /// Stops loop playback and clears the time already shown on this frame.
    pub fn stop(&mut self) {
        self.playback = Playback::stopped();
    }

    /// Advances the preview clock by `delta_ms`.
    ///
    /// Returns whether the active frame changed. The change is not an undo
    /// entry. A stopped document ignores `delta_ms`. Time past the end of the
    /// last frame wraps to the start of the first.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.playback.playing || delta_ms == 0 {
            return false;
        }
        let duration = self.duration_ms();
        if duration == 0 {
            return false;
        }
        let time = self.playback_time_ms().saturating_add(u64::from(delta_ms)) % duration;
        let index = self.frame_at_time(time);
        let changed = index != self.active;
        self.active = index;
        let elapsed = time.saturating_sub(self.frame_start_ms(index));
        self.playback.elapsed_ms = u32::try_from(elapsed).unwrap_or(MAX_FRAME_DELAY_MS);
        changed
    }

    /// Frame under `ratio_ppm` along the timeline.
    ///
    /// `0` is the start of frame 0. [`TIMELINE_RATIO_PPM`] is the end of the
    /// last frame. Each frame occupies a share of the line proportional to
    /// its delay. Values past the scale clamp to the last frame.
    pub fn frame_at_ratio(&self, ratio_ppm: u32) -> usize {
        let last = self.frames.len().saturating_sub(1);
        if last == 0 {
            return 0;
        }
        let ppm = u64::from(ratio_ppm.min(TIMELINE_RATIO_PPM));
        if ppm == 0 {
            return 0;
        }
        if ppm == u64::from(TIMELINE_RATIO_PPM) {
            return last;
        }
        let duration = self.duration_ms();
        if duration == 0 {
            return 0;
        }
        let time = ppm.saturating_mul(duration) / u64::from(TIMELINE_RATIO_PPM);
        self.frame_at_time(time.min(duration - 1))
    }

    /// Stops playback and selects the frame under `ratio_ppm`.
    ///
    /// Selecting the frame that is already active is not recorded. A scrub
    /// drag should sit inside one gesture so the selection collapses to a
    /// single undo step.
    pub fn scrub(&mut self, ratio_ppm: u32) -> Result<(), KernelError> {
        self.stop();
        let index = self.frame_at_ratio(ratio_ppm);
        self.set_active_frame(index)
    }

    /// Groups later successful edits into one undo step until [`Self::end_gesture`].
    ///
    /// A drag should call this once on pointer down. Nested gestures error.
    pub fn begin_gesture(&mut self) -> Result<(), KernelError> {
        self.history.begin_gesture()
    }

    /// Seals the open gesture. Does nothing when no gesture is open. A gesture
    /// that changed nothing does not push an undo entry or clear redo.
    pub fn end_gesture(&mut self) {
        self.history.end_gesture();
    }

    pub fn gesture_open(&self) -> bool {
        self.history.gesture_open()
    }

    /// True when [`Self::undo`] would restore an edit, including an open gesture
    /// that has already changed the document.
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    /// True when [`Self::redo`] would reapply an undone edit. False while an
    /// open gesture already has changes, because that gesture is a new branch.
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Reverts the latest kernel operation.
    ///
    /// An open gesture with edits is sealed and then reverted, so one undo
    /// cancels the in-progress stroke. An empty open gesture is dropped.
    pub fn undo(&mut self) -> Result<(), KernelError> {
        self.stop();
        let operation = self.history.take_undo()?;
        self.history.set_replaying(true);
        for change in operation.changes().iter().rev() {
            self.apply_change(change, false);
        }
        self.history.set_replaying(false);
        self.history.push_redo(operation);
        Ok(())
    }

    /// Reapplies the latest undone operation. Errors if the undo stack's redo
    /// side is empty, or if a gesture in progress already has edits.
    pub fn redo(&mut self) -> Result<(), KernelError> {
        self.stop();
        let operation = self.history.take_redo()?;
        self.history.set_replaying(true);
        for change in operation.changes() {
            self.apply_change(change, true);
        }
        self.history.set_replaying(false);
        self.history.push_undo(operation);
        Ok(())
    }

    /// Drops undo, redo, and any open gesture. Pixels and frames stay as they are.
    ///
    /// Used to mark a seeded document as the baseline the user undoes back to.
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// Rebuilds a stored document. The undo stack starts empty.
    ///
    /// The project loader checks size, palette, frames, and the active frame
    /// before calling this.
    pub(crate) fn from_stored(
        width: u16,
        height: u16,
        palette: Palette,
        palette_locked: bool,
        index_policy: IndexPolicy,
        frames: Vec<Frame>,
        active: usize,
        layer_names: Vec<String>,
        layer_roles: Vec<LayerRole>,
        layer_visible: Vec<bool>,
        layer_opacity: Vec<u8>,
        layer_binds: Vec<Option<PolishBind>>,
        active_layer: usize,
        selection: Option<Mask>,
        tags: Vec<Tag>,
        parts: Vec<Part>,
        curves: Vec<Curve>,
        constraints: Vec<Constraint>,
    ) -> Self {
        debug_assert!(!frames.is_empty(), "project files keep at least one frame");
        debug_assert!(active < frames.len(), "active frame was checked");
        debug_assert!(
            !layer_names.is_empty(),
            "project files keep at least one layer"
        );
        debug_assert!(active_layer < layer_names.len(), "active layer was checked");
        debug_assert_eq!(
            layer_roles.len(),
            layer_names.len(),
            "each layer has a bake role"
        );
        debug_assert_eq!(
            layer_visible.len(),
            layer_names.len(),
            "each layer has a visibility flag"
        );
        debug_assert_eq!(
            layer_opacity.len(),
            layer_names.len(),
            "each layer has an opacity"
        );
        debug_assert_eq!(
            layer_binds.len(),
            layer_names.len(),
            "each layer has a polish binding"
        );
        debug_assert!(
            selection.as_ref().is_none_or(|sel| sel.fits(width, height)),
            "selection was checked"
        );
        debug_assert!(
            tags.iter()
                .all(|tag| tag.from <= tag.to && tag.to < frames.len()),
            "tags were checked"
        );
        debug_assert!(
            parts.iter().all(|part| {
                part.x < width
                    && part.y < height
                    && part.layer.is_none_or(|layer| layer < layer_names.len())
            }),
            "parts were checked"
        );
        debug_assert!(
            curves.iter().all(|curve| {
                parts.iter().any(|part| part.name == curve.part)
                    && !curve.keys.is_empty()
                    && curve
                        .keys
                        .windows(2)
                        .all(|pair| pair[0].frame < pair[1].frame)
                    && curve.keys.iter().all(|key| key.frame < frames.len())
            }),
            "curves were checked"
        );
        debug_assert!(
            constraints.iter().all(|constraint| {
                parts.iter().any(|part| part.name == constraint.part)
                    && constraint
                        .target
                        .as_ref()
                        .is_none_or(|target| parts.iter().any(|part| part.name == *target))
            }),
            "constraints were checked"
        );
        let len = pixel_len(width, height);
        debug_assert!(
            frames.iter().all(|frame| {
                frame.layer_count() == layer_names.len()
                    && frame.layers().iter().all(|pixels| pixels.len() == len)
                    && frame.erasures().len() == layer_names.len()
            }),
            "layer buffers were checked"
        );
        Self {
            width,
            height,
            palette,
            palette_locked,
            index_policy,
            frames,
            active,
            layer_names,
            layer_roles,
            layer_binds,
            layer_visible,
            layer_opacity,
            active_layer,
            selection,
            tags,
            parts,
            curves,
            constraints,
            history: History::new(),
            playback: Playback::stopped(),
        }
    }

    fn write_index(&mut self, frame: usize, layer: usize, offset: usize, value: u8) {
        let mut changes = Vec::new();
        self.put_index(frame, layer, offset, value, &mut changes);
        self.history.record_group(changes);
    }

    /// Writes an index and clears an explicit-erase mark on that cell.
    fn put_index(
        &mut self,
        frame: usize,
        layer: usize,
        offset: usize,
        value: u8,
        changes: &mut Vec<Change>,
    ) {
        debug_assert!(
            value == TRANSPARENT_INDEX || self.palette.contains_paint_index(value),
            "pixels store palette indices"
        );
        let before = self.frames[frame].layer_pixels(layer)[offset];
        if before != value {
            self.frames[frame].set_on(layer, offset, value);
            changes.push(Change::Pixel {
                frame,
                layer,
                offset,
                before,
                after: value,
            });
        }
        self.put_mark(frame, layer, offset, 0, changes);
    }

    /// Index 0 plus an explicit erase. On a default layer this is a normal clear.
    fn put_erase(&mut self, frame: usize, layer: usize, offset: usize, changes: &mut Vec<Change>) {
        let mark = if self.layer_roles[layer] == LayerRole::Polish {
            1
        } else {
            0
        };
        let before = self.frames[frame].layer_pixels(layer)[offset];
        if before != TRANSPARENT_INDEX {
            self.frames[frame].set_on(layer, offset, TRANSPARENT_INDEX);
            changes.push(Change::Pixel {
                frame,
                layer,
                offset,
                before,
                after: TRANSPARENT_INDEX,
            });
        }
        self.put_mark(frame, layer, offset, mark, changes);
    }

    fn put_mark(
        &mut self,
        frame: usize,
        layer: usize,
        offset: usize,
        mark: u8,
        changes: &mut Vec<Change>,
    ) {
        let before = self.frames[frame].layer_erase(layer)[offset];
        if before == mark {
            return;
        }
        self.frames[frame].set_erase(layer, offset, mark);
        changes.push(Change::EraseMark {
            frame,
            layer,
            offset,
            before,
            after: mark,
        });
    }

    fn record_frame_change(
        &mut self,
        before_tags: Vec<Tag>,
        before_curves: Vec<Curve>,
        before_constraints: Vec<Constraint>,
        change: Change,
    ) {
        let mut changes = vec![change];
        if before_tags != self.tags {
            changes.push(Change::Tags {
                before: before_tags,
                after: self.tags.clone(),
            });
        }
        if before_curves != self.curves {
            changes.push(Change::Curves {
                before: before_curves,
                after: self.curves.clone(),
            });
        }
        if before_constraints != self.constraints {
            changes.push(Change::Constraints {
                before: before_constraints,
                after: self.constraints.clone(),
            });
        }
        self.history.record_group(changes);
    }

    fn record_tags(&mut self, before: Vec<Tag>) {
        if before != self.tags {
            self.history.record(Change::Tags {
                before,
                after: self.tags.clone(),
            });
        }
    }

    fn record_with_parts(&mut self, before_parts: Vec<Part>, change: Change) {
        let mut changes = vec![change];
        if before_parts != self.parts {
            changes.push(Change::Parts {
                before: before_parts,
                after: self.parts.clone(),
            });
        }
        self.history.record_group(changes);
    }

    fn record_parts(&mut self, before: Vec<Part>) {
        self.record_parts_with(before, Vec::new());
    }

    fn record_parts_with(&mut self, before: Vec<Part>, mut extra: Vec<Change>) {
        if before != self.parts {
            extra.push(Change::Parts {
                before,
                after: self.parts.clone(),
            });
        }
        self.history.record_group(extra);
    }

    /// Renames or clears polish bindings that name `from`.
    ///
    /// `to: None` drops the binding and leaves the overlay in canvas coordinates.
    fn retarget_binds(&mut self, from: &str, to: Option<&str>) -> Vec<Change> {
        let mut changes = Vec::new();
        for (index, bind) in self.layer_binds.iter_mut().enumerate() {
            let Some(current) = bind.clone() else {
                continue;
            };
            if current.part != from {
                continue;
            }
            let next = match to {
                Some(name) => Some(PolishBind {
                    part: name.to_string(),
                    anchor: current.anchor.clone(),
                }),
                None => None,
            };
            if bind.as_ref() == next.as_ref() {
                continue;
            }
            *bind = next.clone();
            changes.push(Change::LayerBind {
                index,
                before: Some(current),
                after: next,
            });
        }
        changes
    }

    /// A removed anchor falls back to the part origin. Pixels stay.
    fn clear_anchor_binds(&mut self, part: &str, anchor: &str) -> Vec<Change> {
        let mut changes = Vec::new();
        for (index, bind) in self.layer_binds.iter_mut().enumerate() {
            let Some(current) = bind.clone() else {
                continue;
            };
            if current.part != part || current.anchor.as_deref() != Some(anchor) {
                continue;
            }
            let next = PolishBind {
                part: current.part.clone(),
                anchor: None,
            };
            *bind = Some(next.clone());
            changes.push(Change::LayerBind {
                index,
                before: Some(current),
                after: Some(next),
            });
        }
        changes
    }

    fn record_curves(&mut self, before: Vec<Curve>) {
        if before != self.curves {
            self.history.record(Change::Curves {
                before,
                after: self.curves.clone(),
            });
        }
    }

    fn record_rig(
        &mut self,
        before_parts: Vec<Part>,
        before_curves: Vec<Curve>,
        before_constraints: Vec<Constraint>,
        extra: Vec<Change>,
    ) {
        let mut changes = extra;
        if before_parts != self.parts {
            changes.push(Change::Parts {
                before: before_parts,
                after: self.parts.clone(),
            });
        }
        if before_curves != self.curves {
            changes.push(Change::Curves {
                before: before_curves,
                after: self.curves.clone(),
            });
        }
        if before_constraints != self.constraints {
            changes.push(Change::Constraints {
                before: before_constraints,
                after: self.constraints.clone(),
            });
        }
        if !changes.is_empty() {
            self.history.record_group(changes);
        }
    }

    fn record_constraints(&mut self, before: Vec<Constraint>) {
        if before != self.constraints {
            self.history.record(Change::Constraints {
                before,
                after: self.constraints.clone(),
            });
        }
    }

    fn curve_position(&self, part: &str, channel: CurveChannel) -> Option<usize> {
        self.curves
            .iter()
            .position(|curve| curve.part == part && curve.channel == channel)
    }

    fn part_position(&self, name: &str) -> Result<usize, KernelError> {
        self.parts
            .iter()
            .position(|part| part.name == name)
            .ok_or(KernelError::PartMissing)
    }

    fn check_origin(&self, x: u16, y: u16) -> Result<(), KernelError> {
        if x >= self.width || y >= self.height {
            Err(KernelError::OutOfBounds {
                x,
                y,
                width: self.width,
                height: self.height,
            })
        } else {
            Ok(())
        }
    }

    fn tag_position(&self, name: &str) -> Result<usize, KernelError> {
        self.tags
            .iter()
            .position(|tag| tag.name == name)
            .ok_or(KernelError::TagMissing)
    }

    fn check_tag_range(&self, from: usize, to: usize) -> Result<(), KernelError> {
        if from > to || to >= self.frames.len() {
            Err(KernelError::TagRange {
                from,
                to,
                len: self.frames.len(),
            })
        } else {
            Ok(())
        }
    }

    fn blank_frame(&self) -> Frame {
        Frame::blank(
            self.layer_names.len(),
            self.pixel_count(),
            DEFAULT_FRAME_DELAY_MS,
        )
    }

    fn next_layer_name(&self) -> String {
        let mut n = self.layer_names.len() + 1;
        loop {
            let candidate = format!("Layer {n}");
            if !self.layer_names.iter().any(|name| name == &candidate) {
                return candidate;
            }
            n += 1;
        }
    }

    fn move_layer(&mut self, from: usize, to: usize) {
        let name = self.layer_names.remove(from);
        self.layer_names.insert(to, name);
        let role = self.layer_roles.remove(from);
        self.layer_roles.insert(to, role);
        let visible = self.layer_visible.remove(from);
        self.layer_visible.insert(to, visible);
        let opacity = self.layer_opacity.remove(from);
        self.layer_opacity.insert(to, opacity);
        let bind = self.layer_binds.remove(from);
        self.layer_binds.insert(to, bind);
        for frame in &mut self.frames {
            let (pixels, erase) = frame.remove_layer(from);
            frame.insert_cel(to, pixels, erase);
        }
    }

    fn apply_change(&mut self, change: &Change, forward: bool) {
        match change {
            Change::Pixel {
                frame,
                layer,
                offset,
                before,
                after,
            } => {
                let current = self.frames[*frame].layer_pixels(*layer)[*offset];
                let (expected, next) = if forward {
                    (*before, *after)
                } else {
                    (*after, *before)
                };
                debug_assert_eq!(
                    current, expected,
                    "undo stack diverged from the document pixels"
                );
                self.frames[*frame].set_on(*layer, *offset, next);
            }
            Change::EraseMark {
                frame,
                layer,
                offset,
                before,
                after,
            } => {
                let current = self.frames[*frame].layer_erase(*layer)[*offset];
                let (expected, next) = if forward {
                    (*before, *after)
                } else {
                    (*after, *before)
                };
                debug_assert_eq!(current, expected, "undo stack diverged from the erase mask");
                self.frames[*frame].set_erase(*layer, *offset, next);
            }
            Change::AddFrame { index } => {
                if forward {
                    debug_assert_eq!(*index, self.frames.len());
                    self.frames.push(self.blank_frame());
                } else {
                    debug_assert_eq!(self.frames.len() - 1, *index);
                    self.frames.remove(*index);
                    self.clamp_active();
                }
            }
            Change::InsertFrame {
                index,
                frame,
                active_before,
                active_after,
            } => {
                if forward {
                    self.frames.insert(*index, frame.clone());
                    self.assign_active(*active_before, *active_after);
                } else {
                    self.frames.remove(*index);
                    self.assign_active(*active_after, *active_before);
                }
            }
            Change::RemoveFrame {
                index,
                frame,
                active_before,
                active_after,
            } => {
                if forward {
                    self.frames.remove(*index);
                    self.assign_active(*active_before, *active_after);
                } else {
                    self.frames.insert(*index, frame.clone());
                    self.assign_active(*active_after, *active_before);
                }
            }
            Change::ActiveFrame { before, after } => {
                self.active = if forward { *after } else { *before };
                self.playback.elapsed_ms = 0;
            }
            Change::FrameDelay {
                index,
                before,
                after,
            } => {
                let current = self.frames[*index].delay_ms();
                let (expected, next) = if forward {
                    (*before, *after)
                } else {
                    (*after, *before)
                };
                debug_assert_eq!(current, expected, "undo stack diverged from frame delays");
                self.frames[*index].set_delay_ms(next);
                self.clamp_elapsed();
            }
            Change::PaletteColor {
                index,
                before,
                after,
            } => {
                let current = self
                    .palette
                    .get(*index)
                    .expect("palette entry still exists");
                let (expected, next) = if forward {
                    (*before, *after)
                } else {
                    (*after, *before)
                };
                debug_assert_eq!(current, expected, "undo stack diverged from the palette");
                self.palette.write(*index, next);
            }
            Change::AddPaletteColor { index, color } => {
                if forward {
                    debug_assert_eq!(usize::from(*index), self.palette.len());
                    let added = self
                        .palette
                        .push_rgb(color.r, color.g, color.b)
                        .expect("undone palette color still fits");
                    debug_assert_eq!(added, *index);
                } else {
                    debug_assert_eq!(usize::from(*index) + 1, self.palette.len());
                    debug_assert!(
                        !self.frames.iter().any(|frame| frame.contains_index(*index)),
                        "palette index {index} is still stored in a frame"
                    );
                    let removed = self.palette.pop();
                    debug_assert_eq!(removed, *color);
                }
            }
            Change::PaletteReplace { before, after } => {
                let next = if forward { after } else { before };
                self.palette.install(next.clone());
            }
            Change::PaletteLocked { before, after } => {
                self.palette_locked = if forward { *after } else { *before };
            }
            Change::IndexPolicy { before, after } => {
                self.index_policy = if forward { *after } else { *before };
            }
            Change::CanvasResize {
                before_width,
                before_height,
                after_width,
                after_height,
                before_pixels,
                after_pixels,
                before_erase,
                after_erase,
            } => {
                let (width, height, pixels, erase) = if forward {
                    (*after_width, *after_height, after_pixels, after_erase)
                } else {
                    (*before_width, *before_height, before_pixels, before_erase)
                };
                debug_assert_eq!(pixels.len(), self.frames.len());
                debug_assert_eq!(erase.len(), self.frames.len());
                self.width = width;
                self.height = height;
                for (frame, (pixels, erase)) in
                    self.frames.iter_mut().zip(pixels.iter().zip(erase.iter()))
                {
                    frame.set_stack(pixels.clone(), erase.clone());
                }
            }
            Change::AddLayer {
                index,
                name,
                role,
                active_before,
                active_after,
            } => {
                if forward {
                    let blank = vec![TRANSPARENT_INDEX; self.pixel_count()];
                    for frame in &mut self.frames {
                        frame.insert_layer(*index, blank.clone());
                    }
                    self.layer_names.insert(*index, name.clone());
                    self.layer_roles.insert(*index, *role);
                    self.layer_binds.insert(*index, None);
                    self.layer_visible.insert(*index, true);
                    self.layer_opacity.insert(*index, LAYER_OPACITY_OPAQUE);
                    self.active_layer = *active_after;
                } else {
                    self.layer_names.remove(*index);
                    self.layer_roles.remove(*index);
                    self.layer_binds.remove(*index);
                    self.layer_visible.remove(*index);
                    self.layer_opacity.remove(*index);
                    for frame in &mut self.frames {
                        frame.remove_layer(*index);
                    }
                    self.active_layer = *active_before;
                }
            }
            Change::RemoveLayer {
                index,
                name,
                role,
                visible,
                opacity,
                bind,
                pixels,
                erase,
                active_before,
                active_after,
            } => {
                if forward {
                    self.layer_names.remove(*index);
                    self.layer_roles.remove(*index);
                    self.layer_binds.remove(*index);
                    self.layer_visible.remove(*index);
                    self.layer_opacity.remove(*index);
                    for frame in &mut self.frames {
                        frame.remove_layer(*index);
                    }
                    self.active_layer = *active_after;
                } else {
                    debug_assert_eq!(pixels.len(), self.frames.len());
                    debug_assert_eq!(erase.len(), self.frames.len());
                    self.layer_names.insert(*index, name.clone());
                    self.layer_roles.insert(*index, *role);
                    self.layer_binds.insert(*index, bind.clone());
                    self.layer_visible.insert(*index, *visible);
                    self.layer_opacity.insert(*index, *opacity);
                    for (frame, (buffer, mask)) in
                        self.frames.iter_mut().zip(pixels.iter().zip(erase.iter()))
                    {
                        frame.insert_cel(*index, buffer.clone(), mask.clone());
                    }
                    self.active_layer = *active_before;
                }
            }
            Change::RenameLayer {
                index,
                before,
                after,
            } => {
                self.layer_names[*index] = if forward {
                    after.clone()
                } else {
                    before.clone()
                };
            }
            Change::LayerRole {
                index,
                before,
                after,
            } => {
                self.layer_roles[*index] = if forward { *after } else { *before };
            }
            Change::LayerBind {
                index,
                before,
                after,
            } => {
                self.layer_binds[*index] = if forward {
                    after.clone()
                } else {
                    before.clone()
                };
            }
            Change::LayerVisible {
                index,
                before,
                after,
            } => {
                self.layer_visible[*index] = if forward { *after } else { *before };
            }
            Change::LayerOpacity {
                index,
                before,
                after,
            } => {
                self.layer_opacity[*index] = if forward { *after } else { *before };
            }
            Change::ActiveLayer { before, after } => {
                self.active_layer = if forward { *after } else { *before };
            }
            Change::Selection { before, after } => {
                self.selection = if forward {
                    after.clone()
                } else {
                    before.clone()
                };
            }
            Change::Tags { before, after } => {
                self.tags = if forward {
                    after.clone()
                } else {
                    before.clone()
                };
            }
            Change::Parts { before, after } => {
                self.parts = if forward {
                    after.clone()
                } else {
                    before.clone()
                };
            }
            Change::Curves { before, after } => {
                self.curves = if forward {
                    after.clone()
                } else {
                    before.clone()
                };
            }
            Change::Constraints { before, after } => {
                self.constraints = if forward {
                    after.clone()
                } else {
                    before.clone()
                };
            }
            Change::ReorderLayer {
                from,
                to,
                active_before,
                active_after,
            } => {
                if forward {
                    self.move_layer(*from, *to);
                    self.active_layer = *active_after;
                } else {
                    self.move_layer(*to, *from);
                    self.active_layer = *active_before;
                }
            }
            Change::Flatten {
                before_names,
                before_roles,
                before_visible,
                before_opacity,
                before_binds,
                before_pixels,
                before_erase,
                before_active,
                after_pixels,
            } => {
                if forward {
                    debug_assert_eq!(after_pixels.len(), self.frames.len());
                    for (frame, pixels) in self.frames.iter_mut().zip(after_pixels.iter()) {
                        frame.set_stack(vec![pixels.clone()], vec![vec![0; pixels.len()]]);
                    }
                    self.layer_names = vec![DEFAULT_LAYER_NAME.to_string()];
                    self.layer_roles = vec![LayerRole::Default];
                    self.layer_binds = vec![None];
                    self.layer_visible = vec![true];
                    self.layer_opacity = vec![LAYER_OPACITY_OPAQUE];
                    self.active_layer = 0;
                } else {
                    debug_assert_eq!(before_pixels.len(), self.frames.len());
                    debug_assert_eq!(before_erase.len(), self.frames.len());
                    debug_assert_eq!(before_roles.len(), before_names.len());
                    debug_assert_eq!(before_binds.len(), before_names.len());
                    debug_assert_eq!(before_visible.len(), before_names.len());
                    debug_assert_eq!(before_opacity.len(), before_names.len());
                    for (frame, (layers, erase)) in self
                        .frames
                        .iter_mut()
                        .zip(before_pixels.iter().zip(before_erase.iter()))
                    {
                        frame.set_stack(layers.clone(), erase.clone());
                    }
                    self.layer_names = before_names.clone();
                    self.layer_roles = before_roles.clone();
                    self.layer_binds = before_binds.clone();
                    self.layer_visible = before_visible.clone();
                    self.layer_opacity = before_opacity.clone();
                    self.active_layer = *before_active;
                }
            }
        }
    }

    /// Restores an active index a structural edit itself changed. When the edit
    /// left the index numerically unchanged, keep the current selection unless
    /// it now points past the frame list.
    fn assign_active(&mut self, previous: usize, next: usize) {
        if previous != next {
            self.active = next;
        } else {
            self.clamp_active();
        }
    }

    fn clamp_active(&mut self) {
        if self.active >= self.frames.len() {
            self.active = self.frames.len() - 1;
        }
    }

    fn clamp_elapsed(&mut self) {
        let delay = self.frames[self.active].delay_ms();
        if self.playback.elapsed_ms >= delay {
            self.playback.elapsed_ms = 0;
        }
    }

    fn playback_time_ms(&self) -> u64 {
        self.frame_start_ms(self.active)
            .saturating_add(u64::from(self.playback.elapsed_ms))
    }

    fn frame_start_ms(&self, index: usize) -> u64 {
        self.frames[..index]
            .iter()
            .map(|frame| u64::from(frame.delay_ms()))
            .sum()
    }

    fn frame_at_time(&self, time_ms: u64) -> usize {
        let mut cursor = 0u64;
        for (index, frame) in self.frames.iter().enumerate() {
            cursor = cursor.saturating_add(u64::from(frame.delay_ms()));
            if time_ms < cursor {
                return index;
            }
        }
        self.frames.len().saturating_sub(1)
    }

    fn pixel_count(&self) -> usize {
        pixel_len(self.width, self.height)
    }

    /// Writes each cell at most once, in row-major order, as one undo step.
    ///
    /// Cells outside the selection are skipped. A cell that already holds
    /// `color` is skipped. An empty change is not recorded.
    fn stamp_cells(
        &mut self,
        layer: usize,
        color: u8,
        cells: &[(u16, u16)],
    ) -> Result<(), KernelError> {
        let frame = self.active;
        let mut seen = vec![false; self.pixel_count()];
        let mut offsets = Vec::new();
        for &(x, y) in cells {
            if !self.accepts_write(x, y) {
                continue;
            }
            let offset = pixel_len(self.width, y) + usize::from(x);
            if seen[offset] {
                continue;
            }
            seen[offset] = true;
            if self.frames[frame].layer_pixels(layer)[offset] == color
                && self.frames[frame].layer_erase(layer)[offset] == 0
            {
                continue;
            }
            offsets.push(offset);
        }
        offsets.sort_unstable();
        let mut changes = Vec::with_capacity(offsets.len());
        for offset in offsets {
            self.put_index(frame, layer, offset, color, &mut changes);
        }
        self.history.record_group(changes);
        Ok(())
    }

    /// Maps raster cells onto the canvas. One cell outside fails the whole call.
    fn canvas_cells(&self, cells: &[(i32, i32)]) -> Result<Vec<(u16, u16)>, KernelError> {
        let mut out = Vec::with_capacity(cells.len());
        for &(x, y) in cells {
            if x < 0 || y < 0 || x >= i32::from(self.width) || y >= i32::from(self.height) {
                let x = u16::try_from(x).unwrap_or(0);
                let y = u16::try_from(y).unwrap_or(0);
                return Err(KernelError::OutOfBounds {
                    x,
                    y,
                    width: self.width,
                    height: self.height,
                });
            }
            out.push((
                u16::try_from(x).expect("x was checked"),
                u16::try_from(y).expect("y was checked"),
            ));
        }
        Ok(out)
    }

    /// A cell outside the selection is protected. No selection accepts every cell.
    fn accepts_write(&self, x: u16, y: u16) -> bool {
        self.selection.as_ref().is_none_or(|sel| sel.contains(x, y))
    }

    /// True when the whole rectangle is writable. No selection accepts every rect
    /// that the caller has already placed on the canvas.
    fn rect_inside_selection(&self, x: u16, y: u16, width: u16, height: u16) -> bool {
        match &self.selection {
            None => true,
            Some(sel) => sel.covers_rect(x, y, width, height),
        }
    }

    fn stored_index(&self, index: u8) -> Result<u8, KernelError> {
        if usize::from(index) < self.palette.len() {
            Ok(index)
        } else {
            Err(KernelError::PaletteIndex { index })
        }
    }

    /// Index 0 clears cells. Any other index is resolved like [`Self::paint`].
    fn resolve_replace_index(&self, index: u8) -> Result<u8, KernelError> {
        if index == TRANSPARENT_INDEX {
            Ok(TRANSPARENT_INDEX)
        } else {
            self.resolve_paint_index(index)
        }
    }

    fn offset(&self, x: u16, y: u16) -> Result<usize, KernelError> {
        if x >= self.width || y >= self.height {
            return Err(KernelError::OutOfBounds {
                x,
                y,
                width: self.width,
                height: self.height,
            });
        }
        Ok(pixel_len(self.width, y) + usize::from(x))
    }
}

/// Clockwise quarter turns. 90° and 270° stay inside a square rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuarterTurn {
    /// 90 degrees clockwise.
    Cw90,
    /// 180 degrees.
    Cw180,
    /// 270 degrees clockwise.
    Cw270,
}

#[derive(Clone, Copy)]
enum Turn {
    Horizontal,
    Vertical,
    Cw90,
    Cw180,
    Cw270,
}

/// Local source cell whose index lands on local `(x, y)`.
fn source_local(turn: Turn, width: u16, height: u16, x: u16, y: u16) -> (u16, u16) {
    match turn {
        Turn::Horizontal => (width - 1 - x, y),
        Turn::Vertical => (x, height - 1 - y),
        Turn::Cw180 => (width - 1 - x, height - 1 - y),
        Turn::Cw90 => (y, height - 1 - x),
        Turn::Cw270 => (width - 1 - y, x),
    }
}

fn check_size(width: u16, height: u16) -> Result<(), KernelError> {
    if width == 0 || height == 0 || width > MAX_CANVAS_EDGE || height > MAX_CANVAS_EDGE {
        Err(KernelError::InvalidSize { width, height })
    } else {
        Ok(())
    }
}

fn i32_from_i64(value: i64) -> i32 {
    i32::try_from(value).unwrap_or(if value > 0 { i32::MAX } else { i32::MIN })
}

fn pixel_len(width: u16, height: u16) -> usize {
    usize::from(width) * usize::from(height)
}

fn normalize_layer_name(name: &str) -> Result<String, KernelError> {
    let trimmed = name.trim();
    let chars = trimmed.chars().count();
    if trimmed.is_empty() || chars > MAX_LAYER_NAME_CHARS || trimmed.chars().any(char::is_control) {
        Err(KernelError::LayerName)
    } else {
        Ok(trimmed.to_string())
    }
}

/// Where `active` sits after `move_layer(from, to)`.
fn remap_layer_index(active: usize, from: usize, to: usize) -> usize {
    if active == from {
        to
    } else if from < to && active > from && active <= to {
        active - 1
    } else if to < from && active >= to && active < from {
        active + 1
    } else {
        active
    }
}

/// Copies `source` onto a new buffer. `offset_x`/`offset_y` is where the old
/// origin lands. Cells outside either canvas are skipped.
fn place_pixels(
    source: &[u8],
    old_width: u16,
    old_height: u16,
    new_width: u16,
    new_height: u16,
    offset_x: i32,
    offset_y: i32,
) -> Vec<u8> {
    let mut next = vec![TRANSPARENT_INDEX; pixel_len(new_width, new_height)];
    let old_stride = usize::from(old_width);
    let new_stride = usize::from(new_width);
    for y in 0..u32::from(old_height) {
        for x in 0..u32::from(old_width) {
            let nx = x as i32 + offset_x;
            let ny = y as i32 + offset_y;
            if nx < 0 || ny < 0 || nx >= i32::from(new_width) || ny >= i32::from(new_height) {
                continue;
            }
            let src = y as usize * old_stride + x as usize;
            let dst = ny as usize * new_stride + nx as usize;
            next[dst] = source[src];
        }
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        PaletteImportMode, Rgba, DEFAULT_FRAME_DELAY_MS, MAX_FRAME_DELAY_MS, TIMELINE_RATIO_PPM,
    };

    fn doc() -> Document {
        Document::new(16, 16).unwrap()
    }

    #[test]
    fn new_document_is_transparent() {
        let document = doc();
        assert_eq!(document.width(), 16);
        assert_eq!(document.height(), 16);
        assert_eq!(document.frame_count(), 1);
        assert_eq!(document.active_frame(), 0);
        assert_eq!(document.pixels().len(), 16 * 16);
        assert!(document
            .pixels()
            .iter()
            .all(|index| *index == TRANSPARENT_INDEX));
    }

    #[test]
    fn accepts_m1_canvas_sizes() {
        for size in [16_u16, 32] {
            let mut document = Document::new(size, size).unwrap();
            document.paint(size - 1, size - 1, 1).unwrap();
            assert_eq!(document.get(size - 1, size - 1).unwrap(), 1);
        }
    }

    #[test]
    fn rejects_empty_and_oversized_canvas() {
        assert!(matches!(
            Document::new(0, 16),
            Err(KernelError::InvalidSize {
                width: 0,
                height: 16
            })
        ));
        assert!(matches!(
            Document::new(257, 1),
            Err(KernelError::InvalidSize {
                width: 257,
                height: 1
            })
        ));
    }

    #[test]
    fn game_sprite_edges_96_and_128_resize_fill_export_and_layers() {
        for (width, height) in [(96_u16, 96), (128, 128), (96, 128), (64, 128)] {
            let document = Document::new(width, height).unwrap();
            assert_eq!((document.width(), document.height()), (width, height));
            assert_eq!(
                document.pixels().len(),
                usize::from(width) * usize::from(height)
            );
        }
        assert!(matches!(
            Document::new(257, 128),
            Err(KernelError::InvalidSize {
                width: 257,
                height: 128
            })
        ));

        let mut document = Document::new(64, 64).unwrap();
        assert!(matches!(
            document.resize_canvas(257, 96, 0, 0),
            Err(KernelError::InvalidSize {
                width: 257,
                height: 96
            })
        ));
        assert_eq!((document.width(), document.height()), (64, 64));
        document.paint(0, 0, 1).unwrap();
        document.paint(63, 63, 2).unwrap();
        document.resize_canvas(96, 96, 0, 0).unwrap();
        assert_eq!((document.width(), document.height()), (96, 96));
        assert_eq!(document.pixels().len(), 96 * 96);
        assert_eq!(document.get(0, 0).unwrap(), 1);
        assert_eq!(document.get(63, 63).unwrap(), 2);
        assert_eq!(document.get(95, 95).unwrap(), TRANSPARENT_INDEX);

        document.resize_canvas(128, 128, 0, 0).unwrap();
        assert_eq!((document.width(), document.height()), (128, 128));
        assert_eq!(document.pixels().len(), 128 * 128);
        assert_eq!(document.get(0, 0).unwrap(), 1);
        assert_eq!(document.get(63, 63).unwrap(), 2);
        assert_eq!(document.get(127, 127).unwrap(), TRANSPARENT_INDEX);

        document.fill_rect(100, 100, 20, 20, 4).unwrap();
        assert_eq!(document.get(100, 100).unwrap(), 4);
        assert_eq!(document.get(119, 119).unwrap(), 4);
        assert_eq!(document.get(120, 100).unwrap(), TRANSPARENT_INDEX);
        assert!(matches!(
            document.fill_rect(120, 120, 20, 20, 4),
            Err(KernelError::OutOfBounds { .. })
        ));
        assert_eq!(document.get(120, 120).unwrap(), TRANSPARENT_INDEX);
        assert_eq!(document.get(0, 0).unwrap(), 1);

        assert_eq!(png_edge(&document.export_frame_png(0).unwrap()), (128, 128));
        assert_eq!(png_edge(&document.export_sprite_sheet()), (128, 128));
        assert_eq!(
            png_edge(&document.export_frame_png_scaled(0, 2).unwrap()),
            (256, 256)
        );

        let gear = document.add_layer(Some("gear")).unwrap();
        assert_eq!(gear, 1);
        document.fill_rect_on(1, 10, 10, 8, 8, 5).unwrap();
        assert_eq!(document.layer_pixels(0, 0).unwrap()[10 * 128 + 10], 0);
        assert_eq!(document.composite_pixels(0).unwrap()[10 * 128 + 10], 5);
        assert_eq!(png_edge(&document.export_frame_png(0).unwrap()), (128, 128));
        assert_eq!((document.width(), document.height()), (128, 128));
    }

    fn png_edge(bytes: &[u8]) -> (u32, u32) {
        assert!(
            bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
            "png magic, got {:02x?}",
            &bytes[..bytes.len().min(8)]
        );
        let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
        (width, height)
    }

    #[test]
    fn paint_and_erase_one_cell() {
        let mut document = doc();
        document.paint(2, 3, 4).unwrap();
        assert_eq!(document.get(2, 3).unwrap(), 4);
        assert_eq!(document.get(2, 4).unwrap(), TRANSPARENT_INDEX);
        document.erase(2, 3).unwrap();
        assert_eq!(document.get(2, 3).unwrap(), TRANSPARENT_INDEX);
    }

    #[test]
    fn paint_rejects_transparent_unknown_and_out_of_bounds() {
        let mut document = doc();
        document.paint(1, 1, 2).unwrap();

        assert_eq!(
            document.paint(0, 0, TRANSPARENT_INDEX).unwrap_err(),
            KernelError::PaletteIndex { index: 0 }
        );
        assert_eq!(
            document.paint(0, 0, 99).unwrap_err(),
            KernelError::PaletteIndex { index: 99 }
        );
        assert!(matches!(
            document.paint(16, 0, 1),
            Err(KernelError::OutOfBounds { x: 16, y: 0, .. })
        ));
        assert!(matches!(
            document.erase(0, 16),
            Err(KernelError::OutOfBounds { x: 0, y: 16, .. })
        ));
        assert_eq!(document.get(1, 1).unwrap(), 2);
        assert_eq!(document.get(0, 0).unwrap(), TRANSPARENT_INDEX);
    }

    #[test]
    fn fill_rect_is_one_undo_and_refuses_a_partial_rectangle() {
        let mut document = doc();
        document.fill_rect(1, 2, 3, 2, 4).unwrap();
        for y in 2..4 {
            for x in 1..4 {
                assert_eq!(document.get(x, y).unwrap(), 4);
            }
        }
        assert_eq!(document.get(0, 2).unwrap(), TRANSPARENT_INDEX);
        assert_eq!(document.get(4, 2).unwrap(), TRANSPARENT_INDEX);
        assert_eq!(document.get(1, 4).unwrap(), TRANSPARENT_INDEX);
        document.paint(0, 0, 1).unwrap();
        document.undo().unwrap();
        assert_eq!(document.get(0, 0).unwrap(), TRANSPARENT_INDEX);
        assert_eq!(document.get(1, 2).unwrap(), 4);
        document.undo().unwrap();
        assert!(document
            .pixels()
            .iter()
            .all(|index| *index == TRANSPARENT_INDEX));
        assert!(!document.can_undo());
        document.redo().unwrap();
        assert_eq!(document.get(2, 3).unwrap(), 4);
        assert_eq!(document.get(0, 0).unwrap(), TRANSPARENT_INDEX);

        document.fill_rect(1, 2, 3, 2, 4).unwrap();
        document.undo().unwrap();
        assert_eq!(document.get(1, 2).unwrap(), TRANSPARENT_INDEX);

        document.fill_rect(14, 0, 2, 2, 5).unwrap();
        assert_eq!(document.get(15, 1).unwrap(), 5);
        let before = document.pixels().to_vec();
        assert!(matches!(
            document.fill_rect(14, 0, 4, 1, 2),
            Err(KernelError::OutOfBounds { x: 16, y: 0, .. })
        ));
        assert_eq!(document.pixels(), before.as_slice());
        assert_eq!(
            document
                .fill_rect(0, 0, 1, 1, TRANSPARENT_INDEX)
                .unwrap_err(),
            KernelError::PaletteIndex { index: 0 }
        );
        assert_eq!(
            document.fill_rect(20, 0, 1, 1, 0).unwrap_err(),
            KernelError::PaletteIndex { index: 0 }
        );
        assert!(document.fill_rect(0, 0, 0, 2, 1).is_err());
        assert_eq!(document.pixels(), before.as_slice());

        document.set_palette_locked(true);
        document.fill_rect(0, 1, 2, 1, 3).unwrap();
        assert_eq!(document.get(0, 1).unwrap(), 3);
        assert_eq!(document.get(1, 1).unwrap(), 3);
        assert!(document.palette_locked());

        let other = document.add_frame();
        document.set_active_frame(other).unwrap();
        assert!(document
            .pixels()
            .iter()
            .all(|index| *index == TRANSPARENT_INDEX));
        assert_eq!(document.frame_pixels(0).unwrap()[16], 3);

        document.begin_gesture().unwrap();
        document.fill_rect(0, 0, 2, 1, 6).unwrap();
        document.paint(3, 0, 7).unwrap();
        document.end_gesture();
        document.undo().unwrap();
        assert_eq!(document.get(0, 0).unwrap(), TRANSPARENT_INDEX);
        assert_eq!(document.get(3, 0).unwrap(), TRANSPARENT_INDEX);
        assert!(document.can_redo());
        assert_eq!(document.frame_pixels(0).unwrap()[16], 3);
    }

    #[test]
    fn erase_rect_is_one_undo_and_refuses_a_partial_rectangle() {
        let mut document = doc();
        document.fill_rect(1, 2, 3, 2, 4).unwrap();
        document.paint(0, 0, 1).unwrap();
        document.erase_rect(1, 2, 2, 2).unwrap();
        assert_eq!(document.get(1, 2).unwrap(), TRANSPARENT_INDEX);
        assert_eq!(document.get(2, 3).unwrap(), TRANSPARENT_INDEX);
        assert_eq!(document.get(3, 2).unwrap(), 4);
        assert_eq!(document.get(0, 0).unwrap(), 1);
        document.undo().unwrap();
        assert_eq!(document.get(1, 2).unwrap(), 4);
        assert_eq!(document.get(2, 3).unwrap(), 4);
        assert_eq!(document.get(0, 0).unwrap(), 1);
        document.redo().unwrap();
        assert_eq!(document.get(1, 2).unwrap(), TRANSPARENT_INDEX);

        document.erase_rect(1, 2, 2, 2).unwrap();
        document.undo().unwrap();
        assert_eq!(document.get(1, 2).unwrap(), 4);
        assert_eq!(document.get(0, 0).unwrap(), 1);

        let before = document.pixels().to_vec();
        assert!(matches!(
            document.erase_rect(14, 0, 4, 1),
            Err(KernelError::OutOfBounds { x: 16, y: 0, .. })
        ));
        assert!(document.erase_rect(0, 0, 0, 1).is_err());
        assert_eq!(document.pixels(), before.as_slice());

        document.set_palette_locked(true);
        document.erase_rect(0, 0, 1, 1).unwrap();
        assert_eq!(document.get(0, 0).unwrap(), TRANSPARENT_INDEX);
        assert!(document.palette_locked());

        let other = document.add_frame();
        document.paint(0, 0, 2).unwrap();
        document.set_active_frame(other).unwrap();
        document.paint(0, 0, 5).unwrap();
        document.erase_rect(0, 0, 1, 1).unwrap();
        assert_eq!(document.get(0, 0).unwrap(), TRANSPARENT_INDEX);
        assert_eq!(document.frame_pixels(0).unwrap()[0], 2);

        document.paint(1, 0, 7).unwrap();
        document.begin_gesture().unwrap();
        document.erase_rect(1, 0, 1, 1).unwrap();
        document.paint(2, 0, 6).unwrap();
        document.end_gesture();
        document.undo().unwrap();
        assert_eq!(document.get(1, 0).unwrap(), 7);
        assert_eq!(document.get(2, 0).unwrap(), TRANSPARENT_INDEX);
        assert_eq!(document.frame_pixels(0).unwrap()[0], 2);
    }

    #[test]
    fn copy_and_move_rects_are_one_undo_and_refuse_a_partial() {
        let mut document = Document::new(4, 1).unwrap();
        document.paint(0, 0, 1).unwrap();
        document.paint(1, 0, 2).unwrap();
        document.paint(2, 0, 3).unwrap();
        document.clear_history();

        document.copy_rect(0, 0, 2, 1, 2, 0).unwrap();
        assert_eq!(document.pixels(), &[1, 2, 1, 2]);
        document.undo().unwrap();
        assert_eq!(document.pixels(), &[1, 2, 3, 0]);
        assert!(!document.can_undo());
        document.redo().unwrap();
        assert_eq!(document.pixels(), &[1, 2, 1, 2]);
        document.undo().unwrap();

        document.copy_rect(0, 0, 2, 1, 1, 0).unwrap();
        assert_eq!(document.pixels(), &[1, 1, 2, 0]);
        document.undo().unwrap();
        assert_eq!(document.pixels(), &[1, 2, 3, 0]);

        let hole = document.pixels().to_vec();
        document.erase(1, 0).unwrap();
        document.copy_rect(0, 0, 2, 1, 2, 0).unwrap();
        assert_eq!(document.pixels(), &[1, 0, 1, 0]);
        document.undo().unwrap();
        document.undo().unwrap();
        assert_eq!(document.pixels(), hole.as_slice());

        document.copy_rect(0, 0, 2, 1, 0, 0).unwrap();
        assert_eq!(document.pixels(), &[1, 2, 3, 0]);
        assert!(!document.can_undo());

        document.move_rect(0, 0, 2, 1, 2, 0).unwrap();
        assert_eq!(document.pixels(), &[0, 0, 1, 2]);
        document.undo().unwrap();
        assert_eq!(document.pixels(), &[1, 2, 3, 0]);
        document.redo().unwrap();
        document.undo().unwrap();

        document.move_rect(0, 0, 2, 1, 1, 0).unwrap();
        assert_eq!(document.pixels(), &[0, 1, 2, 0]);
        document.undo().unwrap();
        assert_eq!(document.pixels(), &[1, 2, 3, 0]);

        document.move_rect(0, 0, 2, 1, 0, 0).unwrap();
        assert_eq!(document.pixels(), &[1, 2, 3, 0]);
        assert!(!document.can_undo());

        let before = document.pixels().to_vec();
        assert!(matches!(
            document.copy_rect(2, 0, 3, 1, 0, 0),
            Err(KernelError::OutOfBounds { .. })
        ));
        assert!(matches!(
            document.move_rect(0, 0, 2, 1, 3, 0),
            Err(KernelError::OutOfBounds { .. })
        ));
        assert_eq!(document.pixels(), before.as_slice());
        assert!(!document.can_undo());

        document.set_palette_locked(true);
        document.copy_rect(1, 0, 1, 1, 3, 0).unwrap();
        assert_eq!(document.get(3, 0).unwrap(), 2);
        document.move_rect(1, 0, 1, 1, 0, 0).unwrap();
        assert_eq!(document.pixels(), &[2, 0, 3, 2]);
        assert!(document.palette_locked());
        document.undo().unwrap();
        assert_eq!(document.pixels(), &[1, 2, 3, 2]);
        document.undo().unwrap();
        assert_eq!(document.pixels(), &[1, 2, 3, 0]);

        let kept = document.pixels().to_vec();
        let other = document.add_frame();
        document.set_active_frame(other).unwrap();
        document.paint(0, 0, 5).unwrap();
        document.copy_rect(0, 0, 1, 1, 1, 0).unwrap();
        assert_eq!(document.pixels(), &[5, 5, 0, 0]);
        assert_eq!(document.frame_pixels(0).unwrap(), kept.as_slice());

        document.begin_gesture().unwrap();
        document.move_rect(0, 0, 1, 1, 2, 0).unwrap();
        document.paint(3, 0, 6).unwrap();
        document.end_gesture();
        assert_eq!(document.pixels(), &[0, 5, 5, 6]);
        document.undo().unwrap();
        assert_eq!(document.pixels(), &[5, 5, 0, 0]);
        assert_eq!(document.frame_pixels(0).unwrap(), kept.as_slice());
    }

    #[test]
    fn copy_and_move_can_read_another_frame() {
        let mut document = Document::new(4, 2).unwrap();
        document.paint(0, 0, 1).unwrap();
        document.paint(1, 0, 2).unwrap();
        document.paint(0, 1, 3).unwrap();
        document.add_frame();
        document.set_active_frame(1).unwrap();
        document.paint(3, 1, 4).unwrap();
        document.clear_history();

        document.copy_rect_from(0, 0, 0, 2, 2, 1, 1, 0).unwrap();
        assert_eq!(document.active_frame(), 1);
        assert_eq!(document.frame_pixels(0).unwrap()[0], 1);
        assert_eq!(document.frame_pixels(0).unwrap()[1], 2);
        assert_eq!(document.get(1, 0).unwrap(), 1);
        assert_eq!(document.get(2, 0).unwrap(), 2);
        assert_eq!(document.get(1, 1).unwrap(), 3);
        assert_eq!(document.get(3, 1).unwrap(), 4);
        document.undo().unwrap();
        assert_eq!(document.get(1, 0).unwrap(), 0);
        assert_eq!(document.get(1, 1).unwrap(), 0);
        assert_eq!(document.get(3, 1).unwrap(), 4);

        document.move_rect_from(0, 0, 0, 2, 1, 1, 0, 0).unwrap();
        assert_eq!(document.frame_pixels(0).unwrap()[0], 0);
        assert_eq!(document.frame_pixels(0).unwrap()[1], 0);
        assert_eq!(document.frame_pixels(0).unwrap()[4], 3);
        assert_eq!(document.get(0, 0).unwrap(), 1);
        assert_eq!(document.get(1, 0).unwrap(), 2);
        document.undo().unwrap();
        assert_eq!(document.frame_pixels(0).unwrap()[0], 1);
        assert_eq!(document.frame_pixels(0).unwrap()[1], 2);
        assert_eq!(document.get(0, 0).unwrap(), 0);

        let before = document.frame_pixels(0).unwrap().to_vec();
        assert!(matches!(
            document.copy_rect_from(2, 0, 0, 1, 1, 1, 0, 0),
            Err(KernelError::FrameIndex { index: 2, len: 2 })
        ));
        assert!(matches!(
            document.move_rect_from(0, 0, 0, 1, 1, 1, 4, 0),
            Err(KernelError::OutOfBounds { .. })
        ));
        assert_eq!(document.frame_pixels(0).unwrap(), before.as_slice());
        assert!(!document.can_undo());
    }

    #[test]
    fn flip_horizontal_mirrors_indices_and_keeps_colors() {
        let mut document = Document::new(4, 2).unwrap();
        document.paint(0, 0, 1).unwrap();
        document.paint(1, 0, 2).unwrap();
        document.paint(0, 1, 3).unwrap();
        document.set_palette_color(1, 9, 8, 7).unwrap();
        let color = document.palette_colors()[1];
        document.clear_history();

        document.flip_horizontal(0, 0, 0, 4, 2).unwrap();
        assert_eq!(document.pixels(), &[0, 0, 2, 1, 0, 0, 0, 3]);
        assert_eq!(document.palette_colors()[1], color);
        document.undo().unwrap();
        assert_eq!(document.pixels(), &[1, 2, 0, 0, 3, 0, 0, 0]);

        document.flip_horizontal(0, 0, 0, 3, 1).unwrap();
        assert_eq!(&document.pixels()[..4], &[0, 2, 1, 0]);
        assert_eq!(document.pixels()[4], 3);
        document.clear_history();
        document.flip_horizontal(0, 1, 0, 1, 1).unwrap();
        assert!(!document.can_undo());

        document.add_frame();
        document.set_active_frame(1).unwrap();
        document.flip_horizontal(0, 0, 1, 4, 1).unwrap();
        assert_eq!(document.active_frame(), 1);
        assert_eq!(&document.frame_pixels(0).unwrap()[4..8], &[0, 0, 0, 3]);
        assert!(document
            .frame_pixels(1)
            .unwrap()
            .iter()
            .all(|index| *index == 0));
        document.undo().unwrap();
        assert_eq!(document.frame_pixels(0).unwrap()[4], 3);

        assert!(matches!(
            document.flip_horizontal(0, 2, 0, 3, 1),
            Err(KernelError::OutOfBounds { .. })
        ));
        assert!(matches!(
            document.flip_horizontal(3, 0, 0, 1, 1),
            Err(KernelError::FrameIndex { index: 3, .. })
        ));
    }

    #[test]
    fn flip_vertical_and_rotate_move_indices_without_recoloring() {
        let mut document = Document::new(4, 2).unwrap();
        document.paint(0, 0, 1).unwrap();
        document.paint(1, 0, 2).unwrap();
        document.paint(0, 1, 3).unwrap();
        document.set_palette_color(1, 9, 8, 7).unwrap();
        let color = document.palette_colors()[1];
        document.clear_history();

        document.flip_vertical(0, 0, 0, 4, 2).unwrap();
        assert_eq!(document.pixels(), &[3, 0, 0, 0, 1, 2, 0, 0]);
        assert_eq!(document.palette_colors()[1], color);
        document.undo().unwrap();
        assert_eq!(document.pixels(), &[1, 2, 0, 0, 3, 0, 0, 0]);

        document.rotate(0, 0, 0, 2, 2, QuarterTurn::Cw180).unwrap();
        assert_eq!(document.pixels(), &[0, 3, 0, 0, 2, 1, 0, 0]);
        document.undo().unwrap();

        let err = document
            .rotate(0, 0, 0, 4, 2, QuarterTurn::Cw90)
            .unwrap_err();
        assert!(matches!(
            err,
            KernelError::RotateSquare {
                width: 4,
                height: 2
            }
        ));
        assert_eq!(document.pixels(), &[1, 2, 0, 0, 3, 0, 0, 0]);
        assert!(!document.can_undo());

        document.rotate(0, 0, 0, 2, 2, QuarterTurn::Cw90).unwrap();
        // 1 2
        // 3 0
        // clockwise -> 3 1 / 0 2, leaving the right column alone.
        assert_eq!(document.pixels(), &[3, 1, 0, 0, 0, 2, 0, 0]);
        document.rotate(0, 0, 0, 2, 2, QuarterTurn::Cw270).unwrap();
        assert_eq!(document.pixels(), &[1, 2, 0, 0, 3, 0, 0, 0]);
        assert_eq!(document.palette_colors()[1], color);
    }

    #[test]
    fn resize_canvas_places_frames_and_undoes() {
        let mut document = Document::new(4, 2).unwrap();
        document.paint(0, 0, 1).unwrap();
        document.paint(3, 1, 2).unwrap();
        document.add_frame();
        document.set_frame_delay(1, 80).unwrap();
        document.set_active_frame(1).unwrap();
        document.paint(1, 0, 3).unwrap();
        document.set_palette_locked(true);
        document.clear_history();

        document.resize_canvas(6, 4, 1, 1).unwrap();
        assert_eq!((document.width(), document.height()), (6, 4));
        assert_eq!(document.active_frame(), 1);
        assert_eq!(document.frame_delay(1).unwrap(), 80);
        assert!(document.palette_locked());
        assert_eq!(document.frame_pixels(0).unwrap()[1 * 6 + 1], 1);
        assert_eq!(document.frame_pixels(0).unwrap()[2 * 6 + 4], 2);
        assert_eq!(document.frame_pixels(0).unwrap()[0], 0);
        assert_eq!(document.get(2, 1).unwrap(), 3);
        let json = document.save_project();
        let loaded = Document::load_project(&json).unwrap();
        assert_eq!(loaded.width(), 6);
        assert_eq!(loaded.frame_pixels(0).unwrap()[1 * 6 + 1], 1);
        assert_eq!(loaded.frame_delay(1).unwrap(), 80);

        document.undo().unwrap();
        assert_eq!((document.width(), document.height()), (4, 2));
        assert_eq!(document.frame_pixels(0).unwrap()[0], 1);
        assert_eq!(document.frame_pixels(0).unwrap()[7], 2);
        assert_eq!(document.frame_pixels(1).unwrap()[1], 3);
        document.redo().unwrap();
        assert_eq!(document.width(), 6);

        document.resize_canvas(6, 4, 0, 0).unwrap();
        assert!(document.can_undo());
        assert!(!document.can_redo());
        document.undo().unwrap();
        assert_eq!(document.width(), 4);
        assert!(!document.can_undo());

        document.resize_canvas(3, 2, 0, 0).unwrap();
        assert_eq!(document.frame_pixels(0).unwrap(), &[1, 0, 0, 0, 0, 0]);
        document.undo().unwrap();
        assert_eq!(document.frame_pixels(0).unwrap()[7], 2);

        document.resize_canvas(4, 2, -1, 0).unwrap();
        assert_eq!(document.frame_pixels(0).unwrap()[0], 0);
        assert_eq!(document.frame_pixels(0).unwrap()[6], 2);
        assert!(matches!(
            document.resize_canvas(0, 2, 0, 0),
            Err(KernelError::InvalidSize { width: 0, .. })
        ));
        assert_eq!(document.width(), 4);
    }

    #[test]
    fn frames_do_not_share_pixels() {
        let mut document = doc();
        document.paint(1, 1, 2).unwrap();
        let added = document.add_frame();
        assert_eq!(added, 1);
        assert_eq!(document.active_frame(), 0);

        document.set_active_frame(1).unwrap();
        assert!(document
            .pixels()
            .iter()
            .all(|index| *index == TRANSPARENT_INDEX));
        document.paint(1, 1, 3).unwrap();

        document.set_active_frame(0).unwrap();
        assert_eq!(document.get(1, 1).unwrap(), 2);
        assert_eq!(document.frame_pixels(1).unwrap()[1 + 16], 3);
    }

    #[test]
    fn duplicate_copies_pixels_and_keeps_active_content() {
        let mut document = doc();
        document.paint(0, 0, 1).unwrap();
        document.add_frame();
        document.set_active_frame(1).unwrap();

        let copy = document.duplicate_frame(0).unwrap();
        assert_eq!(copy, 1);
        assert_eq!(document.frame_count(), 3);
        assert_eq!(document.active_frame(), 2);
        assert_eq!(document.frame_pixels(1).unwrap()[0], 1);

        document.set_active_frame(0).unwrap();
        document.duplicate_frame(0).unwrap();
        assert_eq!(document.active_frame(), 0);
    }

    #[test]
    fn delete_frame_keeps_one_frame_and_shifts_active() {
        let mut document = doc();
        document.paint(0, 0, 1).unwrap();
        document.add_frame();
        document.add_frame();
        document.set_active_frame(2).unwrap();

        document.delete_frame(0).unwrap();
        assert_eq!(document.frame_count(), 2);
        assert_eq!(document.active_frame(), 1);

        document.delete_frame(1).unwrap();
        assert_eq!(document.frame_count(), 1);
        assert_eq!(document.active_frame(), 0);
        assert_eq!(document.pixels()[0], TRANSPARENT_INDEX);
        assert_eq!(
            document.delete_frame(0).unwrap_err(),
            KernelError::LastFrame
        );
        assert_eq!(document.frame_count(), 1);
    }

    #[test]
    fn rejects_unknown_frame_index() {
        let mut document = doc();
        document.add_frame();
        assert!(matches!(
            document.delete_frame(4),
            Err(KernelError::FrameIndex { index: 4, len: 2 })
        ));
        assert!(matches!(
            document.frame_pixels(3),
            Err(KernelError::FrameIndex { index: 3, len: 2 })
        ));
        assert!(matches!(
            document.set_active_frame(3),
            Err(KernelError::FrameIndex { index: 3, len: 2 })
        ));
        assert!(matches!(
            document.duplicate_frame(2),
            Err(KernelError::FrameIndex { index: 2, len: 2 })
        ));
    }

    #[test]
    fn frame_delay_undo_redo_and_duplicate() {
        let mut document = doc();
        assert_eq!(document.frame_delay(0).unwrap(), DEFAULT_FRAME_DELAY_MS);
        assert_eq!(document.duration_ms(), u64::from(DEFAULT_FRAME_DELAY_MS));

        document.set_frame_delay(0, 250).unwrap();
        document.set_frame_delay(0, 250).unwrap();
        assert_eq!(document.frame_delay(0).unwrap(), 250);

        let copy = document.duplicate_frame(0).unwrap();
        assert_eq!(copy, 1);
        assert_eq!(document.frame_delay(1).unwrap(), 250);
        assert_eq!(document.frame_delay(0).unwrap(), 250);

        document.undo().unwrap();
        assert_eq!(document.frame_count(), 1);
        document.undo().unwrap();
        assert_eq!(document.frame_delay(0).unwrap(), DEFAULT_FRAME_DELAY_MS);
        assert!(!document.can_undo());

        document.redo().unwrap();
        assert_eq!(document.frame_delay(0).unwrap(), 250);
        document.redo().unwrap();
        assert_eq!(document.frame_count(), 2);
        assert_eq!(document.frame_delay(1).unwrap(), 250);

        assert_eq!(
            document.set_frame_delay(0, 0).unwrap_err(),
            KernelError::FrameDelay { delay_ms: 0 }
        );
        assert_eq!(
            document
                .set_frame_delay(0, MAX_FRAME_DELAY_MS + 1)
                .unwrap_err(),
            KernelError::FrameDelay {
                delay_ms: MAX_FRAME_DELAY_MS + 1
            }
        );
        assert_eq!(document.frame_delay(0).unwrap(), 250);
        assert!(matches!(
            document.set_frame_delay(4, 20),
            Err(KernelError::FrameIndex { index: 4, len: 2 })
        ));
    }

    #[test]
    fn delay_gesture_is_one_step_and_a_return_records_nothing() {
        let mut document = doc();
        document.begin_gesture().unwrap();
        document.set_frame_delay(0, 120).unwrap();
        document.set_frame_delay(0, 180).unwrap();
        document.end_gesture();

        document.undo().unwrap();
        assert_eq!(document.frame_delay(0).unwrap(), DEFAULT_FRAME_DELAY_MS);
        document.redo().unwrap();
        assert_eq!(document.frame_delay(0).unwrap(), 180);

        document.undo().unwrap();
        assert!(document.can_redo());
        document.begin_gesture().unwrap();
        document.set_frame_delay(0, 150).unwrap();
        document.set_frame_delay(0, DEFAULT_FRAME_DELAY_MS).unwrap();
        document.end_gesture();
        assert!(!document.can_undo());
        assert!(document.can_redo());
        assert_eq!(document.frame_delay(0).unwrap(), DEFAULT_FRAME_DELAY_MS);
    }

    #[test]
    fn scrub_uses_delays_and_one_drag_is_one_undo() {
        let mut document = doc();
        document.add_frame();
        document.add_frame();
        document.set_frame_delay(1, 300).unwrap();
        document.clear_history();
        assert_eq!(document.frame_at_ratio(0), 0);
        assert_eq!(document.frame_at_ratio(198_000), 0);
        assert_eq!(document.frame_at_ratio(200_000), 1);
        assert_eq!(document.frame_at_ratio(800_000), 2);
        assert_eq!(document.frame_at_ratio(TIMELINE_RATIO_PPM), 2);

        document.play();
        assert!(document.playing());
        document.begin_gesture().unwrap();
        document.scrub(0).unwrap();
        document.scrub(250_000).unwrap();
        document.scrub(900_000).unwrap();
        document.end_gesture();

        assert!(!document.playing());
        assert_eq!(document.active_frame(), 2);
        document.undo().unwrap();
        assert_eq!(document.active_frame(), 0);
        assert!(!document.can_undo());
    }

    #[test]
    fn playback_loops_on_each_frame_delay_without_recording_undo() {
        let mut document = doc();
        document.add_frame();
        document.set_frame_delay(0, 100).unwrap();
        document.set_frame_delay(1, 40).unwrap();
        document.clear_history();

        assert!(!document.playing());
        document.play();
        assert!(document.playing());
        document.play();
        assert!(!document.tick(0));
        assert!(!document.tick(99));
        assert_eq!(document.active_frame(), 0);
        assert!(document.tick(1));
        assert_eq!(document.active_frame(), 1);
        assert!(document.tick(40));
        assert_eq!(document.active_frame(), 0);
        assert!(!document.can_undo());
        assert!(!document.can_redo());

        assert!(document.tick(120));
        assert_eq!(document.active_frame(), 1);
        assert!(document.tick(20));
        assert_eq!(document.active_frame(), 0);

        document.stop();
        assert!(!document.playing());
        assert!(!document.tick(500));
        assert_eq!(document.active_frame(), 0);

        document.set_active_frame(1).unwrap();
        document.play();
        assert!(!document.tick(10));
        assert_eq!(document.active_frame(), 1);
        document.undo().unwrap();
        assert!(!document.playing());
        assert_eq!(document.active_frame(), 0);
        assert!(!document.can_undo());
    }

    #[test]
    fn paint_stores_indices_and_rgba_views_follow_the_palette() {
        let mut document = doc();
        document.paint(1, 2, 4).unwrap();
        let offset = (2 * 16 + 1) * 4;
        assert_eq!(document.pixels()[2 * 16 + 1], 4);
        let color = document.palette().get(4).unwrap();
        let rgba = document.frame_rgba(0).unwrap();
        assert_eq!(
            &rgba[offset..offset + 4],
            &[color.r, color.g, color.b, color.a]
        );

        document.set_palette_color(4, 9, 8, 7).unwrap();
        assert_eq!(document.pixels()[2 * 16 + 1], 4);
        let rgba = document.frame_rgba(0).unwrap();
        assert_eq!(&rgba[offset..offset + 4], &[9, 8, 7, 255]);
        let png = document.export_frame_png(0).unwrap();
        assert_ne!(png, {
            let mut restored = document.clone();
            restored.undo().unwrap();
            restored.export_frame_png(0).unwrap()
        });
    }

    #[test]
    fn lock_blocks_edits_and_imports_but_not_paint_or_history() {
        let mut document = doc();
        let original = document.palette().get(1).unwrap();
        document.set_palette_locked(false);
        assert!(!document.can_undo());

        document.set_palette_color(1, 9, 8, 7).unwrap();
        document.set_palette_locked(true);
        document.set_palette_locked(true);
        assert!(document.palette_locked());

        assert_eq!(
            document.set_palette_color(1, 1, 2, 3).unwrap_err(),
            KernelError::PaletteLocked
        );
        assert_eq!(
            document.add_palette_color(4, 5, 6).unwrap_err(),
            KernelError::PaletteLocked
        );
        assert_eq!(
            document
                .import_palette("ff0000\n", PaletteImportMode::Replace)
                .unwrap_err(),
            KernelError::PaletteLocked
        );
        assert_eq!(document.palette().get(1), Some(Rgba::rgb(9, 8, 7)));
        assert_eq!(document.palette().len(), 9);

        document.paint(3, 4, 2).unwrap();
        assert_eq!(document.get(3, 4).unwrap(), 2);

        document.undo().unwrap();
        assert_eq!(document.get(3, 4).unwrap(), TRANSPARENT_INDEX);
        assert!(document.palette_locked());
        document.undo().unwrap();
        assert!(!document.palette_locked());
        assert_eq!(document.palette().get(1), Some(Rgba::rgb(9, 8, 7)));
        document.undo().unwrap();
        assert_eq!(document.palette().get(1), Some(original));
        assert!(!document.can_undo());

        document.redo().unwrap();
        assert_eq!(document.palette().get(1), Some(Rgba::rgb(9, 8, 7)));
        document.redo().unwrap();
        assert!(document.palette_locked());
        document.redo().unwrap();
        assert_eq!(document.get(3, 4).unwrap(), 2);
        assert_eq!(
            document.set_palette_color(1, 1, 2, 3).unwrap_err(),
            KernelError::PaletteLocked
        );
    }

    #[test]
    fn replace_import_remaps_orphans_and_undoes_in_one_step() {
        let mut document = doc();
        let before = document.palette().colors().to_vec();
        document.paint(0, 0, 1).unwrap();
        document.paint(1, 0, 8).unwrap();
        document.add_frame();
        document.set_active_frame(1).unwrap();
        document.paint(2, 2, 8).unwrap();

        document
            .import_palette("aa bb cc\n#ddee00 Name\n", PaletteImportMode::Replace)
            .unwrap_err();
        document
            .import_palette("#aabbcc\n#ddee00 Name\n", PaletteImportMode::Replace)
            .unwrap();
        assert_eq!(document.palette().len(), 3);
        assert_eq!(document.palette().get(0), Some(Rgba::TRANSPARENT));
        assert_eq!(document.palette().get(1), Some(Rgba::rgb(0xaa, 0xbb, 0xcc)));
        assert_eq!(document.palette().get(2), Some(Rgba::rgb(0xdd, 0xee, 0)));
        assert_eq!(document.frame_pixels(0).unwrap()[0], 1);
        assert_eq!(document.frame_pixels(0).unwrap()[1], TRANSPARENT_INDEX);
        assert_eq!(document.get(2, 2).unwrap(), TRANSPARENT_INDEX);

        document
            .import_palette("aabbcc\nddee00\n", PaletteImportMode::Replace)
            .unwrap();
        document.undo().unwrap();
        assert_eq!(document.palette().colors(), before.as_slice());
        assert_eq!(document.frame_pixels(0).unwrap()[0], 1);
        assert_eq!(document.frame_pixels(0).unwrap()[1], 8);
        assert_eq!(document.frame_pixels(1).unwrap()[2 + 2 * 16], 8);
        assert!(document.can_undo());

        document.redo().unwrap();
        assert_eq!(document.palette().len(), 3);
        assert_eq!(document.get(2, 2).unwrap(), TRANSPARENT_INDEX);
    }

    #[test]
    fn merge_import_skips_duplicates_and_groups_appends() {
        let mut document = doc();
        let ink = document.palette().get(1).unwrap();
        let hex = format!("{:02x}{:02x}{:02x}", ink.r, ink.g, ink.b);
        document
            .import_palette(
                &format!("{hex}\nff0000\nff0000\n"),
                PaletteImportMode::Merge,
            )
            .unwrap();
        assert_eq!(document.palette().len(), 10);
        assert_eq!(document.palette().get(9), Some(Rgba::rgb(255, 0, 0)));

        document.undo().unwrap();
        assert_eq!(document.palette().len(), 9);
        assert!(!document.can_undo());

        document
            .import_palette(&hex, PaletteImportMode::Merge)
            .unwrap();
        assert!(!document.can_undo());
        assert_eq!(document.palette().len(), 9);

        let mut overflow = String::new();
        for index in 0..248 {
            overflow.push_str(&format!("{:02x}{:02x}00\n", index, 255 - index));
        }
        assert_eq!(
            document
                .import_palette(&overflow, PaletteImportMode::Merge)
                .unwrap_err(),
            KernelError::PaletteTooLarge { len: 257 }
        );
        assert_eq!(document.palette().len(), 9);
        assert!(!document.can_undo());
    }

    #[test]
    fn import_rejects_a_bad_file_without_changing_pixels() {
        let mut document = doc();
        document.paint(0, 0, 3).unwrap();
        document.clear_history();
        assert_eq!(
            document
                .import_palette("GIMP Palette\n256 0 0\n", PaletteImportMode::Merge)
                .unwrap_err(),
            KernelError::PaletteFile
        );
        assert_eq!(
            document
                .import_palette("GIMP Palette\n#\n", PaletteImportMode::Replace)
                .unwrap_err(),
            KernelError::EmptyPalette
        );
        assert_eq!(document.get(0, 0).unwrap(), 3);
        assert_eq!(document.palette().len(), 9);
        assert!(!document.can_undo());
    }

    #[test]
    fn gpl_replace_and_a_gesture_share_one_undo_step() {
        let mut document = doc();
        let gpl = "GIMP Palette\nName: Demo\nColumns: 1\n#\n255 0 0 Red\n0 128 255 Blue\n";
        document.begin_gesture().unwrap();
        document
            .import_palette(gpl, PaletteImportMode::Replace)
            .unwrap();
        document.paint(4, 4, 2).unwrap();
        document.end_gesture();

        assert_eq!(document.palette().len(), 3);
        assert_eq!(document.get(4, 4).unwrap(), 2);
        document.undo().unwrap();
        assert_eq!(document.palette().len(), 9);
        assert_eq!(document.get(4, 4).unwrap(), TRANSPARENT_INDEX);
        assert!(!document.can_undo());
    }
}
