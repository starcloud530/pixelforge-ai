//! Undo stack of kernel operations.
//!
//! Each entry is one document edit: a paint, an erase, a rectangle fill, a
//! rectangle erase, a rectangle copy or move (including across frames), a
//! horizontal index flip, a canvas resize, a palette color change, a palette
//! import, a palette lock, a frame-list change, an active-frame change, a
//! layer-stack change, an active-layer change, a rectangular selection, a
//! per-frame delay change, a named part list, a part-parameter curve list,
//! a constraint list, or a whole gesture of those.
//! The shell turns one drag into one gesture so a stroke or a color-picker
//! drag undoes in one step. Inverse data lives on the entry; undo does not
//! replay through [`crate::Document::paint`].

use crate::constraint::Constraint;
use crate::curve::Curve;
use crate::error::KernelError;
use crate::frame::Frame;
use crate::palette::IndexPolicy;
use crate::part::Part;
use crate::selection::Mask;
use crate::tag::Tag;
use crate::LayerRole;
use crate::Rgba;
use crate::MAX_UNDO;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct History {
    undo: Vec<Operation>,
    redo: Vec<Operation>,
    /// `Some` while a gesture is open. Empty means the gesture has not edited yet.
    open: Option<Vec<Change>>,
    /// Set while applying an undo or redo so those writes are not recorded again.
    replaying: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Operation {
    changes: Vec<Change>,
}

impl Operation {
    pub(crate) fn changes(&self) -> &[Change] {
        &self.changes
    }
}

/// One mutation inside an [`Operation`], stored so it can be inverted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Change {
    Pixel {
        frame: usize,
        layer: usize,
        offset: usize,
        before: u8,
        after: u8,
    },
    /// A blank frame was appended at `index` (always the new tail).
    AddFrame {
        index: usize,
    },
    /// A copy was inserted at `index`.
    InsertFrame {
        index: usize,
        frame: Frame,
        active_before: usize,
        active_after: usize,
    },
    /// The frame at `index` was removed.
    RemoveFrame {
        index: usize,
        frame: Frame,
        active_before: usize,
        active_after: usize,
    },
    ActiveFrame {
        before: usize,
        after: usize,
    },
    /// How long frame `index` stays on screen during playback, in milliseconds.
    FrameDelay {
        index: usize,
        before: u32,
        after: u32,
    },
    /// RGB of a paint entry was replaced. Index 0 is never stored here.
    PaletteColor {
        index: u8,
        before: Rgba,
        after: Rgba,
    },
    /// An opaque color was appended. `index` is that new tail entry.
    AddPaletteColor {
        index: u8,
        color: Rgba,
    },
    /// The whole palette was replaced, including the transparent slot.
    ///
    /// Pixel remaps that drop orphaned indices are separate [`Change::Pixel`]
    /// entries in the same operation, recorded before this one. Undo restores
    /// the palette first, then those indices.
    PaletteReplace {
        before: Vec<Rgba>,
        after: Vec<Rgba>,
    },
    /// The named frame-range list was replaced.
    ///
    /// Stored as a snapshot so one undo restores tags together with the frame
    /// insert or delete that moved them. This is not a linked cel.
    Tags {
        before: Vec<Tag>,
        after: Vec<Tag>,
    },
    /// The named part list was replaced.
    ///
    /// Stored as a snapshot so one undo restores parts together with the layer
    /// insert, delete, reorder, flatten, or canvas resize that moved them.
    /// Anchors stay in each part's local pixel grid. This is not a transform.
    Parts {
        before: Vec<Part>,
        after: Vec<Part>,
    },
    /// The part-parameter curve list was replaced.
    ///
    /// Stored as a snapshot so one undo restores curves together with the
    /// frame insert or delete that moved their keys, or the part rename,
    /// removal, or resize that renamed or dropped them. Sampling does not
    /// write pixels, so this entry has no pixel payload.
    Curves {
        before: Vec<Curve>,
        after: Vec<Curve>,
    },
    /// The constraint list was replaced.
    ///
    /// Stored as a snapshot so one undo restores constraints together with the
    /// part rename, removal, or canvas resize that renamed or dropped them.
    /// Solving is a separate edit: it moves parts and ink, and this entry is
    /// only the definition list.
    Constraints {
        before: Vec<Constraint>,
        after: Vec<Constraint>,
    },
    /// Whether palette edits and imports are refused.
    PaletteLocked {
        before: bool,
        after: bool,
    },
    /// How out-of-range paint indices are treated. Not a palette color edit.
    IndexPolicy {
        before: IndexPolicy,
        after: IndexPolicy,
    },
    /// The canvas size changed. Each frame keeps its delay; only indices move.
    ///
    /// `before_pixels` and `after_pixels` are one buffer per layer per frame,
    /// in timeline order, at the moment of the resize. Undo restores the
    /// previous size before any later pixel edit is replayed.
    CanvasResize {
        before_width: u16,
        before_height: u16,
        after_width: u16,
        after_height: u16,
        before_pixels: Vec<Vec<Vec<u8>>>,
        after_pixels: Vec<Vec<Vec<u8>>>,
    },
    /// A blank layer was inserted at `index` on every frame.
    AddLayer {
        index: usize,
        name: String,
        role: LayerRole,
        active_before: usize,
        active_after: usize,
    },
    /// The layer at `index` was removed from every frame.
    ///
    /// `pixels` is one buffer per frame, in timeline order.
    RemoveLayer {
        index: usize,
        name: String,
        role: LayerRole,
        pixels: Vec<Vec<u8>>,
        active_before: usize,
        active_after: usize,
    },
    /// One layer's bake role changed. Pixels stay.
    LayerRole {
        index: usize,
        before: LayerRole,
        after: LayerRole,
    },
    RenameLayer {
        index: usize,
        before: String,
        after: String,
    },
    ActiveLayer {
        before: usize,
        after: usize,
    },
    /// The selection mask was set, combined, or cleared. `None` means the
    /// whole canvas is writable. An empty mask protects every cell. This is
    /// not a pixel edit.
    Selection {
        before: Option<Mask>,
        after: Option<Mask>,
    },
    /// The layer at `from` was moved so it sits at `to`.
    ///
    /// `to` is the index after the move. Undo moves it back the same way.
    ReorderLayer {
        from: usize,
        to: usize,
        active_before: usize,
        active_after: usize,
    },
    /// Every frame was composited into the single default layer.
    Flatten {
        before_names: Vec<String>,
        before_roles: Vec<LayerRole>,
        before_pixels: Vec<Vec<Vec<u8>>>,
        before_active: usize,
        after_pixels: Vec<Vec<u8>>,
    },
}

