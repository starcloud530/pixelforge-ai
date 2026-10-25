use pixelforge_kernel::{CurveChannel, Document, Ease, KernelError, LoopMode, ProjectFileError};
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

fn gun() -> Document {
    let mut document = Document::new(8, 8).unwrap();
    document.add_frame();
    document.add_frame();
    document.add_frame();
    document.add_frame();
    document.add_part("gun", 3, 3, Some(0)).unwrap();
    document.add_tag("shoot", 0, 4, LoopMode::Once).unwrap();
    document.set_frame_delay(1, 40).unwrap();
    document.paint(4, 3, 1).unwrap();
    document.clear_history();
    document
}

fn pixels(document: &Document) -> Vec<u8> {
    (0..document.frame_count())
        .flat_map(|frame| document.layer_pixels(frame, 0).unwrap().to_vec())
        .collect()
}

#[test]
fn set_get_linear_and_ease_in_out_midpoints_round_trip_beside_tags() {
    let mut document = gun();
    let before = pixels(&document);
    assert!(tool(&mut document, "scene", json!({}))["curves"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(!document.save_project().contains("\"curves\""));

    let start = tool(
        &mut document,
        "setCurveKey",
        json!({"part": " gun ", "channel": "rotation", "frame": 0, "value": 0}),
    );
    assert_eq!(start["part"], json!("gun"));
    assert_eq!(start["ease"], json!("linear"));
    assert_eq!(start["value"], json!(0));
    assert_eq!(start["keyIndex"], json!(0));

    tool(
        &mut document,
        "setCurveKey",
        json!({"part": "gun", "channel": "rotation", "frame": 4, "value": 90, "ease": "linear"}),
    );
    let linear = tool(
        &mut document,
        "sampleCurve",
        json!({"part": "gun", "channel": "rotation", "frame": 2}),
    );
    assert_eq!(linear["value"], json!(45));
    assert_eq!(linear["interpolation"], json!("linear"));
    assert_eq!(linear["leftFrame"], json!(0));
    assert_eq!(linear["rightFrame"], json!(4));
    let linear_quarter = tool(
        &mut document,
        "sampleCurve",
        json!({"part": "gun", "channel": "rotation", "frame": 1}),
    );
    assert_eq!(linear_quarter["value"], json!(22.5));

    tool(
        &mut document,
        "setCurveKey",
        json!({
            "part": "gun",
            "channel": "rotation",
            "frame": 0,
            "value": 0,
            "ease": "easeInOut"
        }),
    );
    let eased = tool(
        &mut document,
        "sampleCurve",
        json!({"part": "gun", "channel": "rotation", "frame": 2}),
    );
    assert_eq!(eased["value"], json!(45));
    assert_eq!(eased["interpolation"], json!("easeInOut"));
    let quarter = tool(
        &mut document,
        "sampleCurve",
        json!({"part": "gun", "channel": "rotation", "frame": 1}),
    );
    assert_eq!(quarter["value"], json!(14.0625));
    assert_eq!(quarter["interpolation"], json!("easeInOut"));

    tool(
        &mut document,
        "setCurveKey",
        json!({"part": "gun", "channel": "translateX", "frame": 0, "value": 0}),
    );
    tool(
        &mut document,
        "setCurveKey",
        json!({"part": "gun", "channel": "translateX", "frame": 4, "value": 4}),
    );
    tool(
        &mut document,
        "setCurveKey",
        json!({"part": "gun", "channel": "flipHorizontal", "frame": 0, "value": 0}),
    );
    tool(
        &mut document,
        "setCurveKey",
        json!({"part": "gun", "channel": "flipHorizontal", "frame": 4, "value": 1}),
    );
    let pose = tool(
        &mut document,
        "samplePart",
        json!({"part": "gun", "frame": 2}),
    );
    assert_eq!(pose["rotation"], json!(45));
    assert_eq!(pose["translateX"], json!(2));
    assert_eq!(pose["flipHorizontal"], json!(0));
    assert!(pose.get("translateY").is_none());
    assert_eq!(pixels(&document), before);

    assert_eq!(document.tags()[0].name, "shoot");
    assert_eq!(document.frame_delay(1).unwrap(), 40);
    let slow = tool(
        &mut document,
        "sampleCurve",
        json!({"part": "gun", "channel": "rotation", "frame": 2}),
    );
    document.set_frame_delay(0, 500).unwrap();
    let still = tool(
        &mut document,
        "sampleCurve",
        json!({"part": "gun", "channel": "rotation", "frame": 2}),
    );
    assert_eq!(still["value"], slow["value"]);

    let listed = tool(&mut document, "listCurves", json!({"part": "gun"}));
    assert_eq!(listed["curves"].as_array().unwrap().len(), 3);
    let scene = tool(&mut document, "scene", json!({}));
    assert_eq!(scene["tags"][0]["name"], json!("shoot"));
    assert_eq!(scene["curves"][0]["channel"], json!("rotation"));

    let saved = document.save_project();
    assert!(saved.contains("\"curves\""));
    assert!(saved.contains("\"ease\": \"easeInOut\""));
    assert!(!saved.contains("\"ease\": \"linear\""));
    assert!(!saved.contains("\"ease\": \"hold\""));
    let loaded = Document::load_project(&saved).unwrap();
    assert_eq!(loaded.tags()[0].name, "shoot");
    assert_eq!(loaded.frame_delay(1).unwrap(), 40);
    assert_eq!(
        loaded
            .sample_curve("gun", CurveChannel::Rotation, 2.0)
            .unwrap()
            .value,
        45.0
    );
    assert_eq!(
        loaded
            .sample_curve("gun", CurveChannel::Rotation, 1.0)
            .unwrap()
            .value,
        14.0625
    );
    assert_eq!(loaded.save_project(), saved);
    assert_eq!(pixels(&loaded), before);

    let ease = document.curves()[0].keys[0].ease;
    let same = tool(
        &mut document,
        "setCurveKey",
        json!({
            "part": "gun",
            "channel": "rotation",
            "frame": 0,
            "value": 0,
            "ease": "easeInOut"
        }),
    );
    assert_eq!(same["ease"], json!("easeInOut"));
    document.undo().unwrap();
    assert_eq!(document.curves()[0].keys[0].ease, ease);
    assert_eq!(document.frame_delay(0).unwrap(), 100);
}

#[test]
fn undo_shifts_with_frames_and_follows_the_part() {
    let mut document = gun();
    document
        .set_curve_key("gun", CurveChannel::Rotation, 1, 10.0, Some(Ease::Linear))
        .unwrap();
    document
        .set_curve_key("gun", CurveChannel::Rotation, 4, 90.0, None)
        .unwrap();
    document.begin_gesture().unwrap();
    document
        .set_curve_key("gun", CurveChannel::TranslateY, 0, -2.0, None)
        .unwrap();
    document
        .set_curve_key("gun", CurveChannel::TranslateY, 4, 2.0, None)
        .unwrap();
    document.end_gesture();
    document.undo().unwrap();
    assert!(document
        .sample_curve("gun", CurveChannel::TranslateY, 2.0)
        .is_err());
    document.redo().unwrap();
    assert_eq!(
        document
            .sample_curve("gun", CurveChannel::TranslateY, 2.0)
            .unwrap()
            .value,
        0.0
    );

    document.duplicate_frame(0).unwrap();
    assert_eq!(document.curves()[0].keys[0].frame, 2);
    assert_eq!(document.curves()[0].keys[1].frame, 5);
    assert_eq!(document.tags()[0].to, 5);
    document.delete_frame(2).unwrap();
    assert_eq!(document.curves()[0].keys.len(), 1);
    assert_eq!(document.curves()[0].keys[0].frame, 4);
    document.undo().unwrap();
    assert_eq!(document.curves()[0].keys[0].frame, 2);

    document
        .set_part(
            "gun",
            pixelforge_kernel::PartUpdate {
                x: None,
                y: None,
                layer: pixelforge_kernel::LayerAttach::Keep,
                rename: Some("rifle".to_string()),
            },
        )
        .unwrap();
    assert_eq!(document.curves()[0].part, "rifle");
    document.remove_part("rifle").unwrap();
    assert!(document.curves().is_empty());
    document.undo().unwrap();
    assert_eq!(document.curves()[0].part, "rifle");
    assert_eq!(document.curves().len(), 2);
    assert!(document.save_project().contains("\"curves\""));
}

#[test]
fn resize_drops_a_curve_when_the_part_leaves() {
    let mut document = gun();
    document
        .set_curve_key("gun", CurveChannel::Rotation, 0, 15.0, None)
        .unwrap();
    document.resize_canvas(4, 4, -4, -4).unwrap();
    assert!(document.parts().is_empty());
    assert!(document.curves().is_empty());
    document.undo().unwrap();
    assert_eq!(document.curves()[0].part, "gun");
}

#[test]
fn rejects_bad_values_and_unknown_curve_fields() {
    let mut document = gun();
    assert_eq!(
        tool_err(
            &mut document,
            "setCurveKey",
            json!({"part": "gun", "channel": "scale", "frame": 0, "value": 1})
        )["code"],
        json!("InvalidParams")
    );
    assert_eq!(
        tool_err(
            &mut document,
            "setCurveKey",
            json!({"part": "gun", "channel": "flipHorizontal", "frame": 0, "value": 0, "ease": "linear"})
        )["code"],
        json!("CurveEase")
    );
    assert_eq!(
        tool_err(
            &mut document,
            "setCurveKey",
            json!({"part": "gun", "channel": "translateX", "frame": 0, "value": 1.5})
        )["code"],
        json!("CurveValue")
    );
    assert_eq!(
        tool_err(
            &mut document,
            "setCurveKey",
            json!({"part": "gun", "channel": "rotation", "frame": 9, "value": 0})
        )["code"],
        json!("FrameIndex")
    );
    assert_eq!(
        tool_err(
            &mut document,
            "sampleCurve",
            json!({"part": "gun", "channel": "rotation", "frame": 1})
        )["code"],
        json!("CurveMissing")
    );
    tool(
        &mut document,
        "setCurveKey",
        json!({"part": "gun", "channel": "rotation", "frame": 0, "value": 20.5}),
    );
    assert_eq!(
        tool_err(
            &mut document,
            "removeCurveKey",
            json!({"part": "gun", "channel": "rotation", "frame": 1})
        )["code"],
        json!("CurveKeyMissing")
    );

    let mut saved: Value = serde_json::from_str(&document.save_project()).unwrap();
    saved["curves"][0]["keys"][0]["expression"] = json!(1);
    match Document::load_project(&saved.to_string()).unwrap_err() {
        KernelError::ProjectFile(ProjectFileError::Malformed) => {}
        other => panic!("expected malformed, got {other:?}"),
    }
    let mut saved: Value = serde_json::from_str(&document.save_project()).unwrap();
    saved["curves"][0]["channel"] = json!("scale");
    match Document::load_project(&saved.to_string()).unwrap_err() {
        KernelError::ProjectFile(ProjectFileError::CurveChannel { index: 0 }) => {}
        other => panic!("expected CurveChannel, got {other:?}"),
    }

    let bare = Document::new(2, 2).unwrap().save_project();
    assert!(!bare.contains("\"curves\""));
    let loaded = Document::load_project(&bare).unwrap();
    assert!(loaded.curves().is_empty());
    assert_eq!(
        loaded.sample_part("missing", 0.0).unwrap_err(),
        KernelError::PartMissing
    );
}

#[test]
fn hold_flip_and_twenty_point_five_survive_the_file() {
    let mut document = gun();
    document
        .set_curve_key("gun", CurveChannel::Rotation, 0, 0.0, None)
        .unwrap();
    document
        .set_curve_key(
            "gun",
            CurveChannel::Rotation,
            2,
            20.5,
            Some(Ease::EaseInOut),
        )
        .unwrap();
    document
        .set_curve_key("gun", CurveChannel::FlipVertical, 0, 1.0, None)
        .unwrap();
    document
        .set_curve_key("gun", CurveChannel::FlipVertical, 4, 0.0, Some(Ease::Hold))
        .unwrap();
    let loaded = Document::load_project(&document.save_project()).unwrap();
    assert_eq!(
        loaded
            .sample_curve("gun", CurveChannel::Rotation, 0.0)
            .unwrap()
            .value,
        0.0
    );
    assert_eq!(loaded.curves()[0].keys[1].ease, Ease::EaseInOut);
    assert_eq!(loaded.curves()[0].keys[1].value, 2_050);
    let held = loaded
        .sample_curve("gun", CurveChannel::FlipVertical, 2.0)
        .unwrap();
    assert_eq!(held.value, 1.0);
    assert_eq!(held.hit, pixelforge_kernel::SampleHit::Hold);
    let at_key = loaded
        .sample_curve("gun", CurveChannel::FlipVertical, 4.0)
        .unwrap();
    assert_eq!(at_key.value, 0.0);
    assert_eq!(at_key.hit, pixelforge_kernel::SampleHit::Key);
}
