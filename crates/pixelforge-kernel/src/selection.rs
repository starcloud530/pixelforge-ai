//! Axis-aligned selection masks.
//!
//! A mask is a set of rectangles, not a lasso and not a magic wand. A missing
//! selection means every cell is writable. An empty mask protects every cell.
//! The project file stores one rectangle as `{x,y,width,height}` and several
//! rectangles, or an empty mask, as `{rects:[...]}`. It omits the field when
//! the selection is clear.

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

/// How two masks combine. The result replaces the stored selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskOp {
    Union,
    Intersect,
    Subtract,
}

impl MaskOp {
    /// `union`, `intersect`, or `subtract`.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "union" => Some(Self::Union),
            "intersect" => Some(Self::Intersect),
            "subtract" => Some(Self::Subtract),
            _ => None,
        }
    }

    /// The Tool API spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Union => "union",
            Self::Intersect => "intersect",
            Self::Subtract => "subtract",
        }
    }
}

/// Disjoint axis-aligned rectangles covering the writable cells.
///
/// Rectangles are sorted by `y`, then `x`, and merged when they share an
/// edge and form one rectangle. This is the stored selection after a boolean
/// operation. It is not a freeform bitmap and not a lasso.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Mask {
    rects: Vec<Selection>,
}

impl Mask {
    /// A mask that covers `rect` and nothing else.
    pub fn from_rect(rect: Selection) -> Self {
        Self { rects: vec![rect] }
    }

    /// Unions `rects` into one canonical mask.
    ///
    /// A zero width or height is an error. Overlaps and shared edges merge.
    pub fn try_from_rects(rects: impl IntoIterator<Item = Selection>) -> Result<Self, ()> {
        let mut mask = Self { rects: Vec::new() };
        for rect in rects {
            if rect.width == 0 || rect.height == 0 {
                return Err(());
            }
            mask = mask.union(&Self::from_rect(rect));
        }
        Ok(mask)
    }

    /// The rectangles, in row-major order. Empty when the mask protects every cell.
    pub fn rects(&self) -> &[Selection] {
        &self.rects
    }

    /// True when no cell is writable.
    pub fn is_empty(&self) -> bool {
        self.rects.is_empty()
    }

    /// The rectangle when this mask is exactly one of them.
    pub fn as_rect(&self) -> Option<Selection> {
        if self.rects.len() == 1 {
            Some(self.rects[0])
        } else {
            None
        }
    }

    /// How many cells this mask covers. Rectangles do not overlap.
    pub fn cell_count(&self) -> u32 {
        self.rects
            .iter()
            .map(|rect| u32::from(rect.width) * u32::from(rect.height))
            .sum()
    }

    /// True when the cell is inside some rectangle.
    pub fn contains(&self, x: u16, y: u16) -> bool {
        self.rects.iter().any(|rect| rect.contains(x, y))
    }

    /// True when every cell of the rectangle is inside this mask.
    ///
    /// A single-rectangle mask uses the same corner test as a stored selection.
    /// A hole, or a gap between rectangles, makes a covering rectangle fail.
    pub fn covers_rect(&self, x: u16, y: u16, width: u16, height: u16) -> bool {
        if width == 0 || height == 0 {
            return false;
        }
        let Some(x1) = x.checked_add(width - 1) else {
            return false;
        };
        let Some(y1) = y.checked_add(height - 1) else {
            return false;
        };
        if let Some(rect) = self.as_rect() {
            let Some(rect_x1) = rect.x.checked_add(rect.width - 1) else {
                return false;
            };
            let Some(rect_y1) = rect.y.checked_add(rect.height - 1) else {
                return false;
            };
            return x >= rect.x && y >= rect.y && x1 <= rect_x1 && y1 <= rect_y1;
        }
        for y in y..=y1 {
            for x in x..=x1 {
                if !self.contains(x, y) {
                    return false;
                }
            }
        }
        true
    }

    /// True when every rectangle lies on a canvas of `width` by `height`.
    pub fn fits(&self, width: u16, height: u16) -> bool {
        self.rects.iter().all(|rect| rect.fits(width, height))
    }

    /// Moves every rectangle by a canvas-resize offset.
    ///
    /// `None` when any rectangle would not lie entirely on the new canvas.
    /// A partial rectangle is not stored, and neither is the rest of the mask.
    pub fn translated(
        &self,
        offset_x: i32,
        offset_y: i32,
        width: u16,
        height: u16,
    ) -> Option<Self> {
        let mut rects = Vec::with_capacity(self.rects.len());
        for rect in &self.rects {
            rects.push(rect.translated(offset_x, offset_y, width, height)?);
        }
        Some(Self { rects })
    }

    /// Cells in either mask.
    pub fn union(&self, other: &Self) -> Self {
        boolean(self, other, MaskOp::Union)
    }

    /// Cells in both masks.
    pub fn intersect(&self, other: &Self) -> Self {
        boolean(self, other, MaskOp::Intersect)
    }

    /// Cells in `self` and not in `other`.
    pub fn subtract(&self, other: &Self) -> Self {
        boolean(self, other, MaskOp::Subtract)
    }
}

