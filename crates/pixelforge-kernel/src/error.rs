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
    /// The palette is locked, so recolor, append, and import are refused.
    /// Paint of an existing index is still allowed.
    PaletteLocked,
    /// Text is not a GIMP palette (`.gpl`) or a hex list.
    PaletteFile,
    /// [`crate::Document::import_palette`] was asked for an unknown mode.
    /// The wasm bridge accepts `replace` and `merge`.
    PaletteImportMode,
    /// Frame index is outside the frame list.
    FrameIndex { index: usize, len: usize },
    /// The document always keeps one frame.
    LastFrame,
    /// Layer index is outside the layer stack.
    LayerIndex { index: usize, len: usize },
    /// The document always keeps one layer.
    LastLayer,
    /// A layer name is empty, only whitespace, longer than
    /// [`crate::MAX_LAYER_NAME_CHARS`], or contains a control character.
    LayerName,
    /// Delay is outside [`crate::MIN_FRAME_DELAY_MS`]..=[`crate::MAX_FRAME_DELAY_MS`].
    FrameDelay { delay_ms: u32 },
    /// [`crate::Document::undo`] found an empty undo stack.
    NothingToUndo,
    /// [`crate::Document::redo`] found an empty redo stack.
    NothingToRedo,
    /// [`crate::Document::begin_gesture`] was called again, or redo ran
    /// while a gesture already had edits.
    OpenGesture,
    /// Export `scale` is 0, or `scale` times the canvas does not fit in a PNG.
    ExportScale { scale: u32 },
    /// Sprite-sheet column count is 0, or that many columns do not fit a PNG.
    SheetColumns { columns: u32 },
    /// Bytes are not a supported `pixelforge-project` file.
    ProjectFile(ProjectFileError),
}

/// Why [`KernelError::ProjectFile`] rejected a project file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectFileError {
    /// JSON is missing, truncated, or not version 1 of this schema.
    Malformed,
    /// `format` is present and is not `pixelforge-project`.
    Format,
    /// `version` is an integer other than 1.
    Version { version: u64 },
    /// Width or height is outside `1..=MAX_CANVAS_EDGE`.
    Size { width: u64, height: u64 },
    /// A palette must contain at least the transparent slot.
    EmptyPalette,
    /// More than 256 entries would not fit in a `u8` index.
    PaletteTooLarge { len: usize },
    /// Entry 0 has to stay transparent so erase has one meaning.
    TransparentSlot,
    /// `frames` is an empty array.
    EmptyFrames,
    /// `activeFrame` is not an index into `frames`.
    ActiveFrame { index: u64, len: usize },
    /// `activeLayer` is not an index into the layer stack.
    ActiveLayer { index: u64, len: usize },
    /// A layer name is empty or longer than [`crate::MAX_LAYER_NAME_CHARS`].
    LayerName { index: usize },
    /// A frame stores both a legacy `pixels` buffer and a `layers` array.
    LayerPixels { frame: usize },
    /// A frame's layer count does not match the document layer stack.
    LayerCount {
        frame: usize,
        expected: usize,
        actual: usize,
    },
    /// One layer buffer's length is not `width * height`.
    LayerLength {
        frame: usize,
        layer: usize,
        expected: usize,
        actual: usize,
    },
    /// A frame's `pixels` length is not `width * height`.
    FrameLength {
        index: usize,
        expected: usize,
        actual: usize,
    },
    /// A pixel index is outside the palette. Index 0 is allowed.
    PixelIndex {
        frame: usize,
        index: u64,
        palette_len: usize,
    },
    /// `delayMs` is outside [`crate::MIN_FRAME_DELAY_MS`]..=[`crate::MAX_FRAME_DELAY_MS`].
    FrameDelay { frame: usize, delay: u64 },
}

impl KernelError {
    /// Stable name shared with the wasm bridge and the TypeScript shell.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidSize { .. } => "InvalidSize",
            Self::OutOfBounds { .. } => "OutOfBounds",
            Self::PaletteIndex { .. } => "PaletteIndex",
            Self::EmptyPalette => "EmptyPalette",
            Self::PaletteTooLarge { .. } => "PaletteTooLarge",
            Self::TransparentSlot => "TransparentSlot",
            Self::PaletteLocked => "PaletteLocked",
            Self::PaletteFile => "PaletteFile",
            Self::PaletteImportMode => "PaletteImportMode",
            Self::FrameIndex { .. } => "FrameIndex",
            Self::LastFrame => "LastFrame",
            Self::LayerIndex { .. } => "LayerIndex",
            Self::LastLayer => "LastLayer",
            Self::LayerName => "LayerName",
            Self::FrameDelay { .. } => "FrameDelay",
            Self::NothingToUndo => "NothingToUndo",
            Self::NothingToRedo => "NothingToRedo",
            Self::OpenGesture => "OpenGesture",
            Self::ExportScale { .. } => "ExportScale",
            Self::SheetColumns { .. } => "SheetColumns",
            Self::ProjectFile(_) => "ProjectFile",
        }
    }
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
            Self::PaletteLocked => write!(f, "palette is locked"),
            Self::PaletteFile => {
                write!(f, "palette file is not a GIMP palette or a hex list")
            }
            Self::PaletteImportMode => {
                write!(f, "palette import mode must be replace or merge")
            }
            Self::FrameIndex { index, len } => {
                write!(f, "frame {index} is outside 0..{len}")
            }
            Self::LastFrame => write!(f, "cannot delete the only frame"),
            Self::LayerIndex { index, len } => {
                write!(f, "layer {index} is outside 0..{len}")
            }
            Self::LastLayer => write!(f, "cannot delete the only layer"),
            Self::LayerName => write!(
                f,
                "layer name must be 1 to {} characters",
                crate::MAX_LAYER_NAME_CHARS
            ),
            Self::FrameDelay { delay_ms } => write!(
                f,
                "frame delay {delay_ms} ms is outside {}..={}",
                crate::MIN_FRAME_DELAY_MS,
                crate::MAX_FRAME_DELAY_MS
            ),
            Self::NothingToUndo => write!(f, "nothing to undo"),
            Self::NothingToRedo => write!(f, "nothing to redo"),
            Self::OpenGesture => write!(f, "a gesture is already open"),
            Self::ExportScale { scale } => write!(
                f,
                "export scale {scale} must be an integer >= 1 that fits the canvas"
            ),
            Self::SheetColumns { columns } => write!(
                f,
                "sprite sheet columns {columns} must be at least 1 and fit the canvas"
            ),
            Self::ProjectFile(error) => write!(f, "{error}"),
        }
    }
}

