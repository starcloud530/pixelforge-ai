//! Named animation tags.
//!
//! A tag is a name, an inclusive frame range, and a loop mode. It is metadata
//! for agents and for sprite-sheet atlas export. It is not a linked cel:
//! frames do not share pixels, and duplicating a frame still copies indices.
//! [`crate::Document::tick`] still walks the whole timeline. The loop mode is
//! stored and exported; it does not start a second clock.
//!
//! Inserting a frame at `index` shifts a tag that starts at or after that
//! index, and grows a tag that already covers the insertion point but starts
//! earlier. Deleting a frame shrinks tags that covered it and drops a tag
//! that named only that frame. Appending a frame past every tag leaves the
//! tags alone.

use crate::MAX_TAG_NAME_CHARS;

/// How a consumer should play [`Tag::from`] through [`Tag::to`].
///
/// This is not the document playback clock. `forward`, `reverse`, and
/// `pingpong` repeat. `once` plays the range a single time, from `from` to
/// `to`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopMode {
    /// Play `from` through `to`, then repeat.
    Forward,
    /// Play `to` through `from`, then repeat.
    Reverse,
    /// Play `from` through `to` and back toward `from`, then repeat.
    ///
    /// The two ends are not held for an extra frame.
    PingPong,
    /// Play `from` through `to` once.
    Once,
}

impl LoopMode {
    /// Stable name written in project files, tool results, and atlas JSON.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Forward => "forward",
            Self::Reverse => "reverse",
            Self::PingPong => "pingpong",
            Self::Once => "once",
        }
    }

    /// Parses the stable name. Unknown text is `None`.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "forward" => Some(Self::Forward),
            "reverse" => Some(Self::Reverse),
            "pingpong" => Some(Self::PingPong),
            "once" => Some(Self::Once),
            _ => None,
        }
    }
}

impl Default for LoopMode {
    fn default() -> Self {
        Self::Forward
    }
}

/// One named inclusive frame range.
///
/// `from` and `to` are frame indices. Both ends are inside the animation.
/// Ranges may overlap. Names are unique.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub name: String,
    pub from: usize,
    pub to: usize,
    pub loop_mode: LoopMode,
}

/// Fields [`crate::Document::set_tag`] may change. `None` leaves that field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagUpdate {
    pub from: Option<usize>,
    pub to: Option<usize>,
    pub loop_mode: Option<LoopMode>,
    pub rename: Option<String>,
}

/// Trimmed tag name, or an error when it is empty, too long, or has a control character.
pub(crate) fn normalize_tag_name(name: &str) -> Result<String, ()> {
    let trimmed = name.trim();
    let chars = trimmed.chars().count();
    if trimmed.is_empty() || chars > MAX_TAG_NAME_CHARS || trimmed.chars().any(char::is_control) {
        Err(())
    } else {
        Ok(trimmed.to_string())
    }
}

/// Tags after a frame is inserted at `index` (the new frame's index).
///
/// A tag that starts at or after `index` moves right and does not cover the
/// new frame. A tag that starts before `index` and ends at or after it grows
/// by one, so the new frame is inside it. A tag that ends before `index` stays.
pub(crate) fn tags_after_insert(tags: &[Tag], index: usize) -> Vec<Tag> {
    tags.iter()
        .map(|tag| {
            let mut next = tag.clone();
            if next.from >= index {
                next.from += 1;
            }
            if next.to >= index {
                next.to += 1;
            }
            next
        })
        .collect()
}

/// Tags after the frame at `index` is removed.
///
/// Tags entirely after `index` shift left. Tags that covered `index` shrink.
/// A tag that named only that frame is dropped.
pub(crate) fn tags_after_remove(tags: &[Tag], index: usize) -> Vec<Tag> {
    let mut out = Vec::with_capacity(tags.len());
    for tag in tags {
        if tag.to < index {
            out.push(tag.clone());
            continue;
        }
        if tag.from > index {
            out.push(Tag {
                from: tag.from - 1,
                to: tag.to - 1,
                ..tag.clone()
            });
            continue;
        }
        if tag.from == tag.to {
            continue;
        }
        out.push(Tag {
            from: tag.from,
            to: tag.to - 1,
            ..tag.clone()
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(name: &str, from: usize, to: usize) -> Tag {
        Tag {
            name: name.to_string(),
            from,
            to,
            loop_mode: LoopMode::Forward,
        }
    }

    #[test]
    fn insert_shifts_later_tags_and_grows_a_tag_that_covers_the_point() {
        let tags = vec![tag("idle_down", 0, 0), tag("walk_down", 1, 3)];
        let appended = tags_after_insert(&tags, 4);
        assert_eq!(appended, tags);
        let interior = tags_after_insert(&tags, 2);
        assert_eq!(interior[0], tag("idle_down", 0, 0));
        assert_eq!(interior[1], tag("walk_down", 1, 4));
        let before = tags_after_insert(&tags, 0);
        assert_eq!(before[0], tag("idle_down", 1, 1));
        assert_eq!(before[1], tag("walk_down", 2, 4));
    }

    #[test]
    fn remove_shrinks_drops_and_shifts() {
        let tags = vec![tag("idle_down", 0, 0), tag("walk_down", 1, 3)];
        let dropped = tags_after_remove(&tags, 0);
        assert_eq!(dropped, vec![tag("walk_down", 0, 2)]);
        let shrunk = tags_after_remove(&tags, 2);
        assert_eq!(shrunk[0], tag("idle_down", 0, 0));
        assert_eq!(shrunk[1], tag("walk_down", 1, 2));
        let shifted = tags_after_remove(&tags, 0);
        assert_eq!(shifted[0].from, 0);
        assert!(normalize_tag_name(" walk_down ").is_ok());
        assert!(normalize_tag_name(" ").is_err());
        assert!(normalize_tag_name("a\nb").is_err());
        assert_eq!(LoopMode::parse("pingpong"), Some(LoopMode::PingPong));
        assert_eq!(LoopMode::parse("bounce"), None);
        assert_eq!(LoopMode::default().as_str(), "forward");
    }
}
