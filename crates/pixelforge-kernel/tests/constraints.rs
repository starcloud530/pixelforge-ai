//! Follow, attach, and pin constraints. Solving is explicit.

use pixelforge_kernel::{Document, KernelError, ProjectFileError};
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

/// Shoulder, gun, hand, and a planted foot on a 16×16 canvas.
///
/// Rest: gun origin `(4, 4)`, butt `(-2, 0)` on the shoulder joint `(2, 4)`,
/// grip `(2, 0)` at canvas `(6, 4)`, hand palm on that grip, foot sole at
/// `(3, 11)`.
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
        .set_curve_key(
            "gun",
            pixelforge_kernel::CurveChannel::Rotation,
            0,
            0.0,
            None,
        )
        .unwrap();
    document
        .set_curve_key(
            "gun",
            pixelforge_kernel::CurveChannel::Rotation,
            4,
            90.0,
            None,
        )
        .unwrap();
    document.clear_history();
    document
}

fn arm(document: &mut Document) {
    tool(
        document,
        "addConstraint",
        json!({
            "name": "hand-grip",
            "kind": "follow",
            "part": "hand",
            "anchor": "palm",
            "target": "gun",
            "targetAnchor": "grip"
        }),
    );
    let stuck = tool(
        document,
        "addConstraint",
        json!({
            "name": "butt-shoulder",
            "kind": "stick",
            "part": "gun",
            "anchor": "butt",
            "target": "shoulder",
            "targetAnchor": "joint"
        }),
    );
    assert_eq!(stuck["kind"], json!("attach"));
    tool(
        document,
        "addConstraint",
        json!({
            "name": "plant-foot",
            "kind": "pin",
            "part": "foot",
            "anchor": "sole",
            "x": 4,
            "y": 11
        }),
    );
}

fn assert_raised(document: &Document) {
    let gun = document
        .parts()
        .iter()
        .find(|part| part.name == "gun")
        .unwrap();
    let hand = document
        .parts()
        .iter()
        .find(|part| part.name == "hand")
        .unwrap();
    let foot = document
        .parts()
        .iter()
        .find(|part| part.name == "foot")
        .unwrap();
    let shoulder = document
        .parts()
        .iter()
        .find(|part| part.name == "shoulder")
        .unwrap();
    assert_eq!((gun.x, gun.y), (2, 6));
    assert_eq!(gun.anchor_canvas("butt"), Some((2, 4)));
    assert_eq!(gun.anchor_canvas("grip"), Some((2, 8)));
    assert_eq!((hand.x, hand.y), (2, 8));
    assert_eq!(hand.anchor_canvas("palm"), Some((2, 8)));
    assert_eq!((foot.x, foot.y), (4, 10));
    assert_eq!(foot.anchor_canvas("sole"), Some((4, 11)));
    assert_eq!((shoulder.x, shoulder.y), (2, 4));
    assert_eq!(cell(document, 0, 0, 2, 6), 1);
    assert_eq!(cell(document, 0, 0, 2, 4), 2);
    assert_eq!(cell(document, 0, 0, 2, 8), 3);
    assert_eq!(cell(document, 0, 0, 4, 4), 0);
    assert_eq!(cell(document, 0, 0, 6, 4), 0);
    assert_eq!(cell(document, 0, 1, 2, 8), 4);
    assert_eq!(cell(document, 0, 1, 6, 4), 0);
    assert_eq!(cell(document, 0, 2, 4, 11), 5);
    assert_eq!(cell(document, 0, 2, 3, 11), 0);
}

