//! CLI coverage for pixelforge-manifest init, show, and validate.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{json, Value};

fn pixelforge(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pixelforge"))
        .args(args)
        .output()
        .expect("spawn pixelforge")
}

fn status(output: &Output) -> i32 {
    output.status.code().expect("exit")
}

fn json_line(output: &Output) -> Value {
    let text = String::from_utf8(output.stdout.clone()).expect("utf-8");
    serde_json::from_str(text.lines().next().unwrap())
        .unwrap_or_else(|err| panic!("{err} in {text}"))
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("utf-8")
}

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "pixelforge-manifest-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn child(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn new_project(path: &Path, width: u16, height: u16) {
    let width = width.to_string();
    let height = height.to_string();
    let output = pixelforge(&[
        "new",
        "--width",
        &width,
        "--height",
        &height,
        "--output",
        path.to_str().unwrap(),
    ]);
    assert_eq!(status(&output), 0, "{}", stderr(&output));
}

fn init(scratch: &Scratch, canvas: &str, layout_dir: &str) -> PathBuf {
    let manifest = scratch.child("pixelforge.manifest.json");
    let output = pixelforge(&[
        "project",
        "init",
        "--name",
        "demo",
        "--perspective",
        "side",
        "--note",
        "Side-view sprite conventions. Not pixels.",
        "--palette",
        "hero.json",
        "--palette-ref",
        "hero",
        "--canvas",
        canvas,
        "--layout",
        &format!("characters:{layout_dir}:character:sprite"),
        "--output",
        manifest.to_str().unwrap(),
    ]);
    assert_eq!(status(&output), 0, "{}", stderr(&output));
    assert!(stderr(&output).contains("does not store pixels"));
    let body = json_line(&output);
    assert_eq!(body["ok"], json!(true));
    assert_eq!(body["result"]["format"], json!("pixelforge-manifest"));
    assert_eq!(body["result"]["perspective"], json!("side"));
    manifest
}

#[test]
fn init_show_and_validate_round_trip() {
    let scratch = Scratch::new();
    new_project(&scratch.child("hero.json"), 16, 16);
    fs::create_dir(scratch.child("sprites")).unwrap();
    fs::copy(
        scratch.child("hero.json"),
        scratch.child("sprites/hero.json"),
    )
    .unwrap();
    let manifest = init(&scratch, "sprite:16x16", "sprites");

    let shown = pixelforge(&["project", "show", "--manifest", manifest.to_str().unwrap()]);
    assert_eq!(status(&shown), 0, "{}", stderr(&shown));
    let body = json_line(&shown);
    assert_eq!(body["result"]["name"], json!("demo"));
    assert_eq!(body["result"]["palette"]["path"], json!("hero.json"));
    assert_eq!(body["result"]["palette"]["ref"], json!("hero"));
    assert_eq!(body["result"]["layout"][0]["kind"], json!("character"));

    let checked = pixelforge(&[
        "project",
        "validate",
        "--manifest",
        manifest.to_str().unwrap(),
    ]);
    assert_eq!(status(&checked), 0, "{}", stderr(&checked));
    let body = json_line(&checked);
    assert_eq!(body["ok"], json!(true));
    assert_eq!(body["result"]["errorCount"], json!(0));
    assert_eq!(body["result"]["warningCount"], json!(0));
    assert_eq!(body["result"]["checks"]["layout"][0]["projects"], json!(1));
    assert!(stderr(&checked).contains("project validate: ok"));

    let again = pixelforge(&[
        "project",
        "init",
        "--name",
        "demo",
        "--perspective",
        "top",
        "--note",
        "again",
        "--palette",
        "hero.json",
        "--canvas",
        "sprite:16x16",
        "--layout",
        "characters:sprites:character:sprite",
        "--output",
        manifest.to_str().unwrap(),
    ]);
    assert_eq!(status(&again), 2, "{}", stderr(&again));
    assert_eq!(json_line(&again)["error"]["code"], "Cli");
}

#[test]
fn validate_reports_a_canvas_mismatch_and_an_empty_directory() {
    let scratch = Scratch::new();
    new_project(&scratch.child("hero.json"), 16, 16);
    fs::create_dir(scratch.child("sprites")).unwrap();
    new_project(&scratch.child("sprites/tall.json"), 16, 32);
    let manifest = init(&scratch, "sprite:16x16", "sprites");
    let checked = pixelforge(&[
        "project",
        "validate",
        "--manifest",
        manifest.to_str().unwrap(),
    ]);
    assert_eq!(status(&checked), 1, "{}", stderr(&checked));
    let body = json_line(&checked);
    assert_eq!(body["ok"], json!(true));
    assert_eq!(body["result"]["errors"][0]["code"], "canvasSize");
    assert!(stderr(&checked).contains("canvasSize"));

    let empty = scratch.child("pixelforge.empty.json");
    fs::create_dir(scratch.child("props")).unwrap();
    fs::write(scratch.child("colors.gpl"), "GIMP Palette\n0 0 0\n").unwrap();
    let output = pixelforge(&[
        "project",
        "init",
        "--name",
        "empty",
        "--perspective",
        "front",
        "--note",
        "Palette is a gpl file. The prop directory is still empty.",
        "--palette",
        "colors.gpl",
        "--canvas",
        "sprite:16x16",
        "--layout",
        "props:props:prop:sprite",
        "--output",
        empty.to_str().unwrap(),
    ]);
    assert_eq!(status(&output), 0, "{}", stderr(&output));
    let checked = pixelforge(&["project", "validate", "--manifest", empty.to_str().unwrap()]);
    assert_eq!(status(&checked), 0, "{}", stderr(&checked));
    let body = json_line(&checked);
    assert_eq!(body["result"]["errorCount"], json!(0));
    assert_eq!(body["result"]["warnings"][0]["code"], "layoutEmpty");
    assert!(stderr(&checked).contains("warning layoutEmpty"));
}

