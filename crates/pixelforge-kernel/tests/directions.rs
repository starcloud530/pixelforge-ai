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

fn paint_frame(document: &mut Document, frame: u32, x: u32, y: u32, color: u32) {
    tool(document, "setActiveFrame", json!({"index": frame}));
    tool(
        document,
        "paint",
        json!({"x": x, "y": y, "colorIndex": color}),
    );
}

fn cell(x: usize, y: usize) -> usize {
    y * 8 + x
}

#[test]
fn fill_mirrors_left_into_right_skips_ink_and_undoes_together() {
    let mut document = Document::new(8, 4).unwrap();
    frames(&mut document, 16);
    tool(
        &mut document,
        "setDirectionLayout",
        json!({"count": 4, "columns": 4, "apply": true}),
    );
    paint_frame(&mut document, 4, 1, 0, 1);
    tool(
        &mut document,
        "setFrameDelay",
        json!({"index": 5, "delayMs": 120}),
    );
    paint_frame(&mut document, 5, 0, 1, 2);
    assert_eq!(document.active_frame(), 5);

    let report = tool(
        &mut document,
        "fillDirections",
        json!({"from": "west", "mode": "mirror"}),
    );
    assert_eq!(report["mode"], json!("mirror"));
    assert_eq!(report["from"], json!("left"));
    assert_eq!(report["compass"], json!("W"));
    assert_eq!(report["overwrite"], json!(false));
    assert_eq!(report["diagonals"], json!("none"));
    assert_eq!(report["wrote"], json!(true));
    assert_eq!(report["framesWritten"], json!(2));
    assert_eq!(report["framesSkipped"], json!(0));
    assert_eq!(report["targets"][0]["facing"], json!("right"));
    assert_eq!(report["targets"][0]["hflip"], json!(true));
    assert_eq!(report["targets"][0]["frames"][0]["frame"], json!(8));
    assert_eq!(report["targets"][0]["frames"][0]["source"], json!(4));
    assert_eq!(report["targets"][0]["frames"][0]["action"], json!("idle"));
    assert_eq!(
        report["targets"][0]["frames"][0]["status"],
        json!("written")
    );
    assert_eq!(report["targets"][0]["frames"][1]["action"], json!("walk"));
    assert_eq!(
        report["targets"][0]["frames"][1]["status"],
        json!("written")
    );
    assert_eq!(
        report["targets"][0]["frames"][2]["status"],
        json!("unchanged")
    );
    assert_eq!(document.active_frame(), 5);
    assert_eq!(document.frame_pixels(8).unwrap()[cell(6, 0)], 1);
    assert_eq!(document.frame_pixels(8).unwrap()[cell(1, 0)], 0);
    assert_eq!(document.frame_pixels(9).unwrap()[cell(7, 1)], 2);
    assert_eq!(document.frame_delay(9).unwrap(), 120);
    assert_eq!(document.frame_pixels(4).unwrap()[cell(1, 0)], 1);
    assert!(document
        .frame_pixels(0)
        .unwrap()
        .iter()
        .all(|index| *index == 0));
    assert!(document
        .frame_pixels(12)
        .unwrap()
        .iter()
        .all(|index| *index == 0));

    document.undo().unwrap();
    assert!(document
        .frame_pixels(8)
        .unwrap()
        .iter()
        .all(|index| *index == 0));
    assert!(document
        .frame_pixels(9)
        .unwrap()
        .iter()
        .all(|index| *index == 0));
    assert_eq!(document.frame_delay(9).unwrap(), 100);
    assert_eq!(document.frame_pixels(4).unwrap()[cell(1, 0)], 1);
    document.redo().unwrap();
    assert_eq!(document.frame_pixels(8).unwrap()[cell(6, 0)], 1);
    assert_eq!(document.frame_delay(9).unwrap(), 120);

    paint_frame(&mut document, 8, 0, 0, 3);
    let skipped = tool(
        &mut document,
        "fillDirections",
        json!({"from": "left", "mode": "mirror"}),
    );
    assert_eq!(skipped["wrote"], json!(false));
    assert_eq!(skipped["framesSkipped"], json!(2));
    assert_eq!(document.frame_pixels(8).unwrap()[cell(0, 0)], 3);
    assert_eq!(document.frame_pixels(8).unwrap()[cell(6, 0)], 1);

    let replaced = tool(
        &mut document,
        "fillDirections",
        json!({"from": "left", "mode": "mirror", "overwrite": true}),
    );
    assert_eq!(replaced["from"], json!("left"));
    assert_eq!(replaced["targets"][0]["facing"], json!("right"));
    assert_eq!(
        replaced["targets"][0]["frames"][0]["status"],
        json!("written")
    );
    assert_eq!(document.frame_pixels(8).unwrap()[cell(0, 0)], 0);
    assert_eq!(document.frame_pixels(8).unwrap()[cell(6, 0)], 1);
    assert_eq!(document.frame_pixels(4).unwrap()[cell(1, 0)], 1);

    let passed = tool(&mut document, "validateDirections", json!({}));
    assert_eq!(passed["errorCount"], json!(0), "{passed}");
}

