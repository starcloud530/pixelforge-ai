//! Local `pixelforge` command.
//!
//! Opens a version 1 `pixelforge-project` file, runs
//! [`pixelforge_kernel::Document::call_tool`], and prints that response on
//! stdout. Ergonomic subcommands only build the Tool API JSON. They do not
//! write a second pixel buffer.

mod cli;
mod export_files;
mod failure;
mod output;
mod preview;
mod requests;
mod session;

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use clap::Parser;
use pixelforge_kernel::{TOOL_API_VERSION, TOOL_NAMES};
use serde_json::{json, Value};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

use crate::cli::{
    AnchorCmd, BlitRectArgs, Cli, Command, EraseRectArgs, ExportCmd, FillRectArgs, FlipArgs,
    FrameCmd, ImportMode, LayerCmd, PaintArgs, PaletteCmd, PartCmd, ProjectArgs, ReplaceIndexArgs,
    ResizeArgs, RotateArgs, SelectionCmd, TagCmd,
};
use crate::failure::CliFailure;
use crate::output::{emit, print_line, request_tool_name, tool_refusal};
use crate::requests::parse_request_blob;
use crate::session::Session;

/// Parse argv and run one command. `0` success, `1` a tool refused, `2` the
/// command did not finish a tool call (usage errors exit from clap itself).
pub fn run() -> i32 {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => err.exit(),
    };
    execute(cli)
}

fn execute(cli: Cli) -> i32 {
    match cli.command {
        Command::New {
            width,
            height,
            output,
        } => cmd_new(width, height, &output),
        Command::ImportAseprite {
            file,
            output,
            project,
        } => cmd_import_aseprite(&file, output.as_deref(), project.as_deref()),
        Command::Inspect { doc, preview } => {
            apply_with(&doc, vec![request("inspect", json!({}))], preview)
        }
        Command::Scene { doc } => apply(&doc, vec![request("scene", json!({}))]),
        Command::Preview {
            doc,
            frame,
            scale,
            output,
            layer,
        } => cmd_preview(&doc, frame, scale, output.as_deref(), layer),
        Command::Diff {
            doc,
            frame,
            with,
            scale,
            output,
            layer,
        } => cmd_diff(&doc, frame, with, scale, output.as_deref(), layer),
        Command::Call {
            doc,
            request,
            request_file,
            preview,
        } => cmd_call(&doc, request, request_file.as_deref(), preview),
        Command::Paint(args) => cmd_paint(args),
        Command::FillRect(args) => cmd_fill_rect(args),
        Command::Erase(args) => apply(
            &args.doc,
            vec![request(
                "erase",
                with_dirty(
                    with_layer(json!({"x": args.x, "y": args.y}), args.layer),
                    args.dirty,
                ),
            )],
        ),
        Command::EraseRect(args) => cmd_erase_rect(args),
        Command::CopyRect(args) => cmd_blit("copyRect", args),
        Command::MoveRect(args) => cmd_blit("moveRect", args),
        Command::Flip(args) => cmd_flip(args, None),
        Command::Hflip(args) => cmd_flip(args, Some(Axis::Horizontal)),
        Command::Vflip(args) => cmd_flip(args, Some(Axis::Vertical)),
        Command::Rotate(args) => cmd_rotate(args),
        Command::Resize(args) => cmd_resize(args),
        Command::Selection { command } => cmd_selection(command),
        Command::ReplaceIndex(args) => cmd_replace_index(args),
        Command::Palette { command } => cmd_palette(command),
        Command::Frame { command } => cmd_frame(command),
        Command::Tag { command } => cmd_tag(command),
        Command::Layer { command } => cmd_layer(command),
        Command::Part { command } => cmd_part(command),
        Command::Undo { doc } => apply(&doc, vec![request("undo", json!({}))]),
        Command::Redo { doc } => apply(&doc, vec![request("redo", json!({}))]),
        Command::Export { command } => cmd_export(command),
        Command::Tools => cmd_tools(),
    }
}

