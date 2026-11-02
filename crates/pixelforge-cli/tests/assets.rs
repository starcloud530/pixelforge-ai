//! CLI coverage for the asset registry on a pixelforge-manifest.

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
            "pixelforge-assets-{}-{}",
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

fn manifest_path(scratch: &Scratch) -> PathBuf {
    let hero = scratch.child("hero.json");
    let output = pixelforge(&[
        "new",
        "--width",
        "16",
        "--height",
        "16",
        "--output",
        hero.to_str().unwrap(),
    ]);
    assert_eq!(status(&output), 0, "{}", stderr(&output));
    fs::create_dir(scratch.child("sprites")).unwrap();
    fs::copy(&hero, scratch.child("sprites/hero.json")).unwrap();
    let manifest = scratch.child("pixelforge.manifest.json");
    let output = pixelforge(&[
        "project",
        "init",
        "--name",
        "demo",
        "--perspective",
        "top",
        "--note",
        "Top-down sprites. The registry names files; it does not store pixels.",
        "--palette",
        "sprites/hero.json",
        "--canvas",
        "sprite:16x16",
        "--layout",
        "characters:sprites:character:sprite",
        "--output",
        manifest.to_str().unwrap(),
    ]);
    assert_eq!(status(&output), 0, "{}", stderr(&output));
    manifest
}

fn manifest_arg(path: &Path) -> &str {
    path.to_str().unwrap()
}

#[test]
fn add_list_show_graph_and_reload_the_same_id() {
    let scratch = Scratch::new();
    let manifest = manifest_path(&scratch);
    let arg = manifest_arg(&manifest);

    let added = pixelforge(&[
        "asset",
        "add",
        "--manifest",
        arg,
        "--id",
        "hero",
        "--kind",
        "character",
        "--path",
        "sprites/hero.json",
        "--produces",
        "hero.walk",
    ]);
    assert_eq!(status(&added), 0, "{}", stderr(&added));
    assert_eq!(json_line(&added)["result"]["id"], json!("hero"));
    assert!(stderr(&added).contains("wrote"));

    let clip = pixelforge(&[
        "asset",
        "add",
        "--manifest",
        arg,
        "--id",
        "hero.walk",
        "--kind",
        "clip",
        "--path",
        "sprites/hero.json",
        "--owner",
        "hero",
        "--depends-on",
        "hero",
        "--tag",
        "walk_down",
        "--tag",
        "walk_left",
        "--tag",
        "walk_right",
        "--tag",
        "walk_up",
    ]);
    assert_eq!(status(&clip), 0, "{}", stderr(&clip));

    let listed = pixelforge(&[
        "asset",
        "list",
        "--manifest",
        arg,
        "--kind",
        "clip",
        "--tag",
        "walk_down",
    ]);
    assert_eq!(status(&listed), 0, "{}", stderr(&listed));
    assert_eq!(
        json_line(&listed)["result"]["assets"][0]["id"],
        json!("hero.walk")
    );

    let shown = pixelforge(&["asset", "show", "--manifest", arg, "hero"]);
    assert_eq!(status(&shown), 0, "{}", stderr(&shown));
    assert_eq!(json_line(&shown)["result"]["owns"], json!(["hero.walk"]));
    assert_eq!(
        json_line(&shown)["result"]["dependents"],
        json!(["hero.walk"])
    );

    let graph = pixelforge(&["asset", "graph", "--manifest", arg, "hero"]);
    assert_eq!(status(&graph), 0, "{}", stderr(&graph));
    let body = json_line(&graph);
    assert_eq!(body["result"]["missing"], json!([]));
    assert!(body["result"]["edges"]
        .as_array()
        .unwrap()
        .iter()
        .any(|edge| edge["rel"] == "produces" && edge["to"] == "hero.walk"));

    let used = pixelforge(&["asset", "dependents", "--manifest", arg, "hero"]);
    assert_eq!(status(&used), 0, "{}", stderr(&used));
    assert_eq!(
        json_line(&used)["result"]["dependents"][0]["id"],
        json!("hero.walk")
    );

    let reloaded = pixelforge(&["asset", "show", "--manifest", arg, "hero.walk"]);
    assert_eq!(status(&reloaded), 0, "{}", stderr(&reloaded));
    assert_eq!(
        json_line(&reloaded)["result"]["asset"]["tags"],
        json!(["walk_down", "walk_left", "walk_right", "walk_up"])
    );
    let text = fs::read_to_string(&manifest).unwrap();
    assert!(text.contains("\"id\": \"hero.walk\""));
    assert!(!text.contains("\"status\""));

    let validated = pixelforge(&["asset", "validate", "--manifest", arg]);
    assert_eq!(status(&validated), 0, "{}", stderr(&validated));
    assert_eq!(json_line(&validated)["result"]["errorCount"], json!(0));
    assert_eq!(json_line(&validated)["result"]["assets"], json!(2));

    let project = pixelforge(&["project", "validate", "--manifest", arg]);
    assert_eq!(status(&project), 0, "{}", stderr(&project));
    assert_eq!(json_line(&project)["result"]["checks"]["assets"], json!(2));
}

