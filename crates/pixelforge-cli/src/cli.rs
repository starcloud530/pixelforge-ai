//! Argument parser. Help goes to stdout; usage errors go to stderr.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

pub const AFTER_HELP: &str = "\
Stdout is compact JSON for an agent, one Tool API response per line.
Notes for a person go to stderr.

One process is one document. Repeat --request on `call` so a stroke, a walk
cycle, and undo share the kernel undo stack. The project file does not store
that stack. Without --checkpoint the next process starts empty, same as
opening the file in the editor, and `undo` returns NothingToUndo.

--checkpoint FILE keeps a pixelforge-checkpoint sidecar (version 1). Each
process that changes the project appends one undo step (100 max). A later
process restores that step with `undo` or `redo` and the same file. Inside
the process, `undo` still walks kernel operations first; one checkpoint
step is the net edit of that process. A project edited without that file
fails with CheckpointMismatch and writes nothing.

Examples:
  pixelforge new --width 16 --height 16 --output hero.json
  pixelforge paint --project hero.json --x 1 --y 2 --color-index 3
  pixelforge fill-rect --project hero.json --x 4 --y 8 --width 16 --height 40 --color-index 3
  pixelforge erase-rect --project hero.json --x 4 --y 8 --width 16 --height 8
  pixelforge copy-rect --project hero.json --src-x 4 --src-y 8 --width 8 --height 12 --dst-x 20 --dst-y 8
  pixelforge copy-rect --project hero.json --from-frame 0 --to-frame 1 --src-x 8 --src-y 16 --width 16 --height 24 --dst-x 8 --dst-y 16
  pixelforge move-rect --project hero.json --src-x 20 --src-y 8 --width 8 --height 12 --dst-x 24 --dst-y 10
  pixelforge flip --horizontal --project hero.json
  pixelforge hflip --project hero.json --x 0 --y 0 --width 16 --height 32
  pixelforge vflip --project hero.json
  pixelforge rotate --project hero.json --degrees 90
  pixelforge import-aseprite --file hero.aseprite --output hero.json
  pixelforge scene --project hero.json
  pixelforge preview --project hero.json --scale 4
  pixelforge preview --project hero.json --frame 1 --scale 4 --output hero-f1.png
  pixelforge diff --project hero.json --frame 0 --with 1 --scale 4
  pixelforge diff --project hero.json --frame 0 --with 1 --selection
  pixelforge mask subtract --project hero.json --x 8 --y 36 --width 16 --height 6
  pixelforge paint --project hero.json --x 1 --y 2 --color-index 3 --dirty
  pixelforge resize --project hero.json --width 64 --height 64
  pixelforge layer add --project hero.json --name gear
  pixelforge layer add --project soldier.json --name polish --role polish
  pixelforge fill-rect --project hero.json --layer 1 --x 4 --y 8 --width 8 --height 8 --color-index 5
  pixelforge preview --project hero.json --scale 4
  pixelforge layer flatten --project hero.json
  pixelforge paint --project hero.json --x 1 --y 2 --color-index 3 --preview
  pixelforge call --project hero.json \\
      --request '{\"tool\":\"beginGesture\"}' \\
      --request '{\"tool\":\"fillRect\",\"params\":{\"x\":1,\"y\":2,\"width\":4,\"height\":6,\"colorIndex\":3}}' \\
      --request '{\"tool\":\"endGesture\"}'
  pixelforge paint --project hero.json --checkpoint hero.ckpt.json --x 1 --y 2 --color-index 3
  pixelforge undo --project hero.json --checkpoint hero.ckpt.json
  pixelforge export png --project hero.json --out-dir ./frames --scale 2
  pixelforge tag add --project hero.json --name walk_down --from 1 --to 3 --loop forward
  pixelforge part add --project soldier.json --name gun --layer 1 --x 24 --y 12
  pixelforge part anchor set --project soldier.json --part gun --name grip --x 2 --y 2
  pixelforge part translate --project soldier.json --name gun --dx 2 --dy 0
  pixelforge part rotate --project soldier.json --name gun --degrees 90 --anchor grip
  pixelforge part flip --horizontal --project soldier.json --name gun --anchor grip
  pixelforge export sheet --project hero.json --output hero-sheet.png --scale 2 --cols 4 --atlas hero-sheet.json
  pixelforge validate-atlas --atlas hero-sheet.json --sheet hero-sheet.png

`scene` prints a compact summary (canvas, active frame and layer, layer
names, palette hash, frame delays) and does not dump pixels. `visible` is
always true and `opacity` is null: the kernel has no hide flag and no
per-layer opacity. `--dirty` on paint, erase, fill-rect, erase-rect,
copy-rect, move-rect, flip, hflip, vflip, rotate, resize, and
part translate / rotate / flip adds `changed`, `dirty`, and
`regions`. Omit it and those fields stay absent. `preview` JSON also has
`ink`, `bounds`, `indices`, and `pixels`. `diff` JSON also has `bounds`,
`kinds`, and `changes`.

