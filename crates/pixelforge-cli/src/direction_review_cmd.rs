//! `pixelforge direction review`. Read-only. The project file is not saved.

use std::fs;
use std::path::PathBuf;

use pixelforge_kernel::{
    DirectionReviewAlign, DirectionReviewAnchorName, DirectionReviewError, DirectionReviewFoot,
    DirectionReviewOptions, DirectionReviewOrigin, TOOL_API_VERSION,
};
use serde_json::{json, Value};

use crate::cli::{DirectionAlignArg, DirectionCmd};
use crate::failure::CliFailure;
use crate::output::{annotate_revision, emit, print_line};
use crate::session::{Access, Session};

pub fn run(command: DirectionCmd) -> i32 {
    let DirectionCmd::Review {
        doc,
        action,
        at,
        column,
        output,
        scale,
        tolerance,
        origin_x,
        origin_y,
        origin_part,
        foot_y,
        foot_anchor,
        anchor,
        align,
        align_anchor,
    } = command
    else {
        return emit(CliFailure::new(
            "Cli",
            "direction review was not the command",
        ));
    };
    let options = match options_from(
        action,
        at,
        column,
        scale,
        tolerance,
        origin_x,
        origin_y,
        origin_part,
        foot_y,
        foot_anchor,
        anchor,
        align,
        align_anchor,
    ) {
        Ok(options) => options,
        Err(failure) => return emit(failure),
    };
    let session = match Session::open_with(
        &doc.project,
        doc.checkpoint.as_deref(),
        doc.expected_revision.as_deref(),
        Access::ReadOnly,
    ) {
        Ok(session) => session,
        Err(failure) => return emit(failure),
    };
    let review = match session.document().review_directions(&options) {
        Ok(review) => review,
        Err(DirectionReviewError::Action) => {
            return emit(CliFailure::new(
                "DirectionReview",
                "action must be one token of letters and digits, such as idle, walk, or attack",
            ));
        }
        Err(DirectionReviewError::Scale { scale }) => {
            return emit(CliFailure::new(
                "DirectionReview",
                format!("scale {scale} must be an integer >= 1, and the review image must fit"),
            ));
        }
    };
    let revision = session.revision();
    match write_pack(&output, &review, &revision) {
        Ok(files) => emit_report(&review.report, files, &revision),
        Err(failure) => emit(failure),
    }
}

fn options_from(
    action: String,
    at: Option<u32>,
    column: Option<u32>,
    scale: u32,
    tolerance: u32,
    origin_x: Option<i32>,
    origin_y: Option<i32>,
    origin_part: Option<String>,
    foot_y: Option<i32>,
    foot_anchor: Option<String>,
    anchor: Vec<String>,
    align: Option<DirectionAlignArg>,
    align_anchor: Option<String>,
) -> Result<DirectionReviewOptions, CliFailure> {
    let mut anchors = Vec::new();
    for text in &anchor {
        let (part, name) = parse_part_anchor(text)?;
        anchors.push(DirectionReviewAnchorName { part, name });
    }
    let align = match align {
        None => DirectionReviewAlign::Auto,
        Some(DirectionAlignArg::Origin) => {
            if origin_x.is_none() && origin_part.is_none() {
                return Err(CliFailure::new(
                    "DirectionReview",
                    "--align origin needs --origin-x/--origin-y or --origin-part",
                ));
            }
            DirectionReviewAlign::Origin
        }
        Some(DirectionAlignArg::Foot) => {
            if foot_y.is_none() && foot_anchor.is_none() {
                return Err(CliFailure::new(
                    "DirectionReview",
                    "--align foot needs --foot-y or --foot-anchor",
                ));
            }
            DirectionReviewAlign::Foot
        }
        Some(DirectionAlignArg::Anchor) => {
            let text = align_anchor
                .or_else(|| anchor.first().cloned())
                .ok_or_else(|| {
                    CliFailure::new(
                        "DirectionReview",
                        "--align anchor needs --align-anchor or --anchor",
                    )
                })?;
            let (part, name) = parse_part_anchor(&text)?;
            if !anchors
                .iter()
                .any(|item| item.part == part && item.name == name)
            {
                anchors.push(DirectionReviewAnchorName {
                    part: part.clone(),
                    name: name.clone(),
                });
            }
            DirectionReviewAlign::Anchor { part, name }
        }
    };
    let origin = match (origin_x, origin_y, origin_part) {
        (Some(x), Some(y), _) => DirectionReviewOrigin::User { x, y },
        (_, _, Some(part)) => DirectionReviewOrigin::Part(part),
        _ => DirectionReviewOrigin::Unset,
    };
    let foot = match (foot_y, foot_anchor) {
        (Some(y), _) => DirectionReviewFoot::UserY(y),
        (_, Some(text)) => {
            let (part, name) = parse_part_anchor(&text)?;
            DirectionReviewFoot::Anchor { part, name }
        }
        _ => DirectionReviewFoot::Unset,
    };
    Ok(DirectionReviewOptions {
        action,
        at: at.unwrap_or(0),
        column,
        scale,
        tolerance,
        origin,
        foot,
        anchors,
        align,
    })
}

