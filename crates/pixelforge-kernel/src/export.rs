//! Transparent PNG export.
//!
//! Frames stay indexed inside the document. Export resolves each index through
//! the palette into RGBA and encodes that buffer. Index 0 is the transparent
//! slot (`a = 0`). Any other entry keeps the alpha stored on the palette.
//!
//! A sequence is one PNG per frame, in timeline order. A sprite sheet packs
//! those same frames into one PNG. See [`sprite_sheet_layout`] for the grid.
//! `scale` repeats each index into a square block before palette resolution
//! (nearest neighbor, no filtering). Scale 1 is the canvas size.
//! [`Document::frame_rgba`] returns that same resolved buffer for a view such
//! as onion skin. It is not stored and is not encoded into the file.

use png::{BitDepth, ColorType, Encoder};

use crate::document::Document;
use crate::error::KernelError;
use crate::Rgba;

/// Where frames sit in an exported sprite sheet.
///
/// `columns` is `ceil(sqrt(frame_count))` and `rows` is
/// `ceil(frame_count / columns)`, so the grid is as square as whole cells
/// allow. Frames fill left to right, then wrap to the next row. A count that
/// is not a multiple of `columns` leaves fully transparent cells at the end
/// of the last row. Those cells are padding, not frames.
///
/// Examples: 1 → 1×1, 2 → 2×1, 3 → 2×2, 4 → 2×2, 5 → 3×2, 9 → 3×3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpriteSheetLayout {
    pub columns: u32,
    pub rows: u32,
    pub frame_width: u16,
    pub frame_height: u16,
    pub frame_count: usize,
}

impl SpriteSheetLayout {
    /// Sheet width in pixels. Equal to `columns * frame_width`.
    pub fn sheet_width(self) -> u32 {
        self.columns * u32::from(self.frame_width)
    }

    /// Sheet height in pixels. Equal to `rows * frame_height`.
    pub fn sheet_height(self) -> u32 {
        self.rows * u32::from(self.frame_height)
    }

    /// Top-left pixel of frame `index` inside the sheet.
    ///
    /// `None` when `index` is past the frame list. Padding cells have no index.
    pub fn frame_origin(self, index: usize) -> Option<(u32, u32)> {
        if index >= self.frame_count {
            return None;
        }
        let index = u32::try_from(index).expect("frame index fits in u32");
        let column = index % self.columns;
        let row = index / self.columns;
        Some((
            column * u32::from(self.frame_width),
            row * u32::from(self.frame_height),
        ))
    }
}

/// Grid used by [`Document::export_sprite_sheet`] for this frame count and cell size.
pub fn sprite_sheet_layout(
    frame_count: usize,
    frame_width: u16,
    frame_height: u16,
) -> SpriteSheetLayout {
    let frame_count = frame_count.max(1);
    let columns = columns_for(frame_count);
    let rows = u32::try_from(frame_count)
        .expect("frame count fits in u32")
        .div_ceil(columns);
    SpriteSheetLayout {
        columns,
        rows,
        frame_width,
        frame_height,
        frame_count,
    }
}

impl Document {
    /// Packed `r, g, b, a` for one frame, row-major, length `width * height * 4`.
    ///
    /// Same palette resolution as [`Self::export_frame_png`]: index 0 is
    /// transparent, and every other index keeps the alpha stored on the palette.
    /// Onion skin reads this buffer and composites it in the shell. The bytes
    /// are not stored on the document, so they cannot land in the project file
    /// or in an export unless a caller writes them there.
    pub fn frame_rgba(&self, index: usize) -> Result<Vec<u8>, KernelError> {
        let pixels = self.frame_pixels(index)?;
        Ok(expand_indexed(self.palette_colors(), pixels))
    }

    /// RGBA PNG of one frame at scale 1.
    ///
    /// Index 0 and any zero-alpha palette entry are transparent.
    pub fn export_frame_png(&self, index: usize) -> Result<Vec<u8>, KernelError> {
        self.export_frame_png_scaled(index, 1)
    }

