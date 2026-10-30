//! Nearest-color remap of indexed pixels onto a palette.
//!
//! Distance is squared RGB (`dr² + dg² + db²`). Alpha is not part of the
//! distance. An exact hit is equal `r`, `g`, and `b`. A tie keeps the lower
//! palette index. Quantize never invents a new RGB: the survivor keeps the
//! color it already had.
//!
//! This module does not fetch palettes and does not talk to Lospec. Callers
//! pass palette text the kernel already knows how to parse, or PNG bytes.

use std::collections::BTreeMap;
use std::io::Cursor;

use png::{ColorType, Decoder, Transformations};

use crate::error::KernelError;
use crate::palette::Palette;
use crate::Rgba;
use crate::MAX_CANVAS_EDGE;

/// One source color folded onto a destination palette entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ColorMerge {
    /// Palette index before the remap. Absent for a PNG pixel.
    pub from_index: Option<u8>,
    pub from: Rgba,
    pub into_index: u8,
    pub into: Rgba,
    /// Cells that stored `from` before the remap, or PNG pixels of that RGB.
    pub pixels: u32,
    /// Squared RGB distance. `0` is an exact hit.
    pub distance: u32,
    pub exact: bool,
}

/// An opaque palette entry removed because nothing used it and the cap was lower.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DroppedColor {
    pub index: u8,
    pub color: Rgba,
}

/// Conflict report for [`crate::Document`] palette remap and PNG crush.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MapReport {
    pub pixels_changed: usize,
    /// Source colors whose RGB was not already in the destination.
    pub misses: Vec<ColorMerge>,
    /// Near-colors that were folded, plus exact index shifts from compaction.
    pub merges: Vec<ColorMerge>,
    /// Unused colors removed to satisfy `maxColors`. PNG crush leaves this empty.
    pub dropped: Vec<DroppedColor>,
}

/// New palette plus the index map that produces [`MapReport`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PalettePlan {
    pub colors: Vec<Rgba>,
    /// Old index to new index. Length is the old palette.
    pub map: Vec<u8>,
    pub report: MapReport,
}