fn cmd_new(width: u16, height: u16, output: &Path) -> i32 {
    if output.exists() {
        return emit(CliFailure::new(
            "Cli",
            format!("project file already exists: {}", output.display()),
        ));
    }
    let mut session = match Session::blank(width, height, output) {
        Ok(session) => session,
        Err(failure) => return emit(failure),
    };
    match session.save_if_changed() {
        Ok(_) => eprintln!("pixelforge: wrote {}", output.display()),
        Err(failure) => return emit(failure),
    }
    let response = session.call(&request("inspect", json!({})));
    if !print_line(&response) {
        return 2;
    }
    0
}

fn cmd_selection(command: SelectionCmd) -> i32 {
    match command {
        SelectionCmd::Set(args) => apply(
            &args.doc,
            vec![request(
                "setSelection",
                json!({
                    "x": args.x,
                    "y": args.y,
                    "width": args.width,
                    "height": args.height,
                }),
            )],
        ),
        SelectionCmd::Clear { doc } => apply(&doc, vec![request("clearSelection", json!({}))]),
    }
}

fn cmd_replace_index(args: ReplaceIndexArgs) -> i32 {
    apply(
        &args.doc,
        vec![request(
            "replaceIndex",
            with_layer(
                json!({
                    "fromIndex": args.from_index,
                    "toIndex": args.to_index,
                }),
                args.layer,
            ),
        )],
    )
}

fn cmd_paint(args: PaintArgs) -> i32 {
    apply_with(
        &args.doc,
        vec![request(
            "paint",
            with_dirty(
                with_layer(
                    json!({
                        "x": args.x,
                        "y": args.y,
                        "colorIndex": args.color_index,
                    }),
                    args.layer,
                ),
                args.dirty,
            ),
        )],
        args.preview,
    )
}

fn cmd_fill_rect(args: FillRectArgs) -> i32 {
    apply_with(
        &args.doc,
        vec![request(
            "fillRect",
            with_dirty(
                with_layer(
                    json!({
                        "x": args.x,
                        "y": args.y,
                        "width": args.width,
                        "height": args.height,
                        "colorIndex": args.color_index,
                    }),
                    args.layer,
                ),
                args.dirty,
            ),
        )],
        args.preview,
    )
}

fn cmd_erase_rect(args: EraseRectArgs) -> i32 {
    apply_with(
        &args.doc,
        vec![request(
            "eraseRect",
            with_dirty(
                with_layer(
                    json!({
                        "x": args.x,
                        "y": args.y,
                        "width": args.width,
                        "height": args.height,
                    }),
                    args.layer,
                ),
                args.dirty,
            ),
        )],
        args.preview,
    )
}

fn cmd_blit(tool: &str, args: BlitRectArgs) -> i32 {
    let mut params = json!({
        "srcX": args.src_x,
        "srcY": args.src_y,
        "width": args.width,
        "height": args.height,
        "dstX": args.dst_x,
        "dstY": args.dst_y,
    });
    if let Some(frame) = args.from_frame {
        params["fromFrame"] = json!(frame);
    }
    if let Some(frame) = args.to_frame {
        params["toFrame"] = json!(frame);
    }
    apply_with(
        &args.doc,
        vec![request(
            tool,
            with_dirty(with_layer(params, args.layer), args.dirty),
        )],
        args.preview,
    )
}

enum Axis {
    Horizontal,
    Vertical,
}

