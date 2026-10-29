//! Transparent PNG export.
//!
//! Frames stay indexed inside the document. Export composites the layer stack
//! into RGBA and encodes that buffer. A hidden layer, or a layer at opacity 0,
//! is skipped. Index 0 is the transparent slot (`a = 0`). Opacity 255 replaces
//! the cell with the palette color, so palette alpha is not blended into the
//! layer underneath. Opacity 1..=254 blends with straight-alpha source-over
//! and is not snapped back to a palette index.
//!
//! A sequence is one PNG per frame, in timeline order. A sprite sheet packs
//! those same frames into one PNG. See [`sprite_sheet_layout`] for the grid.
//! `scale` repeats each composited cell into a square block (nearest neighbor,
//! no filtering). Scale 1 is the canvas size.
//! [`Document::frame_rgba`] returns that same resolved buffer for a view such
//! as onion skin. It is not stored and is not encoded into the file.

use png::{BitDepth, ColorType, Encoder};
use serde::Serialize;

use crate::document::Document;
use crate::error::KernelError;
use crate::selection::Mask;
use crate::Rgba;
use crate::LAYER_OPACITY_OPAQUE;

/// View-only color for a cell outside a preview or diff mask.
///
/// It is not a palette entry and it is not written into the document.
pub const PROTECT_RGBA: [u8; 4] = [80, 64, 112, 255];

/// ASCII mark for a cell outside a preview or diff mask.
pub const PROTECT_GLYPH: char = '#';

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

/// `format` field of a sprite-sheet atlas.
pub(crate) const ATLAS_FORMAT: &str = "pixelforge-atlas";

/// Version of [`SpriteAtlas`].
pub(crate) const ATLAS_VERSION: u32 = 1;

/// JSON beside a sprite sheet: frame rectangles, delays, columns, and tags.
///
/// Rectangles are in sheet pixels after `scale`. Padding cells are not frames.
/// This is not an Aseprite cel-link file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SpriteAtlas {
    pub format: &'static str,
    pub version: u32,
    pub columns: u32,
    pub rows: u32,
    pub scale: u32,
    pub frame_width: u32,
    pub frame_height: u32,
    pub sheet_width: u32,
    pub sheet_height: u32,
    pub frames: Vec<AtlasFrame>,
    pub tags: Vec<AtlasTag>,
}

/// One real frame inside a [`SpriteAtlas`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AtlasFrame {
    pub index: u32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub delay_ms: u32,
}

/// One tag inside a [`SpriteAtlas`]. `loop` is the loop mode name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AtlasTag {
    pub index: u32,
    pub name: String,
    pub from: u32,
    pub to: u32,
    #[serde(rename = "loop")]
    pub loop_mode: String,
}

/// Grid used by [`Document::export_sprite_sheet`] for this frame count and cell size.
///
/// Columns are `ceil(sqrt(frame_count))`. [`sprite_sheet_layout_with`] takes an
/// explicit column count instead.
pub fn sprite_sheet_layout(
    frame_count: usize,
    frame_width: u16,
    frame_height: u16,
) -> SpriteSheetLayout {
    sprite_sheet_layout_with(frame_count, frame_width, frame_height, None)
        .expect("the square grid fits a canvas this crate can store")
}

/// Same grid as [`sprite_sheet_layout`], or `columns` when that is `Some`.
///
/// `None` keeps `ceil(sqrt(frame_count))`. `Some(n)` uses `n` columns and
/// `ceil(frame_count / n)` rows, so a walk cycle can stay on one row. `n` may
/// be larger than the frame count; the extra cells are transparent padding.
/// `n == 0`, or a sheet that does not fit in `u32` pixels, is
/// [`KernelError::SheetColumns`].
pub fn sprite_sheet_layout_with(
    frame_count: usize,
    frame_width: u16,
    frame_height: u16,
    columns: Option<u32>,
) -> Result<SpriteSheetLayout, KernelError> {
    let frame_count = frame_count.max(1);
    let columns = match columns {
        None => columns_for(frame_count),
        Some(0) => return Err(KernelError::SheetColumns { columns: 0 }),
        Some(columns) => columns,
    };
    let count = u32::try_from(frame_count).map_err(|_| KernelError::SheetColumns { columns })?;
    let rows = count.div_ceil(columns);
    if u32::from(frame_width).checked_mul(columns).is_none()
        || u32::from(frame_height).checked_mul(rows).is_none()
    {
        return Err(KernelError::SheetColumns { columns });
    }
    Ok(SpriteSheetLayout {
        columns,
        rows,
        frame_width,
        frame_height,
        frame_count,
    })
}