#[test]
fn follow_waits_for_solve_after_the_gun_turns() {
    let mut document = Document::new(16, 16).unwrap();
    document.add_layer(Some("hand")).unwrap();
    paint_at(&mut document, 0, 4, 4, 1);
    paint_at(&mut document, 0, 6, 5, 2);
    paint_at(&mut document, 1, 6, 5, 3);
    document.add_part("gun", 4, 4, Some(0)).unwrap();
    document.set_anchor("gun", "grip", 2, 1).unwrap();
    document.add_part("hand", 6, 5, Some(1)).unwrap();
    document.set_anchor("hand", "palm", 0, 0).unwrap();
    tool(
        &mut document,
        "addConstraint",
        json!({
            "name": "hand-grip",
            "kind": "follow",
            "part": "hand",
            "anchor": "palm",
            "target": "gun",
            "targetAnchor": "grip"
        }),
    );
    document.clear_history();
    tool(
        &mut document,
        "rotatePart",
        json!({"name": "gun", "degrees": 90}),
    );
    assert_eq!(document.parts()[1].x, 6);
    assert_eq!(document.parts()[1].y, 5);
    assert_eq!(cell(&document, 0, 1, 6, 5), 3);
    assert_eq!(document.parts()[0].anchor_canvas("grip"), Some((3, 6)));
    assert_eq!(cell(&document, 0, 0, 3, 6), 2);
    assert_eq!(cell(&document, 0, 0, 4, 4), 1);

    let dry = tool(&mut document, "solveConstraints", json!({}));
    assert_eq!(dry["frame"], Value::Null);
    assert_eq!(dry["apply"], json!(false));
    assert_eq!(dry["wrote"], json!(false));
    assert_eq!(dry["order"], json!(["hand-grip"]));
    assert_eq!(dry["constraints"][0]["dx"], json!(-3));
    assert_eq!(dry["constraints"][0]["dy"], json!(1));
    assert_eq!(dry["constraints"][0]["from"], json!({"x": 6, "y": 5}));
    assert_eq!(dry["constraints"][0]["to"], json!({"x": 3, "y": 6}));
    assert_eq!(dry["constraints"][0]["satisfied"], json!(false));
    assert_eq!(document.parts()[1].x, 6);
    assert!(!document.can_undo() || cell(&document, 0, 1, 6, 5) == 3);

    let solved = tool(&mut document, "solveConstraints", json!({"apply": true}));
    assert_eq!(solved["wrote"], json!(true));
    assert_eq!(solved["parts"][1]["x"], json!(3));
    assert_eq!(solved["parts"][1]["y"], json!(6));
    assert_eq!(solved["parts"][1]["ink"], json!("moved"));
    assert_eq!(solved["parts"][0]["ink"], json!("unchanged"));
    assert_eq!(cell(&document, 0, 1, 3, 6), 3);
    assert_eq!(cell(&document, 0, 1, 6, 5), 0);
    assert_eq!(document.parts()[1].anchor_canvas("palm"), Some((3, 6)));
    document.undo().unwrap();
    assert_eq!(document.parts()[1].x, 6);
    assert_eq!(cell(&document, 0, 1, 6, 5), 3);
}

