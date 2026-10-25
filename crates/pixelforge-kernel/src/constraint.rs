//! Follow, attach, and pin constraints on named parts.
//!
//! A constraint is metadata. It does not run when a part is transformed or a
//! curve key is written. [`crate::Document::solve_constraints`] is the step
//! that samples an optional curve pose, then translates driven parts so the
//! named points meet. Linked-layer ink moves with that translation the same
//! way [`crate::Document::translate_part`] moves it. An unattached part only
//! moves its origin. One call poses one pixel frame. [`crate::Document::bake`]
//! walks the timeline by sampling the same pose and writing each frame.

use crate::curve::{sample_keys, Curve, CurveChannel, PartSample};
use crate::error::KernelError;
use crate::part::{anchor_offset_i64, normalize_part_name, Anchor, Part};
use crate::part_xform::{self, map_point, CellMap, Motion};
use crate::TRANSPARENT_INDEX;

/// What a stored constraint asks the solver to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintKind {
    /// Translate `part` so its anchor, or its origin when the anchor is
    /// omitted, lands on `target`'s `target_anchor`.
    Follow,
    /// Translate `part` so its anchor lands on `target`'s `target_anchor`.
    /// Both anchors are required. Used for a butt stuck to a shoulder.
    Attach,
    /// Translate `part` so its anchor, or its origin, lands on a canvas cell.
    Pin,
}

impl ConstraintKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Follow => "follow",
            Self::Attach => "attach",
            Self::Pin => "pin",
        }
    }

    /// Tool and CLI spelling. `stick` is stored as [`Self::Attach`].
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "follow" => Some(Self::Follow),
            "attach" | "stick" => Some(Self::Attach),
            "pin" => Some(Self::Pin),
            _ => None,
        }
    }

    /// Spelling written in the project file. `stick` is not stored.
    pub fn parse_stored(text: &str) -> Option<Self> {
        match text {
            "follow" => Some(Self::Follow),
            "attach" => Some(Self::Attach),
            "pin" => Some(Self::Pin),
            _ => None,
        }
    }
}

/// One constraint. Empty lists are omitted from the project file.
///
/// `follow` and `attach` name another part. `pin` stores a canvas cell in
/// `x` and `y`. Local anchors are not copied here; the solver reads them
/// from the part at solve time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constraint {
    pub name: String,
    pub kind: ConstraintKind,
    pub part: String,
    pub anchor: Option<String>,
    pub target: Option<String>,
    pub target_anchor: Option<String>,
    pub x: Option<u16>,
    pub y: Option<u16>,
}

/// One constraint after a solve, in file order of the returned list's `index`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolvedConstraint {
    pub index: usize,
    pub name: String,
    pub kind: ConstraintKind,
    pub part: String,
    pub anchor: Option<String>,
    pub target: Option<String>,
    pub target_anchor: Option<String>,
    pub x: Option<u16>,
    pub y: Option<u16>,
    /// Whole cells added to the follower by this constraint. Curve translation
    /// is not included.
    pub dx: i32,
    pub dy: i32,
    pub satisfied: bool,
    pub from_x: i32,
    pub from_y: i32,
    pub to_x: i32,
    pub to_y: i32,
}

/// Canvas position of one anchor on the solved pose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolvedAnchor {
    pub name: String,
    pub x: i16,
    pub y: i16,
    pub canvas_x: i32,
    pub canvas_y: i32,
}

/// One part on the solved pose.
///
/// `ink` is `unchanged` when this solve would not move the part, `metadata`
/// when the part has no linked layer so only the origin and anchors change,
/// and `moved` when the linked layer's ink moves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolvedPart {
    pub name: String,
    pub x: u16,
    pub y: u16,
    pub layer: Option<usize>,
    pub ink: &'static str,
    pub dropped: u32,
    pub anchors: Vec<SolvedAnchor>,
}

/// What [`crate::Document::solve_constraints`] computed.
#[derive(Debug, Clone, PartialEq)]
pub struct SolveReport {
    pub frame: Option<f64>,
    pub pixel_frame: usize,
    pub apply: bool,
    pub wrote: bool,
    pub dropped: u32,
    /// Constraint names in solve order.
    pub order: Vec<String>,
    pub constraints: Vec<SolvedConstraint>,
    pub parts: Vec<SolvedPart>,
}

