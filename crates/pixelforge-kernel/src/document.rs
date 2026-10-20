use std::cmp::Ordering;

use crate::error::KernelError;
use crate::frame::Frame;
use crate::history::{Change, History};
use crate::palette::Palette;
use crate::palette_file::{parse_palette_text, PaletteImportMode};
use crate::Rgba;
use crate::DEFAULT_FRAME_DELAY_MS;
use crate::MAX_CANVAS_EDGE;
use crate::MAX_FRAME_DELAY_MS;
use crate::MIN_FRAME_DELAY_MS;
use crate::TIMELINE_RATIO_PPM;
use crate::TRANSPARENT_INDEX;

/// Indexed pixel document: size, palette, frames, delays, and the undo stack.
///
/// UI gestures and Tool API calls meet here. Paint, erase, rectangle fill,
/// rectangle erase, rectangle copy, rectangle move, palette edits, frame
/// edits, and delay edits record kernel operations on this document. Undo
/// restores those operations; it does not patch a buffer owned by the shell.
/// Loop playback is preview state: [`Self::tick`] moves the active frame from
/// each frame's delay and does not push undo entries. Project open and save
/// encode pixels, palette, the palette lock, frames, and delays, not the undo
/// stack or the playback clock. PNG export reads this state and does not
/// record an undo entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    width: u16,
    height: u16,
    palette: Palette,
    /// When set, recolor, append, and import fail. Paint still writes indices.
    palette_locked: bool,
    frames: Vec<Frame>,
    active: usize,
    history: History,
    playback: Playback,
}

/// Preview clock. Not stored in the project file and not part of an undo entry.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Playback {
    playing: bool,
    /// Milliseconds already shown of the active frame.
    elapsed_ms: u32,
}

impl Playback {
    fn stopped() -> Self {
        Self {
            playing: false,
            elapsed_ms: 0,
        }
    }
}

impl Document {
    /// Blank document filled with transparent indices and the studio palette.
    pub fn new(width: u16, height: u16) -> Result<Self, KernelError> {
        Self::with_palette(width, height, Palette::studio_default())
    }