/// Read-only comparison of two frames. Not stored on the document.
///
/// The PNG is three panels, left to right: `frame`, `with`, then an onion
/// blend. Where the indices match, the onion panel uses that palette color.
/// Where only one index is non-zero, it uses that color. Where both differ, it
/// averages the two palette colors. Nothing is written back into the frames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameDiff {
    pub frame: usize,
    pub with: usize,
    pub changed: usize,
    pub width: u16,
    pub height: u16,
    pub scale: u32,
    pub png_width: u32,
    pub png_height: u32,
    pub png: Vec<u8>,
    /// One string per row. `.` means the indices match. `!` means both are
    /// non-zero and different. Otherwise the glyph is the non-zero index.
    pub diff_rows: Vec<String>,
}

impl Document {
    /// Packed `r, g, b, a` for one frame's composite, row-major, length `width * height * 4`.
    ///
    /// Same composite as [`Self::export_frame_png`]. A hidden layer, or a layer
    /// at opacity 0, is skipped. Index 0 does not cover. Opacity 255 replaces
    /// the cell with that palette color, including its alpha, and does not
    /// blend into the layer underneath. Opacity 1..=254 blends with
    /// straight-alpha source-over: source alpha is
    /// `round(palette.a * opacity / 255)`, ties away from zero. The blend is
    /// not written back and is not snapped to a palette index. Onion skin
    /// reads this buffer. The bytes are not stored on the document.
    pub fn frame_rgba(&self, index: usize) -> Result<Vec<u8>, KernelError> {
        let layers = self.frame_layer_buffers(index)?;
        Ok(blend_layers(
            layers,
            self.layer_visibility_flags(),
            self.layer_opacity_values(),
            self.palette_colors(),
        ))
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
        self.export_view_png(index, scale, None)
    }

    /// [`Self::export_frame_png_scaled`] of the composite, or of one layer.
    ///
    /// `layer == None` is the RGBA composite. A set layer is that buffer alone,
    /// including its transparent cells, and ignores the layer's visibility and
    /// opacity so a hidden layer can still be inspected.
    pub fn export_view_png(
        &self,
        index: usize,
        scale: u32,
        layer: Option<usize>,
    ) -> Result<Vec<u8>, KernelError> {
        self.export_view_png_masked(index, scale, layer, None)
    }

    /// [`Self::export_view_png`], with cells outside `mask` painted as protect.
    ///
    /// `mask == None` is the whole canvas. The protect color is a view. It is
    /// not stored and it is not a palette index.
    pub fn export_view_png_masked(
        &self,
        index: usize,
        scale: u32,
        layer: Option<usize>,
        mask: Option<&Mask>,
    ) -> Result<Vec<u8>, KernelError> {
        let unscaled = match layer {
            Some(layer) => {
                let pixels = self.layer_pixels(index, layer)?;
                expand_indexed(self.palette_colors(), pixels)
            }
            None => self.frame_rgba(index)?,
        };
        let (width, height, rgba) =
            scale_rgba_masked(&unscaled, self.width(), self.height(), scale, mask)?;
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
        self.sprite_sheet_layout_with(None)
            .expect("the square grid fits a canvas this crate can store")
    }

    /// [`Self::sprite_sheet_layout`] or an explicit column count. See
    /// [`sprite_sheet_layout_with`].
    pub fn sprite_sheet_layout_with(
        &self,
        columns: Option<u32>,
    ) -> Result<SpriteSheetLayout, KernelError> {
        sprite_sheet_layout_with(self.frame_count(), self.width(), self.height(), columns)
    }

