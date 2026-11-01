//! Game-art project manifest. Not a canvas and not an asset registry.
//!
//! A `pixelforge-manifest` file records production conventions an agent can
//! read back: canvas tiers, perspective, directory rules, the default palette
//! path, a short note, and the asset root. Pixels stay in `pixelforge-project`
//! files. This module does not list individual assets, animation matrices, or
//! export revisions, and it does not draw.

use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::project::PROJECT_FORMAT;
use crate::MAX_CANVAS_EDGE;
use crate::TOOL_API_VERSION;

/// `format` of a project-manifest file.
pub const MANIFEST_FORMAT: &str = "pixelforge-manifest";

/// Tool names [`ArtManifest::call_tool`] accepts. They are not pixel tools.
pub const MANIFEST_TOOL_NAMES: &[&str] = &["inspectManifest", "validateManifest"];

/// Closed set for `perspective`.
pub const PERSPECTIVES: &[&str] = &["top", "side", "front", "oblique"];

/// Closed set for a layout rule's `kind`. A kind is a directory role, not an asset id.
pub const LAYOUT_KINDS: &[&str] = &["character", "prop", "tile", "portrait", "ui", "effect"];

const MANIFEST_VERSION: u64 = 1;
const MAX_NAME_CHARS: usize = 64;
const MAX_NOTE_CHARS: usize = 400;
const MAX_PATH_CHARS: usize = 256;
const MAX_CANVAS_TIERS: usize = 32;
const MAX_LAYOUT_RULES: usize = 64;
const MAX_PROBES: usize = 1024;

/// Why a manifest file could not be read. The checklist did not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestError {
    message: String,
}

impl ManifestError {
    pub fn code(&self) -> &'static str {
        "ManifestFile"
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ManifestError {}

fn file_err(message: impl Into<String>) -> ManifestError {
    ManifestError {
        message: message.into(),
    }
}

/// One allowed canvas size. `name` is what layout rules point at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanvasTier {
    pub name: String,
    pub width: u16,
    pub height: u16,
}

/// A directory convention under the asset root. Not a list of files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutRule {
    pub name: String,
    pub dir: String,
    pub kind: String,
    pub canvas: Option<String>,
}

/// Fields [`ArtManifest::new`] checks before it writes a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestDraft {
    pub name: String,
    pub note: String,
    pub perspective: String,
    pub asset_root: String,
    pub palette_path: String,
    pub palette_ref: Option<String>,
    pub canvases: Vec<CanvasTier>,
    pub layout: Vec<LayoutRule>,
}

/// What the CLI observed on disk. Paths are relative to the asset root and use `/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestProbe {
    pub path: String,
    pub kind: ProbeKind,
    pub format: Option<String>,
    pub width: Option<u16>,
    pub height: Option<u16>,
    /// The path exists as a file but is not a JSON object.
    pub unreadable: bool,
}

/// Whether a relative path exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeKind {
    File,
    Dir,
    Missing,
}

impl ProbeKind {
    fn parse(text: &str) -> Option<Self> {
        match text {
            "file" => Some(Self::File),
            "dir" => Some(Self::Dir),
            "missing" => Some(Self::Missing),
            _ => None,
        }
    }
}

/// One checklist row. `path` is relative to the asset root when the row has one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManifestFinding {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// Declared conventions plus how many project files each layout rule covered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestChecks {
    pub perspective: String,
    pub asset_root: String,
    pub palette: String,
    pub canvases: Vec<CanvasCheck>,
    pub layout: Vec<LayoutCheck>,
}

/// A canvas tier, echoed so a caller can see the gate without re-reading the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CanvasCheck {
    pub name: String,
    pub width: u16,
    pub height: u16,
}

/// One layout rule and the count of `pixelforge-project` files checked in that directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LayoutCheck {
    pub name: String,
    pub dir: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canvas: Option<String>,
    pub projects: u32,
}

/// Checklist for one manifest. Errors fail the gate. Warnings do not.
///
/// `projects` counts files. It does not list them. This report is not an asset index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestReport {
    pub errors: Vec<ManifestFinding>,
    pub warnings: Vec<ManifestFinding>,
    pub error_count: usize,
    pub warning_count: usize,
    pub checks: ManifestChecks,
}

/// Project memory: conventions for a game-art directory. No pixels and no undo stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtManifest {
    name: String,
    note: String,
    perspective: String,
    asset_root: String,
    palette_path: String,
    palette_ref: Option<String>,
    canvases: Vec<CanvasTier>,
    layout: Vec<LayoutRule>,
}

impl ArtManifest {
    /// Checks a draft and builds a manifest. Does not touch the disk.
    pub fn new(draft: ManifestDraft) -> Result<Self, ManifestError> {
        build(draft)
    }

