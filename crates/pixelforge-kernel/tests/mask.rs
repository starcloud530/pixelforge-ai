//! Selection-scoped preview and diff, and rectangle mask booleans.

use pixelforge_kernel::{Document, TOOL_API_VERSION};
use serde_json::{json, Value};

fn call(document: &mut Document, body: Value) -> Value {
    let raw = document.call_tool(&body.to_string());
    serde_json::from_str(&raw).expect("tool response is JSON")
}

fn tool(document: &mut Document, name: &str, params: Value) -> Value {
    let response = call(
        document,
        json!({
            "apiVersion": TOOL_API_VERSION,
            "id": name,
            "tool": name,
            "params": params,
        }),
    );
    assert_eq!(response["ok"], json!(true), "{name}: {response}");
    response["result"].clone()
}

fn tool_err(document: &mut Document, name: &str, params: Value) -> Value {
    let response = call(document, json!({ "tool": name, "params": params }));
    assert_eq!(response["ok"], json!(false), "{name}: {response}");
    response["error"].clone()
}

fn cell(document: &Document, frame: usize, layer: usize, x: usize, y: usize) -> u8 {
    let width = usize::from(document.width());
    document.layer_pixels(frame, layer).unwrap()[y * width + x]
}

#[test]
fn subtract_feet_then_fill_stays_on_the_active_layer_and_reports_dirty() {
    let mut document = Document::new(8, 6).unwrap();
    document.add_layer(Some("gear")).unwrap();
    assert_eq!(document.active_layer(), 1);
    document.set_active_layer(0).unwrap();
    tool(
        &mut document,
        "setSelection",
        json!({"x": 1, "y": 1, "width": 4, "height": 4}),
    );
    let cut = tool(
        &mut document,
        "combineMask",
        json!({"op": "subtract", "x": 2, "y": 4, "width": 2, "height": 1}),
    );
    assert_eq!(cut["op"], json!("subtract"));
    assert_eq!(cut["cells"], json!(14));
    assert!(cut["selection"]["rects"].is_array());
    assert_eq!(document.active_layer(), 0);

    let filled = tool(
        &mut document,
        "fillRect",
        json!({
            "x": 0,
            "y": 0,
            "width": 8,
            "height": 6,
            "colorIndex": 3,
            "layer": 1,
            "reportDirty": true,
        }),
    );
    assert_eq!(filled["changed"], json!(14));
    assert_eq!(filled["regions"][0]["layer"], json!(1));
    assert_eq!(filled["regions"][0]["changed"], json!(14));
    assert_eq!(document.active_layer(), 0);
    assert_eq!(cell(&document, 0, 0, 2, 2), 0);
    assert_eq!(cell(&document, 0, 1, 2, 2), 3);
    assert_eq!(cell(&document, 0, 1, 2, 4), 0);
    assert_eq!(cell(&document, 0, 1, 3, 4), 0);
    assert_eq!(cell(&document, 0, 1, 1, 4), 3);
    assert_eq!(cell(&document, 0, 1, 0, 0), 0);

    tool(&mut document, "undo", json!({}));
    assert_eq!(cell(&document, 0, 1, 2, 2), 0);
    tool(&mut document, "undo", json!({}));
    assert!(document.selection_rect().is_some());
    assert_eq!(document.selection_rect().unwrap().width, 4);
}

#[test]
fn two_masks_combine_without_a_stored_selection_and_chain() {
    let mut document = Document::new(6, 4).unwrap();
    let joined = tool(
        &mut document,
        "combineMask",
        json!({
            "op": "union",
            "base": {"x": 0, "y": 0, "width": 2, "height": 2},
            "mask": [{"x": 2, "y": 0, "width": 2, "height": 2}],
        }),
    );
    assert_eq!(
        joined["selection"],
        json!({"x": 0, "y": 0, "width": 4, "height": 2})
    );
    assert!(!document.save_project().contains("\"rects\""));

    let overlap = tool(
        &mut document,
        "combineMask",
        json!({"op": "intersect", "x": 1, "y": 0, "width": 2, "height": 2}),
    );
    assert_eq!(overlap["cells"], json!(4));
    assert_eq!(
        overlap["selection"],
        json!({"x": 1, "y": 0, "width": 2, "height": 2})
    );

    let empty = tool(
        &mut document,
        "combineMask",
        json!({
            "op": "intersect",
            "base": {"x": 0, "y": 0, "width": 1, "height": 1},
            "x": 4,
            "y": 0,
            "width": 1,
            "height": 1,
        }),
    );
    assert_eq!(empty["selection"], json!({"rects": []}));
    assert_eq!(empty["cells"], json!(0));
    let fill = tool(
        &mut document,
        "fillRect",
        json!({
            "x": 0,
            "y": 0,
            "width": 6,
            "height": 4,
            "colorIndex": 1,
            "reportDirty": true,
        }),
    );
    assert_eq!(fill["changed"], json!(0));
    assert!(fill["dirty"].is_null());
    assert_eq!(document.active_layer(), 0);

    let again = tool(
        &mut document,
        "combineMask",
        json!({
            "op": "intersect",
            "base": {"x": 0, "y": 0, "width": 1, "height": 1},
            "mask": [{"x": 4, "y": 0, "width": 1, "height": 1}],
        }),
    );
    assert_eq!(again["cells"], json!(0));
    tool(&mut document, "undo", json!({}));
    assert_eq!(document.selection().unwrap().cell_count(), 4);
}

