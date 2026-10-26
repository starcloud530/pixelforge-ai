//! Follow, attach, pin, short-chain IK, and footprint constraints.
//!
//! A constraint is metadata. It does not run when a part is transformed or a
//! curve key is written. [`crate::Document::solve_constraints`] is the step
//! that samples an optional curve pose, then poses driven parts.
//! `follow`, `attach`, `pin`, and `plant` translate. `ik` rotates a chain of
//! two or three parts so the end effector lands on a target cell. A miss is
//! [`KernelError::IkUnreachable`] and writes nothing. Linked-layer ink moves
//! with that motion the same way [`crate::Document::translate_part`] moves it.
//! An unattached part only moves its origin. One call poses one pixel frame.
//! [`crate::Document::bake`] walks the timeline by sampling the same pose and
//! writing each frame.

use std::collections::BTreeMap;

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
    /// Rotate a chain of two or three parts so the end effector lands on a
    /// target cell or a target anchor. A miss is an error.
    Ik,
    /// Translate `part` onto a footprint span. Frames outside every span are
    /// left on the curve pose, so a planted span cannot slide.
    Plant,
}

impl ConstraintKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Follow => "follow",
            Self::Attach => "attach",
            Self::Pin => "pin",
            Self::Ik => "ik",
            Self::Plant => "plant",
        }
    }

    /// Tool and CLI spelling. `stick` is stored as [`Self::Attach`].
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "follow" => Some(Self::Follow),
            "attach" | "stick" => Some(Self::Attach),
            "pin" => Some(Self::Pin),
            "ik" => Some(Self::Ik),
            "plant" => Some(Self::Plant),
            _ => None,
        }
    }

    /// Spelling written in the project file. `stick` is not stored.
    pub fn parse_stored(text: &str) -> Option<Self> {
        match text {
            "follow" => Some(Self::Follow),
            "attach" => Some(Self::Attach),
            "pin" => Some(Self::Pin),
            "ik" => Some(Self::Ik),
            "plant" => Some(Self::Plant),
            _ => None,
        }
    }
}

/// One inclusive frame span where a sole, or an IK target, stays on a cell.
///
/// `frame` and `to` are frame indices. A sample at `f` hits the span when
/// `frame <= f <= to`. Spans on one constraint do not overlap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contact {
    pub frame: u16,
    pub to: u16,
    pub x: u16,
    pub y: u16,
}