    /// Reads version 1 `pixelforge-manifest` JSON. Unknown fields are refused.
    pub fn load(json: &str) -> Result<Self, ManifestError> {
        let json = json.strip_prefix('\u{feff}').unwrap_or(json);
        let value: Value =
            serde_json::from_str(json).map_err(|_| file_err("manifest file is not JSON"))?;
        let map = value
            .as_object()
            .ok_or_else(|| file_err("manifest file must be a JSON object"))?;
        reject_unknown(
            map,
            &[
                "format",
                "version",
                "name",
                "note",
                "perspective",
                "assetRoot",
                "palette",
                "canvases",
                "layout",
            ],
            "manifest",
        )?;
        let format = require_string(map, "format", "manifest format")?;
        if format != MANIFEST_FORMAT {
            return Err(file_err(format!(
                "manifest format must be {MANIFEST_FORMAT}, got {format}"
            )));
        }
        match map.get("version") {
            Some(version) if version.as_u64() == Some(MANIFEST_VERSION) => {}
            Some(version) => {
                return Err(file_err(format!(
                    "manifest version must be 1, got {version}"
                )));
            }
            None => return Err(file_err("manifest version is missing")),
        }
        let palette = require_object(map, "palette", "manifest palette")?;
        reject_unknown(palette, &["path", "ref"], "manifest palette")?;
        let palette_ref = match palette.get("ref") {
            None => None,
            Some(value) => Some(parse_string(value, "manifest palette ref")?),
        };
        let canvases = parse_canvases(map.get("canvases"))?;
        let layout = parse_layout(map.get("layout"))?;
        build(ManifestDraft {
            name: require_string(map, "name", "manifest name")?,
            note: require_string(map, "note", "manifest note")?,
            perspective: require_string(map, "perspective", "manifest perspective")?,
            asset_root: require_string(map, "assetRoot", "manifest assetRoot")?,
            palette_path: require_string(palette, "path", "manifest palette path")?,
            palette_ref,
            canvases,
            layout,
        })
    }