    /// RGBA PNG of one frame, nearest-neighbor scaled by `scale` cells per pixel.
    ///
    /// `scale` is an integer >= 1. Each palette index is repeated into a
    /// `scale` by `scale` block, then resolved through the palette. There is
    /// no filtering. Scale 1 matches [`Self::export_frame_png`]. A scale of 0,
    /// or a scaled size that does not fit in a PNG, is
    /// [`KernelError::ExportScale`] and writes nothing.
    pub fn export_frame_png_scaled(
        &self,
        index: usize,
        scale: u32,
    ) -> Result<Vec<u8>, KernelError> {
        let pixels = self.frame_pixels(index)?;
        let (width, height, rgba) = frame_rgba_scaled(
            self.palette_colors(),
            pixels,
            self.width(),
            self.height(),
            scale,
        )?;
        Ok(encode_rgba_png(width, height, &rgba))
    }

    /// One RGBA PNG per frame at scale 1, timeline order.
    ///
    /// Same bytes as [`Self::export_frame_png`].
    pub fn export_png_sequence(&self) -> Vec<Vec<u8>> {
        self.export_png_sequence_scaled(1)
            .expect("scale 1 fits every canvas")
    }

    /// One RGBA PNG per frame, nearest-neighbor scaled. See [`Self::export_frame_png_scaled`].
    pub fn export_png_sequence_scaled(&self, scale: u32) -> Result<Vec<Vec<u8>>, KernelError> {
        let _ = self.scaled_frame_size(scale)?;
        (0..self.frame_count())
            .map(|index| self.export_frame_png_scaled(index, scale))
            .collect()
    }

    /// Pixel size of one frame after nearest-neighbor `scale`.
    pub fn scaled_frame_size(&self, scale: u32) -> Result<(u32, u32), KernelError> {
        let scale = require_scale(scale)?;
        Ok((
            mul_scale(u32::from(self.width()), scale)?,
            mul_scale(u32::from(self.height()), scale)?,
        ))
    }

    /// Grid this document would use for [`Self::export_sprite_sheet`].
    ///
    /// Cell size is the canvas. [`Self::export_sprite_sheet_scaled`] multiplies
    /// that cell by `scale` and leaves `columns` and `rows` unchanged.
    pub fn sprite_sheet_layout(&self) -> SpriteSheetLayout {
        sprite_sheet_layout(self.frame_count(), self.width(), self.height())
    }

    /// Pixel size of the sprite sheet after nearest-neighbor `scale`.
    pub fn scaled_sheet_size(&self, scale: u32) -> Result<(u32, u32), KernelError> {
        let scale = require_scale(scale)?;
        let layout = self.sprite_sheet_layout();
        Ok((
            mul_scale(layout.sheet_width(), scale)?,
            mul_scale(layout.sheet_height(), scale)?,
        ))
    }

    /// One RGBA PNG packing every frame at scale 1.
    pub fn export_sprite_sheet(&self) -> Vec<u8> {
        self.export_sprite_sheet_scaled(1)
            .expect("scale 1 fits every canvas")
    }

    /// Sprite sheet with each cell nearest-neighbor scaled by `scale`.
    ///
    /// Columns and rows stay the frame grid from [`Self::sprite_sheet_layout`].
    /// Each frame cell becomes `frame_width * scale` by `frame_height * scale`
    /// pixels. Padding cells scale as transparent blocks. Scale 1 matches
    /// [`Self::export_sprite_sheet`].
    pub fn export_sprite_sheet_scaled(&self, scale: u32) -> Result<Vec<u8>, KernelError> {
        let scale = require_scale(scale)?;
        let (sheet_width, sheet_height) = self.scaled_sheet_size(scale)?;
        let len = rgba_byte_len(sheet_width, sheet_height, scale)?;
        let mut rgba = vec![0; len];
        let layout = self.sprite_sheet_layout();
        let cell_width = usize::try_from(mul_scale(u32::from(self.width()), scale)?)
            .map_err(|_| KernelError::ExportScale { scale })?;
        let cell_height = usize::try_from(mul_scale(u32::from(self.height()), scale)?)
            .map_err(|_| KernelError::ExportScale { scale })?;
        let palette = self.palette_colors();
        let sheet_stride =
            usize::try_from(sheet_width).map_err(|_| KernelError::ExportScale { scale })?;
        for index in 0..self.frame_count() {
            let (origin_x, origin_y) = layout.frame_origin(index).expect("every frame has a cell");
            let origin_x = mul_scale(origin_x, scale)?;
            let origin_y = mul_scale(origin_y, scale)?;
            let pixels = self
                .frame_pixels(index)
                .expect("frame index comes from the frame list");
            let scaled = nearest_indices(pixels, self.width(), self.height(), scale)?;
            blit_frame(
                &mut rgba,
                sheet_stride,
                usize::try_from(origin_x).map_err(|_| KernelError::ExportScale { scale })?,
                usize::try_from(origin_y).map_err(|_| KernelError::ExportScale { scale })?,
                cell_width,
                cell_height,
                &scaled,
                palette,
            );
        }
        Ok(encode_rgba_png(sheet_width, sheet_height, &rgba))
    }
}

