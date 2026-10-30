//! Direction layout: four cardinals, optional diagonals, tag segments.

use pixelforge_kernel::{Document, LoopMode, TOOL_API_VERSION};
use serde_json::{json, Value};

fn tool(document: &mut Document, name: &str, params: Value) -> Value {
    let request = json!({
        "apiVersion": TOOL_API_VERSION,
        "tool": name,
        "params": params,
    });
    let response: Value = serde_json::from_str(&document.call_tool(&request.to_string())).unwrap();
    assert!(response["ok"].as_bool().unwrap(), "{response}");
    response["result"].clone()
}

fn tool_err(document: &mut Document, name: &str, params: Value) -> Value {
    let request = json!({
        "apiVersion": TOOL_API_VERSION,
        "tool": name,
        "params": params,
    });
    let response: Value = serde_json::from_str(&document.call_tool(&request.to_string())).unwrap();
    assert!(!response["ok"].as_bool().unwrap(), "{response}");
    response["error"].clone()
}

fn frames(document: &mut Document, count: usize) {
    for _ in 1..count {
        document.add_frame();
    }
}

fn tag(document: &mut Document, name: &str, from: usize, to: usize) {
    document.add_tag(name, from, to, LoopMode::Forward).unwrap();
}

fn standard_four(document: &mut Document) {
    frames(document, 16);
    let facings = ["down", "left", "right", "up"];
    for (row, facing) in facings.iter().enumerate() {
        let start = row * 4;
        tag(document, &format!("idle_{facing}"), start, start);
        tag(document, &format!("walk_{facing}"), start + 1, start + 3);
    }
}

