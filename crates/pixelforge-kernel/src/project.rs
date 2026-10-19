//! Version 1 `pixelforge-project` JSON.
//!
//! The kernel owns this format. The shell and, later, the Tool API call
//! [`Document::save_project`] and [`Document::load_project`] instead of
//! writing a second encoder. The file stores canvas size, the palette, every
//! frame's indices and delay, and the active frame. The undo stack and the
//! playback clock are not part of the file, so a loaded document undoes back
//! to the opened pixels and is not playing. A frame written before delays
//! existed omits `delayMs` and loads as [`crate::DEFAULT_FRAME_DELAY_MS`].

use serde::{Deserialize, Serialize};

use crate::document::Document;
use crate::error::{KernelError, ProjectFileError};
use crate::frame::Frame;
use crate::palette::Palette;
use crate::Rgba;
use crate::DEFAULT_FRAME_DELAY_MS;
use crate::MAX_CANVAS_EDGE;
use crate::MAX_FRAME_DELAY_MS;
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
    palette: Vec<ColorFile>,
    frames: Vec<FrameFile>,
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
struct FrameFile {
    pixels: Vec<u64>,
    /// Milliseconds. Missing in files saved before delays existed.
    #[serde(default = "default_delay_ms", rename = "delayMs")]
    delay_ms: u64,
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
            palette: self
                .palette_colors()
                .iter()
                .copied()
                .map(ColorFile::from)
                .collect(),
            frames: (0..self.frame_count())
                .map(|index| FrameFile {
                    pixels: self
                        .frame_pixels(index)
                        .expect("index walks frame_count")
                        .iter()
                        .copied()
                        .map(u64::from)
                        .collect(),
                    delay_ms: u64::from(self.frame_delay(index).expect("index walks frame_count")),
                })
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
    let mut frames = Vec::with_capacity(file.frames.len());
    for (index, frame) in file.frames.into_iter().enumerate() {
        if frame.pixels.len() != expected {
            return Err(invalid(ProjectFileError::FrameLength {
                index,
                expected,
                actual: frame.pixels.len(),
            }));
        }
        let mut pixels = Vec::with_capacity(expected);
        for index_value in frame.pixels {
            if index_value >= u64::try_from(palette.len()).expect("palette length fits in u64") {
                return Err(invalid(ProjectFileError::PixelIndex {
                    frame: index,
                    index: index_value,
                    palette_len: palette.len(),
                }));
            }
            pixels.push(u8::try_from(index_value).expect("index is below the palette length"));
        }
        let delay_ms = frame_delay_from_file(index, frame.delay_ms)?;
        frames.push(Frame::from_pixels(pixels, delay_ms));
    }
    if file.active_frame >= u64::try_from(frames.len()).expect("frame count fits in u64") {
        return Err(invalid(ProjectFileError::ActiveFrame {
            index: file.active_frame,
            len: frames.len(),
        }));
    }
    let active = usize::try_from(file.active_frame).expect("active frame was checked");
    Ok(Document::from_stored(
        width, height, palette, frames, active,
    ))
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