impl fmt::Display for ProjectFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed => write!(f, "project file is not valid version 1 JSON"),
            Self::Format => write!(f, "project file format is not pixelforge-project"),
            Self::Version { version } => {
                write!(f, "project file version {version} is not supported")
            }
            Self::Size { width, height } => {
                write!(
                    f,
                    "canvas size {width}x{height} is outside 1..={MAX_CANVAS_EDGE}"
                )
            }
            Self::EmptyPalette => write!(f, "palette is empty"),
            Self::PaletteTooLarge { len } => write!(f, "palette length {len} exceeds 256"),
            Self::TransparentSlot => write!(f, "palette index 0 must be transparent"),
            Self::EmptyFrames => write!(f, "project file has no frames"),
            Self::ActiveFrame { index, len } => {
                write!(f, "active frame {index} is outside 0..{len}")
            }
            Self::ActiveLayer { index, len } => {
                write!(f, "active layer {index} is outside 0..{len}")
            }
            Self::LayerName { index } => write!(
                f,
                "layer {index} name must be 1 to {} characters",
                crate::MAX_LAYER_NAME_CHARS
            ),
            Self::LayerPixels { frame } => {
                write!(f, "frame {frame} stores both pixels and layers")
            }
            Self::LayerCount {
                frame,
                expected,
                actual,
            } => write!(f, "frame {frame} has {actual} layers, expected {expected}"),
            Self::LayerLength {
                frame,
                layer,
                expected,
                actual,
            } => write!(
                f,
                "frame {frame} layer {layer} has {actual} pixels, expected {expected}"
            ),
            Self::FrameLength {
                index,
                expected,
                actual,
            } => write!(f, "frame {index} has {actual} pixels, expected {expected}"),
            Self::PixelIndex {
                frame,
                index,
                palette_len,
            } => write!(
                f,
                "frame {frame} uses palette index {index}, palette length is {palette_len}"
            ),
            Self::FrameDelay { frame, delay } => write!(
                f,
                "frame {frame} delay {delay} ms is outside {}..={}",
                crate::MIN_FRAME_DELAY_MS,
                crate::MAX_FRAME_DELAY_MS
            ),
        }
    }
}

impl std::error::Error for KernelError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_match_variant_names() {
        assert_eq!(KernelError::LastFrame.code(), "LastFrame");
        assert_eq!(KernelError::FrameDelay { delay_ms: 0 }.code(), "FrameDelay");
        assert_eq!(KernelError::NothingToUndo.code(), "NothingToUndo");
        assert_eq!(KernelError::OpenGesture.code(), "OpenGesture");
        assert_eq!(KernelError::ExportScale { scale: 0 }.code(), "ExportScale");
        assert_eq!(
            KernelError::SheetColumns { columns: 0 }.code(),
            "SheetColumns"
        );
        assert_eq!(
            KernelError::ExportScale { scale: 0 }.to_string(),
            "export scale 0 must be an integer >= 1 that fits the canvas"
        );
        assert_eq!(
            KernelError::ProjectFile(ProjectFileError::Malformed).code(),
            "ProjectFile"
        );
        assert_eq!(
            KernelError::ProjectFile(ProjectFileError::Version { version: 2 }).to_string(),
            "project file version 2 is not supported"
        );
        assert_eq!(
            KernelError::PaletteIndex { index: 0 }.code(),
            "PaletteIndex"
        );
        assert_eq!(KernelError::PaletteLocked.code(), "PaletteLocked");
        assert_eq!(KernelError::PaletteLocked.to_string(), "palette is locked");
        assert_eq!(KernelError::PaletteFile.code(), "PaletteFile");
        assert_eq!(KernelError::PaletteImportMode.code(), "PaletteImportMode");
        assert_eq!(
            KernelError::InvalidSize {
                width: 0,
                height: 1
            }
            .code(),
            "InvalidSize"
        );
    }
}
