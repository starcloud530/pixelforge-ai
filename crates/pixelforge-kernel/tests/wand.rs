//! Indexed connected-component selection. The result is the rectangle mask
//! paint, erase, fill, combineMask, and selection-scoped diff already read.

use pixelforge_kernel::{Document, Selection, TOOL_API_VERSION};
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

fn paint_l(document: &mut Document) {
    for (x, y) in [(0, 0), (1, 0), (0, 1), (0, 2), (1, 2), (2, 2)] {
        tool(document, "paint", json!({"x": x, "y": y, "colorIndex": 1}));
    }
}

#[test]
fn exact_component_is_the_mask_paint_and_erase_already_use() {
    let mut document = Document::new(4, 3).unwrap();
    tool(
        &mut document,
        "fillRect",
        json!({"x": 1, "y": 1, "width": 2, "height": 2, "colorIndex": 1}),
    );
    let solid = tool(&mut document, "selectConnected", json!({"x": 2, "y": 2}));
    assert_eq!(solid["cells"], json!(4));
    assert_eq!(solid["index"], json!(1));
    assert_eq!(solid["tolerance"], json!(0));
    assert_eq!(solid["connectivity"], json!(4));
    assert_eq!(solid["frame"], json!(0));
    assert_eq!(solid["layer"], json!(0));
    assert_eq!(
        solid["selection"],
        json!({"x": 1, "y": 1, "width": 2, "height": 2})
    );
    tool(&mut document, "clearSelection", json!({}));
    tool(
        &mut document,
        "eraseRect",
        json!({"x": 0, "y": 0, "width": 4, "height": 3}),
    );

    paint_l(&mut document);
    let pixels = document.pixels().to_vec();
    tool(
        &mut document,
        "setSelection",
        json!({"x": 0, "y": 0, "width": 1, "height": 1}),
    );
    let wand = tool(
        &mut document,
        "selectConnected",
        json!({"x": 0, "y": 0, "connectivity": 4}),
    );
    assert_eq!(document.pixels(), pixels.as_slice());
    assert_eq!(document.active_layer(), 0);
    assert_eq!(wand["cells"], json!(6));
    assert_eq!(wand["index"], json!(1));
    assert_eq!(
        wand["selection"]["rects"],
        json!([
            {"x": 0, "y": 0, "width": 2, "height": 1},
            {"x": 0, "y": 1, "width": 1, "height": 2},
            {"x": 1, "y": 2, "width": 2, "height": 1}
        ])
    );
    let mask = document.selection().unwrap().clone();
    assert!(mask.contains(1, 0));
    assert!(mask.contains(0, 2));
    assert!(mask.contains(2, 2));
    assert!(!mask.contains(2, 0));
    assert!(!mask.contains(1, 1));
    assert!(!mask.contains(3, 2));
    let loaded = Document::load_project(&document.save_project()).unwrap();
    assert_eq!(loaded.selection().unwrap().rects(), mask.rects());

    tool(&mut document, "selectConnected", json!({"x": 2, "y": 2}));
    tool(&mut document, "undo", json!({}));
    assert_eq!(document.selection_rect(), Some(rect(0, 0, 1, 1)));
    tool(&mut document, "redo", json!({}));
    assert_eq!(document.selection().unwrap().cell_count(), 6);

    let replaced = tool(
        &mut document,
        "replaceIndex",
        json!({"fromIndex": 1, "toIndex": 6}),
    );
    assert_eq!(replaced["changed"], json!(6));
    assert_eq!(document.get(3, 0).unwrap(), 0);
    assert_eq!(document.get(1, 1).unwrap(), 0);
    assert_eq!(document.get(2, 2).unwrap(), 6);

    tool(
        &mut document,
        "fillRect",
        json!({"x": 0, "y": 0, "width": 4, "height": 3, "colorIndex": 5}),
    );
    for y in 0..3 {
        for x in 0..4 {
            let index = document.get(x, y).unwrap();
            if mask.contains(x, y) {
                assert_eq!(index, 5, "({x},{y})");
            } else {
                assert_eq!(index, 0, "({x},{y})");
            }
        }
    }
    tool(&mut document, "erase", json!({"x": 0, "y": 0}));
    assert_eq!(document.get(0, 0).unwrap(), 0);
    assert_eq!(document.get(1, 0).unwrap(), 5);
    tool(
        &mut document,
        "paint",
        json!({"x": 3, "y": 2, "colorIndex": 7}),
    );
    assert_eq!(document.get(3, 2).unwrap(), 0);
}

