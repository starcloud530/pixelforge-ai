//! Stable Tool API for agents.
//!
//! [`Document::call_tool`] reads one JSON request and writes one JSON response.
//! Every mutation calls the same [`Document`] methods the shell uses, so the
//! undo stack, palette lock, and project file stay the source of truth. The
//! response carries a structured error when a call is refused. The document
//! is left unchanged on failure, except that [`Document::load_project`]
//! replaces it only after the file parses.
//!
//! This module does not call a model and does not read an API key. Callers
//! enter through [`Document::call_tool`]: the `pixelforge` CLI
//! (`docs/agent-skill.md`) and, as an optional one-turn demo, the TypeScript
//! driver in `docs/ai-driver.md`. The catalog is `docs/tool-api.md`.

use serde::Serialize;
use serde_json::{Map, Number, Value};

use crate::aseprite::AsepriteReport;
use crate::constraint::{Constraint, ConstraintKind, Contact, SolveReport};
use crate::curve::{CurveChannel, CurveSample, Ease, PartSample};
use crate::document::{BakeOptions, Document, QuarterTurn};
use crate::error::KernelError;
use crate::palette::IndexPolicy;
use crate::palette_file::PaletteImportMode;
use crate::palette_map::{ColorMerge, DroppedColor, MapReport};
use crate::part::{LayerAttach, PartUpdate};
use crate::scene::{self, CellBounds, CellChange, DiffSummary, DirtyRegion, ViewSummary};
use crate::selection::{Mask, MaskOp, Selection};
use crate::tag::{LoopMode, TagUpdate};
use crate::LayerRole;
use crate::MAX_CANVAS_EDGE;

/// Version written on every response. An omitted request version means this one.
pub const TOOL_API_VERSION: u32 = 1;

/// Tool names [`Document::call_tool`] accepts, in catalog order.
///
/// `docs/tool-api.md` and `apps/web/src/kernel/toolApi.ts` list the same names.
pub const TOOL_NAMES: &[&str] = &[
    "inspect",
    "scene",
    "paint",
    "fillRect",
    "erase",
    "eraseRect",
    "copyRect",
    "moveRect",
    "setSelection",
    "clearSelection",
    "combineMask",
    "replaceIndex",
    "setPaletteColor",
    "addPaletteColor",
    "importPalette",
    "setPaletteLocked",
    "setIndexPolicy",
    "remapPalette",
    "applyPng",
    "setActiveFrame",
    "addFrame",
    "deleteFrame",
    "duplicateFrame",
    "setFrameDelay",
    "addTag",
    "setTag",
    "removeTag",
    "listParts",
    "addPart",
    "setPart",
    "removePart",
    "setAnchor",
    "removeAnchor",
    "translatePart",
    "rotatePart",
    "flipPart",
    "setCurveKey",
    "removeCurveKey",
    "clearCurve",
    "listCurves",
    "sampleCurve",
    "samplePart",
    "addConstraint",
    "removeConstraint",
    "listConstraints",
    "solveConstraints",
    "bake",
    "beginGesture",
    "endGesture",
    "undo",
    "redo",
    "saveProject",
    "loadProject",
    "importAseprite",
    "exportPngSequence",
    "exportSpriteSheet",
    "previewFrame",
    "diffFrames",
    "flipHorizontal",
    "flipVertical",
    "rotate",
    "resizeCanvas",
    "listLayers",
    "addLayer",
    "deleteLayer",
    "renameLayer",
    "setLayerRole",
    "setActiveLayer",
    "reorderLayer",
    "flattenLayers",
];

const U8_MAX: u64 = 255;
const U16_MAX: u64 = 65_535;
const U32_MAX: u64 = 4_294_967_295;

impl Document {
    /// Runs one Tool API request and returns the JSON response.
    ///
    /// The response is always JSON. `ok` is false when the tool is refused;
    /// this method does not return that refusal as a [`Result`]. See
    /// `docs/tool-api.md`.
    #[must_use = "tool errors are reported in the JSON body"]
    pub fn call_tool(&mut self, request: &str) -> String {
        let request = request.strip_prefix('\u{feff}').unwrap_or(request);
        let (id, outcome) = match parse_request(request) {
            Ok(call) => {
                let id = call.id.clone();
                (id, dispatch(self, &call))
            }
            Err(error) => (error.id, Err(error.failure)),
        };
        match outcome {
            Ok(result) => encode(&Success {
                ok: true,
                api_version: TOOL_API_VERSION,
                id,
                result,
            }),
            Err(failure) => encode(&Failure {
                ok: false,
                api_version: TOOL_API_VERSION,
                id,
                error: failure.body(),
            }),
        }
    }
}

struct Call {
    id: Option<RequestId>,
    tool: String,
    params: Value,
}

struct RequestError {
    id: Option<RequestId>,
    failure: ToolFailure,
}

#[derive(Clone, Serialize)]
#[serde(untagged)]
enum RequestId {
    Text(String),
    Number(Number),
}

enum ToolFailure {
    InvalidRequest(String),
    UnsupportedVersion { version: u32 },
    UnknownTool { name: String },
    InvalidParams { message: String },
    Kernel(KernelError),
}

struct ErrorBody {
    code: &'static str,
    message: String,
}

impl ToolFailure {
    fn body(self) -> ErrorBody {
        match self {
            Self::InvalidRequest(message) => ErrorBody {
                code: "InvalidRequest",
                message,
            },
            Self::UnsupportedVersion { version } => ErrorBody {
                code: "UnsupportedVersion",
                message: format!("tool api version {version} is not supported"),
            },
            Self::UnknownTool { name } => ErrorBody {
                code: "UnknownTool",
                message: format!("unknown tool \"{name}\""),
            },
            Self::InvalidParams { message } => ErrorBody {
                code: "InvalidParams",
                message,
            },
            Self::Kernel(error) => ErrorBody {
                code: error.code(),
                message: error.to_string(),
            },
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Success {
    ok: bool,
    api_version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<RequestId>,
    result: Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Failure {
    ok: bool,
    api_version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<RequestId>,
    error: ErrorBody,
}

impl Serialize for ErrorBody {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut body = serializer.serialize_struct("ErrorBody", 2)?;
        body.serialize_field("code", self.code)?;
        body.serialize_field("message", &self.message)?;
        body.end()
    }
}

fn encode<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("tool response is JSON")
}

fn parse_request(request: &str) -> Result<Call, RequestError> {
    let value: Value = match serde_json::from_str(request) {
        Ok(value) => value,
        Err(_) => {
            return Err(request_error(
                None,
                ToolFailure::InvalidRequest("request is not JSON".to_string()),
            ));
        }
    };
    let Value::Object(map) = value else {
        return Err(request_error(
            None,
            ToolFailure::InvalidRequest("request must be a JSON object".to_string()),
        ));
    };
    let id = match optional_id(&map) {
        Ok(id) => id,
        Err(failure) => return Err(request_error(None, failure)),
    };
    let fail = |failure| request_error(id.clone(), failure);
    if let Err(failure) = reject_unknown(&map, &["apiVersion", "id", "tool", "params"], |field| {
        ToolFailure::InvalidRequest(format!("unknown field {field}"))
    }) {
        return Err(fail(failure));
    }
    let api_version = match optional_version(&map) {
        Ok(version) => version,
        Err(failure) => return Err(fail(failure)),
    };
    if let Some(version) = api_version {
        if version != TOOL_API_VERSION {
            return Err(fail(ToolFailure::UnsupportedVersion { version }));
        }
    }
    let tool = match map.get("tool") {
        None => {
            return Err(fail(ToolFailure::InvalidRequest(
                "missing tool".to_string(),
            )))
        }
        Some(Value::String(name)) => name.clone(),
        Some(_) => {
            return Err(fail(ToolFailure::InvalidRequest(
                "tool must be a string".to_string(),
            )));
        }
    };
    let params = match map.get("params") {
        None | Some(Value::Null) => Value::Null,
        Some(params) => params.clone(),
    };
    Ok(Call { id, tool, params })
}

fn request_error(id: Option<RequestId>, failure: ToolFailure) -> RequestError {
    RequestError { id, failure }
}

fn optional_version(map: &Map<String, Value>) -> Result<Option<u32>, ToolFailure> {
    match map.get("apiVersion") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => {
            match number.as_u64().and_then(|value| u32::try_from(value).ok()) {
                Some(version) => Ok(Some(version)),
                None => Err(ToolFailure::InvalidRequest(
                    "apiVersion must be an integer from 0 to 4294967295".to_string(),
                )),
            }
        }
        Some(_) => Err(ToolFailure::InvalidRequest(
            "apiVersion must be an integer from 0 to 4294967295".to_string(),
        )),
    }
}

fn optional_id(map: &Map<String, Value>) -> Result<Option<RequestId>, ToolFailure> {
    match map.get("id") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(RequestId::Text(text.clone()))),
        Some(Value::Number(number)) => Ok(Some(RequestId::Number(number.clone()))),
        Some(_) => Err(ToolFailure::InvalidRequest(
            "id must be a string or number".to_string(),
        )),
    }
}

