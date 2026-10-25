//! Part translate, rotate, and flip on the integer pixel grid.
//!
//! The pivot is the center of a cell: the part origin, or a named anchor's
//! canvas cell. A cell center rotated around that pivot lands on another cell
//! by nearest neighbor. Multiples of 90° (after 0.01° quantization) use an
//! integer map, so they are exact and reversible when every ink cell stays on
//! the canvas. Rectangle `flipHorizontal` / `rotate` are a different pivot
//! and do not move parts.

use crate::error::KernelError;

/// Largest absolute degree value accepted by [`classify_degrees`].
///
/// Values inside this bound reduce modulo 360 in `f64` without dropping the
/// 0.01° place. Larger magnitudes are [`KernelError::PartAngle`].
pub(crate) const MAX_PART_DEGREES: f64 = 1_000_000.0;

/// How a canvas cell moves. Exact maps are injective. Nearest is not.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum CellMap {
    Translate {
        dx: i32,
        dy: i32,
    },
    /// Clockwise quarter turns: 0, 1 (90°), 2 (180°), 3 (270°).
    Quarter(u8),
    FlipHorizontal,
    FlipVertical,
    /// Clockwise rotation by the already-quantized angle. `cos` and `sin`
    /// match [`sin_cos_centi`].
    Nearest {
        cos: f64,
        sin: f64,
    },
}

/// A parsed part motion, ready to apply.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedMotion {
    pub map: CellMap,
    /// `exact` for translate, flip, and quarter turns. `nearest` otherwise.
    pub sampling: &'static str,
    /// Present for rotation: `quarter` or `nearest`.
    pub grid: Option<&'static str>,
    /// Quantized hundredths of a degree in `(-18000, 18000]`, rotation only.
    /// `-180°` is stored as `18000`.
    pub degrees_centi: Option<i32>,
    pub dx: Option<i32>,
    pub dy: Option<i32>,
    pub direction: Option<&'static str>,
}