impl History {
    pub(crate) fn new() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            open: None,
            replaying: false,
        }
    }

    pub(crate) fn can_undo(&self) -> bool {
        self.gesture_dirty() || !self.undo.is_empty()
    }

    pub(crate) fn can_redo(&self) -> bool {
        !self.gesture_dirty() && !self.redo.is_empty()
    }

    pub(crate) fn gesture_open(&self) -> bool {
        self.open.is_some()
    }

    fn gesture_dirty(&self) -> bool {
        self.open
            .as_ref()
            .is_some_and(|changes| !changes.is_empty())
    }

    pub(crate) fn set_replaying(&mut self, replaying: bool) {
        self.replaying = replaying;
    }

    pub(crate) fn begin_gesture(&mut self) -> Result<(), KernelError> {
        if self.open.is_some() {
            return Err(KernelError::OpenGesture);
        }
        self.open = Some(Vec::new());
        Ok(())
    }

    /// Seals the open gesture. A no-op when none is open, or when the gesture
    /// did not change the document. Redo is cleared only if an entry is pushed.
    pub(crate) fn end_gesture(&mut self) {
        let Some(changes) = self.open.take() else {
            return;
        };
        self.commit_new(Operation { changes });
    }

    pub(crate) fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.open = None;
        self.replaying = false;
    }

    pub(crate) fn record(&mut self, change: Change) {
        self.record_group(vec![change]);
    }

    /// Records `changes` as one undo step, or folds them into the open gesture.
    ///
    /// An empty group is ignored. Used when one edit rewrites the palette and
    /// the indices that no longer fit it.
    pub(crate) fn record_group(&mut self, changes: Vec<Change>) {
        if self.replaying || changes.is_empty() {
            return;
        }
        if let Some(open) = &mut self.open {
            for change in changes {
                coalesce(open, change);
            }
            return;
        }
        self.commit_new(Operation { changes });
    }

    /// Closes a dirty gesture (so one undo cancels the stroke), drops an empty
    /// one, then pops the latest operation.
    pub(crate) fn take_undo(&mut self) -> Result<Operation, KernelError> {
        if self.gesture_dirty() {
            self.end_gesture();
        } else {
            self.open = None;
        }
        self.undo.pop().ok_or(KernelError::NothingToUndo)
    }

    /// Refuses to run while a gesture already changed pixels, so redo cannot
    /// commit that gesture and discard the redo branch by accident.
    pub(crate) fn take_redo(&mut self) -> Result<Operation, KernelError> {
        if self.gesture_dirty() {
            return Err(KernelError::OpenGesture);
        }
        self.open = None;
        self.redo.pop().ok_or(KernelError::NothingToRedo)
    }

    pub(crate) fn push_redo(&mut self, operation: Operation) {
        self.redo.push(operation);
    }

    pub(crate) fn push_undo(&mut self, operation: Operation) {
        self.undo.push(operation);
    }

    fn commit_new(&mut self, operation: Operation) {
        if operation.changes.is_empty() {
            return;
        }
        self.redo.clear();
        self.undo.push(operation);
        if self.undo.len() > MAX_UNDO {
            let extra = self.undo.len() - MAX_UNDO;
            self.undo.drain(0..extra);
        }
    }
}