    /// Pretty JSON, including a trailing newline. Key order follows the JSON map.
    pub fn save(&self) -> String {
        let mut json = serde_json::to_string_pretty(&self.snapshot()).expect("manifest is JSON");
        json.push('\n');
        json
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn note(&self) -> &str {
        &self.note
    }

    pub fn perspective(&self) -> &str {
        &self.perspective
    }

    pub fn asset_root(&self) -> &str {
        &self.asset_root
    }

    pub fn palette_path(&self) -> &str {
        &self.palette_path
    }

    pub fn palette_ref(&self) -> Option<&str> {
        self.palette_ref.as_deref()
    }

    pub fn canvases(&self) -> &[CanvasTier] {
        &self.canvases
    }

    pub fn layout(&self) -> &[LayoutRule] {
        &self.layout
    }

    /// The manifest object `inspectManifest` returns.
    pub fn snapshot(&self) -> Value {
        let mut palette = Map::new();
        palette.insert("path".to_string(), json!(self.palette_path));
        if let Some(name) = &self.palette_ref {
            palette.insert("ref".to_string(), json!(name));
        }
        let canvases = self
            .canvases
            .iter()
            .map(|tier| {
                json!({
                    "name": tier.name,
                    "width": tier.width,
                    "height": tier.height,
                })
            })
            .collect::<Vec<_>>();
        let layout = self
            .layout
            .iter()
            .map(|rule| {
                let mut item = Map::new();
                if let Some(canvas) = &rule.canvas {
                    item.insert("canvas".to_string(), json!(canvas));
                }
                item.insert("dir".to_string(), json!(rule.dir));
                item.insert("kind".to_string(), json!(rule.kind));
                item.insert("name".to_string(), json!(rule.name));
                Value::Object(item)
            })
            .collect::<Vec<_>>();
        json!({
            "format": MANIFEST_FORMAT,
            "version": MANIFEST_VERSION,
            "name": self.name,
            "note": self.note,
            "perspective": self.perspective,
            "assetRoot": self.asset_root,
            "palette": palette,
            "canvases": canvases,
            "layout": layout,
        })
    }

    /// Compare declared conventions with a disk snapshot the caller already took.
    ///
    /// This does not read the filesystem. A path with no probe is `probeMissing`,
    /// so a caller cannot skip the disk and still pass. An empty layout directory
    /// that names a canvas is a warning (`layoutEmpty`), not an error.
    /// Atlas JSON and other non-project files in that directory are ignored.
    /// The report counts project files and does not list them.
    pub fn validate(&self, root_exists: bool, files: &[ManifestProbe]) -> ManifestReport {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut layout_counts = vec![0u32; self.layout.len()];

        if let Some(path) = duplicate_probe(files) {
            push_finding(
                &mut errors,
                "probeConflict",
                format!("probe path {path} is repeated"),
                Some(path),
            );
        } else if !root_exists {
            push_finding(
                &mut errors,
                "assetRoot",
                format!("asset root {} is not a directory", self.asset_root),
                Some(&self.asset_root),
            );
        } else {
            self.check_palette(files, &mut errors);
            for (index, rule) in self.layout.iter().enumerate() {
                layout_counts[index] = self.check_layout(rule, files, &mut errors, &mut warnings);
            }
        }

        let checks = ManifestChecks {
            perspective: self.perspective.clone(),
            asset_root: self.asset_root.clone(),
            palette: self.palette_path.clone(),
            canvases: self
                .canvases
                .iter()
                .map(|tier| CanvasCheck {
                    name: tier.name.clone(),
                    width: tier.width,
                    height: tier.height,
                })
                .collect(),
            layout: self
                .layout
                .iter()
                .zip(layout_counts)
                .map(|(rule, projects)| LayoutCheck {
                    name: rule.name.clone(),
                    dir: rule.dir.clone(),
                    kind: rule.kind.clone(),
                    canvas: rule.canvas.clone(),
                    projects,
                })
                .collect(),
        };
        let error_count = errors.len();
        let warning_count = warnings.len();
        ManifestReport {
            errors,
            warnings,
            error_count,
            warning_count,
            checks,
        }
    }

    /// Runs one manifest Tool API request. The envelope matches `Document::call_tool`.
    ///
    /// The file has no undo stack. `validateManifest` does not read the disk; pass
    /// `rootExists` and `files` from the caller.
    #[must_use = "tool errors are reported in the JSON body"]
    pub fn call_tool(&self, request: &str) -> String {
        let request = request.strip_prefix('\u{feff}').unwrap_or(request);
        let (id, outcome) = match parse_request(request) {
            Ok(call) => {
                let id = call.id.clone();
                (id, dispatch(self, &call))
            }
            Err(error) => (error.id, Err(error.failure)),
        };
        match outcome {
            Ok(result) => encode_ok(id, result),
            Err(failure) => encode_err(id, failure),
        }
    }

    fn check_palette(&self, files: &[ManifestProbe], errors: &mut Vec<ManifestFinding>) {
        let Some(probe) = files.iter().find(|probe| probe.path == self.palette_path) else {
            push_finding(
                errors,
                "probeMissing",
                format!("no probe for palette {}", self.palette_path),
                Some(&self.palette_path),
            );
            return;
        };
        match probe.kind {
            ProbeKind::Missing => push_finding(
                errors,
                "paletteMissing",
                format!("palette {} is missing", self.palette_path),
                Some(&self.palette_path),
            ),
            ProbeKind::Dir => push_finding(
                errors,
                "paletteFormat",
                format!("palette {} is a directory", self.palette_path),
                Some(&self.palette_path),
            ),
            ProbeKind::File => self.check_palette_file(probe, errors),
        }
    }

    fn check_palette_file(&self, probe: &ManifestProbe, errors: &mut Vec<ManifestFinding>) {
        match palette_suffix(&self.palette_path) {
            PaletteSuffix::Text => {}
            PaletteSuffix::Json => {
                if probe.unreadable {
                    push_finding(
                        errors,
                        "paletteFormat",
                        format!("palette {} is not a JSON object", self.palette_path),
                        Some(&self.palette_path),
                    );
                    return;
                }
                match probe.format.as_deref() {
                    Some(format) if format == PROJECT_FORMAT => {}
                    Some(format) => push_finding(
                        errors,
                        "paletteFormat",
                        format!(
                            "palette {} format is {format}, expected {PROJECT_FORMAT}",
                            self.palette_path
                        ),
                        Some(&self.palette_path),
                    ),
                    None => push_finding(
                        errors,
                        "paletteFormat",
                        format!(
                            "palette {} has no {PROJECT_FORMAT} format",
                            self.palette_path
                        ),
                        Some(&self.palette_path),
                    ),
                }
            }
            PaletteSuffix::Other => push_finding(
                errors,
                "paletteFormat",
                format!(
                    "palette {} must be a .json {PROJECT_FORMAT}, or a .gpl, .hex, or .txt file",
                    self.palette_path
                ),
                Some(&self.palette_path),
            ),
        }
    }

    fn check_layout(
        &self,
        rule: &LayoutRule,
        files: &[ManifestProbe],
        errors: &mut Vec<ManifestFinding>,
        warnings: &mut Vec<ManifestFinding>,
    ) -> u32 {
        let Some(probe) = files.iter().find(|probe| probe.path == rule.dir) else {
            push_finding(
                errors,
                "probeMissing",
                format!("no probe for layout {} directory {}", rule.name, rule.dir),
                Some(&rule.dir),
            );
            return 0;
        };
        match probe.kind {
            ProbeKind::Missing => {
                push_finding(
                    errors,
                    "layoutMissing",
                    format!("layout {} directory {} is missing", rule.name, rule.dir),
                    Some(&rule.dir),
                );
                return 0;
            }
            ProbeKind::File => {
                push_finding(
                    errors,
                    "layoutKind",
                    format!(
                        "layout {} path {} is a file, expected a directory",
                        rule.name, rule.dir
                    ),
                    Some(&rule.dir),
                );
                return 0;
            }
            ProbeKind::Dir => {}
        }
        let Some(canvas_name) = &rule.canvas else {
            return 0;
        };
        let Some(tier) = self.canvases.iter().find(|tier| &tier.name == canvas_name) else {
            return 0;
        };
        let mut children = direct_files(files, &rule.dir);
        children.sort_by(|left, right| left.path.cmp(&right.path));
        let mut projects = 0u32;
        for child in children {
            if child.unreadable {
                push_finding(
                    errors,
                    "projectFile",
                    format!("{} is not a JSON object", child.path),
                    Some(&child.path),
                );
                continue;
            }
            if child.format.as_deref() != Some(PROJECT_FORMAT) {
                continue;
            }
            projects += 1;
            match (child.width, child.height) {
                (Some(width), Some(height)) if width == tier.width && height == tier.height => {}
                (Some(width), Some(height)) => push_finding(
                    errors,
                    "canvasSize",
                    format!(
                        "{} is {width}x{height}, layout {} expects {} {}x{}",
                        child.path, rule.name, tier.name, tier.width, tier.height
                    ),
                    Some(&child.path),
                ),
                _ => push_finding(
                    errors,
                    "canvasUnread",
                    format!("{} is a {PROJECT_FORMAT} without a canvas size", child.path),
                    Some(&child.path),
                ),
            }
        }
        if projects == 0 {
            push_finding(
                warnings,
                "layoutEmpty",
                format!(
                    "layout {} has no {PROJECT_FORMAT} file in {}",
                    rule.name, rule.dir
                ),
                Some(&rule.dir),
            );
        }
        projects
    }
}

fn build(draft: ManifestDraft) -> Result<ArtManifest, ManifestError> {
    let name = check_name(&draft.name, "manifest name")?;
    let note = check_note(&draft.note)?;
    let perspective = check_token(&draft.perspective, "perspective", PERSPECTIVES)?;
    let asset_root = check_root(&draft.asset_root)?;
    let palette_path = check_rel_path(&draft.palette_path, "palette path")?;
    let palette_ref = match draft.palette_ref {
        Some(name) => Some(check_name(&name, "palette ref")?),
        None => None,
    };
    if draft.canvases.is_empty() {
        return Err(file_err("manifest canvases must list at least one tier"));
    }
    if draft.canvases.len() > MAX_CANVAS_TIERS {
        return Err(file_err(format!(
            "manifest canvases must be at most {MAX_CANVAS_TIERS}"
        )));
    }
    if draft.layout.is_empty() {
        return Err(file_err(
            "manifest layout must list at least one directory rule",
        ));
    }
    if draft.layout.len() > MAX_LAYOUT_RULES {
        return Err(file_err(format!(
            "manifest layout must be at most {MAX_LAYOUT_RULES}"
        )));
    }
    let mut canvas_names = Vec::with_capacity(draft.canvases.len());
    let mut canvases = Vec::with_capacity(draft.canvases.len());
    for tier in draft.canvases {
        let tier_name = check_name(&tier.name, "canvas name")?;
        if canvas_names.iter().any(|name| name == &tier_name) {
            return Err(file_err(format!("canvas name {tier_name} is repeated")));
        }
        if tier.width == 0
            || tier.height == 0
            || tier.width > MAX_CANVAS_EDGE
            || tier.height > MAX_CANVAS_EDGE
        {
            return Err(file_err(format!(
                "canvas {tier_name} edges must be 1..={MAX_CANVAS_EDGE}, got {}x{}",
                tier.width, tier.height
            )));
        }
        canvas_names.push(tier_name.clone());
        canvases.push(CanvasTier {
            name: tier_name,
            width: tier.width,
            height: tier.height,
        });
    }
    let mut layout_names = Vec::with_capacity(draft.layout.len());
    let mut layout_dirs = Vec::with_capacity(draft.layout.len());
    let mut layout = Vec::with_capacity(draft.layout.len());
    for rule in draft.layout {
        let rule_name = check_name(&rule.name, "layout name")?;
        if layout_names.iter().any(|name| name == &rule_name) {
            return Err(file_err(format!("layout name {rule_name} is repeated")));
        }
        let dir = check_rel_path(&rule.dir, "layout dir")?;
        if layout_dirs.iter().any(|existing| existing == &dir) {
            return Err(file_err(format!("layout dir {dir} is repeated")));
        }
        let kind = check_token(&rule.kind, "layout kind", LAYOUT_KINDS)?;
        let canvas = match rule.canvas {
            None => None,
            Some(name) => {
                let canvas = check_name(&name, "layout canvas")?;
                if !canvas_names.iter().any(|tier| tier == &canvas) {
                    return Err(file_err(format!(
                        "layout {rule_name} canvas {canvas} is not a canvas tier"
                    )));
                }
                Some(canvas)
            }
        };
        layout_names.push(rule_name.clone());
        layout_dirs.push(dir.clone());
        layout.push(LayoutRule {
            name: rule_name,
            dir,
            kind,
            canvas,
        });
    }
    Ok(ArtManifest {
        name,
        note,
        perspective,
        asset_root,
        palette_path,
        palette_ref,
        canvases,
        layout,
    })
}

fn parse_canvases(value: Option<&Value>) -> Result<Vec<CanvasTier>, ManifestError> {
    let Some(value) = value else {
        return Err(file_err("manifest canvases is missing"));
    };
    let list = value
        .as_array()
        .ok_or_else(|| file_err("manifest canvases must be an array"))?;
    let mut tiers = Vec::with_capacity(list.len());
    for (index, item) in list.iter().enumerate() {
        let map = item
            .as_object()
            .ok_or_else(|| file_err(format!("manifest canvases[{index}] must be an object")))?;
        reject_unknown(
            map,
            &["name", "width", "height"],
            &format!("canvases[{index}]"),
        )?;
        let name = require_string(map, "name", &format!("canvases[{index}] name"))?;
        let width = require_u16(map, "width", &format!("canvases[{index}] width"))?;
        let height = require_u16(map, "height", &format!("canvases[{index}] height"))?;
        tiers.push(CanvasTier {
            name,
            width,
            height,
        });
    }
    Ok(tiers)
}

fn parse_layout(value: Option<&Value>) -> Result<Vec<LayoutRule>, ManifestError> {
    let Some(value) = value else {
        return Err(file_err("manifest layout is missing"));
    };
    let list = value
        .as_array()
        .ok_or_else(|| file_err("manifest layout must be an array"))?;
    let mut rules = Vec::with_capacity(list.len());
    for (index, item) in list.iter().enumerate() {
        let map = item
            .as_object()
            .ok_or_else(|| file_err(format!("manifest layout[{index}] must be an object")))?;
        reject_unknown(
            map,
            &["name", "dir", "kind", "canvas"],
            &format!("layout[{index}]"),
        )?;
        let canvas = match map.get("canvas") {
            None => None,
            Some(value) => Some(parse_string(value, &format!("layout[{index}] canvas"))?),
        };
        rules.push(LayoutRule {
            name: require_string(map, "name", &format!("layout[{index}] name"))?,
            dir: require_string(map, "dir", &format!("layout[{index}] dir"))?,
            kind: require_string(map, "kind", &format!("layout[{index}] kind"))?,
            canvas,
        });
    }
    Ok(rules)
}

fn check_name(text: &str, what: &str) -> Result<String, ManifestError> {
    let trimmed = text.trim();
    let chars = trimmed.chars().count();
    if trimmed.is_empty() || chars > MAX_NAME_CHARS || trimmed.chars().any(char::is_control) {
        return Err(file_err(format!(
            "{what} must be 1 to {MAX_NAME_CHARS} characters without control characters"
        )));
    }
    Ok(trimmed.to_string())
}

fn check_note(text: &str) -> Result<String, ManifestError> {
    let trimmed = text.trim();
    let chars = trimmed.chars().count();
    if trimmed.is_empty() || chars > MAX_NOTE_CHARS || trimmed.chars().any(char::is_control) {
        return Err(file_err(format!(
            "manifest note must be 1 to {MAX_NOTE_CHARS} characters without control characters"
        )));
    }
    Ok(trimmed.to_string())
}

fn check_token(text: &str, what: &str, allowed: &[&str]) -> Result<String, ManifestError> {
    let trimmed = text.trim();
    if allowed.contains(&trimmed) {
        return Ok(trimmed.to_string());
    }
    Err(file_err(format!(
        "{what} must be {}, got {text}",
        allowed.join(", ")
    )))
}

fn check_root(text: &str) -> Result<String, ManifestError> {
    if text == "." {
        return Ok(text.to_string());
    }
    check_rel_path(text, "asset root")
}

fn check_rel_path(text: &str, what: &str) -> Result<String, ManifestError> {
    if text != text.trim() {
        return Err(file_err(format!(
            "{what} must not have surrounding whitespace"
        )));
    }
    if text.is_empty() || text.chars().count() > MAX_PATH_CHARS {
        return Err(file_err(format!(
            "{what} must be 1 to {MAX_PATH_CHARS} characters"
        )));
    }
    if text.chars().any(char::is_control) || text.contains('\\') || text.contains(':') {
        return Err(file_err(format!(
            "{what} must be a relative path with '/' separators"
        )));
    }
    if text.starts_with('/') {
        return Err(file_err(format!("{what} must be relative")));
    }
    for segment in text.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(file_err(format!("{what} must stay inside the asset root")));
        }
    }
    Ok(text.to_string())
}

