//! Decode Tool API export payloads and write the PNG bytes.

use std::fs;
use std::path::Path;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde_json::{json, Value};

use crate::failure::CliFailure;
use crate::output::{print_line, tool_refusal};
use crate::session::Session;
use pixelforge_kernel::TOOL_API_VERSION;

pub fn export_png_sequence(session: &mut Session, out_dir: &Path, scale: u32) -> i32 {
    let request = serde_json::json!({
        "apiVersion": TOOL_API_VERSION,
        "tool": "exportPngSequence",
        "params": { "scale": scale },
    })
    .to_string();
    let response = session.call(&request);
    if let Some(code) = refuse_or_print(&response) {
        return code;
    }
    let value: Value = match serde_json::from_str(&response) {
        Ok(value) => value,
        Err(_) => {
            return crate::output::emit(CliFailure::new("Cli", "tool response is not JSON"));
        }
    };
    let Some(frames) = value["result"]["frames"].as_array() else {
        return crate::output::emit(CliFailure::new(
            "Cli",
            "exportPngSequence result is missing frames",
        ));
    };
    let mut decoded = Vec::with_capacity(frames.len());
    for (index, frame) in frames.iter().enumerate() {
        let Some(text) = frame.as_str() else {
            return crate::output::emit(CliFailure::new(
                "Cli",
                format!("exportPngSequence frame {index} is not a string"),
            ));
        };
        match STANDARD.decode(text) {
            Ok(bytes) => decoded.push(bytes),
            Err(_) => {
                return crate::output::emit(CliFailure::new(
                    "Cli",
                    format!("exportPngSequence frame {index} is not base64"),
                ));
            }
        }
    }
    if let Err(failure) = fs::create_dir_all(out_dir) {
        return crate::output::emit(CliFailure::io(out_dir, failure));
    }
    let mut written = Vec::with_capacity(decoded.len());
    for (index, bytes) in decoded.iter().enumerate() {
        let path = out_dir.join(format!("pixelforge-{index:03}.png"));
        if let Err(failure) = fs::write(&path, bytes) {
            return crate::output::emit(CliFailure::io(&path, failure));
        }
        eprintln!("pixelforge: wrote {}", path.display());
        let Some(path_text) = path.to_str() else {
            return crate::output::emit(CliFailure::new(
                "Cli",
                format!("path is not UTF-8: {}", path.display()),
            ));
        };
        written.push(json!({
            "path": path_text,
            "bytes": bytes.len(),
        }));
    }
    let summary = json!({
        "ok": true,
        "apiVersion": TOOL_API_VERSION,
        "tool": "exportPngSequence",
        "result": {
            "frames": written,
            "scale": value["result"]["scale"].clone(),
            "width": value["result"]["width"].clone(),
            "height": value["result"]["height"].clone(),
        },
    });
    finish_summary(&summary)
}

