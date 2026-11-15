//! Optional content fingerprint. The token is the canonical project text, not a counter.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use pixelforge_kernel::Document;
use serde_json::Value;

fn command(args: &[&str], env: &[(&str, &str)]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_pixelforge"));
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    for (key, value) in env {
        cmd.env(key, value);
    }
    cmd
}

fn pixelforge(args: &[&str]) -> Output {
    command(args, &[]).output().expect("spawn")
}

fn json_lines(output: &Output) -> Vec<Value> {
    String::from_utf8(output.stdout.clone())
        .unwrap()
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_str(line).unwrap_or_else(|err| panic!("{err} in {line}")))
        .collect()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

fn status(output: &Output) -> i32 {
    output.status.code().expect("exited")
}

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("pixelforge-rev-{nanos}-{n}-{}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn child(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn new_project(path: &Path) {
    let output = pixelforge(&[
        "new",
        "--width",
        "8",
        "--height",
        "8",
        "--output",
        path.to_str().unwrap(),
    ]);
    assert_eq!(status(&output), 0, "{}", stderr(&output));
}

fn scene(path: &Path) -> Value {
    let output = pixelforge(&["scene", "--project", path.to_str().unwrap()]);
    assert_eq!(status(&output), 0, "{}", stderr(&output));
    json_lines(&output).pop().unwrap()
}

fn revision_of(body: &Value) -> String {
    let top = body["revision"].as_str().unwrap().to_string();
    let nested = body["result"]["revision"].as_str().unwrap();
    assert_eq!(top, nested);
    top
}

fn canonical_revision(path: &Path) -> String {
    let text = fs::read_to_string(path).unwrap();
    let document = Document::load_project(&text).unwrap();
    fnv(&document.save_project())
}

fn fnv(text: &str) -> String {
    const OFFSET: u64 = 14_695_981_039_346_656_037;
    const PRIME: u64 = 1_099_511_628_211;
    let mut hash = OFFSET;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    format!("{hash:016x}")
}

fn paint(path: &Path, x: &str, expected: Option<&str>) -> Output {
    let mut args = vec![
        "paint",
        "--project",
        path.to_str().unwrap(),
        "--x",
        x,
        "--y",
        "0",
        "--color-index",
        "1",
    ];
    if let Some(expected) = expected {
        args.push("--expected-revision");
        args.push(expected);
    }
    pixelforge(&args)
}

fn assert_conflict(output: &Output, expected: &str) {
    assert_eq!(status(output), 2, "{}", stderr(output));
    let lines = json_lines(output);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0]["ok"], false);
    assert_eq!(lines[0]["error"]["code"], "RevisionConflict");
    assert_eq!(lines[0]["error"]["expected"], expected);
    assert!(lines[0]["error"]["current"].is_string());
    assert!(lines[0]["error"]["hint"].is_string());
    assert!(!stderr(output).contains("wrote"));
    let stdout = String::from_utf8(output.stdout.clone()).unwrap();
    assert!(!stdout.contains("\"ok\":true"), "{stdout}");
}

#[test]
fn scene_revision_matches_the_loaded_snapshot_and_a_reread_does_not_change_it() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    new_project(&project);
    let before = fs::read(&project).unwrap();
    let first = scene(&project);
    let token = revision_of(&first);
    assert_eq!(token, canonical_revision(&project));
    assert_eq!(first["result"]["width"], 8);
    let second = scene(&project);
    assert_eq!(revision_of(&second), token);
    assert_eq!(fs::read(&project).unwrap(), before);
}

#[test]
fn an_edit_without_a_token_still_saves_and_returns_the_new_fingerprint() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    new_project(&project);
    let before = revision_of(&scene(&project));
    let painted = paint(&project, "1", None);
    assert_eq!(status(&painted), 0, "{}", stderr(&painted));
    let after = revision_of(&json_lines(&painted)[0]);
    assert_ne!(after, before);
    assert_eq!(after, canonical_revision(&project));
    assert_eq!(
        Document::load_project(&fs::read_to_string(&project).unwrap())
            .unwrap()
            .get(1, 0)
            .unwrap(),
        1
    );
}

