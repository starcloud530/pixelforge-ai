//! Indexed `.aseprite` / `.ase` import.
//!
//! Reads canvas size, the indexed palette, image-layer pixels, and frame
//! durations. Group folders are not stored: their children become a flat
//! stack in file order (bottom to top). The kernel has no blend mode, no
//! per-layer opacity, and no hide flag, so those Aseprite fields are counted
//! and then dropped.
//!
//! `asefile` composites through the `image` crate and returns RGBA.
//! `aseprite-io` can write the format back, and its stated minimum Rust is
//! newer than this repo. This reader stops at palette indices. It does not
//! write `.aseprite` and it does not load plugins.

use std::collections::BTreeSet;
use std::io::Read;

use flate2::read::ZlibDecoder;

use crate::document::Document;
use crate::error::KernelError;
use crate::frame::Frame;
use crate::palette::{IndexPolicy, Palette};
use crate::Rgba;
use crate::DEFAULT_FRAME_DELAY_MS;
use crate::DEFAULT_LAYER_NAME;
use crate::MAX_CANVAS_EDGE;
use crate::MAX_LAYER_NAME_CHARS;

/// Widest indexed cel this importer will inflate, on either edge.
const MAX_CEL_EDGE: u16 = MAX_CANVAS_EDGE;

/// `width * height * imported layers * frames`.
const MAX_IMPORT_CELLS: u64 = 1 << 24;

const HEADER_LEN: usize = 128;
const MAGIC: u16 = 0xA5E0;
const FRAME_MAGIC: u16 = 0xF1FA;

const CHUNK_OLD_PALETTE: u16 = 0x0004;
const CHUNK_OLD_PALETTE_6: u16 = 0x0011;
const CHUNK_LAYER: u16 = 0x2004;
const CHUNK_CEL: u16 = 0x2005;
const CHUNK_PALETTE: u16 = 0x2019;

const LAYER_VISIBLE: u16 = 1;
const LAYER_BACKGROUND: u16 = 8;
const LAYER_REFERENCE: u16 = 64;
const HEADER_OPACITY_VALID: u32 = 1;
const HEADER_LAYER_UUID: u32 = 4;

/// What [`Document::from_aseprite`] kept, dropped, or remapped.
///
/// Counts are about the file, not about pixels the caller painted afterwards.
/// `ignored` is sorted and names chunk types that were skipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsepriteReport {
    pub source_transparent_index: u8,
    pub transparent_swapped: bool,
    pub hidden_layers: u32,
    pub skipped_groups: u32,
    pub skipped_reference: u32,
    pub background_layers: u32,
    pub non_normal_blend: u32,
    pub partial_opacity: u32,
    pub partial_cel_opacity: u32,
    pub nonzero_z_index: u32,
    pub duration_defaulted: u32,
    pub clipped_cells: u32,
    pub ignored: Vec<String>,
}

struct Parsed {
    width: u16,
    height: u16,
    transparent_index: u8,
    header_speed: u16,
    opacity_valid: bool,
    has_uuid: bool,
    layers: Vec<LayerRec>,
    frames: Vec<ParsedFrame>,
    palette: Vec<Rgba>,
    saw_new_palette: bool,
    saw_old_palette: bool,
    saw_old_palette_6: bool,
    ignored: BTreeSet<String>,
}

struct LayerRec {
    flags: u16,
    kind: u16,
    blend: u16,
    opacity: u8,
    name: String,
}

struct ParsedFrame {
    duration: u16,
    cels: Vec<CelRec>,
}

struct CelRec {
    layer: u16,
    x: i16,
    y: i16,
    opacity: u8,
    z_index: i16,
    image: CelImage,
}

enum CelImage {
    Raw {
        width: u16,
        height: u16,
        pixels: Vec<u8>,
    },
    Link {
        frame: u16,
    },
}

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn seek(&mut self, pos: usize) -> Result<(), KernelError> {
        if pos > self.data.len() {
            return Err(ase("truncated"));
        }
        self.pos = pos;
        Ok(())
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], KernelError> {
        if self.remaining() < len {
            return Err(ase("truncated"));
        }
        let start = self.pos;
        self.pos += len;
        Ok(&self.data[start..self.pos])
    }

    fn skip(&mut self, len: usize) -> Result<(), KernelError> {
        self.take(len).map(|_| ())
    }

    fn rest(&mut self) -> &'a [u8] {
        let start = self.pos;
        self.pos = self.data.len();
        &self.data[start..]
    }

    fn u8(&mut self) -> Result<u8, KernelError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, KernelError> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn i16(&mut self) -> Result<i16, KernelError> {
        Ok(self.u16()? as i16)
    }

    fn u32(&mut self) -> Result<u32, KernelError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn string(&mut self) -> Result<String, KernelError> {
        let len = usize::from(self.u16()?);
        let bytes = self.take(len)?;
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }
}

