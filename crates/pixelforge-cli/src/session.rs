//! One process, one [`pixelforge_kernel::Document`].
//!
//! The project file stores pixels, the palette, frames, delays, and, when set,
//! one rectangular selection. It does not store the undo stack. Loading a file
//! therefore starts from the same empty stack as
//! [`pixelforge_kernel::Document::load_project`].
//!
//! [`Session::open_with`] can attach a [`crate::checkpoint::Checkpoint`]. That
//! sidecar is the only place a later process finds those steps.

use std::fs;
use std::path::{Path, PathBuf};

use pixelforge_kernel::{Document, TOOL_API_VERSION};
use serde_json::{json, Value};

use crate::checkpoint::{write_atomic, Checkpoint};
use crate::failure::CliFailure;

pub struct SaveReport {
    pub project: bool,
    pub checkpoint: bool,
}

pub struct Session {
    path: PathBuf,
    document: Document,
    /// Canonical project text last known to be on disk.
    disk: String,
    checkpoint: Option<Checkpoint>,
}

enum HistoryTool {
    Undo,
    Redo,
}

impl Session {
    pub fn open(path: &Path) -> Result<Self, CliFailure> {
        let text = fs::read_to_string(path).map_err(|err| CliFailure::io(path, err))?;
        let document = Document::load_project(&text).map_err(CliFailure::kernel)?;
        let disk = document.save_project();
        Ok(Self {
            path: path.to_path_buf(),
            document,
            disk,
            checkpoint: None,
        })
    }

    /// Opens the project and, when `checkpoint` is set, its sidecar.
    pub fn open_with(path: &Path, checkpoint: Option<&Path>) -> Result<Self, CliFailure> {
        let mut session = Self::open(path)?;
        if let Some(file) = checkpoint {
            if paths_same(path, file) {
                return Err(CliFailure::new(
                    "Cli",
                    "checkpoint path and project path are the same file",
                ));
            }
            session.checkpoint = Some(Checkpoint::attach(file, &session.disk)?);
        }
        Ok(session)
    }

    pub fn blank(width: u16, height: u16, path: &Path) -> Result<Self, CliFailure> {
        let document = Document::new(width, height).map_err(CliFailure::kernel)?;
        Ok(Self {
            path: path.to_path_buf(),
            document,
            // Empty so the first save always writes the new file.
            disk: String::new(),
            checkpoint: None,
        })
    }

