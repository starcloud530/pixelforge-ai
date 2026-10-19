use std::cmp::Ordering;

use crate::error::KernelError;
use crate::frame::Frame;
use crate::history::{Change, History};
use crate::palette::Palette;
use crate::Rgba;
use crate::DEFAULT_FRAME_DELAY_MS;
use crate::MAX_CANVAS_EDGE;
use crate::MAX_FRAME_DELAY_MS;
use crate::MIN_FRAME_DELAY_MS;
use crate::TIMELINE_RATIO_PPM;
use crate::TRANSPARENT_INDEX;

/// Indexed pixel document: size, palette, frames, delays, and the undo stack.
///
/// UI gestures and, later, Tool API calls meet here. Paint, erase, palette
/// edits, frame edits, and delay edits record kernel operations on this
/// document. Undo restores those operations; it does not patch a buffer owned
/// by the shell. Loop playback is preview state: [`Self::tick`] moves the
/// active frame from each frame's delay and does not push undo entries.
/// Project open and save encode pixels, palette, frames, and delays, not the
/// undo stack or the playback clock. PNG export is not implemented.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    width: u16,
    height: u16,
    palette: Palette,
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
    /// A cell that already holds `color_index` is left alone and is not recorded.
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
        let index = self.palette.push_rgb(r, g, b)?;
        self.history.record(Change::AddPaletteColor {
            index,
            color: Rgba::rgb(r, g, b),
        });
        Ok(index)
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

    /// Inserts a copy after `index`. Active frame still names the same content
    /// when it sat before the insertion point; later frames shift right.
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
            frames,
            active,
            history: History::new(),
            playback: Playback::stopped(),
        }
    }

    fn write_index(&mut self, frame: usize, offset: usize, value: u8) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DEFAULT_FRAME_DELAY_MS, MAX_FRAME_DELAY_MS, TIMELINE_RATIO_PPM};

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
}