fn require_scale(scale: u32) -> Result<u32, KernelError> {
    if scale == 0 {
        Err(KernelError::ExportScale { scale })
    } else {
        Ok(scale)
    }
}

fn mul_scale(edge: u32, scale: u32) -> Result<u32, KernelError> {
    if scale == 0 {
        return Err(KernelError::ExportScale { scale });
    }
    edge.checked_mul(scale)
        .ok_or(KernelError::ExportScale { scale })
}

fn rgba_byte_len(width: u32, height: u32, scale: u32) -> Result<usize, KernelError> {
    let width = usize::try_from(width).map_err(|_| KernelError::ExportScale { scale })?;
    let height = usize::try_from(height).map_err(|_| KernelError::ExportScale { scale })?;
    width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(KernelError::ExportScale { scale })
}

fn frame_rgba_scaled(
    palette: &[Rgba],
    pixels: &[u8],
    width: u16,
    height: u16,
    scale: u32,
) -> Result<(u32, u32, Vec<u8>), KernelError> {
    let scale = require_scale(scale)?;
    let out_width = mul_scale(u32::from(width), scale)?;
    let out_height = mul_scale(u32::from(height), scale)?;
    let indexed = nearest_indices(pixels, width, height, scale)?;
    let rgba = expand_indexed(palette, &indexed);
    Ok((out_width, out_height, rgba))
}

/// Repeats each index into a `scale` by `scale` block. Scale 1 clones `pixels`.
fn nearest_indices(
    pixels: &[u8],
    width: u16,
    height: u16,
    scale: u32,
) -> Result<Vec<u8>, KernelError> {
    let scale = require_scale(scale)?;
    if scale == 1 {
        return Ok(pixels.to_vec());
    }
    let out_width = mul_scale(u32::from(width), scale)?;
    let out_height = mul_scale(u32::from(height), scale)?;
    let len = usize::try_from(out_width)
        .ok()
        .and_then(|w| {
            usize::try_from(out_height)
                .ok()
                .and_then(|h| w.checked_mul(h))
        })
        .ok_or(KernelError::ExportScale { scale })?;
    let mut out = vec![0u8; len];
    let src_width = usize::from(width);
    let out_stride = usize::try_from(out_width).map_err(|_| KernelError::ExportScale { scale })?;
    let scale_usize = usize::try_from(scale).map_err(|_| KernelError::ExportScale { scale })?;
    for y in 0..u32::from(height) {
        for x in 0..u32::from(width) {
            let index = pixels[y as usize * src_width + x as usize];
            let x0 = (x as usize) * scale_usize;
            let y0 = (y as usize) * scale_usize;
            for dy in 0..scale_usize {
                let row = (y0 + dy) * out_stride;
                for dx in 0..scale_usize {
                    out[row + x0 + dx] = index;
                }
            }
        }
    }
    Ok(out)
}

/// Smallest `columns` whose square covers `frame_count`.
fn columns_for(frame_count: usize) -> u32 {
    let mut columns: u32 = 1;
    while (columns as usize).saturating_mul(columns as usize) < frame_count {
        columns += 1;
    }
    columns
}

