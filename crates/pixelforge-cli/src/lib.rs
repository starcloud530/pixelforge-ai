//! Local `pixelforge` command.
//!
//! Opens a version 1 `pixelforge-project` file, runs
//! [`pixelforge_kernel::Document::call_tool`], and prints that response on
//! stdout. Ergonomic subcommands only build the Tool API JSON. They do not
//! write a second pixel buffer.

mod asset_cmd;
mod checkpoint;
mod cli;
mod export_files;
mod failure;
mod manifest_cmd;
mod output;
mod preview;
mod requests;
mod session;
mod tiles_cmd;
mod validate_atlas;

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use clap::Parser;
use pixelforge_kernel::{
    Mask, Selection, MANIFEST_TOOL_NAMES, TILEMAP_TOOL_NAMES, TOOL_API_VERSION, TOOL_NAMES,
};
use serde_json::{json, Value};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

use crate::cli::{
    AnchorCmd, BakeArgs, BlitRectArgs, Cli, Command, ConstraintCmd, CurveCmd, DirectionCmd,
    EllipseArgs, EraseRectArgs, ExportCmd, FillRectArgs, FlipArgs, FloodArgs, FrameCmd, ImportMode,
    LayerCmd, LineArgs, MaskArgs, MaskCmd, PaintArgs, PaletteCmd, PartCmd, PolygonArgs,
    ProjectArgs, ReplaceIndexArgs, ResizeArgs, RotateArgs, ScopeArgs, SelectionCmd, StrokeRectArgs,
    TagCmd,
};
use crate::failure::CliFailure;
use crate::output::{emit, print_line, request_tool_name, tool_refusal};
use crate::requests::parse_request_blob;
use crate::session::{SaveReport, Session};

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
            compare,
            scope,
        } => cmd_preview(
            &doc,
            frame,
            scale,
            output.as_deref(),
            layer,
            compare,
            &scope,
        ),
        Command::Diff {
            doc,
            frame,
            with,
            scale,
            output,
            layer,
            scope,
        } => cmd_diff(&doc, frame, with, scale, output.as_deref(), layer, &scope),
        Command::Call {
            doc,
            request,
            request_file,
            preview,
        } => cmd_call(&doc, request, request_file.as_deref(), preview),
        Command::Paint(args) => cmd_paint(args),
        Command::FillRect(args) => cmd_fill_rect(args),
        Command::StrokeRect(args) => cmd_stroke_rect(args),
        Command::Line(args) => cmd_line(args),
        Command::Ellipse(args) => cmd_ellipse(args),
        Command::Polygon(args) => cmd_polygon(args),
        Command::Fill(args) => cmd_flood(args),
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
        Command::Mask { command } => cmd_mask(command),
        Command::ReplaceIndex(args) => cmd_replace_index(args),
        Command::Palette { command } => cmd_palette(command),
        Command::Frame { command } => cmd_frame(command),
        Command::Tag { command } => cmd_tag(command),
        Command::Direction { command } => cmd_direction(command),
        Command::Layer { command } => cmd_layer(command),
        Command::Part { command } => cmd_part(command),
        Command::Curve { command } => cmd_curve(command),
        Command::Constraint { command } => cmd_constraint(command),
        Command::Bake(args) => cmd_bake(args),
        Command::Undo { doc } => apply(&doc, vec![request("undo", json!({}))]),
        Command::Redo { doc } => apply(&doc, vec![request("redo", json!({}))]),
        Command::Export { command } => cmd_export(command),
        Command::Tileset { command } => tiles_cmd::tileset(command),
        Command::Tilemap { command } => tiles_cmd::tilemap(command),
        Command::Autotile { command } => tiles_cmd::autotile(command),
        Command::ValidateTiles(args) => tiles_cmd::validate(args),
        Command::ValidateAtlas(args) => validate_atlas::run(args),
        Command::Project { command } => manifest_cmd::run(command),
        Command::Asset { command } => asset_cmd::run(command),
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
        Ok(report) => {
            if report.project {
                eprintln!("pixelforge: wrote {}", output.display());
            }
        }
        Err(failure) => return emit(failure),
    }
    let response = session.call(&request("inspect", json!({})));
    if !print_line(&response) {
        return 2;
    }
    0
}