#[test]
fn the_same_token_lets_one_edit_through_and_the_next_conflicts_without_writing() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    let checkpoint = scratch.child("hero.ckpt.json");
    new_project(&project);
    let original = revision_of(&scene(&project));
    let first = command(
        &[
            "paint",
            "--project",
            project.to_str().unwrap(),
            "--checkpoint",
            checkpoint.to_str().unwrap(),
            "--expected-revision",
            &original,
            "--x",
            "0",
            "--y",
            "0",
            "--color-index",
            "1",
        ],
        &[],
    )
    .output()
    .unwrap();
    assert_eq!(status(&first), 0, "{}", stderr(&first));
    let saved = fs::read(&project).unwrap();
    let sidecar = fs::read(&checkpoint).unwrap();
    let current = revision_of(&json_lines(&first)[0]);
    assert_ne!(current, original);
    let conflict = command(
        &[
            "paint",
            "--project",
            project.to_str().unwrap(),
            "--checkpoint",
            checkpoint.to_str().unwrap(),
            "--expected-revision",
            &original,
            "--x",
            "1",
            "--y",
            "0",
            "--color-index",
            "2",
        ],
        &[],
    )
    .output()
    .unwrap();
    assert_conflict(&conflict, &original);
    assert_eq!(fs::read(&project).unwrap(), saved);
    assert_eq!(fs::read(&checkpoint).unwrap(), sidecar);
    assert_eq!(
        Document::load_project(&fs::read_to_string(&project).unwrap())
            .unwrap()
            .get(1, 0)
            .unwrap(),
        0
    );

    let again = scene(&project);
    assert_eq!(revision_of(&again), current);
    let second = paint(&project, "1", Some(&current));
    assert_eq!(status(&second), 0, "{}", stderr(&second));
    assert_eq!(
        Document::load_project(&fs::read_to_string(&project).unwrap())
            .unwrap()
            .get(1, 0)
            .unwrap(),
        1
    );
}

#[test]
fn a_no_op_keeps_the_fingerprint_and_a_same_size_edit_changes_it() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    new_project(&project);
    let painted = paint(&project, "0", None);
    assert_eq!(status(&painted), 0, "{}", stderr(&painted));
    let token = revision_of(&json_lines(&painted)[0]);
    let bytes = fs::read(&project).unwrap();
    let again = paint(&project, "0", Some(&token));
    assert_eq!(status(&again), 0, "{}", stderr(&again));
    assert_eq!(revision_of(&json_lines(&again)[0]), token);
    assert_eq!(fs::read(&project).unwrap(), bytes);

    let modified = fs::metadata(&project).unwrap().modified().unwrap();
    let mut text = String::from_utf8(bytes.clone()).unwrap();
    let at = text.find("\"pixels\"").expect("pixels");
    let raw = text.as_bytes();
    let mut found = None;
    for index in at..raw.len() {
        if raw[index] == b'1'
            && (index == 0 || !raw[index - 1].is_ascii_digit())
            && (index + 1 == raw.len() || !raw[index + 1].is_ascii_digit())
        {
            found = Some(index);
            break;
        }
    }
    let index = found.expect("single-digit 1");
    text.replace_range(index..index + 1, "2");
    assert_eq!(text.len(), bytes.len());
    fs::write(&project, &text).unwrap();
    File::options()
        .write(true)
        .open(&project)
        .unwrap()
        .set_modified(modified)
        .unwrap();
    let meta = fs::metadata(&project).unwrap();
    assert_eq!(meta.len() as usize, bytes.len());
    assert_eq!(meta.modified().unwrap(), modified);
    let observed = scene(&project);
    assert_ne!(revision_of(&observed), token);
    assert_eq!(revision_of(&observed), canonical_revision(&project));
    assert_eq!(observed["result"]["width"], 8);
}