/// Folds a later write into the earlier one when they name the same pixel,
/// the same palette entry, the same frame delay, or the active frame.
///
/// Pixel folding stops at a frame-structure change, because that can make the
/// offset a different cell. Palette folding does not: frame edits do not
/// renumber colors, and a later recolor of the same index still means "this
/// entry changed from its original RGB". Delay folding stops at a structural
/// frame edit, because that can renumber the frame. Active-frame folding only
/// merges a run of selections, so one timeline drag is one undo step.
fn coalesce(changes: &mut Vec<Change>, change: Change) {
    match change {
        Change::Pixel {
            frame,
            layer,
            offset,
            before,
            after,
        } => coalesce_pixel(changes, frame, layer, offset, before, after),
        Change::ActiveLayer { before, after } => coalesce_active_layer(changes, before, after),
        Change::Selection { before, after } => coalesce_selection(changes, before, after),
        Change::PaletteColor {
            index,
            before,
            after,
        } => coalesce_palette(changes, index, before, after),
        Change::FrameDelay {
            index,
            before,
            after,
        } => coalesce_delay(changes, index, before, after),
        Change::ActiveFrame { before, after } => coalesce_active(changes, before, after),
        Change::Tags { before, after } => coalesce_tags(changes, before, after),
        Change::Parts { before, after } => coalesce_parts(changes, before, after),
        Change::Curves { before, after } => coalesce_curves(changes, before, after),
        Change::Constraints { before, after } => coalesce_constraints(changes, before, after),
        other => changes.push(other),
    }
}

fn coalesce_pixel(
    changes: &mut Vec<Change>,
    frame: usize,
    layer: usize,
    offset: usize,
    before: u8,
    after: u8,
) {
    let mut index = changes.len();
    while index > 0 {
        index -= 1;
        match &changes[index] {
            Change::Pixel {
                frame: existing_frame,
                layer: existing_layer,
                offset: existing_offset,
                ..
            } if *existing_frame == frame
                && *existing_layer == layer
                && *existing_offset == offset =>
            {
                let Change::Pixel {
                    before: original,
                    after: slot,
                    ..
                } = &mut changes[index]
                else {
                    unreachable!("matched a pixel change");
                };
                *slot = after;
                if *original == *slot {
                    changes.remove(index);
                }
                return;
            }
            Change::Pixel { .. } => continue,
            _ => break,
        }
    }

    changes.push(Change::Pixel {
        frame,
        layer,
        offset,
        before,
        after,
    });
}