enum PaletteSuffix {
    Json,
    Text,
    Other,
}

fn palette_suffix(path: &str) -> PaletteSuffix {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".json") {
        PaletteSuffix::Json
    } else if lower.ends_with(".gpl") || lower.ends_with(".hex") || lower.ends_with(".txt") {
        PaletteSuffix::Text
    } else {
        PaletteSuffix::Other
    }
}

fn duplicate_probe(files: &[ManifestProbe]) -> Option<&str> {
    let mut seen: Vec<&str> = Vec::with_capacity(files.len());
    for probe in files {
        if seen.contains(&probe.path.as_str()) {
            return Some(probe.path.as_str());
        }
        seen.push(probe.path.as_str());
    }
    None
}

fn direct_files<'a>(files: &'a [ManifestProbe], dir: &str) -> Vec<&'a ManifestProbe> {
    let prefix = format!("{dir}/");
    files
        .iter()
        .filter(|probe| {
            probe.kind == ProbeKind::File
                && probe
                    .path
                    .strip_prefix(&prefix)
                    .is_some_and(|rest| !rest.is_empty() && !rest.contains('/'))
        })
        .collect()
}

fn push_finding(
    findings: &mut Vec<ManifestFinding>,
    code: &str,
    message: String,
    path: Option<&str>,
) {
    findings.push(ManifestFinding {
        code: code.to_string(),
        message,
        path: path.map(str::to_string),
    });
}

