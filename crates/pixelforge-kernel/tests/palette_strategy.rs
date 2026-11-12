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
    assert_eq!(kept["transform"]["palette"]["mode"], json!("keep"));
    assert_eq!(kept["paletteMode"], json!("keep"));
    assert!(kept["transform"]["palette"].get("maxColors").is_none());
    assert_eq!(kept["paletteReport"]["otherCellsChanged"], json!(0));
    assert_eq!(kept["paletteReport"]["frames"][0]["frame"], json!(0));
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
    assert_eq!(capped["transform"]["palette"]["mode"], json!("limit"));
    assert_eq!(capped["paletteMode"], json!("limit"));
    assert_eq!(capped["transform"]["palette"]["maxColors"], json!(2));
    assert_eq!(capped["paletteReport"]["otherCellsChanged"], json!(0));
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
    assert_eq!(capped["transform"]["palette"]["mode"], json!("source"));
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
    assert_eq!(kept["transform"]["palette"]["mode"], json!("keep"));
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

    tool(
        &mut document,
        "importPalette",
        json!({"text": "ff0000\n00ff00\n", "mode": "replace"}),
    );
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

fn paint_on(document: &mut Document, frame: u32, x: u32, color: u32) {
    tool(document, "setActiveFrame", json!({"index": frame}));
    tool(
        document,
        "paint",
        json!({"x": x, "y": 0, "colorIndex": color}),
    );
}

#[test]
fn source_and_limit_refuse_to_replace_a_palette_other_ink_still_uses() {
    let png = encode_png(2, 1, &[9, 0, 0, 255, 0, 9, 0, 255]);
    let mut document = Document::new(2, 1).unwrap();
    tool(
        &mut document,
        "importPalette",
        json!({"text": "010203\n040506\n070809\n", "mode": "replace"}),
    );
    paint_on(&mut document, 0, 0, 1);
    tool(&mut document, "addFrame", json!({}));
    paint_on(&mut document, 1, 1, 3);
    tool(&mut document, "setActiveFrame", json!({"index": 0}));
    let before = document.save_project();
    for palette in [
        json!({ "strategy": "source" }),
        json!({ "strategy": "limit", "maxColors": 1 }),
    ] {
        let error = tool_err(
            &mut document,
            "convertPng",
            json!({
                "png": png,
                "width": 2,
                "height": 1,
                "frame": 0,
                "palette": palette,
            }),
        );
        assert_eq!(error["code"], json!("PaletteInUse"));
        assert_eq!(document.save_project(), before);
    }

    let legacy = tool(
        &mut document,
        "convertPng",
        json!({
            "png": png,
            "width": 2,
            "height": 1,
            "frame": 0,
            "palette": { "mode": "source", "maxColors": 2 },
        }),
    );
    assert!(legacy.get("paletteReport").is_none());
    assert_eq!(legacy["transform"]["palette"]["mode"], json!("source"));
    assert!(legacy["transform"]["palette"].get("strategy").is_none());
}

#[test]
fn ink_on_another_layer_blocks_a_palette_replacement() {
    let png = encode_png(2, 1, &[1, 2, 3, 255, 4, 5, 6, 255]);
    let mut document = Document::new(2, 1).unwrap();
    tool(
        &mut document,
        "importPalette",
        json!({"text": "111111\n222222\n", "mode": "replace"}),
    );
    tool(
        &mut document,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 1}),
    );
    tool(&mut document, "addLayer", json!({"name": "gear"}));
    tool(
        &mut document,
        "paint",
        json!({"x": 1, "y": 0, "colorIndex": 2}),
    );
    let before = document.save_project();
    let error = tool_err(
        &mut document,
        "convertPng",
        json!({
            "png": png,
            "palette": { "strategy": "source" },
            "layer": 0,
        }),
    );
    assert_eq!(error["code"], json!("PaletteInUse"));
    assert_eq!(document.save_project(), before);
}

