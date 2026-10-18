use pixelforge_kernel::{
    Document, KernelError, Palette, ProjectFileError, Rgba, PROJECT_FORMAT, PROJECT_VERSION,
    TRANSPARENT_INDEX,
};

fn tiny() -> Document {
    let palette = Palette::new(vec![
        Rgba::TRANSPARENT,
        Rgba::rgb(9, 8, 7),
        Rgba::new(4, 5, 6, 128),
    ])
    .unwrap();
    let mut document = Document::with_palette(2, 1, palette).unwrap();
    document.paint(1, 0, 1).unwrap();
    document.add_frame();
    document.set_active_frame(1).unwrap();
    document.paint(0, 0, 2).unwrap();
    document
}

fn rejected(json: &str) -> ProjectFileError {
    match Document::load_project(json).unwrap_err() {
        KernelError::ProjectFile(error) => error,
        other => panic!("expected ProjectFile, got {other:?}"),
    }
}

#[test]
fn save_writes_version_1_json() {
    assert_eq!(PROJECT_FORMAT, "pixelforge-project");
    assert_eq!(PROJECT_VERSION, 1);
    assert_eq!(
        tiny().save_project(),
        r#"{
  "format": "pixelforge-project",
  "version": 1,
  "width": 2,
  "height": 1,
  "activeFrame": 1,
  "palette": [
    {
      "r": 0,
      "g": 0,
      "b": 0,
      "a": 0
    },
    {
      "r": 9,
      "g": 8,
      "b": 7,
      "a": 255
    },
    {
      "r": 4,
      "g": 5,
      "b": 6,
      "a": 128
    }
  ],
  "frames": [
    {
      "pixels": [
        0,
        1
      ]
    },
    {
      "pixels": [
        2,
        0
      ]
    }
  ]
}
"#
    );
}

#[test]
fn round_trip_keeps_size_palette_frames_and_active_frame() {
    let mut document = Document::new(32, 32).unwrap();
    document.paint(31, 31, 8).unwrap();
    document.add_frame();
    document.set_active_frame(1).unwrap();
    document.paint(0, 1, 3).unwrap();
    document.add_frame();
    let copy = document.duplicate_frame(0).unwrap();
    document.set_active_frame(copy).unwrap();
    assert!(document.can_undo());

    let loaded = Document::load_project(&document.save_project()).unwrap();
    assert_eq!(loaded.width(), 32);
    assert_eq!(loaded.height(), 32);
    assert_eq!(loaded.palette(), document.palette());
    assert_eq!(loaded.frame_count(), document.frame_count());
    assert_eq!(loaded.active_frame(), document.active_frame());
    for index in 0..document.frame_count() {
        assert_eq!(
            loaded.frame_pixels(index).unwrap(),
            document.frame_pixels(index).unwrap()
        );
    }
    assert!(!loaded.can_undo());
    assert!(!loaded.can_redo());
    assert_eq!(loaded.save_project(), document.save_project());

    let mut edited = Document::load_project(&loaded.save_project()).unwrap();
    edited.paint(1, 1, 2).unwrap();
    edited.undo().unwrap();
    assert_eq!(edited.get(1, 1).unwrap(), TRANSPARENT_INDEX);
    assert_eq!(edited.undo().unwrap_err(), KernelError::NothingToUndo);
}

#[test]
fn documented_example_loads() {
    let loaded = Document::load_project(
        r#"{
  "format": "pixelforge-project",
  "version": 1,
  "width": 2,
  "height": 1,
  "activeFrame": 1,
  "palette": [
    { "r": 0, "g": 0, "b": 0, "a": 0 },
    { "r": 232, "g": 93, "b": 76, "a": 255 }
  ],
  "frames": [
    { "pixels": [0, 1] },
    { "pixels": [1, 0] }
  ]
}
"#,
    )
    .unwrap();
    assert_eq!(loaded.width(), 2);
    assert_eq!(loaded.height(), 1);
    assert_eq!(loaded.active_frame(), 1);
    assert_eq!(loaded.frame_count(), 2);
    assert_eq!(loaded.frame_pixels(0).unwrap(), &[0, 1]);
    assert_eq!(loaded.frame_pixels(1).unwrap(), &[1, 0]);
    assert_eq!(loaded.palette().colors()[1], Rgba::rgb(232, 93, 76));
    assert!(!loaded.can_undo());
}

