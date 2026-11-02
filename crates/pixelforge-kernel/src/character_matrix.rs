//! Character animation facing × action matrix.
//!
//! Reports which `(facing, action)` cells are present, missing, empty, or a
//! copy/mirror placeholder. Facings come from the project's `directionLayout`.
//! Expected actions come from owned clip asset `tags` (and any
//! `{action}_{facing}` already on the project). This reuses
//! [`crate::direction`] tag parse and the same idle/walk sheet rules; it is
//! not a second animation language and not a gait judge.
//!
//! The caller opens the pixel project and passes a probe. `validateAssets`
//! still does not open pixels. A green matrix is not a natural walk and not
//! an aesthetic pass.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::direction::{
    parse_direction_tag, report as direction_report, DirectionLayout, DirectionReport, Facing,
};
use crate::manifest::{ArtManifest, AssetRecord};
use crate::tag::{LoopMode, Tag};

/// One animation tag the caller read from the character project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterTagProbe {
    pub name: String,
    pub from: u32,
    pub to: u32,
    pub loop_mode: LoopMode,
}

/// Per-frame ink probe. Hashes are FNV-1a 64 over layer indices and erase marks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacterFrameProbe {
    pub has_content: bool,
    pub hash: u64,
    pub hash_hflip: u64,
}

/// Pixel-project snapshot for [`character_matrix`]. The manifest tools do not
/// read the disk; the CLI (or another caller) fills this.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterProjectProbe {
    pub frame_count: usize,
    pub direction_layout: Option<DirectionLayout>,
    pub tags: Vec<CharacterTagProbe>,
    pub frames: Vec<CharacterFrameProbe>,
}

/// One matrix cell. `status` is `present`, `missing`, `empty`, or `placeholder`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterMatrixCell {
    pub action: String,
    pub facing: &'static str,
    pub compass: &'static str,
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<u32>,
    /// Other facing this cell matches when `status` is `placeholder`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_facing: Option<&'static str>,
    /// `copy` or `mirror`. Only set with `matchFacing`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_mode: Option<&'static str>,
}

/// A hard gap an agent should fill next (`missing` or `empty`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterMatrixGap {
    pub action: String,
    pub facing: &'static str,
    pub compass: &'static str,
    pub status: &'static str,
}

/// Registered gear owned by the character. Omitted from the report when none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterGearStatus {
    pub id: String,
    pub path: String,
    /// False when the caller marked the path missing.
    pub path_ok: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterFinding {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facing: Option<&'static str>,
}

/// Facing × action checklist for one character (or its owner).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterMatrixReport {
    pub id: String,
    pub path: String,
    pub count: Option<u8>,
    pub columns: Option<u32>,
    pub directions: Vec<&'static str>,
    pub actions: Vec<String>,
    pub cells: Vec<CharacterMatrixCell>,
    pub gaps: Vec<CharacterMatrixGap>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub gear: Vec<CharacterGearStatus>,
    pub error_count: usize,
    pub warning_count: usize,
    pub errors: Vec<CharacterFinding>,
    pub warnings: Vec<CharacterFinding>,
    /// Same checklist as `direction validate`. Matrix gaps do not replace it.
    pub direction: DirectionReport,
}

