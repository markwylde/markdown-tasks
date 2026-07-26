use std::process::ExitCode;

use markdown_tasks::{cli::Cli, run};

fn main() -> ExitCode {
    let cli = Cli::parse_env();
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("mdt: {error}");
            ExitCode::from(error.exit_code())
        }
    }
}