`preview` prints an ASCII view on stderr (one glyph per cell: `.` is
transparent, `1`-`9` and `A`-`Z` / `a`-`z` are indices, `*` is index 62 or
above) and writes a nearest-neighbor color PNG. The picture is the composite:
index 0 shows the layer under it, and any other index covers that layer.
`--layer N` draws that layer alone. `--frame N` draws frame N and does not
change activeFrame or the project file. `--scale K` matches export (integer
>= 1, default 1). Stdout JSON keeps `result.rows` and adds `result.path`.
`diff` is read-only: ASCII index diff plus a PNG with frame, with, and an
onion blend, all from the composite unless `--layer` is set. `--selection`
limits preview and diff to the stored mask. `--x --y --width --height` or
repeat `--mask x,y,width,height` limits them to that rectangle instead.
Cells outside are `#` and are not part of `changed`. `mask union`,
`mask intersect`, and `mask subtract` combine that rectangle with the stored
selection, or with `--base-x --base-y --base-width --base-height`. The result
is the selection paint tools already use. There is no lasso. `flip
--horizontal` / `hflip` mirrors indices left to right. `flip --vertical` /
`vflip` mirrors them top to bottom. `rotate --degrees 90|180|270` turns
indices clockwise. 90 and 270 need a square rectangle; the canvas size
stays. All three write the active layer, or `--layer`, and do not recolor.
A stored selection writes a cell only when that cell and the cell it reads
are both inside. `part translate`, `part rotate`, and `part flip` move that
part's origin, its anchors, and the ink on its linked layer in one undo
step. Rotation and flip pivot on `--anchor` or the origin cell. `--degrees`
within 0.01° of a multiple of 90 is an exact quarter turn; any other angle
quantizes to 0.01° and then to the nearest cell, then seals covered notches
up to the collision count. Read `dropped`, `conflicts`, `sealed`, and
`breaks`. `dropped` stays when a notch is filled. Rotation does not replace
hand cleanup. Rectangle flip and rotate
do not move parts. A selection must cover every ink cell and its destination
or the part transform writes nothing. `curve` stores keyframes of those
parameters on frame indices and samples them. It does not write pixels.
`constraint` stores a follow, attach, or pin. `constraint solve` is the
step that tracks them: pass `--frame` to sample curves first, and `--apply`
to write one pixel frame. Omit `--apply` and the file stays as it was.
Solving does not walk the timeline. `bake` does: it samples that pose onto
each frame in a range and leaves the curves in place. It rebuilds every
layer a curve or constraint manages, including a pose that returned to zero.
A retouch on that layer is replaced. `layer add --role polish` (or
`layer role --role polish`) is an overlay bake never writes, so a finger
fix stays when the raise changes. Polish cells stay on the canvas cells
where they were painted.
`import-aseprite --file sprite.aseprite --output hero.json`
reads an indexed `.aseprite` into a new project. `--project` replaces an
existing one. Indexed 8-bit only. It does not write `.aseprite`.
`copy-rect` and `move-rect` take optional
`--from-frame` and `--to-frame` and stay on one layer. `export` writes the
composite. `layer flatten` collapses that composite into one layer.
`export sheet --cols N` keeps one row when N is the frame count. `--atlas FILE`
writes pixelforge-atlas JSON (frame rects, delayMs, columns, tags) and adds
`atlas` to the result. Omit it and the result stays the PNG fields. `tag add`
stores a name, an inclusive frame range, and a loop (`forward`, `reverse`,
`pingpong`, or `once`). Tags are not linked cels. `resize`
copies every layer of every frame into a new canvas; omit offsets to center.
A standing figure still wants a tall canvas (64x128 or 96x128). `selection set`
stores one rectangle in the project file. Later paint, erase, fill-rect,
erase-rect, copy-rect, flip, hflip, vflip, rotate, and replace-index leave cells outside
that rectangle unchanged. `move-rect` fails with SelectionProtected unless
both rectangles lie inside it. `selection clear` removes the field. There
is no lasso. The rectangle is not per layer: pass `--layer` to edit a helmet
without touching the body. `frame
duplicate` does not select the new frame. `layer add` inserts above the
active layer and selects the new one. `palette enforce` sets indexPolicy
(`reject` by default, or `clamp`). `palette remap` quantizes with
`--max-colors` or retargets a `.gpl` / hex file. `palette apply-png` crushes
one canvas-sized PNG onto the current palette. There is no Lospec browser.
`validate-atlas` reads an exported sheet PNG and its pixelforge-atlas JSON.
It does not edit pixels. Exit 0 means the checklist has no errors. Exit 1
means it has at least one error. Warnings alone stay exit 0, in --strict and
in --loose. --loose turns canvas tier, four-direction names, an empty tag
list, image file name, delay range, and pivot drift into warnings. Overlap,
out-of-bounds frames, a broken grid, and opaque padding stay errors. Default
canvas edges are 64, 96, and 128, checked as frameWidth / scale and
frameHeight / scale.";

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
    /// Import an indexed `.aseprite` file. Does not write `.aseprite`.
    ///
    /// Pass `--output` to create a project, or `--project` to replace one.
    /// Indexed 8-bit only. The new document has an empty undo stack.
    ImportAseprite {
        /// Source `.aseprite` or `.ase` file.
        #[arg(long, value_name = "FILE")]
        file: PathBuf,
        /// New project path. Refuses when the file already exists.
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
        /// Existing project to replace after a successful import.
        #[arg(short, long, value_name = "FILE")]
        project: Option<PathBuf>,
    },
    /// Print the Tool API inspect result. Does not write the file.
    Inspect {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Also print an ASCII view of the active frame on stderr.
        #[arg(long)]
        preview: bool,
    },
    /// Print a compact scene summary. Does not write the file or return pixels.
    ///
    /// Canvas size, active frame and layer, layer names, palette hash, and
    /// per-frame delay. `visible` is always true. `opacity` is null.
    Scene {
        #[command(flatten)]
        doc: ProjectArgs,
    },
    /// Print an ASCII view and write a scaled color PNG of one frame.
    ///
    /// Stdout is one JSON line (`result.rows`, `result.frame`, `result.activeFrame`,
    /// `result.legend`, `result.ink`, `result.bounds`, `result.indices`,
    /// `result.pixels`, `result.scale`, and `result.path`). The same rows are
    /// written to stderr. Does not change the project file or the active frame.
    /// `--frame` draws that index; omit it to draw the active frame. `--scale`
    /// matches `export` (integer >= 1, default 1).
    Preview {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Frame to draw. Defaults to the active frame. Does not select it.
        #[arg(long)]
        frame: Option<u32>,
        /// Nearest-neighbor scale, an integer >= 1. Default 1 keeps cell size.
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..))]
        scale: u32,
        /// Color PNG to write. Default is `<project-stem>.preview.png` beside the project.
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
        /// Draw this layer only. Omit to draw the composite. Does not select it.
        #[arg(long)]
        layer: Option<u32>,
        #[command(flatten)]
        scope: ScopeArgs,
    },
    /// Read-only index diff and onion PNG of two frames.
    ///
    /// Does not change the project or the active frame. The PNG is three
    /// panels: the first frame, the second frame, then a blend that keeps a
    /// shared index solid and averages palette colors only where they differ.
    Diff {
        #[command(flatten)]
        doc: ProjectArgs,
        /// First frame. Shown on the left of the PNG.
        #[arg(long)]
        frame: u32,
        /// Second frame. Shown in the middle of the PNG.
        #[arg(long)]
        with: u32,
        /// Nearest-neighbor scale, an integer >= 1. Default 1 keeps cell size.
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..))]
        scale: u32,
        /// Color PNG to write. Default is `<project-stem>.diff.png` beside the project.
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
        /// Compare this layer only. Omit to compare the composite.
        #[arg(long)]
        layer: Option<u32>,
        #[command(flatten)]
        scope: ScopeArgs,
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
    /// Clear an axis-aligned rectangle on the active frame to index 0.
    ///
    /// One undo step. The rectangle must lie entirely inside the canvas.
    /// A locked palette still allows this.
    EraseRect(EraseRectArgs),
    /// Copy palette indices from one rectangle to another.
    ///
    /// Includes index 0. One undo step. Source and destination must both lie
    /// inside the canvas. `--from-frame` reads another frame. `--to-frame`
    /// writes another frame. Omit both to stay on the active frame. Does not
    /// change which frame is active.
    CopyRect(BlitRectArgs),
    /// Move palette indices, then clear the source rectangle.
    ///
    /// One undo step. `--from-frame` / `--to-frame` move a rectangle onto
    /// another frame and clear the source there. Omit both to stay on the
    /// active frame.
    MoveRect(BlitRectArgs),
    /// Mirror palette indices left to right or top to bottom. Does not recolor.
    ///
    /// Pass `--horizontal` or `--vertical`. Omit the rectangle to flip the
    /// whole frame. One undo step. `--frame` chooses the frame and does not
    /// select it.
    Flip(FlipArgs),
    /// Mirror palette indices left to right. Same as `flip --horizontal`.
    Hflip(FlipArgs),
    /// Mirror palette indices top to bottom. Same as `flip --vertical`.
    Vflip(FlipArgs),
    /// Turn palette indices clockwise by 90, 180, or 270 degrees.
    ///
    /// 90 and 270 need a square rectangle. Omit the rectangle to turn the
    /// whole frame, which then has to be square. The canvas size stays.
    Rotate(RotateArgs),
    /// Copy every frame into a new canvas size. The project format stays version 1.
    ///
    /// Omit `--offset-x` and `--offset-y` to center the old pixels. One undo
    /// step. A size outside 1..=256 is refused and writes nothing. A stored
    /// selection moves with the same offset, or is cleared when it would leave
    /// the new canvas. A part origin moves the same way, and that part is
    /// dropped when the origin would leave. Anchors stay in local cells.
    Resize(ResizeArgs),
    /// One rectangular mask stored in the project file.
    ///
    /// Not a lasso. Cells outside the rectangle are protected from paint tools.
    Selection {
        #[command(subcommand)]
        command: SelectionCmd,
    },
    /// Combine rectangles into the stored selection.
    ///
    /// Not a lasso. `union`, `intersect`, and `subtract` update the same
    /// selection paint tools already read. Pass one rectangle, or repeat
    /// `--mask`. Omit the base to use the current selection.
    Mask {
        #[command(subcommand)]
        command: MaskCmd,
    },
    /// Replace one stored palette index on the active frame.
    ///
    /// A selection limits the replace to that rectangle. Does not change the
    /// active layer. `to-index` 0 clears those cells.
    ReplaceIndex(ReplaceIndexArgs),
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
    /// Named animation tags. A tag is a frame range, not a linked cel.
    Tag {
        #[command(subcommand)]
        command: TagCmd,
    },
    /// Layer stack shared by every frame. Index 0 is the bottom.
    Layer {
        #[command(subcommand)]
        command: LayerCmd,
    },
    /// Named parts and local anchors. Not a layer and not a bone UI.
    ///
    /// A part has a rest-pose origin on the canvas and optional layer index.
    /// Anchors are pixel offsets from that origin. `part translate`,
    /// `part rotate`, and `part flip` move the linked ink. Keyframed
    /// parameters live under `curve`, which samples and does not write pixels.
    /// `bake` writes those samples across a frame range. `constraint solve`
    /// tracks a hand, a butt, or a planted foot on one frame.
    Part {
        #[command(subcommand)]
        command: PartCmd,
    },
    /// Keyframes for part translate, rotate, and flip. Frame indices, not a second clock.
    ///
    /// A key sits on an integer frame, the same axis as tags and frame delays.
    /// `sample` may use a fractional frame. The result is the pose relative
    /// to the rest origin. It does not write pixels. Empty curves are omitted
    /// from the project file. Tags and delays stay.
    Curve {
        #[command(subcommand)]
        command: CurveCmd,
    },
    /// Follow, attach, and pin constraints. Solving is an explicit step.
    ///
    /// Adding a constraint does not move pixels. `solve` reads the stored
    /// pose, or a curve pose when `--frame` is set, then translates each
    /// driven part so the named points meet. `--apply` writes that pose
    /// onto one pixel frame. Omit it and the file is unchanged. A second
    /// solve stacks on the stored pose. `bake` walks the timeline instead.
    Constraint {
        #[command(subcommand)]
        command: ConstraintCmd,
    },
    /// Sample curves and constraints into editable pixel frames.
    ///
    /// The source frame (default 0) stays the rest pose and is not rewritten.
    /// Other frames in `--from`..=`--to` get that pose. `--fps` sets each
    /// frame's delay to round(1000/fps) milliseconds. Curves and constraints
    /// stay. One bake is one undo step. There is no script runtime.
    Bake(BakeArgs),
    /// Undo the last edit.
    ///
    /// Without `--checkpoint`, only this process's kernel stack counts, so a
    /// fresh process returns NothingToUndo. With `--checkpoint FILE`, a fresh
    /// process undoes the last saved step: the net project change of one
    /// earlier process. Put `{"tool":"undo"}` in the same `call` to undo a
    /// kernel operation before exit.
    Undo {
        #[command(flatten)]
        doc: ProjectArgs,
    },
    /// Redo the last undone edit.
    ///
    /// Without `--checkpoint`, only this process's kernel stack counts. With
    /// `--checkpoint FILE`, a fresh process reapplies the last restored step.
    Redo {
        #[command(flatten)]
        doc: ProjectArgs,
    },
    /// Write PNG files from the kernel export tools.
    Export {
        #[command(subcommand)]
        command: ExportCmd,
    },
    /// Check a sheet PNG against pixelforge-atlas JSON. Does not edit pixels.
    ///
    /// Exit 0 when the checklist has no errors. Exit 1 when it has an error.
    /// Warnings alone stay exit 0. Exit 2 when the files cannot be read.
    ValidateAtlas(ValidateAtlasArgs),
    /// Print the Tool API tool names as JSON.
    Tools,
}

