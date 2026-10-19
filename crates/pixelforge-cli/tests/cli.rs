//! Drives the `pixelforge` binary against temp project files.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use pixelforge_kernel::Document;
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

fn json_lines(output: &Output) -> Vec<Value> {
    let text = String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8");
    text.lines()
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_str(line).unwrap_or_else(|err| panic!("{err} in line {line}")))
        .collect()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is UTF-8")
}

fn status(output: &Output) -> i32 {
    output.status.code().expect("process exited")
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
        let path =
            std::env::temp_dir().join(format!("pixelforge-cli-{nanos}-{n}-{}", std::process::id()));
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

fn load(path: &Path) -> Document {
    let text = fs::read_to_string(path).unwrap();
    Document::load_project(&text).expect("project file")
}

fn png_size(bytes: &[u8]) -> (u32, u32) {
    assert!(
        bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "png magic, got {:02x?}",
        &bytes[..bytes.len().min(8)]
    );
    let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    (width, height)
}

#[test]
fn help_lists_the_agent_commands() {
    let output = pixelforge(&arg(&["--help"]));
    assert_eq!(status(&output), 0, "{}", stderr(&output));
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains("pixelforge"), "{help}");
    for word in [
        "new", "call", "paint", "export", "inspect", "Stdout", "Tool API",
    ] {
        assert!(help.contains(word), "help missing {word}:\n{help}");
    }
}

#[test]
fn new_paint_export_and_call_round_trip() {
    let scratch = Scratch::new();
    let project = scratch.child("walk.json");
    let project_arg = project.to_str().unwrap();

    let created = pixelforge(&arg(&[
        "new",
        "--width",
        "16",
        "--height",
        "16",
        "--output",
        project_arg,
    ]));
    assert_eq!(status(&created), 0, "{}", stderr(&created));
    let inspected = &json_lines(&created)[0];
    assert_eq!(inspected["ok"], true);
    assert_eq!(inspected["apiVersion"], 1);
    assert_eq!(inspected["result"]["width"], 16);
    assert_eq!(inspected["result"]["height"], 16);
    assert_eq!(inspected["result"]["frameCount"], 1);
    assert!(stderr(&created).contains("wrote"));
    let blank = fs::read(&project).unwrap();
    assert!(load(&project).pixels().iter().all(|index| *index == 0));

    let clobber = pixelforge(&arg(&[
        "new",
        "--width",
        "8",
        "--height",
        "8",
        "--output",
        project_arg,
    ]));
    assert_eq!(status(&clobber), 2, "{}", stderr(&clobber));
    assert_eq!(json_lines(&clobber)[0]["error"]["code"], "Cli");
    assert_eq!(fs::read(&project).unwrap(), blank);

    let painted = pixelforge(&arg(&[
        "paint",
        "--project",
        project_arg,
        "--x",
        "1",
        "--y",
        "2",
        "--color-index",
        "3",
    ]));
    assert_eq!(status(&painted), 0, "{}", stderr(&painted));
    let paint_body = &json_lines(&painted)[0];
    assert_eq!(paint_body["ok"], true);
    assert_eq!(paint_body["apiVersion"], 1);
    assert_eq!(paint_body["result"], serde_json::json!({}));
    assert_eq!(load(&project).get(1, 2).unwrap(), 3);

    let batch = pixelforge(&arg(&[
        "call",
        "--project",
        project_arg,
        "--request",
        r#"{"tool":"beginGesture"}"#,
        "--request",
        r#"{"tool":"paint","params":{"x":2,"y":2,"colorIndex":4}}"#,
        "--request",
        r#"{"tool":"paint","params":{"x":3,"y":2,"colorIndex":4}}"#,
        "--request",
        r#"{"tool":"endGesture"}"#,
        "--request",
        r#"{"tool":"duplicateFrame","params":{"index":0}}"#,
        "--request",
        r#"{"tool":"setActiveFrame","params":{"index":1}}"#,
        "--request",
        r#"{"tool":"setFrameDelay","params":{"index":1,"delayMs":120}}"#,
        "--request",
        r#"{"tool":"paint","params":{"x":0,"y":0,"colorIndex":1}}"#,
    ]));
    assert_eq!(status(&batch), 0, "{}", stderr(&batch));
    assert_eq!(json_lines(&batch).len(), 8);
    let doc = load(&project);
    assert_eq!(doc.frame_count(), 2);
    assert_eq!(doc.active_frame(), 1);
    assert_eq!(doc.frame_delay(1).unwrap(), 120);
    assert_eq!(doc.get(0, 0).unwrap(), 1);
    assert_eq!(doc.frame_pixels(0).unwrap()[2 * 16 + 1], 3);
    assert_eq!(doc.frame_pixels(0).unwrap()[2 * 16 + 2], 4);
    assert_eq!(doc.frame_pixels(0).unwrap()[2 * 16 + 3], 4);

    let before_export = fs::read(&project).unwrap();
    let frames_dir = scratch.child("frames");
    let sequence = pixelforge(&arg(&[
        "export",
        "png",
        "--project",
        project_arg,
        "--out-dir",
        frames_dir.to_str().unwrap(),
    ]));
    assert_eq!(status(&sequence), 0, "{}", stderr(&sequence));
    let sequence_body = &json_lines(&sequence)[0];
    assert_eq!(sequence_body["tool"], "exportPngSequence");
    assert_eq!(
        sequence_body["result"]["frames"].as_array().unwrap().len(),
        2
    );
    let first = fs::read(frames_dir.join("pixelforge-000.png")).unwrap();
    let second = fs::read(frames_dir.join("pixelforge-001.png")).unwrap();
    assert_eq!(png_size(&first), (16, 16));
    assert_eq!(png_size(&second), (16, 16));
    assert_ne!(first, second);
    assert_eq!(fs::read(&project).unwrap(), before_export);

    let sheet_path = scratch.child("walk-sheet.png");
    let sheet = pixelforge(&arg(&[
        "export",
        "sheet",
        "--project",
        project_arg,
        "--output",
        sheet_path.to_str().unwrap(),
    ]));
    assert_eq!(status(&sheet), 0, "{}", stderr(&sheet));
    let sheet_body = &json_lines(&sheet)[0];
    assert_eq!(sheet_body["ok"], true);
    assert_eq!(sheet_body["tool"], "exportSpriteSheet");
    assert_eq!(sheet_body["result"]["columns"], 2);
    assert_eq!(sheet_body["result"]["rows"], 1);
    assert_eq!(sheet_body["result"]["sheetWidth"], 32);
    assert_eq!(sheet_body["result"]["sheetHeight"], 16);
    assert!(sheet_body["result"].get("png").is_none());
    let sheet_bytes = fs::read(&sheet_path).unwrap();
    assert_eq!(png_size(&sheet_bytes), (32, 16));
    assert_eq!(fs::read(&project).unwrap(), before_export);

    let raw = pixelforge(&arg(&[
        "call",
        "--project",
        project_arg,
        "--request",
        r#"{"tool":"exportSpriteSheet"}"#,
    ]));
    assert_eq!(status(&raw), 0, "{}", stderr(&raw));
    let raw_body = &json_lines(&raw)[0];
    let png = raw_body["result"]["png"].as_str().unwrap();
    assert!(png.starts_with("iVBORw0KGgo"), "{png}");
    assert_eq!(fs::read(&project).unwrap(), before_export);
    assert_ne!(blank, before_export);
}