fn reject_unknown(
    map: &Map<String, Value>,
    known: &[&str],
    what: &str,
) -> Result<(), ManifestError> {
    for key in map.keys() {
        if !known.contains(&key.as_str()) {
            return Err(file_err(format!("{what}: unknown field {key}")));
        }
    }
    Ok(())
}

fn require_object<'a>(
    map: &'a Map<String, Value>,
    key: &str,
    what: &str,
) -> Result<&'a Map<String, Value>, ManifestError> {
    match map.get(key).and_then(Value::as_object) {
        Some(object) => Ok(object),
        None => Err(file_err(format!("{what} must be an object"))),
    }
}

fn require_string(
    map: &Map<String, Value>,
    key: &str,
    what: &str,
) -> Result<String, ManifestError> {
    match map.get(key) {
        Some(value) => parse_string(value, what),
        None => Err(file_err(format!("{what} is missing"))),
    }
}

fn parse_string(value: &Value, what: &str) -> Result<String, ManifestError> {
    value
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| file_err(format!("{what} must be a string")))
}

fn require_u16(map: &Map<String, Value>, key: &str, what: &str) -> Result<u16, ManifestError> {
    let Some(value) = map.get(key) else {
        return Err(file_err(format!("{what} is missing")));
    };
    let Some(number) = value.as_u64() else {
        return Err(file_err(format!("{what} must be an integer")));
    };
    u16::try_from(number).map_err(|_| file_err(format!("{what} is out of range")))
}