impl CharacterMatrixReport {
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Why [`character_matrix`] could not build a report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterMatrixError {
    message: String,
}

impl CharacterMatrixError {
    pub fn code(&self) -> &'static str {
        "InvalidParams"
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

fn matrix_err(message: impl Into<String>) -> CharacterMatrixError {
    CharacterMatrixError {
        message: message.into(),
    }
}

/// Resolve `id` to a character (or group used as owner), then build the matrix.
///
/// `missing_paths` lists asset-root-relative paths the caller found missing.
/// Only gear records owned by the character are listed in `gear`.
pub fn character_matrix(
    manifest: &ArtManifest,
    id: &str,
    project: &CharacterProjectProbe,
    missing_paths: &[String],
) -> Result<CharacterMatrixReport, CharacterMatrixError> {
    let (owner, path) = resolve_owner(manifest, id)?;
    if project.frames.len() != project.frame_count {
        return Err(matrix_err(format!(
            "characterMatrix: frames probe has {} entries; frameCount is {}",
            project.frames.len(),
            project.frame_count
        )));
    }

    let tags: Vec<Tag> = project
        .tags
        .iter()
        .map(|tag| Tag {
            name: tag.name.clone(),
            from: tag.from as usize,
            to: tag.to as usize,
            loop_mode: tag.loop_mode,
        })
        .collect();
    let direction = direction_report(project.frame_count, &tags, project.direction_layout);

    let layout = project.direction_layout;
    let actions = expected_actions(manifest, &owner, &tags, layout.is_some());
    let directions: Vec<&'static str> = layout
        .map(|layout| layout.facings().iter().map(|facing| facing.screen).collect())
        .unwrap_or_default();

    let mut cells = Vec::new();
    let mut gaps = Vec::new();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    for finding in &direction.errors {
        errors.push(CharacterFinding {
            code: finding.code,
            message: finding.message.clone(),
            action: finding.action.clone(),
            facing: finding.facing,
        });
    }

    if let Some(layout) = layout {
        let parsed = parse_project_tags(&tags);
        for action in &actions {
            for facing in layout.facings() {
                let cell = classify_cell(action, facing, layout, &parsed, project);
                if cell.status == "missing" || cell.status == "empty" {
                    gaps.push(CharacterMatrixGap {
                        action: cell.action.clone(),
                        facing: cell.facing,
                        compass: cell.compass,
                        status: cell.status,
                    });
                    // Direction report already covers missingDirection / segment
                    // for tags that participate in its rules. Still emit a
                    // matrix-level finding so gaps stay actionable when the
                    // action is expected from asset tags but not yet on the
                    // project (direction validate only requires other actions
                    // once any facing has them).
                    if cell.status == "missing"
                        && !direction_already_reports(&direction, action, facing.screen)
                    {
                        errors.push(CharacterFinding {
                            code: "missing",
                            message: format!(
                                "facing {} ({}) has no {action}_{} tag",
                                facing.screen, facing.compass, facing.screen
                            ),
                            action: Some(action.clone()),
                            facing: Some(facing.screen),
                        });
                    }
                    if cell.status == "empty" {
                        errors.push(CharacterFinding {
                            code: "empty",
                            message: format!(
                                "tag `{}` covers frames {}-{} with no ink",
                                cell.tag.as_deref().unwrap_or("?"),
                                cell.from.unwrap_or(0),
                                cell.to.unwrap_or(0)
                            ),
                            action: Some(action.clone()),
                            facing: Some(facing.screen),
                        });
                    }
                } else if cell.status == "placeholder" {
                    warnings.push(CharacterFinding {
                        code: "placeholder",
                        message: format!(
                            "tag `{}` matches {} ({}) via {}; not new art",
                            cell.tag.as_deref().unwrap_or("?"),
                            cell.match_facing.unwrap_or("?"),
                            cell.match_mode.unwrap_or("copy"),
                            cell.match_mode.unwrap_or("copy")
                        ),
                        action: Some(action.clone()),
                        facing: Some(facing.screen),
                    });
                }
                cells.push(cell);
            }
        }
    } else if !actions.is_empty() {
        // No layout: still list expected actions so an agent sees what was asked.
        for action in &actions {
            errors.push(CharacterFinding {
                code: "layout",
                message: format!(
                    "action `{action}` is expected, but direction layout is not set"
                ),
                action: Some(action.clone()),
                facing: None,
            });
        }
    }

    let gear = gear_status(manifest, &owner, missing_paths);
    for item in &gear {
        if !item.path_ok {
            errors.push(CharacterFinding {
                code: "gearMissing",
                message: format!("gear `{}` path `{}` is missing", item.id, item.path),
                action: None,
                facing: None,
            });
        }
    }

    Ok(CharacterMatrixReport {
        id: owner,
        path,
        count: direction.count,
        columns: direction.columns,
        directions,
        actions,
        cells,
        gaps,
        gear,
        error_count: errors.len(),
        warning_count: warnings.len(),
        errors,
        warnings,
        direction,
    })
}

fn resolve_owner(
    manifest: &ArtManifest,
    id: &str,
) -> Result<(String, String), CharacterMatrixError> {
    let asset = manifest
        .assets()
        .iter()
        .find(|asset| asset.id == id)
        .ok_or_else(|| matrix_err(format!("characterMatrix: unknown asset id `{id}`")))?;
    if asset.kind == "character" || asset.kind == "group" {
        return Ok((asset.id.clone(), asset.path.clone()));
    }
    let owner_id = asset.owner.as_deref().ok_or_else(|| {
        matrix_err(format!(
            "characterMatrix: asset `{id}` is not a character or group and has no owner"
        ))
    })?;
    let owner = manifest
        .assets()
        .iter()
        .find(|asset| asset.id == owner_id)
        .ok_or_else(|| {
            matrix_err(format!(
                "characterMatrix: owner `{owner_id}` for `{id}` is not registered"
            ))
        })?;
    if owner.kind != "character" && owner.kind != "group" {
        return Err(matrix_err(format!(
            "characterMatrix: owner `{owner_id}` must be a character or group"
        )));
    }
    Ok((owner.id.clone(), owner.path.clone()))
}

fn expected_actions(
    manifest: &ArtManifest,
    owner_id: &str,
    project_tags: &[Tag],
    has_layout: bool,
) -> Vec<String> {
    let mut actions = BTreeSet::new();
    for asset in manifest.assets() {
        if asset.id == owner_id || asset.owner.as_deref() == Some(owner_id) {
            if asset.kind == "clip" || asset.id == owner_id {
                for tag in &asset.tags {
                    if let Some((action, _)) = parse_direction_tag(tag) {
                        actions.insert(action.as_str().to_string());
                    }
                }
            }
        }
    }
    for tag in project_tags {
        if let Some((action, _)) = parse_direction_tag(&tag.name) {
            actions.insert(action.as_str().to_string());
        }
    }
    // Sheet convention: a direction layout always implies idle/walk coverage.
    if has_layout {
        actions.insert("idle".to_string());
        actions.insert("walk".to_string());
    }
    actions.into_iter().collect()
}

struct ParsedTag {
    action: String,
    facing: &'static Facing,
    name: String,
    from: u32,
    to: u32,
}

fn parse_project_tags(tags: &[Tag]) -> Vec<ParsedTag> {
    let mut out = Vec::new();
    for tag in tags {
        let Some((action, facing)) = parse_direction_tag(&tag.name) else {
            continue;
        };
        out.push(ParsedTag {
            action: action.as_str().to_string(),
            facing,
            name: tag.name.clone(),
            from: tag.from as u32,
            to: tag.to as u32,
        });
    }
    out
}

fn classify_cell(
    action: &str,
    facing: &'static Facing,
    layout: DirectionLayout,
    parsed: &[ParsedTag],
    project: &CharacterProjectProbe,
) -> CharacterMatrixCell {
    let hits: Vec<&ParsedTag> = parsed
        .iter()
        .filter(|tag| tag.facing.index == facing.index && tag.action == action)
        .collect();
    if hits.is_empty() {
        return CharacterMatrixCell {
            action: action.to_string(),
            facing: facing.screen,
            compass: facing.compass,
            status: "missing",
            tag: None,
            from: None,
            to: None,
            match_facing: None,
            match_mode: None,
        };
    }
    let tag = hits[0];
    let from = tag.from as usize;
    let to = tag.to as usize;
    let span_ok = from <= to && to < project.frame_count;
    let has_ink = span_ok
        && (from..=to).any(|frame| project.frames.get(frame).is_some_and(|probe| probe.has_content));
    if !has_ink {
        return CharacterMatrixCell {
            action: action.to_string(),
            facing: facing.screen,
            compass: facing.compass,
            status: "empty",
            tag: Some(tag.name.clone()),
            from: Some(tag.from),
            to: Some(tag.to),
            match_facing: None,
            match_mode: None,
        };
    }

    if let Some((match_facing, match_mode)) =
        placeholder_match(action, facing, tag, layout, parsed, project)
    {
        return CharacterMatrixCell {
            action: action.to_string(),
            facing: facing.screen,
            compass: facing.compass,
            status: "placeholder",
            tag: Some(tag.name.clone()),
            from: Some(tag.from),
            to: Some(tag.to),
            match_facing: Some(match_facing),
            match_mode: Some(match_mode),
        };
    }

    CharacterMatrixCell {
        action: action.to_string(),
        facing: facing.screen,
        compass: facing.compass,
        status: "present",
        tag: Some(tag.name.clone()),
        from: Some(tag.from),
        to: Some(tag.to),
        match_facing: None,
        match_mode: None,
    }
}

fn placeholder_match(
    action: &str,
    facing: &'static Facing,
    tag: &ParsedTag,
    layout: DirectionLayout,
    parsed: &[ParsedTag],
    project: &CharacterProjectProbe,
) -> Option<(&'static str, &'static str)> {
    let len = tag.to.saturating_sub(tag.from) + 1;
    for other in layout.facings() {
        if other.index == facing.index {
            continue;
        }
        let Some(other_tag) = parsed
            .iter()
            .find(|item| item.facing.index == other.index && item.action == action)
        else {
            continue;
        };
        let other_len = other_tag.to.saturating_sub(other_tag.from) + 1;
        if other_len != len {
            continue;
        }
        let mut copy = true;
        let mut mirror = true;
        for offset in 0..len {
            let Some(left) = project.frames.get((tag.from + offset) as usize) else {
                copy = false;
                mirror = false;
                break;
            };
            let Some(right) = project.frames.get((other_tag.from + offset) as usize) else {
                copy = false;
                mirror = false;
                break;
            };
            if left.hash != right.hash {
                copy = false;
            }
            // Mirror: this facing's upright cells equal the other's hflip, or
            // the reverse. direction fill mirrors left↔right this way.
            if left.hash != right.hash_hflip && left.hash_hflip != right.hash {
                mirror = false;
            }
            if !copy && !mirror {
                break;
            }
        }
        if copy {
            return Some((other.screen, "copy"));
        }
        if mirror {
            return Some((other.screen, "mirror"));
        }
    }
    None
}

fn direction_already_reports(
    direction: &DirectionReport,
    action: &str,
    facing: &str,
) -> bool {
    direction.errors.iter().any(|finding| {
        finding.facing == Some(facing)
            && (finding.action.as_deref() == Some(action)
                || (matches!(action, "idle" | "walk")
                    && finding.action.is_none()
                    && finding.code == "missingDirection")
                || (matches!(action, "idle" | "walk")
                    && finding.action.as_deref() == Some(action)))
    })
}

fn gear_status(
    manifest: &ArtManifest,
    owner_id: &str,
    missing_paths: &[String],
) -> Vec<CharacterGearStatus> {
    manifest
        .assets()
        .iter()
        .filter(|asset| asset.kind == "gear" && asset.owner.as_deref() == Some(owner_id))
        .map(|asset| CharacterGearStatus {
            id: asset.id.clone(),
            path: asset.path.clone(),
            path_ok: !missing_paths.iter().any(|path| path == &asset.path),
        })
        .collect()
}

/// Build a [`CharacterProjectProbe`] helper for tests and callers that already
/// have [`Tag`] values.
pub fn tags_to_probes(tags: &[Tag]) -> Vec<CharacterTagProbe> {
    tags.iter()
        .map(|tag| CharacterTagProbe {
            name: tag.name.clone(),
            from: tag.from as u32,
            to: tag.to as u32,
            loop_mode: tag.loop_mode,
        })
        .collect()
}

/// Owned clip records for `owner_id`. Used by docs/tests; the matrix reads
/// them through [`expected_actions`].
pub fn owned_clips<'a>(manifest: &'a ArtManifest, owner_id: &str) -> Vec<&'a AssetRecord> {
    manifest
        .assets()
        .iter()
        .filter(|asset| asset.kind == "clip" && asset.owner.as_deref() == Some(owner_id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::direction::DirectionLayout;
    use crate::manifest::{ArtManifest, CanvasTier, LayoutRule, ManifestDraft};
    use crate::tag::LoopMode;

    fn draft_with(assets: Vec<crate::manifest::AssetRecord>) -> ArtManifest {
        ArtManifest::new(ManifestDraft {
            name: "test".to_string(),
            note: "matrix unit test".to_string(),
            perspective: "top".to_string(),
            asset_root: ".".to_string(),
            palette_path: "hero.json".to_string(),
            palette_ref: None,
            canvases: vec![CanvasTier {
                name: "sprite".to_string(),
                width: 16,
                height: 16,
            }],
            layout: vec![LayoutRule {
                name: "characters".to_string(),
                dir: "hero".to_string(),
                kind: "character".to_string(),
                canvas: Some("sprite".to_string()),
            }],
            assets,
        })
        .expect("draft")
    }

    fn ink(hash: u64) -> CharacterFrameProbe {
        CharacterFrameProbe {
            has_content: true,
            hash,
            hash_hflip: hash ^ 0x9e3779b97f4a7c15,
        }
    }

    fn blank() -> CharacterFrameProbe {
        CharacterFrameProbe {
            has_content: false,
            hash: 0,
            hash_hflip: 0,
        }
    }

    #[test]
    fn missing_action_from_clip_tags_is_a_gap() {
        let manifest = draft_with(vec![
            crate::manifest::AssetRecord {
                id: "hero".to_string(),
                kind: "character".to_string(),
                path: "hero.json".to_string(),
                owner: None,
                depends_on: vec![],
                produces: vec!["hero.walk".to_string()],
                tags: vec![],
            },
            crate::manifest::AssetRecord {
                id: "hero.walk".to_string(),
                kind: "clip".to_string(),
                path: "hero.json".to_string(),
                owner: Some("hero".to_string()),
                depends_on: vec!["hero".to_string()],
                produces: vec![],
                tags: vec![
                    "walk_down".to_string(),
                    "walk_left".to_string(),
                    "walk_right".to_string(),
                    "walk_up".to_string(),
                    "attack_down".to_string(),
                    "attack_left".to_string(),
                    "attack_right".to_string(),
                    "attack_up".to_string(),
                ],
            },
        ]);
        let layout = DirectionLayout::new(4, 1).unwrap();
        let mut frames = vec![ink(1); 4];
        let tags = vec![
            CharacterTagProbe {
                name: "idle_down".into(),
                from: 0,
                to: 0,
                loop_mode: LoopMode::Forward,
            },
            CharacterTagProbe {
                name: "idle_left".into(),
                from: 1,
                to: 1,
                loop_mode: LoopMode::Forward,
            },
            CharacterTagProbe {
                name: "idle_right".into(),
                from: 2,
                to: 2,
                loop_mode: LoopMode::Forward,
            },
            CharacterTagProbe {
                name: "idle_up".into(),
                from: 3,
                to: 3,
                loop_mode: LoopMode::Forward,
            },
        ];
        // one-column row: idle only, no walk. attack missing entirely.
        let _ = &mut frames;
        let report = character_matrix(
            &manifest,
            "hero",
            &CharacterProjectProbe {
                frame_count: 4,
                direction_layout: Some(layout),
                tags,
                frames,
            },
            &[],
        )
        .unwrap();
        assert!(report.actions.iter().any(|a| a == "attack"));
        assert!(report.gaps.iter().any(|g| g.action == "attack" && g.status == "missing"));
        assert!(report.error_count > 0);
    }

    #[test]
    fn empty_and_copy_placeholder_are_reported() {
        let manifest = draft_with(vec![crate::manifest::AssetRecord {
            id: "hero".to_string(),
            kind: "character".to_string(),
            path: "hero.json".to_string(),
            owner: None,
            depends_on: vec![],
            produces: vec![],
            tags: vec![],
        }]);
        let layout = DirectionLayout::new(4, 2).unwrap();
        // down has ink; left copies down; right empty tags; up missing walk handled by direction
        let mut frames = vec![blank(); 8];
        frames[0] = ink(11);
        frames[1] = ink(12);
        frames[2] = ink(11); // copy of down idle
        frames[3] = ink(12); // copy of down walk
        // right idle/walk tags point at empty frames 4-5
        let tags = vec![
            ("idle_down", 0, 0),
            ("walk_down", 1, 1),
            ("idle_left", 2, 2),
            ("walk_left", 3, 3),
            ("idle_right", 4, 4),
            ("walk_right", 5, 5),
            ("idle_up", 6, 6),
            ("walk_up", 7, 7),
        ]
        .into_iter()
        .map(|(name, from, to)| CharacterTagProbe {
            name: name.into(),
            from,
            to,
            loop_mode: LoopMode::Forward,
        })
        .collect();
        let report = character_matrix(
            &manifest,
            "hero",
            &CharacterProjectProbe {
                frame_count: 8,
                direction_layout: Some(layout),
                tags,
                frames,
            },
            &[],
        )
        .unwrap();
        let left_walk = report
            .cells
            .iter()
            .find(|c| c.action == "walk" && c.facing == "left")
            .unwrap();
        assert_eq!(left_walk.status, "placeholder");
        assert_eq!(left_walk.match_mode, Some("copy"));
        assert!(report.warnings.iter().any(|w| w.code == "placeholder"));
        assert!(report.gaps.iter().any(|g| g.facing == "right" && g.status == "empty"));
        assert!(report.errors.iter().any(|e| e.code == "empty"));
    }
}
