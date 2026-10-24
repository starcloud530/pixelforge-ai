//! Index policy, quantize / remap, and PNG crush through the Tool API.

use pixelforge_kernel::{Document, Palette, Rgba, TOOL_API_VERSION};
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

fn paint(document: &mut Document, x: u16, y: u16, color: u8) {
    tool(
        document,
        "paint",
        json!({"x": x, "y": y, "colorIndex": color}),
    );
}

#[test]
fn out_of_range_indices_reject_unless_clamp_is_set() {
    let mut document = Document::new(4, 2).unwrap();
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["indexPolicy"],
        json!("reject")
    );
    assert_eq!(
        tool(&mut document, "scene", json!({}))["indexPolicy"],
        json!("reject")
    );
    let refused = tool_err(
        &mut document,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 40}),
    );
    assert_eq!(refused["code"], json!("PaletteIndex"));
    assert!(!document.can_undo());

    let same = tool(&mut document, "setIndexPolicy", json!({"mode": "reject"}));
    assert_eq!(same["mode"], json!("reject"));
    assert!(!document.can_undo());

    let bad = tool_err(&mut document, "setIndexPolicy", json!({"mode": "wrap"}));
    assert_eq!(bad["code"], json!("InvalidParams"));

    tool(&mut document, "setIndexPolicy", json!({"mode": "clamp"}));
    let painted = tool(
        &mut document,
        "paint",
        json!({"x": 1, "y": 0, "colorIndex": 40, "reportDirty": true}),
    );
    assert_eq!(painted["indexPolicy"], json!("clamp"));
    assert_eq!(painted["requestedIndex"], json!(40));
    assert_eq!(painted["colorIndex"], json!(8));
    assert_eq!(painted["changed"], json!(1));
    assert_eq!(document.get(1, 0).unwrap(), 8);

    let filled = tool(
        &mut document,
        "fillRect",
        json!({"x": 0, "y": 1, "width": 2, "height": 1, "colorIndex": 90}),
    );
    assert_eq!(filled["colorIndex"], json!(8));
    assert_eq!(document.get(0, 1).unwrap(), 8);

    let transparent = tool_err(
        &mut document,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 0}),
    );
    assert_eq!(transparent["code"], json!("PaletteIndex"));

    let inside = tool(
        &mut document,
        "paint",
        json!({"x": 2, "y": 0, "colorIndex": 3}),
    );
    assert!(inside.get("requestedIndex").is_none());
    assert!(inside.as_object().unwrap().is_empty());

    paint(&mut document, 3, 0, 8);
    let already = tool(
        &mut document,
        "paint",
        json!({"x": 3, "y": 0, "colorIndex": 200}),
    );
    assert_eq!(already["requestedIndex"], json!(200));
    assert_eq!(already["colorIndex"], json!(8));
    document.undo().unwrap();
    assert_eq!(document.get(3, 0).unwrap(), 0);

    tool(&mut document, "setPaletteLocked", json!({"locked": true}));
    let locked_paint = tool(
        &mut document,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 50}),
    );
    assert_eq!(locked_paint["colorIndex"], json!(8));
    assert_eq!(
        tool_err(
            &mut document,
            "setPaletteColor",
            json!({"index": 1, "r": 1, "g": 2, "b": 3})
        )["code"],
        json!("PaletteLocked")
    );

    let saved = tool(&mut document, "saveProject", json!({}))["json"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(saved.contains("\"indexPolicy\": \"clamp\""));
    let mut loaded = Document::new(1, 1).unwrap();
    assert_eq!(
        tool(&mut loaded, "loadProject", json!({"json": saved}))["indexPolicy"],
        json!("clamp")
    );
}

#[test]
fn clamp_does_not_change_import_replace_orphan_clearing() {
    let mut document = Document::new(2, 1).unwrap();
    tool(
        &mut document,
        "importPalette",
        json!({"text": "010101\n020202\n", "mode": "replace"}),
    );
    paint(&mut document, 0, 0, 2);
    tool(&mut document, "setIndexPolicy", json!({"mode": "clamp"}));
    tool(
        &mut document,
        "importPalette",
        json!({"text": "ffffff\n", "mode": "replace"}),
    );
    assert_eq!(document.get(0, 0).unwrap(), 0);
    assert_eq!(document.palette_colors().len(), 2);
}

