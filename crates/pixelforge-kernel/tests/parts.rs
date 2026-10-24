use pixelforge_kernel::{
    Document, KernelError, LayerAttach, PartUpdate, ProjectFileError, QuarterTurn,
};

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

fn rejected(json_text: &str) -> ProjectFileError {
    match Document::load_project(json_text).unwrap_err() {
        KernelError::ProjectFile(error) => error,
        other => panic!("expected ProjectFile, got {other:?}"),
    }
}

fn project_with_parts(parts: &str) -> String {
    format!(
        r#"{{"format":"pixelforge-project","version":1,"width":4,"height":4,"activeFrame":0,"palette":[{{"r":0,"g":0,"b":0,"a":0}}],"frames":[{{"pixels":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]}}],"parts":{parts}}}"#
    )
}

#[test]
fn soldier_gun_and_grip_round_trip_without_writing_pixels() {
    let mut document = Document::new(48, 32).unwrap();
    document.rename_layer(0, "body").unwrap();
    document.fill_rect(16, 6, 10, 18, 3).unwrap();
    let gear = document.add_layer(Some("gear")).unwrap();
    assert_eq!(gear, 1);
    document.fill_rect_on(1, 24, 12, 14, 4, 1).unwrap();
    let before = document.layer_pixels(0, 1).unwrap().to_vec();
    document.clear_history();

    assert!(tool(&mut document, "inspect", json!({}))["parts"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(tool(&mut document, "listParts", json!({}))["parts"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(!document.save_project().contains("\"parts\""));

    let gun = tool(
        &mut document,
        "addPart",
        json!({"name": " gun ", "x": 24, "y": 12, "layer": 1}),
    );
    assert_eq!(gun["name"], json!("gun"));
    assert_eq!(gun["x"], json!(24));
    assert_eq!(gun["y"], json!(12));
    assert_eq!(gun["layer"], json!(1));
    assert!(gun["anchors"].as_array().unwrap().is_empty());

    let shoulder = tool(
        &mut document,
        "addPart",
        json!({"name": "shoulder", "x": 22, "y": 12}),
    );
    assert!(shoulder["layer"].is_null());
    tool(
        &mut document,
        "addPart",
        json!({"name": "elbow", "x": 26, "y": 16}),
    );

    let grip = tool(
        &mut document,
        "setAnchor",
        json!({"part": "gun", "name": " grip ", "x": 2, "y": 2}),
    );
    assert_eq!(grip["part"], json!("gun"));
    assert_eq!(grip["name"], json!("grip"));
    assert_eq!(grip["x"], json!(2));
    assert_eq!(grip["y"], json!(2));
    assert_eq!(grip["partIndex"], json!(0));
    assert_eq!(grip["index"], json!(0));
    tool(
        &mut document,
        "setAnchor",
        json!({"part": "shoulder", "name": "joint", "x": 0, "y": 0}),
    );
    tool(
        &mut document,
        "setAnchor",
        json!({"part": "elbow", "name": "joint", "x": 0, "y": 0}),
    );

    assert_eq!(document.parts()[0].anchor_canvas("grip"), Some((26, 14)));
    assert_eq!(document.layer_pixels(0, 1).unwrap(), before.as_slice());
    assert_eq!(document.layer_pixels(0, 1).unwrap()[12 * 48 + 24], 1);

    let saved = document.save_project();
    assert!(saved.contains("\"grip\""));
    assert!(saved.contains("\"layer\": 1"));
    let loaded = Document::load_project(&saved).unwrap();
    assert_eq!(loaded.parts().len(), 3);
    assert_eq!(loaded.parts()[0].name, "gun");
    assert_eq!(loaded.parts()[0].layer, Some(1));
    assert_eq!(loaded.parts()[0].anchors[0].name, "grip");
    assert_eq!(loaded.parts()[0].anchor_canvas("grip"), Some((26, 14)));
    assert_eq!(loaded.parts()[1].layer, None);
    assert_eq!(loaded.parts()[1].anchors[0].name, "joint");
    assert_eq!(loaded.parts()[2].anchors[0].name, "joint");
    assert!(!loaded.can_undo());

    let scene = tool(&mut document, "scene", json!({}));
    assert_eq!(scene["parts"][0]["name"], json!("gun"));
    assert_eq!(scene["parts"][0]["anchors"][0]["name"], json!("grip"));
    assert!(scene.get("pixels").is_none());
}

#[test]
fn part_and_anchor_edits_undo_and_reject_bad_input() {
    let mut document = Document::new(8, 8).unwrap();
    tool(
        &mut document,
        "addPart",
        json!({"name": "gun", "x": 1, "y": 2}),
    );
    tool(
        &mut document,
        "setAnchor",
        json!({"part": "gun", "name": "grip", "x": 1, "y": -1}),
    );
    let same = document.can_undo();
    tool(
        &mut document,
        "setAnchor",
        json!({"part": "gun", "name": "grip", "x": 1, "y": -1}),
    );
    assert_eq!(document.can_undo(), same);
    tool(
        &mut document,
        "setPart",
        json!({"name": "gun", "x": 1, "y": 2}),
    );
    assert_eq!(document.can_undo(), same);

    assert_eq!(
        tool_err(
            &mut document,
            "addPart",
            json!({"name": "gun", "x": 0, "y": 0})
        )["code"],
        json!("PartExists")
    );
    assert_eq!(
        tool_err(
            &mut document,
            "addPart",
            json!({"name": " ", "x": 0, "y": 0})
        )["code"],
        json!("PartName")
    );
    assert_eq!(
        tool_err(
            &mut document,
            "addPart",
            json!({"name": "arm", "x": 8, "y": 0})
        )["code"],
        json!("OutOfBounds")
    );
    assert_eq!(
        tool_err(
            &mut document,
            "addPart",
            json!({"name": "arm", "x": 0, "y": 0, "layer": 2})
        )["code"],
        json!("LayerIndex")
    );
    assert_eq!(
        tool_err(
            &mut document,
            "setAnchor",
            json!({"part": "gun", "name": "muzzle", "x": 257, "y": 0})
        )["code"],
        json!("AnchorCoord")
    );
    assert_eq!(
        tool_err(
            &mut document,
            "setAnchor",
            json!({"part": "gun", "name": " ", "x": 0, "y": 0})
        )["code"],
        json!("AnchorName")
    );
    assert_eq!(
        tool_err(
            &mut document,
            "setAnchor",
            json!({"part": "missing", "name": "grip", "x": 0, "y": 0})
        )["code"],
        json!("PartMissing")
    );
    assert_eq!(
        tool_err(&mut document, "setPart", json!({"name": "gun"}))["code"],
        json!("InvalidParams")
    );
    assert_eq!(
        tool_err(
            &mut document,
            "setPart",
            json!({"name": "gun", "layer": 0, "clearLayer": true})
        )["code"],
        json!("InvalidParams")
    );
    assert_eq!(
        tool_err(
            &mut document,
            "removeAnchor",
            json!({"part": "gun", "name": "stock"})
        )["code"],
        json!("AnchorMissing")
    );

    let renamed = tool(
        &mut document,
        "setPart",
        json!({"name": "gun", "rename": "rifle", "x": 3}),
    );
    assert_eq!(renamed["name"], json!("rifle"));
    assert_eq!(renamed["x"], json!(3));
    assert_eq!(renamed["y"], json!(2));
    assert_eq!(renamed["anchors"][0]["x"], json!(1));

    document.undo().unwrap();
    assert_eq!(document.parts()[0].name, "gun");
    assert_eq!(document.parts()[0].x, 1);
    document.undo().unwrap();
    assert!(document.parts()[0].anchors.is_empty());
    document.undo().unwrap();
    assert!(document.parts().is_empty());
    document.redo().unwrap();
    assert_eq!(document.parts()[0].name, "gun");

    tool(
        &mut document,
        "setAnchor",
        json!({"part": "gun", "name": "grip", "x": -256, "y": 256}),
    );
    assert_eq!(document.parts()[0].anchors[0].x, -256);
    assert_eq!(document.parts()[0].anchors[0].y, 256);
    tool(
        &mut document,
        "removeAnchor",
        json!({"part": " gun ", "name": " grip "}),
    );
    assert!(document.parts()[0].anchors.is_empty());
    tool(&mut document, "removePart", json!({"name": "gun"}));
    assert!(document.parts().is_empty());
    assert!(!document.save_project().contains("\"parts\""));
}

#[test]
fn one_gesture_undoes_a_part_and_its_grip() {
    let mut document = Document::new(8, 8).unwrap();
    document.begin_gesture().unwrap();
    document.add_part("gun", 2, 3, None).unwrap();
    document.set_anchor("gun", "grip", 1, 1).unwrap();
    document.end_gesture();
    document.undo().unwrap();
    assert!(document.parts().is_empty());
    assert!(!document.can_undo());
}

#[test]
fn layer_structure_keeps_part_attachments_in_the_same_undo_step() {
    let mut document = Document::new(4, 4).unwrap();
    document.add_layer(Some("gear")).unwrap();
    document.add_layer(Some("limbs")).unwrap();
    document.clear_history();
    document.add_part("gun", 1, 1, Some(1)).unwrap();
    document.add_part("shoulder", 0, 0, None).unwrap();
    document.set_anchor("gun", "grip", 1, 0).unwrap();

    document.set_active_layer(0).unwrap();
    document.add_layer(Some("under")).unwrap();
    assert_eq!(document.parts()[0].layer, Some(2));
    assert_eq!(document.parts()[0].anchors[0].name, "grip");
    assert_eq!(document.parts()[1].layer, None);
    document.undo().unwrap();
    assert_eq!(document.parts()[0].layer, Some(1));
    assert_eq!(document.layer_count(), 3);

    document.reorder_layer(1, 0).unwrap();
    assert_eq!(document.parts()[0].layer, Some(0));
    document.undo().unwrap();
    assert_eq!(document.parts()[0].layer, Some(1));

    document.delete_layer(1).unwrap();
    assert_eq!(document.parts()[0].layer, None);
    assert_eq!(document.parts()[0].anchors[0].name, "grip");
    document.undo().unwrap();
    assert_eq!(document.parts()[0].layer, Some(1));

    document.flatten().unwrap();
    assert_eq!(document.layer_count(), 1);
    assert_eq!(document.parts()[0].layer, Some(0));
    assert_eq!(document.parts()[1].layer, None);
    document.undo().unwrap();
    assert_eq!(document.parts()[0].layer, Some(1));
    assert_eq!(document.layer_count(), 3);
}

#[test]
fn resize_moves_origins_and_flip_does_not() {
    let mut document = Document::new(4, 4).unwrap();
    document.paint(0, 0, 1).unwrap();
    document.add_part("gun", 0, 0, Some(0)).unwrap();
    document.set_anchor("gun", "grip", 1, 0).unwrap();
    document.clear_history();

    document.resize_canvas(4, 4, 1, 0).unwrap();
    assert_eq!(document.parts()[0].x, 1);
    assert_eq!(document.parts()[0].y, 0);
    assert_eq!(document.parts()[0].anchors[0].x, 1);
    assert_eq!(document.parts()[0].anchor_canvas("grip"), Some((2, 0)));
    assert_eq!(document.pixels()[1], 1);
    document.undo().unwrap();
    assert_eq!(document.parts()[0].x, 0);
    assert_eq!(document.pixels()[0], 1);

    document.resize_canvas(4, 4, -1, 0).unwrap();
    assert!(document.parts().is_empty());
    document.undo().unwrap();
    assert_eq!(document.parts()[0].name, "gun");
    assert_eq!(document.parts()[0].anchors[0].name, "grip");

    document.flip_horizontal(0, 0, 0, 4, 4).unwrap();
    document.rotate(0, 0, 0, 4, 4, QuarterTurn::Cw180).unwrap();
    assert_eq!(document.parts()[0].x, 0);
    assert_eq!(document.parts()[0].y, 0);
    assert_eq!(document.parts()[0].anchors[0].x, 1);
    assert_eq!(document.parts()[0].anchors[0].y, 0);
}

#[test]
fn set_part_can_clear_a_layer_attachment() {
    let mut document = Document::new(4, 4).unwrap();
    document.add_part("gun", 1, 1, Some(0)).unwrap();
    document
        .set_part(
            "gun",
            PartUpdate {
                x: None,
                y: None,
                layer: LayerAttach::Clear,
                rename: None,
            },
        )
        .unwrap();
    assert_eq!(document.parts()[0].layer, None);
    document
        .set_part(
            "gun",
            PartUpdate {
                x: None,
                y: None,
                layer: LayerAttach::Clear,
                rename: None,
            },
        )
        .unwrap();
    document.undo().unwrap();
    assert_eq!(document.parts()[0].layer, Some(0));
}

#[test]
fn project_file_rejects_a_bad_part_and_loads_a_file_without_parts() {
    let old = r#"{"format":"pixelforge-project","version":1,"width":1,"height":1,"activeFrame":0,"palette":[{"r":0,"g":0,"b":0,"a":0}],"frames":[{"pixels":[0]}]}"#;
    let loaded = Document::load_project(old).unwrap();
    assert!(loaded.parts().is_empty());
    assert!(!loaded.save_project().contains("\"parts\""));

    let ok = project_with_parts(
        r#"[{"name":"gun","x":1,"y":2,"anchors":[{"name":"grip","x":2,"y":-1}]}]"#,
    );
    let loaded = Document::load_project(&ok).unwrap();
    assert_eq!(loaded.parts()[0].anchor_canvas("grip"), Some((3, 1)));

    assert_eq!(
        rejected(&project_with_parts(
            r#"[{"name":"gun","x":1,"y":2},{"name":"gun","x":0,"y":0}]"#
        )),
        ProjectFileError::PartDuplicate { index: 1 }
    );
    assert_eq!(
        rejected(&project_with_parts(r#"[{"name":" ","x":0,"y":0}]"#)),
        ProjectFileError::PartName { index: 0 }
    );
    assert_eq!(
        rejected(&project_with_parts(r#"[{"name":"gun","x":4,"y":0}]"#)),
        ProjectFileError::PartOrigin { index: 0 }
    );
    assert_eq!(
        rejected(&project_with_parts(
            r#"[{"name":"gun","x":0,"y":0,"layer":1}]"#
        )),
        ProjectFileError::PartLayer { index: 0 }
    );
    assert_eq!(
        rejected(&project_with_parts(
            r#"[{"name":"gun","x":0,"y":0,"anchors":[{"name":"grip","x":0,"y":0},{"name":"grip","x":1,"y":0}]}]"#
        )),
        ProjectFileError::AnchorDuplicate { part: 0, index: 1 }
    );
    assert_eq!(
        rejected(&project_with_parts(
            r#"[{"name":"gun","x":0,"y":0,"anchors":[{"name":"grip","x":300,"y":0}]}]"#
        )),
        ProjectFileError::AnchorCoord { part: 0, index: 0 }
    );
    assert_eq!(
        rejected(&project_with_parts(
            r#"[{"name":"gun","x":0,"y":0,"rotation":90}]"#
        )),
        ProjectFileError::Malformed
    );
}
