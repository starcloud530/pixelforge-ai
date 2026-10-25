//! Drives a document only through [`pixelforge_kernel::Document::call_tool`].

use pixelforge_kernel::{Document, TOOL_API_VERSION, TOOL_NAMES};
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
    assert_eq!(response["apiVersion"], json!(TOOL_API_VERSION));
    assert_eq!(response["id"], json!(name));
    response["result"].clone()
}

fn tool_err(document: &mut Document, name: &str, params: Value) -> Value {
    let response = call(document, json!({ "tool": name, "params": params }));
    assert_eq!(response["ok"], json!(false), "{name}: {response}");
    assert_eq!(response["apiVersion"], json!(TOOL_API_VERSION));
    assert!(response.get("id").is_none(), "{response}");
    response["error"].clone()
}

fn doc() -> Document {
    Document::new(16, 16).expect("16 is a valid canvas edge")
}

#[test]
fn paint_frame_export_undo_and_project_round_trip() {
    let mut document = doc();
    let before = tool(&mut document, "inspect", json!({}));
    assert_eq!(before["width"], json!(16));
    assert_eq!(before["height"], json!(16));
    assert_eq!(before["paletteLocked"], json!(false));
    assert_eq!(before["frameCount"], json!(1));
    assert_eq!(before["activeFrame"], json!(0));
    assert_eq!(before["delayMs"], json!(100));
    assert_eq!(before["frameDelays"], json!([100]));
    assert_eq!(before["canUndo"], json!(false));
    assert_eq!(before["pixels"].as_array().unwrap().len(), 16 * 16);
    assert_eq!(
        before["palette"][0],
        json!({"r": 0, "g": 0, "b": 0, "a": 0})
    );

    tool(&mut document, "beginGesture", json!({}));
    tool(
        &mut document,
        "paint",
        json!({"x": 1, "y": 2, "colorIndex": 3}),
    );
    tool(
        &mut document,
        "paint",
        json!({"x": 2, "y": 2, "colorIndex": 4}),
    );
    tool(&mut document, "endGesture", json!({}));

    let painted = tool(&mut document, "inspect", json!({}));
    assert_eq!(painted["pixels"][2 * 16 + 1], json!(3));
    assert_eq!(painted["pixels"][2 * 16 + 2], json!(4));
    assert_eq!(painted["canUndo"], json!(true));
    assert_eq!(painted["gestureOpen"], json!(false));

    assert_eq!(
        tool(&mut document, "addFrame", json!({}))["index"],
        json!(1)
    );
    tool(&mut document, "setActiveFrame", json!({"index": 1}));
    tool(
        &mut document,
        "setFrameDelay",
        json!({"index": 1, "delayMs": 80}),
    );
    tool(
        &mut document,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 1}),
    );
    let copied = tool(&mut document, "duplicateFrame", json!({"index": 1}));
    assert_eq!(copied["index"], json!(2));

    let active = tool(&mut document, "inspect", json!({}));
    assert_eq!(active["frameCount"], json!(3));
    assert_eq!(active["activeFrame"], json!(1));
    assert_eq!(active["delayMs"], json!(80));
    assert_eq!(active["frameDelays"], json!([100, 80, 80]));
    assert_eq!(active["pixels"][0], json!(1));

    let sequence = tool(&mut document, "exportPngSequence", json!({}));
    let frames = sequence["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 3);
    let first = frames[0].as_str().unwrap();
    let second = frames[1].as_str().unwrap();
    assert!(first.starts_with("iVBORw0KGgoAAA"), "{first}");
    assert!(second.starts_with("iVBORw0KGgoAAA"));
    assert_ne!(first, second);
    assert_eq!(second, frames[2].as_str().unwrap());

    let sheet = tool(&mut document, "exportSpriteSheet", json!({}));
    assert_eq!(sheet["columns"], json!(2));
    assert_eq!(sheet["rows"], json!(2));
    assert_eq!(sheet["sheetWidth"], json!(32));
    assert_eq!(sheet["sheetHeight"], json!(32));
    let png = sheet["png"].as_str().unwrap();
    assert!(png.starts_with("iVBORw0KGgoAAA"));
    assert!(png.len() > first.len());
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["canUndo"],
        json!(true),
        "export does not push an undo entry"
    );

    let undone = tool(&mut document, "undo", json!({}));
    assert_eq!(undone["canRedo"], json!(true));
    let after_undo = tool(&mut document, "inspect", json!({}));
    assert_eq!(after_undo["frameCount"], json!(2));
    assert_eq!(after_undo["pixels"][0], json!(1));

    let redone = tool(&mut document, "redo", json!({}));
    assert_eq!(redone["canUndo"], json!(true));
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["frameCount"],
        json!(3)
    );

    let saved = tool(&mut document, "saveProject", json!({}));
    let project = saved["json"].as_str().unwrap().to_string();
    assert!(project.contains("\"format\": \"pixelforge-project\""));
    tool(
        &mut document,
        "paint",
        json!({"x": 4, "y": 4, "colorIndex": 2}),
    );
    let loaded = tool(&mut document, "loadProject", json!({ "json": project }));
    assert_eq!(loaded["frameCount"], json!(3));
    assert_eq!(loaded["activeFrame"], json!(1));
    assert_eq!(loaded["frameDelays"], json!([100, 80, 80]));
    assert_eq!(loaded["pixels"][0], json!(1));
    assert_eq!(loaded["pixels"][4 * 16 + 4], json!(0));
    assert_eq!(loaded["canUndo"], json!(false));
    assert_eq!(loaded["canRedo"], json!(false));

    tool(&mut document, "setActiveFrame", json!({"index": 0}));
    let frame_zero = tool(&mut document, "inspect", json!({}));
    assert_eq!(frame_zero["pixels"][2 * 16 + 1], json!(3));
    assert_eq!(frame_zero["pixels"][2 * 16 + 2], json!(4));

    let cleared = tool(&mut document, "undo", json!({}));
    assert_eq!(cleared["canUndo"], json!(false));
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["activeFrame"],
        json!(1)
    );
}

