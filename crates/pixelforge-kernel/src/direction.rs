//! Four-direction and optional eight-direction frame layout.
//!
//! This is not an animation DSL and not a gait. A layout names how many
//! facing rows the existing tag list must cover, and how many frames sit in
//! each row. `idle` and `walk` use the same split the sprite sheet already
//! uses. Any other `{action}_{facing}` tag is the same action on every
//! facing, with the same columns. Naming those tags does not paint pixels.
//!
//! Row order matches the sheet, not compass order:
//!
//! | Row | Compass | Tag suffix |
//! | --- | --- | --- |
//! | 0 | S | `down` |
//! | 1 | W | `left` |
//! | 2 | E | `right` |
//! | 3 | N | `up` |
//! | 4 | SW | `downleft` |
//! | 5 | SE | `downright` |
//! | 6 | NW | `upleft` |
//! | 7 | NE | `upright` |
//!
//! Count 4 is the formal set (N, E, S, W). Count 8 appends the four diagonals
//! and does not move the first four rows. A tag may use the screen suffix or
//! a compass alias (`south`, `s`, `sw`, …). Auto-named tags use the screen
//! suffix, so an existing `idle_down` sheet stays the canonical name.

use serde::Serialize;

use crate::error::KernelError;
use crate::tag::{LoopMode, Tag};
use crate::MAX_BAKE_FRAMES;

/// One facing row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Facing {
    pub index: u8,
    pub compass: &'static str,
    pub formal: &'static str,
    pub screen: &'static str,
    pub diagonal: bool,
}

const FACINGS: [Facing; 8] = [
    Facing {
        index: 0,
        compass: "S",
        formal: "south",
        screen: "down",
        diagonal: false,
    },
    Facing {
        index: 1,
        compass: "W",
        formal: "west",
        screen: "left",
        diagonal: false,
    },
    Facing {
        index: 2,
        compass: "E",
        formal: "east",
        screen: "right",
        diagonal: false,
    },
    Facing {
        index: 3,
        compass: "N",
        formal: "north",
        screen: "up",
        diagonal: false,
    },
    Facing {
        index: 4,
        compass: "SW",
        formal: "southwest",
        screen: "downleft",
        diagonal: true,
    },
    Facing {
        index: 5,
        compass: "SE",
        formal: "southeast",
        screen: "downright",
        diagonal: true,
    },
    Facing {
        index: 6,
        compass: "NW",
        formal: "northwest",
        screen: "upleft",
        diagonal: true,
    },
    Facing {
        index: 7,
        compass: "NE",
        formal: "northeast",
        screen: "upright",
        diagonal: true,
    },
];

/// Facings for a layout count of 4 or 8. Any other count is the four cardinals.
pub fn facings_for(count: u8) -> &'static [Facing] {
    if count == 8 {
        &FACINGS
    } else {
        &FACINGS[..4]
    }
}

/// Stored grid. Omitted from the project file when unset. Version stays 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectionLayout {
    pub count: u8,
    pub columns: u32,
}

impl DirectionLayout {
    /// `count` is 4 or 8. `columns` is at least 1. The grid is at most
    /// [`MAX_BAKE_FRAMES`] frames.
    pub fn new(count: u8, columns: u32) -> Result<Self, KernelError> {
        if count != 4 && count != 8 {
            return Err(KernelError::DirectionLayout);
        }
        if columns == 0 {
            return Err(KernelError::DirectionLayout);
        }
        let frames = u64::from(count) * u64::from(columns);
        if frames == 0 || frames > MAX_BAKE_FRAMES as u64 {
            return Err(KernelError::DirectionLayout);
        }
        Ok(Self { count, columns })
    }

    pub fn frame_len(self) -> usize {
        usize::from(self.count) * self.columns as usize
    }

    pub fn facings(self) -> &'static [Facing] {
        facings_for(self.count)
    }
}

/// What `{action}_{facing}` names. `idle` and `walk` are the sheet split.
/// Anything else is an existing animation type that must repeat per facing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectionAction {
    Idle,
    Walk,
    Other(String),
}

impl DirectionAction {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Idle => "idle",
            Self::Walk => "walk",
            Self::Other(name) => name,
        }
    }

    fn is(&self, action: &str) -> bool {
        self.as_str() == action
    }
}

/// Parse a direction argument. Case and `-` / `_` do not matter.
///
/// `S`, `south`, and `down` are the same facing. Diagonals accept `SW`,
/// `southwest`, `downleft`, and `down-left`.
pub fn parse_facing(text: &str) -> Option<&'static Facing> {
    let mut key = String::new();
    for ch in text.chars() {
        if ch == '-' || ch == '_' || ch.is_whitespace() {
            continue;
        }
        key.push(ch.to_ascii_lowercase());
    }
    FACINGS.iter().find(|facing| {
        key == facing.screen || key == facing.formal || key.eq_ignore_ascii_case(facing.compass)
    })
}

