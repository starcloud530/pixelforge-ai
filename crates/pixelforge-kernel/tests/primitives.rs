//! Line, ellipse, polygon, flood fill, and rectangle outline.

use std::path::PathBuf;

use pixelforge_kernel::{
    Connectivity, Document, IndexPolicy, KernelError, Mask, MaskOp, Selection,
};

fn blank(width: u16, height: u16) -> Document {
    Document::new(width, height).expect("canvas")
}

fn count(document: &Document, index: u8) -> usize {
    document
        .pixels()
        .iter()
        .filter(|cell| **cell == index)
        .count()
}

fn draw_sheet(document: &mut Document) {
    document.line(1, 1, 40, 20, 1, false).unwrap();
    document.line(1, 24, 40, 46, 2, true).unwrap();
    document.ellipse(44, 2, 18, 18, 3, false).unwrap();
    document.ellipse(44, 24, 18, 14, 4, true).unwrap();
    document
        .polygon(&[(2, 50), (20, 50), (18, 62), (4, 60)], 5, false)
        .unwrap();
    document
        .polygon(&[(24, 50), (40, 52), (36, 62), (22, 60)], 6, true)
        .unwrap();
    document.stroke_rect(46, 44, 16, 16, 7).unwrap();
    document.flood_fill(52, 52, 8, Connectivity::Four).unwrap();
}

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/primitives-64.png")
}

#[test]
fn sheet_on_64_matches_the_golden_png_and_is_deterministic() {
    let mut document = blank(64, 64);
    draw_sheet(&mut document);
    assert_eq!(document.get(1, 1).unwrap(), 1);
    assert_eq!(document.get(40, 20).unwrap(), 1);
    assert_eq!(document.get(1, 24).unwrap(), 2);
    assert_eq!(document.get(40, 46).unwrap(), 2);
    assert_eq!(document.get(46, 44).unwrap(), 7);
    assert_eq!(document.get(52, 52).unwrap(), 8);
    assert_eq!(document.get(0, 0).unwrap(), 0);
    assert_eq!(document.get(63, 63).unwrap(), 0);
    assert_eq!(count(&document, 7), 60, "stroke border");
    assert_eq!(count(&document, 8), 196, "flood stays inside the stroke");
    assert!(count(&document, 3) > 8, "ellipse outline");
    assert!(count(&document, 4) > count(&document, 3), "filled ellipse");
    assert!(count(&document, 6) > count(&document, 5), "filled polygon");

    let png = document.export_frame_png(0).unwrap();
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
    let path = golden_path();
    if std::env::var_os("PIXELFORGE_UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &png).unwrap();
    }
    let golden = std::fs::read(&path).unwrap_or_else(|err| {
        panic!(
            "missing {} ({err}); rerun with PIXELFORGE_UPDATE_GOLDEN=1",
            path.display()
        )
    });
    assert_eq!(png, golden, "64x64 primitive sheet drifted");

    let mut again = blank(64, 64);
    draw_sheet(&mut again);
    assert_eq!(document.pixels(), again.pixels());
    assert_eq!(png, again.export_frame_png(0).unwrap());

    for _ in 0..8 {
        document.undo().unwrap();
    }
    assert!(document.pixels().iter().all(|cell| *cell == 0));
    assert!(matches!(document.undo(), Err(KernelError::NothingToUndo)));
}

#[test]
fn each_shape_is_one_undo_and_a_gesture_groups_them() {
    let mut document = blank(16, 16);
    document.line(0, 0, 4, 2, 1, false).unwrap();
    document.ellipse(1, 1, 6, 5, 2, true).unwrap();
    document.undo().unwrap();
    assert_eq!(document.get(0, 0).unwrap(), 1);
    assert_eq!(document.get(3, 3).unwrap(), 0);
    document.undo().unwrap();
    assert!(document.pixels().iter().all(|cell| *cell == 0));

    document.begin_gesture().unwrap();
    document.line(0, 8, 8, 8, 3, true).unwrap();
    document.stroke_rect(2, 2, 5, 4, 4).unwrap();
    document.end_gesture();
    document.undo().unwrap();
    assert!(document.pixels().iter().all(|cell| *cell == 0));
    assert!(matches!(document.undo(), Err(KernelError::NothingToUndo)));
}