#[test]
fn rejects_bad_indices_and_a_locked_palette_without_mutating() {
    let mut document = doc();
    let palette = tool(&mut document, "inspect", json!({}))["palette"].clone();

    let out_of_bounds = tool_err(
        &mut document,
        "paint",
        json!({"x": 16, "y": 0, "colorIndex": 1}),
    );
    assert_eq!(out_of_bounds["code"], json!("OutOfBounds"));
    let transparent = tool_err(
        &mut document,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 0}),
    );
    assert_eq!(transparent["code"], json!("PaletteIndex"));
    let erase = tool_err(&mut document, "erase", json!({"x": 0, "y": 16}));
    assert_eq!(erase["code"], json!("OutOfBounds"));
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["canUndo"],
        json!(false)
    );

    tool(
        &mut document,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 1}),
    );
    let still_there = tool_err(&mut document, "erase", json!({"x": 99, "y": 0}));
    assert_eq!(still_there["code"], json!("OutOfBounds"));
    tool(&mut document, "undo", json!({}));
    let cleared = tool(&mut document, "inspect", json!({}));
    assert_eq!(cleared["pixels"][0], json!(0));
    assert_eq!(cleared["canUndo"], json!(false));
    let empty_undo = tool_err(&mut document, "undo", json!({}));
    assert_eq!(empty_undo["code"], json!("NothingToUndo"));

    tool(&mut document, "setPaletteLocked", json!({"locked": true}));
    let locked = tool(&mut document, "inspect", json!({}));
    assert_eq!(locked["paletteLocked"], json!(true));
    let recolor = tool_err(
        &mut document,
        "setPaletteColor",
        json!({"index": 1, "r": 9, "g": 8, "b": 7}),
    );
    assert_eq!(recolor["code"], json!("PaletteLocked"));
    assert_eq!(recolor["message"], json!("palette is locked"));
    let append = tool_err(
        &mut document,
        "addPaletteColor",
        json!({"r": 1, "g": 2, "b": 3}),
    );
    assert_eq!(append["code"], json!("PaletteLocked"));
    let import = tool_err(
        &mut document,
        "importPalette",
        json!({"text": "ff0000\n", "mode": "replace"}),
    );
    assert_eq!(import["code"], json!("PaletteLocked"));
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["palette"],
        palette
    );
    tool(
        &mut document,
        "paint",
        json!({"x": 1, "y": 0, "colorIndex": 2}),
    );
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["pixels"][1],
        json!(2)
    );

    tool(&mut document, "setPaletteLocked", json!({"locked": false}));
    let added = tool(
        &mut document,
        "addPaletteColor",
        json!({"r": 1, "g": 2, "b": 3}),
    );
    assert_eq!(added["index"], json!(9));
    tool(
        &mut document,
        "importPalette",
        json!({"text": "#112233 ink\n112233\n", "mode": "merge"}),
    );
    let merged = tool(&mut document, "inspect", json!({}));
    assert_eq!(merged["palette"].as_array().unwrap().len(), 11);
    assert_eq!(
        merged["palette"][10],
        json!({"r": 17, "g": 34, "b": 51, "a": 255})
    );
    let mode = tool_err(
        &mut document,
        "importPalette",
        json!({"text": "ff0000\n", "mode": "nope"}),
    );
    assert_eq!(mode["code"], json!("PaletteImportMode"));
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["palette"]
            .as_array()
            .unwrap()
            .len(),
        11
    );

    tool(
        &mut document,
        "paint",
        json!({"x": 3, "y": 0, "colorIndex": 5}),
    );
    tool(
        &mut document,
        "importPalette",
        json!({"text": "ff0000\n", "mode": "replace"}),
    );
    let replaced = tool(&mut document, "inspect", json!({}));
    assert_eq!(replaced["palette"].as_array().unwrap().len(), 2);
    assert_eq!(replaced["pixels"][3], json!(0));
    tool(&mut document, "undo", json!({}));
    let restored = tool(&mut document, "inspect", json!({}));
    assert_eq!(restored["pixels"][3], json!(5));
    assert!(restored["palette"].as_array().unwrap().len() > 2);

    let last = tool_err(&mut document, "deleteFrame", json!({"index": 0}));
    assert_eq!(last["code"], json!("LastFrame"));
    assert_eq!(
        tool(&mut document, "addFrame", json!({}))["index"],
        json!(1)
    );
    let missing_frame = tool_err(&mut document, "deleteFrame", json!({"index": 4}));
    assert_eq!(missing_frame["code"], json!("FrameIndex"));
    tool(&mut document, "deleteFrame", json!({"index": 1}));
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["frameCount"],
        json!(1)
    );
    let delay = tool_err(
        &mut document,
        "setFrameDelay",
        json!({"index": 0, "delayMs": 0}),
    );
    assert_eq!(delay["code"], json!("FrameDelay"));
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["delayMs"],
        json!(100)
    );
}

