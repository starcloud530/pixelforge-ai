//! `direction review` is read-only. The project bytes before and after match,
//! so every hash of those bytes matches.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use pixelforge_kernel::{CurveChannel, DirectionLayout, Document, LoopMode};
use serde_json::Value;

fn pixelforge(args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pixelforge"))
        .args(args)
        .output()
        .expect("spawn pixelforge")
}

fn arg(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_string()).collect()
}

fn json_line(output: &Output) -> Value {
    let text = String::from_utf8(output.stdout.clone()).expect("stdout utf-8");
    let line = text
        .lines()
        .find(|line| !line.is_empty())
        .expect("json line");
    serde_json::from_str(line).unwrap_or_else(|err| panic!("{err} in {line}"))
}

fn status(output: &Output) -> i32 {
    output.status.code().expect("exit")
}

fn fnv1a64(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new() -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "pixelforge-review-{nanos}-{n}-{}",
            std::process::id()
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

fn sheet(frames: usize) -> Document {
    let mut doc = Document::new(16, 16).unwrap();
    for _ in 1..frames {
        doc.add_frame();
    }
    doc
}

fn tag_all(doc: &mut Document, action: &str) {
    for index in 0..8 {
        let screen = [
            "down",
            "left",
            "right",
            "up",
            "downleft",
            "downright",
            "upleft",
            "upright",
        ][index];
        doc.add_tag(
            &format!("{action}_{screen}"),
            index,
            index,
            LoopMode::Forward,
        )
        .unwrap();
    }
}

fn save(doc: &Document, path: &Path) {
    fs::write(path, doc.save_project()).unwrap();
}

fn review(project: &Path, output: &Path, extra: &[&str]) -> Output {
    let mut parts = vec![
        "direction",
        "review",
        "--project",
        project.to_str().unwrap(),
        "--output",
        output.to_str().unwrap(),
    ];
    parts.extend_from_slice(extra);
    pixelforge(&arg(&parts))
}

#[test]
fn review_leaves_the_project_bytes_and_their_hash_unchanged() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    let mut doc = sheet(8);
    doc.set_direction_layout(DirectionLayout::new(8, 1).unwrap(), false)
        .unwrap();
    tag_all(&mut doc, "attack");
    doc.add_part("body", 6, 8, None).unwrap();
    doc.set_anchor("body", "sole", 0, 3).unwrap();
    for frame in 0..8 {
        doc.set_active_frame(frame).unwrap();
        doc.paint(6, 11, 3).unwrap();
    }
    save(&doc, &project);
    let before = fs::read(&project).unwrap();
    let before_hash = fnv1a64(&before);
    let output = scratch.child("out");
    let ran = review(
        &project,
        &output,
        &[
            "--action",
            "attack",
            "--at",
            "0",
            "--scale",
            "2",
            "--anchor",
            "body:sole",
            "--tolerance",
            "0",
        ],
    );
    let after = fs::read(&project).unwrap();
    assert_eq!(after, before, "project bytes changed");
    assert_eq!(fnv1a64(&after), before_hash);
    assert_eq!(status(&ran), 0, "{}", String::from_utf8_lossy(&ran.stderr));
    let body = json_line(&ran);
    assert_eq!(body["ok"], true);
    assert_eq!(body["tool"], "directionReview");
    assert_eq!(body["result"]["judgesArt"], false);
    assert_eq!(body["result"]["errorCount"], 0);
    assert_eq!(body["result"]["directions"][0]["borrowed"], false);
    assert!(body["result"]["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["code"] == "restPoseBaseline" && item["severity"] == "observation"));
    assert!(output.join("positions.png").is_file());
    assert!(output.join("aligned.png").is_file());
    assert!(output.join("overlay-up-upright.png").is_file());
    assert!(output.join("review.json").is_file());
    let png = fs::read(output.join("positions.png")).unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
}

#[test]
fn a_missing_facing_is_exit_1_and_the_file_hash_stays() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    let mut doc = sheet(8);
    doc.set_direction_layout(DirectionLayout::new(8, 1).unwrap(), false)
        .unwrap();
    for (index, screen) in [
        "down",
        "left",
        "right",
        "up",
        "downleft",
        "downright",
        "upright",
    ]
    .into_iter()
    .enumerate()
    {
        doc.add_tag(&format!("attack_{screen}"), index, index, LoopMode::Forward)
            .unwrap();
    }
    save(&doc, &project);
    let before = fs::read(&project).unwrap();
    let output = scratch.child("out");
    let ran = review(&project, &output, &["--action", "attack"]);
    assert_eq!(fs::read(&project).unwrap(), before);
    assert_eq!(fnv1a64(&fs::read(&project).unwrap()), fnv1a64(&before));
    assert_eq!(status(&ran), 1, "{}", String::from_utf8_lossy(&ran.stderr));
    let body = json_line(&ran);
    assert_eq!(body["ok"], true);
    let missing = body["result"]["directions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["screen"] == "upleft")
        .unwrap();
    assert_eq!(missing["slot"], "missingDirection");
    assert!(missing["frame"].is_null());
    assert_eq!(missing["borrowed"], false);
    assert!(output.join("aligned.png").exists() == false);
}

#[test]
fn curve_offset_is_reported_and_zombie_idle_is_not_walk() {
    let scratch = Scratch::new();
    let project = scratch.child("shifted.json");
    let mut doc = sheet(8);
    doc.set_direction_layout(DirectionLayout::new(8, 1).unwrap(), false)
        .unwrap();
    tag_all(&mut doc, "attack");
    doc.add_part("body", 6, 8, None).unwrap();
    doc.set_anchor("body", "sole", 0, 3).unwrap();
    for frame in 0..8 {
        let value = if frame == 2 { 3.0 } else { 0.0 };
        doc.set_curve_key("body", CurveChannel::TranslateY, frame, value, None)
            .unwrap();
        doc.set_active_frame(frame).unwrap();
        doc.paint(6, 11, 3).unwrap();
    }
    save(&doc, &project);
    let before = fs::read(&project).unwrap();
    let output = scratch.child("shift-out");
    let ran = review(
        &project,
        &output,
        &[
            "--action",
            "attack",
            "--foot-anchor",
            "body:sole",
            "--tolerance",
            "0",
        ],
    );
    assert_eq!(fs::read(&project).unwrap(), before);
    assert_eq!(status(&ran), 1);
    let body = json_line(&ran);
    let right = body["result"]["directions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["screen"] == "right")
        .unwrap();
    assert_eq!(right["foot"]["source"], "curve");
    assert_eq!(right["foot"]["y"], 14);
    assert_eq!(right["offset"]["y"], -3);
    assert_eq!(right["subjectBounds"]["y"], 11);

    let zombie = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/game-pack-zombie/zombie/zombie.json");
    let zombie_before = fs::read(&zombie).unwrap();
    let zombie_out = scratch.child("zombie-out");
    let idle = review(
        &zombie,
        &zombie_out,
        &["--action", "idle", "--at", "0", "--scale", "1"],
    );
    assert_eq!(fs::read(&zombie).unwrap(), zombie_before);
    assert_eq!(
        fnv1a64(&fs::read(&zombie).unwrap()),
        fnv1a64(&zombie_before)
    );
    assert_eq!(status(&idle), 1);
    let idle_body = json_line(&idle);
    let walk_out = scratch.child("zombie-walk");
    let walk = review(&zombie, &walk_out, &["--action", "walk", "--at", "0"]);
    assert_eq!(fs::read(&zombie).unwrap(), zombie_before);
    let walk_body = json_line(&walk);
    let idle_down = &idle_body["result"]["directions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["screen"] == "down")
        .unwrap()["frame"];
    let walk_down = &walk_body["result"]["directions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["screen"] == "down")
        .unwrap()["frame"];
    assert_ne!(idle_down, walk_down);
    assert!(zombie_out.join("positions.png").is_file());
}

#[test]
fn a_bad_action_exits_2_without_writing_the_project() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    save(&Document::new(8, 8).unwrap(), &project);
    let before = fs::read(&project).unwrap();
    let ran = review(
        &project,
        &scratch.child("out"),
        &["--action", "attack_down"],
    );
    assert_eq!(status(&ran), 2);
    assert_eq!(fs::read(&project).unwrap(), before);
    let body = json_line(&ran);
    assert_eq!(body["ok"], false);
}