    pub fn with_palette(width: u16, height: u16, palette: Palette) -> Result<Self, KernelError> {
        check_size(width, height)?;
        let len = pixel_len(width, height);
        Ok(Self {
            width,
            height,
            palette,
            palette_locked: false,
            frames: vec![Frame::blank(len, DEFAULT_FRAME_DELAY_MS)],
            active: 0,
            history: History::new(),
            playback: Playback::stopped(),
        })
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    pub fn palette_colors(&self) -> &[Rgba] {
        self.palette.colors()
    }

    /// True when recolor, append, and import are refused.
    pub fn palette_locked(&self) -> bool {
        self.palette_locked
    }

    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    pub fn active_frame(&self) -> usize {
        self.active
    }

    /// Palette indices for the active frame, row-major, length `width * height`.
    pub fn pixels(&self) -> &[u8] {
        self.frames[self.active].pixels()
    }

    pub fn frame_pixels(&self, index: usize) -> Result<&[u8], KernelError> {
        self.frames
            .get(index)
            .map(Frame::pixels)
            .ok_or(KernelError::FrameIndex {
                index,
                len: self.frames.len(),
            })
    }

    pub fn get(&self, x: u16, y: u16) -> Result<u8, KernelError> {
        let offset = self.offset(x, y)?;
        Ok(self.pixels()[offset])
    }

    /// Writes a non-transparent palette index. Index 0 is rejected; use [`Self::erase`].
    ///
    /// The cell stores `color_index`, not the entry's RGB. Recoloring that
    /// entry later changes how the cell is shown and exported. A cell that
    /// already holds `color_index` is left alone and is not recorded. A locked
    /// palette does not block this write.
    pub fn paint(&mut self, x: u16, y: u16, color_index: u8) -> Result<(), KernelError> {
        if !self.palette.contains_paint_index(color_index) {
            return Err(KernelError::PaletteIndex { index: color_index });
        }
        let offset = self.offset(x, y)?;
        self.write_index(self.active, offset, color_index);
        Ok(())
    }

    /// Replaces the RGB of a paint entry. Index 0 cannot be changed.
    ///
    /// Cells keep their indices, so every frame that uses `index` shows the
    /// new color. The same RGB is not recorded. A color-picker drag should
    /// sit inside one gesture; those writes fold into a single undo step.
    pub fn set_palette_color(&mut self, index: u8, r: u8, g: u8, b: u8) -> Result<(), KernelError> {
        self.ensure_palette_unlocked()?;
        let before = self.palette.set_rgb(index, r, g, b)?;
        let after = Rgba::rgb(r, g, b);
        if before == after {
            return Ok(());
        }
        self.history.record(Change::PaletteColor {
            index,
            before,
            after,
        });
        Ok(())
    }

    /// Appends an opaque color and returns its index. Paint can use it immediately.
    pub fn add_palette_color(&mut self, r: u8, g: u8, b: u8) -> Result<u8, KernelError> {
        self.ensure_palette_unlocked()?;
        let index = self.palette.push_rgb(r, g, b)?;
        self.history.record(Change::AddPaletteColor {
            index,
            color: Rgba::rgb(r, g, b),
        });
        Ok(index)
    }

    /// Locks or unlocks palette edits. Painting is unaffected.
    ///
    /// The same value is not recorded. The flag is stored in the project file.
    /// Undo and redo still apply palette operations that were already on the
    /// stack; the lock only refuses new recolors, appends, and imports.
    pub fn set_palette_locked(&mut self, locked: bool) {
        if self.palette_locked == locked {
            return;
        }
        let before = self.palette_locked;
        self.palette_locked = locked;
        self.history.record(Change::PaletteLocked {
            before,
            after: locked,
        });
    }

    /// Reads a GIMP palette (`.gpl`) or a hex list into this document.
    ///
    /// See [`PaletteImportMode`]. Index 0 stays transparent, and every imported
    /// color is stored opaque. A locked palette is refused before the text is
    /// applied. A file that does not change the document is not recorded.
    /// [`PaletteImportMode::Replace`] that drops indices, and
    /// [`PaletteImportMode::Merge`] that appends several colors, are one undo
    /// step. Callers wrap this in a gesture only when it should join other edits.
    pub fn import_palette(
        &mut self,
        text: &str,
        mode: PaletteImportMode,
    ) -> Result<(), KernelError> {
        self.ensure_palette_unlocked()?;
        let imported = parse_palette_text(text)?;
        match mode {
            PaletteImportMode::Replace => self.replace_palette(imported),
            PaletteImportMode::Merge => self.merge_palette(imported),
        }
    }

    fn replace_palette(&mut self, imported: Vec<Rgba>) -> Result<(), KernelError> {
        let mut colors = Vec::with_capacity(imported.len() + 1);
        colors.push(Rgba::TRANSPARENT);
        colors.extend(imported);
        let checked = Palette::new(colors.clone())?;
        if checked.colors() == self.palette.colors() {
            return Ok(());
        }
        let mut changes = Vec::new();
        if colors.len() < self.palette.len() {
            let limit = u8::try_from(colors.len()).expect("palette length fits in u8");
            for (frame, pixels) in self.frames.iter().enumerate() {
                for (offset, index) in pixels.pixels().iter().copied().enumerate() {
                    if index >= limit {
                        changes.push(Change::Pixel {
                            frame,
                            offset,
                            before: index,
                            after: TRANSPARENT_INDEX,
                        });
                    }
                }
            }
        }
        for change in &changes {
            let Change::Pixel {
                frame,
                offset,
                after,
                ..
            } = change
            else {
                continue;
            };
            self.frames[*frame].set(*offset, *after);
        }
        let before = self.palette.colors().to_vec();
        self.palette = checked;
        changes.push(Change::PaletteReplace {
            before,
            after: colors,
        });
        self.history.record_group(changes);
        Ok(())
    }

    fn merge_palette(&mut self, imported: Vec<Rgba>) -> Result<(), KernelError> {
        let mut existing = self.palette.colors().to_vec();
        let mut additions = Vec::new();
        for color in imported {
            if existing.contains(&color) {
                continue;
            }
            existing.push(color);
            additions.push(color);
        }
        if additions.is_empty() {
            return Ok(());
        }
        if existing.len() > usize::from(u8::MAX) + 1 {
            return Err(KernelError::PaletteTooLarge {
                len: existing.len(),
            });
        }
        let mut changes = Vec::new();
        for color in additions {
            let index = self
                .palette
                .push_rgb(color.r, color.g, color.b)
                .expect("merged color fits");
            changes.push(Change::AddPaletteColor { index, color });
        }
        self.history.record_group(changes);
        Ok(())
    }

    fn ensure_palette_unlocked(&self) -> Result<(), KernelError> {
        if self.palette_locked {
            Err(KernelError::PaletteLocked)
        } else {
            Ok(())
        }
    }

    /// Fills an axis-aligned rectangle on the active frame with one palette index.
    ///
    /// `x` and `y` are the inclusive top-left cell. `width` and `height` are
    /// the size in cells. Every cell must lie on the canvas; a zero edge, an
    /// origin outside the canvas, or a far edge that falls outside is refused
    /// and writes nothing. The cell stores `color_index`, not RGB. Cells that
    /// already hold that index are skipped. The cells that do change are one
    /// undo step, or part of the open gesture when one is open. A fill that
    /// changes nothing is not recorded. A locked palette does not block this
    /// write. Index 0 is rejected; use [`Self::erase`] or [`Self::erase_rect`].
    pub fn fill_rect(
        &mut self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        color_index: u8,
    ) -> Result<(), KernelError> {
        if !self.palette.contains_paint_index(color_index) {
            return Err(KernelError::PaletteIndex { index: color_index });
        }
        let (x1, y1) = self.inclusive_rect(x, y, width, height)?;
        let frame = self.active;
        let mut changes = Vec::new();
        for row in y..=y1 {
            let row_offset = pixel_len(self.width, row);
            for col in x..=x1 {
                let offset = row_offset + usize::from(col);
                let before = self.frames[frame].pixels()[offset];
                if before == color_index {
                    continue;
                }
                self.frames[frame].set(offset, color_index);
                changes.push(Change::Pixel {
                    frame,
                    offset,
                    before,
                    after: color_index,
                });
            }
        }
        self.history.record_group(changes);
        Ok(())
    }

    /// Sets an axis-aligned rectangle on the active frame to [`TRANSPARENT_INDEX`].
    ///
    /// Bounds match [`Self::fill_rect`]: a zero edge or any cell outside the
    /// canvas is refused and writes nothing. Cells that are already transparent
    /// are skipped. The cells that do change are one undo step, or part of the
    /// open gesture when one is open. A rectangle that changes nothing is not
    /// recorded. A locked palette does not block this write.
    pub fn erase_rect(
        &mut self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    ) -> Result<(), KernelError> {
        let (x1, y1) = self.inclusive_rect(x, y, width, height)?;
        let frame = self.active;
        let mut changes = Vec::new();
        for row in y..=y1 {
            let row_offset = pixel_len(self.width, row);
            for col in x..=x1 {
                let offset = row_offset + usize::from(col);
                let before = self.frames[frame].pixels()[offset];
                if before == TRANSPARENT_INDEX {
                    continue;
                }
                self.frames[frame].set(offset, TRANSPARENT_INDEX);
                changes.push(Change::Pixel {
                    frame,
                    offset,
                    before,
                    after: TRANSPARENT_INDEX,
                });
            }
        }
        self.history.record_group(changes);
        Ok(())
    }

    /// Copies palette indices from one rectangle to another on the active frame.
    ///
    /// `src_x` and `src_y` are the source top-left. `dst_x` and `dst_y` are the
    /// destination top-left. `width` and `height` are the size in cells. Index
    /// 0 is copied as transparent. The source stays. Both rectangles must lie
    /// entirely on the canvas; if either would fall outside, nothing is written.
    /// Overlap reads the source before any destination cell is written. The
    /// cells that change are one undo step, or part of the open gesture. A copy
    /// that changes nothing is not recorded. A locked palette does not block
    /// this write. The active frame is both the source and the destination.
    pub fn copy_rect(
        &mut self,
        src_x: u16,
        src_y: u16,
        width: u16,
        height: u16,
        dst_x: u16,
        dst_y: u16,
    ) -> Result<(), KernelError> {
        let frame = self.active;
        self.copy_rect_from(frame, src_x, src_y, width, height, frame, dst_x, dst_y)
    }

    /// Copies indices from `src_frame` onto `dst_frame`. Does not change the
    /// active frame. Same bounds, overlap, and undo rules as [`Self::copy_rect`].
    ///
    /// When the frames differ there is no overlap: the source rectangle is read
    /// as it is, and the destination rectangle is replaced, including index 0.
    pub fn copy_rect_from(
        &mut self,
        src_frame: usize,
        src_x: u16,
        src_y: u16,
        width: u16,
        height: u16,
        dst_frame: usize,
        dst_x: u16,
        dst_y: u16,
    ) -> Result<(), KernelError> {
        self.blit_between(
            src_frame, src_x, src_y, width, height, dst_frame, dst_x, dst_y, false,
        )
    }

    /// Moves palette indices from one rectangle to another on the active frame.
    ///
    /// Same bounds and overlap rules as [`Self::copy_rect`]. Destination cells
    /// receive the source indices, including 0. Source cells that are not also
    /// in the destination become [`TRANSPARENT_INDEX`]. The whole move is one
    /// undo step, or part of the open gesture. A move onto itself changes
    /// nothing and is not recorded. A locked palette does not block this write.
    pub fn move_rect(
        &mut self,
        src_x: u16,
        src_y: u16,
        width: u16,
        height: u16,
        dst_x: u16,
        dst_y: u16,
    ) -> Result<(), KernelError> {
        let frame = self.active;
        self.move_rect_from(frame, src_x, src_y, width, height, frame, dst_x, dst_y)
    }

    /// Moves indices from `src_frame` onto `dst_frame`. Does not change the
    /// active frame. Source cells that are not part of the destination become
    /// [`TRANSPARENT_INDEX`]. Across frames the whole source rectangle is
    /// cleared, because the destination lives on another frame.
    pub fn move_rect_from(
        &mut self,
        src_frame: usize,
        src_x: u16,
        src_y: u16,
        width: u16,
        height: u16,
        dst_frame: usize,
        dst_x: u16,
        dst_y: u16,
    ) -> Result<(), KernelError> {
        self.blit_between(
            src_frame, src_x, src_y, width, height, dst_frame, dst_x, dst_y, true,
        )
    }

    /// Mirrors palette indices left to right inside a rectangle.
    ///
    /// Cell `x` receives the index that was at `origin_x + width - 1 - (x - origin_x)`.
    /// On a full frame of width 64 that is `63 - x`. Palette colors are not
    /// changed. The rectangle must lie on the canvas. The mirror is one undo
    /// step. A symmetric rectangle is not recorded. `frame` is not selected.
    pub fn flip_horizontal(
        &mut self,
        frame: usize,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    ) -> Result<(), KernelError> {
        let frame = self.frame_or_err(frame)?;
        let (x1, y1) = self.inclusive_rect(x, y, width, height)?;
        let mut sample = Vec::with_capacity(usize::from(width) * usize::from(height));
        {
            let pixels = self.frames[frame].pixels();
            for row in y..=y1 {
                let row_offset = pixel_len(self.width, row);
                for col in x..=x1 {
                    sample.push(pixels[row_offset + usize::from(col)]);
                }
            }
        }
        let mut changes = Vec::new();
        let span = usize::from(width);
        for row in y..=y1 {
            let local_y = usize::from(row - y);
            let row_offset = pixel_len(self.width, row);
            for col in x..=x1 {
                let local_x = usize::from(col - x);
                let mirrored = span - 1 - local_x;
                let after = sample[local_y * span + mirrored];
                let offset = row_offset + usize::from(col);
                let before = self.frames[frame].pixels()[offset];
                if before == after {
                    continue;
                }
                self.frames[frame].set(offset, after);
                changes.push(Change::Pixel {
                    frame,
                    offset,
                    before,
                    after,
                });
            }
        }
        self.history.record_group(changes);
        Ok(())
    }

    /// Places every frame's indices onto a new canvas.
    ///
    /// `offset_x` and `offset_y` are where the old origin lands on the new
    /// canvas. They may be negative, which clips the old left or top edge.
    /// Cells with no source stay [`TRANSPARENT_INDEX`]. Content that falls
    /// outside the new canvas is dropped. Delays, the palette, the lock, and
    /// the active frame stay. The project file stays version 1; only `width`,
    /// `height`, and each frame's `pixels` change. One undo step restores the
    /// previous buffers. A resize that leaves every index where it was is not
    /// recorded.
    pub fn resize_canvas(
        &mut self,
        width: u16,
        height: u16,
        offset_x: i32,
        offset_y: i32,
    ) -> Result<(), KernelError> {
        check_size(width, height)?;
        let before_pixels: Vec<Vec<u8>> = self
            .frames
            .iter()
            .map(|frame| frame.pixels().to_vec())
            .collect();
        let after_pixels: Vec<Vec<u8>> = before_pixels
            .iter()
            .map(|pixels| {
                place_pixels(
                    pixels,
                    self.width,
                    self.height,
                    width,
                    height,
                    offset_x,
                    offset_y,
                )
            })
            .collect();
        if self.width == width && self.height == height && before_pixels == after_pixels {
            return Ok(());
        }
        let before_width = self.width;
        let before_height = self.height;
        self.width = width;
        self.height = height;
        for (frame, pixels) in self.frames.iter_mut().zip(after_pixels.iter().cloned()) {
            frame.set_pixels(pixels);
        }
        self.history.record(Change::CanvasResize {
            before_width,
            before_height,
            after_width: width,
            after_height: height,
            before_pixels,
            after_pixels,
        });
        Ok(())
    }

    /// Copies `width` by `height` indices from one frame's rectangle to another.
    ///
    /// When `clear_source` is set, source cells outside the destination are
    /// cleared to [`TRANSPARENT_INDEX`] in the same undo step. Across frames
    /// the destination never covers the source, so the whole source rectangle
    /// is cleared.
    fn blit_between(
        &mut self,
        src_frame: usize,
        src_x: u16,
        src_y: u16,
        width: u16,
        height: u16,
        dst_frame: usize,
        dst_x: u16,
        dst_y: u16,
        clear_source: bool,
    ) -> Result<(), KernelError> {
        let src_frame = self.frame_or_err(src_frame)?;
        let dst_frame = self.frame_or_err(dst_frame)?;
        let (src_x1, src_y1) = self.inclusive_rect(src_x, src_y, width, height)?;
        let (dst_x1, dst_y1) = self.inclusive_rect(dst_x, dst_y, width, height)?;
        let mut sample = Vec::with_capacity(usize::from(width) * usize::from(height));
        {
            let pixels = self.frames[src_frame].pixels();
            for row in src_y..=src_y1 {
                let row_offset = pixel_len(self.width, row);
                for col in src_x..=src_x1 {
                    sample.push(pixels[row_offset + usize::from(col)]);
                }
            }
        }

        let count = self.pixel_count();
        let mut src_plan = vec![None; count];
        let mut dst_plan = if src_frame == dst_frame {
            None
        } else {
            Some(vec![None; count])
        };
        if clear_source {
            for row in src_y..=src_y1 {
                for col in src_x..=src_x1 {
                    if src_frame == dst_frame
                        && col >= dst_x
                        && col <= dst_x1
                        && row >= dst_y
                        && row <= dst_y1
                    {
                        continue;
                    }
                    let offset = pixel_len(self.width, row) + usize::from(col);
                    src_plan[offset] = Some(TRANSPARENT_INDEX);
                }
            }
        }
        let mut sample_at = 0;
        if src_frame == dst_frame {
            for row in dst_y..=dst_y1 {
                let row_offset = pixel_len(self.width, row);
                for col in dst_x..=dst_x1 {
                    let offset = row_offset + usize::from(col);
                    src_plan[offset] = Some(sample[sample_at]);
                    sample_at += 1;
                }
            }
        } else {
            let plan = dst_plan.as_mut().expect("cross-frame blit has a dest plan");
            for row in dst_y..=dst_y1 {
                let row_offset = pixel_len(self.width, row);
                for col in dst_x..=dst_x1 {
                    let offset = row_offset + usize::from(col);
                    plan[offset] = Some(sample[sample_at]);
                    sample_at += 1;
                }
            }
        }

        let mut changes = Vec::new();
        self.push_plan(src_frame, &src_plan, &mut changes);
        if let Some(plan) = &dst_plan {
            self.push_plan(dst_frame, plan, &mut changes);
        }
        self.history.record_group(changes);
        Ok(())
    }

    fn push_plan(&mut self, frame: usize, plan: &[Option<u8>], changes: &mut Vec<Change>) {
        for (offset, after) in plan.iter().enumerate() {
            let Some(after) = after else {
                continue;
            };
            let before = self.frames[frame].pixels()[offset];
            if before == *after {
                continue;
            }
            self.frames[frame].set(offset, *after);
            changes.push(Change::Pixel {
                frame,
                offset,
                before,
                after: *after,
            });
        }
    }

    fn frame_or_err(&self, index: usize) -> Result<usize, KernelError> {
        if index < self.frames.len() {
            Ok(index)
        } else {
            Err(KernelError::FrameIndex {
                index,
                len: self.frames.len(),
            })
        }
    }

    /// Inclusive bottom-right of a rectangle that lies entirely on the canvas.
    ///
    /// A zero edge or a span that does not fit is [`KernelError::OutOfBounds`]
    /// and names a cell that is not on the canvas when one exists.
    fn inclusive_rect(
        &self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    ) -> Result<(u16, u16), KernelError> {
        let outside = |x: u16, y: u16| KernelError::OutOfBounds {
            x,
            y,
            width: self.width,
            height: self.height,
        };
        if width == 0 || height == 0 {
            return Err(outside(x, y));
        }
        let x1 = x.checked_add(width - 1);
        let y1 = y.checked_add(height - 1);
        let x_ok = x1.is_some_and(|end| x < self.width && end < self.width);
        let y_ok = y1.is_some_and(|end| y < self.height && end < self.height);
        if x_ok && y_ok {
            return Ok((
                x1.expect("x span was checked"),
                y1.expect("y span was checked"),
            ));
        }
        let bad_x = if x >= self.width { x } else { self.width };
        let bad_y = if y >= self.height {
            y
        } else if y1.is_some_and(|end| end < self.height) {
            y
        } else {
            self.height
        };
        Err(outside(bad_x, bad_y))
    }

    /// Sets one cell back to [`TRANSPARENT_INDEX`].
    ///
    /// An empty cell is left alone and is not recorded.
    pub fn erase(&mut self, x: u16, y: u16) -> Result<(), KernelError> {
        let offset = self.offset(x, y)?;
        self.write_index(self.active, offset, TRANSPARENT_INDEX);
        Ok(())
    }

    /// Appends a blank frame and returns its index. The active frame stays put.
    pub fn add_frame(&mut self) -> usize {
        let index = self.frames.len();
        self.frames
            .push(Frame::blank(self.pixel_count(), DEFAULT_FRAME_DELAY_MS));
        self.history.record(Change::AddFrame { index });
        index
    }

    /// Inserts a copy after `index` and returns the new index.
    ///
    /// Does not select the copy. The active frame keeps showing the same
    /// pixels: an active index at or after the insertion point increases by
    /// one, and an earlier active index stays put. Call
    /// [`Self::set_active_frame`] with the returned index to edit the copy.
    pub fn duplicate_frame(&mut self, index: usize) -> Result<usize, KernelError> {
        let frame = self
            .frames
            .get(index)
            .cloned()
            .ok_or(KernelError::FrameIndex {
                index,
                len: self.frames.len(),
            })?;
        let inserted = index + 1;
        let active_before = self.active;
        self.frames.insert(inserted, frame.clone());
        if self.active >= inserted {
            self.active += 1;
        }
        self.history.record(Change::InsertFrame {
            index: inserted,
            frame,
            active_before,
            active_after: self.active,
        });
        Ok(inserted)
    }

    /// Removes a frame. The last remaining frame cannot be deleted.
    pub fn delete_frame(&mut self, index: usize) -> Result<(), KernelError> {
        if self.frames.len() == 1 {
            return Err(KernelError::LastFrame);
        }
        if index >= self.frames.len() {
            return Err(KernelError::FrameIndex {
                index,
                len: self.frames.len(),
            });
        }
        let active_before = self.active;
        let frame = self.frames.remove(index);
        self.active = match active_before.cmp(&index) {
            Ordering::Greater => active_before - 1,
            Ordering::Equal => index.min(self.frames.len() - 1),
            Ordering::Less => active_before,
        };
        self.clamp_elapsed();
        self.history.record(Change::RemoveFrame {
            index,
            frame,
            active_before,
            active_after: self.active,
        });
        Ok(())
    }

    /// Selects a frame. The same index is not recorded. Switching frames is an
    /// undoable kernel operation so later undos return to the frame that was
    /// being edited.
    pub fn set_active_frame(&mut self, index: usize) -> Result<(), KernelError> {
        if index >= self.frames.len() {
            return Err(KernelError::FrameIndex {
                index,
                len: self.frames.len(),
            });
        }
        if index == self.active {
            return Ok(());
        }
        let before = self.active;
        self.active = index;
        self.playback.elapsed_ms = 0;
        self.history.record(Change::ActiveFrame {
            before,
            after: index,
        });
        Ok(())
    }

    /// Milliseconds the frame stays on screen during [`Self::tick`].
    pub fn frame_delay(&self, index: usize) -> Result<u32, KernelError> {
        self.frames
            .get(index)
            .map(Frame::delay_ms)
            .ok_or(KernelError::FrameIndex {
                index,
                len: self.frames.len(),
            })
    }

    /// Delay of every frame, in timeline order.
    pub fn frame_delays(&self) -> Vec<u32> {
        self.frames.iter().map(Frame::delay_ms).collect()
    }

    /// Sum of every frame delay. Loop playback wraps at this length.
    pub fn duration_ms(&self) -> u64 {
        self.frames
            .iter()
            .map(|frame| u64::from(frame.delay_ms()))
            .sum()
    }

    /// Sets one frame's delay in milliseconds.
    ///
    /// The same value is not recorded. A drag or a typing session should sit
    /// inside one gesture; those writes fold into a single undo step.
    pub fn set_frame_delay(&mut self, index: usize, delay_ms: u32) -> Result<(), KernelError> {
        if index >= self.frames.len() {
            return Err(KernelError::FrameIndex {
                index,
                len: self.frames.len(),
            });
        }
        if !(MIN_FRAME_DELAY_MS..=MAX_FRAME_DELAY_MS).contains(&delay_ms) {
            return Err(KernelError::FrameDelay { delay_ms });
        }
        let before = self.frames[index].delay_ms();
        if before == delay_ms {
            return Ok(());
        }
        self.frames[index].set_delay_ms(delay_ms);
        if index == self.active && self.playback.elapsed_ms >= delay_ms {
            self.playback.elapsed_ms = 0;
        }
        self.history.record(Change::FrameDelay {
            index,
            before,
            after: delay_ms,
        });
        Ok(())
    }

    /// True while [`Self::tick`] advances frames.
    pub fn playing(&self) -> bool {
        self.playback.playing
    }

    /// Starts loop playback at the beginning of the active frame.
    ///
    /// A second call while already playing leaves the clock where it is.
    pub fn play(&mut self) {
        if self.playback.playing {
            return;
        }
        self.playback.elapsed_ms = 0;
        self.playback.playing = true;
    }

    /// Stops loop playback and clears the time already shown on this frame.
    pub fn stop(&mut self) {
        self.playback = Playback::stopped();
    }

    /// Advances the preview clock by `delta_ms`.
    ///
    /// Returns whether the active frame changed. The change is not an undo
    /// entry. A stopped document ignores `delta_ms`. Time past the end of the
    /// last frame wraps to the start of the first.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.playback.playing || delta_ms == 0 {
            return false;
        }
        let duration = self.duration_ms();
        if duration == 0 {
            return false;
        }
        let time = self.playback_time_ms().saturating_add(u64::from(delta_ms)) % duration;
        let index = self.frame_at_time(time);
        let changed = index != self.active;
        self.active = index;
        let elapsed = time.saturating_sub(self.frame_start_ms(index));
        self.playback.elapsed_ms = u32::try_from(elapsed).unwrap_or(MAX_FRAME_DELAY_MS);
        changed
    }

    /// Frame under `ratio_ppm` along the timeline.
    ///
    /// `0` is the start of frame 0. [`TIMELINE_RATIO_PPM`] is the end of the
    /// last frame. Each frame occupies a share of the line proportional to
    /// its delay. Values past the scale clamp to the last frame.
    pub fn frame_at_ratio(&self, ratio_ppm: u32) -> usize {
        let last = self.frames.len().saturating_sub(1);
        if last == 0 {
            return 0;
        }
        let ppm = u64::from(ratio_ppm.min(TIMELINE_RATIO_PPM));
        if ppm == 0 {
            return 0;
        }
        if ppm == u64::from(TIMELINE_RATIO_PPM) {
            return last;
        }
        let duration = self.duration_ms();
        if duration == 0 {
            return 0;
        }
        let time = ppm.saturating_mul(duration) / u64::from(TIMELINE_RATIO_PPM);
        self.frame_at_time(time.min(duration - 1))
    }

    /// Stops playback and selects the frame under `ratio_ppm`.
    ///
    /// Selecting the frame that is already active is not recorded. A scrub
    /// drag should sit inside one gesture so the selection collapses to a
    /// single undo step.
    pub fn scrub(&mut self, ratio_ppm: u32) -> Result<(), KernelError> {
        self.stop();
        let index = self.frame_at_ratio(ratio_ppm);
        self.set_active_frame(index)
    }

    /// Groups later successful edits into one undo step until [`Self::end_gesture`].
    ///
    /// A drag should call this once on pointer down. Nested gestures error.
    pub fn begin_gesture(&mut self) -> Result<(), KernelError> {
        self.history.begin_gesture()
    }

    /// Seals the open gesture. Does nothing when no gesture is open. A gesture
    /// that changed nothing does not push an undo entry or clear redo.
    pub fn end_gesture(&mut self) {
        self.history.end_gesture();
    }

    pub fn gesture_open(&self) -> bool {
        self.history.gesture_open()
    }

    /// True when [`Self::undo`] would restore an edit, including an open gesture
    /// that has already changed the document.
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    /// True when [`Self::redo`] would reapply an undone edit. False while an
    /// open gesture already has changes, because that gesture is a new branch.
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Reverts the latest kernel operation.
    ///
    /// An open gesture with edits is sealed and then reverted, so one undo
    /// cancels the in-progress stroke. An empty open gesture is dropped.
    pub fn undo(&mut self) -> Result<(), KernelError> {
        self.stop();
        let operation = self.history.take_undo()?;
        self.history.set_replaying(true);
        for change in operation.changes().iter().rev() {
            self.apply_change(change, false);
        }
        self.history.set_replaying(false);
        self.history.push_redo(operation);
        Ok(())
    }

    /// Reapplies the latest undone operation. Errors if the undo stack's redo
    /// side is empty, or if a gesture in progress already has edits.
    pub fn redo(&mut self) -> Result<(), KernelError> {
        self.stop();
        let operation = self.history.take_redo()?;
        self.history.set_replaying(true);
        for change in operation.changes() {
            self.apply_change(change, true);
        }
        self.history.set_replaying(false);
        self.history.push_undo(operation);
        Ok(())
    }

    /// Drops undo, redo, and any open gesture. Pixels and frames stay as they are.
    ///
    /// Used to mark a seeded document as the baseline the user undoes back to.
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// Rebuilds a stored document. The undo stack starts empty.
    ///
    /// The project loader checks size, palette, frames, and the active frame
    /// before calling this.
    pub(crate) fn from_stored(
        width: u16,
        height: u16,
        palette: Palette,
        palette_locked: bool,
        frames: Vec<Frame>,
        active: usize,
    ) -> Self {
        debug_assert!(!frames.is_empty(), "project files keep at least one frame");
        debug_assert!(active < frames.len(), "active frame was checked");
        let len = pixel_len(width, height);
        debug_assert!(
            frames.iter().all(|frame| frame.pixels().len() == len),
            "frame length was checked"
        );
        Self {
            width,
            height,
            palette,
            palette_locked,
            frames,
            active,
            history: History::new(),
            playback: Playback::stopped(),
        }
    }

    fn write_index(&mut self, frame: usize, offset: usize, value: u8) {
        debug_assert!(
            value == TRANSPARENT_INDEX || self.palette.contains_paint_index(value),
            "pixels store palette indices"
        );
        let before = self.frames[frame].pixels()[offset];
        if before == value {
            return;
        }
        self.frames[frame].set(offset, value);
        self.history.record(Change::Pixel {
            frame,
            offset,
            before,
            after: value,
        });
    }

    fn apply_change(&mut self, change: &Change, forward: bool) {
        match change {
            Change::Pixel {
                frame,
                offset,
                before,
                after,
            } => {
                let current = self.frames[*frame].pixels()[*offset];
                let (expected, next) = if forward {
                    (*before, *after)
                } else {
                    (*after, *before)
                };
                debug_assert_eq!(
                    current, expected,
                    "undo stack diverged from the document pixels"
                );
                self.frames[*frame].set(*offset, next);
            }
            Change::AddFrame { index } => {
                if forward {
                    debug_assert_eq!(*index, self.frames.len());
                    self.frames
                        .push(Frame::blank(self.pixel_count(), DEFAULT_FRAME_DELAY_MS));
                } else {
                    debug_assert_eq!(self.frames.len() - 1, *index);
                    self.frames.remove(*index);
                    self.clamp_active();
                }
            }
            Change::InsertFrame {
                index,
                frame,
                active_before,
                active_after,
            } => {
                if forward {
                    self.frames.insert(*index, frame.clone());
                    self.assign_active(*active_before, *active_after);
                } else {
                    self.frames.remove(*index);
                    self.assign_active(*active_after, *active_before);
                }
            }
            Change::RemoveFrame {
                index,
                frame,
                active_before,
                active_after,
            } => {
                if forward {
                    self.frames.remove(*index);
                    self.assign_active(*active_before, *active_after);
                } else {
                    self.frames.insert(*index, frame.clone());
                    self.assign_active(*active_after, *active_before);
                }
            }
            Change::ActiveFrame { before, after } => {
                self.active = if forward { *after } else { *before };
                self.playback.elapsed_ms = 0;
            }
            Change::FrameDelay {
                index,
                before,
                after,
            } => {
                let current = self.frames[*index].delay_ms();
                let (expected, next) = if forward {
                    (*before, *after)
                } else {
                    (*after, *before)
                };
                debug_assert_eq!(current, expected, "undo stack diverged from frame delays");
                self.frames[*index].set_delay_ms(next);
                self.clamp_elapsed();
            }
            Change::PaletteColor {
                index,
                before,
                after,
            } => {
                let current = self
                    .palette
                    .get(*index)
                    .expect("palette entry still exists");
                let (expected, next) = if forward {
                    (*before, *after)
                } else {
                    (*after, *before)
                };
                debug_assert_eq!(current, expected, "undo stack diverged from the palette");
                self.palette.write(*index, next);
            }
            Change::AddPaletteColor { index, color } => {
                if forward {
                    debug_assert_eq!(usize::from(*index), self.palette.len());
                    let added = self
                        .palette
                        .push_rgb(color.r, color.g, color.b)
                        .expect("undone palette color still fits");
                    debug_assert_eq!(added, *index);
                } else {
                    debug_assert_eq!(usize::from(*index) + 1, self.palette.len());
                    debug_assert!(
                        !self
                            .frames
                            .iter()
                            .any(|frame| frame.pixels().contains(index)),
                        "palette index {index} is still stored in a frame"
                    );
                    let removed = self.palette.pop();
                    debug_assert_eq!(removed, *color);
                }
            }
            Change::PaletteReplace { before, after } => {
                let next = if forward { after } else { before };
                self.palette.install(next.clone());
            }
            Change::PaletteLocked { before, after } => {
                self.palette_locked = if forward { *after } else { *before };
            }
            Change::CanvasResize {
                before_width,
                before_height,
                after_width,
                after_height,
                before_pixels,
                after_pixels,
            } => {
                let (width, height, pixels) = if forward {
                    (*after_width, *after_height, after_pixels)
                } else {
                    (*before_width, *before_height, before_pixels)
                };
                debug_assert_eq!(pixels.len(), self.frames.len());
                self.width = width;
                self.height = height;
                for (frame, pixels) in self.frames.iter_mut().zip(pixels.iter()) {
                    frame.set_pixels(pixels.clone());
                }
            }
        }
    }

    /// Restores an active index a structural edit itself changed. When the edit
    /// left the index numerically unchanged, keep the current selection unless
    /// it now points past the frame list.
    fn assign_active(&mut self, previous: usize, next: usize) {
        if previous != next {
            self.active = next;
        } else {
            self.clamp_active();
        }
    }

    fn clamp_active(&mut self) {
        if self.active >= self.frames.len() {
            self.active = self.frames.len() - 1;
        }
    }

    fn clamp_elapsed(&mut self) {
        let delay = self.frames[self.active].delay_ms();
        if self.playback.elapsed_ms >= delay {
            self.playback.elapsed_ms = 0;
        }
    }

    fn playback_time_ms(&self) -> u64 {
        self.frame_start_ms(self.active)
            .saturating_add(u64::from(self.playback.elapsed_ms))
    }

    fn frame_start_ms(&self, index: usize) -> u64 {
        self.frames[..index]
            .iter()
            .map(|frame| u64::from(frame.delay_ms()))
            .sum()
    }

    fn frame_at_time(&self, time_ms: u64) -> usize {
        let mut cursor = 0u64;
        for (index, frame) in self.frames.iter().enumerate() {
            cursor = cursor.saturating_add(u64::from(frame.delay_ms()));
            if time_ms < cursor {
                return index;
            }
        }
        self.frames.len().saturating_sub(1)
    }

    fn pixel_count(&self) -> usize {
        pixel_len(self.width, self.height)
    }

    fn offset(&self, x: u16, y: u16) -> Result<usize, KernelError> {
        if x >= self.width || y >= self.height {
            return Err(KernelError::OutOfBounds {
                x,
                y,
                width: self.width,
                height: self.height,
            });
        }
        Ok(pixel_len(self.width, y) + usize::from(x))
    }
}

