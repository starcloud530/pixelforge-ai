//! PixelForge kernel.
//!
//! Source of truth for canvas size, indexed pixels, palette entries, frames,
//! layers, per-frame delay, named animation tags, named parts with
//! anchors, keyframed part-transform curves, and follow, attach, pin,
//! short-chain IK, and footprint constraints. Paint, erase, rectangle fill, rectangle erase,
//! rectangle copy or move, horizontal flip, and canvas resize write palette
//! indices on the active layer, never raw RGB. `line`, `ellipse`, `polygon`,
//! `fill`, and `stroke_rect` are the same kind of indexed write. Preview, diff, and PNG export
//! composite the stack. A hidden layer, or a layer at opacity 0, is skipped.
//! Index 0 is transparent and any other index covers the layer under it.
//! Partial layer opacity blends only in the RGBA composite and is not snapped
//! back to a palette index.
//! Palette edits, palette lock, palette import, frame edits, and delay edits
//! write this document and push the same edits onto its undo stack.
//! A selection mask, when stored, limits those paint writes to that mask.
//! The mask is one rectangle, the rectangles of a connected component, or the
//! rectangles left by union, intersect, and subtract. The project file stores
//! it and omits it when clear. There is no lasso.
//! Loop playback advances the active frame from those delays. The TypeScript
//! shell previews this state; it does not own the pixels, the clock, or a
//! second history.
//!
//! Project files are version 1 `pixelforge-project` JSON from
//! [`Document::save_project`] and [`Document::load_project`]. PNG sequence and
//! sprite-sheet export resolve the indexed palette to RGBA. APNG preview
//! export writes that same composite as one looping file. It is not a sprite
//! sheet.
//! [`validate_atlas`] checks that sheet against its `pixelforge-atlas` and does
//! not edit pixels.
//! [`Document::frame_rgba`] returns that same buffer for view compositing such
//! as onion skin; it is not part of the document. [`Document::call_tool`] is
//! the stable Tool API: named JSON tools that dispatch onto this document and
//! its undo stack. The local `pixelforge` command (`crates/pixelforge-cli`)
//! and the browser shell (through `pixelforge-kernel-wasm`) are callers.
//! See `docs/tech-stack.md`, `docs/dev-setup.md`, `docs/export.md`,
//! `docs/tool-api.md`, and `docs/agent-skill.md`.

mod apng;
mod aseprite;
mod atlas;
mod color;
mod constraint;
mod curve;
mod document;
mod error;
mod export;
mod frame;
mod history;
mod palette;
mod palette_file;
mod palette_map;
mod part;
mod part_xform;
mod project;
mod raster;
mod scene;
mod selection;
mod tag;
mod tool_api;

pub use apng::{ApngExport, ApngFrame, ApngSource};
pub use aseprite::AsepriteReport;
pub use atlas::{
    decode_sheet_png, validate_atlas, AtlasChecks, AtlasFinding, AtlasInputError, AtlasMode,
    AtlasReport, AtlasRules, FourDirection, Pivot, SheetImage, TagPolicy, FORMAL_FRAME_EDGES,
};
pub use color::Rgba;
pub use constraint::{Constraint, ConstraintKind, Contact, SolveReport};
pub use curve::{
    Curve, CurveChannel, CurveKey, CurveRemoval, CurveSample, CurveWrite, Ease, PartSample,
    SampleHit,
};
pub use document::{BakeFrame, BakeOptions, BakeReport, Document, QuarterTurn};
pub use error::{KernelError, ProjectFileError};
pub use export::{sprite_sheet_layout, SpriteSheetLayout};
pub use frame::Frame;
pub use palette::{IndexPolicy, Palette};
pub use palette_file::PaletteImportMode;
pub use part::{Anchor, LayerAttach, Part, PartUpdate};
pub use part_xform::PartPose;
pub use project::{PROJECT_FORMAT, PROJECT_VERSION};
pub use raster::Connectivity;
pub use selection::{Mask, MaskOp, Selection};
pub use tag::{LoopMode, Tag, TagUpdate};
pub use tool_api::{TOOL_API_VERSION, TOOL_NAMES};

