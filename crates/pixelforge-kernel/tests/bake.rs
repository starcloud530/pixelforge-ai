//! Bake samples curves and constraints into editable pixel frames.

use pixelforge_kernel::{CurveChannel, Document, KernelError, LoopMode};
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

fn paint_at(document: &mut Document, layer: usize, x: u16, y: u16, color: u8) {
    document.set_active_layer(layer).unwrap();
    document.paint(x, y, color).unwrap();
}

/// Rest pose on frame 0. Frames 1..=4 start blank. Gun rotation is 0 at frame 0
/// and 90 at frame 4.
fn soldier() -> Document {
    let mut document = Document::new(16, 16).unwrap();
    for _ in 0..4 {
        document.add_frame();
    }
    document.add_layer(Some("hand")).unwrap();
    document.add_layer(Some("foot")).unwrap();
    paint_at(&mut document, 0, 4, 4, 1);
    paint_at(&mut document, 0, 2, 4, 2);
    paint_at(&mut document, 0, 6, 4, 3);
    paint_at(&mut document, 1, 6, 4, 4);
    paint_at(&mut document, 2, 3, 11, 5);
    document.add_part("shoulder", 2, 4, None).unwrap();
    document.set_anchor("shoulder", "joint", 0, 0).unwrap();
    document.add_part("gun", 4, 4, Some(0)).unwrap();
    document.set_anchor("gun", "butt", -2, 0).unwrap();
    document.set_anchor("gun", "grip", 2, 0).unwrap();
    document.add_part("hand", 6, 4, Some(1)).unwrap();
    document.set_anchor("hand", "palm", 0, 0).unwrap();
    document.add_part("foot", 3, 10, Some(2)).unwrap();
    document.set_anchor("foot", "sole", 0, 1).unwrap();
    document
        .set_curve_key("gun", CurveChannel::Rotation, 0, 0.0, None)
        .unwrap();
    document
        .set_curve_key("gun", CurveChannel::Rotation, 4, 90.0, None)
        .unwrap();
    document.clear_history();
    document
}

fn arm(document: &mut Document) {
    document
        .add_constraint(
            "hand-grip",
            pixelforge_kernel::ConstraintKind::Follow,
            "hand",
            Some("palm"),
            Some("gun"),
            Some("grip"),
            None,
            None,
        )
        .unwrap();
    document
        .add_constraint(
            "butt-shoulder",
            pixelforge_kernel::ConstraintKind::Attach,
            "gun",
            Some("butt"),
            Some("shoulder"),
            Some("joint"),
            None,
            None,
        )
        .unwrap();
    document
        .add_constraint(
            "plant-foot",
            pixelforge_kernel::ConstraintKind::Pin,
            "foot",
            Some("sole"),
            None,
            None,
            Some(4),
            Some(11),
        )
        .unwrap();
}

fn copy_rest(document: &mut Document, frame: usize) {
    for layer in 0..document.layer_count() {
        document
            .copy_rect_from_on(layer, 0, 0, 0, 16, 16, frame, 0, 0)
            .unwrap();
    }
}

fn solved_like(frame: usize) -> Document {
    let mut document = soldier();
    arm(&mut document);
    copy_rest(&mut document, frame);
    document
        .solve_constraints(Some(frame as f64), true, Some(frame))
        .unwrap();
    document
}

fn part<'a>(document: &'a Document, name: &str) -> &'a pixelforge_kernel::Part {
    document
        .parts()
        .iter()
        .find(|part| part.name == name)
        .unwrap()
}

#[test]
fn bake_matches_solve_and_leaves_the_rest_pose() {
    let mut document = soldier();
    arm(&mut document);
    document.clear_history();
    let curves = document.curves().to_vec();
    let constraints = document.constraints().to_vec();
    let gun = part(&document, "gun").clone();

    let baked = tool(&mut document, "bake", json!({"from": 0, "to": 4}));
    assert_eq!(baked["source"], json!(0));
    assert_eq!(baked["wrote"], json!(true));
    assert_eq!(baked["appended"], json!(0));
    assert_eq!(baked["constraints"], json!(true));
    assert_eq!(baked["fps"], Value::Null);
    assert_eq!(baked["frames"][0]["rest"], json!(true));
    assert_eq!(baked["frames"][0]["layers"], json!([]));
    assert_eq!(baked["frames"][4]["parts"][1]["name"], json!("gun"));
    assert_eq!(baked["frames"][4]["parts"][1]["x"], json!(2));
    assert_eq!(baked["frames"][4]["parts"][1]["y"], json!(6));
    assert_eq!(baked["frames"][4]["parts"][1]["ink"], json!("moved"));
    assert_eq!(
        baked["frames"][4]["parts"][1]["anchors"][1]["canvasX"],
        json!(2)
    );
    assert_eq!(
        baked["frames"][4]["parts"][1]["anchors"][1]["canvasY"],
        json!(8)
    );

    let reference = solved_like(4);
    for layer in 0..3 {
        assert_eq!(
            document.layer_pixels(4, layer).unwrap(),
            reference.layer_pixels(4, layer).unwrap(),
            "layer {layer}"
        );
    }
    let mid = solved_like(2);
    assert_eq!(
        document.layer_pixels(2, 0).unwrap(),
        mid.layer_pixels(2, 0).unwrap()
    );
    assert_ne!(
        document.layer_pixels(2, 0).unwrap(),
        document.layer_pixels(4, 0).unwrap()
    );

    assert_eq!(cell(&document, 0, 0, 4, 4), 1);
    assert_eq!(cell(&document, 0, 0, 6, 4), 3);
    assert_eq!(cell(&document, 0, 1, 6, 4), 4);
    assert_eq!(cell(&document, 0, 2, 3, 11), 5);
    assert_eq!(
        (part(&document, "gun").x, part(&document, "gun").y),
        (gun.x, gun.y)
    );
    assert_eq!(document.curves(), curves.as_slice());
    assert_eq!(document.constraints(), constraints.as_slice());
    assert!(document.can_undo());

    let again = tool(&mut document, "bake", json!({}));
    assert_eq!(again["wrote"], json!(false));
    assert_eq!(cell(&document, 4, 0, 2, 8), 3);
    document.undo().unwrap();
    assert_eq!(cell(&document, 4, 0, 4, 4), 0);
    assert_eq!(cell(&document, 0, 0, 4, 4), 1);
    assert!(document.undo().is_err());
}