#[test]
fn validate_reports_a_missing_path_a_cycle_and_a_blocked_remove() {
    let scratch = Scratch::new();
    let manifest = manifest_path(&scratch);
    let arg = manifest_arg(&manifest);

    let added = pixelforge(&[
        "asset",
        "add",
        "--manifest",
        arg,
        "--id",
        "hero",
        "--kind",
        "character",
        "--path",
        "sprites/gone.json",
    ]);
    assert_eq!(status(&added), 0, "{}", stderr(&added));
    let validated = pixelforge(&["asset", "validate", "--manifest", arg]);
    assert_eq!(status(&validated), 1, "{}", stderr(&validated));
    assert_eq!(
        json_line(&validated)["result"]["errors"][0]["code"],
        json!("assetMissing")
    );

    let fixed = pixelforge(&[
        "asset",
        "set",
        "--manifest",
        arg,
        "--id",
        "hero",
        "--path",
        "sprites/hero.json",
    ]);
    assert_eq!(status(&fixed), 0, "{}", stderr(&fixed));

    pixelforge(&[
        "asset",
        "add",
        "--manifest",
        arg,
        "--id",
        "pack",
        "--kind",
        "group",
        "--path",
        "sprites/hero.json",
        "--owner",
        "hero",
    ]);
    let cycled = pixelforge(&[
        "asset",
        "set",
        "--manifest",
        arg,
        "--id",
        "hero",
        "--owner",
        "pack",
    ]);
    assert_eq!(status(&cycled), 0, "{}", stderr(&cycled));
    let validated = pixelforge(&["asset", "validate", "--manifest", arg]);
    assert_eq!(status(&validated), 1, "{}", stderr(&validated));
    assert!(json_line(&validated)["result"]["errors"]
        .as_array()
        .unwrap()
        .iter()
        .any(|error| error["code"] == "assetOwner"));

    let blocked = pixelforge(&["asset", "remove", "--manifest", arg, "--id", "pack"]);
    assert_eq!(status(&blocked), 1, "{}", stderr(&blocked));
    assert!(stderr(&blocked).contains("hero"));

    let cleared = pixelforge(&[
        "asset",
        "set",
        "--manifest",
        arg,
        "--id",
        "hero",
        "--clear-owner",
    ]);
    assert_eq!(status(&cleared), 0, "{}", stderr(&cleared));
    let removed = pixelforge(&["asset", "remove", "--manifest", arg, "--id", "pack"]);
    assert_eq!(status(&removed), 0, "{}", stderr(&removed));
    assert_eq!(json_line(&removed)["result"]["removed"], json!(true));

    let spaced = pixelforge(&[
        "asset",
        "add",
        "--manifest",
        arg,
        "--id",
        "hero.note",
        "--kind",
        "clip",
        "--path",
        "sprites/hero.json",
        "--owner",
        "hero",
        "--depends-on",
        "hero",
        "--tag",
        "walk down",
        "--tag",
        "行走",
        "--tag",
        "01_walk",
    ]);
    assert_eq!(status(&spaced), 0, "{}", stderr(&spaced));
    assert_eq!(
        json_line(&spaced)["result"]["tags"],
        json!(["walk down", "行走", "01_walk"])
    );
    let filtered = pixelforge(&["asset", "list", "--manifest", arg, "--tag", "walk down"]);
    assert_eq!(status(&filtered), 0, "{}", stderr(&filtered));
    assert_eq!(
        json_line(&filtered)["result"]["assets"][0]["id"],
        json!("hero.note")
    );
    let renamed = pixelforge(&[
        "asset",
        "set",
        "--manifest",
        arg,
        "--id",
        "hero.note",
        "--tag",
        "行走",
    ]);
    assert_eq!(status(&renamed), 0, "{}", stderr(&renamed));
    assert_eq!(json_line(&renamed)["result"]["tags"], json!(["行走"]));

    let call = pixelforge(&[
        "call",
        "--project",
        arg,
        "--request",
        r#"{"tool":"listAssets","params":{"kind":"character"}}"#,
    ]);
    assert_eq!(status(&call), 0, "{}", stderr(&call));
    assert_eq!(json_line(&call)["result"]["assets"][0]["id"], json!("hero"));
}
