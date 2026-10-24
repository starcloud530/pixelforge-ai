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
        "new",
        "call",
        "paint",
        "fill-rect",
        "erase-rect",
        "copy-rect",
        "move-rect",
        "preview",
        "--frame",
        "--preview",
        "--scale",
        "diff",
        "flip",
        "hflip",
        "resize",
        "--from-frame",
        "--horizontal",
        "--cols",
        "export",
        "inspect",
        "Stdout",
        "Tool API",
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
    assert!(names.contains(&"fillRect"));
    assert!(names.contains(&"exportSpriteSheet"));
    assert!(names.contains(&"previewFrame"));
    assert!(names.contains(&"diffFrames"));
    assert!(names.contains(&"flipHorizontal"));
    assert!(names.contains(&"resizeCanvas"));
    assert!(names.contains(&"undo"));
}

#[test]
fn fill_rect_and_preview_show_the_active_frame() {
    let scratch = Scratch::new();
    let project = scratch.child("portrait.json");
    let project_arg = project.to_str().unwrap();
    assert_eq!(
        status(&pixelforge(&arg(&[
            "new",
            "--width",
            "16",
            "--height",
            "8",
            "--output",
            project_arg,
        ]))),
        0
    );

    let filled = pixelforge(&arg(&[
        "fill-rect",
        "--project",
        project_arg,
        "--x",
        "1",
        "--y",
        "0",
        "--width",
        "3",
        "--height",
        "2",
        "--color-index",
        "4",
        "--preview",
    ]));
    assert_eq!(status(&filled), 0, "{}", stderr(&filled));
    assert_eq!(json_lines(&filled).len(), 1);
    assert_eq!(json_lines(&filled)[0]["ok"], true);
    assert_eq!(json_lines(&filled)[0]["result"], serde_json::json!({}));
    let notes = stderr(&filled);
    assert!(notes.contains("preview 16x8 frame 0"), "{notes}");
    assert!(notes.contains(".444............"), "{notes}");
    assert!(notes.contains("4=#f4a259"), "{notes}");
    let doc = load(&project);
    assert_eq!(doc.get(1, 0).unwrap(), 4);
    assert_eq!(doc.get(3, 1).unwrap(), 4);
    assert_eq!(doc.get(4, 0).unwrap(), 0);
    assert_eq!(doc.get(1, 2).unwrap(), 0);

    let before_preview = fs::read(&project).unwrap();
    let preview = pixelforge(&arg(&["preview", "--project", project_arg]));
    assert_eq!(status(&preview), 0, "{}", stderr(&preview));
    let body = &json_lines(&preview)[0];
    assert_eq!(body["ok"], true);
    assert_eq!(body["result"]["width"], 16);
    assert_eq!(body["result"]["height"], 8);
    assert_eq!(body["result"]["activeFrame"], 0);
    assert_eq!(body["result"]["rows"][0], ".444............");
    assert_eq!(body["result"]["rows"][1], ".444............");
    assert_eq!(body["result"]["rows"][2], "................");
    let legend = body["result"]["legend"].as_array().unwrap();
    assert!(legend
        .iter()
        .any(|entry| entry["glyph"] == "4" && entry["index"] == 4));
    assert!(stderr(&preview).contains(".444............"));
    assert_eq!(fs::read(&project).unwrap(), before_preview);

    let overflow = pixelforge(&arg(&[
        "fill-rect",
        "--project",
        project_arg,
        "--x",
        "14",
        "--y",
        "0",
        "--width",
        "4",
        "--height",
        "1",
        "--color-index",
        "1",
    ]));
    assert_eq!(status(&overflow), 1, "{}", stderr(&overflow));
    assert_eq!(json_lines(&overflow)[0]["error"]["code"], "OutOfBounds");
    assert_eq!(fs::read(&project).unwrap(), before_preview);

    let zero = pixelforge(&arg(&[
        "fill-rect",
        "--project",
        project_arg,
        "--x",
        "0",
        "--y",
        "0",
        "--width",
        "0",
        "--height",
        "1",
        "--color-index",
        "1",
    ]));
    assert_eq!(status(&zero), 1, "{}", stderr(&zero));
    assert_eq!(json_lines(&zero)[0]["error"]["code"], "InvalidParams");
    assert_eq!(fs::read(&project).unwrap(), before_preview);

    let painted = pixelforge(&arg(&[
        "paint",
        "--project",
        project_arg,
        "--x",
        "0",
        "--y",
        "2",
        "--color-index",
        "1",
        "--preview",
    ]));
    assert_eq!(status(&painted), 0, "{}", stderr(&painted));
    assert_eq!(json_lines(&painted).len(), 1);
    assert!(
        stderr(&painted).contains("1..............."),
        "{}",
        stderr(&painted)
    );

    let batch = pixelforge(&arg(&[
        "call",
        "--project",
        project_arg,
        "--preview",
        "--request",
        r#"{"tool":"fillRect","params":{"x":0,"y":3,"width":2,"height":1,"colorIndex":2}}"#,
    ]));
    assert_eq!(status(&batch), 0, "{}", stderr(&batch));
    assert_eq!(json_lines(&batch).len(), 1);
    assert!(
        stderr(&batch).contains("22.............."),
        "{}",
        stderr(&batch)
    );
    assert_eq!(load(&project).get(0, 3).unwrap(), 2);
    assert_eq!(load(&project).get(1, 3).unwrap(), 2);

    let looked = pixelforge(&arg(&["inspect", "--project", project_arg, "--preview"]));
    assert_eq!(status(&looked), 0, "{}", stderr(&looked));
    assert_eq!(json_lines(&looked)[0]["result"]["width"], 16);
    assert!(stderr(&looked).contains("preview 16x8 frame 0"));
}