/// One constraint. Empty lists are omitted from the project file.
///
/// `follow` and `attach` name another part. `pin` stores a canvas cell in
/// `x` and `y`. `plant` stores footprint spans in `contacts`. `ik` stores a
/// root-first `chain` of two or three parts (the last is `part`) and either
/// a canvas cell, a target anchor, or footprint spans. Local anchors are not
/// copied here; the solver reads them from the part at solve time.
///
/// `bend` is `1` when the first joint sits on the clockwise side of the
/// root-to-target direction, and `-1` for the other side. `None` keeps the
/// rest pose's side. A straight rest pose accepts either side.
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
    pub chain: Vec<String>,
    pub bend: Option<i8>,
    pub contacts: Vec<Contact>,
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
    /// True when a footprint span chose the target cell for this frame.
    pub planted: bool,
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
    frame_count: usize,
    name: &str,
    kind: ConstraintKind,
    part: &str,
    anchor: Option<&str>,
    target: Option<&str>,
    target_anchor: Option<&str>,
    x: Option<i32>,
    y: Option<i32>,
    chain: &[String],
    bend: Option<i8>,
    contacts: &[Contact],
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
    if bend.is_some_and(|bend| bend != -1 && bend != 1) {
        return Err(KernelError::ConstraintFields);
    }
    let contacts = normalize_contacts(contacts, width, height, frame_count)?;
    let chain = normalize_chain(parts, &part_name, chain)?;
    match kind {
        ConstraintKind::Follow => {
            if x.is_some() || bend.is_some() || !chain.is_empty() || !contacts.is_empty() {
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
            if x.is_some()
                || anchor.is_none()
                || bend.is_some()
                || !chain.is_empty()
                || !contacts.is_empty()
            {
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
            if x.is_none()
                || target.is_some()
                || target_anchor.is_some()
                || bend.is_some()
                || !chain.is_empty()
                || !contacts.is_empty()
            {
                return Err(KernelError::ConstraintFields);
            }
        }
        ConstraintKind::Plant => {
            if x.is_some()
                || target.is_some()
                || target_anchor.is_some()
                || bend.is_some()
                || !chain.is_empty()
                || contacts.is_empty()
            {
                return Err(KernelError::ConstraintFields);
            }
        }
        ConstraintKind::Ik => {
            if chain.is_empty() {
                return Err(KernelError::ConstraintFields);
            }
            if let Some(anchor_name) = &anchor {
                require_anchor(part_named(parts, &part_name)?, anchor_name)?;
            }
            let has_cell = x.is_some();
            let has_target = target.is_some() || target_anchor.is_some();
            if has_cell && has_target {
                return Err(KernelError::ConstraintFields);
            }
            if has_target && (target.is_none() || target_anchor.is_none()) {
                return Err(KernelError::ConstraintFields);
            }
            if !has_cell && !has_target && contacts.is_empty() {
                return Err(KernelError::ConstraintFields);
            }
            if let Some(target_name) = target.as_deref() {
                let target_anchor = target_anchor
                    .as_deref()
                    .ok_or(KernelError::ConstraintFields)?;
                let target_part = part_named(parts, target_name)?;
                require_anchor(target_part, target_anchor)?;
                if chain.iter().any(|joint| joint == target_name) {
                    return Err(KernelError::ConstraintCycle { name: name.clone() });
                }
            }
            ik_bones(parts, &chain, anchor.as_deref())?;
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
        chain,
        bend,
        contacts,
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
        for part in driven_parts(constraint) {
            if constraints
                .iter()
                .take(index)
                .any(|earlier| driven_parts(earlier).iter().any(|driven| *driven == part))
            {
                return Err(KernelError::ConstraintConflict {
                    part: part.to_string(),
                });
            }
        }
    }
    let mut incoming = vec![0usize; constraints.len()];
    let mut outgoing = vec![Vec::new(); constraints.len()];
    for (index, constraint) in constraints.iter().enumerate() {
        let Some(target) = constraint.target.as_deref() else {
            continue;
        };
        if driven_parts(constraint).iter().any(|part| *part == target) {
            return Err(KernelError::ConstraintCycle {
                name: constraint.name.clone(),
            });
        }
        if let Some(source) = constraints
            .iter()
            .position(|item| driven_parts(item).iter().any(|part| *part == target))
        {
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

/// Pose driven parts. Targets are already solved when a follower reads them.
///
/// `frame` is the curve sample. When it is omitted, footprint spans are read
/// at `pixel_frame`. An `ik` or `plant` with no span on that frame, and no
/// static target, leaves the curve pose in place.
pub(crate) fn apply_deltas(
    posed: &mut [PosedPart],
    stored: &[Part],
    constraints: &[Constraint],
    order: &[usize],
    width: u16,
    height: u16,
    frame: Option<f64>,
    pixel_frame: usize,
) -> Result<Vec<SolvedConstraint>, KernelError> {
    let sample = frame.unwrap_or(pixel_frame as f64);
    let mut solved = Vec::with_capacity(order.len());
    for &index in order {
        let constraint = &constraints[index];
        let solved_one = match constraint.kind {
            ConstraintKind::Follow | ConstraintKind::Attach | ConstraintKind::Pin => {
                apply_translation(posed, constraint, index, width, height)?
            }
            ConstraintKind::Plant => apply_plant(posed, constraint, index, sample, width, height)?,
            ConstraintKind::Ik => {
                apply_ik(posed, stored, constraint, index, sample, width, height)?
            }
        };
        solved.push(solved_one);
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
            for joint in &mut next.chain {
                if joint == from {
                    *joint = to.to_string();
                }
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
            driven_parts(constraint)
                .iter()
                .all(|name| parts.iter().any(|part| part.name == *name))
                && constraint
                    .target
                    .as_ref()
                    .is_none_or(|target| parts.iter().any(|part| part.name == *target))
        })
        .cloned()
        .collect()
}

/// Keys at or after `index` move right by one, the same rule as curve keys.
pub(crate) fn constraints_after_insert(
    constraints: &[Constraint],
    index: usize,
) -> Vec<Constraint> {
    constraints
        .iter()
        .map(|constraint| {
            let mut next = constraint.clone();
            for contact in &mut next.contacts {
                if usize::from(contact.frame) >= index {
                    contact.frame += 1;
                }
                if usize::from(contact.to) >= index {
                    contact.to += 1;
                }
            }
            next
        })
        .collect()
}

/// The footprint on `index` shrinks. A plant or contact-only IK with no spans left is dropped.
pub(crate) fn constraints_after_remove(
    constraints: &[Constraint],
    index: usize,
) -> Vec<Constraint> {
    let mut out = Vec::with_capacity(constraints.len());
    for constraint in constraints {
        let mut next = constraint.clone();
        next.contacts = contacts_after_remove(&constraint.contacts, index);
        if next.kind == ConstraintKind::Plant && next.contacts.is_empty() {
            continue;
        }
        if next.kind == ConstraintKind::Ik
            && !constraint.contacts.is_empty()
            && next.contacts.is_empty()
            && next.x.is_none()
            && next.target.is_none()
        {
            continue;
        }
        out.push(next);
    }
    out
}

fn contacts_after_remove(contacts: &[Contact], index: usize) -> Vec<Contact> {
    let mut out = Vec::with_capacity(contacts.len());
    for contact in contacts {
        let frame = usize::from(contact.frame);
        let to = usize::from(contact.to);
        if to < index {
            out.push(contact.clone());
            continue;
        }
        if frame > index {
            out.push(Contact {
                frame: contact.frame - 1,
                to: contact.to - 1,
                ..contact.clone()
            });
            continue;
        }
        if frame == index && to == index {
            continue;
        }
        let frame = u16::try_from(frame).expect("frame index fits");
        let to = contact.to - 1;
        if frame <= to {
            out.push(Contact {
                frame,
                to,
                x: contact.x,
                y: contact.y,
            });
        }
    }
    out
}

fn shift_cell(
    x: u16,
    y: u16,
    offset_x: i32,
    offset_y: i32,
    width: u16,
    height: u16,
) -> Option<(u16, u16)> {
    let x = i32::from(x).checked_add(offset_x)?;
    let y = i32::from(y).checked_add(offset_y)?;
    let x = u16::try_from(x).ok()?;
    let y = u16::try_from(y).ok()?;
    if x >= width || y >= height {
        return None;
    }
    Some((x, y))
}

/// Moves pin cells and footprint spans with the canvas.
///
/// A constraint is dropped when it loses its part, its target, or a stored cell.
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
            if matches!(constraint.kind, ConstraintKind::Pin | ConstraintKind::Ik) {
                if let (Some(x), Some(y)) = (constraint.x, constraint.y) {
                    let shifted = shift_cell(x, y, offset_x, offset_y, width, height)?;
                    constraint.x = Some(shifted.0);
                    constraint.y = Some(shifted.1);
                }
            }
            let mut contacts = Vec::with_capacity(constraint.contacts.len());
            for contact in constraint.contacts {
                let (x, y) = shift_cell(contact.x, contact.y, offset_x, offset_y, width, height)?;
                contacts.push(Contact { x, y, ..contact });
            }
            if constraint.kind == ConstraintKind::Plant && contacts.is_empty() {
                return None;
            }
            constraint.contacts = contacts;
            Some(constraint)
        })
        .collect()
}

fn apply_translation(
    posed: &mut [PosedPart],
    constraint: &Constraint,
    index: usize,
    width: u16,
    height: u16,
) -> Result<SolvedConstraint, KernelError> {
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
        ConstraintKind::Ik | ConstraintKind::Plant => {
            return Err(KernelError::ConstraintFields);
        }
    };
    translate_follower(posed, follower, from_x, from_y, to_x, to_y, width, height)?;
    Ok(solved_record(
        constraint,
        index,
        to_x - from_x,
        to_y - from_y,
        from_x == to_x && from_y == to_y,
        from_x,
        from_y,
        to_x,
        to_y,
        false,
    ))
}

fn apply_plant(
    posed: &mut [PosedPart],
    constraint: &Constraint,
    index: usize,
    frame: f64,
    width: u16,
    height: u16,
) -> Result<SolvedConstraint, KernelError> {
    let follower = part_index(posed, &constraint.part)?;
    let (local_x, local_y) = local_of(&posed[follower].oriented, constraint.anchor.as_deref())?;
    let (from_x, from_y) = canvas_local(&posed[follower], local_x, local_y)?;
    let Some(contact) = contact_at(&constraint.contacts, frame) else {
        return Ok(solved_record(
            constraint, index, 0, 0, true, from_x, from_y, from_x, from_y, false,
        ));
    };
    let (to_x, to_y) = (i32::from(contact.x), i32::from(contact.y));
    translate_follower(posed, follower, from_x, from_y, to_x, to_y, width, height)?;
    Ok(solved_record(
        constraint,
        index,
        to_x - from_x,
        to_y - from_y,
        from_x == to_x && from_y == to_y,
        from_x,
        from_y,
        to_x,
        to_y,
        true,
    ))
}

fn apply_ik(
    posed: &mut [PosedPart],
    stored: &[Part],
    constraint: &Constraint,
    index: usize,
    frame: f64,
    width: u16,
    height: u16,
) -> Result<SolvedConstraint, KernelError> {
    let end = part_index(posed, &constraint.part)?;
    let (local_x, local_y) = local_of(&posed[end].oriented, constraint.anchor.as_deref())?;
    let (from_x, from_y) = canvas_local(&posed[end], local_x, local_y)?;
    let planted = contact_at(&constraint.contacts, frame).is_some();
    let Some((to_x, to_y)) = ik_target(posed, constraint, frame)? else {
        return Ok(solved_record(
            constraint, index, 0, 0, true, from_x, from_y, from_x, from_y, false,
        ));
    };
    if !cell_on_canvas(to_x, to_y, width, height) {
        return Err(KernelError::PartOffCanvas { x: to_x, y: to_y });
    }
    let bones = ik_bones(stored, &constraint.chain, constraint.anchor.as_deref())?;
    let root_index = part_index(posed, &constraint.chain[0])?;
    let root = posed_origin(&posed[root_index])?;
    let bend = constraint.bend.unwrap_or_else(|| rest_bend(&bones));
    let Some(solved) = search_chain(&bones, root, (to_x, to_y), bend, width, height) else {
        return Err(KernelError::IkUnreachable {
            name: constraint.name.clone(),
            x: to_x,
            y: to_y,
        });
    };
    for (chain_index, name) in constraint.chain.iter().enumerate() {
        let posed_index = part_index(posed, name)?;
        let stored_part = stored
            .iter()
            .find(|part| part.name == *name)
            .ok_or(KernelError::PartMissing)?;
        let keep_translation = chain_index == 0;
        let curve_dx = if keep_translation {
            posed[posed_index].curve_dx
        } else {
            0
        };
        let curve_dy = if keep_translation {
            posed[posed_index].curve_dy
        } else {
            0
        };
        posed[posed_index] = PosedPart {
            oriented: stored_part.clone(),
            steps: Vec::new(),
            curve_dx,
            curve_dy,
            constraint_dx: 0,
            constraint_dy: 0,
        };
        let centi = solved.centis.get(chain_index).copied().unwrap_or(0);
        if centi != 0 {
            apply_rotation(
                &mut posed[posed_index].oriented,
                f64::from(centi) / 100.0,
                width,
                height,
                &mut posed[posed_index].steps,
            )?;
        }
    }
    for (chain_index, name) in constraint.chain.iter().enumerate().skip(1) {
        let posed_index = part_index(posed, name)?;
        let stored_part = stored
            .iter()
            .find(|part| part.name == *name)
            .ok_or(KernelError::PartMissing)?;
        let pivot = solved
            .pivots
            .get(chain_index)
            .copied()
            .unwrap_or((to_x, to_y));
        posed[posed_index].constraint_dx = pivot.0 - i32::from(stored_part.x);
        posed[posed_index].constraint_dy = pivot.1 - i32::from(stored_part.y);
        place_origin(&posed[posed_index], width, height)?;
    }
    place_origin(&posed[root_index], width, height)?;
    let end = part_index(posed, &constraint.part)?;
    let (hit_local_x, hit_local_y) = local_of(&posed[end].oriented, constraint.anchor.as_deref())?;
    let (hit_x, hit_y) = canvas_local(&posed[end], hit_local_x, hit_local_y)?;
    if hit_x != to_x || hit_y != to_y {
        return Err(KernelError::IkUnreachable {
            name: constraint.name.clone(),
            x: to_x,
            y: to_y,
        });
    }
    let dx = to_x - from_x;
    let dy = to_y - from_y;
    Ok(solved_record(
        constraint,
        index,
        dx,
        dy,
        dx == 0 && dy == 0,
        from_x,
        from_y,
        to_x,
        to_y,
        planted,
    ))
}

fn translate_follower(
    posed: &mut [PosedPart],
    follower: usize,
    from_x: i32,
    from_y: i32,
    to_x: i32,
    to_y: i32,
    width: u16,
    height: u16,
) -> Result<(), KernelError> {
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
    Ok(())
}

fn solved_record(
    constraint: &Constraint,
    index: usize,
    dx: i32,
    dy: i32,
    satisfied: bool,
    from_x: i32,
    from_y: i32,
    to_x: i32,
    to_y: i32,
    planted: bool,
) -> SolvedConstraint {
    SolvedConstraint {
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
        satisfied,
        from_x,
        from_y,
        to_x,
        to_y,
        planted,
    }
}

struct ChainSolve {
    centis: Vec<i32>,
    pivots: Vec<(i32, i32)>,
}

fn search_chain(
    bones: &[(i32, i32)],
    root: (i32, i32),
    target: (i32, i32),
    bend: i8,
    width: u16,
    height: u16,
) -> Option<ChainSolve> {
    let maps: Vec<BTreeMap<(i32, i32), i32>> =
        bones.iter().copied().map(rotation_targets).collect();
    let mut best: Option<ChainSolve> = None;
    let mut centis = Vec::with_capacity(bones.len());
    let mut pivots = Vec::with_capacity(bones.len() + 1);
    pivots.push(root);
    search_bone(
        &maps,
        bones,
        root,
        target,
        bend,
        width,
        height,
        0,
        &mut centis,
        &mut pivots,
        &mut best,
    );
    best
}

fn search_bone(
    maps: &[BTreeMap<(i32, i32), i32>],
    bones: &[(i32, i32)],
    root: (i32, i32),
    target: (i32, i32),
    bend: i8,
    width: u16,
    height: u16,
    depth: usize,
    centis: &mut Vec<i32>,
    pivots: &mut Vec<(i32, i32)>,
    best: &mut Option<ChainSolve>,
) {
    let pivot = *pivots.last().expect("the root pivot is pushed");
    if depth + 1 == bones.len() {
        let rel = (target.0 - pivot.0, target.1 - pivot.1);
        let Some(&centi) = maps[depth].get(&rel) else {
            return;
        };
        centis.push(centi);
        if best_so_far(centis, best) {
            *best = Some(ChainSolve {
                centis: centis.clone(),
                pivots: pivots.clone(),
            });
        }
        centis.pop();
        return;
    }
    let rel_target = (target.0 - root.0, target.1 - root.1);
    for (&offset, &centi) in &maps[depth] {
        if depth == 0 && bend != 0 {
            let side = cross(rel_target.0, rel_target.1, offset.0, offset.1);
            if side != 0 && side.signum() as i8 != bend {
                continue;
            }
        }
        let Some(next_x) = pivot.0.checked_add(offset.0) else {
            continue;
        };
        let Some(next_y) = pivot.1.checked_add(offset.1) else {
            continue;
        };
        if !cell_on_canvas(next_x, next_y, width, height) {
            continue;
        }
        centis.push(centi);
        pivots.push((next_x, next_y));
        search_bone(
            maps,
            bones,
            root,
            target,
            bend,
            width,
            height,
            depth + 1,
            centis,
            pivots,
            best,
        );
        pivots.pop();
        centis.pop();
    }
}

fn best_so_far(centis: &[i32], best: &Option<ChainSolve>) -> bool {
    let Some(best) = best else {
        return true;
    };
    let score = |values: &[i32]| values.iter().map(|value| value.abs()).sum::<i32>();
    let next = score(centis);
    let prev = score(&best.centis);
    next < prev || (next == prev && centis < best.centis.as_slice())
}

fn rest_bend(bones: &[(i32, i32)]) -> i8 {
    let mut end = (0, 0);
    for bone in bones {
        end.0 += bone.0;
        end.1 += bone.1;
    }
    let side = cross(end.0, end.1, bones[0].0, bones[0].1);
    if side > 0 {
        1
    } else if side < 0 {
        -1
    } else {
        0
    }
}

fn cross(ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
    ax.saturating_mul(by).saturating_sub(ay.saturating_mul(bx))
}

fn rotation_targets(offset: (i32, i32)) -> BTreeMap<(i32, i32), i32> {
    let mut best = BTreeMap::new();
    if offset == (0, 0) {
        best.insert((0, 0), 0);
        return best;
    }
    for centi in -17_999i32..=18_000 {
        let degrees = f64::from(centi) / 100.0;
        let Ok(resolved) = part_xform::resolve_motion(Motion::Rotate(degrees)) else {
            continue;
        };
        let Ok((x, y)) = map_point(&resolved.map, offset.0, offset.1, 0, 0) else {
            continue;
        };
        best.entry((x, y))
            .and_modify(|prev| {
                let better =
                    centi.abs() < prev.abs() || (centi.abs() == prev.abs() && centi > *prev);
                if better {
                    *prev = centi;
                }
            })
            .or_insert(centi);
    }
    best
}

fn ik_target(
    posed: &[PosedPart],
    constraint: &Constraint,
    frame: f64,
) -> Result<Option<(i32, i32)>, KernelError> {
    if let Some(contact) = contact_at(&constraint.contacts, frame) {
        return Ok(Some((i32::from(contact.x), i32::from(contact.y))));
    }
    if let (Some(x), Some(y)) = (constraint.x, constraint.y) {
        return Ok(Some((i32::from(x), i32::from(y))));
    }
    if let Some(target_name) = constraint.target.as_deref() {
        let target_anchor = constraint
            .target_anchor
            .as_deref()
            .ok_or(KernelError::ConstraintFields)?;
        let target = part_index(posed, target_name)?;
        let (local_x, local_y) = local_of(&posed[target].oriented, Some(target_anchor))?;
        return Ok(Some(canvas_local(&posed[target], local_x, local_y)?));
    }
    Ok(None)
}

pub(crate) fn ik_bones(
    parts: &[Part],
    chain: &[String],
    anchor: Option<&str>,
) -> Result<Vec<(i32, i32)>, KernelError> {
    if chain.len() < 2 || chain.len() > 3 {
        return Err(KernelError::ConstraintFields);
    }
    let mut bones = Vec::with_capacity(chain.len());
    for pair in chain.windows(2) {
        let parent = part_named(parts, &pair[0])?;
        let child = part_named(parts, &pair[1])?;
        let dx = i32::from(child.x) - i32::from(parent.x);
        let dy = i32::from(child.y) - i32::from(parent.y);
        if dx == 0 && dy == 0 {
            return Err(KernelError::ConstraintFields);
        }
        bones.push((dx, dy));
    }
    if let Some(anchor) = anchor {
        let end = part_named(parts, chain.last().expect("chain length was checked"))?;
        let (dx, dy) = local_of(end, Some(anchor))?;
        if dx != 0 || dy != 0 {
            bones.push((dx, dy));
        }
    }
    if bones.is_empty() || bones.len() > 3 {
        return Err(KernelError::ConstraintFields);
    }
    Ok(bones)
}

fn contact_at(contacts: &[Contact], frame: f64) -> Option<&Contact> {
    if !frame.is_finite() {
        return None;
    }
    contacts
        .iter()
        .find(|contact| frame >= f64::from(contact.frame) && frame <= f64::from(contact.to))
}

fn normalize_contacts(
    contacts: &[Contact],
    width: u16,
    height: u16,
    frame_count: usize,
) -> Result<Vec<Contact>, KernelError> {
    let mut stored = contacts.to_vec();
    stored.sort_by_key(|contact| (contact.frame, contact.to, contact.x, contact.y));
    for (index, contact) in stored.iter().enumerate() {
        if contact.frame > contact.to || usize::from(contact.to) >= frame_count {
            return Err(KernelError::ConstraintFields);
        }
        if contact.x >= width || contact.y >= height {
            return Err(KernelError::PartOffCanvas {
                x: i32::from(contact.x),
                y: i32::from(contact.y),
            });
        }
        if stored
            .iter()
            .take(index)
            .any(|earlier| earlier.frame <= contact.to && contact.frame <= earlier.to)
        {
            return Err(KernelError::ConstraintFields);
        }
    }
    Ok(stored)
}

fn normalize_chain(
    parts: &[Part],
    end: &str,
    chain: &[String],
) -> Result<Vec<String>, KernelError> {
    if chain.is_empty() {
        return Ok(Vec::new());
    }
    if chain.len() < 2 || chain.len() > 3 {
        return Err(KernelError::ConstraintFields);
    }
    let mut names = Vec::with_capacity(chain.len());
    for name in chain {
        let name = normalize_part_name(name).map_err(|()| KernelError::PartName)?;
        if names.iter().any(|existing| existing == &name)
            || !parts.iter().any(|part| part.name == name)
        {
            return Err(KernelError::ConstraintFields);
        }
        names.push(name);
    }
    if names.last().map(String::as_str) != Some(end) {
        return Err(KernelError::ConstraintFields);
    }
    Ok(names)
}

fn driven_parts(constraint: &Constraint) -> Vec<&str> {
    match constraint.kind {
        ConstraintKind::Ik => constraint.chain.iter().map(String::as_str).collect(),
        _ => vec![constraint.part.as_str()],
    }
}

fn posed_origin(posed: &PosedPart) -> Result<(i32, i32), KernelError> {
    checked_origin(posed)
}

fn cell_on_canvas(x: i32, y: i32, width: u16, height: u16) -> bool {
    matches!(
        (u16::try_from(x), u16::try_from(y)),
        (Ok(x), Ok(y)) if x < width && y < height
    )
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
            chain: Vec::new(),
            bend: None,
            contacts: Vec::new(),
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