fn ase(reason: &'static str) -> KernelError {
    KernelError::AsepriteFile { reason }
}

impl Document {
    /// Builds a new document from indexed `.aseprite` bytes.
    ///
    /// The result has an empty undo stack, no selection, no tags, an unlocked
    /// palette, and `indexPolicy` `reject`. Frame 0 and the bottom layer are
    /// active. This does not merge into an existing document.
    pub fn from_aseprite(bytes: &[u8]) -> Result<(Self, AsepriteReport), KernelError> {
        let parsed = parse_file(bytes)?;
        assemble(parsed)
    }
}

fn parse_file(bytes: &[u8]) -> Result<Parsed, KernelError> {
    if bytes.len() < HEADER_LEN {
        return Err(ase("truncated"));
    }
    let file_size = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    if file_size < HEADER_LEN || file_size > bytes.len() {
        return Err(ase("truncated"));
    }
    let mut cursor = Cursor::new(&bytes[..file_size]);
    let _size = cursor.u32()?;
    let magic = cursor.u16()?;
    if magic != MAGIC {
        return Err(ase("magic is not 0xA5E0"));
    }
    let frame_count = cursor.u16()?;
    let width = cursor.u16()?;
    let height = cursor.u16()?;
    let depth = cursor.u16()?;
    let header_flags = cursor.u32()?;
    let header_speed = cursor.u16()?;
    cursor.skip(8)?;
    let transparent_index = cursor.u8()?;
    cursor.skip(3)?;
    let _color_count = cursor.u16()?;
    cursor.seek(HEADER_LEN)?;

    if frame_count == 0 {
        return Err(ase("no frames"));
    }
    if depth != 8 {
        return Err(KernelError::AsepriteColorMode { depth });
    }
    if width == 0 || height == 0 || width > MAX_CANVAS_EDGE || height > MAX_CANVAS_EDGE {
        return Err(KernelError::AsepriteSize { width, height });
    }

    let mut parsed = Parsed {
        width,
        height,
        transparent_index,
        header_speed,
        opacity_valid: header_flags & HEADER_OPACITY_VALID != 0,
        has_uuid: header_flags & HEADER_LAYER_UUID != 0,
        layers: Vec::new(),
        frames: Vec::with_capacity(usize::from(frame_count)),
        palette: Vec::new(),
        saw_new_palette: false,
        saw_old_palette: false,
        saw_old_palette_6: false,
        ignored: BTreeSet::new(),
    };

    for _ in 0..frame_count {
        let frame = parse_frame(&mut cursor, &mut parsed)?;
        parsed.frames.push(frame);
    }
    Ok(parsed)
}

fn parse_frame(cursor: &mut Cursor<'_>, parsed: &mut Parsed) -> Result<ParsedFrame, KernelError> {
    let frame_start = cursor.pos;
    let frame_size = cursor.u32()? as usize;
    if frame_size < 16 {
        return Err(ase("truncated"));
    }
    let frame_end = frame_start
        .checked_add(frame_size)
        .ok_or_else(|| ase("truncated"))?;
    if frame_end > cursor.data.len() {
        return Err(ase("truncated"));
    }
    let magic = cursor.u16()?;
    if magic != FRAME_MAGIC {
        return Err(ase("frame magic is not 0xF1FA"));
    }
    let old_chunks = cursor.u16()?;
    let duration = cursor.u16()?;
    cursor.skip(2)?;
    let new_chunks = cursor.u32()?;
    let chunk_count = if old_chunks == 0xFFFF || new_chunks != 0 {
        new_chunks
    } else {
        u32::from(old_chunks)
    };
    let mut frame = ParsedFrame {
        duration,
        cels: Vec::new(),
    };
    for _ in 0..chunk_count {
        if cursor.pos >= frame_end {
            return Err(ase("truncated"));
        }
        parse_chunk(cursor, frame_end, parsed, &mut frame)?;
    }
    cursor.seek(frame_end)?;
    Ok(frame)
}

