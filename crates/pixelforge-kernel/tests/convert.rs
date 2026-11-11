//! Arbitrary PNG → indexed frame, through the Tool API only.

use pixelforge_kernel::{Document, KernelError, ProjectFileError, Rgba, TOOL_API_VERSION};
use png::{BitDepth, ColorType, Encoder};
use serde_json::{json, Value};

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

fn encode_png(width: u32, height: u32, rgba: &[u8]) -> String {
    let mut bytes = Vec::new();
    let mut encoder = Encoder::new(&mut bytes, width, height);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    encoder
        .write_header()
        .expect("png header")
        .write_image_data(rgba)
        .expect("png data");
    encode_b64(&bytes)
}

fn encode_b64(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let mut buf = [0u8; 3];
        buf[..chunk.len()].copy_from_slice(chunk);
        let value = (u32::from(buf[0]) << 16) | (u32::from(buf[1]) << 8) | u32::from(buf[2]);
        out.push(TABLE[((value >> 18) & 63) as usize] as char);
        out.push(TABLE[((value >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((value >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(value & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn indices(document: &Document) -> Vec<u8> {
    let inspect = tool(&mut document.clone(), "inspect", json!({}));
    inspect["pixels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_u64().unwrap() as u8)
        .collect()
}

#[test]
fn center_sample_resizes_preserves_transparency_and_undoes() {
    let mut rgba = vec![0u8; 8 * 4];
    for x in 0..4 {
        let offset = x * 4;
        rgba[offset] = 255;
        rgba[offset + 3] = 255;
    }
    for x in 4..8 {
        let offset = x * 4;
        rgba[offset + 2] = 255;
        rgba[offset + 3] = 127;
    }
    let png = encode_png(8, 1, &rgba);
    let mut document = Document::new(4, 4).unwrap();
    tool(
        &mut document,
        "importPalette",
        json!({"text": "ff0000\n0000ff\n", "mode": "replace"}),
    );
    document.add_frame();
    let before = document.save_project();
    let converted = tool(
        &mut document,
        "convertPng",
        json!({
            "png": png,
            "width": 2,
            "height": 1,
            "engine": "center-sample",
            "palette": { "mode": "project" },
        }),
    );
    assert_eq!(converted["width"], json!(2));
    assert_eq!(converted["height"], json!(1));
    assert_eq!(converted["engine"]["id"], json!("center-sample"));
    assert_eq!(converted["engine"]["version"], json!("1"));
    assert_eq!(converted["paletteMode"], json!("project"));
    assert_eq!(converted["transform"]["params"], json!({}));
    assert_eq!(converted["featuresIgnored"], json!([]));
    assert_eq!(document.width(), 2);
    assert_eq!(document.height(), 1);
    assert_eq!(document.frame_count(), 2);
    let pixels = indices(&document);
    assert_eq!(pixels.len(), 2);
    assert!(pixels
        .iter()
        .all(|index| *index == 0 || *index == 1 || *index == 2));
    assert_eq!(pixels[0], 1, "center of the opaque red half maps to red");
    assert_eq!(pixels[1], 0, "alpha 127 stays index 0");
    assert_eq!(document.palette_colors()[1], Rgba::rgb(255, 0, 0));
    assert_eq!(document.palette_colors()[2], Rgba::rgb(0, 0, 255));
    tool(
        &mut document,
        "paint",
        json!({"x": 1, "y": 0, "colorIndex": 2}),
    );
    assert_eq!(indices(&document)[1], 2);
    document.undo().unwrap();
    assert_eq!(indices(&document)[1], 0);
    document.undo().unwrap();
    assert_eq!(document.save_project(), before);

    let again = tool(
        &mut Document::new(4, 4).unwrap(),
        "convertPng",
        json!({
            "png": png,
            "width": 2,
            "height": 1,
            "engine": "center-sample",
            "palette": { "mode": "project" },
        }),
    );
    // The second document still has the studio palette, so this only checks
    // the sampler metadata. The edited project below is the reproducibility case.
    assert_eq!(again["transform"]["id"], converted["transform"]["id"]);
    assert_eq!(again["report"], converted["report"]);
}

#[test]
fn same_request_is_stable_and_engines_are_selectable() {
    let mut rgba = Vec::new();
    for shade in [0u8, 8, 16, 24, 200, 210, 220, 230] {
        rgba.extend_from_slice(&[shade, 0, 0, 255]);
    }
    let png = encode_png(8, 1, &rgba);
    let request = json!({
        "png": png,
        "width": 4,
        "height": 1,
        "engine": "kmeans",
        "palette": { "mode": "source", "maxColors": 2 },
        "params": { "k": 2 },
        "features": ["eye"],
    });
    let mut first = Document::new(3, 3).unwrap();
    let mut second = Document::new(3, 3).unwrap();
    let a = tool(&mut first, "convertPng", request.clone());
    let b = tool(&mut second, "convertPng", request);
    assert_eq!(indices(&first), indices(&second));
    assert_eq!(a["transform"], b["transform"]);
    assert_eq!(a["featuresIgnored"], json!(["eye"]));
    assert_eq!(first.palette_colors().len(), 3);
    for index in indices(&first) {
        assert!(usize::from(index) < first.palette_colors().len());
    }
    let saved = first.save_project();
    assert!(!saved.contains("\n  \"engine\""));
    assert!(saved.contains("\"id\": \"kmeans\""));
    let loaded = Document::load_project(&saved).unwrap();
    assert_eq!(loaded.transform_engine(), "center-sample");
    assert_eq!(indices(&loaded), indices(&first));

    let listed = tool(&mut first, "listEngines", json!({}));
    let ids: Vec<&str> = listed["engines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        vec![
            "center-sample",
            "dominant",
            "kmeans",
            "contrast-outline",
            "grid-snap",
            "pixelforge-native",
        ]
    );
    assert_eq!(listed["engines"][5]["status"], json!("reserved"));
    assert_eq!(listed["engines"][5]["available"], json!(false));
    tool(&mut first, "setTransformEngine", json!({"id": "dominant"}));
    assert_eq!(first.transform_engine(), "dominant");
    let round = Document::load_project(&first.save_project()).unwrap();
    assert_eq!(round.transform_engine(), "dominant");
    assert!(round
        .save_project()
        .contains("\n  \"engine\": \"dominant\""));
    first.undo().unwrap();
    assert_eq!(first.transform_engine(), "center-sample");
}

#[test]
fn convert_errors_leave_the_document_unchanged() {
    let mut document = Document::new(4, 4).unwrap();
    let before = document.save_project();
    let native = tool_err(
        &mut document,
        "convertPng",
        json!({"png": encode_png(1, 1, &[1, 2, 3, 255]), "engine": "pixelforge-native"}),
    );
    assert_eq!(native["code"], json!("EngineUnavailable"));
    assert!(native["message"]
        .as_str()
        .unwrap()
        .contains("not installed or not authorized"));
    let unknown = tool_err(
        &mut document,
        "convertPng",
        json!({"png": encode_png(1, 1, &[1, 2, 3, 255]), "engine": "no-such"}),
    );
    assert_eq!(unknown["code"], json!("EngineUnknown"));
    let garbage = tool_err(
        &mut document,
        "convertPng",
        json!({"png": encode_b64(b"not a png")}),
    );
    assert_eq!(garbage["code"], json!("PngFile"));
    let huge = tool_err(
        &mut document,
        "convertPng",
        json!({"png": encode_png(4097, 1, &vec![0; 4097 * 4]), "width": 2, "height": 1}),
    );
    assert_eq!(huge["code"], json!("ConvertSource"));
    tool(&mut document, "setPaletteLocked", json!({"locked": true}));
    let locked = tool_err(
        &mut document,
        "convertPng",
        json!({
            "png": encode_png(1, 1, &[9, 8, 7, 255]),
            "palette": { "mode": "source", "maxColors": 4 },
        }),
    );
    assert_eq!(locked["code"], json!("PaletteLocked"));
    let allowed = tool(
        &mut document,
        "convertPng",
        json!({
            "png": encode_png(2, 2, &vec![255, 0, 0, 200, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            "width": 4,
            "height": 4,
            "palette": { "mode": "project" },
        }),
    );
    assert_eq!(allowed["width"], json!(4));
    assert!(document.palette_locked());
    document.undo().unwrap();
    assert_eq!(document.save_project().contains("\"transform\""), false);
    let _ = before;
    match Document::load_project("{\"format\":\"pixelforge-project\",\"version\":1,\"width\":1,\"height\":1,\"activeFrame\":0,\"palette\":[{\"r\":0,\"g\":0,\"b\":0,\"a\":0}],\"paletteLocked\":false,\"frames\":[{\"pixels\":[0],\"delayMs\":100}],\"engine\":\"nope\"}") {
        Err(KernelError::ProjectFile(ProjectFileError::Engine)) => {}
        other => panic!("expected Engine, got {other:?}"),
    }
}

#[test]
fn contrast_outline_differs_from_center_sample_on_a_thin_edge() {
    let rgba = vec![
        0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    ];
    let png = encode_png(4, 1, &rgba);
    let mut center = Document::new(4, 1).unwrap();
    let mut outline = Document::new(4, 1).unwrap();
    for document in [&mut center, &mut outline] {
        tool(
            document,
            "importPalette",
            json!({"text": "000000\nffffff\n", "mode": "replace"}),
        );
    }
    tool(
        &mut center,
        "convertPng",
        json!({"png": png, "width": 1, "height": 1, "engine": "center-sample"}),
    );
    tool(
        &mut outline,
        "convertPng",
        json!({
            "png": png,
            "width": 1,
            "height": 1,
            "engine": "contrast-outline",
            "params": { "contrast": 48, "expand": 0 },
        }),
    );
    assert_ne!(indices(&center), indices(&outline));
    assert_eq!(indices(&outline), vec![1]);
}

#[test]
fn a_wide_source_downsamples_and_export_uses_the_target() {
    let rgba = vec![255u8, 0, 0, 255].repeat(300);
    let png = encode_png(300, 1, &rgba);
    let mut document = Document::new(16, 16).unwrap();
    tool(
        &mut document,
        "importPalette",
        json!({"text": "ff0000\n", "mode": "replace"}),
    );
    let converted = tool(
        &mut document,
        "convertPng",
        json!({
            "png": png,
            "width": 8,
            "height": 4,
            "engine": "dominant",
        }),
    );
    assert_eq!((document.width(), document.height()), (8, 4));
    assert!(indices(&document).iter().all(|index| *index == 1));
    let exported = tool(&mut document, "exportPngSequence", json!({}));
    assert_eq!(exported["width"], json!(8));
    assert_eq!(exported["height"], json!(4));
    assert_eq!(converted["engine"]["id"], json!("dominant"));
}
