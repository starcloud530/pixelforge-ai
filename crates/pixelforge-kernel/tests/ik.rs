//! Short-chain IK and footprint spans. A miss does not move the rig.

use pixelforge_kernel::{ConstraintKind, Contact, CurveChannel, Document};
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

fn paint(document: &mut Document, layer: usize, x: u16, y: u16, color: u8) {
    document.set_active_layer(layer).unwrap();
    document.paint(x, y, color).unwrap();
}

fn anchor(parts: &[pixelforge_kernel::Part], name: &str, anchor: &str) -> (i32, i32) {
    let part = parts.iter().find(|part| part.name == name).unwrap();
    let anchor = part
        .anchors
        .iter()
        .find(|item| item.name == anchor)
        .unwrap();
    (
        i32::from(part.x) + i32::from(anchor.x),
        i32::from(part.y) + i32::from(anchor.y),
    )
}

/// Thigh down 4, shin down 4, sole at the shin tip. Each joint has its own layer.
fn leg() -> Document {
    let mut document = Document::new(24, 24).unwrap();
    document.add_layer(Some("shin")).unwrap();
    document.add_part("thigh", 4, 4, Some(0)).unwrap();
    document.add_part("shin", 4, 8, Some(1)).unwrap();
    document.set_anchor("shin", "sole", 0, 4).unwrap();
    paint(&mut document, 0, 4, 4, 1);
    paint(&mut document, 0, 4, 7, 1);
    paint(&mut document, 1, 4, 8, 2);
    paint(&mut document, 1, 4, 12, 2);
    document
}

#[test]
fn a_two_bone_chain_hits_the_target_cell_or_refuses() {
    let mut document = leg();
    tool(
        &mut document,
        "addConstraint",
        json!({
            "name": "left-leg",
            "kind": "ik",
            "part": "shin",
            "anchor": "sole",
            "chain": ["thigh", "shin"],
            "bend": 1,
            "x": 8,
            "y": 8
        }),
    );
    let solved = tool(&mut document, "solveConstraints", json!({"apply": true}));
    assert_eq!(solved["constraints"][0]["to"], json!({"x": 8, "y": 8}));
    assert_eq!(solved["constraints"][0]["planted"], json!(false));
    assert_eq!(anchor(document.parts(), "shin", "sole"), (8, 8));
    let shin = document
        .parts()
        .iter()
        .find(|part| part.name == "shin")
        .unwrap();
    assert_eq!((shin.x, shin.y), (4, 8));
    let mut other = leg();
    tool(
        &mut other,
        "addConstraint",
        json!({
            "name": "left-leg",
            "kind": "ik",
            "part": "shin",
            "anchor": "sole",
            "chain": ["thigh", "shin"],
            "bend": -1,
            "x": 8,
            "y": 8
        }),
    );
    tool(&mut other, "solveConstraints", json!({"apply": true}));
    let shin = other
        .parts()
        .iter()
        .find(|part| part.name == "shin")
        .unwrap();
    assert_eq!((shin.x, shin.y), (8, 4));
    assert_eq!(anchor(other.parts(), "shin", "sole"), (8, 8));
    assert_eq!(cell(&document, 0, 1, 4, 12), 0);
    assert_eq!(cell(&document, 0, 1, 8, 8), 2);

    let straight = leg();
    let mut straight = straight;
    tool(
        &mut straight,
        "addConstraint",
        json!({
            "name": "left-leg",
            "kind": "ik",
            "part": "shin",
            "anchor": "sole",
            "chain": ["thigh", "shin"],
            "x": 4,
            "y": 12
        }),
    );
    let solved = tool(&mut straight, "solveConstraints", json!({"apply": false}));
    assert_eq!(solved["wrote"], json!(false));
    assert_eq!(solved["constraints"][0]["satisfied"], json!(true));
    assert_eq!(anchor(straight.parts(), "shin", "sole"), (4, 12));

    let err = tool_err(
        &mut document,
        "addConstraint",
        json!({
            "name": "too-far",
            "kind": "ik",
            "part": "shin",
            "anchor": "sole",
            "chain": ["thigh", "shin"],
            "x": 20,
            "y": 20
        }),
    );
    assert_eq!(err["code"], json!("ConstraintConflict"));

    let mut missed = leg();
    tool(
        &mut missed,
        "addConstraint",
        json!({
            "name": "too-far",
            "kind": "ik",
            "part": "shin",
            "anchor": "sole",
            "chain": ["thigh", "shin"],
            "x": 20,
            "y": 20
        }),
    );
    let parts = missed.parts().to_vec();
    missed.clear_history();
    let err = tool_err(&mut missed, "solveConstraints", json!({"apply": true}));
    assert_eq!(err["code"], json!("IkUnreachable"));
    assert!(err["message"].as_str().unwrap().contains("20,20"));
    assert_eq!(missed.parts(), parts.as_slice());
    assert_eq!(cell(&missed, 0, 1, 4, 12), 2);
    assert!(!missed.can_undo());
}

