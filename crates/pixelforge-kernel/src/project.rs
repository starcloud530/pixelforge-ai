//! Version 1 `pixelforge-project` JSON.
//!
//! The kernel owns this format. The shell and, later, the Tool API call
//! [`Document::save_project`] and [`Document::load_project`] instead of
//! writing a second encoder. The file stores canvas size, the palette, the
//! palette lock, every frame's indices and delay, the active frame, and, when
//! the document is more than the original single layer, the layer stack.
//! The undo stack and the playback clock are not part of the file, so a loaded
//! document undoes back to the opened pixels and is not playing. A frame
//! written before delays existed omits `delayMs` and loads as
//! [`crate::DEFAULT_FRAME_DELAY_MS`]. A file written before the palette lock
//! existed omits `paletteLocked` and loads unlocked. A missing `indexPolicy`
//! loads as `reject`; only `clamp` is written back. A file written before
//! layers existed omits `layers` and `activeLayer` and loads as one layer
//! named [`crate::DEFAULT_LAYER_NAME`]. A single default layer is still saved
//! in that older shape. A missing `selection` loads as no mask. The rectangle
//! is written only while one is set. The undo stack is still omitted.

use serde::{Deserialize, Serialize};

use crate::document::Document;
use crate::error::{KernelError, ProjectFileError};
use crate::frame::Frame;
use crate::palette::{IndexPolicy, Palette};
use crate::selection::Selection;
use crate::Rgba;
use crate::DEFAULT_FRAME_DELAY_MS;
use crate::DEFAULT_LAYER_NAME;
use crate::MAX_CANVAS_EDGE;
use crate::MAX_FRAME_DELAY_MS;
use crate::MAX_LAYER_NAME_CHARS;
use crate::MIN_FRAME_DELAY_MS;

/// `format` field written by [`Document::save_project`].
pub const PROJECT_FORMAT: &str = "pixelforge-project";

/// Only version [`Document::load_project`] accepts.
///
/// Per-frame `delayMs` was added without a version bump. Files that omit it
/// still load: each missing delay becomes [`crate::DEFAULT_FRAME_DELAY_MS`].
pub const PROJECT_VERSION: u32 = 1;

