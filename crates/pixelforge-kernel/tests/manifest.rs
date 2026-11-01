//! Project manifest: conventions, not pixels.

use std::fs;
use std::path::PathBuf;

use pixelforge_kernel::{
    ArtManifest, CanvasTier, LayoutRule, ManifestDraft, ManifestProbe, ProbeKind, LAYOUT_KINDS,
    MANIFEST_FORMAT, MANIFEST_TOOL_NAMES, PERSPECTIVES, PROJECT_FORMAT,
};
use serde_json::json;

fn draft() -> ManifestDraft {
    ManifestDraft {
        name: "zombie-outpost".to_string(),
        note: "Top-down 64x64 sprites. This file stores conventions, not pixels.".to_string(),
        perspective: "top".to_string(),
        asset_root: ".".to_string(),
        palette_path: "zombie/zombie.json".to_string(),
        palette_ref: Some("zombie".to_string()),
        canvases: vec![CanvasTier {
            name: "sprite".to_string(),
            width: 64,
            height: 64,
        }],
        layout: vec![
            LayoutRule {
                name: "characters".to_string(),
                dir: "zombie".to_string(),
                kind: "character".to_string(),
                canvas: Some("sprite".to_string()),
            },
            LayoutRule {
                name: "props".to_string(),
                dir: "props".to_string(),
                kind: "prop".to_string(),
                canvas: Some("sprite".to_string()),
            },
        ],
    }
}

fn manifest() -> ArtManifest {
    ArtManifest::new(draft()).expect("draft")
}

fn file(path: &str, format: Option<&str>, width: u16, height: u16) -> ManifestProbe {
    ManifestProbe {
        path: path.to_string(),
        kind: ProbeKind::File,
        format: format.map(str::to_string),
        width: Some(width),
        height: Some(height),
        unreadable: false,
    }
}

fn dir(path: &str) -> ManifestProbe {
    ManifestProbe {
        path: path.to_string(),
        kind: ProbeKind::Dir,
        format: None,
        width: None,
        height: None,
        unreadable: false,
    }
}

fn missing(path: &str) -> ManifestProbe {
    ManifestProbe {
        path: path.to_string(),
        kind: ProbeKind::Missing,
        format: None,
        width: None,
        height: None,
        unreadable: false,
    }
}

fn healthy() -> Vec<ManifestProbe> {
    vec![
        dir("zombie"),
        dir("props"),
        file("zombie/zombie.json", Some(PROJECT_FORMAT), 64, 64),
        file("props/props.json", Some(PROJECT_FORMAT), 64, 64),
        file(
            "zombie/zombie-sheet.json",
            Some("pixelforge-atlas"),
            256,
            256,
        ),
        file(
            "props/props-rules.json",
            Some("pixelforge-atlas-rules"),
            1,
            1,
        ),
    ]
}

#[test]
fn save_and_load_round_trip_and_drop_optional_fields() {
    let manifest = manifest();
    let text = manifest.save();
    let loaded = ArtManifest::load(&text).expect("load");
    assert_eq!(loaded, manifest);
    assert_eq!(loaded.save(), text);
    assert!(text.ends_with('\n'));
    assert!(text.contains(MANIFEST_FORMAT));
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["version"], json!(1));
    assert_eq!(value["palette"]["ref"], json!("zombie"));
    assert!(value.get("assets").is_none());

    let mut bare = draft();
    bare.palette_ref = None;
    bare.layout[1].canvas = None;
    let bare = ArtManifest::new(bare).unwrap();
    let text = bare.save();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(value["palette"].get("ref").is_none());
    assert!(value["layout"][1].get("canvas").is_none());
    assert_eq!(ArtManifest::load(&text).unwrap(), bare);
}

