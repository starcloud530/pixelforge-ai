use pixelforge_kernel::{Document, KernelError, Rgba, MAX_UNDO, TRANSPARENT_INDEX};

fn doc(size: u16) -> Document {
    Document::new(size, size).unwrap()
}

#[test]
fn paint_and_erase_undo_and_redo() {
    let mut document = doc(4);
    document.paint(1, 2, 4).unwrap();
    assert!(document.can_undo());
    assert!(!document.can_redo());

    document.undo().unwrap();
    assert_eq!(document.get(1, 2).unwrap(), TRANSPARENT_INDEX);
    assert!(!document.can_undo());
    assert!(document.can_redo());

    document.redo().unwrap();
    assert_eq!(document.get(1, 2).unwrap(), 4);

    document.erase(1, 2).unwrap();
    assert_eq!(document.get(1, 2).unwrap(), TRANSPARENT_INDEX);
    document.undo().unwrap();
    assert_eq!(document.get(1, 2).unwrap(), 4);
    document.redo().unwrap();
    assert_eq!(document.get(1, 2).unwrap(), TRANSPARENT_INDEX);
}

#[test]
fn empty_stacks_error() {
    let mut document = doc(4);
    assert_eq!(document.undo().unwrap_err(), KernelError::NothingToUndo);
    assert_eq!(document.redo().unwrap_err(), KernelError::NothingToRedo);
}

#[test]
fn failed_edits_are_not_recorded() {
    let mut document = doc(4);
    assert!(document.paint(0, 0, TRANSPARENT_INDEX).is_err());
    assert!(document.paint(0, 0, 99).is_err());
    assert!(document.paint(4, 0, 1).is_err());
    assert!(document.erase(0, 4).is_err());
    assert_eq!(
        document.delete_frame(0).unwrap_err(),
        KernelError::LastFrame
    );
    assert!(document.set_active_frame(2).is_err());
    assert!(document.duplicate_frame(1).is_err());
    assert_eq!(
        document.set_palette_color(0, 1, 2, 3).unwrap_err(),
        KernelError::TransparentSlot
    );
    assert_eq!(
        document.set_palette_color(40, 1, 2, 3).unwrap_err(),
        KernelError::PaletteIndex { index: 40 }
    );
    assert!(!document.can_undo());
    assert_eq!(document.get(0, 0).unwrap(), TRANSPARENT_INDEX);
    assert_eq!(document.palette().get(0), Some(Rgba::TRANSPARENT));
}

#[test]
fn no_op_paint_and_active_frame_are_not_recorded() {
    let mut document = doc(4);
    document.paint(0, 0, 1).unwrap();
    document.paint(0, 0, 1).unwrap();
    document.erase(1, 1).unwrap();
    document.set_active_frame(0).unwrap();
    let ink = document.palette().get(1).unwrap();
    document.set_palette_color(1, ink.r, ink.g, ink.b).unwrap();

    document.undo().unwrap();
    assert_eq!(document.get(0, 0).unwrap(), TRANSPARENT_INDEX);
    assert!(!document.can_undo());
}

#[test]
fn new_edit_drops_the_redo_branch() {
    let mut document = doc(4);
    document.paint(0, 0, 1).unwrap();
    document.undo().unwrap();
    assert!(document.can_redo());

    document.paint(1, 0, 2).unwrap();
    assert!(!document.can_redo());
    document.undo().unwrap();
    assert_eq!(document.get(0, 0).unwrap(), TRANSPARENT_INDEX);
    assert_eq!(document.get(1, 0).unwrap(), TRANSPARENT_INDEX);
    assert!(!document.can_undo());
}