/// Tag suffix lookup. The suffix is the text after the last `_`, compared
/// case-sensitively to the screen name, the formal name, or the lowercase
/// compass token (`s`, `sw`).
pub fn facing_from_suffix(suffix: &str) -> Option<&'static Facing> {
    FACINGS.iter().find(|facing| {
        suffix == facing.screen
            || suffix == facing.formal
            || suffix.eq_ignore_ascii_case(facing.compass)
    })
}

/// `{action}_{facing}` with a single-token action and a known facing suffix.
///
/// `walk_down_left` is not a direction tag: the action would contain `_`.
/// Diagonal names are one token (`downleft`, `sw`), not `down_left`.
pub fn parse_direction_tag(name: &str) -> Option<(DirectionAction, &'static Facing)> {
    let (action, suffix) = name.rsplit_once('_')?;
    if !is_action(action) {
        return None;
    }
    let facing = facing_from_suffix(suffix)?;
    let kind = match action {
        "idle" => DirectionAction::Idle,
        "walk" => DirectionAction::Walk,
        other => DirectionAction::Other(other.to_string()),
    };
    Some((kind, facing))
}

fn is_action(action: &str) -> bool {
    let mut chars = action.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() => chars.all(|ch| ch.is_ascii_alphanumeric()),
        _ => false,
    }
}

/// What is wrong with the idle/walk pair on one row.
///
/// The sheet rule: idle, when present, is the first frame only. Walk is the
/// rest of the row. A row with no idle may be walk across the whole row. A
/// one-frame row has no walk after idle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleWalkFault {
    MissingBoth,
    MissingWalk { walk_from: u32, walk_to: u32 },
    IdleSpan { frame: u32 },
    WalkSpan { from: u32, to: u32 },
    WalkBesideOnlyIdle,
    DuplicateIdle,
    DuplicateWalk,
}

pub fn idle_walk_faults(
    idle_spans: &[(u32, u32)],
    walk_spans: &[(u32, u32)],
    row_start: u32,
    row_end: u32,
) -> Vec<IdleWalkFault> {
    let mut faults = Vec::new();
    if idle_spans.len() > 1 {
        faults.push(IdleWalkFault::DuplicateIdle);
    }
    if walk_spans.len() > 1 {
        faults.push(IdleWalkFault::DuplicateWalk);
    }
    let idle = idle_spans.first().copied();
    let walk = walk_spans.first().copied();
    if idle.is_none() && walk.is_none() {
        faults.push(IdleWalkFault::MissingBoth);
        return faults;
    }
    if let Some((from, to)) = idle {
        if from != row_start || to != row_start {
            faults.push(IdleWalkFault::IdleSpan { frame: row_start });
        }
    }
    if idle.is_none() {
        if let Some((from, to)) = walk {
            if from != row_start || to != row_end {
                faults.push(IdleWalkFault::WalkSpan {
                    from: row_start,
                    to: row_end,
                });
            }
        }
    } else if row_start >= row_end {
        if walk.is_some() {
            faults.push(IdleWalkFault::WalkBesideOnlyIdle);
        }
    } else if let Some((from, to)) = walk {
        let expect_from = row_start + 1;
        if from != expect_from || to != row_end {
            faults.push(IdleWalkFault::WalkSpan {
                from: expect_from,
                to: row_end,
            });
        }
    } else {
        faults.push(IdleWalkFault::MissingWalk {
            walk_from: row_start + 1,
            walk_to: row_end,
        });
    }
    faults
}

/// One hard miss. `code` is `layout`, `frames`, `missingDirection`, or `segment`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectionFinding {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facing: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compass: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectionSegmentView {
    pub action: String,
    pub name: String,
    pub from: u32,
    pub to: u32,
    #[serde(rename = "loop")]
    pub loop_mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectionFacingView {
    pub row: u32,
    pub compass: &'static str,
    pub screen: &'static str,
    pub from: u32,
    pub to: u32,
    pub present: bool,
    pub segments: Vec<DirectionSegmentView>,
}

/// Read-only checklist. Errors are hard misses: a caller exits 1 when
/// `error_count` is not zero. There is no loose mode on this gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectionReport {
    pub count: Option<u8>,
    pub columns: Option<u32>,
    pub error_count: usize,
    pub errors: Vec<DirectionFinding>,
    pub facings: Vec<DirectionFacingView>,
}

