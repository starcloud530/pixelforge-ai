//! Failures that happen before a Tool API response exists, or while writing files.

use std::path::Path;

use pixelforge_kernel::KernelError;

#[derive(Debug)]
pub struct CliFailure {
    code: &'static str,
    message: String,
}

impl CliFailure {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn io(path: &Path, err: std::io::Error) -> Self {
        Self::new("Cli", format!("cannot access {}: {err}", path.display()))
    }

    pub fn kernel(err: KernelError) -> Self {
        Self::new(err.code(), err.to_string())
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}