fn parse_chunk(
    cursor: &mut Cursor<'_>,
    frame_end: usize,
    parsed: &mut Parsed,
    frame: &mut ParsedFrame,
) -> Result<(), KernelError> {
    let chunk_start = cursor.pos;
    let chunk_size = cursor.u32()? as usize;
    if chunk_size < 6 {
        return Err(ase("chunk is shorter than 6 bytes"));
    }
    let chunk_end = chunk_start
        .checked_add(chunk_size)
        .ok_or_else(|| ase("truncated"))?;
    if chunk_end > frame_end {
        return Err(ase("truncated"));
    }
    let kind = cursor.u16()?;
    let data = cursor.take(chunk_end - cursor.pos)?;
    match kind {
        CHUNK_LAYER => parsed.layers.push(parse_layer(data, parsed.has_uuid)?),
        CHUNK_CEL => frame.cels.push(parse_cel(data)?),
        CHUNK_PALETTE => {
            let replace = !parsed.saw_new_palette;
            parse_new_palette(data, &mut parsed.palette, replace)?;
            parsed.saw_new_palette = true;
        }
        CHUNK_OLD_PALETTE | CHUNK_OLD_PALETTE_6 => {
            if parsed.saw_new_palette {
                if kind == CHUNK_OLD_PALETTE {
                    parsed.saw_old_palette = true;
                } else {
                    parsed.saw_old_palette_6 = true;
                }
            } else {
                let six_bit = kind == CHUNK_OLD_PALETTE_6;
                parse_old_palette(data, &mut parsed.palette, six_bit)?;
                if six_bit {
                    parsed.saw_old_palette_6 = true;
                } else {
                    parsed.saw_old_palette = true;
                }
            }
        }
        other => {
            parsed.ignored.insert(chunk_label(other));
        }
    }
    Ok(())
}

fn chunk_label(kind: u16) -> String {
    match kind {
        0x2006 => "celExtra".to_string(),
        0x2007 => "colorProfile".to_string(),
        0x2008 => "externalFiles".to_string(),
        0x2016 => "mask".to_string(),
        0x2017 => "path".to_string(),
        0x2018 => "tags".to_string(),
        0x2020 => "userData".to_string(),
        0x2022 => "slice".to_string(),
        0x2023 => "tileset".to_string(),
        _ => format!("unknown:{kind:#06x}"),
    }
}

fn parse_layer(data: &[u8], has_uuid: bool) -> Result<LayerRec, KernelError> {
    let mut cursor = Cursor::new(data);
    let flags = cursor.u16()?;
    let kind = cursor.u16()?;
    let _child = cursor.u16()?;
    let _default_width = cursor.u16()?;
    let _default_height = cursor.u16()?;
    let blend = cursor.u16()?;
    let opacity = cursor.u8()?;
    cursor.skip(3)?;
    let name = cursor.string()?;
    if kind == 2 {
        let _tileset = cursor.u32()?;
    }
    if has_uuid {
        cursor.skip(16)?;
    }
    Ok(LayerRec {
        flags,
        kind,
        blend,
        opacity,
        name,
    })
}

fn parse_cel(data: &[u8]) -> Result<CelRec, KernelError> {
    let mut cursor = Cursor::new(data);
    let layer = cursor.u16()?;
    let x = cursor.i16()?;
    let y = cursor.i16()?;
    let opacity = cursor.u8()?;
    let cel_type = cursor.u16()?;
    let z_index = cursor.i16()?;
    cursor.skip(5)?;
    let image = match cel_type {
        0 => read_raw(&mut cursor)?,
        1 => CelImage::Link {
            frame: cursor.u16()?,
        },
        2 => read_compressed(&mut cursor)?,
        3 => return Err(KernelError::AsepriteTilemap),
        _ => return Err(ase("unknown cel type")),
    };
    Ok(CelRec {
        layer,
        x,
        y,
        opacity,
        z_index,
        image,
    })
}

fn read_raw(cursor: &mut Cursor<'_>) -> Result<CelImage, KernelError> {
    let width = cursor.u16()?;
    let height = cursor.u16()?;
    let pixels = read_indices(cursor, width, height, false)?;
    Ok(CelImage::Raw {
        width,
        height,
        pixels,
    })
}