fn coalesce_palette(changes: &mut Vec<Change>, index: u8, before: Rgba, after: Rgba) {
    let mut cursor = changes.len();
    while cursor > 0 {
        cursor -= 1;
        if let Change::PaletteColor {
            index: existing,
            before: original,
            after: slot,
        } = &mut changes[cursor]
        {
            if *existing != index {
                continue;
            }
            debug_assert_eq!(*slot, before, "palette history skipped a color");
            *slot = after;
            if *original == *slot {
                changes.remove(cursor);
            }
            return;
        }
    }

    changes.push(Change::PaletteColor {
        index,
        before,
        after,
    });
}

fn coalesce_delay(changes: &mut Vec<Change>, index: usize, before: u32, after: u32) {
    let mut cursor = changes.len();
    while cursor > 0 {
        cursor -= 1;
        match &mut changes[cursor] {
            Change::FrameDelay {
                index: existing,
                before: original,
                after: slot,
            } if *existing == index => {
                debug_assert_eq!(*slot, before, "delay history skipped a write");
                *slot = after;
                if *original == *slot {
                    changes.remove(cursor);
                }
                return;
            }
            Change::FrameDelay { .. } => continue,
            Change::AddFrame { .. } | Change::InsertFrame { .. } | Change::RemoveFrame { .. } => {
                break
            }
            _ => continue,
        }
    }

    changes.push(Change::FrameDelay {
        index,
        before,
        after,
    });
}

fn coalesce_active(changes: &mut Vec<Change>, before: usize, after: usize) {
    if let Some(Change::ActiveFrame {
        before: original,
        after: slot,
    }) = changes.last_mut()
    {
        debug_assert_eq!(*slot, before, "active-frame history skipped a selection");
        *slot = after;
        if *original == *slot {
            changes.pop();
        }
        return;
    }

    changes.push(Change::ActiveFrame { before, after });
}

fn coalesce_selection(changes: &mut Vec<Change>, before: Option<Mask>, after: Option<Mask>) {
    if let Some(Change::Selection {
        before: original,
        after: slot,
    }) = changes.last_mut()
    {
        debug_assert_eq!(slot, &before, "selection history skipped a write");
        *slot = after;
        if *original == *slot {
            changes.pop();
        }
        return;
    }

    changes.push(Change::Selection { before, after });
}

fn coalesce_parts(changes: &mut Vec<Change>, before: Vec<Part>, after: Vec<Part>) {
    if let Some(Change::Parts {
        before: original,
        after: slot,
    }) = changes.last_mut()
    {
        debug_assert_eq!(slot, &before, "part history skipped a write");
        *slot = after;
        if *original == *slot {
            changes.pop();
        }
        return;
    }

    changes.push(Change::Parts { before, after });
}

fn coalesce_curves(changes: &mut Vec<Change>, before: Vec<Curve>, after: Vec<Curve>) {
    if let Some(Change::Curves {
        before: original,
        after: slot,
    }) = changes.last_mut()
    {
        debug_assert_eq!(slot, &before, "curve history skipped a write");
        *slot = after;
        if *original == *slot {
            changes.pop();
        }
        return;
    }

    changes.push(Change::Curves { before, after });
}

fn coalesce_constraints(
    changes: &mut Vec<Change>,
    before: Vec<Constraint>,
    after: Vec<Constraint>,
) {
    if let Some(Change::Constraints {
        before: original,
        after: slot,
    }) = changes.last_mut()
    {
        debug_assert_eq!(slot, &before, "constraint history skipped a write");
        *slot = after;
        if *original == *slot {
            changes.pop();
        }
        return;
    }

    changes.push(Change::Constraints { before, after });
}

fn coalesce_tags(changes: &mut Vec<Change>, before: Vec<Tag>, after: Vec<Tag>) {
    if let Some(Change::Tags {
        before: original,
        after: slot,
    }) = changes.last_mut()
    {
        debug_assert_eq!(slot, &before, "tag history skipped a write");
        *slot = after;
        if *original == *slot {
            changes.pop();
        }
        return;
    }

    changes.push(Change::Tags { before, after });
}

fn coalesce_active_layer(changes: &mut Vec<Change>, before: usize, after: usize) {
    if let Some(Change::ActiveLayer {
        before: original,
        after: slot,
    }) = changes.last_mut()
    {
        debug_assert_eq!(*slot, before, "active-layer history skipped a selection");
        *slot = after;
        if *original == *slot {
            changes.pop();
        }
        return;
    }

    changes.push(Change::ActiveLayer { before, after });
}
