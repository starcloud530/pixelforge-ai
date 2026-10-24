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

use crate::document::Document;
use crate::error::KernelError;
use crate::palette_file::PaletteImportMode;
use crate::scene::{self, CellBounds, CellChange, DiffSummary, DirtyRegion, ViewSummary};
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
    "setPaletteColor",
    "addPaletteColor",
    "importPalette",
    "setPaletteLocked",
    "setActiveFrame",
    "addFrame",
    "deleteFrame",
    "duplicateFrame",
    "setFrameDelay",
    "beginGesture",
    "endGesture",
    "undo",
    "redo",
    "saveProject",
    "loadProject",
    "exportPngSequence",
    "exportSpriteSheet",
    "previewFrame",
    "diffFrames",
    "flipHorizontal",
    "resizeCanvas",
    "listLayers",
    "addLayer",
    "deleteLayer",
    "renameLayer",
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
            let before = snaps_if(document, report, &[document.active_frame()], layer)?;
            match layer {
                Some(layer) => document.paint_on(layer, x, y, color_index),
                None => document.paint(x, y, color_index),
            }
            .map_err(ToolFailure::Kernel)?;
            Ok(with_dirty(empty(), document, before, false))
        }
        "fillRect" => {
            let report = take_report_dirty(tool, &mut params)?;
            let (x, y, width, height, color_index, layer) = parse_fill_rect(params)?;
            let before = snaps_if(document, report, &[document.active_frame()], layer)?;
            match layer {
                Some(layer) => document.fill_rect_on(layer, x, y, width, height, color_index),
                None => document.fill_rect(x, y, width, height, color_index),
            }
            .map_err(ToolFailure::Kernel)?;
            Ok(with_dirty(empty(), document, before, false))
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
            let (scale, columns) = parse_sheet(params)?;
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
            Ok(serde_json::json!({
                "png": png,
                "columns": layout.columns,
                "rows": layout.rows,
                "sheetWidth": sheet_width,
                "sheetHeight": sheet_height,
                "scale": scale,
            }))
        }
        "previewFrame" => {
            let (frame, scale, layer) = parse_preview(params, document.active_frame())?;
            let (png_width, png_height) = document
                .scaled_frame_size(scale)
                .map_err(ToolFailure::Kernel)?;
            let pixels = document
                .view_pixels(frame, layer)
                .map_err(ToolFailure::Kernel)?;
            let summary = scene::summarize_view(&pixels, document.width());
            let png = document
                .export_view_png(frame, scale, layer)
                .map_err(ToolFailure::Kernel)?;
            Ok(serde_json::json!({
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
            }))
        }
        "diffFrames" => {
            let (frame, with, scale, layer) = parse_diff(params)?;
            let left = document
                .view_pixels(frame, layer)
                .map_err(ToolFailure::Kernel)?;
            let right = document
                .view_pixels(with, layer)
                .map_err(ToolFailure::Kernel)?;
            let summary = scene::summarize_diff(&left, &right, document.width());
            let diff = document
                .diff_frames_on(frame, with, scale, layer)
                .map_err(ToolFailure::Kernel)?;
            Ok(serde_json::json!({
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
            }))
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
            let name = parse_add_layer(params)?;
            let index = document
                .add_layer(name.as_deref())
                .map_err(ToolFailure::Kernel)?;
            Ok(serde_json::json!({
                "index": as_u32(index),
                "name": document.layer_name(index).expect("new layer is in range"),
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
        "frameCount": document.frame_count(),
        "activeFrame": active,
        "delayMs": delay_ms,
        "frameDelays": document.frame_delays(),
        "pixels": document.pixels(),
        "composite": document.composite_pixels(active).expect("active frame is in range"),
        "layerCount": document.layer_count(),
        "activeLayer": document.active_layer(),
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
        "layerCount": document.layer_count(),
        "layers": scene_layers(document),
        "paletteLocked": document.palette_locked(),
        "paletteCount": document.palette_colors().len(),
        "paletteHash": scene::palette_hash(document.palette_colors()),
        "palette": palette_entries(document),
        "frameCount": document.frame_count(),
        "delayMs": delay_ms,
        "frames": frames,
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
            })
        })
        .collect()
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

fn parse_sheet(params: Value) -> Result<(u32, Option<u32>), ToolFailure> {
    let map = expect_object("exportSpriteSheet", params)?;
    reject_params("exportSpriteSheet", &map, &["scale", "columns"])?;
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
    Ok((scale, columns))
}

fn parse_preview(params: Value, active: usize) -> Result<(usize, u32, Option<usize>), ToolFailure> {
    let map = expect_object("previewFrame", params)?;
    reject_params("previewFrame", &map, &["frame", "scale", "layer"])?;
    let frame = match optional_frame("previewFrame", &map, "frame")? {
        Some(frame) => frame,
        None => active,
    };
    let scale = match map.get("scale") {
        None => 1,
        Some(_) => require_u32_at_least("previewFrame", &map, "scale", 1)?,
    };
    let layer = optional_frame("previewFrame", &map, "layer")?;
    Ok((frame, scale, layer))
}

fn parse_diff(params: Value) -> Result<(usize, usize, u32, Option<usize>), ToolFailure> {
    let map = expect_object("diffFrames", params)?;
    reject_params("diffFrames", &map, &["frame", "with", "scale", "layer"])?;
    let frame = require_frame("diffFrames", &map, "frame")?;
    let with = require_frame("diffFrames", &map, "with")?;
    let scale = match map.get("scale") {
        None => 1,
        Some(_) => require_u32_at_least("diffFrames", &map, "scale", 1)?,
    };
    let layer = optional_frame("diffFrames", &map, "layer")?;
    Ok((frame, with, scale, layer))
}

struct FlipSpec {
    frame: Option<usize>,
    rect: Option<(u16, u16, u16, u16)>,
    layer: Option<usize>,
}

fn parse_flip(params: Value) -> Result<FlipSpec, ToolFailure> {
    let map = expect_object("flipHorizontal", params)?;
    reject_params(
        "flipHorizontal",
        &map,
        &["frame", "x", "y", "width", "height", "layer"],
    )?;
    let frame = optional_frame("flipHorizontal", &map, "frame")?;
    let layer = optional_frame("flipHorizontal", &map, "layer")?;
    let present = ["x", "y", "width", "height"]
        .iter()
        .filter(|key| map.contains_key(**key))
        .count();
    let rect = if present == 0 {
        None
    } else if present == 4 {
        Some(rect_from("flipHorizontal", &map)?)
    } else {
        return Err(invalid_params(
            "flipHorizontal",
            "x, y, width, and height must all be set, or all omitted",
        ));
    };
    Ok(FlipSpec { frame, rect, layer })
}

fn parse_add_layer(params: Value) -> Result<Option<String>, ToolFailure> {
    let map = expect_object("addLayer", params)?;
    reject_params("addLayer", &map, &["name"])?;
    match map.get("name") {
        None => Ok(None),
        Some(_) => Ok(Some(require_string("addLayer", &map, "name")?)),
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