#[test]
fn attach_follow_and_pin_compose_with_a_curve_or_a_stored_pose() {
    let mut document = soldier();
    assert!(tool(&mut document, "scene", json!({}))["constraints"]
        .as_array()
        .unwrap()
        .is_empty());
    arm(&mut document);
    let listed = tool(&mut document, "listConstraints", json!({}));
    assert_eq!(listed["constraints"].as_array().unwrap().len(), 3);
    assert_eq!(listed["constraints"][1]["kind"], json!("attach"));
    assert_eq!(listed["constraints"][1]["anchor"], json!("butt"));
    assert_eq!(listed["constraints"][2]["x"], json!(4));
    assert_eq!(listed["constraints"][2]["target"], Value::Null);
    document.clear_history();

    let rest = tool(&mut document, "solveConstraints", json!({"apply": true}));
    assert_eq!(rest["frame"], Value::Null);
    assert_eq!(rest["wrote"], json!(true));
    assert_eq!(rest["constraints"][0]["satisfied"], json!(true));
    assert_eq!(rest["constraints"][1]["satisfied"], json!(true));
    assert_eq!(rest["constraints"][2]["dx"], json!(1));
    assert_eq!(rest["constraints"][2]["dy"], json!(0));
    assert_eq!(document.parts()[1].x, 4);
    assert_eq!(document.parts()[3].x, 4);
    assert_eq!(cell(&document, 0, 0, 4, 4), 1);
    assert_eq!(cell(&document, 0, 2, 4, 11), 5);
    document.undo().unwrap();
    assert_eq!(document.parts()[3].x, 3);
    assert_eq!(cell(&document, 0, 2, 3, 11), 5);

    let dry = tool(&mut document, "solveConstraints", json!({"frame": 4}));
    assert_eq!(dry["frame"], json!(4));
    assert_eq!(dry["pixelFrame"], json!(0));
    assert_eq!(dry["apply"], json!(false));
    assert_eq!(dry["wrote"], json!(false));
    assert_eq!(
        dry["order"],
        json!(["butt-shoulder", "hand-grip", "plant-foot"])
    );
    assert_eq!(dry["constraints"][0]["index"], json!(0));
    assert_eq!(dry["constraints"][0]["dx"], json!(-4));
    assert_eq!(dry["constraints"][0]["dy"], json!(4));
    assert_eq!(dry["constraints"][1]["dx"], json!(-2));
    assert_eq!(dry["constraints"][1]["dy"], json!(2));
    assert_eq!(dry["constraints"][1]["to"], json!({"x": 2, "y": 4}));
    assert_eq!(dry["constraints"][2]["dx"], json!(1));
    assert_eq!(dry["parts"][1]["x"], json!(2));
    assert_eq!(dry["parts"][1]["y"], json!(6));
    assert_eq!(dry["parts"][1]["anchors"][0]["name"], json!("butt"));
    assert_eq!(dry["parts"][1]["anchors"][0]["canvasX"], json!(2));
    assert_eq!(dry["parts"][1]["anchors"][0]["canvasY"], json!(4));
    assert_eq!(dry["parts"][1]["anchors"][1]["canvasX"], json!(2));
    assert_eq!(dry["parts"][1]["anchors"][1]["canvasY"], json!(8));
    assert_eq!(dry["parts"][2]["ink"], json!("moved"));
    assert_eq!(dry["parts"][0]["ink"], json!("unchanged"));
    assert_eq!(document.parts()[1].x, 4);
    assert_eq!(cell(&document, 0, 0, 6, 4), 3);
    assert!(!document.can_undo());

    let posed = tool(
        &mut document,
        "solveConstraints",
        json!({"frame": 4, "apply": true}),
    );
    assert_eq!(posed["wrote"], json!(true));
    assert_raised(&document);
    assert_eq!(cell(&document, 1, 0, 4, 4), 0);
    document.undo().unwrap();
    assert_eq!(cell(&document, 0, 0, 4, 4), 1);

    tool(
        &mut document,
        "rotatePart",
        json!({"name": "gun", "degrees": 90}),
    );
    assert_eq!(document.parts()[2].x, 6);
    assert_eq!(cell(&document, 0, 1, 6, 4), 4);
    let again = tool(&mut document, "solveConstraints", json!({"apply": true}));
    assert_eq!(again["frame"], Value::Null);
    assert_raised(&document);
    let held = tool(&mut document, "solveConstraints", json!({"apply": true}));
    assert_eq!(held["wrote"], json!(false));
    assert_eq!(held["constraints"][0]["satisfied"], json!(true));
    assert_eq!(held["constraints"][1]["satisfied"], json!(true));
    assert_eq!(held["constraints"][2]["satisfied"], json!(true));
    document.undo().unwrap();
    assert_eq!(document.parts()[1].x, 4);
    assert_eq!(document.parts()[2].x, 6);
}