#[test]
fn erase_copy_move_preview_frame_and_export_scale() {
    let scratch = Scratch::new();
    let project = scratch.child("pose.json");
    let project_arg = project.to_str().unwrap();
    assert_eq!(
        status(&pixelforge(&arg(&[
            "new",
            "--width",
            "8",
            "--height",
            "4",
            "--output",
            project_arg,
        ]))),
        0
    );

    assert_eq!(
        status(&pixelforge(&arg(&[
            "fill-rect",
            "--project",
            project_arg,
            "--x",
            "0",
            "--y",
            "0",
            "--width",
            "3",
            "--height",
            "1",
            "--color-index",
            "5",
        ]))),
        0
    );
    let duplicated = pixelforge(&arg(&[
        "frame",
        "duplicate",
        "--project",
        project_arg,
        "--index",
        "0",
    ]));
    assert_eq!(status(&duplicated), 0, "{}", stderr(&duplicated));
    assert_eq!(json_lines(&duplicated)[0]["result"]["index"], 1);
    assert_eq!(load(&project).active_frame(), 0);
    assert_eq!(load(&project).frame_pixels(1).unwrap()[0], 5);

    assert_eq!(
        status(&pixelforge(&arg(&[
            "frame",
            "select",
            "--project",
            project_arg,
            "--index",
            "1",
        ]))),
        0
    );
    assert_eq!(
        status(&pixelforge(&arg(&[
            "paint",
            "--project",
            project_arg,
            "--x",
            "0",
            "--y",
            "1",
            "--color-index",
            "2",
        ]))),
        0
    );
    assert_eq!(
        status(&pixelforge(&arg(&[
            "frame",
            "select",
            "--project",
            project_arg,
            "--index",
            "0",
        ]))),
        0
    );

    let before_preview = fs::read(&project).unwrap();
    let preview = pixelforge(&arg(&["preview", "--project", project_arg, "--frame", "1"]));
    assert_eq!(status(&preview), 0, "{}", stderr(&preview));
    let body = &json_lines(&preview)[0];
    assert_eq!(body["result"]["frame"], 1);
    assert_eq!(body["result"]["activeFrame"], 0);
    assert_eq!(body["result"]["rows"][1], "2.......");
    assert!(stderr(&preview).contains("preview 8x4 frame 1 active 0"));
    assert_eq!(fs::read(&project).unwrap(), before_preview);
    assert_eq!(load(&project).active_frame(), 0);

    let missing = pixelforge(&arg(&["preview", "--project", project_arg, "--frame", "4"]));
    assert_eq!(status(&missing), 2, "{}", stderr(&missing));
    assert_eq!(json_lines(&missing)[0]["error"]["code"], "FrameIndex");
    assert_eq!(fs::read(&project).unwrap(), before_preview);

    let erased = pixelforge(&arg(&[
        "erase-rect",
        "--project",
        project_arg,
        "--x",
        "0",
        "--y",
        "0",
        "--width",
        "2",
        "--height",
        "1",
    ]));
    assert_eq!(status(&erased), 0, "{}", stderr(&erased));
    assert_eq!(load(&project).get(0, 0).unwrap(), 0);
    assert_eq!(load(&project).get(2, 0).unwrap(), 5);
    assert_eq!(load(&project).frame_pixels(1).unwrap()[0], 5);

    let overflow = pixelforge(&arg(&[
        "erase-rect",
        "--project",
        project_arg,
        "--x",
        "6",
        "--y",
        "0",
        "--width",
        "4",
        "--height",
        "1",
    ]));
    assert_eq!(status(&overflow), 1);
    assert_eq!(json_lines(&overflow)[0]["error"]["code"], "OutOfBounds");
    assert_eq!(load(&project).get(2, 0).unwrap(), 5);

    let copied = pixelforge(&arg(&[
        "copy-rect",
        "--project",
        project_arg,
        "--src-x",
        "2",
        "--src-y",
        "0",
        "--width",
        "1",
        "--height",
        "1",
        "--dst-x",
        "4",
        "--dst-y",
        "0",
    ]));
    assert_eq!(status(&copied), 0, "{}", stderr(&copied));
    assert_eq!(load(&project).get(2, 0).unwrap(), 5);
    assert_eq!(load(&project).get(4, 0).unwrap(), 5);

    let moved = pixelforge(&arg(&[
        "call",
        "--project",
        project_arg,
        "--request",
        r#"{"tool":"moveRect","params":{"srcX":4,"srcY":0,"width":1,"height":1,"dstX":5,"dstY":0}}"#,
        "--request",
        r#"{"tool":"undo"}"#,
    ]));
    assert_eq!(status(&moved), 0, "{}", stderr(&moved));
    assert_eq!(json_lines(&moved).len(), 2);
    assert_eq!(load(&project).get(4, 0).unwrap(), 5);
    assert_eq!(load(&project).get(5, 0).unwrap(), 0);

    let shifted = pixelforge(&arg(&[
        "move-rect",
        "--project",
        project_arg,
        "--src-x",
        "2",
        "--src-y",
        "0",
        "--width",
        "1",
        "--height",
        "1",
        "--dst-x",
        "3",
        "--dst-y",
        "0",
    ]));
    assert_eq!(status(&shifted), 0, "{}", stderr(&shifted));
    assert_eq!(load(&project).get(2, 0).unwrap(), 0);
    assert_eq!(load(&project).get(3, 0).unwrap(), 5);
    assert_eq!(load(&project).get(4, 0).unwrap(), 5);

    let before_export = fs::read(&project).unwrap();
    let frames_dir = scratch.child("frames");
    let sequence = pixelforge(&arg(&[
        "export",
        "png",
        "--project",
        project_arg,
        "--out-dir",
        frames_dir.to_str().unwrap(),
        "--scale",
        "2",
    ]));
    assert_eq!(status(&sequence), 0, "{}", stderr(&sequence));
    let sequence_body = &json_lines(&sequence)[0];
    assert_eq!(sequence_body["result"]["scale"], 2);
    assert_eq!(sequence_body["result"]["width"], 16);
    assert_eq!(sequence_body["result"]["height"], 8);
    assert_eq!(
        png_size(&fs::read(frames_dir.join("pixelforge-000.png")).unwrap()),
        (16, 8)
    );
    assert_eq!(
        png_size(&fs::read(frames_dir.join("pixelforge-001.png")).unwrap()),
        (16, 8)
    );

    let sheet_path = scratch.child("pose-sheet.png");
    let sheet = pixelforge(&arg(&[
        "export",
        "sheet",
        "--project",
        project_arg,
        "--output",
        sheet_path.to_str().unwrap(),
        "--scale",
        "2",
    ]));
    assert_eq!(status(&sheet), 0, "{}", stderr(&sheet));
    let sheet_body = &json_lines(&sheet)[0];
    assert_eq!(sheet_body["result"]["scale"], 2);
    assert_eq!(sheet_body["result"]["columns"], 2);
    assert_eq!(sheet_body["result"]["rows"], 1);
    assert_eq!(sheet_body["result"]["sheetWidth"], 32);
    assert_eq!(sheet_body["result"]["sheetHeight"], 8);
    assert_eq!(png_size(&fs::read(&sheet_path).unwrap()), (32, 8));
    assert_eq!(fs::read(&project).unwrap(), before_export);

    let bad_scale = pixelforge(&arg(&[
        "export",
        "png",
        "--project",
        project_arg,
        "--out-dir",
        scratch.child("nope").to_str().unwrap(),
        "--scale",
        "0",
    ]));
    assert_eq!(status(&bad_scale), 2);
    assert!(json_lines(&bad_scale).is_empty());
}

