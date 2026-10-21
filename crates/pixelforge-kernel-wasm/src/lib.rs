//! Browser bridge for [`pixelforge_kernel::Document`].
//!
//! The TypeScript shell loads this crate as WebAssembly and keeps using
//! `snapshot()`, `paint`, `fillRect`, `erase`, `eraseRect`, `copyRect`, `moveRect`, palette edits, palette lock, palette import,
//! frame-list methods, per-frame delay, timeline scrub, loop playback, undo /
//! redo, project save / load, `frameRgba`, and PNG export. `callTool` is the
//! same Tool API as [`pixelforge_kernel::Document::call_tool`]: one JSON
//! request in, one JSON response out, on this document. Pixel rules, delays,
//! the playback clock, the undo stack, the project file, and PNG bytes stay in
//! the kernel. `frameRgba` copies the same RGBA a PNG would encode for that
//! frame; the shell uses it for onion skin and does not write it back. This
//! module only copies bytes across the boundary and turns
//! [`pixelforge_kernel::KernelError`] into a `Code: message` string the shell
//! maps back onto its `KernelError`. Tool API failures stay in the JSON body.

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

    /// Runs one Tool API request against this document.
    ///
    /// `request` is JSON `{ "tool", "params" }`. The return value is always
    /// JSON `{ "ok", "apiVersion", "result" | "error" }`. A refused tool stays
    /// in that object and is not thrown. Mutations use the same document as
    /// `paint` and `undo`. See `docs/tool-api.md`.
    #[wasm_bindgen(js_name = callTool)]
    pub fn call_tool(&mut self, request: &str) -> String {
        self.inner.call_tool(request)
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

    /// Composite palette indices for the active frame, row-major.
    ///
    /// Index 0 shows the layer underneath. Any other index covers it. The
    /// shell draws this buffer. Paint still writes the active layer.
    pub fn pixels(&self) -> Vec<u8> {
        self.inner
            .composite_pixels(self.inner.active_frame())
            .expect("active frame is in range")
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

    /// Fills an axis-aligned rectangle on the active frame. One undo step.
    ///
    /// `width` and `height` are at least 1. The rectangle must lie entirely
    /// inside the canvas. A locked palette still allows this. Index 0 is refused.
    #[wasm_bindgen(js_name = fillRect)]
    pub fn fill_rect(
        &mut self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        color_index: u8,
    ) -> Result<(), String> {
        self.inner
            .fill_rect(x, y, width, height, color_index)
            .map_err(reject)
    }

    pub fn erase(&mut self, x: u16, y: u16) -> Result<(), String> {
        self.inner.erase(x, y).map_err(reject)
    }

    /// Clears an axis-aligned rectangle to transparent index 0. One undo step.
    ///
    /// Same bounds as [`Self::fill_rect`]: any cell outside the canvas writes nothing.
    #[wasm_bindgen(js_name = eraseRect)]
    pub fn erase_rect(&mut self, x: u16, y: u16, width: u16, height: u16) -> Result<(), String> {
        self.inner.erase_rect(x, y, width, height).map_err(reject)
    }

    /// Copies palette indices, including 0, on the active frame. One undo step.
    ///
    /// Source and destination must both lie on the canvas.
    #[wasm_bindgen(js_name = copyRect)]
    pub fn copy_rect(
        &mut self,
        src_x: u16,
        src_y: u16,
        width: u16,
        height: u16,
        dst_x: u16,
        dst_y: u16,
    ) -> Result<(), String> {
        self.inner
            .copy_rect(src_x, src_y, width, height, dst_x, dst_y)
            .map_err(reject)
    }

    /// Moves palette indices on the active frame and clears the source. One undo step.
    #[wasm_bindgen(js_name = moveRect)]
    pub fn move_rect(
        &mut self,
        src_x: u16,
        src_y: u16,
        width: u16,
        height: u16,
        dst_x: u16,
        dst_y: u16,
    ) -> Result<(), String> {
        self.inner
            .move_rect(src_x, src_y, width, height, dst_x, dst_y)
            .map_err(reject)
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

    /// True when recolor, append, and import are refused. Paint still works.
    #[wasm_bindgen(js_name = paletteLocked)]
    pub fn palette_locked(&self) -> bool {
        self.inner.palette_locked()
    }

    /// Locks or unlocks palette edits. The same value is not recorded.
    /// The flag is stored in the project file.
    #[wasm_bindgen(js_name = setPaletteLocked)]
    pub fn set_palette_locked(&mut self, locked: bool) {
        self.inner.set_palette_locked(locked);
    }

    /// Imports a GIMP palette (`.gpl`) or a hex list.
    ///
    /// `mode` is `replace` or `merge`. Replace makes the paint colors match
    /// the file and clears indices that no longer exist. Merge appends colors
    /// the palette does not already have. Either change is one undo step.
    /// Index 0 stays transparent. A locked palette is rejected.
    #[wasm_bindgen(js_name = importPalette)]
    pub fn import_palette(&mut self, text: &str, mode: &str) -> Result<(), String> {
        let mode = match mode {
            "replace" => pixelforge_kernel::PaletteImportMode::Replace,
            "merge" => pixelforge_kernel::PaletteImportMode::Merge,
            _ => return Err(reject(pixelforge_kernel::KernelError::PaletteImportMode)),
        };
        self.inner.import_palette(text, mode).map_err(reject)
    }

    /// Appends a blank frame and returns its index. The active frame stays put.
    #[wasm_bindgen(js_name = addFrame)]
    pub fn add_frame(&mut self) -> u32 {
        as_u32(self.inner.add_frame())
    }

    /// Inserts a copy after `index` and returns the new index.
    ///
    /// Does not select the copy. The shell's duplicate button calls
    /// [`Self::set_active_frame`] with that index; this method does not.
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

    /// Packed RGBA of one frame, row-major. Same colors as [`Self::export_frame_png`].
    ///
    /// The copy is for view compositing. It does not change pixels, history, or the project file.
    #[wasm_bindgen(js_name = frameRgba)]
    pub fn frame_rgba(&self, index: u32) -> Result<Vec<u8>, String> {
        self.inner.frame_rgba(as_index(index)).map_err(reject)
    }

    /// RGBA PNG of one frame. Index 0 is transparent. Does not change the document.
    #[wasm_bindgen(js_name = exportFramePng)]
    pub fn export_frame_png(&self, index: u32) -> Result<Vec<u8>, String> {
        self.inner.export_frame_png(as_index(index)).map_err(reject)
    }

    /// RGBA PNG of the grid sprite sheet. Does not change the document.
    #[wasm_bindgen(js_name = exportSpriteSheet)]
    pub fn export_sprite_sheet(&self) -> Vec<u8> {
        self.inner.export_sprite_sheet()
    }

    /// Columns in [`Self::export_sprite_sheet`]: `ceil(sqrt(frame_count))`.
    #[wasm_bindgen(js_name = spriteSheetColumns)]
    pub fn sprite_sheet_columns(&self) -> u32 {
        self.inner.sprite_sheet_layout().columns
    }

    /// Rows in [`Self::export_sprite_sheet`]: `ceil(frame_count / columns)`.
    #[wasm_bindgen(js_name = spriteSheetRows)]
    pub fn sprite_sheet_rows(&self) -> u32 {
        self.inner.sprite_sheet_layout().rows
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

    #[test]
    fn export_pngs_come_from_the_kernel() {
        let mut document = doc();
        document.paint(0, 0, 1).unwrap();
        document.add_frame();
        document.set_active_frame(1).unwrap();
        document.paint(1, 0, 2).unwrap();

        let first = document.export_frame_png(0).unwrap();
        let second = document.export_frame_png(1).unwrap();
        assert!(first.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(second.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert_ne!(first, second);
        assert!(document
            .export_frame_png(2)
            .unwrap_err()
            .starts_with("FrameIndex:"));

        assert_eq!(document.sprite_sheet_columns(), 2);
        assert_eq!(document.sprite_sheet_rows(), 1);
        let sheet = document.export_sprite_sheet();
        assert!(sheet.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(sheet.len() > first.len());
    }

    #[test]
    fn frame_rgba_reads_neighbors_without_writing() {
        let mut document = doc();
        document.paint(0, 0, 1).unwrap();
        document.add_frame();
        document.set_active_frame(1).unwrap();
        document.paint(1, 0, 2).unwrap();
        let saved = document.save_project();
        let active = document.pixels();

        let first = document.frame_rgba(0).unwrap();
        let second = document.frame_rgba(1).unwrap();
        assert_eq!(first.len(), 16 * 16 * 4);
        assert_eq!(second.len(), first.len());
        assert_ne!(first, second);

        let palette = document.palette_rgba();
        assert_eq!(&first[0..4], &palette[4..8]);
        assert_eq!(&first[4..8], &[0, 0, 0, 0]);
        assert_eq!(&second[0..4], &[0, 0, 0, 0]);
        assert_eq!(&second[4..8], &palette[8..12]);

        assert_eq!(document.pixels(), active);
        assert_eq!(document.active_frame(), 1);
        assert_eq!(document.save_project(), saved);
        assert!(document.can_undo());
        assert!(document
            .frame_rgba(2)
            .unwrap_err()
            .starts_with("FrameIndex:"));
        assert!(document
            .export_frame_png(0)
            .unwrap()
            .starts_with(b"\x89PNG"));
        assert!(!first.starts_with(b"\x89PNG"));
    }

    #[test]
    fn lock_and_import_round_trip_through_the_bridge() {
        let mut document = doc();
        assert!(!document.palette_locked());
        document.paint(0, 0, 8).unwrap();
        document
            .import_palette("#112233\n445566\n", "replace")
            .unwrap();
        assert_eq!(document.palette_rgba().len(), 3 * 4);
        assert_eq!(&document.palette_rgba()[4..8], &[0x11, 0x22, 0x33, 255]);
        assert_eq!(document.pixels()[0], 0);

        document.undo().unwrap();
        assert_eq!(document.palette_rgba().len(), 9 * 4);
        assert_eq!(document.pixels()[0], 8);

        document
            .import_palette("GIMP Palette\nName: Demo\n#\n1 2 3\n", "merge")
            .unwrap();
        assert_eq!(&document.palette_rgba()[9 * 4..10 * 4], &[1, 2, 3, 255]);
        document.undo().unwrap();
        assert_eq!(document.palette_rgba().len(), 9 * 4);

        document.set_palette_locked(true);
        assert!(document.palette_locked());
        assert_eq!(
            document.import_palette("ff0000\n", "replace").unwrap_err(),
            "PaletteLocked: palette is locked"
        );
        assert_eq!(
            document.set_palette_color(1, 4, 5, 6).unwrap_err(),
            "PaletteLocked: palette is locked"
        );
        document.paint(1, 0, 2).unwrap();
        assert_eq!(document.pixels()[1], 2);
        document.fill_rect(2, 0, 2, 1, 3).unwrap();
        assert_eq!(document.pixels()[2], 3);
        assert_eq!(document.pixels()[3], 3);
        let filled = document.call_tool(
            r#"{"tool":"fillRect","params":{"x":0,"y":1,"width":2,"height":1,"colorIndex":4}}"#,
        );
        assert!(filled.contains("\"ok\":true"), "{filled}");
        assert_eq!(document.pixels()[16], 4);
        assert_eq!(document.pixels()[17], 4);
        document.erase_rect(0, 1, 1, 1).unwrap();
        assert_eq!(document.pixels()[16], 0);
        assert_eq!(document.pixels()[17], 4);
        document.copy_rect(2, 0, 1, 1, 0, 1).unwrap();
        assert_eq!(document.pixels()[16], 3);
        document.move_rect(3, 0, 1, 1, 1, 1).unwrap();
        assert_eq!(document.pixels()[3], 0);
        assert_eq!(document.pixels()[17], 3);
        document.undo().unwrap();
        assert_eq!(document.pixels()[3], 3);
        assert_eq!(document.pixels()[17], 4);
        document.undo().unwrap();
        document.undo().unwrap();
        assert_eq!(document.pixels()[16], 4);
        assert_eq!(document.pixels()[17], 4);

        let loaded = PixelDocument::load_project(&document.save_project()).unwrap();
        assert!(loaded.palette_locked());
        assert_eq!(loaded.pixels()[1], 2);
        assert_eq!(loaded.pixels()[2], 3);
        assert_eq!(loaded.pixels()[16], 4);
        assert!(!loaded.can_undo());
        assert_eq!(
            document.import_palette("ff0000\n", "nope").unwrap_err(),
            "PaletteImportMode: palette import mode must be replace or merge"
        );
    }

    #[test]
    fn call_tool_writes_the_same_document() {
        let mut document = doc();
        let painted =
            document.call_tool(r#"{"tool":"paint","params":{"x":1,"y":0,"colorIndex":2}}"#);
        assert!(painted.contains("\"ok\":true"), "{painted}");
        assert_eq!(document.pixels()[1], 2);
        assert!(document.can_undo());

        let missing = document.call_tool(r#"{"tool":"setActiveFrame","params":{"index":3}}"#);
        assert!(missing.contains("\"code\":\"FrameIndex\""), "{missing}");
        assert_eq!(document.active_frame(), 0);
        assert_eq!(document.pixels()[1], 2);

        document.call_tool(r#"{"tool":"setPaletteLocked","params":{"locked":true}}"#);
        let locked = document
            .call_tool(r#"{"tool":"setPaletteColor","params":{"index":1,"r":1,"g":2,"b":3}}"#);
        assert!(locked.contains("\"code\":\"PaletteLocked\""), "{locked}");
        assert_eq!(&document.palette_rgba()[4..8], &[26, 28, 36, 255]);
        assert_eq!(document.pixels()[1], 2);
    }
}