    /// Pixel size of the sprite sheet after nearest-neighbor `scale`.
    pub fn scaled_sheet_size(&self, scale: u32) -> Result<(u32, u32), KernelError> {
        self.scaled_sheet_size_with(scale, None)
    }

    /// [`Self::scaled_sheet_size`] with an explicit column count.
    pub fn scaled_sheet_size_with(
        &self,
        scale: u32,
        columns: Option<u32>,
    ) -> Result<(u32, u32), KernelError> {
        let scale = require_scale(scale)?;
        let layout = self.sprite_sheet_layout_with(columns)?;
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
        self.export_sprite_sheet_scaled_with(scale, None)
    }

    /// [`Self::export_sprite_sheet_scaled`] with an explicit column count.
    ///
    /// `columns == None` keeps the square grid. The document is not changed.
    pub fn export_sprite_sheet_scaled_with(
        &self,
        scale: u32,
        columns: Option<u32>,
    ) -> Result<Vec<u8>, KernelError> {
        let scale = require_scale(scale)?;
        let (sheet_width, sheet_height) = self.scaled_sheet_size_with(scale, columns)?;
        let len = rgba_byte_len(sheet_width, sheet_height, scale)?;
        let mut rgba = vec![0; len];
        let layout = self.sprite_sheet_layout_with(columns)?;
        let cell_width = usize::try_from(mul_scale(u32::from(self.width()), scale)?)
            .map_err(|_| KernelError::ExportScale { scale })?;
        let cell_height = usize::try_from(mul_scale(u32::from(self.height()), scale)?)
            .map_err(|_| KernelError::ExportScale { scale })?;
        let sheet_stride =
            usize::try_from(sheet_width).map_err(|_| KernelError::ExportScale { scale })?;
        for index in 0..self.frame_count() {
            let (origin_x, origin_y) = layout.frame_origin(index).expect("every frame has a cell");
            let origin_x = mul_scale(origin_x, scale)?;
            let origin_y = mul_scale(origin_y, scale)?;
            let pixels = self
                .frame_rgba(index)
                .expect("frame index comes from the frame list");
            let scaled = nearest_rgba(&pixels, self.width(), self.height(), scale)?;
            blit_rgba(
                &mut rgba,
                sheet_stride,
                usize::try_from(origin_x).map_err(|_| KernelError::ExportScale { scale })?,
                usize::try_from(origin_y).map_err(|_| KernelError::ExportScale { scale })?,
                cell_width,
                cell_height,
                &scaled,
            );
        }
        Ok(encode_rgba_png(sheet_width, sheet_height, &rgba))
    }

    /// Atlas for the same grid as [`Self::export_sprite_sheet_scaled_with`].
    ///
    /// Does not encode a PNG and does not change the document. Frame rectangles
    /// are in sheet pixels. `columns` is the frame grid; `scale` does not
    /// change it. Tags are the document's tags, in list order.
    pub(crate) fn sprite_sheet_atlas(
        &self,
        scale: u32,
        columns: Option<u32>,
    ) -> Result<SpriteAtlas, KernelError> {
        let scale = require_scale(scale)?;
        let layout = self.sprite_sheet_layout_with(columns)?;
        let (sheet_width, sheet_height) = self.scaled_sheet_size_with(scale, columns)?;
        let frame_width = mul_scale(u32::from(layout.frame_width), scale)?;
        let frame_height = mul_scale(u32::from(layout.frame_height), scale)?;
        let mut frames = Vec::with_capacity(self.frame_count());
        for index in 0..self.frame_count() {
            let (x, y) = layout.frame_origin(index).expect("every frame has a cell");
            frames.push(AtlasFrame {
                index: u32::try_from(index).expect("frame index fits in u32"),
                x: mul_scale(x, scale)?,
                y: mul_scale(y, scale)?,
                width: frame_width,
                height: frame_height,
                delay_ms: self.frame_delay(index).expect("index walks the frame list"),
            });
        }
        let tags = self
            .tags()
            .iter()
            .enumerate()
            .map(|(index, tag)| AtlasTag {
                index: u32::try_from(index).expect("tag index fits in u32"),
                name: tag.name.clone(),
                from: u32::try_from(tag.from).expect("frame index fits in u32"),
                to: u32::try_from(tag.to).expect("frame index fits in u32"),
                loop_mode: tag.loop_mode.as_str().to_string(),
            })
            .collect();
        Ok(SpriteAtlas {
            format: ATLAS_FORMAT,
            version: ATLAS_VERSION,
            columns: layout.columns,
            rows: layout.rows,
            scale,
            frame_width,
            frame_height,
            sheet_width,
            sheet_height,
            frames,
            tags,
        })
    }