#[test]
fn a_second_curve_solve_stacks_and_half_cells_snap_away_from_zero() {
    let mut document = Document::new(16, 16).unwrap();
    document.add_frame();
    document.add_frame();
    paint_at(&mut document, 0, 4, 4, 1);
    paint_at(&mut document, 0, 6, 5, 2);
    document.add_part("gun", 4, 4, Some(0)).unwrap();
    document.set_anchor("gun", "grip", 2, 1).unwrap();
    document
        .set_curve_key(
            "gun",
            pixelforge_kernel::CurveChannel::Rotation,
            0,
            90.0,
            None,
        )
        .unwrap();
    document.clear_history();
    document.solve_constraints(Some(0.0), true, None).unwrap();
    assert_eq!(document.parts()[0].anchor_canvas("grip"), Some((3, 6)));
    assert_eq!(cell(&document, 0, 0, 3, 6), 2);
    document.solve_constraints(Some(0.0), true, None).unwrap();
    assert_eq!(document.parts()[0].x, 4);
    assert_eq!(document.parts()[0].y, 4);
    assert_eq!(document.parts()[0].anchor_canvas("grip"), Some((2, 3)));
    assert_eq!(cell(&document, 0, 0, 2, 3), 2);

    let mut slide = Document::new(16, 16).unwrap();
    slide.add_frame();
    slide.add_frame();
    slide.add_part("gun", 4, 4, None).unwrap();
    slide
        .set_curve_key(
            "gun",
            pixelforge_kernel::CurveChannel::TranslateX,
            0,
            0.0,
            None,
        )
        .unwrap();
    slide
        .set_curve_key(
            "gun",
            pixelforge_kernel::CurveChannel::TranslateX,
            2,
            1.0,
            None,
        )
        .unwrap();
    let posed = slide.solve_constraints(Some(1.0), true, None).unwrap();
    assert_eq!(posed.parts[0].x, 5);
    assert_eq!(posed.parts[0].ink, "metadata");
    assert_eq!(slide.parts()[0].x, 5);

    let mut back = Document::new(16, 16).unwrap();
    back.add_frame();
    back.add_frame();
    back.add_part("gun", 4, 4, None).unwrap();
    back.set_curve_key(
        "gun",
        pixelforge_kernel::CurveChannel::TranslateX,
        0,
        0.0,
        None,
    )
    .unwrap();
    back.set_curve_key(
        "gun",
        pixelforge_kernel::CurveChannel::TranslateX,
        2,
        -1.0,
        None,
    )
    .unwrap();
    let posed = back.solve_constraints(Some(1.0), false, None).unwrap();
    assert!(!posed.wrote);
    assert_eq!(posed.parts[0].x, 3);
    assert_eq!(back.parts()[0].x, 4);
}