#[test]
fn preview_png_diff_flip_resize_and_sheet_columns() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    let project_arg = project.to_str().unwrap();
    assert_eq!(
        status(&pixelforge(&arg(&[
            "new",
            "--width",
            "4",
            "--height",
            "2",
            "--output",
            project_arg,
        ]))),
        0
    );
    assert_eq!(
        status(&pixelforge(&arg(&[
            "fill-rect",
            "--project",
            project_arg,
            "--x",
            "0",
            "--y",
            "0",
            "--width",
            "2",
            "--height",
            "1",
            "--color-index",
            "3",
        ]))),
        0
    );

    let before = fs::read(&project).unwrap();
    let preview = pixelforge(&arg(&["preview", "--project", project_arg, "--scale", "2"]));
    assert_eq!(status(&preview), 0, "{}", stderr(&preview));
    let body = &json_lines(&preview)[0];
    assert_eq!(body["result"]["rows"][0], "33..");
    assert_eq!(body["result"]["scale"], 2);
    assert_eq!(body["result"]["pngWidth"], 8);
    assert_eq!(body["result"]["pngHeight"], 4);
    let preview_path = body["result"]["path"].as_str().unwrap();
    assert_eq!(png_size(&fs::read(preview_path).unwrap()), (8, 4));
    assert!(stderr(&preview).contains("33.."));
    assert_eq!(fs::read(&project).unwrap(), before);

    assert_eq!(
        status(&pixelforge(&arg(&[
            "frame",
            "duplicate",
            "--project",
            project_arg,
            "--index",
            "0",
        ]))),
        0
    );
    assert_eq!(
        status(&pixelforge(&arg(&[
            "frame",
            "select",
            "--project",
            project_arg,
            "--index",
            "1",
        ]))),
        0
    );
    assert_eq!(
        status(&pixelforge(&arg(&[
            "paint",
            "--project",
            project_arg,
            "--x",
            "3",
            "--y",
            "1",
            "--color-index",
            "5",
        ]))),
        0
    );
    assert_eq!(
        status(&pixelforge(&arg(&[
            "frame",
            "select",
            "--project",
            project_arg,
            "--index",
            "0",
        ]))),
        0
    );
    assert_eq!(
        status(&pixelforge(&arg(&[
            "copy-rect",
            "--project",
            project_arg,
            "--from-frame",
            "0",
            "--to-frame",
            "1",
            "--src-x",
            "0",
            "--src-y",
            "0",
            "--width",
            "2",
            "--height",
            "1",
            "--dst-x",
            "1",
            "--dst-y",
            "0",
        ]))),
        0
    );
    assert_eq!(load(&project).active_frame(), 0);
    assert_eq!(load(&project).get(0, 0).unwrap(), 3);
    assert_eq!(load(&project).frame_pixels(1).unwrap()[1], 3);
    assert_eq!(load(&project).frame_pixels(1).unwrap()[2], 3);
    assert_eq!(load(&project).frame_pixels(1).unwrap()[7], 5);

    let before_diff = fs::read(&project).unwrap();
    let diff_path = scratch.child("walk.diff.png");
    let diff = pixelforge(&arg(&[
        "diff",
        "--project",
        project_arg,
        "--frame",
        "0",
        "--with",
        "1",
        "--scale",
        "2",
        "--output",
        diff_path.to_str().unwrap(),
    ]));
    assert_eq!(status(&diff), 0, "{}", stderr(&diff));
    let diff_body = &json_lines(&diff)[0];
    assert!(diff_body["result"]["changed"].as_u64().unwrap() > 0);
    assert_eq!(diff_body["result"]["pngWidth"], 24);
    assert_eq!(diff_body["result"]["pngHeight"], 4);
    assert_eq!(png_size(&fs::read(&diff_path).unwrap()), (24, 4));
    assert!(stderr(&diff).contains("diff frame 0 with 1"));
    assert_eq!(fs::read(&project).unwrap(), before_diff);

    let bare = pixelforge(&arg(&["flip", "--project", project_arg]));
    assert_eq!(status(&bare), 2, "{}", stderr(&bare));
    assert_eq!(fs::read(&project).unwrap(), before_diff);

    assert_eq!(
        status(&pixelforge(&arg(&["hflip", "--project", project_arg,]))),
        0
    );
    assert_eq!(load(&project).get(2, 0).unwrap(), 3);
    assert_eq!(load(&project).get(3, 0).unwrap(), 3);
    assert_eq!(load(&project).get(0, 0).unwrap(), 0);
    assert_eq!(load(&project).frame_pixels(1).unwrap()[1], 3);

    let sheet_path = scratch.child("row.png");
    let sheet = pixelforge(&arg(&[
        "export",
        "sheet",
        "--project",
        project_arg,
        "--output",
        sheet_path.to_str().unwrap(),
        "--cols",
        "2",
    ]));
    assert_eq!(status(&sheet), 0, "{}", stderr(&sheet));
    assert_eq!(json_lines(&sheet)[0]["result"]["columns"], 2);
    assert_eq!(json_lines(&sheet)[0]["result"]["rows"], 1);
    assert_eq!(png_size(&fs::read(&sheet_path).unwrap()), (8, 2));

    let resized = pixelforge(&arg(&[
        "resize",
        "--project",
        project_arg,
        "--width",
        "6",
        "--height",
        "4",
    ]));
    assert_eq!(status(&resized), 0, "{}", stderr(&resized));
    assert_eq!(json_lines(&resized)[0]["result"]["offsetX"], 1);
    assert_eq!(json_lines(&resized)[0]["result"]["offsetY"], 1);
    let doc = load(&project);
    assert_eq!((doc.width(), doc.height()), (6, 4));
    assert_eq!(doc.get(3, 1).unwrap(), 3);
    assert_eq!(doc.frame_pixels(1).unwrap()[1 * 6 + 2], 3);

    let undone = pixelforge(&arg(&[
        "call",
        "--project",
        project_arg,
        "--request",
        r#"{"tool":"resizeCanvas","params":{"width":8,"height":4,"offsetX":0,"offsetY":0}}"#,
        "--request",
        r#"{"tool":"undo"}"#,
    ]));
    assert_eq!(status(&undone), 0, "{}", stderr(&undone));
    assert_eq!(load(&project).width(), 6);
}

