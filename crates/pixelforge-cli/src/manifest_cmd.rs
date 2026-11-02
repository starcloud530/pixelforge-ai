//! `pixelforge project`: read and write a pixelforge-manifest.
//!
//! The manifest is project memory. It does not store pixels, and these
//! commands do not paint. Disk checks stay here; the kernel only compares the
//! snapshot this module collects.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use pixelforge_kernel::{
    ArtManifest, CanvasTier, LayoutRule, ManifestDraft, ManifestProbe, ProbeKind, MANIFEST_FORMAT,
    PROJECT_FORMAT, TILEMAP_FORMAT, TOOL_API_VERSION,
};
use serde_json::{json, Value};

use crate::cli::{PerspectiveArg, ProjectCmd};
use crate::failure::CliFailure;
use crate::output::{emit, print_line, tool_refusal};

/// Which sidecar `pixelforge call --project` should dispatch.
pub enum Sidecar {
    Manifest,
    Tilemap,
    Other,
}

pub fn peek_sidecar(path: &Path) -> Result<Sidecar, CliFailure> {
    let text = fs::read_to_string(path).map_err(|err| CliFailure::io(path, err))?;
    let value: Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(_) => return Ok(Sidecar::Other),
    };
    match value.get("format").and_then(Value::as_str) {
        Some(MANIFEST_FORMAT) => Ok(Sidecar::Manifest),
        Some(TILEMAP_FORMAT) => Ok(Sidecar::Tilemap),
        _ => Ok(Sidecar::Other),
    }
}

pub fn run(command: ProjectCmd) -> i32 {
    match command {
        ProjectCmd::Init {
            name,
            note,
            perspective,
            asset_root,
            palette,
            palette_ref,
            canvas,
            layout,
            output,
        } => init(
            &name,
            &note,
            perspective,
            &asset_root,
            &palette,
            palette_ref.as_deref(),
            &canvas,
            &layout,
            &output,
        ),
        ProjectCmd::Show { manifest } => show(&manifest),
        ProjectCmd::Validate { manifest } => validate(&manifest),
    }
}

pub fn apply_call(path: &Path, requests: Vec<String>, preview: bool) -> i32 {
    if preview {
        return emit(CliFailure::new(
            "Cli",
            "a project manifest has no pixels to preview",
        ));
    }
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => return emit(CliFailure::io(path, err)),
    };
    let mut manifest = match ArtManifest::load(&text) {
        Ok(manifest) => manifest,
        Err(error) => {
            return emit(CliFailure::new(error.code(), error.message()));
        }
    };
    let original = manifest.save();
    let mut status = 0;
    for request in &requests {
        let response = manifest.call_tool(request);
        if !print_line(&response) {
            return 2;
        }
        if let Some((code, message)) = tool_refusal(&response) {
            eprintln!("pixelforge: tool failed: {code}: {message}");
            status = 1;
            break;
        }
    }
    let updated = manifest.save();
    if updated != original {
        if let Err(err) = fs::write(path, updated) {
            return emit(CliFailure::io(path, err));
        }
        eprintln!("pixelforge: wrote {}", path.display());
    }
    status
}