#[test]
fn fill_copy_repeats_cardinals_without_flipping() {
    let mut document = Document::new(8, 4).unwrap();
    frames(&mut document, 16);
    tool(
        &mut document,
        "setDirectionLayout",
        json!({"count": 4, "columns": 4, "apply": true}),
    );
    tool(
        &mut document,
        "setFrameDelay",
        json!({"index": 0, "delayMs": 200}),
    );
    paint_frame(&mut document, 0, 1, 0, 4);

    let report = tool(
        &mut document,
        "fillDirections",
        json!({"from": "S", "mode": "copy"}),
    );
    assert_eq!(report["mode"], json!("copy"));
    assert_eq!(report["from"], json!("down"));
    assert_eq!(report["diagonals"], json!("none"));
    assert_eq!(report["framesWritten"], json!(3));
    let facings: Vec<&str> = report["targets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|target| target["facing"].as_str().unwrap())
        .collect();
    assert_eq!(facings, vec!["left", "right", "up"]);
    assert!(report["targets"]
        .as_array()
        .unwrap()
        .iter()
        .all(|target| target["hflip"] == false));
    for frame in [4u32, 8, 12] {
        assert_eq!(
            document.frame_pixels(frame as usize).unwrap()[cell(1, 0)],
            4
        );
        assert_eq!(
            document.frame_pixels(frame as usize).unwrap()[cell(6, 0)],
            0
        );
        assert_eq!(document.frame_delay(frame as usize).unwrap(), 200);
    }
    assert_eq!(document.frame_pixels(0).unwrap()[cell(1, 0)], 4);

    document.undo().unwrap();
    for frame in [4usize, 8, 12] {
        assert!(document
            .frame_pixels(frame)
            .unwrap()
            .iter()
            .all(|index| *index == 0));
        assert_eq!(document.frame_delay(frame).unwrap(), 100);
    }
    document.redo().unwrap();

    let mirror = tool_err(
        &mut document,
        "fillDirections",
        json!({"from": "down", "mode": "mirror"}),
    );
    assert_eq!(mirror["code"], json!("DirectionFill"));
    assert!(
        mirror["message"].as_str().unwrap().contains("mirror"),
        "{mirror}"
    );
    assert_eq!(document.frame_pixels(4).unwrap()[cell(1, 0)], 4);

    let skipped = tool(
        &mut document,
        "fillDirections",
        json!({"from": "left", "mode": "mirror"}),
    );
    assert_eq!(
        skipped["targets"][0]["frames"][0]["status"],
        json!("skipped")
    );
    assert_eq!(document.frame_pixels(8).unwrap()[cell(1, 0)], 4);

    let flipped = tool(
        &mut document,
        "fillDirections",
        json!({"from": "left", "mode": "mirror", "overwrite": true}),
    );
    assert_eq!(flipped["targets"][0]["facing"], json!("right"));
    assert_eq!(document.frame_pixels(8).unwrap()[cell(6, 0)], 4);
    assert_eq!(document.frame_pixels(8).unwrap()[cell(1, 0)], 0);
    assert_eq!(document.frame_pixels(4).unwrap()[cell(1, 0)], 4);

    let passed = tool(&mut document, "validateDirections", json!({}));
    assert_eq!(passed["errorCount"], json!(0), "{passed}");
}

#[test]
fn fill_skips_diagonals_and_refuses_a_missing_source_segment() {
    let mut eight = Document::new(8, 4).unwrap();
    frames(&mut eight, 8);
    tool(
        &mut eight,
        "setDirectionLayout",
        json!({"count": 8, "columns": 1, "apply": true}),
    );
    paint_frame(&mut eight, 1, 1, 0, 1);
    let mirrored = tool(
        &mut eight,
        "fillDirections",
        json!({"from": "left", "mode": "mirror"}),
    );
    assert_eq!(mirrored["diagonals"], json!("skipped"));
    assert_eq!(mirrored["targets"].as_array().unwrap().len(), 1);
    assert_eq!(eight.frame_pixels(2).unwrap()[cell(6, 0)], 1);
    for frame in 4..8 {
        assert!(
            eight
                .frame_pixels(frame)
                .unwrap()
                .iter()
                .all(|index| *index == 0),
            "diagonal frame {frame}"
        );
    }
    let diagonal = tool_err(
        &mut eight,
        "fillDirections",
        json!({"from": "downleft", "mode": "copy"}),
    );
    assert_eq!(diagonal["code"], json!("DirectionFill"));
    assert!(
        diagonal["message"].as_str().unwrap().contains("diagonal"),
        "{diagonal}"
    );
    assert_eq!(eight.frame_pixels(2).unwrap()[cell(6, 0)], 1);
    paint_frame(&mut eight, 0, 1, 0, 4);
    let copied = tool(
        &mut eight,
        "fillDirections",
        json!({"from": "south", "mode": "copy"}),
    );
    assert_eq!(copied["from"], json!("down"));
    assert_eq!(copied["framesWritten"], json!(1));
    assert_eq!(copied["framesSkipped"], json!(2));
    assert_eq!(eight.frame_pixels(3).unwrap()[cell(1, 0)], 4);
    assert_eq!(eight.frame_pixels(1).unwrap()[cell(1, 0)], 1);
    assert_eq!(eight.frame_pixels(2).unwrap()[cell(6, 0)], 1);
    for frame in 4..8 {
        assert!(eight
            .frame_pixels(frame)
            .unwrap()
            .iter()
            .all(|index| *index == 0));
    }
    let passed = tool(&mut eight, "validateDirections", json!({}));
    assert_eq!(passed["errorCount"], json!(0), "{passed}");

    let mut sheet = Document::new(8, 4).unwrap();
    frames(&mut sheet, 16);
    tool(
        &mut sheet,
        "setDirectionLayout",
        json!({"count": 4, "columns": 4, "apply": true}),
    );
    let facings = ["down", "left", "right", "up"];
    for (row, facing) in facings.iter().enumerate() {
        let start = row * 4;
        tag(&mut sheet, &format!("attack_{facing}"), start, start);
    }
    paint_frame(&mut sheet, 4, 1, 0, 1);
    let with_attack = tool(
        &mut sheet,
        "fillDirections",
        json!({"from": "left", "mode": "mirror"}),
    );
    assert_eq!(
        with_attack["targets"][0]["frames"][0]["action"],
        json!("idle,attack")
    );
    sheet.remove_tag("attack_left").unwrap();
    let missing = tool_err(
        &mut sheet,
        "fillDirections",
        json!({"from": "left", "mode": "mirror", "overwrite": true}),
    );
    assert_eq!(missing["code"], json!("DirectionFill"));
    assert!(
        missing["message"].as_str().unwrap().contains("attack_left"),
        "{missing}"
    );
    assert_eq!(sheet.frame_pixels(8).unwrap()[cell(6, 0)], 1);

    sheet.remove_tag("walk_right").unwrap();
    let missing_walk = tool_err(
        &mut sheet,
        "fillDirections",
        json!({"from": "right", "mode": "mirror"}),
    );
    assert_eq!(missing_walk["code"], json!("DirectionFill"));
    assert!(
        missing_walk["message"]
            .as_str()
            .unwrap()
            .contains("walk_right"),
        "{missing_walk}"
    );

    let mut duplicate = Document::new(8, 4).unwrap();
    frames(&mut duplicate, 16);
    tool(
        &mut duplicate,
        "setDirectionLayout",
        json!({"count": 4, "columns": 4, "apply": true}),
    );
    tag(&mut duplicate, "walk_west", 5, 7);
    let twice = tool_err(
        &mut duplicate,
        "fillDirections",
        json!({"from": "left", "mode": "mirror"}),
    );
    assert_eq!(twice["code"], json!("DirectionSegment"));
    assert!(duplicate
        .frame_pixels(8)
        .unwrap()
        .iter()
        .all(|index| *index == 0));

    let mut bare = Document::new(4, 4).unwrap();
    let absent = tool_err(
        &mut bare,
        "fillDirections",
        json!({"from": "left", "mode": "copy"}),
    );
    assert_eq!(absent["code"], json!("DirectionMissing"));
    tool(
        &mut bare,
        "setDirectionLayout",
        json!({"count": 4, "columns": 4}),
    );
    let short = tool_err(
        &mut bare,
        "fillDirections",
        json!({"from": "left", "mode": "copy"}),
    );
    assert_eq!(short["code"], json!("DirectionFrames"));

    let mut named = Document::new(8, 4).unwrap();
    frames(&mut named, 16);
    tool(
        &mut named,
        "setDirectionLayout",
        json!({"count": 4, "columns": 4, "apply": true}),
    );
    let outside = tool_err(
        &mut named,
        "fillDirections",
        json!({"from": "sw", "mode": "copy"}),
    );
    assert_eq!(outside["code"], json!("DirectionFacing"));
    let bad_mode = tool_err(
        &mut named,
        "fillDirections",
        json!({"from": "left", "mode": "spin"}),
    );
    assert_eq!(bad_mode["code"], json!("InvalidParams"));
}

#[test]
fn fill_copies_every_layer_and_bake_and_flatten_still_run() {
    let mut document = Document::new(8, 4).unwrap();
    frames(&mut document, 16);
    tool(
        &mut document,
        "setDirectionLayout",
        json!({"count": 4, "columns": 4, "apply": true}),
    );
    tool(&mut document, "addLayer", json!({"name": "gear"}));
    tool(
        &mut document,
        "addLayer",
        json!({"name": "polish", "role": "polish"}),
    );
    tool(&mut document, "setActiveFrame", json!({"index": 4}));
    tool(
        &mut document,
        "paint",
        json!({"x": 1, "y": 0, "colorIndex": 1, "layer": 0}),
    );
    tool(
        &mut document,
        "paint",
        json!({"x": 2, "y": 0, "colorIndex": 2, "layer": 1}),
    );
    tool(&mut document, "erase", json!({"x": 3, "y": 0, "layer": 2}));
    tool(&mut document, "setActiveLayer", json!({"index": 0}));
    tool(
        &mut document,
        "addPart",
        json!({"name": "mark", "x": 3, "y": 1}),
    );
    tool(
        &mut document,
        "setSelection",
        json!({"x": 0, "y": 0, "width": 1, "height": 1}),
    );
    assert_eq!(document.active_frame(), 4);
    assert_eq!(document.active_layer(), 0);

    let report = tool(
        &mut document,
        "fillDirections",
        json!({"from": "left", "mode": "mirror"}),
    );
    assert_eq!(report["framesWritten"], json!(1));
    assert_eq!(document.active_frame(), 4);
    assert_eq!(document.active_layer(), 0);
    assert!(document.selection().is_some());
    assert_eq!(document.parts()[0].x, 3);
    assert_eq!(document.parts()[0].y, 1);
    assert_eq!(document.layer_pixels(8, 0).unwrap()[cell(6, 0)], 1);
    assert_eq!(document.layer_pixels(8, 1).unwrap()[cell(5, 0)], 2);
    assert_eq!(document.layer_erase(8, 2).unwrap()[cell(4, 0)], 1);
    assert_eq!(document.layer_pixels(4, 0).unwrap()[cell(1, 0)], 1);
    assert_eq!(document.tags().len(), 8);

    tool(&mut document, "bake", json!({}));
    assert_eq!(document.layer_pixels(8, 0).unwrap()[cell(6, 0)], 1);
    assert_eq!(document.layer_erase(8, 2).unwrap()[cell(4, 0)], 1);
    assert_eq!(document.parts()[0].x, 3);

    let flat = tool(&mut document, "flattenLayers", json!({}));
    assert_eq!(flat["layerCount"], json!(1));
    assert_eq!(document.layer_pixels(8, 0).unwrap()[cell(6, 0)], 1);
    assert_eq!(document.layer_pixels(8, 0).unwrap()[cell(5, 0)], 2);
    assert_eq!(document.layer_pixels(8, 0).unwrap()[cell(4, 0)], 0);
    assert_eq!(document.parts()[0].x, 3);
    assert_eq!(document.parts()[0].y, 1);
}