#[test]
fn layers_composite_preview_and_export_leave_the_body_alone() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    let project_arg = project.to_str().unwrap();
    assert_eq!(
        status(&pixelforge(&arg(&[
            "new",
            "--width",
            "4",
            "--height",
            "2",
            "--output",
            project_arg,
        ]))),
        0
    );
    assert_eq!(load(&project).layer_count(), 1);
    assert_eq!(load(&project).layer_name(0).unwrap(), "Layer 1");

    let rename = pixelforge(&arg(&[
        "layer",
        "rename",
        "--project",
        project_arg,
        "--index",
        "0",
        "--name",
        "body",
    ]));
    assert_eq!(status(&rename), 0, "{}", stderr(&rename));
    let body = pixelforge(&arg(&[
        "fill-rect",
        "--project",
        project_arg,
        "--layer",
        "0",
        "--x",
        "0",
        "--y",
        "0",
        "--width",
        "2",
        "--height",
        "2",
        "--color-index",
        "1",
    ]));
    assert_eq!(status(&body), 0, "{}", stderr(&body));
    let gear = pixelforge(&arg(&[
        "layer",
        "add",
        "--project",
        project_arg,
        "--name",
        "gear",
    ]));
    assert_eq!(status(&gear), 0, "{}", stderr(&gear));
    assert_eq!(json_lines(&gear)[0]["result"]["index"], 1);
    assert_eq!(json_lines(&gear)[0]["result"]["name"], "gear");

    let armor = pixelforge(&arg(&[
        "fill-rect",
        "--project",
        project_arg,
        "--x",
        "1",
        "--y",
        "0",
        "--width",
        "1",
        "--height",
        "1",
        "--color-index",
        "2",
    ]));
    assert_eq!(status(&armor), 0, "{}", stderr(&armor));

    let preview = pixelforge(&arg(&["preview", "--project", project_arg, "--scale", "1"]));
    assert_eq!(status(&preview), 0, "{}", stderr(&preview));
    let rows = json_lines(&preview)[0]["result"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row.as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(rows, vec!["12..".to_string(), "11..".to_string()]);
    assert_eq!(json_lines(&preview)[0]["result"]["composite"], true);
    assert!(stderr(&preview).contains("composite"));

    let repaint = pixelforge(&arg(&[
        "paint",
        "--project",
        project_arg,
        "--x",
        "1",
        "--y",
        "0",
        "--color-index",
        "3",
    ]));
    assert_eq!(status(&repaint), 0, "{}", stderr(&repaint));
    let doc = load(&project);
    assert_eq!(doc.layer_pixels(0, 0).unwrap(), &[1, 1, 0, 0, 1, 1, 0, 0]);
    assert_eq!(doc.layer_pixels(0, 1).unwrap()[1], 3);
    assert_eq!(doc.composite_pixels(0).unwrap()[1], 3);

    let only = pixelforge(&arg(&["preview", "--project", project_arg, "--layer", "0"]));
    assert_eq!(status(&only), 0, "{}", stderr(&only));
    assert_eq!(json_lines(&only)[0]["result"]["rows"][0], "11..");
    assert_eq!(json_lines(&only)[0]["result"]["composite"], false);

    let png = scratch.child("frames");
    let exported = pixelforge(&arg(&[
        "export",
        "png",
        "--project",
        project_arg,
        "--out-dir",
        png.to_str().unwrap(),
    ]));
    assert_eq!(status(&exported), 0, "{}", stderr(&exported));
    let bytes = fs::read(png.join("pixelforge-000.png")).unwrap();
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));

    let flat = pixelforge(&arg(&["layer", "flatten", "--project", project_arg]));
    assert_eq!(status(&flat), 0, "{}", stderr(&flat));
    let flat_doc = load(&project);
    assert_eq!(flat_doc.layer_count(), 1);
    assert_eq!(flat_doc.layer_name(0).unwrap(), "Layer 1");
    assert_eq!(flat_doc.composite_pixels(0).unwrap()[1], 3);
    assert!(!flat_doc.save_project().contains("\"layers\""));
}