#[test]
fn connectivity_tolerance_and_a_foreign_index_follow_the_flood() {
    let mut diagonal = Document::new(2, 2).unwrap();
    tool(
        &mut diagonal,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 1}),
    );
    tool(
        &mut diagonal,
        "paint",
        json!({"x": 1, "y": 1, "colorIndex": 1}),
    );
    let four = tool(
        &mut diagonal,
        "selectConnected",
        json!({"x": 0, "y": 0, "connectivity": 4}),
    );
    assert_eq!(four["cells"], json!(1));
    let eight = tool(
        &mut diagonal,
        "selectConnected",
        json!({"x": 0, "y": 0, "connectivity": 8}),
    );
    assert_eq!(eight["cells"], json!(2));
    assert!(diagonal.selection().unwrap().as_rect().is_none());

    let mut split = Document::new(4, 1).unwrap();
    tool(
        &mut split,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 1}),
    );
    tool(
        &mut split,
        "paint",
        json!({"x": 3, "y": 0, "colorIndex": 1}),
    );
    let alone = tool(&mut split, "selectConnected", json!({"x": 0, "y": 0}));
    assert_eq!(alone["cells"], json!(1));
    assert!(!split.selection().unwrap().contains(3, 0));

    let mut near = Document::new(4, 1).unwrap();
    tool(
        &mut near,
        "setPaletteColor",
        json!({"index": 1, "r": 10, "g": 20, "b": 30}),
    );
    tool(
        &mut near,
        "setPaletteColor",
        json!({"index": 2, "r": 12, "g": 20, "b": 30}),
    );
    tool(
        &mut near,
        "setPaletteColor",
        json!({"index": 3, "r": 10, "g": 20, "b": 30}),
    );
    for (x, color) in [(0, 1), (1, 2), (2, 3), (3, 1)] {
        tool(
            &mut near,
            "paint",
            json!({"x": x, "y": 0, "colorIndex": color}),
        );
    }
    assert_eq!(
        tool(
            &mut near,
            "selectConnected",
            json!({"x": 0, "y": 0, "tolerance": 0})
        )["cells"],
        json!(1),
        "tolerance 0 is an index match, so the duplicate color stays out"
    );
    assert_eq!(
        tool(
            &mut near,
            "selectConnected",
            json!({"x": 0, "y": 0, "tolerance": 1})
        )["cells"],
        json!(1)
    );
    let joined = tool(
        &mut near,
        "selectConnected",
        json!({"x": 0, "y": 0, "tolerance": 2}),
    );
    assert_eq!(joined["cells"], json!(4));
    assert_eq!(joined["tolerance"], json!(2));
    assert_eq!(near.get(3, 0).unwrap(), 1);

    let mut alpha = Document::new(2, 1).unwrap();
    tool(
        &mut alpha,
        "setPaletteColor",
        json!({"index": 1, "r": 0, "g": 0, "b": 0}),
    );
    tool(
        &mut alpha,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 1}),
    );
    assert_eq!(
        tool(
            &mut alpha,
            "selectConnected",
            json!({"x": 0, "y": 0, "tolerance": 254})
        )["cells"],
        json!(1)
    );
    assert_eq!(
        tool(
            &mut alpha,
            "selectConnected",
            json!({"x": 0, "y": 0, "tolerance": 255})
        )["cells"],
        json!(2)
    );
}

