//! Layer stack: old files stay one layer, drawing hits one layer, views composite.

use pixelforge_kernel::{
    Document, KernelError, ProjectFileError, DEFAULT_LAYER_NAME, TRANSPARENT_INDEX,
};
use serde_json::{json, Value};

fn doc() -> Document {
    Document::new(4, 2).unwrap()
}

fn call(document: &mut Document, request: Value) -> Value {
    let response = document.call_tool(&request.to_string());
    serde_json::from_str(&response).expect("tool response is JSON")
}

fn tool(document: &mut Document, name: &str, params: Value) -> Value {
    let response = call(document, json!({ "tool": name, "params": params }));
    assert_eq!(response["ok"], json!(true), "{name}: {response}");
    response["result"].clone()
}

fn tool_err(document: &mut Document, name: &str, params: Value) -> Value {
    let response = call(document, json!({ "tool": name, "params": params }));
    assert_eq!(response["ok"], json!(false), "{name}: {response}");
    response["error"].clone()
}

#[test]
fn a_new_document_is_one_default_layer_and_old_files_load_that_way() {
    let document = doc();
    assert_eq!(document.layer_count(), 1);
    assert_eq!(document.active_layer(), 0);
    assert_eq!(document.layer_name(0).unwrap(), DEFAULT_LAYER_NAME);
    assert!(!document.save_project().contains("\"layers\""));
    assert!(!document.save_project().contains("activeLayer"));

    let loaded = Document::load_project(
        r#"{
  "format": "pixelforge-project",
  "version": 1,
  "width": 2,
  "height": 1,
  "activeFrame": 0,
  "palette": [
    { "r": 0, "g": 0, "b": 0, "a": 0 },
    { "r": 1, "g": 2, "b": 3, "a": 255 }
  ],
  "frames": [ { "pixels": [1, 0] } ]
}"#,
    )
    .unwrap();
    assert_eq!(loaded.layer_count(), 1);
    assert_eq!(loaded.layer_name(0).unwrap(), DEFAULT_LAYER_NAME);
    assert_eq!(loaded.frame_pixels(0).unwrap(), &[1, 0]);
    assert_eq!(loaded.composite_pixels(0).unwrap(), vec![1, 0]);
    assert!(!loaded.save_project().contains("\"layers\""));
}

#[test]
fn upper_layer_covers_index_zero_and_does_not_change_the_body() {
    let mut document = doc();
    document.rename_layer(0, "body").unwrap();
    document.fill_rect(0, 0, 2, 2, 1).unwrap();
    let gear = document.add_layer(Some("gear")).unwrap();
    assert_eq!(gear, 1);
    assert_eq!(document.active_layer(), 1);
    document.fill_rect(1, 0, 2, 1, 3).unwrap();

    assert_eq!(
        document.layer_pixels(0, 0).unwrap(),
        &[1, 1, 0, 0, 1, 1, 0, 0]
    );
    assert_eq!(
        document.layer_pixels(0, 1).unwrap(),
        &[0, 3, 3, 0, 0, 0, 0, 0]
    );
    assert_eq!(
        document.composite_pixels(0).unwrap(),
        vec![1, 3, 3, 0, 1, 1, 0, 0]
    );

    document.paint(1, 0, 4).unwrap();
    assert_eq!(document.layer_pixels(0, 0).unwrap()[1], 1);
    assert_eq!(document.layer_pixels(0, 1).unwrap()[1], 4);
    assert_eq!(document.composite_pixels(0).unwrap()[1], 4);

    document.erase(1, 0).unwrap();
    assert_eq!(document.layer_pixels(0, 1).unwrap()[1], TRANSPARENT_INDEX);
    assert_eq!(document.layer_pixels(0, 0).unwrap()[1], 1);
    assert_eq!(document.composite_pixels(0).unwrap()[1], 1);

    let png = document.export_frame_png(0).unwrap();
    let body_only = {
        let mut hidden = document.clone();
        hidden.set_active_layer(1).unwrap();
        hidden.erase_rect(0, 0, 4, 2).unwrap();
        hidden.export_frame_png(0).unwrap()
    };
    assert_ne!(png, body_only);
    assert_eq!(document.layer_pixels(0, 0).unwrap()[0], 1);
}

#[test]
fn drawing_on_one_layer_leaves_the_other_and_other_frames_alone() {
    let mut document = doc();
    document.fill_rect(0, 0, 4, 2, 2).unwrap();
    document.add_frame();
    document.add_layer(Some("gear")).unwrap();
    document.fill_rect_on(1, 0, 0, 1, 1, 5).unwrap();
    document
        .copy_rect_from_on(1, 0, 0, 0, 1, 1, 0, 2, 0)
        .unwrap();
    document.flip_horizontal_on(1, 0, 0, 0, 4, 1).unwrap();

    assert!(document
        .layer_pixels(0, 0)
        .unwrap()
        .iter()
        .all(|index| *index == 2));
    assert_eq!(document.layer_pixels(1, 0).unwrap(), &[0; 8]);
    assert_eq!(document.layer_pixels(1, 1).unwrap(), &[0; 8]);
    let gear = document.layer_pixels(0, 1).unwrap();
    assert_eq!(gear[0], 0);
    assert_ne!(gear, &[0; 8]);
    assert_eq!(document.active_layer(), 1);
}