fn check_size(width: u16, height: u16) -> Result<(), KernelError> {
    if width == 0 || height == 0 || width > MAX_CANVAS_EDGE || height > MAX_CANVAS_EDGE {
        Err(KernelError::InvalidSize { width, height })
    } else {
        Ok(())
    }
}

fn pixel_len(width: u16, height: u16) -> usize {
    usize::from(width) * usize::from(height)
}

/// Copies `source` onto a new buffer. `offset_x`/`offset_y` is where the old
/// origin lands. Cells outside either canvas are skipped.
fn place_pixels(
    source: &[u8],
    old_width: u16,
    old_height: u16,
    new_width: u16,
    new_height: u16,
    offset_x: i32,
    offset_y: i32,
) -> Vec<u8> {
    let mut next = vec![TRANSPARENT_INDEX; pixel_len(new_width, new_height)];
    let old_stride = usize::from(old_width);
    let new_stride = usize::from(new_width);
    for y in 0..u32::from(old_height) {
        for x in 0..u32::from(old_width) {
            let nx = x as i32 + offset_x;
            let ny = y as i32 + offset_y;
            if nx < 0 || ny < 0 || nx >= i32::from(new_width) || ny >= i32::from(new_height) {
                continue;
            }
            let src = y as usize * old_stride + x as usize;
            let dst = ny as usize * new_stride + nx as usize;
            next[dst] = source[src];
        }
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        PaletteImportMode, Rgba, DEFAULT_FRAME_DELAY_MS, MAX_FRAME_DELAY_MS, TIMELINE_RATIO_PPM,
    };

    fn doc() -> Document {
        Document::new(16, 16).unwrap()
    }

    #[test]
    fn new_document_is_transparent() {
        let document = doc();
        assert_eq!(document.width(), 16);
        assert_eq!(document.height(), 16);
        assert_eq!(document.frame_count(), 1);
        assert_eq!(document.active_frame(), 0);
        assert_eq!(document.pixels().len(), 16 * 16);
        assert!(document
            .pixels()
            .iter()
            .all(|index| *index == TRANSPARENT_INDEX));
    }

    #[test]
    fn accepts_m1_canvas_sizes() {
        for size in [16_u16, 32] {
            let mut document = Document::new(size, size).unwrap();
            document.paint(size - 1, size - 1, 1).unwrap();
            assert_eq!(document.get(size - 1, size - 1).unwrap(), 1);
        }
    }

    #[test]
    fn rejects_empty_and_oversized_canvas() {
        assert!(matches!(
            Document::new(0, 16),
            Err(KernelError::InvalidSize {
                width: 0,
                height: 16
            })
        ));
        assert!(matches!(
            Document::new(257, 1),
            Err(KernelError::InvalidSize {
                width: 257,
                height: 1
            })
        ));
    }

    #[test]
    fn paint_and_erase_one_cell() {
        let mut document = doc();
        document.paint(2, 3, 4).unwrap();
        assert_eq!(document.get(2, 3).unwrap(), 4);
        assert_eq!(document.get(2, 4).unwrap(), TRANSPARENT_INDEX);
        document.erase(2, 3).unwrap();
        assert_eq!(document.get(2, 3).unwrap(), TRANSPARENT_INDEX);
    }

    #[test]
    fn paint_rejects_transparent_unknown_and_out_of_bounds() {
        let mut document = doc();
        document.paint(1, 1, 2).unwrap();

        assert_eq!(
            document.paint(0, 0, TRANSPARENT_INDEX).unwrap_err(),
            KernelError::PaletteIndex { index: 0 }
        );
        assert_eq!(
            document.paint(0, 0, 99).unwrap_err(),
            KernelError::PaletteIndex { index: 99 }
        );
        assert!(matches!(
            document.paint(16, 0, 1),
            Err(KernelError::OutOfBounds { x: 16, y: 0, .. })
        ));
        assert!(matches!(
            document.erase(0, 16),
            Err(KernelError::OutOfBounds { x: 0, y: 16, .. })
        ));
        assert_eq!(document.get(1, 1).unwrap(), 2);
        assert_eq!(document.get(0, 0).unwrap(), TRANSPARENT_INDEX);
    }

    #[test]
    fn fill_rect_is_one_undo_and_refuses_a_partial_rectangle() {
        let mut document = doc();
        document.fill_rect(1, 2, 3, 2, 4).unwrap();
        for y in 2..4 {
            for x in 1..4 {
                assert_eq!(document.get(x, y).unwrap(), 4);
            }
        }
        assert_eq!(document.get(0, 2).unwrap(), TRANSPARENT_INDEX);
        assert_eq!(document.get(4, 2).unwrap(), TRANSPARENT_INDEX);
        assert_eq!(document.get(1, 4).unwrap(), TRANSPARENT_INDEX);
        document.paint(0, 0, 1).unwrap();
        document.undo().unwrap();
        assert_eq!(document.get(0, 0).unwrap(), TRANSPARENT_INDEX);
        assert_eq!(document.get(1, 2).unwrap(), 4);
        document.undo().unwrap();
        assert!(document
            .pixels()
            .iter()
            .all(|index| *index == TRANSPARENT_INDEX));
        assert!(!document.can_undo());
        document.redo().unwrap();
        assert_eq!(document.get(2, 3).unwrap(), 4);
        assert_eq!(document.get(0, 0).unwrap(), TRANSPARENT_INDEX);

        document.fill_rect(1, 2, 3, 2, 4).unwrap();
        document.undo().unwrap();
        assert_eq!(document.get(1, 2).unwrap(), TRANSPARENT_INDEX);

        document.fill_rect(14, 0, 2, 2, 5).unwrap();
        assert_eq!(document.get(15, 1).unwrap(), 5);
        let before = document.pixels().to_vec();
        assert!(matches!(
            document.fill_rect(14, 0, 4, 1, 2),
            Err(KernelError::OutOfBounds { x: 16, y: 0, .. })
        ));
        assert_eq!(document.pixels(), before.as_slice());
        assert_eq!(
            document
                .fill_rect(0, 0, 1, 1, TRANSPARENT_INDEX)
                .unwrap_err(),
            KernelError::PaletteIndex { index: 0 }
        );
        assert_eq!(
            document.fill_rect(20, 0, 1, 1, 0).unwrap_err(),
            KernelError::PaletteIndex { index: 0 }
        );
        assert!(document.fill_rect(0, 0, 0, 2, 1).is_err());
        assert_eq!(document.pixels(), before.as_slice());

        document.set_palette_locked(true);
        document.fill_rect(0, 1, 2, 1, 3).unwrap();
        assert_eq!(document.get(0, 1).unwrap(), 3);
        assert_eq!(document.get(1, 1).unwrap(), 3);
        assert!(document.palette_locked());

        let other = document.add_frame();
        document.set_active_frame(other).unwrap();
        assert!(document
            .pixels()
            .iter()
            .all(|index| *index == TRANSPARENT_INDEX));
        assert_eq!(document.frame_pixels(0).unwrap()[16], 3);

        document.begin_gesture().unwrap();
        document.fill_rect(0, 0, 2, 1, 6).unwrap();
        document.paint(3, 0, 7).unwrap();
        document.end_gesture();
        document.undo().unwrap();
        assert_eq!(document.get(0, 0).unwrap(), TRANSPARENT_INDEX);
        assert_eq!(document.get(3, 0).unwrap(), TRANSPARENT_INDEX);
        assert!(document.can_redo());
        assert_eq!(document.frame_pixels(0).unwrap()[16], 3);
    }

    #[test]
    fn erase_rect_is_one_undo_and_refuses_a_partial_rectangle() {
        let mut document = doc();
        document.fill_rect(1, 2, 3, 2, 4).unwrap();
        document.paint(0, 0, 1).unwrap();
        document.erase_rect(1, 2, 2, 2).unwrap();
        assert_eq!(document.get(1, 2).unwrap(), TRANSPARENT_INDEX);
        assert_eq!(document.get(2, 3).unwrap(), TRANSPARENT_INDEX);
        assert_eq!(document.get(3, 2).unwrap(), 4);
        assert_eq!(document.get(0, 0).unwrap(), 1);
        document.undo().unwrap();
        assert_eq!(document.get(1, 2).unwrap(), 4);
        assert_eq!(document.get(2, 3).unwrap(), 4);
        assert_eq!(document.get(0, 0).unwrap(), 1);
        document.redo().unwrap();
        assert_eq!(document.get(1, 2).unwrap(), TRANSPARENT_INDEX);

        document.erase_rect(1, 2, 2, 2).unwrap();
        document.undo().unwrap();
        assert_eq!(document.get(1, 2).unwrap(), 4);
        assert_eq!(document.get(0, 0).unwrap(), 1);

        let before = document.pixels().to_vec();
        assert!(matches!(
            document.erase_rect(14, 0, 4, 1),
            Err(KernelError::OutOfBounds { x: 16, y: 0, .. })
        ));
        assert!(document.erase_rect(0, 0, 0, 1).is_err());
        assert_eq!(document.pixels(), before.as_slice());

        document.set_palette_locked(true);
        document.erase_rect(0, 0, 1, 1).unwrap();
        assert_eq!(document.get(0, 0).unwrap(), TRANSPARENT_INDEX);
        assert!(document.palette_locked());

        let other = document.add_frame();
        document.paint(0, 0, 2).unwrap();
        document.set_active_frame(other).unwrap();
        document.paint(0, 0, 5).unwrap();
        document.erase_rect(0, 0, 1, 1).unwrap();
        assert_eq!(document.get(0, 0).unwrap(), TRANSPARENT_INDEX);
        assert_eq!(document.frame_pixels(0).unwrap()[0], 2);

        document.paint(1, 0, 7).unwrap();
        document.begin_gesture().unwrap();
        document.erase_rect(1, 0, 1, 1).unwrap();
        document.paint(2, 0, 6).unwrap();
        document.end_gesture();
        document.undo().unwrap();
        assert_eq!(document.get(1, 0).unwrap(), 7);
        assert_eq!(document.get(2, 0).unwrap(), TRANSPARENT_INDEX);
        assert_eq!(document.frame_pixels(0).unwrap()[0], 2);
    }

    #[test]
    fn copy_and_move_rects_are_one_undo_and_refuse_a_partial() {
        let mut document = Document::new(4, 1).unwrap();
        document.paint(0, 0, 1).unwrap();
        document.paint(1, 0, 2).unwrap();
        document.paint(2, 0, 3).unwrap();
        document.clear_history();

        document.copy_rect(0, 0, 2, 1, 2, 0).unwrap();
        assert_eq!(document.pixels(), &[1, 2, 1, 2]);
        document.undo().unwrap();
        assert_eq!(document.pixels(), &[1, 2, 3, 0]);
        assert!(!document.can_undo());
        document.redo().unwrap();
        assert_eq!(document.pixels(), &[1, 2, 1, 2]);
        document.undo().unwrap();

        document.copy_rect(0, 0, 2, 1, 1, 0).unwrap();
        assert_eq!(document.pixels(), &[1, 1, 2, 0]);
        document.undo().unwrap();
        assert_eq!(document.pixels(), &[1, 2, 3, 0]);

        let hole = document.pixels().to_vec();
        document.erase(1, 0).unwrap();
        document.copy_rect(0, 0, 2, 1, 2, 0).unwrap();
        assert_eq!(document.pixels(), &[1, 0, 1, 0]);
        document.undo().unwrap();
        document.undo().unwrap();
        assert_eq!(document.pixels(), hole.as_slice());

        document.copy_rect(0, 0, 2, 1, 0, 0).unwrap();
        assert_eq!(document.pixels(), &[1, 2, 3, 0]);
        assert!(!document.can_undo());

        document.move_rect(0, 0, 2, 1, 2, 0).unwrap();
        assert_eq!(document.pixels(), &[0, 0, 1, 2]);
        document.undo().unwrap();
        assert_eq!(document.pixels(), &[1, 2, 3, 0]);
        document.redo().unwrap();
        document.undo().unwrap();

        document.move_rect(0, 0, 2, 1, 1, 0).unwrap();
        assert_eq!(document.pixels(), &[0, 1, 2, 0]);
        document.undo().unwrap();
        assert_eq!(document.pixels(), &[1, 2, 3, 0]);

        document.move_rect(0, 0, 2, 1, 0, 0).unwrap();
        assert_eq!(document.pixels(), &[1, 2, 3, 0]);
        assert!(!document.can_undo());

        let before = document.pixels().to_vec();
        assert!(matches!(
            document.copy_rect(2, 0, 3, 1, 0, 0),
            Err(KernelError::OutOfBounds { .. })
        ));
        assert!(matches!(
            document.move_rect(0, 0, 2, 1, 3, 0),
            Err(KernelError::OutOfBounds { .. })
        ));
        assert_eq!(document.pixels(), before.as_slice());
        assert!(!document.can_undo());

        document.set_palette_locked(true);
        document.copy_rect(1, 0, 1, 1, 3, 0).unwrap();
        assert_eq!(document.get(3, 0).unwrap(), 2);
        document.move_rect(1, 0, 1, 1, 0, 0).unwrap();
        assert_eq!(document.pixels(), &[2, 0, 3, 2]);
        assert!(document.palette_locked());
        document.undo().unwrap();
        assert_eq!(document.pixels(), &[1, 2, 3, 2]);
        document.undo().unwrap();
        assert_eq!(document.pixels(), &[1, 2, 3, 0]);

        let kept = document.pixels().to_vec();
        let other = document.add_frame();
        document.set_active_frame(other).unwrap();
        document.paint(0, 0, 5).unwrap();
        document.copy_rect(0, 0, 1, 1, 1, 0).unwrap();
        assert_eq!(document.pixels(), &[5, 5, 0, 0]);
        assert_eq!(document.frame_pixels(0).unwrap(), kept.as_slice());

        document.begin_gesture().unwrap();
        document.move_rect(0, 0, 1, 1, 2, 0).unwrap();
        document.paint(3, 0, 6).unwrap();
        document.end_gesture();
        assert_eq!(document.pixels(), &[0, 5, 5, 6]);
        document.undo().unwrap();
        assert_eq!(document.pixels(), &[5, 5, 0, 0]);
        assert_eq!(document.frame_pixels(0).unwrap(), kept.as_slice());
    }

    #[test]
    fn copy_and_move_can_read_another_frame() {
        let mut document = Document::new(4, 2).unwrap();
        document.paint(0, 0, 1).unwrap();
        document.paint(1, 0, 2).unwrap();
        document.paint(0, 1, 3).unwrap();
        document.add_frame();
        document.set_active_frame(1).unwrap();
        document.paint(3, 1, 4).unwrap();
        document.clear_history();

        document.copy_rect_from(0, 0, 0, 2, 2, 1, 1, 0).unwrap();
        assert_eq!(document.active_frame(), 1);
        assert_eq!(document.frame_pixels(0).unwrap()[0], 1);
        assert_eq!(document.frame_pixels(0).unwrap()[1], 2);
        assert_eq!(document.get(1, 0).unwrap(), 1);
        assert_eq!(document.get(2, 0).unwrap(), 2);
        assert_eq!(document.get(1, 1).unwrap(), 3);
        assert_eq!(document.get(3, 1).unwrap(), 4);
        document.undo().unwrap();
        assert_eq!(document.get(1, 0).unwrap(), 0);
        assert_eq!(document.get(1, 1).unwrap(), 0);
        assert_eq!(document.get(3, 1).unwrap(), 4);

        document.move_rect_from(0, 0, 0, 2, 1, 1, 0, 0).unwrap();
        assert_eq!(document.frame_pixels(0).unwrap()[0], 0);
        assert_eq!(document.frame_pixels(0).unwrap()[1], 0);
        assert_eq!(document.frame_pixels(0).unwrap()[4], 3);
        assert_eq!(document.get(0, 0).unwrap(), 1);
        assert_eq!(document.get(1, 0).unwrap(), 2);
        document.undo().unwrap();
        assert_eq!(document.frame_pixels(0).unwrap()[0], 1);
        assert_eq!(document.frame_pixels(0).unwrap()[1], 2);
        assert_eq!(document.get(0, 0).unwrap(), 0);

        let before = document.frame_pixels(0).unwrap().to_vec();
        assert!(matches!(
            document.copy_rect_from(2, 0, 0, 1, 1, 1, 0, 0),
            Err(KernelError::FrameIndex { index: 2, len: 2 })
        ));
        assert!(matches!(
            document.move_rect_from(0, 0, 0, 1, 1, 1, 4, 0),
            Err(KernelError::OutOfBounds { .. })
        ));
        assert_eq!(document.frame_pixels(0).unwrap(), before.as_slice());
        assert!(!document.can_undo());
    }

    #[test]
    fn flip_horizontal_mirrors_indices_and_keeps_colors() {
        let mut document = Document::new(4, 2).unwrap();
        document.paint(0, 0, 1).unwrap();
        document.paint(1, 0, 2).unwrap();
        document.paint(0, 1, 3).unwrap();
        document.set_palette_color(1, 9, 8, 7).unwrap();
        let color = document.palette_colors()[1];
        document.clear_history();

        document.flip_horizontal(0, 0, 0, 4, 2).unwrap();
        assert_eq!(document.pixels(), &[0, 0, 2, 1, 0, 0, 0, 3]);
        assert_eq!(document.palette_colors()[1], color);
        document.undo().unwrap();
        assert_eq!(document.pixels(), &[1, 2, 0, 0, 3, 0, 0, 0]);

        document.flip_horizontal(0, 0, 0, 3, 1).unwrap();
        assert_eq!(&document.pixels()[..4], &[0, 2, 1, 0]);
        assert_eq!(document.pixels()[4], 3);
        document.clear_history();
        document.flip_horizontal(0, 1, 0, 1, 1).unwrap();
        assert!(!document.can_undo());

        document.add_frame();
        document.set_active_frame(1).unwrap();
        document.flip_horizontal(0, 0, 1, 4, 1).unwrap();
        assert_eq!(document.active_frame(), 1);
        assert_eq!(&document.frame_pixels(0).unwrap()[4..8], &[0, 0, 0, 3]);
        assert!(document
            .frame_pixels(1)
            .unwrap()
            .iter()
            .all(|index| *index == 0));
        document.undo().unwrap();
        assert_eq!(document.frame_pixels(0).unwrap()[4], 3);

        assert!(matches!(
            document.flip_horizontal(0, 2, 0, 3, 1),
            Err(KernelError::OutOfBounds { .. })
        ));
        assert!(matches!(
            document.flip_horizontal(3, 0, 0, 1, 1),
            Err(KernelError::FrameIndex { index: 3, .. })
        ));
    }

    #[test]
    fn resize_canvas_places_frames_and_undoes() {
        let mut document = Document::new(4, 2).unwrap();
        document.paint(0, 0, 1).unwrap();
        document.paint(3, 1, 2).unwrap();
        document.add_frame();
        document.set_frame_delay(1, 80).unwrap();
        document.set_active_frame(1).unwrap();
        document.paint(1, 0, 3).unwrap();
        document.set_palette_locked(true);
        document.clear_history();

        document.resize_canvas(6, 4, 1, 1).unwrap();
        assert_eq!((document.width(), document.height()), (6, 4));
        assert_eq!(document.active_frame(), 1);
        assert_eq!(document.frame_delay(1).unwrap(), 80);
        assert!(document.palette_locked());
        assert_eq!(document.frame_pixels(0).unwrap()[1 * 6 + 1], 1);
        assert_eq!(document.frame_pixels(0).unwrap()[2 * 6 + 4], 2);
        assert_eq!(document.frame_pixels(0).unwrap()[0], 0);
        assert_eq!(document.get(2, 1).unwrap(), 3);
        let json = document.save_project();
        let loaded = Document::load_project(&json).unwrap();
        assert_eq!(loaded.width(), 6);
        assert_eq!(loaded.frame_pixels(0).unwrap()[1 * 6 + 1], 1);
        assert_eq!(loaded.frame_delay(1).unwrap(), 80);

        document.undo().unwrap();
        assert_eq!((document.width(), document.height()), (4, 2));
        assert_eq!(document.frame_pixels(0).unwrap()[0], 1);
        assert_eq!(document.frame_pixels(0).unwrap()[7], 2);
        assert_eq!(document.frame_pixels(1).unwrap()[1], 3);
        document.redo().unwrap();
        assert_eq!(document.width(), 6);

        document.resize_canvas(6, 4, 0, 0).unwrap();
        assert!(document.can_undo());
        assert!(!document.can_redo());
        document.undo().unwrap();
        assert_eq!(document.width(), 4);
        assert!(!document.can_undo());

        document.resize_canvas(3, 2, 0, 0).unwrap();
        assert_eq!(document.frame_pixels(0).unwrap(), &[1, 0, 0, 0, 0, 0]);
        document.undo().unwrap();
        assert_eq!(document.frame_pixels(0).unwrap()[7], 2);

        document.resize_canvas(4, 2, -1, 0).unwrap();
        assert_eq!(document.frame_pixels(0).unwrap()[0], 0);
        assert_eq!(document.frame_pixels(0).unwrap()[6], 2);
        assert!(matches!(
            document.resize_canvas(0, 2, 0, 0),
            Err(KernelError::InvalidSize { width: 0, .. })
        ));
        assert_eq!(document.width(), 4);
    }

    #[test]
    fn frames_do_not_share_pixels() {
        let mut document = doc();
        document.paint(1, 1, 2).unwrap();
        let added = document.add_frame();
        assert_eq!(added, 1);
        assert_eq!(document.active_frame(), 0);

        document.set_active_frame(1).unwrap();
        assert!(document
            .pixels()
            .iter()
            .all(|index| *index == TRANSPARENT_INDEX));
        document.paint(1, 1, 3).unwrap();

        document.set_active_frame(0).unwrap();
        assert_eq!(document.get(1, 1).unwrap(), 2);
        assert_eq!(document.frame_pixels(1).unwrap()[1 + 16], 3);
    }

    #[test]
    fn duplicate_copies_pixels_and_keeps_active_content() {
        let mut document = doc();
        document.paint(0, 0, 1).unwrap();
        document.add_frame();
        document.set_active_frame(1).unwrap();

        let copy = document.duplicate_frame(0).unwrap();
        assert_eq!(copy, 1);
        assert_eq!(document.frame_count(), 3);
        assert_eq!(document.active_frame(), 2);
        assert_eq!(document.frame_pixels(1).unwrap()[0], 1);

        document.set_active_frame(0).unwrap();
        document.duplicate_frame(0).unwrap();
        assert_eq!(document.active_frame(), 0);
    }

    #[test]
    fn delete_frame_keeps_one_frame_and_shifts_active() {
        let mut document = doc();
        document.paint(0, 0, 1).unwrap();
        document.add_frame();
        document.add_frame();
        document.set_active_frame(2).unwrap();

        document.delete_frame(0).unwrap();
        assert_eq!(document.frame_count(), 2);
        assert_eq!(document.active_frame(), 1);

        document.delete_frame(1).unwrap();
        assert_eq!(document.frame_count(), 1);
        assert_eq!(document.active_frame(), 0);
        assert_eq!(document.pixels()[0], TRANSPARENT_INDEX);
        assert_eq!(
            document.delete_frame(0).unwrap_err(),
            KernelError::LastFrame
        );
        assert_eq!(document.frame_count(), 1);
    }

    #[test]
    fn rejects_unknown_frame_index() {
        let mut document = doc();
        document.add_frame();
        assert!(matches!(
            document.delete_frame(4),
            Err(KernelError::FrameIndex { index: 4, len: 2 })
        ));
        assert!(matches!(
            document.frame_pixels(3),
            Err(KernelError::FrameIndex { index: 3, len: 2 })
        ));
        assert!(matches!(
            document.set_active_frame(3),
            Err(KernelError::FrameIndex { index: 3, len: 2 })
        ));
        assert!(matches!(
            document.duplicate_frame(2),
            Err(KernelError::FrameIndex { index: 2, len: 2 })
        ));
    }

    #[test]
    fn frame_delay_undo_redo_and_duplicate() {
        let mut document = doc();
        assert_eq!(document.frame_delay(0).unwrap(), DEFAULT_FRAME_DELAY_MS);
        assert_eq!(document.duration_ms(), u64::from(DEFAULT_FRAME_DELAY_MS));

        document.set_frame_delay(0, 250).unwrap();
        document.set_frame_delay(0, 250).unwrap();
        assert_eq!(document.frame_delay(0).unwrap(), 250);

        let copy = document.duplicate_frame(0).unwrap();
        assert_eq!(copy, 1);
        assert_eq!(document.frame_delay(1).unwrap(), 250);
        assert_eq!(document.frame_delay(0).unwrap(), 250);

        document.undo().unwrap();
        assert_eq!(document.frame_count(), 1);
        document.undo().unwrap();
        assert_eq!(document.frame_delay(0).unwrap(), DEFAULT_FRAME_DELAY_MS);
        assert!(!document.can_undo());

        document.redo().unwrap();
        assert_eq!(document.frame_delay(0).unwrap(), 250);
        document.redo().unwrap();
        assert_eq!(document.frame_count(), 2);
        assert_eq!(document.frame_delay(1).unwrap(), 250);

        assert_eq!(
            document.set_frame_delay(0, 0).unwrap_err(),
            KernelError::FrameDelay { delay_ms: 0 }
        );
        assert_eq!(
            document
                .set_frame_delay(0, MAX_FRAME_DELAY_MS + 1)
                .unwrap_err(),
            KernelError::FrameDelay {
                delay_ms: MAX_FRAME_DELAY_MS + 1
            }
        );
        assert_eq!(document.frame_delay(0).unwrap(), 250);
        assert!(matches!(
            document.set_frame_delay(4, 20),
            Err(KernelError::FrameIndex { index: 4, len: 2 })
        ));
    }

    #[test]
    fn delay_gesture_is_one_step_and_a_return_records_nothing() {
        let mut document = doc();
        document.begin_gesture().unwrap();
        document.set_frame_delay(0, 120).unwrap();
        document.set_frame_delay(0, 180).unwrap();
        document.end_gesture();

        document.undo().unwrap();
        assert_eq!(document.frame_delay(0).unwrap(), DEFAULT_FRAME_DELAY_MS);
        document.redo().unwrap();
        assert_eq!(document.frame_delay(0).unwrap(), 180);

        document.undo().unwrap();
        assert!(document.can_redo());
        document.begin_gesture().unwrap();
        document.set_frame_delay(0, 150).unwrap();
        document.set_frame_delay(0, DEFAULT_FRAME_DELAY_MS).unwrap();
        document.end_gesture();
        assert!(!document.can_undo());
        assert!(document.can_redo());
        assert_eq!(document.frame_delay(0).unwrap(), DEFAULT_FRAME_DELAY_MS);
    }

    #[test]
    fn scrub_uses_delays_and_one_drag_is_one_undo() {
        let mut document = doc();
        document.add_frame();
        document.add_frame();
        document.set_frame_delay(1, 300).unwrap();
        document.clear_history();
        assert_eq!(document.frame_at_ratio(0), 0);
        assert_eq!(document.frame_at_ratio(198_000), 0);
        assert_eq!(document.frame_at_ratio(200_000), 1);
        assert_eq!(document.frame_at_ratio(800_000), 2);
        assert_eq!(document.frame_at_ratio(TIMELINE_RATIO_PPM), 2);

        document.play();
        assert!(document.playing());
        document.begin_gesture().unwrap();
        document.scrub(0).unwrap();
        document.scrub(250_000).unwrap();
        document.scrub(900_000).unwrap();
        document.end_gesture();

        assert!(!document.playing());
        assert_eq!(document.active_frame(), 2);
        document.undo().unwrap();
        assert_eq!(document.active_frame(), 0);
        assert!(!document.can_undo());
    }

    #[test]
    fn playback_loops_on_each_frame_delay_without_recording_undo() {
        let mut document = doc();
        document.add_frame();
        document.set_frame_delay(0, 100).unwrap();
        document.set_frame_delay(1, 40).unwrap();
        document.clear_history();

        assert!(!document.playing());
        document.play();
        assert!(document.playing());
        document.play();
        assert!(!document.tick(0));
        assert!(!document.tick(99));
        assert_eq!(document.active_frame(), 0);
        assert!(document.tick(1));
        assert_eq!(document.active_frame(), 1);
        assert!(document.tick(40));
        assert_eq!(document.active_frame(), 0);
        assert!(!document.can_undo());
        assert!(!document.can_redo());

        assert!(document.tick(120));
        assert_eq!(document.active_frame(), 1);
        assert!(document.tick(20));
        assert_eq!(document.active_frame(), 0);

        document.stop();
        assert!(!document.playing());
        assert!(!document.tick(500));
        assert_eq!(document.active_frame(), 0);

        document.set_active_frame(1).unwrap();
        document.play();
        assert!(!document.tick(10));
        assert_eq!(document.active_frame(), 1);
        document.undo().unwrap();
        assert!(!document.playing());
        assert_eq!(document.active_frame(), 0);
        assert!(!document.can_undo());
    }

    #[test]
    fn paint_stores_indices_and_rgba_views_follow_the_palette() {
        let mut document = doc();
        document.paint(1, 2, 4).unwrap();
        let offset = (2 * 16 + 1) * 4;
        assert_eq!(document.pixels()[2 * 16 + 1], 4);
        let color = document.palette().get(4).unwrap();
        let rgba = document.frame_rgba(0).unwrap();
        assert_eq!(
            &rgba[offset..offset + 4],
            &[color.r, color.g, color.b, color.a]
        );

        document.set_palette_color(4, 9, 8, 7).unwrap();
        assert_eq!(document.pixels()[2 * 16 + 1], 4);
        let rgba = document.frame_rgba(0).unwrap();
        assert_eq!(&rgba[offset..offset + 4], &[9, 8, 7, 255]);
        let png = document.export_frame_png(0).unwrap();
        assert_ne!(png, {
            let mut restored = document.clone();
            restored.undo().unwrap();
            restored.export_frame_png(0).unwrap()
        });
    }

    #[test]
    fn lock_blocks_edits_and_imports_but_not_paint_or_history() {
        let mut document = doc();
        let original = document.palette().get(1).unwrap();
        document.set_palette_locked(false);
        assert!(!document.can_undo());

        document.set_palette_color(1, 9, 8, 7).unwrap();
        document.set_palette_locked(true);
        document.set_palette_locked(true);
        assert!(document.palette_locked());

        assert_eq!(
            document.set_palette_color(1, 1, 2, 3).unwrap_err(),
            KernelError::PaletteLocked
        );
        assert_eq!(
            document.add_palette_color(4, 5, 6).unwrap_err(),
            KernelError::PaletteLocked
        );
        assert_eq!(
            document
                .import_palette("ff0000\n", PaletteImportMode::Replace)
                .unwrap_err(),
            KernelError::PaletteLocked
        );
        assert_eq!(document.palette().get(1), Some(Rgba::rgb(9, 8, 7)));
        assert_eq!(document.palette().len(), 9);

        document.paint(3, 4, 2).unwrap();
        assert_eq!(document.get(3, 4).unwrap(), 2);

        document.undo().unwrap();
        assert_eq!(document.get(3, 4).unwrap(), TRANSPARENT_INDEX);
        assert!(document.palette_locked());
        document.undo().unwrap();
        assert!(!document.palette_locked());
        assert_eq!(document.palette().get(1), Some(Rgba::rgb(9, 8, 7)));
        document.undo().unwrap();
        assert_eq!(document.palette().get(1), Some(original));
        assert!(!document.can_undo());

        document.redo().unwrap();
        assert_eq!(document.palette().get(1), Some(Rgba::rgb(9, 8, 7)));
        document.redo().unwrap();
        assert!(document.palette_locked());
        document.redo().unwrap();
        assert_eq!(document.get(3, 4).unwrap(), 2);
        assert_eq!(
            document.set_palette_color(1, 1, 2, 3).unwrap_err(),
            KernelError::PaletteLocked
        );
    }

    #[test]
    fn replace_import_remaps_orphans_and_undoes_in_one_step() {
        let mut document = doc();
        let before = document.palette().colors().to_vec();
        document.paint(0, 0, 1).unwrap();
        document.paint(1, 0, 8).unwrap();
        document.add_frame();
        document.set_active_frame(1).unwrap();
        document.paint(2, 2, 8).unwrap();

        document
            .import_palette("aa bb cc\n#ddee00 Name\n", PaletteImportMode::Replace)
            .unwrap_err();
        document
            .import_palette("#aabbcc\n#ddee00 Name\n", PaletteImportMode::Replace)
            .unwrap();
        assert_eq!(document.palette().len(), 3);
        assert_eq!(document.palette().get(0), Some(Rgba::TRANSPARENT));
        assert_eq!(document.palette().get(1), Some(Rgba::rgb(0xaa, 0xbb, 0xcc)));
        assert_eq!(document.palette().get(2), Some(Rgba::rgb(0xdd, 0xee, 0)));
        assert_eq!(document.frame_pixels(0).unwrap()[0], 1);
        assert_eq!(document.frame_pixels(0).unwrap()[1], TRANSPARENT_INDEX);
        assert_eq!(document.get(2, 2).unwrap(), TRANSPARENT_INDEX);

        document
            .import_palette("aabbcc\nddee00\n", PaletteImportMode::Replace)
            .unwrap();
        document.undo().unwrap();
        assert_eq!(document.palette().colors(), before.as_slice());
        assert_eq!(document.frame_pixels(0).unwrap()[0], 1);
        assert_eq!(document.frame_pixels(0).unwrap()[1], 8);
        assert_eq!(document.frame_pixels(1).unwrap()[2 + 2 * 16], 8);
        assert!(document.can_undo());

        document.redo().unwrap();
        assert_eq!(document.palette().len(), 3);
        assert_eq!(document.get(2, 2).unwrap(), TRANSPARENT_INDEX);
    }

    #[test]
    fn merge_import_skips_duplicates_and_groups_appends() {
        let mut document = doc();
        let ink = document.palette().get(1).unwrap();
        let hex = format!("{:02x}{:02x}{:02x}", ink.r, ink.g, ink.b);
        document
            .import_palette(
                &format!("{hex}\nff0000\nff0000\n"),
                PaletteImportMode::Merge,
            )
            .unwrap();
        assert_eq!(document.palette().len(), 10);
        assert_eq!(document.palette().get(9), Some(Rgba::rgb(255, 0, 0)));

        document.undo().unwrap();
        assert_eq!(document.palette().len(), 9);
        assert!(!document.can_undo());

        document
            .import_palette(&hex, PaletteImportMode::Merge)
            .unwrap();
        assert!(!document.can_undo());
        assert_eq!(document.palette().len(), 9);

        let mut overflow = String::new();
        for index in 0..248 {
            overflow.push_str(&format!("{:02x}{:02x}00\n", index, 255 - index));
        }
        assert_eq!(
            document
                .import_palette(&overflow, PaletteImportMode::Merge)
                .unwrap_err(),
            KernelError::PaletteTooLarge { len: 257 }
        );
        assert_eq!(document.palette().len(), 9);
        assert!(!document.can_undo());
    }

    #[test]
    fn import_rejects_a_bad_file_without_changing_pixels() {
        let mut document = doc();
        document.paint(0, 0, 3).unwrap();
        document.clear_history();
        assert_eq!(
            document
                .import_palette("GIMP Palette\n256 0 0\n", PaletteImportMode::Merge)
                .unwrap_err(),
            KernelError::PaletteFile
        );
        assert_eq!(
            document
                .import_palette("GIMP Palette\n#\n", PaletteImportMode::Replace)
                .unwrap_err(),
            KernelError::EmptyPalette
        );
        assert_eq!(document.get(0, 0).unwrap(), 3);
        assert_eq!(document.palette().len(), 9);
        assert!(!document.can_undo());
    }

    #[test]
    fn gpl_replace_and_a_gesture_share_one_undo_step() {
        let mut document = doc();
        let gpl = "GIMP Palette\nName: Demo\nColumns: 1\n#\n255 0 0 Red\n0 128 255 Blue\n";
        document.begin_gesture().unwrap();
        document
            .import_palette(gpl, PaletteImportMode::Replace)
            .unwrap();
        document.paint(4, 4, 2).unwrap();
        document.end_gesture();

        assert_eq!(document.palette().len(), 3);
        assert_eq!(document.get(4, 4).unwrap(), 2);
        document.undo().unwrap();
        assert_eq!(document.palette().len(), 9);
        assert_eq!(document.get(4, 4).unwrap(), TRANSPARENT_INDEX);
        assert!(!document.can_undo());
    }
}
