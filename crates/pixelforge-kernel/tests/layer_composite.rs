//! Layer visibility and opacity in the index composite and the RGBA export.
//!
//! Opacity 255 covers with the palette color. Opacity 1..=254 blends in RGBA.
//! ASCII still shows the covering index. `flatten` snaps that RGBA blend onto
//! the palette. The expected pixels below are literals, not a second call into
//! the blender.

use pixelforge_kernel::{
    decode_sheet_png, Document, KernelError, ProjectFileError, LAYER_OPACITY_OPAQUE,
};
use serde_json::{json, Value};

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

fn stacked() -> Document {
    let mut document = Document::new(2, 1).unwrap();
    document.paint(0, 0, 3).unwrap();
    document.add_layer(Some("gear")).unwrap();
    document.paint(0, 0, 7).unwrap();
    document
}

fn png_rgba(bytes: &[u8]) -> Vec<u8> {
    decode_sheet_png(bytes).expect("png").rgba
}

#[test]
fn an_old_project_file_loads_visible_and_fully_opaque() {
    let loaded = Document::load_project(
        r#"{
  "format": "pixelforge-project",
  "version": 1,
  "width": 2,
  "height": 1,
  "activeFrame": 0,
  "palette": [
    { "r": 0, "g": 0, "b": 0, "a": 0 },
    { "r": 9, "g": 8, "b": 7, "a": 255 }
  ],
  "frames": [ { "pixels": [1, 0] } ]
}"#,
    )
    .unwrap();
    assert!(loaded.layer_visible(0).unwrap());
    assert_eq!(loaded.layer_opacity(0).unwrap(), LAYER_OPACITY_OPAQUE);
    assert_eq!(loaded.composite_pixels(0).unwrap(), vec![1, 0]);
    assert_eq!(
        loaded.frame_rgba(0).unwrap(),
        vec![9, 8, 7, 255, 0, 0, 0, 0]
    );
    assert!(!loaded.save_project().contains("visible"));
    assert!(!loaded.save_project().contains("opacity"));

    let layered = Document::load_project(
        r#"{
  "format": "pixelforge-project",
  "version": 1,
  "width": 1,
  "height": 1,
  "activeFrame": 0,
  "activeLayer": 1,
  "palette": [
    { "r": 0, "g": 0, "b": 0, "a": 0 },
    { "r": 10, "g": 0, "b": 0, "a": 255 },
    { "r": 0, "g": 0, "b": 10, "a": 128 }
  ],
  "layers": [ { "name": "body" }, { "name": "gear" } ],
  "frames": [ { "layers": [ { "pixels": [1] }, { "pixels": [2] } ] } ]
}"#,
    )
    .unwrap();
    assert!(layered.layer_visible(1).unwrap());
    assert_eq!(layered.layer_opacity(1).unwrap(), 255);
    // Opacity 255 replaces with the palette color, including a hole from alpha.
    assert_eq!(layered.composite_pixels(0).unwrap(), vec![2]);
    assert_eq!(layered.frame_rgba(0).unwrap(), vec![0, 0, 10, 128]);
}

#[test]
fn a_bad_opacity_in_the_file_is_refused() {
    let err = Document::load_project(
        r#"{
  "format": "pixelforge-project",
  "version": 1,
  "width": 1,
  "height": 1,
  "activeFrame": 0,
  "palette": [ { "r": 0, "g": 0, "b": 0, "a": 0 }, { "r": 1, "g": 1, "b": 1, "a": 255 } ],
  "layers": [ { "name": "body", "opacity": 256 } ],
  "frames": [ { "pixels": [0] } ]
}"#,
    )
    .unwrap_err();
    assert_eq!(
        err,
        KernelError::ProjectFile(ProjectFileError::LayerOpacity { index: 0 })
    );
}