#[test]
fn gesture_is_one_undo_step_and_coalesces_a_cell() {
    let mut document = doc(4);
    document.begin_gesture().unwrap();
    document.paint(0, 0, 1).unwrap();
    document.paint(0, 0, 2).unwrap();
    document.paint(1, 0, 3).unwrap();
    document.erase(2, 0).unwrap();
    document.end_gesture();

    assert!(document.can_undo());
    document.undo().unwrap();
    assert_eq!(document.get(0, 0).unwrap(), TRANSPARENT_INDEX);
    assert_eq!(document.get(1, 0).unwrap(), TRANSPARENT_INDEX);
    assert!(!document.can_undo());

    document.redo().unwrap();
    assert_eq!(document.get(0, 0).unwrap(), 2);
    assert_eq!(document.get(1, 0).unwrap(), 3);
}

#[test]
fn gesture_that_paints_then_erases_the_same_cell_records_nothing() {
    let mut document = doc(4);
    document.paint(0, 0, 1).unwrap();
    document.undo().unwrap();
    assert!(document.can_redo());

    document.begin_gesture().unwrap();
    document.paint(1, 0, 2).unwrap();
    document.erase(1, 0).unwrap();
    document.end_gesture();

    assert!(!document.can_undo());
    assert!(document.can_redo());
    assert!(!document.gesture_open());
    document.redo().unwrap();
    assert_eq!(document.get(0, 0).unwrap(), 1);
    assert_eq!(document.get(1, 0).unwrap(), TRANSPARENT_INDEX);
}

#[test]
fn undo_cancels_an_open_stroke_and_redo_restores_it() {
    let mut document = doc(4);
    document.begin_gesture().unwrap();
    document.paint(0, 0, 1).unwrap();
    document.paint(0, 1, 1).unwrap();

    assert_eq!(document.redo().unwrap_err(), KernelError::OpenGesture);
    assert!(document.gesture_open());
    assert_eq!(document.get(0, 1).unwrap(), 1);

    document.undo().unwrap();
    assert!(!document.gesture_open());
    assert_eq!(document.get(0, 0).unwrap(), TRANSPARENT_INDEX);
    assert_eq!(document.get(0, 1).unwrap(), TRANSPARENT_INDEX);

    document.redo().unwrap();
    assert_eq!(document.get(0, 0).unwrap(), 1);
    assert_eq!(document.get(0, 1).unwrap(), 1);
}

#[test]
fn nested_gesture_is_rejected_and_keeps_the_open_one() {
    let mut document = doc(4);
    document.begin_gesture().unwrap();
    assert_eq!(
        document.begin_gesture().unwrap_err(),
        KernelError::OpenGesture
    );
    document.paint(0, 0, 1).unwrap();
    document.end_gesture();
    document.undo().unwrap();
    assert_eq!(document.get(0, 0).unwrap(), TRANSPARENT_INDEX);
    document.end_gesture();
    assert!(!document.gesture_open());
}

#[test]
fn frame_add_duplicate_delete_and_active_are_undoable() {
    let mut document = doc(2);
    document.paint(0, 0, 3).unwrap();
    let added = document.add_frame();
    assert_eq!(added, 1);
    document.set_active_frame(1).unwrap();
    document.paint(1, 0, 4).unwrap();

    let copy = document.duplicate_frame(0).unwrap();
    assert_eq!(copy, 1);
    assert_eq!(document.frame_count(), 3);
    assert_eq!(document.active_frame(), 2);
    assert_eq!(document.frame_pixels(1).unwrap()[0], 3);

    document.set_active_frame(copy).unwrap();
    document.delete_frame(0).unwrap();
    assert_eq!(document.frame_count(), 2);
    assert_eq!(document.active_frame(), 0);
    assert_eq!(document.frame_pixels(0).unwrap()[0], 3);

    document.undo().unwrap();
    assert_eq!(document.frame_count(), 3);
    assert_eq!(document.active_frame(), 1);
    assert_eq!(document.frame_pixels(0).unwrap()[0], 3);
    assert_eq!(document.frame_pixels(1).unwrap()[0], 3);

    document.undo().unwrap();
    assert_eq!(document.active_frame(), 2);

    document.undo().unwrap();
    assert_eq!(document.frame_count(), 2);
    assert_eq!(document.active_frame(), 1);
    assert_eq!(document.frame_pixels(0).unwrap()[0], 3);
    assert_eq!(document.get(1, 0).unwrap(), 4);

    document.undo().unwrap();
    assert_eq!(document.get(1, 0).unwrap(), TRANSPARENT_INDEX);
    document.undo().unwrap();
    assert_eq!(document.active_frame(), 0);
    document.undo().unwrap();
    assert_eq!(document.frame_count(), 1);
    document.undo().unwrap();
    assert_eq!(document.get(0, 0).unwrap(), TRANSPARENT_INDEX);
    assert!(!document.can_undo());

    document.redo().unwrap();
    assert_eq!(document.get(0, 0).unwrap(), 3);
    assert_eq!(document.frame_count(), 1);
}

