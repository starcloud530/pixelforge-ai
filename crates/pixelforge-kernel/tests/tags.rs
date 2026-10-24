use pixelforge_kernel::{
    Document, KernelError, LoopMode, ProjectFileError, TagUpdate, DEFAULT_FRAME_DELAY_MS,
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

fn four_frames() -> Document {
    let mut document = Document::new(2, 2).unwrap();
    document.add_frame();
    document.add_frame();
    document.add_frame();
    document.set_frame_delay(1, 120).unwrap();
    document.clear_history();
    document
}

fn rejected(json_text: &str) -> ProjectFileError {
    match Document::load_project(json_text).unwrap_err() {
        KernelError::ProjectFile(error) => error,
        other => panic!("expected ProjectFile, got {other:?}"),
    }
}

#[test]
fn tag_crud_round_trips_and_omits_an_empty_list() {
    let mut document = four_frames();
    assert!(tool(&mut document, "inspect", json!({}))["tags"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(tool(&mut document, "scene", json!({}))["tags"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(!document.save_project().contains("\"tags\""));

    let added = tool(
        &mut document,
        "addTag",
        json!({"name": " walk_down ", "from": 1, "to": 3}),
    );
    assert_eq!(added["index"], json!(0));
    assert_eq!(added["name"], json!("walk_down"));
    assert_eq!(added["from"], json!(1));
    assert_eq!(added["to"], json!(3));
    assert_eq!(added["loop"], json!("forward"));
    assert!(document.can_undo());

    let idle = tool(
        &mut document,
        "addTag",
        json!({"name": "idle_down", "from": 0, "to": 0, "loop": "once"}),
    );
    assert_eq!(idle["index"], json!(1));
    assert_eq!(
        tool_err(
            &mut document,
            "addTag",
            json!({"name": "walk_down", "from": 0, "to": 0})
        )["code"],
        json!("TagExists")
    );
    assert_eq!(
        tool_err(
            &mut document,
            "addTag",
            json!({"name": "walk_up", "from": 2, "to": 1})
        )["code"],
        json!("TagRange")
    );
    assert_eq!(
        tool_err(
            &mut document,
            "addTag",
            json!({"name": " ", "from": 0, "to": 0})
        )["code"],
        json!("TagName")
    );
    assert_eq!(
        tool_err(
            &mut document,
            "addTag",
            json!({"name": "spin", "from": 0, "to": 0, "loop": "bounce"})
        )["code"],
        json!("InvalidParams")
    );

    let renamed = tool(
        &mut document,
        "setTag",
        json!({"name": "walk_down", "loop": "pingpong", "rename": "walk_south", "to": 2}),
    );
    assert_eq!(renamed["name"], json!("walk_south"));
    assert_eq!(renamed["loop"], json!("pingpong"));
    assert_eq!(renamed["from"], json!(1));
    assert_eq!(renamed["to"], json!(2));
    assert_eq!(
        tool_err(
            &mut document,
            "setTag",
            json!({"name": "walk_south", "rename": "idle_down"})
        )["code"],
        json!("TagExists")
    );
    assert_eq!(
        tool_err(
            &mut document,
            "setTag",
            json!({"name": "missing", "from": 0})
        )["code"],
        json!("TagMissing")
    );
    assert_eq!(
        tool_err(&mut document, "setTag", json!({"name": "walk_south"}))["code"],
        json!("InvalidParams")
    );
    let same = document.can_undo();
    tool(
        &mut document,
        "setTag",
        json!({"name": "walk_south", "from": 1, "to": 2, "loop": "pingpong"}),
    );
    assert_eq!(
        document.can_undo(),
        same,
        "an unchanged tag is not recorded"
    );

    let saved = document.save_project();
    assert!(saved.contains("\"name\": \"walk_south\""));
    assert!(saved.contains("\"loop\": \"pingpong\""));
    assert!(saved.contains("\"name\": \"idle_down\""));
    let loaded = Document::load_project(&saved).unwrap();
    assert_eq!(loaded.tags().len(), 2);
    assert_eq!(loaded.tags()[0].name, "walk_south");
    assert_eq!(loaded.tags()[0].loop_mode, LoopMode::PingPong);
    assert_eq!(loaded.tags()[1].loop_mode, LoopMode::Once);
    assert_eq!(loaded.save_project(), saved);

    let removed = tool(&mut document, "removeTag", json!({"name": "idle_down"}));
    assert_eq!(removed["index"], json!(1));
    assert_eq!(removed["name"], json!("idle_down"));
    assert_eq!(document.tags().len(), 1);
    document.undo().unwrap();
    assert_eq!(document.tags().len(), 2);
    document.redo().unwrap();
    assert_eq!(document.tags()[0].name, "walk_south");
}

#[test]
fn frame_edits_move_tags_and_one_undo_restores_both() {
    let mut document = four_frames();
    document
        .add_tag("idle_down", 0, 0, LoopMode::Forward)
        .unwrap();
    document
        .add_tag("walk_down", 1, 3, LoopMode::Forward)
        .unwrap();
    document.clear_history();

    let copy = document.duplicate_frame(0).unwrap();
    assert_eq!(copy, 1);
    assert_eq!(document.tags()[0].from, 0);
    assert_eq!(
        document.tags()[0].to,
        0,
        "a one-frame tag does not take the copy"
    );
    assert_eq!(document.tags()[1].from, 2);
    assert_eq!(document.tags()[1].to, 4);
    document.undo().unwrap();
    assert_eq!(document.frame_count(), 4);
    assert_eq!(document.tags()[1].from, 1);
    assert_eq!(document.tags()[1].to, 3);
    assert!(!document.can_undo());

    document.duplicate_frame(2).unwrap();
    assert_eq!(document.tags()[1].from, 1);
    assert_eq!(document.tags()[1].to, 4, "an interior copy grows the tag");
    document.undo().unwrap();
    assert_eq!(document.tags()[1].to, 3);

    document.add_frame();
    assert_eq!(document.frame_count(), 5);
    assert_eq!(
        document.tags()[1].to,
        3,
        "appending past the tag does not grow it"
    );
    document.undo().unwrap();

    document.delete_frame(0).unwrap();
    assert!(document.tags().iter().all(|tag| tag.name != "idle_down"));
    assert_eq!(document.tags()[0].name, "walk_down");
    assert_eq!(document.tags()[0].from, 0);
    assert_eq!(document.tags()[0].to, 2);
    document.undo().unwrap();
    assert_eq!(document.tags()[0].name, "idle_down");
    assert_eq!(document.frame_count(), 4);

    document.delete_frame(2).unwrap();
    assert_eq!(document.tags()[1].from, 1);
    assert_eq!(document.tags()[1].to, 2);
    document.undo().unwrap();
    assert_eq!(document.frame_delay(1).unwrap(), 120);
    assert_eq!(document.frame_delay(0).unwrap(), DEFAULT_FRAME_DELAY_MS);
}

#[test]
fn a_gesture_that_adds_and_renames_a_tag_undoes_as_one_step() {
    let mut document = Document::new(2, 2).unwrap();
    document.begin_gesture().unwrap();
    document.add_tag("idle", 0, 0, LoopMode::Forward).unwrap();
    document
        .set_tag(
            "idle",
            TagUpdate {
                from: None,
                to: None,
                loop_mode: Some(LoopMode::Reverse),
                rename: Some("idle_down".to_string()),
            },
        )
        .unwrap();
    document.end_gesture();
    assert_eq!(document.tags()[0].name, "idle_down");
    assert_eq!(document.tags()[0].loop_mode, LoopMode::Reverse);
    document.undo().unwrap();
    assert!(document.tags().is_empty());
    assert!(!document.can_undo());
}

#[test]
fn atlas_is_opt_in_and_uses_sheet_pixels() {
    let mut document = four_frames();
    document
        .add_tag("walk_down", 1, 3, LoopMode::Forward)
        .unwrap();
    let plain = tool(
        &mut document,
        "exportSpriteSheet",
        json!({"columns": 4, "scale": 1}),
    );
    assert!(plain.get("atlas").is_none());
    assert_eq!(plain["columns"], json!(4));
    assert_eq!(plain["rows"], json!(1));
    assert!(document.can_undo(), "tag add is still the only undo");
    let before = document.can_undo();

    let sheet = tool(
        &mut document,
        "exportSpriteSheet",
        json!({"columns": 3, "scale": 2, "atlas": true}),
    );
    assert_eq!(
        document.can_undo(),
        before,
        "atlas export is not an undo entry"
    );
    assert_eq!(sheet["columns"], json!(3));
    assert_eq!(sheet["rows"], json!(2));
    assert_eq!(sheet["scale"], json!(2));
    assert_eq!(sheet["sheetWidth"], json!(12));
    assert_eq!(sheet["sheetHeight"], json!(8));
    let atlas = &sheet["atlas"];
    assert_eq!(atlas["format"], json!("pixelforge-atlas"));
    assert_eq!(atlas["version"], json!(1));
    assert_eq!(atlas["columns"], json!(3));
    assert_eq!(atlas["rows"], json!(2));
    assert_eq!(atlas["scale"], json!(2));
    assert_eq!(atlas["frameWidth"], json!(4));
    assert_eq!(atlas["frameHeight"], json!(4));
    assert_eq!(atlas["frames"].as_array().unwrap().len(), 4);
    assert_eq!(atlas["frames"][1]["x"], json!(4));
    assert_eq!(atlas["frames"][1]["y"], json!(0));
    assert_eq!(atlas["frames"][1]["width"], json!(4));
    assert_eq!(atlas["frames"][1]["delayMs"], json!(120));
    assert_eq!(atlas["frames"][3]["x"], json!(0));
    assert_eq!(atlas["frames"][3]["y"], json!(4));
    assert_eq!(atlas["tags"][0]["name"], json!("walk_down"));
    assert_eq!(atlas["tags"][0]["from"], json!(1));
    assert_eq!(atlas["tags"][0]["to"], json!(3));
    assert_eq!(atlas["tags"][0]["loop"], json!("forward"));
    assert!(sheet["png"].as_str().unwrap().starts_with("iVBORw0KGgoAAA"));

    let off = tool(&mut document, "exportSpriteSheet", json!({"atlas": false}));
    assert!(off.get("atlas").is_none());
    assert_eq!(
        tool_err(&mut document, "exportSpriteSheet", json!({"atlas": "yes"}))["code"],
        json!("InvalidParams")
    );
}

#[test]
fn project_file_rejects_a_bad_tag_and_defaults_a_missing_loop() {
    let ok = r#"{"format":"pixelforge-project","version":1,"width":1,"height":1,"activeFrame":0,"palette":[{"r":0,"g":0,"b":0,"a":0}],"frames":[{"pixels":[0],"delayMs":100},{"pixels":[0],"delayMs":100}],"tags":[{"name":"idle","from":0,"to":1}]}"#;
    let loaded = Document::load_project(ok).unwrap();
    assert_eq!(loaded.tags()[0].loop_mode, LoopMode::Forward);
    assert!(!loaded.save_project().contains("\"loop\": null"));
    assert!(loaded.save_project().contains("\"loop\": \"forward\""));

    let base = r#"{"format":"pixelforge-project","version":1,"width":1,"height":1,"activeFrame":0,"palette":[{"r":0,"g":0,"b":0,"a":0}],"frames":[{"pixels":[0],"delayMs":100}],"tags":"#;
    assert_eq!(
        rejected(&format!(
            r#"{base}[{{"name":"idle","from":0,"to":0,"loop":"bounce"}}]}}"#
        )),
        ProjectFileError::TagLoop { index: 0 }
    );
    assert_eq!(
        rejected(&format!(
            r#"{base}[{{"name":"idle","from":1,"to":0,"loop":"forward"}}]}}"#
        )),
        ProjectFileError::TagRange { index: 0 }
    );
    assert_eq!(
        rejected(&format!(
            r#"{base}[{{"name":" ","from":0,"to":0,"loop":"forward"}}]}}"#
        )),
        ProjectFileError::TagName { index: 0 }
    );
    assert_eq!(
        rejected(&format!(
            r#"{base}[{{"name":"idle","from":0,"to":0}},{{"name":"idle","from":0,"to":0}}]}}"#
        )),
        ProjectFileError::TagDuplicate { index: 1 }
    );
    assert!(matches!(
        rejected(&format!(
            r#"{base}[{{"name":"idle","from":0,"to":0,"color":1}}]}}"#
        )),
        ProjectFileError::Malformed
    ));
}
