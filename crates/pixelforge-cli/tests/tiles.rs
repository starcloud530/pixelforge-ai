//! CLI coverage for tileset export, 4-neighbor autotile, flips, and `.tmj`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

fn pixelforge(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pixelforge"))
        .args(args)
        .output()
        .expect("spawn pixelforge")
}

fn status(output: &Output) -> i32 {
    output.status.code().expect("exit")
}

fn json_line(output: &Output) -> Value {
    let text = String::from_utf8(output.stdout.clone()).expect("utf-8");
    serde_json::from_str(text.lines().next().unwrap()).unwrap_or_else(|err| {
        panic!("{err} in {}", text);
    })
}

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "pixelforge-tiles-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn child(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn rules_text(bad_tile: Option<u32>) -> String {
    let mut rules = Vec::new();
    for mask in 0..16 {
        let tile = if bad_tile.is_some() && mask == 0 {
            bad_tile.unwrap()
        } else {
            mask
        };
        rules.push(format!(
            r#"{{"mask":{mask},"tile":{tile},"flipH":{},"flipV":{}}}"#,
            mask % 2 == 1,
            mask >= 8
        ));
    }
    format!(
        r#"{{"format":"pixelforge-autotile","version":1,"neighbors":4,"bits":["n","e","s","w"],"rules":[{}]}}"#,
        rules.join(",")
    )
}

