//! Tileset, tilemap, and autotile commands.
//!
//! Pixel edits stay on the project session. A tilemap file is a different
//! document, so these commands call [`TileMap::call_tool`] and
//! [`Document::export_tileset`] instead of the pixel session.

use std::fs;
use std::path::Path;

use pixelforge_kernel::{parse_autotile, Document, TileMap, TOOL_API_VERSION};
use serde_json::{json, Value};

use crate::cli::{
    AutotileCmd, MapArgs, TileRectArgs, TileRefArgs, TilemapCmd, TilesetCmd, ValidateTilesArgs,
};
use crate::failure::CliFailure;
use crate::output::{annotate_revision, emit, print_line, tool_refusal};
use crate::session::{Access, Session};
use crate::store::{self, ProjectLock};

pub fn tileset(command: TilesetCmd) -> i32 {
    match command {
        TilesetCmd::Set {
            doc,
            tile_width,
            tile_height,
        } => {
            let mut session = match open_session(
                &doc.project,
                doc.checkpoint.as_deref(),
                doc.expected_revision.as_deref(),
                Access::Exclusive,
            ) {
                Ok(session) => session,
                Err(failure) => return emit(failure),
            };
            let response = session.call(&request(
                "setTileset",
                json!({"tileWidth": tile_width, "tileHeight": tile_height}),
            ));
            finish_session(&mut session, &response)
        }
        TilesetCmd::Clear { doc } => {
            let mut session = match open_session(
                &doc.project,
                doc.checkpoint.as_deref(),
                doc.expected_revision.as_deref(),
                Access::Exclusive,
            ) {
                Ok(session) => session,
                Err(failure) => return emit(failure),
            };
            let response = session.call(&request("clearTileset", json!({})));
            finish_session(&mut session, &response)
        }
        TilesetCmd::Validate { doc } => {
            let mut session = match open_session(
                &doc.project,
                doc.checkpoint.as_deref(),
                doc.expected_revision.as_deref(),
                Access::ReadOnly,
            ) {
                Ok(session) => session,
                Err(failure) => return emit(failure),
            };
            let response = session.call(&request("validateTileset", json!({})));
            finish_session(&mut session, &response)
        }
    }
}

pub fn tilemap(command: TilemapCmd) -> i32 {
    match command {
        TilemapCmd::New {
            width,
            height,
            tile_width,
            tile_height,
            tileset,
            output,
            layers,
        } => tilemap_new(
            width,
            height,
            tile_width,
            tile_height,
            &tileset,
            &output,
            &layers,
        ),
        TilemapCmd::Inspect { map } => one_call(&map, request("inspectTilemap", json!({})), false),
        TilemapCmd::Get(args) => one_call(
            &args.map,
            request("getTile", cell_params(&args, None)),
            false,
        ),
        TilemapCmd::Set(args) => {
            let mut params = cell_params(&args.cell, None);
            params["tile"] = match args.tile {
                Some(tile) => json!(tile),
                None => Value::Null,
            };
            if args.tile.is_some() {
                if args.flip_h {
                    params["flipH"] = json!(true);
                }
                if args.flip_v {
                    params["flipV"] = json!(true);
                }
            }
            one_call(&args.cell.map, request("setTile", params), true)
        }
        TilemapCmd::Fill(args) => {
            let mut params = rect_params(&args.rect);
            params["tile"] = json!(args.tile);
            if args.flip_h {
                params["flipH"] = json!(true);
            }
            if args.flip_v {
                params["flipV"] = json!(true);
            }
            one_call(&args.rect.cell.map, request("fillTiles", params), true)
        }
        TilemapCmd::Clear(args) => one_call(
            &args.cell.map,
            request("clearTiles", rect_params(&args)),
            true,
        ),
        TilemapCmd::Layer { command } => match command {
            crate::cli::TileLayerCmd::Add { map, name } => {
                one_call(&map, request("addTileLayer", json!({"name": name})), true)
            }
        },
    }
}