fn cmd_flip(args: FlipArgs, forced: Option<Axis>) -> i32 {
    let axis = match (forced, args.horizontal, args.vertical) {
        (Some(Axis::Horizontal), _, true) => {
            return emit(CliFailure::new(
                "Cli",
                "hflip mirrors left to right; drop --vertical",
            ))
        }
        (Some(Axis::Vertical), true, _) => {
            return emit(CliFailure::new(
                "Cli",
                "vflip mirrors top to bottom; drop --horizontal",
            ))
        }
        (Some(axis), _, _) => axis,
        (None, true, false) => Axis::Horizontal,
        (None, false, true) => Axis::Vertical,
        (None, true, true) => {
            return emit(CliFailure::new(
                "Cli",
                "flip takes one of --horizontal or --vertical",
            ))
        }
        (None, false, false) => {
            return emit(CliFailure::new(
                "Cli",
                "flip needs --horizontal or --vertical; or run hflip / vflip",
            ))
        }
    };
    let tool = match axis {
        Axis::Horizontal => "flipHorizontal",
        Axis::Vertical => "flipVertical",
    };
    let rect = match rect_edges(args.x, args.y, args.width, args.height, "flip") {
        Ok(rect) => rect,
        Err(failure) => return emit(failure),
    };
    let params = mirror_params(args.frame, args.layer, args.dirty, rect);
    apply_with(&args.doc, vec![request(tool, params)], args.preview)
}

fn cmd_rotate(args: RotateArgs) -> i32 {
    if !matches!(args.degrees, 90 | 180 | 270) {
        return emit(CliFailure::new(
            "Cli",
            "rotate --degrees must be 90, 180, or 270",
        ));
    }
    let rect = match rect_edges(args.x, args.y, args.width, args.height, "rotate") {
        Ok(rect) => rect,
        Err(failure) => return emit(failure),
    };
    let mut params = mirror_params(args.frame, args.layer, args.dirty, rect);
    params["degrees"] = json!(args.degrees);
    apply_with(&args.doc, vec![request("rotate", params)], args.preview)
}

fn rect_edges(
    x: Option<u16>,
    y: Option<u16>,
    width: Option<u16>,
    height: Option<u16>,
    command: &str,
) -> Result<Option<(u16, u16, u16, u16)>, CliFailure> {
    match (x, y, width, height) {
        (None, None, None, None) => Ok(None),
        (Some(x), Some(y), Some(width), Some(height)) => Ok(Some((x, y, width, height))),
        _ => Err(CliFailure::new(
            "Cli",
            format!("{command} needs x, y, width, and height together, or none of them"),
        )),
    }
}

fn mirror_params(
    frame: Option<u32>,
    layer: Option<u32>,
    dirty: bool,
    rect: Option<(u16, u16, u16, u16)>,
) -> Value {
    let mut params = json!({});
    if let Some(frame) = frame {
        params["frame"] = json!(frame);
    }
    if let Some((x, y, width, height)) = rect {
        params["x"] = json!(x);
        params["y"] = json!(y);
        params["width"] = json!(width);
        params["height"] = json!(height);
    }
    with_dirty(with_layer(params, layer), dirty)
}

fn cmd_import_aseprite(file: &Path, output: Option<&Path>, project: Option<&Path>) -> i32 {
    let target = match (output, project) {
        (Some(output), None) => output,
        (None, Some(project)) => project,
        _ => {
            return emit(CliFailure::new(
                "Cli",
                "import-aseprite needs --output for a new project, or --project to replace one",
            ))
        }
    };
    if output.is_some() && target.exists() {
        return emit(CliFailure::new(
            "Cli",
            format!("project file already exists: {}", target.display()),
        ));
    }
    let bytes = match fs::read(file) {
        Ok(bytes) => bytes,
        Err(err) => return emit(CliFailure::io(file, err)),
    };
    let params = json!({ "bytes": STANDARD.encode(&bytes) });
    let request = vec![request("importAseprite", params)];
    if output.is_some() {
        let mut session = match Session::placeholder(target) {
            Ok(session) => session,
            Err(failure) => return emit(failure),
        };
        apply_session(&mut session, request, false)
    } else {
        let mut session = match Session::open(target) {
            Ok(session) => session,
            Err(failure) => return emit(failure),
        };
        apply_session(&mut session, request, false)
    }
}