fn read_compressed(cursor: &mut Cursor<'_>) -> Result<CelImage, KernelError> {
    let width = cursor.u16()?;
    let height = cursor.u16()?;
    let pixels = read_indices(cursor, width, height, true)?;
    Ok(CelImage::Raw {
        width,
        height,
        pixels,
    })
}

fn read_indices(
    cursor: &mut Cursor<'_>,
    width: u16,
    height: u16,
    compressed: bool,
) -> Result<Vec<u8>, KernelError> {
    if width > MAX_CEL_EDGE || height > MAX_CEL_EDGE {
        return Err(ase("cel edge is outside 0..=256"));
    }
    let len = usize::from(width) * usize::from(height);
    if len == 0 {
        return Ok(Vec::new());
    }
    if compressed {
        let src = cursor.rest().to_vec();
        inflate(&src, len)
    } else {
        Ok(cursor.take(len)?.to_vec())
    }
}

fn inflate(src: &[u8], expected: usize) -> Result<Vec<u8>, KernelError> {
    let mut decoder = ZlibDecoder::new(src);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(|_| ase("zlib inflate failed"))?;
    if out.len() != expected {
        return Err(ase("compressed cel length does not match the image"));
    }
    Ok(out)
}

fn parse_new_palette(
    data: &[u8],
    palette: &mut Vec<Rgba>,
    replace: bool,
) -> Result<(), KernelError> {
    let mut cursor = Cursor::new(data);
    let size = cursor.u32()? as usize;
    let first = cursor.u32()? as usize;
    let last = cursor.u32()? as usize;
    cursor.skip(8)?;
    if size == 0 || size > usize::from(u8::MAX) + 1 || first > last || last >= size {
        return Err(KernelError::AsepritePalette);
    }
    if replace {
        *palette = vec![Rgba::rgb(0, 0, 0); size];
        palette[0] = Rgba::TRANSPARENT;
    } else {
        resize_palette(palette, size);
    }
    for index in first..=last {
        let flags = cursor.u16()?;
        let red = cursor.u8()?;
        let green = cursor.u8()?;
        let blue = cursor.u8()?;
        let alpha = cursor.u8()?;
        palette[index] = Rgba::new(red, green, blue, alpha);
        if flags & 1 != 0 {
            let _name = cursor.string()?;
        }
    }
    Ok(())
}

fn parse_old_palette(
    data: &[u8],
    palette: &mut Vec<Rgba>,
    six_bit: bool,
) -> Result<(), KernelError> {
    let mut cursor = Cursor::new(data);
    let packets = cursor.u16()?;
    let mut index: usize = 0;
    for _ in 0..packets {
        let skip = usize::from(cursor.u8()?);
        let count_byte = cursor.u8()?;
        let count = if count_byte == 0 {
            256
        } else {
            usize::from(count_byte)
        };
        index = index.saturating_add(skip);
        for _ in 0..count {
            if index > usize::from(u8::MAX) {
                return Err(KernelError::AsepritePalette);
            }
            let red = scale_old(cursor.u8()?, six_bit);
            let green = scale_old(cursor.u8()?, six_bit);
            let blue = scale_old(cursor.u8()?, six_bit);
            ensure_palette(palette, index + 1);
            palette[index] = Rgba::rgb(red, green, blue);
            index += 1;
        }
    }
    if palette.is_empty() {
        return Err(KernelError::AsepritePalette);
    }
    Ok(())
}

fn scale_old(value: u8, six_bit: bool) -> u8 {
    if six_bit {
        let value = u16::from(value.min(63));
        u8::try_from(value * 255 / 63).unwrap_or(255)
    } else {
        value
    }
}

fn resize_palette(palette: &mut Vec<Rgba>, size: usize) {
    if palette.len() < size {
        let start = palette.len();
        palette.resize(size, Rgba::rgb(0, 0, 0));
        if start == 0 && size > 0 {
            palette[0] = Rgba::TRANSPARENT;
        }
    } else if palette.len() > size {
        palette.truncate(size);
    }
}

fn ensure_palette(palette: &mut Vec<Rgba>, size: usize) {
    resize_palette(palette, size.max(palette.len()));
}

