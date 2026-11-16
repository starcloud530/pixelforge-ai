//! Read-only eight-direction contact sheet.
//!
//! Picks one frame of one action from each facing, draws those frames in
//! screen order, and reports baselines the caller actually named. It does not
//! write pixels, tags, parts, anchors, or history. An empty slot stays empty.
//! Another facing is never copied in to fill it.
//!
//! This is not a judgment of the drawing. A narrower side view is a hint.
//! The bounding box is not an origin, and the lowest opaque cell is not a foot.
//! `parts` without a curve or a driving constraint is the rest pose, not the
//! anchor of the frame on screen.

use std::collections::HashMap;

use serde::Serialize;

use crate::direction::{facings_for, parse_direction_tag, Facing};
use crate::document::Document;
use crate::export::encode_rgba_png;
use crate::part::Part;
use crate::tag::Tag;
use crate::SolveReport;

/// `format` field of a direction-review report.
pub const DIRECTION_REVIEW_FORMAT: &str = "pixelforge-direction-review";

/// Version of [`DirectionReviewReport`].
pub const DIRECTION_REVIEW_VERSION: u32 = 1;

const NOTE: &str = "不判定画得好不好。空槽是缺方向或缺帧，不会用别的方向或别的动作补齐。没有给出的基准写「未提供」：不用包围盒中心当原点，不用最低不透明像素当脚底。没有曲线或约束可求值时，parts 是静止姿势，不是该帧的实际锚点。侧面变窄、包围盒变化只作观察，不判错。 This review does not grade the drawing.";

const BOUNDS_NOTE: &str = "Subject bounds are the non-zero cells of the index composite. They are not an origin and not a foot.";

const SELECTION_RULE: &str = "Each facing uses the frame at this offset inside its own {action}_{facing} tag, or the layout column when that column lies inside the tag. A missing tag, a short span, or a duplicate name leaves an empty slot. Frames are not borrowed from another facing or another action.";

const REST_TEXT: &str = "工程字段 parts。静止姿势，不是该帧的实际锚点。";
const USER_TEXT: &str = "用户参数。";
const CURVE_TEXT: &str = "曲线求值。该帧 solve_constraints（apply 为 false）的采样，不是包围盒。";
const CONSTRAINT_TEXT: &str = "约束求值。该帧 solve_constraints（apply 为 false），不是包围盒。";
const BOTH_TEXT: &str =
    "曲线采样后再解约束。该帧 solve_constraints（apply 为 false），不是包围盒。";
const NOT_PROVIDED: &str = "未提供";

const LABEL_H: u32 = 7;
const GUTTER: u32 = 1;
const LABEL_BG: [u8; 4] = [32, 28, 40, 255];
const LABEL_INK: [u8; 4] = [232, 220, 180, 255];
const EMPTY_MARK: [u8; 4] = [96, 64, 48, 255];
const CENTER_BG: [u8; 4] = [24, 36, 48, 255];
const GUTTER_RGBA: [u8; 4] = [16, 14, 20, 255];

/// Why [`Document::review_directions`] did not produce a report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectionReviewError {
    /// The action is not one `{action}_{facing}` token.
    Action,
    /// Scale is 0, or the review image does not fit in a PNG.
    Scale { scale: u32 },
}

impl std::fmt::Display for DirectionReviewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Action => write!(
                f,
                "action must be one token of letters and digits, such as idle, walk, or attack"
            ),
            Self::Scale { scale } => write!(
                f,
                "scale {scale} must be an integer >= 1, and the review image must fit in a PNG"
            ),
        }
    }
}

/// Where the registration origin comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectionReviewOrigin {
    /// No origin was requested.
    Unset,
    /// One canvas cell, the same on every facing. May lie off the canvas.
    User { x: i32, y: i32 },
    /// A part origin. Rest pose unless that part has a curve or a constraint.
    Part(String),
}

/// Where the foot baseline comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectionReviewFoot {
    /// No foot baseline was requested.
    Unset,
    /// One horizontal row, the same on every facing. Not a pixel search.
    UserY(i32),
    /// A named anchor. Rest pose unless that part has a curve or a constraint.
    Anchor { part: String, name: String },
}

/// One anchor the report must list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectionReviewAnchorName {
    pub part: String,
    pub name: String,
}

/// Which provided baseline the aligned sheet and the overlays use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectionReviewAlign {
    /// Foot if requested, otherwise origin, otherwise the first anchor.
    /// A missing higher-priority baseline does not fall through.
    Auto,
    Origin,
    Foot,
    Anchor {
        part: String,
        name: String,
    },
}

/// Arguments for [`Document::review_directions`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectionReviewOptions {
    pub action: String,
    /// 0-based index inside each `{action}_{facing}` tag. Ignored when
    /// `column` is set.
    pub at: u32,
    /// Column inside the facing row. The frame is used only when that column
    /// lies inside the action tag. Requires `directionLayout`.
    pub column: Option<u32>,
    pub scale: u32,
    /// Chebyshev distance, in source pixels, before an alignment offset is a finding.
    pub tolerance: u32,
    pub origin: DirectionReviewOrigin,
    pub foot: DirectionReviewFoot,
    pub anchors: Vec<DirectionReviewAnchorName>,
    pub align: DirectionReviewAlign,
}

/// PNG pack plus the JSON report. Building it does not change the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectionReview {
    pub report: DirectionReviewReport,
    pub positions_png: Vec<u8>,
    pub aligned_png: Option<Vec<u8>>,
    /// Parallel to `report.overlays`.
    pub overlay_pngs: Vec<Vec<u8>>,
}

