//! Looping APNG preview.
//!
//! Each frame is a full-canvas RGBA PNG, the same bytes
//! [`crate::export::encode_rgba_png`] writes for one frame. Those payloads are
//! wrapped as one APNG: `acTL`, then `fcTL` + `IDAT` for the first frame, then
//! `fcTL` + `fdAT` for the rest. A viewer that ignores animation chunks still
//! shows the first frame.
//!
//! GIF is not written. The composite is straight RGBA: palette entries carry
//! alpha, and layer opacity 1..=254 blends colors that are not palette
//! indices. GIF has one transparent index and no partial alpha, so those
//! pixels would have to be matted or dithered. GIF also stores delay in
//! centiseconds, and a 83 ms frame (12 fps) does not fit. APNG stores
//! `delayMs / 1000` exactly and keeps the RGBA bytes.
//!
//! This file is a preview. It is not a sprite sheet.

use crate::tag::LoopMode;

/// PNG signature.
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Clear the canvas after this frame, before the next one is drawn.
const DISPOSE_BACKGROUND: u8 = 1;

/// Replace the output region, including pixels this frame leaves transparent.
const BLEND_SOURCE: u8 = 1;

/// One frame inside an APNG, in playback order.
///
/// `index` is the document frame. Ping-pong lists a middle frame twice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApngFrame {
    pub index: u32,
    pub delay_ms: u32,
}

/// Bytes and playback metadata for one APNG preview.
///
/// `plays` is the APNG `num_plays`: `0` repeats, `1` plays once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApngExport {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub scale: u32,
    pub loop_mode: LoopMode,
    pub plays: u32,
    pub frames: Vec<ApngFrame>,
    pub tag: Option<String>,
}

/// Which frames an APNG preview plays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApngSource {
    /// Inclusive range and loop mode stored on this tag.
    Tag(String),
    /// Inclusive range. A missing end is the whole timeline.
    Range {
        from: Option<usize>,
        to: Option<usize>,
        loop_mode: LoopMode,
    },
}

/// Document frames in playback order.
///
/// `from` and `to` are inclusive and `from <= to`. `once` uses the forward
/// order; the play count is chosen separately. Ping-pong walks back toward
/// `from` without holding either end for an extra frame, so two frames stay
/// two frames and one frame stays one.
pub(crate) fn playback_order(from: usize, to: usize, mode: LoopMode) -> Vec<usize> {
    debug_assert!(from <= to);
    let mut order: Vec<usize> = (from..=to).collect();
    match mode {
        LoopMode::Forward | LoopMode::Once => order,
        LoopMode::Reverse => {
            order.reverse();
            order
        }
        LoopMode::PingPong => {
            if to - from >= 2 {
                for index in ((from + 1)..to).rev() {
                    order.push(index);
                }
            }
            order
        }
    }
}

/// `0` loops. `once` plays a single time and then rests on the last frame.
pub(crate) fn plays_for(mode: LoopMode) -> u32 {
    match mode {
        LoopMode::Once => 1,
        LoopMode::Forward | LoopMode::Reverse | LoopMode::PingPong => 0,
    }
}

/// One encoded PNG plus the delay to store on its `fcTL`.
pub(crate) struct PngFrame {
    pub png: Vec<u8>,
    pub delay_ms: u32,
}

/// Wrap already-encoded RGBA PNGs as one APNG.
///
/// Every PNG must share an IHDR. `plays` is written as `num_plays`.
pub(crate) fn assemble(frames: &[PngFrame], plays: u32) -> Vec<u8> {
    assert!(!frames.is_empty(), "apng needs at least one frame");
    let (ihdr, first_idat) = split_png(&frames[0].png);
    let mut images = Vec::with_capacity(frames.len());
    images.push(first_idat);
    for frame in frames.iter().skip(1) {
        let (next_ihdr, idat) = split_png(&frame.png);
        assert_eq!(next_ihdr, ihdr, "apng frames share one IHDR");
        images.push(idat);
    }
    let width = u32::from_be_bytes(ihdr[0..4].try_into().expect("IHDR width"));
    let height = u32::from_be_bytes(ihdr[4..8].try_into().expect("IHDR height"));
    let num_frames = u32::try_from(frames.len()).expect("frame count fits in u32");

    let mut out = Vec::new();
    out.extend_from_slice(PNG_SIGNATURE);
    write_chunk(&mut out, b"IHDR", &ihdr);
    let mut control = [0u8; 8];
    control[0..4].copy_from_slice(&num_frames.to_be_bytes());
    control[4..8].copy_from_slice(&plays.to_be_bytes());
    write_chunk(&mut out, b"acTL", &control);

    let mut sequence = 0u32;
    for (index, frame) in frames.iter().enumerate() {
        let seq = next_sequence(&mut sequence);
        write_chunk(
            &mut out,
            b"fcTL",
            &frame_control(seq, width, height, frame.delay_ms),
        );
        if index == 0 {
            write_chunk(&mut out, b"IDAT", &images[index]);
        } else {
            let seq = next_sequence(&mut sequence);
            let mut data = Vec::with_capacity(4 + images[index].len());
            data.extend_from_slice(&seq.to_be_bytes());
            data.extend_from_slice(&images[index]);
            write_chunk(&mut out, b"fdAT", &data);
        }
    }
    write_chunk(&mut out, b"IEND", &[]);
    out
}

