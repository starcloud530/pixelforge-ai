//! Turn a blob of text into Tool API request strings.
//!
//! `--request` is passed through to [`pixelforge_kernel::Document::call_tool`].
//! A request file or stdin is one JSON value, a JSON array of values, or one
//! JSON value per line.

use serde_json::Value;

use crate::failure::CliFailure;

pub fn parse_request_blob(text: &str) -> Result<Vec<String>, CliFailure> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if text.trim().is_empty() {
        return Err(CliFailure::new(
            "Cli",
            "no Tool API request. Pass --request, --request-file, or pipe JSON on stdin",
        ));
    }
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Array(items)) => {
            if items.is_empty() {
                return Err(CliFailure::new("Cli", "Tool API request array is empty"));
            }
            items
                .iter()
                .map(|item| {
                    serde_json::to_string(item)
                        .map_err(|_| CliFailure::new("Cli", "Tool API request is not JSON"))
                })
                .collect()
        }
        Ok(value) => serde_json::to_string(&value)
            .map(|line| vec![line])
            .map_err(|_| CliFailure::new("Cli", "Tool API request is not JSON")),
        Err(_) => parse_jsonl(text),
    }
}

fn parse_jsonl(text: &str) -> Result<Vec<String>, CliFailure> {
    let mut requests = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line).map_err(|_| {
            CliFailure::new(
                "Cli",
                format!(
                    "request line {} is not a JSON value. Use one JSON object, a JSON array, or one JSON value per line",
                    index + 1
                ),
            )
        })?;
        let encoded = serde_json::to_string(&value)
            .map_err(|_| CliFailure::new("Cli", "Tool API request is not JSON"))?;
        requests.push(encoded);
    }
    if requests.is_empty() {
        return Err(CliFailure::new(
            "Cli",
            "no Tool API request. Pass --request, --request-file, or pipe JSON on stdin",
        ));
    }
    Ok(requests)
}

#[cfg(test)]
mod tests {
    use super::parse_request_blob;

    #[test]
    fn accepts_one_object_an_array_and_jsonl() {
        let one = parse_request_blob("{\"tool\":\"inspect\"}\n").unwrap();
        assert_eq!(one, vec!["{\"tool\":\"inspect\"}".to_string()]);

        let pretty = parse_request_blob("{\n  \"tool\": \"paint\"\n}\n").unwrap();
        assert_eq!(pretty.len(), 1);
        assert!(pretty[0].contains("paint"));

        let array = parse_request_blob("[{\"tool\":\"undo\"},{\"tool\":\"redo\"}]").unwrap();
        assert_eq!(array.len(), 2);

        let lines = parse_request_blob("{\"tool\":\"undo\"}\n\n{\"tool\":\"redo\"}\n").unwrap();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn rejects_an_empty_blob_and_a_broken_line() {
        assert_eq!(parse_request_blob(" \n").unwrap_err().code(), "Cli");
        assert_eq!(parse_request_blob("[]").unwrap_err().code(), "Cli");
        let broken = parse_request_blob("{\"tool\":\"undo\"}\nnot-json\n").unwrap_err();
        assert!(broken.message().contains("line 2"));
    }
}
