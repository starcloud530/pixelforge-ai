//! Part translate, rotate, and flip land on the indexed grid.

use pixelforge_kernel::Document;
use serde_json::{json, Value};

fn tool(document: &mut Document, name: &str, params: Value) -> Value {
    let response = document.call_tool(
        &json!({
            "tool": name,
            "params": params,
        })
        .to_string(),
    );
    let value: Value = serde_json::from_str(&response).expect("tool JSON");
    assert_eq!(value["ok"], json!(true), "{name}: {response}");
    value["result"].clone()
}

fn tool_err(document: &mut Document, name: &str, params: Value) -> Value {
    let response = document.call_tool(
        &json!({
            "tool": name,
            "params": params,
        })
        .to_string(),
    );
    let value: Value = serde_json::from_str(&response).expect("tool JSON");
    assert_eq!(value["ok"], json!(false), "{name}: {response}");
    value["error"].clone()
}

fn cell(document: &Document, frame: usize, layer: usize, x: u16, y: u16) -> u8 {
    let width = usize::from(document.width());
    document.layer_pixels(frame, layer).unwrap()[usize::from(y) * width + usize::from(x)]
}

fn gun() -> Document {
    let mut document = Document::new(8, 8).unwrap();
    document.paint(4, 3, 1).unwrap();
    document.paint(5, 3, 2).unwrap();
    document.add_part("gun", 3, 3, Some(0)).unwrap();
    document.set_anchor("gun", "grip", 1, 0).unwrap();
    document.set_anchor("gun", "muzzle", 2, 1).unwrap();
    document.clear_history();
    document
}

#[test]
fn quarter_turn_around_the_grip_is_reversible_and_keeps_indices() {
    let mut document = gun();
    let palette = document.palette().len();
    let mut seen = Vec::new();
    for _ in 0..4 {
        let turned = tool(
            &mut document,
            "rotatePart",
            json!({"name": "gun", "degrees": 90, "anchor": "grip", "reportDirty": true}),
        );
        assert_eq!(turned["grid"], json!("quarter"));
        assert_eq!(turned["sampling"], json!("exact"));
        assert_eq!(turned["alignment"], json!("center-on-anchor"));
        assert_eq!(turned["degreesCenti"], json!(9000));
        assert_eq!(turned["dropped"], json!(0));
        assert_eq!(turned["conflicts"], json!(0));
        assert_eq!(turned["sealed"], json!(0));
        assert_eq!(turned["breaks"], json!(0));
        assert_eq!(turned["pivot"]["anchor"], json!("grip"));
        assert!(turned["changed"].as_u64().unwrap() > 0);
        assert!(turned.get("anchors").is_some());
        seen.push((
            document.parts()[0].x,
            document.parts()[0].y,
            document.parts()[0].anchors[0].x,
            document.parts()[0].anchors[0].y,
        ));
        let pivot_x = turned["pivot"]["x"].as_i64().unwrap();
        let pivot_y = turned["pivot"]["y"].as_i64().unwrap();
        assert_eq!(
            document.parts()[0].anchor_canvas("grip"),
            Some((pivot_x as i32, pivot_y as i32))
        );
    }
    assert_eq!(document.parts()[0].x, 3);
    assert_eq!(document.parts()[0].y, 3);
    assert_eq!(document.parts()[0].anchors[0].x, 1);
    assert_eq!(document.parts()[0].anchors[0].y, 0);
    assert_eq!(document.parts()[0].anchors[1].x, 2);
    assert_eq!(document.parts()[0].anchors[1].y, 1);
    assert_eq!(cell(&document, 0, 0, 4, 3), 1);
    assert_eq!(cell(&document, 0, 0, 5, 3), 2);
    assert_eq!(document.palette().len(), palette);
    assert_ne!(seen[0], seen[1]);

    tool(
        &mut document,
        "rotatePart",
        json!({"name": "gun", "degrees": 90, "anchor": "grip"}),
    );
    tool(
        &mut document,
        "rotatePart",
        json!({"name": "gun", "degrees": -90, "anchor": "grip"}),
    );
    assert_eq!(cell(&document, 0, 0, 5, 3), 2);
    assert_eq!(document.parts()[0].anchor_canvas("grip"), Some((4, 3)));

    let near = tool(
        &mut document,
        "rotatePart",
        json!({"name": "gun", "degrees": 90.004, "anchor": "grip"}),
    );
    assert_eq!(near["grid"], json!("quarter"));
    assert_eq!(near["degreesCenti"], json!(9000));
    tool(
        &mut document,
        "rotatePart",
        json!({"name": "gun", "degrees": -90, "anchor": "grip"}),
    );
    assert_eq!(cell(&document, 0, 0, 4, 3), 1);
}