#[test]
fn hidden_and_partial_layers_composite_in_png_sheet_and_indices() {
    let mut document = stacked();
    let covered = document.export_frame_png(0).unwrap();
    assert_eq!(
        &png_rgba(&covered)[..4],
        &[74, 140, 196, 255],
        "opaque gear covers the body"
    );

    document.set_layer_visible(1, false).unwrap();
    assert_eq!(document.composite_pixels(0).unwrap(), vec![3, 0]);
    assert_eq!(document.layer_pixels(0, 1).unwrap()[0], 7);
    let hidden = png_rgba(&document.export_frame_png(0).unwrap());
    assert_eq!(&hidden[..4], &[232, 93, 76, 255]);
    assert_eq!(&hidden[4..], &[0, 0, 0, 0]);
    let sheet = png_rgba(&document.export_sprite_sheet());
    assert_eq!(&sheet[..4], &[232, 93, 76, 255]);
    let preview = png_rgba(&document.export_view_png(0, 1, None).unwrap());
    assert_eq!(preview, hidden);
    let gear_alone = png_rgba(&document.export_view_png(0, 1, Some(1)).unwrap());
    assert_eq!(&gear_alone[..4], &[74, 140, 196, 255]);

    document.set_layer_visible(1, true).unwrap();
    document.set_layer_opacity(1, 0).unwrap();
    assert!(document.layer_visible(1).unwrap());
    assert_eq!(png_rgba(&document.export_frame_png(0).unwrap()), hidden);

    document.set_layer_opacity(1, 128).unwrap();
    assert_eq!(document.composite_pixels(0).unwrap(), vec![7, 0]);
    let blended = png_rgba(&document.export_frame_png(0).unwrap());
    assert_eq!(&blended[..4], &[153, 117, 136, 255]);
    assert_eq!(&blended[4..], &[0, 0, 0, 0]);
    assert_eq!(document.frame_rgba(0).unwrap(), blended);
    let scaled = png_rgba(&document.export_frame_png_scaled(0, 2).unwrap());
    assert_eq!(scaled.len(), 4 * 2 * 4);
    for y in 0..2 {
        for x in 0..2 {
            let offset = (y * 4 + x) * 4;
            assert_eq!(&scaled[offset..offset + 4], &[153, 117, 136, 255]);
        }
        for x in 2..4 {
            let offset = (y * 4 + x) * 4;
            assert_eq!(&scaled[offset..offset + 4], &[0, 0, 0, 0]);
        }
    }
    let sheet = png_rgba(&document.export_sprite_sheet_scaled(1).unwrap());
    assert_eq!(&sheet[..4], &[153, 117, 136, 255]);

    let saved = document.save_project();
    assert!(saved.contains("\"opacity\": 128"));
    assert!(!saved.contains("\"visible\""));
    let loaded = Document::load_project(&saved).unwrap();
    assert_eq!(loaded.frame_rgba(0).unwrap(), blended);
    assert_eq!(loaded.layer_opacity(1).unwrap(), 128);

    document.set_layer_visible(1, false).unwrap();
    let saved = document.save_project();
    assert!(saved.contains("\"visible\": false"));
    assert!(saved.contains("\"opacity\": 128"));
    let loaded = Document::load_project(&saved).unwrap();
    assert!(!loaded.layer_visible(1).unwrap());
    assert_eq!(loaded.frame_rgba(0).unwrap(), hidden);
}

#[test]
fn visibility_and_opacity_are_undoable_and_flatten_uses_the_index_composite() {
    let mut document = stacked();
    document.set_layer_opacity(1, 128).unwrap();
    document.set_layer_visible(1, false).unwrap();
    assert!(document.can_undo());
    document.undo().unwrap();
    assert!(document.layer_visible(1).unwrap());
    assert_eq!(document.layer_opacity(1).unwrap(), 128);
    document.undo().unwrap();
    assert_eq!(document.layer_opacity(1).unwrap(), LAYER_OPACITY_OPAQUE);
    document.redo().unwrap();
    assert_eq!(document.layer_opacity(1).unwrap(), 128);

    document.set_layer_opacity(1, 128).unwrap();
    document.flatten().unwrap();
    assert_eq!(document.layer_count(), 1);
    assert!(document.layer_visible(0).unwrap());
    assert_eq!(document.layer_opacity(0).unwrap(), 255);
    // Preview blend (153, 117, 136) is nearest studio index 8 (140, 110, 184),
    // squared distance 2522, not the covering index 7.
    assert_eq!(document.composite_pixels(0).unwrap(), vec![8, 0]);
    assert_eq!(
        &document.frame_rgba(0).unwrap()[..4],
        &[140, 110, 184, 255],
        "flatten snaps the blend to the nearest palette color"
    );
    document.undo().unwrap();
    assert_eq!(document.layer_count(), 2);
    assert_eq!(document.layer_opacity(1).unwrap(), 128);
    assert_eq!(&document.frame_rgba(0).unwrap()[..4], &[153, 117, 136, 255]);

    document.set_layer_visible(1, false).unwrap();
    document.flatten().unwrap();
    assert_eq!(document.composite_pixels(0).unwrap(), vec![3, 0]);
    document.undo().unwrap();
    assert!(!document.layer_visible(1).unwrap());
    assert_eq!(document.layer_pixels(0, 1).unwrap()[0], 7);
}

