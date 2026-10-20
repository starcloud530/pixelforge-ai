//! Argument parser. Help goes to stdout; usage errors go to stderr.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

pub const AFTER_HELP: &str = "\
Stdout is compact JSON for an agent, one Tool API response per line.
Notes for a person go to stderr.

One process is one document. Repeat --request on `call` so a stroke, a walk
cycle, and undo share the kernel undo stack. The project file does not store
that stack: the next process starts empty, same as opening the file in the
editor.

Examples:
  pixelforge new --width 16 --height 16 --output hero.json
  pixelforge paint --project hero.json --x 1 --y 2 --color-index 3
  pixelforge fill-rect --project hero.json --x 4 --y 8 --width 16 --height 40 --color-index 3
  pixelforge preview --project hero.json
  pixelforge paint --project hero.json --x 1 --y 2 --color-index 3 --preview
  pixelforge call --project hero.json \\
      --request '{\"tool\":\"beginGesture\"}' \\
      --request '{\"tool\":\"fillRect\",\"params\":{\"x\":1,\"y\":2,\"width\":4,\"height\":6,\"colorIndex\":3}}' \\
      --request '{\"tool\":\"endGesture\"}'
  pixelforge export sheet --project hero.json --output hero-sheet.png

`preview` and `--preview` print an ASCII view of the active frame on stderr
(one glyph per cell: `.` is transparent, `1`-`9` and `A`-`Z` / `a`-`z` are
indices, `*` is index 62 or above). Stdout stays JSON. `preview` also puts
those rows in `result.rows`. A standing figure needs a tall canvas (64x128
or 96x128); a 48x64 full body stays schematic.";

#[derive(Parser)]
#[command(
    name = "pixelforge",
    version,
    about = "Edit a pixelforge-project file through the kernel Tool API",
    after_help = AFTER_HELP,
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create a blank pixelforge-project and print inspect JSON.
    ///
    /// Refuses when the output file already exists.
    New {
        /// Canvas width, from 1 to 256.
        #[arg(long, default_value_t = 16)]
        width: u16,
        /// Canvas height, from 1 to 256.
        #[arg(long, default_value_t = 16)]
        height: u16,
        /// Where to write the project file.
        #[arg(short, long, value_name = "FILE")]
        output: PathBuf,
    },
    /// Print the Tool API inspect result. Does not write the file.
    Inspect {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Also print an ASCII view of the active frame on stderr.
        #[arg(long)]
        preview: bool,
    },
    /// Print an ASCII view of the active frame.
    ///
    /// Stdout is one JSON line (`result.rows` and `result.legend`). The same
    /// rows are written to stderr. Does not change the project file.
    Preview {
        #[command(flatten)]
        doc: ProjectArgs,
    },
    /// Run one or more Tool API requests against one document.
    ///
    /// Alias: `tool`. Each response is one JSON line. A refused call stops
    /// the rest. Edits before that call are saved.
    #[command(alias = "tool")]
    Call {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Tool API request JSON. Repeat to share one undo stack.
        #[arg(long, value_name = "JSON")]
        request: Vec<String>,
        /// One JSON value, a JSON array, or one JSON value per line.
        #[arg(long, value_name = "FILE")]
        request_file: Option<PathBuf>,
        /// After the batch, print an ASCII view of the active frame on stderr.
        #[arg(long)]
        preview: bool,
    },
    /// Paint one cell with a palette index. Does not write RGB.
    Paint(PaintArgs),
    /// Fill an axis-aligned rectangle on the active frame with one palette index.
    ///
    /// One undo step. The rectangle must lie entirely inside the canvas.
    /// Does not write RGB. A locked palette still allows this.
    FillRect(FillRectArgs),
    /// Set one cell back to transparent index 0.
    Erase(EraseArgs),
    /// Palette edits. Paint of an existing index still works while locked.
    Palette {
        #[command(subcommand)]
        command: PaletteCmd,
    },
    /// Frame list, active frame, and per-frame delay.
    Frame {
        #[command(subcommand)]
        command: FrameCmd,
    },
    /// Undo the last edit recorded in this process.
    ///
    /// A freshly opened file has an empty stack, and the file does not store
    /// one. A separate invocation from `paint` returns NothingToUndo. Put
    /// `{"tool":"undo"}` in the same `call` as the edit to undo it.
    Undo {
        #[command(flatten)]
        doc: ProjectArgs,
    },
    /// Redo the last undone edit recorded in this process.
    Redo {
        #[command(flatten)]
        doc: ProjectArgs,
    },
    /// Write PNG files from the kernel export tools.
    Export {
        #[command(subcommand)]
        command: ExportCmd,
    },
    /// Print the Tool API tool names as JSON.
    Tools,
}