#[test]
fn a_three_joint_chain_places_the_end_origin() {
    let mut document = Document::new(24, 24).unwrap();
    document.add_layer(Some("knee")).unwrap();
    document.add_layer(Some("ankle")).unwrap();
    document.add_part("hip", 4, 2, None).unwrap();
    document.add_part("knee", 4, 6, Some(1)).unwrap();
    document.add_part("ankle", 4, 10, Some(2)).unwrap();
    paint(&mut document, 1, 4, 6, 3);
    paint(&mut document, 2, 4, 10, 4);
    tool(
        &mut document,
        "addConstraint",
        json!({
            "name": "leg",
            "kind": "ik",
            "part": "ankle",
            "chain": ["hip", "knee", "ankle"],
            "bend": 1,
            "x": 8,
            "y": 6
        }),
    );
    tool(&mut document, "solveConstraints", json!({"apply": true}));
    let ankle = document
        .parts()
        .iter()
        .find(|part| part.name == "ankle")
        .unwrap();
    assert_eq!((ankle.x, ankle.y), (8, 6));
    assert_eq!(cell(&document, 0, 2, 4, 10), 0);
    assert_eq!(cell(&document, 0, 2, 8, 6), 4);
}

#[test]
fn footprint_spans_pin_before_bake_and_do_not_slide() {
    let mut document = Document::new(24, 24).unwrap();
    document.add_layer(Some("shin")).unwrap();
    document.add_part("thigh", 6, 6, Some(0)).unwrap();
    document.add_part("shin", 10, 6, Some(1)).unwrap();
    document.set_anchor("shin", "sole", 0, 4).unwrap();
    paint(&mut document, 0, 6, 6, 1);
    paint(&mut document, 1, 10, 6, 2);
    paint(&mut document, 1, 10, 10, 2);
    document.duplicate_frame(0).unwrap();
    document.duplicate_frame(1).unwrap();
    document
        .set_curve_key("thigh", CurveChannel::TranslateX, 0, 0.0, None)
        .unwrap();
    document
        .set_curve_key("thigh", CurveChannel::TranslateX, 2, 2.0, None)
        .unwrap();
    document
        .set_curve_key("shin", CurveChannel::TranslateX, 0, 0.0, None)
        .unwrap();
    document
        .set_curve_key("shin", CurveChannel::TranslateX, 2, 3.0, None)
        .unwrap();
    tool(
        &mut document,
        "addConstraint",
        json!({
            "name": "left-sole",
            "kind": "ik",
            "part": "shin",
            "anchor": "sole",
            "chain": ["thigh", "shin"],
            "bend": 1,
            "contacts": [
                {"frame": 0, "to": 1, "x": 10, "y": 10},
                {"frame": 2, "x": 12, "y": 10}
            ]
        }),
    );
    let held = tool(
        &mut document,
        "solveConstraints",
        json!({"frame": 1, "apply": false}),
    );
    assert_eq!(held["constraints"][0]["planted"], json!(true));
    assert_eq!(held["constraints"][0]["to"], json!({"x": 10, "y": 10}));

    let baked = tool(
        &mut document,
        "bake",
        json!({"from": 0, "to": 2, "fps": 12}),
    );
    assert_eq!(baked["wrote"], json!(true));
    assert_eq!(cell(&document, 0, 1, 10, 10), 2);
    assert_eq!(cell(&document, 1, 1, 10, 10), 2);
    assert_eq!(cell(&document, 1, 1, 12, 10), 0);
    assert_eq!(cell(&document, 2, 1, 12, 10), 2);
    assert_eq!(cell(&document, 2, 1, 10, 10), 0);
    assert_eq!(document.constraints()[0].kind, ConstraintKind::Ik);
    assert_eq!(document.curves().len(), 2);

    let mut boot = Document::new(16, 16).unwrap();
    boot.add_frame();
    boot.add_part("boot", 4, 12, Some(0)).unwrap();
    boot.set_anchor("boot", "sole", 0, 0).unwrap();
    paint(&mut boot, 0, 4, 12, 5);
    boot.duplicate_frame(0).unwrap();
    boot.set_curve_key("boot", CurveChannel::TranslateX, 0, 0.0, None)
        .unwrap();
    boot.set_curve_key("boot", CurveChannel::TranslateX, 1, 4.0, None)
        .unwrap();
    boot.add_constraint_full(
        "plant-boot",
        ConstraintKind::Plant,
        "boot",
        Some("sole"),
        None,
        None,
        None,
        None,
        &[],
        None,
        &[Contact {
            frame: 0,
            to: 1,
            x: 4,
            y: 12,
        }],
    )
    .unwrap();
    let solved = boot.solve_constraints(Some(1.0), true, Some(1)).unwrap();
    assert!(solved.constraints[0].planted);
    assert_eq!(solved.constraints[0].to_x, 4);
    assert_eq!(cell(&boot, 1, 0, 4, 12), 5);
    assert_eq!(cell(&boot, 1, 0, 8, 12), 0);

    boot.remove_constraint("plant-boot").unwrap();
    boot.solve_constraints(Some(1.0), true, Some(1)).unwrap();
    assert_eq!(cell(&boot, 1, 0, 8, 12), 5);
}