fn cmd_resize(args: ResizeArgs) -> i32 {
    match (args.offset_x, args.offset_y) {
        (None, None) | (Some(_), Some(_)) => {}
        _ => {
            return emit(CliFailure::new(
                "Cli",
                "resize needs --offset-x and --offset-y together, or neither to center",
            ))
        }
    }
    let mut params = json!({
        "width": args.width,
        "height": args.height,
    });
    if let (Some(x), Some(y)) = (args.offset_x, args.offset_y) {
        params["offsetX"] = json!(x);
        params["offsetY"] = json!(y);
    }
    params = with_dirty(params, args.dirty);
    apply_with(
        &args.doc,
        vec![request("resizeCanvas", params)],
        args.preview,
    )
}

fn cmd_preview(
    doc: &ProjectArgs,
    frame: Option<u32>,
    scale: u32,
    output: Option<&Path>,
    layer: Option<u32>,
) -> i32 {
    let mut session = match Session::open(&doc.project) {
        Ok(session) => session,
        Err(failure) => return emit(failure),
    };
    let mut params = json!({ "scale": scale });
    if let Some(frame) = frame {
        params["frame"] = json!(frame);
    }
    if let Some(layer) = layer {
        params["layer"] = json!(layer);
    }
    let response = session.call(&request("previewFrame", params));
    if let Some((code, message)) = tool_refusal(&response) {
        return emit(CliFailure::new(code, message));
    }
    let value: Value = match serde_json::from_str(&response) {
        Ok(value) => value,
        Err(_) => return emit(CliFailure::new("Cli", "tool response is not JSON")),
    };
    let index = value["result"]["frame"].as_u64().unwrap_or(0) as usize;
    let layer = value["result"]["layer"]
        .as_u64()
        .map(|layer| layer as usize);
    let ascii = match preview::render_view(session.document(), index, layer) {
        Ok(frame) => frame,
        Err(err) => return emit(CliFailure::kernel(err)),
    };
    let path = output
        .map(Path::to_path_buf)
        .unwrap_or_else(|| sibling_png(&doc.project, "preview"));
    let bytes = match export_files::write_png_b64(&path, value["result"]["png"].as_str()) {
        Ok(bytes) => bytes,
        Err(failure) => return emit(failure),
    };
    let Some(path_text) = path.to_str() else {
        return emit(CliFailure::new(
            "Cli",
            format!("path is not UTF-8: {}", path.display()),
        ));
    };
    let mut result = preview::result_json(&ascii);
    if let Some(object) = result.as_object_mut() {
        object.insert("scale".to_string(), value["result"]["scale"].clone());
        object.insert("pngWidth".to_string(), value["result"]["pngWidth"].clone());
        object.insert(
            "pngHeight".to_string(),
            value["result"]["pngHeight"].clone(),
        );
        object.insert("path".to_string(), json!(path_text));
        object.insert("bytes".to_string(), json!(bytes));
        for key in ["ink", "bounds", "indices", "pixels"] {
            object.insert(key.to_string(), value["result"][key].clone());
        }
    }
    let body = json!({
        "ok": true,
        "apiVersion": TOOL_API_VERSION,
        "tool": "previewFrame",
        "result": result,
    });
    let line = serde_json::to_string(&body).expect("preview is JSON");
    if !print_line(&line) {
        return 2;
    }
    preview::eprint_frame(&ascii);
    eprintln!("pixelforge: wrote {}", path.display());
    0
}