#[derive(Args)]
pub struct ProjectArgs {
    /// pixelforge-project file. Saved when a tool changes the document.
    #[arg(short, long, value_name = "FILE")]
    pub project: PathBuf,
    /// pixelforge-checkpoint sidecar. The project file stays the pixels.
    ///
    /// Each process that changes the project appends one undo step. A later
    /// `undo` or `redo` with the same file restores that step. Omit it and
    /// the next process starts from an empty stack.
    #[arg(long, value_name = "FILE")]
    pub checkpoint: Option<PathBuf>,
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
    /// Layer to edit. Defaults to the active layer. Does not select it.
    #[arg(long)]
    pub layer: Option<u32>,
    /// Add changed count and dirty bbox to the JSON result.
    #[arg(long)]
    pub dirty: bool,
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
    /// Layer to edit. Defaults to the active layer. Does not select it.
    #[arg(long)]
    pub layer: Option<u32>,
    /// Add changed count and dirty bbox to the JSON result.
    #[arg(long)]
    pub dirty: bool,
    /// After the edit, print an ASCII view of the active frame on stderr.
    #[arg(long)]
    pub preview: bool,
}

#[derive(Args)]
pub struct EraseRectArgs {
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
    /// Layer to edit. Defaults to the active layer. Does not select it.
    #[arg(long)]
    pub layer: Option<u32>,
    /// Add changed count and dirty bbox to the JSON result.
    #[arg(long)]
    pub dirty: bool,
    /// After the edit, print an ASCII view of the active frame on stderr.
    #[arg(long)]
    pub preview: bool,
}