fn next_sequence(sequence: &mut u32) -> u32 {
    let current = *sequence;
    *sequence = sequence.checked_add(1).expect("apng sequence number fits");
    current
}

fn frame_control(sequence: u32, width: u32, height: u32, delay_ms: u32) -> [u8; 26] {
    let delay = u16::try_from(delay_ms).expect("frame delay fits in the APNG numerator");
    let mut data = [0u8; 26];
    data[0..4].copy_from_slice(&sequence.to_be_bytes());
    data[4..8].copy_from_slice(&width.to_be_bytes());
    data[8..12].copy_from_slice(&height.to_be_bytes());
    data[20..22].copy_from_slice(&delay.to_be_bytes());
    data[22..24].copy_from_slice(&1000u16.to_be_bytes());
    data[24] = DISPOSE_BACKGROUND;
    data[25] = BLEND_SOURCE;
    data
}

fn split_png(bytes: &[u8]) -> (Vec<u8>, Vec<u8>) {
    assert!(
        bytes.starts_with(PNG_SIGNATURE),
        "png encoder wrote a signature"
    );
    let mut offset = PNG_SIGNATURE.len();
    let mut ihdr = None;
    let mut idat = Vec::new();
    while offset + 8 <= bytes.len() {
        let len = u32::from_be_bytes(bytes[offset..offset + 4].try_into().expect("chunk length"))
            as usize;
        let kind = &bytes[offset + 4..offset + 8];
        let start = offset + 8;
        let end = start + len;
        assert!(end + 4 <= bytes.len(), "png chunk fits");
        let data = &bytes[start..end];
        if kind == b"IHDR" {
            ihdr = Some(data.to_vec());
        } else if kind == b"IDAT" {
            idat.extend_from_slice(data);
        } else if kind == b"IEND" {
            break;
        }
        offset = end + 4;
    }
    let ihdr = ihdr.expect("png encoder wrote IHDR");
    assert!(!idat.is_empty(), "png encoder wrote IDAT");
    (ihdr, idat)
}

fn write_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    let len = u32::try_from(data.len()).expect("chunk fits in u32");
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&png_crc(kind, data).to_be_bytes());
}

