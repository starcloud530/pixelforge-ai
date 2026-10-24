//! One axis-aligned rectangle in canvas cells.
//!
//! This is the only mask the kernel stores. It is not a lasso and not a
//! magic wand. A missing selection means every cell is writable. The project
//! file stores the rectangle and omits the field when it is clear.

/// Inclusive origin and size, in cells. The rectangle lies on the canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Selection {
    /// True when every cell of this rectangle lies on a canvas of `width` by `height`.
    pub fn fits(self, width: u16, height: u16) -> bool {
        if self.width == 0 || self.height == 0 {
            return false;
        }
        let Some(x1) = self.x.checked_add(self.width - 1) else {
            return false;
        };
        let Some(y1) = self.y.checked_add(self.height - 1) else {
            return false;
        };
        self.x < width && x1 < width && self.y < height && y1 < height
    }

    /// True when the cell is inside this rectangle.
    pub fn contains(self, x: u16, y: u16) -> bool {
        if self.width == 0 || self.height == 0 {
            return false;
        }
        let Some(x1) = self.x.checked_add(self.width - 1) else {
            return false;
        };
        let Some(y1) = self.y.checked_add(self.height - 1) else {
            return false;
        };
        x >= self.x && x <= x1 && y >= self.y && y <= y1
    }

    /// Moves the rectangle by the same offset a canvas resize applies to pixels.
    ///
    /// `None` when the moved rectangle would not lie entirely on the new canvas.
    /// A partial rectangle is not stored.
    pub fn translated(self, offset_x: i32, offset_y: i32, width: u16, height: u16) -> Option<Self> {
        let x = i32::from(self.x).checked_add(offset_x)?;
        let y = i32::from(self.y).checked_add(offset_y)?;
        let x = u16::try_from(x).ok()?;
        let y = u16::try_from(y).ok()?;
        let next = Self {
            x,
            y,
            width: self.width,
            height: self.height,
        };
        next.fits(width, height).then_some(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translated_moves_with_the_canvas_and_drops_a_rect_that_falls_off() {
        let helmet = Selection {
            x: 1,
            y: 1,
            width: 2,
            height: 2,
        };
        assert_eq!(
            helmet.translated(1, 2, 8, 8),
            Some(Selection {
                x: 2,
                y: 3,
                width: 2,
                height: 2,
            })
        );
        assert_eq!(helmet.translated(0, 0, 2, 2), None);
        assert!(helmet.fits(4, 4));
        assert!(helmet.contains(2, 2));
        assert!(!helmet.contains(3, 1));
    }
}