/// Decoded PNG, or a size the canvas cannot hold.
pub(crate) enum DecodedPng {
    TooBig {
        width: u32,
        height: u32,
    },
    Image {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
}

/// Blended alpha below this value becomes index 0.
///
/// [`crush_rgba`] and [`bake_composite`] share the cutoff. 128 is the same
/// threshold `applyPng` already uses.
pub(crate) const INDEX_ALPHA_CUTOFF: u8 = 128;

/// Nearest opaque palette entry for `rgb`. Index 0 is never chosen.
pub(crate) fn nearest_opaque(colors: &[Rgba], rgb: (u8, u8, u8)) -> Option<Nearest> {
    let mut best: Option<Nearest> = None;
    for (index, color) in colors.iter().copied().enumerate().skip(1) {
        let distance = distance_sq(rgb, color);
        let replace = best
            .as_ref()
            .is_none_or(|current| distance < current.distance);
        if replace {
            best = Some(Nearest {
                index: u8::try_from(index).expect("palette index fits in u8"),
                color,
                distance,
                exact: distance == 0,
            });
        }
    }
    best
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Nearest {
    pub index: u8,
    pub color: Rgba,
    pub distance: u32,
    pub exact: bool,
}

/// Shrink `colors` to at most `max_colors` opaque entries.
///
/// `max_colors` counts opaque entries, not the transparent slot. Unused
/// entries are dropped from the highest index first. Remaining pairs merge by
/// squared RGB distance; the entry with more pixels survives, and a tie keeps
/// the lower index. The survivor's RGB is not averaged.
pub(crate) fn plan_max_colors(colors: &[Rgba], counts: &[u32], max_colors: usize) -> PalettePlan {
    debug_assert_eq!(colors.len(), counts.len());
    debug_assert!(colors.len() <= 256);
    let opaque = colors.len().saturating_sub(1);
    if opaque <= max_colors {
        return identity(colors);
    }

    let mut parent: Vec<usize> = (0..colors.len()).collect();
    let mut weight = counts.to_vec();
    let mut alive: Vec<usize> = (1..colors.len()).collect();
    let mut dropped_at = Vec::new();

    while alive.len() > max_colors {
        if alive.len() <= 1 {
            break;
        }
        if let Some(pos) = alive.iter().rposition(|&index| counts[index] == 0) {
            let index = alive.remove(pos);
            dropped_at.push(index);
            continue;
        }
        let (left, right) = closest_pair(&alive, colors);
        let (winner, loser) =
            if weight[left] > weight[right] || (weight[left] == weight[right] && left < right) {
                (left, right)
            } else {
                (right, left)
            };
        parent[loser] = winner;
        weight[winner] = weight[winner].saturating_add(weight[loser]);
        alive.retain(|&index| index != loser);
    }

    let mut new_colors = Vec::with_capacity(alive.len() + 1);
    new_colors.push(Rgba::TRANSPARENT);
    let mut new_index_of = vec![0u8; colors.len()];
    for old in &alive {
        let index = u8::try_from(new_colors.len()).expect("opaque cap fits in u8");
        new_index_of[*old] = index;
        new_colors.push(colors[*old]);
    }

    let mut map = vec![0u8; colors.len()];
    let mut merges = Vec::new();
    for old in 1..colors.len() {
        if dropped_at.contains(&old) {
            continue;
        }
        let root = find_root(&parent, old);
        let into_index = new_index_of[root];
        map[old] = into_index;
        let into = new_colors[usize::from(into_index)];
        let exact = rgb_eq(colors[old], into);
        if usize::from(into_index) == old && exact {
            continue;
        }
        merges.push(ColorMerge {
            from_index: Some(u8::try_from(old).expect("old index fits in u8")),
            from: colors[old],
            into_index,
            into,
            pixels: counts[old],
            distance: distance_sq((colors[old].r, colors[old].g, colors[old].b), into),
            exact,
        });
    }

    let mut dropped = dropped_at
        .into_iter()
        .map(|index| DroppedColor {
            index: u8::try_from(index).expect("dropped index fits in u8"),
            color: colors[index],
        })
        .collect::<Vec<_>>();
    dropped.sort_by_key(|color| color.index);
    merges.sort_by_key(|merge| merge.from_index);
    finish(new_colors, map, merges, dropped)
}

/// Map every old opaque entry onto `target`, which does not include index 0.
pub(crate) fn plan_target(
    colors: &[Rgba],
    counts: &[u32],
    target: &[Rgba],
) -> Result<PalettePlan, KernelError> {
    debug_assert_eq!(colors.len(), counts.len());
    if target.is_empty() {
        return Err(KernelError::EmptyPalette);
    }
    if target.len() > usize::from(u8::MAX) {
        return Err(KernelError::PaletteTooLarge {
            len: target.len() + 1,
        });
    }
    let mut new_colors = Vec::with_capacity(target.len() + 1);
    new_colors.push(Rgba::TRANSPARENT);
    new_colors.extend(target.iter().copied());
    Palette::new(new_colors.clone())?;

    let mut map = vec![0u8; colors.len()];
    let mut merges = Vec::new();
    for old in 1..colors.len() {
        let nearest = nearest_opaque(&new_colors, (colors[old].r, colors[old].g, colors[old].b))
            .expect("target keeps an opaque color");
        map[old] = nearest.index;
        if usize::from(nearest.index) == old && nearest.exact {
            continue;
        }
        merges.push(ColorMerge {
            from_index: Some(u8::try_from(old).expect("old index fits in u8")),
            from: colors[old],
            into_index: nearest.index,
            into: nearest.color,
            pixels: counts[old],
            distance: nearest.distance,
            exact: nearest.exact,
        });
    }
    merges.sort_by_key(|merge| merge.from_index);
    Ok(finish(new_colors, map, merges, Vec::new()))
}

/// Snap one frame's RGBA composite onto palette indices.
///
/// This is the `layer flatten` rule. It reads the same straight-alpha buffer
/// as color preview and PNG export, then writes an index:
///
/// - Alpha below [`INDEX_ALPHA_CUTOFF`] becomes index 0.
/// - Otherwise the nearest palette entry at index 1 or above, by squared RGB.
///   Alpha is not part of that distance. A tie keeps the lower index.
/// - A cell whose RGBA already equals the covering index's palette color keeps
///   that index. An opaque stack therefore does not move when two entries
///   share a color.
///
/// The result layer is fully opaque, so a blend whose only translucency came
/// from layer opacity is stored as that palette entry, not as a new alpha.
pub(crate) fn bake_composite(
    palette: &[Rgba],
    rgba: &[u8],
    cover: &[u8],
) -> Result<Vec<u8>, KernelError> {
    debug_assert_eq!(
        rgba.len(),
        cover.len().saturating_mul(4),
        "blend and cover index the same cells"
    );
    let mut out = Vec::with_capacity(cover.len());
    for (cell, &cover_index) in cover.iter().enumerate() {
        let offset = cell * 4;
        let pixel = [
            rgba[offset],
            rgba[offset + 1],
            rgba[offset + 2],
            rgba[offset + 3],
        ];
        if pixel == palette_bytes(palette, cover_index) {
            out.push(cover_index);
            continue;
        }
        out.push(quantize_pixel(palette, pixel)?);
    }
    Ok(out)
}

fn quantize_pixel(palette: &[Rgba], pixel: [u8; 4]) -> Result<u8, KernelError> {
    if pixel[3] < INDEX_ALPHA_CUTOFF {
        return Ok(0);
    }
    nearest_opaque(palette, (pixel[0], pixel[1], pixel[2]))
        .map(|nearest| nearest.index)
        .ok_or(KernelError::NoPaintColor)
}

fn palette_bytes(palette: &[Rgba], index: u8) -> [u8; 4] {
    match palette.get(usize::from(index)) {
        Some(color) => [color.r, color.g, color.b, color.a],
        None => [0, 0, 0, 0],
    }
}

/// Crush expanded RGBA onto `palette`. Alpha below [`INDEX_ALPHA_CUTOFF`] becomes index 0.
///
/// `pixels_changed` stays 0. The caller counts cells whose stored index moved.
/// [`bake_composite`] uses the same cutoff and the same nearest-index rule,
/// and additionally keeps a covering index whose RGBA already matches.
pub(crate) fn crush_rgba(
    palette: &[Rgba],
    rgba: &[u8],
) -> Result<(Vec<u8>, MapReport), KernelError> {
    if rgba.len() % 4 != 0 {
        return Err(KernelError::PngFile);
    }
    let mut indices = Vec::with_capacity(rgba.len() / 4);
    let mut groups: BTreeMap<(u8, u8, u8), (u32, u8, Rgba, u32)> = BTreeMap::new();
    for pixel in rgba.chunks_exact(4) {
        if pixel[3] < INDEX_ALPHA_CUTOFF {
            indices.push(0);
            continue;
        }
        let nearest = nearest_opaque(palette, (pixel[0], pixel[1], pixel[2]))
            .ok_or(KernelError::NoPaintColor)?;
        indices.push(nearest.index);
        if nearest.exact {
            continue;
        }
        let entry = groups.entry((pixel[0], pixel[1], pixel[2])).or_insert((
            0,
            nearest.index,
            nearest.color,
            nearest.distance,
        ));
        entry.0 = entry.0.saturating_add(1);
    }
    let merges = groups
        .into_iter()
        .map(
            |((r, g, b), (pixels, into_index, into, distance))| ColorMerge {
                from_index: None,
                from: Rgba::rgb(r, g, b),
                into_index,
                into,
                pixels,
                distance,
                exact: false,
            },
        )
        .collect::<Vec<_>>();
    Ok((
        indices,
        MapReport {
            pixels_changed: 0,
            misses: merges.clone(),
            merges,
            dropped: Vec::new(),
        },
    ))
}

/// Read one PNG into 8-bit RGBA. Images larger than the canvas edge are not expanded.
pub(crate) fn decode_png(bytes: &[u8]) -> Result<DecodedPng, KernelError> {
    let mut decoder = Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(Transformations::EXPAND | Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|_| KernelError::PngFile)?;
    let (width, height) = {
        let info = reader.info();
        (info.width, info.height)
    };
    if width == 0 || height == 0 {
        return Err(KernelError::PngFile);
    }
    if width > u32::from(MAX_CANVAS_EDGE) || height > u32::from(MAX_CANVAS_EDGE) {
        return Ok(DecodedPng::TooBig { width, height });
    }
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|_| KernelError::PngFile)?;
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        ColorType::Rgba => buf,
        ColorType::Rgb => expand_channels(&buf, 3, false)?,
        ColorType::Grayscale => expand_channels(&buf, 1, false)?,
        ColorType::GrayscaleAlpha => expand_channels(&buf, 2, true)?,
        _ => return Err(KernelError::PngFile),
    };
    let expected = usize::try_from(width)
        .unwrap_or(usize::MAX)
        .saturating_mul(usize::try_from(height).unwrap_or(usize::MAX))
        .saturating_mul(4);
    if rgba.len() != expected {
        return Err(KernelError::PngFile);
    }
    Ok(DecodedPng::Image {
        width,
        height,
        rgba,
    })
}

