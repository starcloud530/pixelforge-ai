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

pub fn export_sprite_sheet(session: &mut Session, output: &Path, scale: u32) -> i32 {
    let request = serde_json::json!({
        "apiVersion": TOOL_API_VERSION,
        "tool": "exportSpriteSheet",
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
    let summary = json!({
        "ok": true,
        "apiVersion": TOOL_API_VERSION,
        "tool": "exportSpriteSheet",
        "result": {
            "path": path_text,
            "bytes": bytes.len(),
            "columns": result["columns"].clone(),
            "rows": result["rows"].clone(),
            "sheetWidth": result["sheetWidth"].clone(),
            "sheetHeight": result["sheetHeight"].clone(),
            "scale": result["scale"].clone(),
        },
    });
    finish_summary(&summary)
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