#[derive(Args)]
pub struct BlitRectArgs {
    #[command(flatten)]
    pub doc: ProjectArgs,
    /// Source left column, from 0.
    #[arg(long)]
    pub src_x: u16,
    /// Source top row, from 0.
    #[arg(long)]
    pub src_y: u16,
    /// Width in cells, from 1.
    #[arg(long)]
    pub width: u16,
    /// Height in cells, from 1.
    #[arg(long)]
    pub height: u16,
    /// Destination left column, from 0.
    #[arg(long)]
    pub dst_x: u16,
    /// Destination top row, from 0.
    #[arg(long)]
    pub dst_y: u16,
    /// Frame to read. Defaults to the active frame. Does not select it.
    #[arg(long)]
    pub from_frame: Option<u32>,
    /// Frame to write. Defaults to the active frame. Does not select it.
    #[arg(long)]
    pub to_frame: Option<u32>,
    /// Layer to read and write. Defaults to the active layer. Does not select it.
    #[arg(long)]
    pub layer: Option<u32>,
    /// Add changed count and dirty bbox to the JSON result.
    #[arg(long)]
    pub dirty: bool,
    /// After the edit, print an ASCII view of the active frame on stderr.
    #[arg(long)]
    pub preview: bool,
}

#[derive(Args)]
pub struct FlipArgs {
    #[command(flatten)]
    pub doc: ProjectArgs,
    /// Mirror left to right. Indices move; palette colors stay.
    #[arg(long)]
    pub horizontal: bool,
    /// Mirror top to bottom. Indices move; palette colors stay.
    #[arg(long)]
    pub vertical: bool,
    /// Frame to mirror. Defaults to the active frame. Does not select it.
    #[arg(long)]
    pub frame: Option<u32>,
    /// Left column of the rectangle. Omit with the other edges to flip the whole frame.
    #[arg(long)]
    pub x: Option<u16>,
    /// Top row of the rectangle.
    #[arg(long)]
    pub y: Option<u16>,
    /// Width in cells, from 1.
    #[arg(long)]
    pub width: Option<u16>,
    /// Height in cells, from 1.
    #[arg(long)]
    pub height: Option<u16>,
    /// Layer to mirror. Defaults to the active layer. Does not select it.
    #[arg(long)]
    pub layer: Option<u32>,
    /// Add changed count and dirty bbox to the JSON result.
    #[arg(long)]
    pub dirty: bool,
    /// After the edit, print an ASCII view of the active frame on stderr.
    #[arg(long)]
    pub preview: bool,
}

#[derive(Args)]
pub struct RotateArgs {
    #[command(flatten)]
    pub doc: ProjectArgs,
    /// Clockwise degrees: 90, 180, or 270.
    #[arg(long)]
    pub degrees: u16,
    /// Frame to turn. Defaults to the active frame. Does not select it.
    #[arg(long)]
    pub frame: Option<u32>,
    /// Left column of the rectangle. Omit with the other edges to turn the whole frame.
    #[arg(long)]
    pub x: Option<u16>,
    /// Top row of the rectangle.
    #[arg(long)]
    pub y: Option<u16>,
    /// Width in cells, from 1.
    #[arg(long)]
    pub width: Option<u16>,
    /// Height in cells, from 1.
    #[arg(long)]
    pub height: Option<u16>,
    /// Layer to turn. Defaults to the active layer. Does not select it.
    #[arg(long)]
    pub layer: Option<u32>,
    /// Add changed count and dirty bbox to the JSON result.
    #[arg(long)]
    pub dirty: bool,
    /// After the edit, print an ASCII view of the active frame on stderr.
    #[arg(long)]
    pub preview: bool,
}

#[derive(Args)]
pub struct ResizeArgs {
    #[command(flatten)]
    pub doc: ProjectArgs,
    /// New canvas width, from 1 to 256.
    #[arg(long)]
    pub width: u16,
    /// New canvas height, from 1 to 256.
    #[arg(long)]
    pub height: u16,
    /// Where the old left edge lands. Omit both offsets to center.
    #[arg(long)]
    pub offset_x: Option<i32>,
    /// Where the old top edge lands. Omit both offsets to center.
    #[arg(long)]
    pub offset_y: Option<i32>,
    /// Add changed count, dirty bbox, and clipped ink to the JSON result.
    #[arg(long)]
    pub dirty: bool,
    /// After the edit, print an ASCII view of the active frame on stderr.
    #[arg(long)]
    pub preview: bool,
}