/// `pixelforge-direction-review` version 1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectionReviewReport {
    pub format: &'static str,
    pub version: u32,
    /// Always false. Exit 0 is not an aesthetic pass.
    pub judges_art: bool,
    pub note: &'static str,
    pub bounds_note: &'static str,
    pub action: String,
    pub selection: DirectionReviewSelection,
    pub scale: u32,
    pub tolerance: u32,
    pub canvas: DirectionReviewSize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction_layout: Option<DirectionReviewLayout>,
    pub baselines: DirectionReviewBaselines,
    pub alignment: DirectionReviewAlignment,
    pub image: DirectionReviewImage,
    pub directions: Vec<DirectionReviewFacing>,
    pub overlays: Vec<DirectionReviewOverlay>,
    pub findings: Vec<DirectionReviewFinding>,
    pub structural_count: usize,
    pub alignment_count: usize,
    pub observation_count: usize,
    /// Structural plus alignment. Observations are not included.
    pub error_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectionReviewSelection {
    pub action: String,
    /// `at` or `column`.
    pub mode: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    pub rule: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectionReviewSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectionReviewLayout {
    pub count: u8,
    pub columns: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectionReviewBaselines {
    pub origin: DirectionReviewPoint,
    pub foot: DirectionReviewPoint,
    pub anchors: Vec<DirectionReviewNamedPoint>,
}

/// One baseline. `text` is `未提供` when `status` is `notProvided`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectionReviewPoint {
    pub status: &'static str,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectionReviewNamedPoint {
    pub part: String,
    pub name: String,
    pub status: &'static str,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectionReviewAlignment {
    pub applied: bool,
    /// `未提供` when alignment did not run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// `origin`, `foot`, or `anchor`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
    /// `xy` or `y`. A user foot row compares `y` only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub axes: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_compass: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_x: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_y: Option<i32>,
    pub tolerance: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectionReviewImage {
    pub scale: u32,
    pub cell_width: u32,
    pub cell_height: u32,
    pub label_height: u32,
    pub gutter: u32,
    pub width: u32,
    pub height: u32,
    /// Screen order of the 3×3 sheet. Row 0 is north.
    pub slots: Vec<DirectionReviewSlot>,
    pub center: DirectionReviewSlot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectionReviewSlot {
    pub screen: &'static str,
    pub compass: &'static str,
    pub column: u32,
    pub row: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectionReviewFacing {
    pub screen: &'static str,
    pub compass: &'static str,
    pub row: u32,
    pub column: u32,
    /// `present`, `missingDirection`, `missingFrame`, or `duplicate`.
    pub slot: &'static str,
    /// Always false. An empty slot is not filled from another facing.
    pub borrowed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_from: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_to: Option<u32>,
    pub canvas: DirectionReviewSize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_bounds: Option<DirectionReviewBounds>,
    pub origin: DirectionReviewPoint,
    pub foot: DirectionReviewPoint,
    pub anchors: Vec<DirectionReviewNamedPoint>,
    /// Source pixels added to this frame so its baseline meets the reference.
    /// Absent when this slot was not shifted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<DirectionReviewOffset>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectionReviewBounds {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectionReviewOffset {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectionReviewOverlay {
    pub file: String,
    pub a: &'static str,
    pub b: &'static str,
    pub a_compass: &'static str,
    pub b_compass: &'static str,
    pub a_slot: &'static str,
    pub b_slot: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub a_offset: Option<DirectionReviewOffset>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub b_offset: Option<DirectionReviewOffset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectionReviewFinding {
    /// `structural`, `alignment`, or `observation`.
    pub severity: &'static str,
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facing: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compass: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dx: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dy: Option<i32>,
}

#[derive(Clone, Copy)]
struct Influence {
    curve: bool,
    constraint: bool,
}

impl Influence {
    fn evaluable(self) -> bool {
        self.curve || self.constraint
    }

    fn source(self) -> &'static str {
        match (self.curve, self.constraint) {
            (true, true) => "curveAndConstraint",
            (true, false) => "curve",
            (false, true) => "constraint",
            (false, false) => "restPose",
        }
    }

    fn text(self) -> &'static str {
        match (self.curve, self.constraint) {
            (true, true) => BOTH_TEXT,
            (true, false) => CURVE_TEXT,
            (false, true) => CONSTRAINT_TEXT,
            (false, false) => REST_TEXT,
        }
    }
}

struct Chosen {
    frame: Option<usize>,
    kind: &'static str,
    tag: Option<String>,
    tag_from: Option<u32>,
    tag_to: Option<u32>,
}

struct BuiltPoint {
    point: DirectionReviewPoint,
    /// Solve failed on a frame that exists. A missing name is structural elsewhere.
    eval_failed: bool,
}

struct DirBuilt {
    chosen: Chosen,
    bounds: Option<DirectionReviewBounds>,
    origin: BuiltPoint,
    foot: BuiltPoint,
    anchors: Vec<BuiltPoint>,
    offset: Option<DirectionReviewOffset>,
    rgba: Option<Vec<u8>>,
}

enum AlignPick {
    None,
    Origin,
    Foot,
    Anchor(usize),
}

impl Document {
    /// Eight-facing review of one action. Does not change this document.
    ///
    /// The aligned PNG shifts frames so an explicit baseline meets the
    /// reference facing (`down` when that slot has the point). Parts are the
    /// rest pose unless a curve or a driving constraint can be solved for
    /// that frame. A failed solve is `未提供`. The bounding box is not used
    /// as a stand-in.
    pub fn review_directions(
        &self,
        options: &DirectionReviewOptions,
    ) -> Result<DirectionReview, DirectionReviewError> {
        review(self, options)
    }
}

fn review(
    doc: &Document,
    options: &DirectionReviewOptions,
) -> Result<DirectionReview, DirectionReviewError> {
    if !is_action(&options.action) {
        return Err(DirectionReviewError::Action);
    }
    if options.scale == 0 {
        return Err(DirectionReviewError::Scale { scale: 0 });
    }

    let facings = facings_for(8);
    let layout_missing = options.column.is_some() && doc.direction_layout().is_none();
    let chosen: Vec<Chosen> = facings
        .iter()
        .map(|facing| select_facing(doc, options, facing, layout_missing))
        .collect();

    let mut findings = Vec::new();
    if layout_missing {
        findings.push(finding(
            "structural",
            "layout",
            "column needs directionLayout; no facing frame was selected, and none was copied"
                .to_string(),
            None,
        ));
    }
    for (facing, slot) in facings.iter().zip(chosen.iter()) {
        if let Some(message) = slot_message(facing, slot, &options.action) {
            let code = match slot.kind {
                "missingDirection" => "missingDirection",
                "duplicate" => "duplicateSegment",
                _ => "missingFrame",
            };
            findings.push(finding("structural", code, message, Some(facing)));
        }
    }
    push_name_findings(doc, options, &mut findings);

    let mut cache: HashMap<usize, Result<SolveReport, String>> = HashMap::new();
    let mut built = Vec::with_capacity(facings.len());
    for (facing, slot) in facings.iter().zip(chosen.into_iter()) {
        let _ = facing;
        let origin = build_origin(doc, options, &slot, &mut cache);
        let foot = build_foot(doc, options, &slot, &mut cache);
        let anchors = options
            .anchors
            .iter()
            .map(|anchor| build_named(doc, &slot, &anchor.part, &anchor.name, &mut cache))
            .collect::<Vec<_>>();
        if origin.eval_failed {
            findings.push(finding(
                "structural",
                "baselineUnavailable",
                format!(
                    "facing {} ({}) origin is 未提供: {}",
                    facing.screen,
                    facing.compass,
                    origin.point.reason.as_deref().unwrap_or("solve failed")
                ),
                Some(facing),
            ));
        }
        if foot.eval_failed {
            findings.push(finding(
                "structural",
                "baselineUnavailable",
                format!(
                    "facing {} ({}) foot is 未提供: {}",
                    facing.screen,
                    facing.compass,
                    foot.point.reason.as_deref().unwrap_or("solve failed")
                ),
                Some(facing),
            ));
        }
        for (spec, anchor) in options.anchors.iter().zip(anchors.iter()) {
            if anchor.eval_failed {
                findings.push(finding(
                    "structural",
                    "baselineUnavailable",
                    format!(
                        "facing {} ({}) anchor {}:{} is 未提供: {}",
                        facing.screen,
                        facing.compass,
                        spec.part,
                        spec.name,
                        anchor.point.reason.as_deref().unwrap_or("solve failed")
                    ),
                    Some(facing),
                ));
            }
        }
        let rgba = match slot.frame {
            Some(frame) => {
                Some(
                    doc.frame_rgba(frame)
                        .map_err(|_| DirectionReviewError::Scale {
                            scale: options.scale,
                        })?,
                )
            }
            None => None,
        };
        let bounds = slot.frame.and_then(|frame| subject_bounds(doc, frame));
        built.push(DirBuilt {
            chosen: slot,
            bounds,
            origin,
            foot,
            anchors,
            offset: None,
            rgba,
        });
    }

    let pick = align_pick(options);
    let y_only =
        matches!(options.foot, DirectionReviewFoot::UserY(_)) && matches!(pick, AlignPick::Foot);
    let (alignment, align_source) =
        assign_offsets(options, &pick, y_only, &mut built, &mut findings);
    if alignment.applied {
        if align_source == Some("restPose") {
            findings.push(finding(
                "observation",
                "restPoseBaseline",
                "alignment used the parts rest pose. That point is stored once for the document, not per frame. Offset 0 does not mean the drawn feet match, and the lowest opaque pixel was not used.".to_string(),
                None,
            ));
        } else if align_source == Some("user") {
            findings.push(finding(
                "observation",
                "userBaseline",
                "alignment used one user point for every facing. Offset 0 does not mean the drawn feet match.".to_string(),
                None,
            ));
        }
    }
    observe_bounds(&built, &mut findings);

    let image = compose(doc, options, &built, &alignment)?;
    let directions: Vec<DirectionReviewFacing> = built
        .iter()
        .zip(facings.iter())
        .map(|(item, facing)| DirectionReviewFacing {
            screen: facing.screen,
            compass: facing.compass,
            row: grid_cell(facing.index).1,
            column: grid_cell(facing.index).0,
            slot: item.chosen.kind,
            borrowed: false,
            frame: item.chosen.frame.map(|frame| frame as u32),
            tag: item.chosen.tag.clone(),
            tag_from: item.chosen.tag_from,
            tag_to: item.chosen.tag_to,
            canvas: DirectionReviewSize {
                width: u32::from(doc.width()),
                height: u32::from(doc.height()),
            },
            subject_bounds: item.bounds,
            origin: item.origin.point.clone(),
            foot: item.foot.point.clone(),
            anchors: item
                .anchors
                .iter()
                .zip(options.anchors.iter())
                .map(|(anchor, spec)| named_from(spec, &anchor.point))
                .collect(),
            offset: item.offset,
        })
        .collect();
    let overlays = overlay_meta(&directions);
    let mut structural_count = 0;
    let mut alignment_count = 0;
    let mut observation_count = 0;
    for item in &findings {
        match item.severity {
            "alignment" => alignment_count += 1,
            "observation" => observation_count += 1,
            _ => structural_count += 1,
        }
    }
    let report = DirectionReviewReport {
        format: DIRECTION_REVIEW_FORMAT,
        version: DIRECTION_REVIEW_VERSION,
        judges_art: false,
        note: NOTE,
        bounds_note: BOUNDS_NOTE,
        action: options.action.clone(),
        selection: DirectionReviewSelection {
            action: options.action.clone(),
            mode: if options.column.is_some() {
                "column"
            } else {
                "at"
            },
            at: if options.column.is_some() {
                None
            } else {
                Some(options.at)
            },
            column: options.column,
            rule: SELECTION_RULE,
        },
        scale: options.scale,
        tolerance: options.tolerance,
        canvas: DirectionReviewSize {
            width: u32::from(doc.width()),
            height: u32::from(doc.height()),
        },
        direction_layout: doc.direction_layout().map(|layout| DirectionReviewLayout {
            count: layout.count,
            columns: layout.columns,
        }),
        baselines: summary_baselines(options, &built),
        alignment,
        image: image.meta,
        directions,
        overlays,
        findings,
        structural_count,
        alignment_count,
        observation_count,
        error_count: structural_count + alignment_count,
    };
    Ok(DirectionReview {
        report,
        positions_png: image.positions,
        aligned_png: image.aligned,
        overlay_pngs: image.overlays,
    })
}

fn is_action(action: &str) -> bool {
    let mut chars = action.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() => chars.all(|ch| ch.is_ascii_alphanumeric()),
        _ => false,
    }
}

fn select_facing(
    doc: &Document,
    options: &DirectionReviewOptions,
    facing: &Facing,
    layout_missing: bool,
) -> Chosen {
    if layout_missing {
        return Chosen {
            frame: None,
            kind: "missingFrame",
            tag: None,
            tag_from: None,
            tag_to: None,
        };
    }
    let hits: Vec<&Tag> = doc
        .tags()
        .iter()
        .filter(|tag| {
            matches!(
                parse_direction_tag(&tag.name),
                Some((action, found))
                    if action.as_str() == options.action && found.index == facing.index
            )
        })
        .collect();
    match hits.as_slice() {
        [] => Chosen {
            frame: None,
            kind: "missingDirection",
            tag: None,
            tag_from: None,
            tag_to: None,
        },
        [tag] => place_tag(doc, options, facing, tag),
        _ => Chosen {
            frame: None,
            kind: "duplicate",
            tag: None,
            tag_from: None,
            tag_to: None,
        },
    }
}

fn place_tag(
    doc: &Document,
    options: &DirectionReviewOptions,
    facing: &Facing,
    tag: &Tag,
) -> Chosen {
    let frame = if let Some(column) = options.column {
        let Some(layout) = doc.direction_layout() else {
            return empty_tag(tag, "missingFrame");
        };
        if column >= layout.columns {
            return empty_tag(tag, "missingFrame");
        }
        let row_start = u32::from(facing.index) * layout.columns;
        let frame = row_start.saturating_add(column);
        let frame = usize::try_from(frame).unwrap_or(usize::MAX);
        if frame < tag.from || frame > tag.to {
            return empty_tag(tag, "missingFrame");
        }
        frame
    } else {
        let Ok(at) = usize::try_from(options.at) else {
            return empty_tag(tag, "missingFrame");
        };
        let Some(frame) = tag.from.checked_add(at) else {
            return empty_tag(tag, "missingFrame");
        };
        if frame > tag.to {
            return empty_tag(tag, "missingFrame");
        }
        frame
    };
    if frame >= doc.frame_count() {
        return empty_tag(tag, "missingFrame");
    }
    Chosen {
        frame: Some(frame),
        kind: "present",
        tag: Some(tag.name.clone()),
        tag_from: u32::try_from(tag.from).ok(),
        tag_to: u32::try_from(tag.to).ok(),
    }
}

fn empty_tag(tag: &Tag, kind: &'static str) -> Chosen {
    Chosen {
        frame: None,
        kind,
        tag: Some(tag.name.clone()),
        tag_from: u32::try_from(tag.from).ok(),
        tag_to: u32::try_from(tag.to).ok(),
    }
}

fn slot_message(facing: &Facing, slot: &Chosen, action: &str) -> Option<String> {
    match slot.kind {
        "missingDirection" => Some(format!(
            "facing {} ({}) has no {action}_{} tag; the slot stays empty and is not copied",
            facing.screen, facing.compass, facing.screen
        )),
        "duplicate" => Some(format!(
            "facing {} ({}) {action} is named more than once; the slot stays empty",
            facing.screen, facing.compass
        )),
        "missingFrame" => Some(format!(
            "facing {} ({}) has no {action} frame at the requested position; the slot stays empty and is not copied",
            facing.screen, facing.compass
        )),
        _ => None,
    }
}

fn push_name_findings(
    doc: &Document,
    options: &DirectionReviewOptions,
    findings: &mut Vec<DirectionReviewFinding>,
) {
    let mut missing_parts = Vec::new();
    let mut note_part = |name: &str| {
        let trimmed = name.trim();
        if find_part(doc, trimmed).is_none()
            && !missing_parts.iter().any(|have: &String| have == trimmed)
        {
            missing_parts.push(trimmed.to_string());
        }
    };
    if let DirectionReviewOrigin::Part(name) = &options.origin {
        note_part(name);
    }
    if let DirectionReviewFoot::Anchor { part, .. } = &options.foot {
        note_part(part);
    }
    for anchor in &options.anchors {
        note_part(&anchor.part);
    }
    if let DirectionReviewAlign::Anchor { part, .. } = &options.align {
        note_part(part);
    }
    for name in missing_parts {
        findings.push(finding(
            "structural",
            "missingPart",
            format!("part {name} is not in the project; its baseline is 未提供 and the bounding box is not used"),
            None,
        ));
    }
    let mut missing_anchors = Vec::new();
    let mut note_anchor = |part: &str, name: &str| {
        let part_name = part.trim();
        let anchor_name = name.trim();
        let Some(part) = find_part(doc, part_name) else {
            return;
        };
        if part.anchor_canvas(anchor_name).is_none() {
            let key = (part.name.clone(), anchor_name.to_string());
            if !missing_anchors.contains(&key) {
                missing_anchors.push(key);
            }
        }
    };
    if let DirectionReviewFoot::Anchor { part, name } = &options.foot {
        note_anchor(part, name);
    }
    for anchor in &options.anchors {
        note_anchor(&anchor.part, &anchor.name);
    }
    if let DirectionReviewAlign::Anchor { part, name } = &options.align {
        note_anchor(part, name);
    }
    for (part, name) in missing_anchors {
        findings.push(finding(
            "structural",
            "missingAnchor",
            format!(
                "part {part} has no anchor {name}; the point is 未提供. The bounding-box center and the lowest opaque pixel are not substitutes"
            ),
            None,
        ));
    }
    match &options.align {
        DirectionReviewAlign::Origin if matches!(options.origin, DirectionReviewOrigin::Unset) => {
            findings.push(finding(
                "structural",
                "baselineMissing",
                "alignment asked for an origin, and none was provided (未提供)".to_string(),
                None,
            ));
        }
        DirectionReviewAlign::Foot if matches!(options.foot, DirectionReviewFoot::Unset) => {
            findings.push(finding(
                "structural",
                "baselineMissing",
                "alignment asked for a foot baseline, and none was provided (未提供)".to_string(),
                None,
            ));
        }
        DirectionReviewAlign::Anchor { .. } if align_anchor_spec(options).is_none() => {
            findings.push(finding(
                "structural",
                "baselineMissing",
                "alignment asked for an anchor, and none was provided (未提供)".to_string(),
                None,
            ));
        }
        _ => {}
    }
}

fn align_anchor_spec(options: &DirectionReviewOptions) -> Option<(String, String)> {
    match &options.align {
        DirectionReviewAlign::Anchor { part, name } => {
            if part.trim().is_empty() || name.trim().is_empty() {
                None
            } else {
                Some((part.trim().to_string(), name.trim().to_string()))
            }
        }
        _ => None,
    }
}

fn find_part<'a>(doc: &'a Document, name: &str) -> Option<&'a Part> {
    let name = name.trim();
    doc.parts().iter().find(|part| part.name == name)
}