impl DirectionReport {
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

struct OwnedTag {
    name: String,
    action: DirectionAction,
    facing: &'static Facing,
    from: u32,
    to: u32,
    loop_mode: LoopMode,
}

/// Checklist for `tags` against `layout`. Does not write.
pub fn report(
    frame_count: usize,
    tags: &[Tag],
    layout: Option<DirectionLayout>,
) -> DirectionReport {
    let Some(layout) = layout else {
        let errors = vec![finding("layout", "direction layout is not set", None, None)];
        return DirectionReport {
            count: None,
            columns: None,
            error_count: errors.len(),
            errors,
            facings: Vec::new(),
        };
    };

    let mut errors = Vec::new();
    let need = layout.frame_len();
    if frame_count != need {
        errors.push(DirectionFinding {
            code: "frames",
            message: format!(
                "document has {frame_count} frames; a {}-direction layout of {} columns covers {need}",
                layout.count, layout.columns
            ),
            facing: None,
            compass: None,
            action: None,
        });
    }

    let parsed = parse_owned(tags);
    for tag in &parsed {
        if tag.facing.index >= layout.count {
            errors.push(DirectionFinding {
                code: "segment",
                message: format!(
                    "tag `{}` names {} ({}), outside the {}-direction layout",
                    tag.name, tag.facing.formal, tag.facing.compass, layout.count
                ),
                facing: Some(tag.facing.screen),
                compass: Some(tag.facing.compass),
                action: Some(tag.action.as_str().to_string()),
            });
        }
    }

    let mut facings = Vec::new();
    for facing in layout.facings() {
        let row_start = u32::from(facing.index) * layout.columns;
        let row_end = row_start + layout.columns - 1;
        let present = (row_end as usize) < frame_count;
        if !present {
            errors.push(DirectionFinding {
                code: "missingDirection",
                message: format!(
                    "facing {} ({}) needs frames {row_start}-{row_end}; the document has {frame_count} frames",
                    facing.screen, facing.compass
                ),
                facing: Some(facing.screen),
                compass: Some(facing.compass),
                action: None,
            });
        }

        let on_row: Vec<&OwnedTag> = parsed
            .iter()
            .filter(|tag| tag.facing.index == facing.index)
            .collect();
        let mut segments = Vec::new();
        for tag in &on_row {
            segments.push(DirectionSegmentView {
                action: tag.action.as_str().to_string(),
                name: tag.name.clone(),
                from: tag.from,
                to: tag.to,
                loop_mode: tag.loop_mode.as_str().to_string(),
            });
        }

        if present {
            let idles = spans_of(&on_row, "idle");
            let walks = spans_of(&on_row, "walk");
            for fault in idle_walk_faults(&idles, &walks, row_start, row_end) {
                errors.push(fault_finding(facing, fault));
            }
        }

        facings.push(DirectionFacingView {
            row: u32::from(facing.index),
            compass: facing.compass,
            screen: facing.screen,
            from: row_start,
            to: row_end,
            present,
            segments,
        });
    }

    check_other_actions(layout, &parsed, &mut errors);

    DirectionReport {
        count: Some(layout.count),
        columns: Some(layout.columns),
        error_count: errors.len(),
        errors,
        facings,
    }
}

fn check_other_actions(
    layout: DirectionLayout,
    tags: &[OwnedTag],
    errors: &mut Vec<DirectionFinding>,
) {
    let mut actions: Vec<String> = Vec::new();
    for tag in tags {
        if tag.facing.index >= layout.count {
            continue;
        }
        if matches!(tag.action, DirectionAction::Idle | DirectionAction::Walk) {
            continue;
        }
        let name = tag.action.as_str().to_string();
        if !actions.iter().any(|existing| existing == &name) {
            actions.push(name);
        }
    }
    for action in actions {
        let mut reference: Option<(u32, u32)> = None;
        for facing in layout.facings() {
            let row_start = u32::from(facing.index) * layout.columns;
            let row_end = row_start + layout.columns - 1;
            let hits: Vec<&OwnedTag> = tags
                .iter()
                .filter(|tag| tag.facing.index == facing.index && tag.action.is(&action))
                .collect();
            if hits.is_empty() {
                errors.push(DirectionFinding {
                    code: "missingDirection",
                    message: format!(
                        "facing {} ({}) has no {action}_{} tag",
                        facing.screen, facing.compass, facing.screen
                    ),
                    facing: Some(facing.screen),
                    compass: Some(facing.compass),
                    action: Some(action.clone()),
                });
                continue;
            }
            if hits.len() > 1 {
                errors.push(DirectionFinding {
                    code: "segment",
                    message: format!(
                        "facing {} ({}) {action} is named more than once",
                        facing.screen, facing.compass
                    ),
                    facing: Some(facing.screen),
                    compass: Some(facing.compass),
                    action: Some(action.clone()),
                });
                continue;
            }
            let tag = hits[0];
            if tag.from < row_start || tag.to > row_end || tag.from > tag.to {
                errors.push(DirectionFinding {
                    code: "segment",
                    message: format!(
                        "tag `{}` covers frames {}-{}, outside {} ({}) frames {row_start}-{row_end}",
                        tag.name, tag.from, tag.to, facing.screen, facing.compass
                    ),
                    facing: Some(facing.screen),
                    compass: Some(facing.compass),
                    action: Some(action.clone()),
                });
                continue;
            }
            let span = (tag.from - row_start, tag.to - row_start);
            match reference {
                None => reference = Some(span),
                Some(expected) if expected != span => {
                    errors.push(DirectionFinding {
                        code: "segment",
                        message: format!(
                            "{action} on {} ({}) covers columns {}-{}, expected {}-{}",
                            facing.screen, facing.compass, span.0, span.1, expected.0, expected.1
                        ),
                        facing: Some(facing.screen),
                        compass: Some(facing.compass),
                        action: Some(action.clone()),
                    });
                }
                Some(_) => {}
            }
        }
    }
}

fn spans_of(tags: &[&OwnedTag], action: &str) -> Vec<(u32, u32)> {
    tags.iter()
        .filter(|tag| tag.action.is(action))
        .map(|tag| (tag.from, tag.to))
        .collect()
}

fn fault_finding(facing: &Facing, fault: IdleWalkFault) -> DirectionFinding {
    let (code, message) = match fault {
        IdleWalkFault::MissingBoth => (
            "missingDirection",
            format!(
                "facing {} ({}) has no idle_{} or walk_{} tag",
                facing.screen, facing.compass, facing.screen, facing.screen
            ),
        ),
        IdleWalkFault::MissingWalk { walk_from, walk_to } => (
            "missingDirection",
            format!(
                "facing {} ({}) has no walk_{} for frames {walk_from}-{walk_to}",
                facing.screen, facing.compass, facing.screen
            ),
        ),
        IdleWalkFault::IdleSpan { frame } => (
            "segment",
            format!(
                "idle_{} ({}) must cover only frame {frame}",
                facing.screen, facing.compass
            ),
        ),
        IdleWalkFault::WalkSpan { from, to } => (
            "segment",
            format!(
                "walk_{} ({}) must cover frames {from}-{to}",
                facing.screen, facing.compass
            ),
        ),
        IdleWalkFault::WalkBesideOnlyIdle => (
            "segment",
            format!(
                "walk_{} ({}) has no frames after idle on a one-frame row",
                facing.screen, facing.compass
            ),
        ),
        IdleWalkFault::DuplicateIdle => (
            "segment",
            format!(
                "facing {} ({}) idle is named more than once",
                facing.screen, facing.compass
            ),
        ),
        IdleWalkFault::DuplicateWalk => (
            "segment",
            format!(
                "facing {} ({}) walk is named more than once",
                facing.screen, facing.compass
            ),
        ),
    };
    let action = match fault {
        IdleWalkFault::MissingBoth
        | IdleWalkFault::DuplicateIdle
        | IdleWalkFault::IdleSpan { .. } => Some("idle".to_string()),
        IdleWalkFault::MissingWalk { .. }
        | IdleWalkFault::WalkSpan { .. }
        | IdleWalkFault::WalkBesideOnlyIdle
        | IdleWalkFault::DuplicateWalk => Some("walk".to_string()),
    };
    let action = if matches!(fault, IdleWalkFault::MissingBoth) {
        None
    } else {
        action
    };
    DirectionFinding {
        code,
        message,
        facing: Some(facing.screen),
        compass: Some(facing.compass),
        action,
    }
}

fn finding(
    code: &'static str,
    message: &str,
    facing: Option<&'static Facing>,
    action: Option<String>,
) -> DirectionFinding {
    DirectionFinding {
        code,
        message: message.to_string(),
        facing: facing.map(|facing| facing.screen),
        compass: facing.map(|facing| facing.compass),
        action,
    }
}

fn parse_owned(tags: &[Tag]) -> Vec<OwnedTag> {
    tags.iter()
        .filter_map(|tag| {
            let (action, facing) = parse_direction_tag(&tag.name)?;
            let from = u32::try_from(tag.from).ok()?;
            let to = u32::try_from(tag.to).ok()?;
            Some(OwnedTag {
                name: tag.name.clone(),
                action,
                facing,
                from,
                to,
                loop_mode: tag.loop_mode,
            })
        })
        .collect()
}

/// Tags after writing the standard idle/walk split. Other tags stay.
///
/// A one-frame row gets `idle_{screen}` only. A longer row gets idle on the
/// first frame and walk on the rest. An alias such as `idle_south` is kept
/// and its range is corrected. Two names for the same segment is
/// [`KernelError::DirectionSegment`] and the returned list is not applied.
pub fn tags_after_apply(tags: &[Tag], layout: DirectionLayout) -> Result<Vec<Tag>, KernelError> {
    let mut next = tags.to_vec();
    for facing in layout.facings() {
        let row = usize::from(facing.index) * layout.columns as usize;
        for (action, rel_from, rel_to) in standard_spans(layout.columns) {
            let from = row + rel_from;
            let to = row + rel_to;
            let hits: Vec<usize> = next
                .iter()
                .enumerate()
                .filter(|(_, tag)| {
                    matches!(
                        parse_direction_tag(&tag.name),
                        Some((kind, found)) if kind.is(action) && found.index == facing.index
                    )
                })
                .map(|(index, _)| index)
                .collect();
            match hits.as_slice() {
                [] => {
                    let name = format!("{action}_{}", facing.screen);
                    if next.iter().any(|tag| tag.name == name) {
                        return Err(KernelError::DirectionSegment);
                    }
                    next.push(Tag {
                        name,
                        from,
                        to,
                        loop_mode: LoopMode::Forward,
                    });
                }
                [index] => {
                    next[*index].from = from;
                    next[*index].to = to;
                }
                _ => return Err(KernelError::DirectionSegment),
            }
        }
    }
    Ok(next)
}

/// One facing exported as a single row. Not a second atlas format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectionSheet {
    pub png: Vec<u8>,
    pub columns: u32,
    pub sheet_width: u32,
    pub sheet_height: u32,
    pub scale: u32,
    pub from: u32,
    pub to: u32,
    pub screen: &'static str,
    pub compass: &'static str,
    pub row: u32,
}