fn expand_indexed(palette: &[Rgba], pixels: &[u8]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(pixels.len() * 4);
    for index in pixels {
        rgba.extend(rgba_of(palette, *index));
    }
    rgba
}

fn blit_frame(
    dest: &mut [u8],
    dest_width: usize,
    origin_x: usize,
    origin_y: usize,
    frame_width: usize,
    frame_height: usize,
    pixels: &[u8],
    palette: &[Rgba],
) {
    for y in 0..frame_height {
        for x in 0..frame_width {
            let color = rgba_of(palette, pixels[y * frame_width + x]);
            let dest_x = origin_x + x;
            let dest_y = origin_y + y;
            let offset = (dest_y * dest_width + dest_x) * 4;
            dest[offset..offset + 4].copy_from_slice(&color);
        }
    }
}

fn rgba_of(palette: &[Rgba], index: u8) -> [u8; 4] {
    match palette.get(usize::from(index)) {
        Some(color) => [color.r, color.g, color.b, color.a],
        None => [0, 0, 0, 0],
    }
}

fn encode_rgba_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    debug_assert_eq!(rgba.len(), width as usize * height as usize * 4);
    let mut bytes = Vec::new();
    {
        let mut encoder = Encoder::new(&mut bytes, width, height);
        encoder.set_color(ColorType::Rgba);
        encoder.set_depth(BitDepth::Eight);
        encoder
            .write_header()
            .expect("png header fits in memory")
            .write_image_data(rgba)
            .expect("png image data fits in memory");
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Palette;
    use png::Decoder;
    use std::io::Cursor;

    fn decode_rgba(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
        let decoder = Decoder::new(Cursor::new(bytes));
        let mut reader = decoder.read_info().expect("png header");
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).expect("png frame");
        assert_eq!(info.color_type, ColorType::Rgba);
        assert_eq!(info.bit_depth, BitDepth::Eight);
        buf.truncate(info.buffer_size());
        (info.width, info.height, buf)
    }

    fn doc() -> Document {
        Document::new(4, 2).unwrap()
    }

    #[test]
    fn layout_is_nearly_square_left_to_right() {
        let cases = [
            (1, 1, 1),
            (2, 2, 1),
            (3, 2, 2),
            (4, 2, 2),
            (5, 3, 2),
            (9, 3, 3),
            (10, 4, 3),
        ];
        for (count, columns, rows) in cases {
            let layout = sprite_sheet_layout(count, 16, 8);
            assert_eq!(layout.columns, columns, "columns for {count}");
            assert_eq!(layout.rows, rows, "rows for {count}");
            assert_eq!(layout.sheet_width(), columns * 16);
            assert_eq!(layout.sheet_height(), rows * 8);
            assert_eq!(layout.frame_origin(0), Some((0, 0)));
            if count > 1 {
                assert_eq!(layout.frame_origin(1), Some((16, 0)));
            }
            assert_eq!(layout.frame_origin(count), None);
        }

        let wrapped = sprite_sheet_layout(3, 4, 2);
        assert_eq!(wrapped.frame_origin(2), Some((0, 2)));
    }

    #[test]
    fn frame_png_resolves_palette_and_keeps_index_zero_clear() {
        let mut document = doc();
        document.paint(1, 0, 3).unwrap();
        document.set_palette_color(3, 10, 20, 30).unwrap();

        let png = document.export_frame_png(0).unwrap();
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
        let (width, height, rgba) = decode_rgba(&png);
        assert_eq!((width, height), (4, 2));
        assert_eq!(&rgba[0..4], &[0, 0, 0, 0]);
        assert_eq!(&rgba[4..8], &[10, 20, 30, 255]);
        assert!(rgba[8..].iter().all(|byte| *byte == 0));

        assert_eq!(
            document.export_frame_png(1).unwrap_err(),
            KernelError::FrameIndex { index: 1, len: 1 }
        );
        assert_eq!(document.frame_rgba(0).unwrap(), rgba);
        assert_eq!(
            document.frame_rgba(1).unwrap_err(),
            KernelError::FrameIndex { index: 1, len: 1 }
        );
    }

    #[test]
    fn frame_rgba_matches_each_frame_png_and_leaves_the_document() {
        let mut document = doc();
        document.paint(0, 0, 1).unwrap();
        document.add_frame();
        document.set_active_frame(1).unwrap();
        document.paint(3, 1, 2).unwrap();
        let before = document.clone();

        let first = document.frame_rgba(0).unwrap();
        let second = document.frame_rgba(1).unwrap();
        let (_, _, png_first) = decode_rgba(&document.export_frame_png(0).unwrap());
        let (_, _, png_second) = decode_rgba(&document.export_frame_png(1).unwrap());
        assert_eq!(first, png_first);
        assert_eq!(second, png_second);
        assert_ne!(first, second);
        assert_eq!(document, before);
    }

    #[test]
    fn partial_alpha_on_a_palette_entry_is_kept() {
        let palette = Palette::new(vec![Rgba::TRANSPARENT, Rgba::new(9, 8, 7, 128)]).unwrap();
        let mut document = Document::with_palette(2, 1, palette).unwrap();
        document.paint(1, 0, 1).unwrap();

        let (_, _, rgba) = decode_rgba(&document.export_frame_png(0).unwrap());
        assert_eq!(&rgba[0..4], &[0, 0, 0, 0]);
        assert_eq!(&rgba[4..8], &[9, 8, 7, 128]);
    }

    #[test]
    fn sequence_matches_each_frame_and_does_not_touch_history() {
        let mut document = doc();
        document.paint(0, 0, 1).unwrap();
        document.add_frame();
        document.set_active_frame(1).unwrap();
        document.paint(3, 1, 2).unwrap();
        document.clear_history();

        let sequence = document.export_png_sequence();
        assert_eq!(sequence.len(), 2);
        assert_eq!(sequence[0], document.export_frame_png(0).unwrap());
        assert_eq!(sequence[1], document.export_frame_png(1).unwrap());

        let (_, _, first) = decode_rgba(&sequence[0]);
        let (_, _, second) = decode_rgba(&sequence[1]);
        let ink = document.palette_colors()[1];
        let paper = document.palette_colors()[2];
        assert_eq!(&first[0..4], &[ink.r, ink.g, ink.b, ink.a]);
        assert!(first[4..].iter().all(|byte| *byte == 0));
        let last = (1 * 4 + 3) * 4;
        assert!(second[..last].iter().all(|byte| *byte == 0));
        assert_eq!(
            &second[last..last + 4],
            &[paper.r, paper.g, paper.b, paper.a]
        );

        assert!(!document.can_undo());
        assert!(!document.can_redo());
        assert_eq!(document.active_frame(), 1);
        assert_eq!(document.get(3, 1).unwrap(), 2);
    }

    #[test]
    fn sprite_sheet_packs_left_to_right_then_wraps() {
        let mut document = Document::new(2, 2).unwrap();
        document.paint(0, 0, 1).unwrap();
        document.add_frame();
        document.set_active_frame(1).unwrap();
        document.paint(1, 0, 2).unwrap();
        document.add_frame();
        document.set_active_frame(2).unwrap();
        document.paint(0, 1, 3).unwrap();

        let layout = document.sprite_sheet_layout();
        assert_eq!(layout.columns, 2);
        assert_eq!(layout.rows, 2);
        assert_eq!((layout.sheet_width(), layout.sheet_height()), (4, 4));

        let (width, height, rgba) = decode_rgba(&document.export_sprite_sheet());
        assert_eq!((width, height), (4, 4));

        let palette = document.palette_colors();
        assert_eq!(pixel_at(&rgba, 4, 0, 0), rgba_bytes(palette[1]));
        assert_eq!(pixel_at(&rgba, 4, 1, 0), [0, 0, 0, 0]);
        assert_eq!(pixel_at(&rgba, 4, 3, 0), rgba_bytes(palette[2]));
        assert_eq!(pixel_at(&rgba, 4, 0, 3), rgba_bytes(palette[3]));
        assert_eq!(pixel_at(&rgba, 4, 2, 2), [0, 0, 0, 0]);
        assert_eq!(pixel_at(&rgba, 4, 3, 3), [0, 0, 0, 0]);

        document.clear_history();
        let _ = document.export_sprite_sheet();
        assert!(!document.can_undo());
        assert_eq!(document.frame_count(), 3);
    }

    #[test]
    fn nearest_neighbor_scale_repeats_indices_and_leaves_the_document() {
        let mut document = doc();
        document.paint(1, 0, 3).unwrap();
        document.set_palette_color(3, 10, 20, 30).unwrap();
        document.add_frame();
        document.set_active_frame(1).unwrap();
        document.paint(0, 1, 2).unwrap();
        let before = document.clone();

        assert_eq!(document.scaled_frame_size(1).unwrap(), (4, 2));
        assert_eq!(document.scaled_frame_size(2).unwrap(), (8, 4));
        assert_eq!(
            document.export_frame_png_scaled(0, 1).unwrap(),
            document.export_frame_png(0).unwrap()
        );
        assert_eq!(
            document.export_png_sequence_scaled(1).unwrap(),
            document.export_png_sequence()
        );
        assert_eq!(
            document.export_sprite_sheet_scaled(1).unwrap(),
            document.export_sprite_sheet()
        );

        let (width, height, rgba) = decode_rgba(&document.export_frame_png_scaled(0, 2).unwrap());
        assert_eq!((width, height), (8, 4));
        let ink = [10, 20, 30, 255];
        for (x, y) in [(2, 0), (3, 0), (2, 1), (3, 1)] {
            assert_eq!(pixel_at(&rgba, 8, x, y), ink);
        }
        assert_eq!(pixel_at(&rgba, 8, 1, 0), [0, 0, 0, 0]);
        assert_eq!(pixel_at(&rgba, 8, 4, 0), [0, 0, 0, 0]);
        assert_eq!(pixel_at(&rgba, 8, 2, 2), [0, 0, 0, 0]);

        let sequence = document.export_png_sequence_scaled(2).unwrap();
        assert_eq!(sequence.len(), 2);
        assert_eq!(png_size_of(&sequence[0]), (8, 4));
        let (_, _, second) = decode_rgba(&sequence[1]);
        let paper = document.palette_colors()[2];
        for (x, y) in [(0, 2), (1, 2), (0, 3), (1, 3)] {
            assert_eq!(pixel_at(&second, 8, x, y), rgba_bytes(paper));
        }
        assert_eq!(pixel_at(&second, 8, 0, 1), [0, 0, 0, 0]);

        let (sheet_w, sheet_h) = document.scaled_sheet_size(2).unwrap();
        assert_eq!((sheet_w, sheet_h), (16, 4));
        let (width, height, sheet) = decode_rgba(&document.export_sprite_sheet_scaled(2).unwrap());
        assert_eq!((width, height), (16, 4));
        assert_eq!(pixel_at(&sheet, 16, 2, 0), ink);
        assert_eq!(pixel_at(&sheet, 16, 8, 2), rgba_bytes(paper));
        assert_eq!(pixel_at(&sheet, 16, 6, 0), [0, 0, 0, 0]);

        assert_eq!(
            document.export_frame_png_scaled(0, 0).unwrap_err(),
            KernelError::ExportScale { scale: 0 }
        );
        assert_eq!(
            document.export_sprite_sheet_scaled(0).unwrap_err(),
            KernelError::ExportScale { scale: 0 }
        );
        assert_eq!(document, before);
    }

    fn png_size_of(bytes: &[u8]) -> (u32, u32) {
        let (width, height, _) = decode_rgba(bytes);
        (width, height)
    }

    fn pixel_at(rgba: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
        let offset = (y * width + x) * 4;
        rgba[offset..offset + 4].try_into().unwrap()
    }

    fn rgba_bytes(color: Rgba) -> [u8; 4] {
        [color.r, color.g, color.b, color.a]
    }
}
