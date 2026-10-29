//! Deterministic 1px raster shapes.
//!
//! Lines are Bresenham-class and 8-connected, so consecutive cells touch by
//! an edge or a corner. `pixel-perfect` is the thinner major-axis walk: one
//! cell per step along the longer axis, still 8-connected. Ellipses are the
//! integer midpoint ellipse inscribed in a rectangle. Polygons close the
//! vertex list with the same line, and a fill is the even-odd interior plus
//! that outline. Flood fill replaces one index. There is no anti-aliasing.

/// 4-connected or 8-connected neighborhood for [`flood_cells`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connectivity {
    /// Orthogonal neighbors only.
    Four,
    /// Orthogonal and diagonal neighbors.
    Eight,
}

impl Connectivity {
    /// `4` or `8`. Anything else is `None`.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            4 => Some(Self::Four),
            8 => Some(Self::Eight),
            _ => None,
        }
    }

    /// Stable parameter value.
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Four => 4,
            Self::Eight => 8,
        }
    }
}

/// Zingl's continuous Bresenham line, inclusive of both endpoints.
///
/// A step may change x and y together. The cells stay 8-connected: each
/// cell after the first shares an edge or a corner with the previous one.
pub fn line_continuous(x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;
    let mut cells = Vec::new();
    loop {
        cells.push((x, y));
        let e2 = 2 * err;
        if e2 >= dy {
            if x == x1 {
                break;
            }
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            if y == y1 {
                break;
            }
            err += dx;
            y += sy;
        }
    }
    cells
}

/// One cell per step along the longer axis. Still 8-connected.
///
/// A single cell is that cell. Steep lines walk y and plot x as the minor
/// axis, matching the thinner "pixel perfect" line.
pub fn line_perfect(x1: i32, y1: i32, x2: i32, y2: i32) -> Vec<(i32, i32)> {
    if x1 == x2 && y1 == y2 {
        return vec![(x1, y1)];
    }
    let mut x1 = x1;
    let mut y1 = y1;
    let mut x2 = x2;
    let mut y2 = y2;
    let yaxis = (y2 - y1).abs() > (x2 - x1).abs();
    if yaxis {
        std::mem::swap(&mut x1, &mut y1);
        std::mem::swap(&mut x2, &mut y2);
    }
    let w = (x2 - x1).abs() + 1;
    let h = (y2 - y1).abs() + 1;
    let dx = sgn(x2 - x1);
    let dy = sgn(y2 - y1);
    let mut e = 0;
    let mut y = y1;
    let end = x2 + dx;
    let mut cells = Vec::with_capacity(w as usize);
    let mut x = x1;
    while x != end {
        if yaxis {
            cells.push((y, x));
        } else {
            cells.push((x, y));
        }
        e += h;
        if e >= w {
            y += dy;
            e -= w;
        }
        x += dx;
    }
    cells
}

fn sgn(value: i32) -> i32 {
    match value.cmp(&0) {
        std::cmp::Ordering::Greater => 1,
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
    }
}

/// Ellipse inscribed in the inclusive rectangle. `width` and `height` are at least 1.
///
/// A zero-thickness side is the rectangle itself: outline and fill match.
/// The fill includes every outline cell.
pub fn ellipse_cells(x: i32, y: i32, width: i32, height: i32, fill: bool) -> Vec<(i32, i32)> {
    debug_assert!(width >= 1 && height >= 1);
    let x0 = x;
    let y0 = y;
    let x1 = x + width - 1;
    let y1 = y + height - 1;
    if width == 1 || height == 1 {
        return rect_cells(x0, y0, x1, y1);
    }
    if fill {
        let mut cells = ellipse_zingl(x0, y0, x1, y1, true);
        cells.extend(ellipse_zingl(x0, y0, x1, y1, false));
        cells
    } else {
        ellipse_zingl(x0, y0, x1, y1, false)
    }
}