fn dispatch(document: &mut Document, call: &Call) -> Result<Value, ToolFailure> {
    let tool = call.tool.as_str();
    let mut params = call.params.clone();
    match tool {
        "inspect" => {
            parse_empty(tool, params)?;
            Ok(inspect_value(document))
        }
        "scene" => {
            parse_empty(tool, params)?;
            Ok(scene_value(document))
        }
        "paint" => {
            let report = take_report_dirty(tool, &mut params)?;
            let (x, y, color_index, layer) = parse_paint(params)?;
            let written = document
                .resolve_paint_index(color_index)
                .map_err(ToolFailure::Kernel)?;
            let before = snaps_if(document, report, &[document.active_frame()], layer)?;
            match layer {
                Some(layer) => document.paint_on(layer, x, y, color_index),
                None => document.paint(x, y, color_index),
            }
            .map_err(ToolFailure::Kernel)?;
            Ok(with_dirty(
                clamp_note(color_index, written),
                document,
                before,
                false,
            ))
        }
        "fillRect" => {
            let report = take_report_dirty(tool, &mut params)?;
            let (x, y, width, height, color_index, layer) = parse_fill_rect(params)?;
            let written = document
                .resolve_paint_index(color_index)
                .map_err(ToolFailure::Kernel)?;
            let before = snaps_if(document, report, &[document.active_frame()], layer)?;
            match layer {
                Some(layer) => document.fill_rect_on(layer, x, y, width, height, color_index),
                None => document.fill_rect(x, y, width, height, color_index),
            }
            .map_err(ToolFailure::Kernel)?;
            Ok(with_dirty(
                clamp_note(color_index, written),
                document,
                before,
                false,
            ))
        }
        "erase" => {
            let report = take_report_dirty(tool, &mut params)?;
            let (x, y, layer) = parse_cell(tool, params)?;
            let before = snaps_if(document, report, &[document.active_frame()], layer)?;
            match layer {
                Some(layer) => document.erase_on(layer, x, y),
                None => document.erase(x, y),
            }
            .map_err(ToolFailure::Kernel)?;
            Ok(with_dirty(empty(), document, before, false))
        }
        "eraseRect" => {
            let report = take_report_dirty(tool, &mut params)?;
            let (x, y, width, height, layer) = parse_rect(tool, params)?;
            let before = snaps_if(document, report, &[document.active_frame()], layer)?;
            match layer {
                Some(layer) => document.erase_rect_on(layer, x, y, width, height),
                None => document.erase_rect(x, y, width, height),
            }
            .map_err(ToolFailure::Kernel)?;
            Ok(with_dirty(empty(), document, before, false))
        }
        "copyRect" => {
            let report = take_report_dirty(tool, &mut params)?;
            let blit = parse_blit(tool, params)?;
            let (src_frame, dst_frame) = blit_frames(document, &blit);
            let before = snaps_if(document, report, &[src_frame, dst_frame], blit.layer)?;
            match blit.layer {
                Some(layer) => document.copy_rect_from_on(
                    layer,
                    src_frame,
                    blit.src_x,
                    blit.src_y,
                    blit.width,
                    blit.height,
                    dst_frame,
                    blit.dst_x,
                    blit.dst_y,
                ),
                None => document.copy_rect_from(
                    src_frame,
                    blit.src_x,
                    blit.src_y,
                    blit.width,
                    blit.height,
                    dst_frame,
                    blit.dst_x,
                    blit.dst_y,
                ),
            }
            .map_err(ToolFailure::Kernel)?;
            Ok(with_dirty(empty(), document, before, false))
        }
        "moveRect" => {
            let report = take_report_dirty(tool, &mut params)?;
            let blit = parse_blit(tool, params)?;
            let (src_frame, dst_frame) = blit_frames(document, &blit);
            let before = snaps_if(document, report, &[src_frame, dst_frame], blit.layer)?;
            match blit.layer {
                Some(layer) => document.move_rect_from_on(
                    layer,
                    src_frame,
                    blit.src_x,
                    blit.src_y,
                    blit.width,
                    blit.height,
                    dst_frame,
                    blit.dst_x,
                    blit.dst_y,
                ),
                None => document.move_rect_from(
                    src_frame,
                    blit.src_x,
                    blit.src_y,
                    blit.width,
                    blit.height,
                    dst_frame,
                    blit.dst_x,
                    blit.dst_y,
                ),
            }
            .map_err(ToolFailure::Kernel)?;
            Ok(with_dirty(empty(), document, before, false))
        }
        "setSelection" => {
            let (x, y, width, height) = parse_selection(params)?;
            document
                .set_selection(x, y, width, height)
                .map_err(ToolFailure::Kernel)?;
            Ok(selection_value(document))
        }
        "clearSelection" => {
            parse_empty(tool, params)?;
            document.clear_selection();
            Ok(selection_value(document))
        }
        "combineMask" => {
            let (op, base, operand) = parse_combine_mask(params)?;
            document
                .combine_mask(op, base, operand)
                .map_err(ToolFailure::Kernel)?;
            Ok(combine_value(document, op))
        }
        "replaceIndex" => {
            let (from_index, to_index, layer) = parse_replace(params)?;
            let written = if to_index == crate::TRANSPARENT_INDEX {
                crate::TRANSPARENT_INDEX
            } else {
                document
                    .resolve_paint_index(to_index)
                    .map_err(ToolFailure::Kernel)?
            };
            let changed = match layer {
                Some(layer) => document.replace_index_on(layer, from_index, to_index),
                None => document.replace_index(from_index, to_index),
            }
            .map_err(ToolFailure::Kernel)?;
            Ok(replace_result(from_index, to_index, written, changed))
        }
        "setPaletteColor" => {
            let (index, r, g, b) = parse_set_palette_color(params)?;
            document
                .set_palette_color(index, r, g, b)
                .map_err(ToolFailure::Kernel)?;
            Ok(empty())
        }
        "addPaletteColor" => {
            let (r, g, b) = parse_rgb(tool, params)?;
            let index = document
                .add_palette_color(r, g, b)
                .map_err(ToolFailure::Kernel)?;
            Ok(index_value(u32::from(index)))
        }
        "importPalette" => {
            let (text, mode) = parse_import(params)?;
            document
                .import_palette(&text, mode)
                .map_err(ToolFailure::Kernel)?;
            Ok(empty())
        }
        "setPaletteLocked" => {
            let locked = parse_locked(params)?;
            document.set_palette_locked(locked);
            Ok(empty())
        }
        "setIndexPolicy" => {
            let policy = parse_index_policy(params)?;
            document.set_index_policy(policy);
            Ok(serde_json::json!({
                "mode": document.index_policy().as_str(),
            }))
        }
        "remapPalette" => {
            let request = parse_remap(params)?;
            let report = match request {
                RemapRequest::Text(text) => document.remap_palette_text(&text),
                RemapRequest::MaxColors(max_colors) => document.quantize_palette(max_colors),
            }
            .map_err(ToolFailure::Kernel)?;
            let mut result = conflict_json(&report);
            result["paletteLength"] = serde_json::json!(document.palette_colors().len());
            Ok(result)
        }
        "applyPng" => {
            let (png, frame, layer) = parse_apply_png(params)?;
            let frame = frame.unwrap_or(document.active_frame());
            let layer = layer.unwrap_or(document.active_layer());
            let report = document
                .apply_png(frame, layer, &png)
                .map_err(ToolFailure::Kernel)?;
            let mut result = conflict_json(&report);
            result["frame"] = serde_json::json!(frame);
            result["layer"] = serde_json::json!(layer);
            result["width"] = serde_json::json!(document.width());
            result["height"] = serde_json::json!(document.height());
            Ok(result)
        }
        "setActiveFrame" => {
            let index = parse_index(tool, params)?;
            document
                .set_active_frame(index)
                .map_err(ToolFailure::Kernel)?;
            Ok(empty())
        }
        "addFrame" => {
            parse_empty(tool, params)?;
            Ok(index_value(as_u32(document.add_frame())))
        }
        "deleteFrame" => {
            let index = parse_index(tool, params)?;
            document.delete_frame(index).map_err(ToolFailure::Kernel)?;
            Ok(empty())
        }
        "duplicateFrame" => {
            let index = parse_index(tool, params)?;
            let inserted = document
                .duplicate_frame(index)
                .map_err(ToolFailure::Kernel)?;
            Ok(index_value(as_u32(inserted)))
        }
        "setFrameDelay" => {
            let (index, delay_ms) = parse_delay(params)?;
            document
                .set_frame_delay(index, delay_ms)
                .map_err(ToolFailure::Kernel)?;
            Ok(empty())
        }
        "addTag" => {
            let (name, from, to, loop_mode) = parse_add_tag(params)?;
            let index = document
                .add_tag(&name, from, to, loop_mode)
                .map_err(ToolFailure::Kernel)?;
            Ok(tag_value(document, index))
        }
        "setTag" => {
            let (name, update) = parse_set_tag(params)?;
            let index = document
                .set_tag(&name, update)
                .map_err(ToolFailure::Kernel)?;
            Ok(tag_value(document, index))
        }
        "removeTag" => {
            let name = parse_remove_tag(params)?;
            let index = document.remove_tag(&name).map_err(ToolFailure::Kernel)?;
            Ok(serde_json::json!({
                "index": as_u32(index),
                "name": name.trim(),
            }))
        }
        "listParts" => {
            parse_empty(tool, params)?;
            Ok(parts_value(document))
        }
        "addPart" => {
            let (name, x, y, layer) = parse_add_part(params)?;
            let index = document
                .add_part(&name, x, y, layer)
                .map_err(ToolFailure::Kernel)?;
            Ok(part_value(document, index))
        }
        "setPart" => {
            let (name, update) = parse_set_part(params)?;
            let index = document
                .set_part(&name, update)
                .map_err(ToolFailure::Kernel)?;
            Ok(part_value(document, index))
        }
        "removePart" => {
            let name = parse_remove_part(params)?;
            let index = document.remove_part(&name).map_err(ToolFailure::Kernel)?;
            Ok(serde_json::json!({
                "index": as_u32(index),
                "name": name.trim(),
            }))
        }
        "setAnchor" => {
            let (part, name, x, y) = parse_set_anchor(params)?;
            let (part_index, anchor_index) = document
                .set_anchor(&part, &name, x, y)
                .map_err(ToolFailure::Kernel)?;
            Ok(anchor_value(document, part_index, anchor_index))
        }
        "removeAnchor" => {
            let (part, name) = parse_remove_anchor(params)?;
            let (part_index, anchor_index) = document
                .remove_anchor(&part, &name)
                .map_err(ToolFailure::Kernel)?;
            Ok(serde_json::json!({
                "part": part.trim(),
                "partIndex": as_u32(part_index),
                "index": as_u32(anchor_index),
                "name": name.trim(),
            }))
        }
        "translatePart" => {
            let report = take_report_dirty(tool, &mut params)?;
            let (name, dx, dy, frame) = parse_translate_part(params)?;
            let frame = frame.unwrap_or(document.active_frame());
            let before = part_dirty_snap(document, report, &name, frame)?;
            let pose = document
                .translate_part(&name, dx, dy, frame)
                .map_err(ToolFailure::Kernel)?;
            Ok(with_dirty(
                part_pose_value(document, &pose),
                document,
                before,
                false,
            ))
        }
        "rotatePart" => {
            let report = take_report_dirty(tool, &mut params)?;
            let (name, degrees, anchor, frame) = parse_rotate_part(params)?;
            let frame = frame.unwrap_or(document.active_frame());
            let before = part_dirty_snap(document, report, &name, frame)?;
            let pose = document
                .rotate_part(&name, degrees, anchor.as_deref(), frame)
                .map_err(ToolFailure::Kernel)?;
            Ok(with_dirty(
                part_pose_value(document, &pose),
                document,
                before,
                false,
            ))
        }
        "flipPart" => {
            let report = take_report_dirty(tool, &mut params)?;
            let (name, vertical, anchor, frame) = parse_flip_part(params)?;
            let frame = frame.unwrap_or(document.active_frame());
            let before = part_dirty_snap(document, report, &name, frame)?;
            let pose = document
                .flip_part(&name, vertical, anchor.as_deref(), frame)
                .map_err(ToolFailure::Kernel)?;
            Ok(with_dirty(
                part_pose_value(document, &pose),
                document,
                before,
                false,
            ))
        }
        "setCurveKey" => {
            let (part, channel, frame, value, ease) = parse_set_curve_key(params)?;
            let written = document
                .set_curve_key(&part, channel, frame, value, ease)
                .map_err(ToolFailure::Kernel)?;
            Ok(curve_write_value(
                document,
                written.index,
                written.key_index,
            ))
        }
        "removeCurveKey" => {
            let (part, channel, frame) = parse_curve_key(params, "removeCurveKey")?;
            let removed = document
                .remove_curve_key(&part, channel, frame)
                .map_err(ToolFailure::Kernel)?;
            Ok(serde_json::json!({
                "index": as_u32(removed.index),
                "part": removed.part,
                "channel": removed.channel.as_str(),
                "frame": as_u32(removed.frame),
                "removedCurve": removed.removed_curve,
            }))
        }
        "clearCurve" => {
            let (part, channel) = parse_curve_channel(params, "clearCurve")?;
            let index = document
                .clear_curve(&part, channel)
                .map_err(ToolFailure::Kernel)?;
            Ok(serde_json::json!({
                "index": as_u32(index),
                "part": part.trim(),
                "channel": channel.as_str(),
            }))
        }
        "listCurves" => {
            let part = parse_list_curves(params)?;
            if let Some(name) = &part {
                document
                    .sample_part(name, 0.0)
                    .map_err(ToolFailure::Kernel)?;
            }
            Ok(curves_value(document, part.as_deref()))
        }
        "sampleCurve" => {
            let (part, channel, frame) = parse_sample_curve(params)?;
            let sample = document
                .sample_curve(&part, channel, frame)
                .map_err(ToolFailure::Kernel)?;
            Ok(curve_sample_value(&part.trim(), channel, frame, &sample))
        }
        "samplePart" => {
            let (part, frame) = parse_sample_part(params)?;
            let sample = document
                .sample_part(&part, frame)
                .map_err(ToolFailure::Kernel)?;
            Ok(part_sample_value(&sample))
        }
        "addConstraint" => {
            let input = parse_add_constraint(params)?;
            let index = document
                .add_constraint_full(
                    &input.name,
                    input.kind,
                    &input.part,
                    input.anchor.as_deref(),
                    input.target.as_deref(),
                    input.target_anchor.as_deref(),
                    input.x,
                    input.y,
                    &input.chain,
                    input.bend,
                    &input.contacts,
                )
                .map_err(ToolFailure::Kernel)?;
            Ok(constraint_value(document, index))
        }
        "removeConstraint" => {
            let name = parse_remove_constraint(params)?;
            let index = document
                .remove_constraint(&name)
                .map_err(ToolFailure::Kernel)?;
            Ok(serde_json::json!({
                "index": as_u32(index),
                "name": name.trim(),
            }))
        }
        "listConstraints" => {
            parse_empty(tool, params)?;
            Ok(constraints_value(document))
        }
        "solveConstraints" => {
            let (frame, apply, pixel_frame) = parse_solve_constraints(params)?;
            let report = document
                .solve_constraints(frame, apply, pixel_frame)
                .map_err(ToolFailure::Kernel)?;
            Ok(solve_value(&report))
        }
        "bake" => {
            let options = parse_bake(params)?;
            let report = document.bake(options).map_err(ToolFailure::Kernel)?;
            Ok(bake_value(&report))
        }
        "beginGesture" => {
            parse_empty(tool, params)?;
            document.begin_gesture().map_err(ToolFailure::Kernel)?;
            Ok(empty())
        }
        "endGesture" => {
            parse_empty(tool, params)?;
            document.end_gesture();
            Ok(empty())
        }
        "undo" => {
            parse_empty(tool, params)?;
            document.undo().map_err(ToolFailure::Kernel)?;
            Ok(history_value(document))
        }
        "redo" => {
            parse_empty(tool, params)?;
            document.redo().map_err(ToolFailure::Kernel)?;
            Ok(history_value(document))
        }
        "saveProject" => {
            parse_empty(tool, params)?;
            Ok(serde_json::json!({ "json": document.save_project() }))
        }
        "loadProject" => {
            let json = parse_project_json(params)?;
            let loaded = Document::load_project(&json).map_err(ToolFailure::Kernel)?;
            *document = loaded;
            Ok(inspect_value(document))
        }
        "importAseprite" => {
            let bytes = parse_aseprite(params)?;
            let (loaded, report) = Document::from_aseprite(&bytes).map_err(ToolFailure::Kernel)?;
            *document = loaded;
            let mut result = inspect_value(document);
            result["aseprite"] = aseprite_json(&report);
            Ok(result)
        }
        "exportPngSequence" => {
            let scale = parse_scale(tool, params)?;
            let (width, height) = document
                .scaled_frame_size(scale)
                .map_err(ToolFailure::Kernel)?;
            let frames = document
                .export_png_sequence_scaled(scale)
                .map_err(ToolFailure::Kernel)?
                .into_iter()
                .map(|png| encode_base64(&png))
                .collect::<Vec<_>>();
            Ok(serde_json::json!({
                "frames": frames,
                "scale": scale,
                "width": width,
                "height": height,
            }))
        }
        "exportSpriteSheet" => {
            let (scale, columns, atlas) = parse_sheet(params)?;
            let layout = document
                .sprite_sheet_layout_with(columns)
                .map_err(ToolFailure::Kernel)?;
            let (sheet_width, sheet_height) = document
                .scaled_sheet_size_with(scale, columns)
                .map_err(ToolFailure::Kernel)?;
            let png = encode_base64(
                &document
                    .export_sprite_sheet_scaled_with(scale, columns)
                    .map_err(ToolFailure::Kernel)?,
            );
            let mut result = serde_json::json!({
                "png": png,
                "columns": layout.columns,
                "rows": layout.rows,
                "sheetWidth": sheet_width,
                "sheetHeight": sheet_height,
                "scale": scale,
            });
            if atlas {
                let atlas = document
                    .sprite_sheet_atlas(scale, columns)
                    .map_err(ToolFailure::Kernel)?;
                result["atlas"] = serde_json::to_value(atlas)
                    .expect("sprite atlas fields are JSON numbers and strings");
            }
            Ok(result)
        }
        "previewFrame" => {
            let (frame, scale, layer, scope) = parse_preview(params, document)?;
            let (png_width, png_height) = document
                .scaled_frame_size(scale)
                .map_err(ToolFailure::Kernel)?;
            let pixels = document
                .view_pixels(frame, layer)
                .map_err(ToolFailure::Kernel)?;
            let mask = scope.as_ref().map(|scope| &scope.mask);
            let summary = match mask {
                Some(mask) => scene::summarize_view_where(&pixels, document.width(), |x, y| {
                    mask.contains(x, y)
                }),
                None => scene::summarize_view(&pixels, document.width()),
            };
            let png = document
                .export_view_png_masked(frame, scale, layer, mask)
                .map_err(ToolFailure::Kernel)?;
            let mut result = serde_json::json!({
                "frame": frame,
                "activeFrame": document.active_frame(),
                "activeLayer": document.active_layer(),
                "layer": layer,
                "composite": layer.is_none(),
                "width": document.width(),
                "height": document.height(),
                "scale": scale,
                "ink": summary.ink,
                "bounds": bounds_json(summary.bounds),
                "indices": indices_json(&summary),
                "pixels": pixels,
                "pngWidth": png_width,
                "pngHeight": png_height,
                "png": encode_base64(&png),
            });
            attach_scope(
                &mut result,
                scope.as_ref(),
                document.width(),
                document.height(),
            );
            Ok(result)
        }
        "diffFrames" => {
            let (frame, with, scale, layer, scope) = parse_diff(params, document)?;
            let left = document
                .view_pixels(frame, layer)
                .map_err(ToolFailure::Kernel)?;
            let right = document
                .view_pixels(with, layer)
                .map_err(ToolFailure::Kernel)?;
            let mask = scope.as_ref().map(|scope| &scope.mask);
            let summary = match mask {
                Some(mask) => {
                    scene::summarize_diff_where(&left, &right, document.width(), |x, y| {
                        mask.contains(x, y)
                    })
                }
                None => scene::summarize_diff(&left, &right, document.width()),
            };
            let diff = document
                .diff_frames_masked(frame, with, scale, layer, mask)
                .map_err(ToolFailure::Kernel)?;
            let mut result = serde_json::json!({
                "frame": diff.frame,
                "with": diff.with,
                "activeFrame": document.active_frame(),
                "activeLayer": document.active_layer(),
                "layer": layer,
                "composite": layer.is_none(),
                "width": diff.width,
                "height": diff.height,
                "scale": diff.scale,
                "changed": diff.changed,
                "bounds": bounds_json(summary.bounds),
                "kinds": kinds_json(&summary),
                "changes": changes_json(&summary.changes),
                "panels": ["frame", "with", "onion"],
                "diffRows": diff.diff_rows,
                "pngWidth": diff.png_width,
                "pngHeight": diff.png_height,
                "png": encode_base64(&diff.png),
            });
            attach_scope(
                &mut result,
                scope.as_ref(),
                document.width(),
                document.height(),
            );
            Ok(result)
        }
        "flipHorizontal" => {
            let report = take_report_dirty(tool, &mut params)?;
            let spec = parse_flip(params)?;
            let frame = spec.frame.unwrap_or(document.active_frame());
            let before = snaps_if(document, report, &[frame], spec.layer)?;
            let (x, y, width, height) =
                spec.rect
                    .unwrap_or((0, 0, document.width(), document.height()));
            match spec.layer {
                Some(layer) => document.flip_horizontal_on(layer, frame, x, y, width, height),
                None => document.flip_horizontal(frame, x, y, width, height),
            }
            .map_err(ToolFailure::Kernel)?;
            Ok(with_dirty(empty(), document, before, false))
        }
        "flipVertical" => {
            let report = take_report_dirty(tool, &mut params)?;
            let spec = parse_mirror(tool, params)?;
            let frame = spec.frame.unwrap_or(document.active_frame());
            let before = snaps_if(document, report, &[frame], spec.layer)?;
            let (x, y, width, height) =
                spec.rect
                    .unwrap_or((0, 0, document.width(), document.height()));
            match spec.layer {
                Some(layer) => document.flip_vertical_on(layer, frame, x, y, width, height),
                None => document.flip_vertical(frame, x, y, width, height),
            }
            .map_err(ToolFailure::Kernel)?;
            Ok(with_dirty(empty(), document, before, false))
        }
        "rotate" => {
            let report = take_report_dirty(tool, &mut params)?;
            let (spec, turn) = parse_rotate(params)?;
            let frame = spec.frame.unwrap_or(document.active_frame());
            let before = snaps_if(document, report, &[frame], spec.layer)?;
            let (x, y, width, height) =
                spec.rect
                    .unwrap_or((0, 0, document.width(), document.height()));
            match spec.layer {
                Some(layer) => document.rotate_on(layer, frame, x, y, width, height, turn),
                None => document.rotate(frame, x, y, width, height, turn),
            }
            .map_err(ToolFailure::Kernel)?;
            Ok(with_dirty(empty(), document, before, false))
        }
        "resizeCanvas" => {
            let report = take_report_dirty(tool, &mut params)?;
            let (width, height, offset) = parse_resize(params)?;
            let (offset_x, offset_y) = offset.unwrap_or((
                center_offset(document.width(), width),
                center_offset(document.height(), height),
            ));
            let before = if report {
                Some(snap_document(document))
            } else {
                None
            };
            document
                .resize_canvas(width, height, offset_x, offset_y)
                .map_err(ToolFailure::Kernel)?;
            Ok(with_dirty(
                serde_json::json!({
                    "width": document.width(),
                    "height": document.height(),
                    "offsetX": offset_x,
                    "offsetY": offset_y,
                }),
                document,
                before,
                true,
            ))
        }
        "listLayers" => {
            parse_empty(tool, params)?;
            Ok(layers_value(document))
        }
        "addLayer" => {
            let (name, role) = parse_add_layer(params)?;
            let index = document
                .add_layer_with_role(name.as_deref(), role)
                .map_err(ToolFailure::Kernel)?;
            Ok(serde_json::json!({
                "index": as_u32(index),
                "name": document.layer_name(index).expect("new layer is in range"),
                "role": document.layer_role(index).expect("new layer is in range").as_str(),
                "activeLayer": as_u32(document.active_layer()),
            }))
        }
        "deleteLayer" => {
            let index = parse_optional_index(tool, params)?.unwrap_or(document.active_layer());
            document.delete_layer(index).map_err(ToolFailure::Kernel)?;
            Ok(serde_json::json!({
                "index": as_u32(index),
                "activeLayer": as_u32(document.active_layer()),
                "layerCount": as_u32(document.layer_count()),
            }))
        }
        "renameLayer" => {
            let (index, name) = parse_rename_layer(params, document.active_layer())?;
            document
                .rename_layer(index, &name)
                .map_err(ToolFailure::Kernel)?;
            Ok(serde_json::json!({
                "index": as_u32(index),
                "name": document.layer_name(index).expect("renamed layer is in range"),
            }))
        }
        "setLayerRole" => {
            let (index, role) = parse_set_layer_role(params, document.active_layer())?;
            document
                .set_layer_role(index, role)
                .map_err(ToolFailure::Kernel)?;
            Ok(serde_json::json!({
                "index": as_u32(index),
                "role": document.layer_role(index).expect("layer role is in range").as_str(),
            }))
        }
        "setActiveLayer" => {
            let index = parse_index(tool, params)?;
            document
                .set_active_layer(index)
                .map_err(ToolFailure::Kernel)?;
            Ok(serde_json::json!({ "index": as_u32(index) }))
        }
        "reorderLayer" => {
            let (from, to) = parse_reorder(params)?;
            document
                .reorder_layer(from, to)
                .map_err(ToolFailure::Kernel)?;
            Ok(serde_json::json!({
                "from": as_u32(from),
                "to": as_u32(to),
                "activeLayer": as_u32(document.active_layer()),
            }))
        }
        "flattenLayers" => {
            parse_empty(tool, params)?;
            document.flatten().map_err(ToolFailure::Kernel)?;
            Ok(serde_json::json!({
                "layerCount": as_u32(document.layer_count()),
                "activeLayer": as_u32(document.active_layer()),
                "name": document.layer_name(0).expect("flatten leaves one layer"),
            }))
        }
        _ => Err(ToolFailure::UnknownTool {
            name: tool.to_string(),
        }),
    }
}

