use std::fmt;

use crate::MAX_CANVAS_EDGE;

/// Recoverable kernel failure. Callers leave the document unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelError {
    /// Width or height is outside `1..=MAX_CANVAS_EDGE`.
    InvalidSize { width: u16, height: u16 },
    /// Cell is outside the canvas.
    OutOfBounds {
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    },
    /// Index is 0 (transparent) or past the end of the palette.
    PaletteIndex { index: u8 },
    /// A palette must contain at least the transparent slot.
    EmptyPalette,
    /// More than 256 entries would not fit in a `u8` index.
    PaletteTooLarge { len: usize },
    /// Entry 0 has to stay transparent so erase has one meaning.
    TransparentSlot,
    /// Frame index is outside the frame list.
    FrameIndex { index: usize, len: usize },
    /// The document always keeps one frame.
    LastFrame,
}

impl fmt::Display for KernelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSize { width, height } => write!(
                f,
                "canvas size {width}x{height} is outside 1..={MAX_CANVAS_EDGE}"
            ),
            Self::OutOfBounds {
                x,
                y,
                width,
                height,
            } => write!(f, "cell {x},{y} is outside {width}x{height}"),
            Self::PaletteIndex { index } => {
                write!(f, "palette index {index} cannot be painted")
            }
            Self::EmptyPalette => write!(f, "palette is empty"),
            Self::PaletteTooLarge { len } => {
                write!(f, "palette length {len} exceeds 256")
            }
            Self::TransparentSlot => {
                write!(f, "palette index 0 must be transparent")
            }
            Self::FrameIndex { index, len } => {
                write!(f, "frame {index} is outside 0..{len}")
            }
            Self::LastFrame => write!(f, "cannot delete the only frame"),
        }
    }
}

impl std::error::Error for KernelError {}
