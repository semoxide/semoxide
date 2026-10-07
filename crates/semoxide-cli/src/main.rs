//! Command-line interface of semoxide.

#![forbid(unsafe_code)]

use std::process::ExitCode;

use clap::Parser;

/// A fully automated release tool, inspired by semantic-release.
///
/// Bare `semoxide` prints this help; it never starts a release (CLI.md).
#[derive(Parser)]
#[command(name = "semoxide", bin_name = "semoxide", version = semoxide::VERSION, arg_required_else_help = true)]
struct Cli {}

fn main() -> ExitCode {
    let _cli = Cli::parse();
    ExitCode::SUCCESS
}