#[derive(Subcommand)]
pub enum SelectionCmd {
    /// Store one rectangle. Later paint tools skip cells outside it.
    Set(SelectionSetArgs),
    /// Remove the rectangle. The project file omits `selection`.
    Clear {
        #[command(flatten)]
        doc: ProjectArgs,
    },
}

/// Limit `preview` or `diff` to the stored selection, one rectangle, or a mask.
#[derive(Args, Default)]
pub struct ScopeArgs {
    /// Use the selection stored in the project. Outside cells are protected.
    #[arg(long)]
    pub selection: bool,
    /// Left column of an explicit rectangle. Pass y, width, and height too.
    #[arg(long)]
    pub x: Option<u16>,
    /// Top row of an explicit rectangle.
    #[arg(long)]
    pub y: Option<u16>,
    /// Width of an explicit rectangle, from 1.
    #[arg(long)]
    pub width: Option<u16>,
    /// Height of an explicit rectangle, from 1.
    #[arg(long)]
    pub height: Option<u16>,
    /// One rectangle of an explicit mask, as `x,y,width,height`. Repeat to combine.
    #[arg(long, value_parser = parse_cell_rect)]
    pub mask: Vec<CellRect>,
}

/// `x,y,width,height` in cells.
#[derive(Clone, Debug)]
pub struct CellRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

fn parse_cell_rect(text: &str) -> Result<CellRect, String> {
    let parts: Vec<&str> = text.split(',').collect();
    if parts.len() != 4 {
        return Err("expected x,y,width,height".to_string());
    }
    let numbers: Vec<u16> = parts
        .iter()
        .map(|part| {
            part.parse::<u16>()
                .map_err(|_| format!("expected x,y,width,height, got {text}"))
        })
        .collect::<Result<_, _>>()?;
    if numbers[2] == 0 || numbers[3] == 0 {
        return Err("width and height must be at least 1".to_string());
    }
    Ok(CellRect {
        x: numbers[0],
        y: numbers[1],
        width: numbers[2],
        height: numbers[3],
    })
}

#[derive(Subcommand)]
pub enum MaskCmd {
    /// Add a rectangle to the current selection, or to `--base-*`.
    Union(MaskArgs),
    /// Keep only the overlap with the current selection, or with `--base-*`.
    Intersect(MaskArgs),
    /// Remove a rectangle from the current selection, or from `--base-*`.
    Subtract(MaskArgs),
}

#[derive(Args)]
pub struct MaskArgs {
    #[command(flatten)]
    pub doc: ProjectArgs,
    /// Left column of the rectangle to combine.
    #[arg(long)]
    pub x: Option<u16>,
    /// Top row of the rectangle to combine.
    #[arg(long)]
    pub y: Option<u16>,
    /// Width of the rectangle to combine, from 1.
    #[arg(long)]
    pub width: Option<u16>,
    /// Height of the rectangle to combine, from 1.
    #[arg(long)]
    pub height: Option<u16>,
    /// One rectangle of the operand, as `x,y,width,height`. Repeat to union them first.
    #[arg(long, value_parser = parse_cell_rect)]
    pub mask: Vec<CellRect>,
    /// Left column of the other mask. Omit all four base edges to use the selection.
    #[arg(long)]
    pub base_x: Option<u16>,
    /// Top row of the other mask.
    #[arg(long)]
    pub base_y: Option<u16>,
    /// Width of the other mask, from 1.
    #[arg(long)]
    pub base_width: Option<u16>,
    /// Height of the other mask, from 1.
    #[arg(long)]
    pub base_height: Option<u16>,
    /// One rectangle of the other mask, as `x,y,width,height`. Repeat to union them first.
    #[arg(long, value_parser = parse_cell_rect)]
    pub base_mask: Vec<CellRect>,
}

#[derive(Args)]
pub struct SelectionSetArgs {
    #[command(flatten)]
    pub doc: ProjectArgs,
    /// Left column, from 0.
    #[arg(long)]
    pub x: u16,
    /// Top row, from 0.
    #[arg(long)]
    pub y: u16,
    /// Width in cells, from 1. The whole rectangle must lie on the canvas.
    #[arg(long)]
    pub width: u16,
    /// Height in cells, from 1.
    #[arg(long)]
    pub height: u16,
}

