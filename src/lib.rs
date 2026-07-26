//! Core library for the `mdt` Markdown task reporter and terminal explorer.

pub mod cli;
pub mod discover;
pub mod error;
pub mod markdown;
pub mod model;
pub mod plain;
pub mod snapshot;
pub mod tui;
pub mod watch;

use std::{
    env,
    io::{self, IsTerminal, Write, stdout},
};

use cli::{Cli, ColorWhen};
use discover::{DiscoverOptions, validate_target};
use error::MdtError;
use plain::{ColorEnvironment, ColorMode, PlainReport, render_report};
use snapshot::build_snapshot;

/// Run one parsed CLI invocation.
///
/// # Errors
///
/// Returns argument/target, terminal, scan, or watcher failures that prevent
/// the selected mode from running.
pub fn run(cli: &Cli) -> Result<(), MdtError> {
    cli.validate_mode()?;
    let target = cli.resolved_path()?;
    let options = DiscoverOptions {
        ignore: cli.ignore.clone(),
        no_default_ignore: cli.no_default_ignore,
    };
    if cli.tui {
        validate_target(&target)?;
        return tui::run(&target, &cli.path, &options, cli.color);
    }

    let snapshot = build_snapshot(&target, &options)?;
    let report = PlainReport::from(&snapshot);
    let stdout_is_terminal = stdout().is_terminal();
    let environment = ColorEnvironment {
        stdout_is_terminal,
        color_supported: env::var("TERM").map_or(true, |term| term != "dumb"),
        no_color: env::var_os("NO_COLOR").is_some(),
    };
    let color = match cli.color {
        ColorWhen::Auto => ColorMode::Auto,
        ColorWhen::Always => ColorMode::Always,
        ColorWhen::Never => ColorMode::Never,
    };
    let rendered = render_report(&report, color, environment);
    write_non_interactive_report(&mut stdout().lock(), rendered.as_bytes())
}

/// Write a completed plain report, treating a closed downstream pipe as a
/// successful early consumer exit.
///
/// `mdt` commonly participates in pipelines such as `mdt | head`. Once that
/// consumer closes its input, no more report bytes can be observed, so a
/// `BrokenPipe` is not a runtime failure. Every other stdout error remains a
/// fatal output error.
fn write_non_interactive_report(output: &mut impl Write, report: &[u8]) -> Result<(), MdtError> {
    match output.write_all(report).and_then(|()| output.flush()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(MdtError::Output(error)),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};

    use super::write_non_interactive_report;
    use crate::error::MdtError;

    struct FailingWriter {
        write_error: Option<io::ErrorKind>,
        flush_error: Option<io::ErrorKind>,
    }

    impl Write for FailingWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            if let Some(kind) = self.write_error {
                Err(io::Error::from(kind))
            } else {
                Ok(buffer.len())
            }
        }

        fn flush(&mut self) -> io::Result<()> {
            if let Some(kind) = self.flush_error {
                Err(io::Error::from(kind))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn broken_pipe_while_writing_is_successful() {
        let mut writer = FailingWriter {
            write_error: Some(io::ErrorKind::BrokenPipe),
            flush_error: None,
        };

        assert!(write_non_interactive_report(&mut writer, b"report\n").is_ok());
    }

    #[test]
    fn broken_pipe_while_flushing_is_successful() {
        let mut writer = FailingWriter {
            write_error: None,
            flush_error: Some(io::ErrorKind::BrokenPipe),
        };

        assert!(write_non_interactive_report(&mut writer, b"report\n").is_ok());
    }

    #[test]
    fn other_stdout_errors_are_runtime_failures() {
        let mut writer = FailingWriter {
            write_error: Some(io::ErrorKind::PermissionDenied),
            flush_error: None,
        };

        let error = write_non_interactive_report(&mut writer, b"report\n")
            .expect_err("permission denied must remain fatal");

        assert_eq!(error.exit_code(), 1);
        assert!(matches!(&error, MdtError::Output(source)
            if source.kind() == io::ErrorKind::PermissionDenied));
        assert!(
            error
                .to_string()
                .starts_with("cannot write report to stdout:")
        );
    }
}
