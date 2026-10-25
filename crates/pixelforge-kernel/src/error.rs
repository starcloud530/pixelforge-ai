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
    /// Quantize or PNG crush needs an opaque palette entry and none exists.
    NoPaintColor,
    /// Bytes are not a decodable PNG, or the expanded image is not 8-bit RGBA.
    PngFile,
    /// A PNG's pixel size is not the canvas size. Crush does not scale.
    PngSize {
        png_width: u32,
        png_height: u32,
        width: u16,
        height: u16,
    },
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
    /// A tag name is empty, only whitespace, longer than
    /// [`crate::MAX_TAG_NAME_CHARS`], or contains a control character.
    TagName,
    /// A tag already uses this name.
    TagExists,
    /// No tag uses this name.
    TagMissing,
    /// `from` is past `to`, or the range is outside the frame list.
    ///
    /// `from` and `to` are inclusive. `len` is the frame count.
    TagRange { from: usize, to: usize, len: usize },
    /// A part name is empty, only whitespace, longer than
    /// [`crate::MAX_PART_NAME_CHARS`], or contains a control character.
    PartName,
    /// A part already uses this name.
    PartExists,
    /// No part uses this name.
    PartMissing,
    /// An anchor name is empty, only whitespace, longer than
    /// [`crate::MAX_PART_NAME_CHARS`], or contains a control character.
    AnchorName,
    /// This part has no anchor with this name.
    AnchorMissing,
    /// An anchor's local `x` or `y` is outside `-MAX_CANVAS_EDGE..=MAX_CANVAS_EDGE`.
    AnchorCoord { x: i32, y: i32 },
    /// A part angle is not finite, or its absolute value is above 1_000_000.
    PartAngle,
    /// A part transform would move the origin or an ink cell off the canvas.
    ///
    /// `x` and `y` are the cell that would not fit. The call writes nothing.
    PartOffCanvas { x: i32, y: i32 },
    /// A move would write or clear a cell outside the rectangular selection.
    ///
    /// The call writes nothing. Fill, paint, erase, copy, and flip skip those
    /// cells instead of failing.
    SelectionProtected,
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
    /// Bytes are not a readable indexed `.aseprite` file in the supported subset.
    AsepriteFile { reason: &'static str },
    /// The file's canvas is outside `1..=MAX_CANVAS_EDGE`.
    AsepriteSize { width: u16, height: u16 },
    /// Color depth is not indexed 8-bit.
    AsepriteColorMode { depth: u16 },
    /// The indexed palette is missing, external, or does not cover the transparent index.
    AsepritePalette,
    /// A tilemap layer or tilemap cel is present. Those pixels are not imported.
    AsepriteTilemap,
    /// A cel index does not fit the palette.
    AsepritePixel { index: u8, palette_len: usize },
    /// Groups and reference layers were skipped and no image layer remained.
    AsepriteLayers,
    /// 90° or 270° rotation was asked of a rectangle that is not square.
    ///
    /// The canvas size stays. The call writes nothing.
    RotateSquare { width: u16, height: u16 },
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
    /// `indexPolicy` is present and is not `reject` or `clamp`.
    IndexPolicy,
    /// `selection` is present and is not a rectangle that lies on the canvas.
    Selection,
    /// A tag name is empty, too long, or contains a control character.
    TagName { index: usize },
    /// A tag's inclusive range is outside the frame list, or `from` is past `to`.
    TagRange { index: usize },
    /// A tag's `loop` is not `forward`, `reverse`, `pingpong`, or `once`.
    TagLoop { index: usize },
    /// Two tags in the file share a name. `index` is the later one.
    TagDuplicate { index: usize },
    /// A part name is empty, too long, or contains a control character.
    PartName { index: usize },
    /// Two parts in the file share a name. `index` is the later one.
    PartDuplicate { index: usize },
    /// A part origin is not a cell on the canvas.
    PartOrigin { index: usize },
    /// A part's `layer` is not an index into the layer stack.
    PartLayer { index: usize },
    /// An anchor name is empty, too long, or contains a control character.
    AnchorName { part: usize, index: usize },
    /// Two anchors on one part share a name. `index` is the later one.
    AnchorDuplicate { part: usize, index: usize },
    /// An anchor's local coordinate is outside `-MAX_CANVAS_EDGE..=MAX_CANVAS_EDGE`.
    AnchorCoord { part: usize, index: usize },
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
            Self::NoPaintColor => "NoPaintColor",
            Self::PngFile => "PngFile",
            Self::PngSize { .. } => "PngSize",
            Self::FrameIndex { .. } => "FrameIndex",
            Self::LastFrame => "LastFrame",
            Self::LayerIndex { .. } => "LayerIndex",
            Self::LastLayer => "LastLayer",
            Self::LayerName => "LayerName",
            Self::FrameDelay { .. } => "FrameDelay",
            Self::TagName => "TagName",
            Self::TagExists => "TagExists",
            Self::TagMissing => "TagMissing",
            Self::TagRange { .. } => "TagRange",
            Self::PartName => "PartName",
            Self::PartExists => "PartExists",
            Self::PartMissing => "PartMissing",
            Self::AnchorName => "AnchorName",
            Self::AnchorMissing => "AnchorMissing",
            Self::AnchorCoord { .. } => "AnchorCoord",
            Self::PartAngle => "PartAngle",
            Self::PartOffCanvas { .. } => "PartOffCanvas",
            Self::NothingToUndo => "NothingToUndo",
            Self::NothingToRedo => "NothingToRedo",
            Self::OpenGesture => "OpenGesture",
            Self::SelectionProtected => "SelectionProtected",
            Self::ExportScale { .. } => "ExportScale",
            Self::SheetColumns { .. } => "SheetColumns",
            Self::AsepriteFile { .. } => "AsepriteFile",
            Self::AsepriteSize { .. } => "AsepriteSize",
            Self::AsepriteColorMode { .. } => "AsepriteColorMode",
            Self::AsepritePalette => "AsepritePalette",
            Self::AsepriteTilemap => "AsepriteTilemap",
            Self::AsepritePixel { .. } => "AsepritePixel",
            Self::AsepriteLayers => "AsepriteLayers",
            Self::RotateSquare { .. } => "RotateSquare",
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
            Self::NoPaintColor => write!(f, "palette has no opaque color"),
            Self::PngFile => write!(f, "png is not a decodable image"),
            Self::PngSize {
                png_width,
                png_height,
                width,
                height,
            } => write!(
                f,
                "png is {png_width}x{png_height}, canvas is {width}x{height}"
            ),
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
            Self::TagName => write!(
                f,
                "tag name must be 1 to {} characters",
                crate::MAX_TAG_NAME_CHARS
            ),
            Self::TagExists => write!(f, "tag name is already used"),
            Self::TagMissing => write!(f, "tag name is not on this document"),
            Self::TagRange { from, to, len } => {
                write!(f, "tag frames {from}..={to} must lie inside 0..{len}")
            }
            Self::PartName => write!(
                f,
                "part name must be 1 to {} characters",
                crate::MAX_PART_NAME_CHARS
            ),
            Self::PartExists => write!(f, "part name is already used"),
            Self::PartMissing => write!(f, "part name is not on this document"),
            Self::AnchorName => write!(
                f,
                "anchor name must be 1 to {} characters",
                crate::MAX_PART_NAME_CHARS
            ),
            Self::AnchorMissing => write!(f, "anchor name is not on this part"),
            Self::AnchorCoord { x, y } => write!(
                f,
                "anchor {x},{y} is outside -{MAX_CANVAS_EDGE}..={MAX_CANVAS_EDGE}"
            ),
            Self::PartAngle => write!(
                f,
                "part angle must be a finite number of degrees from -1000000 to 1000000"
            ),
            Self::PartOffCanvas { x, y } => {
                write!(
                    f,
                    "part transform would place a cell at {x},{y} outside the canvas"
                )
            }
            Self::NothingToUndo => write!(f, "nothing to undo"),
            Self::NothingToRedo => write!(f, "nothing to redo"),
            Self::OpenGesture => write!(f, "a gesture is already open"),
            Self::SelectionProtected => {
                write!(f, "selection protects pixels outside the rectangle")
            }
            Self::ExportScale { scale } => write!(
                f,
                "export scale {scale} must be an integer >= 1 that fits the canvas"
            ),
            Self::SheetColumns { columns } => write!(
                f,
                "sprite sheet columns {columns} must be at least 1 and fit the canvas"
            ),
            Self::AsepriteFile { reason } => write!(f, "aseprite: {reason}"),
            Self::AsepriteSize { width, height } => write!(
                f,
                "aseprite canvas {width}x{height} is outside 1..={MAX_CANVAS_EDGE}"
            ),
            Self::AsepriteColorMode { depth } => {
                write!(f, "aseprite color depth {depth} is not indexed 8-bit")
            }
            Self::AsepritePalette => {
                write!(f, "aseprite file has no usable indexed palette")
            }
            Self::AsepriteTilemap => write!(f, "aseprite tilemaps are not imported"),
            Self::AsepritePixel { index, palette_len } => write!(
                f,
                "aseprite pixel index {index} is outside palette length {palette_len}"
            ),
            Self::AsepriteLayers => write!(f, "aseprite file has no image layer to import"),
            Self::RotateSquare { width, height } => write!(
                f,
                "90 and 270 degree rotation needs a square rectangle, got {width}x{height}"
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
            Self::IndexPolicy => write!(f, "indexPolicy must be reject or clamp"),
            Self::Selection => write!(f, "selection is not a rectangle on the canvas"),
            Self::TagName { index } => write!(
                f,
                "tag {index} name must be 1 to {} characters",
                crate::MAX_TAG_NAME_CHARS
            ),
            Self::TagRange { index } => {
                write!(f, "tag {index} frames must lie inside the frame list")
            }
            Self::TagLoop { index } => write!(
                f,
                "tag {index} loop must be forward, reverse, pingpong, or once"
            ),
            Self::TagDuplicate { index } => write!(f, "tag {index} name is already used"),
            Self::PartName { index } => write!(
                f,
                "part {index} name must be 1 to {} characters",
                crate::MAX_PART_NAME_CHARS
            ),
            Self::PartDuplicate { index } => write!(f, "part {index} name is already used"),
            Self::PartOrigin { index } => {
                write!(f, "part {index} origin is not a cell on the canvas")
            }
            Self::PartLayer { index } => {
                write!(f, "part {index} layer is outside the layer stack")
            }
            Self::AnchorName { part, index } => write!(
                f,
                "part {part} anchor {index} name must be 1 to {} characters",
                crate::MAX_PART_NAME_CHARS
            ),
            Self::AnchorDuplicate { part, index } => {
                write!(f, "part {part} anchor {index} name is already used")
            }
            Self::AnchorCoord { part, index } => write!(
                f,
                "part {part} anchor {index} is outside -{MAX_CANVAS_EDGE}..={MAX_CANVAS_EDGE}"
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
        assert_eq!(KernelError::TagName.code(), "TagName");
        assert_eq!(KernelError::TagExists.code(), "TagExists");
        assert_eq!(
            KernelError::TagMissing.to_string(),
            "tag name is not on this document"
        );
        assert_eq!(
            KernelError::TagRange {
                from: 2,
                to: 1,
                len: 1
            }
            .to_string(),
            "tag frames 2..=1 must lie inside 0..1"
        );
        assert_eq!(
            KernelError::ProjectFile(ProjectFileError::TagLoop { index: 0 }).to_string(),
            "tag 0 loop must be forward, reverse, pingpong, or once"
        );
        assert_eq!(KernelError::PartMissing.code(), "PartMissing");
        assert_eq!(KernelError::PartAngle.code(), "PartAngle");
        assert_eq!(
            KernelError::PartOffCanvas { x: -1, y: 2 }.to_string(),
            "part transform would place a cell at -1,2 outside the canvas"
        );
        assert_eq!(
            KernelError::AnchorCoord { x: 300, y: 0 }.to_string(),
            "anchor 300,0 is outside -256..=256"
        );
        assert_eq!(
            KernelError::ProjectFile(ProjectFileError::PartOrigin { index: 1 }).to_string(),
            "part 1 origin is not a cell on the canvas"
        );
        assert_eq!(KernelError::NothingToUndo.code(), "NothingToUndo");
        assert_eq!(KernelError::OpenGesture.code(), "OpenGesture");
        assert_eq!(KernelError::SelectionProtected.code(), "SelectionProtected");
        assert_eq!(
            KernelError::SelectionProtected.to_string(),
            "selection protects pixels outside the rectangle"
        );
        assert_eq!(
            KernelError::ProjectFile(ProjectFileError::Selection).to_string(),
            "selection is not a rectangle on the canvas"
        );
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