struct ToolCall {
    id: Option<Value>,
    tool: String,
    params: Map<String, Value>,
}

struct ToolError {
    id: Option<Value>,
    failure: ToolFail,
}

enum ToolFail {
    InvalidRequest(String),
    UnsupportedVersion { version: u64 },
    UnknownTool { name: String },
    InvalidParams { message: String },
}

impl ToolFail {
    fn code(&self) -> &'static str {
        match self {
            Self::InvalidRequest(_) => "InvalidRequest",
            Self::UnsupportedVersion { .. } => "UnsupportedVersion",
            Self::UnknownTool { .. } => "UnknownTool",
            Self::InvalidParams { .. } => "InvalidParams",
        }
    }

    fn message(&self) -> String {
        match self {
            Self::InvalidRequest(message) => message.clone(),
            Self::UnsupportedVersion { version } => {
                format!("apiVersion must be {TOOL_API_VERSION}, got {version}")
            }
            Self::UnknownTool { name } => format!("unknown tool \"{name}\""),
            Self::InvalidParams { message } => message.clone(),
        }
    }
}

fn dispatch(manifest: &ArtManifest, call: &ToolCall) -> Result<Value, ToolFail> {
    match call.tool.as_str() {
        "inspectManifest" => {
            if let Err(failure) = reject_params(&call.params, &[]) {
                return Err(prefix_params("inspectManifest", failure));
            }
            Ok(manifest.snapshot())
        }
        "validateManifest" => {
            let (root_exists, files) = parse_validate_params(&call.params)?;
            Ok(serde_json::to_value(manifest.validate(root_exists, &files))
                .expect("manifest report is JSON"))
        }
        other => Err(ToolFail::UnknownTool {
            name: other.to_string(),
        }),
    }
}

