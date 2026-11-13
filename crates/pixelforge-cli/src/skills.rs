//! List and read agent skills bundled into this CLI binary.
//!
//! Skills are compiled in from repository-root `skills/` so a packaged install can
//! discover and read them without the git checkout or a private bot tree.

use crate::failure::CliFailure;
use crate::output::{emit, print_line};
use pixelforge_kernel::TOOL_API_VERSION;
use serde_json::json;
use std::io::{self, Write};

include!(concat!(env!("OUT_DIR"), "/bundled_skills.rs"));

pub fn list(spec: Option<&str>, _json_flag: bool) -> i32 {
    match resolve_list(spec) {
        Ok(value) => emit_value(&value),
        Err(err) => emit_error(err),
    }
}

pub fn read(spec: &str, path: Option<&str>, json: bool) -> i32 {
    match resolve_read(spec, path) {
        Ok(file) => {
            if json {
                let mut value = json!({
                    "ok": true,
                    "apiVersion": TOOL_API_VERSION,
                    "skill": file.skill,
                    "path": file.path,
                    "content": file.content,
                });
                if let Some(guidance) = skill_md_guidance(file.skill, file.path) {
                    value["guidance"] = json!(guidance);
                }
                emit_value(&value)
            } else {
                if let Some(guidance) = skill_md_guidance(file.skill, file.path) {
                    let _ = writeln!(io::stderr(), "{guidance}");
                }
                print!("{}", file.content);
                let _ = io::stdout().flush();
                0
            }
        }
        Err(err) => emit_error(err),
    }
}

struct SkillFile {
    skill: &'static str,
    path: &'static str,
    content: &'static str,
}

fn resolve_list(spec: Option<&str>) -> Result<serde_json::Value, CliFailure> {
    let Some(spec) = spec.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(list_all_payload());
    };
    let (skill_name, rel) = split_spec(spec, None)?;
    let skill = find_skill(skill_name)?;
    let rel = normalize_rel(rel)?;
    if !rel.is_empty() && skill_file(skill, rel).is_some() {
        return Err(fail(
            "invalid_argument",
            format!("`{spec}` is a file, not a directory."),
        )
        .with_hint(format!(
            "Use `pixelforge skills read {spec}` to print the file, or list its parent directory."
        )));
    }
    let entries = list_entries(skill, rel)?;
    Ok(json!({
        "ok": true,
        "apiVersion": TOOL_API_VERSION,
        "path": display_path(skill.name, rel),
        "entries": entries,
        "count": entries.len(),
    }))
}

fn resolve_read(spec: &str, path: Option<&str>) -> Result<SkillFile, CliFailure> {
    let (skill_name, rel) = split_spec(spec, path)?;
    let skill = find_skill(skill_name)?;
    let rel = normalize_rel(rel)?;
    let rel = if rel.is_empty() { "SKILL.md" } else { rel };
    if skill_dir_exists(skill, rel) {
        return Err(
            fail(
                "invalid_argument",
                format!("`{rel}` is a directory, not a file."),
            )
            .with_hint(format!(
                "Use `pixelforge skills list {}/{rel}` to see files, then `pixelforge skills read {}/<file>`.",
                skill.name, skill.name
            )),
        );
    }
    let file = skill_file(skill, rel).ok_or_else(|| {
        fail(
            "not_found",
            format!("Path `{rel}` was not found in skill `{}`.", skill.name),
        )
        .with_hint(format!(
            "Run `pixelforge skills list {}` to see the files bundled with this CLI.",
            skill.name
        ))
    })?;
    Ok(SkillFile {
        skill: skill.name,
        path: file.path,
        content: file.content,
    })
}

fn list_all_payload() -> serde_json::Value {
    let version = env!("CARGO_PKG_VERSION");
    let skills: Vec<serde_json::Value> = SKILLS
        .iter()
        .map(|skill| {
            json!({
                "name": skill.name,
                "description": skill.description,
                "version": version,
                "metadata": {
                    "cliHelp": "pixelforge --help",
                    "requires": { "bins": ["pixelforge"] },
                },
            })
        })
        .collect();
    json!({
        "ok": true,
        "apiVersion": TOOL_API_VERSION,
        "skills": skills,
        "count": skills.len(),
    })
}

fn list_entries(
    skill: &'static BundledSkill,
    rel: &str,
) -> Result<Vec<serde_json::Value>, CliFailure> {
    let prefix = if rel.is_empty() {
        String::new()
    } else {
        format!("{rel}/")
    };
    let mut names: Vec<(String, bool)> = Vec::new();
    for file in skill.files {
        let rest = if prefix.is_empty() {
            file.path
        } else if let Some(rest) = file.path.strip_prefix(&prefix) {
            rest
        } else {
            continue;
        };
        if rest.is_empty() {
            continue;
        }
        let (name, is_dir) = match rest.split_once('/') {
            Some((dir, _)) => (dir.to_string(), true),
            None => (rest.to_string(), false),
        };
        if !names.iter().any(|(existing, _)| existing == &name) {
            names.push((name, is_dir));
        }
    }
    if names.is_empty() && !rel.is_empty() {
        return Err(fail(
            "not_found",
            format!("Path `{rel}` was not found in skill `{}`.", skill.name),
        )
        .with_hint(format!(
            "Run `pixelforge skills list {}` to see the files bundled with this CLI.",
            skill.name
        )));
    }
    names.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(names
        .into_iter()
        .map(|(name, is_dir)| {
            json!({
                "path": if rel.is_empty() {
                    name
                } else {
                    format!("{rel}/{name}")
                },
                "is_dir": is_dir,
            })
        })
        .collect())
}