#[test]
fn missing_selection_refuses_subtract_and_scoped_diff_without_writing() {
    let mut document = Document::new(4, 2).unwrap();
    let err = tool_err(
        &mut document,
        "combineMask",
        json!({"op": "subtract", "x": 0, "y": 0, "width": 1, "height": 1}),
    );
    assert_eq!(err["code"], json!("MaskMissing"));
    assert!(document.selection().is_none());
    let union = tool(
        &mut document,
        "combineMask",
        json!({"op": "union", "x": 0, "y": 0, "width": 2, "height": 1}),
    );
    assert_eq!(union["cells"], json!(2));

    document.clear_selection();
    let preview = tool_err(&mut document, "previewFrame", json!({"selection": true}));
    assert_eq!(preview["code"], json!("MaskMissing"));
    let both = tool_err(
        &mut document,
        "diffFrames",
        json!({"frame": 0, "with": 0, "selection": true, "x": 0, "y": 0, "width": 1, "height": 1}),
    );
    assert_eq!(both["code"], json!("InvalidParams"));
    assert_eq!(
        tool_err(&mut document, "lasso", json!({}))["code"],
        json!("UnknownTool")
    );
    assert_eq!(
        tool_err(
            &mut document,
            "combineMask",
            json!({"op": "union", "x": 0, "y": 0, "width": 1, "height": 1, "reportDirty": true}),
        )["code"],
        json!("InvalidParams")
    );
}

#[test]
fn preview_and_diff_honor_the_helmet_and_leave_the_layer_alone() {
    let mut document = Document::new(4, 2).unwrap();
    document.add_layer(Some("gear")).unwrap();
    document.set_active_layer(0).unwrap();
    tool(
        &mut document,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 1, "layer": 0}),
    );
    tool(
        &mut document,
        "paint",
        json!({"x": 1, "y": 0, "colorIndex": 4, "layer": 1}),
    );
    document.add_frame();
    document.set_active_frame(1).unwrap();
    tool(
        &mut document,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 2, "layer": 0}),
    );
    tool(
        &mut document,
        "paint",
        json!({"x": 1, "y": 0, "colorIndex": 6, "layer": 1}),
    );
    document.set_active_frame(0).unwrap();
    tool(
        &mut document,
        "setSelection",
        json!({"x": 1, "y": 0, "width": 1, "height": 1}),
    );

    let preview = tool(
        &mut document,
        "previewFrame",
        json!({"selection": true, "layer": 1}),
    );
    assert_eq!(preview["activeLayer"], json!(0));
    assert_eq!(preview["layer"], json!(1));
    assert_eq!(preview["scope"], json!("selection"));
    assert_eq!(preview["ink"], json!(1));
    assert_eq!(preview["protected"], json!(7));
    assert_eq!(
        preview["bounds"],
        json!({"x": 1, "y": 0, "width": 1, "height": 1})
    );
    assert_eq!(document.active_layer(), 0);
    assert_eq!(document.active_frame(), 0);

    let diff = tool(
        &mut document,
        "diffFrames",
        json!({"frame": 0, "with": 1, "selection": true, "layer": 1}),
    );
    assert_eq!(diff["changed"], json!(1));
    assert_eq!(diff["activeLayer"], json!(0));
    assert_eq!(diff["scope"], json!("selection"));
    assert_eq!(diff["protected"], json!(7));
    assert_eq!(diff["diffRows"][0], json!("#!##"));
    assert_eq!(diff["changes"].as_array().unwrap().len(), 1);
    assert_eq!(diff["changes"][0]["x"], json!(1));
    assert_eq!(diff["changes"][0]["left"], json!(4));
    assert_eq!(diff["changes"][0]["right"], json!(6));
    assert_eq!(diff["kinds"]["differ"], json!(1));
    assert_eq!(diff["kinds"]["same"], json!(0));

    let rect = tool(
        &mut document,
        "diffFrames",
        json!({"frame": 0, "with": 1, "x": 0, "y": 0, "width": 1, "height": 1, "layer": 0}),
    );
    assert_eq!(rect["scope"], json!("rect"));
    assert_eq!(rect["changed"], json!(1));
    assert_eq!(rect["diffRows"][0], json!("!###"));
    assert_eq!(document.active_layer(), 0);

    let whole = tool(
        &mut document,
        "diffFrames",
        json!({"frame": 0, "with": 1, "layer": 0}),
    );
    assert!(whole.get("scope").is_none());
    assert_eq!(whole["changed"], json!(1));
}

#[test]
fn a_multi_rect_mask_roundtrips_and_resize_drops_it_when_a_piece_leaves() {
    let mut document = Document::new(4, 4).unwrap();
    tool(
        &mut document,
        "combineMask",
        json!({
            "op": "union",
            "base": [
                {"x": 0, "y": 0, "width": 1, "height": 1},
                {"x": 3, "y": 3, "width": 1, "height": 1},
            ],
            "x": 0,
            "y": 0,
            "width": 1,
            "height": 1,
        }),
    );
    let saved = document.save_project();
    assert!(saved.contains("\"rects\""));
    let loaded = Document::load_project(&saved).unwrap();
    assert_eq!(loaded.selection().unwrap().cell_count(), 2);
    assert!(loaded.selection_rect().is_none());

    let moved = tool(
        &mut document,
        "resizeCanvas",
        json!({"width": 4, "height": 4, "offsetX": -1, "offsetY": 0}),
    );
    assert_eq!(moved["width"], json!(4));
    assert!(document.selection().is_none());
}
