//! Palette strategies on the convert path: source, limit, and project.

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

fn indexed_png(width: u32, height: u32, indices: &[u8], palette: &[u8], trns: &[u8]) -> String {
    let mut bytes = Vec::new();
    let mut encoder = Encoder::new(&mut bytes, width, height);
    encoder.set_color(ColorType::Indexed);
    encoder.set_depth(BitDepth::Eight);
    encoder.set_palette(palette.to_vec());
    encoder.set_trns(trns.to_vec());
    encoder
        .write_header()
        .expect("indexed png header")
        .write_image_data(indices)
        .expect("indexed png data");
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

fn convert(document: &mut Document, png: &str, palette: Value) -> Value {
    tool(
        document,
        "convertPng",
        json!({
            "png": png,
            "width": 4,
            "height": 1,
            "engine": "center-sample",
            "palette": palette,
        }),
    )
}

fn opaque(document: &Document) -> Vec<Rgba> {
    document.palette_colors()[1..].to_vec()
}

#[test]
fn three_strategies_are_deterministic_and_report_merges() {
    let mut rgba = Vec::new();
    for (r, g, b) in [(0, 0, 0), (0, 0, 0), (1, 0, 0), (255, 0, 0)] {
        rgba.extend_from_slice(&[r, g, b, 255]);
    }
    let png = encode_png(4, 1, &rgba);

    let mut first = Document::new(4, 1).unwrap();
    let mut second = Document::new(4, 1).unwrap();
    let kept = convert(&mut first, &png, json!({ "strategy": "source" }));
    let kept_again = convert(&mut second, &png, json!({ "strategy": "source" }));
    assert_eq!(indices(&first), indices(&second));
    assert_eq!(first.palette_colors(), second.palette_colors());
    assert_eq!(kept["paletteReport"], kept_again["paletteReport"]);
    assert_eq!(kept["transform"]["palette"]["strategy"], json!("source"));
    assert!(kept["transform"]["palette"].get("maxColors").is_none());
    assert_eq!(kept["paletteReport"]["origin"], json!("colors"));
    assert_eq!(kept["paletteReport"]["metric"], json!("squared-rgb"));
    assert_eq!(kept["paletteReport"]["keptExisting"], json!(false));
    assert_eq!(kept["paletteReport"]["sourceColors"], json!(3));
    assert_eq!(kept["paletteReport"]["paletteColors"], json!(3));
    assert_eq!(kept["paletteReport"]["exactPixels"], json!(4));
    assert_eq!(kept["paletteReport"]["inexactPixels"], json!(0));
    assert_eq!(kept["paletteReport"]["totalDistance"], json!(0));
    assert_eq!(kept["paletteReport"]["merges"], json!([]));
    assert_eq!(
        opaque(&first),
        vec![Rgba::rgb(0, 0, 0), Rgba::rgb(1, 0, 0), Rgba::rgb(255, 0, 0)]
    );
    let saved = first.save_project();
    assert!(saved.contains("\"strategy\": \"source\""));
    let loaded = Document::load_project(&saved).unwrap();
    assert_eq!(loaded.palette_colors(), first.palette_colors());
    assert_eq!(indices(&loaded), indices(&first));

    let mut limited = Document::new(4, 1).unwrap();
    let mut limited_again = Document::new(4, 1).unwrap();
    let capped = convert(
        &mut limited,
        &png,
        json!({ "strategy": "limit", "maxColors": 2 }),
    );
    let capped_again = convert(
        &mut limited_again,
        &png,
        json!({ "strategy": "limit", "maxColors": 2 }),
    );
    assert_eq!(indices(&limited), indices(&limited_again));
    assert_eq!(capped["paletteReport"], capped_again["paletteReport"]);
    assert_eq!(capped["transform"]["palette"]["strategy"], json!("limit"));
    assert_eq!(capped["transform"]["palette"]["maxColors"], json!(2));
    assert_eq!(capped["paletteReport"]["origin"], json!("sampled"));
    assert_eq!(capped["paletteReport"]["paletteColors"], json!(2));
    assert_eq!(capped["paletteReport"]["mergedColors"], json!(1));
    assert_eq!(capped["paletteReport"]["inexactPixels"], json!(1));
    assert_eq!(capped["paletteReport"]["exactPixels"], json!(3));
    assert_eq!(capped["paletteReport"]["maxDistance"], json!(1));
    assert_eq!(capped["paletteReport"]["totalDistance"], json!(1));
    assert_eq!(capped["paletteReport"]["merges"][0]["from"]["r"], json!(1));
    assert_eq!(capped["paletteReport"]["merges"][0]["into"]["r"], json!(0));
    assert_eq!(capped["paletteReport"]["merges"][0]["distance"], json!(1));
    assert_eq!(capped["paletteReport"]["merges"][0]["pixels"], json!(1));
    assert_eq!(capped["paletteReport"]["merges"][0]["exact"], json!(false));
    assert_eq!(
        opaque(&limited),
        vec![Rgba::rgb(0, 0, 0), Rgba::rgb(255, 0, 0)]
    );
    assert_eq!(indices(&limited)[2], 1);

    let mut project = Document::new(2, 1).unwrap();
    tool(
        &mut project,
        "importPalette",
        json!({"text": "ff0000\n0000ff\n", "mode": "replace"}),
    );
    let before = project.palette_colors().to_vec();
    let mapped = tool(
        &mut project,
        "convertPng",
        json!({
            "png": encode_png(2, 1, &[250, 0, 0, 255, 0, 0, 250, 255]),
            "width": 2,
            "height": 1,
            "engine": "center-sample",
            "palette": { "strategy": "project" },
        }),
    );
    assert_eq!(project.palette_colors(), before.as_slice());
    assert_eq!(indices(&project), vec![1, 2]);
    assert_eq!(mapped["paletteReport"]["strategy"], json!("project"));
    assert_eq!(mapped["paletteReport"]["origin"], json!("project"));
    assert_eq!(mapped["paletteReport"]["paletteColors"], json!(2));
    assert_eq!(mapped["paletteReport"]["inexactPixels"], json!(2));
    assert_eq!(mapped["paletteReport"]["maxDistance"], json!(25));
    assert_eq!(mapped["paletteReport"]["totalDistance"], json!(50));
    assert_eq!(mapped["paletteReport"]["dropped"], json!([]));
    assert_eq!(mapped["transform"]["palette"]["strategy"], json!("project"));
    assert_eq!(mapped["transform"]["palette"]["mode"], json!("project"));
}

#[test]
fn source_keeps_an_indexed_palette_and_limit_drops_unused() {
    // PLTE: red, green (transparent), blue (alpha 200), yellow (unused).
    let png = indexed_png(
        4,
        1,
        &[0, 1, 2, 1],
        &[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0],
        &[255, 0, 200, 255],
    );
    let mut document = Document::new(4, 1).unwrap();
    let kept = convert(&mut document, &png, json!({ "strategy": "source" }));
    assert_eq!(kept["paletteReport"]["origin"], json!("indexed"));
    assert_eq!(kept["paletteReport"]["totalDistance"], json!(0));
    assert_eq!(kept["paletteReport"]["exactPixels"], json!(2));
    assert_eq!(kept["paletteReport"]["transparentPixels"], json!(2));
    assert_eq!(kept["paletteReport"]["paletteColors"], json!(3));
    assert_eq!(
        opaque(&document),
        vec![
            Rgba::rgb(255, 0, 0),
            Rgba::rgb(0, 0, 255),
            Rgba::rgb(255, 255, 0)
        ]
    );
    assert_eq!(indices(&document), vec![1, 0, 2, 0]);

    // Only red is used, so blue and yellow are unused and the cap drops them.
    let unused = indexed_png(
        4,
        1,
        &[0, 1, 0, 1],
        &[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0],
        &[255, 0, 200, 255],
    );
    let mut capped = Document::new(4, 1).unwrap();
    let limited = convert(
        &mut capped,
        &unused,
        json!({ "strategy": "limit", "maxColors": 1 }),
    );
    assert_eq!(limited["paletteReport"]["origin"], json!("indexed"));
    assert_eq!(limited["paletteReport"]["paletteColors"], json!(1));
    assert_eq!(limited["paletteReport"]["droppedColors"], json!(2));
    assert_eq!(limited["paletteReport"]["totalDistance"], json!(0));
    assert_eq!(opaque(&capped), vec![Rgba::rgb(255, 0, 0)]);
    let dropped = limited["paletteReport"]["dropped"].as_array().unwrap();
    assert_eq!(dropped[0]["sourceIndex"], json!(2));
    assert_eq!(dropped[0]["pixels"], json!(0));
    assert_eq!(dropped[1]["sourceIndex"], json!(3));
}

#[test]
fn source_keeps_colors_the_sample_never_uses() {
    let rgba = [1u8, 0, 0, 255, 2, 0, 0, 255, 3, 0, 0, 255, 4, 0, 0, 255];
    let png = encode_png(4, 1, &rgba);
    let mut source = Document::new(1, 1).unwrap();
    let kept = tool(
        &mut source,
        "convertPng",
        json!({
            "png": png,
            "width": 1,
            "height": 1,
            "engine": "center-sample",
            "palette": { "strategy": "source" },
        }),
    );
    assert_eq!(kept["paletteReport"]["sourceColors"], json!(4));
    assert_eq!(kept["paletteReport"]["paletteColors"], json!(4));
    assert_eq!(kept["paletteReport"]["opaquePixels"], json!(1));
    assert_eq!(kept["paletteReport"]["totalDistance"], json!(0));
    assert_eq!(indices(&source), vec![3]);
    assert_eq!(opaque(&source)[2], Rgba::rgb(3, 0, 0));

    let mut limited = Document::new(1, 1).unwrap();
    let capped = tool(
        &mut limited,
        "convertPng",
        json!({
            "png": png,
            "width": 1,
            "height": 1,
            "engine": "center-sample",
            "palette": { "strategy": "limit" },
        }),
    );
    assert_eq!(capped["paletteReport"]["origin"], json!("sampled"));
    assert_eq!(capped["paletteReport"]["paletteColors"], json!(1));
    assert_eq!(opaque(&limited), vec![Rgba::rgb(3, 0, 0)]);
}

#[test]
fn legacy_source_mode_still_caps_and_has_no_strategy_report() {
    let mut rgba = Vec::new();
    for shade in 0..20u8 {
        rgba.extend_from_slice(&[shade, 0, 0, 255]);
    }
    let png = encode_png(20, 1, &rgba);
    let mut legacy = Document::new(20, 1).unwrap();
    let capped = tool(
        &mut legacy,
        "convertPng",
        json!({
            "png": png,
            "width": 20,
            "height": 1,
            "palette": { "mode": "source" },
        }),
    );
    assert!(capped.get("paletteReport").is_none());
    assert!(capped["transform"]["palette"].get("strategy").is_none());
    assert_eq!(capped["transform"]["palette"]["maxColors"], json!(16));
    assert_eq!(legacy.palette_colors().len(), 17);

    let mut full = Document::new(20, 1).unwrap();
    let kept = tool(
        &mut full,
        "convertPng",
        json!({
            "png": png,
            "width": 20,
            "height": 1,
            "palette": { "strategy": "source" },
        }),
    );
    assert_eq!(kept["paletteReport"]["paletteColors"], json!(20));
    assert_eq!(kept["paletteReport"]["totalDistance"], json!(0));
    assert_eq!(full.palette_colors().len(), 21);
}

#[test]
fn unknown_strategy_writes_nothing_and_does_not_fall_back() {
    let png = encode_png(1, 1, &[9, 8, 7, 255]);
    let mut document = Document::new(4, 4).unwrap();
    let before = document.save_project();
    for id in ["no-such", "SOURCE", ""] {
        let error = tool_err(
            &mut document,
            "convertPng",
            json!({ "png": png, "palette": { "strategy": id } }),
        );
        assert_eq!(error["code"], json!("PaletteStrategyUnknown"), "{id}");
        assert!(error["message"]
            .as_str()
            .unwrap()
            .contains("is not registered"));
        assert_eq!(document.save_project(), before);
    }
    let both = tool_err(
        &mut document,
        "convertPng",
        json!({
            "png": png,
            "palette": { "strategy": "source", "mode": "project" },
        }),
    );
    assert_eq!(both["code"], json!("InvalidParams"));
    let extra = tool_err(
        &mut document,
        "convertPng",
        json!({
            "png": png,
            "palette": { "strategy": "source", "maxColors": 4 },
        }),
    );
    assert_eq!(extra["code"], json!("InvalidParams"));
    assert_eq!(document.save_project(), before);

    tool(&mut document, "setPaletteLocked", json!({"locked": true}));
    let locked = tool_err(
        &mut document,
        "convertPng",
        json!({ "png": png, "palette": { "strategy": "source" } }),
    );
    assert_eq!(locked["code"], json!("PaletteLocked"));
    let allowed = tool(
        &mut document,
        "convertPng",
        json!({
            "png": png,
            "width": 1,
            "height": 1,
            "palette": { "strategy": "project" },
        }),
    );
    assert_eq!(allowed["paletteReport"]["strategy"], json!("project"));
    assert!(document.palette_locked());

    let too_many = {
        let mut rgba = Vec::new();
        for y in 0..16u8 {
            for x in 0..16u8 {
                rgba.extend_from_slice(&[x, y, 0, 255]);
            }
        }
        encode_png(16, 16, &rgba)
    };
    document.undo().unwrap();
    let refused = tool_err(
        &mut document,
        "convertPng",
        json!({
            "png": too_many,
            "width": 16,
            "height": 16,
            "palette": { "strategy": "source" },
        }),
    );
    assert_eq!(refused["code"], json!("PaletteTooLarge"));
    assert!(!document.save_project().contains("\"strategy\""));

    let mut limited = Document::new(16, 16).unwrap();
    let mut again = Document::new(16, 16).unwrap();
    let reduced = tool(
        &mut limited,
        "convertPng",
        json!({
            "png": too_many,
            "width": 16,
            "height": 16,
            "palette": { "strategy": "limit", "maxColors": 255 },
        }),
    );
    let reduced_again = tool(
        &mut again,
        "convertPng",
        json!({
            "png": too_many,
            "width": 16,
            "height": 16,
            "palette": { "strategy": "limit", "maxColors": 255 },
        }),
    );
    assert_eq!(reduced["paletteReport"], reduced_again["paletteReport"]);
    assert_eq!(indices(&limited), indices(&again));
    assert_eq!(reduced["paletteReport"]["sourceColors"], json!(256));
    assert_eq!(reduced["paletteReport"]["paletteColors"], json!(255));
    assert_eq!(reduced["paletteReport"]["inexactPixels"], json!(1));
    assert_eq!(reduced["paletteReport"]["maxDistance"], json!(1));
    assert_eq!(
        reduced["paletteReport"]["merges"][0]["from"],
        json!({"r": 15, "g": 15, "b": 0})
    );
    assert_eq!(
        reduced["paletteReport"]["merges"][0]["into"],
        json!({"r": 15, "g": 14, "b": 0})
    );

    let bad = "{\"format\":\"pixelforge-project\",\"version\":1,\"width\":1,\"height\":1,\"activeFrame\":0,\"palette\":[{\"r\":0,\"g\":0,\"b\":0,\"a\":0},{\"r\":1,\"g\":0,\"b\":0,\"a\":255}],\"frames\":[{\"pixels\":[1],\"delayMs\":100}],\"transform\":{\"id\":\"center-sample\",\"version\":\"1\",\"width\":1,\"height\":1,\"palette\":{\"mode\":\"source\",\"strategy\":\"no-such\"},\"params\":{}}}";
    match Document::load_project(bad) {
        Err(KernelError::ProjectFile(ProjectFileError::Transform)) => {}
        other => panic!("expected Transform, got {other:?}"),
    }
}
