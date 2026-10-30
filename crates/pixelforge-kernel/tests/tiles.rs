//! Tileset size, 16 autotile masks, flips, and Tiled export.

use pixelforge_kernel::{
    parse_autotile, AutotileRules, Document, TileCell, TileMap, AUTOTILE_EAST, AUTOTILE_NORTH,
    AUTOTILE_SOUTH, AUTOTILE_WEST, FLIPPED_HORIZONTALLY_FLAG, FLIPPED_VERTICALLY_FLAG,
    TILESET_FORMAT,
};
use serde_json::{json, Value};

fn rules(tile_of: impl Fn(u8) -> u32) -> AutotileRules {
    let mut list = Vec::new();
    for mask in 0..16u8 {
        list.push(json!({
            "mask": mask,
            "tile": tile_of(mask),
            "flipH": mask % 2 == 1,
            "flipV": mask >= 8,
        }));
    }
    let text = json!({
        "format": "pixelforge-autotile",
        "version": 1,
        "neighbors": 4,
        "bits": ["n", "e", "s", "w"],
        "rules": list,
    })
    .to_string();
    parse_autotile(&text).expect("16 masks")
}

fn occupy(mask: u8) -> TileMap {
    let mut map = TileMap::new(3, 3, 16, 16, 16, "tiles.json", "wall").unwrap();
    let mut cells = vec![(1u16, 1u16)];
    if mask & AUTOTILE_NORTH != 0 {
        cells.push((1, 0));
    }
    if mask & AUTOTILE_EAST != 0 {
        cells.push((2, 1));
    }
    if mask & AUTOTILE_SOUTH != 0 {
        cells.push((1, 2));
    }
    if mask & AUTOTILE_WEST != 0 {
        cells.push((0, 1));
    }
    for (x, y) in cells {
        map.set(0, x, y, Some(TileCell::plain(0))).unwrap();
    }
    map
}

#[test]
fn sixteen_masks_are_deterministic_and_carry_flips() {
    let rules = rules(|mask| u32::from(mask));
    for mask in 0..16u8 {
        let mut map = occupy(mask);
        let before = map.save();
        map.autotile(0, &rules).unwrap();
        let center = map.get(0, 1, 1).unwrap().unwrap();
        assert_eq!(center.tile, u32::from(mask), "mask {mask}");
        assert_eq!(center.flip_h, mask % 2 == 1, "mask {mask}");
        assert_eq!(center.flip_v, mask >= 8, "mask {mask}");
        assert_eq!(
            map.autotile(0, &rules).unwrap(),
            0,
            "mask {mask} second pass"
        );
        let again = map.save();
        map.autotile(0, &rules).unwrap();
        assert_eq!(map.save(), again);

        let mut doc = Document::new(16, 16).unwrap();
        doc.set_tileset(16, 16).unwrap();
        for _ in 1..16 {
            doc.add_frame();
        }
        let export = map
            .export_with_document(&doc, "tiles.png", None, 1)
            .unwrap();
        let gid = export.tmj["layers"][0]["data"][4].as_u64().unwrap() as u32;
        let mut expect = u32::from(mask) + 1;
        if center.flip_h {
            expect |= FLIPPED_HORIZONTALLY_FLAG;
        }
        if center.flip_v {
            expect |= FLIPPED_VERTICALLY_FLAG;
        }
        assert_eq!(gid, expect, "mask {mask}");
        assert_eq!(export.tmj["type"], "map");
        assert_eq!(export.tmj["orientation"], "orthogonal");
        assert_eq!(export.tmj["tilewidth"], 16);
        assert_eq!(export.tmj["tilesets"][0]["firstgid"], 1);
        assert_eq!(export.tmj["tilesets"][0]["image"], "tiles.png");
        assert_eq!(export.tmj["layers"][0]["data"].as_array().unwrap().len(), 9);
        if mask == 0 {
            assert_eq!(before, map.save());
        } else {
            assert_ne!(before, map.save());
        }
    }
}

#[test]
fn missing_mask_and_bad_tile_write_nothing() {
    let mut list = Vec::new();
    for mask in 0..16u8 {
        if mask == 9 {
            continue;
        }
        list.push(json!({"mask": mask, "tile": 0}));
    }
    let text = json!({
        "format": "pixelforge-autotile",
        "version": 1,
        "neighbors": 4,
        "rules": list,
    })
    .to_string();
    let error = parse_autotile(&text).unwrap_err();
    assert_eq!(error.code(), "AutotileRule");
    assert!(error.to_string().contains("9"), "{error}");

    let eight = json!({
        "format": "pixelforge-autotile",
        "version": 1,
        "neighbors": 8,
        "rules": [],
    })
    .to_string();
    assert_eq!(
        parse_autotile(&eight).unwrap_err().code(),
        "AutotileNeighbors"
    );

    let mut map = occupy(0);
    let saved = map.save();
    let text = {
        let mut list = Vec::new();
        for mask in 0..16u8 {
            let tile = if mask == 0 { 16 } else { u32::from(mask) };
            list.push(json!({"mask": mask, "tile": tile}));
        }
        json!({
            "format": "pixelforge-autotile",
            "version": 1,
            "neighbors": 4,
            "rules": list,
        })
        .to_string()
    };
    let bad = parse_autotile(&text).unwrap();
    let error = map.autotile(0, &bad).unwrap_err();
    assert_eq!(error.code(), "TileId");
    assert_eq!(map.save(), saved);

    let error = map.set(0, 1, 1, Some(TileCell::plain(16))).unwrap_err();
    assert_eq!(error.code(), "TileId");
    assert_eq!(map.get(0, 1, 1).unwrap().unwrap().tile, 0);
    assert!(map.fill(0, 1, 1, 3, 1, Some(TileCell::plain(1))).is_err());
    assert!(map.get(0, 2, 1).unwrap().is_none());
}