/// Oriented part plus the translations that still have to be applied.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PosedPart {
    pub oriented: Part,
    pub steps: Vec<MotionStep>,
    pub curve_dx: i32,
    pub curve_dy: i32,
    pub constraint_dx: i32,
    pub constraint_dy: i32,
}

/// One grid map already applied to `oriented`, or waiting as a final translation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MotionStep {
    pub map: CellMap,
    pub pivot_x: i32,
    pub pivot_y: i32,
}

/// Build the canonical constraint, or refuse it before it is stored.
pub(crate) fn prepare_constraint(
    existing: &[Constraint],
    parts: &[Part],
    width: u16,
    height: u16,
    name: &str,
    kind: ConstraintKind,
    part: &str,
    anchor: Option<&str>,
    target: Option<&str>,
    target_anchor: Option<&str>,
    x: Option<i32>,
    y: Option<i32>,
) -> Result<Constraint, KernelError> {
    let name = normalize_part_name(name).map_err(|()| KernelError::ConstraintName)?;
    if existing.iter().any(|constraint| constraint.name == name) {
        return Err(KernelError::ConstraintExists);
    }
    let part_name = normalize_part_name(part).map_err(|()| KernelError::PartName)?;
    let follower = part_named(parts, &part_name)?;
    let anchor = optional_anchor(anchor)?;
    let target = match target {
        None => None,
        Some(target) => Some(normalize_part_name(target).map_err(|()| KernelError::PartName)?),
    };
    let target_anchor = optional_anchor(target_anchor)?;
    if let Some(anchor) = &anchor {
        require_anchor(follower, anchor)?;
    }
    let (x, y) = match (x, y) {
        (None, None) => (None, None),
        (Some(x), Some(y)) => {
            if x < 0 || y < 0 || x >= i32::from(width) || y >= i32::from(height) {
                return Err(KernelError::PartOffCanvas { x, y });
            }
            (
                Some(u16::try_from(x).expect("pin cell was bounded by the canvas")),
                Some(u16::try_from(y).expect("pin cell was bounded by the canvas")),
            )
        }
        _ => return Err(KernelError::ConstraintFields),
    };
    match kind {
        ConstraintKind::Follow => {
            if x.is_some() {
                return Err(KernelError::ConstraintFields);
            }
            let target_name = target.as_deref().ok_or(KernelError::ConstraintFields)?;
            let target_anchor = target_anchor
                .as_deref()
                .ok_or(KernelError::ConstraintFields)?;
            let target_part = part_named(parts, target_name)?;
            require_anchor(target_part, target_anchor)?;
            if target_name == part_name {
                return Err(KernelError::ConstraintCycle { name: name.clone() });
            }
        }
        ConstraintKind::Attach => {
            if x.is_some() || anchor.is_none() {
                return Err(KernelError::ConstraintFields);
            }
            let target_name = target.as_deref().ok_or(KernelError::ConstraintFields)?;
            let target_anchor = target_anchor
                .as_deref()
                .ok_or(KernelError::ConstraintFields)?;
            let target_part = part_named(parts, target_name)?;
            require_anchor(target_part, target_anchor)?;
            if target_name == part_name {
                return Err(KernelError::ConstraintCycle { name: name.clone() });
            }
        }
        ConstraintKind::Pin => {
            if x.is_none() || target.is_some() || target_anchor.is_some() {
                return Err(KernelError::ConstraintFields);
            }
        }
    }
    let constraint = Constraint {
        name,
        kind,
        part: part_name,
        anchor,
        target,
        target_anchor,
        x,
        y,
    };
    let mut trial = existing.to_vec();
    trial.push(constraint.clone());
    solve_order(&trial)?;
    Ok(constraint)
}