pub fn export_sprite_sheet(
    session: &mut Session,
    output: &Path,
    scale: u32,
    columns: Option<u32>,
    atlas: Option<&Path>,
) -> i32 {
    let mut params = serde_json::json!({ "scale": scale });
    if let Some(columns) = columns {
        params["columns"] = serde_json::json!(columns);
    }
    if atlas.is_some() {
        params["atlas"] = serde_json::json!(true);
    }
    let request = serde_json::json!({
        "apiVersion": TOOL_API_VERSION,
        "tool": "exportSpriteSheet",
        "params": params,
    })
    .to_string();
    let response = session.call(&request);
    if let Some(code) = refuse_or_print(&response) {
        return code;
    }
    let value: Value = match serde_json::from_str(&response) {
        Ok(value) => value,
        Err(_) => {
            return crate::output::emit(CliFailure::new("Cli", "tool response is not JSON"));
        }
    };
    let result = &value["result"];
    let Some(text) = result["png"].as_str() else {
        return crate::output::emit(CliFailure::new(
            "Cli",
            "exportSpriteSheet result is missing png",
        ));
    };
    let bytes = match STANDARD.decode(text) {
        Ok(bytes) => bytes,
        Err(_) => {
            return crate::output::emit(CliFailure::new(
                "Cli",
                "exportSpriteSheet png is not base64",
            ));
        }
    };
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(failure) = fs::create_dir_all(parent) {
                return crate::output::emit(CliFailure::io(parent, failure));
            }
        }
    }
    if let Err(failure) = fs::write(output, &bytes) {
        return crate::output::emit(CliFailure::io(output, failure));
    }
    eprintln!("pixelforge: wrote {}", output.display());
    let Some(path_text) = output.to_str() else {
        return crate::output::emit(CliFailure::new(
            "Cli",
            format!("path is not UTF-8: {}", output.display()),
        ));
    };
    let mut summary_result = json!({
        "path": path_text,
        "bytes": bytes.len(),
        "columns": result["columns"].clone(),
        "rows": result["rows"].clone(),
        "sheetWidth": result["sheetWidth"].clone(),
        "sheetHeight": result["sheetHeight"].clone(),
        "scale": result["scale"].clone(),
    });
    if let Some(atlas_path) = atlas {
        let Some(atlas_value) = result.get("atlas").filter(|value| value.is_object()) else {
            return crate::output::emit(CliFailure::new(
                "Cli",
                "exportSpriteSheet result is missing atlas",
            ));
        };
        let mut atlas_value = atlas_value.clone();
        if let Some(name) = output.file_name().and_then(|name| name.to_str()) {
            atlas_value["image"] = json!(name);
        }
        if let Err(failure) = write_atlas(atlas_path, &atlas_value) {
            return crate::output::emit(failure);
        }
        eprintln!("pixelforge: wrote {}", atlas_path.display());
        let Some(atlas_text) = atlas_path.to_str() else {
            return crate::output::emit(CliFailure::new(
                "Cli",
                format!("path is not UTF-8: {}", atlas_path.display()),
            ));
        };
        summary_result["atlas"] = json!(atlas_text);
    }
    let summary = json!({
        "ok": true,
        "apiVersion": TOOL_API_VERSION,
        "tool": "exportSpriteSheet",
        "result": summary_result,
    });
    finish_summary(&summary)
}

pub fn export_direction(session: &mut Session, facing: &str, output: &Path, scale: u32) -> i32 {
    let request = serde_json::json!({
        "apiVersion": TOOL_API_VERSION,
        "tool": "exportDirection",
        "params": { "facing": facing, "scale": scale },
    })
    .to_string();
    let response = session.call(&request);
    if let Some(code) = refuse_or_print(&response) {
        return code;
    }
    let value: Value = match serde_json::from_str(&response) {
        Ok(value) => value,
        Err(_) => {
            return crate::output::emit(CliFailure::new("Cli", "tool response is not JSON"));
        }
    };
    let result = &value["result"];
    let bytes = match write_png_b64(output, result["png"].as_str()) {
        Ok(bytes) => bytes,
        Err(failure) => return crate::output::emit(failure),
    };
    eprintln!("pixelforge: wrote {}", output.display());
    let Some(path_text) = output.to_str() else {
        return crate::output::emit(CliFailure::new(
            "Cli",
            format!("path is not UTF-8: {}", output.display()),
        ));
    };
    let summary = json!({
        "ok": true,
        "apiVersion": TOOL_API_VERSION,
        "tool": "exportDirection",
        "result": {
            "path": path_text,
            "bytes": bytes,
            "facing": result["facing"].clone(),
            "compass": result["compass"].clone(),
            "row": result["row"].clone(),
            "from": result["from"].clone(),
            "to": result["to"].clone(),
            "columns": result["columns"].clone(),
            "rows": result["rows"].clone(),
            "sheetWidth": result["sheetWidth"].clone(),
            "sheetHeight": result["sheetHeight"].clone(),
            "scale": result["scale"].clone(),
        },
    });
    finish_summary(&summary)
}