#[test]
fn flatten_snaps_a_translucent_stack_to_the_preview_blend() {
    let mut document = stacked();
    document.set_layer_opacity(1, 128).unwrap();
    let preview = document.frame_rgba(0).unwrap();
    assert_eq!(&preview[..4], &[153, 117, 136, 255]);
    assert_eq!(&preview[4..], &[0, 0, 0, 0]);
    assert_eq!(
        document.composite_pixels(0).unwrap(),
        vec![7, 0],
        "ASCII still shows the covering index before flatten"
    );

    document.add_frame();
    document.set_active_frame(1).unwrap();
    document.paint_on(0, 0, 0, 3).unwrap();
    let uncovered = document.frame_rgba(1).unwrap();
    assert_eq!(&uncovered[..4], &[232, 93, 76, 255]);

    document.flatten().unwrap();
    assert_eq!(document.layer_count(), 1);
    assert_eq!(document.layer_opacity(0).unwrap(), 255);
    assert_eq!(document.composite_pixels(0).unwrap(), vec![8, 0]);
    assert_eq!(document.composite_pixels(1).unwrap(), vec![3, 0]);
    let baked = document.frame_rgba(0).unwrap();
    assert_eq!(&baked[..4], &[140, 110, 184, 255]);
    assert_eq!(&baked[4..], &[0, 0, 0, 0]);

    let blend = [153u8, 117, 136];
    let snapped = [140u8, 110, 184];
    let covering = [74u8, 140, 196];
    let under = [232u8, 93, 76];
    assert!(
        dist(blend, snapped) < dist(blend, covering),
        "snapped color must beat the covering index"
    );
    assert!(dist(blend, snapped) < dist(blend, under));
    assert_eq!(dist(blend, snapped), 13 * 13 + 7 * 7 + 48 * 48);

    document.undo().unwrap();
    assert_eq!(document.layer_opacity(1).unwrap(), 128);
    assert_eq!(&document.frame_rgba(0).unwrap()[..4], &[153, 117, 136, 255]);
}

#[test]
fn flatten_drops_a_faint_layer_and_keeps_a_half_opaque_color() {
    let mut faint = Document::new(1, 1).unwrap();
    faint.paint(0, 0, 7).unwrap();
    faint.set_layer_opacity(0, 10).unwrap();
    assert_eq!(faint.frame_rgba(0).unwrap(), vec![74, 140, 196, 10]);
    faint.flatten().unwrap();
    assert_eq!(faint.composite_pixels(0).unwrap(), vec![0]);
    assert_eq!(faint.frame_rgba(0).unwrap(), vec![0, 0, 0, 0]);

    let mut half = Document::new(1, 1).unwrap();
    half.paint(0, 0, 7).unwrap();
    half.set_layer_opacity(0, 128).unwrap();
    assert_eq!(half.frame_rgba(0).unwrap(), vec![74, 140, 196, 128]);
    half.flatten().unwrap();
    assert_eq!(half.composite_pixels(0).unwrap(), vec![7]);
    assert_eq!(
        half.frame_rgba(0).unwrap(),
        vec![74, 140, 196, 255],
        "RGB matches; the opaque result layer does not keep blend alpha"
    );
}

