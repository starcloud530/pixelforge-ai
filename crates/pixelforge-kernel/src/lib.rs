//! PixelForge kernel.
//!
//! Source of truth for canvas size, indexed pixels, palette entries, and
//! frames. Paint, erase, palette edits, and frame edits write this document
//! and push the same edits onto its undo stack. The TypeScript shell previews
//! this state;
//! it does not own the pixels or a second history.
//!
//! Project files, PNG export, and the Tool API are later layers on this
//! document model. See `docs/tech-stack.md` and `docs/dev-setup.md`.
//! The browser shell calls this crate through `pixelforge-kernel-wasm`.

mod color;
mod document;
mod error;
mod frame;
mod history;
mod palette;

pub use color::Rgba;
pub use document::Document;
pub use error::KernelError;
pub use frame::Frame;
pub use palette::Palette;

/// Oldest undo entries past this depth are dropped.
pub const MAX_UNDO: usize = 100;

/// Largest canvas edge this skeleton accepts. M1 sizes are 16 and 32.
pub const MAX_CANVAS_EDGE: u16 = 256;

/// Palette index of an empty cell. Paint cannot write this index; erase can.
pub const TRANSPARENT_INDEX: u8 = 0;