#[test]
fn solver_failures_leave_the_rig_where_it_was() {
    let mut document = soldier();
    arm(&mut document);
    document.clear_history();
    document.add_part("elbow", 1, 1, None).unwrap();
    document.set_anchor("elbow", "tip", 0, 0).unwrap();
    document.add_part("wrist", 2, 1, None).unwrap();
    document.set_anchor("wrist", "tip", 0, 0).unwrap();
    tool(
        &mut document,
        "addConstraint",
        json!({
            "name": "elbow-wrist",
            "kind": "follow",
            "part": "elbow",
            "target": "wrist",
            "targetAnchor": "tip"
        }),
    );
    let err = tool_err(
        &mut document,
        "addConstraint",
        json!({
            "name": "wrist-elbow",
            "kind": "follow",
            "part": "wrist",
            "target": "elbow",
            "targetAnchor": "tip"
        }),
    );
    assert_eq!(err["code"], json!("ConstraintCycle"));
    assert!(err["message"].as_str().unwrap().contains("cycle"));
    assert_eq!(document.constraints().len(), 4);
    tool(
        &mut document,
        "removeConstraint",
        json!({"name": "elbow-wrist"}),
    );

    let clash = tool_err(
        &mut document,
        "addConstraint",
        json!({
            "name": "hand-again",
            "kind": "follow",
            "part": "hand",
            "target": "gun",
            "targetAnchor": "grip"
        }),
    );
    assert_eq!(clash["code"], json!("ConstraintConflict"));
    assert!(clash["message"].as_str().unwrap().contains("hand"));
    let off = tool_err(
        &mut document,
        "addConstraint",
        json!({"name": "off", "kind": "pin", "part": "foot", "x": -1, "y": 0}),
    );
    assert_eq!(off["code"], json!("PartOffCanvas"));
    let kind = tool_err(
        &mut document,
        "addConstraint",
        json!({"name": "nope", "kind": "ik", "part": "foot", "x": 0, "y": 0}),
    );
    assert_eq!(kind["code"], json!("InvalidParams"));
    assert_eq!(document.constraints().len(), 3);

    document.remove_anchor("gun", "grip").unwrap();
    document.clear_history();
    let missing = tool_err(&mut document, "solveConstraints", json!({"apply": true}));
    assert_eq!(missing["code"], json!("AnchorMissing"));
    assert_eq!(document.parts()[1].x, 4);
    assert_eq!(cell(&document, 0, 0, 6, 4), 3);
    assert!(!document.can_undo());
    document.set_anchor("gun", "grip", 2, 0).unwrap();

    let mut follow = Document::new(8, 8).unwrap();
    follow.add_part("gun", 0, 0, None).unwrap();
    follow.set_anchor("gun", "grip", 0, 0).unwrap();
    follow.add_part("hand", 0, 0, None).unwrap();
    follow.set_anchor("hand", "palm", 1, 0).unwrap();
    follow
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
    let pushed = follow.solve_constraints(None, true, None).unwrap_err();
    assert!(matches!(pushed, KernelError::PartOffCanvas { x: -1, y: 0 }));
    assert_eq!(follow.parts()[1].x, 0);

    let mut masked = Document::new(8, 8).unwrap();
    masked.paint(1, 1, 1).unwrap();
    masked.add_part("foot", 1, 1, Some(0)).unwrap();
    masked
        .add_constraint(
            "plant",
            pixelforge_kernel::ConstraintKind::Pin,
            "foot",
            None,
            None,
            None,
            Some(3),
            Some(1),
        )
        .unwrap();
    masked.set_selection(1, 1, 1, 1).unwrap();
    masked.clear_history();
    let blocked = masked.solve_constraints(None, true, None).unwrap_err();
    assert!(matches!(blocked, KernelError::SelectionProtected));
    assert_eq!(masked.parts()[0].x, 1);
    assert_eq!(cell(&masked, 0, 0, 1, 1), 1);
    assert!(!masked.can_undo());

    let mut shared = Document::new(8, 8).unwrap();
    shared.add_part("gun", 1, 1, Some(0)).unwrap();
    shared.add_part("hand", 2, 1, Some(0)).unwrap();
    shared
        .add_constraint(
            "gun-pin",
            pixelforge_kernel::ConstraintKind::Pin,
            "gun",
            None,
            None,
            None,
            Some(2),
            Some(1),
        )
        .unwrap();
    shared
        .add_constraint(
            "hand-pin",
            pixelforge_kernel::ConstraintKind::Pin,
            "hand",
            None,
            None,
            None,
            Some(3),
            Some(1),
        )
        .unwrap();
    let layer = shared.solve_constraints(None, true, None).unwrap_err();
    assert!(matches!(layer, KernelError::ConstraintLayer { layer: 0 }));
    assert_eq!(shared.parts()[0].x, 1);

    let mut mark = Document::new(8, 8).unwrap();
    mark.paint(1, 1, 4).unwrap();
    mark.add_part("badge", 1, 1, None).unwrap();
    mark.add_constraint(
        "park",
        pixelforge_kernel::ConstraintKind::Pin,
        "badge",
        None,
        None,
        None,
        Some(2),
        Some(1),
    )
    .unwrap();
    let parked = mark.solve_constraints(None, true, None).unwrap();
    assert_eq!(parked.parts[0].ink, "metadata");
    assert_eq!(mark.parts()[0].x, 2);
    assert_eq!(cell(&mark, 0, 0, 1, 1), 4);
}