fn inspect_value(document: &Document) -> Value {
    let active = document.active_frame();
    let delay_ms = document
        .frame_delay(active)
        .expect("active frame is in range");
    serde_json::json!({
        "width": document.width(),
        "height": document.height(),
        "palette": palette_entries(document),
        "paletteHash": scene::palette_hash(document.palette_colors()),
        "paletteLocked": document.palette_locked(),
        "indexPolicy": document.index_policy().as_str(),
        "frameCount": document.frame_count(),
        "activeFrame": active,
        "delayMs": delay_ms,
        "frameDelays": document.frame_delays(),
        "pixels": document.pixels(),
        "composite": document.composite_pixels(active).expect("active frame is in range"),
        "layerCount": document.layer_count(),
        "activeLayer": document.active_layer(),
        "selection": selection_json(document),
        "tags": tags_json(document),
        "parts": parts_json(document),
        "curves": curves_json(document, None),
        "constraints": constraints_json(document),
        "layers": layer_entries(document),
        "canUndo": document.can_undo(),
        "canRedo": document.can_redo(),
        "gestureOpen": document.gesture_open(),
    })
}

fn scene_value(document: &Document) -> Value {
    let active = document.active_frame();
    let delay_ms = document
        .frame_delay(active)
        .expect("active frame is in range");
    let frames = document
        .frame_delays()
        .into_iter()
        .enumerate()
        .map(|(index, delay_ms)| {
            serde_json::json!({
                "index": index,
                "delayMs": delay_ms,
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "width": document.width(),
        "height": document.height(),
        "activeFrame": active,
        "activeLayer": document.active_layer(),
        "selection": selection_json(document),
        "layerCount": document.layer_count(),
        "layers": scene_layers(document),
        "paletteLocked": document.palette_locked(),
        "indexPolicy": document.index_policy().as_str(),
        "paletteCount": document.palette_colors().len(),
        "paletteHash": scene::palette_hash(document.palette_colors()),
        "palette": palette_entries(document),
        "frameCount": document.frame_count(),
        "delayMs": delay_ms,
        "frames": frames,
        "tags": tags_json(document),
        "parts": parts_json(document),
        "curves": curves_json(document, None),
        "constraints": constraints_json(document),
    })
}

fn palette_entries(document: &Document) -> Vec<Value> {
    document
        .palette_colors()
        .iter()
        .map(|color| {
            serde_json::json!({
                "r": color.r,
                "g": color.g,
                "b": color.b,
                "a": color.a,
            })
        })
        .collect()
}

fn scene_layers(document: &Document) -> Vec<Value> {
    document
        .layer_names()
        .iter()
        .enumerate()
        .map(|(index, name)| {
            serde_json::json!({
                "index": index,
                "name": name,
                "role": document.layer_role(index).expect("scene layer is in range").as_str(),
                "visible": true,
                "opacity": Value::Null,
            })
        })
        .collect()
}

fn layers_value(document: &Document) -> Value {
    serde_json::json!({
        "layerCount": document.layer_count(),
        "activeLayer": document.active_layer(),
        "layers": layer_entries(document),
    })
}

fn layer_entries(document: &Document) -> Vec<Value> {
    document
        .layer_names()
        .iter()
        .enumerate()
        .map(|(index, name)| {
            serde_json::json!({
                "index": index,
                "name": name,
                "role": document.layer_role(index).expect("listed layer is in range").as_str(),
            })
        })
        .collect()
}

fn tags_json(document: &Document) -> Vec<Value> {
    document
        .tags()
        .iter()
        .enumerate()
        .map(|(index, tag)| tag_fields(index, &tag.name, tag.from, tag.to, tag.loop_mode))
        .collect()
}

fn tag_value(document: &Document, index: usize) -> Value {
    let tag = &document.tags()[index];
    tag_fields(index, &tag.name, tag.from, tag.to, tag.loop_mode)
}

fn parts_value(document: &Document) -> Value {
    serde_json::json!({ "parts": parts_json(document) })
}

fn parts_json(document: &Document) -> Vec<Value> {
    document
        .parts()
        .iter()
        .enumerate()
        .map(|(index, part)| part_fields(index, part))
        .collect()
}

fn part_value(document: &Document, index: usize) -> Value {
    part_fields(index, &document.parts()[index])
}

fn curves_value(document: &Document, part: Option<&str>) -> Value {
    serde_json::json!({ "curves": curves_json(document, part) })
}

fn curves_json(document: &Document, part: Option<&str>) -> Vec<Value> {
    let part = part.map(str::trim);
    document
        .curves()
        .iter()
        .enumerate()
        .filter(|(_, curve)| part.is_none_or(|name| curve.part == name))
        .map(|(index, curve)| curve_fields(index, curve))
        .collect()
}

fn constraints_value(document: &Document) -> Value {
    serde_json::json!({ "constraints": constraints_json(document) })
}

fn constraints_json(document: &Document) -> Vec<Value> {
    document
        .constraints()
        .iter()
        .enumerate()
        .map(|(index, constraint)| constraint_fields(index, constraint))
        .collect()
}

fn constraint_value(document: &Document, index: usize) -> Value {
    constraint_fields(index, &document.constraints()[index])
}

fn constraint_fields(index: usize, constraint: &Constraint) -> Value {
    serde_json::json!({
        "index": as_u32(index),
        "name": constraint.name,
        "kind": constraint.kind.as_str(),
        "part": constraint.part,
        "anchor": constraint.anchor,
        "target": constraint.target,
        "targetAnchor": constraint.target_anchor,
        "x": constraint.x,
        "y": constraint.y,
        "chain": constraint.chain,
        "bend": constraint.bend,
        "contacts": constraint.contacts.iter().map(contact_fields).collect::<Vec<_>>(),
    })
}

fn contact_fields(contact: &Contact) -> Value {
    serde_json::json!({
        "frame": contact.frame,
        "to": contact.to,
        "x": contact.x,
        "y": contact.y,
    })
}

fn bake_value(report: &crate::document::BakeReport) -> Value {
    serde_json::json!({
        "from": as_u32(report.from),
        "to": as_u32(report.to),
        "source": as_u32(report.source),
        "fps": match report.fps {
            None => Value::Null,
            Some(fps) => json_finite(fps),
        },
        "delayMs": report.delay_ms,
        "constraints": report.constraints,
        "appended": as_u32(report.appended),
        "wrote": report.wrote,
        "dropped": report.dropped,
        "conflicts": report.conflicts,
        "sealed": report.sealed,
        "breaks": report.breaks,
        "polish": report.polish.iter().copied().map(as_u32).collect::<Vec<_>>(),
        "frames": report
            .frames
            .iter()
            .map(|frame| {
                serde_json::json!({
                    "index": as_u32(frame.index),
                    "rest": frame.rest,
                    "delayMs": frame.delay_ms,
                    "dropped": frame.dropped,
                    "conflicts": frame.conflicts,
                    "sealed": frame.sealed,
                    "breaks": frame.breaks,
                    "layers": frame.layers.iter().copied().map(as_u32).collect::<Vec<_>>(),
                    "parts": frame.parts.iter().map(solved_part_value).collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>(),
    })
}

fn solved_part_value(part: &crate::constraint::SolvedPart) -> Value {
    serde_json::json!({
        "name": part.name,
        "x": part.x,
        "y": part.y,
        "layer": part.layer.map(as_u32),
        "ink": part.ink,
        "dropped": part.dropped,
        "conflicts": part.conflicts,
        "sealed": part.sealed,
        "breaks": part.breaks,
        "anchors": part
            .anchors
            .iter()
            .map(|anchor| {
                serde_json::json!({
                    "name": anchor.name,
                    "x": anchor.x,
                    "y": anchor.y,
                    "canvasX": anchor.canvas_x,
                    "canvasY": anchor.canvas_y,
                })
            })
            .collect::<Vec<_>>(),
    })
}

fn solve_value(report: &SolveReport) -> Value {
    serde_json::json!({
        "frame": match report.frame {
            None => Value::Null,
            Some(frame) => json_finite(frame),
        },
        "pixelFrame": as_u32(report.pixel_frame),
        "apply": report.apply,
        "wrote": report.wrote,
        "dropped": report.dropped,
        "conflicts": report.conflicts,
        "sealed": report.sealed,
        "breaks": report.breaks,
        "order": report.order,
        "constraints": report
            .constraints
            .iter()
            .map(|constraint| {
                serde_json::json!({
                    "index": as_u32(constraint.index),
                    "name": constraint.name,
                    "kind": constraint.kind.as_str(),
                    "part": constraint.part,
                    "anchor": constraint.anchor,
                    "target": constraint.target,
                    "targetAnchor": constraint.target_anchor,
                    "x": constraint.x,
                    "y": constraint.y,
                    "dx": constraint.dx,
                    "dy": constraint.dy,
                    "satisfied": constraint.satisfied,
                    "planted": constraint.planted,
                    "from": { "x": constraint.from_x, "y": constraint.from_y },
                    "to": { "x": constraint.to_x, "y": constraint.to_y },
                })
            })
            .collect::<Vec<_>>(),
        "parts": report
            .parts
            .iter()
            .map(|part| {
                serde_json::json!({
                    "name": part.name,
                    "x": part.x,
                    "y": part.y,
                    "layer": part.layer.map(as_u32),
                    "ink": part.ink,
                    "dropped": part.dropped,
                    "conflicts": part.conflicts,
                    "sealed": part.sealed,
                    "breaks": part.breaks,
                    "anchors": part
                        .anchors
                        .iter()
                        .map(|anchor| {
                            serde_json::json!({
                                "name": anchor.name,
                                "x": anchor.x,
                                "y": anchor.y,
                                "canvasX": anchor.canvas_x,
                                "canvasY": anchor.canvas_y,
                            })
                        })
                        .collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>(),
    })
}

fn curve_write_value(document: &Document, index: usize, key_index: usize) -> Value {
    let curve = &document.curves()[index];
    let key = &curve.keys[key_index];
    serde_json::json!({
        "index": as_u32(index),
        "keyIndex": as_u32(key_index),
        "part": curve.part,
        "channel": curve.channel.as_str(),
        "frame": as_u32(key.frame),
        "value": json_finite(curve.channel.decode(key.value)),
        "ease": key.ease.as_str(),
    })
}

fn curve_fields(index: usize, curve: &crate::curve::Curve) -> Value {
    serde_json::json!({
        "index": as_u32(index),
        "part": curve.part,
        "channel": curve.channel.as_str(),
        "keys": curve
            .keys
            .iter()
            .map(|key| {
                serde_json::json!({
                    "frame": as_u32(key.frame),
                    "value": json_finite(curve.channel.decode(key.value)),
                    "ease": key.ease.as_str(),
                })
            })
            .collect::<Vec<_>>(),
    })
}

fn curve_sample_value(
    part: &str,
    channel: CurveChannel,
    frame: f64,
    sample: &CurveSample,
) -> Value {
    serde_json::json!({
        "part": part,
        "channel": channel.as_str(),
        "frame": json_finite(frame),
        "value": json_finite(sample.value),
        "interpolation": sample.hit.as_str(),
        "leftFrame": as_u32(sample.left_frame),
        "rightFrame": sample.right_frame.map(as_u32),
    })
}

fn part_sample_value(sample: &PartSample) -> Value {
    let mut map = Map::new();
    map.insert("part".to_string(), Value::String(sample.part.clone()));
    map.insert("frame".to_string(), json_finite(sample.frame));
    if let Some(value) = sample.rotation {
        map.insert("rotation".to_string(), json_finite(value));
    }
    if let Some(value) = sample.translate_x {
        map.insert("translateX".to_string(), json_finite(value));
    }
    if let Some(value) = sample.translate_y {
        map.insert("translateY".to_string(), json_finite(value));
    }
    if let Some(value) = sample.flip_horizontal {
        map.insert("flipHorizontal".to_string(), json_finite(value));
    }
    if let Some(value) = sample.flip_vertical {
        map.insert("flipVertical".to_string(), json_finite(value));
    }
    Value::Object(map)
}

fn json_finite(value: f64) -> Value {
    if value.is_finite() && value.fract() == 0.0 && value.abs() <= i64::MAX as f64 {
        Value::Number(Number::from(value as i64))
    } else {
        Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

fn part_fields(index: usize, part: &crate::part::Part) -> Value {
    serde_json::json!({
        "index": as_u32(index),
        "name": part.name,
        "x": part.x,
        "y": part.y,
        "layer": part.layer.map(as_u32),
        "anchors": part
            .anchors
            .iter()
            .enumerate()
            .map(|(anchor_index, anchor)| {
                serde_json::json!({
                    "index": as_u32(anchor_index),
                    "name": anchor.name,
                    "x": anchor.x,
                    "y": anchor.y,
                })
            })
            .collect::<Vec<_>>(),
    })
}

fn anchor_value(document: &Document, part_index: usize, anchor_index: usize) -> Value {
    let part = &document.parts()[part_index];
    let anchor = &part.anchors[anchor_index];
    serde_json::json!({
        "part": part.name,
        "partIndex": as_u32(part_index),
        "index": as_u32(anchor_index),
        "name": anchor.name,
        "x": anchor.x,
        "y": anchor.y,
    })
}

fn tag_fields(index: usize, name: &str, from: usize, to: usize, loop_mode: LoopMode) -> Value {
    serde_json::json!({
        "index": as_u32(index),
        "name": name,
        "from": as_u32(from),
        "to": as_u32(to),
        "loop": loop_mode.as_str(),
    })
}

fn selection_json(document: &Document) -> Value {
    match document.selection() {
        None => Value::Null,
        Some(mask) => mask_json(mask),
    }
}

fn mask_json(mask: &Mask) -> Value {
    if let Some(selection) = mask.as_rect() {
        serde_json::json!({
            "x": selection.x,
            "y": selection.y,
            "width": selection.width,
            "height": selection.height,
        })
    } else {
        serde_json::json!({
            "rects": mask.rects().iter().map(|selection| serde_json::json!({
                "x": selection.x,
                "y": selection.y,
                "width": selection.width,
                "height": selection.height,
            })).collect::<Vec<_>>(),
        })
    }
}

fn combine_value(document: &Document, op: MaskOp) -> Value {
    let mask = document
        .selection()
        .expect("combineMask stores a mask, including an empty one");
    serde_json::json!({
        "op": op.as_str(),
        "selection": mask_json(mask),
        "cells": mask.cell_count(),
    })
}

struct ViewScope {
    mode: &'static str,
    mask: Mask,
}

fn attach_scope(result: &mut Value, scope: Option<&ViewScope>, width: u16, height: u16) {
    let Some(scope) = scope else {
        return;
    };
    let Some(object) = result.as_object_mut() else {
        return;
    };
    let protected = u32::from(width) * u32::from(height) - scope.mask.cell_count();
    object.insert("scope".to_string(), serde_json::json!(scope.mode));
    object.insert("mask".to_string(), mask_json(&scope.mask));
    object.insert("protected".to_string(), serde_json::json!(protected));
}

fn selection_value(document: &Document) -> Value {
    serde_json::json!({ "selection": selection_json(document) })
}

/// `toIndex` is the index stored. Clamp adds `indexPolicy` and `requestedIndex`.
fn replace_result(from_index: u8, requested: u8, written: u8, changed: u32) -> Value {
    let mut result = serde_json::json!({
        "fromIndex": from_index,
        "toIndex": written,
        "changed": changed,
    });
    if requested != written {
        result["indexPolicy"] = serde_json::json!(IndexPolicy::Clamp.as_str());
        result["requestedIndex"] = serde_json::json!(requested);
    }
    result
}

fn history_value(document: &Document) -> Value {
    serde_json::json!({
        "canUndo": document.can_undo(),
        "canRedo": document.can_redo(),
    })
}

fn index_value(index: u32) -> Value {
    serde_json::json!({ "index": index })
}

fn empty() -> Value {
    Value::Object(Map::new())
}

/// Present only when clamp rewrote the requested index. Otherwise `{}`.
fn clamp_note(requested: u8, written: u8) -> Value {
    if requested == written {
        empty()
    } else {
        serde_json::json!({
            "indexPolicy": IndexPolicy::Clamp.as_str(),
            "requestedIndex": requested,
            "colorIndex": written,
        })
    }
}

fn conflict_json(report: &MapReport) -> Value {
    serde_json::json!({
        "pixelsChanged": report.pixels_changed,
        "misses": report.misses.iter().map(miss_json).collect::<Vec<_>>(),
        "merges": report.merges.iter().map(merge_json).collect::<Vec<_>>(),
        "dropped": report.dropped.iter().map(dropped_json).collect::<Vec<_>>(),
    })
}

fn miss_json(merge: &ColorMerge) -> Value {
    let mut value = serde_json::json!({
        "r": merge.from.r,
        "g": merge.from.g,
        "b": merge.from.b,
        "pixels": merge.pixels,
        "intoIndex": merge.into_index,
        "into": {
            "r": merge.into.r,
            "g": merge.into.g,
            "b": merge.into.b,
        },
        "distance": merge.distance,
    });
    if let Some(index) = merge.from_index {
        value["fromIndex"] = serde_json::json!(index);
    }
    value
}

fn merge_json(merge: &ColorMerge) -> Value {
    let mut value = serde_json::json!({
        "from": {
            "r": merge.from.r,
            "g": merge.from.g,
            "b": merge.from.b,
        },
        "intoIndex": merge.into_index,
        "into": {
            "r": merge.into.r,
            "g": merge.into.g,
            "b": merge.into.b,
        },
        "pixels": merge.pixels,
        "distance": merge.distance,
        "exact": merge.exact,
    });
    if let Some(index) = merge.from_index {
        value["fromIndex"] = serde_json::json!(index);
    }
    value
}

fn dropped_json(color: &DroppedColor) -> Value {
    serde_json::json!({
        "index": color.index,
        "r": color.color.r,
        "g": color.color.g,
        "b": color.color.b,
    })
}

struct LayerSnap {
    frame: usize,
    layer: usize,
    width: u16,
    height: u16,
    pixels: Vec<u8>,
}

fn take_report_dirty(tool: &str, params: &mut Value) -> Result<bool, ToolFailure> {
    let Some(map) = params.as_object_mut() else {
        return Ok(false);
    };
    match map.remove("reportDirty") {
        None => Ok(false),
        Some(Value::Bool(value)) => Ok(value),
        Some(_) => Err(invalid_params(tool, "reportDirty must be a boolean")),
    }
}

/// `None` when the caller did not ask, or when the frame or layer is missing.
///
/// A missing frame or layer is left for the mutation to refuse, so a bad color
/// index is still reported before a bad layer.
fn snaps_if(
    document: &Document,
    report: bool,
    frames: &[usize],
    layer: Option<usize>,
) -> Result<Option<Vec<LayerSnap>>, ToolFailure> {
    if !report {
        return Ok(None);
    }
    let layer = layer.unwrap_or(document.active_layer());
    if document.layer_name(layer).is_err() {
        return Ok(None);
    }
    let mut snaps = Vec::new();
    let mut seen = Vec::new();
    for frame in frames {
        if seen.contains(frame) {
            continue;
        }
        seen.push(*frame);
        if document.frame_delay(*frame).is_err() {
            return Ok(None);
        }
        snaps.push(LayerSnap {
            frame: *frame,
            layer,
            width: document.width(),
            height: document.height(),
            pixels: document
                .layer_pixels(*frame, layer)
                .expect("frame and layer were checked")
                .to_vec(),
        });
    }
    Ok(Some(snaps))
}

fn snap_document(document: &Document) -> Vec<LayerSnap> {
    let mut snaps = Vec::new();
    for frame in 0..document.frame_count() {
        for layer in 0..document.layer_count() {
            snaps.push(LayerSnap {
                frame,
                layer,
                width: document.width(),
                height: document.height(),
                pixels: document
                    .layer_pixels(frame, layer)
                    .expect("frame and layer come from the document")
                    .to_vec(),
            });
        }
    }
    snaps
}

fn with_dirty(
    mut result: Value,
    document: &Document,
    before: Option<Vec<LayerSnap>>,
    include_clipped: bool,
) -> Value {
    let Some(before) = before else {
        return result;
    };
    let Some(dest) = result.as_object_mut() else {
        return result;
    };
    let regions = before
        .iter()
        .map(|snap| {
            let after = document
                .layer_pixels(snap.frame, snap.layer)
                .expect("a dirty report follows a successful edit");
            scene::dirty_region(
                snap.frame,
                snap.layer,
                &snap.pixels,
                snap.width,
                snap.height,
                after,
                document.width(),
                document.height(),
            )
        })
        .collect::<Vec<_>>();
    let changed: usize = regions.iter().map(|region| region.changed).sum();
    let clipped: usize = regions.iter().map(|region| region.clipped).sum();
    let dirty = scene::union_bounds(regions.iter().filter_map(|region| region.bounds));
    dest.insert("changed".to_string(), serde_json::json!(changed));
    dest.insert("dirty".to_string(), bounds_json(dirty));
    dest.insert(
        "regions".to_string(),
        serde_json::json!(region_entries(&regions)),
    );
    if include_clipped {
        dest.insert("clipped".to_string(), serde_json::json!(clipped));
    }
    result
}

fn region_entries(regions: &[DirtyRegion]) -> Vec<Value> {
    regions
        .iter()
        .filter(|region| region.changed > 0)
        .map(|region| {
            let bounds = region.bounds.expect("changed cells have a bbox");
            serde_json::json!({
                "frame": as_u32(region.frame),
                "layer": as_u32(region.layer),
                "changed": region.changed,
                "x": bounds.x,
                "y": bounds.y,
                "width": bounds.width,
                "height": bounds.height,
            })
        })
        .collect()
}

fn bounds_json(bounds: Option<CellBounds>) -> Value {
    match bounds {
        None => Value::Null,
        Some(bounds) => serde_json::json!({
            "x": bounds.x,
            "y": bounds.y,
            "width": bounds.width,
            "height": bounds.height,
        }),
    }
}

fn indices_json(summary: &ViewSummary) -> Vec<Value> {
    summary
        .indices
        .iter()
        .map(|(index, count)| {
            serde_json::json!({
                "index": index,
                "count": count,
            })
        })
        .collect()
}

fn kinds_json(summary: &DiffSummary) -> Value {
    serde_json::json!({
        "same": summary.same,
        "differ": summary.differ,
        "onlyFrame": summary.only_frame,
        "onlyWith": summary.only_with,
    })
}

fn changes_json(changes: &[CellChange]) -> Vec<Value> {
    changes
        .iter()
        .map(|change| {
            serde_json::json!({
                "x": change.x,
                "y": change.y,
                "left": change.left,
                "right": change.right,
            })
        })
        .collect()
}

fn as_u32(index: usize) -> u32 {
    u32::try_from(index).expect("frame index fits in u32")
}

fn parse_empty(tool: &str, params: Value) -> Result<(), ToolFailure> {
    let map = expect_object(tool, params)?;
    reject_params(tool, &map, &[])?;
    Ok(())
}

fn parse_paint(params: Value) -> Result<(u16, u16, u8, Option<usize>), ToolFailure> {
    let map = expect_object("paint", params)?;
    reject_params("paint", &map, &["x", "y", "colorIndex", "layer"])?;
    let (x, y) = cell_from("paint", &map)?;
    let color_index = require_u8("paint", &map, "colorIndex")?;
    let layer = optional_frame("paint", &map, "layer")?;
    Ok((x, y, color_index, layer))
}

fn parse_fill_rect(params: Value) -> Result<(u16, u16, u16, u16, u8, Option<usize>), ToolFailure> {
    let map = expect_object("fillRect", params)?;
    reject_params(
        "fillRect",
        &map,
        &["x", "y", "width", "height", "colorIndex", "layer"],
    )?;
    let (x, y, width, height) = rect_from("fillRect", &map)?;
    let color_index = require_u8("fillRect", &map, "colorIndex")?;
    let layer = optional_frame("fillRect", &map, "layer")?;
    Ok((x, y, width, height, color_index, layer))
}

fn parse_selection(params: Value) -> Result<(u16, u16, u16, u16), ToolFailure> {
    let map = expect_object("setSelection", params)?;
    reject_params("setSelection", &map, &["x", "y", "width", "height"])?;
    rect_from("setSelection", &map)
}

fn parse_replace(params: Value) -> Result<(u8, u8, Option<usize>), ToolFailure> {
    let map = expect_object("replaceIndex", params)?;
    reject_params("replaceIndex", &map, &["fromIndex", "toIndex", "layer"])?;
    let from_index = require_u8("replaceIndex", &map, "fromIndex")?;
    let to_index = require_u8("replaceIndex", &map, "toIndex")?;
    let layer = optional_frame("replaceIndex", &map, "layer")?;
    Ok((from_index, to_index, layer))
}

fn parse_rect(
    tool: &str,
    params: Value,
) -> Result<(u16, u16, u16, u16, Option<usize>), ToolFailure> {
    let map = expect_object(tool, params)?;
    reject_params(tool, &map, &["x", "y", "width", "height", "layer"])?;
    let rect = rect_from(tool, &map)?;
    let layer = optional_frame(tool, &map, "layer")?;
    Ok((rect.0, rect.1, rect.2, rect.3, layer))
}

fn rect_from(tool: &str, map: &Map<String, Value>) -> Result<(u16, u16, u16, u16), ToolFailure> {
    let x = require_u16(tool, map, "x")?;
    let y = require_u16(tool, map, "y")?;
    let width = require_u16_at_least(tool, map, "width", 1)?;
    let height = require_u16_at_least(tool, map, "height", 1)?;
    Ok((x, y, width, height))
}

struct Blit {
    src_x: u16,
    src_y: u16,
    width: u16,
    height: u16,
    dst_x: u16,
    dst_y: u16,
    from_frame: Option<usize>,
    to_frame: Option<usize>,
    layer: Option<usize>,
}

fn parse_blit(tool: &str, params: Value) -> Result<Blit, ToolFailure> {
    let map = expect_object(tool, params)?;
    reject_params(
        tool,
        &map,
        &[
            "srcX",
            "srcY",
            "width",
            "height",
            "dstX",
            "dstY",
            "fromFrame",
            "toFrame",
            "layer",
        ],
    )?;
    let src_x = require_u16(tool, &map, "srcX")?;
    let src_y = require_u16(tool, &map, "srcY")?;
    let width = require_u16_at_least(tool, &map, "width", 1)?;
    let height = require_u16_at_least(tool, &map, "height", 1)?;
    let dst_x = require_u16(tool, &map, "dstX")?;
    let dst_y = require_u16(tool, &map, "dstY")?;
    let from_frame = optional_frame(tool, &map, "fromFrame")?;
    let to_frame = optional_frame(tool, &map, "toFrame")?;
    let layer = optional_frame(tool, &map, "layer")?;
    Ok(Blit {
        src_x,
        src_y,
        width,
        height,
        dst_x,
        dst_y,
        from_frame,
        to_frame,
        layer,
    })
}

fn blit_frames(document: &Document, blit: &Blit) -> (usize, usize) {
    let active = document.active_frame();
    (
        blit.from_frame.unwrap_or(active),
        blit.to_frame.unwrap_or(active),
    )
}

fn parse_sheet(params: Value) -> Result<(u32, Option<u32>, bool), ToolFailure> {
    let map = expect_object("exportSpriteSheet", params)?;
    reject_params("exportSpriteSheet", &map, &["scale", "columns", "atlas"])?;
    let scale = match map.get("scale") {
        None => 1,
        Some(_) => require_u32_at_least("exportSpriteSheet", &map, "scale", 1)?,
    };
    let columns = match map.get("columns") {
        None => None,
        Some(_) => Some(require_u32_at_least(
            "exportSpriteSheet",
            &map,
            "columns",
            1,
        )?),
    };
    let atlas = match map.get("atlas") {
        None => false,
        Some(_) => require_bool("exportSpriteSheet", &map, "atlas")?,
    };
    Ok((scale, columns, atlas))
}

fn parse_add_part(params: Value) -> Result<(String, u16, u16, Option<usize>), ToolFailure> {
    let map = expect_object("addPart", params)?;
    reject_params("addPart", &map, &["name", "x", "y", "layer"])?;
    let name = require_string("addPart", &map, "name")?;
    let x = require_u16("addPart", &map, "x")?;
    let y = require_u16("addPart", &map, "y")?;
    let layer = optional_frame("addPart", &map, "layer")?;
    Ok((name, x, y, layer))
}

fn parse_set_part(params: Value) -> Result<(String, PartUpdate), ToolFailure> {
    let map = expect_object("setPart", params)?;
    reject_params(
        "setPart",
        &map,
        &["name", "x", "y", "layer", "clearLayer", "rename"],
    )?;
    let name = require_string("setPart", &map, "name")?;
    let x = optional_u16("setPart", &map, "x")?;
    let y = optional_u16("setPart", &map, "y")?;
    let layer = optional_frame("setPart", &map, "layer")?;
    let clear_layer = match map.get("clearLayer") {
        None => false,
        Some(_) => require_bool("setPart", &map, "clearLayer")?,
    };
    if clear_layer && layer.is_some() {
        return Err(invalid_params(
            "setPart",
            "pass layer or clearLayer, not both",
        ));
    }
    let rename = match map.get("rename") {
        None => None,
        Some(_) => Some(require_string("setPart", &map, "rename")?),
    };
    if x.is_none() && y.is_none() && layer.is_none() && !clear_layer && rename.is_none() {
        return Err(invalid_params(
            "setPart",
            "pass x, y, layer, clearLayer, or rename",
        ));
    }
    let attach = if clear_layer {
        LayerAttach::Clear
    } else if let Some(layer) = layer {
        LayerAttach::Index(layer)
    } else {
        LayerAttach::Keep
    };
    Ok((
        name,
        PartUpdate {
            x,
            y,
            layer: attach,
            rename,
        },
    ))
}

fn parse_remove_part(params: Value) -> Result<String, ToolFailure> {
    let map = expect_object("removePart", params)?;
    reject_params("removePart", &map, &["name"])?;
    require_string("removePart", &map, "name")
}

fn parse_set_anchor(params: Value) -> Result<(String, String, i32, i32), ToolFailure> {
    let map = expect_object("setAnchor", params)?;
    reject_params("setAnchor", &map, &["part", "name", "x", "y"])?;
    let part = require_string("setAnchor", &map, "part")?;
    let name = require_string("setAnchor", &map, "name")?;
    let x = require_i32("setAnchor", &map, "x")?;
    let y = require_i32("setAnchor", &map, "y")?;
    Ok((part, name, x, y))
}

fn part_pose_value(document: &Document, pose: &crate::part_xform::PartPose) -> Value {
    let mut value = part_value(document, pose.index);
    let Some(dest) = value.as_object_mut() else {
        return value;
    };
    dest.insert("frame".to_string(), serde_json::json!(as_u32(pose.frame)));
    dest.insert(
        "pivot".to_string(),
        serde_json::json!({
            "x": pose.pivot_x,
            "y": pose.pivot_y,
            "anchor": pose.pivot_anchor,
        }),
    );
    dest.insert("alignment".to_string(), serde_json::json!(pose.alignment));
    dest.insert("sampling".to_string(), serde_json::json!(pose.sampling));
    if let Some(grid) = pose.grid {
        dest.insert("grid".to_string(), serde_json::json!(grid));
    }
    if let Some(centi) = pose.degrees_centi {
        dest.insert("degreesCenti".to_string(), serde_json::json!(centi));
    }
    if let Some(dx) = pose.dx {
        dest.insert("dx".to_string(), serde_json::json!(dx));
    }
    if let Some(dy) = pose.dy {
        dest.insert("dy".to_string(), serde_json::json!(dy));
    }
    if let Some(direction) = pose.direction {
        dest.insert("direction".to_string(), serde_json::json!(direction));
    }
    dest.insert("dropped".to_string(), serde_json::json!(pose.dropped));
    dest.insert("conflicts".to_string(), serde_json::json!(pose.conflicts));
    dest.insert("sealed".to_string(), serde_json::json!(pose.sealed));
    dest.insert("breaks".to_string(), serde_json::json!(pose.breaks));
    value
}

fn part_dirty_snap(
    document: &Document,
    report: bool,
    name: &str,
    frame: usize,
) -> Result<Option<Vec<LayerSnap>>, ToolFailure> {
    if !report {
        return Ok(None);
    }
    let Some(part) = document
        .parts()
        .iter()
        .find(|part| part.name == name.trim())
    else {
        return Ok(Some(Vec::new()));
    };
    let Some(layer) = part.layer else {
        return Ok(Some(Vec::new()));
    };
    snaps_if(document, true, &[frame], Some(layer))
}

fn parse_translate_part(params: Value) -> Result<(String, i32, i32, Option<usize>), ToolFailure> {
    let map = expect_object("translatePart", params)?;
    reject_params("translatePart", &map, &["name", "dx", "dy", "frame"])?;
    let name = require_string("translatePart", &map, "name")?;
    let dx = require_i32("translatePart", &map, "dx")?;
    let dy = require_i32("translatePart", &map, "dy")?;
    let frame = optional_frame("translatePart", &map, "frame")?;
    Ok((name, dx, dy, frame))
}

fn parse_rotate_part(
    params: Value,
) -> Result<(String, f64, Option<String>, Option<usize>), ToolFailure> {
    let map = expect_object("rotatePart", params)?;
    reject_params("rotatePart", &map, &["name", "degrees", "anchor", "frame"])?;
    let name = require_string("rotatePart", &map, "name")?;
    let degrees = require_degrees("rotatePart", &map, "degrees")?;
    let anchor = optional_string("rotatePart", &map, "anchor")?;
    let frame = optional_frame("rotatePart", &map, "frame")?;
    Ok((name, degrees, anchor, frame))
}

fn parse_flip_part(
    params: Value,
) -> Result<(String, bool, Option<String>, Option<usize>), ToolFailure> {
    let map = expect_object("flipPart", params)?;
    reject_params("flipPart", &map, &["name", "direction", "anchor", "frame"])?;
    let name = require_string("flipPart", &map, "name")?;
    let direction = require_string("flipPart", &map, "direction")?;
    let vertical = match direction.as_str() {
        "horizontal" => false,
        "vertical" => true,
        _ => {
            return Err(invalid_params(
                "flipPart",
                "direction must be horizontal or vertical",
            ))
        }
    };
    let anchor = optional_string("flipPart", &map, "anchor")?;
    let frame = optional_frame("flipPart", &map, "frame")?;
    Ok((name, vertical, anchor, frame))
}

fn parse_set_curve_key(
    params: Value,
) -> Result<(String, CurveChannel, usize, f64, Option<Ease>), ToolFailure> {
    let map = expect_object("setCurveKey", params)?;
    reject_params(
        "setCurveKey",
        &map,
        &["part", "channel", "frame", "value", "ease"],
    )?;
    let part = require_string("setCurveKey", &map, "part")?;
    let channel = parse_channel(
        "setCurveKey",
        &require_string("setCurveKey", &map, "channel")?,
    )?;
    let frame = require_frame("setCurveKey", &map, "frame")?;
    let value = require_finite("setCurveKey", &map, "value")?;
    let ease = match optional_string("setCurveKey", &map, "ease")? {
        None => None,
        Some(text) => Some(parse_ease("setCurveKey", &text)?),
    };
    Ok((part, channel, frame, value, ease))
}

fn parse_curve_key(
    params: Value,
    tool: &str,
) -> Result<(String, CurveChannel, usize), ToolFailure> {
    let map = expect_object(tool, params)?;
    reject_params(tool, &map, &["part", "channel", "frame"])?;
    let part = require_string(tool, &map, "part")?;
    let channel = parse_channel(tool, &require_string(tool, &map, "channel")?)?;
    let frame = require_frame(tool, &map, "frame")?;
    Ok((part, channel, frame))
}

fn parse_curve_channel(params: Value, tool: &str) -> Result<(String, CurveChannel), ToolFailure> {
    let map = expect_object(tool, params)?;
    reject_params(tool, &map, &["part", "channel"])?;
    let part = require_string(tool, &map, "part")?;
    let channel = parse_channel(tool, &require_string(tool, &map, "channel")?)?;
    Ok((part, channel))
}

fn parse_list_curves(params: Value) -> Result<Option<String>, ToolFailure> {
    let map = expect_object("listCurves", params)?;
    reject_params("listCurves", &map, &["part"])?;
    optional_string("listCurves", &map, "part")
}

fn parse_sample_curve(params: Value) -> Result<(String, CurveChannel, f64), ToolFailure> {
    let map = expect_object("sampleCurve", params)?;
    reject_params("sampleCurve", &map, &["part", "channel", "frame"])?;
    let part = require_string("sampleCurve", &map, "part")?;
    let channel = parse_channel(
        "sampleCurve",
        &require_string("sampleCurve", &map, "channel")?,
    )?;
    let frame = require_finite("sampleCurve", &map, "frame")?;
    Ok((part, channel, frame))
}

struct ConstraintInput {
    name: String,
    kind: ConstraintKind,
    part: String,
    anchor: Option<String>,
    target: Option<String>,
    target_anchor: Option<String>,
    x: Option<i32>,
    y: Option<i32>,
    chain: Vec<String>,
    bend: Option<i8>,
    contacts: Vec<Contact>,
}

fn parse_add_constraint(params: Value) -> Result<ConstraintInput, ToolFailure> {
    let map = expect_object("addConstraint", params)?;
    reject_params(
        "addConstraint",
        &map,
        &[
            "name",
            "kind",
            "part",
            "anchor",
            "target",
            "targetAnchor",
            "x",
            "y",
            "chain",
            "bend",
            "contacts",
        ],
    )?;
    let name = require_string("addConstraint", &map, "name")?;
    let kind = parse_constraint_kind(
        "addConstraint",
        &require_string("addConstraint", &map, "kind")?,
    )?;
    let part = require_string("addConstraint", &map, "part")?;
    let anchor = optional_string("addConstraint", &map, "anchor")?;
    let target = optional_string("addConstraint", &map, "target")?;
    let target_anchor = optional_string("addConstraint", &map, "targetAnchor")?;
    let x = optional_i32("addConstraint", &map, "x")?;
    let y = optional_i32("addConstraint", &map, "y")?;
    let chain = optional_string_list("addConstraint", &map, "chain")?.unwrap_or_default();
    let bend = optional_bend("addConstraint", &map)?;
    let contacts = optional_contacts("addConstraint", &map)?;
    Ok(ConstraintInput {
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
    })
}

fn optional_bend(tool: &str, map: &Map<String, Value>) -> Result<Option<i8>, ToolFailure> {
    match map.get("bend") {
        None | Some(Value::Null) => Ok(None),
        Some(value) => {
            let number = value
                .as_i64()
                .ok_or_else(|| invalid_params(tool, "bend must be 1 or -1"))?;
            if number == 1 || number == -1 {
                Ok(Some(number as i8))
            } else {
                Err(invalid_params(tool, "bend must be 1 or -1"))
            }
        }
    }
}

fn optional_contacts(tool: &str, map: &Map<String, Value>) -> Result<Vec<Contact>, ToolFailure> {
    let Some(value) = map.get("contacts") else {
        return Ok(Vec::new());
    };
    let Some(items) = value.as_array() else {
        return Err(invalid_params(tool, "contacts must be an array"));
    };
    if items.is_empty() {
        return Err(invalid_params(tool, "contacts must list at least one span"));
    }
    let mut contacts = Vec::with_capacity(items.len());
    for item in items {
        let Some(contact) = item.as_object() else {
            return Err(invalid_params(tool, "contacts must be an array of objects"));
        };
        reject_unknown(contact, &["frame", "to", "x", "y"], |field| {
            invalid_params(tool, &format!("unknown field {field}"))
        })?;
        let frame = require_u16(tool, contact, "frame")?;
        let to = match contact.get("to") {
            None => frame,
            Some(_) => require_u16(tool, contact, "to")?,
        };
        let x = require_u16(tool, contact, "x")?;
        let y = require_u16(tool, contact, "y")?;
        contacts.push(Contact { frame, to, x, y });
    }
    Ok(contacts)
}

fn parse_remove_constraint(params: Value) -> Result<String, ToolFailure> {
    let map = expect_object("removeConstraint", params)?;
    reject_params("removeConstraint", &map, &["name"])?;
    require_string("removeConstraint", &map, "name")
}

fn parse_solve_constraints(
    params: Value,
) -> Result<(Option<f64>, bool, Option<usize>), ToolFailure> {
    let map = expect_object("solveConstraints", params)?;
    reject_params("solveConstraints", &map, &["frame", "apply", "pixelFrame"])?;
    let frame = optional_finite("solveConstraints", &map, "frame")?;
    let apply = optional_bool("solveConstraints", &map, "apply")?;
    let pixel_frame = optional_frame("solveConstraints", &map, "pixelFrame")?;
    Ok((frame, apply, pixel_frame))
}

fn parse_bake(params: Value) -> Result<BakeOptions, ToolFailure> {
    let map = expect_object("bake", params)?;
    reject_params(
        "bake",
        &map,
        &[
            "from",
            "to",
            "source",
            "fps",
            "parts",
            "layers",
            "constraints",
        ],
    )?;
    let from = optional_frame("bake", &map, "from")?;
    let to = optional_frame("bake", &map, "to")?;
    let source = optional_frame("bake", &map, "source")?;
    let fps = optional_finite("bake", &map, "fps")?;
    let parts = optional_string_list("bake", &map, "parts")?;
    let layers = optional_index_list("bake", &map, "layers")?;
    let constraints = match map.get("constraints") {
        None => true,
        Some(Value::Bool(value)) => *value,
        Some(_) => {
            return Err(invalid_params("bake", "constraints must be a boolean"));
        }
    };
    Ok(BakeOptions {
        from,
        to,
        source,
        fps,
        parts,
        layers,
        constraints,
    })
}

fn optional_string_list(
    tool: &str,
    map: &Map<String, Value>,
    key: &str,
) -> Result<Option<Vec<String>>, ToolFailure> {
    match map.get(key) {
        None => Ok(None),
        Some(Value::Array(items)) => {
            if items.is_empty() {
                return Err(invalid_params(
                    tool,
                    &format!("{key} must list at least one name"),
                ));
            }
            let mut names = Vec::with_capacity(items.len());
            for item in items {
                let Some(text) = item.as_str() else {
                    return Err(invalid_params(
                        tool,
                        &format!("{key} must be an array of strings"),
                    ));
                };
                names.push(text.to_string());
            }
            Ok(Some(names))
        }
        Some(_) => Err(invalid_params(
            tool,
            &format!("{key} must be an array of strings"),
        )),
    }
}

fn optional_index_list(
    tool: &str,
    map: &Map<String, Value>,
    key: &str,
) -> Result<Option<Vec<usize>>, ToolFailure> {
    match map.get(key) {
        None => Ok(None),
        Some(Value::Array(items)) => {
            if items.is_empty() {
                return Err(invalid_params(
                    tool,
                    &format!("{key} must list at least one index"),
                ));
            }
            let mut indexes = Vec::with_capacity(items.len());
            for item in items {
                let Some(value) = item.as_u64() else {
                    return Err(invalid_params(
                        tool,
                        &format!("{key} must be an array of integers from 0 to {U32_MAX}"),
                    ));
                };
                if value > U32_MAX {
                    return Err(invalid_params(
                        tool,
                        &format!("{key} must be an array of integers from 0 to {U32_MAX}"),
                    ));
                }
                indexes.push(usize::try_from(value).expect("u32 fits in usize"));
            }
            Ok(Some(indexes))
        }
        Some(_) => Err(invalid_params(
            tool,
            &format!("{key} must be an array of integers from 0 to {U32_MAX}"),
        )),
    }
}

fn parse_constraint_kind(tool: &str, text: &str) -> Result<ConstraintKind, ToolFailure> {
    ConstraintKind::parse(text).ok_or_else(|| {
        invalid_params(
            tool,
            "kind must be follow, attach, stick, pin, ik, or plant",
        )
    })
}

fn optional_finite(
    tool: &str,
    map: &Map<String, Value>,
    key: &str,
) -> Result<Option<f64>, ToolFailure> {
    if map.get(key).is_none() {
        return Ok(None);
    }
    Ok(Some(require_finite(tool, map, key)?))
}

fn optional_bool(tool: &str, map: &Map<String, Value>, key: &str) -> Result<bool, ToolFailure> {
    match map.get(key) {
        None => Ok(false),
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(invalid_params(tool, &format!("{key} must be a boolean"))),
    }
}

fn parse_sample_part(params: Value) -> Result<(String, f64), ToolFailure> {
    let map = expect_object("samplePart", params)?;
    reject_params("samplePart", &map, &["part", "frame"])?;
    let part = require_string("samplePart", &map, "part")?;
    let frame = require_finite("samplePart", &map, "frame")?;
    Ok((part, frame))
}

fn parse_channel(tool: &str, text: &str) -> Result<CurveChannel, ToolFailure> {
    CurveChannel::parse(text).ok_or_else(|| {
        invalid_params(
            tool,
            "channel must be rotation, translateX, translateY, flipHorizontal, or flipVertical",
        )
    })
}

fn parse_ease(tool: &str, text: &str) -> Result<Ease, ToolFailure> {
    Ease::parse(text).ok_or_else(|| invalid_params(tool, "ease must be linear, easeInOut, or hold"))
}

fn require_finite(tool: &str, map: &Map<String, Value>, key: &str) -> Result<f64, ToolFailure> {
    match map.get(key) {
        None => Err(invalid_params(tool, &format!("missing {key}"))),
        Some(Value::Number(number)) => match number.as_f64() {
            Some(value) if value.is_finite() => Ok(value),
            _ => Err(invalid_params(
                tool,
                &format!("{key} must be a finite number"),
            )),
        },
        Some(_) => Err(invalid_params(
            tool,
            &format!("{key} must be a finite number"),
        )),
    }
}

fn parse_remove_anchor(params: Value) -> Result<(String, String), ToolFailure> {
    let map = expect_object("removeAnchor", params)?;
    reject_params("removeAnchor", &map, &["part", "name"])?;
    let part = require_string("removeAnchor", &map, "part")?;
    let name = require_string("removeAnchor", &map, "name")?;
    Ok((part, name))
}

fn parse_add_tag(params: Value) -> Result<(String, usize, usize, LoopMode), ToolFailure> {
    let map = expect_object("addTag", params)?;
    reject_params("addTag", &map, &["name", "from", "to", "loop"])?;
    let name = require_string("addTag", &map, "name")?;
    let from = require_frame("addTag", &map, "from")?;
    let to = require_frame("addTag", &map, "to")?;
    let loop_mode = match map.get("loop") {
        None => LoopMode::Forward,
        Some(value) => parse_loop_mode("addTag", value)?,
    };
    Ok((name, from, to, loop_mode))
}

fn parse_set_tag(params: Value) -> Result<(String, TagUpdate), ToolFailure> {
    let map = expect_object("setTag", params)?;
    reject_params("setTag", &map, &["name", "from", "to", "loop", "rename"])?;
    let name = require_string("setTag", &map, "name")?;
    let from = optional_frame("setTag", &map, "from")?;
    let to = optional_frame("setTag", &map, "to")?;
    let loop_mode = match map.get("loop") {
        None => None,
        Some(value) => Some(parse_loop_mode("setTag", value)?),
    };
    let rename = match map.get("rename") {
        None => None,
        Some(_) => Some(require_string("setTag", &map, "rename")?),
    };
    if from.is_none() && to.is_none() && loop_mode.is_none() && rename.is_none() {
        return Err(invalid_params("setTag", "pass from, to, loop, or rename"));
    }
    Ok((
        name,
        TagUpdate {
            from,
            to,
            loop_mode,
            rename,
        },
    ))
}

fn parse_remove_tag(params: Value) -> Result<String, ToolFailure> {
    let map = expect_object("removeTag", params)?;
    reject_params("removeTag", &map, &["name"])?;
    require_string("removeTag", &map, "name")
}

fn parse_loop_mode(tool: &str, value: &Value) -> Result<LoopMode, ToolFailure> {
    let Some(text) = value.as_str() else {
        return Err(invalid_params(tool, "loop must be a string"));
    };
    LoopMode::parse(text)
        .ok_or_else(|| invalid_params(tool, "loop must be forward, reverse, pingpong, or once"))
}

fn parse_preview(
    params: Value,
    document: &Document,
) -> Result<(usize, u32, Option<usize>, Option<ViewScope>), ToolFailure> {
    let map = expect_object("previewFrame", params)?;
    reject_params(
        "previewFrame",
        &map,
        &[
            "frame",
            "scale",
            "layer",
            "selection",
            "x",
            "y",
            "width",
            "height",
            "mask",
        ],
    )?;
    let frame = match optional_frame("previewFrame", &map, "frame")? {
        Some(frame) => frame,
        None => document.active_frame(),
    };
    let scale = match map.get("scale") {
        None => 1,
        Some(_) => require_u32_at_least("previewFrame", &map, "scale", 1)?,
    };
    let layer = optional_frame("previewFrame", &map, "layer")?;
    let scope = resolve_scope(document, "previewFrame", &map)?;
    Ok((frame, scale, layer, scope))
}

fn parse_diff(
    params: Value,
    document: &Document,
) -> Result<(usize, usize, u32, Option<usize>, Option<ViewScope>), ToolFailure> {
    let map = expect_object("diffFrames", params)?;
    reject_params(
        "diffFrames",
        &map,
        &[
            "frame",
            "with",
            "scale",
            "layer",
            "selection",
            "x",
            "y",
            "width",
            "height",
            "mask",
        ],
    )?;
    let frame = require_frame("diffFrames", &map, "frame")?;
    let with = require_frame("diffFrames", &map, "with")?;
    let scale = match map.get("scale") {
        None => 1,
        Some(_) => require_u32_at_least("diffFrames", &map, "scale", 1)?,
    };
    let layer = optional_frame("diffFrames", &map, "layer")?;
    let scope = resolve_scope(document, "diffFrames", &map)?;
    Ok((frame, with, scale, layer, scope))
}

fn resolve_scope(
    document: &Document,
    tool: &str,
    map: &Map<String, Value>,
) -> Result<Option<ViewScope>, ToolFailure> {
    let selection = match map.get("selection") {
        None => false,
        Some(Value::Bool(true)) => true,
        Some(_) => return Err(invalid_params(tool, "selection must be true")),
    };
    let has_rect = ["x", "y", "width", "height"]
        .iter()
        .any(|key| map.contains_key(*key));
    let has_mask = map.contains_key("mask");
    let chosen = usize::from(selection) + usize::from(has_rect) + usize::from(has_mask);
    if chosen > 1 {
        return Err(invalid_params(
            tool,
            "pass only one of selection, x/y/width/height, or mask",
        ));
    }
    if chosen == 0 {
        return Ok(None);
    }
    if selection {
        let Some(mask) = document.selection().cloned() else {
            return Err(ToolFailure::Kernel(KernelError::MaskMissing));
        };
        return Ok(Some(ViewScope {
            mode: "selection",
            mask,
        }));
    }
    let mask = if has_rect {
        let (x, y, width, height) = rect_from(tool, map)?;
        Mask::from_rect(Selection {
            x,
            y,
            width,
            height,
        })
    } else {
        mask_array(tool, "mask", map.get("mask").expect("mask was checked"))?
    };
    for rect in mask.rects() {
        document
            .inclusive_rect(rect.x, rect.y, rect.width, rect.height)
            .map_err(ToolFailure::Kernel)?;
    }
    let mode = if has_rect { "rect" } else { "mask" };
    Ok(Some(ViewScope { mode, mask }))
}

fn parse_combine_mask(params: Value) -> Result<(MaskOp, Option<Mask>, Mask), ToolFailure> {
    let map = expect_object("combineMask", params)?;
    reject_params(
        "combineMask",
        &map,
        &[
            "op",
            "x",
            "y",
            "width",
            "height",
            "mask",
            "base",
            "baseX",
            "baseY",
            "baseWidth",
            "baseHeight",
        ],
    )?;
    let op = match map
        .get("op")
        .and_then(Value::as_str)
        .and_then(MaskOp::parse)
    {
        Some(op) => op,
        None => {
            return Err(invalid_params(
                "combineMask",
                "op must be union, intersect, or subtract",
            ))
        }
    };
    let has_rect = ["x", "y", "width", "height"]
        .iter()
        .any(|key| map.contains_key(*key));
    let has_mask = map.contains_key("mask");
    if has_rect == has_mask {
        return Err(invalid_params(
            "combineMask",
            "pass x, y, width, and height, or mask",
        ));
    }
    let operand = if has_rect {
        let (x, y, width, height) = rect_from("combineMask", &map)?;
        Mask::from_rect(Selection {
            x,
            y,
            width,
            height,
        })
    } else {
        mask_array(
            "combineMask",
            "mask",
            map.get("mask").expect("mask was checked"),
        )?
    };
    let has_base_rect = ["baseX", "baseY", "baseWidth", "baseHeight"]
        .iter()
        .any(|key| map.contains_key(*key));
    let has_base = map.contains_key("base");
    if has_base && has_base_rect {
        return Err(invalid_params(
            "combineMask",
            "pass base or baseX, baseY, baseWidth, and baseHeight",
        ));
    }
    let base = if has_base_rect {
        let x = require_u16("combineMask", &map, "baseX")?;
        let y = require_u16("combineMask", &map, "baseY")?;
        let width = require_u16_at_least("combineMask", &map, "baseWidth", 1)?;
        let height = require_u16_at_least("combineMask", &map, "baseHeight", 1)?;
        Some(Mask::from_rect(Selection {
            x,
            y,
            width,
            height,
        }))
    } else if has_base {
        Some(mask_value(
            "combineMask",
            "base",
            map.get("base").expect("base was checked"),
        )?)
    } else {
        None
    };
    Ok((op, base, operand))
}

fn mask_array(tool: &str, key: &str, value: &Value) -> Result<Mask, ToolFailure> {
    let Some(items) = value.as_array() else {
        return Err(invalid_params(
            tool,
            &format!("{key} must be an array of rectangles"),
        ));
    };
    if items.is_empty() {
        return Err(invalid_params(
            tool,
            &format!("{key} must list at least one rectangle"),
        ));
    }
    let mut rects = Vec::with_capacity(items.len());
    for item in items {
        rects.push(rect_value(tool, key, item)?);
    }
    Mask::try_from_rects(rects)
        .map_err(|()| invalid_params(tool, &format!("{key} has an empty rectangle")))
}

fn mask_value(tool: &str, key: &str, value: &Value) -> Result<Mask, ToolFailure> {
    if value.is_array() {
        return mask_array(tool, key, value);
    }
    Ok(Mask::from_rect(rect_value(tool, key, value)?))
}

fn rect_value(tool: &str, key: &str, value: &Value) -> Result<Selection, ToolFailure> {
    let Some(map) = value.as_object() else {
        return Err(invalid_params(
            tool,
            &format!("{key} must be a rectangle with x, y, width, and height"),
        ));
    };
    if map.len() != 4
        || ["x", "y", "width", "height"]
            .iter()
            .any(|field| !map.contains_key(*field))
    {
        return Err(invalid_params(
            tool,
            &format!("{key} must be a rectangle with x, y, width, and height"),
        ));
    }
    let owned = Map::clone(map);
    let (x, y, width, height) = rect_from(tool, &owned)?;
    Ok(Selection {
        x,
        y,
        width,
        height,
    })
}

struct FlipSpec {
    frame: Option<usize>,
    rect: Option<(u16, u16, u16, u16)>,
    layer: Option<usize>,
}

fn parse_flip(params: Value) -> Result<FlipSpec, ToolFailure> {
    parse_mirror("flipHorizontal", params)
}

fn parse_mirror(tool: &str, params: Value) -> Result<FlipSpec, ToolFailure> {
    let map = expect_object(tool, params)?;
    reject_params(tool, &map, &["frame", "x", "y", "width", "height", "layer"])?;
    let frame = optional_frame(tool, &map, "frame")?;
    let layer = optional_frame(tool, &map, "layer")?;
    let present = ["x", "y", "width", "height"]
        .iter()
        .filter(|key| map.contains_key(**key))
        .count();
    let rect = if present == 0 {
        None
    } else if present == 4 {
        Some(rect_from(tool, &map)?)
    } else {
        return Err(invalid_params(
            tool,
            "x, y, width, and height must all be set, or all omitted",
        ));
    };
    Ok(FlipSpec { frame, rect, layer })
}

fn parse_rotate(params: Value) -> Result<(FlipSpec, QuarterTurn), ToolFailure> {
    let map = expect_object("rotate", params)?;
    reject_params(
        "rotate",
        &map,
        &["frame", "x", "y", "width", "height", "layer", "degrees"],
    )?;
    let degrees = require_u16("rotate", &map, "degrees")?;
    let turn = match degrees {
        90 => QuarterTurn::Cw90,
        180 => QuarterTurn::Cw180,
        270 => QuarterTurn::Cw270,
        _ => return Err(invalid_params("rotate", "degrees must be 90, 180, or 270")),
    };
    let spec = parse_mirror(
        "rotate",
        Value::Object({
            let mut kept = map;
            kept.remove("degrees");
            kept
        }),
    )?;
    Ok((spec, turn))
}

fn parse_aseprite(params: Value) -> Result<Vec<u8>, ToolFailure> {
    let map = expect_object("importAseprite", params)?;
    reject_params("importAseprite", &map, &["bytes"])?;
    let text = require_string("importAseprite", &map, "bytes")?;
    decode_base64(&text).map_err(|_| invalid_params("importAseprite", "bytes must be base64"))
}

fn aseprite_json(report: &AsepriteReport) -> Value {
    serde_json::json!({
        "sourceTransparentIndex": report.source_transparent_index,
        "transparentSwapped": report.transparent_swapped,
        "hiddenLayers": report.hidden_layers,
        "skippedGroups": report.skipped_groups,
        "skippedReference": report.skipped_reference,
        "backgroundLayers": report.background_layers,
        "nonNormalBlend": report.non_normal_blend,
        "partialOpacity": report.partial_opacity,
        "partialCelOpacity": report.partial_cel_opacity,
        "nonzeroZIndex": report.nonzero_z_index,
        "durationDefaulted": report.duration_defaulted,
        "clippedCells": report.clipped_cells,
        "ignored": report.ignored,
    })
}

fn parse_add_layer(params: Value) -> Result<(Option<String>, LayerRole), ToolFailure> {
    let map = expect_object("addLayer", params)?;
    reject_params("addLayer", &map, &["name", "role"])?;
    let name = match map.get("name") {
        None => None,
        Some(_) => Some(require_string("addLayer", &map, "name")?),
    };
    let role = optional_layer_role("addLayer", &map)?.unwrap_or(LayerRole::Default);
    Ok((name, role))
}

fn parse_set_layer_role(params: Value, active: usize) -> Result<(usize, LayerRole), ToolFailure> {
    let map = expect_object("setLayerRole", params)?;
    reject_params("setLayerRole", &map, &["index", "role"])?;
    let index = optional_frame("setLayerRole", &map, "index")?.unwrap_or(active);
    let role = optional_layer_role("setLayerRole", &map)?
        .ok_or_else(|| invalid_params("setLayerRole", "role must be default or polish"))?;
    Ok((index, role))
}

fn optional_layer_role(
    tool: &str,
    map: &Map<String, Value>,
) -> Result<Option<LayerRole>, ToolFailure> {
    match map.get("role") {
        None => Ok(None),
        Some(value) => {
            let text = value
                .as_str()
                .ok_or_else(|| invalid_params(tool, "role must be default or polish"))?;
            LayerRole::parse(text)
                .map(Some)
                .ok_or_else(|| invalid_params(tool, "role must be default or polish"))
        }
    }
}

fn parse_optional_index(tool: &str, params: Value) -> Result<Option<usize>, ToolFailure> {
    let map = expect_object(tool, params)?;
    reject_params(tool, &map, &["index"])?;
    optional_frame(tool, &map, "index")
}

fn parse_rename_layer(params: Value, active: usize) -> Result<(usize, String), ToolFailure> {
    let map = expect_object("renameLayer", params)?;
    reject_params("renameLayer", &map, &["index", "name"])?;
    let index = optional_frame("renameLayer", &map, "index")?.unwrap_or(active);
    let name = require_string("renameLayer", &map, "name")?;
    Ok((index, name))
}

fn parse_reorder(params: Value) -> Result<(usize, usize), ToolFailure> {
    let map = expect_object("reorderLayer", params)?;
    reject_params("reorderLayer", &map, &["from", "to"])?;
    let from = require_frame("reorderLayer", &map, "from")?;
    let to = require_frame("reorderLayer", &map, "to")?;
    Ok((from, to))
}

fn parse_resize(params: Value) -> Result<(u16, u16, Option<(i32, i32)>), ToolFailure> {
    let map = expect_object("resizeCanvas", params)?;
    reject_params(
        "resizeCanvas",
        &map,
        &["width", "height", "offsetX", "offsetY"],
    )?;
    let edge = u64::from(MAX_CANVAS_EDGE);
    let width = require_u16_range("resizeCanvas", &map, "width", 1, edge)?;
    let height = require_u16_range("resizeCanvas", &map, "height", 1, edge)?;
    let offset_x = optional_i32("resizeCanvas", &map, "offsetX")?;
    let offset_y = optional_i32("resizeCanvas", &map, "offsetY")?;
    let offset = match (offset_x, offset_y) {
        (None, None) => None,
        (Some(x), Some(y)) => Some((x, y)),
        _ => {
            return Err(invalid_params(
                "resizeCanvas",
                "offsetX and offsetY must both be set, or both omitted",
            ))
        }
    };
    Ok((width, height, offset))
}

fn center_offset(old: u16, new: u16) -> i32 {
    (i32::from(new) - i32::from(old)) / 2
}

fn optional_frame(
    tool: &str,
    map: &Map<String, Value>,
    key: &str,
) -> Result<Option<usize>, ToolFailure> {
    if map.get(key).is_none() {
        return Ok(None);
    }
    Ok(Some(require_frame(tool, map, key)?))
}

fn require_frame(tool: &str, map: &Map<String, Value>, key: &str) -> Result<usize, ToolFailure> {
    let index = require_u32(tool, map, key)?;
    Ok(usize::try_from(index).expect("u32 fits in usize"))
}

fn parse_scale(tool: &str, params: Value) -> Result<u32, ToolFailure> {
    let map = expect_object(tool, params)?;
    reject_params(tool, &map, &["scale"])?;
    match map.get("scale") {
        None => Ok(1),
        Some(_) => require_u32_at_least(tool, &map, "scale", 1),
    }
}

fn parse_cell(tool: &str, params: Value) -> Result<(u16, u16, Option<usize>), ToolFailure> {
    let map = expect_object(tool, params)?;
    reject_params(tool, &map, &["x", "y", "layer"])?;
    let (x, y) = cell_from(tool, &map)?;
    let layer = optional_frame(tool, &map, "layer")?;
    Ok((x, y, layer))
}

fn cell_from(tool: &str, map: &Map<String, Value>) -> Result<(u16, u16), ToolFailure> {
    let x = require_u16(tool, map, "x")?;
    let y = require_u16(tool, map, "y")?;
    Ok((x, y))
}

fn parse_set_palette_color(params: Value) -> Result<(u8, u8, u8, u8), ToolFailure> {
    let map = expect_object("setPaletteColor", params)?;
    reject_params("setPaletteColor", &map, &["index", "r", "g", "b"])?;
    let index = require_u8("setPaletteColor", &map, "index")?;
    let (r, g, b) = rgb_from("setPaletteColor", &map)?;
    Ok((index, r, g, b))
}

fn parse_rgb(tool: &str, params: Value) -> Result<(u8, u8, u8), ToolFailure> {
    let map = expect_object(tool, params)?;
    reject_params(tool, &map, &["r", "g", "b"])?;
    rgb_from(tool, &map)
}

fn rgb_from(tool: &str, map: &Map<String, Value>) -> Result<(u8, u8, u8), ToolFailure> {
    let r = require_u8(tool, map, "r")?;
    let g = require_u8(tool, map, "g")?;
    let b = require_u8(tool, map, "b")?;
    Ok((r, g, b))
}

fn parse_import(params: Value) -> Result<(String, PaletteImportMode), ToolFailure> {
    let map = expect_object("importPalette", params)?;
    reject_params("importPalette", &map, &["text", "mode"])?;
    let text = require_string("importPalette", &map, "text")?;
    let mode = match require_string("importPalette", &map, "mode")?.as_str() {
        "replace" => PaletteImportMode::Replace,
        "merge" => PaletteImportMode::Merge,
        _ => return Err(ToolFailure::Kernel(KernelError::PaletteImportMode)),
    };
    Ok((text, mode))
}

fn parse_locked(params: Value) -> Result<bool, ToolFailure> {
    let map = expect_object("setPaletteLocked", params)?;
    reject_params("setPaletteLocked", &map, &["locked"])?;
    require_bool("setPaletteLocked", &map, "locked")
}

fn parse_index_policy(params: Value) -> Result<IndexPolicy, ToolFailure> {
    let map = expect_object("setIndexPolicy", params)?;
    reject_params("setIndexPolicy", &map, &["mode"])?;
    match require_string("setIndexPolicy", &map, "mode")?.as_str() {
        "reject" => Ok(IndexPolicy::Reject),
        "clamp" => Ok(IndexPolicy::Clamp),
        _ => Err(invalid_params(
            "setIndexPolicy",
            "mode must be reject or clamp",
        )),
    }
}

enum RemapRequest {
    Text(String),
    MaxColors(u8),
}

fn parse_remap(params: Value) -> Result<RemapRequest, ToolFailure> {
    let map = expect_object("remapPalette", params)?;
    reject_params("remapPalette", &map, &["text", "maxColors"])?;
    let text = match map.get("text") {
        None => None,
        Some(_) => Some(require_string("remapPalette", &map, "text")?),
    };
    let max_colors = match map.get("maxColors") {
        None => None,
        Some(Value::Number(number)) => match number.as_u64() {
            Some(value) if (1..=255).contains(&value) => {
                Some(u8::try_from(value).expect("value was bounded by 255"))
            }
            _ => {
                return Err(invalid_params(
                    "remapPalette",
                    "maxColors must be an integer from 1 to 255",
                ))
            }
        },
        Some(_) => {
            return Err(invalid_params(
                "remapPalette",
                "maxColors must be an integer from 1 to 255",
            ))
        }
    };
    match (text, max_colors) {
        (Some(text), None) => Ok(RemapRequest::Text(text)),
        (None, Some(max_colors)) => Ok(RemapRequest::MaxColors(max_colors)),
        (Some(_), Some(_)) => Err(invalid_params(
            "remapPalette",
            "pass text or maxColors, not both",
        )),
        (None, None) => Err(invalid_params("remapPalette", "pass text or maxColors")),
    }
}

fn parse_apply_png(params: Value) -> Result<(Vec<u8>, Option<usize>, Option<usize>), ToolFailure> {
    let map = expect_object("applyPng", params)?;
    reject_params("applyPng", &map, &["png", "frame", "layer"])?;
    let text = require_string("applyPng", &map, "png")?;
    let png = decode_base64(&text).map_err(|_| invalid_params("applyPng", "png must be base64"))?;
    let frame = optional_frame("applyPng", &map, "frame")?;
    let layer = optional_frame("applyPng", &map, "layer")?;
    Ok((png, frame, layer))
}

fn parse_index(tool: &str, params: Value) -> Result<usize, ToolFailure> {
    let map = expect_object(tool, params)?;
    reject_params(tool, &map, &["index"])?;
    let index = require_u32(tool, &map, "index")?;
    Ok(usize::try_from(index).expect("u32 fits in usize"))
}

fn parse_delay(params: Value) -> Result<(usize, u32), ToolFailure> {
    let map = expect_object("setFrameDelay", params)?;
    reject_params("setFrameDelay", &map, &["index", "delayMs"])?;
    let index = require_u32("setFrameDelay", &map, "index")?;
    let delay_ms = require_u32("setFrameDelay", &map, "delayMs")?;
    Ok((usize::try_from(index).expect("u32 fits in usize"), delay_ms))
}

fn parse_project_json(params: Value) -> Result<String, ToolFailure> {
    let map = expect_object("loadProject", params)?;
    reject_params("loadProject", &map, &["json"])?;
    require_string("loadProject", &map, "json")
}

fn expect_object(tool: &str, params: Value) -> Result<Map<String, Value>, ToolFailure> {
    match params {
        Value::Null => Ok(Map::new()),
        Value::Object(map) => Ok(map),
        _ => Err(invalid_params(tool, "params must be an object")),
    }
}

fn reject_params(tool: &str, map: &Map<String, Value>, known: &[&str]) -> Result<(), ToolFailure> {
    reject_unknown(map, known, |field| {
        invalid_params(tool, &format!("unknown field {field}"))
    })
}

fn reject_unknown<E>(
    map: &Map<String, Value>,
    known: &[&str],
    error: impl Fn(&str) -> E,
) -> Result<(), E> {
    for key in map.keys() {
        if !known.contains(&key.as_str()) {
            return Err(error(key));
        }
    }
    Ok(())
}

fn require_u8(tool: &str, map: &Map<String, Value>, key: &str) -> Result<u8, ToolFailure> {
    Ok(u8::try_from(require_int(tool, map, key, U8_MAX)?).expect("value was bounded by u8::MAX"))
}

fn require_u16(tool: &str, map: &Map<String, Value>, key: &str) -> Result<u16, ToolFailure> {
    require_u16_at_least(tool, map, key, 0)
}

fn optional_u16(
    tool: &str,
    map: &Map<String, Value>,
    key: &str,
) -> Result<Option<u16>, ToolFailure> {
    if map.get(key).is_none() {
        return Ok(None);
    }
    Ok(Some(require_u16(tool, map, key)?))
}

fn require_i32(tool: &str, map: &Map<String, Value>, key: &str) -> Result<i32, ToolFailure> {
    match map.get(key) {
        None => Err(invalid_params(tool, &format!("missing {key}"))),
        Some(Value::Number(number)) => match number.as_i64() {
            Some(value) if i64::from(i32::MIN) <= value && value <= i64::from(i32::MAX) => {
                Ok(i32::try_from(value).expect("value was bounded by i32"))
            }
            _ => Err(invalid_params(
                tool,
                &format!("{key} must be an integer from {} to {}", i32::MIN, i32::MAX),
            )),
        },
        Some(_) => Err(invalid_params(
            tool,
            &format!("{key} must be an integer from {} to {}", i32::MIN, i32::MAX),
        )),
    }
}

fn require_u16_at_least(
    tool: &str,
    map: &Map<String, Value>,
    key: &str,
    min: u64,
) -> Result<u16, ToolFailure> {
    require_u16_range(tool, map, key, min, U16_MAX)
}

fn require_u16_range(
    tool: &str,
    map: &Map<String, Value>,
    key: &str,
    min: u64,
    max: u64,
) -> Result<u16, ToolFailure> {
    match map.get(key) {
        None => Err(invalid_params(tool, &format!("missing {key}"))),
        Some(Value::Number(number)) => match number.as_u64() {
            Some(value) if (min..=max).contains(&value) => {
                Ok(u16::try_from(value).expect("value was bounded by u16"))
            }
            _ => Err(invalid_params(
                tool,
                &format!("{key} must be an integer from {min} to {max}"),
            )),
        },
        Some(_) => Err(invalid_params(
            tool,
            &format!("{key} must be an integer from {min} to {max}"),
        )),
    }
}

fn optional_i32(
    tool: &str,
    map: &Map<String, Value>,
    key: &str,
) -> Result<Option<i32>, ToolFailure> {
    match map.get(key) {
        None => Ok(None),
        Some(Value::Number(number)) => match number.as_i64() {
            Some(value) if i64::from(i32::MIN) <= value && value <= i64::from(i32::MAX) => Ok(
                Some(i32::try_from(value).expect("value was bounded by i32")),
            ),
            _ => Err(invalid_params(
                tool,
                &format!("{key} must be an integer from {} to {}", i32::MIN, i32::MAX),
            )),
        },
        Some(_) => Err(invalid_params(
            tool,
            &format!("{key} must be an integer from {} to {}", i32::MIN, i32::MAX),
        )),
    }
}

fn require_u32(tool: &str, map: &Map<String, Value>, key: &str) -> Result<u32, ToolFailure> {
    require_u32_at_least(tool, map, key, 0)
}

fn require_u32_at_least(
    tool: &str,
    map: &Map<String, Value>,
    key: &str,
    min: u64,
) -> Result<u32, ToolFailure> {
    match map.get(key) {
        None => Err(invalid_params(tool, &format!("missing {key}"))),
        Some(Value::Number(number)) => match number.as_u64() {
            Some(value) if (min..=U32_MAX).contains(&value) => {
                Ok(u32::try_from(value).expect("value was bounded by u32::MAX"))
            }
            _ => Err(invalid_params(
                tool,
                &format!("{key} must be an integer from {min} to {U32_MAX}"),
            )),
        },
        Some(_) => Err(invalid_params(
            tool,
            &format!("{key} must be an integer from {min} to {U32_MAX}"),
        )),
    }
}

fn require_int(
    tool: &str,
    map: &Map<String, Value>,
    key: &str,
    max: u64,
) -> Result<u64, ToolFailure> {
    match map.get(key) {
        None => Err(invalid_params(tool, &format!("missing {key}"))),
        Some(Value::Number(number)) => match number.as_u64() {
            Some(value) if value <= max => Ok(value),
            _ => Err(invalid_params(
                tool,
                &format!("{key} must be an integer from 0 to {max}"),
            )),
        },
        Some(_) => Err(invalid_params(
            tool,
            &format!("{key} must be an integer from 0 to {max}"),
        )),
    }
}

fn require_bool(tool: &str, map: &Map<String, Value>, key: &str) -> Result<bool, ToolFailure> {
    match map.get(key) {
        None => Err(invalid_params(tool, &format!("missing {key}"))),
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(invalid_params(tool, &format!("{key} must be a boolean"))),
    }
}

fn require_string(tool: &str, map: &Map<String, Value>, key: &str) -> Result<String, ToolFailure> {
    match map.get(key) {
        None => Err(invalid_params(tool, &format!("missing {key}"))),
        Some(Value::String(value)) => Ok(value.clone()),
        Some(_) => Err(invalid_params(tool, &format!("{key} must be a string"))),
    }
}

fn optional_string(
    tool: &str,
    map: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, ToolFailure> {
    if map.get(key).is_none() {
        return Ok(None);
    }
    Ok(Some(require_string(tool, map, key)?))
}

fn require_degrees(tool: &str, map: &Map<String, Value>, key: &str) -> Result<f64, ToolFailure> {
    match map.get(key) {
        None => Err(invalid_params(tool, &format!("missing {key}"))),
        Some(Value::Number(number)) => match number.as_f64() {
            Some(value) if value.is_finite() => Ok(value),
            _ => Err(invalid_params(
                tool,
                "degrees must be a finite number from -1000000 to 1000000",
            )),
        },
        Some(_) => Err(invalid_params(
            tool,
            "degrees must be a finite number from -1000000 to 1000000",
        )),
    }
}

fn invalid_params(tool: &str, detail: &str) -> ToolFailure {
    ToolFailure::InvalidParams {
        message: format!("{tool}: {detail}"),
    }
}

fn encode_base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    const SHIFTS: [u32; 4] = [18, 12, 6, 0];
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let push = |out: &mut String, n: u32, count: usize| {
        for shift in SHIFTS.into_iter().take(count) {
            let index = usize::try_from((n >> shift) & 0x3f).expect("sextet fits in 0..=63");
            out.push(ALPHABET[index] as char);
        }
    };
    let mut chunks = bytes.chunks_exact(3);
    for chunk in chunks.by_ref() {
        let n = (u32::from(chunk[0]) << 16) | (u32::from(chunk[1]) << 8) | u32::from(chunk[2]);
        push(&mut out, n, 4);
    }
    match chunks.remainder() {
        [] => {}
        [a] => {
            push(&mut out, u32::from(*a) << 16, 2);
            out.push('=');
            out.push('=');
        }
        [a, b] => {
            push(&mut out, (u32::from(*a) << 16) | (u32::from(*b) << 8), 3);
            out.push('=');
        }
        _ => unreachable!("chunks_exact remainder is shorter than 3"),
    }
    out
}

fn decode_base64(text: &str) -> Result<Vec<u8>, ()> {
    fn value(byte: u8) -> Option<u8> {
        match byte {
            b'A'..=b'Z' => Some(byte - b'A'),
            b'a'..=b'z' => Some(byte - b'a' + 26),
            b'0'..=b'9' => Some(byte - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let bytes: Vec<u8> = text
        .bytes()
        .filter(|byte| !matches!(byte, b' ' | b'\n' | b'\r' | b'\t'))
        .collect();
    if bytes.is_empty() || bytes.len() % 4 != 0 {
        return Err(());
    }
    let pad = bytes.iter().rev().take_while(|byte| **byte == b'=').count();
    if pad > 2 || bytes[..bytes.len() - pad].contains(&b'=') {
        return Err(());
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks_exact(4) {
        let mut sextets = [0u8; 4];
        for (index, byte) in chunk.iter().copied().enumerate() {
            if byte == b'=' {
                if index < 2 {
                    return Err(());
                }
                sextets[index] = 0;
            } else {
                sextets[index] = value(byte).ok_or(())?;
            }
        }
        let packed = (u32::from(sextets[0]) << 18)
            | (u32::from(sextets[1]) << 12)
            | (u32::from(sextets[2]) << 6)
            | u32::from(sextets[3]);
        out.push(u8::try_from((packed >> 16) & 0xff).expect("masked to a byte"));
        if chunk[2] != b'=' {
            out.push(u8::try_from((packed >> 8) & 0xff).expect("masked to a byte"));
        }
        if chunk[3] != b'=' {
            out.push(u8::try_from(packed & 0xff).expect("masked to a byte"));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(encode_base64(b""), "");
        assert_eq!(encode_base64(b"f"), "Zg==");
        assert_eq!(encode_base64(b"fo"), "Zm8=");
        assert_eq!(encode_base64(b"foo"), "Zm9v");
        assert!(encode_base64(b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0d").starts_with("iVBORw0KGgoAAA"));
        assert_eq!(decode_base64("Zm9v").unwrap(), b"foo");
        assert_eq!(decode_base64("Zm8=").unwrap(), b"fo");
        assert_eq!(decode_base64("Zg==").unwrap(), b"f");
        assert_eq!(decode_base64("Zm\n9v").unwrap(), b"foo");
        assert!(decode_base64("").is_err());
        assert!(decode_base64("!!!!").is_err());
        assert!(decode_base64("Zm9v====").is_err());
    }

    #[test]
    fn tool_catalog_has_no_duplicates() {
        let mut names = TOOL_NAMES.to_vec();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), TOOL_NAMES.len());
        assert_eq!(TOOL_API_VERSION, 1);
    }
}
