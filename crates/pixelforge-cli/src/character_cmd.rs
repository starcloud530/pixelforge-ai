//! `pixelforge character`: facing × action matrix for a registered character.
//!
//! Opens the character's `pixelforge-project`, builds a probe, and calls the
//! manifest Tool API `characterMatrix`. Does not paint. A green check means
//! tags and ink cover the expected cells; it is not a gait or art pass.

use std::fs;
use std::path::Path;

use pixelforge_kernel::{Document, TOOL_API_VERSION};
use serde_json::{json, Value};

use crate::cli::CharacterCmd;
use crate::failure::CliFailure;
use crate::manifest_cmd::{self, load};
use crate::output::{emit, print_line, tool_refusal};

pub fn run(command: CharacterCmd) -> i32 {
    match command {
        CharacterCmd::Matrix { manifest, id } => matrix_or_check(&manifest, &id, false),
        CharacterCmd::Check { manifest, id } => matrix_or_check(&manifest, &id, true),
    }
}

fn matrix_or_check(manifest_path: &Path, id: &str, check: bool) -> i32 {
    let mut manifest = match load(manifest_path) {
        Ok(manifest) => manifest,
        Err(failure) => return emit(failure),
    };
    let owner = match resolve_character_path(&manifest, id) {
        Ok(owner) => owner,
        Err(failure) => return emit(failure),
    };
    let root = manifest_cmd::asset_root_path(manifest_path, manifest.asset_root());
    let project_path = root.join(&owner.path);
    let text = match fs::read_to_string(&project_path) {
        Ok(text) => text,
        Err(err) => {
            return emit(CliFailure::new(
                "Cli",
                format!(
                    "cannot read character project {}: {err}",
                    project_path.display()
                ),
            ))
        }
    };
    let document = match Document::load_project(&text) {
        Ok(document) => document,
        Err(err) => {
            return emit(CliFailure::new(
                err.code(),
                format!("{}: {}", project_path.display(), err),
            ))
        }
    };
    let project = project_probe(&document);
    let missing_paths = missing_gear_paths(&manifest, &owner.id, &root);
    let response = manifest.call_tool(
        &json!({
            "apiVersion": TOOL_API_VERSION,
            "tool": "characterMatrix",
            "params": {
                "id": id,
                "project": project,
                "missingPaths": missing_paths,
            },
        })
        .to_string(),
    );
    if !print_line(&response) {
        return 2;
    }
    if let Some((code, message)) = tool_refusal(&response) {
        eprintln!("pixelforge: characterMatrix failed: {code}: {message}");
        return 1;
    }
    let value: Value = match serde_json::from_str(&response) {
        Ok(value) => value,
        Err(_) => return emit(CliFailure::new("Cli", "tool response is not JSON")),
    };
    let result = &value["result"];
    let error_count = result["errorCount"].as_u64().unwrap_or(0);
    let warning_count = result["warningCount"].as_u64().unwrap_or(0);
    let gaps = result["gaps"].as_array().map(|list| list.len()).unwrap_or(0);
    print_human_summary(result, check);
    if !check {
        eprintln!(
            "pixelforge: character matrix: {gaps} gaps, {error_count} errors, {warning_count} warnings"
        );
        return 0;
    }
    if error_count == 0 {
        eprintln!("pixelforge: character check: ok");
        if warning_count > 0 {
            eprintln!(
                "pixelforge: character check: {warning_count} warnings (placeholders are not new art)"
            );
        }
        0
    } else {
        if let Some(errors) = result["errors"].as_array() {
            for error in errors {
                let code = error["code"].as_str().unwrap_or("matrix");
                let message = error["message"].as_str().unwrap_or("");
                eprintln!("pixelforge: error {code}: {message}");
            }
        }
        eprintln!("pixelforge: character check: {error_count} errors");
        1
    }
}

struct OwnerPath {
    id: String,
    path: String,
}

fn resolve_character_path(
    manifest: &pixelforge_kernel::ArtManifest,
    id: &str,
) -> Result<OwnerPath, CliFailure> {
    let asset = manifest
        .assets()
        .iter()
        .find(|asset| asset.id == id)
        .ok_or_else(|| CliFailure::new("Cli", format!("unknown asset id `{id}`")))?;
    if asset.kind == "character" || asset.kind == "group" {
        return Ok(OwnerPath {
            id: asset.id.clone(),
            path: asset.path.clone(),
        });
    }
    let owner_id = asset.owner.as_deref().ok_or_else(|| {
        CliFailure::new(
            "Cli",
            format!("asset `{id}` is not a character or group and has no owner"),
        )
    })?;
    let owner = manifest
        .assets()
        .iter()
        .find(|asset| asset.id == owner_id)
        .ok_or_else(|| {
            CliFailure::new(
                "Cli",
                format!("owner `{owner_id}` for `{id}` is not registered"),
            )
        })?;
    if owner.kind != "character" && owner.kind != "group" {
        return Err(CliFailure::new(
            "Cli",
            format!("owner `{owner_id}` must be a character or group"),
        ));
    }
    Ok(OwnerPath {
        id: owner.id.clone(),
        path: owner.path.clone(),
    })
}

fn project_probe(document: &Document) -> Value {
    let tags: Vec<Value> = document
        .tags()
        .iter()
        .map(|tag| {
            json!({
                "name": tag.name,
                "from": tag.from,
                "to": tag.to,
                "loop": tag.loop_mode.as_str(),
            })
        })
        .collect();
    let frames: Vec<Value> = (0..document.frame_count())
        .map(|index| {
            let has_content = document.frame_has_content(index);
            let hash = document.frame_content_hash(index, false);
            let hash_hflip = document.frame_content_hash(index, true);
            json!({
                "hasContent": has_content,
                "hash": format!("0x{hash:016x}"),
                "hashHflip": format!("0x{hash_hflip:016x}"),
            })
        })
        .collect();
    let direction_layout = match document.direction_layout() {
        Some(layout) => json!({
            "count": layout.count,
            "columns": layout.columns,
        }),
        None => Value::Null,
    };
    json!({
        "frameCount": document.frame_count(),
        "directionLayout": direction_layout,
        "tags": tags,
        "frames": frames,
    })
}

fn missing_gear_paths(
    manifest: &pixelforge_kernel::ArtManifest,
    owner_id: &str,
    root: &Path,
) -> Vec<String> {
    manifest
        .assets()
        .iter()
        .filter(|asset| asset.kind == "gear" && asset.owner.as_deref() == Some(owner_id))
        .filter(|asset| !root.join(&asset.path).is_file())
        .map(|asset| asset.path.clone())
        .collect()
}

fn print_human_summary(result: &Value, check: bool) {
    let id = result["id"].as_str().unwrap_or("?");
    let actions = result["actions"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    let directions = result["directions"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    let label = if check { "check" } else { "matrix" };
    eprintln!("pixelforge: character {label}: {id} actions=[{actions}] directions=[{directions}]");
    if let Some(gaps) = result["gaps"].as_array() {
        for gap in gaps {
            let action = gap["action"].as_str().unwrap_or("?");
            let facing = gap["facing"].as_str().unwrap_or("?");
            let status = gap["status"].as_str().unwrap_or("?");
            eprintln!("pixelforge: gap {action}_{facing}: {status}");
        }
    }
}
