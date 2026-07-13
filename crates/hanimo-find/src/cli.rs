use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "hanimo", version, about = "Evidence-first local source search")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: TopCommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum TopCommand {
    Find(FindArgs),
}

#[derive(Debug, Args)]
pub(crate) struct FindArgs {
    #[command(subcommand)]
    pub(crate) command: FindCommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum FindCommand {
    Search(SearchArgs),
    Verify(VerifyArgs),
    Diagnose(DiagnoseArgs),
    Mcp,
}

#[derive(Debug, Args)]
pub(crate) struct SearchArgs {
    pub(crate) query: String,
    #[arg(default_value = ".")]
    pub(crate) path: PathBuf,
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    pub(crate) format: OutputFormat,
}

#[derive(Debug, Args)]
pub(crate) struct VerifyArgs {
    pub(crate) bundle: PathBuf,
    #[arg(long, default_value = ".")]
    pub(crate) root: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct DiagnoseArgs {
    #[arg(default_value = ".")]
    pub(crate) path: PathBuf,
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    pub(crate) format: OutputFormat,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum OutputFormat {
    Json,
    Md,
}