    /// Side-by-side frames plus an index onion. Read-only. See [`FrameDiff`].
    pub fn diff_frames(
        &self,
        frame: usize,
        with: usize,
        scale: u32,
    ) -> Result<FrameDiff, KernelError> {
        self.diff_frames_on(frame, with, scale, None)
    }

    /// [`Self::diff_frames`] of the composite, or of one layer on both frames.
    pub fn diff_frames_on(
        &self,
        frame: usize,
        with: usize,
        scale: u32,
        layer: Option<usize>,
    ) -> Result<FrameDiff, KernelError> {
        self.diff_frames_masked(frame, with, scale, layer, None)
    }

    /// [`Self::diff_frames_on`], ignoring cells outside `mask`.
    ///
    /// Those cells are `#` in `diff_rows` and [`PROTECT_RGBA`] in every PNG
    /// panel. They are not part of `changed`. `mask == None` compares the
    /// whole canvas. Nothing is written back into the frames.
    pub fn diff_frames_masked(
        &self,
        frame: usize,
        with: usize,
        scale: u32,
        layer: Option<usize>,
        mask: Option<&Mask>,
    ) -> Result<FrameDiff, KernelError> {
        let left = self.view_pixels(frame, layer)?;
        let right = self.view_pixels(with, layer)?;
        let scale = require_scale(scale)?;
        let width = self.width();
        let height = self.height();
        let changed = left
            .iter()
            .zip(right.iter())
            .enumerate()
            .filter(|(offset, (a, b))| a != b && cell_in_mask(mask, *offset, width))
            .count();
        let diff_rows = diff_rows(&left, &right, width, mask);
        let (png_width, png_height, rgba) = diff_rgba(
            self.palette_colors(),
            &left,
            &right,
            width,
            height,
            scale,
            mask,
        )?;
        Ok(FrameDiff {
            frame,
            with,
            changed,
            width,
            height,
            scale,
            png_width,
            png_height,
            png: encode_rgba_png(png_width, png_height, &rgba),
            diff_rows,
        })
    }
}