#[test]
fn game_sprite_canvases_96_and_128_round_trip() {
    let scratch = Scratch::new();

    let defaulted = scratch.child("default.json");
    let created = pixelforge(&arg(&["new", "--output", defaulted.to_str().unwrap()]));
    assert_eq!(status(&created), 0, "{}", stderr(&created));
    assert_eq!(json_lines(&created)[0]["result"]["width"], 16);
    assert_eq!(json_lines(&created)[0]["result"]["height"], 16);

    for (name, width, height) in [
        ("sq96.json", "96", "96"),
        ("sq128.json", "128", "128"),
        ("tall.json", "96", "128"),
        ("slim.json", "64", "128"),
    ] {
        let path = scratch.child(name);
        let made = pixelforge(&arg(&[
            "new",
            "--width",
            width,
            "--height",
            height,
            "--output",
            path.to_str().unwrap(),
        ]));
        assert_eq!(status(&made), 0, "{}", stderr(&made));
        let body = &json_lines(&made)[0];
        assert_eq!(body["ok"], true);
        assert_eq!(body["result"]["width"], width.parse::<u64>().unwrap());
        assert_eq!(body["result"]["height"], height.parse::<u64>().unwrap());
        let doc = load(&path);
        let width = width.parse::<u16>().unwrap();
        let height = height.parse::<u16>().unwrap();
        assert_eq!((doc.width(), doc.height()), (width, height));
        assert_eq!(doc.pixels().len(), usize::from(width) * usize::from(height));
    }

    let refused = scratch.child("too-big.json");
    let sized = pixelforge(&arg(&[
        "new",
        "--width",
        "257",
        "--height",
        "128",
        "--output",
        refused.to_str().unwrap(),
    ]));
    assert_eq!(status(&sized), 2, "{}", stderr(&sized));
    assert_eq!(json_lines(&sized)[0]["error"]["code"], "InvalidSize");
    assert!(!refused.exists());

    let project = scratch.child("grow.json");
    let project_arg = project.to_str().unwrap();
    assert_eq!(
        status(&pixelforge(&arg(&[
            "new",
            "--width",
            "64",
            "--height",
            "64",
            "--output",
            project_arg,
        ]))),
        0
    );
    assert_eq!(
        status(&pixelforge(&arg(&[
            "paint",
            "--project",
            project_arg,
            "--x",
            "0",
            "--y",
            "0",
            "--color-index",
            "1",
        ]))),
        0
    );
    assert_eq!(
        status(&pixelforge(&arg(&[
            "paint",
            "--project",
            project_arg,
            "--x",
            "63",
            "--y",
            "63",
            "--color-index",
            "2",
        ]))),
        0
    );

    let to_96 = pixelforge(&arg(&[
        "resize",
        "--project",
        project_arg,
        "--width",
        "96",
        "--height",
        "96",
        "--offset-x",
        "0",
        "--offset-y",
        "0",
    ]));
    assert_eq!(status(&to_96), 0, "{}", stderr(&to_96));
    assert_eq!(json_lines(&to_96)[0]["result"]["width"], 96);
    assert_eq!(json_lines(&to_96)[0]["result"]["height"], 96);
    assert_eq!(json_lines(&to_96)[0]["result"]["offsetX"], 0);
    assert_eq!(json_lines(&to_96)[0]["result"]["offsetY"], 0);
    let doc = load(&project);
    assert_eq!((doc.width(), doc.height()), (96, 96));
    assert_eq!(doc.pixels().len(), 96 * 96);
    assert_eq!(doc.get(0, 0).unwrap(), 1);
    assert_eq!(doc.get(63, 63).unwrap(), 2);
    assert_eq!(doc.get(95, 95).unwrap(), 0);

    let preview_96 = scratch.child("grow-96.preview.png");
    let looked_96 = pixelforge(&arg(&[
        "preview",
        "--project",
        project_arg,
        "--scale",
        "2",
        "--output",
        preview_96.to_str().unwrap(),
    ]));
    assert_eq!(status(&looked_96), 0, "{}", stderr(&looked_96));
    let body_96 = &json_lines(&looked_96)[0];
    assert_eq!(body_96["result"]["width"], 96);
    assert_eq!(body_96["result"]["height"], 96);
    assert_eq!(body_96["result"]["scale"], 2);
    assert_full_grid(body_96, 96, 96);
    assert_eq!(glyph_at(body_96, 0, 0), '1');
    assert_eq!(glyph_at(body_96, 63, 63), '2');
    assert_eq!(glyph_at(body_96, 95, 95), '.');
    assert_eq!(png_size(&fs::read(&preview_96).unwrap()), (192, 192));
    assert!(stderr(&looked_96).contains("preview 96x96 frame 0"));

    let png_96 = scratch.child("frames-96");
    let exported_96 = pixelforge(&arg(&[
        "export",
        "png",
        "--project",
        project_arg,
        "--out-dir",
        png_96.to_str().unwrap(),
    ]));
    assert_eq!(status(&exported_96), 0, "{}", stderr(&exported_96));
    assert_eq!(json_lines(&exported_96)[0]["result"]["width"], 96);
    assert_eq!(json_lines(&exported_96)[0]["result"]["height"], 96);
    assert_eq!(
        png_size(&fs::read(png_96.join("pixelforge-000.png")).unwrap()),
        (96, 96)
    );
    let sheet_96 = scratch.child("sheet-96.png");
    let sheeted_96 = pixelforge(&arg(&[
        "export",
        "sheet",
        "--project",
        project_arg,
        "--output",
        sheet_96.to_str().unwrap(),
        "--cols",
        "1",
    ]));
    assert_eq!(status(&sheeted_96), 0, "{}", stderr(&sheeted_96));
    assert_eq!(png_size(&fs::read(&sheet_96).unwrap()), (96, 96));

    let to_128 = pixelforge(&arg(&[
        "resize",
        "--project",
        project_arg,
        "--width",
        "128",
        "--height",
        "128",
        "--offset-x",
        "0",
        "--offset-y",
        "0",
    ]));
    assert_eq!(status(&to_128), 0, "{}", stderr(&to_128));
    assert_eq!(json_lines(&to_128)[0]["result"]["width"], 128);
    assert_eq!(json_lines(&to_128)[0]["result"]["height"], 128);
    let doc = load(&project);
    assert_eq!((doc.width(), doc.height()), (128, 128));
    assert_eq!(doc.pixels().len(), 128 * 128);
    assert_eq!(doc.get(0, 0).unwrap(), 1);
    assert_eq!(doc.get(63, 63).unwrap(), 2);
    assert_eq!(doc.get(127, 127).unwrap(), 0);

    let filled = pixelforge(&arg(&[
        "fill-rect",
        "--project",
        project_arg,
        "--x",
        "100",
        "--y",
        "100",
        "--width",
        "8",
        "--height",
        "8",
        "--color-index",
        "4",
    ]));
    assert_eq!(status(&filled), 0, "{}", stderr(&filled));
    assert_eq!(load(&project).get(100, 100).unwrap(), 4);
    assert_eq!(load(&project).get(107, 107).unwrap(), 4);
    assert_eq!(load(&project).get(108, 100).unwrap(), 0);
    let before_overflow = fs::read(&project).unwrap();
    let overflow = pixelforge(&arg(&[
        "fill-rect",
        "--project",
        project_arg,
        "--x",
        "120",
        "--y",
        "120",
        "--width",
        "20",
        "--height",
        "20",
        "--color-index",
        "4",
    ]));
    assert_eq!(status(&overflow), 1, "{}", stderr(&overflow));
    assert_eq!(json_lines(&overflow)[0]["error"]["code"], "OutOfBounds");
    assert_eq!(fs::read(&project).unwrap(), before_overflow);

    let preview_128 = scratch.child("grow-128.preview.png");
    let looked_128 = pixelforge(&arg(&[
        "preview",
        "--project",
        project_arg,
        "--scale",
        "1",
        "--output",
        preview_128.to_str().unwrap(),
    ]));
    assert_eq!(status(&looked_128), 0, "{}", stderr(&looked_128));
    let body_128 = &json_lines(&looked_128)[0];
    assert_full_grid(body_128, 128, 128);
    assert_eq!(glyph_at(body_128, 0, 0), '1');
    assert_eq!(glyph_at(body_128, 63, 63), '2');
    assert_eq!(glyph_at(body_128, 100, 100), '4');
    assert_eq!(glyph_at(body_128, 127, 127), '.');
    assert_eq!(png_size(&fs::read(&preview_128).unwrap()), (128, 128));
    assert!(stderr(&looked_128).contains("preview 128x128 frame 0"));

    let png_128 = scratch.child("frames-128");
    let exported_128 = pixelforge(&arg(&[
        "export",
        "png",
        "--project",
        project_arg,
        "--out-dir",
        png_128.to_str().unwrap(),
        "--scale",
        "2",
    ]));
    assert_eq!(status(&exported_128), 0, "{}", stderr(&exported_128));
    assert_eq!(json_lines(&exported_128)[0]["result"]["width"], 256);
    assert_eq!(json_lines(&exported_128)[0]["result"]["height"], 256);
    assert_eq!(
        png_size(&fs::read(png_128.join("pixelforge-000.png")).unwrap()),
        (256, 256)
    );

    assert_eq!(
        status(&pixelforge(&arg(&[
            "frame",
            "duplicate",
            "--project",
            project_arg,
            "--index",
            "0",
        ]))),
        0
    );
    let sheet_128 = scratch.child("sheet-128.png");
    let sheeted_128 = pixelforge(&arg(&[
        "export",
        "sheet",
        "--project",
        project_arg,
        "--output",
        sheet_128.to_str().unwrap(),
        "--cols",
        "2",
    ]));
    assert_eq!(status(&sheeted_128), 0, "{}", stderr(&sheeted_128));
    assert_eq!(json_lines(&sheeted_128)[0]["result"]["columns"], 2);
    assert_eq!(json_lines(&sheeted_128)[0]["result"]["rows"], 1);
    assert_eq!(png_size(&fs::read(&sheet_128).unwrap()), (256, 128));

    let gear = pixelforge(&arg(&[
        "layer",
        "add",
        "--project",
        project_arg,
        "--name",
        "gear",
    ]));
    assert_eq!(status(&gear), 0, "{}", stderr(&gear));
    assert_eq!(json_lines(&gear)[0]["result"]["index"], 1);
    let armor = pixelforge(&arg(&[
        "fill-rect",
        "--project",
        project_arg,
        "--layer",
        "1",
        "--x",
        "4",
        "--y",
        "0",
        "--width",
        "1",
        "--height",
        "1",
        "--color-index",
        "5",
    ]));
    assert_eq!(status(&armor), 0, "{}", stderr(&armor));
    let layered = pixelforge(&arg(&["preview", "--project", project_arg, "--scale", "1"]));
    assert_eq!(status(&layered), 0, "{}", stderr(&layered));
    let layered_body = &json_lines(&layered)[0];
    assert_eq!(layered_body["result"]["composite"], true);
    assert_full_grid(layered_body, 128, 128);
    assert_eq!(glyph_at(layered_body, 0, 0), '1');
    assert_eq!(glyph_at(layered_body, 4, 0), '5');
    let body_only = pixelforge(&arg(&["preview", "--project", project_arg, "--layer", "0"]));
    assert_eq!(status(&body_only), 0, "{}", stderr(&body_only));
    assert_eq!(glyph_at(&json_lines(&body_only)[0], 4, 0), '.');
    assert_eq!(glyph_at(&json_lines(&body_only)[0], 0, 0), '1');
    let png_layers = scratch.child("frames-layers");
    let exported_layers = pixelforge(&arg(&[
        "export",
        "png",
        "--project",
        project_arg,
        "--out-dir",
        png_layers.to_str().unwrap(),
    ]));
    assert_eq!(status(&exported_layers), 0, "{}", stderr(&exported_layers));
    assert_eq!(
        png_size(&fs::read(png_layers.join("pixelforge-000.png")).unwrap()),
        (128, 128)
    );
    let doc = load(&project);
    assert_eq!(doc.layer_pixels(0, 0).unwrap()[4], 0);
    assert_eq!(doc.composite_pixels(0).unwrap()[4], 5);
    assert_eq!((doc.width(), doc.height()), (128, 128));

    let too_wide = pixelforge(&arg(&[
        "resize",
        "--project",
        project_arg,
        "--width",
        "257",
        "--height",
        "128",
    ]));
    assert_eq!(status(&too_wide), 1, "{}", stderr(&too_wide));
    // The tool schema is 1..=256, so this is InvalidParams. `new` hits
    // Document::new and reports InvalidSize. Either way the file stays put.
    assert_eq!(json_lines(&too_wide)[0]["error"]["code"], "InvalidParams");
    let doc = load(&project);
    assert_eq!((doc.width(), doc.height()), (128, 128));
    assert_eq!(doc.layer_pixels(0, 0).unwrap()[0], 1);
    assert_eq!(doc.composite_pixels(0).unwrap()[0], 1);
}