#[test]
fn flatten_keeps_a_translucent_palette_entry_that_already_matches() {
    let mut document = Document::load_project(
        r#"{
  "format": "pixelforge-project",
  "version": 1,
  "width": 1,
  "height": 1,
  "activeFrame": 0,
  "palette": [
    { "r": 0, "g": 0, "b": 0, "a": 0 },
    { "r": 232, "g": 93, "b": 76, "a": 255 },
    { "r": 0, "g": 0, "b": 10, "a": 100 }
  ],
  "layers": [ { "name": "body" }, { "name": "gear" } ],
  "frames": [ { "layers": [ { "pixels": [1] }, { "pixels": [2] } ] } ]
}"#,
    )
    .unwrap();
    assert_eq!(document.frame_rgba(0).unwrap(), vec![0, 0, 10, 100]);
    assert_eq!(document.composite_pixels(0).unwrap(), vec![2]);
    document.flatten().unwrap();
    assert_eq!(document.composite_pixels(0).unwrap(), vec![2]);
    assert_eq!(document.frame_rgba(0).unwrap(), vec![0, 0, 10, 100]);
}

#[test]
fn flatten_of_an_opaque_stack_keeps_the_covering_index() {
    let mut document = stacked();
    document.flatten().unwrap();
    assert_eq!(document.composite_pixels(0).unwrap(), vec![7, 0]);
    assert_eq!(&document.frame_rgba(0).unwrap()[..4], &[74, 140, 196, 255]);
}

/// Left half of a side-by-side RGBA buffer. `png_width` is the full image.
fn left_panel(rgba: &[u8], png_width: u32) -> Vec<u8> {
    let width = png_width as usize;
    let half = width / 2;
    let mut out = Vec::new();
    let height = rgba.len() / (width * 4);
    for y in 0..height {
        let row = y * width * 4;
        out.extend_from_slice(&rgba[row..row + half * 4]);
    }
    out
}

fn right_panel(rgba: &[u8], png_width: u32) -> Vec<u8> {
    let width = png_width as usize;
    let half = width / 2;
    let mut out = Vec::new();
    let height = rgba.len() / (width * 4);
    for y in 0..height {
        let row = y * width * 4;
        out.extend_from_slice(&rgba[row + half * 4..row + width * 4]);
    }
    out
}

#[test]
fn flatten_compare_matches_rgba_and_a_real_flatten_without_writing() {
    let mut document = stacked();
    document.set_layer_opacity(1, 128).unwrap();
    document.add_frame();
    document.set_active_frame(1).unwrap();
    document.paint_on(0, 0, 0, 3).unwrap();
    document.set_active_frame(0).unwrap();
    let saved = document.save_project();
    let blend = document.frame_rgba(0).unwrap();
    assert_eq!(&blend[..4], &[153, 117, 136, 255]);
    assert_eq!(document.composite_pixels(0).unwrap(), vec![7, 0]);

    let compared = document.export_flatten_compare(0, 1).unwrap();
    assert_eq!(document.save_project(), saved);
    assert_eq!(compared.cover, vec![7, 0]);
    assert_eq!(compared.indices, vec![8, 0]);
    assert_eq!(compared.index_changes, 1);
    assert_eq!(compared.color_changes, 1);
    assert_eq!(compared.flatten_rows, vec!["8.".to_string()]);
    assert_eq!((compared.png_width, compared.png_height), (4, 1));
    let image = decode_sheet_png(&compared.png).unwrap();
    assert_eq!(left_panel(&image.rgba, image.width), blend);
    let mut baked = document.clone();
    baked.flatten().unwrap();
    assert_eq!(
        right_panel(&image.rgba, image.width),
        baked.frame_rgba(0).unwrap()
    );
    assert_eq!(
        &right_panel(&image.rgba, image.width)[..4],
        &[140, 110, 184, 255]
    );
    assert_eq!(document.save_project(), saved);
    assert_eq!(document.layer_count(), 2);
    assert_eq!(document.layer_opacity(1).unwrap(), 128);

    let uncovered = document.export_flatten_compare(1, 1).unwrap();
    assert_eq!(uncovered.indices, vec![3, 0]);
    assert_eq!(uncovered.index_changes, 0);
    assert_eq!(uncovered.color_changes, 0);
    let uncovered_png = decode_sheet_png(&uncovered.png).unwrap();
    let frame_rgba = document.frame_rgba(1).unwrap();
    assert_eq!(
        left_panel(&uncovered_png.rgba, uncovered_png.width),
        frame_rgba
    );
    assert_eq!(
        right_panel(&uncovered_png.rgba, uncovered_png.width),
        baked.frame_rgba(1).unwrap()
    );

    let scaled = document.export_flatten_compare(0, 2).unwrap();
    assert_eq!((scaled.png_width, scaled.png_height), (8, 2));
    let scaled_png = decode_sheet_png(&scaled.png).unwrap();
    assert_eq!(&scaled_png.rgba[..4], &[153, 117, 136, 255]);
    assert_eq!(&scaled_png.rgba[16..20], &[140, 110, 184, 255]);
    assert_eq!(document.save_project(), saved);

    let settled = baked.export_flatten_compare(0, 1).unwrap();
    assert_eq!(settled.index_changes, 0);
    assert_eq!(settled.color_changes, 0);
    let settled_png = decode_sheet_png(&settled.png).unwrap();
    assert_eq!(
        left_panel(&settled_png.rgba, settled_png.width),
        right_panel(&settled_png.rgba, settled_png.width)
    );
}

