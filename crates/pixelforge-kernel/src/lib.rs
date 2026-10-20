//! PixelForge kernel.
//!
//! Source of truth for canvas size, indexed pixels, palette entries, frames,
//! and per-frame delay. Paint, erase, rectangle fill, rectangle erase, and
//! rectangle copy or move write palette indices, never raw RGB.
//! Palette edits, palette lock, palette import, frame edits, and delay edits
//! write this document and push the same edits onto its undo stack.
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
mod project;
mod tool_api;

pub use color::Rgba;
pub use document::Document;
pub use error::{KernelError, ProjectFileError};
pub use export::{sprite_sheet_layout, SpriteSheetLayout};
pub use frame::Frame;
pub use palette::Palette;
pub use palette_file::PaletteImportMode;
pub use project::{PROJECT_FORMAT, PROJECT_VERSION};
pub use tool_api::{TOOL_API_VERSION, TOOL_NAMES};

/// Oldest undo entries past this depth are dropped.
pub const MAX_UNDO: usize = 100;

/// Largest canvas edge this skeleton accepts. M1 sizes are 16 and 32.
pub const MAX_CANVAS_EDGE: u16 = 256;

/// Palette index of an empty cell. Paint cannot write this index; erase can.
pub const TRANSPARENT_INDEX: u8 = 0;

/// Delay used when a frame is created, or when a project file omits `delayMs`.
pub const DEFAULT_FRAME_DELAY_MS: u32 = 100;

/// Shortest delay [`Document::set_frame_delay`] accepts.
pub const MIN_FRAME_DELAY_MS: u32 = 1;

/// Longest delay [`Document::set_frame_delay`] accepts.
pub const MAX_FRAME_DELAY_MS: u32 = 65_535;

/// Scale for [`Document::scrub`] and [`Document::frame_at_ratio`].
/// `0` is the start of the first frame and this value is the end of the last.
pub const TIMELINE_RATIO_PPM: u32 = 1_000_000;