/// Dependency order. A follower waits for a target that is itself driven.
///
/// Independent constraints keep file order. A cycle or a second driver for
/// the same part is an error.
pub(crate) fn solve_order(constraints: &[Constraint]) -> Result<Vec<usize>, KernelError> {
    for (index, constraint) in constraints.iter().enumerate() {
        if constraints
            .iter()
            .take(index)
            .any(|earlier| earlier.part == constraint.part)
        {
            return Err(KernelError::ConstraintConflict {
                part: constraint.part.clone(),
            });
        }
    }
    let mut incoming = vec![0usize; constraints.len()];
    let mut outgoing = vec![Vec::new(); constraints.len()];
    for (index, constraint) in constraints.iter().enumerate() {
        let Some(target) = constraint.target.as_deref() else {
            continue;
        };
        if target == constraint.part {
            return Err(KernelError::ConstraintCycle {
                name: constraint.name.clone(),
            });
        }
        if let Some(source) = constraints.iter().position(|item| item.part == target) {
            outgoing[source].push(index);
            incoming[index] += 1;
        }
    }
    let mut ready: Vec<usize> = incoming
        .iter()
        .enumerate()
        .filter(|(_, count)| **count == 0)
        .map(|(index, _)| index)
        .collect();
    let mut order = Vec::with_capacity(constraints.len());
    while !ready.is_empty() {
        ready.sort_unstable();
        let index = ready.remove(0);
        order.push(index);
        for dependent in outgoing[index].clone() {
            incoming[dependent] -= 1;
            if incoming[dependent] == 0 {
                ready.push(dependent);
            }
        }
    }
    if order.len() != constraints.len() {
        let index = incoming
            .iter()
            .position(|count| *count > 0)
            .expect("an unordered constraint remains");
        return Err(KernelError::ConstraintCycle {
            name: constraints[index].name.clone(),
        });
    }
    Ok(order)
}

/// Curve pose for one part. `frame: None` leaves the stored pose alone.
///
/// Channel order when `frame` is set: rotation, then horizontal flip, then
/// vertical flip, each around the stored origin, then whole-cell translation.
/// A fractional translation snaps to the nearest cell, ties away from zero.
pub(crate) fn orient_part(
    part: &Part,
    curves: &[Curve],
    frame: Option<f64>,
    width: u16,
    height: u16,
) -> Result<PosedPart, KernelError> {
    let mut oriented = part.clone();
    let mut steps = Vec::new();
    let mut curve_dx = 0;
    let mut curve_dy = 0;
    if let Some(frame) = frame {
        let sample = sample_pose(curves, &part.name, frame);
        if let Some(degrees) = sample.rotation {
            apply_rotation(&mut oriented, degrees, width, height, &mut steps)?;
        }
        if let Some(value) = sample.flip_horizontal {
            if flip_flag(value)? {
                apply_map(
                    &mut oriented,
                    &CellMap::FlipHorizontal,
                    width,
                    height,
                    &mut steps,
                )?;
            }
        }
        if let Some(value) = sample.flip_vertical {
            if flip_flag(value)? {
                apply_map(
                    &mut oriented,
                    &CellMap::FlipVertical,
                    width,
                    height,
                    &mut steps,
                )?;
            }
        }
        if let Some(value) = sample.translate_x {
            curve_dx = snap_cells(value)?;
        }
        if let Some(value) = sample.translate_y {
            curve_dy = snap_cells(value)?;
        }
    }
    Ok(PosedPart {
        oriented,
        steps,
        curve_dx,
        curve_dy,
        constraint_dx: 0,
        constraint_dy: 0,
    })
}

