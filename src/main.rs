mod cli;
mod core;

use std::io::{IsTerminal, stdout};
use std::process::ExitCode;

use clap::{CommandFactory, Parser};
use colored::Colorize;

use crate::core::prompt::Cancelled;

fn main() -> ExitCode {
    // Handles shell completion requests (`COMPLETE=bash secli ...`) and exits.
    clap_complete::CompleteEnv::with_factory(cli::Cli::command).complete();

    let cli = cli::Cli::parse();
    if !stdout().is_terminal() {
        colored::control::set_override(false);
    }

    match cli.run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) if err.is::<Cancelled>() => ExitCode::from(130),
        Err(err) => {
            eprintln!("{} {err:#}", "Error:".red().bold());
            ExitCode::FAILURE
        }
    }
}