#[derive(Args)]
pub struct ProjectArgs {
    /// pixelforge-project file. Saved when a tool changes the document.
    #[arg(short, long, value_name = "FILE")]
    pub project: PathBuf,
}

#[derive(Args)]
pub struct PaintArgs {
    #[command(flatten)]
    pub doc: ProjectArgs,
    /// Column, from 0.
    #[arg(long)]
    pub x: u16,
    /// Row, from 0.
    #[arg(long)]
    pub y: u16,
    /// Palette index to store. Index 0 is refused; use erase.
    #[arg(long)]
    pub color_index: u8,
    /// After the edit, print an ASCII view of the active frame on stderr.
    #[arg(long)]
    pub preview: bool,
}

#[derive(Args)]
pub struct FillRectArgs {
    #[command(flatten)]
    pub doc: ProjectArgs,
    /// Left column, from 0.
    #[arg(long)]
    pub x: u16,
    /// Top row, from 0.
    #[arg(long)]
    pub y: u16,
    /// Width in cells, from 1.
    #[arg(long)]
    pub width: u16,
    /// Height in cells, from 1.
    #[arg(long)]
    pub height: u16,
    /// Palette index to store. Index 0 is refused.
    #[arg(long)]
    pub color_index: u8,
    /// After the edit, print an ASCII view of the active frame on stderr.
    #[arg(long)]
    pub preview: bool,
}

#[derive(Args)]
pub struct EraseArgs {
    #[command(flatten)]
    pub doc: ProjectArgs,
    /// Column, from 0.
    #[arg(long)]
    pub x: u16,
    /// Row, from 0.
    #[arg(long)]
    pub y: u16,
}

#[derive(Subcommand)]
pub enum PaletteCmd {
    /// Replace the RGB of a palette index. Cells keep the index.
    Set {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Palette index. Index 0 is refused.
        #[arg(long)]
        index: u8,
        /// Red channel, 0..=255.
        #[arg(long)]
        r: u8,
        /// Green channel, 0..=255.
        #[arg(long)]
        g: u8,
        /// Blue channel, 0..=255.
        #[arg(long)]
        b: u8,
    },
    /// Append an opaque color. The result `index` is what paint uses.
    Add {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Red channel, 0..=255.
        #[arg(long)]
        r: u8,
        /// Green channel, 0..=255.
        #[arg(long)]
        g: u8,
        /// Blue channel, 0..=255.
        #[arg(long)]
        b: u8,
    },
    /// Refuse recolor, append, and import. Paint of an existing index still works.
    Lock {
        #[command(flatten)]
        doc: ProjectArgs,
    },
    /// Allow palette edits again.
    Unlock {
        #[command(flatten)]
        doc: ProjectArgs,
    },
    /// Import a GIMP palette (`.gpl`) or a hex list. Mode is `replace` or `merge`.
    Import {
        #[command(flatten)]
        doc: ProjectArgs,
        #[arg(long, value_enum)]
        mode: ImportMode,
        /// Palette text file.
        #[arg(long, value_name = "FILE")]
        file: PathBuf,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub enum ImportMode {
    Replace,
    Merge,
}

impl ImportMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Replace => "replace",
            Self::Merge => "merge",
        }
    }
}

#[derive(Subcommand)]
pub enum FrameCmd {
    /// Append a blank frame. Delay is 100 ms. The active frame stays put.
    Add {
        #[command(flatten)]
        doc: ProjectArgs,
    },
    /// Delete a frame. The last remaining frame is refused.
    Delete {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Frame index, from 0.
        #[arg(long)]
        index: u32,
    },
    /// Insert a copy of this frame (pixels and delay) after it.
    Duplicate {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Frame index to copy, from 0.
        #[arg(long)]
        index: u32,
    },
    /// Select the active frame.
    Select {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Frame index, from 0.
        #[arg(long)]
        index: u32,
    },
    /// Set one frame's delay in milliseconds (1..=65535).
    Delay {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Frame index, from 0.
        #[arg(long)]
        index: u32,
        /// Delay in milliseconds, from 1 to 65535.
        #[arg(long)]
        delay_ms: u32,
    },
}

#[derive(Subcommand)]
pub enum ExportCmd {
    /// Write one RGBA PNG per frame as pixelforge-000.png onward.
    Png {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Directory to create and fill. Does not change the project file.
        #[arg(long, value_name = "DIR")]
        out_dir: PathBuf,
    },
    /// Write one grid sprite sheet. Layout matches the kernel.
    Sheet {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Destination PNG path.
        #[arg(short, long, value_name = "FILE")]
        output: PathBuf,
    },
}