#[test]
fn one_gesture_undoes_both_pixels() {
    let mut document = doc();
    tool(&mut document, "beginGesture", json!({}));
    tool(
        &mut document,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 1}),
    );
    tool(
        &mut document,
        "paint",
        json!({"x": 1, "y": 0, "colorIndex": 2}),
    );
    let open = tool_err(&mut document, "beginGesture", json!({}));
    assert_eq!(open["code"], json!("OpenGesture"));
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["gestureOpen"],
        json!(true)
    );
    tool(&mut document, "endGesture", json!({}));
    tool(&mut document, "undo", json!({}));
    let cleared = tool(&mut document, "inspect", json!({}));
    assert_eq!(cleared["pixels"][0], json!(0));
    assert_eq!(cleared["pixels"][1], json!(0));
    assert_eq!(cleared["canUndo"], json!(false));
    assert_eq!(cleared["canRedo"], json!(true));
}

#[test]
fn rejects_unknown_tools_and_bad_requests_without_mutating() {
    let mut document = doc();
    let response = call(
        &mut document,
        json!({"id": "step-1", "tool": "paintPixels"}),
    );
    assert_eq!(response["ok"], json!(false));
    assert_eq!(response["id"], json!("step-1"));
    assert_eq!(response["error"]["code"], json!("UnknownTool"));
    assert_eq!(
        response["error"]["message"],
        json!("unknown tool \"paintPixels\"")
    );
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["canUndo"],
        json!(false)
    );

    let missing = tool_err(&mut document, "paint", json!({"x": 0, "y": 0}));
    assert_eq!(missing["code"], json!("InvalidParams"));
    assert_eq!(missing["message"], json!("paint: missing colorIndex"));
    let extra = tool_err(
        &mut document,
        "erase",
        json!({"x": 0, "y": 0, "colorIndex": 1}),
    );
    assert_eq!(extra["message"], json!("erase: unknown field colorIndex"));
    let typed = tool_err(&mut document, "setPaletteLocked", json!({"locked": 1}));
    assert_eq!(
        typed["message"],
        json!("setPaletteLocked: locked must be a boolean")
    );
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["pixels"][0],
        json!(0)
    );

    let broken = call(&mut document, json!("paint"));
    assert_eq!(broken["error"]["code"], json!("InvalidRequest"));
    assert_eq!(
        broken["error"]["message"],
        json!("request must be a JSON object")
    );
    let junk = document.call_tool("not json");
    let junk: Value = serde_json::from_str(&junk).unwrap();
    assert_eq!(junk["error"]["message"], json!("request is not JSON"));

    let version = call(
        &mut document,
        json!({"apiVersion": 2, "id": 7, "tool": "inspect"}),
    );
    assert_eq!(version["id"], json!(7));
    assert_eq!(version["error"]["code"], json!("UnsupportedVersion"));
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["frameCount"],
        json!(1)
    );

    let project = tool_err(&mut document, "loadProject", json!({"json": "{"}));
    assert_eq!(project["code"], json!("ProjectFile"));
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["width"],
        json!(16)
    );
}

