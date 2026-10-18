//! Undo stack of kernel operations.
//!
//! Each entry is one document edit: a paint or erase, a palette color change,
//! a frame-list change, an active-frame change, or a whole gesture of those.
//! The shell turns one drag into one gesture so a stroke or a color-picker
//! drag undoes in one step. Inverse data lives on the entry; undo does not
//! replay through [`crate::Document::paint`].

use crate::error::KernelError;
use crate::frame::Frame;
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
        if self.replaying {
            return;
        }
        if let Some(open) = &mut self.open {
            coalesce(open, change);
            return;
        }
        self.commit_new(Operation {
            changes: vec![change],
        });
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

/// Folds a later write into the earlier one when they name the same pixel or
/// the same palette entry.
///
/// Pixel folding stops at a frame-structure change, because that can make the
/// offset a different cell. Palette folding does not: frame edits do not
/// renumber colors, and a later recolor of the same index still means "this
/// entry changed from its original RGB".
fn coalesce(changes: &mut Vec<Change>, change: Change) {
    match change {
        Change::Pixel {
            frame,
            offset,
            before,
            after,
        } => coalesce_pixel(changes, frame, offset, before, after),
        Change::PaletteColor {
            index,
            before,
            after,
        } => coalesce_palette(changes, index, before, after),
        other => changes.push(other),
    }
}

fn coalesce_pixel(changes: &mut Vec<Change>, frame: usize, offset: usize, before: u8, after: u8) {
    let mut index = changes.len();
    while index > 0 {
        index -= 1;
        match &changes[index] {
            Change::Pixel {
                frame: existing_frame,
                offset: existing_offset,
                ..
            } if *existing_frame == frame && *existing_offset == offset => {
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