#[test]
fn flatten_compare_follows_the_alpha_cutoff_and_an_exact_palette_alpha() {
    let mut faint = Document::new(1, 1).unwrap();
    faint.paint(0, 0, 7).unwrap();
    faint.set_layer_opacity(0, 10).unwrap();
    let saved = faint.save_project();
    let compared = faint.export_flatten_compare(0, 1).unwrap();
    assert_eq!(compared.indices, vec![0]);
    assert_eq!(compared.index_changes, 1);
    assert_eq!(compared.color_changes, 1);
    let image = decode_sheet_png(&compared.png).unwrap();
    assert_eq!(left_panel(&image.rgba, image.width), vec![74, 140, 196, 10]);
    assert_eq!(right_panel(&image.rgba, image.width), vec![0, 0, 0, 0]);
    let mut baked = faint.clone();
    baked.flatten().unwrap();
    assert_eq!(
        right_panel(&image.rgba, image.width),
        baked.frame_rgba(0).unwrap()
    );
    assert_eq!(faint.save_project(), saved);

    let mut half = Document::new(1, 1).unwrap();
    half.paint(0, 0, 7).unwrap();
    half.set_layer_opacity(0, 128).unwrap();
    let half_view = half.export_flatten_compare(0, 1).unwrap();
    assert_eq!(half_view.indices, vec![7]);
    assert_eq!(half_view.index_changes, 0);
    assert_eq!(half_view.color_changes, 1);
    let half_png = decode_sheet_png(&half_view.png).unwrap();
    assert_eq!(
        left_panel(&half_png.rgba, half_png.width),
        vec![74, 140, 196, 128]
    );
    assert_eq!(
        right_panel(&half_png.rgba, half_png.width),
        vec![74, 140, 196, 255]
    );

    let mut kept = Document::load_project(
        r#"{
  "format": "pixelforge-project",
  "version": 1,
  "width": 1,
  "height": 1,
  "activeFrame": 0,
  "palette": [
    { "r": 0, "g": 0, "b": 0, "a": 0 },
    { "r": 232, "g": 93, "b": 76, "a": 255 },
    { "r": 0, "g": 0, "b": 10, "a": 100 }
  ],
  "layers": [ { "name": "body" }, { "name": "gear" } ],
  "frames": [ { "layers": [ { "pixels": [1] }, { "pixels": [2] } ] } ]
}"#,
    )
    .unwrap();
    let exact = kept.export_flatten_compare(0, 1).unwrap();
    assert_eq!(exact.indices, vec![2]);
    assert_eq!(exact.index_changes, 0);
    assert_eq!(exact.color_changes, 0);
    let exact_png = decode_sheet_png(&exact.png).unwrap();
    let color = vec![0, 0, 10, 100];
    assert_eq!(left_panel(&exact_png.rgba, exact_png.width), color);
    assert_eq!(right_panel(&exact_png.rgba, exact_png.width), color);
    kept.flatten().unwrap();
    assert_eq!(kept.frame_rgba(0).unwrap(), color);
}