fn cmd_diff(
    doc: &ProjectArgs,
    frame: u32,
    with: u32,
    scale: u32,
    output: Option<&Path>,
    layer: Option<u32>,
) -> i32 {
    let mut session = match Session::open(&doc.project) {
        Ok(session) => session,
        Err(failure) => return emit(failure),
    };
    let mut params = json!({ "frame": frame, "with": with, "scale": scale });
    if let Some(layer) = layer {
        params["layer"] = json!(layer);
    }
    let response = session.call(&request("diffFrames", params));
    if let Some((code, message)) = tool_refusal(&response) {
        return emit(CliFailure::new(code, message));
    }
    let value: Value = match serde_json::from_str(&response) {
        Ok(value) => value,
        Err(_) => return emit(CliFailure::new("Cli", "tool response is not JSON")),
    };
    let result = &value["result"];
    let path = output
        .map(Path::to_path_buf)
        .unwrap_or_else(|| sibling_png(&doc.project, "diff"));
    let bytes = match export_files::write_png_b64(&path, result["png"].as_str()) {
        Ok(bytes) => bytes,
        Err(failure) => return emit(failure),
    };
    let Some(path_text) = path.to_str() else {
        return emit(CliFailure::new(
            "Cli",
            format!("path is not UTF-8: {}", path.display()),
        ));
    };
    let frame_index = usize::try_from(frame).expect("u32 fits in usize");
    let with_index = usize::try_from(with).expect("u32 fits in usize");
    let view_layer = result["layer"].as_u64().map(|layer| layer as usize);
    let left = preview::render_view(session.document(), frame_index, view_layer);
    let right = preview::render_view(session.document(), with_index, view_layer);
    let (left, right) = match (left, right) {
        (Ok(left), Ok(right)) => (left, right),
        (Err(err), _) | (_, Err(err)) => return emit(CliFailure::kernel(err)),
    };
    let summary = json!({
        "ok": true,
        "apiVersion": TOOL_API_VERSION,
        "tool": "diffFrames",
        "result": {
            "frame": result["frame"].clone(),
            "with": result["with"].clone(),
            "activeFrame": result["activeFrame"].clone(),
            "activeLayer": result["activeLayer"].clone(),
            "layer": result["layer"].clone(),
            "composite": result["composite"].clone(),
            "width": result["width"].clone(),
            "height": result["height"].clone(),
            "scale": result["scale"].clone(),
            "changed": result["changed"].clone(),
            "bounds": result["bounds"].clone(),
            "kinds": result["kinds"].clone(),
            "changes": result["changes"].clone(),
            "panels": result["panels"].clone(),
            "rows": left.rows,
            "withRows": right.rows,
            "diffRows": result["diffRows"].clone(),
            "pngWidth": result["pngWidth"].clone(),
            "pngHeight": result["pngHeight"].clone(),
            "path": path_text,
            "bytes": bytes,
        },
    });
    let line = serde_json::to_string(&summary).expect("diff is JSON");
    if !print_line(&line) {
        return 2;
    }
    eprintln!(
        "pixelforge: diff frame {frame} with {with} changed {}",
        result["changed"]
    );
    if let Some(rows) = result["diffRows"].as_array() {
        for row in rows {
            if let Some(text) = row.as_str() {
                eprintln!("{text}");
            }
        }
    }
    eprintln!(
        "pixelforge: diff legend . = same index, ! = both differ, other glyphs are the only ink"
    );
    eprintln!("pixelforge: wrote {}", path.display());
    0
}

fn sibling_png(project: &Path, kind: &str) -> PathBuf {
    let stem = project
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("pixelforge");
    let name = format!("{stem}.{kind}.png");
    match project.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.join(name),
        _ => PathBuf::from(name),
    }
}