fn parse_part_anchor(text: &str) -> Result<(String, String), CliFailure> {
    let Some((part, name)) = text.split_once(':') else {
        return Err(CliFailure::new(
            "DirectionReview",
            format!("expected part:name, got `{text}`"),
        ));
    };
    if part.trim().is_empty() || name.trim().is_empty() || name.contains(':') {
        return Err(CliFailure::new(
            "DirectionReview",
            format!("expected part:name, got `{text}`"),
        ));
    }
    Ok((part.trim().to_string(), name.trim().to_string()))
}

fn write_pack(
    output: &PathBuf,
    review: &pixelforge_kernel::DirectionReview,
    revision: &str,
) -> Result<Value, CliFailure> {
    fs::create_dir_all(output).map_err(|err| CliFailure::io(output, err))?;
    let positions = output.join("positions.png");
    let json_path = output.join("review.json");
    fs::write(&positions, &review.positions_png).map_err(|err| CliFailure::io(&positions, err))?;
    eprintln!("pixelforge: wrote {}", positions.display());
    let aligned_path = if let Some(bytes) = &review.aligned_png {
        let path = output.join("aligned.png");
        fs::write(&path, bytes).map_err(|err| CliFailure::io(&path, err))?;
        eprintln!("pixelforge: wrote {}", path.display());
        Some(path)
    } else {
        None
    };
    if review.overlay_pngs.len() != review.report.overlays.len() {
        return Err(CliFailure::new(
            "Cli",
            "direction review overlay count does not match the report",
        ));
    }
    let mut overlays = Vec::new();
    for (meta, bytes) in review
        .report
        .overlays
        .iter()
        .zip(review.overlay_pngs.iter())
    {
        let path = output.join(&meta.file);
        fs::write(&path, bytes).map_err(|err| CliFailure::io(&path, err))?;
        eprintln!("pixelforge: wrote {}", path.display());
        overlays.push(path_value(&path)?);
    }
    let mut files = json!({
        "positions": path_value(&positions)?,
        "overlays": overlays,
        "review": path_value(&json_path)?,
    });
    if let Some(path) = &aligned_path {
        files["aligned"] = path_value(path)?;
    } else {
        files["aligned"] = Value::Null;
    }
    let mut result = serde_json::to_value(&review.report).expect("direction review report is JSON");
    result["files"] = files.clone();
    result["revision"] = json!(revision);
    let mut text = serde_json::to_string_pretty(&result).expect("direction review report is JSON");
    text.push('\n');
    fs::write(&json_path, text).map_err(|err| CliFailure::io(&json_path, err))?;
    eprintln!("pixelforge: wrote {}", json_path.display());
    Ok(files)
}

fn path_value(path: &std::path::Path) -> Result<Value, CliFailure> {
    let Some(text) = path.to_str() else {
        return Err(CliFailure::new(
            "Cli",
            format!("path is not UTF-8: {}", path.display()),
        ));
    };
    Ok(json!(text))
}

fn emit_report(
    report: &pixelforge_kernel::DirectionReviewReport,
    files: Value,
    revision: &str,
) -> i32 {
    let mut result = serde_json::to_value(report).expect("direction review report is JSON");
    result["files"] = files;
    let body = json!({
        "ok": true,
        "apiVersion": TOOL_API_VERSION,
        "tool": "directionReview",
        "result": result,
    });
    let line = serde_json::to_string(&body).expect("direction review is JSON");
    let line = annotate_revision(&line, revision);
    if !print_line(&line) {
        return 2;
    }
    eprintln!(
        "pixelforge: direction review: {} structural, {} alignment, {} observation",
        report.structural_count, report.alignment_count, report.observation_count
    );
    for item in &report.findings {
        eprintln!(
            "pixelforge: {} {}: {}",
            item.severity, item.code, item.message
        );
    }
    eprintln!("pixelforge: direction review does not judge the drawing");
    if report.error_count == 0 {
        0
    } else {
        1
    }
}