pub fn autotile(command: AutotileCmd) -> i32 {
    match command {
        AutotileCmd::Apply {
            map,
            rules,
            layer,
            name,
        } => {
            let text = match fs::read_to_string(&rules) {
                Ok(text) => text,
                Err(err) => return emit(CliFailure::io(&rules, err)),
            };
            let mut params = json!({"rules": text});
            if let Some(layer) = layer {
                params["layer"] = json!(layer);
            }
            if let Some(name) = name {
                params["name"] = json!(name);
            }
            one_call(&map, request("autotile", params), true)
        }
        AutotileCmd::Validate { rules, tile_count } => {
            let text = match fs::read_to_string(&rules) {
                Ok(text) => text,
                Err(err) => return emit(CliFailure::io(&rules, err)),
            };
            match parse_autotile(&text) {
                Ok(parsed) => {
                    if let Some(tile_count) = tile_count {
                        if let Err(error) = parsed.check_tiles(tile_count) {
                            return kernel_line(error);
                        }
                    }
                    print_ok(json!({"masks": 16, "tileCount": tile_count}))
                }
                Err(error) => kernel_line(error),
            }
        }
    }
}

pub fn validate(args: ValidateTilesArgs) -> i32 {
    if args.project.is_none() && args.map.is_none() && args.rules.is_none() {
        return emit(CliFailure::new("Cli", "pass --project, --map, or --rules"));
    }
    let mut result = serde_json::Map::new();
    let report = if let Some(path) = &args.project {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) => return emit(CliFailure::io(path, err)),
        };
        let document = match Document::load_project(&text) {
            Ok(document) => document,
            Err(error) => return kernel_line(error),
        };
        match document.validate_tileset() {
            Ok(report) => {
                result.insert(
                    "tileset".to_string(),
                    json!({
                        "tileWidth": report.tile_width,
                        "tileHeight": report.tile_height,
                        "tileCount": report.tile_count,
                    }),
                );
                Some(report)
            }
            Err(error) => return kernel_line(error),
        }
    } else {
        None
    };
    let map = if let Some(path) = &args.map {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) => return emit(CliFailure::io(path, err)),
        };
        let map = match TileMap::load(&text) {
            Ok(map) => map,
            Err(error) => return kernel_line(error),
        };
        if let Some(report) = &report {
            if let Err(error) = map.validate_tileset(report) {
                return kernel_line(error);
            }
        } else if let Err(error) = map.validate() {
            return kernel_line(error);
        }
        result.insert(
            "tilemap".to_string(),
            json!({
                "width": map.width(),
                "height": map.height(),
                "tileWidth": map.tile_width(),
                "tileHeight": map.tile_height(),
                "tileCount": map.tile_count(),
                "layers": map.layer_count(),
            }),
        );
        Some(map)
    } else {
        None
    };
    if let Some(path) = &args.rules {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) => return emit(CliFailure::io(path, err)),
        };
        let rules = match parse_autotile(&text) {
            Ok(rules) => rules,
            Err(error) => return kernel_line(error),
        };
        let tile_count = map
            .as_ref()
            .map(TileMap::tile_count)
            .or_else(|| report.as_ref().map(|report| report.tile_count));
        if let Some(tile_count) = tile_count {
            if let Err(error) = rules.check_tiles(tile_count) {
                return kernel_line(error);
            }
        }
        result.insert("autotile".to_string(), json!({"masks": 16}));
    }
    print_ok(Value::Object(result))
}