#[test]
fn translate_and_flip_move_ink_with_the_origin_in_one_undo() {
    let mut document = gun();
    let gear = document.add_layer(Some("gear")).unwrap();
    document.paint_on(gear, 7, 0, 3).unwrap();
    let other = document.layer_pixels(0, gear).unwrap().to_vec();
    document.clear_history();

    let moved = tool(
        &mut document,
        "translatePart",
        json!({"name": "gun", "dx": 1, "dy": -1}),
    );
    assert_eq!(moved["alignment"], json!("translate"));
    assert_eq!(moved["sampling"], json!("exact"));
    assert_eq!(moved["dx"], json!(1));
    assert_eq!(moved["dy"], json!(-1));
    assert_eq!(moved["dropped"], json!(0));
    assert!(moved.get("changed").is_none());
    assert_eq!(document.parts()[0].x, 4);
    assert_eq!(document.parts()[0].y, 2);
    assert_eq!(document.parts()[0].anchors[0].x, 1);
    assert_eq!(document.parts()[0].anchors[0].y, 0);
    assert_eq!(cell(&document, 0, 0, 5, 2), 1);
    assert_eq!(cell(&document, 0, 0, 6, 2), 2);
    assert_eq!(cell(&document, 0, 0, 4, 3), 0);
    assert_eq!(document.layer_pixels(0, gear).unwrap(), other.as_slice());
    assert_eq!(document.active_layer(), gear);

    document.undo().unwrap();
    assert_eq!(document.parts()[0].x, 3);
    assert_eq!(cell(&document, 0, 0, 5, 3), 2);
    assert!(!document.can_undo());

    let flipped = tool(
        &mut document,
        "flipPart",
        json!({"name": "gun", "direction": "horizontal", "anchor": "grip"}),
    );
    assert_eq!(flipped["direction"], json!("horizontal"));
    assert_eq!(flipped["alignment"], json!("center-on-anchor"));
    assert_eq!(document.parts()[0].anchor_canvas("grip"), Some((4, 3)));
    assert_eq!(cell(&document, 0, 0, 4, 3), 1);
    assert_eq!(cell(&document, 0, 0, 3, 3), 2);
    tool(
        &mut document,
        "flipPart",
        json!({"name": "gun", "direction": "horizontal", "anchor": "grip"}),
    );
    assert_eq!(document.parts()[0].x, 3);
    assert_eq!(document.parts()[0].anchors[0].x, 1);
    assert_eq!(cell(&document, 0, 0, 5, 3), 2);

    let vertical = tool(
        &mut document,
        "flipPart",
        json!({"name": "gun", "direction": "vertical"}),
    );
    assert_eq!(vertical["alignment"], json!("center-on-origin"));
    assert_eq!(document.parts()[0].x, 3);
    assert_eq!(document.parts()[0].anchors[1].x, 2);
    assert_eq!(document.parts()[0].anchors[1].y, -1);
    tool(
        &mut document,
        "flipPart",
        json!({"name": "gun", "direction": "vertical"}),
    );
    assert_eq!(document.parts()[0].anchors[1].y, 1);
}