    /// A 1×1 document that matches its baseline, so a failed import does not create `path`.
    pub fn placeholder(path: &Path) -> Result<Self, CliFailure> {
        let document = Document::new(1, 1).map_err(CliFailure::kernel)?;
        let disk = document.save_project();
        Ok(Self {
            path: path.to_path_buf(),
            document,
            disk,
            checkpoint: None,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn checkpoint_path(&self) -> Option<&Path> {
        self.checkpoint.as_ref().map(Checkpoint::path)
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn call(&mut self, request: &str) -> String {
        if let Some((kind, id)) = history_request(request) {
            let take = match kind {
                HistoryTool::Undo => self.checkpoint_undo_ready(),
                HistoryTool::Redo => self.checkpoint_redo_ready(),
            };
            if take {
                return self.apply_checkpoint(kind, id);
            }
            let response = self.document.call_tool(request);
            return self.annotate_history(response);
        }
        self.document.call_tool(request)
    }

    /// Writes the kernel's project text when a tool changed it, then the sidecar.
    pub fn save_if_changed(&mut self) -> Result<SaveReport, CliFailure> {
        let text = self.document.save_project();
        let mut report = SaveReport {
            project: false,
            checkpoint: false,
        };
        if text != self.disk {
            write_atomic(&self.path, &text)?;
            self.disk = text.clone();
            report.project = true;
        }
        if let Some(checkpoint) = &mut self.checkpoint {
            checkpoint.note_change(&text);
            if checkpoint.flush()? {
                report.checkpoint = true;
            }
        }
        Ok(report)
    }

    fn checkpoint_undo_ready(&self) -> bool {
        self.checkpoint.as_ref().is_some_and(Checkpoint::can_undo)
            && !self.document.can_undo()
            && !self.document.gesture_open()
    }

    fn checkpoint_redo_ready(&self) -> bool {
        self.checkpoint.as_ref().is_some_and(Checkpoint::can_redo)
            && !self.document.can_redo()
            && !self.document.gesture_open()
    }

    fn apply_checkpoint(&mut self, kind: HistoryTool, id: Option<Value>) -> String {
        let current = self.document.save_project();
        let stored = match kind {
            HistoryTool::Undo => self
                .checkpoint
                .as_ref()
                .and_then(Checkpoint::undo_last)
                .map(str::to_string),
            HistoryTool::Redo => self
                .checkpoint
                .as_ref()
                .and_then(Checkpoint::redo_last)
                .map(str::to_string),
        };
        let Some(stored) = stored else {
            return match kind {
                HistoryTool::Undo => history_err(id, "NothingToUndo", "nothing to undo"),
                HistoryTool::Redo => history_err(id, "NothingToRedo", "nothing to redo"),
            };
        };
        let document = match Document::load_project(&stored) {
            Ok(document) => document,
            Err(err) => return history_err(id, err.code(), err.to_string()),
        };
        let restored = document.save_project();
        self.document = document;
        let checkpoint = self
            .checkpoint
            .as_mut()
            .expect("checkpoint undo runs only when a sidecar is attached");
        match kind {
            HistoryTool::Undo => checkpoint.commit_undo(&current, &restored),
            HistoryTool::Redo => checkpoint.commit_redo(&current, &restored),
        }
        let (can_undo, can_redo) = self.history_flags();
        history_ok(id, can_undo, can_redo)
    }

    /// Kernel `undo` / `redo` only see this process. Advertise a further
    /// checkpoint step once that stack is idle.
    fn annotate_history(&self, response: String) -> String {
        if self.checkpoint.is_none() {
            return response;
        }
        let mut value: Value = match serde_json::from_str(&response) {
            Ok(value) => value,
            Err(_) => return response,
        };
        if value.get("ok") != Some(&Value::Bool(true)) {
            return response;
        }
        let Some(result) = value.get_mut("result").and_then(Value::as_object_mut) else {
            return response;
        };
        if !result.contains_key("canUndo") {
            return response;
        }
        let (can_undo, can_redo) = self.history_flags();
        result.insert("canUndo".to_string(), json!(can_undo));
        result.insert("canRedo".to_string(), json!(can_redo));
        value.to_string()
    }

    fn history_flags(&self) -> (bool, bool) {
        let gesture = self.document.gesture_open();
        let (checkpoint_undo, checkpoint_redo) = self
            .checkpoint
            .as_ref()
            .map(|checkpoint| (checkpoint.can_undo(), checkpoint.can_redo()))
            .unwrap_or((false, false));
        let can_undo = self.document.can_undo() || (!gesture && checkpoint_undo);
        let can_redo = self.document.can_redo() || (!gesture && checkpoint_redo);
        (can_undo, can_redo)
    }
}

fn paths_same(project: &Path, checkpoint: &Path) -> bool {
    if project == checkpoint {
        return true;
    }
    match (fs::canonicalize(project), fs::canonicalize(checkpoint)) {
        (Ok(project), Ok(checkpoint)) => project == checkpoint,
        _ => false,
    }
}

/// A checkpoint step is taken only for a request the kernel would accept.
fn history_request(request: &str) -> Option<(HistoryTool, Option<Value>)> {
    let value: Value = serde_json::from_str(request).ok()?;
    let object = value.as_object()?;
    for key in object.keys() {
        if !matches!(key.as_str(), "apiVersion" | "id" | "tool" | "params") {
            return None;
        }
    }
    if !version_ok(&value) || !id_ok(&value) || !params_empty(&value) {
        return None;
    }
    let kind = match object.get("tool").and_then(Value::as_str) {
        Some("undo") => HistoryTool::Undo,
        Some("redo") => HistoryTool::Redo,
        _ => return None,
    };
    let id = match object.get("id") {
        Some(Value::String(_)) | Some(Value::Number(_)) => object.get("id").cloned(),
        _ => None,
    };
    Some((kind, id))
}

fn version_ok(value: &Value) -> bool {
    match value.get("apiVersion") {
        None | Some(Value::Null) => true,
        Some(Value::Number(number)) => number.as_u64() == Some(u64::from(TOOL_API_VERSION)),
        Some(_) => false,
    }
}

fn id_ok(value: &Value) -> bool {
    matches!(
        value.get("id"),
        None | Some(Value::Null) | Some(Value::String(_)) | Some(Value::Number(_))
    )
}

fn params_empty(value: &Value) -> bool {
    match value.get("params") {
        None | Some(Value::Null) => true,
        Some(Value::Object(map)) => map.is_empty(),
        Some(_) => false,
    }
}

fn history_ok(id: Option<Value>, can_undo: bool, can_redo: bool) -> String {
    let mut body = json!({
        "ok": true,
        "apiVersion": TOOL_API_VERSION,
        "result": { "canUndo": can_undo, "canRedo": can_redo },
    });
    if let Some(id) = id {
        body.as_object_mut()
            .expect("history response is an object")
            .insert("id".to_string(), id);
    }
    body.to_string()
}

fn history_err(id: Option<Value>, code: &str, message: impl AsRef<str>) -> String {
    let mut body = json!({
        "ok": false,
        "apiVersion": TOOL_API_VERSION,
        "error": { "code": code, "message": message.as_ref() },
    });
    if let Some(id) = id {
        body.as_object_mut()
            .expect("history response is an object")
            .insert("id".to_string(), id);
    }
    body.to_string()
}