#[test]
fn load_refuses_unknown_fields_bad_version_and_paths() {
    let text = manifest().save();
    let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
    value["assets"] = json!([]);
    let error = ArtManifest::load(&value.to_string()).unwrap_err();
    assert_eq!(error.code(), "ManifestFile");
    assert!(error.message().contains("unknown field assets"));

    value = serde_json::from_str(&text).unwrap();
    value["version"] = json!(2);
    assert!(ArtManifest::load(&value.to_string())
        .unwrap_err()
        .message()
        .contains("version must be 1"));

    value = serde_json::from_str(&text).unwrap();
    value["palette"]["path"] = json!("../secret.json");
    assert!(ArtManifest::load(&value.to_string())
        .unwrap_err()
        .message()
        .contains("asset root"));

    value = serde_json::from_str(&text).unwrap();
    value["assetRoot"] = json!("/tmp/art");
    assert!(ArtManifest::load(&value.to_string())
        .unwrap_err()
        .message()
        .contains("relative"));

    value = serde_json::from_str(&text).unwrap();
    value["perspective"] = json!("isometric");
    assert!(ArtManifest::load(&value.to_string())
        .unwrap_err()
        .message()
        .contains("perspective"));

    value = serde_json::from_str(&text).unwrap();
    value["canvases"][0]["width"] = json!(0);
    assert!(ArtManifest::load(&value.to_string())
        .unwrap_err()
        .message()
        .contains("1..=256"));

    value = serde_json::from_str(&text).unwrap();
    value["canvases"][0]["height"] = json!(257);
    assert!(ArtManifest::load(&value.to_string()).is_err());

    value = serde_json::from_str(&text).unwrap();
    value["layout"][0]["kind"] = json!("hero");
    assert!(ArtManifest::load(&value.to_string())
        .unwrap_err()
        .message()
        .contains("layout kind"));

    value = serde_json::from_str(&text).unwrap();
    value["layout"][0]["canvas"] = json!("missing");
    assert!(ArtManifest::load(&value.to_string())
        .unwrap_err()
        .message()
        .contains("not a canvas tier"));

    value = serde_json::from_str(&text).unwrap();
    value["note"] = json!("");
    assert!(ArtManifest::load(&value.to_string()).is_err());
}

#[test]
fn zombie_pack_manifest_matches_the_writer() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/game-pack-zombie/pixelforge.manifest.json");
    let text = fs::read_to_string(&path).expect("example manifest");
    let manifest = ArtManifest::load(&text).expect("load example");
    assert_eq!(manifest.save(), text);
    assert_eq!(manifest.name(), "zombie-outpost");
    assert_eq!(manifest.perspective(), "top");
    assert_eq!(manifest.asset_root(), ".");
    assert_eq!(manifest.palette_path(), "zombie/zombie.json");
    assert_eq!(manifest.palette_ref(), Some("zombie"));
    assert_eq!(manifest.canvases().len(), 1);
    assert_eq!(manifest.canvases()[0].width, 64);
    assert_eq!(manifest.canvases()[0].height, 64);
    assert_eq!(manifest.layout().len(), 2);
    assert!(manifest.note().contains("不存像素"));
}

#[test]
fn tokens_match_the_closed_sets() {
    assert_eq!(PERSPECTIVES, ["top", "side", "front", "oblique"]);
    assert_eq!(
        LAYOUT_KINDS,
        ["character", "prop", "tile", "portrait", "ui", "effect"]
    );
    assert_eq!(MANIFEST_TOOL_NAMES, ["inspectManifest", "validateManifest"]);
}

#[test]
fn validate_accepts_matching_projects_and_ignores_atlas_files() {
    let report = manifest().validate(true, &healthy());
    assert_eq!(report.error_count, 0, "{:?}", report.errors);
    assert_eq!(report.warning_count, 0, "{:?}", report.warnings);
    assert_eq!(report.checks.layout[0].projects, 1);
    assert_eq!(report.checks.layout[1].projects, 1);
    assert_eq!(report.checks.canvases[0].width, 64);
}