#[test]
fn layer_edits_undo_and_flatten_writes_a_single_layer_file() {
    let mut document = doc();
    document.rename_layer(0, "body").unwrap();
    document.fill_rect(0, 0, 1, 1, 1).unwrap();
    document.add_layer(Some("gear")).unwrap();
    document.fill_rect(1, 0, 1, 1, 4).unwrap();
    document.add_layer(None).unwrap();
    assert_eq!(document.layer_name(2).unwrap(), "Layer 3");
    document.reorder_layer(2, 0).unwrap();
    assert_eq!(
        document.layer_names(),
        &[
            "Layer 3".to_string(),
            "body".to_string(),
            "gear".to_string()
        ]
    );
    assert_eq!(document.active_layer(), 0);
    document.undo().unwrap();
    assert_eq!(
        document.layer_names(),
        &[
            "body".to_string(),
            "gear".to_string(),
            "Layer 3".to_string()
        ]
    );
    document.delete_layer(2).unwrap();
    assert_eq!(document.layer_count(), 2);
    assert_eq!(document.active_layer(), 1);
    document.delete_layer(1).unwrap();
    assert_eq!(
        document.delete_layer(0).unwrap_err(),
        KernelError::LastLayer
    );

    document.undo().unwrap();
    assert_eq!(document.layer_count(), 2);
    assert_eq!(document.layer_name(1).unwrap(), "gear");
    assert_eq!(
        document.composite_pixels(0).unwrap(),
        vec![1, 4, 0, 0, 0, 0, 0, 0]
    );

    document.flatten().unwrap();
    assert_eq!(document.layer_count(), 1);
    assert_eq!(document.layer_name(0).unwrap(), DEFAULT_LAYER_NAME);
    assert_eq!(document.active_layer(), 0);
    assert_eq!(
        document.composite_pixels(0).unwrap(),
        vec![1, 4, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(document.frame_pixels(0).unwrap(), &[1, 4, 0, 0, 0, 0, 0, 0]);
    assert!(!document.save_project().contains("\"layers\""));
    document.undo().unwrap();
    assert_eq!(document.layer_name(1).unwrap(), "gear");
    assert_eq!(document.layer_pixels(0, 0).unwrap()[0], 1);
    assert_eq!(document.layer_pixels(0, 1).unwrap()[1], 4);
}

#[test]
fn tool_api_lists_selects_and_previews_one_layer_or_the_composite() {
    let mut document = doc();
    let added = tool(&mut document, "addLayer", json!({"name": " gear "}));
    assert_eq!(added["index"], json!(1));
    assert_eq!(added["name"], json!("gear"));
    tool(&mut document, "setActiveLayer", json!({"index": 0}));
    tool(
        &mut document,
        "fillRect",
        json!({"x": 0, "y": 0, "width": 2, "height": 1, "colorIndex": 1}),
    );
    tool(
        &mut document,
        "fillRect",
        json!({"layer": 1, "x": 1, "y": 0, "width": 1, "height": 2, "colorIndex": 2}),
    );
    assert_eq!(document.active_layer(), 0);
    assert_eq!(document.layer_pixels(0, 0).unwrap()[1], 1);
    assert_eq!(document.layer_pixels(0, 0).unwrap()[5], 0);
    assert_eq!(document.layer_pixels(0, 1).unwrap()[1], 2);
    assert_eq!(document.layer_pixels(0, 1).unwrap()[5], 2);

    let listed = tool(&mut document, "listLayers", json!({}));
    assert_eq!(listed["layerCount"], json!(2));
    assert_eq!(listed["layers"][1]["name"], json!("gear"));

    let inspect = tool(&mut document, "inspect", json!({}));
    assert_eq!(inspect["pixels"][1], json!(1));
    assert_eq!(inspect["composite"][1], json!(2));
    assert_eq!(inspect["composite"][5], json!(2));

    let composite = tool(&mut document, "previewFrame", json!({}));
    assert_eq!(composite["composite"], json!(true));
    assert_eq!(composite["layer"], json!(null));
    let only = tool(&mut document, "previewFrame", json!({"layer": 0}));
    assert_eq!(only["composite"], json!(false));
    assert_eq!(only["layer"], json!(0));
    assert_ne!(composite["png"], only["png"]);

    let diff = tool(
        &mut document,
        "diffFrames",
        json!({"frame": 0, "with": 0, "layer": 1}),
    );
    assert_eq!(diff["changed"], json!(0));
    assert_eq!(diff["composite"], json!(false));

    assert_eq!(
        tool_err(&mut document, "addLayer", json!({"name": "  "}))["code"],
        json!("LayerName")
    );
    assert_eq!(
        tool_err(&mut document, "reorderLayer", json!({"from": 0, "to": 3}))["code"],
        json!("LayerIndex")
    );
    assert_eq!(
        tool_err(
            &mut document,
            "paint",
            json!({"x": 0, "y": 0, "colorIndex": 1, "layer": 4})
        )["code"],
        json!("LayerIndex")
    );
}

#[test]
fn duplicate_resize_and_a_second_frame_keep_the_stack() {
    let mut document = doc();
    document.fill_rect(0, 0, 1, 1, 1).unwrap();
    document.add_layer(Some("gear")).unwrap();
    document.paint(3, 1, 6).unwrap();
    let copy = document.duplicate_frame(0).unwrap();
    document.set_active_frame(copy).unwrap();
    document.paint(3, 1, 7).unwrap();
    assert_eq!(document.layer_pixels(0, 1).unwrap()[7], 6);
    assert_eq!(document.layer_pixels(1, 1).unwrap()[7], 7);
    assert_eq!(document.layer_pixels(1, 0).unwrap()[0], 1);

    document.resize_canvas(6, 2, 1, 0).unwrap();
    assert_eq!(document.layer_pixels(0, 0).unwrap()[1], 1);
    assert_eq!(document.layer_pixels(0, 1).unwrap()[1 * 6 + 4], 6);
    assert_eq!(document.layer_count(), 2);
    document.undo().unwrap();
    assert_eq!(document.width(), 4);
    assert_eq!(document.layer_pixels(0, 1).unwrap()[7], 6);
}

#[test]
fn project_file_round_trips_layers_and_rejects_a_broken_stack() {
    let mut document = doc();
    document.rename_layer(0, "body").unwrap();
    document.paint(0, 0, 1).unwrap();
    document.add_layer(Some("gear")).unwrap();
    document.paint(1, 0, 2).unwrap();
    document.add_frame();
    document.set_frame_delay(1, 80).unwrap();
    let saved = document.save_project();
    assert!(saved.contains("\"name\": \"body\""));
    assert!(saved.contains("\"name\": \"gear\""));
    assert!(saved.contains("\"activeLayer\": 1"));
    let loaded = Document::load_project(&saved).unwrap();
    assert_eq!(loaded.layer_names(), document.layer_names());
    assert_eq!(loaded.active_layer(), 1);
    assert_eq!(
        loaded.layer_pixels(0, 0).unwrap(),
        document.layer_pixels(0, 0).unwrap()
    );
    assert_eq!(
        loaded.layer_pixels(0, 1).unwrap(),
        document.layer_pixels(0, 1).unwrap()
    );
    assert_eq!(loaded.layer_pixels(1, 0).unwrap(), &[0; 8]);
    assert_eq!(loaded.frame_delay(1).unwrap(), 80);
    assert_eq!(loaded.save_project(), saved);

    let broken = |body: &str| match Document::load_project(body).unwrap_err() {
        KernelError::ProjectFile(error) => error,
        other => panic!("expected ProjectFile, got {other:?}"),
    };
    let palette = r#"[{"r":0,"g":0,"b":0,"a":0},{"r":1,"g":1,"b":1,"a":255}]"#;
    assert_eq!(
        broken(&format!(
            r#"{{"format":"pixelforge-project","version":1,"width":1,"height":1,"activeFrame":0,"palette":{palette},"layers":[{{"name":"body"}},{{"name":"gear"}}],"frames":[{{"pixels":[1]}}]}}"#
        )),
        ProjectFileError::LayerCount {
            frame: 0,
            expected: 2,
            actual: 1
        }
    );
    assert_eq!(
        broken(&format!(
            r#"{{"format":"pixelforge-project","version":1,"width":1,"height":1,"activeFrame":0,"activeLayer":2,"palette":{palette},"layers":[{{"name":"body"}}],"frames":[{{"pixels":[0]}}]}}"#
        )),
        ProjectFileError::ActiveLayer { index: 2, len: 1 }
    );
    assert_eq!(
        broken(&format!(
            r#"{{"format":"pixelforge-project","version":1,"width":1,"height":1,"activeFrame":0,"palette":{palette},"layers":[{{"name":"  "}}],"frames":[{{"pixels":[0]}}]}}"#
        )),
        ProjectFileError::LayerName { index: 0 }
    );
    assert_eq!(
        broken(&format!(
            r#"{{"format":"pixelforge-project","version":1,"width":1,"height":1,"activeFrame":0,"palette":{palette},"frames":[{{"pixels":[1],"layers":[{{"pixels":[1]}}]}}]}}"#
        )),
        ProjectFileError::LayerPixels { frame: 0 }
    );
}
