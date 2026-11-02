//! `pixelforge asset`: registry records on a pixelforge-manifest.
//!
//! The commands do not paint. Disk probes stay here; the kernel compares the
//! snapshot this module collects and the ids stored in the file.

use std::path::Path;

use pixelforge_kernel::TOOL_API_VERSION;
use serde_json::{json, Value};

use crate::cli::AssetCmd;
use crate::failure::CliFailure;
use crate::manifest_cmd::{gather, load, probe_json};
use crate::output::{emit, print_line, tool_refusal};

pub fn run(command: AssetCmd) -> i32 {
    match command {
        AssetCmd::Add {
            manifest,
            id,
            kind,
            path,
            owner,
            depends_on,
            produces,
            tag,
        } => {
            let mut params = json!({
                "id": id,
                "kind": kind,
                "path": path,
            });
            if let Some(owner) = owner {
                params["owner"] = json!(owner);
            }
            if !depends_on.is_empty() {
                params["dependsOn"] = json!(depends_on);
            }
            if !produces.is_empty() {
                params["produces"] = json!(produces);
            }
            if !tag.is_empty() {
                params["tags"] = json!(tag);
            }
            mutate(&manifest, "addAsset", params)
        }
        AssetCmd::Set {
            manifest,
            id,
            kind,
            path,
            owner,
            clear_owner,
            depends_on,
            clear_depends_on,
            produces,
            clear_produces,
            tag,
            clear_tags,
        } => {
            if clear_owner && owner.is_some() {
                return emit(CliFailure::new(
                    "Cli",
                    "asset set takes --owner or --clear-owner, not both",
                ));
            }
            if clear_depends_on && !depends_on.is_empty() {
                return emit(CliFailure::new(
                    "Cli",
                    "asset set takes --depends-on or --clear-depends-on, not both",
                ));
            }
            if clear_produces && !produces.is_empty() {
                return emit(CliFailure::new(
                    "Cli",
                    "asset set takes --produces or --clear-produces, not both",
                ));
            }
            if clear_tags && !tag.is_empty() {
                return emit(CliFailure::new(
                    "Cli",
                    "asset set takes --tag or --clear-tags, not both",
                ));
            }
            let mut params = json!({ "id": id });
            if let Some(kind) = kind {
                params["kind"] = json!(kind);
            }
            if let Some(path) = path {
                params["path"] = json!(path);
            }
            if clear_owner {
                params["owner"] = Value::Null;
            } else if let Some(owner) = owner {
                params["owner"] = json!(owner);
            }
            if clear_depends_on {
                params["dependsOn"] = json!([]);
            } else if !depends_on.is_empty() {
                params["dependsOn"] = json!(depends_on);
            }
            if clear_produces {
                params["produces"] = json!([]);
            } else if !produces.is_empty() {
                params["produces"] = json!(produces);
            }
            if clear_tags {
                params["tags"] = json!([]);
            } else if !tag.is_empty() {
                params["tags"] = json!(tag);
            }
            mutate(&manifest, "setAsset", params)
        }
        AssetCmd::Remove { manifest, id } => mutate(&manifest, "removeAsset", json!({ "id": id })),
        AssetCmd::List {
            manifest,
            kind,
            owner,
            tag,
        } => {
            let mut params = json!({});
            if let Some(kind) = kind {
                params["kind"] = json!(kind);
            }
            if let Some(owner) = owner {
                params["owner"] = json!(owner);
            }
            if let Some(tag) = tag {
                params["tag"] = json!(tag);
            }
            query(&manifest, "listAssets", params)
        }
        AssetCmd::Show { manifest, id } => query(&manifest, "showAsset", json!({ "id": id })),
        AssetCmd::Graph { manifest, id } => query(&manifest, "graphAsset", json!({ "id": id })),
        AssetCmd::Dependents { manifest, id } => {
            query(&manifest, "dependentsAsset", json!({ "id": id }))
        }
        AssetCmd::Validate { manifest } => validate(&manifest),
    }
}

fn mutate(path: &Path, tool: &str, params: Value) -> i32 {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => return emit(CliFailure::io(path, err)),
    };
    let mut manifest = match pixelforge_kernel::ArtManifest::load(&text) {
        Ok(manifest) => manifest,
        Err(error) => return emit(CliFailure::new(error.code(), error.message())),
    };
    let original = manifest.save();
    let response = manifest.call_tool(&request(tool, params));
    let status = finish(&response, tool);
    if status == 2 {
        return 2;
    }
    let updated = manifest.save();
    if updated != original {
        if let Err(err) = std::fs::write(path, updated) {
            return emit(CliFailure::io(path, err));
        }
        eprintln!("pixelforge: wrote {}", path.display());
    }
    status
}

fn query(path: &Path, tool: &str, params: Value) -> i32 {
    let mut manifest = match load(path) {
        Ok(manifest) => manifest,
        Err(failure) => return emit(failure),
    };
    let response = manifest.call_tool(&request(tool, params));
    finish(&response, tool)
}

fn validate(path: &Path) -> i32 {
    let mut manifest = match load(path) {
        Ok(manifest) => manifest,
        Err(failure) => return emit(failure),
    };
    let (root_exists, files) = match gather(&manifest, path) {
        Ok(snapshot) => snapshot,
        Err(failure) => return emit(failure),
    };
    let response = manifest.call_tool(&request(
        "validateAssets",
        json!({
            "rootExists": root_exists,
            "files": files.iter().map(probe_json).collect::<Vec<_>>(),
        }),
    ));
    if !print_line(&response) {
        return 2;
    }
    if let Some((code, message)) = tool_refusal(&response) {
        eprintln!("pixelforge: asset validate failed: {code}: {message}");
        return 1;
    }
    let value: Value = match serde_json::from_str(&response) {
        Ok(value) => value,
        Err(_) => return emit(CliFailure::new("Cli", "tool response is not JSON")),
    };
    let errors = value["result"]["errorCount"].as_u64().unwrap_or(0);
    if let Some(rows) = value["result"]["errors"].as_array() {
        for row in rows {
            eprintln!(
                "pixelforge: error {}: {}",
                row["code"].as_str().unwrap_or("asset"),
                row["message"].as_str().unwrap_or("")
            );
        }
    }
    if errors == 0 {
        let count = value["result"]["assets"].as_u64().unwrap_or(0);
        eprintln!("pixelforge: asset validate: ok ({count} assets)");
        0
    } else {
        eprintln!("pixelforge: asset validate: {errors} errors");
        1
    }
}

fn finish(response: &str, tool: &str) -> i32 {
    if !print_line(response) {
        return 2;
    }
    if let Some((code, message)) = tool_refusal(response) {
        eprintln!("pixelforge: {tool} failed: {code}: {message}");
        return 1;
    }
    0
}

fn request(tool: &str, params: Value) -> String {
    json!({
        "apiVersion": TOOL_API_VERSION,
        "tool": tool,
        "params": params,
    })
    .to_string()
}
