//! Compact scene facts and dirty-region geometry.
//!
//! Cells stay palette indices. This module does not draw, does not hash a
//! command log. Layer opacity is a composite property, not a new index.
//! A dirty box is the
//! cells whose stored index changed, not the rectangle a tool was asked to
//! touch.

use crate::Rgba;

/// FNV-1a 64-bit offset basis.
const FNV_OFFSET: u64 = 0xcbf29ce484222325;
/// FNV-1a 64-bit prime.
const FNV_PRIME: u64 = 0x100000001b3;

/// Inclusive top-left and size, in cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CellBounds {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

/// One cell whose index differs between two views.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CellChange {
    pub x: u16,
    pub y: u16,
    pub left: u8,
    pub right: u8,
}

/// How many cells of a view use each palette index. Index order, unused omitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ViewSummary {
    pub ink: usize,
    pub bounds: Option<CellBounds>,
    pub indices: Vec<(u8, u32)>,
}

/// Index comparison of two same-sized buffers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiffSummary {
    pub bounds: Option<CellBounds>,
    pub same: usize,
    pub differ: usize,
    pub only_frame: usize,
    pub only_with: usize,
    pub changes: Vec<CellChange>,
}

/// Cells on one layer whose index changed, measured on the canvas after the edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DirtyRegion {
    pub frame: usize,
    pub layer: usize,
    pub changed: usize,
    /// Non-zero cells that sat on an `x,y` the new canvas does not have.
    pub clipped: usize,
    pub bounds: Option<CellBounds>,
}