fn init(
    name: &str,
    note: &str,
    perspective: PerspectiveArg,
    asset_root: &str,
    palette: &str,
    palette_ref: Option<&str>,
    canvas: &[String],
    layout: &[String],
    output: &Path,
) -> i32 {
    if output.exists() {
        return emit(CliFailure::new(
            "Cli",
            format!("manifest file already exists: {}", output.display()),
        ));
    }
    if canvas.is_empty() {
        return emit(CliFailure::new(
            "Cli",
            "project init needs at least one --canvas name:WxH",
        ));
    }
    if layout.is_empty() {
        return emit(CliFailure::new(
            "Cli",
            "project init needs at least one --layout name:dir:kind[:canvas]",
        ));
    }
    let canvases = match canvas.iter().map(|text| parse_canvas(text)).collect() {
        Ok(canvases) => canvases,
        Err(failure) => return emit(failure),
    };
    let rules = match layout.iter().map(|text| parse_layout(text)).collect() {
        Ok(rules) => rules,
        Err(failure) => return emit(failure),
    };
    let mut manifest = match ArtManifest::new(ManifestDraft {
        name: name.to_string(),
        note: note.to_string(),
        perspective: perspective.as_str().to_string(),
        asset_root: asset_root.to_string(),
        palette_path: palette.to_string(),
        palette_ref: palette_ref.map(str::to_string),
        canvases,
        layout: rules,
        assets: Vec::new(),
    }) {
        Ok(manifest) => manifest,
        Err(error) => return emit(CliFailure::new(error.code(), error.message())),
    };
    let text = manifest.save();
    if let Err(err) = fs::write(output, &text) {
        return emit(CliFailure::io(output, err));
    }
    let response = manifest.call_tool(r#"{"tool":"inspectManifest"}"#);
    if !print_line(&response) {
        return 2;
    }
    eprintln!("pixelforge: wrote {}", output.display());
    eprintln!("pixelforge: project manifest records conventions; it does not store pixels");
    0
}

fn show(path: &Path) -> i32 {
    let mut manifest = match load(path) {
        Ok(manifest) => manifest,
        Err(failure) => return emit(failure),
    };
    let response = manifest.call_tool(r#"{"tool":"inspectManifest"}"#);
    if print_line(&response) {
        0
    } else {
        2
    }
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
    let response = manifest.call_tool(
        &json!({
            "apiVersion": TOOL_API_VERSION,
            "tool": "validateManifest",
            "params": {
                "rootExists": root_exists,
                "files": files.iter().map(probe_json).collect::<Vec<_>>(),
            }
        })
        .to_string(),
    );
    if !print_line(&response) {
        return 2;
    }
    if let Some((code, message)) = tool_refusal(&response) {
        eprintln!("pixelforge: project validate failed: {code}: {message}");
        return 1;
    }
    let value: Value = match serde_json::from_str(&response) {
        Ok(value) => value,
        Err(_) => return emit(CliFailure::new("Cli", "tool response is not JSON")),
    };
    let errors = value["result"]["errorCount"].as_u64().unwrap_or(0);
    let warnings = value["result"]["warningCount"].as_u64().unwrap_or(0);
    if let Some(rows) = value["result"]["errors"].as_array() {
        for row in rows {
            eprintln!(
                "pixelforge: error {}: {}",
                row["code"].as_str().unwrap_or("manifest"),
                row["message"].as_str().unwrap_or("")
            );
        }
    }
    if let Some(rows) = value["result"]["warnings"].as_array() {
        for row in rows {
            eprintln!(
                "pixelforge: warning {}: {}",
                row["code"].as_str().unwrap_or("manifest"),
                row["message"].as_str().unwrap_or("")
            );
        }
    }
    if errors == 0 {
        if warnings == 0 {
            eprintln!("pixelforge: project validate: ok");
        } else {
            eprintln!("pixelforge: project validate: 0 errors, {warnings} warnings");
        }
        0
    } else {
        eprintln!("pixelforge: project validate: {errors} errors, {warnings} warnings");
        1
    }
}

pub(crate) fn load(path: &Path) -> Result<ArtManifest, CliFailure> {
    let text = fs::read_to_string(path).map_err(|err| CliFailure::io(path, err))?;
    ArtManifest::load(&text).map_err(|error| CliFailure::new(error.code(), error.message()))
}

pub(crate) fn gather(
    manifest: &ArtManifest,
    manifest_path: &Path,
) -> Result<(bool, Vec<ManifestProbe>), CliFailure> {
    let root = resolve_root(manifest_path, manifest.asset_root());
    let root_exists = root.is_dir();
    let mut probes = BTreeMap::<String, ManifestProbe>::new();
    if root_exists {
        insert_probe(&mut probes, observe(&root, manifest.palette_path(), true)?);
        for rule in manifest.layout() {
            insert_probe(&mut probes, observe(&root, &rule.dir, false)?);
            if rule.canvas.is_none() {
                continue;
            }
            let dir = root.join(&rule.dir);
            if !dir.is_dir() {
                continue;
            }
            for name in json_files(&dir)? {
                let rel = format!("{}/{}", rule.dir, name);
                if probes.contains_key(&rel) {
                    continue;
                }
                insert_probe(&mut probes, observe(&root, &rel, true)?);
            }
        }
        for asset in manifest.assets() {
            if probes.contains_key(&asset.path) {
                continue;
            }
            insert_probe(&mut probes, observe(&root, &asset.path, true)?);
        }
    }
    Ok((root_exists, probes.into_values().collect()))
}

fn insert_probe(probes: &mut BTreeMap<String, ManifestProbe>, probe: ManifestProbe) {
    probes.entry(probe.path.clone()).or_insert(probe);
}

fn observe(root: &Path, rel: &str, read_json: bool) -> Result<ManifestProbe, CliFailure> {
    let path = root.join(rel);
    if !path.exists() {
        return Ok(empty_probe(rel, ProbeKind::Missing));
    }
    if path.is_dir() {
        return Ok(empty_probe(rel, ProbeKind::Dir));
    }
    if !path.is_file() {
        return Ok(empty_probe(rel, ProbeKind::Missing));
    }
    if !read_json || !rel.to_ascii_lowercase().ends_with(".json") {
        return Ok(ManifestProbe {
            path: rel.to_string(),
            kind: ProbeKind::File,
            format: None,
            width: None,
            height: None,
            unreadable: false,
        });
    }
    let text = fs::read_to_string(&path).map_err(|err| CliFailure::io(&path, err))?;
    let value: Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(_) => return Ok(unreadable(rel)),
    };
    let Some(object) = value.as_object() else {
        return Ok(unreadable(rel));
    };
    let format = object
        .get("format")
        .and_then(Value::as_str)
        .map(str::to_string);
    let (width, height) = if format.as_deref() == Some(PROJECT_FORMAT) {
        (
            json_u16(object.get("width")),
            json_u16(object.get("height")),
        )
    } else {
        (None, None)
    };
    Ok(ManifestProbe {
        path: rel.to_string(),
        kind: ProbeKind::File,
        format,
        width,
        height,
        unreadable: false,
    })
}

fn json_files(dir: &Path) -> Result<Vec<String>, CliFailure> {
    let mut names = Vec::new();
    let entries = fs::read_dir(dir).map_err(|err| CliFailure::io(dir, err))?;
    for entry in entries {
        let entry = entry.map_err(|err| CliFailure::io(dir, err))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            return Err(CliFailure::new(
                "Cli",
                format!("file name is not UTF-8 in {}", dir.display()),
            ));
        };
        if name.to_ascii_lowercase().ends_with(".json") {
            names.push(name.to_string());
        }
    }
    names.sort();
    Ok(names)
}