/// Translate driven parts. Targets are already solved when a follower reads them.
pub(crate) fn apply_deltas(
    posed: &mut [PosedPart],
    constraints: &[Constraint],
    order: &[usize],
    width: u16,
    height: u16,
) -> Result<Vec<SolvedConstraint>, KernelError> {
    let mut solved = Vec::with_capacity(order.len());
    for &index in order {
        let constraint = &constraints[index];
        let follower = part_index(posed, &constraint.part)?;
        let (local_x, local_y) = local_of(&posed[follower].oriented, constraint.anchor.as_deref())?;
        let (from_x, from_y) = canvas_local(&posed[follower], local_x, local_y)?;
        let (to_x, to_y) = match constraint.kind {
            ConstraintKind::Follow | ConstraintKind::Attach => {
                let target_name = constraint
                    .target
                    .as_deref()
                    .ok_or(KernelError::ConstraintFields)?;
                let target_anchor = constraint
                    .target_anchor
                    .as_deref()
                    .ok_or(KernelError::ConstraintFields)?;
                let target = part_index(posed, target_name)?;
                let (local_x, local_y) = local_of(&posed[target].oriented, Some(target_anchor))?;
                canvas_local(&posed[target], local_x, local_y)?
            }
            ConstraintKind::Pin => {
                let x = constraint.x.ok_or(KernelError::ConstraintFields)?;
                let y = constraint.y.ok_or(KernelError::ConstraintFields)?;
                (i32::from(x), i32::from(y))
            }
        };
        let dx = to_x
            .checked_sub(from_x)
            .ok_or(KernelError::PartOffCanvas { x: to_x, y: to_y })?;
        let dy = to_y
            .checked_sub(from_y)
            .ok_or(KernelError::PartOffCanvas { x: to_x, y: to_y })?;
        posed[follower].constraint_dx = posed[follower]
            .constraint_dx
            .checked_add(dx)
            .ok_or(KernelError::PartOffCanvas { x: to_x, y: to_y })?;
        posed[follower].constraint_dy = posed[follower]
            .constraint_dy
            .checked_add(dy)
            .ok_or(KernelError::PartOffCanvas { x: to_x, y: to_y })?;
        place_origin(&posed[follower], width, height)?;
        solved.push(SolvedConstraint {
            index,
            name: constraint.name.clone(),
            kind: constraint.kind,
            part: constraint.part.clone(),
            anchor: constraint.anchor.clone(),
            target: constraint.target.clone(),
            target_anchor: constraint.target_anchor.clone(),
            x: constraint.x,
            y: constraint.y,
            dx,
            dy,
            satisfied: dx == 0 && dy == 0,
            from_x,
            from_y,
            to_x,
            to_y,
        });
    }
    for part in posed.iter() {
        place_origin(part, width, height)?;
    }
    solved.sort_by_key(|item| item.index);
    Ok(solved)
}

/// Origin after curve translation and constraint translation.
pub(crate) fn place_origin(
    posed: &PosedPart,
    width: u16,
    height: u16,
) -> Result<(u16, u16), KernelError> {
    let (x, y) = checked_origin(posed)?;
    match (u16::try_from(x), u16::try_from(y)) {
        (Ok(x), Ok(y)) if x < width && y < height => Ok((x, y)),
        _ => Err(KernelError::PartOffCanvas { x, y }),
    }
}

/// Canvas cell of a local offset on the posed part.
pub(crate) fn canvas_local(
    posed: &PosedPart,
    local_x: i32,
    local_y: i32,
) -> Result<(i32, i32), KernelError> {
    let (origin_x, origin_y) = checked_origin(posed)?;
    let x = origin_x
        .checked_add(local_x)
        .ok_or(KernelError::PartOffCanvas {
            x: local_x,
            y: local_y,
        })?;
    let y = origin_y
        .checked_add(local_y)
        .ok_or(KernelError::PartOffCanvas {
            x: local_x,
            y: local_y,
        })?;
    Ok((x, y))
}

/// Curve translation plus constraint translation, as one whole-cell shift.
pub(crate) fn total_delta(posed: &PosedPart) -> Result<(i32, i32), KernelError> {
    let dx = posed
        .curve_dx
        .checked_add(posed.constraint_dx)
        .ok_or(KernelError::PartOffCanvas {
            x: posed.curve_dx,
            y: posed.curve_dy,
        })?;
    let dy = posed
        .curve_dy
        .checked_add(posed.constraint_dy)
        .ok_or(KernelError::PartOffCanvas {
            x: posed.curve_dx,
            y: posed.curve_dy,
        })?;
    Ok((dx, dy))
}