/// Oldest undo entries past this depth are dropped.
pub const MAX_UNDO: usize = 100;

/// Largest canvas edge this skeleton accepts. M1 sizes are 16 and 32.
pub const MAX_CANVAS_EDGE: u16 = 256;

/// Palette index of an empty cell. Paint cannot write this index; erase can.
pub const TRANSPARENT_INDEX: u8 = 0;

/// Delay used when a frame is created, or when a project file omits `delayMs`.
pub const DEFAULT_FRAME_DELAY_MS: u32 = 100;

/// Name of the only layer when a project file has no layer stack.
pub const DEFAULT_LAYER_NAME: &str = "Layer 1";

/// Fully opaque layer. The project file omits `opacity` at this value.
///
/// `0` skips the layer in every composite. `1..=254` blends in RGBA export
/// and still covers in the index composite. `255` covers with the palette
/// color and does not blend palette alpha into the layer underneath.
pub const LAYER_OPACITY_OPAQUE: u8 = 255;

/// Whether bake may rebuild a layer from the rest pose.
///
/// `default` is an ordinary paint layer. A curve or a constraint can manage
/// it, and bake then replaces that whole layer. `polish` is an overlay for
/// hand-tuned cells. Bake does not rebuild it from a linked part. A polish
/// layer may instead bind to a part; bake then restamps that overlay from the
/// source frame through the part's pose. Unbound polish stays on the canvas
/// cells where it was painted. The project file omits `role` when every layer
/// is `default`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayerRole {
    /// Ordinary pixels. Bake may replace this layer from the rest pose.
    #[default]
    Default,
    /// Overlay. Bake does not rebuild these cells from a linked part.
    Polish,
}

/// A polish overlay whose cells follow one part.
///
/// Cells stay in the frame buffer at canvas coordinates. On the source frame
/// those coordinates are the rest pose, so local `(0, 0)` is the part origin,
/// or `anchor` when that name is set. Bake and part translate / rotate / flip
/// move the marked cells with that part. The file omits the binding when the
/// layer is unbound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolishBind {
    pub part: String,
    /// Named anchor. `None` means the part origin is local `(0, 0)`.
    pub anchor: Option<String>,
}

impl LayerRole {
    /// Stable name stored in the project file and the Tool API.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Polish => "polish",
        }
    }

    /// Parses `default` or `polish`.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "default" => Some(Self::Default),
            "polish" => Some(Self::Polish),
            _ => None,
        }
    }
}

/// Longest layer name, in Unicode scalar values, after trimming.
pub const MAX_LAYER_NAME_CHARS: usize = 64;

/// Longest animation tag name, in Unicode scalar values, after trimming.
pub const MAX_TAG_NAME_CHARS: usize = 64;

/// Longest part or anchor name, in Unicode scalar values, after trimming.
pub const MAX_PART_NAME_CHARS: usize = 64;

/// Shortest delay [`Document::set_frame_delay`] accepts.
pub const MIN_FRAME_DELAY_MS: u32 = 1;

/// Longest delay [`Document::set_frame_delay`] accepts.
pub const MAX_FRAME_DELAY_MS: u32 = 65_535;

/// Largest frame count [`Document::bake`] will leave behind.
///
/// `to` is an inclusive index, so it must be below this. The cap stops a
/// request from appending billions of frames.
pub const MAX_BAKE_FRAMES: usize = 4096;

/// Scale for [`Document::scrub`] and [`Document::frame_at_ratio`].
/// `0` is the start of the first frame and this value is the end of the last.
pub const TIMELINE_RATIO_PPM: u32 = 1_000_000;

/// Glyph for an ASCII view of one palette index.
///
/// Index 0 is `.`. Indices 1–9 are digits, 10–35 are `A`–`Z`, 36–61 are
/// `a`–`z`, and 62 and above share `*`. This does not invent a color.
pub fn cell_glyph(index: u8) -> char {
    match index {
        0 => '.',
        1..=9 => char::from(b'0' + index),
        10..=35 => char::from(b'A' + (index - 10)),
        36..=61 => char::from(b'a' + (index - 36)),
        _ => '*',
    }
}
