//! Real-binary contract tests for the frozen Hanimo Find CLI.

use std::{
    ffi::OsStr,
    fs::{read, write},
    num::NonZeroUsize,
    path::Path,
    process::{Command, Output},
};

use hanimo_core::{
    EncodedBytes, EvidenceBundle, SkipReason, SkippedEvidence, bundle_sha256,
    identity::{BlockIdentityInput, block_id},
};
use tempfile::TempDir;

use hanimo_core::model::{MAX_QUERY_BYTES, MAX_QUERY_LITERALS};

const MAX_VERIFY_BUNDLE_BYTES: usize = 16_777_216;

fn run<I, S>(cwd: &Path, args: I) -> std::io::Result<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new(env!("CARGO_BIN_EXE_hanimo"))
        .current_dir(cwd)
        .args(args)
        .output()
}

fn stdout_json(output: &Output) -> serde_json::Result<serde_json::Value> {
    serde_json::from_slice(&output.stdout)
}

#[test]
fn help_describes_the_frozen_find_surface() {
    // Given: the installed command-line binary.
    let sandbox = TempDir::new().expect("sandbox is created");

    // When: the nested find help is requested.
    let output = run(sandbox.path(), ["find", "--help"]).expect("hanimo process runs");

    // Then: help succeeds and names every frozen command without protocol noise.
    let stdout = String::from_utf8(output.stdout).expect("help is UTF-8");
    assert!(output.status.success());
    assert!(stdout.contains("search"));
    assert!(stdout.contains("verify"));
    assert!(stdout.contains("diagnose"));
    assert!(stdout.contains("mcp"));
    assert!(output.stderr.is_empty());
}

#[test]
fn search_emits_json_and_markdown_from_the_same_core_bundle() {
    // Given: a local source with one exact literal hit.
    let sandbox = TempDir::new().expect("sandbox is created");
    write(sandbox.path().join("doc.txt"), b"# Heading\nneedle\n").expect("fixture is written");
    let canonical = std::fs::canonicalize(sandbox.path()).expect("sandbox root canonicalizes");
    let path = canonical.as_os_str();

    // When: JSON and Markdown searches run through the binary.
    let json = run(
        sandbox.path(),
        [
            "find".as_ref(),
            "search".as_ref(),
            "needle".as_ref(),
            path,
            "--format".as_ref(),
            "json".as_ref(),
        ],
    )
    .expect("JSON search process runs");
    let markdown = run(
        sandbox.path(),
        [
            "find".as_ref(),
            "search".as_ref(),
            "needle".as_ref(),
            path,
            "--format".as_ref(),
            "md".as_ref(),
        ],
    )
    .expect("Markdown search process runs");

    // Then: both succeed and Markdown embeds the exact authoritative JSON identity.
    let bundle = stdout_json(&json).expect("stdout is JSON");
    let block_id = bundle
        .pointer("/blocks/0/block_id")
        .and_then(serde_json::Value::as_str)
        .expect("block ID is present");
    let markdown = String::from_utf8(markdown.stdout).expect("Markdown is UTF-8");
    assert!(json.status.success());
    assert!(markdown.contains(block_id));
    assert!(json.stderr.is_empty());
}

#[test]
fn no_hit_and_invalid_inputs_use_frozen_exit_codes() {
    // Given: an empty searchable directory and a missing path.
    let sandbox = TempDir::new().expect("sandbox is created");
    let missing = sandbox.path().join("missing");

    // When: no-hit, empty-query, and missing-root searches run.
    let no_hit = run(
        sandbox.path(),
        ["find", "search", "absent", ".", "--format", "json"],
    )
    .expect("no-hit search process runs");
    let invalid_query = run(
        sandbox.path(),
        ["find", "search", "", ".", "--format", "json"],
    )
    .expect("invalid-query process runs");
    let invalid_path = run(
        sandbox.path(),
        [
            "find".as_ref(),
            "search".as_ref(),
            "needle".as_ref(),
            missing.as_os_str(),
            "--format".as_ref(),
            "json".as_ref(),
        ],
    )
    .expect("invalid-path process runs");

    // Then: critic rejection is 1, usage is 2, and scan failure is 5.
    assert_eq!(no_hit.status.code(), Some(1));
    assert_eq!(
        stdout_json(&no_hit)
            .expect("no-hit stdout is JSON")
            .pointer("/critic/verdict"),
        Some(&serde_json::json!("rejected"))
    );
    assert_eq!(invalid_query.status.code(), Some(2));
    assert!(invalid_query.stdout.is_empty());
    assert_eq!(invalid_path.status.code(), Some(5));
    assert!(invalid_path.stdout.is_empty());
}