/// Maps every non-zero cell through `steps`. The final cell must lie on the
/// canvas. The source cell and that final cell must both be accepted.
///
/// A collision keeps the later row-major index. `Ok` is the new buffer and
/// the number of dropped cells.
pub(crate) fn warp_ink(
    width: u16,
    height: u16,
    old: &[u8],
    steps: &[MotionStep],
    mut accepts: impl FnMut(u16, u16) -> bool,
) -> Result<(Vec<u8>, u32), KernelError> {
    let mut moves = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let offset = usize::from(width) * usize::from(y) + usize::from(x);
            let index = old[offset];
            if index == TRANSPARENT_INDEX {
                continue;
            }
            let (mut dest_x, mut dest_y) = (i32::from(x), i32::from(y));
            for step in steps {
                (dest_x, dest_y) = map_point(&step.map, dest_x, dest_y, step.pivot_x, step.pivot_y)
                    .map_err(|(x, y)| KernelError::PartOffCanvas { x, y })?;
            }
            let (dest_x, dest_y) = on_canvas(dest_x, dest_y, width, height)?;
            if !accepts(x, y) || !accepts(dest_x, dest_y) {
                return Err(KernelError::SelectionProtected);
            }
            let dest = usize::from(width) * usize::from(dest_y) + usize::from(dest_x);
            moves.push((index, dest));
        }
    }
    let mut claimed = vec![false; old.len()];
    let mut unique = 0u32;
    for (_, dest) in &moves {
        if !claimed[*dest] {
            claimed[*dest] = true;
            unique += 1;
        }
    }
    let dropped = u32::try_from(moves.len())
        .unwrap_or(u32::MAX)
        .saturating_sub(unique);
    let mut next = vec![TRANSPARENT_INDEX; old.len()];
    for (index, dest) in moves {
        next[dest] = index;
    }
    Ok((next, dropped))
}

/// Steps that take stored ink onto the solved pose.
pub(crate) fn motion_steps(posed: &PosedPart) -> Result<Vec<MotionStep>, KernelError> {
    let mut steps = posed.steps.clone();
    let (dx, dy) = total_delta(posed)?;
    if dx != 0 || dy != 0 {
        steps.push(MotionStep {
            map: CellMap::Translate { dx, dy },
            pivot_x: 0,
            pivot_y: 0,
        });
    }
    Ok(steps)
}

/// `ink` label for a part given whether its motion steps are empty.
pub(crate) fn ink_label(layer: Option<usize>, moves: bool) -> &'static str {
    match (layer, moves) {
        (_, false) => "unchanged",
        (None, true) => "metadata",
        (Some(_), true) => "moved",
    }
}

/// Constraints that named `from` now name `to`, as a part or as a target.
pub(crate) fn constraints_after_rename(
    constraints: &[Constraint],
    from: &str,
    to: &str,
) -> Vec<Constraint> {
    constraints
        .iter()
        .map(|constraint| {
            let mut next = constraint.clone();
            if next.part == from {
                next.part = to.to_string();
            }
            if next.target.as_deref() == Some(from) {
                next.target = Some(to.to_string());
            }
            next
        })
        .collect()
}

/// Drops a constraint whose part, or whose target, is no longer in `parts`.
pub(crate) fn constraints_for_parts(constraints: &[Constraint], parts: &[Part]) -> Vec<Constraint> {
    constraints
        .iter()
        .filter(|constraint| {
            parts.iter().any(|part| part.name == constraint.part)
                && constraint
                    .target
                    .as_ref()
                    .is_none_or(|target| parts.iter().any(|part| part.name == *target))
        })
        .cloned()
        .collect()
}

/// Moves pin cells with the canvas and drops a constraint that loses its part,
/// its target, or a pin cell.
pub(crate) fn constraints_after_resize(
    constraints: &[Constraint],
    parts: &[Part],
    offset_x: i32,
    offset_y: i32,
    width: u16,
    height: u16,
) -> Vec<Constraint> {
    constraints_for_parts(constraints, parts)
        .into_iter()
        .filter_map(|mut constraint| {
            if constraint.kind != ConstraintKind::Pin {
                return Some(constraint);
            }
            let (Some(x), Some(y)) = (constraint.x, constraint.y) else {
                return None;
            };
            let x = i32::from(x).checked_add(offset_x)?;
            let y = i32::from(y).checked_add(offset_y)?;
            let x = u16::try_from(x).ok()?;
            let y = u16::try_from(y).ok()?;
            if x >= width || y >= height {
                return None;
            }
            constraint.x = Some(x);
            constraint.y = Some(y);
            Some(constraint)
        })
        .collect()
}

