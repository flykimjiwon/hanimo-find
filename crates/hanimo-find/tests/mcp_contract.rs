//! Literal stdio JSON-RPC contract tests for the official rmcp adapter.

use std::{
    fs::write,
    io::Write as _,
    path::Path,
    process::{Command, Output, Stdio},
};

use serde_json::{Value, json};
use tempfile::TempDir;

fn run_with_stdin(cwd: &Path, args: &[&str], input: &[u8]) -> std::io::Result<Output> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_hanimo"))
        .current_dir(cwd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| std::io::Error::other("stdin pipe is unavailable"))?;
    stdin.write_all(input)?;
    drop(stdin);
    child.wait_with_output()
}

#[test]
fn stdio_mcp_initializes_lists_and_calls_the_single_search_tool() {
    // Given: a local source and literal initialize/list/call JSON-RPC frames.
    let sandbox = TempDir::new().expect("sandbox is created");
    write(sandbox.path().join("doc.txt"), b"needle\n").expect("fixture is written");
    let frames = [
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "hanimo-contract", "version": "0.1.0"}
            }
        }),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}),
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "search_evidence",
                "arguments": {"query": "needle"}
            }
        }),
    ];
    let input = frames
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";

    // When: the frames are sent to the real stdio server until EOF.
    let output = run_with_stdin(sandbox.path(), &["find", "mcp"], input.as_bytes())
        .expect("stdio MCP process runs");

    // Then: stdout is protocol-only and exposes one tool backed by a typed evidence bundle.
    let responses = output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(serde_json::from_slice::<Value>)
        .collect::<Result<Vec<_>, _>>()
        .expect("every stdout line is a JSON-RPC frame");
    let initialized = responses
        .iter()
        .find(|value| value.get("id").and_then(Value::as_i64) == Some(1))
        .expect("initialize response");
    let listed = responses
        .iter()
        .find(|value| value.get("id").and_then(Value::as_i64) == Some(2))
        .expect("list response");
    let called = responses
        .iter()
        .find(|value| value.get("id").and_then(Value::as_i64) == Some(3))
        .expect("call response");
    assert_eq!(
        initialized.pointer("/result/serverInfo/name"),
        Some(&json!("hanimo-find"))
    );
    let mut tool_names: Vec<&str> = listed
        .pointer("/result/tools")
        .and_then(Value::as_array)
        .expect("tools array")
        .iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str))
        .collect();
    tool_names.sort_unstable();
    assert_eq!(
        tool_names,
        ["diagnose_repo", "search_evidence", "verify_evidence"]
    );
    assert_eq!(
        called.pointer("/result/structuredContent/critic/verdict"),
        Some(&json!("accepted"))
    );
    assert_eq!(
        called.pointer("/result/structuredContent/blocks/0/content/text"),
        Some(&json!("needle"))
    );
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}

#[cfg(unix)]
#[test]
fn stdio_mcp_rejects_parent_absolute_and_symlink_roots() {
    use std::{fs::create_dir, os::unix::fs::symlink};

    // Given: the server starts inside a trusted base with an external sentinel sibling.
    let sandbox = TempDir::new().expect("sandbox is created");
    let base = sandbox.path().join("base");
    let outside = sandbox.path().join("outside");
    create_dir(&base).expect("base is created");
    create_dir(&outside).expect("outside is created");
    write(outside.join("outside.txt"), b"needle OUTSIDE_SENTINEL\n")
        .expect("outside fixture is written");
    create_dir(outside.join("nested")).expect("outside nested directory is created");
    write(
        outside.join("nested/outside.txt"),
        b"needle OUTSIDE_SENTINEL\n",
    )
    .expect("outside nested fixture is written");
    symlink(&outside, base.join("linked-outside")).expect("external directory link is created");
    let frames = request_frames([
        (3, "..".to_owned()),
        (4, outside.to_string_lossy().into_owned()),
        (5, "linked-outside".to_owned()),
        (6, "linked-outside/nested".to_owned()),
    ]);

    // When: one MCP session submits all three hostile root selectors.
    let output =
        run_with_stdin(&base, &["find", "mcp"], frames.as_bytes()).expect("stdio MCP process runs");
    let responses = parse_responses(&output).expect("every stdout line is a JSON-RPC frame");

    // Then: unsafe syntax is rejected at the boundary and core rejects both symlink shapes.
    for id in [3, 4] {
        let response = response_by_id(&responses, id).expect("response id is present");
        assert!(response.get("error").is_some(), "request {id} must fail");
    }
    for id in [5, 6] {
        let response = response_by_id(&responses, id).expect("response id is present");
        assert_eq!(
            response.pointer("/result/isError"),
            Some(&json!(true)),
            "request {id} must be rejected: {response}"
        );
    }
    assert!(!String::from_utf8_lossy(&output.stdout).contains("OUTSIDE_SENTINEL"));
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}

