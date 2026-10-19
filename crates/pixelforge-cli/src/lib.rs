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
mod requests;
mod session;

use std::fs;
use std::io::Read;
use std::path::Path;

use clap::Parser;
use pixelforge_kernel::{TOOL_API_VERSION, TOOL_NAMES};
use serde_json::{json, Value};

use crate::cli::{
    Cli, Command, ExportCmd, FrameCmd, ImportMode, PaintArgs, PaletteCmd, ProjectArgs,
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
        Command::Inspect { doc } => apply(&doc, vec![request("inspect", json!({}))]),
        Command::Call {
            doc,
            request,
            request_file,
        } => cmd_call(&doc, request, request_file.as_deref()),
        Command::Paint(args) => cmd_paint(args),
        Command::Erase(args) => apply(
            &args.doc,
            vec![request("erase", json!({"x": args.x, "y": args.y}))],
        ),
        Command::Palette { command } => cmd_palette(command),
        Command::Frame { command } => cmd_frame(command),
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

fn cmd_paint(args: PaintArgs) -> i32 {
    apply(
        &args.doc,
        vec![request(
            "paint",
            json!({
                "x": args.x,
                "y": args.y,
                "colorIndex": args.color_index,
            }),
        )],
    )
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
    }
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

fn cmd_export(command: ExportCmd) -> i32 {
    match command {
        ExportCmd::Png { doc, out_dir } => {
            let mut session = match Session::open(&doc.project) {
                Ok(session) => session,
                Err(failure) => return emit(failure),
            };
            export_files::export_png_sequence(&mut session, &out_dir)
        }
        ExportCmd::Sheet { doc, output } => {
            let mut session = match Session::open(&doc.project) {
                Ok(session) => session,
                Err(failure) => return emit(failure),
            };
            export_files::export_sprite_sheet(&mut session, &output)
        }
    }
}

fn cmd_call(doc: &ProjectArgs, inline: Vec<String>, file: Option<&Path>) -> i32 {
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
    apply(doc, requests)
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
    let mut session = match Session::open(&doc.project) {
        Ok(session) => session,
        Err(failure) => return emit(failure),
    };
    apply_session(&mut session, requests)
}

fn apply_session(session: &mut Session, requests: Vec<String>) -> i32 {
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
    status
}

fn request(tool: &str, params: Value) -> String {
    json!({
        "apiVersion": TOOL_API_VERSION,
        "tool": tool,
        "params": params,
    })
    .to_string()
}