#[test]
fn non_quarter_angles_quantize_and_collisions_keep_the_later_cell() {
    let mut document = Document::new(8, 8).unwrap();
    document.paint(3, 2, 1).unwrap();
    document.add_part("gun", 2, 2, Some(0)).unwrap();
    document.clear_history();
    let turned = tool(
        &mut document,
        "rotatePart",
        json!({"name": "gun", "degrees": 30}),
    );
    assert_eq!(turned["grid"], json!("nearest"));
    assert_eq!(turned["sampling"], json!("nearest"));
    assert_eq!(turned["degreesCenti"], json!(3000));
    assert_eq!(turned["dropped"], json!(0));
    assert_eq!(cell(&document, 0, 0, 3, 3), 1);
    assert_eq!(cell(&document, 0, 0, 3, 2), 0);
    assert_eq!(document.parts()[0].x, 2);
    assert_eq!(document.parts()[0].y, 2);

    let mut crowded = Document::new(8, 8).unwrap();
    crowded.paint(5, 1, 1).unwrap();
    crowded.paint(5, 2, 2).unwrap();
    crowded.add_part("gun", 4, 4, Some(0)).unwrap();
    crowded.clear_history();
    let before = tool(
        &mut crowded,
        "rotatePart",
        json!({"name": "gun", "degrees": 20}),
    );
    assert_eq!(before["degreesCenti"], json!(2000));
    assert_eq!(before["dropped"], json!(1));
    assert_eq!(before["conflicts"], json!(1));
    assert_eq!(before["sealed"], json!(0));
    assert_eq!(cell(&crowded, 0, 0, 6, 2), 2);
    assert_eq!(cell(&crowded, 0, 0, 5, 1), 0);
    assert_eq!(cell(&crowded, 0, 0, 5, 2), 0);
    let pixels = crowded.layer_pixels(0, 0).unwrap().to_vec();
    crowded.undo().unwrap();
    tool(
        &mut crowded,
        "rotatePart",
        json!({"name": "gun", "degrees": 20.004}),
    );
    assert_eq!(crowded.layer_pixels(0, 0).unwrap(), pixels.as_slice());
    crowded.undo().unwrap();
    let finer = tool(
        &mut crowded,
        "rotatePart",
        json!({"name": "gun", "degrees": 20.005}),
    );
    assert_eq!(finer["grid"], json!("nearest"));
    assert_eq!(finer["degreesCenti"], json!(2001));

    let mut clean = Document::new(8, 8).unwrap();
    clean.paint(5, 3, 1).unwrap();
    clean.add_part("gun", 4, 4, Some(0)).unwrap();
    clean.clear_history();
    let almost = tool(
        &mut clean,
        "rotatePart",
        json!({"name": "gun", "degrees": 90.005}),
    );
    assert_eq!(almost["grid"], json!("nearest"));
    assert_eq!(almost["degreesCenti"], json!(9001));
}

#[test]
fn zero_motion_is_not_recorded_and_failures_write_nothing() {
    let mut document = gun();
    tool(
        &mut document,
        "translatePart",
        json!({"name": "gun", "dx": 0, "dy": 0}),
    );
    tool(
        &mut document,
        "rotatePart",
        json!({"name": "gun", "degrees": 360}),
    );
    tool(
        &mut document,
        "rotatePart",
        json!({"name": "gun", "degrees": 0.004}),
    );
    assert!(!document.can_undo());

    let pixels = document.layer_pixels(0, 0).unwrap().to_vec();
    let err = tool_err(
        &mut document,
        "translatePart",
        json!({"name": "gun", "dx": 0, "dy": -4}),
    );
    assert_eq!(err["code"], json!("PartOffCanvas"));
    assert_eq!(document.layer_pixels(0, 0).unwrap(), pixels.as_slice());
    assert_eq!(document.parts()[0].y, 3);
    assert!(!document.can_undo());

    assert_eq!(
        tool_err(
            &mut document,
            "rotatePart",
            json!({"name": "gun", "degrees": 1_000_000.5})
        )["code"],
        json!("PartAngle")
    );
    assert_eq!(
        tool_err(
            &mut document,
            "rotatePart",
            json!({"name": "gun", "degrees": "90"})
        )["code"],
        json!("InvalidParams")
    );
    assert_eq!(
        tool_err(
            &mut document,
            "flipPart",
            json!({"name": "gun", "direction": "sideways"})
        )["code"],
        json!("InvalidParams")
    );
    assert_eq!(
        tool_err(
            &mut document,
            "rotatePart",
            json!({"name": "gun", "degrees": 15, "anchor": "stock"})
        )["code"],
        json!("AnchorMissing")
    );
    assert_eq!(document.layer_pixels(0, 0).unwrap(), pixels.as_slice());

    document.set_anchor("gun", "far", 256, 256).unwrap();
    document.clear_history();
    let wide = tool_err(
        &mut document,
        "rotatePart",
        json!({"name": "gun", "degrees": 20}),
    );
    assert_eq!(wide["code"], json!("AnchorCoord"));
    assert_eq!(document.parts()[0].anchors[2].x, 256);
    assert_eq!(cell(&document, 0, 0, 5, 3), 2);
    assert!(!document.can_undo());
}