#[test]
fn one_process_undoes_a_gesture_and_the_next_process_cannot() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    let project_arg = project.to_str().unwrap();
    assert_eq!(
        status(&pixelforge(&arg(&[
            "new",
            "--width",
            "8",
            "--height",
            "8",
            "--output",
            project_arg,
        ]))),
        0
    );
    let original = fs::read(&project).unwrap();

    let undone = pixelforge(&arg(&[
        "call",
        "--project",
        project_arg,
        "--request",
        r#"{"tool":"beginGesture"}"#,
        "--request",
        r#"{"tool":"paint","params":{"x":0,"y":0,"colorIndex":1}}"#,
        "--request",
        r#"{"tool":"paint","params":{"x":1,"y":0,"colorIndex":1}}"#,
        "--request",
        r#"{"tool":"endGesture"}"#,
        "--request",
        r#"{"tool":"undo"}"#,
    ]));
    assert_eq!(status(&undone), 0, "{}", stderr(&undone));
    let lines = json_lines(&undone);
    assert_eq!(lines.len(), 5);
    assert_eq!(lines[4]["result"]["canUndo"], false);
    assert_eq!(lines[4]["result"]["canRedo"], true);
    assert_eq!(fs::read(&project).unwrap(), original);
    assert!(load(&project).pixels().iter().all(|index| *index == 0));

    let again = pixelforge(&arg(&["undo", "--project", project_arg]));
    assert_eq!(status(&again), 1, "{}", stderr(&again));
    let body = &json_lines(&again)[0];
    assert_eq!(body["ok"], false);
    assert_eq!(body["error"]["code"], "NothingToUndo");
    assert_eq!(fs::read(&project).unwrap(), original);
}

#[test]
fn a_refused_call_keeps_earlier_edits_and_skips_later_ones() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    let project_arg = project.to_str().unwrap();
    assert_eq!(
        status(&pixelforge(&arg(&["new", "--output", project_arg,]))),
        0
    );

    let batch = pixelforge(&arg(&[
        "call",
        "--project",
        project_arg,
        "--request",
        r#"{"tool":"paint","params":{"x":0,"y":0,"colorIndex":1}}"#,
        "--request",
        r#"{"tool":"paint","params":{"x":99,"y":0,"colorIndex":1}}"#,
        "--request",
        r#"{"tool":"paint","params":{"x":1,"y":0,"colorIndex":2}}"#,
    ]));
    assert_eq!(status(&batch), 1, "{}", stderr(&batch));
    let lines = json_lines(&batch);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["ok"], true);
    assert_eq!(lines[1]["ok"], false);
    assert_eq!(lines[1]["error"]["code"], "OutOfBounds");
    assert!(stderr(&batch).contains("OutOfBounds"));
    let doc = load(&project);
    assert_eq!(doc.get(0, 0).unwrap(), 1);
    assert_eq!(doc.get(1, 0).unwrap(), 0);
}

