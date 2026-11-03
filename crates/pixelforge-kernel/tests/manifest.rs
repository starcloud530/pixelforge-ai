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
        assets: Vec::new(),
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
    value["status"] = json!("draft");
    let error = ArtManifest::load(&value.to_string()).unwrap_err();
    assert_eq!(error.code(), "ManifestFile");
    assert!(error.message().contains("unknown field status"));

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
        .join("tests/fixtures/game-pack-zombie/pixelforge.manifest.json");
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
    let ids: Vec<&str> = manifest
        .assets()
        .iter()
        .map(|asset| asset.id.as_str())
        .collect();
    assert_eq!(
        ids,
        [
            "zombie",
            "zombie.idle",
            "zombie.walk",
            "zombie.sheet",
            "props",
            "props.barricade",
            "props.crate",
            "props.generator",
            "props.medkit",
            "props.sheet",
        ]
    );
    assert_eq!(manifest.assets()[2].kind, "clip");
    assert_eq!(
        manifest.assets()[2].tags,
        [
            "walk_down".to_string(),
            "walk_left".to_string(),
            "walk_right".to_string(),
            "walk_up".to_string(),
        ]
    );
    let report = manifest.asset_report(
        true,
        &[
            file("zombie/zombie.json", Some(PROJECT_FORMAT), 64, 64),
            file(
                "zombie/zombie-sheet.json",
                Some("pixelforge-atlas"),
                256,
                256,
            ),
            file("props/props.json", Some(PROJECT_FORMAT), 64, 64),
            file("props/props-sheet.json", Some("pixelforge-atlas"), 256, 64),
        ],
    );
    assert!(report.errors.is_empty(), "{:?}", report.errors);
}