#[test]
fn selection_must_cover_the_ink_and_rectangle_flip_leaves_the_part() {
    let mut document = gun();
    document.set_selection(4, 3, 2, 1).unwrap();
    document.clear_history();
    let moved = tool(
        &mut document,
        "translatePart",
        json!({"name": "gun", "dx": 0, "dy": 0}),
    );
    assert_eq!(moved["x"], json!(3));
    document.undo().ok();
    let blocked = tool_err(
        &mut document,
        "translatePart",
        json!({"name": "gun", "dx": 1, "dy": 0}),
    );
    assert_eq!(blocked["code"], json!("SelectionProtected"));
    assert_eq!(cell(&document, 0, 0, 5, 3), 2);
    assert_eq!(document.parts()[0].x, 3);

    tool(&mut document, "clearSelection", json!({}));
    document.paint(0, 0, 3).unwrap();
    document.set_selection(4, 3, 2, 1).unwrap();
    document.clear_history();
    let shared = tool_err(
        &mut document,
        "rotatePart",
        json!({"name": "gun", "degrees": 180}),
    );
    assert_eq!(shared["code"], json!("SelectionProtected"));
    assert_eq!(cell(&document, 0, 0, 0, 0), 3);

    tool(&mut document, "clearSelection", json!({}));
    let origin = (document.parts()[0].x, document.parts()[0].y);
    let anchors = document.parts()[0].anchors.clone();
    tool(&mut document, "flipHorizontal", json!({}));
    assert_eq!((document.parts()[0].x, document.parts()[0].y), origin);
    assert_eq!(document.parts()[0].anchors, anchors);
    tool(&mut document, "undo", json!({}));

    let sibling = document.add_part("badge", 1, 1, Some(0)).unwrap();
    document.clear_history();
    tool(
        &mut document,
        "translatePart",
        json!({"name": "gun", "dx": 1, "dy": 0}),
    );
    assert_eq!(document.parts()[sibling].x, 1);
    assert_eq!(document.parts()[sibling].y, 1);
    assert_eq!(document.parts()[0].x, 4);
}

#[test]
fn one_frame_moves_and_an_unattached_part_has_no_pixels() {
    let mut document = gun();
    document.duplicate_frame(0).unwrap();
    assert_eq!(document.active_frame(), 0);
    document.clear_history();
    let frame0 = document.layer_pixels(0, 0).unwrap().to_vec();
    tool(
        &mut document,
        "rotatePart",
        json!({"name": "gun", "degrees": 90, "anchor": "grip", "frame": 1}),
    );
    assert_eq!(document.active_frame(), 0);
    assert_eq!(document.layer_pixels(0, 0).unwrap(), frame0.as_slice());
    assert_ne!(document.layer_pixels(1, 0).unwrap(), frame0.as_slice());
    assert_eq!(document.parts()[0].anchor_canvas("grip"), Some((4, 3)));
}

#[test]
fn unattached_part_translates_without_touching_ink() {
    let mut document = gun();
    document.add_part("shoulder", 1, 1, None).unwrap();
    document.clear_history();
    let pixels = document.layer_pixels(0, 0).unwrap().to_vec();
    let moved = tool(
        &mut document,
        "translatePart",
        json!({"name": "shoulder", "dx": 2, "dy": 3, "reportDirty": true}),
    );
    assert_eq!(moved["x"], json!(3));
    assert_eq!(moved["y"], json!(4));
    assert_eq!(moved["layer"], Value::Null);
    assert_eq!(moved["changed"], json!(0));
    assert!(moved["dirty"].is_null());
    assert_eq!(document.layer_pixels(0, 0).unwrap(), pixels.as_slice());
    document.undo().unwrap();
    assert_eq!(document.parts()[1].x, 1);
    assert_eq!(cell(&document, 0, 0, 5, 3), 2);
}

fn ink_count(document: &Document, frame: usize) -> usize {
    document
        .layer_pixels(frame, 0)
        .unwrap()
        .iter()
        .filter(|pixel| **pixel != 0)
        .count()
}