#[test]
fn search_rejects_query_bytes_above_the_hard_limit_before_scanning() {
    // Given: exact-boundary and one-byte-over queries plus a missing root.
    let sandbox = TempDir::new().expect("sandbox is created");
    let accepted_query = "a".repeat(MAX_QUERY_BYTES);
    let rejected_query = "a".repeat(MAX_QUERY_BYTES + 1);
    let missing = sandbox.path().join("missing");
    let missing = missing.to_str().expect("temporary path is UTF-8");

    // When: both queries run through the real CLI boundary.
    let accepted = run(
        sandbox.path(),
        [
            "find",
            "search",
            accepted_query.as_str(),
            ".",
            "--format",
            "json",
        ],
    )
    .expect("boundary query runs");
    let rejected = run(
        sandbox.path(),
        [
            "find",
            "search",
            rejected_query.as_str(),
            missing,
            "--format",
            "json",
        ],
    )
    .expect("over-limit query runs");

    // Then: the maximum is admitted and max+1 is a typed usage failure before root I/O.
    assert_eq!(accepted.status.code(), Some(1));
    assert_eq!(rejected.status.code(), Some(2));
    assert!(rejected.stdout.is_empty());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("query exceeds 4096 bytes"));
}

#[test]
fn search_applies_query_byte_limits_to_multibyte_utf8() {
    // Given: UTF-8 queries at exactly 4,096 bytes and one byte above it.
    let sandbox = TempDir::new().expect("sandbox is created");
    let accepted_query = format!("{}a", "가".repeat(1_365));
    let rejected_query = format!("{accepted_query}b");
    assert_eq!(accepted_query.len(), MAX_QUERY_BYTES);
    assert_eq!(rejected_query.len(), MAX_QUERY_BYTES + 1);

    // When: both queries run through the real CLI boundary.
    let accepted = run(
        sandbox.path(),
        [
            "find",
            "search",
            accepted_query.as_str(),
            ".",
            "--format",
            "json",
        ],
    )
    .expect("boundary UTF-8 query runs");
    let rejected = run(
        sandbox.path(),
        [
            "find",
            "search",
            rejected_query.as_str(),
            ".",
            "--format",
            "json",
        ],
    )
    .expect("over-limit UTF-8 query runs");

    // Then: byte length, not Unicode scalar count, controls admission.
    assert_eq!(accepted.status.code(), Some(1));
    assert_eq!(rejected.status.code(), Some(2));
}

#[test]
fn search_rejects_literal_count_above_the_hard_limit() {
    // Given: exact-boundary and one-literal-over ordinary queries.
    let sandbox = TempDir::new().expect("sandbox is created");
    let accepted_query = vec!["a"; MAX_QUERY_LITERALS].join(" ");
    let rejected_query = vec!["a"; MAX_QUERY_LITERALS + 1].join(" ");

    // When: both queries run through the real CLI boundary.
    let accepted = run(
        sandbox.path(),
        [
            "find",
            "search",
            accepted_query.as_str(),
            ".",
            "--format",
            "json",
        ],
    )
    .expect("boundary literal query runs");
    let rejected = run(
        sandbox.path(),
        [
            "find",
            "search",
            rejected_query.as_str(),
            ".",
            "--format",
            "json",
        ],
    )
    .expect("over-limit literal query runs");

    // Then: the maximum is admitted and max+1 is a typed usage failure.
    assert_eq!(accepted.status.code(), Some(1));
    assert_eq!(rejected.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("query exceeds 64 literals"));
}

#[cfg(unix)]
#[test]
fn search_cli_rejects_a_symlink_root_as_a_scan_failure() {
    use std::os::unix::fs::symlink;

    // Given: a CLI root symlink targeting a matching outside file.
    let sandbox = TempDir::new().expect("sandbox is created");
    let outside = sandbox.path().join("outside");
    std::fs::create_dir(&outside).expect("outside directory is created");
    write(
        outside.join("sentinel.txt"),
        b"needle ROOT_SYMLINK_SENTINEL\n",
    )
    .expect("sentinel is written");
    let linked_root = sandbox.path().join("linked-root");
    symlink(&outside, &linked_root).expect("root symlink is created");

    // When: the real binary searches the linked root.
    let output = run(
        sandbox.path(),
        [
            "find".as_ref(),
            "search".as_ref(),
            "needle".as_ref(),
            linked_root.as_os_str(),
            "--format".as_ref(),
            "json".as_ref(),
        ],
    )
    .expect("search process runs");

    // Then: it exits 5 without emitting target evidence.
    assert_eq!(output.status.code(), Some(5));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("search root must not be a symbolic link"));
    assert!(!stderr.contains("ROOT_SYMLINK_SENTINEL"));
}