#[test]
fn tokens_match_the_closed_sets() {
    assert_eq!(PERSPECTIVES, ["top", "side", "front", "oblique"]);
    assert_eq!(
        LAYOUT_KINDS,
        ["character", "prop", "tile", "portrait", "ui", "effect"]
    );
    assert_eq!(
        MANIFEST_TOOL_NAMES,
        [
            "inspectManifest",
            "validateManifest",
            "addAsset",
            "setAsset",
            "removeAsset",
            "listAssets",
            "showAsset",
            "graphAsset",
            "dependentsAsset",
            "validateAssets",
            "characterMatrix",
        ]
    );
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
    let mut manifest = manifest();
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

fn tool(manifest: &mut ArtManifest, request: serde_json::Value) -> serde_json::Value {
    let response = manifest.call_tool(&request.to_string());
    serde_json::from_str(&response).unwrap()
}

fn register_hero(manifest: &mut ArtManifest) {
    let added = tool(
        manifest,
        json!({
            "tool": "addAsset",
            "params": {
                "id": "hero",
                "kind": "character",
                "path": "sprites/hero.json",
                "produces": ["hero.walk", "hero.sheet"]
            }
        }),
    );
    assert_eq!(added["ok"], json!(true), "{added}");
    assert_eq!(added["result"]["id"], json!("hero"));
    let clip = tool(
        manifest,
        json!({
            "tool": "addAsset",
            "params": {
                "id": "hero.walk",
                "kind": "clip",
                "path": "sprites/hero.json",
                "owner": "hero",
                "dependsOn": ["hero"],
                "tags": ["walk_down", "walk_left", "walk_right", "walk_up"]
            }
        }),
    );
    assert_eq!(clip["ok"], json!(true), "{clip}");
    let sheet = tool(
        manifest,
        json!({
            "tool": "addAsset",
            "params": {
                "id": "hero.sheet",
                "kind": "sheet",
                "path": "sprites/hero-sheet.json",
                "owner": "hero",
                "dependsOn": ["hero"]
            }
        }),
    );
    assert_eq!(sheet["ok"], json!(true), "{sheet}");
}

#[test]
fn asset_registry_round_trips_and_omits_empty_lists() {
    let mut manifest = manifest();
    register_hero(&mut manifest);
    let text = manifest.save();
    let loaded = ArtManifest::load(&text).unwrap();
    assert_eq!(loaded, manifest);
    assert_eq!(loaded.save(), text);
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(value["assets"][1].get("produces").is_none());
    assert_eq!(value["assets"][1]["dependsOn"], json!(["hero"]));
    assert_eq!(value["assets"][1]["tags"][0], json!("walk_down"));
    assert_eq!(loaded.assets().len(), 3);

    let mut bare =
        serde_json::from_str::<serde_json::Value>(&ArtManifest::new(draft()).unwrap().save())
            .unwrap();
    bare["assets"] = json!([]);
    let loaded = ArtManifest::load(&bare.to_string()).unwrap();
    assert!(loaded.assets().is_empty());
    assert!(!loaded.save().contains("\"assets\""));
}

#[test]
fn asset_queries_follow_owner_depends_on_and_produces() {
    let mut manifest = manifest();
    register_hero(&mut manifest);
    let listed = tool(
        &mut manifest,
        json!({"tool": "listAssets", "params": {"kind": "clip", "tag": "walk_down", "owner": "hero"}}),
    );
    assert_eq!(listed["result"]["assets"].as_array().unwrap().len(), 1);
    assert_eq!(listed["result"]["assets"][0]["id"], json!("hero.walk"));

    let shown = tool(
        &mut manifest,
        json!({"tool": "showAsset", "params": {"id": "hero"}}),
    );
    assert_eq!(shown["result"]["owns"], json!(["hero.walk", "hero.sheet"]));
    assert_eq!(
        shown["result"]["dependents"],
        json!(["hero.walk", "hero.sheet"])
    );

    let graph = tool(
        &mut manifest,
        json!({"tool": "graphAsset", "params": {"id": "hero.walk"}}),
    );
    assert_eq!(graph["result"]["missing"], json!([]));
    let edges = graph["result"]["edges"].as_array().unwrap();
    assert!(edges
        .iter()
        .any(|edge| edge["rel"] == "owner" && edge["to"] == "hero"));
    assert!(edges
        .iter()
        .any(|edge| edge["rel"] == "dependsOn" && edge["to"] == "hero"));
    assert!(edges.iter().any(|edge| edge["rel"] == "produces"
        && edge["from"] == "hero"
        && edge["to"] == "hero.walk"));

    let dependents = tool(
        &mut manifest,
        json!({"tool": "dependentsAsset", "params": {"id": "hero"}}),
    );
    assert_eq!(
        dependents["result"]["dependents"].as_array().unwrap().len(),
        2
    );

    let missing = tool(
        &mut manifest,
        json!({"tool": "showAsset", "params": {"id": "nope"}}),
    );
    assert_eq!(missing["error"]["code"], "InvalidParams");
}

#[test]
fn asset_validate_reports_dangling_ids_cycles_and_missing_paths() {
    let mut manifest = manifest();
    let added = tool(
        &mut manifest,
        json!({
            "tool": "addAsset",
            "params": {"id": "hero", "kind": "character", "path": "sprites/hero.json", "produces": ["hero.walk"]}
        }),
    );
    assert_eq!(added["ok"], json!(true), "{added}");
    let report = manifest.asset_report(true, &healthy_hero());
    assert!(report.errors.iter().any(|error| error.code == "assetRef"));

    tool(
        &mut manifest,
        json!({
            "tool": "addAsset",
            "params": {
                "id": "hero.walk",
                "kind": "clip",
                "path": "sprites/hero.json",
                "owner": "hero.walk"
            }
        }),
    );
    let report = manifest.asset_report(true, &healthy_hero());
    assert!(
        report.errors.iter().any(|error| error.code == "assetOwner"),
        "{:?}",
        report.errors
    );
    tool(
        &mut manifest,
        json!({"tool": "setAsset", "params": {"id": "hero.walk", "owner": "hero", "dependsOn": ["hero"]}}),
    );
    let report = manifest.asset_report(true, &healthy_hero());
    assert!(report.errors.is_empty(), "{:?}", report.errors);

    tool(
        &mut manifest,
        json!({"tool": "setAsset", "params": {"id": "hero", "dependsOn": ["hero.walk"]}}),
    );
    let report = manifest.asset_report(true, &healthy_hero());
    assert!(report.errors.iter().any(|error| error.code == "assetCycle"));

    tool(
        &mut manifest,
        json!({"tool": "setAsset", "params": {"id": "hero", "dependsOn": []}}),
    );
    tool(
        &mut manifest,
        json!({"tool": "setAsset", "params": {"id": "hero.walk", "path": "sprites/missing.json"}}),
    );
    let mut files = healthy_hero();
    files.push(missing("sprites/missing.json"));
    let report = manifest.asset_report(true, &files);
    assert!(report
        .errors
        .iter()
        .any(|error| error.code == "assetMissing"));
    let report = manifest.asset_report(true, &[]);
    assert!(report
        .errors
        .iter()
        .any(|error| error.code == "probeMissing"));
    tool(
        &mut manifest,
        json!({"tool": "setAsset", "params": {"id": "hero.walk", "path": "zombie"}}),
    );
    let report = manifest.asset_report(true, &healthy());
    assert!(
        report.errors.iter().any(|error| error.code == "assetPath"),
        "{:?}",
        report.errors
    );

    let refused = tool(
        &mut manifest,
        json!({"tool": "removeAsset", "params": {"id": "hero"}}),
    );
    assert_eq!(refused["error"]["code"], "InvalidParams");
    assert!(refused["error"]["message"]
        .as_str()
        .unwrap()
        .contains("hero.walk"));
    let blocked = tool(
        &mut manifest,
        json!({"tool": "removeAsset", "params": {"id": "hero.walk"}}),
    );
    assert!(blocked["error"]["message"]
        .as_str()
        .unwrap()
        .contains("hero"));
    tool(
        &mut manifest,
        json!({"tool": "setAsset", "params": {"id": "hero", "produces": []}}),
    );
    let removed = tool(
        &mut manifest,
        json!({"tool": "removeAsset", "params": {"id": "hero.walk"}}),
    );
    assert_eq!(removed["result"]["removed"], json!(true));
}

#[test]
fn asset_owner_must_be_a_character_or_group_and_produces_must_be_a_dependency() {
    let mut manifest = manifest();
    tool(
        &mut manifest,
        json!({"tool": "addAsset", "params": {"id": "hero", "kind": "character", "path": "sprites/hero.json"}}),
    );
    tool(
        &mut manifest,
        json!({
            "tool": "addAsset",
            "params": {
                "id": "crate",
                "kind": "prop",
                "path": "props/props.json",
                "owner": "hero",
                "dependsOn": ["palette"]
            }
        }),
    );
    let report = manifest.asset_report(true, &healthy_hero());
    assert!(report.errors.iter().any(|error| error.code == "assetRef"));
    assert!(report.errors.iter().all(|error| error.code != "assetOwner"));

    tool(
        &mut manifest,
        json!({"tool": "setAsset", "params": {"id": "crate", "owner": "crate"}}),
    );
    let report = manifest.asset_report(true, &healthy_hero());
    assert!(report
        .errors
        .iter()
        .any(|error| { error.code == "assetOwner" && error.message.contains("itself") }));

    tool(
        &mut manifest,
        json!({"tool": "setAsset", "params": {"id": "hero", "produces": ["crate"], "owner": null}}),
    );
    tool(
        &mut manifest,
        json!({"tool": "setAsset", "params": {"id": "crate", "owner": "hero", "dependsOn": []}}),
    );
    let report = manifest.asset_report(true, &healthy_hero());
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.code == "assetProduces"),
        "{:?}",
        report.errors
    );
}