#[test]
fn coverage_seal_reports_on_rotate_and_bake() {
    let mut document = Document::new(32, 32).unwrap();
    tool(
        &mut document,
        "fillRect",
        json!({"x": 8, "y": 8, "width": 7, "height": 3, "colorIndex": 1}),
    );
    tool(
        &mut document,
        "addPart",
        json!({"name": "block", "x": 11, "y": 9, "layer": 0}),
    );
    let turned = tool(
        &mut document,
        "rotatePart",
        json!({"name": "block", "degrees": 20}),
    );
    assert_eq!(turned["grid"], json!("nearest"));
    assert_eq!(turned["sampling"], json!("nearest"));
    assert_eq!(turned["dropped"], json!(2));
    assert_eq!(turned["conflicts"], json!(2));
    assert_eq!(turned["sealed"], json!(2));
    assert_eq!(turned["breaks"], json!(0));
    assert_eq!(ink_count(&document, 0), 21);

    document.undo().unwrap();
    assert_eq!(ink_count(&document, 0), 21);
    tool(&mut document, "addFrame", json!({}));
    tool(
        &mut document,
        "setCurveKey",
        json!({"part": "block", "channel": "rotation", "frame": 0, "value": 0}),
    );
    tool(
        &mut document,
        "setCurveKey",
        json!({"part": "block", "channel": "rotation", "frame": 1, "value": 20}),
    );
    let baked = tool(&mut document, "bake", json!({"from": 0, "to": 1}));
    assert_eq!(baked["dropped"], json!(2));
    assert_eq!(baked["conflicts"], json!(2));
    assert_eq!(baked["sealed"], json!(2));
    assert_eq!(baked["breaks"], json!(0));
    assert_eq!(baked["frames"][0]["dropped"], json!(0));
    assert_eq!(baked["frames"][0]["sealed"], json!(0));
    assert_eq!(baked["frames"][1]["dropped"], json!(2));
    assert_eq!(baked["frames"][1]["conflicts"], json!(2));
    assert_eq!(baked["frames"][1]["sealed"], json!(2));
    assert_eq!(baked["frames"][1]["breaks"], json!(0));
    assert_eq!(baked["frames"][1]["parts"][0]["dropped"], json!(2));
    assert_eq!(baked["frames"][1]["parts"][0]["sealed"], json!(2));
    assert_eq!(ink_count(&document, 0), 21);
    assert_eq!(ink_count(&document, 1), 21);

    let mut barrel = Document::new(32, 32).unwrap();
    tool(
        &mut barrel,
        "fillRect",
        json!({"x": 8, "y": 8, "width": 2, "height": 12, "colorIndex": 1}),
    );
    tool(
        &mut barrel,
        "addPart",
        json!({"name": "barrel", "x": 8, "y": 13, "layer": 0}),
    );
    let turned = tool(
        &mut barrel,
        "rotatePart",
        json!({"name": "barrel", "degrees": 20}),
    );
    assert_eq!(turned["dropped"], json!(1));
    assert_eq!(turned["sealed"], json!(1));
    assert_eq!(turned["breaks"], json!(0));
    assert_eq!(ink_count(&barrel, 0), 24);

    let mut line = Document::new(32, 32).unwrap();
    tool(
        &mut line,
        "fillRect",
        json!({"x": 16, "y": 10, "width": 1, "height": 9, "colorIndex": 1}),
    );
    tool(
        &mut line,
        "addPart",
        json!({"name": "strip", "x": 16, "y": 14, "layer": 0}),
    );
    let turned = tool(
        &mut line,
        "rotatePart",
        json!({"name": "strip", "degrees": 20}),
    );
    assert_eq!(turned["dropped"], json!(0));
    assert_eq!(turned["sealed"], json!(0));
    assert!(turned["breaks"].as_u64().unwrap() > 0);
    assert_eq!(ink_count(&line, 0), 9);
    line.undo().unwrap();

    let steep = tool(
        &mut line,
        "rotatePart",
        json!({"name": "strip", "degrees": 45}),
    );
    assert!(steep["dropped"].as_u64().unwrap() > 0);
    assert_eq!(steep["sealed"], json!(0));
    assert!(ink_count(&line, 0) < 9);
}
