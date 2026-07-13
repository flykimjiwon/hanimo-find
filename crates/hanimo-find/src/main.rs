#![forbid(unsafe_code)]
//! Hanimo Find command-line and stdio MCP entry point.

use std::{io::Write as _, process::ExitCode};

use clap::Parser as _;

use crate::{cli::Cli, commands::Failure};

mod cli;
mod commands;
mod mcp;
mod search_adapter;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    match commands::dispatch(Cli::parse()).await {
        Ok(status) => ExitCode::from(status.code()),
        Err(failure) => report_failure(&failure),
    }
}

fn report_failure(failure: &Failure) -> ExitCode {
    let stderr = std::io::stderr();
    let mut output = stderr.lock();
    if writeln!(output, "error: {}", failure.error).is_err() {
        return ExitCode::from(commands::Status::ScanFailure.code());
    }
    ExitCode::from(failure.status.code())
}
