//! Artifact destinations must never destroy their source or history.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "pixelforge-outputs-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pixelforge"))
        .args(args)
        .output()
        .unwrap()
}
fn text(path: &Path) -> &str {
    path.to_str().unwrap()
}
fn ok(args: &[&str]) {
    let out = run(args);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
fn project(path: &Path) {
    ok(&[
        "new",
        "--width",
        "16",
        "--height",
        "16",
        "--output",
        text(path),
    ]);
}
fn conflict(args: &[&str]) {
    let out = run(args);
    assert_eq!(
        out.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["error"]["code"], "OutputConflict");
}

#[test]
fn preview_diff_sheet_and_apng_reject_the_project_and_lock_as_destinations() {
    let scratch = Scratch::new();
    let p = scratch.path("hero.json");
    project(&p);
    let before = fs::read(&p).unwrap();
    for args in [
        vec!["preview", "--project", text(&p), "--output", text(&p)],
        vec![
            "diff",
            "--project",
            text(&p),
            "--frame",
            "0",
            "--with",
            "0",
            "--output",
            text(&p),
        ],
        vec![
            "export",
            "sheet",
            "--project",
            text(&p),
            "--output",
            text(&p),
        ],
        vec![
            "export",
            "apng",
            "--project",
            text(&p),
            "--output",
            text(&p),
        ],
    ] {
        conflict(&args);
        assert_eq!(fs::read(&p).unwrap(), before);
    }
    let lock = scratch.path("hero.json.pixelforge.lock");
    let lock_before = fs::read(&lock).unwrap();
    conflict(&["preview", "--project", text(&p), "--output", text(&lock)]);
    assert_eq!(fs::read(&lock).unwrap(), lock_before);
}

#[test]
fn checkpoint_and_new_output_aliases_are_checked_before_any_publish() {
    let scratch = Scratch::new();
    let p = scratch.path("hero.json");
    let ck = scratch.path("hero.ckpt.json");
    project(&p);
    ok(&[
        "paint",
        "--project",
        text(&p),
        "--checkpoint",
        text(&ck),
        "--x",
        "0",
        "--y",
        "0",
        "--color-index",
        "1",
    ]);
    let before = fs::read(&ck).unwrap();
    conflict(&[
        "preview",
        "--project",
        text(&p),
        "--checkpoint",
        text(&ck),
        "--output",
        text(&ck),
    ]);
    assert_eq!(fs::read(&ck).unwrap(), before);
    let output = scratch.path("new/sheet.png");
    let alias = scratch.path("new/./sheet.png");
    conflict(&[
        "export",
        "sheet",
        "--project",
        text(&p),
        "--output",
        text(&output),
        "--atlas",
        text(&alias),
    ]);
    assert!(!output.exists());
    assert!(!scratch.path("new").exists());
}

#[test]
fn png_sequence_preflights_later_frames_before_writing_the_first_one() {
    let scratch = Scratch::new();
    let p = scratch.path("pixelforge-001.png");
    project(&p);
    ok(&["frame", "add", "--project", text(&p)]);
    let before = fs::read(&p).unwrap();
    conflict(&[
        "export",
        "png",
        "--project",
        text(&p),
        "--out-dir",
        text(&scratch.0),
    ]);
    assert!(!scratch.path("pixelforge-000.png").exists());
    assert_eq!(fs::read(&p).unwrap(), before);
}

#[test]
fn hard_link_destination_cannot_overwrite_a_document() {
    let scratch = Scratch::new();
    let p = scratch.path("hero.json");
    let alias = scratch.path("alias.png");
    project(&p);
    fs::hard_link(&p, &alias).unwrap();
    let before = fs::read(&p).unwrap();
    conflict(&["preview", "--project", text(&p), "--output", text(&alias)]);
    assert_eq!(fs::read(&p).unwrap(), before);
    assert_eq!(fs::read(&alias).unwrap(), before);
}

#[cfg(unix)]
#[test]
fn symlink_and_parent_aliases_cannot_overwrite_a_document() {
    let scratch = Scratch::new();
    let p = scratch.path("hero.json");
    project(&p);
    let before = fs::read(&p).unwrap();
    let alias = scratch.path("alias.png");
    std::os::unix::fs::symlink(&p, &alias).unwrap();
    conflict(&["preview", "--project", text(&p), "--output", text(&alias)]);
    let parent = scratch.path("alias-dir");
    std::os::unix::fs::symlink(&scratch.0, &parent).unwrap();
    let alias = parent.join("hero.json");
    conflict(&["preview", "--project", text(&p), "--output", text(&alias)]);
    assert_eq!(fs::read(&p).unwrap(), before);
}

#[test]
fn an_atomic_export_failure_keeps_the_previous_artifact() {
    let scratch = Scratch::new();
    let p = scratch.path("hero.json");
    let output = scratch.path("preview.png");
    project(&p);
    fs::write(&output, b"previous artifact").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_pixelforge"))
        .args(["preview", "--project", text(&p), "--output", text(&output)])
        .env("PIXELFORGE_TEST_FAIL_SAVE", "replace")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(fs::read(&output).unwrap(), b"previous artifact");
    assert!(!fs::read_dir(&scratch.0).unwrap().any(|p| p
        .unwrap()
        .file_name()
        .to_string_lossy()
        .ends_with(".tmp")));
}

#[test]
fn tileset_and_tilemap_exports_protect_inputs_before_writing_metadata() {
    let scratch = Scratch::new();
    let p = scratch.path("tiles.json");
    project(&p);
    ok(&[
        "tileset",
        "set",
        "--project",
        text(&p),
        "--tile-width",
        "16",
        "--tile-height",
        "16",
    ]);
    let meta = scratch.path("tiles.meta.json");
    let before = fs::read(&p).unwrap();
    conflict(&[
        "export",
        "tileset",
        "--project",
        text(&p),
        "--output",
        text(&p),
        "--meta",
        text(&meta),
    ]);
    assert_eq!(fs::read(&p).unwrap(), before);
    assert!(!meta.exists());
    let map = scratch.path("map.json");
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base-tiles/map.json"),
        &map,
    )
    .unwrap();
    let before = fs::read(&map).unwrap();
    let tmj = scratch.path("map.tmj");
    conflict(&[
        "export",
        "tilemap",
        "--map",
        text(&map),
        "--tileset",
        text(&p),
        "--output",
        text(&map),
        "--tmj",
        text(&tmj),
    ]);
    assert_eq!(fs::read(&map).unwrap(), before);
    assert!(!tmj.exists());
}