fn expand_channels(bytes: &[u8], width: usize, alpha: bool) -> Result<Vec<u8>, KernelError> {
    if width == 0 || bytes.len() % width != 0 {
        return Err(KernelError::PngFile);
    }
    let mut out = Vec::with_capacity(bytes.len() / width * 4);
    for pixel in bytes.chunks_exact(width) {
        let (r, g, b, a) = if width == 1 {
            (pixel[0], pixel[0], pixel[0], 255)
        } else if alpha && width == 2 {
            (pixel[0], pixel[0], pixel[0], pixel[1])
        } else if width == 3 {
            (pixel[0], pixel[1], pixel[2], 255)
        } else {
            return Err(KernelError::PngFile);
        };
        out.extend_from_slice(&[r, g, b, a]);
    }
    Ok(out)
}

fn finish(
    colors: Vec<Rgba>,
    map: Vec<u8>,
    merges: Vec<ColorMerge>,
    dropped: Vec<DroppedColor>,
) -> PalettePlan {
    let misses = merges
        .iter()
        .filter(|merge| !merge.exact)
        .cloned()
        .collect();
    PalettePlan {
        colors,
        map,
        report: MapReport {
            pixels_changed: 0,
            misses,
            merges,
            dropped,
        },
    }
}

fn identity(colors: &[Rgba]) -> PalettePlan {
    let map = (0..colors.len())
        .map(|index| u8::try_from(index).expect("palette index fits in u8"))
        .collect();
    finish(colors.to_vec(), map, Vec::new(), Vec::new())
}

