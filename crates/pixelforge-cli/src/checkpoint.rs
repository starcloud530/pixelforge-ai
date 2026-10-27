//! Sidecar undo for a later `pixelforge` process.
//!
//! The project file stays the pixels. This file stores earlier copies of that
//! text so `undo` / `redo` can put one back after the process that painted it
//! has exited. One step is the net change of one process, capped at
//! [`MAX_UNDO`]. Kernel `undo` inside the process still walks single
//! operations before this stack is touched.

use std::fs;
use std::path::{Path, PathBuf};

use pixelforge_kernel::MAX_UNDO;
use serde_json::{json, Value};

use crate::failure::CliFailure;

pub const CHECKPOINT_FORMAT: &str = "pixelforge-checkpoint";
const CHECKPOINT_VERSION: u64 = 1;

/// FNV-1a 64, same offset and prime as the palette hash, over the canonical
/// project text. An identity for the sidecar, not a secret.
fn project_hash(text: &str) -> String {
    const OFFSET: u64 = 14_695_981_039_346_656_037;
    const PRIME: u64 = 1_099_511_628_211;
    let mut hash = OFFSET;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    format!("{hash:016x}")
}

#[derive(Debug)]
pub struct Checkpoint {
    path: PathBuf,
    undo: Vec<String>,
    redo: Vec<String>,
    /// Canonical project text these stacks belong to.
    bound: String,
    dirty: bool,
}

impl Checkpoint {
    /// Loads `path`, or starts an empty session when the file is absent.
    ///
    /// A present file must name `project_text`. A mismatch leaves both files
    /// alone: the caller has not edited yet.
    pub fn attach(path: &Path, project_text: &str) -> Result<Self, CliFailure> {
        if !path.exists() {
            return Ok(Self {
                path: path.to_path_buf(),
                undo: Vec::new(),
                redo: Vec::new(),
                bound: project_text.to_string(),
                dirty: false,
            });
        }
        let raw = fs::read_to_string(path).map_err(|err| CliFailure::io(path, err))?;
        let (undo, redo, hash) = parse(&raw, path)?;
        let actual = project_hash(project_text);
        if hash != actual {
            return Err(CliFailure::new(
                "CheckpointMismatch",
                format!(
                    "checkpoint {} does not match the project file; it was edited without this --checkpoint, so nothing was written",
                    path.display()
                ),
            ));
        }
        Ok(Self {
            path: path.to_path_buf(),
            undo,
            redo,
            bound: project_text.to_string(),
            dirty: false,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_last(&self) -> Option<&str> {
        self.undo.last().map(String::as_str)
    }

    pub fn redo_last(&self) -> Option<&str> {
        self.redo.last().map(String::as_str)
    }

    /// Records `after` as one new step when it differs from the bound text.
    ///
    /// Clears redo. Oldest undo entries past [`MAX_UNDO`] are dropped.
    pub fn note_change(&mut self, after: &str) {
        if after == self.bound {
            return;
        }
        self.undo.push(self.bound.clone());
        trim(&mut self.undo);
        self.redo.clear();
        self.bound = after.to_string();
        self.dirty = true;
    }

    /// Pops one undo snapshot. `current` becomes the nearest redo.
    /// `restored` is the canonical text now on the document.
    pub fn commit_undo(&mut self, current: &str, restored: &str) {
        self.undo.pop();
        self.redo.push(current.to_string());
        trim(&mut self.redo);
        self.bound = restored.to_string();
        self.dirty = true;
    }

    /// Pops one redo snapshot. `current` becomes the nearest undo.
    pub fn commit_redo(&mut self, current: &str, restored: &str) {
        self.redo.pop();
        self.undo.push(current.to_string());
        trim(&mut self.undo);
        self.bound = restored.to_string();
        self.dirty = true;
    }

    /// Writes the sidecar when a step was recorded. Absent files stay absent
    /// until the first change.
    pub fn flush(&mut self) -> Result<bool, CliFailure> {
        if !self.dirty {
            return Ok(false);
        }
        let body = json!({
            "format": CHECKPOINT_FORMAT,
            "version": CHECKPOINT_VERSION,
            "projectHash": project_hash(&self.bound),
            "undo": self.undo,
            "redo": self.redo,
        });
        let mut text = serde_json::to_string_pretty(&body).expect("checkpoint fields are JSON");
        text.push('\n');
        write_atomic(&self.path, &text)?;
        self.dirty = false;
        Ok(true)
    }
}

fn trim(stack: &mut Vec<String>) {
    if stack.len() > MAX_UNDO {
        let extra = stack.len() - MAX_UNDO;
        stack.drain(0..extra);
    }
}

fn parse(raw: &str, path: &Path) -> Result<(Vec<String>, Vec<String>, String), CliFailure> {
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let value: Value = match serde_json::from_str(raw) {
        Ok(value) => value,
        Err(_) => {
            return Err(file_error(path, "checkpoint is not JSON"));
        }
    };
    let Some(object) = value.as_object() else {
        return Err(file_error(path, "checkpoint must be a JSON object"));
    };
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "format" | "version" | "projectHash" | "undo" | "redo"
        ) {
            return Err(file_error(
                path,
                &format!("checkpoint has unknown field \"{key}\""),
            ));
        }
    }
    match object.get("format") {
        Some(Value::String(name)) if name == CHECKPOINT_FORMAT => {}
        Some(_) => {
            return Err(file_error(
                path,
                "checkpoint format is not pixelforge-checkpoint",
            ));
        }
        None => return Err(file_error(path, "checkpoint is missing format")),
    }
    match object.get("version") {
        Some(Value::Number(number)) => match number.as_u64() {
            Some(CHECKPOINT_VERSION) => {}
            Some(version) => {
                return Err(file_error(
                    path,
                    &format!("checkpoint version {version} is not supported"),
                ));
            }
            None => return Err(file_error(path, "checkpoint version is not an integer")),
        },
        _ => return Err(file_error(path, "checkpoint is missing version")),
    }
    let Some(hash) = object.get("projectHash").and_then(Value::as_str) else {
        return Err(file_error(path, "checkpoint is missing projectHash"));
    };
    let undo = stack_from(object.get("undo"), "undo", path)?;
    let redo = stack_from(object.get("redo"), "redo", path)?;
    Ok((undo, redo, hash.to_string()))
}

fn stack_from(value: Option<&Value>, name: &str, path: &Path) -> Result<Vec<String>, CliFailure> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Err(file_error(
            path,
            &format!("checkpoint {name} must be an array"),
        ));
    };
    let mut stack = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let Some(text) = item.as_str() else {
            return Err(file_error(
                path,
                &format!("checkpoint {name}[{index}] must be a string"),
            ));
        };
        stack.push(text.to_string());
    }
    if stack.len() > MAX_UNDO {
        let extra = stack.len() - MAX_UNDO;
        stack.drain(0..extra);
    }
    Ok(stack)
}

