//! Rectangular selection: persisted mask, paint tools, and explicit non-goals.

use pixelforge_kernel::{Document, KernelError, ProjectFileError, Selection, TOOL_API_VERSION};
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

fn rect(x: u16, y: u16, width: u16, height: u16) -> Selection {
    Selection {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn selection_is_stored_in_the_project_and_undoes_inside_one_process() {
    let mut document = Document::new(4, 4).unwrap();
    assert!(tool(&mut document, "inspect", json!({}))["selection"].is_null());
    assert!(tool(&mut document, "scene", json!({}))["selection"].is_null());
    assert!(!document.save_project().contains("selection"));

    let set = tool(
        &mut document,
        "setSelection",
        json!({"x": 1, "y": 2, "width": 2, "height": 1}),
    );
    assert_eq!(
        set["selection"],
        json!({"x": 1, "y": 2, "width": 2, "height": 1})
    );
    assert_eq!(document.selection_rect(), Some(rect(1, 2, 2, 1)));
    assert_eq!(document.active_layer(), 0);
    let saved = document.save_project();
    assert!(saved.contains("\"selection\""));
    let loaded = Document::load_project(&saved).unwrap();
    assert_eq!(loaded.selection_rect(), Some(rect(1, 2, 2, 1)));
    assert!(!loaded.can_undo());

    tool(
        &mut document,
        "setSelection",
        json!({"x": 1, "y": 2, "width": 2, "height": 1}),
    );
    tool(&mut document, "undo", json!({}));
    assert_eq!(document.selection(), None);
    tool(&mut document, "redo", json!({}));
    assert_eq!(document.selection_rect(), Some(rect(1, 2, 2, 1)));

    let cleared = tool(&mut document, "clearSelection", json!({}));
    assert!(cleared["selection"].is_null());
    assert!(!document.save_project().contains("selection"));
    tool(&mut document, "clearSelection", json!({}));
    tool(&mut document, "undo", json!({}));
    assert_eq!(document.selection_rect(), Some(rect(1, 2, 2, 1)));
}

#[test]
fn a_selection_off_the_canvas_writes_nothing() {
    let mut document = Document::new(4, 4).unwrap();
    let err = tool_err(
        &mut document,
        "setSelection",
        json!({"x": 3, "y": 0, "width": 2, "height": 1}),
    );
    assert_eq!(err["code"], json!("OutOfBounds"));
    assert_eq!(document.selection(), None);
    assert!(!document.can_undo());
    let zero = tool_err(
        &mut document,
        "setSelection",
        json!({"x": 0, "y": 0, "width": 0, "height": 1}),
    );
    assert_eq!(zero["code"], json!("InvalidParams"));
}

#[test]
fn paint_fill_erase_and_replace_protect_cells_outside_the_rectangle() {
    let mut document = Document::new(4, 2).unwrap();
    tool(
        &mut document,
        "setSelection",
        json!({"x": 1, "y": 0, "width": 2, "height": 1}),
    );
    let outside = tool_err(
        &mut document,
        "paint",
        json!({"x": 9, "y": 0, "colorIndex": 1}),
    );
    assert_eq!(outside["code"], json!("OutOfBounds"));

    tool(
        &mut document,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 1}),
    );
    tool(
        &mut document,
        "paint",
        json!({"x": 1, "y": 0, "colorIndex": 1}),
    );
    assert_eq!(document.pixels(), &[0, 1, 0, 0, 0, 0, 0, 0]);

    let dirty = tool(
        &mut document,
        "fillRect",
        json!({"x": 0, "y": 0, "width": 4, "height": 1, "colorIndex": 2, "reportDirty": true}),
    );
    assert_eq!(dirty["changed"], json!(2));
    assert_eq!(
        dirty["dirty"],
        json!({"x": 1, "y": 0, "width": 2, "height": 1})
    );
    assert_eq!(document.pixels()[..4], [0, 2, 2, 0]);

    tool(
        &mut document,
        "fillRect",
        json!({"x": 0, "y": 1, "width": 4, "height": 1, "colorIndex": 3}),
    );
    assert_eq!(document.pixels()[4..], [0, 0, 0, 0]);

    tool(&mut document, "erase", json!({"x": 0, "y": 0}));
    tool(&mut document, "erase", json!({"x": 1, "y": 0}));
    assert_eq!(document.pixels()[..4], [0, 0, 2, 0]);
    tool(
        &mut document,
        "eraseRect",
        json!({"x": 0, "y": 0, "width": 4, "height": 1}),
    );
    assert_eq!(document.pixels()[..4], [0, 0, 0, 0]);

    tool(&mut document, "clearSelection", json!({}));
    tool(
        &mut document,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 1}),
    );
    tool(
        &mut document,
        "paint",
        json!({"x": 1, "y": 0, "colorIndex": 1}),
    );
    tool(
        &mut document,
        "paint",
        json!({"x": 3, "y": 0, "colorIndex": 1}),
    );
    tool(
        &mut document,
        "setSelection",
        json!({"x": 0, "y": 0, "width": 2, "height": 1}),
    );
    let replaced = tool(
        &mut document,
        "replaceIndex",
        json!({"fromIndex": 1, "toIndex": 4}),
    );
    assert_eq!(replaced["changed"], json!(2));
    assert_eq!(replaced["fromIndex"], json!(1));
    assert_eq!(replaced["toIndex"], json!(4));
    assert_eq!(document.get(0, 0).unwrap(), 4);
    assert_eq!(document.get(1, 0).unwrap(), 4);
    assert_eq!(document.get(3, 0).unwrap(), 1);

    let cleared = tool(
        &mut document,
        "replaceIndex",
        json!({"fromIndex": 4, "toIndex": 0}),
    );
    assert_eq!(cleared["changed"], json!(2));
    assert_eq!(document.get(0, 0).unwrap(), 0);
    assert_eq!(document.get(3, 0).unwrap(), 1);

    tool(&mut document, "clearSelection", json!({}));
    tool(&mut document, "erase", json!({"x": 3, "y": 0}));
    let filled = tool(
        &mut document,
        "replaceIndex",
        json!({"fromIndex": 0, "toIndex": 5}),
    );
    assert_eq!(filled["changed"], json!(8));
    assert!(document.pixels().iter().all(|index| *index == 5));
}

