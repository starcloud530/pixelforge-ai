//! Preflight artifact destinations before writing any member of an export.
//!
//! Source documents, history and lock sidecars are not artifact destinations.
//! Canonical paths catch symlinks and parent aliases; hard-linked destinations
//! are refused because replacing one name would silently split its aliases.

use std::fs;
use std::path::{Path, PathBuf};

use crate::failure::CliFailure;

pub(crate) fn check(inputs: &[&Path], outputs: &[&Path]) -> Result<(), CliFailure> {
    let mut reserved = Vec::new();
    for input in inputs {
        let source = resolve(input)?;
        reserved.push(source.clone());
        if let Some(name) = source.file_name() {
            let mut name = name.to_os_string();
            name.push(".pixelforge.lock");
            reserved.push(source.with_file_name(name));
        }
    }
    for output in outputs {
        let target = resolve(output)?;
        if reserved.contains(&target) {
            return Err(conflict(
                output,
                "aliases an input, history/lock sidecar, or another output",
            ));
        }
        if let Ok(meta) = fs::symlink_metadata(output) {
            if meta.file_type().is_symlink() {
                return Err(conflict(
                    output,
                    "is a symbolic link; pass an ordinary destination",
                ));
            }
            if !meta.is_file() {
                return Err(conflict(output, "is not an ordinary file"));
            }
            if crate::store::link_count(output)? > 1 {
                return Err(conflict(output, "has hard-link aliases"));
            }
        }
        reserved.push(target);
    }
    Ok(())
}

fn conflict(path: &Path, detail: &str) -> CliFailure {
    CliFailure::new(
        "OutputConflict",
        format!(
            "refusing output {}: {detail}; no export files were written",
            path.display()
        ),
    )
}

// Resolve the nearest existing ancestor so two new output names with different
// parent spellings are compared before directories or files are created.
fn resolve(path: &Path) -> Result<PathBuf, CliFailure> {
    match fs::canonicalize(path) {
        Ok(path) => Ok(path),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let name = path.file_name().ok_or_else(|| CliFailure::io(path, err))?;
            let parent = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            Ok(resolve(parent)?.join(name))
        }
        Err(err) => Err(CliFailure::io(path, err)),
    }
}