#[test]
fn stdio_mcp_nested_relative_path_matches_cli_search() {
    // Given: a nested source beneath the MCP process startup directory.
    let sandbox = TempDir::new().expect("sandbox is created");
    let nested = sandbox.path().join("nested");
    std::fs::create_dir(&nested).expect("nested root is created");
    write(nested.join("doc.txt"), b"needle\n").expect("nested fixture is written");
    let nested = nested.canonicalize().expect("nested root canonicalizes");
    let frames = request_frames([(3, "nested".to_owned())]);

    // When: MCP uses a relative subpath and CLI searches the resolved absolute target.
    let mcp = run_with_stdin(sandbox.path(), &["find", "mcp"], frames.as_bytes())
        .expect("stdio MCP process runs");
    let cli = Command::new(env!("CARGO_BIN_EXE_hanimo"))
        .current_dir(sandbox.path())
        .args([
            "find".as_ref(),
            "search".as_ref(),
            "needle".as_ref(),
            nested.as_os_str(),
            "--format".as_ref(),
            "json".as_ref(),
        ])
        .output()
        .expect("CLI search process runs");

    // Then: MCP returns the same authoritative evidence bundle as CLI.
    let responses = parse_responses(&mcp).expect("every stdout line is a JSON-RPC frame");
    let mcp_bundle = response_by_id(&responses, 3)
        .expect("response id is present")
        .pointer("/result/structuredContent")
        .expect("MCP structured bundle");
    let cli_bundle: Value = serde_json::from_slice(&cli.stdout).expect("CLI bundle is JSON");
    assert_eq!(mcp_bundle, &cli_bundle);
    assert!(mcp.status.success());
    assert!(mcp.stderr.is_empty());
    assert!(cli.status.success());
    assert!(cli.stderr.is_empty());
}

#[test]
fn stdio_mcp_verify_accepts_a_fresh_search_bundle_and_rejects_a_stale_one() {
    // Given: an authoritative bundle produced by one MCP search session.
    let sandbox = TempDir::new().expect("sandbox is created");
    write(sandbox.path().join("doc.txt"), b"needle\n").expect("fixture is written");
    let search = run_with_stdin(
        sandbox.path(),
        &["find", "mcp"],
        tool_call_frames(3, "search_evidence", &json!({"query": "needle"})).as_bytes(),
    )
    .expect("stdio MCP search process runs");
    let responses = parse_responses(&search).expect("every stdout line is a JSON-RPC frame");
    let bundle = response_by_id(&responses, 3)
        .expect("search response is present")
        .pointer("/result/structuredContent")
        .expect("search bundle is structured")
        .to_string();

    // When: the bundle is verified before and after its source disappears.
    let fresh = run_with_stdin(
        sandbox.path(),
        &["find", "mcp"],
        tool_call_frames(
            4,
            "verify_evidence",
            &json!({"bundle_json": bundle.as_str()}),
        )
        .as_bytes(),
    )
    .expect("stdio MCP verify process runs");
    std::fs::remove_file(sandbox.path().join("doc.txt")).expect("source is removed");
    let stale = run_with_stdin(
        sandbox.path(),
        &["find", "mcp"],
        tool_call_frames(
            5,
            "verify_evidence",
            &json!({"bundle_json": bundle.as_str()}),
        )
        .as_bytes(),
    )
    .expect("stdio MCP stale-verify process runs");

    // Then: live bytes verify as accepted and the missing source is stale, never accepted.
    let fresh_responses = parse_responses(&fresh).expect("fresh frames parse");
    let fresh_result = response_by_id(&fresh_responses, 4).expect("fresh response is present");
    assert_eq!(
        fresh_result.pointer("/result/structuredContent/accepted"),
        Some(&json!(true))
    );
    assert_eq!(
        fresh_result.pointer("/result/structuredContent/report/status"),
        Some(&json!("verified"))
    );
    let stale_responses = parse_responses(&stale).expect("stale frames parse");
    let stale_result = response_by_id(&stale_responses, 5).expect("stale response is present");
    assert_eq!(
        stale_result.pointer("/result/structuredContent/accepted"),
        Some(&json!(false))
    );
    assert_eq!(
        stale_result.pointer("/result/structuredContent/report/status"),
        Some(&json!("stale"))
    );
    for output in [&search, &fresh, &stale] {
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn stdio_mcp_diagnose_reports_versioned_rules_for_the_startup_root() {
    // Given: a repository with one inspectable RAG risk beneath the startup root.
    let sandbox = TempDir::new().expect("sandbox is created");
    write(sandbox.path().join("app.py"), b"import chromadb\n").expect("fixture is written");

    // When: the MCP session diagnoses its trusted base without a subpath.
    let output = run_with_stdin(
        sandbox.path(),
        &["find", "mcp"],
        tool_call_frames(3, "diagnose_repo", &json!({})).as_bytes(),
    )
    .expect("stdio MCP diagnose process runs");

    // Then: the structured diagnosis carries the frozen schema and a cited finding.
    let responses = parse_responses(&output).expect("every stdout line is a JSON-RPC frame");
    let called = response_by_id(&responses, 3).expect("diagnose response is present");
    assert_eq!(
        called.pointer("/result/structuredContent/schema_version"),
        Some(&json!("0.1.0"))
    );
    assert_eq!(
        called.pointer("/result/structuredContent/findings/0/citations/0/path"),
        Some(&json!("app.py"))
    );
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}

fn tool_call_frames(id: i64, tool: &str, arguments: &Value) -> String {
    let frames = [
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "hanimo-contract", "version": "0.1.0"}
            }
        }),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {"name": tool, "arguments": arguments}
        }),
    ];
    frames
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn request_frames<const N: usize>(calls: [(i64, String); N]) -> String {
    let mut frames = vec![
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "hanimo-contract", "version": "0.1.0"}
            }
        }),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
    ];
    frames.extend(calls.map(|(id, path)| {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "name": "search_evidence",
                "arguments": {"query": "needle", "path": path}
            }
        })
    }));
    frames
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn parse_responses(output: &Output) -> Result<Vec<Value>, serde_json::Error> {
    output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(serde_json::from_slice::<Value>)
        .collect()
}

fn response_by_id(responses: &[Value], id: i64) -> Option<&Value> {
    responses
        .iter()
        .find(|value| value.get("id").and_then(Value::as_i64) == Some(id))
}