fn sample_pose(curves: &[Curve], part: &str, frame: f64) -> PartSample {
    let mut sample = PartSample {
        part: part.to_string(),
        frame,
        rotation: None,
        translate_x: None,
        translate_y: None,
        flip_horizontal: None,
        flip_vertical: None,
    };
    for curve in curves.iter().filter(|curve| curve.part == part) {
        let value = sample_keys(curve.channel, &curve.keys, frame).value;
        match curve.channel {
            CurveChannel::Rotation => sample.rotation = Some(value),
            CurveChannel::TranslateX => sample.translate_x = Some(value),
            CurveChannel::TranslateY => sample.translate_y = Some(value),
            CurveChannel::FlipHorizontal => sample.flip_horizontal = Some(value),
            CurveChannel::FlipVertical => sample.flip_vertical = Some(value),
        }
    }
    sample
}

fn apply_rotation(
    part: &mut Part,
    degrees: f64,
    width: u16,
    height: u16,
    steps: &mut Vec<MotionStep>,
) -> Result<(), KernelError> {
    let resolved = part_xform::resolve_motion(Motion::Rotate(degrees))?;
    if resolved.degrees_centi == Some(0) {
        return Ok(());
    }
    apply_map(part, &resolved.map, width, height, steps)
}

fn apply_map(
    part: &mut Part,
    map: &CellMap,
    width: u16,
    height: u16,
    steps: &mut Vec<MotionStep>,
) -> Result<(), KernelError> {
    let pivot_x = i32::from(part.x);
    let pivot_y = i32::from(part.y);
    let (origin_x, origin_y) = map_point(map, pivot_x, pivot_y, pivot_x, pivot_y)
        .map_err(|(x, y)| KernelError::PartOffCanvas { x, y })?;
    let (origin_x, origin_y) = on_canvas(origin_x, origin_y, width, height)?;
    let mut anchors = Vec::with_capacity(part.anchors.len());
    for anchor in &part.anchors {
        let canvas_x = i32::from(part.x).checked_add(i32::from(anchor.x)).ok_or(
            KernelError::PartOffCanvas {
                x: i32::from(part.x),
                y: i32::from(part.y),
            },
        )?;
        let canvas_y = i32::from(part.y).checked_add(i32::from(anchor.y)).ok_or(
            KernelError::PartOffCanvas {
                x: i32::from(part.x),
                y: i32::from(part.y),
            },
        )?;
        let (mapped_x, mapped_y) = map_point(map, canvas_x, canvas_y, pivot_x, pivot_y)
            .map_err(|(x, y)| KernelError::PartOffCanvas { x, y })?;
        let local_x = i64::from(mapped_x) - i64::from(origin_x);
        let local_y = i64::from(mapped_y) - i64::from(origin_y);
        let Some((local_x, local_y)) = anchor_offset_i64(local_x, local_y) else {
            return Err(KernelError::AnchorCoord {
                x: i32_sat(local_x),
                y: i32_sat(local_y),
            });
        };
        anchors.push(Anchor {
            name: anchor.name.clone(),
            x: local_x,
            y: local_y,
        });
    }
    steps.push(MotionStep {
        map: *map,
        pivot_x,
        pivot_y,
    });
    part.x = origin_x;
    part.y = origin_y;
    part.anchors = anchors;
    Ok(())
}

fn checked_origin(posed: &PosedPart) -> Result<(i32, i32), KernelError> {
    let x = i32::from(posed.oriented.x)
        .checked_add(posed.curve_dx)
        .and_then(|value| value.checked_add(posed.constraint_dx))
        .ok_or(KernelError::PartOffCanvas {
            x: posed.curve_dx,
            y: posed.curve_dy,
        })?;
    let y = i32::from(posed.oriented.y)
        .checked_add(posed.curve_dy)
        .and_then(|value| value.checked_add(posed.constraint_dy))
        .ok_or(KernelError::PartOffCanvas {
            x: posed.curve_dx,
            y: posed.curve_dy,
        })?;
    Ok((x, y))
}