#[test]
fn fill_rect_is_one_undo_step_and_rejects_a_partial_rectangle() {
    let mut document = doc();
    tool(
        &mut document,
        "fillRect",
        json!({"x": 1, "y": 2, "width": 3, "height": 2, "colorIndex": 4}),
    );
    let painted = tool(&mut document, "inspect", json!({}));
    for y in 2..4 {
        for x in 1..4 {
            assert_eq!(painted["pixels"][y * 16 + x], json!(4));
        }
    }
    assert_eq!(painted["pixels"][2 * 16], json!(0));
    assert_eq!(painted["pixels"][2 * 16 + 4], json!(0));
    assert_eq!(painted["canUndo"], json!(true));

    tool(
        &mut document,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 1}),
    );
    tool(&mut document, "undo", json!({}));
    let kept = tool(&mut document, "inspect", json!({}));
    assert_eq!(kept["pixels"][0], json!(0));
    assert_eq!(kept["pixels"][2 * 16 + 1], json!(4));
    tool(&mut document, "undo", json!({}));
    let cleared = tool(&mut document, "inspect", json!({}));
    assert!(cleared["pixels"]
        .as_array()
        .unwrap()
        .iter()
        .all(|index| index == &json!(0)));
    assert_eq!(cleared["canUndo"], json!(false));
    assert_eq!(cleared["canRedo"], json!(true));

    tool(
        &mut document,
        "fillRect",
        json!({"x": 0, "y": 0, "width": 2, "height": 1, "colorIndex": 2}),
    );
    tool(
        &mut document,
        "fillRect",
        json!({"x": 0, "y": 0, "width": 2, "height": 1, "colorIndex": 2}),
    );
    tool(&mut document, "undo", json!({}));
    let once = tool(&mut document, "inspect", json!({}));
    assert_eq!(once["pixels"][0], json!(0));
    assert_eq!(once["canUndo"], json!(false));

    tool(&mut document, "beginGesture", json!({}));
    tool(
        &mut document,
        "fillRect",
        json!({"x": 0, "y": 1, "width": 2, "height": 1, "colorIndex": 3}),
    );
    tool(
        &mut document,
        "paint",
        json!({"x": 4, "y": 1, "colorIndex": 5}),
    );
    tool(&mut document, "endGesture", json!({}));
    tool(&mut document, "undo", json!({}));
    let gestured = tool(&mut document, "inspect", json!({}));
    assert_eq!(gestured["pixels"][16], json!(0));
    assert_eq!(gestured["pixels"][16 + 4], json!(0));
    assert_eq!(gestured["canUndo"], json!(false));

    let err = tool_err(
        &mut document,
        "fillRect",
        json!({"x": 14, "y": 0, "width": 4, "height": 1, "colorIndex": 1}),
    );
    assert_eq!(err["code"], json!("OutOfBounds"));
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["pixels"][14],
        json!(0)
    );
    let zero = tool_err(
        &mut document,
        "fillRect",
        json!({"x": 0, "y": 0, "width": 0, "height": 2, "colorIndex": 1}),
    );
    assert_eq!(zero["code"], json!("InvalidParams"));
    assert_eq!(
        zero["message"],
        json!("fillRect: width must be an integer from 1 to 65535")
    );
    let extra = tool_err(
        &mut document,
        "fillRect",
        json!({"x": 0, "y": 0, "width": 1, "height": 1, "colorIndex": 1, "rgb": 1}),
    );
    assert_eq!(extra["message"], json!("fillRect: unknown field rgb"));
    let index = tool_err(
        &mut document,
        "fillRect",
        json!({"x": 0, "y": 0, "width": 1, "height": 1, "colorIndex": 0}),
    );
    assert_eq!(index["code"], json!("PaletteIndex"));

    tool(&mut document, "setPaletteLocked", json!({"locked": true}));
    tool(
        &mut document,
        "fillRect",
        json!({"x": 2, "y": 2, "width": 2, "height": 1, "colorIndex": 6}),
    );
    let locked = tool(&mut document, "inspect", json!({}));
    assert_eq!(locked["paletteLocked"], json!(true));
    assert_eq!(locked["pixels"][2 * 16 + 2], json!(6));
    assert_eq!(locked["pixels"][2 * 16 + 3], json!(6));
}