#[test]
fn quantize_and_remap_report_misses_merges_and_changed_pixels() {
    let mut document = Document::new(4, 2).unwrap();
    tool(
        &mut document,
        "importPalette",
        json!({"text": "000000\n010000\nff0000\n", "mode": "replace"}),
    );
    paint(&mut document, 0, 0, 1);
    paint(&mut document, 1, 0, 1);
    paint(&mut document, 0, 1, 2);
    paint(&mut document, 1, 1, 2);
    paint(&mut document, 2, 0, 3);

    let neither = tool_err(&mut document, "remapPalette", json!({}));
    assert_eq!(neither["code"], json!("InvalidParams"));
    let both = tool_err(
        &mut document,
        "remapPalette",
        json!({"text": "ff0000\n", "maxColors": 1}),
    );
    assert_eq!(both["code"], json!("InvalidParams"));
    let zero = tool_err(&mut document, "remapPalette", json!({"maxColors": 0}));
    assert_eq!(zero["code"], json!("InvalidParams"));

    let quantized = tool(&mut document, "remapPalette", json!({"maxColors": 2}));
    assert_eq!(quantized["paletteLength"], json!(3));
    assert_eq!(quantized["pixelsChanged"], json!(3));
    assert_eq!(quantized["dropped"].as_array().unwrap().len(), 0);
    assert_eq!(quantized["misses"].as_array().unwrap().len(), 1);
    assert_eq!(quantized["misses"][0]["fromIndex"], json!(2));
    assert_eq!(quantized["misses"][0]["intoIndex"], json!(1));
    assert_eq!(quantized["misses"][0]["pixels"], json!(2));
    assert_eq!(quantized["misses"][0]["distance"], json!(1));
    assert_eq!(quantized["merges"].as_array().unwrap().len(), 2);
    assert_eq!(quantized["merges"][1]["exact"], json!(true));
    assert_eq!(document.get(0, 1).unwrap(), 1);
    assert_eq!(document.get(2, 0).unwrap(), 2);
    assert_eq!(document.palette_colors()[1], Rgba::rgb(0, 0, 0));
    assert_eq!(document.palette_colors()[2], Rgba::rgb(255, 0, 0));

    document.undo().unwrap();
    assert_eq!(document.palette_colors().len(), 4);
    assert_eq!(document.get(0, 1).unwrap(), 2);
    assert_eq!(document.get(2, 0).unwrap(), 3);

    let remapped = tool(
        &mut document,
        "remapPalette",
        json!({"text": "0000ff\nff0000\n"}),
    );
    assert_eq!(remapped["paletteLength"], json!(3));
    assert_eq!(remapped["pixelsChanged"], json!(1));
    assert_eq!(remapped["misses"].as_array().unwrap().len(), 2);
    assert_eq!(remapped["misses"][0]["fromIndex"], json!(1));
    assert_eq!(remapped["misses"][0]["intoIndex"], json!(1));
    assert_eq!(remapped["misses"][0]["r"], json!(0));
    assert_eq!(remapped["misses"][0]["into"]["b"], json!(255));
    assert_eq!(remapped["merges"].as_array().unwrap().len(), 3);
    assert_eq!(remapped["merges"][2]["exact"], json!(true));
    assert_eq!(remapped["merges"][2]["fromIndex"], json!(3));
    assert_eq!(document.palette_colors()[1], Rgba::rgb(0, 0, 255));
    assert_eq!(document.palette_colors()[2], Rgba::rgb(255, 0, 0));
    assert_eq!(document.get(2, 0).unwrap(), 2);
    assert_eq!(document.get(0, 0).unwrap(), 1);

    document.undo().unwrap();
    tool(&mut document, "setPaletteLocked", json!({"locked": true}));
    let locked = tool_err(&mut document, "remapPalette", json!({"maxColors": 1}));
    assert_eq!(locked["code"], json!("PaletteLocked"));
    assert_eq!(document.get(2, 0).unwrap(), 3);
    assert_eq!(document.palette_colors().len(), 4);
}

#[test]
fn blank_quantize_drops_unused_swatches_without_touching_pixels() {
    let mut document = Document::new(2, 2).unwrap();
    assert!(!document.can_undo());
    let same = tool(&mut document, "remapPalette", json!({"maxColors": 8}));
    assert_eq!(same["pixelsChanged"], json!(0));
    assert!(same["dropped"].as_array().unwrap().is_empty());
    assert!(!document.can_undo());

    let reduced = tool(&mut document, "remapPalette", json!({"maxColors": 2}));
    assert_eq!(reduced["paletteLength"], json!(3));
    assert_eq!(reduced["pixelsChanged"], json!(0));
    assert_eq!(reduced["dropped"].as_array().unwrap().len(), 6);
    assert_eq!(reduced["dropped"][0]["index"], json!(3));
    assert!(reduced["misses"].as_array().unwrap().is_empty());
    document.undo().unwrap();
    assert_eq!(document.palette_colors().len(), 9);
}

