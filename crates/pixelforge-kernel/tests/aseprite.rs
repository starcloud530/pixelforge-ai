//! Indexed `.aseprite` bytes are built in this test. There is no external fixture.

use std::io::Write;

use flate2::write::ZlibEncoder;
use flate2::Compression;
use pixelforge_kernel::{Document, KernelError, Rgba, TOOL_API_VERSION, TOOL_NAMES};
use serde_json::{json, Value};

fn le_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn le_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn le_i16(out: &mut Vec<u8>, value: i16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_string(out: &mut Vec<u8>, text: &str) {
    le_u16(out, text.len() as u16);
    out.extend_from_slice(text.as_bytes());
}

fn chunk(kind: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    le_u32(&mut out, (6 + payload.len()) as u32);
    le_u16(&mut out, kind);
    out.extend_from_slice(payload);
    out
}

fn layer(name: &str, flags: u16, kind: u16, blend: u16, opacity: u8) -> Vec<u8> {
    let mut payload = Vec::new();
    le_u16(&mut payload, flags);
    le_u16(&mut payload, kind);
    le_u16(&mut payload, 0);
    le_u16(&mut payload, 0);
    le_u16(&mut payload, 0);
    le_u16(&mut payload, blend);
    payload.push(opacity);
    payload.extend_from_slice(&[0, 0, 0]);
    push_string(&mut payload, name);
    if kind == 2 {
        le_u32(&mut payload, 0);
    }
    chunk(0x2004, &payload)
}

fn palette(colors: &[[u8; 4]]) -> Vec<u8> {
    let mut payload = Vec::new();
    le_u32(&mut payload, colors.len() as u32);
    le_u32(&mut payload, 0);
    le_u32(&mut payload, (colors.len() - 1) as u32);
    payload.extend_from_slice(&[0; 8]);
    for color in colors {
        le_u16(&mut payload, 0);
        payload.extend_from_slice(color);
    }
    chunk(0x2019, &payload)
}

fn cel(
    layer_index: u16,
    x: i16,
    y: i16,
    width: u16,
    height: u16,
    pixels: &[u8],
    opacity: u8,
    z: i16,
    compressed: bool,
) -> Vec<u8> {
    let mut payload = Vec::new();
    le_u16(&mut payload, layer_index);
    le_i16(&mut payload, x);
    le_i16(&mut payload, y);
    payload.push(opacity);
    le_u16(&mut payload, if compressed { 2 } else { 0 });
    le_i16(&mut payload, z);
    payload.extend_from_slice(&[0; 5]);
    le_u16(&mut payload, width);
    le_u16(&mut payload, height);
    if compressed {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(pixels).unwrap();
        payload.extend(encoder.finish().unwrap());
    } else {
        payload.extend_from_slice(pixels);
    }
    chunk(0x2005, &payload)
}

fn link(layer_index: u16, x: i16, y: i16, frame: u16) -> Vec<u8> {
    let mut payload = Vec::new();
    le_u16(&mut payload, layer_index);
    le_i16(&mut payload, x);
    le_i16(&mut payload, y);
    payload.push(255);
    le_u16(&mut payload, 1);
    le_i16(&mut payload, 0);
    payload.extend_from_slice(&[0; 5]);
    le_u16(&mut payload, frame);
    chunk(0x2005, &payload)
}

fn frame(duration: u16, chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut body = Vec::new();
    le_u16(&mut body, 0xF1FA);
    le_u16(&mut body, chunks.len() as u16);
    le_u16(&mut body, duration);
    body.extend_from_slice(&[0, 0]);
    le_u32(&mut body, chunks.len() as u32);
    for item in chunks {
        body.extend_from_slice(item);
    }
    let mut out = Vec::new();
    le_u32(&mut out, (4 + body.len()) as u32);
    out.extend(body);
    out
}

fn file(
    width: u16,
    height: u16,
    depth: u16,
    transparent: u8,
    speed: u16,
    flags: u32,
    frames: &[Vec<u8>],
) -> Vec<u8> {
    let mut header = vec![0u8; 128];
    header[4..6].copy_from_slice(&0xA5E0u16.to_le_bytes());
    header[6..8].copy_from_slice(&(frames.len() as u16).to_le_bytes());
    header[8..10].copy_from_slice(&width.to_le_bytes());
    header[10..12].copy_from_slice(&height.to_le_bytes());
    header[12..14].copy_from_slice(&depth.to_le_bytes());
    header[14..18].copy_from_slice(&flags.to_le_bytes());
    header[18..20].copy_from_slice(&speed.to_le_bytes());
    header[28] = transparent;
    header[32..34].copy_from_slice(&(4u16).to_le_bytes());
    let mut out = header;
    for item in frames {
        out.extend_from_slice(item);
    }
    let size = out.len() as u32;
    out[0..4].copy_from_slice(&size.to_le_bytes());
    out
}

fn sample() -> Vec<u8> {
    let colors = [[0, 0, 0, 0], [10, 20, 30, 255], [40, 50, 60, 255]];
    let frame0 = frame(
        80,
        &[
            layer("folder", 1, 1, 0, 255),
            layer("torso", 1 | 8, 0, 0, 255),
            layer("ghost", 0, 0, 3, 128),
            layer("guide", 1 | 64, 0, 0, 255),
            palette(&colors),
            chunk(0x2018, &[]),
            cel(1, 1, 0, 2, 2, &[1, 2, 1, 0], 255, 0, false),
            cel(2, 3, 1, 1, 1, &[2], 128, 2, true),
            cel(3, 0, 1, 1, 1, &[1], 255, 0, false),
        ],
    );
    let frame1 = frame(0, &[link(1, 0, 0, 0)]);
    file(4, 2, 8, 0, 0, 1, &[frame0, frame1])
}

fn call(document: &mut Document, body: Value) -> Value {
    let raw = document.call_tool(&body.to_string());
    serde_json::from_str(&raw).expect("tool response is JSON")
}

fn tool(document: &mut Document, name: &str, params: Value) -> Value {
    let response = call(
        document,
        json!({
            "apiVersion": TOOL_API_VERSION,
            "tool": name,
            "params": params,
        }),
    );
    assert_eq!(response["ok"], json!(true), "{name}: {response}");
    response["result"].clone()
}

fn tool_err(document: &mut Document, name: &str, params: Value) -> Value {
    let response = call(document, json!({ "tool": name, "params": params }));
    assert_eq!(response["ok"], json!(false), "{name}: {response}");
    response["error"].clone()
}

#[test]
fn indexed_import_maps_layers_and_keeps_frame_pixels() {
    let (document, report) = Document::from_aseprite(&sample()).unwrap();
    assert_eq!((document.width(), document.height()), (4, 2));
    assert_eq!(document.frame_count(), 2);
    assert_eq!(document.layer_names(), ["torso", "ghost"]);
    assert_eq!(document.active_layer(), 0);
    assert_eq!(document.active_frame(), 0);
    assert!(document.selection().is_none());
    assert!(document.tags().is_empty());
    assert!(!document.palette_locked());
    assert!(!document.can_undo());
    assert_eq!(document.frame_delay(0).unwrap(), 80);
    assert_eq!(document.frame_delay(1).unwrap(), 100);
    assert_eq!(document.palette_colors()[0], Rgba::TRANSPARENT);
    assert_eq!(document.palette_colors()[1], Rgba::rgb(10, 20, 30));
    assert_eq!(document.palette_colors()[2], Rgba::rgb(40, 50, 60));
    assert_eq!(
        document.layer_pixels(0, 0).unwrap(),
        &[0, 1, 2, 0, 0, 1, 0, 0]
    );
    assert_eq!(
        document.layer_pixels(0, 1).unwrap(),
        &[0, 0, 0, 0, 0, 0, 0, 2]
    );
    assert_eq!(
        document.composite_pixels(0).unwrap(),
        &[0, 1, 2, 0, 0, 1, 0, 2]
    );
    assert_eq!(
        document.layer_pixels(1, 0).unwrap(),
        &[1, 2, 0, 0, 1, 0, 0, 0]
    );
    assert!(document
        .layer_pixels(1, 1)
        .unwrap()
        .iter()
        .all(|index| *index == 0));
    assert!(!report.transparent_swapped);
    assert_eq!(report.hidden_layers, 1);
    assert_eq!(report.skipped_groups, 1);
    assert_eq!(report.skipped_reference, 1);
    assert_eq!(report.background_layers, 1);
    assert_eq!(report.non_normal_blend, 1);
    assert_eq!(report.partial_opacity, 1);
    assert_eq!(report.partial_cel_opacity, 1);
    assert_eq!(report.nonzero_z_index, 1);
    assert_eq!(report.duration_defaulted, 1);
    assert_eq!(report.clipped_cells, 0);
    assert_eq!(report.ignored, vec!["tags".to_string()]);

    let saved = Document::load_project(&document.save_project()).unwrap();
    assert_eq!(
        saved.layer_pixels(0, 0).unwrap(),
        document.layer_pixels(0, 0).unwrap()
    );
    assert_eq!(saved.frame_delay(1).unwrap(), 100);
}

#[test]
fn transparent_index_swaps_into_slot_zero() {
    let colors = [[1, 2, 3, 255], [9, 0, 0, 255], [255, 0, 255, 255]];
    let bytes = file(
        2,
        1,
        8,
        2,
        50,
        0,
        &[frame(
            0,
            &[
                layer("Layer 1", 1, 0, 0, 255),
                palette(&colors),
                cel(0, 0, 0, 2, 1, &[2, 0], 255, 0, false),
            ],
        )],
    );
    let (document, report) = Document::from_aseprite(&bytes).unwrap();
    assert!(report.transparent_swapped);
    assert_eq!(report.source_transparent_index, 2);
    assert_eq!(report.duration_defaulted, 0);
    assert_eq!(document.frame_delay(0).unwrap(), 50);
    assert_eq!(document.pixels(), &[0, 2]);
    assert_eq!(document.palette_colors()[0], Rgba::TRANSPARENT);
    assert_eq!(document.palette_colors()[1], Rgba::rgb(9, 0, 0));
    assert_eq!(document.palette_colors()[2], Rgba::rgb(1, 2, 3));
}

#[test]
fn old_palette_is_used_until_a_new_one_replaces_it() {
    let mut old = Vec::new();
    le_u16(&mut old, 1);
    old.push(0);
    old.push(2);
    old.extend_from_slice(&[8, 8, 8, 1, 2, 3]);
    let legacy = file(
        1,
        1,
        8,
        0,
        0,
        0,
        &[frame(
            40,
            &[
                layer("ink", 1, 0, 0, 255),
                chunk(0x0004, &old),
                cel(0, 0, 0, 1, 1, &[1], 255, 0, false),
            ],
        )],
    );
    let (document, report) = Document::from_aseprite(&legacy).unwrap();
    assert_eq!(document.pixels(), &[1]);
    assert_eq!(document.palette_colors()[1], Rgba::rgb(1, 2, 3));
    assert_eq!(document.palette_colors()[0], Rgba::TRANSPARENT);
    assert!(report.ignored.is_empty());

    let both = file(
        1,
        1,
        8,
        0,
        0,
        0,
        &[frame(
            40,
            &[
                layer("ink", 1, 0, 0, 255),
                chunk(0x0004, &old),
                palette(&[[0, 0, 0, 0], [7, 8, 9, 255]]),
                cel(0, 0, 0, 1, 1, &[1], 255, 0, false),
            ],
        )],
    );
    let (document, report) = Document::from_aseprite(&both).unwrap();
    assert_eq!(document.palette_colors()[1], Rgba::rgb(7, 8, 9));
    assert_eq!(report.ignored, vec!["oldPalette".to_string()]);
}

#[test]
fn six_bit_palette_scales_to_byte_channels() {
    let mut old = Vec::new();
    le_u16(&mut old, 1);
    old.push(0);
    old.push(2);
    old.extend_from_slice(&[0, 0, 0, 63, 0, 0]);
    let bytes = file(
        1,
        1,
        8,
        0,
        0,
        0,
        &[frame(
            10,
            &[chunk(0x0011, &old), cel(0, 0, 0, 1, 1, &[1], 255, 0, false)],
        )],
    );
    let (document, _) = Document::from_aseprite(&bytes).unwrap();
    assert_eq!(document.layer_names(), ["Layer 1"]);
    assert_eq!(document.palette_colors()[1], Rgba::rgb(255, 0, 0));
}

#[test]
fn clipped_cel_drops_cells_outside_the_canvas() {
    let bytes = file(
        2,
        1,
        8,
        0,
        0,
        0,
        &[frame(
            10,
            &[
                palette(&[[0, 0, 0, 0], [1, 0, 0, 255]]),
                cel(0, 1, 0, 2, 1, &[1, 1], 255, 0, false),
            ],
        )],
    );
    let (document, report) = Document::from_aseprite(&bytes).unwrap();
    assert_eq!(document.pixels(), &[0, 1]);
    assert_eq!(report.clipped_cells, 1);
}

#[test]
fn import_refuses_the_formats_it_does_not_read() {
    let mut document = Document::new(4, 2).unwrap();
    document.paint(0, 0, 1).unwrap();
    let kept = document.pixels().to_vec();

    let rgba = file(2, 2, 32, 0, 0, 0, &[frame(10, &[])]);
    let err = tool_err(
        &mut document,
        "importAseprite",
        json!({ "bytes": base64(&rgba) }),
    );
    assert_eq!(err["code"], "AsepriteColorMode");
    assert_eq!(document.pixels(), kept.as_slice());

    let huge = file(300, 1, 8, 0, 0, 0, &[frame(10, &[])]);
    let err = tool_err(
        &mut document,
        "importAseprite",
        json!({ "bytes": base64(&huge) }),
    );
    assert_eq!(err["code"], "AsepriteSize");
    assert!(err["message"].as_str().unwrap().contains("300x1"));

    let tile = file(
        2,
        2,
        8,
        0,
        0,
        0,
        &[frame(10, &[layer("tiles", 1, 2, 0, 255)])],
    );
    let err = tool_err(
        &mut document,
        "importAseprite",
        json!({ "bytes": base64(&tile) }),
    );
    assert_eq!(err["code"], "AsepriteTilemap");
    assert_eq!(document.get(0, 0).unwrap(), 1);
    assert!(document.can_undo());

    let mut bad = sample();
    bad[4] = 0;
    bad[5] = 0;
    assert!(matches!(
        Document::from_aseprite(&bad),
        Err(KernelError::AsepriteFile { .. })
    ));
    assert!(matches!(
        Document::from_aseprite(&[0, 1, 2]),
        Err(KernelError::AsepriteFile { .. })
    ));
    assert_eq!(document.pixels(), kept.as_slice());
}

#[test]
fn import_tool_replaces_the_document_and_reports_the_subset() {
    assert!(TOOL_NAMES.contains(&"importAseprite"));
    assert!(TOOL_NAMES.contains(&"flipVertical"));
    assert!(TOOL_NAMES.contains(&"rotate"));
    let mut document = Document::new(8, 8).unwrap();
    document.paint(1, 1, 4).unwrap();
    let result = tool(
        &mut document,
        "importAseprite",
        json!({ "bytes": base64(&sample()) }),
    );
    assert_eq!(result["width"], 4);
    assert_eq!(result["layerCount"], 2);
    assert_eq!(result["aseprite"]["skippedGroups"], 1);
    assert_eq!(result["aseprite"]["ignored"], json!(["tags"]));
    assert_eq!(result["canUndo"], false);
    assert_eq!(result["pixels"], json!([0, 1, 2, 0, 0, 1, 0, 0]));
    let err = tool_err(&mut document, "importAseprite", json!({ "png": "aa" }));
    assert_eq!(err["code"], "InvalidParams");
}

#[test]
fn vertical_flip_and_rotation_follow_the_horizontal_flip_rules() {
    let mut document = Document::new(4, 4).unwrap();
    tool(
        &mut document,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 1}),
    );
    tool(
        &mut document,
        "paint",
        json!({"x": 1, "y": 0, "colorIndex": 2}),
    );
    tool(
        &mut document,
        "paint",
        json!({"x": 0, "y": 1, "colorIndex": 3}),
    );
    tool(
        &mut document,
        "paint",
        json!({"x": 1, "y": 1, "colorIndex": 4}),
    );
    tool(&mut document, "addLayer", json!({"name": "gear"}));
    tool(
        &mut document,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 5, "layer": 1}),
    );
    let gear = document.layer_pixels(0, 1).unwrap().to_vec();

    tool(
        &mut document,
        "setSelection",
        json!({"x": 0, "y": 0, "width": 2, "height": 1}),
    );
    tool(
        &mut document,
        "flipVertical",
        json!({"layer": 0, "x": 0, "y": 0, "width": 2, "height": 2}),
    );
    assert_eq!(
        &document.layer_pixels(0, 0).unwrap()[..8],
        &[1, 2, 0, 0, 3, 4, 0, 0][..]
    );
    assert_eq!(document.layer_pixels(0, 1).unwrap(), gear.as_slice());

    tool(&mut document, "clearSelection", json!({}));
    let dirty = tool(
        &mut document,
        "flipVertical",
        json!({"reportDirty": true, "layer": 0, "x": 0, "y": 0, "width": 2, "height": 2}),
    );
    assert_eq!(dirty["changed"], json!(4));
    assert!(dirty.get("clipped").is_none());
    assert_eq!(
        &document.layer_pixels(0, 0).unwrap()[..8],
        &[3, 4, 0, 0, 1, 2, 0, 0][..]
    );
    assert_eq!(document.palette_colors()[1], Rgba::rgb(26, 28, 36));

    let err = tool_err(&mut document, "rotate", json!({"degrees": 45}));
    assert_eq!(err["code"], "InvalidParams");
    let err = tool_err(
        &mut document,
        "rotate",
        json!({"degrees": 90, "x": 0, "y": 0, "width": 3, "height": 2}),
    );
    assert_eq!(err["code"], "RotateSquare");
    assert_eq!(document.width(), 4);
    assert_eq!(
        &document.layer_pixels(0, 0).unwrap()[..8],
        &[3, 4, 0, 0, 1, 2, 0, 0][..]
    );

    let before = document.layer_pixels(0, 0).unwrap().to_vec();
    tool(&mut document, "rotate", json!({"degrees": 90, "layer": 0}));
    assert_ne!(document.layer_pixels(0, 0).unwrap(), before.as_slice());
    assert_eq!(document.layer_pixels(0, 1).unwrap(), gear.as_slice());
    tool(&mut document, "rotate", json!({"degrees": 270, "layer": 0}));
    assert_eq!(document.layer_pixels(0, 0).unwrap(), before.as_slice());
    document.undo().unwrap();
    assert_ne!(document.layer_pixels(0, 0).unwrap(), before.as_slice());
    document.undo().unwrap();
    assert_eq!(document.layer_pixels(0, 0).unwrap(), before.as_slice());
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let pack = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        out.push(TABLE[((pack >> 18) & 63) as usize] as char);
        out.push(TABLE[((pack >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((pack >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(pack & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}