fn cmd_mask(command: MaskCmd) -> i32 {
    let (op, args) = match command {
        MaskCmd::Union(args) => ("union", args),
        MaskCmd::Intersect(args) => ("intersect", args),
        MaskCmd::Subtract(args) => ("subtract", args),
    };
    apply(
        &args.doc,
        vec![request("combineMask", mask_params(op, &args))],
    )
}

fn mask_params(op: &str, args: &MaskArgs) -> Value {
    let mut params = json!({ "op": op });
    put_rect(
        &mut params,
        args.x,
        args.y,
        args.width,
        args.height,
        "x",
        "y",
        "width",
        "height",
    );
    if !args.mask.is_empty() {
        params["mask"] = rect_list(&args.mask);
    }
    put_rect(
        &mut params,
        args.base_x,
        args.base_y,
        args.base_width,
        args.base_height,
        "baseX",
        "baseY",
        "baseWidth",
        "baseHeight",
    );
    if !args.base_mask.is_empty() {
        params["base"] = rect_list(&args.base_mask);
    }
    params
}

fn put_rect(
    params: &mut Value,
    x: Option<u16>,
    y: Option<u16>,
    width: Option<u16>,
    height: Option<u16>,
    x_key: &str,
    y_key: &str,
    width_key: &str,
    height_key: &str,
) {
    let Some(object) = params.as_object_mut() else {
        return;
    };
    if let Some(x) = x {
        object.insert(x_key.to_string(), json!(x));
    }
    if let Some(y) = y {
        object.insert(y_key.to_string(), json!(y));
    }
    if let Some(width) = width {
        object.insert(width_key.to_string(), json!(width));
    }
    if let Some(height) = height {
        object.insert(height_key.to_string(), json!(height));
    }
}

fn rect_list(rects: &[crate::cli::CellRect]) -> Value {
    json!(rects
        .iter()
        .map(|rect| json!({
            "x": rect.x,
            "y": rect.y,
            "width": rect.width,
            "height": rect.height,
        }))
        .collect::<Vec<_>>())
}

fn scope_requested(scope: &ScopeArgs) -> bool {
    scope.selection
        || scope.x.is_some()
        || scope.y.is_some()
        || scope.width.is_some()
        || scope.height.is_some()
        || !scope.mask.is_empty()
}

fn scope_params(params: &mut Value, scope: &ScopeArgs) {
    if scope.selection {
        params["selection"] = json!(true);
    }
    put_rect(
        params,
        scope.x,
        scope.y,
        scope.width,
        scope.height,
        "x",
        "y",
        "width",
        "height",
    );
    if !scope.mask.is_empty() {
        params["mask"] = rect_list(&scope.mask);
    }
}

fn scope_mask(result: &Value) -> Option<Mask> {
    let mask = result.get("mask")?;
    if mask.is_null() {
        return None;
    }
    if let Some(rects) = mask.get("rects").and_then(Value::as_array) {
        let parsed = rects.iter().filter_map(json_rect).collect::<Vec<_>>();
        return Mask::try_from_rects(parsed).ok();
    }
    json_rect(mask).map(Mask::from_rect)
}