#[test]
fn selection_masks_one_layer_and_does_not_select_it() {
    let mut document = Document::new(4, 2).unwrap();
    let added = tool(&mut document, "addLayer", json!({"name": "gear"}));
    assert_eq!(added["activeLayer"], json!(1));
    tool(
        &mut document,
        "setSelection",
        json!({"x": 0, "y": 0, "width": 1, "height": 1}),
    );
    assert_eq!(document.active_layer(), 1);

    tool(
        &mut document,
        "paint",
        json!({"layer": 0, "x": 0, "y": 0, "colorIndex": 1}),
    );
    tool(
        &mut document,
        "paint",
        json!({"layer": 0, "x": 3, "y": 0, "colorIndex": 1}),
    );
    tool(
        &mut document,
        "paint",
        json!({"layer": 1, "x": 0, "y": 0, "colorIndex": 2}),
    );
    tool(
        &mut document,
        "paint",
        json!({"layer": 1, "x": 3, "y": 0, "colorIndex": 2}),
    );
    assert_eq!(document.layer_pixels(0, 0).unwrap()[0], 1);
    assert_eq!(document.layer_pixels(0, 0).unwrap()[3], 0);
    assert_eq!(document.layer_pixels(0, 1).unwrap()[0], 2);
    assert_eq!(document.layer_pixels(0, 1).unwrap()[3], 0);
    assert_eq!(document.active_layer(), 1);

    tool(
        &mut document,
        "replaceIndex",
        json!({"layer": 1, "fromIndex": 2, "toIndex": 6}),
    );
    assert_eq!(document.layer_pixels(0, 1).unwrap()[0], 6);
    assert_eq!(document.layer_pixels(0, 0).unwrap()[0], 1);
    assert_eq!(document.active_layer(), 1);

    tool(&mut document, "addFrame", json!({}));
    tool(&mut document, "setActiveFrame", json!({"index": 1}));
    tool(
        &mut document,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 3}),
    );
    tool(&mut document, "setActiveFrame", json!({"index": 0}));
    tool(
        &mut document,
        "replaceIndex",
        json!({"fromIndex": 6, "toIndex": 7}),
    );
    assert_eq!(document.layer_pixels(0, 1).unwrap()[0], 7);
    assert_eq!(document.layer_pixels(1, 1).unwrap()[0], 3);
    assert_eq!(document.active_layer(), 1);
}