pub fn export_tileset(
    project: &Path,
    output: &Path,
    meta: &Path,
    scale: u32,
    cols: Option<u32>,
    image: Option<String>,
) -> i32 {
    let text = match fs::read_to_string(project) {
        Ok(text) => text,
        Err(err) => return emit(CliFailure::io(project, err)),
    };
    let document = match Document::load_project(&text) {
        Ok(document) => document,
        Err(error) => return kernel_line(error),
    };
    let image = match image {
        Some(image) => image,
        None => match output.file_name().and_then(|name| name.to_str()) {
            Some(name) => name.to_string(),
            None => return emit(CliFailure::new("Cli", "tileset png name is not UTF-8")),
        },
    };
    let export = match document.export_tileset(scale, cols, &image) {
        Ok(export) => export,
        Err(error) => return kernel_line(error),
    };
    if let Err(err) = fs::write(output, &export.png) {
        return emit(CliFailure::io(output, err));
    }
    eprintln!("pixelforge: wrote {}", output.display());
    let mut meta_text = serde_json::to_string_pretty(&export.meta).expect("tileset meta is JSON");
    meta_text.push('\n');
    if let Err(err) = fs::write(meta, meta_text) {
        return emit(CliFailure::io(meta, err));
    }
    eprintln!("pixelforge: wrote {}", meta.display());
    print_ok(json!({
        "png": path_string(output),
        "meta": path_string(meta),
        "image": export.image,
        "columns": export.columns,
        "rows": export.rows,
        "scale": export.scale,
        "imageWidth": export.image_width,
        "imageHeight": export.image_height,
        "tileWidth": export.tile_width,
        "tileHeight": export.tile_height,
        "tileCount": export.tile_count,
        "bytes": export.png.len(),
    }))
}

pub fn export_tilemap(
    map_path: &Path,
    tileset: &Path,
    output: &Path,
    tmj: &Path,
    scale: u32,
    cols: Option<u32>,
    image: Option<String>,
) -> i32 {
    let map_text = match fs::read_to_string(map_path) {
        Ok(text) => text,
        Err(err) => return emit(CliFailure::io(map_path, err)),
    };
    let mut map = match TileMap::load(&map_text) {
        Ok(map) => map,
        Err(error) => return kernel_line(error),
    };
    let project = match fs::read_to_string(tileset) {
        Ok(text) => text,
        Err(err) => return emit(CliFailure::io(tileset, err)),
    };
    let image = image.unwrap_or_else(|| "tiles.png".to_string());
    let mut params = json!({
        "tilesetProject": project,
        "image": image,
        "scale": scale,
    });
    if let Some(cols) = cols {
        params["columns"] = json!(cols);
    }
    let response = map.call_tool(&request("exportTilemap", params));
    if let Some((code, message)) = tool_refusal(&response) {
        eprintln!("pixelforge: exportTilemap failed: {code}: {message}");
        return 1;
    }
    let value: Value = match serde_json::from_str(&response) {
        Ok(value) => value,
        Err(_) => return emit(CliFailure::new("Cli", "tool response is not JSON")),
    };
    let png = match value["result"]["png"].as_str() {
        Some(text) => match export_base64_decode(text) {
            Ok(bytes) => bytes,
            Err(failure) => return emit(failure),
        },
        None => {
            return emit(CliFailure::new(
                "Cli",
                "exportTilemap result is missing png",
            ))
        }
    };
    if let Err(err) = fs::write(output, &png) {
        return emit(CliFailure::io(output, err));
    }
    eprintln!("pixelforge: wrote {}", output.display());
    let mut tmj_text = serde_json::to_string_pretty(&value["result"]["tmj"]).expect("tmj is JSON");
    tmj_text.push('\n');
    if let Err(err) = fs::write(tmj, tmj_text) {
        return emit(CliFailure::io(tmj, err));
    }
    eprintln!("pixelforge: wrote {}", tmj.display());
    print_ok(json!({
        "png": path_string(output),
        "tmj": path_string(tmj),
        "image": image,
        "width": value["result"]["width"],
        "height": value["result"]["height"],
        "bytes": png.len(),
    }))
}

pub fn apply_call(
    path: &Path,
    requests: Vec<String>,
    preview: bool,
    expected: Option<&str>,
) -> i32 {
    edit_requests(path, false, requests, preview, expected)
}

pub fn apply_call_locked(
    lock: ProjectLock,
    requests: Vec<String>,
    preview: bool,
    expected: Option<&str>,
) -> i32 {
    let path = lock.target().to_path_buf();
    edit_requests(&path, true, requests, preview, expected)
}