#[test]
fn four_direction_idle_walk_passes_and_a_missing_facing_is_a_hard_error() {
    let mut document = Document::new(16, 16).unwrap();
    standard_four(&mut document);
    let set = tool(
        &mut document,
        "setDirectionLayout",
        json!({"count": 4, "columns": 4}),
    );
    assert_eq!(set["count"], json!(4));
    assert_eq!(set["wrote"], json!(true));
    let report = tool(&mut document, "validateDirections", json!({}));
    assert_eq!(report["errorCount"], json!(0), "{report}");
    assert_eq!(report["facings"].as_array().unwrap().len(), 4);
    assert_eq!(report["facings"][0]["compass"], json!("S"));
    assert_eq!(report["facings"][0]["screen"], json!("down"));
    assert_eq!(report["facings"][3]["compass"], json!("N"));
    assert_eq!(report["facings"][3]["from"], json!(12));
    assert_eq!(report["facings"][3]["to"], json!(15));

    document.remove_tag("walk_up").unwrap();
    let missing = tool(&mut document, "validateDirections", json!({}));
    assert!(missing["errorCount"].as_u64().unwrap() >= 1);
    let codes: Vec<&str> = missing["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|error| error["code"].as_str().unwrap())
        .collect();
    assert!(codes.contains(&"missingDirection"), "{missing}");
    assert!(missing["errors"][0]["message"]
        .as_str()
        .unwrap()
        .contains("walk_up"));
}

#[test]
fn compass_alias_counts_and_apply_does_not_paint() {
    let mut document = Document::new(8, 8).unwrap();
    frames(&mut document, 4);
    tag(&mut document, "idle_south", 0, 0);
    tag(&mut document, "idle_w", 1, 1);
    tag(&mut document, "idle_east", 2, 2);
    tag(&mut document, "idle_n", 3, 3);
    let before = document.pixels().to_vec();
    tool(
        &mut document,
        "setDirectionLayout",
        json!({"count": 4, "columns": 1}),
    );
    let report = tool(&mut document, "validateDirections", json!({}));
    assert_eq!(report["errorCount"], json!(0), "{report}");
    assert_eq!(document.pixels(), before);

    let err = tool_err(
        &mut document,
        "setDirectionLayout",
        json!({"count": 6, "columns": 1}),
    );
    assert_eq!(err["code"], json!("DirectionLayout"));
}

#[test]
fn apply_names_the_standard_split_and_undo_restores_it() {
    let mut document = Document::new(8, 8).unwrap();
    frames(&mut document, 8);
    let wrote = tool(
        &mut document,
        "setDirectionLayout",
        json!({"count": 4, "columns": 2, "apply": true}),
    );
    assert_eq!(wrote["wrote"], json!(true));
    let names: Vec<&str> = document
        .tags()
        .iter()
        .map(|tag| tag.name.as_str())
        .collect();
    assert_eq!(
        names,
        vec![
            "idle_down",
            "walk_down",
            "idle_left",
            "walk_left",
            "idle_right",
            "walk_right",
            "idle_up",
            "walk_up",
        ]
    );
    assert_eq!(document.tags()[1].from, 1);
    assert_eq!(document.tags()[1].to, 1);
    let report = tool(&mut document, "validateDirections", json!({}));
    assert_eq!(report["errorCount"], json!(0), "{report}");

    document.undo().unwrap();
    assert!(document.tags().is_empty());
    assert!(document.direction_layout().is_none());
    assert_eq!(document.frame_count(), 8);
}

#[test]
fn eight_directions_require_diagonals_and_export_one_row() {
    let mut document = Document::new(4, 4).unwrap();
    frames(&mut document, 8);
    let facings = [
        "down",
        "left",
        "right",
        "up",
        "downleft",
        "downright",
        "upleft",
        "upright",
    ];
    for (row, facing) in facings.iter().enumerate() {
        tag(&mut document, &format!("idle_{facing}"), row, row);
    }
    tool(
        &mut document,
        "setDirectionLayout",
        json!({"count": 8, "columns": 1}),
    );
    let report = tool(&mut document, "validateDirections", json!({}));
    assert_eq!(report["errorCount"], json!(0), "{report}");
    assert_eq!(report["facings"][4]["compass"], json!("SW"));
    assert_eq!(report["facings"][7]["compass"], json!("NE"));

    document.remove_tag("idle_upright").unwrap();
    let missing = tool(&mut document, "validateDirections", json!({}));
    assert!(missing["errorCount"].as_u64().unwrap() >= 1);

    document
        .add_tag("idle_upright", 7, 7, LoopMode::Forward)
        .unwrap();
    let sheet = tool(
        &mut document,
        "exportDirection",
        json!({"facing": "NE", "scale": 1}),
    );
    assert_eq!(sheet["compass"], json!("NE"));
    assert_eq!(sheet["facing"], json!("upright"));
    assert_eq!(sheet["from"], json!(7));
    assert_eq!(sheet["to"], json!(7));
    assert_eq!(sheet["columns"], json!(1));
    assert_eq!(sheet["rows"], json!(1));
    assert!(sheet["png"].as_str().unwrap().starts_with("iVBOR"));

    let refused = tool_err(
        &mut document,
        "exportDirection",
        json!({"facing": "sideways"}),
    );
    assert_eq!(refused["code"], json!("DirectionFacing"));
}

#[test]
fn other_actions_must_cover_every_facing_with_the_same_columns() {
    let mut document = Document::new(8, 8).unwrap();
    frames(&mut document, 8);
    let facings = ["down", "left", "right", "up"];
    for (row, facing) in facings.iter().enumerate() {
        let start = row * 2;
        tag(&mut document, &format!("idle_{facing}"), start, start);
        tag(
            &mut document,
            &format!("walk_{facing}"),
            start + 1,
            start + 1,
        );
        if *facing != "up" {
            tag(&mut document, &format!("attack_{facing}"), start, start);
        }
    }
    tool(
        &mut document,
        "setDirectionLayout",
        json!({"count": 4, "columns": 2}),
    );
    let missing = tool(&mut document, "validateDirections", json!({}));
    let messages: Vec<&str> = missing["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|error| error["message"].as_str().unwrap())
        .collect();
    assert!(
        messages.iter().any(|message| message.contains("attack_up")),
        "{messages:?}"
    );

    tag(&mut document, "attack_up", 6, 7);
    let span = tool(&mut document, "validateDirections", json!({}));
    assert!(
        span["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error["code"] == "segment" && error["action"] == "attack"),
        "{span}"
    );
}

#[test]
fn a_diagonal_tag_on_a_four_direction_layout_is_an_error() {
    let mut document = Document::new(8, 8).unwrap();
    frames(&mut document, 4);
    let facings = ["down", "left", "right", "up"];
    for (row, facing) in facings.iter().enumerate() {
        tag(&mut document, &format!("idle_{facing}"), row, row);
    }
    tag(&mut document, "idle_sw", 0, 0);
    tool(
        &mut document,
        "setDirectionLayout",
        json!({"count": 4, "columns": 1}),
    );
    let report = tool(&mut document, "validateDirections", json!({}));
    assert!(
        report["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error["code"] == "segment" && error["compass"] == "SW"),
        "{report}"
    );
}

#[test]
fn layout_round_trips_and_is_omitted_when_clear() {
    let mut document = Document::new(8, 8).unwrap();
    frames(&mut document, 4);
    tool(
        &mut document,
        "setDirectionLayout",
        json!({"count": 4, "columns": 1}),
    );
    let saved = document.save_project();
    assert!(saved.contains("directionLayout"));
    assert!(saved.contains("\"count\": 4"));
    let loaded = Document::load_project(&saved).unwrap();
    assert_eq!(loaded.direction_layout().unwrap().count, 4);
    assert_eq!(loaded.direction_layout().unwrap().columns, 1);

    let mut loaded = loaded;
    tool(&mut loaded, "clearDirectionLayout", json!({}));
    let cleared = loaded.save_project();
    assert!(!cleared.contains("directionLayout"));
    let scene = tool(&mut loaded, "scene", json!({}));
    assert!(scene["directionLayout"].is_null());
}

#[test]
fn short_sheet_is_a_frames_error_and_apply_writes_nothing() {
    let mut document = Document::new(8, 8).unwrap();
    let err = tool_err(
        &mut document,
        "setDirectionLayout",
        json!({"count": 4, "columns": 4, "apply": true}),
    );
    assert_eq!(err["code"], json!("DirectionFrames"));
    assert!(document.direction_layout().is_none());
    assert!(document.tags().is_empty());

    tool(
        &mut document,
        "setDirectionLayout",
        json!({"count": 4, "columns": 4}),
    );
    let report = tool(&mut document, "validateDirections", json!({}));
    let codes: Vec<&str> = report["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|error| error["code"].as_str().unwrap())
        .collect();
    assert!(codes.contains(&"frames"), "{report}");
    assert!(codes.contains(&"missingDirection"), "{report}");
}
