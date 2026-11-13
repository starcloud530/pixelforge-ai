//! Failures that happen before a Tool API response exists, or while writing files.

use std::path::Path;

use pixelforge_kernel::KernelError;

#[derive(Debug)]
pub struct CliFailure {
    code: String,
    message: String,
    hint: Option<String>,
}

impl CliFailure {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            hint: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn io(path: &Path, err: std::io::Error) -> Self {
        Self::new("Cli", format!("cannot access {}: {err}", path.display()))
    }

    pub fn kernel(err: KernelError) -> Self {
        Self::new(err.code(), err.to_string())
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn hint(&self) -> Option<&str> {
        self.hint.as_deref()
    }
}