#[test]
fn erase_copy_and_move_rects_match_fill_rect_rules() {
    let mut document = doc();
    tool(
        &mut document,
        "fillRect",
        json!({"x": 1, "y": 0, "width": 3, "height": 1, "colorIndex": 4}),
    );
    tool(
        &mut document,
        "eraseRect",
        json!({"x": 1, "y": 0, "width": 2, "height": 1}),
    );
    let erased = tool(&mut document, "inspect", json!({}));
    assert_eq!(erased["pixels"][1], json!(0));
    assert_eq!(erased["pixels"][2], json!(0));
    assert_eq!(erased["pixels"][3], json!(4));
    tool(&mut document, "undo", json!({}));
    let restored = tool(&mut document, "inspect", json!({}));
    assert_eq!(restored["pixels"][1], json!(4));
    assert_eq!(restored["pixels"][3], json!(4));
    assert_eq!(restored["canUndo"], json!(true));

    let overflow = tool_err(
        &mut document,
        "eraseRect",
        json!({"x": 14, "y": 0, "width": 4, "height": 1}),
    );
    assert_eq!(overflow["code"], json!("OutOfBounds"));
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["pixels"][14],
        json!(0)
    );
    let zero = tool_err(
        &mut document,
        "eraseRect",
        json!({"x": 0, "y": 0, "width": 0, "height": 1}),
    );
    assert_eq!(zero["code"], json!("InvalidParams"));

    tool(&mut document, "beginGesture", json!({}));
    tool(
        &mut document,
        "eraseRect",
        json!({"x": 3, "y": 0, "width": 1, "height": 1}),
    );
    tool(
        &mut document,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 2}),
    );
    tool(&mut document, "endGesture", json!({}));
    tool(&mut document, "undo", json!({}));
    let gestured = tool(&mut document, "inspect", json!({}));
    assert_eq!(gestured["pixels"][3], json!(4));
    assert_eq!(gestured["pixels"][0], json!(0));

    tool(
        &mut document,
        "copyRect",
        json!({"srcX": 1, "srcY": 0, "width": 2, "height": 1, "dstX": 5, "dstY": 0}),
    );
    let copied = tool(&mut document, "inspect", json!({}));
    assert_eq!(copied["pixels"][5], json!(4));
    assert_eq!(copied["pixels"][6], json!(4));
    assert_eq!(copied["pixels"][1], json!(4));
    tool(&mut document, "undo", json!({}));
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["pixels"][5],
        json!(0)
    );

    tool(
        &mut document,
        "moveRect",
        json!({"srcX": 1, "srcY": 0, "width": 3, "height": 1, "dstX": 2, "dstY": 0}),
    );
    let moved = tool(&mut document, "inspect", json!({}));
    assert_eq!(moved["pixels"][1], json!(0));
    assert_eq!(moved["pixels"][2], json!(4));
    assert_eq!(moved["pixels"][3], json!(4));
    assert_eq!(moved["pixels"][4], json!(4));
    tool(&mut document, "undo", json!({}));
    let back = tool(&mut document, "inspect", json!({}));
    assert_eq!(back["pixels"][1], json!(4));
    assert_eq!(back["pixels"][4], json!(0));

    let partial = tool_err(
        &mut document,
        "copyRect",
        json!({"srcX": 0, "srcY": 0, "width": 2, "height": 1, "dstX": 15, "dstY": 0}),
    );
    assert_eq!(partial["code"], json!("OutOfBounds"));
    let moved_off = tool_err(
        &mut document,
        "moveRect",
        json!({"srcX": 14, "srcY": 0, "width": 4, "height": 1, "dstX": 0, "dstY": 0}),
    );
    assert_eq!(moved_off["code"], json!("OutOfBounds"));
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["pixels"][1],
        json!(4)
    );

    tool(&mut document, "setPaletteLocked", json!({"locked": true}));
    tool(
        &mut document,
        "eraseRect",
        json!({"x": 1, "y": 0, "width": 1, "height": 1}),
    );
    tool(
        &mut document,
        "copyRect",
        json!({"srcX": 2, "srcY": 0, "width": 1, "height": 1, "dstX": 1, "dstY": 0}),
    );
    let locked = tool(&mut document, "inspect", json!({}));
    assert_eq!(locked["paletteLocked"], json!(true));
    assert_eq!(locked["pixels"][1], json!(4));
}

#[test]
fn export_scale_is_nearest_neighbor_and_defaults_to_one() {
    let mut document = Document::new(4, 2).unwrap();
    tool(
        &mut document,
        "paint",
        json!({"x": 1, "y": 0, "colorIndex": 3}),
    );
    tool(&mut document, "addFrame", json!({}));
    let plain = tool(&mut document, "exportPngSequence", json!({}));
    let same = tool(&mut document, "exportPngSequence", json!({"scale": 1}));
    assert_eq!(plain["frames"], same["frames"]);
    assert_eq!(plain["scale"], json!(1));
    assert_eq!(plain["width"], json!(4));
    assert_eq!(plain["height"], json!(2));
    let scaled = tool(&mut document, "exportPngSequence", json!({"scale": 2}));
    assert_eq!(scaled["scale"], json!(2));
    assert_eq!(scaled["width"], json!(8));
    assert_eq!(scaled["height"], json!(4));
    assert_ne!(scaled["frames"][0], plain["frames"][0]);

    let sheet = tool(&mut document, "exportSpriteSheet", json!({"scale": 2}));
    assert_eq!(sheet["columns"], json!(2));
    assert_eq!(sheet["rows"], json!(1));
    assert_eq!(sheet["sheetWidth"], json!(16));
    assert_eq!(sheet["sheetHeight"], json!(4));
    assert_eq!(sheet["scale"], json!(2));
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["canUndo"],
        json!(true)
    );

    let zero = tool_err(&mut document, "exportPngSequence", json!({"scale": 0}));
    assert_eq!(zero["code"], json!("InvalidParams"));
    assert_eq!(
        zero["message"],
        json!("exportPngSequence: scale must be an integer from 1 to 4294967295")
    );
    let extra = tool_err(
        &mut document,
        "exportSpriteSheet",
        json!({"scale": 1, "filter": "linear"}),
    );
    assert_eq!(
        extra["message"],
        json!("exportSpriteSheet: unknown field filter")
    );
}