/// What one successful part transform reports, besides the part itself.
#[derive(Debug, Clone, PartialEq)]
pub struct PartPose {
    pub index: usize,
    pub frame: usize,
    pub pivot_x: i32,
    pub pivot_y: i32,
    pub pivot_anchor: Option<String>,
    /// `center-on-origin` or `center-on-anchor`.
    pub alignment: &'static str,
    pub sampling: &'static str,
    pub grid: Option<&'static str>,
    pub degrees_centi: Option<i32>,
    pub dx: Option<i32>,
    pub dy: Option<i32>,
    pub direction: Option<&'static str>,
    /// Ink cells that lost a nearest-neighbor collision. Exact maps report 0.
    pub dropped: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Motion {
    Translate { dx: i32, dy: i32 },
    Rotate(f64),
    FlipHorizontal,
    FlipVertical,
}

/// Quantized hundredths of a degree in `(-18000, 18000]`, or [`KernelError::PartAngle`].
///
/// The same wrap and 0.01° rule as [`classify_degrees`]. Curve keys store this
/// integer so a sampled angle matches what [`crate::Document::rotate_part`]
/// would quantize.
pub(crate) fn quantize_degrees(degrees: f64) -> Result<i32, KernelError> {
    classify_degrees(degrees).map(|class| class.centi)
}

pub(crate) fn resolve_motion(motion: Motion) -> Result<ResolvedMotion, KernelError> {
    match motion {
        Motion::Translate { dx, dy } => Ok(ResolvedMotion {
            map: CellMap::Translate { dx, dy },
            sampling: "exact",
            grid: None,
            degrees_centi: None,
            dx: Some(dx),
            dy: Some(dy),
            direction: None,
        }),
        Motion::FlipHorizontal => Ok(ResolvedMotion {
            map: CellMap::FlipHorizontal,
            sampling: "exact",
            grid: None,
            degrees_centi: None,
            dx: None,
            dy: None,
            direction: Some("horizontal"),
        }),
        Motion::FlipVertical => Ok(ResolvedMotion {
            map: CellMap::FlipVertical,
            sampling: "exact",
            grid: None,
            degrees_centi: None,
            dx: None,
            dy: None,
            direction: Some("vertical"),
        }),
        Motion::Rotate(degrees) => {
            let class = classify_degrees(degrees)?;
            let quarter = matches!(class.map, CellMap::Quarter(_));
            Ok(ResolvedMotion {
                map: class.map,
                sampling: if quarter { "exact" } else { "nearest" },
                grid: Some(if quarter { "quarter" } else { "nearest" }),
                degrees_centi: Some(class.centi),
                dx: None,
                dy: None,
                direction: None,
            })
        }
    }
}

struct Classified {
    centi: i32,
    map: CellMap,
}

/// Reduce `degrees` clockwise into a grid map.
///
/// 1. Reject non-finite values and magnitudes above [`MAX_PART_DEGREES`].
/// 2. Wrap into `(-180, 180]`. `-180` becomes `180`.
/// 3. Quantize to the nearest `0.01` degree. A tie (`*.xx5` with no further
///    fraction) rounds away from zero.
/// 4. A quantized multiple of 90 uses [`CellMap::Quarter`]. Anything else
///    uses [`CellMap::Nearest`] with the kernel sin/cos of that 0.01° angle.
fn classify_degrees(degrees: f64) -> Result<Classified, KernelError> {
    if !degrees.is_finite() || degrees.abs() > MAX_PART_DEGREES {
        return Err(KernelError::PartAngle);
    }
    let wrapped = wrap_degrees(degrees);
    let mut centi = quantize_centi(wrapped);
    if centi <= -18_000 {
        centi = 18_000;
    }
    let map = if centi % 9_000 == 0 {
        let turns = match centi {
            0 => 0,
            9_000 => 1,
            18_000 => 2,
            -9_000 => 3,
            _ => unreachable!("centi was a multiple of 9000 inside the wrapped range"),
        };
        CellMap::Quarter(turns)
    } else {
        let (sin, cos) = sin_cos_centi(centi);
        CellMap::Nearest { cos, sin }
    };
    Ok(Classified { centi, map })
}

fn wrap_degrees(degrees: f64) -> f64 {
    let mut wrapped = degrees % 360.0;
    if wrapped <= -180.0 {
        wrapped += 360.0;
    } else if wrapped > 180.0 {
        wrapped -= 360.0;
    }
    wrapped
}

fn quantize_centi(wrapped: f64) -> i32 {
    let rounded = (wrapped * 100.0).round();
    debug_assert!(rounded.is_finite());
    rounded as i32
}

/// Image of cell `(x, y)` pivoted on cell `(px, py)`.
///
/// `Err` is an off-grid stand-in when the sum does not fit in `i32`.
pub(crate) fn map_point(
    map: &CellMap,
    x: i32,
    y: i32,
    px: i32,
    py: i32,
) -> Result<(i32, i32), (i32, i32)> {
    match *map {
        CellMap::Translate { dx, dy } => {
            let nx = x.checked_add(dx).ok_or((overflow_axis(dx), y))?;
            let ny = y.checked_add(dy).ok_or((nx, overflow_axis(dy)))?;
            Ok((nx, ny))
        }
        CellMap::Quarter(turns) => {
            let dx = x.checked_sub(px).ok_or((x, y))?;
            let dy = y.checked_sub(py).ok_or((x, y))?;
            match turns % 4 {
                0 => Ok((x, y)),
                1 => Ok((
                    px.checked_sub(dy).ok_or((overflow_axis(-dy), y))?,
                    py.checked_add(dx).ok_or((x, overflow_axis(dx)))?,
                )),
                2 => Ok((
                    px.checked_sub(dx).ok_or((overflow_axis(-dx), y))?,
                    py.checked_sub(dy).ok_or((x, overflow_axis(-dy)))?,
                )),
                _ => Ok((
                    px.checked_add(dy).ok_or((overflow_axis(dy), y))?,
                    py.checked_sub(dx).ok_or((x, overflow_axis(-dx)))?,
                )),
            }
        }
        CellMap::FlipHorizontal => {
            let dx = x.checked_sub(px).ok_or((x, y))?;
            Ok((px.checked_sub(dx).ok_or((overflow_axis(-dx), y))?, y))
        }
        CellMap::FlipVertical => {
            let dy = y.checked_sub(py).ok_or((x, y))?;
            Ok((x, py.checked_sub(dy).ok_or((x, overflow_axis(-dy)))?))
        }
        CellMap::Nearest { cos, sin } => {
            let dx = f64::from(x.checked_sub(px).ok_or((x, y))?);
            let dy = f64::from(y.checked_sub(py).ok_or((x, y))?);
            // Screen y grows downward, so this matrix is clockwise.
            let fx = dx * cos - dy * sin;
            let fy = dx * sin + dy * cos;
            let rx = round_half_away(fx).ok_or((x, y))?;
            let ry = round_half_away(fy).ok_or((x, y))?;
            Ok((
                px.checked_add(rx).ok_or((overflow_axis(rx), y))?,
                py.checked_add(ry).ok_or((x, overflow_axis(ry)))?,
            ))
        }
    }
}

fn overflow_axis(delta: i32) -> i32 {
    if delta >= 0 {
        i32::MAX
    } else {
        i32::MIN
    }
}

/// Nearest integer, ties away from zero.
///
/// A value within `1e-9` of a half-integer is treated as that half-integer
/// first, so a Taylor residue on an exact tie (30° of a one-cell offset)
/// still rounds away from the pivot.
fn round_half_away(value: f64) -> Option<i32> {
    if !value.is_finite() {
        return None;
    }
    let nearest_half = (value * 2.0).round() / 2.0;
    let snapped = if (value - nearest_half).abs() < 1e-9 {
        nearest_half
    } else {
        value
    };
    let rounded = snapped.round();
    if !rounded.is_finite() || rounded < f64::from(i32::MIN) || rounded > f64::from(i32::MAX) {
        return None;
    }
    Some(rounded as i32)
}

/// Sin and cos of `centi` hundredths of a degree, via a fixed Taylor series.
///
/// The series is plain `f64` arithmetic, so the same angle maps to the same
/// cell on every platform. It is not `libm`.
fn sin_cos_centi(centi: i32) -> (f64, f64) {
    let radians = f64::from(centi) * (std::f64::consts::PI / 18_000.0);
    (sin_taylor(radians), cos_taylor(radians))
}

fn sin_taylor(x: f64) -> f64 {
    let mut sum = 0.0;
    let mut term = x;
    let x2 = x * x;
    let mut n = 1.0;
    for _ in 0..16 {
        sum += term;
        term = -term * x2 / ((n + 1.0) * (n + 2.0));
        n += 2.0;
    }
    sum
}

fn cos_taylor(x: f64) -> f64 {
    let mut sum = 0.0;
    let mut term = 1.0;
    let x2 = x * x;
    let mut n = 0.0;
    for _ in 0..16 {
        sum += term;
        term = -term * x2 / ((n + 1.0) * (n + 2.0));
        n += 2.0;
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    fn centi(degrees: f64) -> i32 {
        classify_degrees(degrees).unwrap().centi
    }

    fn quarter(degrees: f64) -> bool {
        matches!(classify_degrees(degrees).unwrap().map, CellMap::Quarter(_))
    }

    #[test]
    fn degrees_quantize_to_a_hundredth_and_snap_to_quarter_turns() {
        assert_eq!(centi(90.0), 9_000);
        assert_eq!(centi(90.004), 9_000);
        assert_eq!(centi(89.995), 9_000);
        assert_eq!(centi(450.0), 9_000);
        assert_eq!(centi(-270.0), 9_000);
        assert!(quarter(90.004));
        assert_eq!(centi(90.005), 9_001);
        assert!(!quarter(90.005));
        assert_eq!(centi(89.994), 8_999);
        assert_eq!(centi(0.004), 0);
        assert!(quarter(0.004));
        assert_eq!(centi(0.005), 1);
        assert_eq!(centi(20.0), 2_000);
        assert_eq!(centi(20.004), 2_000);
        assert_eq!(centi(20.005), 2_001);
        assert_eq!(centi(-90.0), -9_000);
        assert_eq!(centi(270.0), -9_000);
        assert_eq!(centi(180.0), 18_000);
        assert_eq!(centi(-180.0), 18_000);
        assert_eq!(centi(360.0), 0);
        assert!(classify_degrees(f64::NAN).is_err());
        assert!(classify_degrees(1_000_000.5).is_err());
        assert!(classify_degrees(1_000_000.0).is_ok());
    }

    #[test]
    fn quarter_turns_and_flips_are_integer_and_involutions() {
        let pivot = (4, 3);
        let cell = (5, 3);
        let cw90 = CellMap::Quarter(1);
        let once = map_point(&cw90, cell.0, cell.1, pivot.0, pivot.1).unwrap();
        assert_eq!(once, (4, 4));
        let mut x = cell.0;
        let mut y = cell.1;
        for _ in 0..4 {
            (x, y) = map_point(&cw90, x, y, pivot.0, pivot.1).unwrap();
        }
        assert_eq!((x, y), cell);
        let back = map_point(&CellMap::Quarter(3), once.0, once.1, pivot.0, pivot.1).unwrap();
        assert_eq!(back, cell);

        let flip = CellMap::FlipHorizontal;
        let mirrored = map_point(&flip, cell.0, cell.1, pivot.0, pivot.1).unwrap();
        assert_eq!(mirrored, (3, 3));
        let restored = map_point(&flip, mirrored.0, mirrored.1, pivot.0, pivot.1).unwrap();
        assert_eq!(restored, cell);
        assert_eq!(
            map_point(&CellMap::Translate { dx: 2, dy: -1 }, 3, 3, 0, 0).unwrap(),
            (5, 2)
        );
    }

    #[test]
    fn thirty_degrees_of_one_cell_rounds_away_from_the_pivot() {
        let map = classify_degrees(30.0).unwrap().map;
        assert!(matches!(map, CellMap::Nearest { .. }));
        assert_eq!(map_point(&map, 3, 2, 2, 2).unwrap(), (3, 3));
        let (sin, _cos) = sin_cos_centi(3_000);
        assert!((sin - 0.5).abs() < 1e-12);
    }

    #[test]
    fn twenty_degrees_can_drop_a_colliding_cell() {
        let map = classify_degrees(20.0).unwrap().map;
        let pivot = (4, 4);
        let a = map_point(&map, 5, 1, pivot.0, pivot.1).unwrap();
        let b = map_point(&map, 5, 2, pivot.0, pivot.1).unwrap();
        assert_eq!(a, (6, 2));
        assert_eq!(a, b);
        let same = classify_degrees(20.004).unwrap().map;
        assert_eq!(
            map_point(&same, 5, 1, pivot.0, pivot.1).unwrap(),
            map_point(&map, 5, 1, pivot.0, pivot.1).unwrap()
        );
    }
}
