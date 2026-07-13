use std::{
    fs::File,
    io::{self, Read as _, Write as _},
    path::Path,
};

use hanimo_core::{
    CriticVerdict, EvidenceBundle, MAX_VERIFY_BUNDLE_BYTES, VerificationStatus, VerifyError,
    diagnose, model::SCHEMA_VERSION, render_markdown, verify,
};
use serde::Serialize;
use thiserror::Error;

use crate::{
    cli::{Cli, DiagnoseArgs, FindCommand, OutputFormat, SearchArgs, TopCommand, VerifyArgs},
    mcp,
    search_adapter::{SearchAdapterError, search_evidence},
};

#[derive(Debug, Clone, Copy)]
pub(crate) enum Status {
    Success,
    Rejected,
    Usage,
    InvalidBundle,
    EvidenceMismatch,
    ScanFailure,
}

impl Status {
    pub(crate) const fn code(self) -> u8 {
        match self {
            Self::Success => 0,
            Self::Rejected => 1,
            Self::Usage => 2,
            Self::InvalidBundle => 3,
            Self::EvidenceMismatch => 4,
            Self::ScanFailure => 5,
        }
    }
}

#[derive(Debug, Error)]
pub(crate) enum CommandError {
    #[error("invalid query: {0}")]
    InvalidQuery(#[source] SearchAdapterError),
    #[error("search failed: {0}")]
    Search(#[source] SearchAdapterError),
    #[error("invalid evidence bundle: {0}")]
    InvalidBundle(String),
    #[error("verification failed: {0}")]
    Verify(String),
    #[error("diagnosis failed: {0}")]
    Diagnose(String),
    #[error("output failed: {0}")]
    Output(#[from] io::Error),
    #[error(transparent)]
    Mcp(#[from] mcp::McpError),
}

#[derive(Debug)]
pub(crate) struct Failure {
    pub(crate) status: Status,
    pub(crate) error: CommandError,
}

pub(crate) async fn dispatch(cli: Cli) -> Result<Status, Failure> {
    match cli.command {
        TopCommand::Find(arguments) => match arguments.command {
            FindCommand::Search(arguments) => run_search(&arguments),
            FindCommand::Verify(arguments) => run_verify(&arguments),
            FindCommand::Diagnose(arguments) => run_diagnose(&arguments),
            FindCommand::Mcp => {
                mcp::serve_stdio()
                    .await
                    .map(|()| Status::Success)
                    .map_err(|error| Failure {
                        status: Status::ScanFailure,
                        error: error.into(),
                    })
            }
        },
    }
}

fn run_search(arguments: &SearchArgs) -> Result<Status, Failure> {
    let bundle = search_evidence(&arguments.query, &arguments.path).map_err(|error| Failure {
        status: if error.is_usage() {
            Status::Usage
        } else {
            Status::ScanFailure
        },
        error: if error.is_usage() {
            CommandError::InvalidQuery(error)
        } else {
            CommandError::Search(error)
        },
    })?;
    write_bundle(&bundle, arguments.format).map_err(output_failure)?;
    Ok(match bundle.critic.verdict {
        CriticVerdict::Accepted => Status::Success,
        CriticVerdict::Rejected => Status::Rejected,
    })
}

fn run_verify(arguments: &VerifyArgs) -> Result<Status, Failure> {
    let bytes = read_bundle(&arguments.bundle)?;
    let bundle: EvidenceBundle =
        serde_json::from_slice(&bytes).map_err(|error| invalid_bundle(&error))?;
    if bundle.schema_version != SCHEMA_VERSION || bundle.root.is_empty() {
        return Err(invalid_bundle(&"unsupported schema or empty root"));
    }
    if arguments.root.to_str() != Some(&bundle.root) {
        return Err(Failure {
            status: Status::ScanFailure,
            error: CommandError::Verify(
                "trusted verification root does not match recorded display root".to_owned(),
            ),
        });
    }
    let report = verify(&arguments.root, &bundle).map_err(|error| verification_failure(&error))?;
    write_json(&report).map_err(output_failure)?;
    Ok(match report.status {
        VerificationStatus::Verified => match bundle.critic.verdict {
            CriticVerdict::Accepted => Status::Success,
            CriticVerdict::Rejected => Status::Rejected,
        },
        VerificationStatus::Stale
        | VerificationStatus::Forged
        | VerificationStatus::SourceDrift => Status::EvidenceMismatch,
    })
}

fn read_bundle(path: &Path) -> Result<Vec<u8>, Failure> {
    let file = File::open(path).map_err(|error| invalid_bundle(&error))?;
    let limit = u64::try_from(MAX_VERIFY_BUNDLE_BYTES).map_err(|error| invalid_bundle(&error))?;
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| invalid_bundle(&error))?;
    if bytes.len() > MAX_VERIFY_BUNDLE_BYTES {
        return Err(invalid_bundle(&format_args!(
            "evidence bundle exceeds {MAX_VERIFY_BUNDLE_BYTES}-byte verification input limit"
        )));
    }
    Ok(bytes)
}

fn verification_failure(error: &VerifyError) -> Failure {
    if let Some(reason) = error.invalid_bundle_reason() {
        return invalid_bundle(&reason);
    }
    Failure {
        status: Status::ScanFailure,
        error: CommandError::Verify(error.to_string()),
    }
}

fn run_diagnose(arguments: &DiagnoseArgs) -> Result<Status, Failure> {
    let diagnosis = diagnose::diagnose(&arguments.path).map_err(|error| Failure {
        status: Status::ScanFailure,
        error: CommandError::Diagnose(error.to_string()),
    })?;
    match arguments.format {
        OutputFormat::Json => write_json(&diagnosis),
        OutputFormat::Md => write_text(&diagnose::render_markdown(&diagnosis)),
    }
    .map_err(output_failure)?;
    Ok(Status::Success)
}

fn write_bundle(bundle: &EvidenceBundle, format: OutputFormat) -> Result<(), CommandError> {
    match format {
        OutputFormat::Json => write_json(bundle),
        OutputFormat::Md => render_markdown(bundle)
            .map_err(|error| CommandError::Search(SearchAdapterError::Evidence(error)))
            .and_then(|markdown| write_text(&markdown)),
    }
}

fn write_json(value: &impl Serialize) -> Result<(), CommandError> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    serde_json::to_writer(&mut output, value)
        .map_err(io::Error::other)
        .map_err(CommandError::Output)?;
    output.write_all(b"\n").map_err(CommandError::Output)
}

fn write_text(value: &str) -> Result<(), CommandError> {
    io::stdout()
        .lock()
        .write_all(value.as_bytes())
        .map_err(CommandError::Output)
}

fn invalid_bundle(error: &impl ToString) -> Failure {
    Failure {
        status: Status::InvalidBundle,
        error: CommandError::InvalidBundle(error.to_string()),
    }
}

const fn output_failure(error: CommandError) -> Failure {
    Failure {
        status: Status::ScanFailure,
        error,
    }
}
