//! Browser bridge for [`pixelforge_kernel::Document`].
//!
//! The TypeScript shell loads this crate as WebAssembly and keeps using
//! `snapshot()`, `paint`, `erase`, and the frame-list methods. Pixel rules
//! stay in the kernel. This module only copies bytes across the boundary
//! and turns [`pixelforge_kernel::KernelError`] into a `Code: message` string
//! the shell maps back onto its `KernelError`.

use pixelforge_kernel::{Document, KernelError};
use wasm_bindgen::prelude::*;

/// Indexed document exposed to JavaScript.
///
/// Each `pixels` / `paletteRgba` call copies bytes into a new `Uint8Array`.
/// The copy is the shell's snapshot. It is not a live view of Wasm memory.
#[derive(Debug)]
#[wasm_bindgen]
pub struct PixelDocument {
    inner: Document,
}

#[wasm_bindgen]
impl PixelDocument {
    /// Blank document filled with transparent indices and the studio palette.
    #[wasm_bindgen(constructor)]
    pub fn new(width: u16, height: u16) -> Result<PixelDocument, String> {
        Document::new(width, height)
            .map(|inner| Self { inner })
            .map_err(reject)
    }

    pub fn width(&self) -> u16 {
        self.inner.width()
    }

    pub fn height(&self) -> u16 {
        self.inner.height()
    }

    /// Palette entries packed as repeating `r, g, b, a` bytes.
    #[wasm_bindgen(js_name = paletteRgba)]
    pub fn palette_rgba(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.inner.palette_colors().len() * 4);
        for color in self.inner.palette_colors() {
            bytes.extend([color.r, color.g, color.b, color.a]);
        }
        bytes
    }

    /// Palette indices for the active frame, row-major.
    pub fn pixels(&self) -> Vec<u8> {
        self.inner.pixels().to_vec()
    }

    #[wasm_bindgen(js_name = frameCount)]
    pub fn frame_count(&self) -> u32 {
        as_u32(self.inner.frame_count())
    }

    #[wasm_bindgen(js_name = activeFrame)]
    pub fn active_frame(&self) -> u32 {
        as_u32(self.inner.active_frame())
    }

    pub fn paint(&mut self, x: u16, y: u16, color_index: u8) -> Result<(), String> {
        self.inner.paint(x, y, color_index).map_err(reject)
    }

    pub fn erase(&mut self, x: u16, y: u16) -> Result<(), String> {
        self.inner.erase(x, y).map_err(reject)
    }

    /// Appends a blank frame and returns its index. The active frame stays put.
    #[wasm_bindgen(js_name = addFrame)]
    pub fn add_frame(&mut self) -> u32 {
        as_u32(self.inner.add_frame())
    }

    /// Inserts a copy after `index` and returns the new index.
    #[wasm_bindgen(js_name = duplicateFrame)]
    pub fn duplicate_frame(&mut self, index: u32) -> Result<u32, String> {
        self.inner
            .duplicate_frame(as_index(index))
            .map(as_u32)
            .map_err(reject)
    }

    /// Removes a frame. The last remaining frame cannot be deleted.
    #[wasm_bindgen(js_name = deleteFrame)]
    pub fn delete_frame(&mut self, index: u32) -> Result<(), String> {
        self.inner.delete_frame(as_index(index)).map_err(reject)
    }

    #[wasm_bindgen(js_name = setActiveFrame)]
    pub fn set_active_frame(&mut self, index: u32) -> Result<(), String> {
        self.inner.set_active_frame(as_index(index)).map_err(reject)
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
fn install_panic_hook() {
    console_error_panic_hook::set_once();
}

fn reject(err: KernelError) -> String {
    format!("{}: {err}", err.code())
}

fn as_index(index: u32) -> usize {
    index as usize
}

fn as_u32(index: usize) -> u32 {
    u32::try_from(index).expect("frame index fits in u32")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> PixelDocument {
        PixelDocument::new(16, 16).unwrap()
    }

    #[test]
    fn palette_and_blank_pixels_come_from_the_kernel() {
        let document = doc();
        assert_eq!(document.width(), 16);
        assert_eq!(document.height(), 16);
        assert_eq!(document.frame_count(), 1);
        assert_eq!(document.active_frame(), 0);
        assert_eq!(document.pixels(), vec![0; 16 * 16]);

        let rgba = document.palette_rgba();
        assert_eq!(rgba.len(), 9 * 4);
        assert_eq!(&rgba[..4], &[0, 0, 0, 0]);
        assert_eq!(&rgba[4..8], &[26, 28, 36, 255]);
    }

    #[test]
    fn frames_keep_separate_pixels() {
        let mut document = doc();
        document.paint(1, 1, 2).unwrap();
        assert_eq!(document.add_frame(), 1);
        assert_eq!(document.active_frame(), 0);

        document.set_active_frame(1).unwrap();
        assert!(document.pixels().iter().all(|pixel| *pixel == 0));
        document.paint(1, 1, 3).unwrap();

        document.set_active_frame(0).unwrap();
        assert_eq!(document.pixels()[1 + 16], 2);

        document.set_active_frame(1).unwrap();
        assert_eq!(document.pixels()[1 + 16], 3);

        let copy = document.duplicate_frame(0).unwrap();
        assert_eq!(copy, 1);
        assert_eq!(document.frame_count(), 3);
        assert_eq!(document.active_frame(), 2);
        assert_eq!(document.pixels()[1 + 16], 3);

        document.delete_frame(0).unwrap();
        assert_eq!(document.frame_count(), 2);
        assert_eq!(document.active_frame(), 1);

        document.delete_frame(0).unwrap();
        assert_eq!(document.frame_count(), 1);
        assert_eq!(
            document.delete_frame(0).unwrap_err(),
            "LastFrame: cannot delete the only frame"
        );
        assert_eq!(document.frame_count(), 1);
    }

    #[test]
    fn rejects_bad_size_index_and_cell() {
        assert_eq!(
            PixelDocument::new(0, 16).unwrap_err(),
            "InvalidSize: canvas size 0x16 is outside 1..=256"
        );
        let mut document = doc();
        document.paint(0, 0, 1).unwrap();
        assert_eq!(
            document.paint(0, 0, 0).unwrap_err(),
            "PaletteIndex: palette index 0 cannot be painted"
        );
        assert_eq!(document.pixels()[0], 1);
        assert!(document
            .erase(16, 0)
            .unwrap_err()
            .starts_with("OutOfBounds:"));
        assert!(document
            .set_active_frame(1)
            .unwrap_err()
            .starts_with("FrameIndex:"));
        assert!(document
            .duplicate_frame(2)
            .unwrap_err()
            .starts_with("FrameIndex:"));
    }
}