#[test]
fn copy_skips_protected_destinations_and_move_refuses_a_partial_move() {
    let mut document = Document::new(4, 1).unwrap();
    tool(
        &mut document,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 1}),
    );
    tool(
        &mut document,
        "setSelection",
        json!({"x": 2, "y": 0, "width": 1, "height": 1}),
    );
    tool(
        &mut document,
        "copyRect",
        json!({"srcX": 0, "srcY": 0, "width": 1, "height": 1, "dstX": 2, "dstY": 0}),
    );
    tool(
        &mut document,
        "copyRect",
        json!({"srcX": 0, "srcY": 0, "width": 1, "height": 1, "dstX": 3, "dstY": 0}),
    );
    assert_eq!(document.pixels(), &[1, 0, 1, 0]);

    let err = tool_err(
        &mut document,
        "moveRect",
        json!({"srcX": 0, "srcY": 0, "width": 1, "height": 1, "dstX": 2, "dstY": 0}),
    );
    assert_eq!(err["code"], json!("SelectionProtected"));
    assert_eq!(
        err["message"],
        json!("selection protects pixels outside the rectangle")
    );
    assert_eq!(document.pixels(), &[1, 0, 1, 0]);

    tool(
        &mut document,
        "setSelection",
        json!({"x": 0, "y": 0, "width": 4, "height": 1}),
    );
    tool(
        &mut document,
        "moveRect",
        json!({"srcX": 0, "srcY": 0, "width": 1, "height": 1, "dstX": 1, "dstY": 0}),
    );
    assert_eq!(document.pixels(), &[0, 1, 1, 0]);
}

#[test]
fn flip_swaps_only_pairs_that_both_sit_inside_the_selection() {
    let mut document = Document::new(4, 1).unwrap();
    for (x, color) in [(0, 1), (1, 2), (2, 3), (3, 4)] {
        tool(
            &mut document,
            "paint",
            json!({"x": x, "y": 0, "colorIndex": color}),
        );
    }
    tool(
        &mut document,
        "setSelection",
        json!({"x": 1, "y": 0, "width": 2, "height": 1}),
    );
    tool(&mut document, "flipHorizontal", json!({}));
    assert_eq!(document.pixels(), &[1, 3, 2, 4]);
}

#[test]
fn resize_moves_the_rectangle_with_the_pixels_or_clears_it() {
    let mut document = Document::new(8, 8).unwrap();
    tool(
        &mut document,
        "setSelection",
        json!({"x": 1, "y": 1, "width": 2, "height": 2}),
    );
    tool(
        &mut document,
        "resizeCanvas",
        json!({"width": 10, "height": 10, "offsetX": 1, "offsetY": 1}),
    );
    assert_eq!(document.selection_rect(), Some(rect(2, 2, 2, 2)));
    assert_eq!((document.width(), document.height()), (10, 10));
    tool(&mut document, "undo", json!({}));
    assert_eq!(document.selection_rect(), Some(rect(1, 1, 2, 2)));
    assert_eq!((document.width(), document.height()), (8, 8));

    tool(
        &mut document,
        "resizeCanvas",
        json!({"width": 2, "height": 2, "offsetX": 0, "offsetY": 0}),
    );
    assert_eq!(document.selection(), None);
    tool(&mut document, "undo", json!({}));
    assert_eq!(document.selection_rect(), Some(rect(1, 1, 2, 2)));
    assert_eq!((document.width(), document.height()), (8, 8));
}

