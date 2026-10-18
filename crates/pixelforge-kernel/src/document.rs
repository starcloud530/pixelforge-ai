use std::cmp::Ordering;

use crate::error::KernelError;
use crate::frame::Frame;
use crate::palette::Palette;
use crate::Rgba;
use crate::MAX_CANVAS_EDGE;
use crate::TRANSPARENT_INDEX;

/// Indexed pixel document: size, palette, and one or more frames.
///
/// UI gestures and, later, Tool API calls are meant to meet here. This
/// skeleton implements paint, erase, and minimal frame-list edits. It does
/// not record undo, save a project file, or export PNG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    width: u16,
    height: u16,
    palette: Palette,
    frames: Vec<Frame>,
    active: usize,
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
            frames: vec![Frame::blank(len)],
            active: 0,
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
    pub fn paint(&mut self, x: u16, y: u16, color_index: u8) -> Result<(), KernelError> {
        if !self.palette.contains_paint_index(color_index) {
            return Err(KernelError::PaletteIndex { index: color_index });
        }
        let offset = self.offset(x, y)?;
        self.frames[self.active].set(offset, color_index);
        Ok(())
    }

    /// Sets one cell back to [`TRANSPARENT_INDEX`].
    pub fn erase(&mut self, x: u16, y: u16) -> Result<(), KernelError> {
        let offset = self.offset(x, y)?;
        self.frames[self.active].set(offset, TRANSPARENT_INDEX);
        Ok(())
    }

    /// Appends a blank frame and returns its index. The active frame stays put.
    ///
    /// This is a kernel primitive for issue #5, not the editor frame list.
    pub fn add_frame(&mut self) -> usize {
        self.frames.push(Frame::blank(self.pixel_count()));
        self.frames.len() - 1
    }

    /// Inserts a copy after `index`. Active frame still names the same content.
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
        self.frames.insert(inserted, frame);
        if self.active >= inserted {
            self.active += 1;
        }
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
        self.frames.remove(index);
        self.active = match self.active.cmp(&index) {
            Ordering::Greater => self.active - 1,
            Ordering::Equal => index.min(self.frames.len() - 1),
            Ordering::Less => self.active,
        };
        Ok(())
    }

    pub fn set_active_frame(&mut self, index: usize) -> Result<(), KernelError> {
        if index >= self.frames.len() {
            return Err(KernelError::FrameIndex {
                index,
                len: self.frames.len(),
            });
        }
        self.active = index;
        Ok(())
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
}