#[test]
fn edited_palette_round_trips_without_its_undo_stack() {
    let mut document = Document::new(2, 2).unwrap();
    document.set_palette_color(3, 1, 2, 3).unwrap();
    let added = document.add_palette_color(9, 8, 7).unwrap();
    document.paint(0, 0, added).unwrap();
    document.add_frame();
    document.set_active_frame(1).unwrap();
    assert!(document.can_undo());

    let loaded = Document::load_project(&document.save_project()).unwrap();
    assert_eq!(loaded.palette(), document.palette());
    assert_eq!(loaded.palette_colors()[3], Rgba::rgb(1, 2, 3));
    assert_eq!(
        loaded.palette_colors()[usize::from(added)],
        Rgba::rgb(9, 8, 7)
    );
    assert_eq!(loaded.active_frame(), 1);
    assert_eq!(loaded.frame_pixels(0).unwrap()[0], added);
    assert!(!loaded.can_undo());
    assert!(!loaded.can_redo());
    assert_eq!(loaded.save_project(), document.save_project());
}

#[test]
fn load_ignores_whitespace_order_and_a_leading_bom() {
    let json = "\u{feff}{\"frames\":[{\"pixels\":[0,1]},{\"pixels\":[2,0]}],\"height\":1,\
        \"palette\":[{\"a\":0,\"b\":0,\"g\":0,\"r\":0},{\"a\":255,\"b\":7,\"g\":8,\"r\":9},\
        {\"a\":128,\"b\":6,\"g\":5,\"r\":4}],\"width\":2,\"activeFrame\":1,\
        \"format\":\"pixelforge-project\",\"version\":1}";
    let loaded = Document::load_project(json).unwrap();
    assert_eq!(loaded.save_project(), tiny().save_project());
}

#[test]
fn save_uses_current_pixels_and_drops_history() {
    let mut document = Document::new(2, 2).unwrap();
    document.paint(0, 0, 1).unwrap();
    document.undo().unwrap();
    assert!(document.can_redo());

    let loaded = Document::load_project(&document.save_project()).unwrap();
    assert_eq!(loaded.get(0, 0).unwrap(), TRANSPARENT_INDEX);
    assert!(!loaded.can_undo());
    assert!(!loaded.can_redo());
    assert!(!loaded.save_project().contains("history"));

    let mut stroked = Document::new(2, 2).unwrap();
    stroked.begin_gesture().unwrap();
    stroked.paint(1, 1, 4).unwrap();
    let loaded = Document::load_project(&stroked.save_project()).unwrap();
    assert_eq!(loaded.get(1, 1).unwrap(), 4);
    assert!(!loaded.can_undo());
    assert!(!loaded.gesture_open());
}

#[test]
fn same_pixels_save_the_same_json_whatever_the_undo_stack() {
    let mut painted = Document::new(2, 2).unwrap();
    painted.paint(0, 0, 1).unwrap();
    let mut undone = Document::load_project(&painted.save_project()).unwrap();
    undone.paint(1, 1, 2).unwrap();
    undone.undo().unwrap();
    assert_eq!(painted.save_project(), undone.save_project());
}