fn empty_probe(path: &str, kind: ProbeKind) -> ManifestProbe {
    ManifestProbe {
        path: path.to_string(),
        kind,
        format: None,
        width: None,
        height: None,
        unreadable: false,
    }
}

fn unreadable(path: &str) -> ManifestProbe {
    ManifestProbe {
        path: path.to_string(),
        kind: ProbeKind::File,
        format: None,
        width: None,
        height: None,
        unreadable: true,
    }
}

fn json_u16(value: Option<&Value>) -> Option<u16> {
    let number = value?.as_u64()?;
    u16::try_from(number).ok()
}

pub(crate) fn probe_json(probe: &ManifestProbe) -> Value {
    let mut value = json!({
        "path": probe.path,
        "kind": match probe.kind {
            ProbeKind::File => "file",
            ProbeKind::Dir => "dir",
            ProbeKind::Missing => "missing",
        },
    });
    if probe.kind == ProbeKind::File {
        if let Some(format) = &probe.format {
            value["format"] = json!(format);
        }
        if let (Some(width), Some(height)) = (probe.width, probe.height) {
            value["width"] = json!(width);
            value["height"] = json!(height);
        }
        if probe.unreadable {
            value["unreadable"] = json!(true);
        }
    }
    value
}

pub(crate) fn asset_root_path(manifest_path: &Path, asset_root: &str) -> PathBuf {
    resolve_root(manifest_path, asset_root)
}

fn resolve_root(manifest_path: &Path, asset_root: &str) -> PathBuf {
    let parent = match manifest_path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.to_path_buf(),
        _ => PathBuf::from("."),
    };
    if asset_root == "." {
        parent
    } else {
        parent.join(asset_root)
    }
}

fn parse_canvas(text: &str) -> Result<CanvasTier, CliFailure> {
    let Some((name, size)) = text.split_once(':') else {
        return Err(canvas_flag(text));
    };
    let Some((width, height)) = size.split_once('x').or_else(|| size.split_once('X')) else {
        return Err(canvas_flag(text));
    };
    let width = width.parse::<u16>().map_err(|_| canvas_flag(text))?;
    let height = height.parse::<u16>().map_err(|_| canvas_flag(text))?;
    Ok(CanvasTier {
        name: name.to_string(),
        width,
        height,
    })
}

fn canvas_flag(text: &str) -> CliFailure {
    CliFailure::new(
        "Cli",
        format!("canvas {text} must be name:WxH, for example sprite:64x64"),
    )
}

fn parse_layout(text: &str) -> Result<LayoutRule, CliFailure> {
    let parts: Vec<&str> = text.split(':').collect();
    let (name, dir, kind, canvas) = match parts.as_slice() {
        [name, dir, kind] => (*name, *dir, *kind, None),
        [name, dir, kind, canvas] => (*name, *dir, *kind, Some((*canvas).to_string())),
        _ => {
            return Err(CliFailure::new(
                "Cli",
                format!("layout {text} must be name:dir:kind or name:dir:kind:canvas"),
            ))
        }
    };
    Ok(LayoutRule {
        name: name.to_string(),
        dir: dir.to_string(),
        kind: kind.to_string(),
        canvas,
    })
}
