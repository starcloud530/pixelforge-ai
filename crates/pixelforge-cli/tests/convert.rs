//! CLI convert: import a PNG, edit the indexed frame, export it again.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use pixelforge_kernel::{Document, PaletteImportMode};
use serde_json::{json, Value};

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
        .map(|line| serde_json::from_str(line).expect(line))
        .collect()
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
        let path = std::env::temp_dir().join(format!(
            "pixelforge-convert-{nanos}-{n}-{}",
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

fn png_size(bytes: &[u8]) -> (u32, u32) {
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    (width, height)
}

fn reference_png(path: &Path) {
    let mut source = Document::new(4, 2).unwrap();
    source
        .import_palette("ff0000\n0000ff\n", PaletteImportMode::Replace)
        .unwrap();
    source.paint(0, 0, 1).unwrap();
    source.paint(1, 0, 1).unwrap();
    source.paint(2, 0, 2).unwrap();
    source.paint(3, 0, 2).unwrap();
    let png = source.export_png_sequence().remove(0);
    fs::write(path, png).unwrap();
}

#[test]
fn convert_edit_undo_and_export_round_trip() {
    let scratch = Scratch::new();
    let png = scratch.child("ref.png");
    reference_png(&png);
    let project = scratch.child("hero.json");
    let checkpoint = scratch.child("hero.ckpt.json");
    let again = scratch.child("again.json");
    let frames = scratch.child("frames");

    let created = pixelforge(&arg(&[
        "new",
        "--width",
        "8",
        "--height",
        "8",
        "--output",
        project.to_str().unwrap(),
    ]));
    assert_eq!(
        status(&created),
        0,
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );

    let imported = pixelforge(&arg(&[
        "convert",
        "--file",
        png.to_str().unwrap(),
        "--project",
        project.to_str().unwrap(),
        "--checkpoint",
        checkpoint.to_str().unwrap(),
        "--width",
        "4",
        "--height",
        "2",
        "--engine",
        "center-sample",
        "--palette",
        "project",
    ]));
    assert_eq!(
        status(&imported),
        0,
        "{}",
        String::from_utf8_lossy(&imported.stderr)
    );
    let body = &json_lines(&imported)[0];
    assert_eq!(body["ok"], json!(true));
    assert_eq!(body["result"]["width"], json!(4));
    assert_eq!(body["result"]["height"], json!(2));
    assert_eq!(body["result"]["engine"]["id"], json!("center-sample"));
    assert_eq!(body["result"]["transform"]["version"], json!("1"));
    let imported_doc = Document::load_project(&fs::read_to_string(&project).unwrap()).unwrap();
    assert_eq!((imported_doc.width(), imported_doc.height()), (4, 2));
    let before_edit = imported_doc.pixels().to_vec();
    assert_eq!(before_edit[4], 0, "the second row starts transparent");

    let painted = pixelforge(&arg(&[
        "paint",
        "--project",
        project.to_str().unwrap(),
        "--checkpoint",
        checkpoint.to_str().unwrap(),
        "--x",
        "0",
        "--y",
        "1",
        "--color-index",
        "1",
    ]));
    assert_eq!(
        status(&painted),
        0,
        "{}",
        String::from_utf8_lossy(&painted.stderr)
    );

    let exported = pixelforge(&arg(&[
        "export",
        "png",
        "--project",
        project.to_str().unwrap(),
        "--out-dir",
        frames.to_str().unwrap(),
    ]));
    assert_eq!(
        status(&exported),
        0,
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    let edited_png = fs::read(frames.join("pixelforge-000.png")).unwrap();
    assert_eq!(png_size(&edited_png), (4, 2));
    assert_eq!(
        Document::load_project(&fs::read_to_string(&project).unwrap())
            .unwrap()
            .pixels()[4],
        1
    );

    let undone = pixelforge(&arg(&[
        "undo",
        "--project",
        project.to_str().unwrap(),
        "--checkpoint",
        checkpoint.to_str().unwrap(),
    ]));
    assert_eq!(
        status(&undone),
        0,
        "{}",
        String::from_utf8_lossy(&undone.stderr)
    );
    assert_eq!(
        Document::load_project(&fs::read_to_string(&project).unwrap())
            .unwrap()
            .pixels(),
        before_edit
    );

    let restored = pixelforge(&arg(&[
        "undo",
        "--project",
        project.to_str().unwrap(),
        "--checkpoint",
        checkpoint.to_str().unwrap(),
    ]));
    assert_eq!(
        status(&restored),
        0,
        "{}",
        String::from_utf8_lossy(&restored.stderr)
    );
    let back = Document::load_project(&fs::read_to_string(&project).unwrap()).unwrap();
    assert_eq!((back.width(), back.height()), (8, 8));

    let repeat = pixelforge(&arg(&[
        "convert",
        "--file",
        png.to_str().unwrap(),
        "--output",
        again.to_str().unwrap(),
        "--width",
        "4",
        "--height",
        "2",
        "--engine",
        "center-sample",
        "--palette",
        "project",
    ]));
    assert_eq!(
        status(&repeat),
        0,
        "{}",
        String::from_utf8_lossy(&repeat.stderr)
    );
    let repeated = Document::load_project(&fs::read_to_string(&again).unwrap()).unwrap();
    assert_eq!(repeated.pixels(), before_edit.as_slice());
    let repeat_again = scratch.child("again-2.json");
    let repeat_2 = pixelforge(&arg(&[
        "convert",
        "--file",
        png.to_str().unwrap(),
        "--output",
        repeat_again.to_str().unwrap(),
        "--width",
        "4",
        "--height",
        "2",
        "--engine",
        "center-sample",
        "--palette",
        "project",
    ]));
    assert_eq!(status(&repeat_2), 0);
    assert_eq!(
        Document::load_project(&fs::read_to_string(&repeat_again).unwrap())
            .unwrap()
            .pixels(),
        repeated.pixels()
    );
}

#[test]
fn engine_list_set_and_reserved_id() {
    let scratch = Scratch::new();
    let project = scratch.child("hero.json");
    let png = scratch.child("ref.png");
    reference_png(&png);
    let created = pixelforge(&arg(&[
        "new",
        "--width",
        "4",
        "--height",
        "2",
        "--output",
        project.to_str().unwrap(),
    ]));
    assert_eq!(status(&created), 0);

    let listed = pixelforge(&arg(&["engine", "list"]));
    assert_eq!(
        status(&listed),
        0,
        "{}",
        String::from_utf8_lossy(&listed.stderr)
    );
    let body = &json_lines(&listed)[0]["result"];
    assert_eq!(body["default"], json!("center-sample"));
    assert_eq!(body["engines"][5]["id"], json!("pixelforge-native"));
    assert_eq!(body["engines"][5]["status"], json!("reserved"));
    assert_eq!(body["engines"][5]["licensed"], json!(false));

    let set = pixelforge(&arg(&[
        "engine",
        "set",
        "kmeans",
        "--project",
        project.to_str().unwrap(),
    ]));
    assert_eq!(status(&set), 0, "{}", String::from_utf8_lossy(&set.stderr));
    assert_eq!(
        Document::load_project(&fs::read_to_string(&project).unwrap())
            .unwrap()
            .transform_engine(),
        "kmeans"
    );

    let refused = pixelforge(&arg(&[
        "convert",
        "--file",
        png.to_str().unwrap(),
        "--project",
        project.to_str().unwrap(),
        "--engine",
        "pixelforge-native",
    ]));
    assert_eq!(status(&refused), 1);
    let error = &json_lines(&refused)[0];
    assert_eq!(error["ok"], json!(false));
    assert_eq!(error["error"]["code"], json!("EngineUnavailable"));
    assert_eq!(
        Document::load_project(&fs::read_to_string(&project).unwrap())
            .unwrap()
            .transform_engine(),
        "kmeans"
    );
}

#[test]
fn palette_strategies_are_selectable_from_the_cli() {
    let scratch = Scratch::new();
    let png = scratch.child("ref.png");
    reference_png(&png);
    let source = scratch.child("source.json");
    let limited = scratch.child("limit.json");
    let project = scratch.child("project.json");
    let png_arg = png.to_str().unwrap();

    let kept = pixelforge(&arg(&[
        "convert",
        "--file",
        png_arg,
        "--output",
        source.to_str().unwrap(),
        "--width",
        "4",
        "--height",
        "2",
        "--palette-strategy",
        "source",
    ]));
    assert_eq!(
        status(&kept),
        0,
        "{}",
        String::from_utf8_lossy(&kept.stderr)
    );
    let kept_body = &json_lines(&kept)[0];
    assert_eq!(
        kept_body["result"]["paletteReport"]["strategy"],
        json!("source")
    );
    assert_eq!(
        kept_body["result"]["paletteReport"]["totalDistance"],
        json!(0)
    );
    assert_eq!(
        kept_body["result"]["paletteReport"]["paletteColors"],
        json!(2)
    );
    let kept_doc = Document::load_project(&fs::read_to_string(&source).unwrap()).unwrap();
    assert_eq!(kept_doc.palette_colors().len(), 3);

    let capped = pixelforge(&arg(&[
        "convert",
        "--file",
        png_arg,
        "--output",
        limited.to_str().unwrap(),
        "--width",
        "4",
        "--height",
        "2",
        "--palette-strategy",
        "limit",
        "--max-colors",
        "1",
    ]));
    assert_eq!(
        status(&capped),
        0,
        "{}",
        String::from_utf8_lossy(&capped.stderr)
    );
    let capped_body = &json_lines(&capped)[0];
    assert_eq!(
        capped_body["result"]["paletteReport"]["strategy"],
        json!("limit")
    );
    assert_eq!(
        capped_body["result"]["paletteReport"]["paletteColors"],
        json!(1)
    );
    assert!(
        capped_body["result"]["paletteReport"]["totalDistance"]
            .as_u64()
            .unwrap()
            > 0
    );

    let created = pixelforge(&arg(&[
        "new",
        "--width",
        "4",
        "--height",
        "2",
        "--output",
        project.to_str().unwrap(),
    ]));
    assert_eq!(status(&created), 0);
    let before = fs::read_to_string(&project).unwrap();
    let mapped = pixelforge(&arg(&[
        "convert",
        "--file",
        png_arg,
        "--project",
        project.to_str().unwrap(),
        "--palette-strategy",
        "project",
    ]));
    assert_eq!(
        status(&mapped),
        0,
        "{}",
        String::from_utf8_lossy(&mapped.stderr)
    );
    let mapped_body = &json_lines(&mapped)[0];
    assert_eq!(
        mapped_body["result"]["paletteReport"]["strategy"],
        json!("project")
    );
    assert_eq!(
        mapped_body["result"]["paletteReport"]["origin"],
        json!("project")
    );
    assert_eq!(
        Document::load_project(&fs::read_to_string(&project).unwrap())
            .unwrap()
            .palette_colors()
            .len(),
        9
    );

    let before_unknown = fs::read_to_string(&project).unwrap();
    assert!(before_unknown.contains("\"strategy\": \"project\""));
    assert_ne!(before_unknown, before);
    let unknown = pixelforge(&arg(&[
        "convert",
        "--file",
        png_arg,
        "--project",
        project.to_str().unwrap(),
        "--palette-strategy",
        "no-such",
    ]));
    assert_eq!(status(&unknown), 1);
    assert_eq!(
        json_lines(&unknown)[0]["error"]["code"],
        json!("PaletteStrategyUnknown")
    );
    assert_eq!(fs::read_to_string(&project).unwrap(), before_unknown);

    let both = pixelforge(&arg(&[
        "convert",
        "--file",
        png_arg,
        "--project",
        project.to_str().unwrap(),
        "--palette",
        "project",
        "--palette-strategy",
        "source",
    ]));
    assert_eq!(status(&both), 2);
    assert_eq!(json_lines(&both)[0]["error"]["code"], json!("Cli"));
}