#[cfg(unix)]
#[test]
fn search_cli_rejects_an_intermediate_symlink_root() {
    use std::os::unix::fs::symlink;

    // Given: a relative CLI root whose intermediate component links outside.
    let sandbox = TempDir::new().expect("sandbox is created");
    let base = sandbox.path().join("base");
    let outside_child = sandbox.path().join("outside/child");
    std::fs::create_dir_all(&base).expect("base directory is created");
    std::fs::create_dir_all(&outside_child).expect("outside child is created");
    write(
        outside_child.join("sentinel.txt"),
        b"needle OUTSIDE_INTERMEDIATE_ROOT_SENTINEL\n",
    )
    .expect("sentinel is written");
    symlink("../outside", base.join("link")).expect("intermediate symlink is created");

    // When: the real binary searches through the relative linked component.
    let output = run(
        sandbox.path(),
        [
            "find",
            "search",
            "needle",
            "base/link/child",
            "--format",
            "json",
        ],
    )
    .expect("search process runs");

    // Then: it exits 5 without exposing outside evidence.
    assert_eq!(output.status.code(), Some(5));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("search root must not be a symbolic link"));
    assert!(!stderr.contains("OUTSIDE_INTERMEDIATE_ROOT_SENTINEL"));
}

#[test]
fn verify_distinguishes_valid_stale_forged_and_invalid_bundles() {
    // Given: a searched bundle plus malformed and forged bundle files.
    let sandbox = TempDir::new().expect("sandbox is created");
    let source = sandbox.path().join("doc.txt");
    let bundle_path = sandbox.path().join("bundle.json");
    let forged_path = sandbox.path().join("forged.json");
    let invalid_path = sandbox.path().join("invalid.json");
    write(&source, b"needle\n").expect("fixture is written");
    let search = run(
        sandbox.path(),
        ["find", "search", "needle", ".", "--format", "json"],
    )
    .expect("search process runs");
    write(&bundle_path, &search.stdout).expect("bundle is stored");
    let mut forged = stdout_json(&search).expect("search stdout is JSON");
    *forged
        .pointer_mut("/blocks/0/content/text")
        .expect("block content exists") = "forged".into();
    write(
        &forged_path,
        serde_json::to_vec(&forged).expect("forged bundle serializes"),
    )
    .expect("forged bundle is stored");
    write(&invalid_path, b"{not-json").expect("invalid bundle is stored");

    // When: valid, stale, forged, and malformed bundles are verified.
    let valid = run(
        sandbox.path(),
        ["find".as_ref(), "verify".as_ref(), bundle_path.as_os_str()],
    )
    .expect("valid verification process runs");
    write(&source, b"changed\n").expect("fixture is changed");
    let stale = run(
        sandbox.path(),
        ["find".as_ref(), "verify".as_ref(), bundle_path.as_os_str()],
    )
    .expect("drift verification process runs");
    let forged = run(
        sandbox.path(),
        ["find".as_ref(), "verify".as_ref(), forged_path.as_os_str()],
    )
    .expect("forged verification process runs");
    let invalid = run(
        sandbox.path(),
        ["find".as_ref(), "verify".as_ref(), invalid_path.as_os_str()],
    )
    .expect("invalid verification process runs");

    // Then: verified is 0, all evidence mismatch is 4, and malformed JSON is 3.
    assert!(valid.status.success());
    assert_eq!(
        stdout_json(&valid)
            .expect("valid stdout is JSON")
            .get("status"),
        Some(&serde_json::json!("verified"))
    );
    assert_eq!(stale.status.code(), Some(4));
    assert_eq!(
        stdout_json(&stale)
            .expect("drift stdout is JSON")
            .get("status"),
        Some(&serde_json::json!("source_drift"))
    );
    assert_eq!(forged.status.code(), Some(4));
    assert_eq!(
        stdout_json(&forged)
            .expect("forged stdout is JSON")
            .get("status"),
        Some(&serde_json::json!("forged"))
    );
    assert_eq!(invalid.status.code(), Some(3));
    assert!(invalid.stdout.is_empty());
    assert!(
        !read(invalid_path)
            .expect("invalid input remains")
            .is_empty()
    );
}