fn influences(doc: &Document, part: &str) -> Influence {
    Influence {
        curve: doc.curves().iter().any(|curve| curve.part == part),
        constraint: doc.constraints().iter().any(|constraint| {
            constraint.part == part || constraint.chain.iter().any(|name| name == part)
        }),
    }
}

fn pose<'a>(
    doc: &Document,
    cache: &'a mut HashMap<usize, Result<SolveReport, String>>,
    frame: usize,
) -> &'a Result<SolveReport, String> {
    cache.entry(frame).or_insert_with(|| {
        let mut clone = doc.clone();
        clone
            .solve_constraints(Some(frame as f64), false, Some(frame))
            .map_err(|err| format!("{}: {err}", err.code()))
    })
}

fn not_provided(reason: Option<String>, eval_failed: bool) -> BuiltPoint {
    BuiltPoint {
        point: DirectionReviewPoint {
            status: "notProvided",
            text: NOT_PROVIDED.to_string(),
            source: None,
            x: None,
            y: None,
            reason,
        },
        eval_failed,
    }
}

fn provided(source: &'static str, text: &str, x: Option<i32>, y: Option<i32>) -> BuiltPoint {
    BuiltPoint {
        point: DirectionReviewPoint {
            status: "provided",
            text: text.to_string(),
            source: Some(source),
            x,
            y,
            reason: None,
        },
        eval_failed: false,
    }
}

fn build_origin(
    doc: &Document,
    options: &DirectionReviewOptions,
    slot: &Chosen,
    cache: &mut HashMap<usize, Result<SolveReport, String>>,
) -> BuiltPoint {
    match &options.origin {
        DirectionReviewOrigin::Unset => not_provided(None, false),
        DirectionReviewOrigin::User { x, y } => provided("user", USER_TEXT, Some(*x), Some(*y)),
        DirectionReviewOrigin::Part(name) => part_point(doc, slot, name, None, cache),
    }
}

fn build_foot(
    doc: &Document,
    options: &DirectionReviewOptions,
    slot: &Chosen,
    cache: &mut HashMap<usize, Result<SolveReport, String>>,
) -> BuiltPoint {
    match &options.foot {
        DirectionReviewFoot::Unset => not_provided(None, false),
        DirectionReviewFoot::UserY(y) => provided("user", USER_TEXT, None, Some(*y)),
        DirectionReviewFoot::Anchor { part, name } => {
            part_point(doc, slot, part, Some(name), cache)
        }
    }
}

fn build_named(
    doc: &Document,
    slot: &Chosen,
    part: &str,
    name: &str,
    cache: &mut HashMap<usize, Result<SolveReport, String>>,
) -> BuiltPoint {
    part_point(doc, slot, part, Some(name), cache)
}

fn part_point(
    doc: &Document,
    slot: &Chosen,
    part_name: &str,
    anchor: Option<&str>,
    cache: &mut HashMap<usize, Result<SolveReport, String>>,
) -> BuiltPoint {
    let Some(part) = find_part(doc, part_name) else {
        return not_provided(
            Some(format!("part {} is not in the project", part_name.trim())),
            false,
        );
    };
    if let Some(anchor) = anchor {
        if part.anchor_canvas(anchor.trim()).is_none() {
            return not_provided(
                Some(format!(
                    "part {} has no anchor {}",
                    part.name,
                    anchor.trim()
                )),
                false,
            );
        }
    }
    let influence = influences(doc, &part.name);
    if !influence.evaluable() {
        return rest_point(part, anchor, influence);
    }
    let Some(frame) = slot.frame else {
        return not_provided(
            Some(
                "no frame on this facing; another facing was not sampled, and the rest pose is not this frame's anchor"
                    .to_string(),
            ),
            false,
        );
    };
    match pose(doc, cache, frame) {
        Ok(report) => {
            let Some(solved) = report.parts.iter().find(|item| item.name == part.name) else {
                return not_provided(Some("solved pose has no such part".to_string()), true);
            };
            if let Some(anchor) = anchor {
                let Some(found) = solved
                    .anchors
                    .iter()
                    .find(|item| item.name == anchor.trim())
                else {
                    return not_provided(
                        Some(format!("solved pose has no anchor {}", anchor.trim())),
                        true,
                    );
                };
                provided(
                    influence.source(),
                    influence.text(),
                    Some(found.canvas_x),
                    Some(found.canvas_y),
                )
            } else {
                provided(
                    influence.source(),
                    influence.text(),
                    Some(i32::from(solved.x)),
                    Some(i32::from(solved.y)),
                )
            }
        }
        Err(reason) => not_provided(Some(reason.clone()), true),
    }
}

