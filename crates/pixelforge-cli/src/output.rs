//! Stdout is compact JSON. Stderr is a short note for a person.

use serde_json::{json, Value};

use crate::failure::CliFailure;
use pixelforge_kernel::TOOL_API_VERSION;

pub fn emit(failure: CliFailure) -> i32 {
    let mut error = json!({
        "code": failure.code(),
        "message": failure.message(),
    });
    if let Some(hint) = failure.hint() {
        error["hint"] = json!(hint);
    }
    let body = json!({
        "ok": false,
        "apiVersion": TOOL_API_VERSION,
        "error": error,
    });
    let line = serde_json::to_string(&body).expect("cli error is JSON");
    if !print_line(&line) {
        return 2;
    }
    eprintln!("pixelforge: {}: {}", failure.code(), failure.message());
    if let Some(hint) = failure.hint() {
        eprintln!("pixelforge: hint: {hint}");
    }
    2
}

pub fn print_line(line: &str) -> bool {
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    writeln!(out, "{line}").is_ok()
}

/// `None` when the Tool API accepted the call.
pub fn tool_refusal(response: &str) -> Option<(String, String)> {
    let value: Value = match serde_json::from_str(response) {
        Ok(value) => value,
        Err(_) => {
            return Some(("Cli".to_string(), "tool response is not JSON".to_string()));
        }
    };
    if value.get("ok") == Some(&Value::Bool(true)) {
        return None;
    }
    let code = value["error"]["code"].as_str().unwrap_or("Cli").to_string();
    let message = value["error"]["message"]
        .as_str()
        .unwrap_or("tool call failed")
        .to_string();
    Some((code, message))
}

pub fn request_tool_name(request: &str) -> Option<String> {
    let value: Value = serde_json::from_str(request).ok()?;
    value.get("tool")?.as_str().map(str::to_string)
}
