//! Stable Tool API for agents.
//!
//! [`Document::call_tool`] reads one JSON request and writes one JSON response.
//! Every mutation calls the same [`Document`] methods the shell uses, so the
//! undo stack, palette lock, and project file stay the source of truth. The
//! response carries a structured error when a call is refused. The document
//! is left unchanged on failure, except that [`Document::load_project`]
//! replaces it only after the file parses.
//!
//! This module does not call a model. External AI is a later caller of this
//! surface. The catalog is `docs/tool-api.md`.

use serde::Serialize;
use serde_json::{Map, Number, Value};

use crate::document::Document;
use crate::error::KernelError;
use crate::palette_file::PaletteImportMode;

/// Version written on every response. An omitted request version means this one.
pub const TOOL_API_VERSION: u32 = 1;

/// Tool names [`Document::call_tool`] accepts, in catalog order.
///
/// `docs/tool-api.md` and `apps/web/src/kernel/toolApi.ts` list the same names.
pub const TOOL_NAMES: &[&str] = &[
    "inspect",
    "paint",
    "erase",
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
    let params = call.params.clone();
    match tool {
        "inspect" => {
            parse_empty(tool, params)?;
            Ok(inspect_value(document))
        }
        "paint" => {
            let (x, y, color_index) = parse_paint(params)?;
            document
                .paint(x, y, color_index)
                .map_err(ToolFailure::Kernel)?;
            Ok(empty())
        }
        "erase" => {
            let (x, y) = parse_cell(tool, params)?;
            document.erase(x, y).map_err(ToolFailure::Kernel)?;
            Ok(empty())
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
            parse_empty(tool, params)?;
            let frames = document
                .export_png_sequence()
                .into_iter()
                .map(|png| encode_base64(&png))
                .collect::<Vec<_>>();
            Ok(serde_json::json!({ "frames": frames }))
        }
        "exportSpriteSheet" => {
            parse_empty(tool, params)?;
            let layout = document.sprite_sheet_layout();
            let png = encode_base64(&document.export_sprite_sheet());
            Ok(serde_json::json!({
                "png": png,
                "columns": layout.columns,
                "rows": layout.rows,
                "sheetWidth": layout.sheet_width(),
                "sheetHeight": layout.sheet_height(),
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
    let palette = document
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
        .collect::<Vec<_>>();
    serde_json::json!({
        "width": document.width(),
        "height": document.height(),
        "palette": palette,
        "paletteLocked": document.palette_locked(),
        "frameCount": document.frame_count(),
        "activeFrame": active,
        "delayMs": delay_ms,
        "frameDelays": document.frame_delays(),
        "pixels": document.pixels(),
        "canUndo": document.can_undo(),
        "canRedo": document.can_redo(),
        "gestureOpen": document.gesture_open(),
    })
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

fn as_u32(index: usize) -> u32 {
    u32::try_from(index).expect("frame index fits in u32")
}

fn parse_empty(tool: &str, params: Value) -> Result<(), ToolFailure> {
    let map = expect_object(tool, params)?;
    reject_params(tool, &map, &[])?;
    Ok(())
}

fn parse_paint(params: Value) -> Result<(u16, u16, u8), ToolFailure> {
    let map = expect_object("paint", params)?;
    reject_params("paint", &map, &["x", "y", "colorIndex"])?;
    let (x, y) = cell_from("paint", &map)?;
    let color_index = require_u8("paint", &map, "colorIndex")?;
    Ok((x, y, color_index))
}

fn parse_cell(tool: &str, params: Value) -> Result<(u16, u16), ToolFailure> {
    let map = expect_object(tool, params)?;
    reject_params(tool, &map, &["x", "y"])?;
    cell_from(tool, &map)
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
    Ok(
        u16::try_from(require_int(tool, map, key, U16_MAX)?)
            .expect("value was bounded by u16::MAX"),
    )
}

fn require_u32(tool: &str, map: &Map<String, Value>, key: &str) -> Result<u32, ToolFailure> {
    Ok(
        u32::try_from(require_int(tool, map, key, U32_MAX)?)
            .expect("value was bounded by u32::MAX"),
    )
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