fn edit_requests(
    path: &Path,
    writable: bool,
    requests: Vec<String>,
    preview: bool,
    expected: Option<&str>,
) -> i32 {
    if preview {
        return emit(CliFailure::new(
            "Cli",
            "preview reads a pixel frame; a tilemap has no frame",
        ));
    }
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => return emit(CliFailure::io(path, err)),
    };
    let mut map = match TileMap::load(&text) {
        Ok(map) => map,
        Err(error) => return kernel_line(error),
    };
    let original = map.save();
    if let Err(failure) = store::check_revision(expected, &original) {
        return emit(failure);
    }
    let mut status = 0;
    for request in &requests {
        let response = map.call_tool(request);
        let response = annotate_revision(&response, &store::revision_of(&map.save()));
        if !print_line(&response) {
            return 2;
        }
        if let Some((code, message)) = tool_refusal(&response) {
            eprintln!("pixelforge: tool failed: {code}: {message}");
            status = 1;
            break;
        }
    }
    let updated = map.save();
    if updated != original {
        if let Err(failure) = publish_map(path, writable, &updated) {
            return emit(failure);
        }
    }
    status
}

fn tilemap_new(
    width: u16,
    height: u16,
    tile_width: u16,
    tile_height: u16,
    tileset: &Path,
    output: &Path,
    layers: &[String],
) -> i32 {
    if output.exists() {
        return emit(CliFailure::new(
            "Cli",
            format!("tilemap file already exists: {}", output.display()),
        ));
    }
    let text = match fs::read_to_string(tileset) {
        Ok(text) => text,
        Err(err) => return emit(CliFailure::io(tileset, err)),
    };
    let document = match Document::load_project(&text) {
        Ok(document) => document,
        Err(error) => return kernel_line(error),
    };
    let report = match document.validate_tileset() {
        Ok(report) => report,
        Err(error) => return kernel_line(error),
    };
    if report.tile_width != tile_width || report.tile_height != tile_height {
        return kernel_line(pixelforge_kernel::KernelError::TileAlign {
            tile_width: report.tile_width,
            tile_height: report.tile_height,
            width: tile_width,
            height: tile_height,
        });
    }
    let tileset_ref = match tileset_label(tileset) {
        Ok(text) => text,
        Err(failure) => return emit(failure),
    };
    let names = if layers.is_empty() {
        vec!["ground".to_string()]
    } else {
        layers.to_vec()
    };
    let mut map = match TileMap::new(
        width,
        height,
        tile_width,
        tile_height,
        report.tile_count,
        &tileset_ref,
        &names[0],
    ) {
        Ok(map) => map,
        Err(error) => return kernel_line(error),
    };
    for name in names.iter().skip(1) {
        let response = map.call_tool(&request("addTileLayer", json!({"name": name})));
        if tool_refusal(&response).is_some() {
            if !print_line(&response) {
                return 2;
            }
            return 1;
        }
    }
    let lock = match ProjectLock::acquire(output) {
        Ok(lock) => lock,
        Err(failure) => return emit(failure),
    };
    if lock.target().exists() {
        return emit(CliFailure::new(
            "Cli",
            format!("tilemap file already exists: {}", output.display()),
        ));
    }
    let text = map.save();
    if let Err(failure) = store::write_atomic(lock.target(), text.as_bytes()) {
        return emit(failure);
    }
    eprintln!("pixelforge: wrote {}", output.display());
    let response = map.call_tool(&request("inspectTilemap", json!({})));
    if !print_line(&response) {
        return 2;
    }
    0
}

fn one_call(args: &MapArgs, request: String, mutate: bool) -> i32 {
    if !mutate {
        return edit_map(&args.map, false, request);
    }
    let lock = match ProjectLock::acquire(&args.map) {
        Ok(lock) => lock,
        Err(failure) => return emit(failure),
    };
    edit_map(lock.target(), true, request)
}