#[test]
fn extra_whitespace_does_not_change_the_canonical_fingerprint() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    new_project(&project);
    let token = revision_of(&scene(&project));
    let mut file = fs::OpenOptions::new().append(true).open(&project).unwrap();
    file.write_all(b"\n").unwrap();
    assert_eq!(revision_of(&scene(&project)), token);
    assert_eq!(token, canonical_revision(&project));
}

#[test]
fn a_bad_token_is_not_ignored_and_create_rejects_the_flag() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    new_project(&project);
    let before = fs::read(&project).unwrap();
    let bad = paint(&project, "3", Some("not-a-token"));
    assert_eq!(status(&bad), 2, "{}", stderr(&bad));
    assert_eq!(json_lines(&bad)[0]["error"]["code"], "RevisionInvalid");
    assert_eq!(fs::read(&project).unwrap(), before);

    let created = scratch.child("other.json");
    let refused = pixelforge(&[
        "new",
        "--output",
        created.to_str().unwrap(),
        "--expected-revision",
        "0123456789abcdef",
    ]);
    assert_eq!(status(&refused), 2, "{}", stderr(&refused));
    assert_eq!(
        json_lines(&refused)[0]["error"]["code"],
        "RevisionUnsupported"
    );
    assert!(!created.exists());

    let imported = pixelforge(&[
        "import-aseprite",
        "--file",
        scratch.child("missing.ase").to_str().unwrap(),
        "--output",
        scratch.child("imported.json").to_str().unwrap(),
        "--expected-revision",
        "0123456789abcdef",
    ]);
    assert_eq!(status(&imported), 2, "{}", stderr(&imported));
    assert_eq!(
        json_lines(&imported)[0]["error"]["code"],
        "RevisionUnsupported"
    );
    assert!(!scratch.child("imported.json").exists());
}

#[test]
fn a_batch_checks_the_starting_snapshot_once_and_still_keeps_a_failed_prefix() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    new_project(&project);
    let token = revision_of(&scene(&project));
    let batch = pixelforge(&[
        "call",
        "--project",
        project.to_str().unwrap(),
        "--expected-revision",
        &token,
        "--request",
        r#"{"tool":"paint","params":{"x":0,"y":0,"colorIndex":1}}"#,
        "--request",
        r#"{"tool":"paint","params":{"x":1,"y":0,"colorIndex":2}}"#,
    ]);
    assert_eq!(status(&batch), 0, "{}", stderr(&batch));
    let lines = json_lines(&batch);
    assert_eq!(lines.len(), 2);
    let committed = revision_of(&lines[1]);
    assert_eq!(committed, canonical_revision(&project));
    let doc = Document::load_project(&fs::read_to_string(&project).unwrap()).unwrap();
    assert_eq!(doc.get(0, 0).unwrap(), 1);
    assert_eq!(doc.get(1, 0).unwrap(), 2);

    let stale = pixelforge(&[
        "call",
        "--project",
        project.to_str().unwrap(),
        "--expected-revision",
        &token,
        "--request",
        r#"{"tool":"paint","params":{"x":2,"y":0,"colorIndex":3}}"#,
    ]);
    assert_conflict(&stale, &token);
    assert_eq!(
        Document::load_project(&fs::read_to_string(&project).unwrap())
            .unwrap()
            .get(2, 0)
            .unwrap(),
        0
    );

    let prefix = pixelforge(&[
        "call",
        "--project",
        project.to_str().unwrap(),
        "--expected-revision",
        &committed,
        "--request",
        r#"{"tool":"paint","params":{"x":3,"y":0,"colorIndex":4}}"#,
        "--request",
        r#"{"tool":"paint","params":{"x":99,"y":0,"colorIndex":4}}"#,
    ]);
    assert_eq!(status(&prefix), 1, "{}", stderr(&prefix));
    let prefix_lines = json_lines(&prefix);
    assert_eq!(prefix_lines[0]["ok"], true);
    assert_eq!(prefix_lines[1]["error"]["code"], "OutOfBounds");
    let doc = Document::load_project(&fs::read_to_string(&project).unwrap()).unwrap();
    assert_eq!(doc.get(3, 0).unwrap(), 4);
    assert_eq!(doc.get(0, 0).unwrap(), 1);
}