#[test]
fn a_repeat_of_the_same_ink_is_not_recorded() {
    let mut document = blank(8, 8);
    document.line(0, 0, 3, 0, 1, false).unwrap();
    document.line(0, 0, 3, 0, 1, false).unwrap();
    document.undo().unwrap();
    assert!(document.pixels().iter().all(|cell| *cell == 0));
}

#[test]
fn out_of_bounds_writes_nothing() {
    let mut document = blank(8, 8);
    document.paint(1, 1, 1).unwrap();
    let before = document.pixels().to_vec();

    assert!(matches!(
        document.line(0, 0, 8, 1, 2, false),
        Err(KernelError::OutOfBounds { x: 8, .. })
    ));
    assert!(matches!(
        document.line(9, 0, 1, 1, 2, true),
        Err(KernelError::OutOfBounds { .. })
    ));
    assert!(matches!(
        document.ellipse(6, 6, 4, 4, 2, true),
        Err(KernelError::OutOfBounds { .. })
    ));
    assert!(matches!(
        document.stroke_rect(0, 0, 9, 2, 2),
        Err(KernelError::OutOfBounds { .. })
    ));
    assert!(matches!(
        document.polygon(&[(0, 0), (1, 1), (8, 1)], 2, false),
        Err(KernelError::OutOfBounds { x: 8, .. })
    ));
    assert!(matches!(
        document.flood_fill(8, 0, 2, Connectivity::Four),
        Err(KernelError::OutOfBounds { .. })
    ));
    assert!(matches!(
        document.polygon(&[(0, 0), (1, 0)], 2, true),
        Err(KernelError::PolygonPoints)
    ));
    assert_eq!(document.pixels(), before.as_slice());
    document.undo().unwrap();
    assert!(document.pixels().iter().all(|cell| *cell == 0));
    assert!(matches!(document.undo(), Err(KernelError::NothingToUndo)));
}

#[test]
fn selection_skips_outside_cells_and_blocks_the_flood() {
    let mut document = blank(8, 8);
    document.set_selection(2, 0, 3, 8).unwrap();
    document.line(0, 1, 7, 1, 1, false).unwrap();
    for x in 0..8 {
        let expect = if (2..5).contains(&x) { 1 } else { 0 };
        assert_eq!(document.get(x, 1).unwrap(), expect, "x={x}");
    }
    document.ellipse(0, 0, 8, 8, 2, true).unwrap();
    assert_eq!(document.get(0, 4).unwrap(), 0);
    assert_eq!(document.get(3, 4).unwrap(), 2);

    document.clear_selection();
    document.line(0, 3, 7, 3, 3, false).unwrap();
    document.set_selection(0, 3, 1, 1).unwrap();
    document.flood_fill(0, 3, 4, Connectivity::Eight).unwrap();
    assert_eq!(document.get(0, 3).unwrap(), 4);
    assert_eq!(document.get(1, 3).unwrap(), 3, "mask is a barrier");

    let mask = Mask::from_rect(Selection {
        x: 0,
        y: 3,
        width: 1,
        height: 1,
    });
    document.combine_mask(MaskOp::Subtract, None, mask).unwrap();
    let before = document.pixels().to_vec();
    document.line(0, 0, 7, 0, 5, false).unwrap();
    assert_eq!(
        document.pixels(),
        before.as_slice(),
        "empty mask writes nothing"
    );
}

#[test]
fn locked_palette_still_paints_an_existing_index() {
    let mut document = blank(8, 8);
    document.set_palette_locked(true);
    document.line(0, 0, 4, 0, 1, false).unwrap();
    document.ellipse(1, 1, 4, 4, 2, false).unwrap();
    document
        .polygon(&[(0, 6), (3, 6), (2, 7)], 3, true)
        .unwrap();
    document.stroke_rect(0, 2, 3, 3, 4).unwrap();
    document.flood_fill(0, 0, 5, Connectivity::Four).unwrap();
    assert_eq!(document.get(0, 0).unwrap(), 5);
    assert!(matches!(
        document.set_palette_color(1, 9, 9, 9),
        Err(KernelError::PaletteLocked)
    ));
    assert!(matches!(
        document.line(0, 4, 1, 4, 0, false),
        Err(KernelError::PaletteIndex { index: 0 })
    ));
    assert!(matches!(
        document.ellipse(0, 0, 3, 3, 40, true),
        Err(KernelError::PaletteIndex { index: 40 })
    ));
    document.set_index_policy(IndexPolicy::Clamp);
    document.stroke_rect(5, 5, 2, 2, 40).unwrap();
    assert_eq!(
        document.get(5, 5).unwrap(),
        8,
        "clamp writes the last opaque index"
    );
}

