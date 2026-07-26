use std::path::PathBuf;

use thiserror::Error;

/// Failures that prevent `mdt` from producing or maintaining a task snapshot.
#[derive(Debug, Error)]
pub enum MdtError {
    #[error("target does not exist: {0}")]
    TargetMissing(PathBuf),
    #[error("unsupported Markdown file extension: {0}")]
    UnsupportedFile(PathBuf),
    #[error("cannot read target {path}: {source}")]
    UnreadableTarget {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot write report to stdout: {0}")]
    Output(std::io::Error),
    #[error("interactive mode requires both stdin and stdout to be terminals")]
    TuiRequiresTerminal,
    #[error("terminal error: {0}")]
    Terminal(String),
    #[error("filesystem watcher error: {0}")]
    Watch(String),
}

impl MdtError {
    /// The process exit status defined by the CLI contract.
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::UnsupportedFile(_) | Self::TuiRequiresTerminal => 2,
            Self::TargetMissing(_)
            | Self::UnreadableTarget { .. }
            | Self::Output(_)
            | Self::Terminal(_)
            | Self::Watch(_) => 1,
        }
    }
}