fn cell_in_mask(mask: Option<&Mask>, offset: usize, width: u16) -> bool {
    let Some(mask) = mask else {
        return true;
    };
    let stride = usize::from(width);
    let x = u16::try_from(offset % stride).expect("column fits in u16");
    let y = u16::try_from(offset / stride).expect("row fits in u16");
    mask.contains(x, y)
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

fn scale_rgba_masked(
    rgba: &[u8],
    width: u16,
    height: u16,
    scale: u32,
    mask: Option<&Mask>,
) -> Result<(u32, u32, Vec<u8>), KernelError> {
    let scale = require_scale(scale)?;
    let out_width = mul_scale(u32::from(width), scale)?;
    let out_height = mul_scale(u32::from(height), scale)?;
    let mut rgba = nearest_rgba(rgba, width, height, scale)?;
    if let Some(mask) = mask {
        let scale_usize = usize::try_from(scale).map_err(|_| KernelError::ExportScale { scale })?;
        let out_stride =
            usize::try_from(out_width).map_err(|_| KernelError::ExportScale { scale })?;
        for y in 0..usize::from(height) {
            for x in 0..usize::from(width) {
                if mask.contains(x as u16, y as u16) {
                    continue;
                }
                let x0 = x * scale_usize;
                let y0 = y * scale_usize;
                for dy in 0..scale_usize {
                    let row = (y0 + dy) * out_stride;
                    for dx in 0..scale_usize {
                        let offset = (row + x0 + dx) * 4;
                        rgba[offset..offset + 4].copy_from_slice(&PROTECT_RGBA);
                    }
                }
            }
        }
    }
    Ok((out_width, out_height, rgba))
}

/// Repeats each RGBA cell into a `scale` by `scale` block. Scale 1 clones `rgba`.
fn nearest_rgba(rgba: &[u8], width: u16, height: u16, scale: u32) -> Result<Vec<u8>, KernelError> {
    let scale = require_scale(scale)?;
    if scale == 1 {
        return Ok(rgba.to_vec());
    }
    let out_width = mul_scale(u32::from(width), scale)?;
    let out_height = mul_scale(u32::from(height), scale)?;
    let len = rgba_byte_len(out_width, out_height, scale)?;
    let mut out = vec![0u8; len];
    let src_width = usize::from(width);
    let out_stride = usize::try_from(out_width).map_err(|_| KernelError::ExportScale { scale })?;
    let scale_usize = usize::try_from(scale).map_err(|_| KernelError::ExportScale { scale })?;
    for y in 0..usize::from(height) {
        for x in 0..src_width {
            let pixel = &rgba[(y * src_width + x) * 4..(y * src_width + x) * 4 + 4];
            let x0 = x * scale_usize;
            let y0 = y * scale_usize;
            for dy in 0..scale_usize {
                let row = (y0 + dy) * out_stride;
                for dx in 0..scale_usize {
                    let offset = (row + x0 + dx) * 4;
                    out[offset..offset + 4].copy_from_slice(pixel);
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

/// Bottom-to-top RGBA composite.
///
/// Hidden layers and opacity 0 are skipped. Index 0 does not cover. Opacity
/// [`LAYER_OPACITY_OPAQUE`] replaces the cell with the palette color. A lower
/// opacity blends with straight-alpha source-over and does not invent an index.
fn blend_layers(layers: &[Vec<u8>], visible: &[bool], opacity: &[u8], palette: &[Rgba]) -> Vec<u8> {
    debug_assert_eq!(visible.len(), layers.len());
    debug_assert_eq!(opacity.len(), layers.len());
    let mut out = vec![0u8; layers[0].len() * 4];
    for (layer, (&shown, &layer_opacity)) in layers.iter().zip(visible.iter().zip(opacity.iter())) {
        if !shown || layer_opacity == 0 {
            continue;
        }
        for (cell, index) in layer.iter().copied().enumerate() {
            if index == 0 {
                continue;
            }
            let color = rgba_of(palette, index);
            let offset = cell * 4;
            if layer_opacity == LAYER_OPACITY_OPAQUE {
                out[offset..offset + 4].copy_from_slice(&color);
            } else {
                let mut pixel = [
                    out[offset],
                    out[offset + 1],
                    out[offset + 2],
                    out[offset + 3],
                ];
                source_over(&mut pixel, color, mul_u8(color[3], layer_opacity));
                out[offset..offset + 4].copy_from_slice(&pixel);
            }
        }
    }
    out
}

/// `round(left * right / 255)`, ties away from zero.
fn mul_u8(left: u8, right: u8) -> u8 {
    u8::try_from((u16::from(left) * u16::from(right) + 127) / 255).expect("product fits in u8")
}

/// Straight-alpha source-over. `src_a` is the source alpha after layer opacity.
///
/// Channel rounding is nearest, ties away from zero (`+ 127` before `/ 255`,
/// and `+ out_a / 2` before `/ out_a`).
fn source_over(dst: &mut [u8; 4], src: [u8; 4], src_a: u8) {
    if src_a == 0 {
        return;
    }
    let inv = 255u32 - u32::from(src_a);
    let out_a = u32::from(src_a) + (u32::from(dst[3]) * inv + 127) / 255;
    if out_a == 0 {
        *dst = [0, 0, 0, 0];
        return;
    }
    for channel in 0..3 {
        let src_term = u32::from(src[channel]) * u32::from(src_a);
        let dst_term = (u32::from(dst[channel]) * u32::from(dst[3]) * inv + 127) / 255;
        let mixed = (src_term + dst_term + out_a / 2) / out_a;
        dst[channel] = u8::try_from(mixed.min(255)).expect("channel was clamped");
    }
    dst[3] = u8::try_from(out_a.min(255)).expect("alpha was clamped");
}

fn expand_indexed(palette: &[Rgba], pixels: &[u8]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(pixels.len() * 4);
    for index in pixels {
        rgba.extend(rgba_of(palette, *index));
    }
    rgba
}

fn blit_rgba(
    dest: &mut [u8],
    dest_width: usize,
    origin_x: usize,
    origin_y: usize,
    frame_width: usize,
    frame_height: usize,
    rgba: &[u8],
) {
    for y in 0..frame_height {
        for x in 0..frame_width {
            let src = (y * frame_width + x) * 4;
            let dest_x = origin_x + x;
            let dest_y = origin_y + y;
            let offset = (dest_y * dest_width + dest_x) * 4;
            dest[offset..offset + 4].copy_from_slice(&rgba[src..src + 4]);
        }
    }
}

fn rgba_of(palette: &[Rgba], index: u8) -> [u8; 4] {
    match palette.get(usize::from(index)) {
        Some(color) => [color.r, color.g, color.b, color.a],
        None => [0, 0, 0, 0],
    }
}

fn diff_rows(left: &[u8], right: &[u8], width: u16, mask: Option<&Mask>) -> Vec<String> {
    let stride = usize::from(width);
    left.chunks(stride)
        .zip(right.chunks(stride))
        .enumerate()
        .map(|(y, (row_a, row_b))| {
            row_a
                .iter()
                .zip(row_b.iter())
                .enumerate()
                .map(|(x, (a, b))| {
                    if mask.is_some_and(|mask| !mask.contains(x as u16, y as u16)) {
                        PROTECT_GLYPH
                    } else {
                        diff_glyph(*a, *b)
                    }
                })
                .collect()
        })
        .collect()
}

fn diff_glyph(left: u8, right: u8) -> char {
    if left == right {
        '.'
    } else if left != 0 && right != 0 {
        '!'
    } else if right != 0 {
        crate::cell_glyph(right)
    } else {
        crate::cell_glyph(left)
    }
}

fn onion_color(palette: &[Rgba], left: u8, right: u8) -> [u8; 4] {
    if left == right {
        return rgba_of(palette, left);
    }
    let a = rgba_of(palette, left);
    let b = rgba_of(palette, right);
    if a[3] == 0 {
        return b;
    }
    if b[3] == 0 {
        return a;
    }
    [
        ((u16::from(a[0]) + u16::from(b[0])) / 2) as u8,
        ((u16::from(a[1]) + u16::from(b[1])) / 2) as u8,
        ((u16::from(a[2]) + u16::from(b[2])) / 2) as u8,
        a[3].max(b[3]),
    ]
}

/// Three panels: frame, with, onion. Each source cell is a `scale` block.
fn diff_rgba(
    palette: &[Rgba],
    left: &[u8],
    right: &[u8],
    width: u16,
    height: u16,
    scale: u32,
    mask: Option<&Mask>,
) -> Result<(u32, u32, Vec<u8>), KernelError> {
    let cell_width = mul_scale(u32::from(width), scale)?;
    let cell_height = mul_scale(u32::from(height), scale)?;
    let png_width = cell_width
        .checked_mul(3)
        .ok_or(KernelError::ExportScale { scale })?;
    let png_height = cell_height;
    let len = rgba_byte_len(png_width, png_height, scale)?;
    let mut rgba = vec![0; len];
    let stride = usize::try_from(png_width).map_err(|_| KernelError::ExportScale { scale })?;
    let scale_usize = usize::try_from(scale).map_err(|_| KernelError::ExportScale { scale })?;
    let src_width = usize::from(width);
    for y in 0..usize::from(height) {
        for x in 0..src_width {
            let index = y * src_width + x;
            let outside = mask.is_some_and(|mask| !mask.contains(x as u16, y as u16));
            let colors = if outside {
                [PROTECT_RGBA, PROTECT_RGBA, PROTECT_RGBA]
            } else {
                [
                    rgba_of(palette, left[index]),
                    rgba_of(palette, right[index]),
                    onion_color(palette, left[index], right[index]),
                ]
            };
            for (panel, color) in colors.into_iter().enumerate() {
                let x0 = panel
                    * usize::try_from(cell_width)
                        .map_err(|_| KernelError::ExportScale { scale })?
                    + x * scale_usize;
                let y0 = y * scale_usize;
                for dy in 0..scale_usize {
                    let row = (y0 + dy) * stride;
                    for dx in 0..scale_usize {
                        let offset = (row + x0 + dx) * 4;
                        rgba[offset..offset + 4].copy_from_slice(&color);
                    }
                }
            }
        }
    }
    Ok((png_width, png_height, rgba))
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

        let wide = document
            .export_sprite_sheet_scaled_with(1, Some(2))
            .unwrap();
        let (width, height, _) = decode_rgba(&wide);
        assert_eq!((width, height), (8, 2));
        assert_eq!(document.sprite_sheet_layout_with(Some(2)).unwrap().rows, 1);
        assert_eq!(
            document.sprite_sheet_layout_with(Some(0)).unwrap_err(),
            KernelError::SheetColumns { columns: 0 }
        );

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

    #[test]
    fn diff_frames_marks_index_changes_and_blends_only_the_view() {
        let mut document = Document::new(4, 1).unwrap();
        document.paint(0, 0, 1).unwrap();
        document.paint(1, 0, 2).unwrap();
        document.set_palette_color(1, 10, 0, 0).unwrap();
        document.set_palette_color(2, 0, 20, 0).unwrap();
        document.add_frame();
        document.set_active_frame(1).unwrap();
        document.paint(1, 0, 1).unwrap();
        document.paint(2, 0, 1).unwrap();
        let before = document.clone();

        let diff = document.diff_frames(0, 1, 2).unwrap();
        assert_eq!(diff.changed, 3);
        assert_eq!(diff.diff_rows, vec!["1!1.".to_string()]);
        assert_eq!((diff.png_width, diff.png_height), (24, 2));
        assert_eq!(document, before);
        assert_eq!(
            document.diff_frames(0, 3, 1).unwrap_err(),
            KernelError::FrameIndex { index: 3, len: 2 }
        );

        let (_, _, rgba) = decode_rgba(&diff.png);
        let red = [10, 0, 0, 255];
        let green = [0, 20, 0, 255];
        assert_eq!(pixel_at(&rgba, 24, 0, 0), red);
        assert_eq!(pixel_at(&rgba, 24, 2, 0), green);
        assert_eq!(pixel_at(&rgba, 24, 8, 0), [0, 0, 0, 0]);
        assert_eq!(pixel_at(&rgba, 24, 10, 0), red);
        assert_eq!(pixel_at(&rgba, 24, 16, 0), red);
        assert_eq!(pixel_at(&rgba, 24, 18, 0), [5, 10, 0, 255]);
        assert_eq!(pixel_at(&rgba, 24, 20, 0), red);

        let same = document.diff_frames(1, 1, 1).unwrap();
        assert_eq!(same.changed, 0);
        assert_eq!(same.diff_rows, vec!["....".to_string()]);
    }

    #[test]
    fn a_mask_keeps_onion_pixels_inside_and_paints_protect_outside() {
        let mut document = Document::new(4, 1).unwrap();
        document.paint(0, 0, 1).unwrap();
        document.paint(1, 0, 2).unwrap();
        document.add_frame();
        document.set_active_frame(1).unwrap();
        document.paint(1, 0, 1).unwrap();
        document.paint(3, 0, 1).unwrap();
        let helmet = Mask::from_rect(crate::Selection {
            x: 1,
            y: 0,
            width: 1,
            height: 1,
        });
        let diff = document
            .diff_frames_masked(0, 1, 1, None, Some(&helmet))
            .unwrap();
        assert_eq!(diff.changed, 1);
        assert_eq!(diff.diff_rows, vec!["#!##".to_string()]);
        let (_, _, rgba) = decode_rgba(&diff.png);
        assert_eq!(pixel_at(&rgba, 12, 0, 0), PROTECT_RGBA);
        assert_eq!(pixel_at(&rgba, 12, 8, 0), PROTECT_RGBA);
        assert_ne!(pixel_at(&rgba, 12, 1, 0), PROTECT_RGBA);
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