fn cmd_palette(command: PaletteCmd) -> i32 {
    match command {
        PaletteCmd::Set {
            doc,
            index,
            r,
            g,
            b,
        } => apply(
            &doc,
            vec![request(
                "setPaletteColor",
                json!({"index": index, "r": r, "g": g, "b": b}),
            )],
        ),
        PaletteCmd::Add { doc, r, g, b } => apply(
            &doc,
            vec![request("addPaletteColor", json!({"r": r, "g": g, "b": b}))],
        ),
        PaletteCmd::Lock { doc } => apply(
            &doc,
            vec![request("setPaletteLocked", json!({"locked": true}))],
        ),
        PaletteCmd::Unlock { doc } => apply(
            &doc,
            vec![request("setPaletteLocked", json!({"locked": false}))],
        ),
        PaletteCmd::Import { doc, mode, file } => cmd_import(&doc, mode, &file),
        PaletteCmd::Enforce { doc, mode } => apply(
            &doc,
            vec![request("setIndexPolicy", json!({"mode": mode.as_str()}))],
        ),
        PaletteCmd::Remap {
            doc,
            max_colors,
            file,
        } => cmd_remap(&doc, max_colors, file.as_deref()),
        PaletteCmd::ApplyPng {
            doc,
            file,
            frame,
            layer,
        } => cmd_apply_png(&doc, &file, frame, layer),
    }
}

fn cmd_remap(doc: &ProjectArgs, max_colors: Option<u8>, file: Option<&Path>) -> i32 {
    match (max_colors, file) {
        (Some(_), Some(_)) => emit(CliFailure::new(
            "Cli",
            "palette remap takes --file or --max-colors, not both",
        )),
        (None, None) => emit(CliFailure::new(
            "Cli",
            "palette remap needs --file or --max-colors",
        )),
        (Some(max_colors), None) => apply(
            doc,
            vec![request("remapPalette", json!({"maxColors": max_colors}))],
        ),
        (None, Some(file)) => {
            let text = match fs::read_to_string(file) {
                Ok(text) => text,
                Err(err) => return emit(CliFailure::io(file, err)),
            };
            apply(doc, vec![request("remapPalette", json!({"text": text}))])
        }
    }
}

fn cmd_apply_png(doc: &ProjectArgs, file: &Path, frame: Option<u32>, layer: Option<u32>) -> i32 {
    let bytes = match fs::read(file) {
        Ok(bytes) => bytes,
        Err(err) => return emit(CliFailure::io(file, err)),
    };
    let mut params = json!({ "png": STANDARD.encode(bytes) });
    if let Some(frame) = frame {
        params["frame"] = json!(frame);
    }
    if let Some(layer) = layer {
        params["layer"] = json!(layer);
    }
    apply(doc, vec![request("applyPng", params)])
}

fn cmd_import(doc: &ProjectArgs, mode: ImportMode, file: &Path) -> i32 {
    let mut session = match Session::open(&doc.project) {
        Ok(session) => session,
        Err(failure) => return emit(failure),
    };
    let text = match fs::read_to_string(file) {
        Ok(text) => text,
        Err(err) => return emit(CliFailure::io(file, err)),
    };
    apply_session(
        &mut session,
        vec![request(
            "importPalette",
            json!({"text": text, "mode": mode.as_str()}),
        )],
        false,
    )
}

fn cmd_frame(command: FrameCmd) -> i32 {
    match command {
        FrameCmd::Add { doc } => apply(&doc, vec![request("addFrame", json!({}))]),
        FrameCmd::Delete { doc, index } => {
            apply(&doc, vec![request("deleteFrame", json!({"index": index}))])
        }
        FrameCmd::Duplicate { doc, index } => apply(
            &doc,
            vec![request("duplicateFrame", json!({"index": index}))],
        ),
        FrameCmd::Select { doc, index } => apply(
            &doc,
            vec![request("setActiveFrame", json!({"index": index}))],
        ),
        FrameCmd::Delay {
            doc,
            index,
            delay_ms,
        } => apply(
            &doc,
            vec![request(
                "setFrameDelay",
                json!({"index": index, "delayMs": delay_ms}),
            )],
        ),
    }
}