fn assemble(mut parsed: Parsed) -> Result<(Document, AsepriteReport), KernelError> {
    if parsed.layers.iter().any(|layer| layer.kind == 2) {
        return Err(KernelError::AsepriteTilemap);
    }
    if parsed.layers.iter().any(|layer| layer.kind > 2) {
        return Err(ase("unknown layer type"));
    }
    if parsed.saw_new_palette && parsed.saw_old_palette {
        parsed.ignored.insert("oldPalette".to_string());
    }
    if parsed.saw_new_palette && parsed.saw_old_palette_6 {
        parsed.ignored.insert("oldPalette2".to_string());
    }
    if parsed.palette.is_empty() {
        return Err(KernelError::AsepritePalette);
    }

    let (names, layer_map, mut report) = map_layers(&parsed)?;
    let transparent = parsed.transparent_index;
    if usize::from(transparent) >= parsed.palette.len() {
        return Err(KernelError::AsepritePalette);
    }
    let swapped = transparent != 0;
    if swapped {
        parsed.palette.swap(0, usize::from(transparent));
    }
    let palette_len = parsed.palette.len();
    parsed.palette[0] = Rgba::TRANSPARENT;
    let palette = Palette::new(parsed.palette)?;

    let cells = u64::from(parsed.width)
        * u64::from(parsed.height)
        * names.len() as u64
        * parsed.frames.len() as u64;
    if cells > MAX_IMPORT_CELLS {
        return Err(ase("image is larger than 16777216 cells"));
    }

    let len = usize::from(parsed.width) * usize::from(parsed.height);
    let mut frames = Vec::with_capacity(parsed.frames.len());
    for frame_index in 0..parsed.frames.len() {
        let mut buffers = vec![vec![0u8; len]; names.len()];
        for cel in &parsed.frames[frame_index].cels {
            let Some(mapped) = layer_of(&layer_map, cel.layer)? else {
                continue;
            };
            if cel.opacity != 255 {
                report.partial_cel_opacity = report.partial_cel_opacity.saturating_add(1);
            }
            if cel.z_index != 0 {
                report.nonzero_z_index = report.nonzero_z_index.saturating_add(1);
            }
            let (width, height, pixels) = match &cel.image {
                CelImage::Raw {
                    width,
                    height,
                    pixels,
                } => (*width, *height, pixels.clone()),
                CelImage::Link { frame } => {
                    resolve_link(&parsed.frames, usize::from(*frame), cel.layer, 0)?
                }
            };
            blit(
                &mut buffers[mapped],
                parsed.width,
                parsed.height,
                cel.x,
                cel.y,
                width,
                height,
                &pixels,
                transparent,
                palette_len,
                &mut report.clipped_cells,
            )?;
        }
        let (delay, defaulted) =
            frame_delay(parsed.frames[frame_index].duration, parsed.header_speed);
        if defaulted {
            report.duration_defaulted = report.duration_defaulted.saturating_add(1);
        }
        frames.push(Frame::from_layers(buffers, delay));
    }

    report.source_transparent_index = transparent;
    report.transparent_swapped = swapped;
    report.ignored = parsed.ignored.into_iter().collect();

    let document = Document::from_stored(
        parsed.width,
        parsed.height,
        palette,
        false,
        IndexPolicy::Reject,
        frames,
        0,
        names,
        0,
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    Ok((document, report))
}

fn map_layers(
    parsed: &Parsed,
) -> Result<(Vec<String>, Vec<Option<usize>>, AsepriteReport), KernelError> {
    let mut report = empty_report(parsed.transparent_index);
    if parsed.layers.is_empty() {
        for frame in &parsed.frames {
            for cel in &frame.cels {
                if cel.layer != 0 {
                    return Err(ase("cel layer is outside the layer list"));
                }
            }
        }
        return Ok((vec![DEFAULT_LAYER_NAME.to_string()], vec![Some(0)], report));
    }

    let mut names = Vec::new();
    let mut layer_map = Vec::with_capacity(parsed.layers.len());
    for layer in &parsed.layers {
        if layer.kind == 1 {
            report.skipped_groups = report.skipped_groups.saturating_add(1);
            layer_map.push(None);
            continue;
        }
        if layer.flags & LAYER_REFERENCE != 0 {
            report.skipped_reference = report.skipped_reference.saturating_add(1);
            layer_map.push(None);
            continue;
        }
        if layer.flags & LAYER_VISIBLE == 0 {
            report.hidden_layers = report.hidden_layers.saturating_add(1);
        }
        if layer.flags & LAYER_BACKGROUND != 0 {
            report.background_layers = report.background_layers.saturating_add(1);
        }
        if layer.blend != 0 {
            report.non_normal_blend = report.non_normal_blend.saturating_add(1);
        }
        if parsed.opacity_valid && layer.opacity != 255 {
            report.partial_opacity = report.partial_opacity.saturating_add(1);
        }
        let name = sanitize_layer_name(&layer.name, names.len());
        layer_map.push(Some(names.len()));
        names.push(name);
    }
    if names.is_empty() {
        return Err(KernelError::AsepriteLayers);
    }
    Ok((names, layer_map, report))
}

fn empty_report(source_transparent_index: u8) -> AsepriteReport {
    AsepriteReport {
        source_transparent_index,
        transparent_swapped: false,
        hidden_layers: 0,
        skipped_groups: 0,
        skipped_reference: 0,
        background_layers: 0,
        non_normal_blend: 0,
        partial_opacity: 0,
        partial_cel_opacity: 0,
        nonzero_z_index: 0,
        duration_defaulted: 0,
        clipped_cells: 0,
        ignored: Vec::new(),
    }
}

fn layer_of(layer_map: &[Option<usize>], layer: u16) -> Result<Option<usize>, KernelError> {
    layer_map
        .get(usize::from(layer))
        .copied()
        .ok_or_else(|| ase("cel layer is outside the layer list"))
}

fn sanitize_layer_name(raw: &str, index: usize) -> String {
    let trimmed: String = raw
        .trim()
        .chars()
        .filter(|ch| !ch.is_control())
        .take(MAX_LAYER_NAME_CHARS)
        .collect();
    let trimmed = trimmed.trim();
    if trimmed.is_empty() {
        format!("Layer {}", index + 1)
    } else {
        trimmed.to_string()
    }
}

fn frame_delay(duration: u16, header_speed: u16) -> (u32, bool) {
    let ms = if duration > 0 { duration } else { header_speed };
    if ms == 0 {
        (DEFAULT_FRAME_DELAY_MS, true)
    } else {
        (u32::from(ms), false)
    }
}

fn resolve_link(
    frames: &[ParsedFrame],
    frame: usize,
    layer: u16,
    depth: usize,
) -> Result<(u16, u16, Vec<u8>), KernelError> {
    if depth > frames.len() {
        return Err(ase("linked cel cycle"));
    }
    let Some(frame) = frames.get(frame) else {
        return Err(ase("linked cel frame is outside the file"));
    };
    let Some(cel) = frame.cels.iter().rev().find(|cel| cel.layer == layer) else {
        return Err(ase("linked cel has no image"));
    };
    match &cel.image {
        CelImage::Raw {
            width,
            height,
            pixels,
        } => Ok((*width, *height, pixels.clone())),
        CelImage::Link { frame } => resolve_link(frames, usize::from(*frame), layer, depth + 1),
    }
}

fn blit(
    buffer: &mut [u8],
    canvas_w: u16,
    canvas_h: u16,
    cel_x: i16,
    cel_y: i16,
    cel_w: u16,
    cel_h: u16,
    pixels: &[u8],
    transparent: u8,
    palette_len: usize,
    clipped: &mut u32,
) -> Result<(), KernelError> {
    let stride = usize::from(canvas_w);
    for y in 0..cel_h {
        for x in 0..cel_w {
            let cx = i32::from(cel_x) + i32::from(x);
            let cy = i32::from(cel_y) + i32::from(y);
            let source = pixels[usize::from(y) * usize::from(cel_w) + usize::from(x)];
            if cx < 0 || cy < 0 || cx >= i32::from(canvas_w) || cy >= i32::from(canvas_h) {
                *clipped = clipped.saturating_add(1);
                continue;
            }
            if usize::from(source) >= palette_len {
                return Err(KernelError::AsepritePixel {
                    index: source,
                    palette_len,
                });
            }
            let mapped = remap_index(source, transparent);
            let offset = cy as usize * stride + cx as usize;
            buffer[offset] = mapped;
        }
    }
    Ok(())
}

fn remap_index(index: u8, transparent: u8) -> u8 {
    if transparent == 0 {
        index
    } else if index == transparent {
        0
    } else if index == 0 {
        transparent
    } else {
        index
    }
}