fn boolean(left: &Mask, right: &Mask, op: MaskOp) -> Mask {
    if left.rects.is_empty() && matches!(op, MaskOp::Union) {
        return right.clone();
    }
    if right.rects.is_empty() && matches!(op, MaskOp::Union | MaskOp::Subtract) {
        return left.clone();
    }
    if left.rects.is_empty() || (right.rects.is_empty() && matches!(op, MaskOp::Intersect)) {
        return Mask::default();
    }
    let Some((origin_x, origin_y, max_x, max_y)) = mask_bounds([left, right]) else {
        return Mask::default();
    };
    let width = usize::from(max_x - origin_x) + 1;
    let height = usize::from(max_y - origin_y) + 1;
    assert!(
        width <= 4096 && height <= 4096,
        "mask boolean stays inside a canvas"
    );
    let mut cells = vec![false; width * height];
    match op {
        MaskOp::Union => {
            stamp(&mut cells, width, origin_x, origin_y, left);
            stamp(&mut cells, width, origin_x, origin_y, right);
        }
        MaskOp::Intersect => {
            stamp(&mut cells, width, origin_x, origin_y, left);
            let mut other = vec![false; width * height];
            stamp(&mut other, width, origin_x, origin_y, right);
            for (cell, keep) in cells.iter_mut().zip(other) {
                *cell &= keep;
            }
        }
        MaskOp::Subtract => {
            stamp(&mut cells, width, origin_x, origin_y, left);
            let mut cut = vec![false; width * height];
            stamp(&mut cut, width, origin_x, origin_y, right);
            for (cell, drop) in cells.iter_mut().zip(cut) {
                if drop {
                    *cell = false;
                }
            }
        }
    }
    extract(cells, width, height, origin_x, origin_y)
}

fn mask_bounds(masks: [&Mask; 2]) -> Option<(u16, u16, u16, u16)> {
    let mut min_x = u16::MAX;
    let mut min_y = u16::MAX;
    let mut max_x = 0u16;
    let mut max_y = 0u16;
    let mut any = false;
    for mask in masks {
        for rect in &mask.rects {
            any = true;
            min_x = min_x.min(rect.x);
            min_y = min_y.min(rect.y);
            max_x = max_x.max(rect.x + rect.width - 1);
            max_y = max_y.max(rect.y + rect.height - 1);
        }
    }
    any.then_some((min_x, min_y, max_x, max_y))
}

fn stamp(cells: &mut [bool], width: usize, origin_x: u16, origin_y: u16, mask: &Mask) {
    for rect in &mask.rects {
        for y in rect.y..rect.y + rect.height {
            for x in rect.x..rect.x + rect.width {
                let col = usize::from(x - origin_x);
                let row = usize::from(y - origin_y);
                cells[row * width + col] = true;
            }
        }
    }
}

/// Greedy row-major rectangles. The same cells always produce the same list.
fn extract(
    mut cells: Vec<bool>,
    width: usize,
    height: usize,
    origin_x: u16,
    origin_y: u16,
) -> Mask {
    let mut rects = Vec::new();
    for y in 0..height {
        let mut x = 0;
        while x < width {
            if !cells[y * width + x] {
                x += 1;
                continue;
            }
            let mut x1 = x + 1;
            while x1 < width && cells[y * width + x1] {
                x1 += 1;
            }
            let mut y1 = y + 1;
            while y1 < height && (x..x1).all(|col| cells[y1 * width + col]) {
                y1 += 1;
            }
            for row in y..y1 {
                for col in x..x1 {
                    cells[row * width + col] = false;
                }
            }
            rects.push(Selection {
                x: origin_x + u16::try_from(x).expect("mask column fits in u16"),
                y: origin_y + u16::try_from(y).expect("mask row fits in u16"),
                width: u16::try_from(x1 - x).expect("mask width fits in u16"),
                height: u16::try_from(y1 - y).expect("mask height fits in u16"),
            });
            x = x1;
        }
    }
    Mask { rects }
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

    fn rect(x: u16, y: u16, width: u16, height: u16) -> Selection {
        Selection {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn union_merges_adjacent_rects_and_subtract_keeps_the_hole() {
        let body = Mask::from_rect(rect(0, 0, 4, 4));
        let beside = Mask::from_rect(rect(4, 0, 2, 4));
        let joined = body.union(&beside);
        assert_eq!(joined.as_rect(), Some(rect(0, 0, 6, 4)));

        let feet = Mask::from_rect(rect(1, 3, 2, 1));
        let cut = body.subtract(&feet);
        assert!(cut.as_rect().is_none());
        assert_eq!(cut.cell_count(), 14);
        assert!(cut.contains(0, 3));
        assert!(!cut.contains(1, 3));
        assert!(!cut.contains(2, 3));
        assert!(cut.contains(3, 0));
        assert!(!cut.covers_rect(0, 0, 4, 4));
        assert!(cut.covers_rect(0, 0, 4, 1));

        let overlap = body.intersect(&Mask::from_rect(rect(2, 2, 4, 4)));
        assert_eq!(overlap.as_rect(), Some(rect(2, 2, 2, 2)));
        assert!(body.intersect(&beside).is_empty());
        assert!(Mask::default().union(&feet).as_rect() == Some(rect(1, 3, 2, 1)));
    }
}