fn closest_pair(alive: &[usize], colors: &[Rgba]) -> (usize, usize) {
    let mut best: Option<(u32, usize, usize)> = None;
    for (left_at, left) in alive.iter().copied().enumerate() {
        for right in alive.iter().copied().skip(left_at + 1) {
            let distance = distance_sq(
                (colors[left].r, colors[left].g, colors[left].b),
                colors[right],
            );
            let candidate = (distance, left, right);
            if best.is_none_or(|current| candidate < current) {
                best = Some(candidate);
            }
        }
    }
    let (_, left, right) = best.expect("a pair exists while more than one color remains");
    (left, right)
}

fn find_root(parent: &[usize], mut index: usize) -> usize {
    while parent[index] != index {
        index = parent[index];
    }
    index
}

fn rgb_eq(left: Rgba, right: Rgba) -> bool {
    left.r == right.r && left.g == right.g && left.b == right.b
}

fn distance_sq(rgb: (u8, u8, u8), color: Rgba) -> u32 {
    let dr = i32::from(rgb.0) - i32::from(color.r);
    let dg = i32::from(rgb.1) - i32::from(color.g);
    let db = i32::from(rgb.2) - i32::from(color.b);
    u32::try_from(dr * dr + dg * dg + db * db).expect("squared RGB distance fits in u32")
}

#[cfg(test)]
mod tests {
    use super::*;
    use png::{BitDepth, Encoder};