#[test]
fn verify_classifies_self_consistent_root_escape_locators_as_security_failures() {
    // Given: valid evidence rewritten with self-consistent traversal and absolute locators.
    let sandbox = TempDir::new().expect("sandbox is created");
    write(sandbox.path().join("doc.txt"), b"needle\n").expect("fixture is written");
    let search = run(
        sandbox.path(),
        ["find", "search", "needle", ".", "--format", "json"],
    )
    .expect("search process runs");
    let original: EvidenceBundle =
        serde_json::from_slice(&search.stdout).expect("search stdout is a bundle");

    for (index, locator) in ["../outside.txt", "/absolute.txt", "dir/../../outside.txt"]
        .into_iter()
        .enumerate()
    {
        let mut bundle = original.clone();
        let block = bundle.blocks.first_mut().expect("fixture has a block");
        let content = match &block.content {
            EncodedBytes::Utf8 { text } => text.as_bytes(),
            EncodedBytes::Base64 { .. } => panic!("fixture content is UTF-8"),
        };
        block.block_id = block_id(BlockIdentityInput {
            path: locator.as_bytes(),
            line_start: block.line_start,
            line_end: block.line_end,
            content,
        })
        .expect("block digest is recomputed");
        block.path = EncodedBytes::Utf8 {
            text: locator.to_owned(),
        };
        bundle.bundle_sha256 = bundle_sha256(&bundle).expect("bundle digest is recomputed");
        let path = sandbox.path().join(format!("unsafe-{index}.json"));
        write(
            &path,
            serde_json::to_vec(&bundle).expect("bundle serializes"),
        )
        .expect("bundle is stored");

        // When: the real CLI verifies the unsafe but self-consistent locator.
        let output = run(
            sandbox.path(),
            ["find".as_ref(), "verify".as_ref(), path.as_os_str()],
        )
        .expect("verification process runs");

        // Then: it is a security/I/O exit, never forged/stale evidence mismatch.
        assert_eq!(output.status.code(), Some(5), "locator: {locator}");
        assert!(output.stdout.is_empty(), "locator: {locator}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("unsafe evidence path"),
            "locator: {locator}"
        );
    }
}

#[test]
fn verify_maps_critic_and_policy_violations_to_frozen_exit_classes() {
    // Given: one valid bundle rewritten as a self-consistent critic contradiction and two over-limit artifacts.
    let sandbox = TempDir::new().expect("sandbox is created");
    write(sandbox.path().join("doc.txt"), b"needle\n").expect("fixture is written");
    let search = run(
        sandbox.path(),
        ["find", "search", "needle", ".", "--format", "json"],
    )
    .expect("search process runs");
    let original: EvidenceBundle =
        serde_json::from_slice(&search.stdout).expect("search stdout is a bundle");

    let mut critic_gap = original.clone();
    critic_gap.skipped.push(SkippedEvidence {
        path: EncodedBytes::Utf8 {
            text: "omitted.txt".to_owned(),
        },
        reason: SkipReason::Budget,
    });
    critic_gap.bundle_sha256 = bundle_sha256(&critic_gap).expect("bundle digest is recomputed");

    let mut numeric_limit = original.clone();
    numeric_limit.budget.max_file_bytes =
        NonZeroUsize::new(16_777_217).expect("test limit is nonzero");

    let mut array_limit = original;
    let block = array_limit
        .blocks
        .first()
        .expect("fixture has a block")
        .clone();
    array_limit.blocks = vec![block; 65];
    array_limit.budget.max_blocks = NonZeroUsize::new(64).expect("test limit is nonzero");
    array_limit.bundle_sha256 = bundle_sha256(&array_limit).expect("bundle digest is recomputed");

    let cases = [
        ("critic-gap.json", critic_gap),
        ("numeric-limit.json", numeric_limit),
        ("array-limit.json", array_limit),
    ];
    let mut outputs = Vec::new();
    for (name, bundle) in cases {
        let path = sandbox.path().join(name);
        write(
            &path,
            serde_json::to_vec(&bundle).expect("bundle serializes"),
        )
        .expect("bundle is stored");
        outputs.push(
            run(
                sandbox.path(),
                ["find".as_ref(), "verify".as_ref(), path.as_os_str()],
            )
            .expect("verification process runs"),
        );
    }

    // When/Then: semantic forgery remains exit 4 while structural resource policy is invalid-bundle exit 3.
    let [critic_gap_output, numeric_limit_output, array_limit_output] = outputs.as_slice() else {
        panic!("three verification outputs are collected");
    };
    assert_eq!(critic_gap_output.status.code(), Some(4));
    assert_eq!(
        stdout_json(critic_gap_output)
            .expect("critic-gap stdout is JSON")
            .get("status"),
        Some(&serde_json::json!("forged"))
    );
    assert_eq!(numeric_limit_output.status.code(), Some(3));
    assert_eq!(array_limit_output.status.code(), Some(3));
    assert!(numeric_limit_output.stdout.is_empty());
    assert!(array_limit_output.stdout.is_empty());
}