fn cmd_layer(command: LayerCmd) -> i32 {
    match command {
        LayerCmd::List { doc } => apply(&doc, vec![request("listLayers", json!({}))]),
        LayerCmd::Add { doc, name } => {
            let mut params = json!({});
            if let Some(name) = name {
                params["name"] = json!(name);
            }
            apply(&doc, vec![request("addLayer", params)])
        }
        LayerCmd::Remove { doc, index } => {
            let mut params = json!({});
            if let Some(index) = index {
                params["index"] = json!(index);
            }
            apply(&doc, vec![request("deleteLayer", params)])
        }
        LayerCmd::Rename { doc, index, name } => {
            let mut params = json!({ "name": name });
            if let Some(index) = index {
                params["index"] = json!(index);
            }
            apply(&doc, vec![request("renameLayer", params)])
        }
        LayerCmd::Select { doc, index } => apply(
            &doc,
            vec![request("setActiveLayer", json!({"index": index}))],
        ),
        LayerCmd::Reorder { doc, from, to } => apply(
            &doc,
            vec![request("reorderLayer", json!({"from": from, "to": to}))],
        ),
        LayerCmd::Flatten { doc } => apply(&doc, vec![request("flattenLayers", json!({}))]),
    }
}

fn cmd_part(command: PartCmd) -> i32 {
    match command {
        PartCmd::List { doc } => apply(&doc, vec![request("listParts", json!({}))]),
        PartCmd::Add {
            doc,
            name,
            x,
            y,
            layer,
        } => {
            let mut params = json!({ "name": name, "x": x, "y": y });
            if let Some(layer) = layer {
                params["layer"] = json!(layer);
            }
            apply(&doc, vec![request("addPart", params)])
        }
        PartCmd::Set {
            doc,
            name,
            x,
            y,
            layer,
            clear_layer,
            rename,
        } => {
            let mut params = json!({ "name": name });
            if let Some(x) = x {
                params["x"] = json!(x);
            }
            if let Some(y) = y {
                params["y"] = json!(y);
            }
            if let Some(layer) = layer {
                params["layer"] = json!(layer);
            }
            if clear_layer {
                params["clearLayer"] = json!(true);
            }
            if let Some(rename) = rename {
                params["rename"] = json!(rename);
            }
            apply(&doc, vec![request("setPart", params)])
        }
        PartCmd::Remove { doc, name } => {
            apply(&doc, vec![request("removePart", json!({ "name": name }))])
        }
        PartCmd::Anchor { command } => match command {
            AnchorCmd::Set {
                doc,
                part,
                name,
                x,
                y,
            } => apply(
                &doc,
                vec![request(
                    "setAnchor",
                    json!({ "part": part, "name": name, "x": x, "y": y }),
                )],
            ),
            AnchorCmd::Remove { doc, part, name } => apply(
                &doc,
                vec![request(
                    "removeAnchor",
                    json!({ "part": part, "name": name }),
                )],
            ),
        },
    }
}

fn cmd_tag(command: TagCmd) -> i32 {
    match command {
        TagCmd::Add {
            doc,
            name,
            from,
            to,
            loop_mode,
        } => apply(
            &doc,
            vec![request(
                "addTag",
                json!({
                    "name": name,
                    "from": from,
                    "to": to,
                    "loop": loop_mode.as_str(),
                }),
            )],
        ),
        TagCmd::Set {
            doc,
            name,
            from,
            to,
            loop_mode,
            rename,
        } => {
            let mut params = json!({ "name": name });
            if let Some(from) = from {
                params["from"] = json!(from);
            }
            if let Some(to) = to {
                params["to"] = json!(to);
            }
            if let Some(loop_mode) = loop_mode {
                params["loop"] = json!(loop_mode.as_str());
            }
            if let Some(rename) = rename {
                params["rename"] = json!(rename);
            }
            apply(&doc, vec![request("setTag", params)])
        }
        TagCmd::Remove { doc, name } => {
            apply(&doc, vec![request("removeTag", json!({ "name": name }))])
        }
    }
}