#[test]
fn owner_cycle_is_an_error() {
    let mut manifest = manifest();
    tool(
        &mut manifest,
        json!({"tool": "addAsset", "params": {"id": "pack", "kind": "group", "path": "props/props.json"}}),
    );
    tool(
        &mut manifest,
        json!({
            "tool": "addAsset",
            "params": {
                "id": "hero",
                "kind": "character",
                "path": "zombie/zombie.json",
                "owner": "pack"
            }
        }),
    );
    tool(
        &mut manifest,
        json!({"tool": "setAsset", "params": {"id": "pack", "owner": "hero"}}),
    );
    let report = manifest.asset_report(true, &healthy());
    assert!(
        report
            .errors
            .iter()
            .any(|error| { error.code == "assetOwner" && error.message.contains("owner cycle") }),
        "{:?}",
        report.errors
    );
}

#[test]
fn asset_tags_follow_animation_tag_rules() {
    let mut value: serde_json::Value = serde_json::from_str(&manifest().save()).unwrap();
    value["assets"] = json!([{
        "id": "hero",
        "kind": "clip",
        "path": "sprites/hero.json",
        "tags": [" walk down ", "行走", "01_walk"]
    }]);
    let loaded = ArtManifest::load(&value.to_string()).expect("load compatible tags");
    assert_eq!(
        loaded.assets()[0].tags,
        [
            "walk down".to_string(),
            "行走".to_string(),
            "01_walk".to_string()
        ]
    );

    value["assets"][0]["tags"] = json!(["walk down", " walk down "]);
    assert!(ArtManifest::load(&value.to_string())
        .unwrap_err()
        .message()
        .contains("repeated"));
    value["assets"][0]["tags"] = json!(["\n"]);
    assert!(ArtManifest::load(&value.to_string()).is_err());

    let mut manifest = manifest();
    let added = tool(
        &mut manifest,
        json!({
            "tool": "addAsset",
            "params": {
                "id": "hero",
                "kind": "clip",
                "path": "sprites/hero.json",
                "tags": ["walk down", "行走", "01_walk"]
            }
        }),
    );
    assert_eq!(added["ok"], json!(true), "{added}");
    assert_eq!(
        added["result"]["tags"],
        json!(["walk down", "行走", "01_walk"])
    );

    let replaced = tool(
        &mut manifest,
        json!({"tool": "setAsset", "params": {"id": "hero", "tags": [" 行走 "]}}),
    );
    assert_eq!(replaced["ok"], json!(true), "{replaced}");
    assert_eq!(replaced["result"]["tags"], json!(["行走"]));

    tool(
        &mut manifest,
        json!({"tool": "setAsset", "params": {"id": "hero", "tags": ["walk down", "行走", "01_walk"]}}),
    );
    for tag in ["walk down", " 行走 ", "01_walk"] {
        let listed = tool(
            &mut manifest,
            json!({"tool": "listAssets", "params": {"tag": tag}}),
        );
        assert_eq!(listed["ok"], json!(true), "{listed}");
        assert_eq!(listed["result"]["assets"].as_array().unwrap().len(), 1);
        assert_eq!(listed["result"]["assets"][0]["id"], json!("hero"));
    }
    let refused = tool(
        &mut manifest,
        json!({"tool": "listAssets", "params": {"tag": " "}}),
    );
    assert_eq!(refused["error"]["code"], "InvalidParams");
}