#[test]
fn tileset_autotile_and_tmj_round_trip() {
    let scratch = Scratch::new();
    let project = scratch.child("tiles.json");
    let map = scratch.child("map.json");
    let rules = scratch.child("wall.json");
    let png = scratch.child("tiles.png");
    let meta = scratch.child("tiles.meta.json");
    let preview = scratch.child("map.png");
    let tmj = scratch.child("base.tmj");
    let project_s = project.to_str().unwrap();
    let map_s = map.to_str().unwrap();

    assert_eq!(
        status(&pixelforge(&[
            "new", "--width", "16", "--height", "16", "--output", project_s
        ])),
        0
    );
    assert_eq!(
        status(&pixelforge(&[
            "tileset",
            "set",
            "--project",
            project_s,
            "--tile-width",
            "16",
            "--tile-height",
            "16"
        ])),
        0
    );
    assert_eq!(
        status(&pixelforge(&[
            "paint",
            "--project",
            project_s,
            "--x",
            "0",
            "--y",
            "0",
            "--color-index",
            "1"
        ])),
        0
    );
    for _ in 1..16 {
        assert_eq!(
            status(&pixelforge(&["frame", "add", "--project", project_s])),
            0
        );
    }
    let made = pixelforge(&[
        "tilemap",
        "new",
        "--width",
        "3",
        "--height",
        "3",
        "--tile-width",
        "16",
        "--tile-height",
        "16",
        "--tileset",
        project_s,
        "--output",
        map_s,
        "--layer",
        "ground",
        "--layer",
        "wall",
    ]);
    assert_eq!(
        status(&made),
        0,
        "{}",
        String::from_utf8_lossy(&made.stderr)
    );
    assert_eq!(json_line(&made)["result"]["layerCount"], 2);

    let filled = pixelforge(&[
        "tilemap", "fill", "--map", map_s, "--name", "wall", "--x", "0", "--y", "0", "--width",
        "3", "--height", "3", "--tile", "1",
    ]);
    assert_eq!(
        status(&filled),
        0,
        "{}",
        String::from_utf8_lossy(&filled.stderr)
    );

    fs::write(&rules, rules_text(None)).unwrap();
    let applied = pixelforge(&[
        "autotile",
        "apply",
        "--map",
        map_s,
        "--rules",
        rules.to_str().unwrap(),
        "--name",
        "wall",
    ]);
    assert_eq!(
        status(&applied),
        0,
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    assert_eq!(json_line(&applied)["result"]["changed"], 9);

    let center = pixelforge(&[
        "tilemap", "get", "--map", map_s, "--name", "wall", "--x", "1", "--y", "1",
    ]);
    assert_eq!(status(&center), 0);
    assert_eq!(json_line(&center)["result"]["tile"], 15);
    assert_eq!(json_line(&center)["result"]["flipH"], true);
    assert_eq!(json_line(&center)["result"]["flipV"], true);

    let flipped = pixelforge(&[
        "tilemap", "set", "--map", map_s, "--name", "ground", "--x", "0", "--y", "0", "--tile",
        "0", "--flip-h",
    ]);
    assert_eq!(
        status(&flipped),
        0,
        "{}",
        String::from_utf8_lossy(&flipped.stderr)
    );

    let sheet = pixelforge(&[
        "export",
        "tileset",
        "--project",
        project_s,
        "--output",
        png.to_str().unwrap(),
        "--meta",
        meta.to_str().unwrap(),
        "--image",
        "tiles.png",
    ]);
    assert_eq!(
        status(&sheet),
        0,
        "{}",
        String::from_utf8_lossy(&sheet.stderr)
    );
    assert_eq!(json_line(&sheet)["result"]["tileCount"], 16);
    assert_eq!(json_line(&sheet)["result"]["imageWidth"], 256);

    let exported = pixelforge(&[
        "export",
        "tilemap",
        "--map",
        map_s,
        "--tileset",
        project_s,
        "--output",
        preview.to_str().unwrap(),
        "--tmj",
        tmj.to_str().unwrap(),
        "--image",
        "tiles.png",
    ]);
    assert_eq!(
        status(&exported),
        0,
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    let tmj_value: Value = serde_json::from_str(&fs::read_to_string(&tmj).unwrap()).unwrap();
    assert_eq!(tmj_value["type"], "map");
    assert_eq!(tmj_value["tilewidth"], 16);
    assert_eq!(tmj_value["tilesets"][0]["image"], "tiles.png");
    assert_eq!(tmj_value["tilesets"][0]["tilecount"], 16);
    assert_eq!(tmj_value["layers"][1]["data"].as_array().unwrap().len(), 9);
    let center_gid = tmj_value["layers"][1]["data"][4].as_u64().unwrap();
    let flags = 0x8000_0000u64 | 0x4000_0000u64;
    assert_eq!(center_gid, 16 | flags);
    let ground_gid = tmj_value["layers"][0]["data"][0].as_u64().unwrap();
    assert_eq!(ground_gid, 1 | 0x8000_0000);
    assert!(png.exists() && preview.exists() && meta.exists());

    let checked = pixelforge(&[
        "validate-tiles",
        "--project",
        project_s,
        "--map",
        map_s,
        "--rules",
        rules.to_str().unwrap(),
    ]);
    assert_eq!(
        status(&checked),
        0,
        "{}",
        String::from_utf8_lossy(&checked.stdout)
    );
    assert_eq!(json_line(&checked)["result"]["autotile"]["masks"], 16);

    fs::write(
        &rules,
        rules_text(None).replace(r#"{"mask":9,"tile":9,"flipH":true,"flipV":true}"#, ""),
    )
    .unwrap();
    let broken = fs::read_to_string(&rules).unwrap().replace(",,", ",");
    fs::write(&rules, broken).unwrap();
    let refused = pixelforge(&["validate-tiles", "--rules", rules.to_str().unwrap()]);
    assert_eq!(
        status(&refused),
        1,
        "{}",
        String::from_utf8_lossy(&refused.stdout)
    );
    assert_eq!(json_line(&refused)["error"]["code"], "AutotileRule");

    let tools = pixelforge(&["tools"]);
    assert_eq!(status(&tools), 0);
    let names = json_line(&tools)["result"]["tools"].to_string();
    assert!(names.contains("exportTileset"));
    assert!(names.contains("setTileset"));
    let tile_tools = json_line(&tools)["result"]["tilemapTools"].to_string();
    assert!(tile_tools.contains("autotile"));
    assert!(tile_tools.contains("exportTilemap"));
    let _ = Path::new(map_s);
}

#[test]
fn base_tiles_example_passes_validate_tiles() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base-tiles");
    let output = pixelforge(&[
        "validate-tiles",
        "--project",
        root.join("tileset.json").to_str().unwrap(),
        "--map",
        root.join("map.json").to_str().unwrap(),
        "--rules",
        root.join("wall-rules.json").to_str().unwrap(),
    ]);
    assert_eq!(
        status(&output),
        0,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let body = json_line(&output);
    assert_eq!(body["result"]["tileset"]["tileCount"], 17);
    assert_eq!(body["result"]["tilemap"]["width"], 10);
    assert_eq!(body["result"]["autotile"]["masks"], 16);
    let tmj: Value =
        serde_json::from_str(&fs::read_to_string(root.join("base.tmj")).unwrap()).unwrap();
    assert_eq!(tmj["type"], "map");
    assert_eq!(tmj["tilesets"][0]["image"], "tiles.png");
}