#[test]
fn locked_palette_refuses_recolor_and_still_paints() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    let project_arg = project.to_str().unwrap();
    assert_eq!(
        status(&pixelforge(&arg(&["new", "--output", project_arg]))),
        0
    );
    let before = load(&project).palette_colors()[1];

    let locked = pixelforge(&arg(&["palette", "lock", "--project", project_arg]));
    assert_eq!(status(&locked), 0, "{}", stderr(&locked));
    assert!(load(&project).palette_locked());

    let recolor = pixelforge(&arg(&[
        "palette",
        "set",
        "--project",
        project_arg,
        "--index",
        "1",
        "--r",
        "9",
        "--g",
        "9",
        "--b",
        "9",
    ]));
    assert_eq!(status(&recolor), 1);
    assert_eq!(json_lines(&recolor)[0]["error"]["code"], "PaletteLocked");
    assert_eq!(load(&project).palette_colors()[1], before);

    let painted = pixelforge(&arg(&[
        "paint",
        "--project",
        project_arg,
        "--x",
        "0",
        "--y",
        "1",
        "--color-index",
        "1",
    ]));
    assert_eq!(status(&painted), 0, "{}", stderr(&painted));
    assert_eq!(load(&project).get(0, 1).unwrap(), 1);
}

#[test]
fn request_file_and_stdin_call_the_tool_api() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    let project_arg = project.to_str().unwrap();
    assert_eq!(
        status(&pixelforge(&arg(&["new", "--output", project_arg]))),
        0
    );
    let requests = scratch.child("requests.json");
    fs::write(
        &requests,
        "[\n  {\"tool\":\"erase\",\"params\":{\"x\":0,\"y\":0}},\n  {\"tool\":\"addFrame\"}\n]\n",
    )
    .unwrap();
    let from_file = pixelforge(&arg(&[
        "call",
        "--project",
        project_arg,
        "--request-file",
        requests.to_str().unwrap(),
    ]));
    assert_eq!(status(&from_file), 0, "{}", stderr(&from_file));
    assert_eq!(json_lines(&from_file)[1]["result"]["index"], 1);
    assert_eq!(load(&project).frame_count(), 2);

    let mut child = Command::new(env!("CARGO_BIN_EXE_pixelforge"))
        .args(["call", "--project", project_arg])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(stdin, r#"{{"tool":"frame","params":{{"index":0}}}}"#).unwrap();
        writeln!(
            stdin,
            r#"{{"tool":"setActiveFrame","params":{{"index":0}}}}"#
        )
        .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert_eq!(status(&output), 1, "{}", stderr(&output));
    let lines = json_lines(&output);
    assert_eq!(lines[0]["error"]["code"], "UnknownTool");
    assert_eq!(lines.len(), 1);
    assert_eq!(load(&project).active_frame(), 0);
}

#[test]
fn bad_project_and_bad_size_are_json_errors() {
    let scratch = Scratch::new();
    let missing = scratch.child("missing.json");
    let absent = pixelforge(&arg(&["inspect", "--project", missing.to_str().unwrap()]));
    assert_eq!(status(&absent), 2);
    assert_eq!(json_lines(&absent)[0]["error"]["code"], "Cli");

    let garbage = scratch.child("nope.json");
    fs::write(&garbage, "{}\n").unwrap();
    let bad = pixelforge(&arg(&["inspect", "--project", garbage.to_str().unwrap()]));
    assert_eq!(status(&bad), 2, "{}", stderr(&bad));
    assert_eq!(json_lines(&bad)[0]["error"]["code"], "ProjectFile");

    let huge = scratch.child("huge.json");
    let sized = pixelforge(&arg(&[
        "new",
        "--width",
        "0",
        "--height",
        "16",
        "--output",
        huge.to_str().unwrap(),
    ]));
    assert_eq!(status(&sized), 2, "{}", stderr(&sized));
    assert_eq!(json_lines(&sized)[0]["error"]["code"], "InvalidSize");
    assert!(!huge.exists());
}

#[test]
fn tools_lists_the_kernel_catalog() {
    let output = pixelforge(&arg(&["tools"]));
    assert_eq!(status(&output), 0, "{}", stderr(&output));
    let body = &json_lines(&output)[0];
    let names: Vec<&str> = body["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect();
    assert!(names.contains(&"paint"));
    assert!(names.contains(&"exportSpriteSheet"));
    assert!(names.contains(&"undo"));
}