#[derive(Args)]
pub struct ReplaceIndexArgs {
    #[command(flatten)]
    pub doc: ProjectArgs,
    /// Stored index to find, including 0.
    #[arg(long)]
    pub from_index: u8,
    /// Index to write. 0 clears those cells. Any other index must exist.
    #[arg(long)]
    pub to_index: u8,
    /// Layer to edit. Defaults to the active layer. Does not select it.
    #[arg(long)]
    pub layer: Option<u32>,
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
    /// Layer to edit. Defaults to the active layer. Does not select it.
    #[arg(long)]
    pub layer: Option<u32>,
    /// Add changed count and dirty bbox to the JSON result.
    #[arg(long)]
    pub dirty: bool,
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
    /// Choose how paint treats an index past the palette. Default is reject.
    ///
    /// `clamp` writes the last opaque index. Index 0 is still refused.
    /// The same value is not recorded. `reject` is omitted from the project file.
    Enforce {
        #[command(flatten)]
        doc: ProjectArgs,
        /// `reject` or `clamp`.
        #[arg(long, value_enum)]
        mode: IndexMode,
    },
    /// Quantize to `--max-colors`, or nearest-map onto a palette file.
    ///
    /// Pass exactly one of `--max-colors` and `--file`. One undo step.
    /// Refused while the palette is locked. The result lists `misses`,
    /// `merges`, `dropped`, and `pixelsChanged`.
    Remap {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Keep at most this many opaque colors. Index 0 is extra.
        #[arg(long, value_parser = clap::value_parser!(u8).range(1..=255))]
        max_colors: Option<u8>,
        /// Target palette: GIMP `.gpl` or a hex list.
        #[arg(long, value_name = "FILE")]
        file: Option<PathBuf>,
    },
    /// Crush one PNG onto the current palette. Does not add colors.
    ///
    /// Width and height must match the canvas. Alpha below 128 becomes
    /// index 0. A locked palette still allows this. Not a Lospec browser
    /// and not a sprite-sheet importer.
    ApplyPng {
        #[command(flatten)]
        doc: ProjectArgs,
        /// PNG file. Indexed, RGB, and RGBA are expanded in the kernel.
        #[arg(long, value_name = "FILE")]
        file: PathBuf,
        /// Frame to write. Defaults to the active frame. Does not select it.
        #[arg(long)]
        frame: Option<u32>,
        /// Layer to write. Defaults to the active layer. Does not select it.
        #[arg(long)]
        layer: Option<u32>,
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

#[derive(Clone, Copy, ValueEnum)]
pub enum IndexMode {
    Reject,
    Clamp,
}

impl IndexMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reject => "reject",
            Self::Clamp => "clamp",
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
    ///
    /// Does not select the copy. The result `index` is the new frame. Run
    /// `frame select` on that index before painting it. If the active frame
    /// sits at or after the insertion point, its index increases by one and
    /// it still shows the same pixels. The web editor's duplicate button
    /// selects the copy; this command does not.
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
pub enum LayerCmd {
    /// Print the layer stack. Index 0 is drawn first. Does not change the file.
    List {
        #[command(flatten)]
        doc: ProjectArgs,
    },
    /// Insert a transparent layer above the active one and select it.
    Add {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Layer name. Omit for the next free `Layer N`.
        #[arg(long)]
        name: Option<String>,
        /// `default` or `polish`. `polish` is an overlay bake never writes.
        #[arg(long, value_enum)]
        role: Option<LayerRoleArg>,
    },
    /// Set whether bake may rebuild this layer. Pixels stay.
    Role {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Layer index, from 0. Defaults to the active layer.
        #[arg(long)]
        index: Option<u32>,
        /// `default` lets bake rebuild this layer. `polish` keeps hand-tuned cells.
        #[arg(long, value_enum)]
        role: LayerRoleArg,
    },
    /// Remove a layer from every frame. The last layer is refused.
    #[command(alias = "delete")]
    Remove {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Layer index, from 0. Defaults to the active layer.
        #[arg(long)]
        index: Option<u32>,
    },
    /// Rename a layer. The stored name is trimmed.
    Rename {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Layer index, from 0. Defaults to the active layer.
        #[arg(long)]
        index: Option<u32>,
        /// New name, 1 to 64 characters.
        #[arg(long)]
        name: String,
    },
    /// Select the active layer. Drawing commands use it when `--layer` is omitted.
    Select {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Layer index, from 0.
        #[arg(long)]
        index: u32,
    },
    /// Move a layer so it ends at `--to`. Index 0 stays the bottom.
    Reorder {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Current index.
        #[arg(long)]
        from: u32,
        /// Index after the move.
        #[arg(long)]
        to: u32,
    },
    /// Composite every frame into one layer named `Layer 1`.
    ///
    /// The saved file is the single-layer project older tools already open.
    Flatten {
        #[command(flatten)]
        doc: ProjectArgs,
    },
}

/// Bake role of one layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum LayerRoleArg {
    /// Ordinary pixels. Bake may rebuild this layer from the rest pose.
    Default,
    /// Overlay. Bake never writes these cells.
    Polish,
}

impl LayerRoleArg {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Polish => "polish",
        }
    }
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
        /// Nearest-neighbor scale, an integer >= 1. Default 1 keeps cell size.
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..))]
        scale: u32,
    },
    /// Write one grid sprite sheet. Layout matches the kernel.
    Sheet {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Destination PNG path.
        #[arg(short, long, value_name = "FILE")]
        output: PathBuf,
        /// Nearest-neighbor scale, an integer >= 1. Default 1 keeps cell size.
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..))]
        scale: u32,
        /// Columns in the frame grid. Omit to keep ceil(sqrt(frame count)).
        #[arg(long, value_name = "N", value_parser = clap::value_parser!(u32).range(1..))]
        cols: Option<u32>,
        /// Write pixelforge-atlas JSON for this sheet. Omit to keep the PNG-only result.
        #[arg(long, value_name = "FILE")]
        atlas: Option<PathBuf>,
    },
}