#[test]
fn flood_4_stops_at_a_corner_that_8_crosses() {
    let mut document = blank(4, 4);
    document.paint(0, 0, 1).unwrap();
    document.paint(1, 1, 1).unwrap();
    document.flood_fill(0, 0, 2, Connectivity::Four).unwrap();
    assert_eq!(document.get(0, 0).unwrap(), 2);
    assert_eq!(document.get(1, 1).unwrap(), 1);
    document.undo().unwrap();
    document.flood_fill(0, 0, 2, Connectivity::Eight).unwrap();
    assert_eq!(document.get(0, 0).unwrap(), 2);
    assert_eq!(document.get(1, 1).unwrap(), 2);
}

#[test]
fn drawing_stays_on_the_named_layer() {
    let mut document = blank(8, 8);
    document.line_on(0, 0, 0, 3, 0, 1, false).unwrap();
    let gear = document.add_layer(Some("gear")).unwrap();
    assert_eq!(document.active_layer(), gear);
    document.ellipse_on(gear, 1, 1, 4, 4, 2, true).unwrap();
    assert_eq!(document.active_layer(), gear);
    assert_eq!(document.layer_pixels(0, 0).unwrap()[0], 1);
    assert!(document
        .layer_pixels(0, 0)
        .unwrap()
        .iter()
        .all(|cell| *cell != 2));
    assert!(document
        .layer_pixels(0, gear)
        .unwrap()
        .iter()
        .any(|cell| *cell == 2));
}

#[test]
fn tool_api_reports_dirty_and_refuses_bad_params() {
    let mut document = blank(8, 8);
    let line = call(
        &mut document,
        serde_json::json!({
            "tool": "line",
            "params": {
                "x0": 0, "y0": 0, "x1": 3, "y1": 0,
                "colorIndex": 1, "reportDirty": true
            }
        }),
    );
    assert_eq!(line["ok"], true);
    assert_eq!(line["result"]["changed"], 4);
    assert_eq!(line["result"]["dirty"]["width"], 4);
    assert_eq!(line["result"]["dirty"]["height"], 1);

    let perfect = call(
        &mut document,
        serde_json::json!({
            "tool": "line",
            "params": {"x0": 0, "y0": 2, "x1": 4, "y1": 5, "colorIndex": 2, "pixelPerfect": true}
        }),
    );
    assert_eq!(perfect["ok"], true);

    let short = call(
        &mut document,
        serde_json::json!({
            "tool": "polygon",
            "params": {"points": [{"x": 0, "y": 0}, {"x": 1, "y": 1}], "colorIndex": 1}
        }),
    );
    assert_eq!(short["ok"], false);
    assert_eq!(short["error"]["code"], "InvalidParams");

    let connectivity = call(
        &mut document,
        serde_json::json!({
            "tool": "fill",
            "params": {"x": 0, "y": 0, "colorIndex": 3, "connectivity": 5}
        }),
    );
    assert_eq!(connectivity["error"]["code"], "InvalidParams");

    document.duplicate_frame(0).unwrap();
    document.set_active_frame(1).unwrap();
    document.stroke_rect(1, 1, 3, 3, 4).unwrap();
    let diff = call(
        &mut document,
        serde_json::json!({"tool": "diffFrames", "params": {"frame": 0, "with": 1}}),
    );
    assert_eq!(diff["ok"], true);
    assert!(diff["result"]["changed"].as_u64().unwrap() > 0);
}

fn call(document: &mut Document, body: serde_json::Value) -> serde_json::Value {
    let raw = document.call_tool(&body.to_string());
    serde_json::from_str(&raw).expect("json")
}