#[test]
fn scene_dirty_preview_and_diff_are_structured() {
    let scratch = Scratch::new();
    let project = scratch.child("state.json");
    let project_arg = project.to_str().unwrap();
    assert_eq!(
        status(&pixelforge(&arg(&[
            "new",
            "--width",
            "8",
            "--height",
            "4",
            "--output",
            project_arg,
        ]))),
        0
    );
    let before = fs::read(&project).unwrap();
    let scene = pixelforge(&arg(&["scene", "--project", project_arg]));
    assert_eq!(status(&scene), 0, "{}", stderr(&scene));
    let body = &json_lines(&scene)[0];
    assert_eq!(body["result"]["width"], 8);
    assert_eq!(body["result"]["height"], 4);
    assert_eq!(body["result"]["activeFrame"], 0);
    assert_eq!(body["result"]["activeLayer"], 0);
    assert_eq!(body["result"]["paletteHash"], "efb5af0bb69fc10e");
    assert_eq!(body["result"]["paletteCount"], 9);
    assert_eq!(body["result"]["frameCount"], 1);
    assert_eq!(body["result"]["delayMs"], 100);
    assert_eq!(body["result"]["frames"][0]["delayMs"], 100);
    assert_eq!(body["result"]["layers"][0]["name"], "Layer 1");
    assert_eq!(body["result"]["layers"][0]["visible"], true);
    assert!(body["result"]["layers"][0]["opacity"].is_null());
    assert!(body["result"].get("pixels").is_none());
    assert_eq!(fs::read(&project).unwrap(), before);

    let plain = pixelforge(&arg(&[
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
    assert_eq!(json_lines(&plain)[0]["result"], serde_json::json!({}));

    let painted = pixelforge(&arg(&[
        "paint",
        "--project",
        project_arg,
        "--x",
        "2",
        "--y",
        "1",
        "--color-index",
        "4",
        "--dirty",
    ]));
    assert_eq!(status(&painted), 0, "{}", stderr(&painted));
    let dirty = &json_lines(&painted)[0]["result"];
    assert_eq!(dirty["changed"], 1);
    assert_eq!(dirty["dirty"]["x"], 2);
    assert_eq!(dirty["dirty"]["y"], 1);
    assert_eq!(dirty["dirty"]["width"], 1);
    assert_eq!(dirty["dirty"]["height"], 1);
    assert_eq!(dirty["regions"][0]["frame"], 0);

    let before_preview = fs::read(&project).unwrap();
    let preview = pixelforge(&arg(&["preview", "--project", project_arg]));
    assert_eq!(status(&preview), 0, "{}", stderr(&preview));
    let view = &json_lines(&preview)[0]["result"];
    assert_eq!(view["ink"], 2);
    assert_eq!(view["bounds"]["width"], 2);
    assert_eq!(view["bounds"]["height"], 2);
    assert!(view["indices"].as_array().unwrap().len() >= 2);
    assert_eq!(view["pixels"][2 * 8 + 1], 3);
    assert_eq!(view["rows"].as_array().unwrap().len(), 4);
    assert_eq!(fs::read(&project).unwrap(), before_preview);

    assert_eq!(
        status(&pixelforge(&arg(&[
            "frame",
            "duplicate",
            "--project",
            project_arg,
            "--index",
            "0",
        ]))),
        0
    );
    assert_eq!(
        status(&pixelforge(&arg(&[
            "frame",
            "select",
            "--project",
            project_arg,
            "--index",
            "1",
        ]))),
        0
    );
    assert_eq!(
        status(&pixelforge(&arg(&[
            "erase",
            "--project",
            project_arg,
            "--x",
            "1",
            "--y",
            "2",
            "--dirty",
        ]))),
        0
    );
    let diff = pixelforge(&arg(&[
        "diff",
        "--project",
        project_arg,
        "--frame",
        "0",
        "--with",
        "1",
    ]));
    assert_eq!(status(&diff), 0, "{}", stderr(&diff));
    let compared = &json_lines(&diff)[0]["result"];
    assert_eq!(compared["changed"], 1);
    assert_eq!(compared["bounds"]["x"], 1);
    assert_eq!(compared["bounds"]["y"], 2);
    assert_eq!(compared["kinds"]["onlyFrame"], 1);
    assert_eq!(compared["changes"][0]["left"], 3);
    assert_eq!(compared["changes"][0]["right"], 0);
    assert!(compared["diffRows"].as_array().unwrap().len() == 4);
    assert!(compared["rows"].as_array().is_some());
}

#[test]
fn palette_enforce_remap_and_apply_png() {
    let scratch = Scratch::new();
    let project = scratch.child("swatch.json");
    let project_arg = project.to_str().unwrap();
    assert_eq!(
        status(&pixelforge(&arg(&[
            "new",
            "--width",
            "2",
            "--height",
            "1",
            "--output",
            project_arg
        ]))),
        0
    );

    let rejected = pixelforge(&arg(&[
        "paint",
        "--project",
        project_arg,
        "--x",
        "0",
        "--y",
        "0",
        "--color-index",
        "40",
    ]));
    assert_eq!(status(&rejected), 1, "{}", stderr(&rejected));
    assert_eq!(json_lines(&rejected)[0]["error"]["code"], "PaletteIndex");

    let clamp = pixelforge(&arg(&[
        "palette",
        "enforce",
        "--project",
        project_arg,
        "--mode",
        "clamp",
    ]));
    assert_eq!(status(&clamp), 0, "{}", stderr(&clamp));
    assert_eq!(json_lines(&clamp)[0]["result"]["mode"], "clamp");
    assert!(load(&project)
        .save_project()
        .contains("\"indexPolicy\": \"clamp\""));

    let painted = pixelforge(&arg(&[
        "paint",
        "--project",
        project_arg,
        "--x",
        "0",
        "--y",
        "0",
        "--color-index",
        "40",
    ]));
    assert_eq!(status(&painted), 0, "{}", stderr(&painted));
    assert_eq!(json_lines(&painted)[0]["result"]["colorIndex"], 8);
    assert_eq!(json_lines(&painted)[0]["result"]["requestedIndex"], 40);
    assert_eq!(load(&project).pixels()[0], 8);

    let missing = pixelforge(&arg(&["palette", "remap", "--project", project_arg]));
    assert_eq!(status(&missing), 2, "{}", stderr(&missing));
    assert_eq!(json_lines(&missing)[0]["error"]["code"], "Cli");

    let reduced = pixelforge(&arg(&[
        "palette",
        "remap",
        "--project",
        project_arg,
        "--max-colors",
        "4",
    ]));
    assert_eq!(status(&reduced), 0, "{}", stderr(&reduced));
    let report = &json_lines(&reduced)[0]["result"];
    assert_eq!(report["paletteLength"], 5);
    assert_eq!(report["pixelsChanged"], 1);
    assert_eq!(report["dropped"].as_array().unwrap().len(), 4);
    assert!(report["misses"].as_array().unwrap().is_empty());
    assert_eq!(report["merges"][0]["exact"], true);
    assert_eq!(report["merges"][0]["fromIndex"], 8);

    let frames = scratch.child("frames");
    assert_eq!(
        status(&pixelforge(&arg(&[
            "export",
            "png",
            "--project",
            project_arg,
            "--out-dir",
            frames.to_str().unwrap(),
        ]))),
        0
    );
    let applied = pixelforge(&arg(&[
        "palette",
        "apply-png",
        "--project",
        project_arg,
        "--file",
        frames.join("pixelforge-000.png").to_str().unwrap(),
    ]));
    assert_eq!(status(&applied), 0, "{}", stderr(&applied));
    let crushed = &json_lines(&applied)[0]["result"];
    assert_eq!(crushed["width"], 2);
    assert_eq!(crushed["height"], 1);
    assert_eq!(crushed["pixelsChanged"], 0);
    assert!(crushed["misses"].as_array().unwrap().is_empty());
}

#[test]
fn selection_limits_a_helmet_recolor_to_the_gear_layer() {
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
    assert_eq!(
        status(&pixelforge(&arg(&[
            "layer",
            "add",
            "--project",
            project_arg,
            "--name",
            "gear",
        ]))),
        0
    );
    assert_eq!(
        status(&pixelforge(&arg(&[
            "fill-rect",
            "--project",
            project_arg,
            "--layer",
            "0",
            "--x",
            "1",
            "--y",
            "2",
            "--width",
            "6",
            "--height",
            "5",
            "--color-index",
            "3",
        ]))),
        0
    );
    assert_eq!(
        status(&pixelforge(&arg(&[
            "fill-rect",
            "--project",
            project_arg,
            "--layer",
            "1",
            "--x",
            "2",
            "--y",
            "0",
            "--width",
            "4",
            "--height",
            "2",
            "--color-index",
            "4",
        ]))),
        0
    );

    let selected = pixelforge(&arg(&[
        "selection",
        "set",
        "--project",
        project_arg,
        "--x",
        "2",
        "--y",
        "0",
        "--width",
        "4",
        "--height",
        "2",
    ]));
    assert_eq!(status(&selected), 0, "{}", stderr(&selected));
    let body = &json_lines(&selected)[0];
    assert_eq!(body["ok"], true);
    assert_eq!(
        body["result"]["selection"],
        serde_json::json!({"x": 2, "y": 0, "width": 4, "height": 2})
    );
    let saved = fs::read_to_string(&project).unwrap();
    assert!(saved.contains("\"selection\""));
    assert_eq!(load(&project).active_layer(), 1);

    let recolored = pixelforge(&arg(&[
        "replace-index",
        "--project",
        project_arg,
        "--layer",
        "1",
        "--from-index",
        "4",
        "--to-index",
        "5",
    ]));
    assert_eq!(status(&recolored), 0, "{}", stderr(&recolored));
    assert_eq!(json_lines(&recolored)[0]["result"]["changed"], 8);

    let flooded = pixelforge(&arg(&[
        "fill-rect",
        "--project",
        project_arg,
        "--layer",
        "1",
        "--x",
        "0",
        "--y",
        "0",
        "--width",
        "8",
        "--height",
        "8",
        "--color-index",
        "6",
    ]));
    assert_eq!(status(&flooded), 0, "{}", stderr(&flooded));

    let doc = load(&project);
    assert_eq!(doc.layer_pixels(0, 0).unwrap()[2 * 8 + 1], 3);
    assert_eq!(doc.layer_pixels(0, 1).unwrap()[0], 0);
    assert_eq!(doc.layer_pixels(0, 1).unwrap()[2], 6);
    assert_eq!(doc.layer_pixels(0, 1).unwrap()[1 * 8 + 2], 6);
    assert_eq!(doc.layer_pixels(0, 1).unwrap()[8], 0);
    assert_eq!(doc.active_layer(), 1);
    assert_eq!(
        doc.selection(),
        Some(pixelforge_kernel::Selection {
            x: 2,
            y: 0,
            width: 4,
            height: 2,
        })
    );

    let cleared = pixelforge(&arg(&["selection", "clear", "--project", project_arg]));
    assert_eq!(status(&cleared), 0, "{}", stderr(&cleared));
    assert!(json_lines(&cleared)[0]["result"]["selection"].is_null());
    assert!(!fs::read_to_string(&project).unwrap().contains("selection"));

    assert_eq!(
        status(&pixelforge(&arg(&[
            "paint",
            "--project",
            project_arg,
            "--layer",
            "1",
            "--x",
            "0",
            "--y",
            "7",
            "--color-index",
            "7",
        ]))),
        0
    );
    assert_eq!(load(&project).layer_pixels(0, 1).unwrap()[7 * 8], 7);
}

fn assert_full_grid(body: &Value, width: usize, height: usize) {
    let rows = body["result"]["rows"].as_array().unwrap();
    assert_eq!(rows.len(), height);
    for row in rows {
        assert_eq!(row.as_str().unwrap().chars().count(), width);
    }
}

fn glyph_at(body: &Value, x: usize, y: usize) -> char {
    body["result"]["rows"][y]
        .as_str()
        .unwrap()
        .chars()
        .nth(x)
        .unwrap()
}
