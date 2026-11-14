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

use crate::checkpoint::Checkpoint;
use crate::failure::CliFailure;
use crate::store::{self, ProjectLock};

/// Whether this command may publish a new project file.
#[derive(Clone, Copy)]
pub enum Access {
    /// Read the bytes now. Do not take the exclusive lock and do not write.
    ReadOnly,
    /// Lock, then read. Hold the lock until this session is dropped.
    Exclusive,
}

pub struct SaveReport {
    pub project: bool,
    pub checkpoint: bool,
}

pub struct Session {
    /// Path the caller passed, used in messages.
    path: PathBuf,
    /// Canonical file the lock covers. Renames land here.
    write_path: PathBuf,
    document: Document,
    /// Canonical project text last known to be on disk.
    disk: String,
    checkpoint: Option<Checkpoint>,
    /// Held so the OS lock stays acquired until the command finishes.
    #[allow(dead_code)]
    lock: Option<ProjectLock>,
    writable: bool,
}

enum HistoryTool {
    Undo,
    Redo,
}

impl Session {
    /// Locks `path`, then loads it. A missing file is an I/O error.
    pub fn open_with(
        path: &Path,
        checkpoint: Option<&Path>,
        access: Access,
    ) -> Result<Self, CliFailure> {
        match access {
            Access::ReadOnly => Self::open_readonly(path, checkpoint),
            Access::Exclusive => {
                let lock = ProjectLock::acquire(path)?;
                Self::open_locked(lock, checkpoint)
            }
        }
    }

    /// The caller already holds `lock`. The project is read after that lock.
    pub fn open_locked(lock: ProjectLock, checkpoint: Option<&Path>) -> Result<Self, CliFailure> {
        let text =
            fs::read_to_string(lock.target()).map_err(|err| CliFailure::io(lock.target(), err))?;
        let document = Document::load_project(&text).map_err(CliFailure::kernel)?;
        let disk = document.save_project();
        let path = lock.user_path().to_path_buf();
        let write_path = lock.target().to_path_buf();
        let mut session = Self {
            path,
            write_path,
            document,
            disk: disk.clone(),
            checkpoint: None,
            lock: Some(lock),
            writable: true,
        };
        session.attach_checkpoint(checkpoint, &disk)?;
        Ok(session)
    }

    fn open_readonly(path: &Path, checkpoint: Option<&Path>) -> Result<Self, CliFailure> {
        let text = fs::read_to_string(path).map_err(|err| CliFailure::io(path, err))?;
        let document = Document::load_project(&text).map_err(CliFailure::kernel)?;
        let disk = document.save_project();
        let mut session = Self {
            path: path.to_path_buf(),
            write_path: path.to_path_buf(),
            document,
            disk: disk.clone(),
            checkpoint: None,
            lock: None,
            writable: false,
        };
        session.attach_checkpoint(checkpoint, &disk)?;
        Ok(session)
    }

    /// Locks `path` and prepares a new document. The file is created on save.
    /// An existing canonical file is refused and left unchanged.
    pub fn create_blank(width: u16, height: u16, path: &Path) -> Result<Self, CliFailure> {
        let lock = ProjectLock::acquire(path)?;
        if lock.target().exists() {
            return Err(CliFailure::new(
                "Cli",
                format!("project file already exists: {}", path.display()),
            ));
        }
        let document = Document::new(width, height).map_err(CliFailure::kernel)?;
        Ok(Self {
            path: path.to_path_buf(),
            write_path: lock.target().to_path_buf(),
            document,
            // Empty so the first save always writes the new file.
            disk: String::new(),
            checkpoint: None,
            lock: Some(lock),
            writable: true,
        })
    }

    /// Locks `path` for a create. The baseline matches the placeholder, so a
    /// failed import does not create the project file.
    pub fn create_placeholder(path: &Path) -> Result<Self, CliFailure> {
        let lock = ProjectLock::acquire(path)?;
        if lock.target().exists() {
            return Err(CliFailure::new(
                "Cli",
                format!("project file already exists: {}", path.display()),
            ));
        }
        let document = Document::new(1, 1).map_err(CliFailure::kernel)?;
        let disk = document.save_project();
        Ok(Self {
            path: path.to_path_buf(),
            write_path: lock.target().to_path_buf(),
            document,
            disk,
            checkpoint: None,
            lock: Some(lock),
            writable: true,
        })
    }

    fn attach_checkpoint(
        &mut self,
        checkpoint: Option<&Path>,
        disk: &str,
    ) -> Result<(), CliFailure> {
        let Some(file) = checkpoint else {
            return Ok(());
        };
        if paths_same(&self.path, file) || paths_same(&self.write_path, file) {
            return Err(CliFailure::new(
                "Cli",
                "checkpoint path and project path are the same file",
            ));
        }
        self.checkpoint = Some(Checkpoint::attach(file, disk)?);
        Ok(())
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
    ///
    /// The project replace and the checkpoint replace are two publishes. If
    /// the checkpoint write fails after the project write, the project file
    /// already contains this edit.
    pub fn save_if_changed(&mut self) -> Result<SaveReport, CliFailure> {
        let text = self.document.save_project();
        let mut report = SaveReport {
            project: false,
            checkpoint: false,
        };
        if text != self.disk {
            if !self.writable {
                return Err(CliFailure::new(
                    "Cli",
                    "refusing to write a project opened for reading",
                ));
            }
            store::test_edit_barrier();
            store::write_atomic(&self.write_path, text.as_bytes())?;
            self.disk = text.clone();
            report.project = true;
        }
        let project_display = self.path.display().to_string();
        if let Some(checkpoint) = &mut self.checkpoint {
            let checkpoint_display = checkpoint.path().display().to_string();
            checkpoint.note_change(&text);
            match checkpoint.flush() {
                Ok(wrote) => {
                    if wrote {
                        report.checkpoint = true;
                    }
                }
                Err(failure) => {
                    if report.project {
                        return Err(CliFailure::new(
                            "CheckpointFile",
                            format!(
                                "wrote {project_display} but could not update checkpoint {checkpoint_display}: {}. The project file already has this edit; the checkpoint was not updated. These two files are not one atomic commit",
                                failure.message()
                            ),
                        ));
                    }
                    return Err(failure);
                }
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
