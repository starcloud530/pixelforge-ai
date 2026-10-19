//! One process, one [`pixelforge_kernel::Document`].
//!
//! The project file stores pixels, the palette, frames, and delays. It does
//! not store the undo stack. Loading a file therefore starts from the same
//! empty stack as [`pixelforge_kernel::Document::load_project`].

use std::fs;
use std::path::{Path, PathBuf};

use pixelforge_kernel::Document;

use crate::failure::CliFailure;

pub struct Session {
    path: PathBuf,
    document: Document,
    /// `save_project()` text from just after load. Unchanged documents are not rewritten.
    baseline: String,
}

impl Session {
    pub fn open(path: &Path) -> Result<Self, CliFailure> {
        let text = fs::read_to_string(path).map_err(|err| CliFailure::io(path, err))?;
        let document = Document::load_project(&text).map_err(CliFailure::kernel)?;
        let baseline = document.save_project();
        Ok(Self {
            path: path.to_path_buf(),
            document,
            baseline,
        })
    }

    pub fn blank(width: u16, height: u16, path: &Path) -> Result<Self, CliFailure> {
        let document = Document::new(width, height).map_err(CliFailure::kernel)?;
        Ok(Self {
            path: path.to_path_buf(),
            document,
            // Empty so the first save always writes the new file.
            baseline: String::new(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn call(&mut self, request: &str) -> String {
        self.document.call_tool(request)
    }

    /// Writes the kernel's project text when a tool changed it.
    pub fn save_if_changed(&self) -> Result<bool, CliFailure> {
        let text = self.document.save_project();
        if text == self.baseline {
            return Ok(false);
        }
        write_atomic(&self.path, &text)?;
        Ok(true)
    }
}

fn write_atomic(path: &Path, text: &str) -> Result<(), CliFailure> {
    if path.file_name().is_none() {
        return Err(CliFailure::new(
            "Cli",
            format!("project path has no file name: {}", path.display()),
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