fn edit_map(path: &Path, writable: bool, request: String) -> i32 {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => return emit(CliFailure::io(path, err)),
    };
    let mut map = match TileMap::load(&text) {
        Ok(map) => map,
        Err(error) => return kernel_line(error),
    };
    let original = map.save();
    let response = map.call_tool(&request);
    if !print_line(&response) {
        return 2;
    }
    let status = if let Some((code, message)) = tool_refusal(&response) {
        eprintln!("pixelforge: tool failed: {code}: {message}");
        1
    } else {
        0
    };
    let updated = map.save();
    if updated != original {
        if let Err(failure) = publish_map(path, writable, &updated) {
            return emit(failure);
        }
    }
    status
}

fn publish_map(path: &Path, writable: bool, text: &str) -> Result<(), CliFailure> {
    if !writable {
        return Err(CliFailure::new(
            "Cli",
            "refusing to write a tilemap opened for reading",
        ));
    }
    store::write_atomic(path, text.as_bytes())?;
    eprintln!("pixelforge: wrote {}", path.display());
    Ok(())
}

fn cell_params(args: &TileRefArgs, extra: Option<Value>) -> Value {
    let mut params = extra.unwrap_or_else(|| json!({}));
    params["x"] = json!(args.x);
    params["y"] = json!(args.y);
    if let Some(layer) = args.layer {
        params["layer"] = json!(layer);
    }
    if let Some(name) = &args.name {
        params["name"] = json!(name);
    }
    params
}

fn rect_params(args: &TileRectArgs) -> Value {
    let mut params = cell_params(&args.cell, None);
    params["width"] = json!(args.width);
    params["height"] = json!(args.height);
    params
}

fn request(tool: &str, params: Value) -> String {
    json!({
        "apiVersion": TOOL_API_VERSION,
        "tool": tool,
        "params": params,
    })
    .to_string()
}

fn open_session(
    path: &Path,
    checkpoint: Option<&Path>,
    expected: Option<&str>,
    access: Access,
) -> Result<Session, CliFailure> {
    Session::open_with(path, checkpoint, expected, access)
}

fn finish_session(session: &mut Session, response: &str) -> i32 {
    if !print_line(response) {
        return 2;
    }
    let status = if let Some((code, message)) = tool_refusal(response) {
        eprintln!("pixelforge: tool failed: {code}: {message}");
        1
    } else {
        0
    };
    match session.save_if_changed() {
        Ok(report) => {
            if report.project {
                eprintln!("pixelforge: wrote {}", session.path().display());
            }
            if report.checkpoint {
                if let Some(path) = session.checkpoint_path() {
                    eprintln!("pixelforge: wrote {}", path.display());
                }
            }
        }
        Err(failure) => return emit(failure),
    }
    status
}

fn kernel_line(error: pixelforge_kernel::KernelError) -> i32 {
    let body = json!({
        "ok": false,
        "apiVersion": TOOL_API_VERSION,
        "error": { "code": error.code(), "message": error.to_string() },
    });
    let line = serde_json::to_string(&body).expect("error is JSON");
    if !print_line(&line) {
        return 2;
    }
    eprintln!("pixelforge: {}: {}", error.code(), error);
    1
}

fn print_ok(result: Value) -> i32 {
    let body = json!({
        "ok": true,
        "apiVersion": TOOL_API_VERSION,
        "result": result,
    });
    let line = serde_json::to_string(&body).expect("result is JSON");
    if print_line(&line) {
        0
    } else {
        2
    }
}

fn tileset_label(path: &Path) -> Result<String, CliFailure> {
    if let Some(text) = path.to_str() {
        if !text.starts_with('/') && !text.contains("..") && !text.contains('\\') {
            return Ok(text.to_string());
        }
    }
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .ok_or_else(|| CliFailure::new("Cli", "tileset path is not UTF-8"))
}

fn path_string(path: &Path) -> String {
    path.display().to_string()
}

pub fn export_base64_decode(text: &str) -> Result<Vec<u8>, CliFailure> {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    STANDARD
        .decode(text)
        .map_err(|_| CliFailure::new("Cli", "export png is not base64"))
}