#[test]
fn two_editors_with_one_token_do_not_both_write() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    let barrier = scratch.child("barrier");
    fs::create_dir_all(&barrier).unwrap();
    new_project(&project);
    let token = revision_of(&scene(&project));
    let mut holder = command(
        &[
            "paint",
            "--project",
            project.to_str().unwrap(),
            "--expected-revision",
            &token,
            "--x",
            "0",
            "--y",
            "0",
            "--color-index",
            "1",
        ],
        &[("PIXELFORGE_TEST_LOCK_BARRIER", barrier.to_str().unwrap())],
    )
    .spawn()
    .unwrap();
    let held = barrier.join(format!("held-{}", holder.id()));
    let deadline = Instant::now() + Duration::from_secs(20);
    while !held.is_file() {
        if Instant::now() > deadline {
            let _ = holder.kill();
            panic!("holder did not take the lock");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut other = command(
        &[
            "paint",
            "--project",
            project.to_str().unwrap(),
            "--expected-revision",
            &token,
            "--x",
            "1",
            "--y",
            "0",
            "--color-index",
            "2",
        ],
        &[("PIXELFORGE_LOCK_TIMEOUT_MS", "20000")],
    )
    .spawn()
    .unwrap();
    fs::write(barrier.join(format!("release-{}", holder.id())), b"1").unwrap();
    let first = holder.wait_with_output().unwrap();
    let second = other.wait_with_output().unwrap();
    assert_eq!(status(&first), 0, "{}", stderr(&first));
    assert_conflict(&second, &token);
    let doc = Document::load_project(&fs::read_to_string(&project).unwrap()).unwrap();
    assert_eq!(doc.get(0, 0).unwrap(), 1);
    assert_eq!(doc.get(1, 0).unwrap(), 0);
}

#[cfg(unix)]
#[test]
fn a_symlink_sees_the_same_fingerprint_as_the_target() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    new_project(&project);
    let link = scratch.child("alias.json");
    std::os::unix::fs::symlink(&project, &link).unwrap();
    let token = revision_of(&scene(&link));
    assert_eq!(token, revision_of(&scene(&project)));
    let painted = paint(&link, "4", Some(&token));
    assert_eq!(status(&painted), 0, "{}", stderr(&painted));
    assert_eq!(
        Document::load_project(&fs::read_to_string(&project).unwrap())
            .unwrap()
            .get(4, 0)
            .unwrap(),
        1
    );
}