fn rest_point(part: &Part, anchor: Option<&str>, influence: Influence) -> BuiltPoint {
    if let Some(anchor) = anchor {
        match part.anchor_canvas(anchor.trim()) {
            Some((x, y)) => provided(influence.source(), influence.text(), Some(x), Some(y)),
            None => not_provided(
                Some(format!(
                    "part {} has no anchor {}",
                    part.name,
                    anchor.trim()
                )),
                false,
            ),
        }
    } else {
        provided(
            influence.source(),
            influence.text(),
            Some(i32::from(part.x)),
            Some(i32::from(part.y)),
        )
    }
}

fn align_pick(options: &DirectionReviewOptions) -> AlignPick {
    match &options.align {
        DirectionReviewAlign::Origin => AlignPick::Origin,
        DirectionReviewAlign::Foot => AlignPick::Foot,
        DirectionReviewAlign::Anchor { part, name } => options
            .anchors
            .iter()
            .position(|anchor| {
                anchor.part.trim() == part.trim() && anchor.name.trim() == name.trim()
            })
            .map(AlignPick::Anchor)
            .unwrap_or(AlignPick::None),
        DirectionReviewAlign::Auto => {
            if !matches!(options.foot, DirectionReviewFoot::Unset) {
                AlignPick::Foot
            } else if !matches!(options.origin, DirectionReviewOrigin::Unset) {
                AlignPick::Origin
            } else if !options.anchors.is_empty() {
                AlignPick::Anchor(0)
            } else {
                AlignPick::None
            }
        }
    }
}

fn align_point<'a>(item: &'a DirBuilt, pick: &AlignPick) -> Option<&'a DirectionReviewPoint> {
    let point = match pick {
        AlignPick::None => return None,
        AlignPick::Origin => &item.origin.point,
        AlignPick::Foot => &item.foot.point,
        AlignPick::Anchor(index) => &item.anchors.get(*index)?.point,
    };
    Some(point)
}

fn assign_offsets(
    options: &DirectionReviewOptions,
    pick: &AlignPick,
    y_only: bool,
    built: &mut [DirBuilt],
    findings: &mut Vec<DirectionReviewFinding>,
) -> (DirectionReviewAlignment, Option<&'static str>) {
    let mut alignment = DirectionReviewAlignment {
        applied: false,
        reason: Some(NOT_PROVIDED.to_string()),
        baseline: None,
        source: None,
        source_text: None,
        part: None,
        anchor: None,
        axes: None,
        reference: None,
        reference_compass: None,
        reference_x: None,
        reference_y: None,
        tolerance: options.tolerance,
    };
    let Some(baseline_name) = baseline_name(options, pick) else {
        return (alignment, None);
    };
    alignment.baseline = Some(baseline_name.0);
    alignment.part = baseline_name.1;
    alignment.anchor = baseline_name.2;
    alignment.axes = Some(if y_only { "y" } else { "xy" });

    let facings = facings_for(8);
    let mut candidates = Vec::new();
    for (index, item) in built.iter().enumerate() {
        if item.chosen.kind != "present" {
            continue;
        }
        let Some(point) = align_point(item, pick) else {
            continue;
        };
        if point.status == "provided" {
            candidates.push(index);
        }
    }
    let Some(reference) = candidates
        .iter()
        .copied()
        .find(|index| facings[*index].screen == "down")
        .or_else(|| candidates.first().copied())
    else {
        return (alignment, None);
    };
    let reference_point = align_point(&built[reference], pick)
        .expect("candidate has a point")
        .clone();
    let source = reference_point.source;
    alignment.applied = true;
    alignment.reason = None;
    alignment.source = source;
    alignment.source_text = Some(reference_point.text.clone());
    alignment.reference = Some(facings[reference].screen);
    alignment.reference_compass = Some(facings[reference].compass);
    alignment.reference_x = reference_point.x;
    alignment.reference_y = reference_point.y;

    for index in candidates {
        let point = align_point(&built[index], pick)
            .expect("candidate has a point")
            .clone();
        let Some((dx, dy)) = delta(&reference_point, &point, y_only) else {
            continue;
        };
        built[index].offset = Some(DirectionReviewOffset { x: dx, y: dy });
        if index == reference {
            continue;
        }
        let distance = dx.unsigned_abs().max(dy.unsigned_abs());
        if distance > options.tolerance {
            findings.push(DirectionReviewFinding {
                severity: "alignment",
                code: "offset",
                message: format!(
                    "facing {} ({}) baseline differs from {} ({}) by dx {dx} dy {dy}, past tolerance {}. Source is {}.",
                    facings[index].screen,
                    facings[index].compass,
                    facings[reference].screen,
                    facings[reference].compass,
                    options.tolerance,
                    source.unwrap_or("unknown")
                ),
                facing: Some(facings[index].screen),
                compass: Some(facings[index].compass),
                dx: Some(dx),
                dy: Some(dy),
            });
        }
    }
    (alignment, source)
}

fn baseline_name(
    options: &DirectionReviewOptions,
    pick: &AlignPick,
) -> Option<(&'static str, Option<String>, Option<String>)> {
    match pick {
        AlignPick::None => None,
        AlignPick::Origin => match &options.origin {
            DirectionReviewOrigin::Unset => None,
            DirectionReviewOrigin::User { .. } => Some(("origin", None, None)),
            DirectionReviewOrigin::Part(name) => {
                Some(("origin", Some(name.trim().to_string()), None))
            }
        },
        AlignPick::Foot => match &options.foot {
            DirectionReviewFoot::Unset => None,
            DirectionReviewFoot::UserY(_) => Some(("foot", None, None)),
            DirectionReviewFoot::Anchor { part, name } => Some((
                "foot",
                Some(part.trim().to_string()),
                Some(name.trim().to_string()),
            )),
        },
        AlignPick::Anchor(index) => {
            let anchor = options.anchors.get(*index)?;
            Some((
                "anchor",
                Some(anchor.part.trim().to_string()),
                Some(anchor.name.trim().to_string()),
            ))
        }
    }
}

fn delta(
    reference: &DirectionReviewPoint,
    this: &DirectionReviewPoint,
    y_only: bool,
) -> Option<(i32, i32)> {
    let (ry, ty) = (reference.y?, this.y?);
    let dy = ry.checked_sub(ty)?;
    if y_only {
        return Some((0, dy));
    }
    let (rx, tx) = (reference.x?, this.x?);
    let dx = rx.checked_sub(tx)?;
    Some((dx, dy))
}

fn observe_bounds(built: &[DirBuilt], findings: &mut Vec<DirectionReviewFinding>) {
    let facings = facings_for(8);
    let reference = built
        .iter()
        .zip(facings.iter())
        .find(|(item, facing)| {
            item.chosen.kind == "present" && item.bounds.is_some() && facing.screen == "down"
        })
        .or_else(|| {
            built
                .iter()
                .zip(facings.iter())
                .find(|(item, _)| item.chosen.kind == "present" && item.bounds.is_some())
        });
    let Some((reference_item, reference_facing)) = reference else {
        return;
    };
    let Some(reference_bounds) = reference_item.bounds else {
        return;
    };
    for (item, facing) in built.iter().zip(facings.iter()) {
        if item.chosen.kind != "present" {
            continue;
        }
        let Some(bounds) = item.bounds else {
            findings.push(finding(
                "observation",
                "emptySubject",
                format!(
                    "facing {} ({}) has a frame and no non-zero cells. This is a hint, not an error.",
                    facing.screen, facing.compass
                ),
                Some(facing),
            ));
            continue;
        };
        if facing.index == reference_facing.index {
            continue;
        }
        let side = matches!(
            facing.screen,
            "left" | "right" | "upleft" | "upright" | "downleft" | "downright"
        );
        if side && bounds.width < reference_bounds.width {
            findings.push(finding(
                "observation",
                "sideNarrower",
                format!(
                    "facing {} ({}) subject width {} is narrower than {} ({}) width {}. A side or diagonal view often is. This is not an error.",
                    facing.screen,
                    facing.compass,
                    bounds.width,
                    reference_facing.screen,
                    reference_facing.compass,
                    reference_bounds.width
                ),
                Some(facing),
            ));
        }
        if bounds.width > reference_bounds.width || bounds.height > reference_bounds.height {
            findings.push(finding(
                "observation",
                "boundsWider",
                format!(
                    "facing {} ({}) subject bounds are larger than {} ({}). Cloth, a weapon, or an effect can extend past the body. This is not an error and is not a foot.",
                    facing.screen, facing.compass, reference_facing.screen, reference_facing.compass
                ),
                Some(facing),
            ));
        }
        let narrow_explained = side && bounds.width < reference_bounds.width;
        let position_differs = bounds.x != reference_bounds.x || bounds.y != reference_bounds.y;
        let height_shrinks = bounds.height < reference_bounds.height;
        if position_differs
            || height_shrinks
            || (bounds.width != reference_bounds.width
                && !narrow_explained
                && bounds.width <= reference_bounds.width
                && bounds.height <= reference_bounds.height)
        {
            // Width growth is boundsWider. A remaining width change on `up` is a hint.
            if bounds.width != reference_bounds.width
                && !narrow_explained
                && bounds.width < reference_bounds.width
                || position_differs
                || height_shrinks
            {
                findings.push(finding(
                    "observation",
                    "boundsDiffer",
                    format!(
                        "facing {} ({}) subject bounds differ from {} ({}). This is a hint, not an error, and it is not a foot line.",
                        facing.screen, facing.compass, reference_facing.screen, reference_facing.compass
                    ),
                    Some(facing),
                ));
            }
        }
    }
}