#[test]
fn every_catalog_name_is_a_known_tool() {
    assert_eq!(TOOL_NAMES.len(), 63);
    for name in TOOL_NAMES {
        let mut document = doc();
        let response = call(&mut document, json!({ "tool": name }));
        let code = response["error"]["code"].as_str();
        assert_ne!(code, Some("UnknownTool"), "{name}: {response}");
        assert_ne!(code, Some("InvalidRequest"), "{name}: {response}");
    }
}

#[test]
fn preview_diff_flip_resize_and_cross_frame_blit() {
    let mut document = Document::new(4, 2).unwrap();
    tool(
        &mut document,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 1}),
    );
    tool(
        &mut document,
        "paint",
        json!({"x": 1, "y": 0, "colorIndex": 2}),
    );
    tool(&mut document, "addFrame", json!({}));
    tool(
        &mut document,
        "setPaletteColor",
        json!({"index": 1, "r": 10, "g": 20, "b": 30}),
    );
    let color = tool(&mut document, "inspect", json!({}))["palette"][1].clone();

    let preview = tool(&mut document, "previewFrame", json!({"scale": 2}));
    assert_eq!(preview["frame"], json!(0));
    assert_eq!(preview["activeFrame"], json!(0));
    assert_eq!(preview["scale"], json!(2));
    assert_eq!(preview["pngWidth"], json!(8));
    assert_eq!(preview["pngHeight"], json!(4));
    assert!(preview["png"].as_str().unwrap().starts_with("iVBORw0KGgo"));
    let looked = tool(
        &mut document,
        "previewFrame",
        json!({"frame": 1, "scale": 1}),
    );
    assert_eq!(looked["frame"], json!(1));
    assert_eq!(looked["activeFrame"], json!(0));
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["canUndo"],
        json!(true)
    );
    let missing = tool_err(&mut document, "previewFrame", json!({"frame": 4}));
    assert_eq!(missing["code"], json!("FrameIndex"));

    tool(
        &mut document,
        "copyRect",
        json!({
            "srcX": 0, "srcY": 0, "width": 2, "height": 1,
            "dstX": 1, "dstY": 1, "fromFrame": 0, "toFrame": 1
        }),
    );
    tool(&mut document, "setActiveFrame", json!({"index": 1}));
    let copied = tool(&mut document, "inspect", json!({}));
    assert_eq!(copied["pixels"][1 * 4 + 1], json!(1));
    assert_eq!(copied["pixels"][1 * 4 + 2], json!(2));
    assert_eq!(copied["activeFrame"], json!(1));
    tool(&mut document, "setActiveFrame", json!({"index": 0}));
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["pixels"][0],
        json!(1)
    );

    tool(
        &mut document,
        "moveRect",
        json!({
            "srcX": 0, "srcY": 0, "width": 1, "height": 1,
            "dstX": 3, "dstY": 0, "fromFrame": 0, "toFrame": 1
        }),
    );
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["pixels"][0],
        json!(0)
    );
    tool(&mut document, "undo", json!({}));
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["pixels"][0],
        json!(1)
    );

    let diff = tool(
        &mut document,
        "diffFrames",
        json!({"frame": 0, "with": 1, "scale": 1}),
    );
    assert!(diff["changed"].as_u64().unwrap() > 0);
    assert_eq!(diff["panels"], json!(["frame", "with", "onion"]));
    assert_eq!(diff["pngWidth"], json!(12));
    assert_eq!(diff["diffRows"].as_array().unwrap().len(), 2);
    assert!(diff["png"].as_str().unwrap().starts_with("iVBORw0KGgo"));
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["activeFrame"],
        json!(0)
    );
    let bad_diff = tool_err(&mut document, "diffFrames", json!({"frame": 0}));
    assert_eq!(bad_diff["code"], json!("InvalidParams"));

    tool(&mut document, "flipHorizontal", json!({}));
    let flipped = tool(&mut document, "inspect", json!({}));
    assert_eq!(flipped["pixels"][3], json!(1));
    assert_eq!(flipped["pixels"][2], json!(2));
    assert_eq!(flipped["pixels"][0], json!(0));
    assert_eq!(flipped["palette"][1], color);
    tool(&mut document, "undo", json!({}));
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["pixels"][0],
        json!(1)
    );
    tool(
        &mut document,
        "flipHorizontal",
        json!({"x": 0, "y": 0, "width": 2, "height": 1, "frame": 0}),
    );
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["pixels"][0],
        json!(2)
    );
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["activeFrame"],
        json!(0)
    );
    let partial = tool_err(&mut document, "flipHorizontal", json!({"x": 0, "width": 2}));
    assert_eq!(partial["code"], json!("InvalidParams"));

    let resized = tool(
        &mut document,
        "resizeCanvas",
        json!({"width": 8, "height": 4}),
    );
    assert_eq!(resized["width"], json!(8));
    assert_eq!(resized["height"], json!(4));
    assert_eq!(resized["offsetX"], json!(2));
    assert_eq!(resized["offsetY"], json!(1));
    let placed = tool(&mut document, "inspect", json!({}));
    assert_eq!(placed["pixels"][1 * 8 + 2], json!(2));
    let saved = tool(&mut document, "saveProject", json!({}));
    let mut loaded = Document::load_project(saved["json"].as_str().unwrap()).unwrap();
    assert_eq!(loaded.width(), 8);
    assert_eq!(loaded.pixels()[1 * 8 + 2], 2);
    tool(&mut document, "undo", json!({}));
    assert_eq!(tool(&mut document, "inspect", json!({}))["width"], json!(4));
    let shifted = tool(
        &mut loaded,
        "resizeCanvas",
        json!({"width": 8, "height": 4, "offsetX": 0, "offsetY": 0}),
    );
    assert_eq!(shifted["offsetX"], json!(0));
    let one_offset = tool_err(
        &mut document,
        "resizeCanvas",
        json!({"width": 6, "height": 2, "offsetX": 1}),
    );
    assert_eq!(one_offset["code"], json!("InvalidParams"));

    let row = tool(
        &mut document,
        "exportSpriteSheet",
        json!({"columns": 2, "scale": 1}),
    );
    assert_eq!(row["columns"], json!(2));
    assert_eq!(row["rows"], json!(1));
    assert_eq!(row["sheetWidth"], json!(8));
    let square = tool(&mut document, "exportSpriteSheet", json!({}));
    assert_eq!(square["columns"], json!(2));
    assert_eq!(square["rows"], json!(1));
}