fn standard_spans(columns: u32) -> Vec<(&'static str, usize, usize)> {
    if columns <= 1 {
        vec![("idle", 0, 0)]
    } else {
        vec![("idle", 0, 0), ("walk", 1, columns as usize - 1)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compass_and_screen_name_the_same_row() {
        assert_eq!(parse_facing("S").unwrap().screen, "down");
        assert_eq!(parse_facing("north").unwrap().index, 3);
        assert_eq!(parse_facing("down-left").unwrap().compass, "SW");
        assert_eq!(parse_facing("NE").unwrap().screen, "upright");
        assert!(parse_facing("side").is_none());
        assert_eq!(parse_direction_tag("idle_south").unwrap().1.screen, "down");
        assert_eq!(parse_direction_tag("walk_sw").unwrap().1.index, 4);
        assert!(parse_direction_tag("walk_down_left").is_none());
        assert!(parse_direction_tag("barricade").is_none());
        assert!(matches!(
            parse_direction_tag("attack_right").unwrap().0,
            DirectionAction::Other(name) if name == "attack"
        ));
    }

    #[test]
    fn idle_walk_split_matches_the_sheet_rule() {
        assert_eq!(
            idle_walk_faults(&[], &[], 0, 3),
            vec![IdleWalkFault::MissingBoth]
        );
        assert!(idle_walk_faults(&[(0, 0)], &[(1, 3)], 0, 3).is_empty());
        assert!(idle_walk_faults(&[], &[(0, 3)], 0, 3).is_empty());
        assert!(idle_walk_faults(&[(4, 4)], &[], 4, 4).is_empty());
        assert_eq!(
            idle_walk_faults(&[(0, 0)], &[], 0, 3),
            vec![IdleWalkFault::MissingWalk {
                walk_from: 1,
                walk_to: 3
            }]
        );
        assert_eq!(
            idle_walk_faults(&[(0, 0)], &[(0, 0)], 0, 0),
            vec![IdleWalkFault::WalkBesideOnlyIdle]
        );
    }

    #[test]
    fn layout_refuses_a_count_other_than_four_or_eight() {
        assert!(DirectionLayout::new(4, 4).is_ok());
        assert!(DirectionLayout::new(8, 1).is_ok());
        assert!(DirectionLayout::new(6, 4).is_err());
        assert!(DirectionLayout::new(4, 0).is_err());
        assert!(DirectionLayout::new(8, 600).is_err());
    }
}
