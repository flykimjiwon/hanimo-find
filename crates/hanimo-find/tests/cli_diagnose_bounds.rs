//! CLI coverage for fail-closed diagnosis resource bounds.

use std::{
    fs, io,
    process::{Command, Output},
};

use tempfile::TempDir;

fn run(cwd: &std::path::Path, args: &[&std::ffi::OsStr]) -> io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_hanimo"))
        .current_dir(cwd)
        .args(args)
        .output()
}

#[test]
fn diagnose_cli_exits_five_when_a_source_exceeds_its_budget() {
    // Given: a local diagnosis root containing a source just over one MiB.
    let sandbox = TempDir::new().expect("sandbox is created");
    fs::write(sandbox.path().join("oversized.py"), vec![b'x'; 1_048_577])
        .expect("oversized fixture is written");

    // When: the real CLI diagnoses the root.
    let output = run(
        sandbox.path(),
        &["find".as_ref(), "diagnose".as_ref(), ".".as_ref()],
    )
    .expect("diagnose process runs");

    // Then: the bounded scan fails through the frozen security/I/O exit.
    assert_eq!(output.status.code(), Some(5));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("per-file byte limit"));
}

#[cfg(unix)]
#[test]
fn diagnose_cli_rejects_a_symlink_root_without_disclosing_target() {
    use std::os::unix::fs::symlink;

    // Given: a diagnosis root symlink targets a directory with an external sentinel.
    let sandbox = TempDir::new().expect("sandbox is created");
    let outside = sandbox.path().join("outside");
    fs::create_dir(&outside).expect("outside directory is created");
    fs::write(
        outside.join("pipeline.py"),
        b"vectors = embeddings.create(input=documents) # DIAGNOSE_OUTSIDE_SENTINEL\n",
    )
    .expect("outside source is written");
    symlink(&outside, sandbox.path().join("linked-outside"))
        .expect("diagnosis root symlink is created");

    // When: the real CLI diagnoses the symlink root.
    let output = run(
        sandbox.path(),
        &[
            "find".as_ref(),
            "diagnose".as_ref(),
            "linked-outside".as_ref(),
        ],
    )
    .expect("diagnose process runs");

    // Then: root acquisition fails closed before external content is emitted.
    assert_eq!(output.status.code(), Some(5));
    assert!(output.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("DIAGNOSE_OUTSIDE_SENTINEL"));
}