#[test]
fn curves_only_skips_constraints_and_filters_limit_layers() {
    let mut spinning = soldier();
    arm(&mut spinning);
    spinning.clear_history();
    tool(
        &mut spinning,
        "bake",
        json!({"from": 4, "to": 4, "constraints": false}),
    );
    assert_eq!(cell(&spinning, 4, 0, 4, 6), 3);
    assert_eq!(cell(&spinning, 4, 0, 4, 2), 2);
    assert_eq!(cell(&spinning, 4, 0, 4, 4), 1);
    assert_eq!(cell(&spinning, 4, 0, 2, 8), 0);
    assert_eq!(cell(&spinning, 4, 1, 2, 8), 0);
    assert_eq!(cell(&spinning, 4, 2, 4, 11), 0);
    assert_eq!(part(&spinning, "gun").x, 4);

    let mut gun_only = soldier();
    arm(&mut gun_only);
    copy_rest(&mut gun_only, 4);
    gun_only.clear_history();
    let filtered = tool(
        &mut gun_only,
        "bake",
        json!({"from": 4, "to": 4, "parts": ["gun"]}),
    );
    assert_eq!(filtered["frames"][0]["layers"], json!([0]));
    assert_eq!(filtered["frames"][0]["parts"][1]["ink"], json!("moved"));
    assert_eq!(filtered["frames"][0]["parts"][2]["ink"], json!("unchanged"));
    assert_eq!(cell(&gun_only, 4, 0, 2, 8), 3);
    assert_eq!(cell(&gun_only, 4, 1, 6, 4), 4);
    assert_eq!(cell(&gun_only, 4, 2, 3, 11), 5);

    let mut hand_layer = soldier();
    arm(&mut hand_layer);
    hand_layer.clear_history();
    tool(
        &mut hand_layer,
        "bake",
        json!({"from": 4, "to": 4, "layers": [1]}),
    );
    assert_eq!(cell(&hand_layer, 4, 1, 2, 8), 4);
    assert_eq!(cell(&hand_layer, 4, 0, 2, 8), 0);
    assert_eq!(cell(&hand_layer, 4, 2, 4, 11), 0);
}

#[test]
fn fps_sets_delay_and_append_holds_the_end_pose() {
    let mut document = soldier();
    arm(&mut document);
    document.add_layer(Some("body")).unwrap();
    paint_at(&mut document, 3, 1, 1, 6);
    document.add_tag("raise", 0, 4, LoopMode::Forward).unwrap();
    document.clear_history();

    let baked = tool(
        &mut document,
        "bake",
        json!({"from": 1, "to": 6, "fps": 12}),
    );
    assert_eq!(baked["delayMs"], json!(83));
    assert_eq!(baked["fps"], json!(12));
    assert_eq!(baked["appended"], json!(2));
    assert_eq!(document.frame_count(), 7);
    assert_eq!(document.frame_delay(0).unwrap(), 100);
    assert_eq!(document.frame_delay(1).unwrap(), 83);
    assert_eq!(document.frame_delay(6).unwrap(), 83);
    assert_eq!(cell(&document, 4, 3, 1, 1), 0);
    assert_eq!(cell(&document, 5, 3, 1, 1), 6);
    assert_eq!(cell(&document, 6, 0, 2, 8), cell(&document, 4, 0, 2, 8));
    assert_eq!(document.tags()[0].to, 4);
    assert_eq!(
        document
            .sample_curve("gun", CurveChannel::Rotation, 4.0)
            .unwrap()
            .value,
        90.0
    );
    assert_eq!(
        document
            .sample_curve("gun", CurveChannel::Rotation, 6.0)
            .unwrap()
            .value,
        90.0
    );

    document.undo().unwrap();
    assert_eq!(document.frame_count(), 5);
    assert_eq!(document.frame_delay(1).unwrap(), 100);
    assert_eq!(cell(&document, 4, 0, 4, 4), 0);
    document.redo().unwrap();
    assert_eq!(document.frame_count(), 7);
    assert_eq!(cell(&document, 6, 0, 2, 8), 3);
    assert_eq!(cell(&document, 5, 3, 1, 1), 6);
}