fn cmd_export(command: ExportCmd) -> i32 {
    match command {
        ExportCmd::Png {
            doc,
            out_dir,
            scale,
        } => {
            let mut session = match Session::open(&doc.project) {
                Ok(session) => session,
                Err(failure) => return emit(failure),
            };
            export_files::export_png_sequence(&mut session, &out_dir, scale)
        }
        ExportCmd::Sheet {
            doc,
            output,
            scale,
            cols,
            atlas,
        } => {
            let mut session = match Session::open(&doc.project) {
                Ok(session) => session,
                Err(failure) => return emit(failure),
            };
            export_files::export_sprite_sheet(&mut session, &output, scale, cols, atlas.as_deref())
        }
    }
}

fn cmd_call(doc: &ProjectArgs, inline: Vec<String>, file: Option<&Path>, preview: bool) -> i32 {
    let mut requests = Vec::new();
    for request in inline {
        let trimmed = request.trim().trim_start_matches('\u{feff}');
        if trimmed.is_empty() {
            return emit(CliFailure::new(
                "Cli",
                "no Tool API request. Pass --request, --request-file, or pipe JSON on stdin",
            ));
        }
        requests.push(trimmed.to_string());
    }
    if let Some(path) = file {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) => return emit(CliFailure::io(path, err)),
        };
        match parse_request_blob(&text) {
            Ok(more) => requests.extend(more),
            Err(failure) => return emit(failure),
        }
    }
    if requests.is_empty() {
        match read_stdin_requests() {
            Ok(more) => requests = more,
            Err(failure) => return emit(failure),
        }
    }
    apply_with(doc, requests, preview)
}

fn read_stdin_requests() -> Result<Vec<String>, CliFailure> {
    use std::io::IsTerminal;
    if std::io::stdin().is_terminal() {
        return Err(CliFailure::new(
            "Cli",
            "no Tool API request. Pass --request, --request-file, or pipe JSON on stdin",
        ));
    }
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .map_err(|err| CliFailure::new("Cli", format!("cannot read stdin: {err}")))?;
    parse_request_blob(&buf)
}

fn cmd_tools() -> i32 {
    let body = json!({
        "ok": true,
        "apiVersion": TOOL_API_VERSION,
        "result": { "tools": TOOL_NAMES },
    });
    let line = serde_json::to_string(&body).expect("tool list is JSON");
    if print_line(&line) {
        0
    } else {
        2
    }
}

fn apply(doc: &ProjectArgs, requests: Vec<String>) -> i32 {
    apply_with(doc, requests, false)
}

fn apply_with(doc: &ProjectArgs, requests: Vec<String>, preview: bool) -> i32 {
    let mut session = match Session::open(&doc.project) {
        Ok(session) => session,
        Err(failure) => return emit(failure),
    };
    apply_session(&mut session, requests, preview)
}

fn apply_session(session: &mut Session, requests: Vec<String>, preview: bool) -> i32 {
    let mut status = 0;
    for request in &requests {
        let response = session.call(request);
        if !print_line(&response) {
            return 2;
        }
        if let Some((code, message)) = tool_refusal(&response) {
            let name = request_tool_name(request).unwrap_or_else(|| "tool".to_string());
            eprintln!("pixelforge: {name} failed: {code}: {message}");
            status = 1;
            break;
        }
    }
    match session.save_if_changed() {
        Ok(true) => eprintln!("pixelforge: wrote {}", session.path().display()),
        Ok(false) => {}
        Err(failure) => return emit(failure),
    }
    if preview {
        let frame = preview::render(session.document());
        preview::eprint_frame(&frame);
    }
    status
}

fn with_layer(mut params: Value, layer: Option<u32>) -> Value {
    if let Some(layer) = layer {
        params["layer"] = json!(layer);
    }
    params
}

fn with_dirty(mut params: Value, dirty: bool) -> Value {
    if dirty {
        params["reportDirty"] = json!(true);
    }
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
