//! PixelForge kernel.
//!
//! Source of truth for canvas size, indexed pixels, palette entries, and
//! frames. Paint and erase write palette indices into the active frame.
//! The TypeScript shell previews this state; it does not own the pixels.
//!
//! Project files, PNG export, undo, and the Tool API are later layers on
//! this document model. See `docs/tech-stack.md` and `docs/dev-setup.md`.
//! The browser shell calls this crate through `pixelforge-kernel-wasm`.

mod color;
mod document;
mod error;
mod frame;
mod palette;

pub use color::Rgba;
pub use document::Document;
pub use error::KernelError;
pub use frame::Frame;
pub use palette::Palette;

/// Largest canvas edge this skeleton accepts. M1 sizes are 16 and 32.
pub const MAX_CANVAS_EDGE: u16 = 256;

/// Palette index of an empty cell. Paint cannot write this index; erase can.
pub const TRANSPARENT_INDEX: u8 = 0;