#[test]
fn a_refused_bake_writes_nothing() {
    let mut document = soldier();
    arm(&mut document);
    document.clear_history();
    let range = tool_err(&mut document, "bake", json!({"from": 3, "to": 1}));
    assert_eq!(range["code"], json!("BakeRange"));
    let fps = tool_err(&mut document, "bake", json!({"fps": -1}));
    assert_eq!(fps["code"], json!("BakeFps"));
    let delay = tool_err(&mut document, "bake", json!({"fps": 0.01}));
    assert_eq!(delay["code"], json!("FrameDelay"));
    let missing = tool_err(&mut document, "bake", json!({"parts": ["nope"]}));
    assert_eq!(missing["code"], json!("PartMissing"));
    let empty = tool_err(&mut document, "bake", json!({"parts": []}));
    assert_eq!(empty["code"], json!("InvalidParams"));
    assert!(!document.can_undo());
    assert_eq!(cell(&document, 4, 0, 4, 4), 0);

    document
        .set_curve_key("gun", CurveChannel::TranslateX, 4, 100.0, None)
        .unwrap();
    document.clear_history();
    let off = tool_err(
        &mut document,
        "bake",
        json!({"from": 4, "to": 4, "constraints": false}),
    );
    assert_eq!(off["code"], json!("PartOffCanvas"));
    assert_eq!(cell(&document, 4, 0, 4, 4), 0);
    assert!(document.curves().iter().any(|curve| curve.part == "gun"));

    let mut selected = soldier();
    arm(&mut selected);
    selected.set_selection(0, 0, 1, 1).unwrap();
    selected.clear_history();
    let protected = tool_err(&mut selected, "bake", json!({"from": 4, "to": 4}));
    assert_eq!(protected["code"], json!("SelectionProtected"));
    assert!(!selected.can_undo());

    let mut shared = Document::new(8, 8).unwrap();
    shared.add_frame();
    shared.add_layer(Some("same")).unwrap();
    paint_at(&mut shared, 0, 1, 1, 1);
    paint_at(&mut shared, 0, 3, 1, 2);
    shared.add_part("gun", 1, 1, Some(0)).unwrap();
    shared.add_part("hand", 3, 1, Some(0)).unwrap();
    shared
        .set_curve_key("gun", CurveChannel::TranslateX, 1, 1.0, None)
        .unwrap();
    shared
        .set_curve_key("hand", CurveChannel::TranslateY, 1, 1.0, None)
        .unwrap();
    shared.clear_history();
    let layer = tool_err(&mut shared, "bake", json!({"from": 1, "to": 1}));
    assert_eq!(layer["code"], json!("ConstraintLayer"));
    assert_eq!(cell(&shared, 1, 0, 1, 1), 0);
    assert!(!shared.can_undo());
}

#[test]
fn baked_pixels_stay_editable_and_the_file_stays_version_1() {
    let mut document = soldier();
    arm(&mut document);
    document.clear_history();
    tool(&mut document, "bake", json!({"from": 0, "to": 4, "fps": 8}));
    assert_eq!(document.frame_delay(4).unwrap(), 125);
    assert_eq!(document.frame_delay(0).unwrap(), 125);

    document.set_active_frame(4).unwrap();
    document.set_active_layer(1).unwrap();
    document.paint(1, 1, 7).unwrap();
    assert_eq!(cell(&document, 4, 1, 1, 1), 7);
    document.add_tag("raise_gun", 1, 4, LoopMode::Once).unwrap();
    document.set_selection(0, 0, 2, 2).unwrap();
    document.paint(1, 1, 8).unwrap();
    assert_eq!(cell(&document, 4, 1, 1, 1), 8);
    document.paint(10, 10, 8).unwrap();
    assert_ne!(cell(&document, 4, 1, 10, 10), 8);
    assert_eq!(cell(&document, 4, 0, 2, 8), 3);

    let text = document.save_project();
    let value: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["version"], json!(1));
    assert!(value.get("bake").is_none());
    assert_eq!(value["curves"].as_array().unwrap().len(), 1);
    assert_eq!(value["constraints"].as_array().unwrap().len(), 3);
    let mut loaded = Document::load_project(&text).unwrap();
    assert_eq!(cell(&loaded, 4, 0, 2, 8), 3);
    assert_eq!(cell(&loaded, 4, 1, 1, 1), 8);
    assert_eq!(loaded.tags()[0].name, "raise_gun");
    assert_eq!(part(&loaded, "gun").x, 4);
    assert!(matches!(
        loaded.bake(pixelforge_kernel::BakeOptions {
            from: Some(8),
            to: Some(1),
            ..pixelforge_kernel::BakeOptions::default()
        }),
        Err(KernelError::BakeRange { .. })
    ));
}