#[test]
fn flips_move_pixels_and_tileset_export_matches_the_sheet() {
    let mut doc = Document::new(16, 16).unwrap();
    assert_eq!(
        doc.export_tileset(1, None, "tiles.png").unwrap_err().code(),
        "TilesetMissing"
    );
    assert_eq!(doc.set_tileset(32, 32).unwrap_err().code(), "TileAlign");
    assert_eq!(doc.set_tileset(16, 48).unwrap_err().code(), "TileAlign");
    doc.set_tileset(16, 16).unwrap();
    doc.paint(0, 0, 1).unwrap();
    doc.add_frame();
    let sheet = doc.export_tileset(1, None, "tiles.png").unwrap();
    assert_eq!(sheet.tile_count, 2);
    assert_eq!(sheet.columns, 2);
    assert_eq!(sheet.rows, 1);
    assert_eq!(sheet.image_width, 32);
    assert_eq!(sheet.image_height, 16);
    assert_eq!(sheet.meta["format"], TILESET_FORMAT);
    assert_eq!(sheet.meta["tiles"][1]["x"], 16);
    let (width, height, rgba) = decode_png(&sheet.png);
    assert_eq!((width, height), (32, 16));
    assert_eq!(&rgba[0..4], &[26, 28, 36, 255]);

    let mut map = TileMap::new(1, 1, 16, 16, 2, "tiles.json", "ground").unwrap();
    map.set(
        0,
        0,
        0,
        Some(TileCell {
            tile: 0,
            flip_h: true,
            flip_v: false,
        }),
    )
    .unwrap();
    let export = map
        .export_with_document(&doc, "tiles.png", Some(2), 1)
        .unwrap();
    assert_eq!(export.width, 16);
    assert_eq!(export.height, 16);
    let (_w, _h, preview) = decode_png(&export.png);
    let flipped = &preview[(15 * 4)..(16 * 4)];
    assert_eq!(flipped, &[26, 28, 36, 255]);
    assert_eq!(&preview[0..4], &[0, 0, 0, 0]);
    let gid = export.tmj["layers"][0]["data"][0].as_u64().unwrap();
    assert_eq!(gid, u64::from(1 | FLIPPED_HORIZONTALLY_FLAG));
    assert_eq!(export.tmj["tilesets"][0]["columns"], 2);
    assert_eq!(export.tmj["tilesets"][0]["tilecount"], 2);

    let loaded = TileMap::load(&map.save()).unwrap();
    assert_eq!(loaded.get(0, 0, 0).unwrap().unwrap().flip_h, true);

    doc.resize_canvas(32, 32, 0, 0).unwrap();
    assert_eq!(doc.validate_tileset().unwrap_err().code(), "TileAlign");
    doc.undo().unwrap();
    assert_eq!(doc.validate_tileset().unwrap().tile_count, 2);
    let saved = doc.save_project();
    assert!(saved.contains("\"tileset\""));
    let round = Document::load_project(&saved).unwrap();
    assert_eq!(round.tileset().unwrap().tile_width, 16);
    let mut cleared = round;
    cleared.clear_tileset();
    assert!(!cleared.save_project().contains("\"tileset\""));
    cleared.undo().unwrap();
    assert!(cleared.tileset().is_some());

    let mut broken: Value = serde_json::from_str(&saved).unwrap();
    broken["tileset"]["tileWidth"] = json!(15);
    assert_eq!(
        Document::load_project(&broken.to_string())
            .unwrap_err()
            .code(),
        "ProjectFile"
    );
}

#[test]
fn tilemap_tool_rejects_an_unknown_field_and_undoes() {
    let mut map = TileMap::new(2, 2, 32, 32, 4, "camp/tiles.json", "ground").unwrap();
    let response = call(
        &mut map,
        json!({"tool": "fillTiles", "params": {"x": 0, "y": 0, "width": 2, "height": 1, "tile": 3, "rgb": 1}}),
    );
    assert_eq!(response["ok"], false);
    assert_eq!(response["error"]["code"], "InvalidParams");
    let response = call(
        &mut map,
        json!({"tool": "fillTiles", "params": {"x": 0, "y": 0, "width": 2, "height": 2, "tile": 3, "flipV": true}}),
    );
    assert_eq!(response["ok"], true);
    assert_eq!(response["result"]["changed"], 4);
    let response = call(&mut map, json!({"tool": "undo"}));
    assert_eq!(response["ok"], true);
    assert!(map.get(0, 1, 1).unwrap().is_none());
    let response = call(&mut map, json!({"tool": "redo"}));
    assert_eq!(map.get(0, 0, 0).unwrap().unwrap().flip_v, true);
    assert_eq!(response["result"]["canUndo"], true);
}

fn call(map: &mut TileMap, body: Value) -> Value {
    serde_json::from_str(&map.call_tool(&body.to_string())).unwrap()
}

fn decode_png(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().expect("png");
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).expect("frame");
    buf.truncate(info.buffer_size());
    (info.width, info.height, buf)
}