fn summary_baselines(
    options: &DirectionReviewOptions,
    built: &[DirBuilt],
) -> DirectionReviewBaselines {
    let anchors = options
        .anchors
        .iter()
        .enumerate()
        .map(|(index, spec)| {
            let point = summary_point(built.iter().map(|item| &item.anchors[index].point));
            named_from(spec, &point)
        })
        .collect();
    DirectionReviewBaselines {
        origin: summary_point(built.iter().map(|item| &item.origin.point)),
        foot: summary_point(built.iter().map(|item| &item.foot.point)),
        anchors,
    }
}

fn summary_point<'a>(
    points: impl Iterator<Item = &'a DirectionReviewPoint>,
) -> DirectionReviewPoint {
    let points: Vec<&DirectionReviewPoint> = points.collect();
    if points.is_empty() {
        return not_provided(None, false).point;
    }
    if points.iter().all(|point| point.status != "provided") {
        return points[0].clone();
    }
    let provided: Vec<&DirectionReviewPoint> = points
        .iter()
        .copied()
        .filter(|point| point.status == "provided")
        .collect();
    let first = provided[0];
    let same = provided
        .iter()
        .all(|point| point.x == first.x && point.y == first.y && point.source == first.source);
    if same {
        return first.clone();
    }
    DirectionReviewPoint {
        status: "provided",
        text: first.text.clone(),
        source: first.source,
        x: None,
        y: None,
        reason: Some("coordinates differ by facing; read each direction".to_string()),
    }
}

fn named_from(
    spec: &DirectionReviewAnchorName,
    point: &DirectionReviewPoint,
) -> DirectionReviewNamedPoint {
    DirectionReviewNamedPoint {
        part: spec.part.trim().to_string(),
        name: spec.name.trim().to_string(),
        status: point.status,
        text: point.text.clone(),
        source: point.source,
        x: point.x,
        y: point.y,
        reason: point.reason.clone(),
    }
}