fn json_rect(value: &Value) -> Option<Selection> {
    Some(Selection {
        x: value.get("x")?.as_u64()? as u16,
        y: value.get("y")?.as_u64()? as u16,
        width: value.get("width")?.as_u64()? as u16,
        height: value.get("height")?.as_u64()? as u16,
    })
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
        SelectionCmd::Wand(args) => {
            let mut params = with_layer(
                json!({
                    "x": args.x,
                    "y": args.y,
                    "tolerance": args.tolerance,
                    "connectivity": args.connectivity,
                }),
                args.layer,
            );
            if let Some(frame) = args.frame {
                params["frame"] = json!(frame);
            }
            apply(&args.doc, vec![request("selectConnected", params)])
        }
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

fn cmd_stroke_rect(args: StrokeRectArgs) -> i32 {
    apply_with(
        &args.doc,
        vec![request(
            "strokeRect",
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

fn cmd_line(args: LineArgs) -> i32 {
    let mut params = with_layer(
        json!({
            "x0": args.x0,
            "y0": args.y0,
            "x1": args.x1,
            "y1": args.y1,
            "colorIndex": args.color_index,
        }),
        args.layer,
    );
    if args.pixel_perfect {
        params["pixelPerfect"] = json!(true);
    }
    apply_with(
        &args.doc,
        vec![request("line", with_dirty(params, args.dirty))],
        args.preview,
    )
}

fn cmd_ellipse(args: EllipseArgs) -> i32 {
    let mut params = with_layer(
        json!({
            "x": args.x,
            "y": args.y,
            "width": args.width,
            "height": args.height,
            "colorIndex": args.color_index,
        }),
        args.layer,
    );
    if args.fill {
        params["fill"] = json!(true);
    }
    apply_with(
        &args.doc,
        vec![request("ellipse", with_dirty(params, args.dirty))],
        args.preview,
    )
}

fn cmd_polygon(args: PolygonArgs) -> i32 {
    let mut points = Vec::with_capacity(args.points.len());
    for text in &args.points {
        match parse_point(text) {
            Ok((x, y)) => points.push(json!({"x": x, "y": y})),
            Err(message) => {
                eprintln!("pixelforge: {message}");
                return 2;
            }
        }
    }
    let mut params = with_layer(
        json!({
            "points": points,
            "colorIndex": args.color_index,
        }),
        args.layer,
    );
    if args.fill {
        params["fill"] = json!(true);
    }
    apply_with(
        &args.doc,
        vec![request("polygon", with_dirty(params, args.dirty))],
        args.preview,
    )
}

fn parse_point(text: &str) -> Result<(u16, u16), String> {
    let Some((x, y)) = text.split_once(',') else {
        return Err(format!("point {text} must be x,y"));
    };
    let x = x
        .trim()
        .parse::<u16>()
        .map_err(|_| format!("point {text} must be x,y"))?;
    let y = y
        .trim()
        .parse::<u16>()
        .map_err(|_| format!("point {text} must be x,y"))?;
    Ok((x, y))
}

fn cmd_flood(args: FloodArgs) -> i32 {
    apply_with(
        &args.doc,
        vec![request(
            "fill",
            with_dirty(
                with_layer(
                    json!({
                        "x": args.x,
                        "y": args.y,
                        "colorIndex": args.color_index,
                        "connectivity": args.connectivity,
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
                        "mode": if args.unmodified { "pass" } else { "transparent" },
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
    compare: bool,
    scope: &ScopeArgs,
) -> i32 {
    if compare && scope_requested(scope) {
        return emit(CliFailure::new(
            "Cli",
            "preview --compare draws the full composite; omit --layer and any mask",
        ));
    }
    let mut session = match open_project(doc) {
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
    if compare {
        params["compare"] = json!(true);
    }
    scope_params(&mut params, scope);
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
    let mask = scope_mask(&value["result"]);
    let ascii = match preview::render_view_scoped(session.document(), index, layer, mask.as_ref()) {
        Ok(frame) => frame,
        Err(err) => return emit(CliFailure::kernel(err)),
    };
    let path = output
        .map(Path::to_path_buf)
        .unwrap_or_else(|| sibling_png(&doc.project, if compare { "compare" } else { "preview" }));
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
        for key in [
            "ink",
            "bounds",
            "indices",
            "pixels",
            "scope",
            "mask",
            "protected",
            "compare",
            "panels",
            "flattenPixels",
            "flattenRows",
            "indexChanges",
            "colorChanges",
        ] {
            if let Some(field) = value["result"].get(key) {
                object.insert(key.to_string(), field.clone());
            }
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
    if compare {
        eprintln!(
            "pixelforge: compare PNG left = unquantized RGBA, right = palette color of the indices layer flatten would write; project unchanged"
        );
        eprintln!(
            "pixelforge: flatten indices indexChanges {} colorChanges {}",
            value["result"]["indexChanges"], value["result"]["colorChanges"]
        );
        if let Some(rows) = value["result"]["flattenRows"].as_array() {
            for row in rows {
                if let Some(text) = row.as_str() {
                    eprintln!("{text}");
                }
            }
        }
    }
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
    scope: &ScopeArgs,
) -> i32 {
    let mut session = match open_project(doc) {
        Ok(session) => session,
        Err(failure) => return emit(failure),
    };
    let mut params = json!({ "frame": frame, "with": with, "scale": scale });
    if let Some(layer) = layer {
        params["layer"] = json!(layer);
    }
    scope_params(&mut params, scope);
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
    let mask = scope_mask(result);
    let left =
        preview::render_view_scoped(session.document(), frame_index, view_layer, mask.as_ref());
    let right =
        preview::render_view_scoped(session.document(), with_index, view_layer, mask.as_ref());
    let (left, right) = match (left, right) {
        (Ok(left), Ok(right)) => (left, right),
        (Err(err), _) | (_, Err(err)) => return emit(CliFailure::kernel(err)),
    };
    let mut summary = json!({
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
    if let Some(object) = summary["result"].as_object_mut() {
        for key in ["scope", "mask", "protected"] {
            if let Some(field) = result.get(key) {
                object.insert(key.to_string(), field.clone());
            }
        }
    }
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
    if result.get("scope").is_some() {
        eprintln!(
            "pixelforge: diff legend . = same index, ! = both differ, other glyphs are the only ink, # = protect"
        );
    } else {
        eprintln!(
            "pixelforge: diff legend . = same index, ! = both differ, other glyphs are the only ink"
        );
    }
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
    let mut session = match open_project(doc) {
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
        LayerCmd::Add { doc, name, role } => {
            let mut params = json!({});
            if let Some(name) = name {
                params["name"] = json!(name);
            }
            if let Some(role) = role {
                params["role"] = json!(role.as_str());
            }
            apply(&doc, vec![request("addLayer", params)])
        }
        LayerCmd::Bind {
            doc,
            index,
            part,
            anchor,
        } => {
            let mut params = json!({ "part": part });
            if let Some(index) = index {
                params["index"] = json!(index);
            }
            if let Some(anchor) = anchor {
                params["anchor"] = json!(anchor);
            }
            apply(&doc, vec![request("setLayerBind", params)])
        }
        LayerCmd::Unbind { doc, index } => {
            let mut params = json!({ "part": Value::Null });
            if let Some(index) = index {
                params["index"] = json!(index);
            }
            apply(&doc, vec![request("setLayerBind", params)])
        }
        LayerCmd::Role { doc, index, role } => {
            let mut params = json!({ "role": role.as_str() });
            if let Some(index) = index {
                params["index"] = json!(index);
            }
            apply(&doc, vec![request("setLayerRole", params)])
        }
        LayerCmd::Show { doc, index } => {
            let mut params = json!({ "visible": true });
            if let Some(index) = index {
                params["index"] = json!(index);
            }
            apply(&doc, vec![request("setLayerVisible", params)])
        }
        LayerCmd::Hide { doc, index } => {
            let mut params = json!({ "visible": false });
            if let Some(index) = index {
                params["index"] = json!(index);
            }
            apply(&doc, vec![request("setLayerVisible", params)])
        }
        LayerCmd::Opacity {
            doc,
            index,
            opacity,
        } => {
            let mut params = json!({ "opacity": opacity });
            if let Some(index) = index {
                params["index"] = json!(index);
            }
            apply(&doc, vec![request("setLayerOpacity", params)])
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
        PartCmd::Translate {
            doc,
            name,
            dx,
            dy,
            frame,
            dirty,
        } => {
            let mut params = json!({ "name": name, "dx": dx, "dy": dy });
            if let Some(frame) = frame {
                params["frame"] = json!(frame);
            }
            apply(
                &doc,
                vec![request("translatePart", with_dirty(params, dirty))],
            )
        }
        PartCmd::Rotate {
            doc,
            name,
            degrees,
            anchor,
            frame,
            dirty,
        } => {
            let mut params = json!({ "name": name, "degrees": degrees });
            if let Some(anchor) = anchor {
                params["anchor"] = json!(anchor);
            }
            if let Some(frame) = frame {
                params["frame"] = json!(frame);
            }
            apply(&doc, vec![request("rotatePart", with_dirty(params, dirty))])
        }
        PartCmd::Flip {
            doc,
            name,
            horizontal,
            vertical,
            anchor,
            frame,
            dirty,
        } => {
            let direction = match (horizontal, vertical) {
                (true, false) => "horizontal",
                (false, true) => "vertical",
                _ => {
                    return emit(CliFailure::new(
                        "Cli",
                        "part flip needs exactly one of --horizontal or --vertical",
                    ))
                }
            };
            let mut params = json!({ "name": name, "direction": direction });
            if let Some(anchor) = anchor {
                params["anchor"] = json!(anchor);
            }
            if let Some(frame) = frame {
                params["frame"] = json!(frame);
            }
            apply(&doc, vec![request("flipPart", with_dirty(params, dirty))])
        }
    }
}

fn cmd_curve(command: CurveCmd) -> i32 {
    match command {
        CurveCmd::Set {
            doc,
            part,
            channel,
            frame,
            value,
            ease,
        } => {
            let mut params = json!({
                "part": part,
                "channel": curve_channel(&channel),
                "frame": frame,
                "value": value,
            });
            if let Some(ease) = ease {
                params["ease"] = json!(curve_ease(&ease));
            }
            apply(&doc, vec![request("setCurveKey", params)])
        }
        CurveCmd::List { doc, part } => {
            let mut params = json!({});
            if let Some(part) = part {
                params["part"] = json!(part);
            }
            apply(&doc, vec![request("listCurves", params)])
        }
        CurveCmd::Sample {
            doc,
            part,
            channel,
            frame,
        } => match channel {
            Some(channel) => apply(
                &doc,
                vec![request(
                    "sampleCurve",
                    json!({
                        "part": part,
                        "channel": curve_channel(&channel),
                        "frame": frame,
                    }),
                )],
            ),
            None => apply(
                &doc,
                vec![request(
                    "samplePart",
                    json!({ "part": part, "frame": frame }),
                )],
            ),
        },
        CurveCmd::Remove {
            doc,
            part,
            channel,
            frame,
        } => apply(
            &doc,
            vec![request(
                "removeCurveKey",
                json!({
                    "part": part,
                    "channel": curve_channel(&channel),
                    "frame": frame,
                }),
            )],
        ),
        CurveCmd::Clear { doc, part, channel } => apply(
            &doc,
            vec![request(
                "clearCurve",
                json!({
                    "part": part,
                    "channel": curve_channel(&channel),
                }),
            )],
        ),
    }
}

fn cmd_bake(args: BakeArgs) -> i32 {
    let mut params = serde_json::Map::new();
    if let Some(from) = args.from {
        params.insert("from".into(), json!(from));
    }
    if let Some(to) = args.to {
        params.insert("to".into(), json!(to));
    }
    if let Some(source) = args.source {
        params.insert("source".into(), json!(source));
    }
    if let Some(fps) = args.fps {
        params.insert("fps".into(), json!(fps));
    }
    if !args.parts.is_empty() {
        params.insert("parts".into(), json!(args.parts));
    }
    if !args.layers.is_empty() {
        params.insert("layers".into(), json!(args.layers));
    }
    if args.curves_only {
        params.insert("constraints".into(), json!(false));
    }
    apply(&args.doc, vec![request("bake", Value::Object(params))])
}

fn parse_contacts(values: &[String]) -> Result<Value, String> {
    let mut contacts = Vec::with_capacity(values.len());
    for value in values {
        let parts: Vec<&str> = value.split(':').collect();
        let numbers: Result<Vec<u16>, _> = parts.iter().map(|part| part.parse::<u16>()).collect();
        let numbers = numbers
            .map_err(|_| format!("contact must be frame:x:y or frame:to:x:y, got {value}"))?;
        let contact = match numbers.as_slice() {
            [frame, x, y] => json!({"frame": frame, "x": x, "y": y}),
            [frame, to, x, y] => json!({"frame": frame, "to": to, "x": x, "y": y}),
            _ => {
                return Err(format!(
                    "contact must be frame:x:y or frame:to:x:y, got {value}"
                ))
            }
        };
        contacts.push(contact);
    }
    Ok(Value::Array(contacts))
}

fn cmd_constraint(command: ConstraintCmd) -> i32 {
    match command {
        ConstraintCmd::Add {
            doc,
            name,
            kind,
            part,
            anchor,
            target,
            target_anchor,
            x,
            y,
            chain,
            bend,
            contact,
        } => {
            let mut params = json!({
                "name": name,
                "kind": kind,
                "part": part,
            });
            if let Some(anchor) = anchor {
                params["anchor"] = json!(anchor);
            }
            if let Some(target) = target {
                params["target"] = json!(target);
            }
            if let Some(target_anchor) = target_anchor {
                params["targetAnchor"] = json!(target_anchor);
            }
            if let Some(x) = x {
                params["x"] = json!(x);
            }
            if let Some(y) = y {
                params["y"] = json!(y);
            }
            if !chain.is_empty() {
                params["chain"] = json!(chain);
            }
            if let Some(bend) = bend {
                params["bend"] = json!(bend);
            }
            if !contact.is_empty() {
                match parse_contacts(&contact) {
                    Ok(contacts) => params["contacts"] = contacts,
                    Err(message) => {
                        eprintln!("error: {message}");
                        return 2;
                    }
                }
            }
            apply(&doc, vec![request("addConstraint", params)])
        }
        ConstraintCmd::List { doc } => apply(&doc, vec![request("listConstraints", json!({}))]),
        ConstraintCmd::Remove { doc, name } => apply(
            &doc,
            vec![request("removeConstraint", json!({ "name": name }))],
        ),
        ConstraintCmd::Solve {
            doc,
            frame,
            apply: write,
            pixel_frame,
        } => {
            let mut params = json!({});
            if let Some(frame) = frame {
                params["frame"] = json!(frame);
            }
            if write {
                params["apply"] = json!(true);
            }
            if let Some(pixel_frame) = pixel_frame {
                params["pixelFrame"] = json!(pixel_frame);
            }
            apply(&doc, vec![request("solveConstraints", params)])
        }
    }
}

fn curve_channel(raw: &str) -> String {
    match raw {
        "translate-x" | "translate_x" => "translateX".to_string(),
        "translate-y" | "translate_y" => "translateY".to_string(),
        "flip-horizontal" | "flip_horizontal" => "flipHorizontal".to_string(),
        "flip-vertical" | "flip_vertical" => "flipVertical".to_string(),
        other => other.to_string(),
    }
}

fn curve_ease(raw: &str) -> String {
    match raw {
        "ease-in-out" | "ease_in_out" => "easeInOut".to_string(),
        other => other.to_string(),
    }
}

fn cmd_direction(command: DirectionCmd) -> i32 {
    match command {
        DirectionCmd::Set {
            doc,
            count,
            columns,
            apply: write_tags,
        } => apply(
            &doc,
            vec![request(
                "setDirectionLayout",
                json!({
                    "count": count,
                    "columns": columns,
                    "apply": write_tags,
                }),
            )],
        ),
        DirectionCmd::Clear { doc } => {
            apply(&doc, vec![request("clearDirectionLayout", json!({}))])
        }
        DirectionCmd::Apply { doc } => apply(&doc, vec![request("applyDirectionTags", json!({}))]),
        DirectionCmd::Fill {
            doc,
            from,
            mode,
            overwrite,
        } => apply(
            &doc,
            vec![request(
                "fillDirections",
                json!({
                    "from": from,
                    "mode": mode.as_str(),
                    "overwrite": overwrite,
                }),
            )],
        ),
        DirectionCmd::List { doc } => apply(&doc, vec![request("listDirections", json!({}))]),
        DirectionCmd::Validate { doc } => {
            let mut session = match open_project(&doc) {
                Ok(session) => session,
                Err(failure) => return emit(failure),
            };
            let response = session.call(&request("validateDirections", json!({})));
            if !print_line(&response) {
                return 2;
            }
            if let Some((code, message)) = tool_refusal(&response) {
                eprintln!("pixelforge: validateDirections failed: {code}: {message}");
                return 1;
            }
            let value: Value = match serde_json::from_str(&response) {
                Ok(value) => value,
                Err(_) => return emit(CliFailure::new("Cli", "tool response is not JSON")),
            };
            let error_count = value["result"]["errorCount"].as_u64().unwrap_or(0);
            if error_count == 0 {
                eprintln!("pixelforge: direction validate: ok");
                0
            } else {
                if let Some(errors) = value["result"]["errors"].as_array() {
                    for error in errors {
                        let code = error["code"].as_str().unwrap_or("direction");
                        let message = error["message"].as_str().unwrap_or("");
                        eprintln!("pixelforge: error {code}: {message}");
                    }
                }
                eprintln!("pixelforge: direction validate: {error_count} errors");
                1
            }
        }
        DirectionCmd::Export {
            doc,
            facing,
            output,
            scale,
        } => {
            let mut session = match open_project(&doc) {
                Ok(session) => session,
                Err(failure) => return emit(failure),
            };
            export_files::export_direction(&mut session, &facing, &output, scale)
        }
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
            let mut session = match open_project(&doc) {
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
            let mut session = match open_project(&doc) {
                Ok(session) => session,
                Err(failure) => return emit(failure),
            };
            export_files::export_sprite_sheet(&mut session, &output, scale, cols, atlas.as_deref())
        }
        ExportCmd::Apng {
            doc,
            output,
            scale,
            tag,
            from,
            to,
            loop_mode,
        } => {
            let mut session = match open_project(&doc) {
                Ok(session) => session,
                Err(failure) => return emit(failure),
            };
            export_files::export_apng(
                &mut session,
                &output,
                scale,
                tag.as_deref(),
                from,
                to,
                loop_mode.map(|mode| mode.as_str()),
            )
        }
        ExportCmd::Tileset {
            doc,
            output,
            meta,
            scale,
            cols,
            image,
        } => tiles_cmd::export_tileset(&doc.project, &output, &meta, scale, cols, image),
        ExportCmd::Tilemap {
            map,
            tileset,
            output,
            tmj,
            scale,
            cols,
            image,
        } => tiles_cmd::export_tilemap(&map, &tileset, &output, &tmj, scale, cols, image),
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
        "result": {
            "tools": TOOL_NAMES,
            "tilemapTools": TILEMAP_TOOL_NAMES,
            "manifestTools": MANIFEST_TOOL_NAMES,
        },
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

fn open_project(doc: &ProjectArgs) -> Result<Session, CliFailure> {
    Session::open_with(&doc.project, doc.checkpoint.as_deref())
}

fn apply_with(doc: &ProjectArgs, requests: Vec<String>, preview: bool) -> i32 {
    let sidecar = match manifest_cmd::peek_sidecar(&doc.project) {
        Ok(sidecar) => sidecar,
        Err(failure) => return emit(failure),
    };
    match sidecar {
        manifest_cmd::Sidecar::Manifest => {
            if doc.checkpoint.is_some() {
                return emit(CliFailure::new(
                    "Cli",
                    "a project manifest does not use --checkpoint",
                ));
            }
            return manifest_cmd::apply_call(&doc.project, requests, preview);
        }
        manifest_cmd::Sidecar::Tilemap => {
            if doc.checkpoint.is_some() {
                return emit(CliFailure::new(
                    "Cli",
                    "a tilemap does not use --checkpoint",
                ));
            }
            return tiles_cmd::apply_call(&doc.project, requests, preview);
        }
        manifest_cmd::Sidecar::Other => {}
    }
    let mut session = match open_project(doc) {
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
        Ok(report) => report_saved(session, report),
        Err(failure) => return emit(failure),
    }
    if preview {
        let frame = preview::render(session.document());
        preview::eprint_frame(&frame);
    }
    status
}

fn report_saved(session: &Session, report: SaveReport) {
    if report.project {
        eprintln!("pixelforge: wrote {}", session.path().display());
    }
    if report.checkpoint {
        if let Some(path) = session.checkpoint_path() {
            eprintln!("pixelforge: wrote {}", path.display());
        }
    }
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