#[test]
fn rejects_malformed_format_and_version() {
    assert_eq!(rejected(""), ProjectFileError::Malformed);
    assert_eq!(rejected("[]"), ProjectFileError::Malformed);
    assert_eq!(rejected("{"), ProjectFileError::Malformed);
    assert_eq!(rejected(r#"{"version":1}"#), ProjectFileError::Malformed);
    assert_eq!(
        rejected(&project(
            r#""format":"nope","version":9,"width":1,"height":1,"activeFrame":0,"palette":[],"frames":[]"#
        )),
        ProjectFileError::Format
    );
    assert_eq!(
        rejected(&project(
            r#""format":"pixelforge-project","version":2,"width":1,"height":1,"activeFrame":0,"palette":[{"r":0,"g":0,"b":0,"a":0}],"frames":[{"pixels":[0]}]"#
        )),
        ProjectFileError::Version { version: 2 }
    );
    assert_eq!(
        rejected(&project(
            r#""format":"pixelforge-project","version":1.5,"width":1,"height":1,"activeFrame":0,"palette":[{"r":0,"g":0,"b":0,"a":0}],"frames":[{"pixels":[0]}]"#
        )),
        ProjectFileError::Malformed
    );
    assert_eq!(
        rejected(&project(
            r#""format":"pixelforge-project","version":1,"width":1,"height":1,"activeFrame":0,"palette":[{"r":0,"g":0,"b":0,"a":0}],"frames":[{"pixels":[0]}],"history":[]"#
        )),
        ProjectFileError::Malformed
    );
    let without_active = r#"{"format":"pixelforge-project","version":1,"width":1,"height":1,"palette":[{"r":0,"g":0,"b":0,"a":0}],"frames":[{"pixels":[0]}]}"#;
    assert_eq!(rejected(without_active), ProjectFileError::Malformed);
}

#[test]
fn rejects_size_palette_frames_and_indices() {
    assert_eq!(
        rejected(&blank(0, 1)),
        ProjectFileError::Size {
            width: 0,
            height: 1
        }
    );
    assert_eq!(
        rejected(&blank(257, 16)),
        ProjectFileError::Size {
            width: 257,
            height: 16
        }
    );
    assert_eq!(
        rejected(&blank(70_000, 1)),
        ProjectFileError::Size {
            width: 70_000,
            height: 1
        }
    );
    assert_eq!(
        rejected(&project(
            r#""format":"pixelforge-project","version":1,"width":1,"height":1,"activeFrame":0,"palette":[],"frames":[{"pixels":[0]}]"#
        )),
        ProjectFileError::EmptyPalette
    );
    assert_eq!(
        rejected(&project(
            r#""format":"pixelforge-project","version":1,"width":1,"height":1,"activeFrame":0,"palette":[{"r":1,"g":2,"b":3,"a":255}],"frames":[{"pixels":[0]}]"#
        )),
        ProjectFileError::TransparentSlot
    );
    assert_eq!(
        rejected(&project(&format!(
            r#""format":"pixelforge-project","version":1,"width":1,"height":1,"activeFrame":0,"palette":[{}],"frames":[{{"pixels":[0]}}]"#,
            palette_json(257)
        ))),
        ProjectFileError::PaletteTooLarge { len: 257 }
    );
    assert_eq!(
        rejected(&project(
            r#""format":"pixelforge-project","version":1,"width":1,"height":1,"activeFrame":0,"palette":[{"r":0,"g":0,"b":0,"a":0}],"frames":[]"#
        )),
        ProjectFileError::EmptyFrames
    );
    assert_eq!(
        rejected(&project(
            r#""format":"pixelforge-project","version":1,"width":2,"height":1,"activeFrame":0,"palette":[{"r":0,"g":0,"b":0,"a":0}],"frames":[{"pixels":[0]}]"#
        )),
        ProjectFileError::FrameLength {
            index: 0,
            expected: 2,
            actual: 1
        }
    );
    assert_eq!(
        rejected(&project(
            r#""format":"pixelforge-project","version":1,"width":1,"height":1,"activeFrame":0,"palette":[{"r":0,"g":0,"b":0,"a":0},{"r":1,"g":1,"b":1,"a":255}],"frames":[{"pixels":[2]}]"#
        )),
        ProjectFileError::PixelIndex {
            frame: 0,
            index: 2,
            palette_len: 2
        }
    );
    assert_eq!(
        rejected(&project(
            r#""format":"pixelforge-project","version":1,"width":1,"height":1,"activeFrame":1,"palette":[{"r":0,"g":0,"b":0,"a":0}],"frames":[{"pixels":[0]}]"#
        )),
        ProjectFileError::ActiveFrame { index: 1, len: 1 }
    );
}

fn project(body: &str) -> String {
    format!("{{{body}}}")
}

fn blank(width: u64, height: u64) -> String {
    project(&format!(
        r#""format":"pixelforge-project","version":1,"width":{width},"height":{height},"activeFrame":0,"palette":[{{"r":0,"g":0,"b":0,"a":0}}],"frames":[{{"pixels":[0]}}]"#
    ))
}

fn palette_json(len: usize) -> String {
    let mut colors = Vec::with_capacity(len);
    colors.push(r#"{"r":0,"g":0,"b":0,"a":0}"#.to_string());
    for _ in 1..len {
        colors.push(r#"{"r":1,"g":2,"b":3,"a":255}"#.to_string());
    }
    colors.join(",")
}