fn default_delay_ms() -> u64 {
    u64::from(DEFAULT_FRAME_DELAY_MS)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectFile {
    format: String,
    version: u64,
    width: u64,
    height: u64,
    active_frame: u64,
    /// Missing, or 0, in a file that still has one default layer.
    #[serde(default, skip_serializing_if = "is_zero")]
    active_layer: u64,
    /// Missing when the file is still the single default layer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    layers: Vec<LayerNameFile>,
    palette: Vec<ColorFile>,
    /// Missing in files saved before the palette could be locked.
    #[serde(default)]
    palette_locked: bool,
    /// Missing, or `reject`, unless paint should clamp out-of-range indices.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    index_policy: Option<String>,
    /// Missing when every cell is writable. One rectangle in canvas cells.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    selection: Option<SelectionFile>,
    frames: Vec<FrameFile>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionFile {
    x: u64,
    y: u64,
    width: u64,
    height: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ColorFile {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LayerNameFile {
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LayerPixelsFile {
    pixels: Vec<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FrameFile {
    /// The only layer, or omitted when [`Self::layers`] holds the stack.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pixels: Vec<u64>,
    /// Milliseconds. Missing in files saved before delays existed.
    #[serde(default = "default_delay_ms", rename = "delayMs")]
    delay_ms: u64,
    /// One buffer per document layer, bottom to top. Omitted for one layer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    layers: Vec<LayerPixelsFile>,
}

fn is_zero(value: &u64) -> bool {
    *value == 0
}

impl Document {
    /// Pretty-printed version 1 project JSON, including a trailing newline.
    ///
    /// An open gesture is saved as the pixels already on the document. Undo
    /// and redo are omitted.
    pub fn save_project(&self) -> String {
        let file = ProjectFile {
            format: PROJECT_FORMAT.to_string(),
            version: u64::from(PROJECT_VERSION),
            width: u64::from(self.width()),
            height: u64::from(self.height()),
            active_frame: u64::try_from(self.active_frame()).expect("frame index fits in u64"),
            active_layer: u64::try_from(self.active_layer()).expect("layer index fits in u64"),
            layers: self.stored_layer_names(),
            palette: self
                .palette_colors()
                .iter()
                .copied()
                .map(ColorFile::from)
                .collect(),
            palette_locked: self.palette_locked(),
            index_policy: match self.index_policy() {
                IndexPolicy::Reject => None,
                IndexPolicy::Clamp => Some("clamp".to_string()),
            },
            selection: self.selection().map(SelectionFile::from),
            frames: (0..self.frame_count())
                .map(|index| self.stored_frame(index))
                .collect(),
        };
        let mut json = serde_json::to_string_pretty(&file)
            .expect("project fields are JSON numbers, strings, and arrays");
        json.push('\n');
        json
    }

    /// Reads version 1 project JSON. The undo stack of the result is empty.
    pub fn load_project(json: &str) -> Result<Self, KernelError> {
        let json = json.strip_prefix('\u{feff}').unwrap_or(json);
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|_| invalid(ProjectFileError::Malformed))?;
        let Some(object) = value.as_object() else {
            return Err(invalid(ProjectFileError::Malformed));
        };
        match object.get("format") {
            Some(serde_json::Value::String(name)) if name == PROJECT_FORMAT => {}
            Some(_) => return Err(invalid(ProjectFileError::Format)),
            None => return Err(invalid(ProjectFileError::Malformed)),
        }
        match object.get("version") {
            Some(serde_json::Value::Number(number)) => match number.as_u64() {
                Some(version) if version == u64::from(PROJECT_VERSION) => {}
                Some(version) => return Err(invalid(ProjectFileError::Version { version })),
                None => return Err(invalid(ProjectFileError::Malformed)),
            },
            _ => return Err(invalid(ProjectFileError::Malformed)),
        }

        let file: ProjectFile =
            serde_json::from_value(value).map_err(|_| invalid(ProjectFileError::Malformed))?;
        document_from_file(file)
    }
}

fn document_from_file(file: ProjectFile) -> Result<Document, KernelError> {
    let (width, height) = canvas_size(file.width, file.height)?;
    let palette = palette_from_file(file.palette)?;
    if file.frames.is_empty() {
        return Err(invalid(ProjectFileError::EmptyFrames));
    }
    let expected = usize::from(width) * usize::from(height);
    let layer_names = layer_names_from_file(file.layers)?;
    let mut frames = Vec::with_capacity(file.frames.len());
    for (index, frame) in file.frames.into_iter().enumerate() {
        let buffers =
            layer_buffers_from_file(index, frame.pixels, frame.layers, expected, &palette)?;
        if layer_names.is_empty() {
            if buffers.len() != 1 {
                return Err(invalid(ProjectFileError::LayerCount {
                    frame: index,
                    expected: 1,
                    actual: buffers.len(),
                }));
            }
        } else if buffers.len() != layer_names.len() {
            return Err(invalid(ProjectFileError::LayerCount {
                frame: index,
                expected: layer_names.len(),
                actual: buffers.len(),
            }));
        }
        let delay_ms = frame_delay_from_file(index, frame.delay_ms)?;
        frames.push(Frame::from_layers(buffers, delay_ms));
    }
    if file.active_frame >= u64::try_from(frames.len()).expect("frame count fits in u64") {
        return Err(invalid(ProjectFileError::ActiveFrame {
            index: file.active_frame,
            len: frames.len(),
        }));
    }
    let active = usize::try_from(file.active_frame).expect("active frame was checked");
    let names = if layer_names.is_empty() {
        vec![DEFAULT_LAYER_NAME.to_string()]
    } else {
        layer_names
    };
    if file.active_layer >= u64::try_from(names.len()).expect("layer count fits in u64") {
        return Err(invalid(ProjectFileError::ActiveLayer {
            index: file.active_layer,
            len: names.len(),
        }));
    }
    let active_layer = usize::try_from(file.active_layer).expect("active layer was checked");
    let index_policy = index_policy_from_file(file.index_policy)?;
    let selection = selection_from_file(file.selection, width, height)?;
    Ok(Document::from_stored(
        width,
        height,
        palette,
        file.palette_locked,
        index_policy,
        frames,
        active,
        names,
        active_layer,
        selection,
    ))
}

fn selection_from_file(
    value: Option<SelectionFile>,
    width: u16,
    height: u16,
) -> Result<Option<Selection>, KernelError> {
    let Some(file) = value else {
        return Ok(None);
    };
    let selection = Selection {
        x: u16::try_from(file.x).map_err(|_| invalid(ProjectFileError::Selection))?,
        y: u16::try_from(file.y).map_err(|_| invalid(ProjectFileError::Selection))?,
        width: u16::try_from(file.width).map_err(|_| invalid(ProjectFileError::Selection))?,
        height: u16::try_from(file.height).map_err(|_| invalid(ProjectFileError::Selection))?,
    };
    if selection.fits(width, height) {
        Ok(Some(selection))
    } else {
        Err(invalid(ProjectFileError::Selection))
    }
}

fn index_policy_from_file(value: Option<String>) -> Result<IndexPolicy, KernelError> {
    match value.as_deref() {
        None | Some("reject") => Ok(IndexPolicy::Reject),
        Some("clamp") => Ok(IndexPolicy::Clamp),
        Some(_) => Err(invalid(ProjectFileError::IndexPolicy)),
    }
}

impl Document {
    fn stored_layer_names(&self) -> Vec<LayerNameFile> {
        let legacy = self.layer_count() == 1
            && self.layer_name(0).expect("a document has a layer") == DEFAULT_LAYER_NAME
            && self.active_layer() == 0;
        if legacy {
            Vec::new()
        } else {
            self.layer_names()
                .iter()
                .map(|name| LayerNameFile { name: name.clone() })
                .collect()
        }
    }

    fn stored_frame(&self, index: usize) -> FrameFile {
        let delay_ms = u64::from(self.frame_delay(index).expect("index walks frame_count"));
        if self.layer_count() == 1 {
            FrameFile {
                pixels: indices_to_file(
                    self.layer_pixels(index, 0)
                        .expect("index walks frame_count"),
                ),
                delay_ms,
                layers: Vec::new(),
            }
        } else {
            FrameFile {
                pixels: Vec::new(),
                delay_ms,
                layers: (0..self.layer_count())
                    .map(|layer| LayerPixelsFile {
                        pixels: indices_to_file(
                            self.layer_pixels(index, layer)
                                .expect("layer walks layer_count"),
                        ),
                    })
                    .collect(),
            }
        }
    }
}

fn indices_to_file(pixels: &[u8]) -> Vec<u64> {
    pixels.iter().copied().map(u64::from).collect()
}

fn layer_names_from_file(layers: Vec<LayerNameFile>) -> Result<Vec<String>, KernelError> {
    let mut names = Vec::with_capacity(layers.len());
    for (index, layer) in layers.into_iter().enumerate() {
        names.push(layer_name_from_file(index, &layer.name)?);
    }
    Ok(names)
}

fn layer_name_from_file(index: usize, name: &str) -> Result<String, KernelError> {
    let trimmed = name.trim();
    let chars = trimmed.chars().count();
    if trimmed.is_empty() || chars > MAX_LAYER_NAME_CHARS || trimmed.chars().any(char::is_control) {
        Err(invalid(ProjectFileError::LayerName { index }))
    } else {
        Ok(trimmed.to_string())
    }
}

fn layer_buffers_from_file(
    frame: usize,
    pixels: Vec<u64>,
    layers: Vec<LayerPixelsFile>,
    expected: usize,
    palette: &Palette,
) -> Result<Vec<Vec<u8>>, KernelError> {
    if !pixels.is_empty() && !layers.is_empty() {
        return Err(invalid(ProjectFileError::LayerPixels { frame }));
    }
    if !layers.is_empty() {
        let mut buffers = Vec::with_capacity(layers.len());
        for (layer, buffer) in layers.into_iter().enumerate() {
            if buffer.pixels.len() != expected {
                return Err(invalid(ProjectFileError::LayerLength {
                    frame,
                    layer,
                    expected,
                    actual: buffer.pixels.len(),
                }));
            }
            buffers.push(decode_indices(frame, buffer.pixels, palette)?);
        }
        return Ok(buffers);
    }
    if pixels.len() != expected {
        return Err(invalid(ProjectFileError::FrameLength {
            index: frame,
            expected,
            actual: pixels.len(),
        }));
    }
    Ok(vec![decode_indices(frame, pixels, palette)?])
}

fn decode_indices(
    frame: usize,
    pixels: Vec<u64>,
    palette: &Palette,
) -> Result<Vec<u8>, KernelError> {
    let mut decoded = Vec::with_capacity(pixels.len());
    for index_value in pixels {
        if index_value >= u64::try_from(palette.len()).expect("palette length fits in u64") {
            return Err(invalid(ProjectFileError::PixelIndex {
                frame,
                index: index_value,
                palette_len: palette.len(),
            }));
        }
        decoded.push(u8::try_from(index_value).expect("index is below the palette length"));
    }
    Ok(decoded)
}

fn frame_delay_from_file(index: usize, delay: u64) -> Result<u32, KernelError> {
    if (u64::from(MIN_FRAME_DELAY_MS)..=u64::from(MAX_FRAME_DELAY_MS)).contains(&delay) {
        Ok(u32::try_from(delay).expect("delay range fits in u32"))
    } else {
        Err(invalid(ProjectFileError::FrameDelay {
            frame: index,
            delay,
        }))
    }
}

fn canvas_size(width: u64, height: u64) -> Result<(u16, u16), KernelError> {
    let width_u16 = u16::try_from(width);
    let height_u16 = u16::try_from(height);
    match (width_u16, height_u16) {
        (Ok(width), Ok(height))
            if width > 0 && height > 0 && width <= MAX_CANVAS_EDGE && height <= MAX_CANVAS_EDGE =>
        {
            Ok((width, height))
        }
        _ => Err(invalid(ProjectFileError::Size { width, height })),
    }
}

fn palette_from_file(colors: Vec<ColorFile>) -> Result<Palette, KernelError> {
    let colors = colors.into_iter().map(Rgba::from).collect();
    Palette::new(colors).map_err(|error| match error {
        KernelError::EmptyPalette => invalid(ProjectFileError::EmptyPalette),
        KernelError::PaletteTooLarge { len } => invalid(ProjectFileError::PaletteTooLarge { len }),
        KernelError::TransparentSlot => invalid(ProjectFileError::TransparentSlot),
        other => other,
    })
}

fn invalid(error: ProjectFileError) -> KernelError {
    KernelError::ProjectFile(error)
}

impl From<Selection> for SelectionFile {
    fn from(selection: Selection) -> Self {
        Self {
            x: u64::from(selection.x),
            y: u64::from(selection.y),
            width: u64::from(selection.width),
            height: u64::from(selection.height),
        }
    }
}

impl From<Rgba> for ColorFile {
    fn from(color: Rgba) -> Self {
        Self {
            r: color.r,
            g: color.g,
            b: color.b,
            a: color.a,
        }
    }
}

impl From<ColorFile> for Rgba {
    fn from(color: ColorFile) -> Self {
        Self::new(color.r, color.g, color.b, color.a)
    }
}