pub fn export_apng(
    session: &mut Session,
    output: &Path,
    scale: u32,
    tag: Option<&str>,
    from: Option<u32>,
    to: Option<u32>,
    loop_mode: Option<&str>,
) -> i32 {
    let mut params = serde_json::json!({ "scale": scale });
    if let Some(tag) = tag {
        params["tag"] = serde_json::json!(tag);
    }
    if let Some(from) = from {
        params["from"] = serde_json::json!(from);
    }
    if let Some(to) = to {
        params["to"] = serde_json::json!(to);
    }
    if let Some(loop_mode) = loop_mode {
        params["loop"] = serde_json::json!(loop_mode);
    }
    let request = serde_json::json!({
        "apiVersion": TOOL_API_VERSION,
        "tool": "exportApng",
        "params": params,
    })
    .to_string();
    let response = session.call(&request);
    if let Some(code) = refuse_or_print(&response) {
        return code;
    }
    let value: Value = match serde_json::from_str(&response) {
        Ok(value) => value,
        Err(_) => {
            return crate::output::emit(CliFailure::new("Cli", "tool response is not JSON"));
        }
    };
    let result = &value["result"];
    let Some(text) = result["apng"].as_str() else {
        return crate::output::emit(CliFailure::new("Cli", "exportApng result is missing apng"));
    };
    let bytes = match STANDARD.decode(text) {
        Ok(bytes) => bytes,
        Err(_) => {
            return crate::output::emit(CliFailure::new("Cli", "exportApng apng is not base64"));
        }
    };
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(failure) = fs::create_dir_all(parent) {
                return crate::output::emit(CliFailure::io(parent, failure));
            }
        }
    }
    if let Err(failure) = fs::write(output, &bytes) {
        return crate::output::emit(CliFailure::io(output, failure));
    }
    eprintln!("pixelforge: wrote {}", output.display());
    let Some(path_text) = output.to_str() else {
        return crate::output::emit(CliFailure::new(
            "Cli",
            format!("path is not UTF-8: {}", output.display()),
        ));
    };
    let mut summary_result = json!({
        "path": path_text,
        "bytes": bytes.len(),
        "format": result["format"].clone(),
        "width": result["width"].clone(),
        "height": result["height"].clone(),
        "scale": result["scale"].clone(),
        "loop": result["loop"].clone(),
        "plays": result["plays"].clone(),
        "frameCount": result["frameCount"].clone(),
        "frames": result["frames"].clone(),
    });
    if let Some(tag) = result.get("tag") {
        summary_result["tag"] = tag.clone();
    }
    let summary = json!({
        "ok": true,
        "apiVersion": TOOL_API_VERSION,
        "tool": "exportApng",
        "result": summary_result,
    });
    finish_summary(&summary)
}

fn write_atlas(path: &Path, atlas: &Value) -> Result<(), CliFailure> {
    let mut text = serde_json::to_string_pretty(atlas).expect("atlas is JSON");
    text.push('\n');
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|err| CliFailure::io(parent, err))?;
        }
    }
    fs::write(path, text).map_err(|err| CliFailure::io(path, err))
}

/// Writes a standard-base64 PNG. Returns the byte length.
pub fn write_png_b64(path: &Path, text: Option<&str>) -> Result<usize, CliFailure> {
    let Some(text) = text else {
        return Err(CliFailure::new("Cli", "tool result is missing png"));
    };
    let bytes = match STANDARD.decode(text) {
        Ok(bytes) => bytes,
        Err(_) => return Err(CliFailure::new("Cli", "png is not base64")),
    };
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|err| CliFailure::io(parent, err))?;
        }
    }
    fs::write(path, &bytes).map_err(|err| CliFailure::io(path, err))?;
    Ok(bytes.len())
}

fn refuse_or_print(response: &str) -> Option<i32> {
    let (code, message) = tool_refusal(response)?;
    if !print_line(response) {
        return Some(2);
    }
    eprintln!("pixelforge: export failed: {code}: {message}");
    Some(1)
}

fn finish_summary(summary: &Value) -> i32 {
    let line = serde_json::to_string(summary).expect("export summary is JSON");
    if print_line(&line) {
        0
    } else {
        2
    }
}
