//! Named parts and the anchors on them.
//!
//! A part is not a layer and not a pixel buffer. Layers stay the shared paint
//! stack. A part is a name, a rest-pose origin on the canvas, an optional
//! layer index for the pixels that belong to it, and named anchors in that
//! part's local pixel grid. Local `(0, 0)` is the origin cell. Canvas position
//! of an anchor at rest is `origin + local`.
//!
//! This is the rest pose. [`crate::Document::translate_part`],
//! [`crate::Document::rotate_part`], and [`crate::Document::flip_part`] move
//! that pose together with the ink on the linked layer. A curve, a constraint,
//! and a bake are not stored here. New fields, if those issues add them,
//! should default so a version 1 file that only has parts still loads.
//! Rectangle flip and rotate still move indices only; they do not move parts.

use crate::MAX_CANVAS_EDGE;
use crate::MAX_PART_NAME_CHARS;

/// How [`crate::Document::set_part`] treats the optional layer attachment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerAttach {
    /// Leave the current attachment alone.
    Keep,
    /// Clear it. The part stays, with its anchors.
    Clear,
    /// Attach the part to this layer index.
    Index(usize),
}

/// Fields [`crate::Document::set_part`] may change. Unset fields stay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartUpdate {
    pub x: Option<u16>,
    pub y: Option<u16>,
    pub layer: LayerAttach,
    pub rename: Option<String>,
}

/// One named point in a part's local pixel grid.
///
/// `x` and `y` are offsets from the part origin, in cells. They are not
/// canvas coordinates. The range is `-MAX_CANVAS_EDGE..=MAX_CANVAS_EDGE`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    pub name: String,
    pub x: i16,
    pub y: i16,
}

/// One named part at its rest pose.
///
/// `x` and `y` are the canvas cell that is local `(0, 0)`. `layer` is the
/// paint layer whose pixels this part names, or `None` when the part is only
/// a rig point (a shoulder, an elbow) and does not own a layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    pub name: String,
    pub x: u16,
    pub y: u16,
    pub layer: Option<usize>,
    pub anchors: Vec<Anchor>,
}

impl Part {
    /// Canvas cell of a named anchor at the rest pose: origin plus the local offset.
    ///
    /// `None` when this part has no anchor with that name. The sum may lie
    /// outside the canvas. This is not a rotation.
    pub fn anchor_canvas(&self, name: &str) -> Option<(i32, i32)> {
        let anchor = self.anchors.iter().find(|anchor| anchor.name == name)?;
        let x = i32::from(self.x).checked_add(i32::from(anchor.x))?;
        let y = i32::from(self.y).checked_add(i32::from(anchor.y))?;
        Some((x, y))
    }
}

/// Trimmed part or anchor name, or an error when it is empty, too long, or has a control character.
pub(crate) fn normalize_part_name(name: &str) -> Result<String, ()> {
    let trimmed = name.trim();
    let chars = trimmed.chars().count();
    if trimmed.is_empty() || chars > MAX_PART_NAME_CHARS || trimmed.chars().any(char::is_control) {
        Err(())
    } else {
        Ok(trimmed.to_string())
    }
}

/// Local anchor offset, or `None` when either axis is outside `-MAX_CANVAS_EDGE..=MAX_CANVAS_EDGE`.
pub(crate) fn anchor_offset_i64(x: i64, y: i64) -> Option<(i16, i16)> {
    let limit = i64::from(MAX_CANVAS_EDGE);
    if !(-limit..=limit).contains(&x) || !(-limit..=limit).contains(&y) {
        return None;
    }
    Some((
        i16::try_from(x).expect("limited offset fits in i16"),
        i16::try_from(y).expect("limited offset fits in i16"),
    ))
}

/// Parts after a layer is inserted at `index`.
///
/// An attachment at or after `index` moves up by one. An unattached part stays.
pub(crate) fn parts_after_layer_insert(parts: &[Part], index: usize) -> Vec<Part> {
    parts
        .iter()
        .map(|part| {
            let mut next = part.clone();
            if let Some(layer) = next.layer {
                if layer >= index {
                    next.layer = Some(layer + 1);
                }
            }
            next
        })
        .collect()
}

