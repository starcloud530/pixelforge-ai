//! PixelForge kernel.
//!
//! Source of truth for canvas size, indexed pixels, palette entries, frames,
//! and per-frame delay. Paint, erase, palette edits, frame edits, and delay
//! edits write this document and push the same edits onto its undo stack.
//! Loop playback advances the active frame from those delays. The TypeScript
//! shell previews this state; it does not own the pixels, the clock, or a
//! second history.
//!
//! Project files are version 1 `pixelforge-project` JSON from
//! [`Document::save_project`] and [`Document::load_project`]. PNG sequence and
//! sprite-sheet export resolve the indexed palette to RGBA. The Tool API is a
//! later layer. See `docs/tech-stack.md`, `docs/dev-setup.md`, and
//! `docs/export.md`. The browser shell calls this crate through
//! `pixelforge-kernel-wasm`.

mod color;
mod document;
mod error;
mod export;
mod frame;
mod history;
mod palette;
mod project;

pub use color::Rgba;
pub use document::Document;
pub use error::{KernelError, ProjectFileError};
pub use export::{sprite_sheet_layout, SpriteSheetLayout};
pub use frame::Frame;
pub use palette::Palette;
pub use project::{PROJECT_FORMAT, PROJECT_VERSION};

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