#[test]
fn an_empty_pixel_frame_can_turn_while_ink_on_another_frame_cannot() {
    let mut document = Document::new(8, 8).unwrap();
    document.add_frame();
    document.paint(0, 1, 1).unwrap();
    document.add_part("gun", 0, 0, Some(0)).unwrap();
    document.set_anchor("gun", "tip", 0, 1).unwrap();
    document
        .set_curve_key(
            "gun",
            pixelforge_kernel::CurveChannel::Rotation,
            0,
            90.0,
            None,
        )
        .unwrap();
    document.clear_history();
    let blocked = document
        .solve_constraints(Some(0.0), true, Some(0))
        .unwrap_err();
    assert!(matches!(
        blocked,
        KernelError::PartOffCanvas { x: -1, y: 0 }
    ));
    assert_eq!(cell(&document, 0, 0, 0, 1), 1);
    assert_eq!(document.parts()[0].anchors[0].x, 0);
    let turned = document
        .solve_constraints(Some(0.0), true, Some(1))
        .unwrap();
    assert!(turned.wrote);
    assert_eq!(document.parts()[0].anchors[0].x, -1);
    assert_eq!(document.parts()[0].anchors[0].y, 0);
    assert_eq!(cell(&document, 0, 0, 0, 1), 1);
    assert_eq!(cell(&document, 1, 0, 0, 1), 0);
}

#[test]
fn a_moving_part_carries_every_ink_cell_on_its_layer() {
    let mut document = Document::new(16, 16).unwrap();
    paint_at(&mut document, 0, 4, 4, 1);
    paint_at(&mut document, 0, 1, 1, 2);
    document.add_part("gun", 4, 4, Some(0)).unwrap();
    document.add_part("badge", 1, 1, Some(0)).unwrap();
    document
        .set_curve_key(
            "gun",
            pixelforge_kernel::CurveChannel::Rotation,
            0,
            90.0,
            None,
        )
        .unwrap();
    let posed = document.solve_constraints(Some(0.0), true, None).unwrap();
    assert_eq!(posed.parts[0].ink, "moved");
    assert_eq!(posed.parts[1].ink, "unchanged");
    assert_eq!(document.parts()[1].x, 1);
    assert_eq!(cell(&document, 0, 0, 4, 4), 1);
    assert_eq!(cell(&document, 0, 0, 1, 1), 0);
    assert_eq!(cell(&document, 0, 0, 7, 1), 2);
}

