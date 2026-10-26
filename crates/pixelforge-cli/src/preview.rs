//! ASCII view of one frame.
//!
//! Cells stay palette indices. This module only picks a glyph so an agent can
//! see the shape in the terminal. Index 0 is `.`. Indices 1–9 are digits,
//! 10–35 are `A`–`Z`, 36–61 are `a`–`z`, and 62 and above share `*`.
//! Rendering a frame does not change `activeFrame`.

use pixelforge_kernel::{Document, Mask, Rgba};
use serde_json::{json, Value};

pub struct AsciiFrame {
    pub width: u16,
    pub height: u16,
    /// Frame whose pixels are in `rows`.
    pub frame: usize,
    /// Document active frame. Unchanged by a preview.
    pub active_frame: usize,
    /// Document active layer. Unchanged by a preview.
    pub active_layer: usize,
    /// `None` is the composite. `Some` is that layer alone.
    pub layer: Option<usize>,
    /// How many layers the document has. Used only to label the preview.
    pub layer_count: usize,
    /// True when cells outside a mask are `#`.
    pub scoped: bool,
    pub rows: Vec<String>,
    pub legend: Vec<LegendEntry>,
}

pub struct LegendEntry {
    pub glyph: char,
    pub index: u8,
    pub color: Rgba,
}

pub fn glyph(index: u8) -> char {
    pixelforge_kernel::cell_glyph(index)
}

pub fn render(document: &Document) -> AsciiFrame {
    render_view(document, document.active_frame(), None).expect("active frame is in range")
}

pub fn render_view(
    document: &Document,
    frame: usize,
    layer: Option<usize>,
) -> Result<AsciiFrame, pixelforge_kernel::KernelError> {
    render_view_scoped(document, frame, layer, None)
}

pub fn render_view_scoped(
    document: &Document,
    frame: usize,
    layer: Option<usize>,
    mask: Option<&Mask>,
) -> Result<AsciiFrame, pixelforge_kernel::KernelError> {
    let width = document.width();
    let height = document.height();
    let pixels = document.view_pixels(frame, layer)?;
    let row_len = usize::from(width);
    let mut seen = [false; 256];
    let mut rows = Vec::with_capacity(usize::from(height));
    for y in 0..usize::from(height) {
        let mut row = String::with_capacity(row_len);
        for (x, index) in pixels[y * row_len..(y + 1) * row_len].iter().enumerate() {
            if mask.is_some_and(|mask| !mask.contains(x as u16, y as u16)) {
                row.push('#');
                continue;
            }
            seen[usize::from(*index)] = true;
            row.push(glyph(*index));
        }
        rows.push(row);
    }
    let palette = document.palette_colors();
    let mut legend = Vec::new();
    for (index, used) in seen.into_iter().enumerate() {
        if !used {
            continue;
        }
        let index = u8::try_from(index).expect("palette index fits in u8");
        let color = palette
            .get(usize::from(index))
            .copied()
            .unwrap_or(Rgba::TRANSPARENT);
        legend.push(LegendEntry {
            glyph: glyph(index),
            index,
            color,
        });
    }
    Ok(AsciiFrame {
        width,
        height,
        frame,
        active_frame: document.active_frame(),
        active_layer: document.active_layer(),
        layer,
        layer_count: document.layer_count(),
        scoped: mask.is_some(),
        rows,
        legend,
    })
}

pub fn result_json(frame: &AsciiFrame) -> Value {
    let legend = frame
        .legend
        .iter()
        .map(|entry| {
            json!({
                "glyph": entry.glyph.to_string(),
                "index": entry.index,
                "r": entry.color.r,
                "g": entry.color.g,
                "b": entry.color.b,
                "a": entry.color.a,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "width": frame.width,
        "height": frame.height,
        "frame": frame.frame,
        "activeFrame": frame.active_frame,
        "activeLayer": frame.active_layer,
        "layer": frame.layer,
        "composite": frame.layer.is_none(),
        "rows": frame.rows,
        "legend": legend,
    })
}

/// Prints the grid to stderr. Stdout stays JSON.
pub fn eprint_frame(frame: &AsciiFrame) {
    let stack = match frame.layer {
        Some(layer) => format!(" layer {layer}"),
        None if frame.layer_count > 1 => " composite".to_string(),
        None => String::new(),
    };
    if frame.frame == frame.active_frame {
        eprintln!(
            "pixelforge: preview {}x{} frame {}{stack}",
            frame.width, frame.height, frame.frame
        );
    } else {
        eprintln!(
            "pixelforge: preview {}x{} frame {} active {}{stack}",
            frame.width, frame.height, frame.frame, frame.active_frame
        );
    }
    for row in &frame.rows {
        eprintln!("{row}");
    }
    eprintln!("pixelforge: legend {}", legend_line(frame));
    if frame.scoped {
        eprintln!("pixelforge: # = protect, outside the mask");
    }
}

fn legend_line(frame: &AsciiFrame) -> String {
    frame
        .legend
        .iter()
        .map(legend_token)
        .collect::<Vec<_>>()
        .join(" ")
}

fn legend_token(entry: &LegendEntry) -> String {
    if entry.index == 0 {
        return ". = 0".to_string();
    }
    let hex = format!(
        "#{:02x}{:02x}{:02x}",
        entry.color.r, entry.color.g, entry.color.b
    );
    if entry.glyph == '*' {
        format!("*={} {hex}", entry.index)
    } else {
        format!("{}={hex}", entry.glyph)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyphs_cover_the_studio_palette_and_stay_one_column() {
        assert_eq!(glyph(0), '.');
        assert_eq!(glyph(1), '1');
        assert_eq!(glyph(9), '9');
        assert_eq!(glyph(10), 'A');
        assert_eq!(glyph(35), 'Z');
        assert_eq!(glyph(36), 'a');
        assert_eq!(glyph(61), 'z');
        assert_eq!(glyph(62), '*');
        assert_eq!(glyph(255), '*');
    }

    #[test]
    fn render_marks_a_filled_rect_and_lists_its_color() {
        let mut document = Document::new(4, 2).unwrap();
        document.fill_rect(1, 0, 2, 1, 3).unwrap();
        let frame = render(&document);
        assert_eq!(frame.rows, vec![".33.".to_string(), "....".to_string()]);
        assert_eq!(frame.frame, 0);
        assert_eq!(frame.active_frame, 0);
        document.add_frame();
        document.set_active_frame(1).unwrap();
        document.paint(0, 1, 1).unwrap();
        document.set_active_frame(0).unwrap();
        let other = render_view(&document, 1, None).unwrap();
        assert_eq!(other.frame, 1);
        assert_eq!(other.active_frame, 0);
        assert_eq!(other.rows, vec!["....".to_string(), "1...".to_string()]);
        assert_eq!(document.active_frame(), 0);
        assert_eq!(document.get(0, 1).unwrap(), 0);
        let three = frame
            .legend
            .iter()
            .find(|entry| entry.index == 3)
            .expect("index 3 is used");
        assert_eq!(three.glyph, '3');
        assert_eq!(three.color.r, 232);
        assert!(legend_line(&frame).contains("3=#e85d4c"));
    }
}