fn find_skill(name: &str) -> Result<&'static BundledSkill, CliFailure> {
    SKILLS
        .iter()
        .find(|skill| skill.name == name)
        .ok_or_else(|| {
            let names: Vec<&str> = SKILLS.iter().map(|s| s.name).collect();
            fail("not_found", format!("Unknown skill `{name}`.")).with_hint(format!(
                "Available skills: {}. Run `pixelforge skills list` to inspect them.",
                names.join(", ")
            ))
        })
}

fn skill_file(skill: &'static BundledSkill, rel: &str) -> Option<&'static BundledFile> {
    skill.files.iter().find(|file| file.path == rel)
}

fn skill_dir_exists(skill: &'static BundledSkill, rel: &str) -> bool {
    let prefix = format!("{rel}/");
    skill
        .files
        .iter()
        .any(|file| file.path.starts_with(&prefix))
}

fn split_spec<'a>(spec: &'a str, path: Option<&'a str>) -> Result<(&'a str, &'a str), CliFailure> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err(fail(
            "invalid_argument",
            "Missing skill name. Use `pixelforge skills read <name>[/<path>] [path]`.",
        ));
    }
    if looks_unsafe(spec) || path.is_some_and(looks_unsafe) {
        return Err(
            fail("invalid_argument", "Skill paths cannot contain `..` or be absolute.")
                .with_hint(
                    "Pass a skill name, optionally followed by a relative file such as `references/beginner.md`.",
                ),
        );
    }
    let (name, from_spec) = match spec.split_once('/') {
        Some((name, rest)) => (name, rest),
        None => (spec, ""),
    };
    if name.is_empty() {
        return Err(fail(
            "invalid_argument",
            "Missing skill name. Use `pixelforge skills read <name>[/<path>] [path]`.",
        ));
    }
    match (from_spec, path) {
        ("", Some(extra)) => Ok((name, extra.trim())),
        ("", None) => Ok((name, "")),
        (rel, None) => Ok((name, rel)),
        (rel, Some(extra)) => Err(fail(
            "invalid_argument",
            format!(
                "Got two paths (`{rel}` and `{}`). Pass only one of `<name>/<path>` or `<name> <path>`.",
                extra.trim()
            ),
        )),
    }
}

fn normalize_rel(rel: &str) -> Result<&str, CliFailure> {
    let rel = rel.trim().trim_matches('/');
    if rel.is_empty() {
        return Ok("");
    }
    if looks_unsafe(rel) {
        return Err(fail(
            "invalid_argument",
            "Skill paths cannot contain `..` or be absolute.",
        )
        .with_hint("Pass a relative file such as `SKILL.md` or `references/beginner.md`."));
    }
    Ok(rel)
}

fn looks_unsafe(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with('/')
        || trimmed.starts_with('\\')
        || trimmed
            .split(['/', '\\'])
            .any(|part| part == ".." || part == ".")
}

fn display_path(skill: &str, rel: &str) -> String {
    if rel.is_empty() {
        skill.to_string()
    } else {
        format!("{skill}/{rel}")
    }
}

fn skill_md_guidance(skill: &str, path: &str) -> Option<String> {
    if path != "SKILL.md" {
        return None;
    }
    Some(format!(
        "> Tip: read this skill's own files (e.g. `references/...`) with `pixelforge skills read {skill} <relative-path>` so they stay in sync with this CLI version. To read another skill, drop a leading `../`: `pixelforge skills read pixelforge-draw`."
    ))
}

fn fail(code: &str, message: impl Into<String>) -> CliFailure {
    CliFailure::new(code, message)
}

fn emit_value(value: &serde_json::Value) -> i32 {
    let line = serde_json::to_string(value).expect("skills payload is JSON");
    if print_line(&line) {
        0
    } else {
        2
    }
}

fn emit_error(err: CliFailure) -> i32 {
    emit(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_exactly_the_product_skills() {
        let mut names: Vec<&str> = SKILLS.iter().map(|s| s.name).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "pixel-art-craft",
                "pixelforge",
                "pixelforge-critique",
                "pixelforge-draw",
                "pixelforge-layers",
                "pixelforge-sprite-4dir",
            ]
        );
    }

    #[test]
    fn bundled_sources_come_from_skills_dir_not_cursor() {
        let generated = include_str!(concat!(env!("OUT_DIR"), "/bundled_skills.rs"));
        assert!(
            !generated.contains(".cursor"),
            "embed catalog must not reference .cursor"
        );
        let mut saw_include = false;
        for line in generated.lines() {
            if line.contains("include_str!") {
                saw_include = true;
                assert!(
                    line.contains("/skills/") && !line.contains(".cursor"),
                    "unexpected include path: {line}"
                );
            }
        }
        assert!(saw_include, "generated catalog had no include_str! paths");
    }

    #[test]
    fn tutorial_references_are_bundled() {
        let skill = find_skill("pixel-art-craft").unwrap();
        for path in [
            "SKILL.md",
            "references/beginner.md",
            "references/intermediate.md",
            "references/advanced.md",
            "references/master.md",
        ] {
            assert!(
                skill_file(skill, path).is_some(),
                "pixel-art-craft missing {path}"
            );
        }
        assert!(skill_file(skill, "REPORT.md").is_none());
    }

    #[test]
    fn parent_paths_are_rejected() {
        let err = split_spec("pixelforge/../secret", None).unwrap_err();
        assert_eq!(err.code(), "invalid_argument");
    }
}
