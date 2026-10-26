//! `validate-atlas`: checklist for an exported sheet. Does not edit pixels.

use std::fs;
use std::path::{Path, PathBuf};

use pixelforge_kernel::{
    decode_sheet_png, validate_atlas, AtlasMode, AtlasReport, AtlasRules, FourDirection, Pivot,
    TagPolicy, TOOL_API_VERSION,
};
use serde_json::{json, Value};

use crate::cli::{FourDirectionArg, TagPolicyArg, ValidateAtlasArgs};
use crate::failure::CliFailure;
use crate::output::{emit, print_line};

pub fn run(args: ValidateAtlasArgs) -> i32 {
    let rules = match load_rules(&args) {
        Ok(rules) => rules,
        Err(failure) => return emit(failure),
    };
    let atlas_text = match fs::read_to_string(&args.atlas) {
        Ok(text) => text,
        Err(err) => return emit(CliFailure::io(&args.atlas, err)),
    };
    let atlas: Value = match serde_json::from_str(&atlas_text) {
        Ok(value) => value,
        Err(_) => return emit(CliFailure::new("AtlasFile", "atlas is not JSON")),
    };
    let sheet_path = match resolve_sheet(&args.atlas, args.sheet.as_deref(), &atlas) {
        Ok(path) => path,
        Err(failure) => return emit(failure),
    };
    let png = match fs::read(&sheet_path) {
        Ok(bytes) => bytes,
        Err(err) => return emit(CliFailure::io(&sheet_path, err)),
    };
    let sheet = match decode_sheet_png(&png) {
        Ok(sheet) => sheet,
        Err(err) => return emit(CliFailure::new("PngFile", err.message())),
    };
    let file_name = match sheet_path.file_name().and_then(|name| name.to_str()) {
        Some(name) => name,
        None => {
            return emit(CliFailure::new(
                "Cli",
                format!("path is not UTF-8: {}", sheet_path.display()),
            ))
        }
    };
    let report = match validate_atlas(&atlas, &sheet, Some(file_name), &rules) {
        Ok(report) => report,
        Err(err) => return emit(CliFailure::new("AtlasFile", err.message())),
    };
    emit_report(&report)
}

fn load_rules(args: &ValidateAtlasArgs) -> Result<AtlasRules, CliFailure> {
    let mut rules = if let Some(path) = &args.rules {
        let text = fs::read_to_string(path).map_err(|err| CliFailure::io(path, err))?;
        AtlasRules::parse(&text).map_err(|err| CliFailure::new("AtlasRules", err.message()))?
    } else {
        AtlasRules::default()
    };
    if args.strict {
        rules.mode = AtlasMode::Strict;
    }
    if args.loose {
        rules.mode = AtlasMode::Loose;
    }
    if args.any_size {
        rules.sizes.clear();
    } else if !args.size.is_empty() {
        if args.size.iter().any(|edge| *edge == 0) {
            return Err(CliFailure::new("AtlasRules", "size must be at least 1"));
        }
        rules.sizes = args.size.clone();
    }
    if !args.require_tag.is_empty() {
        rules.require_tags = args.require_tag.clone();
    }
    if let Some(direction) = args.four_direction {
        rules.four_direction = match direction {
            FourDirectionArg::Auto => FourDirection::Auto,
            FourDirectionArg::On => FourDirection::On,
            FourDirectionArg::Off => FourDirection::Off,
        };
    }
    if let Some(policy) = args.tag_policy {
        rules.tag_policy = Some(match policy {
            TagPolicyArg::Required => TagPolicy::Required,
            TagPolicyArg::Optional => TagPolicy::Optional,
        });
    }
    if let (Some(x), Some(y)) = (args.pivot_x, args.pivot_y) {
        if !x.is_finite() || !y.is_finite() {
            return Err(CliFailure::new(
                "AtlasRules",
                "pivot must be a finite number",
            ));
        }
        rules.pivot = Some(Pivot { x, y });
    }
    Ok(rules)
}

fn resolve_sheet(
    atlas_path: &Path,
    sheet: Option<&Path>,
    atlas: &Value,
) -> Result<PathBuf, CliFailure> {
    if let Some(sheet) = sheet {
        return Ok(sheet.to_path_buf());
    }
    let Some(image) = atlas.get("image").and_then(|value| value.as_str()) else {
        return Err(CliFailure::new(
            "AtlasFile",
            "atlas has no image; pass --sheet",
        ));
    };
    if image.is_empty() {
        return Err(CliFailure::new(
            "AtlasFile",
            "atlas image must be a non-empty file name",
        ));
    }
    let image_path = Path::new(image);
    if image_path.is_absolute() {
        return Ok(image_path.to_path_buf());
    }
    Ok(atlas_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(image_path))
}

fn emit_report(report: &AtlasReport) -> i32 {
    let ok = report.ok();
    let body = json!({
        "ok": ok,
        "apiVersion": TOOL_API_VERSION,
        "tool": "validateAtlas",
        "result": report,
    });
    let line = serde_json::to_string(&body).expect("atlas report is JSON");
    if !print_line(&line) {
        return 2;
    }
    if ok && report.warnings.is_empty() {
        eprintln!("pixelforge: validate-atlas: ok");
    } else {
        eprintln!(
            "pixelforge: validate-atlas: {} errors, {} warnings",
            report.error_count, report.warning_count
        );
    }
    for finding in &report.errors {
        eprintln!("pixelforge: error {}: {}", finding.code, finding.message);
    }
    for finding in &report.warnings {
        eprintln!("pixelforge: warning {}: {}", finding.code, finding.message);
    }
    if ok {
        0
    } else {
        1
    }
}