#[test]
fn deleting_the_active_middle_frame_restores_that_frame() {
    let mut document = doc(2);
    document.paint(0, 0, 1).unwrap();
    document.add_frame();
    document.add_frame();
    document.set_active_frame(1).unwrap();
    document.paint(1, 1, 5).unwrap();
    document.delete_frame(1).unwrap();
    assert_eq!(document.frame_count(), 2);
    assert_eq!(document.active_frame(), 1);
    assert_eq!(document.frame_pixels(1).unwrap()[0], TRANSPARENT_INDEX);

    document.undo().unwrap();
    assert_eq!(document.frame_count(), 3);
    assert_eq!(document.active_frame(), 1);
    assert_eq!(document.frame_pixels(1).unwrap()[1 + 2], 5);
    document.redo().unwrap();
    assert_eq!(document.frame_count(), 2);
    assert_eq!(document.frame_pixels(1).unwrap()[1 + 2], TRANSPARENT_INDEX);
}

#[test]
fn one_gesture_can_group_a_frame_edit_with_pixels() {
    let mut document = doc(2);
    document.paint(0, 0, 1).unwrap();
    document.begin_gesture().unwrap();
    let index = document.add_frame();
    document.set_active_frame(index).unwrap();
    document.paint(1, 1, 2).unwrap();
    document.end_gesture();

    document.undo().unwrap();
    assert_eq!(document.frame_count(), 1);
    assert_eq!(document.active_frame(), 0);
    assert_eq!(document.get(0, 0).unwrap(), 1);
    assert!(document.can_undo());
}

#[test]
fn undo_depth_drops_the_oldest_operation() {
    let mut document = doc(1);
    for step in 0..MAX_UNDO + 1 {
        let color = if step % 2 == 0 { 1 } else { 2 };
        document.paint(0, 0, color).unwrap();
    }
    assert_eq!(document.get(0, 0).unwrap(), 1);

    for _ in 0..MAX_UNDO {
        document.undo().unwrap();
    }
    assert!(!document.can_undo());
    assert_eq!(document.get(0, 0).unwrap(), 1);
    assert!(document.can_redo());
}

#[test]
fn paint_uses_the_edited_palette_color() {
    let mut document = doc(4);
    let original = document.palette().get(3).unwrap();
    document.set_palette_color(3, 8, 9, 10).unwrap();
    document.paint(1, 1, 3).unwrap();

    assert_eq!(document.get(1, 1).unwrap(), 3);
    assert_eq!(document.palette().get(3), Some(Rgba::rgb(8, 9, 10)));

    document.undo().unwrap();
    assert_eq!(document.get(1, 1).unwrap(), TRANSPARENT_INDEX);
    assert_eq!(document.palette().get(3), Some(Rgba::rgb(8, 9, 10)));

    document.undo().unwrap();
    assert_eq!(document.palette().get(3), Some(original));
    assert!(!document.can_undo());

    document.redo().unwrap();
    document.redo().unwrap();
    assert_eq!(document.get(1, 1).unwrap(), 3);
    assert_eq!(document.palette().get(3), Some(Rgba::rgb(8, 9, 10)));
}