#[derive(Args)]
pub struct ValidateAtlasArgs {
    /// pixelforge-atlas JSON written by `export sheet --atlas`.
    #[arg(long, value_name = "FILE")]
    pub atlas: PathBuf,
    /// Sheet PNG. Omit to use the atlas `image` file beside the JSON.
    #[arg(long, value_name = "FILE")]
    pub sheet: Option<PathBuf>,
    /// `pixelforge-atlas-rules` JSON. Flags override the fields they set.
    #[arg(long, value_name = "FILE")]
    pub rules: Option<PathBuf>,
    /// Convention misses are errors. This is the default.
    #[arg(long, conflicts_with = "loose")]
    pub strict: bool,
    /// Convention misses are warnings. Overlap, bounds, grid, and padding stay errors.
    #[arg(long)]
    pub loose: bool,
    /// Allowed unscaled canvas edge. Repeating replaces 64, 96, and 128.
    #[arg(long, value_name = "PX", conflicts_with = "any_size")]
    pub size: Vec<u32>,
    /// Skip the 64 / 96 / 128 canvas-tier check.
    #[arg(long)]
    pub any_size: bool,
    /// Tag name that must be present. Replaces `requireTags` from the rules file.
    #[arg(long = "require-tag", value_name = "NAME")]
    pub require_tag: Vec<String>,
    /// Four-direction names: check automatically, always, or never.
    #[arg(long, value_enum)]
    pub four_direction: Option<FourDirectionArg>,
    /// Empty tag list is an error, or it is not a finding.
    #[arg(long, value_enum)]
    pub tag_policy: Option<TagPolicyArg>,
    /// Expected pivot x in frame pixels. Top-left is 0 and y grows downward.
    #[arg(long, requires = "pivot_y", allow_hyphen_values = true)]
    pub pivot_x: Option<f64>,
    /// Expected pivot y in frame pixels. The bottom edge is the frame height.
    #[arg(long, requires = "pivot_x", allow_hyphen_values = true)]
    pub pivot_y: Option<f64>,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum FourDirectionArg {
    /// Check when a tag is `{idle|walk}_{down|left|right|up}`.
    Auto,
    /// Always apply the four-direction names.
    On,
    /// Do not apply the four-direction names.
    Off,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum TagPolicyArg {
    /// An empty tag list is an error in both modes.
    Required,
    /// An empty tag list is not a finding.
    Optional,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum LoopArg {
    Forward,
    Reverse,
    Pingpong,
    Once,
}

impl LoopArg {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Forward => "forward",
            Self::Reverse => "reverse",
            Self::Pingpong => "pingpong",
            Self::Once => "once",
        }
    }
}

#[derive(Subcommand)]
pub enum TagCmd {
    /// Add a tag over an inclusive frame range. `loop` defaults to forward.
    Add {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Unique name. Trimmed. 1 to 64 characters.
        #[arg(long)]
        name: String,
        /// First frame, inclusive.
        #[arg(long)]
        from: u32,
        /// Last frame, inclusive.
        #[arg(long)]
        to: u32,
        /// forward, reverse, pingpong, or once.
        #[arg(long = "loop", value_enum, default_value = "forward")]
        loop_mode: LoopArg,
    },
    /// Change a tag's range, loop, or name. Pass at least one change.
    Set {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Current name.
        #[arg(long)]
        name: String,
        /// New first frame, inclusive. Omit to keep it.
        #[arg(long)]
        from: Option<u32>,
        /// New last frame, inclusive. Omit to keep it.
        #[arg(long)]
        to: Option<u32>,
        /// forward, reverse, pingpong, or once.
        #[arg(long = "loop", value_enum)]
        loop_mode: Option<LoopArg>,
        /// New unique name. Omit to keep the current name.
        #[arg(long)]
        rename: Option<String>,
    },
    /// Remove a tag by name. Frames stay.
    Remove {
        #[command(flatten)]
        doc: ProjectArgs,
        #[arg(long)]
        name: String,
    },
}

#[derive(Subcommand)]
pub enum PartCmd {
    /// Print parts and anchors. Does not change the file.
    List {
        #[command(flatten)]
        doc: ProjectArgs,
    },
    /// Add a named part. The origin is the canvas cell that is local (0, 0).
    Add {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Unique name. Trimmed. 1 to 64 characters.
        #[arg(long)]
        name: String,
        /// Origin column, from 0. Must lie on the canvas.
        #[arg(long)]
        x: u16,
        /// Origin row, from 0. Must lie on the canvas.
        #[arg(long)]
        y: u16,
        /// Paint layer this part names. Omit to leave the part unattached.
        #[arg(long)]
        layer: Option<u32>,
    },
    /// Change a part's origin, layer, or name. Pass at least one change.
    Set {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Current name.
        #[arg(long)]
        name: String,
        /// New origin column. Omit to keep it.
        #[arg(long)]
        x: Option<u16>,
        /// New origin row. Omit to keep it.
        #[arg(long)]
        y: Option<u16>,
        /// Attach the part to this layer. Omit to keep the attachment.
        #[arg(long)]
        layer: Option<u32>,
        /// Drop the layer attachment. The part and its anchors stay.
        #[arg(long)]
        clear_layer: bool,
        /// New unique name. Omit to keep the current name.
        #[arg(long)]
        rename: Option<String>,
    },
    /// Remove a part and its anchors. Pixels stay.
    Remove {
        #[command(flatten)]
        doc: ProjectArgs,
        #[arg(long)]
        name: String,
    },
    /// Set or remove one anchor in a part's local pixel grid.
    Anchor {
        #[command(subcommand)]
        command: AnchorCmd,
    },
    /// Move a part by whole cells. Local anchors stay. Linked ink moves with the origin.
    Translate {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Part name.
        #[arg(long)]
        name: String,
        /// Cells to add to x. A leading minus is a value.
        #[arg(long, allow_hyphen_values = true)]
        dx: i32,
        /// Cells to add to y. A leading minus is a value.
        #[arg(long, allow_hyphen_values = true)]
        dy: i32,
        /// Frame whose linked layer moves. Defaults to the active frame. Does not select it.
        #[arg(long)]
        frame: Option<u32>,
        /// Add changed count and dirty bbox to the JSON result.
        #[arg(long)]
        dirty: bool,
    },
    /// Turn a part clockwise around its origin, or around `--anchor`.
    Rotate {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Part name.
        #[arg(long)]
        name: String,
        /// Clockwise degrees. Near-multiples of 90 are exact. Other angles snap to 0.01°.
        #[arg(long, allow_hyphen_values = true)]
        degrees: f64,
        /// Pivot anchor. Omit to pivot on the part origin.
        #[arg(long)]
        anchor: Option<String>,
        /// Frame whose linked layer turns. Defaults to the active frame. Does not select it.
        #[arg(long)]
        frame: Option<u32>,
        /// Add changed count and dirty bbox to the JSON result.
        #[arg(long)]
        dirty: bool,
    },
    /// Mirror a part around its origin, or around `--anchor`.
    Flip {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Part name.
        #[arg(long)]
        name: String,
        /// Mirror left to right. Pass exactly one direction.
        #[arg(long)]
        horizontal: bool,
        /// Mirror top to bottom. Pass exactly one direction.
        #[arg(long)]
        vertical: bool,
        /// Pivot anchor. Omit to pivot on the part origin.
        #[arg(long)]
        anchor: Option<String>,
        /// Frame whose linked layer mirrors. Defaults to the active frame. Does not select it.
        #[arg(long)]
        frame: Option<u32>,
        /// Add changed count and dirty bbox to the JSON result.
        #[arg(long)]
        dirty: bool,
    },
}

#[derive(Subcommand)]
pub enum AnchorCmd {
    /// Create or replace a named anchor. x and y are cells from the part origin.
    Set {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Part name.
        #[arg(long)]
        part: String,
        /// Anchor name. Trimmed. 1 to 64 characters. Unique on this part.
        #[arg(long)]
        name: String,
        /// Local column, from -256 to 256. A leading minus is a value.
        #[arg(long, allow_hyphen_values = true)]
        x: i32,
        /// Local row, from -256 to 256. A leading minus is a value.
        #[arg(long, allow_hyphen_values = true)]
        y: i32,
    },
    /// Remove one anchor. The part stays.
    Remove {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Part name.
        #[arg(long)]
        part: String,
        #[arg(long)]
        name: String,
    },
}

#[derive(Args)]
pub struct BakeArgs {
    #[command(flatten)]
    pub doc: ProjectArgs,
    /// First frame index, inclusive. Default 0.
    #[arg(long)]
    pub from: Option<u32>,
    /// Last frame index, inclusive. Default is the last existing frame.
    /// Past the end appends posed frames.
    #[arg(long)]
    pub to: Option<u32>,
    /// Rest-pose frame. Default 0. This frame's pixels are not rewritten.
    #[arg(long)]
    pub source: Option<u32>,
    /// Playback rate. Sets delayMs to round(1000/fps). A leading minus is a value.
    #[arg(long, allow_hyphen_values = true)]
    pub fps: Option<f64>,
    /// Pose only this part. Repeat for more than one. Omit to pose every part.
    #[arg(long = "part")]
    pub parts: Vec<String>,
    /// Write only this layer. Repeat for more than one.
    #[arg(long = "layer")]
    pub layers: Vec<u32>,
    /// Sample curves and do not solve constraints.
    #[arg(long)]
    pub curves_only: bool,
}

#[derive(Subcommand)]
pub enum CurveCmd {
    /// Set one key. The ease leaves this key toward the next one.
    Set {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Part name.
        #[arg(long)]
        part: String,
        /// rotation, translateX, translateY, flipHorizontal, or flipVertical.
        #[arg(long)]
        channel: String,
        /// Frame index, from 0. The same axis as tags.
        #[arg(long)]
        frame: u32,
        /// Degrees, whole cells, or 0/1 for flip. A leading minus is a value.
        #[arg(long, allow_hyphen_values = true)]
        value: f64,
        /// linear, easeInOut, or hold. Flip is hold only. Omit for the channel default.
        #[arg(long)]
        ease: Option<String>,
    },
    /// List curves. Pass `--part` to read one part. Does not change the file.
    List {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Part name. Omit to list every curve.
        #[arg(long)]
        part: Option<String>,
    },
    /// Sample one channel, or every channel of a part when `--channel` is omitted.
    ///
    /// `--frame` may be fractional. 2 is frame 2. 2.5 is halfway to frame 3
    /// in index space, not in delayMs. Does not write pixels.
    Sample {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Part name.
        #[arg(long)]
        part: String,
        /// Channel. Omit to sample every keyed channel of the part.
        #[arg(long)]
        channel: Option<String>,
        /// Frame position. A leading minus is a value.
        #[arg(long, allow_hyphen_values = true)]
        frame: f64,
    },
    /// Remove one key. The last key drops the curve.
    Remove {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Part name.
        #[arg(long)]
        part: String,
        /// Channel name.
        #[arg(long)]
        channel: String,
        /// Frame index of the key.
        #[arg(long)]
        frame: u32,
    },
    /// Remove every key of one part channel.
    Clear {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Part name.
        #[arg(long)]
        part: String,
        /// Channel name.
        #[arg(long)]
        channel: String,
    },
}

#[derive(Subcommand)]
pub enum ConstraintCmd {
    /// Store one constraint. `stick` is saved as `attach`. Does not move pixels.
    Add {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Constraint name. Trimmed. 1 to 64 characters. Unique in the file.
        #[arg(long)]
        name: String,
        /// follow, attach, stick, pin, ik, or plant.
        #[arg(long)]
        kind: String,
        /// Part this constraint moves. For ik, the end of the chain.
        #[arg(long)]
        part: String,
        /// Follower anchor. Omit on follow, pin, or plant to use the origin.
        /// On ik this is the end effector. Omit it to aim the end origin.
        #[arg(long)]
        anchor: Option<String>,
        /// Part that follow, attach, or ik tracks.
        #[arg(long)]
        target: Option<String>,
        /// Anchor on the target. Required for follow, attach, and an ik part target.
        #[arg(long)]
        target_anchor: Option<String>,
        /// Pin or ik column. A leading minus is a value, and it must lie on the canvas.
        #[arg(long, allow_hyphen_values = true)]
        x: Option<i32>,
        /// Pin or ik row. A leading minus is a value, and it must lie on the canvas.
        #[arg(long, allow_hyphen_values = true)]
        y: Option<i32>,
        /// IK joint, root first. Repeat two or three times. The last name is `--part`.
        #[arg(long = "chain")]
        chain: Vec<String>,
        /// `1` puts the first joint on the clockwise side of root-to-target. `-1` is the other side.
        #[arg(long, allow_hyphen_values = true)]
        bend: Option<i8>,
        /// Footprint span `frame:x:y` or `frame:to:x:y`. Repeat for each plant.
        /// A sample inside the span keeps that cell. Frames outside every span are not pinned.
        #[arg(long = "contact")]
        contact: Vec<String>,
    },
    /// List constraints. Does not change the file.
    List {
        #[command(flatten)]
        doc: ProjectArgs,
    },
    /// Remove one constraint. Parts and pixels stay.
    Remove {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Constraint name.
        #[arg(long)]
        name: String,
    },
    /// Sample an optional curve pose, then solve. `--apply` writes one frame.
    ///
    /// Omit `--frame` to use the stored pose, including a pose `part rotate`
    /// already wrote. `--frame` may be fractional. Rotation, flips, and
    /// translation are sampled around the stored origin, then constraints
    /// translate followers. A cycle, a second driver, a missing anchor, or a
    /// cell off the canvas returns an error and writes nothing.
    Solve {
        #[command(flatten)]
        doc: ProjectArgs,
        /// Curve frame. A leading minus is a value. Omit to skip curves.
        #[arg(long, allow_hyphen_values = true)]
        frame: Option<f64>,
        /// Write the pose onto `--pixel-frame` or the active frame.
        #[arg(long)]
        apply: bool,
        /// Pixel frame to write. Omit for the active frame.
        #[arg(long)]
        pixel_frame: Option<u32>,
    },
}