#[test]
fn wand_samples_one_layer_and_frame_then_composes_with_mask_and_diff() {
    let mut document = Document::new(4, 1).unwrap();
    document.add_layer(Some("gear")).unwrap();
    document.set_active_layer(0).unwrap();
    document.set_layer_visible(1, false).unwrap();
    tool(
        &mut document,
        "fillRect",
        json!({"x": 0, "y": 0, "width": 4, "height": 1, "colorIndex": 1, "layer": 0}),
    );
    tool(
        &mut document,
        "paint",
        json!({"x": 1, "y": 0, "colorIndex": 4, "layer": 1}),
    );
    let gear = tool(
        &mut document,
        "selectConnected",
        json!({"x": 1, "y": 0, "layer": 1}),
    );
    assert_eq!(gear["cells"], json!(1));
    assert_eq!(gear["index"], json!(4));
    assert_eq!(gear["layer"], json!(1));
    assert_eq!(document.active_layer(), 0);
    assert!(!document.layer_visible(1).unwrap());
    assert_eq!(document.layer_pixels(0, 0).unwrap()[0], 1);

    tool(&mut document, "clearSelection", json!({}));
    for (x, color) in [(0, 1), (1, 1), (2, 1), (3, 2)] {
        tool(
            &mut document,
            "paint",
            json!({"x": x, "y": 0, "colorIndex": color, "layer": 0}),
        );
    }
    tool(&mut document, "duplicateFrame", json!({"index": 0}));
    tool(&mut document, "setActiveFrame", json!({"index": 1}));
    tool(
        &mut document,
        "paint",
        json!({"x": 1, "y": 0, "colorIndex": 3, "layer": 0}),
    );
    tool(
        &mut document,
        "paint",
        json!({"x": 3, "y": 0, "colorIndex": 4, "layer": 0}),
    );
    tool(&mut document, "setActiveFrame", json!({"index": 0}));
    let sampled = tool(
        &mut document,
        "selectConnected",
        json!({"x": 0, "y": 0, "frame": 1, "layer": 0}),
    );
    assert_eq!(sampled["index"], json!(1));
    assert_eq!(sampled["cells"], json!(1));
    assert_eq!(sampled["frame"], json!(1));
    assert_eq!(document.active_frame(), 0);
    assert_eq!(document.frame_pixels(0).unwrap(), &[1, 1, 1, 2]);
    tool(
        &mut document,
        "selectConnected",
        json!({"x": 0, "y": 0, "layer": 0}),
    );

    let diff = tool(
        &mut document,
        "diffFrames",
        json!({"frame": 0, "with": 1, "selection": true, "layer": 0}),
    );
    assert_eq!(diff["scope"], json!("selection"));
    assert_eq!(diff["changed"], json!(1));
    assert_eq!(diff["protected"], json!(1));
    assert_eq!(diff["diffRows"][0], json!(".!.#"));
    assert_eq!(diff["changes"].as_array().unwrap().len(), 1);
    assert_eq!(document.active_layer(), 0);

    let cut = tool(
        &mut document,
        "combineMask",
        json!({"op": "subtract", "x": 1, "y": 0, "width": 1, "height": 1}),
    );
    assert_eq!(cut["cells"], json!(2));
    tool(
        &mut document,
        "fill",
        json!({"x": 0, "y": 0, "colorIndex": 8, "connectivity": 4}),
    );
    assert_eq!(document.get(0, 0).unwrap(), 8);
    assert_eq!(document.get(1, 0).unwrap(), 1);
    assert_eq!(document.get(2, 0).unwrap(), 1);
    assert_eq!(document.get(3, 0).unwrap(), 2);
}

#[test]
fn a_refused_wand_leaves_the_mask_and_the_pixels() {
    let mut document = Document::new(4, 2).unwrap();
    tool(
        &mut document,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 1}),
    );
    tool(
        &mut document,
        "setSelection",
        json!({"x": 0, "y": 0, "width": 2, "height": 1}),
    );
    let pixels = document.pixels().to_vec();
    for params in [
        json!({"x": 9, "y": 0}),
        json!({"x": 0, "y": 0, "connectivity": 3}),
        json!({"x": 0, "y": 0, "tolerance": 256}),
        json!({"x": 0, "y": 0, "reportDirty": true}),
        json!({"x": 0, "y": 0, "layer": 3}),
        json!({"x": 0, "y": 0, "frame": 2}),
        json!({"x": 0, "y": 0, "lasso": true}),
    ] {
        let error = tool_err(&mut document, "selectConnected", params);
        assert!(
            error["code"] == json!("OutOfBounds")
                || error["code"] == json!("InvalidParams")
                || error["code"] == json!("LayerIndex")
                || error["code"] == json!("FrameIndex"),
            "{error}"
        );
        assert_eq!(document.selection_rect(), Some(rect(0, 0, 2, 1)));
        assert_eq!(document.pixels(), pixels.as_slice());
    }
    assert_eq!(
        tool_err(&mut document, "selectConnected", json!({"x": 9, "y": 0}))["code"],
        json!("OutOfBounds")
    );
    assert_eq!(
        tool_err(
            &mut document,
            "selectConnected",
            json!({"x": 0, "y": 0, "connectivity": "4"})
        )["code"],
        json!("InvalidParams")
    );
}