fn ellipse_zingl(x0: i32, y0: i32, x1: i32, y1: i32, fill: bool) -> Vec<(i32, i32)> {
    let mut x0 = x0;
    let mut y0 = y0;
    let mut x1 = x1;
    let mut y1 = y1;
    if x0 > x1 {
        std::mem::swap(&mut x0, &mut x1);
    }
    if y0 > y1 {
        std::mem::swap(&mut y0, &mut y1);
    }
    let mut a: i64 = i64::from(x1 - x0);
    let b: i64 = i64::from(y1 - y0);
    let mut b1 = b & 1;
    let mut dx = 4 * (1 - a) * b * b;
    let mut dy = 4 * (b1 + 1) * a * a;
    let mut err = dx + dy + b1 * a * a;
    y0 += i32::try_from((b + 1) / 2).unwrap_or(i32::MAX);
    y1 = y0 - i32::try_from(b1).unwrap_or(0);
    a *= 8 * a;
    b1 = 8 * b * b;

    let mut cells = Vec::new();
    loop {
        if fill {
            hspan(&mut cells, x0, x1, y0);
            hspan(&mut cells, x0, x1, y1);
        } else {
            cells.push((x1, y0));
            cells.push((x0, y0));
            cells.push((x0, y1));
            cells.push((x1, y1));
        }
        let e2 = 2 * err;
        if e2 <= dy {
            y0 += 1;
            y1 -= 1;
            dy += a;
            err += dy;
        }
        if e2 >= dx || 2 * err > dy {
            x0 += 1;
            x1 -= 1;
            dx += b1;
            err += dx;
        }
        if x0 > x1 || cells.len() > 1_000_000 {
            break;
        }
    }
    let mut guard = 0;
    while i64::from(y0) - i64::from(y1) < b && guard < 10_000 {
        if fill {
            hspan(&mut cells, x0 - 1, x1 + 1, y0);
            hspan(&mut cells, x0 - 1, x1 + 1, y1);
        } else {
            cells.push((x0 - 1, y0));
            cells.push((x1 + 1, y0));
            cells.push((x0 - 1, y1));
            cells.push((x1 + 1, y1));
        }
        y0 += 1;
        y1 -= 1;
        guard += 1;
    }
    cells
}

fn hspan(cells: &mut Vec<(i32, i32)>, x0: i32, x1: i32, y: i32) {
    let (lo, hi) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
    for x in lo..=hi {
        cells.push((x, y));
    }
}

fn rect_cells(x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
    let mut cells = Vec::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            cells.push((x, y));
        }
    }
    cells
}

/// Closed polygon. `fill` adds the even-odd interior to the outline.
///
/// Fewer than 3 vertices yields an empty list. The caller refuses that.
pub fn polygon_cells(points: &[(i32, i32)], fill: bool) -> Vec<(i32, i32)> {
    if points.len() < 3 {
        return Vec::new();
    }
    let mut cells = polygon_outline(points);
    if fill {
        cells.extend(polygon_even_odd(points));
    }
    cells
}

fn polygon_outline(points: &[(i32, i32)]) -> Vec<(i32, i32)> {
    let mut cells = Vec::new();
    for index in 0..points.len() {
        let (x0, y0) = points[index];
        let (x1, y1) = points[(index + 1) % points.len()];
        cells.extend(line_continuous(x0, y0, x1, y1));
    }
    cells
}

fn polygon_even_odd(points: &[(i32, i32)]) -> Vec<(i32, i32)> {
    let ymin = points.iter().map(|point| point.1).min().unwrap_or(0);
    let ymax = points.iter().map(|point| point.1).max().unwrap_or(0);
    let mut cells = Vec::new();
    for y in ymin..=ymax {
        let mut hits: Vec<(i64, i64)> = Vec::new();
        for index in 0..points.len() {
            let (x0, y0) = points[index];
            let (x1, y1) = points[(index + 1) % points.len()];
            let (xa, ya, xb, yb) = if y0 < y1 {
                (x0, y0, x1, y1)
            } else if y1 < y0 {
                (x1, y1, x0, y0)
            } else {
                continue;
            };
            if y < ya || y >= yb {
                continue;
            }
            let dy = i64::from(yb - ya);
            let dx = i64::from(xb - xa);
            let ty = i64::from(y - ya);
            hits.push((i64::from(xa) * dy + dx * ty, dy));
        }
        hits.sort_by(|left, right| {
            let left_cross = i128::from(left.0) * i128::from(right.1);
            let right_cross = i128::from(right.0) * i128::from(left.1);
            left_cross.cmp(&right_cross)
        });
        let mut index = 0;
        while index + 1 < hits.len() {
            let start = ceil_div(hits[index].0, hits[index].1);
            let end = floor_div(hits[index + 1].0, hits[index + 1].1);
            if start <= end {
                for x in start..=end {
                    let Ok(x) = i32::try_from(x) else {
                        continue;
                    };
                    cells.push((x, y));
                }
            }
            index += 2;
        }
    }
    cells
}

fn floor_div(numer: i64, denom: i64) -> i64 {
    debug_assert!(denom > 0);
    let quot = numer / denom;
    let rem = numer % denom;
    if rem != 0 && numer < 0 {
        quot - 1
    } else {
        quot
    }
}