#[test]
fn apply_png_reports_the_near_color_and_undoes() {
    let mut document = Document::new(2, 1).unwrap();
    tool(
        &mut document,
        "importPalette",
        json!({"text": "ff0000\n0000ff\n", "mode": "replace"}),
    );
    tool(
        &mut document,
        "setPaletteColor",
        json!({"index": 1, "r": 200, "g": 10, "b": 10}),
    );
    paint(&mut document, 0, 0, 2);
    paint(&mut document, 1, 0, 1);
    let png = tool(&mut document, "exportPngSequence", json!({}))["frames"][0]
        .as_str()
        .unwrap()
        .to_string();
    tool(
        &mut document,
        "setPaletteColor",
        json!({"index": 1, "r": 255, "g": 0, "b": 0}),
    );
    paint(&mut document, 1, 0, 2);

    let bad = tool_err(&mut document, "applyPng", json!({"png": "!!!!"}));
    assert_eq!(bad["code"], json!("InvalidParams"));
    let garbage = tool_err(&mut document, "applyPng", json!({"png": "bm90IGEgcG5n"}));
    assert_eq!(garbage["code"], json!("PngFile"));

    let crushed = tool(&mut document, "applyPng", json!({"png": png, "frame": 0}));
    assert_eq!(crushed["frame"], json!(0));
    assert_eq!(crushed["layer"], json!(0));
    assert_eq!(crushed["width"], json!(2));
    assert_eq!(crushed["height"], json!(1));
    assert_eq!(crushed["pixelsChanged"], json!(1));
    assert_eq!(crushed["dropped"].as_array().unwrap().len(), 0);
    assert_eq!(crushed["misses"].as_array().unwrap().len(), 1);
    assert_eq!(crushed["misses"][0]["r"], json!(200));
    assert_eq!(crushed["misses"][0]["g"], json!(10));
    assert_eq!(crushed["misses"][0]["b"], json!(10));
    assert_eq!(crushed["misses"][0]["pixels"], json!(1));
    assert_eq!(crushed["misses"][0]["intoIndex"], json!(1));
    assert_eq!(crushed["misses"][0]["distance"], json!(3225));
    assert_eq!(crushed["merges"][0]["exact"], json!(false));
    assert_eq!(crushed["merges"][0]["into"]["r"], json!(255));
    assert!(crushed["merges"][0].get("fromIndex").is_none());
    assert_eq!(document.get(0, 0).unwrap(), 2);
    assert_eq!(document.get(1, 0).unwrap(), 1);
    assert_eq!(document.palette_colors()[1], Rgba::rgb(255, 0, 0));
    assert_eq!(document.palette_colors()[2], Rgba::rgb(0, 0, 255));

    document.undo().unwrap();
    assert_eq!(document.get(1, 0).unwrap(), 2);

    let again = tool(&mut document, "applyPng", json!({"png": png}));
    assert_eq!(again["pixelsChanged"], json!(1));
    document.undo().unwrap();

    tool(&mut document, "setPaletteLocked", json!({"locked": true}));
    let locked = tool(&mut document, "applyPng", json!({"png": png}));
    assert_eq!(locked["pixelsChanged"], json!(1));
    assert_eq!(document.palette_colors()[1], Rgba::rgb(255, 0, 0));

    let mut other = Document::new(3, 1).unwrap();
    let mismatch = tool_err(&mut other, "applyPng", json!({"png": png}));
    assert_eq!(mismatch["code"], json!("PngSize"));
    assert!(mismatch["message"]
        .as_str()
        .unwrap()
        .contains("png is 2x1, canvas is 3x1"));

    let mut empty =
        Document::with_palette(2, 1, Palette::new(vec![Rgba::TRANSPARENT]).unwrap()).unwrap();
    let no_color = tool_err(&mut empty, "applyPng", json!({"png": png}));
    assert_eq!(no_color["code"], json!("NoPaintColor"));
    assert_eq!(empty.pixels(), &[0, 0]);
}