fn on_canvas(x: i32, y: i32, width: u16, height: u16) -> Result<(u16, u16), KernelError> {
    match (u16::try_from(x), u16::try_from(y)) {
        (Ok(x), Ok(y)) if x < width && y < height => Ok((x, y)),
        _ => Err(KernelError::PartOffCanvas { x, y }),
    }
}

fn local_of(part: &Part, anchor: Option<&str>) -> Result<(i32, i32), KernelError> {
    match anchor {
        None => Ok((0, 0)),
        Some(name) => {
            let anchor = part
                .anchors
                .iter()
                .find(|anchor| anchor.name == name)
                .ok_or(KernelError::AnchorMissing)?;
            Ok((i32::from(anchor.x), i32::from(anchor.y)))
        }
    }
}

fn part_index(posed: &[PosedPart], name: &str) -> Result<usize, KernelError> {
    posed
        .iter()
        .position(|part| part.oriented.name == name)
        .ok_or(KernelError::PartMissing)
}

fn part_named<'a>(parts: &'a [Part], name: &str) -> Result<&'a Part, KernelError> {
    parts
        .iter()
        .find(|part| part.name == name)
        .ok_or(KernelError::PartMissing)
}

fn require_anchor(part: &Part, name: &str) -> Result<(), KernelError> {
    if part.anchors.iter().any(|anchor| anchor.name == name) {
        Ok(())
    } else {
        Err(KernelError::AnchorMissing)
    }
}

fn optional_anchor(name: Option<&str>) -> Result<Option<String>, KernelError> {
    match name {
        None => Ok(None),
        Some(name) => Ok(Some(
            normalize_part_name(name).map_err(|()| KernelError::AnchorName)?,
        )),
    }
}

fn flip_flag(value: f64) -> Result<bool, KernelError> {
    if value == 0.0 {
        Ok(false)
    } else if value == 1.0 {
        Ok(true)
    } else {
        Err(KernelError::CurveValue)
    }
}

/// Nearest cell. A tie rounds away from zero, the same rule as a part angle.
fn snap_cells(value: f64) -> Result<i32, KernelError> {
    if !value.is_finite() {
        return Err(KernelError::CurveValue);
    }
    let rounded = value.round();
    if !rounded.is_finite() || rounded < f64::from(i32::MIN) || rounded > f64::from(i32::MAX) {
        return Err(KernelError::CurveValue);
    }
    Ok(rounded as i32)
}

fn i32_sat(value: i64) -> i32 {
    i32::try_from(value).unwrap_or(if value > 0 { i32::MAX } else { i32::MIN })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn constraint(name: &str, part: &str, target: &str) -> Constraint {
        Constraint {
            name: name.to_string(),
            kind: ConstraintKind::Follow,
            part: part.to_string(),
            anchor: Some("palm".to_string()),
            target: Some(target.to_string()),
            target_anchor: Some("grip".to_string()),
            x: None,
            y: None,
        }
    }

    #[test]
    fn half_cells_snap_away_from_zero() {
        assert_eq!(snap_cells(0.5).unwrap(), 1);
        assert_eq!(snap_cells(-0.5).unwrap(), -1);
        assert_eq!(snap_cells(0.4).unwrap(), 0);
        assert_eq!(snap_cells(1.0).unwrap(), 1);
    }

    #[test]
    fn a_driven_target_is_solved_before_its_follower() {
        let follow = constraint("hand-grip", "hand", "gun");
        let mut attach = constraint("butt-shoulder", "gun", "shoulder");
        attach.kind = ConstraintKind::Attach;
        let order = solve_order(&[follow, attach]).unwrap();
        assert_eq!(order, vec![1, 0]);
    }

    #[test]
    fn a_cycle_and_a_second_driver_are_errors() {
        let hand = constraint("hand-grip", "hand", "gun");
        let gun = constraint("gun-hand", "gun", "hand");
        assert!(matches!(
            solve_order(&[hand, gun]),
            Err(KernelError::ConstraintCycle { .. })
        ));
        let again = constraint("hand-again", "hand", "gun");
        let hand = constraint("hand-grip", "hand", "gun");
        assert!(matches!(
            solve_order(&[hand, again]),
            Err(KernelError::ConstraintConflict { .. })
        ));
    }
}