fn parse_validate_params(
    params: &Map<String, Value>,
) -> Result<(bool, Vec<ManifestProbe>), ToolFail> {
    reject_params(params, &["rootExists", "files"])?;
    let root_exists = match params.get("rootExists") {
        Some(Value::Bool(value)) => *value,
        Some(_) => {
            return Err(ToolFail::InvalidParams {
                message: "validateManifest: rootExists must be a boolean".to_string(),
            })
        }
        None => {
            return Err(ToolFail::InvalidParams {
                message: "validateManifest: rootExists is required".to_string(),
            })
        }
    };
    let files = match params.get("files") {
        None => Vec::new(),
        Some(Value::Array(items)) => {
            if items.len() > MAX_PROBES {
                return Err(ToolFail::InvalidParams {
                    message: format!("validateManifest: files must be at most {MAX_PROBES}"),
                });
            }
            let mut probes = Vec::with_capacity(items.len());
            for (index, item) in items.iter().enumerate() {
                probes.push(parse_probe(item, index)?);
            }
            probes
        }
        Some(_) => {
            return Err(ToolFail::InvalidParams {
                message: "validateManifest: files must be an array".to_string(),
            })
        }
    };
    Ok((root_exists, files))
}

fn parse_probe(value: &Value, index: usize) -> Result<ManifestProbe, ToolFail> {
    let Some(map) = value.as_object() else {
        return Err(ToolFail::InvalidParams {
            message: format!("validateManifest: files[{index}] must be an object"),
        });
    };
    reject_params(
        map,
        &["path", "kind", "format", "width", "height", "unreadable"],
    )
    .map_err(|failure| prefix_params(&format!("validateManifest: files[{index}]"), failure))?;
    let path = match map.get("path").and_then(Value::as_str) {
        Some(path) => path.to_string(),
        _ => {
            return Err(ToolFail::InvalidParams {
                message: format!("validateManifest: files[{index}].path must be a string"),
            })
        }
    };
    let kind = match map
        .get("kind")
        .and_then(Value::as_str)
        .and_then(ProbeKind::parse)
    {
        Some(kind) => kind,
        _ => {
            return Err(ToolFail::InvalidParams {
                message: format!(
                    "validateManifest: files[{index}].kind must be file, dir, or missing"
                ),
            })
        }
    };
    let format = match map.get("format") {
        None => None,
        Some(Value::String(text)) => Some(text.clone()),
        Some(_) => {
            return Err(ToolFail::InvalidParams {
                message: format!("validateManifest: files[{index}].format must be a string"),
            })
        }
    };
    let width = optional_u16(map, "width", index)?;
    let height = optional_u16(map, "height", index)?;
    if width.is_some() != height.is_some() {
        return Err(ToolFail::InvalidParams {
            message: format!("validateManifest: files[{index}] width and height are a pair"),
        });
    }
    let unreadable = match map.get("unreadable") {
        None => false,
        Some(Value::Bool(value)) => *value,
        Some(_) => {
            return Err(ToolFail::InvalidParams {
                message: format!("validateManifest: files[{index}].unreadable must be a boolean"),
            })
        }
    };
    if kind != ProbeKind::File && (format.is_some() || width.is_some() || unreadable) {
        return Err(ToolFail::InvalidParams {
            message: format!(
                "validateManifest: files[{index}] format, size, and unreadable belong on a file"
            ),
        });
    }
    Ok(ManifestProbe {
        path,
        kind,
        format,
        width,
        height,
        unreadable,
    })
}