fn subject_bounds(doc: &Document, frame: usize) -> Option<DirectionReviewBounds> {
    let pixels = doc.composite_pixels(frame).ok()?;
    let width = usize::from(doc.width());
    let mut min_x = u32::MAX;
    let mut min_y = u32::MAX;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut any = false;
    for (index, pixel) in pixels.iter().copied().enumerate() {
        if pixel == 0 {
            continue;
        }
        let x = (index % width) as u32;
        let y = (index / width) as u32;
        any = true;
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    if !any {
        return None;
    }
    Some(DirectionReviewBounds {
        x: min_x,
        y: min_y,
        width: max_x - min_x + 1,
        height: max_y - min_y + 1,
    })
}

struct Composed {
    meta: DirectionReviewImage,
    positions: Vec<u8>,
    aligned: Option<Vec<u8>>,
    overlays: Vec<Vec<u8>>,
}

fn compose(
    doc: &Document,
    options: &DirectionReviewOptions,
    built: &[DirBuilt],
    alignment: &DirectionReviewAlignment,
) -> Result<Composed, DirectionReviewError> {
    let scale = options.scale;
    let canvas_w = u32::from(doc.width());
    let canvas_h = u32::from(doc.height());
    let label_px = facings_for(8)
        .iter()
        .map(|facing| text_px(&facing_label(facing)))
        .max()
        .unwrap_or(0);
    let cell_w = canvas_w
        .max(label_px.saturating_add(2))
        .max(text_px("POSITION").saturating_add(2));
    let cell_h = LABEL_H + canvas_h;
    let grid_w = cell_w
        .checked_mul(3)
        .and_then(|value| value.checked_add(GUTTER * 2))
        .ok_or(DirectionReviewError::Scale { scale })?;
    let grid_h = cell_h
        .checked_mul(3)
        .and_then(|value| value.checked_add(GUTTER * 2))
        .ok_or(DirectionReviewError::Scale { scale })?;
    let positions = png_of(
        &draw_grid(doc, built, cell_w, cell_h, grid_w, grid_h, false, options),
        grid_w,
        grid_h,
        scale,
    )?;
    let aligned = if alignment.applied {
        Some(png_of(
            &draw_grid(doc, built, cell_w, cell_h, grid_w, grid_h, true, options),
            grid_w,
            grid_h,
            scale,
        )?)
    } else {
        None
    };
    let overlay_w = canvas_w.max(text_px("SE+SW MISS"));
    let overlay_h = LABEL_H + canvas_h;
    let mut overlays = Vec::new();
    for (a, b) in overlay_pairs() {
        let raw = draw_overlay(doc, built, a, b, overlay_w, overlay_h, alignment.applied);
        overlays.push(png_of(&raw, overlay_w, overlay_h, scale)?);
    }
    let meta = DirectionReviewImage {
        scale,
        cell_width: mul(cell_w, scale, scale)?,
        cell_height: mul(cell_h, scale, scale)?,
        label_height: mul(LABEL_H, scale, scale)?,
        gutter: mul(GUTTER, scale, scale)?,
        width: mul(grid_w, scale, scale)?,
        height: mul(grid_h, scale, scale)?,
        slots: facings_for(8)
            .iter()
            .map(|facing| {
                let (column, row) = grid_cell(facing.index);
                DirectionReviewSlot {
                    screen: facing.screen,
                    compass: facing.compass,
                    column,
                    row,
                }
            })
            .collect(),
        center: DirectionReviewSlot {
            screen: "center",
            compass: "C",
            column: 1,
            row: 1,
        },
    };
    Ok(Composed {
        meta,
        positions,
        aligned,
        overlays,
    })
}

fn mul(value: u32, scale: u32, reported: u32) -> Result<u32, DirectionReviewError> {
    value
        .checked_mul(scale)
        .ok_or(DirectionReviewError::Scale { scale: reported })
}

fn draw_grid(
    doc: &Document,
    built: &[DirBuilt],
    cell_w: u32,
    cell_h: u32,
    grid_w: u32,
    grid_h: u32,
    aligned: bool,
    options: &DirectionReviewOptions,
) -> Vec<u8> {
    let mut image = Image::new(grid_w, grid_h);
    for column in 0..2 {
        let x = (column + 1) * cell_w + column * GUTTER;
        image.fill_rect(x, 0, GUTTER, grid_h, GUTTER_RGBA);
    }
    for row in 0..2 {
        let y = (row + 1) * cell_h + row * GUTTER;
        image.fill_rect(0, y, grid_w, GUTTER, GUTTER_RGBA);
    }
    for (index, item) in built.iter().enumerate() {
        let facing = &facings_for(8)[index];
        let (column, row) = grid_cell(facing.index);
        let x = column * (cell_w + GUTTER);
        let y = row * (cell_h + GUTTER);
        image.fill_rect(x, y, cell_w, LABEL_H, LABEL_BG);
        image.text(
            x + 1,
            y + 1,
            &fit(&facing_label(facing), cell_w.saturating_sub(2)),
            LABEL_INK,
        );
        let frame_y = y + LABEL_H;
        if let Some(rgba) = &item.rgba {
            let shifted = if aligned {
                let offset = item.offset.unwrap_or(DirectionReviewOffset { x: 0, y: 0 });
                shift_rgba(rgba, doc.width(), doc.height(), offset.x, offset.y)
            } else {
                rgba.clone()
            };
            image.blit(
                x,
                frame_y,
                &shifted,
                u32::from(doc.width()),
                u32::from(doc.height()),
            );
        } else {
            image.empty_slot(x, frame_y, u32::from(doc.width()), u32::from(doc.height()));
        }
    }
    let (column, row) = (1u32, 1u32);
    let x = column * (cell_w + GUTTER);
    let y = row * (cell_h + GUTTER);
    image.fill_rect(x, y, cell_w, LABEL_H, LABEL_BG);
    let strip = if aligned { "ALIGNED" } else { "POSITION" };
    image.text(
        x + 1,
        y + 1,
        &fit(strip, cell_w.saturating_sub(2)),
        LABEL_INK,
    );
    image.fill_rect(x, y + LABEL_H, cell_w, u32::from(doc.height()), CENTER_BG);
    let mode = if options.column.is_some() {
        format!("COL {}", options.column.unwrap_or(0))
    } else {
        format!("AT {}", options.at)
    };
    let lines = [options.action.as_str(), mode.as_str(), "NOT ART"];
    let mut line_y = y + LABEL_H + 1;
    for line in lines {
        if line_y + 5 > y + cell_h {
            break;
        }
        image.text(
            x + 1,
            line_y,
            &fit(line, cell_w.saturating_sub(2)),
            LABEL_INK,
        );
        line_y += 6;
    }
    image.pixels
}

fn draw_overlay(
    doc: &Document,
    built: &[DirBuilt],
    a: &'static str,
    b: &'static str,
    width: u32,
    height: u32,
    aligned: bool,
) -> Vec<u8> {
    let mut image = Image::new(width, height);
    image.fill_rect(0, 0, width, LABEL_H, LABEL_BG);
    let left = built
        .iter()
        .enumerate()
        .find(|(index, _)| facings_for(8)[*index].screen == a);
    let right = built
        .iter()
        .enumerate()
        .find(|(index, _)| facings_for(8)[*index].screen == b);
    let miss = match (&left, &right) {
        (Some((_, item)), Some((_, other))) => {
            let left_miss = item.rgba.is_none();
            let right_miss = other.rgba.is_none();
            if left_miss || right_miss {
                " MISS"
            } else {
                ""
            }
        }
        _ => " MISS",
    };
    let label = format!(
        "{}+{}{miss}",
        left.map(|(index, _)| facings_for(8)[index].compass)
            .unwrap_or("?"),
        right
            .map(|(index, _)| facings_for(8)[index].compass)
            .unwrap_or("?")
    );
    image.text(1, 1, &fit(&label, width.saturating_sub(2)), LABEL_INK);
    let canvas_w = u32::from(doc.width());
    let canvas_h = u32::from(doc.height());
    let left_rgba = left.and_then(|(_, item)| shifted(doc, item, aligned));
    let right_rgba = right.and_then(|(_, item)| shifted(doc, item, aligned));
    match (left_rgba, right_rgba) {
        (None, None) => image.empty_slot(0, LABEL_H, canvas_w, canvas_h),
        (left_rgba, right_rgba) => {
            let blank = vec![0u8; canvas_w as usize * canvas_h as usize * 4];
            let left_px = left_rgba.as_deref().unwrap_or(blank.as_slice());
            let right_px = right_rgba.as_deref().unwrap_or(blank.as_slice());
            let mut onion = vec![0u8; blank.len()];
            for (index, pixel) in onion.chunks_exact_mut(4).enumerate() {
                let a = [
                    left_px[index * 4],
                    left_px[index * 4 + 1],
                    left_px[index * 4 + 2],
                    left_px[index * 4 + 3],
                ];
                let b = [
                    right_px[index * 4],
                    right_px[index * 4 + 1],
                    right_px[index * 4 + 2],
                    right_px[index * 4 + 3],
                ];
                pixel.copy_from_slice(&onion_pixel(a, b));
            }
            image.blit(0, LABEL_H, &onion, canvas_w, canvas_h);
        }
    }
    image.pixels
}

fn shifted(doc: &Document, item: &DirBuilt, aligned: bool) -> Option<Vec<u8>> {
    let rgba = item.rgba.clone()?;
    if aligned {
        if let Some(offset) = item.offset {
            return Some(shift_rgba(
                &rgba,
                doc.width(),
                doc.height(),
                offset.x,
                offset.y,
            ));
        }
    }
    Some(rgba)
}

fn onion_pixel(a: [u8; 4], b: [u8; 4]) -> [u8; 4] {
    if a[3] == 0 {
        return b;
    }
    if b[3] == 0 {
        return a;
    }
    [
        ((u16::from(a[0]) + u16::from(b[0])) / 2) as u8,
        ((u16::from(a[1]) + u16::from(b[1])) / 2) as u8,
        ((u16::from(a[2]) + u16::from(b[2])) / 2) as u8,
        a[3].max(b[3]),
    ]
}

fn shift_rgba(src: &[u8], width: u16, height: u16, dx: i32, dy: i32) -> Vec<u8> {
    let mut out = vec![0u8; src.len()];
    let width = i32::from(width);
    let height = i32::from(height);
    let stride = width as usize;
    for y in 0..height {
        for x in 0..width {
            let nx = x + dx;
            let ny = y + dy;
            if nx < 0 || ny < 0 || nx >= width || ny >= height {
                continue;
            }
            let from = ((y as usize) * stride + x as usize) * 4;
            let to = ((ny as usize) * stride + nx as usize) * 4;
            out[to..to + 4].copy_from_slice(&src[from..from + 4]);
        }
    }
    out
}

fn png_of(
    rgba: &[u8],
    width: u32,
    height: u32,
    scale: u32,
) -> Result<Vec<u8>, DirectionReviewError> {
    let scaled = scale_image(rgba, width, height, scale)?;
    let out_w = mul(width, scale, scale)?;
    let out_h = mul(height, scale, scale)?;
    Ok(encode_rgba_png(out_w, out_h, &scaled))
}

fn scale_image(
    rgba: &[u8],
    width: u32,
    height: u32,
    scale: u32,
) -> Result<Vec<u8>, DirectionReviewError> {
    if scale == 1 {
        return Ok(rgba.to_vec());
    }
    let out_w = mul(width, scale, scale)?;
    let out_h = mul(height, scale, scale)?;
    let scale_usize = usize::try_from(scale).map_err(|_| DirectionReviewError::Scale { scale })?;
    let src_w = usize::try_from(width).map_err(|_| DirectionReviewError::Scale { scale })?;
    let src_h = usize::try_from(height).map_err(|_| DirectionReviewError::Scale { scale })?;
    let dst_w = usize::try_from(out_w).map_err(|_| DirectionReviewError::Scale { scale })?;
    let len = dst_w
        .checked_mul(usize::try_from(out_h).map_err(|_| DirectionReviewError::Scale { scale })?)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(DirectionReviewError::Scale { scale })?;
    let mut out = vec![0u8; len];
    for y in 0..src_h {
        for x in 0..src_w {
            let pixel = &rgba[(y * src_w + x) * 4..(y * src_w + x) * 4 + 4];
            let x0 = x * scale_usize;
            let y0 = y * scale_usize;
            for dy in 0..scale_usize {
                let row = (y0 + dy) * dst_w;
                for dx in 0..scale_usize {
                    let offset = (row + x0 + dx) * 4;
                    out[offset..offset + 4].copy_from_slice(pixel);
                }
            }
        }
    }
    let _ = out_h;
    Ok(out)
}

fn facing_label(facing: &Facing) -> String {
    format!("{} {}", facing.compass, facing.screen)
}

fn grid_cell(index: u8) -> (u32, u32) {
    match index {
        0 => (1, 2), // S down
        1 => (0, 1), // W left
        2 => (2, 1), // E right
        3 => (1, 0), // N up
        4 => (0, 2), // SW
        5 => (2, 2), // SE
        6 => (0, 0), // NW
        7 => (2, 0), // NE
        _ => (1, 1),
    }
}

fn overlay_pairs() -> [(&'static str, &'static str); 8] {
    [
        ("up", "upright"),
        ("upright", "right"),
        ("right", "downright"),
        ("downright", "down"),
        ("down", "downleft"),
        ("downleft", "left"),
        ("left", "upleft"),
        ("upleft", "up"),
    ]
}

fn overlay_meta(directions: &[DirectionReviewFacing]) -> Vec<DirectionReviewOverlay> {
    overlay_pairs()
        .into_iter()
        .map(|(a, b)| {
            let left = directions.iter().find(|item| item.screen == a);
            let right = directions.iter().find(|item| item.screen == b);
            DirectionReviewOverlay {
                file: format!("overlay-{a}-{b}.png"),
                a,
                b,
                a_compass: left.map(|item| item.compass).unwrap_or("?"),
                b_compass: right.map(|item| item.compass).unwrap_or("?"),
                a_slot: left.map(|item| item.slot).unwrap_or("missingDirection"),
                b_slot: right.map(|item| item.slot).unwrap_or("missingDirection"),
                a_offset: left.and_then(|item| item.offset),
                b_offset: right.and_then(|item| item.offset),
            }
        })
        .collect()
}

fn finding(
    severity: &'static str,
    code: &'static str,
    message: String,
    facing: Option<&Facing>,
) -> DirectionReviewFinding {
    DirectionReviewFinding {
        severity,
        code,
        message,
        facing: facing.map(|facing| facing.screen),
        compass: facing.map(|facing| facing.compass),
        dx: None,
        dy: None,
    }
}

struct Image {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl Image {
    fn new(width: u32, height: u32) -> Self {
        let len = width as usize * height as usize * 4;
        Self {
            width,
            height,
            pixels: vec![0; len],
        }
    }

    fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: [u8; 4]) {
        for row in y..y.saturating_add(h).min(self.height) {
            for col in x..x.saturating_add(w).min(self.width) {
                self.set(col, row, color);
            }
        }
    }

    fn set(&mut self, x: u32, y: u32, color: [u8; 4]) {
        if x >= self.width || y >= self.height {
            return;
        }
        let offset = (y as usize * self.width as usize + x as usize) * 4;
        self.pixels[offset..offset + 4].copy_from_slice(&color);
    }

    fn blit(&mut self, x: u32, y: u32, src: &[u8], width: u32, height: u32) {
        for row in 0..height {
            for col in 0..width {
                let from = (row as usize * width as usize + col as usize) * 4;
                if from + 4 > src.len() {
                    return;
                }
                let pixel = [src[from], src[from + 1], src[from + 2], src[from + 3]];
                self.set(x.saturating_add(col), y.saturating_add(row), pixel);
            }
        }
    }

    fn text(&mut self, x: u32, y: u32, text: &str, color: [u8; 4]) {
        let mut cursor = x;
        for ch in text.chars() {
            let glyph = glyph(ch);
            for (dy, bits) in glyph.iter().enumerate() {
                for dx in 0..3 {
                    if bits & (1 << (2 - dx)) != 0 {
                        self.set(cursor + dx, y + dy as u32, color);
                    }
                }
            }
            cursor = cursor.saturating_add(4);
        }
    }

    fn empty_slot(&mut self, x: u32, y: u32, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        for col in x..x.saturating_add(w).min(self.width) {
            self.set(col, y, EMPTY_MARK);
            if h > 1 {
                self.set(col, y + h - 1, EMPTY_MARK);
            }
        }
        for row in y..y.saturating_add(h).min(self.height) {
            self.set(x, row, EMPTY_MARK);
            if w > 1 {
                self.set(x + w - 1, row, EMPTY_MARK);
            }
        }
        for row in 1..h.saturating_sub(1) {
            for col in 1..w.saturating_sub(1) {
                if (col + row) % 4 == 0 {
                    self.set(x + col, y + row, EMPTY_MARK);
                }
            }
        }
        let label = fit("MISS", w.saturating_sub(2));
        let label_w = text_px(&label);
        let text_x = x + w.saturating_sub(label_w) / 2;
        let text_y = y + h.saturating_sub(5) / 2;
        self.text(text_x, text_y, &label, LABEL_INK);
    }
}

fn text_px(text: &str) -> u32 {
    let count = text.chars().count() as u32;
    if count == 0 {
        0
    } else {
        count * 4 - 1
    }
}

fn fit(text: &str, max_w: u32) -> String {
    let mut out: String = text
        .chars()
        .filter(|ch| ch.is_ascii() && !ch.is_ascii_control())
        .collect();
    while text_px(&out) > max_w && !out.is_empty() {
        out.pop();
    }
    out
}

fn glyph(ch: char) -> [u8; 5] {
    match ch.to_ascii_uppercase() {
        'A' => [0b111, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b111, 0b100, 0b100, 0b100, 0b111],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b111, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b111, 0b100, 0b100],
        'G' => [0b111, 0b100, 0b101, 0b101, 0b111],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'J' => [0b001, 0b001, 0b001, 0b101, 0b111],
        'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => [0b101, 0b111, 0b101, 0b101, 0b101],
        'N' => [0b110, 0b101, 0b101, 0b101, 0b101],
        'O' => [0b111, 0b101, 0b101, 0b101, 0b111],
        'P' => [0b111, 0b101, 0b111, 0b100, 0b100],
        'Q' => [0b111, 0b101, 0b101, 0b111, 0b001],
        'R' => [0b111, 0b101, 0b111, 0b110, 0b101],
        'S' => [0b111, 0b100, 0b111, 0b001, 0b111],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'W' => [0b101, 0b101, 0b101, 0b111, 0b101],
        'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        'Y' => [0b101, 0b101, 0b111, 0b010, 0b010],
        'Z' => [0b111, 0b001, 0b010, 0b100, 0b111],
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b001, 0b001, 0b001],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        '+' => [0b000, 0b010, 0b111, 0b010, 0b000],
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        _ => [0, 0, 0, 0, 0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CurveChannel, DirectionLayout, LoopMode};
    use png::{BitDepth, ColorType, Decoder};
    use std::io::Cursor;

    fn decode(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
        let decoder = Decoder::new(Cursor::new(bytes));
        let mut reader = decoder.read_info().expect("png");
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).expect("frame");
        assert_eq!(info.color_type, ColorType::Rgba);
        assert_eq!(info.bit_depth, BitDepth::Eight);
        buf.truncate(info.buffer_size());
        (info.width, info.height, buf)
    }

    fn sheet(frames: usize) -> Document {
        let mut doc = Document::new(16, 16).expect("canvas");
        for _ in 1..frames {
            doc.add_frame();
        }
        doc
    }

    fn tag_all(doc: &mut Document, action: &str, columns: u32, rel_from: u32, rel_to: u32) {
        for facing in facings_for(8) {
            let row = u32::from(facing.index) * columns;
            doc.add_tag(
                &format!("{action}_{}", facing.screen),
                (row + rel_from) as usize,
                (row + rel_to) as usize,
                LoopMode::Forward,
            )
            .expect("tag");
        }
    }

    fn opts(action: &str) -> DirectionReviewOptions {
        DirectionReviewOptions {
            action: action.to_string(),
            at: 0,
            column: None,
            scale: 1,
            tolerance: 0,
            origin: DirectionReviewOrigin::Unset,
            foot: DirectionReviewFoot::Unset,
            anchors: Vec::new(),
            align: DirectionReviewAlign::Auto,
        }
    }

    fn pixel(buf: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let offset = (y * width + x) as usize * 4;
        [
            buf[offset],
            buf[offset + 1],
            buf[offset + 2],
            buf[offset + 3],
        ]
    }

    fn slot_pixel(
        report: &DirectionReviewReport,
        png: &[u8],
        screen: &str,
        local_x: u32,
        local_y: u32,
    ) -> [u8; 4] {
        let (width, _, buf) = decode(png);
        assert_eq!(width, report.image.width);
        let slot = report
            .image
            .slots
            .iter()
            .find(|slot| slot.screen == screen)
            .expect("slot");
        let x = slot.column * (report.image.cell_width + report.image.gutter)
            + local_x * report.image.scale;
        let y = slot.row * (report.image.cell_height + report.image.gutter)
            + report.image.label_height
            + local_y * report.image.scale;
        pixel(&buf, width, x, y)
    }

    #[test]
    fn eight_directions_keep_one_action_and_do_not_mutate() {
        let mut doc = sheet(16);
        doc.set_direction_layout(DirectionLayout::new(8, 2).unwrap(), false)
            .unwrap();
        tag_all(&mut doc, "idle", 2, 0, 0);
        tag_all(&mut doc, "attack", 2, 1, 1);
        for frame in 0..16 {
            doc.set_active_frame(frame).unwrap();
            let index = if frame % 2 == 0 { 6 } else { 3 };
            doc.paint(6, 11, index).unwrap();
        }
        let before = doc.clone();
        let review = doc.review_directions(&opts("attack")).unwrap();
        assert_eq!(doc, before);
        assert!(!review.report.judges_art);
        assert_eq!(review.report.format, DIRECTION_REVIEW_FORMAT);
        assert_eq!(review.report.version, 1);
        assert_eq!(review.report.error_count, 0);
        assert!(review.aligned_png.is_none());
        for facing in &review.report.directions {
            assert!(!facing.borrowed);
            assert_eq!(facing.slot, "present");
            assert_eq!(
                facing.frame,
                Some(
                    u32::from(
                        facings_for(8)
                            .iter()
                            .find(|item| item.screen == facing.screen)
                            .unwrap()
                            .index
                    ) * 2
                        + 1
                )
            );
            assert_eq!(facing.origin.text, NOT_PROVIDED);
            assert!(facing.origin.x.is_none());
            assert_eq!(facing.foot.text, NOT_PROVIDED);
        }
        let red = [232, 93, 76, 255];
        let green = [106, 176, 106, 255];
        assert_eq!(
            slot_pixel(&review.report, &review.positions_png, "down", 6, 11),
            red
        );
        assert_ne!(
            slot_pixel(&review.report, &review.positions_png, "left", 6, 11),
            green
        );
        assert_eq!(review.overlay_pngs.len(), 8);
        assert!(review.report.alignment.reason.as_deref() == Some(NOT_PROVIDED));
    }

    #[test]
    fn a_missing_direction_stays_empty() {
        let mut doc = sheet(8);
        doc.set_direction_layout(DirectionLayout::new(8, 1).unwrap(), false)
            .unwrap();
        for facing in facings_for(8) {
            if facing.screen == "upleft" {
                continue;
            }
            let frame = usize::from(facing.index);
            doc.add_tag(
                &format!("attack_{}", facing.screen),
                frame,
                frame,
                LoopMode::Forward,
            )
            .unwrap();
            doc.set_active_frame(frame).unwrap();
            doc.paint(2, 2, 3).unwrap();
        }
        let before = doc.save_project();
        let review = doc.review_directions(&opts("attack")).unwrap();
        assert_eq!(doc.save_project(), before);
        let missing = review
            .report
            .directions
            .iter()
            .find(|item| item.screen == "upleft")
            .unwrap();
        assert_eq!(missing.slot, "missingDirection");
        assert!(missing.frame.is_none());
        assert!(!missing.borrowed);
        assert!(review.report.findings.iter().any(|item| {
            item.severity == "structural"
                && item.code == "missingDirection"
                && item.facing == Some("upleft")
        }));
        assert!(review.report.error_count >= 1);
        let (width, _, buf) = decode(&review.positions_png);
        let slot = review
            .report
            .image
            .slots
            .iter()
            .find(|slot| slot.screen == "upleft")
            .unwrap();
        let x0 = slot.column * (review.report.image.cell_width + review.report.image.gutter);
        let y0 = slot.row * (review.report.image.cell_height + review.report.image.gutter)
            + review.report.image.label_height;
        let red = [232, 93, 76, 255];
        for y in y0..y0 + 16 {
            for x in x0..x0 + 16 {
                assert_ne!(pixel(&buf, width, x, y), red);
            }
        }
        let overlay = review
            .report
            .overlays
            .iter()
            .position(|item| item.a == "left" && item.b == "upleft")
            .unwrap();
        assert_eq!(review.report.overlays[overlay].b_slot, "missingDirection");
        let (_, _, onion) = decode(&review.overlay_pngs[overlay]);
        assert!(onion.chunks_exact(4).any(|pixel| pixel == red));
        let down_only = review
            .report
            .overlays
            .iter()
            .position(|item| item.a == "down" && item.b == "downleft")
            .unwrap();
        let _ = down_only;
    }

    #[test]
    fn column_of_walk_does_not_read_idle() {
        let mut doc = sheet(16);
        doc.set_direction_layout(DirectionLayout::new(8, 2).unwrap(), false)
            .unwrap();
        tag_all(&mut doc, "idle", 2, 0, 0);
        tag_all(&mut doc, "walk", 2, 1, 1);
        for frame in 0..16 {
            doc.set_active_frame(frame).unwrap();
            doc.paint(1, 1, if frame % 2 == 0 { 5 } else { 7 }).unwrap();
        }
        let mut options = opts("walk");
        options.column = Some(0);
        let review = doc.review_directions(&options).unwrap();
        assert!(review
            .report
            .directions
            .iter()
            .all(|item| { item.slot == "missingFrame" && item.frame.is_none() && !item.borrowed }));
        let yellow = [242, 204, 102, 255];
        let (width, _, buf) = decode(&review.positions_png);
        assert!(buf.chunks_exact(4).all(|pixel| pixel != yellow));
        let _ = width;
    }

    #[test]
    fn a_known_curve_offset_is_alignment_and_pixels_are_not_the_foot() {
        let mut doc = sheet(8);
        doc.set_direction_layout(DirectionLayout::new(8, 1).unwrap(), false)
            .unwrap();
        tag_all(&mut doc, "attack", 1, 0, 0);
        doc.add_part("body", 6, 8, None).unwrap();
        doc.set_anchor("body", "sole", 0, 3).unwrap();
        for frame in 0..8 {
            doc.set_active_frame(frame).unwrap();
            doc.paint(6, 11, 3).unwrap();
            let value = if frame == 2 { 3.0 } else { 0.0 };
            doc.set_curve_key("body", CurveChannel::TranslateY, frame, value, None)
                .unwrap();
        }
        let before = doc.clone();
        let mut options = opts("attack");
        options.anchors.push(DirectionReviewAnchorName {
            part: "body".to_string(),
            name: "sole".to_string(),
        });
        options.foot = DirectionReviewFoot::Anchor {
            part: "body".to_string(),
            name: "sole".to_string(),
        };
        let review = doc.review_directions(&options).unwrap();
        assert_eq!(doc, before);
        let right = review
            .report
            .directions
            .iter()
            .find(|item| item.screen == "right")
            .unwrap();
        let down = review
            .report
            .directions
            .iter()
            .find(|item| item.screen == "down")
            .unwrap();
        assert_eq!(right.foot.source, Some("curve"));
        assert_eq!(right.foot.y, Some(14));
        assert_eq!(down.foot.y, Some(11));
        assert_eq!(right.subject_bounds.unwrap().y, 11);
        assert_ne!(
            right.foot.y,
            Some(
                right.subject_bounds.unwrap().y as i32
                    + right.subject_bounds.unwrap().height as i32
                    - 1
            )
        );
        assert_eq!(right.offset, Some(DirectionReviewOffset { x: 0, y: -3 }));
        assert!(review.report.findings.iter().any(|item| {
            item.severity == "alignment"
                && item.code == "offset"
                && item.facing == Some("right")
                && item.dy == Some(-3)
        }));
        assert_eq!(review.report.alignment.source, Some("curve"));
        assert!(review.aligned_png.is_some());
        let red = [232, 93, 76, 255];
        assert_eq!(
            slot_pixel(&review.report, &review.positions_png, "right", 6, 11),
            red
        );
        let aligned = review.aligned_png.as_ref().unwrap();
        assert_eq!(slot_pixel(&review.report, aligned, "right", 6, 8), red);
        assert_ne!(slot_pixel(&review.report, aligned, "right", 6, 11), red);
        assert_eq!(slot_pixel(&review.report, aligned, "down", 6, 11), red);

        options.tolerance = 3;
        let loose = doc.review_directions(&options).unwrap();
        assert_eq!(loose.report.alignment_count, 0);
        assert!(loose
            .report
            .findings
            .iter()
            .all(|item| item.severity != "alignment"));
    }

    #[test]
    fn a_missing_anchor_is_structural_and_not_a_bounding_box() {
        let mut doc = sheet(8);
        doc.set_direction_layout(DirectionLayout::new(8, 1).unwrap(), false)
            .unwrap();
        tag_all(&mut doc, "attack", 1, 0, 0);
        doc.add_part("body", 4, 4, None).unwrap();
        for frame in 0..8 {
            doc.set_active_frame(frame).unwrap();
            doc.fill_rect(4, 4, 6, 8, 7).unwrap();
            doc.paint(4, 15, 4).unwrap();
        }
        let mut options = opts("attack");
        options.anchors.push(DirectionReviewAnchorName {
            part: "body".to_string(),
            name: "sole".to_string(),
        });
        let review = doc.review_directions(&options).unwrap();
        assert!(review
            .report
            .findings
            .iter()
            .any(|item| item.severity == "structural" && item.code == "missingAnchor"));
        assert!(review.report.alignment.applied == false);
        assert_eq!(
            review.report.alignment.reason.as_deref(),
            Some(NOT_PROVIDED)
        );
        for facing in &review.report.directions {
            assert_eq!(facing.anchors[0].text, NOT_PROVIDED);
            assert!(facing.anchors[0].x.is_none());
            assert_eq!(facing.origin.text, NOT_PROVIDED);
            let bounds = facing.subject_bounds.unwrap();
            assert_eq!(bounds.y + bounds.height - 1, 15);
        }
        assert!(review
            .report
            .findings
            .iter()
            .all(|item| item.severity != "alignment"));
    }

    #[test]
    fn side_narrowing_and_a_flame_are_hints() {
        let mut doc = sheet(8);
        doc.set_direction_layout(DirectionLayout::new(8, 1).unwrap(), false)
            .unwrap();
        tag_all(&mut doc, "idle", 1, 0, 0);
        doc.add_part("body", 8, 12, None).unwrap();
        doc.set_anchor("body", "sole", 0, 0).unwrap();
        for facing in facings_for(8) {
            doc.set_active_frame(usize::from(facing.index)).unwrap();
            let side = matches!(
                facing.screen,
                "left" | "right" | "upleft" | "upright" | "downleft" | "downright"
            );
            if side {
                doc.fill_rect(6, 4, 3, 8, 7).unwrap();
            } else {
                doc.fill_rect(4, 4, 8, 8, 7).unwrap();
            }
            if facing.screen == "up" {
                doc.fill_rect(0, 0, 2, 2, 4).unwrap();
            }
        }
        let mut options = opts("idle");
        options.anchors.push(DirectionReviewAnchorName {
            part: "body".to_string(),
            name: "sole".to_string(),
        });
        let review = doc.review_directions(&options).unwrap();
        assert_eq!(review.report.structural_count, 0);
        assert_eq!(review.report.alignment_count, 0);
        assert!(review.report.error_count == 0);
        assert!(review.report.findings.iter().any(|item| {
            item.severity == "observation"
                && item.code == "sideNarrower"
                && item.facing == Some("left")
        }));
        assert!(review.report.findings.iter().any(|item| {
            item.severity == "observation"
                && item.code == "sideNarrower"
                && item.facing == Some("right")
        }));
        assert!(review
            .report
            .findings
            .iter()
            .all(|item| { item.code != "sideNarrower" || item.facing != Some("up") }));
        assert!(review.report.findings.iter().any(|item| {
            item.severity == "observation"
                && item.code == "boundsWider"
                && item.facing == Some("up")
        }));
        assert!(review
            .report
            .findings
            .iter()
            .any(|item| item.code == "restPoseBaseline"));
        let up = review
            .report
            .directions
            .iter()
            .find(|item| item.screen == "up")
            .unwrap();
        assert_eq!(up.anchors[0].source, Some("restPose"));
        assert_eq!(up.anchors[0].y, Some(12));
        assert_eq!(up.subject_bounds.unwrap().y, 0);
        assert!(review.report.offsets_are_zero());
    }

    #[test]
    fn pixel_shift_without_a_curve_is_not_an_alignment_error() {
        let mut doc = sheet(8);
        doc.set_direction_layout(DirectionLayout::new(8, 1).unwrap(), false)
            .unwrap();
        tag_all(&mut doc, "attack", 1, 0, 0);
        doc.add_part("body", 6, 8, None).unwrap();
        doc.set_anchor("body", "sole", 0, 3).unwrap();
        for frame in 0..8 {
            doc.set_active_frame(frame).unwrap();
            let y = if frame == 2 { 14 } else { 11 };
            doc.paint(6, y, 3).unwrap();
        }
        let mut options = opts("attack");
        options.origin = DirectionReviewOrigin::Part("body".to_string());
        let review = doc.review_directions(&options).unwrap();
        assert_eq!(review.report.alignment_count, 0);
        assert!(review.report.directions.iter().all(|item| {
            item.origin.source == Some("restPose")
                && item.origin.y == Some(8)
                && item.offset == Some(DirectionReviewOffset { x: 0, y: 0 })
        }));
        let right = review
            .report
            .directions
            .iter()
            .find(|item| item.screen == "right")
            .unwrap();
        assert_eq!(right.subject_bounds.unwrap().y, 14);
        assert!(review
            .report
            .findings
            .iter()
            .any(|item| item.code == "boundsDiffer"
                && item.facing == Some("right")
                && item.severity == "observation"));
        assert!(review
            .report
            .findings
            .iter()
            .all(|item| item.severity != "alignment"));
    }

    #[test]
    fn zombie_idle_uses_idle_frames_and_leaves_diagonals_empty() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/game-pack-zombie/zombie/zombie.json"
        );
        let text = std::fs::read_to_string(path).expect("zombie fixture");
        let doc = Document::load_project(&text).expect("zombie");
        let before = doc.clone();
        let review = doc.review_directions(&opts("idle")).unwrap();
        assert_eq!(doc, before);
        let columns = doc.direction_layout().unwrap().columns;
        for facing in ["down", "left", "right", "up"] {
            let item = review
                .report
                .directions
                .iter()
                .find(|item| item.screen == facing)
                .unwrap();
            let row = facings_for(8)
                .iter()
                .find(|item| item.screen == facing)
                .unwrap()
                .index;
            assert_eq!(item.slot, "present");
            assert_eq!(item.frame, Some(u32::from(row) * columns));
            assert!(!item.borrowed);
        }
        for facing in ["downleft", "downright", "upleft", "upright"] {
            let item = review
                .report
                .directions
                .iter()
                .find(|item| item.screen == facing)
                .unwrap();
            assert_eq!(item.slot, "missingDirection");
            assert!(item.frame.is_none());
        }
        let walk = doc.review_directions(&opts("walk")).unwrap();
        let idle_down = review
            .report
            .directions
            .iter()
            .find(|item| item.screen == "down")
            .unwrap()
            .frame;
        let walk_down = walk
            .report
            .directions
            .iter()
            .find(|item| item.screen == "down")
            .unwrap()
            .frame;
        assert_ne!(idle_down, walk_down);
    }

    impl DirectionReviewReport {
        fn offsets_are_zero(&self) -> bool {
            self.directions
                .iter()
                .all(|item| item.offset == Some(DirectionReviewOffset { x: 0, y: 0 }))
        }
    }

    #[test]
    fn refused_action_and_scale_do_not_need_a_file_write() {
        let doc = Document::new(8, 8).unwrap();
        let mut options = opts("attack_down");
        assert!(matches!(
            doc.review_directions(&options),
            Err(DirectionReviewError::Action)
        ));
        options.action = "attack".to_string();
        options.scale = 0;
        assert!(matches!(
            doc.review_directions(&options),
            Err(DirectionReviewError::Scale { scale: 0 })
        ));
    }
}
