//! PixelForge kernel.
//!
//! Source of truth for canvas size, indexed pixels, palette entries, frames,
//! layers, per-frame delay, and named animation tags. Paint, erase, rectangle fill, rectangle erase,
//! rectangle copy or move, horizontal flip, and canvas resize write palette
//! indices on the active layer, never raw RGB. Preview, diff, and PNG export
//! composite the stack: index 0 is transparent and any other index covers the
//! layer under it.
//! Palette edits, palette lock, palette import, frame edits, and delay edits
//! write this document and push the same edits onto its undo stack.
//! A rectangular selection, when stored, limits those paint writes to that
//! rectangle. The project file stores the rectangle and omits it when clear.
//! There is no lasso and no magic wand.
//! Loop playback advances the active frame from those delays. The TypeScript
//! shell previews this state; it does not own the pixels, the clock, or a
//! second history.
//!
//! Project files are version 1 `pixelforge-project` JSON from
//! [`Document::save_project`] and [`Document::load_project`]. PNG sequence and
//! sprite-sheet export resolve the indexed palette to RGBA.
//! [`Document::frame_rgba`] returns that same buffer for view compositing such
//! as onion skin; it is not part of the document. [`Document::call_tool`] is
//! the stable Tool API: named JSON tools that dispatch onto this document and
//! its undo stack. The local `pixelforge` command (`crates/pixelforge-cli`)
//! and the browser shell (through `pixelforge-kernel-wasm`) are callers.
//! See `docs/tech-stack.md`, `docs/dev-setup.md`, `docs/export.md`,
//! `docs/tool-api.md`, and `docs/agent-skill.md`.

mod color;
mod document;
mod error;
mod export;
mod frame;
mod history;
mod palette;
mod palette_file;
mod palette_map;
mod project;
mod scene;
mod selection;
mod tag;
mod tool_api;

pub use color::Rgba;
pub use document::Document;
pub use error::{KernelError, ProjectFileError};
pub use export::{sprite_sheet_layout, SpriteSheetLayout};
pub use frame::Frame;
pub use palette::{IndexPolicy, Palette};
pub use palette_file::PaletteImportMode;
pub use project::{PROJECT_FORMAT, PROJECT_VERSION};
pub use selection::Selection;
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

/// Longest layer name, in Unicode scalar values, after trimming.
pub const MAX_LAYER_NAME_CHARS: usize = 64;

/// Longest animation tag name, in Unicode scalar values, after trimming.
pub const MAX_TAG_NAME_CHARS: usize = 64;

/// Shortest delay [`Document::set_frame_delay`] accepts.
pub const MIN_FRAME_DELAY_MS: u32 = 1;

/// Longest delay [`Document::set_frame_delay`] accepts.
pub const MAX_FRAME_DELAY_MS: u32 = 65_535;

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