#[test]
fn verify_bounds_bundle_bytes_before_json_deserialization() {
    // Given: one valid bundle padded to the exact input envelope and one byte beyond it.
    let sandbox = TempDir::new().expect("sandbox is created");
    write(sandbox.path().join("doc.txt"), b"needle\n").expect("fixture is written");
    let search = run(
        sandbox.path(),
        ["find", "search", "needle", ".", "--format", "json"],
    )
    .expect("search process runs");
    let exact_path = sandbox.path().join("exact-envelope.json");
    let oversized_path = sandbox.path().join("oversized-envelope.json");
    let mut bytes = search.stdout;
    bytes.resize(MAX_VERIFY_BUNDLE_BYTES, b' ');
    write(&exact_path, &bytes).expect("exact envelope is stored");
    bytes.push(b' ');
    write(&oversized_path, bytes).expect("oversized envelope is stored");

    // When: both files are verified through the real CLI input path.
    let exact = run(
        sandbox.path(),
        ["find".as_ref(), "verify".as_ref(), exact_path.as_os_str()],
    )
    .expect("exact verification process runs");
    let oversized = run(
        sandbox.path(),
        [
            "find".as_ref(),
            "verify".as_ref(),
            oversized_path.as_os_str(),
        ],
    )
    .expect("oversized verification process runs");

    // Then: exact input is processed and over-limit input fails before the JSON parser.
    assert!(exact.status.success());
    assert_eq!(oversized.status.code(), Some(3));
    assert!(oversized.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&oversized.stderr)
            .contains("evidence bundle exceeds 16777216-byte verification input limit")
    );
}

#[test]
fn verify_reports_stale_when_the_recorded_source_disappears() {
    // Given: a valid stored bundle whose recorded source still exists.
    let sandbox = TempDir::new().expect("sandbox is created");
    let source = sandbox.path().join("doc.txt");
    let bundle_path = sandbox.path().join("bundle.json");
    write(&source, b"needle\n").expect("fixture is written");
    let search = run(
        sandbox.path(),
        ["find", "search", "needle", ".", "--format", "json"],
    )
    .expect("search process runs");
    write(&bundle_path, search.stdout).expect("bundle is stored");
    std::fs::remove_file(source).expect("recorded source is removed");

    // When: the stored evidence is verified after path removal.
    let output = run(
        sandbox.path(),
        ["find".as_ref(), "verify".as_ref(), bundle_path.as_os_str()],
    )
    .expect("stale verification process runs");

    // Then: the binary reports stale evidence with frozen exit 4.
    assert_eq!(output.status.code(), Some(4));
    assert_eq!(
        stdout_json(&output)
            .expect("stale stdout is JSON")
            .get("status"),
        Some(&serde_json::json!("stale"))
    );
}

#[test]
fn diagnose_supports_authoritative_json_and_pure_markdown() {
    // Given: a source file containing an inspectable vector-store dependency.
    let sandbox = TempDir::new().expect("sandbox is created");
    write(sandbox.path().join("app.py"), b"import chromadb\n").expect("fixture is written");
    let canonical = std::fs::canonicalize(sandbox.path()).expect("sandbox root canonicalizes");
    let path = canonical.as_os_str();

    // When: diagnosis is requested in JSON and Markdown forms.
    let json = run(
        sandbox.path(),
        [
            "find".as_ref(),
            "diagnose".as_ref(),
            path,
            "--format".as_ref(),
            "json".as_ref(),
        ],
    )
    .expect("JSON diagnosis process runs");
    let markdown = run(
        sandbox.path(),
        [
            "find".as_ref(),
            "diagnose".as_ref(),
            path,
            "--format".as_ref(),
            "md".as_ref(),
        ],
    )
    .expect("Markdown diagnosis process runs");

    // Then: both succeed and carry the same source-cited rule identity.
    let diagnosis = stdout_json(&json).expect("diagnosis stdout is JSON");
    let markdown = String::from_utf8(markdown.stdout).expect("Markdown is UTF-8");
    assert!(json.status.success());
    assert!(markdown.contains("RAG001_VECTOR_STORE_DEPENDENCY"));
    assert_eq!(
        diagnosis.pointer("/findings/0/citations/0/path"),
        Some(&serde_json::json!("app.py"))
    );
}