    fn rgba_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut encoder = Encoder::new(&mut bytes, width, height);
        encoder.set_color(ColorType::Rgba);
        encoder.set_depth(BitDepth::Eight);
        encoder
            .write_header()
            .expect("png header")
            .write_image_data(rgba)
            .expect("png data");
        bytes
    }

    #[test]
    fn tie_keeps_the_lower_index_and_skips_transparent() {
        let colors = [
            Rgba::TRANSPARENT,
            Rgba::rgb(0, 0, 255),
            Rgba::rgb(255, 0, 0),
        ];
        let nearest = nearest_opaque(&colors, (0, 0, 0)).unwrap();
        assert_eq!(nearest.index, 1);
        assert!(!nearest.exact);
        assert!(nearest_opaque(&[Rgba::TRANSPARENT], (1, 2, 3)).is_none());
    }

    #[test]
    fn quantize_merges_the_nearest_pair_and_shifts_survivors() {
        let colors = vec![
            Rgba::TRANSPARENT,
            Rgba::rgb(0, 0, 0),
            Rgba::rgb(1, 0, 0),
            Rgba::rgb(255, 0, 0),
        ];
        let counts = [0, 2, 2, 1];
        let plan = plan_max_colors(&colors, &counts, 2);
        assert_eq!(
            plan.colors,
            vec![Rgba::TRANSPARENT, Rgba::rgb(0, 0, 0), Rgba::rgb(255, 0, 0)]
        );
        assert_eq!(plan.map, vec![0, 1, 1, 2]);
        assert_eq!(plan.report.dropped, vec![]);
        assert_eq!(plan.report.misses.len(), 1);
        assert_eq!(plan.report.misses[0].from_index, Some(2));
        assert_eq!(plan.report.misses[0].into_index, 1);
        assert_eq!(plan.report.misses[0].pixels, 2);
        assert_eq!(plan.report.misses[0].distance, 1);
        assert_eq!(plan.report.merges.len(), 2);
        assert!(plan.report.merges[1].exact);
        assert_eq!(plan.report.merges[1].from_index, Some(3));
        assert_eq!(plan.report.merges[1].into_index, 2);
    }

    #[test]
    fn quantize_drops_unused_tail_entries_before_merging() {
        let colors = vec![
            Rgba::TRANSPARENT,
            Rgba::rgb(1, 0, 0),
            Rgba::rgb(0, 9, 0),
            Rgba::rgb(0, 0, 9),
            Rgba::rgb(9, 0, 0),
        ];
        let counts = [0, 3, 0, 0, 4];
        let plan = plan_max_colors(&colors, &counts, 2);
        assert_eq!(plan.colors.len(), 3);
        assert_eq!(plan.report.dropped.len(), 2);
        assert_eq!(plan.report.dropped[0].index, 2);
        assert_eq!(plan.report.dropped[1].index, 3);
        assert!(plan.report.misses.is_empty());
        assert_eq!(plan.report.merges.len(), 1);
        assert!(plan.report.merges[0].exact);
        assert_eq!(plan.report.merges[0].from_index, Some(4));
        assert_eq!(plan.report.merges[0].into_index, 2);
        assert_eq!(plan.map[1], 1);
        assert_eq!(plan.map[4], 2);
    }

    #[test]
    fn decode_png_round_trips_rgba_and_refuses_a_huge_image() {
        let png = rgba_png(1, 1, &[9, 8, 7, 255]);
        match decode_png(&png).unwrap() {
            DecodedPng::Image {
                width,
                height,
                rgba,
            } => {
                assert_eq!((width, height), (1, 1));
                assert_eq!(rgba, vec![9, 8, 7, 255]);
            }
            DecodedPng::TooBig { .. } => panic!("1x1 fits"),
        }
        assert!(matches!(
            decode_png(b"not a png"),
            Err(KernelError::PngFile)
        ));
        assert!(matches!(
            decode_png(&rgba_png(300, 1, &vec![0; 300 * 4])),
            Ok(DecodedPng::TooBig {
                width: 300,
                height: 1
            })
        ));
    }

    #[test]
    fn bake_composite_snaps_a_blend_and_keeps_an_exact_cover() {
        let palette = vec![
            Rgba::TRANSPARENT,
            Rgba::rgb(0, 0, 255),
            Rgba::rgb(255, 0, 0),
            Rgba::rgb(0, 0, 255),
            Rgba::new(0, 0, 10, 100),
        ];
        // Cover index 3 is the higher blue. An exact RGBA match stays 3
        // instead of tying down to index 1.
        let exact = bake_composite(&palette, &[0, 0, 255, 255], &[3]).unwrap();
        assert_eq!(exact, vec![3]);
        // (200, 0, 40) is closer to red than to either blue.
        let snapped = bake_composite(&palette, &[200, 0, 40, 255], &[3]).unwrap();
        assert_eq!(snapped, vec![2]);
        // Alpha below the cutoff is index 0, unless it already is the cover.
        let faint = bake_composite(&palette, &[255, 0, 0, 127], &[2]).unwrap();
        assert_eq!(faint, vec![0]);
        let kept = bake_composite(&palette, &[0, 0, 10, 100], &[4]).unwrap();
        assert_eq!(kept, vec![4]);
        // (128, 0, 128) is equidistant from red index 1 and blue index 2.
        // The tie keeps the lower index.
        let tied = bake_composite(
            &[
                Rgba::TRANSPARENT,
                Rgba::rgb(255, 0, 0),
                Rgba::rgb(0, 0, 255),
            ],
            &[128, 0, 128, 255],
            &[2],
        )
        .unwrap();
        assert_eq!(tied, vec![1]);
        assert!(bake_composite(&[Rgba::TRANSPARENT], &[1, 2, 3, 200], &[0]).is_err());
    }
}