#[test]
fn validate_reports_size_missing_root_and_empty_layout() {
    let mut files = healthy();
    files.retain(|probe| probe.path != "props/props.json");
    files.push(file("props/props.json", Some(PROJECT_FORMAT), 32, 64));
    let report = manifest().validate(true, &files);
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.errors[0].code, "canvasSize");
    assert_eq!(report.errors[0].path.as_deref(), Some("props/props.json"));
    assert_eq!(report.checks.layout[1].projects, 1);

    let report = manifest().validate(false, &[]);
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.errors[0].code, "assetRoot");

    let mut files = healthy();
    files.retain(|probe| !probe.path.starts_with("props/"));
    let report = manifest().validate(true, &files);
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert_eq!(report.warnings.len(), 1);
    assert_eq!(report.warnings[0].code, "layoutEmpty");
    assert_eq!(report.checks.layout[1].projects, 0);
}

#[test]
fn validate_refuses_a_missing_palette_and_a_non_palette_suffix() {
    let mut files = healthy();
    files.retain(|probe| probe.path != "zombie/zombie.json");
    files.push(missing("zombie/zombie.json"));
    let report = manifest().validate(true, &files);
    assert!(report
        .errors
        .iter()
        .any(|error| error.code == "paletteMissing"));

    let mut bare = draft();
    bare.palette_path = "colors.png".to_string();
    let manifest = ArtManifest::new(bare).unwrap();
    let mut files = healthy();
    files.push(file("colors.png", None, 1, 1));
    let report = manifest.validate(true, &files);
    assert!(report
        .errors
        .iter()
        .any(|error| error.code == "paletteFormat"));

    let mut bare = draft();
    bare.palette_path = "colors.gpl".to_string();
    let manifest = ArtManifest::new(bare).unwrap();
    let mut files = healthy();
    files.push(ManifestProbe {
        path: "colors.gpl".to_string(),
        kind: ProbeKind::File,
        format: None,
        width: None,
        height: None,
        unreadable: false,
    });
    let report = manifest.validate(true, &files);
    assert!(
        report
            .errors
            .iter()
            .all(|error| error.code != "paletteFormat"),
        "{:?}",
        report.errors
    );
}

#[test]
fn validate_fails_closed_without_probes() {
    let report = manifest().validate(true, &[]);
    assert!(report.error_count >= 2);
    assert!(report
        .errors
        .iter()
        .all(|error| error.code == "probeMissing"));
}

#[test]
fn call_tool_inspects_and_validates() {
    let manifest = manifest();
    let response = manifest.call_tool(r#"{"tool":"inspectManifest"}"#);
    let value: serde_json::Value = serde_json::from_str(&response).unwrap();
    assert_eq!(value["ok"], json!(true));
    assert_eq!(value["result"]["format"], json!(MANIFEST_FORMAT));
    assert_eq!(value["result"]["name"], json!("zombie-outpost"));
    assert!(value.get("tool").is_none());

    let request = json!({
        "apiVersion": 1,
        "id": "m",
        "tool": "validateManifest",
        "params": {
            "rootExists": true,
            "files": [
                {"path": "zombie", "kind": "dir"},
                {"path": "props", "kind": "dir"},
                {"path": "zombie/zombie.json", "kind": "file", "format": PROJECT_FORMAT, "width": 64, "height": 64},
                {"path": "props/props.json", "kind": "file", "format": PROJECT_FORMAT, "width": 64, "height": 64}
            ]
        }
    });
    let response = manifest.call_tool(&request.to_string());
    let value: serde_json::Value = serde_json::from_str(&response).unwrap();
    assert_eq!(value["ok"], json!(true));
    assert_eq!(value["id"], json!("m"));
    assert_eq!(value["result"]["errorCount"], json!(0));

    let response = manifest.call_tool(r#"{"tool":"paint","params":{"x":0,"y":0,"colorIndex":1}}"#);
    let value: serde_json::Value = serde_json::from_str(&response).unwrap();
    assert_eq!(value["ok"], json!(false));
    assert_eq!(value["error"]["code"], "UnknownTool");

    let response = manifest.call_tool(r#"{"tool":"validateManifest","params":{"files":[]}}"#);
    let value: serde_json::Value = serde_json::from_str(&response).unwrap();
    assert_eq!(value["error"]["code"], "InvalidParams");
}
