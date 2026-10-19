//! Browser bridge for [`pixelforge_kernel::Document`].
//!
//! The TypeScript shell loads this crate as WebAssembly and keeps using
//! `snapshot()`, `paint`, `erase`, palette edits, frame-list methods, per-frame
//! delay, timeline scrub, loop playback, undo / redo, and project save / load.
//! Pixel rules, delays, the playback clock, the undo stack, and the project
//! file stay in the kernel. This module only copies bytes across the boundary
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

    /// Replaces the RGB of a paint entry. Index 0 cannot be changed.
    /// Cells keep their indices, so the new color shows everywhere that entry is used.
    #[wasm_bindgen(js_name = setPaletteColor)]
    pub fn set_palette_color(&mut self, index: u8, r: u8, g: u8, b: u8) -> Result<(), String> {
        self.inner.set_palette_color(index, r, g, b).map_err(reject)
    }

    /// Appends an opaque color and returns its index.
    #[wasm_bindgen(js_name = addPaletteColor)]
    pub fn add_palette_color(&mut self, r: u8, g: u8, b: u8) -> Result<u8, String> {
        self.inner.add_palette_color(r, g, b).map_err(reject)
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

    /// Milliseconds this frame stays up during loop playback.
    #[wasm_bindgen(js_name = frameDelay)]
    pub fn frame_delay(&self, index: u32) -> Result<u32, String> {
        self.inner.frame_delay(as_index(index)).map_err(reject)
    }

    /// Delay of every frame, in timeline order.
    #[wasm_bindgen(js_name = frameDelays)]
    pub fn frame_delays(&self) -> Vec<u32> {
        self.inner.frame_delays()
    }

    /// Sets one frame's delay in milliseconds. Same value is not recorded.
    #[wasm_bindgen(js_name = setFrameDelay)]
    pub fn set_frame_delay(&mut self, index: u32, delay_ms: u32) -> Result<(), String> {
        self.inner
            .set_frame_delay(as_index(index), delay_ms)
            .map_err(reject)
    }

    /// Starts loop playback at the beginning of the active frame.
    pub fn play(&mut self) {
        self.inner.play();
    }

    /// Stops loop playback.
    pub fn stop(&mut self) {
        self.inner.stop();
    }

    /// True while [`Self::tick`] is advancing frames.
    pub fn playing(&self) -> bool {
        self.inner.playing()
    }

    /// Advances the preview clock. Returns whether the active frame changed.
    /// The change is not an undo entry.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        self.inner.tick(delta_ms)
    }

    /// Stops playback and selects the frame under `ratio_ppm` (0..=1_000_000).
    /// A drag should be one gesture so the selection is one undo step.
    pub fn scrub(&mut self, ratio_ppm: u32) -> Result<(), String> {
        self.inner.scrub(ratio_ppm).map_err(reject)
    }

    #[wasm_bindgen(js_name = beginGesture)]
    pub fn begin_gesture(&mut self) -> Result<(), String> {
        self.inner.begin_gesture().map_err(reject)
    }

    #[wasm_bindgen(js_name = endGesture)]
    pub fn end_gesture(&mut self) {
        self.inner.end_gesture();
    }

    #[wasm_bindgen(js_name = gestureOpen)]
    pub fn gesture_open(&self) -> bool {
        self.inner.gesture_open()
    }

    #[wasm_bindgen(js_name = canUndo)]
    pub fn can_undo(&self) -> bool {
        self.inner.can_undo()
    }

    #[wasm_bindgen(js_name = canRedo)]
    pub fn can_redo(&self) -> bool {
        self.inner.can_redo()
    }

    pub fn undo(&mut self) -> Result<(), String> {
        self.inner.undo().map_err(reject)
    }

    pub fn redo(&mut self) -> Result<(), String> {
        self.inner.redo().map_err(reject)
    }

    #[wasm_bindgen(js_name = clearHistory)]
    pub fn clear_history(&mut self) {
        self.inner.clear_history();
    }

    /// Version 1 `pixelforge-project` JSON for the current document.
    #[wasm_bindgen(js_name = saveProject)]
    pub fn save_project(&self) -> String {
        self.inner.save_project()
    }

    /// Replaces nothing in place: returns a new document parsed by the kernel.
    /// History on that document is empty.
    #[wasm_bindgen(js_name = loadProject)]
    pub fn load_project(json: &str) -> Result<PixelDocument, String> {
        Document::load_project(json)
            .map(|inner| Self { inner })
            .map_err(reject)
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

    #[test]
    fn undo_and_redo_share_the_kernel_document() {
        let mut document = doc();
        document.begin_gesture().unwrap();
        document.paint(1, 1, 2).unwrap();
        document.paint(2, 1, 3).unwrap();
        document.end_gesture();
        assert!(document.can_undo());

        document.undo().unwrap();
        assert_eq!(document.pixels()[1 + 16], 0);
        assert_eq!(document.pixels()[2 + 16], 0);
        assert!(document.can_redo());

        document.redo().unwrap();
        assert_eq!(document.pixels()[1 + 16], 2);
        assert_eq!(document.pixels()[2 + 16], 3);

        document.begin_gesture().unwrap();
        let index = document.add_frame();
        document.set_active_frame(index).unwrap();
        document.end_gesture();
        assert_eq!(document.frame_count(), 2);
        assert_eq!(document.active_frame(), 1);
        assert!(document.pixels().iter().all(|pixel| *pixel == 0));

        document.undo().unwrap();
        assert_eq!(document.frame_count(), 1);
        assert_eq!(document.active_frame(), 0);
        assert_eq!(document.pixels()[1 + 16], 2);

        document.undo().unwrap();
        assert_eq!(document.pixels()[1 + 16], 0);
        assert_eq!(
            document.undo().unwrap_err(),
            "NothingToUndo: nothing to undo"
        );
    }

    #[test]
    fn edited_palette_is_what_paint_indexes() {
        let mut document = doc();
        document.paint(2, 0, 3).unwrap();
        document.set_palette_color(3, 1, 2, 3).unwrap();

        let rgba = document.palette_rgba();
        assert_eq!(&rgba[3 * 4..4 * 4], &[1, 2, 3, 255]);
        assert_eq!(document.pixels()[2], 3);

        let added = document.add_palette_color(9, 8, 7).unwrap();
        assert_eq!(added, 9);
        document.paint(3, 0, added).unwrap();
        assert_eq!(document.pixels()[3], added);
        assert_eq!(&document.palette_rgba()[9 * 4..10 * 4], &[9, 8, 7, 255]);

        document.undo().unwrap();
        assert_eq!(document.pixels()[3], 0);
        document.undo().unwrap();
        assert_eq!(document.palette_rgba().len(), 9 * 4);
        document.undo().unwrap();
        assert_eq!(&document.palette_rgba()[3 * 4..4 * 4], &[232, 93, 76, 255]);
        assert_eq!(document.pixels()[2], 3);

        assert_eq!(
            document.set_palette_color(0, 4, 5, 6).unwrap_err(),
            "TransparentSlot: palette index 0 must be transparent"
        );
        assert_eq!(&document.palette_rgba()[..4], &[0, 0, 0, 0]);
    }

    #[test]
    fn save_and_load_round_trip_the_open_document() {
        let mut document = doc();
        document.paint(1, 0, 4).unwrap();
        document.add_frame();
        document.set_active_frame(1).unwrap();
        document.paint(0, 1, 5).unwrap();

        let mut loaded = PixelDocument::load_project(&document.save_project()).unwrap();
        assert_eq!(loaded.width(), 16);
        assert_eq!(loaded.height(), 16);
        assert_eq!(loaded.frame_count(), 2);
        assert_eq!(loaded.active_frame(), 1);
        assert_eq!(loaded.pixels()[0 + 16], 5);
        assert_eq!(loaded.palette_rgba(), document.palette_rgba());
        assert!(!loaded.can_undo());
        assert!(!loaded.can_redo());

        loaded.set_active_frame(0).unwrap();
        assert_eq!(loaded.pixels()[1], 4);
        assert_eq!(
            PixelDocument::load_project("{").unwrap_err(),
            "ProjectFile: project file is not valid version 1 JSON"
        );
    }

    #[test]
    fn delay_scrub_and_loop_stay_in_the_kernel() {
        let mut document = doc();
        assert_eq!(document.frame_delays(), vec![100]);
        document.add_frame();
        document.set_frame_delay(0, 80).unwrap();
        document.set_frame_delay(1, 40).unwrap();
        document.clear_history();
        assert_eq!(document.frame_delays(), vec![80, 40]);
        assert!(!document.can_undo());

        document.play();
        assert!(document.playing());
        assert!(!document.tick(79));
        assert_eq!(document.active_frame(), 0);
        assert!(document.tick(1));
        assert_eq!(document.active_frame(), 1);
        assert!(document.tick(40));
        assert_eq!(document.active_frame(), 0);
        assert!(!document.can_undo());

        document.begin_gesture().unwrap();
        document.scrub(0).unwrap();
        document.scrub(900_000).unwrap();
        document.end_gesture();
        assert!(!document.playing());
        assert_eq!(document.active_frame(), 1);
        document.undo().unwrap();
        assert_eq!(document.active_frame(), 0);
        assert!(!document.can_undo());

        document.set_frame_delay(0, 180).unwrap();
        let loaded = PixelDocument::load_project(&document.save_project()).unwrap();
        assert_eq!(loaded.frame_delays(), vec![180, 40]);
        assert_eq!(loaded.active_frame(), 0);
        assert!(!loaded.playing());
        assert!(!loaded.can_undo());

        let legacy = PixelDocument::load_project(
            r#"{"format":"pixelforge-project","version":1,"width":1,"height":1,"activeFrame":0,"palette":[{"r":0,"g":0,"b":0,"a":0}],"frames":[{"pixels":[0]}]}"#,
        )
        .unwrap();
        assert_eq!(legacy.frame_delays(), vec![100]);
        assert_eq!(
            document.set_frame_delay(0, 0).unwrap_err(),
            "FrameDelay: frame delay 0 ms is outside 1..=65535"
        );
    }

    #[test]
    fn save_and_load_keep_an_edited_palette() {
        let mut document = doc();
        document.set_palette_color(3, 1, 2, 3).unwrap();
        let added = document.add_palette_color(9, 8, 7).unwrap();
        document.paint(0, 0, added).unwrap();
        assert!(document.can_undo());

        let loaded = PixelDocument::load_project(&document.save_project()).unwrap();
        assert_eq!(loaded.palette_rgba(), document.palette_rgba());
        assert_eq!(loaded.pixels()[0], added);
        assert!(!loaded.can_undo());
        assert!(!loaded.can_redo());
    }
}