#[test]
fn project_files_omit_an_empty_list_and_reject_a_bad_constraint() {
    let mut document = soldier();
    arm(&mut document);
    let saved = document.save_project();
    assert!(saved.contains("\"constraints\""));
    assert!(!saved.contains("\"stick\""));
    assert!(saved.contains("\"attach\""));
    let loaded = Document::load_project(&saved).unwrap();
    assert_eq!(loaded.constraints().len(), 3);
    assert_eq!(
        loaded.constraints()[1].kind,
        pixelforge_kernel::ConstraintKind::Attach
    );
    let posed = loaded
        .clone()
        .solve_constraints(Some(4.0), false, None)
        .unwrap();
    assert_eq!(posed.parts[1].x, 2);
    assert_eq!(posed.parts[2].y, 8);

    let bare = Document::new(4, 4).unwrap().save_project();
    assert!(!bare.contains("\"constraints\""));
    assert_eq!(
        bare.contains("\"version\": 1") || bare.contains("\"version\":1"),
        true
    );

    let mut saved: Value = serde_json::from_str(&document.save_project()).unwrap();
    saved["constraints"][0]["expression"] = json!(1);
    match Document::load_project(&saved.to_string()).unwrap_err() {
        KernelError::ProjectFile(ProjectFileError::Malformed) => {}
        other => panic!("expected malformed, got {other:?}"),
    }
    let mut saved: Value = serde_json::from_str(&document.save_project()).unwrap();
    saved["constraints"][1]["kind"] = json!("stick");
    match Document::load_project(&saved.to_string()).unwrap_err() {
        KernelError::ProjectFile(ProjectFileError::ConstraintKind { index: 1 }) => {}
        other => panic!("expected ConstraintKind, got {other:?}"),
    }
    let mut saved: Value = serde_json::from_str(&document.save_project()).unwrap();
    saved["constraints"][2]["x"] = json!(-1);
    match Document::load_project(&saved.to_string()).unwrap_err() {
        KernelError::ProjectFile(ProjectFileError::ConstraintPin { index: 2 }) => {}
        other => panic!("expected ConstraintPin, got {other:?}"),
    }
    let mut saved: Value = serde_json::from_str(&document.save_project()).unwrap();
    saved["constraints"][0]["target"] = json!("hand");
    saved["constraints"][0]["targetAnchor"] = json!("palm");
    match Document::load_project(&saved.to_string()).unwrap_err() {
        KernelError::ProjectFile(ProjectFileError::ConstraintCycle { .. }) => {}
        other => panic!("expected ConstraintCycle, got {other:?}"),
    }
    let mut saved: Value = serde_json::from_str(&document.save_project()).unwrap();
    let parts = saved["parts"].as_array_mut().unwrap();
    let gun = parts.iter_mut().find(|part| part["name"] == "gun").unwrap();
    gun["anchors"]
        .as_array_mut()
        .unwrap()
        .retain(|anchor| anchor["name"] != "grip");
    let mut loaded = Document::load_project(&saved.to_string()).unwrap();
    assert_eq!(
        loaded.solve_constraints(None, false, None).unwrap_err(),
        KernelError::AnchorMissing
    );
}

#[test]
fn rename_remove_and_resize_keep_constraints_with_the_parts() {
    let mut document = soldier();
    arm(&mut document);
    tool(
        &mut document,
        "setPart",
        json!({"name": "gun", "rename": "rifle"}),
    );
    assert_eq!(document.constraints()[0].target.as_deref(), Some("rifle"));
    assert_eq!(document.constraints()[1].part, "rifle");
    tool(&mut document, "removePart", json!({"name": "rifle"}));
    assert_eq!(document.constraints().len(), 1);
    assert_eq!(document.constraints()[0].name, "plant-foot");
    document.undo().unwrap();
    assert_eq!(document.constraints().len(), 3);

    let mut pins = Document::new(8, 8).unwrap();
    pins.add_part("foot", 1, 1, None).unwrap();
    pins.add_part("mark", 1, 2, None).unwrap();
    pins.add_constraint(
        "stay",
        pixelforge_kernel::ConstraintKind::Pin,
        "foot",
        None,
        None,
        None,
        Some(6),
        Some(1),
    )
    .unwrap();
    pins.add_constraint(
        "edge",
        pixelforge_kernel::ConstraintKind::Pin,
        "mark",
        None,
        None,
        None,
        Some(7),
        Some(1),
    )
    .unwrap();
    pins.resize_canvas(8, 8, 1, 0).unwrap();
    assert_eq!(pins.parts()[0].x, 2);
    assert_eq!(pins.constraints().len(), 1);
    assert_eq!(pins.constraints()[0].name, "stay");
    assert_eq!(pins.constraints()[0].x, Some(7));
    pins.undo().unwrap();
    assert_eq!(pins.constraints().len(), 2);
    assert_eq!(pins.constraints()[1].x, Some(7));
}
