use std::path::{Component, Path, PathBuf};

use hanimo_core::{
    CriticVerdict, EvidenceBundle, MAX_VERIFY_BUNDLE_BYTES, VerificationStatus, diagnose,
    model::SCHEMA_VERSION, verify,
};
use rmcp::{
    ErrorData, ServerHandler, ServiceExt, handler::server::wrapper::Parameters,
    model::CallToolResult, tool, tool_handler, tool_router, transport::stdio,
};
use schemars::JsonSchema;
use serde::Deserialize;
use thiserror::Error;

use crate::search_adapter::search_evidence;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SearchEvidenceArgs {
    query: String,
    /// Optional relative subpath beneath the MCP server's startup directory.
    path: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct VerifyEvidenceArgs {
    /// Authoritative `EvidenceBundle` JSON exactly as returned by
    /// `search_evidence` or `hanimo find search --format json`.
    bundle_json: String,
    /// Optional relative subpath beneath the MCP server's startup directory.
    /// The resolved target must equal the bundle's recorded display root.
    path: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct DiagnoseRepoArgs {
    /// Optional relative subpath beneath the MCP server's startup directory.
    path: Option<String>,
}

#[derive(Debug, Clone)]
struct SearchServer {
    base_root: PathBuf,
}

#[tool_router]
impl SearchServer {
    #[tool(
        name = "search_evidence",
        description = "Search local files and return a deterministic evidence bundle"
    )]
    fn search_evidence(
        &self,
        Parameters(arguments): Parameters<SearchEvidenceArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let root = resolve_target(&self.base_root, arguments.path.as_deref())
            .map_err(|error| ErrorData::invalid_params(error.to_string(), None))?;
        match search_evidence(&arguments.query, &root) {
            Ok(bundle) => serde_json::to_value(bundle)
                .map(CallToolResult::structured)
                .map_err(|error| ErrorData::internal_error(error.to_string(), None)),
            Err(error) if error.is_usage() => {
                Err(ErrorData::invalid_params(error.to_string(), None))
            }
            Err(error) => Ok(CallToolResult::structured_error(serde_json::json!({
                "error": error.to_string()
            }))),
        }
    }

    #[tool(
        name = "verify_evidence",
        description = "Reopen the cited source bytes of an evidence bundle and reject stale, forged, or drifted evidence"
    )]
    fn verify_evidence(
        &self,
        Parameters(arguments): Parameters<VerifyEvidenceArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let root = resolve_target(&self.base_root, arguments.path.as_deref())
            .map_err(|error| ErrorData::invalid_params(error.to_string(), None))?;
        if arguments.bundle_json.len() > MAX_VERIFY_BUNDLE_BYTES {
            return Err(ErrorData::invalid_params(
                format!(
                    "evidence bundle exceeds {MAX_VERIFY_BUNDLE_BYTES}-byte verification input limit"
                ),
                None,
            ));
        }
        let bundle: EvidenceBundle = serde_json::from_str(&arguments.bundle_json)
            .map_err(|error| ErrorData::invalid_params(error.to_string(), None))?;
        if bundle.schema_version != SCHEMA_VERSION || bundle.root.is_empty() {
            return Err(ErrorData::invalid_params(
                "unsupported schema or empty root",
                None,
            ));
        }
        if root.to_str() != Some(&bundle.root) {
            return Ok(CallToolResult::structured_error(serde_json::json!({
                "error": "trusted verification root does not match recorded display root"
            })));
        }
        match verify(&root, &bundle) {
            Ok(report) => {
                let accepted = report.status == VerificationStatus::Verified
                    && bundle.critic.verdict == CriticVerdict::Accepted;
                serde_json::to_value(&report)
                    .map(|report| {
                        CallToolResult::structured(serde_json::json!({
                            "accepted": accepted,
                            "report": report
                        }))
                    })
                    .map_err(|error| ErrorData::internal_error(error.to_string(), None))
            }
            Err(error) => {
                if let Some(reason) = error.invalid_bundle_reason() {
                    return Err(ErrorData::invalid_params(reason, None));
                }
                Ok(CallToolResult::structured_error(serde_json::json!({
                    "error": error.to_string()
                })))
            }
        }
    }

    #[tool(
        name = "diagnose_repo",
        description = "Statically diagnose a local repository for the versioned imnotrag RAG-risk rules"
    )]
    fn diagnose_repo(
        &self,
        Parameters(arguments): Parameters<DiagnoseRepoArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let root = resolve_target(&self.base_root, arguments.path.as_deref())
            .map_err(|error| ErrorData::invalid_params(error.to_string(), None))?;
        match diagnose::diagnose(&root) {
            Ok(diagnosis) => serde_json::to_value(diagnosis)
                .map(CallToolResult::structured)
                .map_err(|error| ErrorData::internal_error(error.to_string(), None)),
            Err(error) => Ok(CallToolResult::structured_error(serde_json::json!({
                "error": error.to_string()
            }))),
        }
    }
}

#[tool_handler(name = "hanimo-find", version = "0.1.0")]
impl ServerHandler for SearchServer {}

#[derive(Debug, Error)]
enum TargetError {
    #[error("MCP path must contain only relative normal components")]
    UnsafePath,
}

fn resolve_target(base_root: &Path, requested: Option<&str>) -> Result<PathBuf, TargetError> {
    let Some(requested) = requested else {
        return Ok(base_root.to_path_buf());
    };
    let requested = Path::new(requested);
    if requested.as_os_str().is_empty()
        || requested
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(TargetError::UnsafePath);
    }
    Ok(base_root.join(requested))
}

#[derive(Debug, Error)]
pub(crate) enum McpError {
    #[error("cannot capture MCP startup root: {0}")]
    BaseRoot(String),
    #[error("cannot initialize stdio MCP service: {0}")]
    Initialize(String),
    #[error("stdio MCP service failed: {0}")]
    Serve(String),
}

pub(crate) async fn serve_stdio() -> Result<(), McpError> {
    let base_root = std::env::current_dir()
        .and_then(|root| root.canonicalize())
        .map_err(|error| McpError::BaseRoot(error.to_string()))?;
    let service = SearchServer { base_root }
        .serve(stdio())
        .await
        .map_err(|error| McpError::Initialize(error.to_string()))?;
    service
        .waiting()
        .await
        .map_err(|error| McpError::Serve(error.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::resolve_target;

    #[test]
    fn requested_subpath_is_joined_lexically_without_filesystem_probe() {
        // Given: a trusted base and a normal relative target that does not exist.
        let sandbox = tempfile::TempDir::new().expect("sandbox is created");
        let base = sandbox.path().canonicalize().expect("base canonicalizes");

        // When: the MCP boundary resolves the request path.
        let target = resolve_target(&base, Some("missing/nested"))
            .expect("normal relative path resolves lexically");

        // Then: the boundary delegates the untouched path identity to core acquisition.
        assert_eq!(target, base.join("missing/nested"));
    }
}