/// Lowercase hex of FNV-1a 64 over `r, g, b, a` of each entry, index order.
///
/// Offset basis `14695981039346656037`, prime `1099511628211`. The same
/// colors in the same order always hash the same. This is an identity, not
/// a secret.
pub(crate) fn palette_hash(colors: &[Rgba]) -> String {
    let mut hash = FNV_OFFSET;
    for color in colors {
        for byte in [color.r, color.g, color.b, color.a] {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    format!("{hash:016x}")
}

/// Ink count, tight bounds of non-zero cells, and a count per used index.
pub(crate) fn summarize_view(pixels: &[u8], width: u16) -> ViewSummary {
    summarize_view_where(pixels, width, |_, _| true)
}

/// [`summarize_view`] counting only cells `include` accepts.
///
/// Cells outside the mask are not ink and are not an index count, including
/// index 0. The caller reports them as protected.
pub(crate) fn summarize_view_where(
    pixels: &[u8],
    width: u16,
    include: impl Fn(u16, u16) -> bool,
) -> ViewSummary {
    let mut counts = [0u32; 256];
    let mut ink = 0usize;
    let mut ink_cells = Vec::new();
    let stride = usize::from(width);
    for (offset, index) in pixels.iter().copied().enumerate() {
        let (x, y) = cell_at(offset, stride);
        if !include(x, y) {
            continue;
        }
        counts[usize::from(index)] = counts[usize::from(index)].saturating_add(1);
        if index != 0 {
            ink += 1;
            ink_cells.push((x, y));
        }
    }
    let indices = counts
        .into_iter()
        .enumerate()
        .filter(|(_, count)| *count > 0)
        .map(|(index, count)| (u8::try_from(index).expect("index fits in u8"), count))
        .collect();
    ViewSummary {
        ink,
        bounds: bounds_of(&ink_cells),
        indices,
    }
}

/// `.` / `!` / only-ink counts, plus the indices of every cell that differs.
pub(crate) fn summarize_diff(left: &[u8], right: &[u8], width: u16) -> DiffSummary {
    summarize_diff_where(left, right, width, |_, _| true)
}

/// [`summarize_diff`] counting only cells `include` accepts.
///
/// Cells outside the mask are not `same` and not a change. The caller reports
/// them as protected.
pub(crate) fn summarize_diff_where(
    left: &[u8],
    right: &[u8],
    width: u16,
    include: impl Fn(u16, u16) -> bool,
) -> DiffSummary {
    let stride = usize::from(width);
    let mut same = 0usize;
    let mut differ = 0usize;
    let mut only_frame = 0usize;
    let mut only_with = 0usize;
    let mut changed_at = Vec::new();
    let mut changes = Vec::new();
    for (offset, (left, right)) in left.iter().copied().zip(right.iter().copied()).enumerate() {
        let (x, y) = cell_at(offset, stride);
        if !include(x, y) {
            continue;
        }
        if left == right {
            same += 1;
            continue;
        }
        if left != 0 && right != 0 {
            differ += 1;
        } else if right == 0 {
            only_frame += 1;
        } else {
            only_with += 1;
        }
        changed_at.push((x, y));
        changes.push(CellChange { x, y, left, right });
    }
    DiffSummary {
        bounds: bounds_of(&changed_at),
        same,
        differ,
        only_frame,
        only_with,
        changes,
    }
}

/// Compare one layer before and after an edit.
///
/// Same coordinates are compared directly. A cell that exists only on the new
/// canvas is changed when its index is not 0. A non-zero cell that exists only
/// on the old canvas increments `clipped` and is not part of `bounds`, because
/// that coordinate is gone.
pub(crate) fn dirty_region(
    frame: usize,
    layer: usize,
    before: &[u8],
    old_width: u16,
    old_height: u16,
    after: &[u8],
    new_width: u16,
    new_height: u16,
) -> DirtyRegion {
    let mut changed_at = Vec::new();
    for y in 0..new_height {
        for x in 0..new_width {
            let new_index = after[offset(new_width, x, y)];
            let old_index = if x < old_width && y < old_height {
                before[offset(old_width, x, y)]
            } else {
                0
            };
            if new_index != old_index {
                changed_at.push((x, y));
            }
        }
    }
    let mut clipped = 0usize;
    for y in 0..old_height {
        for x in 0..old_width {
            if x >= new_width || y >= new_height {
                let old_index = before[offset(old_width, x, y)];
                if old_index != 0 {
                    clipped += 1;
                }
            }
        }
    }
    DirtyRegion {
        frame,
        layer,
        changed: changed_at.len(),
        clipped,
        bounds: bounds_of(&changed_at),
    }
}

pub(crate) fn union_bounds(boxes: impl IntoIterator<Item = CellBounds>) -> Option<CellBounds> {
    let mut boxes = boxes.into_iter();
    let Some(first) = boxes.next() else {
        return None;
    };
    let mut x0 = first.x;
    let mut y0 = first.y;
    let mut x1 = first.x + first.width;
    let mut y1 = first.y + first.height;
    for next in boxes {
        x0 = x0.min(next.x);
        y0 = y0.min(next.y);
        x1 = x1.max(next.x + next.width);
        y1 = y1.max(next.y + next.height);
    }
    Some(CellBounds {
        x: x0,
        y: y0,
        width: x1 - x0,
        height: y1 - y0,
    })
}

fn cell_at(offset: usize, stride: usize) -> (u16, u16) {
    let x = u16::try_from(offset % stride).expect("column fits in u16");
    let y = u16::try_from(offset / stride).expect("row fits in u16");
    (x, y)
}

fn offset(width: u16, x: u16, y: u16) -> usize {
    usize::from(y) * usize::from(width) + usize::from(x)
}

fn bounds_of(cells: &[(u16, u16)]) -> Option<CellBounds> {
    let mut cells = cells.iter().copied();
    let (mut min_x, mut min_y) = cells.next()?;
    let mut max_x = min_x;
    let mut max_y = min_y;
    for (x, y) in cells {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    Some(CellBounds {
        x: min_x,
        y: min_y,
        width: max_x - min_x + 1,
        height: max_y - min_y + 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_hash_is_fnv1a64_of_rgba_bytes() {
        assert_eq!(palette_hash(&[Rgba::TRANSPARENT]), "4d25767f9dce13f5");
        assert_eq!(palette_hash(&[Rgba::new(1, 2, 3, 255)]), "be7a03775164ddbc");
        assert_eq!(
            palette_hash(&[Rgba::TRANSPARENT, Rgba::rgb(1, 2, 3)]),
            palette_hash(&[Rgba::TRANSPARENT, Rgba::rgb(1, 2, 3)])
        );
        assert_ne!(
            palette_hash(&[Rgba::rgb(1, 2, 3)]),
            palette_hash(&[Rgba::rgb(1, 2, 4)])
        );
    }

    #[test]
    fn view_bounds_cover_ink_only() {
        let pixels = [0, 3, 0, 0, 3, 0];
        let summary = summarize_view(&pixels, 3);
        assert_eq!(summary.ink, 2);
        assert_eq!(
            summary.bounds,
            Some(CellBounds {
                x: 1,
                y: 0,
                width: 1,
                height: 2,
            })
        );
        assert_eq!(summary.indices, vec![(0, 4), (3, 2)]);
    }

    #[test]
    fn diff_kinds_keep_the_indices_ascii_collapses() {
        let left = [1, 2, 0, 0];
        let right = [1, 3, 4, 0];
        let summary = summarize_diff(&left, &right, 4);
        assert_eq!(summary.same, 2);
        assert_eq!(summary.differ, 1);
        assert_eq!(summary.only_frame, 0);
        assert_eq!(summary.only_with, 1);
        assert_eq!(
            summary.bounds,
            Some(CellBounds {
                x: 1,
                y: 0,
                width: 2,
                height: 1,
            })
        );
        assert_eq!(summary.changes[0].left, 2);
        assert_eq!(summary.changes[0].right, 3);
        assert_eq!(summary.changes[1].left, 0);
        assert_eq!(summary.changes[1].right, 4);
    }

    #[test]
    fn dirty_region_shifts_inside_the_canvas_and_counts_clipped_ink() {
        let before = [1, 0, 0, 0];
        let after = [0, 1, 0, 0];
        let shifted = dirty_region(0, 0, &before, 2, 2, &after, 2, 2);
        assert_eq!(shifted.changed, 2);
        assert_eq!(shifted.clipped, 0);
        assert_eq!(
            shifted.bounds,
            Some(CellBounds {
                x: 0,
                y: 0,
                width: 2,
                height: 1,
            })
        );

        let shrunk = dirty_region(1, 2, &[0, 0, 0, 5], 4, 1, &[0, 0], 2, 1);
        assert_eq!(shrunk.changed, 0);
        assert_eq!(shrunk.clipped, 1);
        assert_eq!(shrunk.bounds, None);
        assert_eq!(
            union_bounds([
                CellBounds {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                },
                CellBounds {
                    x: 2,
                    y: 1,
                    width: 1,
                    height: 1,
                },
            ]),
            Some(CellBounds {
                x: 0,
                y: 0,
                width: 3,
                height: 2,
            })
        );
    }
}