fn optional_u16(
    map: &Map<String, Value>,
    key: &str,
    index: usize,
) -> Result<Option<u16>, ToolFail> {
    match map.get(key) {
        None => Ok(None),
        Some(value) => {
            let Some(number) = value.as_u64() else {
                return Err(ToolFail::InvalidParams {
                    message: format!("validateManifest: files[{index}].{key} must be an integer"),
                });
            };
            u16::try_from(number)
                .map(Some)
                .map_err(|_| ToolFail::InvalidParams {
                    message: format!("validateManifest: files[{index}].{key} is out of range"),
                })
        }
    }
}

fn prefix_params(prefix: &str, failure: ToolFail) -> ToolFail {
    match failure {
        ToolFail::InvalidParams { message } => ToolFail::InvalidParams {
            message: format!("{prefix}: {message}"),
        },
        other => other,
    }
}

fn reject_params(map: &Map<String, Value>, known: &[&str]) -> Result<(), ToolFail> {
    for key in map.keys() {
        if !known.contains(&key.as_str()) {
            return Err(ToolFail::InvalidParams {
                message: format!("unknown field {key}"),
            });
        }
    }
    Ok(())
}

fn parse_request(request: &str) -> Result<ToolCall, ToolError> {
    let value: Value = match serde_json::from_str(request) {
        Ok(value) => value,
        Err(_) => {
            return Err(ToolError {
                id: None,
                failure: ToolFail::InvalidRequest("request is not JSON".to_string()),
            })
        }
    };
    let Some(map) = value.as_object() else {
        return Err(ToolError {
            id: None,
            failure: ToolFail::InvalidRequest("request must be a JSON object".to_string()),
        });
    };
    let id = match map.get("id") {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => Some(Value::String(text.clone())),
        Some(Value::Number(number)) => Some(Value::Number(number.clone())),
        Some(_) => {
            return Err(ToolError {
                id: None,
                failure: ToolFail::InvalidRequest("id must be a string or number".to_string()),
            })
        }
    };
    let fail = |failure| ToolError {
        id: id.clone(),
        failure,
    };
    if let Err(field) = unknown_key(map, &["apiVersion", "id", "tool", "params"]) {
        return Err(fail(ToolFail::InvalidRequest(format!(
            "unknown field {field}"
        ))));
    }
    if let Some(version) = map.get("apiVersion") {
        match version.as_u64() {
            Some(version) if u32::try_from(version).ok() == Some(TOOL_API_VERSION) => {}
            Some(version) => {
                return Err(fail(ToolFail::UnsupportedVersion { version }));
            }
            None if version.is_null() => {}
            None => {
                return Err(fail(ToolFail::InvalidRequest(
                    "apiVersion must be an integer from 0 to 4294967295".to_string(),
                )));
            }
        }
    }
    let tool = match map.get("tool") {
        Some(Value::String(name)) => name.clone(),
        None => return Err(fail(ToolFail::InvalidRequest("missing tool".to_string()))),
        Some(_) => {
            return Err(fail(ToolFail::InvalidRequest(
                "tool must be a string".to_string(),
            )))
        }
    };
    let params = match map.get("params") {
        None | Some(Value::Null) => Map::new(),
        Some(Value::Object(params)) => params.clone(),
        Some(_) => {
            return Err(fail(ToolFail::InvalidParams {
                message: format!("{tool}: params must be an object"),
            }))
        }
    };
    Ok(ToolCall { id, tool, params })
}

fn unknown_key<'a>(map: &'a Map<String, Value>, known: &[&str]) -> Result<(), &'a str> {
    for key in map.keys() {
        if !known.contains(&key.as_str()) {
            return Err(key.as_str());
        }
    }
    Ok(())
}

fn encode_ok(id: Option<Value>, result: Value) -> String {
    let mut body = Map::new();
    body.insert("ok".to_string(), Value::Bool(true));
    body.insert(
        "apiVersion".to_string(),
        Value::Number(TOOL_API_VERSION.into()),
    );
    if let Some(id) = id {
        body.insert("id".to_string(), id);
    }
    body.insert("result".to_string(), result);
    serde_json::to_string(&Value::Object(body)).expect("tool response is JSON")
}

fn encode_err(id: Option<Value>, failure: ToolFail) -> String {
    let mut body = Map::new();
    body.insert("ok".to_string(), Value::Bool(false));
    body.insert(
        "apiVersion".to_string(),
        Value::Number(TOOL_API_VERSION.into()),
    );
    if let Some(id) = id {
        body.insert("id".to_string(), id);
    }
    body.insert(
        "error".to_string(),
        json!({
            "code": failure.code(),
            "message": failure.message(),
        }),
    );
    serde_json::to_string(&Value::Object(body)).expect("tool response is JSON")
}