#[test]
fn scene_summary_lists_canvas_layers_palette_hash_and_delays() {
    let mut document = Document::new(8, 4).unwrap();
    let scene = tool(&mut document, "scene", json!({}));
    assert_eq!(scene["width"], json!(8));
    assert_eq!(scene["height"], json!(4));
    assert_eq!(scene["activeFrame"], json!(0));
    assert_eq!(scene["activeLayer"], json!(0));
    assert_eq!(scene["layerCount"], json!(1));
    assert_eq!(scene["layers"][0]["name"], json!("Layer 1"));
    assert_eq!(scene["layers"][0]["visible"], json!(true));
    assert!(scene["layers"][0]["opacity"].is_null());
    assert!(scene.get("pixels").is_none());
    assert_eq!(scene["paletteCount"], json!(9));
    assert_eq!(scene["paletteHash"], json!("efb5af0bb69fc10e"));
    assert_eq!(scene["palette"][0]["a"], json!(0));
    assert_eq!(scene["frameCount"], json!(1));
    assert_eq!(scene["delayMs"], json!(100));
    assert_eq!(scene["frames"][0]["delayMs"], json!(100));
    assert_eq!(scene["paletteLocked"], json!(false));
    assert!(!document.can_undo());

    tool(&mut document, "addLayer", json!({"name": "gear"}));
    tool(
        &mut document,
        "setFrameDelay",
        json!({"index": 0, "delayMs": 80}),
    );
    tool(
        &mut document,
        "setPaletteColor",
        json!({"index": 1, "r": 1, "g": 2, "b": 3}),
    );
    let scene = tool(&mut document, "scene", json!({}));
    assert_eq!(scene["layers"][1]["name"], json!("gear"));
    assert_eq!(scene["layers"][1]["visible"], json!(true));
    assert!(scene["layers"][1]["opacity"].is_null());
    assert_eq!(scene["activeLayer"], json!(1));
    assert_eq!(scene["frames"][0]["delayMs"], json!(80));
    assert_ne!(scene["paletteHash"], json!("efb5af0bb69fc10e"));
    assert_eq!(
        tool(&mut document, "inspect", json!({}))["paletteHash"],
        scene["paletteHash"]
    );
    let extra = tool_err(&mut document, "scene", json!({"pixels": true}));
    assert_eq!(extra["code"], json!("InvalidParams"));
}