#[test]
fn tileset_set_and_clear_do_not_drop_expected_revision() {
    let scratch = Scratch::new();
    let project = scratch.child("tile.json");
    let checkpoint = scratch.child("tile.ckpt.json");
    let created = pixelforge(&[
        "new",
        "--width",
        "16",
        "--height",
        "16",
        "--output",
        project.to_str().unwrap(),
    ]);
    assert_eq!(status(&created), 0, "{}", stderr(&created));
    let original = revision_of(&scene(&project));
    let painted = paint(&project, "0", None);
    assert_eq!(status(&painted), 0, "{}", stderr(&painted));
    let current = revision_of(&json_lines(&painted)[0]);
    assert_ne!(current, original);
    let painted_bytes = fs::read(&project).unwrap();

    let stale_set = pixelforge(&[
        "tileset",
        "set",
        "--project",
        project.to_str().unwrap(),
        "--checkpoint",
        checkpoint.to_str().unwrap(),
        "--expected-revision",
        &original,
        "--tile-width",
        "16",
        "--tile-height",
        "16",
    ]);
    assert_conflict(&stale_set, &original);
    assert_eq!(fs::read(&project).unwrap(), painted_bytes);
    assert!(!checkpoint.exists());

    let bad_set = pixelforge(&[
        "tileset",
        "set",
        "--project",
        project.to_str().unwrap(),
        "--checkpoint",
        checkpoint.to_str().unwrap(),
        "--expected-revision",
        "not-a-token",
        "--tile-width",
        "16",
        "--tile-height",
        "16",
    ]);
    assert_eq!(status(&bad_set), 2, "{}", stderr(&bad_set));
    assert_eq!(json_lines(&bad_set)[0]["error"]["code"], "RevisionInvalid");
    assert_eq!(json_lines(&bad_set).len(), 1);
    assert_eq!(fs::read(&project).unwrap(), painted_bytes);
    assert!(!checkpoint.exists());

    let set = pixelforge(&[
        "tileset",
        "set",
        "--project",
        project.to_str().unwrap(),
        "--checkpoint",
        checkpoint.to_str().unwrap(),
        "--expected-revision",
        &current,
        "--tile-width",
        "16",
        "--tile-height",
        "16",
    ]);
    assert_eq!(status(&set), 0, "{}", stderr(&set));
    let saved = fs::read(&project).unwrap();
    let sidecar = fs::read(&checkpoint).unwrap();
    assert_ne!(saved, painted_bytes);
    assert!(String::from_utf8_lossy(&saved).contains("tileset"));

    let stale_clear = pixelforge(&[
        "tileset",
        "clear",
        "--project",
        project.to_str().unwrap(),
        "--checkpoint",
        checkpoint.to_str().unwrap(),
        "--expected-revision",
        &current,
    ]);
    assert_conflict(&stale_clear, &current);
    assert_eq!(fs::read(&project).unwrap(), saved);
    assert_eq!(fs::read(&checkpoint).unwrap(), sidecar);

    let bad_clear = pixelforge(&[
        "tileset",
        "clear",
        "--project",
        project.to_str().unwrap(),
        "--checkpoint",
        checkpoint.to_str().unwrap(),
        "--expected-revision",
        "NOTOKEN",
    ]);
    assert_eq!(status(&bad_clear), 2, "{}", stderr(&bad_clear));
    assert_eq!(
        json_lines(&bad_clear)[0]["error"]["code"],
        "RevisionInvalid"
    );
    assert_eq!(fs::read(&project).unwrap(), saved);
    assert_eq!(fs::read(&checkpoint).unwrap(), sidecar);

    let after_set = revision_of(&scene(&project));
    let cleared = pixelforge(&[
        "tileset",
        "clear",
        "--project",
        project.to_str().unwrap(),
        "--checkpoint",
        checkpoint.to_str().unwrap(),
        "--expected-revision",
        &after_set,
    ]);
    assert_eq!(status(&cleared), 0, "{}", stderr(&cleared));
    assert!(!String::from_utf8_lossy(&fs::read(&project).unwrap()).contains("\"tileset\""));

    let plain = scratch.child("plain.json");
    let made = pixelforge(&[
        "new",
        "--width",
        "16",
        "--height",
        "16",
        "--output",
        plain.to_str().unwrap(),
    ]);
    assert_eq!(status(&made), 0, "{}", stderr(&made));
    let unguarded = pixelforge(&[
        "tileset",
        "set",
        "--project",
        plain.to_str().unwrap(),
        "--tile-width",
        "16",
        "--tile-height",
        "16",
    ]);
    assert_eq!(status(&unguarded), 0, "{}", stderr(&unguarded));
    assert!(fs::read_to_string(&plain).unwrap().contains("tileset"));
    let plain_bytes = fs::read(&plain).unwrap();

    let validate = pixelforge(&[
        "tileset",
        "validate",
        "--project",
        plain.to_str().unwrap(),
        "--expected-revision",
        &original,
    ]);
    assert_conflict(&validate, &original);
    assert_eq!(fs::read(&plain).unwrap(), plain_bytes);
}

#[test]
fn tilemap_subcommand_does_not_silently_accept_the_flag() {
    let output = pixelforge(&["tilemap", "set", "--expected-revision", "0123456789abcdef"]);
    assert_eq!(status(&output), 2, "{}", stderr(&output));
    assert!(
        stderr(&output).contains("expected-revision") || stderr(&output).contains("unexpected")
    );
    assert!(json_lines(&output).is_empty());
}