#[test]
fn cycle_path_matches_the_full_asset_id() {
    let mut manifest = manifest();
    tool(
        &mut manifest,
        json!({"tool": "addAsset", "params": {"id": "a", "kind": "character", "path": "a.json"}}),
    );
    tool(
        &mut manifest,
        json!({
            "tool": "addAsset",
            "params": {
                "id": "ab",
                "kind": "character",
                "path": "ab.json",
                "dependsOn": ["ac"]
            }
        }),
    );
    tool(
        &mut manifest,
        json!({
            "tool": "addAsset",
            "params": {
                "id": "ac",
                "kind": "character",
                "path": "ac.json",
                "dependsOn": ["ab"]
            }
        }),
    );
    let report = manifest.asset_report(true, &[]);
    let cycle = report
        .errors
        .iter()
        .find(|error| error.code == "assetCycle")
        .expect("cycle");
    let start = cycle
        .message
        .strip_prefix("dependsOn cycle ")
        .unwrap()
        .split(" -> ")
        .next()
        .unwrap();
    let expected = format!("{start}.json");
    assert_eq!(cycle.path.as_deref(), Some(expected.as_str()));
    assert_ne!(cycle.path.as_deref(), Some("a.json"));
}

#[test]
fn asset_file_refuses_a_bad_id_and_a_repeated_id() {
    let mut value: serde_json::Value = serde_json::from_str(&manifest().save()).unwrap();
    value["assets"] = json!([
        {"id": "Hero", "kind": "character", "path": "sprites/hero.json"},
        {"id": "Hero", "kind": "clip", "path": "sprites/hero.json"}
    ]);
    assert!(ArtManifest::load(&value.to_string())
        .unwrap_err()
        .message()
        .contains("repeated"));

    value["assets"] = json!([
        {"id": "walk down", "kind": "clip", "path": "sprites/hero.json"}
    ]);
    assert!(ArtManifest::load(&value.to_string()).is_err());

    value["assets"] = json!([
        {"id": "hero", "kind": "monster", "path": "sprites/hero.json"}
    ]);
    assert!(ArtManifest::load(&value.to_string())
        .unwrap_err()
        .message()
        .contains("asset kind"));
}

fn healthy_hero() -> Vec<ManifestProbe> {
    let mut files = healthy();
    files.push(dir("sprites"));
    files.push(file("sprites/hero.json", Some(PROJECT_FORMAT), 64, 64));
    files.push(file(
        "sprites/hero-sheet.json",
        Some("pixelforge-atlas"),
        256,
        256,
    ));
    files
}