#[test]
fn report_dirty_is_opt_in_and_bounds_changed_cells() {
    let mut document = Document::new(4, 3).unwrap();
    let plain = tool(
        &mut document,
        "paint",
        json!({"x": 1, "y": 1, "colorIndex": 3}),
    );
    assert_eq!(plain, json!({}));

    let same = tool(
        &mut document,
        "paint",
        json!({"x": 1, "y": 1, "colorIndex": 3, "reportDirty": true}),
    );
    assert_eq!(same["changed"], json!(0));
    assert!(same["dirty"].is_null());
    assert_eq!(same["regions"], json!([]));

    let painted = tool(
        &mut document,
        "paint",
        json!({"x": 2, "y": 0, "colorIndex": 4, "reportDirty": true}),
    );
    assert_eq!(painted["changed"], json!(1));
    assert_eq!(
        painted["dirty"],
        json!({"x": 2, "y": 0, "width": 1, "height": 1})
    );
    assert_eq!(painted["regions"][0]["layer"], json!(0));

    let filled = tool(
        &mut document,
        "fillRect",
        json!({
            "x": 1,
            "y": 0,
            "width": 2,
            "height": 2,
            "colorIndex": 4,
            "reportDirty": true
        }),
    );
    assert_eq!(filled["changed"], json!(3));
    assert_eq!(
        filled["dirty"],
        json!({"x": 1, "y": 0, "width": 2, "height": 2})
    );

    let erased = tool(
        &mut document,
        "eraseRect",
        json!({"x": 1, "y": 0, "width": 2, "height": 1, "reportDirty": true}),
    );
    assert_eq!(erased["changed"], json!(2));
    assert_eq!(
        erased["dirty"],
        json!({"x": 1, "y": 0, "width": 2, "height": 1})
    );

    tool(&mut document, "addFrame", json!({}));
    let moved = tool(
        &mut document,
        "moveRect",
        json!({
            "fromFrame": 0,
            "toFrame": 1,
            "srcX": 1,
            "srcY": 1,
            "width": 1,
            "height": 1,
            "dstX": 3,
            "dstY": 2,
            "reportDirty": true
        }),
    );
    assert_eq!(moved["changed"], json!(2));
    assert_eq!(moved["regions"].as_array().unwrap().len(), 2);
    assert_eq!(
        moved["dirty"],
        json!({"x": 1, "y": 1, "width": 3, "height": 2})
    );

    let flipped = tool(
        &mut document,
        "flipHorizontal",
        json!({"frame": 1, "reportDirty": true}),
    );
    assert!(flipped["changed"].as_u64().unwrap() > 0);
    assert!(flipped.get("clipped").is_none());

    let shifted = tool(
        &mut document,
        "resizeCanvas",
        json!({"width": 4, "height": 3, "offsetX": 1, "offsetY": 0, "reportDirty": true}),
    );
    assert_eq!(shifted["width"], json!(4));
    assert!(shifted["changed"].as_u64().unwrap() > 0);
    assert_eq!(shifted["clipped"], json!(0));
    assert!(shifted.get("dirty").is_some());

    let mut wide = Document::new(4, 1).unwrap();
    tool(&mut wide, "paint", json!({"x": 3, "y": 0, "colorIndex": 1}));
    let shrunk = tool(
        &mut wide,
        "resizeCanvas",
        json!({"width": 2, "height": 1, "offsetX": 0, "offsetY": 0, "reportDirty": true}),
    );
    assert_eq!(shrunk["changed"], json!(0));
    assert!(shrunk["dirty"].is_null());
    assert_eq!(shrunk["clipped"], json!(1));

    let bad = tool_err(
        &mut document,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 1, "reportDirty": "yes"}),
    );
    assert_eq!(bad["code"], json!("InvalidParams"));
    assert_eq!(
        bad["message"],
        json!("paint: reportDirty must be a boolean")
    );
    let off = tool(
        &mut document,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 1, "reportDirty": false}),
    );
    assert_eq!(off, json!({}));
}

#[test]
fn preview_and_diff_include_structured_fields() {
    let mut document = Document::new(4, 2).unwrap();
    tool(
        &mut document,
        "paint",
        json!({"x": 1, "y": 0, "colorIndex": 3}),
    );
    tool(
        &mut document,
        "paint",
        json!({"x": 1, "y": 1, "colorIndex": 3}),
    );
    let preview = tool(&mut document, "previewFrame", json!({}));
    assert_eq!(preview["ink"], json!(2));
    assert_eq!(
        preview["bounds"],
        json!({"x": 1, "y": 0, "width": 1, "height": 2})
    );
    assert_eq!(
        preview["indices"],
        json!([{"index": 0, "count": 6}, {"index": 3, "count": 2}])
    );
    assert_eq!(preview["pixels"][1], json!(3));
    assert_eq!(preview["pixels"].as_array().unwrap().len(), 8);
    assert!(preview["png"].as_str().unwrap().starts_with("iVBORw0KGgo"));

    tool(&mut document, "addFrame", json!({}));
    tool(&mut document, "setActiveFrame", json!({"index": 1}));
    tool(
        &mut document,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 1}),
    );
    tool(
        &mut document,
        "paint",
        json!({"x": 1, "y": 0, "colorIndex": 4}),
    );
    tool(
        &mut document,
        "paint",
        json!({"x": 2, "y": 0, "colorIndex": 5}),
    );
    let diff = tool(&mut document, "diffFrames", json!({"frame": 0, "with": 1}));
    assert_eq!(diff["changed"], json!(4));
    assert_eq!(diff["kinds"]["differ"], json!(1));
    assert_eq!(diff["kinds"]["onlyFrame"], json!(1));
    assert_eq!(diff["kinds"]["onlyWith"], json!(2));
    assert_eq!(
        diff["bounds"],
        json!({"x": 0, "y": 0, "width": 3, "height": 2})
    );
    let changes = diff["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 4);
    assert_eq!(changes[0]["left"], json!(0));
    assert_eq!(changes[0]["right"], json!(1));
    assert_eq!(changes[1]["left"], json!(3));
    assert_eq!(changes[1]["right"], json!(4));
    assert_eq!(diff["diffRows"].as_array().unwrap().len(), 2);
}