#[test]
fn preview_compare_is_read_only_and_rejects_a_scoped_view() {
    let mut document = stacked();
    document.set_layer_opacity(1, 128).unwrap();
    let saved = document.save_project();
    let plain = tool(&mut document, "previewFrame", json!({}));
    assert!(plain.get("compare").is_none());
    assert!(plain.get("flattenPixels").is_none());
    assert_eq!(plain["pngWidth"], json!(2));
    let off = tool(&mut document, "previewFrame", json!({ "compare": false }));
    assert!(off.get("compare").is_none());
    assert_eq!(off["pngWidth"], json!(2));

    let compared = tool(
        &mut document,
        "previewFrame",
        json!({ "compare": true, "scale": 1 }),
    );
    assert_eq!(compared["compare"], json!(true));
    assert_eq!(compared["panels"], json!(["rgba", "indexed"]));
    assert_eq!(compared["pngWidth"], json!(4));
    assert_eq!(compared["pngHeight"], json!(1));
    assert_eq!(compared["pixels"], json!([7, 0]));
    assert_eq!(compared["flattenPixels"], json!([8, 0]));
    assert_eq!(compared["flattenRows"], json!(["8."]));
    assert_eq!(compared["indexChanges"], json!(1));
    assert_eq!(compared["colorChanges"], json!(1));
    assert_eq!(document.save_project(), saved);

    let layered = tool_err(
        &mut document,
        "previewFrame",
        json!({ "compare": true, "layer": 0 }),
    );
    assert_eq!(layered["code"], json!("InvalidParams"));
    let masked = tool_err(
        &mut document,
        "previewFrame",
        json!({ "compare": true, "x": 0, "y": 0, "width": 1, "height": 1 }),
    );
    assert_eq!(masked["code"], json!("InvalidParams"));
    let bad = tool_err(&mut document, "previewFrame", json!({ "compare": "yes" }));
    assert_eq!(bad["code"], json!("InvalidParams"));
    assert_eq!(document.save_project(), saved);
}

fn dist(left: [u8; 3], right: [u8; 3]) -> u32 {
    left.into_iter()
        .zip(right)
        .map(|(a, b)| {
            let delta = i32::from(a) - i32::from(b);
            u32::try_from(delta * delta).unwrap()
        })
        .sum()
}

#[test]
fn tool_api_sets_visibility_and_opacity_without_recording_the_same_value() {
    let mut document = stacked();
    let listed = tool(&mut document, "listLayers", json!({}));
    assert_eq!(listed["layers"][1]["visible"], json!(true));
    assert_eq!(listed["layers"][1]["opacity"], json!(255));

    let hidden = tool(
        &mut document,
        "setLayerVisible",
        json!({ "index": 1, "visible": false }),
    );
    assert_eq!(hidden["visible"], json!(false));
    let again = tool(
        &mut document,
        "setLayerVisible",
        json!({ "index": 1, "visible": false }),
    );
    assert_eq!(again["visible"], json!(false));
    document.undo().unwrap();
    assert!(document.layer_visible(1).unwrap());
    assert_eq!(document.layer_pixels(0, 1).unwrap()[0], 7);

    let faded = tool(&mut document, "setLayerOpacity", json!({ "opacity": 128 }));
    assert_eq!(faded["index"], json!(1));
    assert_eq!(faded["opacity"], json!(128));
    assert_eq!(
        tool_err(&mut document, "setLayerOpacity", json!({ "opacity": 256 }))["code"],
        json!("InvalidParams")
    );
    assert_eq!(document.layer_opacity(1).unwrap(), 128);
    let scene = tool(&mut document, "scene", json!({}));
    assert_eq!(scene["layers"][1]["opacity"], json!(128));
    assert_eq!(scene["layers"][1]["visible"], json!(true));
    let inspect = tool(&mut document, "inspect", json!({}));
    assert_eq!(inspect["layers"][1]["visible"], json!(true));
    assert_eq!(inspect["composite"][0], json!(7));

    document.reorder_layer(1, 0).unwrap();
    assert_eq!(document.layer_opacity(0).unwrap(), 128);
    assert_eq!(document.layer_name(0).unwrap(), "gear");
    document.undo().unwrap();
    document.delete_layer(1).unwrap();
    document.undo().unwrap();
    assert_eq!(document.layer_opacity(1).unwrap(), 128);
}
