//! Palette text the shell can import.
//!
//! Two formats are accepted. A file whose first non-empty line is
//! `GIMP Palette` is a GIMP palette (`.gpl`): `Name:` and `Columns:` headers
//! and `#` comments are skipped, and each color line is `R G B` with channels
//! in `0..=255`. Anything else is a hex list (`.hex` or `.txt`): one
//! `RRGGBB` or `#RRGGBB` per line. Blank lines, `;` comments, and `//`
//! comments are skipped. A trailing name after the hex token is allowed.
//!
//! Parsed colors are opaque. The document keeps index 0 transparent and
//! places these colors at index 1 and up, so a file that starts with black
//! does not replace the transparent slot.

use crate::error::KernelError;
use crate::Rgba;

/// How [`crate::Document::import_palette`] applies a parsed file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteImportMode {
    /// Paint entries become the file colors, in order.
    ///
    /// Indices that no longer exist are set back to transparent on every frame.
    /// That remap is part of the same undo step as the palette change.
    Replace,
    /// Appends file colors whose opaque RGB is not already in the palette.
    ///
    /// Duplicates inside the file are appended once. Existing pixels keep
    /// their indices.
    Merge,
}

/// Opaque colors from `text`, without the transparent slot.
pub(crate) fn parse_palette_text(text: &str) -> Result<Vec<Rgba>, KernelError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let colors = if is_gpl(text) {
        parse_gpl(text)?
    } else {
        parse_hex_list(text)?
    };
    if colors.is_empty() {
        return Err(KernelError::EmptyPalette);
    }
    if colors.len() > usize::from(u8::MAX) {
        return Err(KernelError::PaletteTooLarge {
            len: colors.len() + 1,
        });
    }
    Ok(colors)
}

fn is_gpl(text: &str) -> bool {
    text.lines().map(str::trim).find(|line| !line.is_empty()) == Some("GIMP Palette")
}

fn parse_gpl(text: &str) -> Result<Vec<Rgba>, KernelError> {
    let mut seen_header = false;
    let mut colors = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if !seen_header {
            if line != "GIMP Palette" {
                return Err(KernelError::PaletteFile);
            }
            seen_header = true;
            continue;
        }
        if line.starts_with('#') || line.starts_with("Name:") || line.starts_with("Columns:") {
            continue;
        }
        let mut parts = line.split_whitespace();
        let r = parse_channel(parts.next())?;
        let g = parse_channel(parts.next())?;
        let b = parse_channel(parts.next())?;
        colors.push(Rgba::rgb(r, g, b));
    }
    if !seen_header {
        return Err(KernelError::PaletteFile);
    }
    Ok(colors)
}

fn parse_channel(token: Option<&str>) -> Result<u8, KernelError> {
    let Some(token) = token else {
        return Err(KernelError::PaletteFile);
    };
    if token.is_empty() || !token.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(KernelError::PaletteFile);
    }
    let value: u16 = token.parse().map_err(|_| KernelError::PaletteFile)?;
    u8::try_from(value).map_err(|_| KernelError::PaletteFile)
}

fn parse_hex_list(text: &str) -> Result<Vec<Rgba>, KernelError> {
    let mut colors = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with("//") {
            continue;
        }
        let without_hash = line.strip_prefix('#').unwrap_or(line);
        let token = without_hash
            .split_whitespace()
            .next()
            .unwrap_or(without_hash);
        if token.len() != 6 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(KernelError::PaletteFile);
        }
        let value = u32::from_str_radix(token, 16).map_err(|_| KernelError::PaletteFile)?;
        colors.push(Rgba::rgb(
            u8::try_from((value >> 16) & 0xff).expect("masked to a byte"),
            u8::try_from((value >> 8) & 0xff).expect("masked to a byte"),
            u8::try_from(value & 0xff).expect("masked to a byte"),
        ));
    }
    Ok(colors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpl_skips_headers_and_keeps_named_colors() {
        let text =
            "\u{feff}GIMP Palette\nName: Demo\nColumns: 2\n#\n255 0 0 Red\n0  128\t255\tBlue\n";
        let colors = parse_palette_text(text).unwrap();
        assert_eq!(colors, vec![Rgba::rgb(255, 0, 0), Rgba::rgb(0, 128, 255)]);
    }

    #[test]
    fn hex_list_accepts_hashes_comments_and_names() {
        let text = "; lospec\n\n// note\nff00aa Ink\n#00FF00\n";
        let colors = parse_palette_text(text).unwrap();
        assert_eq!(colors, vec![Rgba::rgb(255, 0, 170), Rgba::rgb(0, 255, 0)]);
    }

    #[test]
    fn rejects_empty_malformed_and_oversized_files() {
        assert_eq!(
            parse_palette_text("GIMP Palette\n#\n").unwrap_err(),
            KernelError::EmptyPalette
        );
        assert_eq!(
            parse_palette_text("   \n").unwrap_err(),
            KernelError::EmptyPalette
        );
        assert_eq!(
            parse_palette_text("not a palette").unwrap_err(),
            KernelError::PaletteFile
        );
        assert_eq!(
            parse_palette_text("GIMP Palette\n256 0 0\n").unwrap_err(),
            KernelError::PaletteFile
        );
        assert_eq!(
            parse_palette_text("#fff\n").unwrap_err(),
            KernelError::PaletteFile
        );
        let huge = "ff0000\n".repeat(256);
        assert_eq!(
            parse_palette_text(&huge).unwrap_err(),
            KernelError::PaletteTooLarge { len: 257 }
        );
    }
}