/// Parts after the layer at `index` is removed.
///
/// A part that named that layer keeps its anchors and drops the attachment.
/// Attachments above `index` move down by one.
pub(crate) fn parts_after_layer_remove(parts: &[Part], index: usize) -> Vec<Part> {
    parts
        .iter()
        .map(|part| {
            let mut next = part.clone();
            if let Some(layer) = next.layer {
                if layer == index {
                    next.layer = None;
                } else if layer > index {
                    next.layer = Some(layer - 1);
                }
            }
            next
        })
        .collect()
}

/// Parts after the layer at `from` is moved so it ends at `to`.
pub(crate) fn parts_after_reorder(parts: &[Part], from: usize, to: usize) -> Vec<Part> {
    parts
        .iter()
        .map(|part| {
            let mut next = part.clone();
            if let Some(layer) = next.layer {
                next.layer = Some(remap_layer(layer, from, to));
            }
            next
        })
        .collect()
}

/// Parts after every frame is composited into one layer.
///
/// An attached part now names layer 0, the only layer left. An unattached
/// part stays unattached.
pub(crate) fn parts_after_flatten(parts: &[Part]) -> Vec<Part> {
    parts
        .iter()
        .map(|part| {
            let mut next = part.clone();
            if next.layer.is_some() {
                next.layer = Some(0);
            }
            next
        })
        .collect()
}

/// Parts after a canvas resize that places the old origin at `offset_x`, `offset_y`.
///
/// Origins move with the pixels. Anchors stay local, so a grip that was two
/// cells from the origin is still two cells from it. A part whose origin
/// would leave the new canvas is dropped.
pub(crate) fn parts_after_resize(
    parts: &[Part],
    offset_x: i32,
    offset_y: i32,
    width: u16,
    height: u16,
) -> Vec<Part> {
    parts
        .iter()
        .filter_map(|part| {
            let x = i32::from(part.x).checked_add(offset_x)?;
            let y = i32::from(part.y).checked_add(offset_y)?;
            let x = u16::try_from(x).ok()?;
            let y = u16::try_from(y).ok()?;
            if x >= width || y >= height {
                return None;
            }
            let mut next = part.clone();
            next.x = x;
            next.y = y;
            Some(next)
        })
        .collect()
}

fn remap_layer(layer: usize, from: usize, to: usize) -> usize {
    if layer == from {
        to
    } else if from < to && layer > from && layer <= to {
        layer - 1
    } else if to < from && layer >= to && layer < from {
        layer + 1
    } else {
        layer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gun() -> Part {
        Part {
            name: "gun".to_string(),
            x: 4,
            y: 5,
            layer: Some(1),
            anchors: vec![Anchor {
                name: "grip".to_string(),
                x: 2,
                y: -1,
            }],
        }
    }

    #[test]
    fn anchor_canvas_is_origin_plus_local() {
        assert_eq!(gun().anchor_canvas("grip"), Some((6, 4)));
        assert_eq!(gun().anchor_canvas("muzzle"), None);
    }

    #[test]
    fn layer_edits_shift_attachments_and_keep_anchors() {
        let inserted = parts_after_layer_insert(&[gun()], 1);
        assert_eq!(inserted[0].layer, Some(2));
        assert_eq!(inserted[0].anchors[0].name, "grip");

        let cleared = parts_after_layer_remove(&[gun()], 1);
        assert_eq!(cleared[0].layer, None);
        assert_eq!(cleared[0].anchors.len(), 1);

        let shifted = parts_after_layer_remove(&[gun()], 0);
        assert_eq!(shifted[0].layer, Some(0));

        let reordered = parts_after_reorder(&[gun()], 1, 0);
        assert_eq!(reordered[0].layer, Some(0));

        let flat = parts_after_flatten(&[gun()]);
        assert_eq!(flat[0].layer, Some(0));
        let point = Part {
            layer: None,
            ..gun()
        };
        assert_eq!(parts_after_flatten(&[point])[0].layer, None);
    }

    #[test]
    fn resize_moves_the_origin_and_drops_a_part_that_leaves() {
        let moved = parts_after_resize(&[gun()], 1, 2, 16, 16);
        assert_eq!(moved[0].x, 5);
        assert_eq!(moved[0].y, 7);
        assert_eq!(moved[0].anchors[0].x, 2);
        assert!(parts_after_resize(&[gun()], -5, 0, 16, 16).is_empty());
    }
}