#[test]
fn ik_composes_with_follow_and_a_miss_does_not_bake() {
    let mut document = leg();
    document.add_layer(Some("hand")).unwrap();
    document.add_part("grip", 10, 4, None).unwrap();
    document.set_anchor("grip", "point", 0, 0).unwrap();
    document.add_part("hand", 10, 6, Some(2)).unwrap();
    document.set_anchor("hand", "palm", 0, 0).unwrap();
    paint(&mut document, 2, 10, 6, 6);
    document.duplicate_frame(0).unwrap();
    tool(
        &mut document,
        "addConstraint",
        json!({
            "name": "left-leg",
            "kind": "ik",
            "part": "shin",
            "anchor": "sole",
            "chain": ["thigh", "shin"],
            "contacts": [{"frame": 1, "x": 4, "y": 12}]
        }),
    );
    tool(
        &mut document,
        "addConstraint",
        json!({
            "name": "hand-grip",
            "kind": "follow",
            "part": "hand",
            "anchor": "palm",
            "target": "grip",
            "targetAnchor": "point"
        }),
    );
    tool(
        &mut document,
        "solveConstraints",
        json!({"frame": 1, "apply": true, "pixelFrame": 1}),
    );
    assert_eq!(anchor(document.parts(), "shin", "sole"), (4, 12));
    let hand = document
        .parts()
        .iter()
        .find(|part| part.name == "hand")
        .unwrap();
    assert_eq!((hand.x, hand.y), (10, 4));
    assert_eq!(cell(&document, 1, 2, 10, 4), 6);

    document.remove_constraint("left-leg").unwrap();
    tool(
        &mut document,
        "addConstraint",
        json!({
            "name": "left-leg",
            "kind": "ik",
            "part": "shin",
            "anchor": "sole",
            "chain": ["thigh", "shin"],
            "x": 20,
            "y": 4
        }),
    );
    let err = tool_err(&mut document, "bake", json!({"from": 0, "to": 1}));
    assert_eq!(err["code"], json!("IkUnreachable"));
    assert_eq!(cell(&document, 1, 1, 4, 12), 2);
}

#[test]
fn footprint_keys_shift_with_frames_and_round_trip() {
    let mut document = leg();
    document.duplicate_frame(0).unwrap();
    document
        .add_constraint_full(
            "left-sole",
            ConstraintKind::Plant,
            "shin",
            Some("sole"),
            None,
            None,
            None,
            None,
            &[],
            None,
            &[Contact {
                frame: 0,
                to: 1,
                x: 4,
                y: 12,
            }],
        )
        .unwrap();
    document.duplicate_frame(0).unwrap();
    assert_eq!(document.constraints()[0].contacts[0].to, 2);
    document.delete_frame(2).unwrap();
    assert_eq!(document.constraints()[0].contacts[0].to, 1);
    let saved = document.save_project();
    assert!(saved.contains("\"plant\""));
    assert!(saved.contains("\"contacts\""));
    let loaded = Document::load_project(&saved).unwrap();
    assert_eq!(loaded.constraints()[0].kind, ConstraintKind::Plant);
    assert_eq!(loaded.constraints()[0].contacts[0].x, 4);
    document.undo().unwrap();
    assert_eq!(document.constraints()[0].contacts[0].to, 2);
}
