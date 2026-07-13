//! Real-binary structured search-gap contract.

use std::{ffi::OsStr, fs, path::Path, process::Command};

use tempfile::TempDir;

fn run<I, S>(cwd: &Path, args: I) -> std::io::Result<std::process::Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new(env!("CARGO_BIN_EXE_hanimo"))
        .current_dir(cwd)
        .args(args)
        .output()
}

#[test]
fn cli_json_and_markdown_disclose_secret_and_oversized_gaps() {
    // Given: a public hit plus secret-like and oversized discovered files.
    let sandbox = TempDir::new().expect("sandbox is created");
    fs::write(sandbox.path().join("public.txt"), b"needle\n").expect("public file is written");
    fs::write(sandbox.path().join("service-token.txt"), b"needle\n")
        .expect("secret-like file is written");
    fs::write(sandbox.path().join("z-large.txt"), vec![b'x'; 1_048_577])
        .expect("oversized file is written");

    // When: both public output formats search the same corpus.
    let json = run(
        sandbox.path(),
        ["find", "search", "needle", ".", "--format", "json"],
    )
    .expect("JSON search runs");
    let markdown_output = run(
        sandbox.path(),
        ["find", "search", "needle", ".", "--format", "md"],
    )
    .expect("Markdown search runs");

    // Then: both succeed and expose the same typed policy gaps.
    let bundle: serde_json::Value = serde_json::from_slice(&json.stdout).expect("stdout is JSON");
    let markdown = String::from_utf8(markdown_output.stdout).expect("Markdown is UTF-8");
    assert!(json.status.success());
    assert!(bundle.pointer("/blocks/0").is_some());
    assert_eq!(
        bundle.pointer("/skipped/0/reason"),
        Some(&serde_json::json!("secret"))
    );
    assert_eq!(
        bundle.pointer("/skipped/1/reason"),
        Some(&serde_json::json!("oversized"))
    );
    assert!(markdown_output.status.success());
    assert!(markdown.contains("\"reason\": \"secret\""));
    assert!(markdown.contains("\"reason\": \"oversized\""));
}