#[test]
fn palette_drag_is_one_undo_step() {
    let mut document = doc(4);
    let original = document.palette().get(2).unwrap();
    document.begin_gesture().unwrap();
    document.set_palette_color(2, 1, 0, 0).unwrap();
    document.set_palette_color(2, 0, 1, 0).unwrap();
    document.paint(0, 0, 2).unwrap();
    document.set_palette_color(2, 0, 0, 1).unwrap();
    document.end_gesture();

    assert_eq!(document.get(0, 0).unwrap(), 2);
    assert_eq!(document.palette().get(2), Some(Rgba::rgb(0, 0, 1)));
    document.undo().unwrap();
    assert_eq!(document.get(0, 0).unwrap(), TRANSPARENT_INDEX);
    assert_eq!(document.palette().get(2), Some(original));
    assert!(!document.can_undo());

    document.redo().unwrap();
    assert_eq!(document.get(0, 0).unwrap(), 2);
    assert_eq!(document.palette().get(2), Some(Rgba::rgb(0, 0, 1)));
}

#[test]
fn palette_gesture_back_to_the_same_color_keeps_redo() {
    let mut document = doc(4);
    document.paint(0, 0, 1).unwrap();
    document.undo().unwrap();
    assert!(document.can_redo());

    let original = document.palette().get(5).unwrap();
    document.begin_gesture().unwrap();
    document.set_palette_color(5, 4, 5, 6).unwrap();
    document
        .set_palette_color(5, original.r, original.g, original.b)
        .unwrap();
    document.end_gesture();

    assert!(!document.can_undo());
    assert!(document.can_redo());
    assert_eq!(document.palette().get(5), Some(original));
    document.redo().unwrap();
    assert_eq!(document.get(0, 0).unwrap(), 1);
}

#[test]
fn added_color_can_be_painted_on_every_frame_and_undone() {
    let mut document = doc(4);
    let index = document.add_palette_color(12, 34, 56).unwrap();
    assert_eq!(index, 9);
    document.paint(0, 0, index).unwrap();
    document.add_frame();
    document.set_active_frame(1).unwrap();
    document.paint(1, 1, index).unwrap();
    document.set_palette_color(index, 7, 8, 9).unwrap();

    assert_eq!(document.frame_pixels(0).unwrap()[0], index);
    assert_eq!(document.get(1, 1).unwrap(), index);
    assert_eq!(document.palette().get(index), Some(Rgba::rgb(7, 8, 9)));

    document.undo().unwrap();
    assert_eq!(document.palette().get(index), Some(Rgba::rgb(12, 34, 56)));
    assert_eq!(document.frame_pixels(0).unwrap()[0], index);

    document.undo().unwrap();
    assert_eq!(document.get(1, 1).unwrap(), TRANSPARENT_INDEX);
    document.undo().unwrap();
    assert_eq!(document.active_frame(), 0);
    document.undo().unwrap();
    assert_eq!(document.frame_count(), 1);
    document.undo().unwrap();
    assert_eq!(document.get(0, 0).unwrap(), TRANSPARENT_INDEX);
    document.undo().unwrap();
    assert_eq!(document.palette().len(), 9);
    assert_eq!(
        document.paint(0, 0, index).unwrap_err(),
        KernelError::PaletteIndex { index }
    );
    assert!(!document.can_undo());

    document.redo().unwrap();
    assert_eq!(document.palette().get(index), Some(Rgba::rgb(12, 34, 56)));
    document.paint(0, 0, index).unwrap();
}

#[test]
fn clear_history_keeps_pixels_and_drops_an_open_gesture() {
    let mut document = doc(4);
    document.begin_gesture().unwrap();
    document.paint(2, 2, 6).unwrap();
    document.clear_history();
    assert!(!document.gesture_open());
    assert!(!document.can_undo());
    assert!(!document.can_redo());
    assert_eq!(document.get(2, 2).unwrap(), 6);
}
