use std::{
    env,
    ffi::OsString,
    io::{IsTerminal, stdin, stdout},
    path::{Path, PathBuf},
};

use clap::{Parser, ValueEnum};

use crate::error::MdtError;

/// Inspect Markdown task lists from the command line or a live terminal UI.
#[derive(Debug, Clone, Parser)]
#[command(name = "mdt", version, about)]
pub struct Cli {
    /// Markdown file or directory to inspect.
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Open the interactive live task explorer.
    #[arg(long)]
    pub tui: bool,

    /// Add an ignored directory name or root-relative path.
    #[arg(long, value_name = "PATTERN", action = clap::ArgAction::Append)]
    pub ignore: Vec<String>,

    /// Disable the built-in ignored-directory list.
    #[arg(long)]
    pub no_default_ignore: bool,

    /// Control ANSI color in the non-interactive report.
    #[arg(long, value_enum, default_value_t = ColorWhen::Auto)]
    pub color: ColorWhen,
}

impl Cli {
    /// Parse arguments from the process environment.
    #[must_use]
    pub fn parse_env() -> Self {
        Self::parse()
    }

    /// Parse an arbitrary argument sequence, primarily for embedding and tests.
    ///
    /// # Errors
    ///
    /// Returns Clap's diagnostic when the argument sequence is invalid.
    pub fn try_parse_from_args<I, T>(args: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        Self::try_parse_from(args)
    }

    /// Validate terminal requirements before any interactive terminal mutation.
    ///
    /// # Errors
    ///
    /// Returns [`MdtError::TuiRequiresTerminal`] when interactive mode is
    /// requested without terminal stdin and stdout.
    pub fn validate_mode(&self) -> Result<(), MdtError> {
        if self.tui && !(stdin().is_terminal() && stdout().is_terminal()) {
            return Err(MdtError::TuiRequiresTerminal);
        }
        Ok(())
    }

    /// Preserve the user's display path while resolving the absolute scan target.
    ///
    /// # Errors
    ///
    /// Returns a target error when the path is missing or cannot be resolved.
    pub fn resolved_path(&self) -> Result<PathBuf, MdtError> {
        resolve_target(&self.path)
    }

    /// Resolve color using CLI precedence over the conventional `NO_COLOR` value.
    #[must_use]
    pub fn effective_color(&self) -> ColorWhen {
        match self.color {
            ColorWhen::Always => ColorWhen::Always,
            ColorWhen::Never => ColorWhen::Never,
            ColorWhen::Auto if env::var_os("NO_COLOR").is_some() => ColorWhen::Never,
            ColorWhen::Auto => ColorWhen::Auto,
        }
    }
}

/// When ANSI styling is emitted by the plain renderer.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ColorWhen {
    #[default]
    Auto,
    Always,
    Never,
}

impl ColorWhen {
    /// Whether color is active for a particular stdout terminal capability.
    #[must_use]
    pub const fn enabled(self, stdout_is_terminal: bool) -> bool {
        match self {
            Self::Auto => stdout_is_terminal,
            Self::Always => true,
            Self::Never => false,
        }
    }
}

fn resolve_target(path: &Path) -> Result<PathBuf, MdtError> {
    if !path.exists() {
        return Err(MdtError::TargetMissing(path.to_path_buf()));
    }
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .map_err(|source| MdtError::UnreadableTarget {
                path: path.to_path_buf(),
                source,
            })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{Cli, ColorWhen};

    #[test]
    fn defaults_to_current_directory_and_plain_mode() {
        let cli = Cli::try_parse_from_args(["mdt"]).unwrap();
        assert_eq!(cli.path, PathBuf::from("."));
        assert!(!cli.tui);
        assert_eq!(cli.color, ColorWhen::Auto);
        assert!(cli.ignore.is_empty());
    }

    #[test]
    fn parses_every_public_option() {
        let cli = Cli::try_parse_from_args([
            "mdt",
            "--tui",
            "--ignore",
            "fixtures",
            "--ignore",
            "generated/docs",
            "--no-default-ignore",
            "--color",
            "always",
            "specs/tasks",
        ])
        .unwrap();

        assert!(cli.tui);
        assert_eq!(cli.path, PathBuf::from("specs/tasks"));
        assert_eq!(cli.ignore, ["fixtures", "generated/docs"]);
        assert!(cli.no_default_ignore);
        assert_eq!(cli.color, ColorWhen::Always);
    }

    #[test]
    fn color_enablement_respects_mode() {
        assert!(ColorWhen::Always.enabled(false));
        assert!(!ColorWhen::Never.enabled(true));
        assert!(ColorWhen::Auto.enabled(true));
        assert!(!ColorWhen::Auto.enabled(false));
    }
}