#[test]
fn bad_manifest_files_do_not_run_the_checklist() {
    let scratch = Scratch::new();
    let path = scratch.child("bad.json");
    fs::write(
        &path,
        r#"{"format":"pixelforge-manifest","version":1,"extra":true}"#,
    )
    .unwrap();
    let output = pixelforge(&["project", "validate", "--manifest", path.to_str().unwrap()]);
    assert_eq!(status(&output), 2, "{}", stderr(&output));
    assert_eq!(json_line(&output)["error"]["code"], "ManifestFile");

    fs::write(
        &path,
        r#"{"format":"pixelforge-manifest","version":1,"name":"x","note":"n","perspective":"top","assetRoot":"..","palette":{"path":"a.json"},"canvases":[{"name":"sprite","width":16,"height":16}],"layout":[{"name":"characters","dir":"sprites","kind":"character","canvas":"sprite"}]}"#,
    )
    .unwrap();
    let output = pixelforge(&["project", "show", "--manifest", path.to_str().unwrap()]);
    assert_eq!(status(&output), 2);
    assert!(json_line(&output)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("asset root"));
}

#[test]
fn call_dispatches_manifest_tools_and_refuses_pixels() {
    let scratch = Scratch::new();
    new_project(&scratch.child("hero.json"), 16, 16);
    fs::create_dir(scratch.child("sprites")).unwrap();
    let manifest = init(&scratch, "sprite:16x16", "sprites");
    let manifest_arg = manifest.to_str().unwrap();

    let inspected = pixelforge(&[
        "call",
        "--project",
        manifest_arg,
        "--request",
        r#"{"tool":"inspectManifest"}"#,
    ]);
    assert_eq!(status(&inspected), 0, "{}", stderr(&inspected));
    assert_eq!(json_line(&inspected)["result"]["name"], json!("demo"));

    let painted = pixelforge(&[
        "call",
        "--project",
        manifest_arg,
        "--request",
        r#"{"tool":"paint","params":{"x":0,"y":0,"colorIndex":1}}"#,
    ]);
    assert_eq!(status(&painted), 1, "{}", stderr(&painted));
    assert_eq!(json_line(&painted)["error"]["code"], "UnknownTool");

    let preview = pixelforge(&[
        "call",
        "--project",
        manifest_arg,
        "--preview",
        "--request",
        r#"{"tool":"inspectManifest"}"#,
    ]);
    assert_eq!(status(&preview), 2, "{}", stderr(&preview));

    let checkpoint = scratch.child("ckpt.json");
    let refused = pixelforge(&[
        "call",
        "--project",
        manifest_arg,
        "--checkpoint",
        checkpoint.to_str().unwrap(),
        "--request",
        r#"{"tool":"inspectManifest"}"#,
    ]);
    assert_eq!(status(&refused), 2, "{}", stderr(&refused));
    assert!(!checkpoint.exists());

    let tools = pixelforge(&["tools"]);
    assert_eq!(status(&tools), 0);
    let names = json_line(&tools)["result"]["manifestTools"].to_string();
    assert!(names.contains("inspectManifest"));
    assert!(names.contains("validateManifest"));
    assert!(!json_line(&tools)["result"]["tools"]
        .to_string()
        .contains("inspectManifest"));
}

#[test]
fn game_pack_zombie_manifest_passes_validate() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/game-pack-zombie");
    let manifest = root.join("pixelforge.manifest.json");
    let output = pixelforge(&[
        "project",
        "validate",
        "--manifest",
        manifest.to_str().unwrap(),
    ]);
    assert_eq!(status(&output), 0, "{}", stderr(&output));
    let body = json_line(&output);
    assert_eq!(body["ok"], json!(true));
    assert_eq!(body["result"]["errorCount"], json!(0));
    assert_eq!(body["result"]["warningCount"], json!(0));
    assert_eq!(body["result"]["checks"]["perspective"], json!("top"));
    assert_eq!(body["result"]["checks"]["canvases"][0]["width"], json!(64));
    assert_eq!(body["result"]["checks"]["canvases"][0]["height"], json!(64));
    assert_eq!(body["result"]["checks"]["layout"][0]["projects"], json!(1));
    assert_eq!(body["result"]["checks"]["layout"][1]["projects"], json!(1));
    assert!(stderr(&output).contains("project validate: ok"));
}