#[test]
fn empty_frames_can_take_a_new_palette_and_the_report_lists_only_them() {
    let png = encode_png(2, 1, &[8, 0, 0, 255, 0, 8, 0, 255]);
    let mut document = Document::new(2, 1).unwrap();
    tool(
        &mut document,
        "paint",
        json!({"x": 0, "y": 0, "colorIndex": 1}),
    );
    tool(&mut document, "addFrame", json!({}));
    tool(&mut document, "addFrame", json!({}));
    let converted = tool(
        &mut document,
        "convertPng",
        json!({
            "png": png,
            "width": 2,
            "height": 1,
            "frame": 0,
            "palette": { "strategy": "source" },
        }),
    );
    assert_eq!(converted["paletteReport"]["otherCellsChanged"], json!(0));
    assert_eq!(
        converted["paletteReport"]["frames"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(converted["paletteReport"]["frames"][0]["frame"], json!(0));
    assert!(
        converted["paletteReport"]["frames"][0]["pixelsChanged"]
            .as_u64()
            .unwrap()
            > 0
    );
    for frame in [1, 2] {
        tool(&mut document, "setActiveFrame", json!({"index": frame}));
        assert!(indices(&document).iter().all(|index| *index == 0));
    }
}

#[test]
fn project_strategy_refuses_the_builtin_default_and_keeps_other_frames() {
    let png = encode_png(2, 1, &[250, 0, 0, 255, 0, 0, 250, 255]);
    let mut fresh = Document::new(2, 1).unwrap();
    let before = fresh.save_project();
    let error = tool_err(
        &mut fresh,
        "convertPng",
        json!({
            "png": png,
            "palette": { "strategy": "project" },
        }),
    );
    assert_eq!(error["code"], json!("PaletteDefault"));
    assert_eq!(fresh.save_project(), before);

    let mut document = Document::new(2, 1).unwrap();
    tool(
        &mut document,
        "importPalette",
        json!({"text": "ff0000\n0000ff\n", "mode": "replace"}),
    );
    paint_on(&mut document, 0, 0, 1);
    tool(&mut document, "addFrame", json!({}));
    paint_on(&mut document, 1, 1, 2);
    tool(&mut document, "setActiveFrame", json!({"index": 1}));
    let other = indices(&document);
    let mapped = tool(
        &mut document,
        "convertPng",
        json!({
            "png": png,
            "width": 2,
            "height": 1,
            "frame": 0,
            "palette": { "strategy": "project" },
        }),
    );
    assert_eq!(mapped["paletteReport"]["otherCellsChanged"], json!(0));
    assert_eq!(mapped["paletteReport"]["strategy"], json!("project"));
    let frames = mapped["paletteReport"]["frames"].as_array().unwrap();
    assert!(frames.iter().all(|frame| frame["frame"] == json!(0)));
    tool(&mut document, "setActiveFrame", json!({"index": 1}));
    assert_eq!(indices(&document), other);
    assert_eq!(
        opaque(&document),
        vec![Rgba::rgb(255, 0, 0), Rgba::rgb(0, 0, 255)]
    );
}

#[test]
fn a_capped_convert_is_not_recorded_as_keeping_the_source_palette() {
    fn wrapped(palette: &str) -> String {
        format!(
            r#"{{"format":"pixelforge-project","version":1,"width":1,"height":1,"activeFrame":0,"palette":[{{"r":0,"g":0,"b":0,"a":0}},{{"r":1,"g":0,"b":0,"a":255}}],"frames":[{{"pixels":[1],"delayMs":100}}],"transform":{{"id":"center-sample","version":"1","width":1,"height":1,"palette":{palette},"params":{{}}}}}}"#
        )
    }
    let limit = Document::load_project(&wrapped(
        r#"{"mode":"source","strategy":"limit","maxColors":4}"#,
    ))
    .unwrap();
    let saved: Value = serde_json::from_str(&limit.save_project()).unwrap();
    assert_eq!(saved["transform"]["palette"]["mode"], json!("limit"));
    assert_eq!(saved["transform"]["palette"]["strategy"], json!("limit"));
    assert_eq!(saved["transform"]["palette"]["maxColors"], json!(4));

    let keep =
        Document::load_project(&wrapped(r#"{"mode":"source","strategy":"source"}"#)).unwrap();
    let saved: Value = serde_json::from_str(&keep.save_project()).unwrap();
    assert_eq!(saved["transform"]["palette"]["mode"], json!("keep"));
    assert_eq!(saved["transform"]["palette"]["strategy"], json!("source"));
    assert!(saved["transform"]["palette"].get("maxColors").is_none());

    let already =
        Document::load_project(&wrapped(r#"{"mode":"keep","strategy":"source"}"#)).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&already.save_project()).unwrap()["transform"]["palette"]
            ["mode"],
        json!("keep")
    );

    let legacy = Document::load_project(&wrapped(r#"{"mode":"source","maxColors":8}"#)).unwrap();
    let saved: Value = serde_json::from_str(&legacy.save_project()).unwrap();
    assert_eq!(saved["transform"]["palette"]["mode"], json!("source"));
    assert!(saved["transform"]["palette"].get("strategy").is_none());

    assert!(matches!(
        Document::load_project(&wrapped(r#"{"mode":"keep"}"#)),
        Err(KernelError::ProjectFile(ProjectFileError::Transform))
    ));
}