fn ceil_div(numer: i64, denom: i64) -> i64 {
    debug_assert!(denom > 0);
    let quot = numer / denom;
    let rem = numer % denom;
    if rem != 0 && numer > 0 {
        quot + 1
    } else {
        quot
    }
}

/// Border of an axis-aligned rectangle. A 1-cell side is the whole rectangle.
pub fn stroke_rect_cells(x: i32, y: i32, width: i32, height: i32) -> Vec<(i32, i32)> {
    debug_assert!(width >= 1 && height >= 1);
    let x1 = x + width - 1;
    let y1 = y + height - 1;
    let mut cells = Vec::new();
    for col in x..=x1 {
        cells.push((col, y));
        if y1 != y {
            cells.push((col, y1));
        }
    }
    if height > 2 {
        for row in (y + 1)..y1 {
            cells.push((x, row));
            if x1 != x {
                cells.push((x1, row));
            }
        }
    }
    cells
}

/// Cells of `source` reached from `(x, y)` through `writable` cells.
///
/// A cell that is not writable, or that holds another index, is a barrier.
/// The seed itself is included when it is writable and holds `source`.
/// This is [`flood_matching`] with an exact index predicate. A selection
/// wand uses the same walk when the match is a palette-distance predicate.
pub fn flood_cells(
    pixels: &[u8],
    width: u16,
    height: u16,
    x: u16,
    y: u16,
    source: u8,
    connectivity: Connectivity,
    writable: &[bool],
) -> Vec<(u16, u16)> {
    flood_matching(
        pixels,
        width,
        height,
        x,
        y,
        connectivity,
        writable,
        |index| index == source,
    )
}

/// Cells reached from `(x, y)` through `writable` cells that `matches` accepts.
///
/// A cell that is not writable, or that `matches` rejects, is a barrier.
/// The seed is included only when it is writable and `matches` accepts its
/// index. The walk is the same stack as [`flood_cells`].
pub fn flood_matching(
    pixels: &[u8],
    width: u16,
    height: u16,
    x: u16,
    y: u16,
    connectivity: Connectivity,
    writable: &[bool],
    matches: impl Fn(u8) -> bool,
) -> Vec<(u16, u16)> {
    let count = usize::from(width) * usize::from(height);
    debug_assert_eq!(pixels.len(), count);
    debug_assert_eq!(writable.len(), count);
    let mut seen = vec![false; count];
    let mut stack = vec![(x, y)];
    let mut cells = Vec::new();
    while let Some((cx, cy)) = stack.pop() {
        let offset = usize::from(width) * usize::from(cy) + usize::from(cx);
        if seen[offset] {
            continue;
        }
        seen[offset] = true;
        if !writable[offset] || !matches(pixels[offset]) {
            continue;
        }
        cells.push((cx, cy));
        for (nx, ny) in neighbors(cx, cy, width, height, connectivity) {
            let next = usize::from(width) * usize::from(ny) + usize::from(nx);
            if !seen[next] {
                stack.push((nx, ny));
            }
        }
    }
    cells
}

fn neighbors(
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    connectivity: Connectivity,
) -> impl Iterator<Item = (u16, u16)> {
    const FOUR: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    const EIGHT: [(i32, i32); 8] = [
        (1, 0),
        (-1, 0),
        (0, 1),
        (0, -1),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
    ];
    let steps: &[(i32, i32)] = match connectivity {
        Connectivity::Four => &FOUR,
        Connectivity::Eight => &EIGHT,
    };
    let x = i32::from(x);
    let y = i32::from(y);
    let width = i32::from(width);
    let height = i32::from(height);
    steps.iter().filter_map(move |(dx, dy)| {
        let nx = x + dx;
        let ny = y + dy;
        if nx < 0 || ny < 0 || nx >= width || ny >= height {
            None
        } else {
            Some((nx as u16, ny as u16))
        }
    })
}