/// CRC-32 of a PNG chunk type and data (ISO 3309).
fn png_crc(kind: &[u8], data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in kind.iter().chain(data.iter()) {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::encode_rgba_png;
    use crate::{Document, KernelError, LoopMode, Palette, Rgba};
    use png::{BitDepth, ColorType, Decoder};
    use std::io::Cursor;

    #[test]
    fn iend_crc_matches_the_png_vector() {
        assert_eq!(png_crc(b"IEND", b""), 0xAE42_6082);
    }

    #[test]
    fn playback_order_follows_the_loop_mode() {
        assert_eq!(playback_order(1, 3, LoopMode::Forward), vec![1, 2, 3]);
        assert_eq!(playback_order(1, 3, LoopMode::Once), vec![1, 2, 3]);
        assert_eq!(playback_order(1, 3, LoopMode::Reverse), vec![3, 2, 1]);
        assert_eq!(
            playback_order(0, 3, LoopMode::PingPong),
            vec![0, 1, 2, 3, 2, 1]
        );
        assert_eq!(playback_order(0, 1, LoopMode::PingPong), vec![0, 1]);
        assert_eq!(playback_order(4, 4, LoopMode::PingPong), vec![4]);
        assert_eq!(plays_for(LoopMode::Forward), 0);
        assert_eq!(plays_for(LoopMode::Reverse), 0);
        assert_eq!(plays_for(LoopMode::PingPong), 0);
        assert_eq!(plays_for(LoopMode::Once), 1);
    }

    #[test]
    fn assemble_round_trips_rgba_delays_and_the_play_count() {
        let red = encode_rgba_png(1, 1, &[9, 8, 7, 128]);
        let green = encode_rgba_png(1, 1, &[1, 2, 3, 255]);
        let bytes = assemble(
            &[
                PngFrame {
                    png: red,
                    delay_ms: 83,
                },
                PngFrame {
                    png: green,
                    delay_ms: 120,
                },
            ],
            0,
        );
        let (plays, frames) = decode_animation(&bytes);
        assert_eq!(plays, 0);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].delay_ms, 83);
        assert_eq!(frames[1].delay_ms, 120);
        assert!(frames.iter().all(|frame| frame.delay_den == 1000));
        assert!(frames
            .iter()
            .all(|frame| frame.dispose == DISPOSE_BACKGROUND && frame.blend == BLEND_SOURCE));
        assert_eq!(frames[0].rgba, vec![9, 8, 7, 128]);
        assert_eq!(frames[1].rgba, vec![1, 2, 3, 255]);
        let (width, height, first) = decode_rgba(&bytes);
        assert_eq!((width, height), (1, 1));
        assert_eq!(first, frames[0].rgba);
    }

    #[test]
    fn export_apng_plays_a_tag_with_its_delays_and_loop() {
        let mut document = Document::new(2, 1).unwrap();
        document.paint(0, 0, 1).unwrap();
        document.add_frame();
        document.set_active_frame(1).unwrap();
        document.paint(0, 0, 2).unwrap();
        document.add_frame();
        document.set_active_frame(2).unwrap();
        document.paint(1, 0, 3).unwrap();
        document.set_frame_delay(0, 83).unwrap();
        document.set_frame_delay(1, 120).unwrap();
        document.set_frame_delay(2, 40).unwrap();
        document.add_tag("walk", 0, 2, LoopMode::PingPong).unwrap();
        document.add_tag("idle", 0, 0, LoopMode::Once).unwrap();
        let saved = document.save_project();
        let undo = document.can_undo();

        let ping = document
            .export_apng(1, ApngSource::Tag("walk".to_string()))
            .unwrap();
        assert_eq!(ping.tag.as_deref(), Some("walk"));
        assert_eq!(ping.loop_mode, LoopMode::PingPong);
        assert_eq!(ping.plays, 0);
        assert_eq!(ping.width, 2);
        assert_eq!(ping.height, 1);
        assert_eq!(
            ping.frames
                .iter()
                .map(|frame| (frame.index, frame.delay_ms))
                .collect::<Vec<_>>(),
            vec![(0, 83), (1, 120), (2, 40), (1, 120)]
        );
        let (_, frames) = decode_animation(&ping.bytes);
        for (played, frame) in ping.frames.iter().zip(frames.iter()) {
            assert_eq!(frame.delay_ms, u16::try_from(played.delay_ms).unwrap());
            assert_eq!(
                frame.rgba,
                document.frame_rgba(played.index as usize).unwrap()
            );
        }

        let once = document
            .export_apng(1, ApngSource::Tag("idle".to_string()))
            .unwrap();
        assert_eq!(once.plays, 1);
        assert_eq!(once.frames.len(), 1);
        assert_eq!(once.frames[0].delay_ms, 83);

        let reversed = document
            .export_apng(
                1,
                ApngSource::Range {
                    from: Some(0),
                    to: Some(2),
                    loop_mode: LoopMode::Reverse,
                },
            )
            .unwrap();
        assert!(reversed.tag.is_none());
        assert_eq!(
            reversed
                .frames
                .iter()
                .map(|frame| frame.index)
                .collect::<Vec<_>>(),
            vec![2, 1, 0]
        );
        assert_eq!(reversed.plays, 0);

        assert_eq!(document.save_project(), saved);
        assert_eq!(document.can_undo(), undo);
    }

    #[test]
    fn export_apng_uses_the_composite_and_keeps_partial_alpha() {
        let mut document = Document::new(2, 1).unwrap();
        document.paint(0, 0, 1).unwrap();
        document.add_layer(Some("gear")).unwrap();
        document.paint_on(1, 0, 0, 2).unwrap();
        document.set_layer_visible(1, false).unwrap();
        let hidden = document
            .export_apng(
                1,
                ApngSource::Range {
                    from: None,
                    to: None,
                    loop_mode: LoopMode::Forward,
                },
            )
            .unwrap();
        let (_, frames) = decode_animation(&hidden.bytes);
        assert_eq!(frames[0].rgba, document.frame_rgba(0).unwrap());
        let gear = document.palette_colors()[2];
        assert_ne!(
            &frames[0].rgba[..4],
            &[gear.r, gear.g, gear.b, gear.a],
            "a hidden layer stays out of the preview"
        );

        document.set_layer_visible(1, true).unwrap();
        document.set_layer_opacity(1, 128).unwrap();
        let blended = document
            .export_apng(
                2,
                ApngSource::Range {
                    from: None,
                    to: None,
                    loop_mode: LoopMode::Forward,
                },
            )
            .unwrap();
        assert_eq!((blended.width, blended.height), (4, 2));
        let (_, frames) = decode_animation(&blended.bytes);
        let unscaled = document.frame_rgba(0).unwrap();
        assert_eq!(&frames[0].rgba[..4], &unscaled[..4]);
        assert_eq!(&frames[0].rgba[4..8], &unscaled[..4]);
        let body = document.palette_colors()[1];
        assert_ne!(&frames[0].rgba[..4], &[body.r, body.g, body.b, body.a]);
        assert_ne!(&frames[0].rgba[..4], &[gear.r, gear.g, gear.b, gear.a]);

        let palette = Palette::new(vec![Rgba::TRANSPARENT, Rgba::new(9, 8, 7, 128)]).unwrap();
        let mut translucent = Document::with_palette(1, 1, palette).unwrap();
        translucent.paint(0, 0, 1).unwrap();
        let export = translucent
            .export_apng(
                1,
                ApngSource::Range {
                    from: None,
                    to: None,
                    loop_mode: LoopMode::Forward,
                },
            )
            .unwrap();
        let (_, frames) = decode_animation(&export.bytes);
        assert_eq!(frames[0].rgba, vec![9, 8, 7, 128]);
    }

    #[test]
    fn export_apng_rejects_a_bad_range_and_a_missing_tag() {
        let document = Document::new(1, 1).unwrap();
        let saved = document.save_project();
        assert_eq!(
            document
                .export_apng(1, ApngSource::Tag("missing".to_string()))
                .unwrap_err(),
            KernelError::TagMissing
        );
        assert_eq!(
            document
                .export_apng(1, ApngSource::Tag(" ".to_string()))
                .unwrap_err(),
            KernelError::TagName
        );
        assert_eq!(
            document
                .export_apng(
                    1,
                    ApngSource::Range {
                        from: Some(2),
                        to: Some(1),
                        loop_mode: LoopMode::Forward,
                    },
                )
                .unwrap_err(),
            KernelError::PreviewRange {
                from: 2,
                to: 1,
                len: 1
            }
        );
        assert_eq!(
            document
                .export_apng(
                    1,
                    ApngSource::Range {
                        from: None,
                        to: Some(4),
                        loop_mode: LoopMode::Forward,
                    },
                )
                .unwrap_err(),
            KernelError::PreviewRange {
                from: 0,
                to: 4,
                len: 1
            }
        );
        assert_eq!(
            document
                .export_apng(
                    0,
                    ApngSource::Range {
                        from: None,
                        to: None,
                        loop_mode: LoopMode::Forward,
                    }
                )
                .unwrap_err(),
            KernelError::ExportScale { scale: 0 }
        );
        assert_eq!(document.save_project(), saved);
        assert!(!document.can_undo());
    }

    #[test]
    fn export_apng_tool_returns_playback_metadata_and_refuses_a_mixed_source() {
        let mut document = Document::new(1, 1).unwrap();
        document.paint(0, 0, 1).unwrap();
        document.set_frame_delay(0, 83).unwrap();
        document.add_tag("idle", 0, 0, LoopMode::Forward).unwrap();
        let saved = document.save_project();

        let response =
            document.call_tool(r#"{"tool":"exportApng","params":{"tag":"idle","scale":1}}"#);
        let body: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(body["ok"], true);
        assert_eq!(body["apiVersion"], 1);
        assert!(body["result"]["apng"]
            .as_str()
            .unwrap()
            .starts_with("iVBORw0KGgo"));
        assert_eq!(body["result"]["format"], "apng");
        assert_eq!(body["result"]["loop"], "forward");
        assert_eq!(body["result"]["plays"], 0);
        assert_eq!(body["result"]["frameCount"], 1);
        assert_eq!(body["result"]["frames"][0]["index"], 0);
        assert_eq!(body["result"]["frames"][0]["delayMs"], 83);
        assert_eq!(body["result"]["tag"], "idle");
        assert!(body["result"].get("png").is_none());

        let mixed = document.call_tool(r#"{"tool":"exportApng","params":{"tag":"idle","from":0}}"#);
        let mixed: serde_json::Value = serde_json::from_str(&mixed).unwrap();
        assert_eq!(mixed["ok"], false);
        assert_eq!(mixed["error"]["code"], "InvalidParams");

        let unknown = document.call_tool(r#"{"tool":"exportApng","params":{"gif":true}}"#);
        let unknown: serde_json::Value = serde_json::from_str(&unknown).unwrap();
        assert_eq!(unknown["error"]["code"], "InvalidParams");

        let missing = document.call_tool(r#"{"tool":"exportApng","params":{"tag":"nope"}}"#);
        let missing: serde_json::Value = serde_json::from_str(&missing).unwrap();
        assert_eq!(missing["error"]["code"], "TagMissing");
        assert_eq!(document.save_project(), saved);
    }

    struct PlayedFrame {
        delay_ms: u16,
        delay_den: u16,
        dispose: u8,
        blend: u8,
        rgba: Vec<u8>,
    }

    fn decode_animation(bytes: &[u8]) -> (u32, Vec<PlayedFrame>) {
        let chunks = read_chunks(bytes);
        assert_eq!(&chunks[0].kind, b"IHDR");
        assert_eq!(&chunks[1].kind, b"acTL");
        let num_frames = u32::from_be_bytes(chunks[1].data[0..4].try_into().unwrap());
        let plays = u32::from_be_bytes(chunks[1].data[4..8].try_into().unwrap());
        let ihdr = chunks[0].data.clone();
        let mut frames = Vec::new();
        let mut index = 2;
        let mut expect_seq = 0u32;
        while index < chunks.len() && chunks[index].kind != *b"IEND" {
            assert_eq!(&chunks[index].kind, b"fcTL");
            let control = &chunks[index].data;
            assert_eq!(control.len(), 26);
            let seq = u32::from_be_bytes(control[0..4].try_into().unwrap());
            assert_eq!(seq, expect_seq);
            expect_seq += 1;
            assert_eq!(u32::from_be_bytes(control[12..16].try_into().unwrap()), 0);
            assert_eq!(u32::from_be_bytes(control[16..20].try_into().unwrap()), 0);
            let delay_ms = u16::from_be_bytes(control[20..22].try_into().unwrap());
            let delay_den = u16::from_be_bytes(control[22..24].try_into().unwrap());
            let dispose = control[24];
            let blend = control[25];
            index += 1;
            let idat = if frames.is_empty() {
                assert_eq!(&chunks[index].kind, b"IDAT");
                let data = chunks[index].data.clone();
                index += 1;
                data
            } else {
                assert_eq!(&chunks[index].kind, b"fdAT");
                let data = &chunks[index].data;
                let seq = u32::from_be_bytes(data[0..4].try_into().unwrap());
                assert_eq!(seq, expect_seq);
                expect_seq += 1;
                let idat = data[4..].to_vec();
                index += 1;
                idat
            };
            let (_, _, rgba) = decode_rgba(&wrap_png(&ihdr, &idat));
            frames.push(PlayedFrame {
                delay_ms,
                delay_den,
                dispose,
                blend,
                rgba,
            });
        }
        assert_eq!(chunks.last().unwrap().kind, *b"IEND");
        assert_eq!(frames.len() as u32, num_frames);
        (plays, frames)
    }

    fn wrap_png(ihdr: &[u8], idat: &[u8]) -> Vec<u8> {
        let mut png = Vec::new();
        png.extend_from_slice(PNG_SIGNATURE);
        write_chunk(&mut png, b"IHDR", ihdr);
        write_chunk(&mut png, b"IDAT", idat);
        write_chunk(&mut png, b"IEND", &[]);
        png
    }

    struct Chunk {
        kind: [u8; 4],
        data: Vec<u8>,
    }

    fn read_chunks(bytes: &[u8]) -> Vec<Chunk> {
        assert!(bytes.starts_with(PNG_SIGNATURE));
        let mut offset = PNG_SIGNATURE.len();
        let mut chunks = Vec::new();
        while offset + 12 <= bytes.len() {
            let len = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
            let kind: [u8; 4] = bytes[offset + 4..offset + 8].try_into().unwrap();
            let data = bytes[offset + 8..offset + 8 + len].to_vec();
            let crc = u32::from_be_bytes(
                bytes[offset + 8 + len..offset + 12 + len]
                    .try_into()
                    .unwrap(),
            );
            assert_eq!(crc, png_crc(&kind, &data));
            offset += 12 + len;
            let done = kind == *b"IEND";
            chunks.push(Chunk { kind, data });
            if done {
                break;
            }
        }
        assert_eq!(offset, bytes.len());
        chunks
    }

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
}