fn file_error(path: &Path, reason: &str) -> CliFailure {
    CliFailure::new(
        "CheckpointFile",
        format!("checkpoint {}: {reason}", path.display()),
    )
}

pub(crate) fn write_atomic(path: &Path, text: &str) -> Result<(), CliFailure> {
    if path.file_name().is_none() {
        return Err(CliFailure::new(
            "Cli",
            format!("path has no file name: {}", path.display()),
        ));
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|err| CliFailure::io(parent, err))?;
        }
    }
    let mut tmp_name = path
        .file_name()
        .expect("file name checked above")
        .to_os_string();
    tmp_name.push(".tmp");
    let tmp = path.with_file_name(tmp_name);
    fs::write(&tmp, text).map_err(|err| CliFailure::io(&tmp, err))?;
    fs::rename(&tmp, path).map_err(|err| {
        let _ = fs::remove_file(&tmp);
        CliFailure::io(path, err)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "pixelforge-ckpt-{}-{}-{name}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn hash_tracks_the_project_text() {
        assert_eq!(project_hash("same"), project_hash("same"));
        assert_ne!(project_hash("same"), project_hash("same\n"));
    }

    #[test]
    fn missing_file_starts_empty_and_note_change_roundtrips() {
        let dir = scratch("round");
        let path = dir.join("hero.ckpt.json");
        let mut checkpoint = Checkpoint::attach(&path, "v0").unwrap();
        assert!(!checkpoint.can_undo());
        checkpoint.note_change("v1");
        checkpoint.note_change("v1");
        assert!(checkpoint.flush().unwrap());
        assert!(path.is_file());
        let loaded = Checkpoint::attach(&path, "v1").unwrap();
        assert_eq!(loaded.undo_last(), Some("v0"));
        assert!(!loaded.can_redo());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn mismatch_and_unknown_field_are_refused() {
        let dir = scratch("bad");
        let path = dir.join("hero.ckpt.json");
        let hash = project_hash("v0");
        fs::write(
            &path,
            format!(
                r#"{{"format":"pixelforge-checkpoint","version":1,"projectHash":"{hash}","undo":[],"redo":[]}}"#
            ),
        )
        .unwrap();
        let mismatch = Checkpoint::attach(&path, "v1").unwrap_err();
        assert_eq!(mismatch.code(), "CheckpointMismatch");

        fs::write(
            &path,
            format!(
                r#"{{"format":"pixelforge-checkpoint","version":1,"projectHash":"{hash}","undo":[],"redo":[],"extra":1}}"#
            ),
        )
        .unwrap();
        let unknown = Checkpoint::attach(&path, "v0").unwrap_err();
        assert_eq!(unknown.code(), "CheckpointFile");

        fs::write(&path, "{").unwrap();
        let broken = Checkpoint::attach(&path, "v0").unwrap_err();
        assert_eq!(broken.code(), "CheckpointFile");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn note_change_keeps_the_newest_max_undo_snapshots() {
        let dir = scratch("depth");
        let path = dir.join("hero.ckpt.json");
        let mut checkpoint = Checkpoint::attach(&path, "v0").unwrap();
        let steps = MAX_UNDO + 5;
        for index in 1..=steps {
            checkpoint.note_change(&format!("v{index}"));
        }
        assert_eq!(checkpoint.undo.len(), MAX_UNDO);
        assert_eq!(checkpoint.undo.first().map(String::as_str), Some("v5"));
        assert_eq!(checkpoint.bound, format!("v{steps}"));
        assert!(checkpoint.redo.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