/// True when each cell after the first shares an edge or a corner with the previous one.
#[cfg(test)]
fn is_8_connected(cells: &[(i32, i32)]) -> bool {
    cells.windows(2).all(|pair| {
        let dx = (pair[1].0 - pair[0].0).abs();
        let dy = (pair[1].1 - pair[0].1).abs();
        dx <= 1 && dy <= 1 && (dx + dy) > 0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continuous_line_includes_ends_and_has_no_gaps() {
        let flat = line_continuous(0, 0, 5, 0);
        assert_eq!(flat.len(), 6);
        assert_eq!(flat[0], (0, 0));
        assert_eq!(*flat.last().unwrap(), (5, 0));
        assert!(is_8_connected(&flat));

        let steep = line_continuous(0, 0, 3, 1);
        assert_eq!(steep, vec![(0, 0), (1, 0), (2, 1), (3, 1)]);
        assert!(is_8_connected(&steep));

        let diagonal = line_continuous(2, 2, 0, 0);
        assert_eq!(diagonal.first().copied(), Some((2, 2)));
        assert_eq!(diagonal.last().copied(), Some((0, 0)));
        assert!(is_8_connected(&diagonal));
        assert_eq!(line_continuous(4, 4, 4, 4), vec![(4, 4)]);
    }

    #[test]
    fn perfect_line_is_thinner_and_still_connected() {
        let continuous = line_continuous(0, 0, 4, 3);
        let perfect = line_perfect(0, 0, 4, 3);
        assert_ne!(continuous, perfect);
        assert_eq!(perfect.first().copied(), Some((0, 0)));
        assert_eq!(perfect.last().copied(), Some((4, 3)));
        assert!(is_8_connected(&perfect));
        assert!(perfect.len() <= continuous.len());
        assert_eq!(line_perfect(3, 8, 3, 8), vec![(3, 8)]);
        let vertical = line_perfect(1, 0, 1, 4);
        assert_eq!(vertical.len(), 5);
        assert!(is_8_connected(&vertical));
    }

    #[test]
    fn ellipse_stays_inside_the_rect_and_fill_covers_the_outline() {
        for width in 1..=32 {
            for height in 1..=32 {
                let outline = ellipse_cells(2, 3, width, height, false);
                let filled = ellipse_cells(2, 3, width, height, true);
                assert!(!outline.is_empty(), "{width}x{height}");
                for &(x, y) in outline.iter().chain(&filled) {
                    assert!(
                        (2..2 + width).contains(&x) && (3..3 + height).contains(&y),
                        "({x},{y}) outside {width}x{height}"
                    );
                }
                let mut cover = filled.clone();
                cover.sort_unstable();
                cover.dedup();
                for cell in &outline {
                    assert!(
                        cover.contains(cell),
                        "outline {cell:?} missing from {width}x{height}"
                    );
                }
            }
        }
        let dot = ellipse_cells(0, 0, 1, 1, false);
        assert_eq!(dot, vec![(0, 0)]);
        assert_eq!(ellipse_cells(0, 0, 1, 4, true).len(), 4);
    }

    #[test]
    fn rectangle_polygon_fills_its_bounds_and_outline_is_the_border() {
        let points = [(0, 0), (4, 0), (4, 4), (0, 4)];
        let outline = polygon_cells(&points, false);
        let mut border = outline.clone();
        border.sort_unstable();
        border.dedup();
        assert_eq!(border.len(), 16);
        let filled = polygon_cells(&points, true);
        let mut unique = filled.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 25);
        let again = polygon_cells(&points, true);
        assert_eq!(filled, again);
        assert!(polygon_cells(&[(0, 0), (1, 1)], true).is_empty());
    }

    #[test]
    fn stroke_rect_is_the_border_only() {
        let cells = stroke_rect_cells(1, 2, 4, 3);
        let mut unique = cells.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 4 * 2 + 1 * 2);
        assert!(!unique.contains(&(2, 3)));
        assert_eq!(stroke_rect_cells(0, 0, 1, 3).len(), 3);
    }

    #[test]
    fn flood_respects_connectivity_and_barriers() {
        let width = 3u16;
        let height = 2u16;
        let pixels = [1u8, 0, 0, 0, 1, 0];
        let writable = [true; 6];
        let four = flood_cells(
            &pixels,
            width,
            height,
            0,
            0,
            1,
            Connectivity::Four,
            &writable,
        );
        assert_eq!(four, vec![(0, 0)]);
        let eight = flood_cells(
            &pixels,
            width,
            height,
            0,
            0,
            1,
            Connectivity::Eight,
            &writable,
        );
        let mut eight = eight;
        eight.sort_unstable();
        assert_eq!(eight, vec![(0, 0), (1, 1)]);

        let pixels = [0u8, 0, 0];
        let blocked = [true, false, true];
        let cells = flood_cells(&pixels, 3, 1, 0, 0, 0, Connectivity::Four, &blocked);
        assert_eq!(cells, vec![(0, 0)]);
    }

    #[test]
    fn flood_matching_uses_the_same_walk_as_an_exact_index() {
        let pixels = [1u8, 2, 1, 9];
        let writable = [true; 4];
        let cells = flood_matching(
            &pixels,
            2,
            2,
            0,
            0,
            Connectivity::Four,
            &writable,
            |index| index == 1 || index == 2,
        );
        let mut cells = cells;
        cells.sort_unstable();
        assert_eq!(cells, vec![(0, 0), (0, 1), (1, 0)]);
    }
}