#[test]
fn one_gesture_undoes_the_mask_and_the_fill() {
    let mut document = Document::new(4, 1).unwrap();
    tool(&mut document, "beginGesture", json!({}));
    tool(
        &mut document,
        "setSelection",
        json!({"x": 1, "y": 0, "width": 2, "height": 1}),
    );
    tool(
        &mut document,
        "fillRect",
        json!({"x": 0, "y": 0, "width": 4, "height": 1, "colorIndex": 1}),
    );
    tool(&mut document, "endGesture", json!({}));
    assert_eq!(document.pixels(), &[0, 1, 1, 0]);
    tool(&mut document, "undo", json!({}));
    assert_eq!(document.selection(), None);
    assert_eq!(document.pixels(), &[0, 0, 0, 0]);
    tool(&mut document, "redo", json!({}));
    assert_eq!(document.selection_rect(), Some(rect(1, 0, 2, 1)));
    assert_eq!(document.pixels(), &[0, 1, 1, 0]);
}

#[test]
fn remap_still_rewrites_pixels_outside_the_selection() {
    let mut document = Document::new(4, 1).unwrap();
    for (x, color) in [(0, 1), (1, 2), (2, 3), (3, 4)] {
        tool(
            &mut document,
            "paint",
            json!({"x": x, "y": 0, "colorIndex": color}),
        );
    }
    tool(
        &mut document,
        "setSelection",
        json!({"x": 0, "y": 0, "width": 1, "height": 1}),
    );
    tool(&mut document, "remapPalette", json!({"maxColors": 1}));
    let outside = [
        document.get(1, 0).unwrap(),
        document.get(2, 0).unwrap(),
        document.get(3, 0).unwrap(),
    ];
    let changed = outside
        .iter()
        .zip([2u8, 3, 4])
        .filter(|(now, before)| *now != before)
        .count();
    assert!(
        changed >= 2,
        "remap ignored the mask, outside cells {outside:?}"
    );
}

#[test]
fn a_bad_selection_field_refuses_the_project_file() {
    let palette = r#"[{"r":0,"g":0,"b":0,"a":0},{"r":1,"g":2,"b":3,"a":255}]"#;
    let pixels = "[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]";
    let hanging = format!(
        r#"{{"format":"pixelforge-project","version":1,"width":4,"height":4,"activeFrame":0,"palette":{palette},"selection":{{"x":3,"y":0,"width":2,"height":1}},"frames":[{{"pixels":{pixels},"delayMs":100}}]}}"#
    );
    let error = Document::load_project(&hanging).unwrap_err();
    assert_eq!(error, KernelError::ProjectFile(ProjectFileError::Selection));

    let old = format!(
        r#"{{"format":"pixelforge-project","version":1,"width":4,"height":4,"activeFrame":0,"palette":{palette},"frames":[{{"pixels":{pixels},"delayMs":100}}]}}"#
    );
    let loaded = Document::load_project(&old).unwrap();
    assert_eq!(loaded.selection(), None);
}

#[test]
fn lasso_is_not_a_tool_and_magic_wand_is_not_the_tool_name() {
    let mut document = Document::new(4, 4).unwrap();
    assert_eq!(
        tool_err(&mut document, "lasso", json!({}))["code"],
        json!("UnknownTool")
    );
    assert_eq!(
        tool_err(&mut document, "magicWand", json!({}))["code"],
        json!("UnknownTool")
    );
}
