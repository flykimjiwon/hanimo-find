//! Real-binary verification authority and error-class contract tests.

use std::{
    error::Error,
    ffi::OsStr,
    fs::write,
    path::Path,
    process::{Command, Output},
};

use hanimo_core::{EvidenceBundle, bundle_sha256};
use tempfile::TempDir;

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

fn search(cwd: &Path, root: &Path) -> Result<EvidenceBundle, Box<dyn Error>> {
    let output = run(
        cwd,
        [
            "find".as_ref(),
            "search".as_ref(),
            "needle".as_ref(),
            root.as_os_str(),
            "--format".as_ref(),
            "json".as_ref(),
        ],
    )?;
    if !output.status.success() {
        return Err(std::io::Error::other(String::from_utf8_lossy(&output.stderr)).into());
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn store(path: &Path, bundle: &EvidenceBundle) -> Result<(), Box<dyn Error>> {
    Ok(write(path, serde_json::to_vec(bundle)?)?)
}

#[test]
fn critic_rejected_bundle_keeps_integrity_report_but_exits_rejected() {
    // Given: the frozen internally consistent budget-rejected evidence fixture.
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture = repository.join("conformance/evidence-bundle.budget-rejected.json");

    // When: the real CLI re-attests it against the caller's current root.
    let output = run(
        &repository,
        ["find".as_ref(), "verify".as_ref(), fixture.as_os_str()],
    )
    .expect("verification process runs");

    // Then: live integrity remains verified while public evidence acceptance is exit 1.
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("verification stdout is JSON");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(report.get("status"), Some(&serde_json::json!("verified")));
    assert!(output.stderr.is_empty());
}

#[test]
fn artifact_root_cannot_retarget_verification_reads() {
    // Given: valid evidence whose unauthenticated display root is retargeted outside the caller root.
    let sandbox = TempDir::new().expect("sandbox is created");
    let authorized = sandbox.path().join("authorized");
    let outside = sandbox.path().join("outside");
    std::fs::create_dir_all(&authorized).expect("authorized root is created");
    std::fs::create_dir_all(&outside).expect("outside root is created");
    write(authorized.join("doc.txt"), b"needle\n").expect("authorized source is written");
    write(outside.join("doc.txt"), b"needle\n").expect("outside source is written");
    let mut bundle = search(&authorized, Path::new(".")).expect("search succeeds");
    let digest = bundle.bundle_sha256.clone();
    bundle.root = outside.to_str().expect("outside path is UTF-8").to_owned();
    assert_eq!(bundle_sha256(&bundle).expect("digest recomputes"), digest);
    let bundle_path = authorized.join("retargeted.json");
    store(&bundle_path, &bundle).expect("bundle is stored");

    // When: verification runs without granting an explicit outside capability.
    let output = run(
        &authorized,
        ["find".as_ref(), "verify".as_ref(), bundle_path.as_os_str()],
    )
    .expect("verification process runs");

    // Then: metadata mismatch is exit 5 before any artifact-selected root is opened.
    assert_eq!(output.status.code(), Some(5));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("verification root"));
}

#[test]
fn explicit_matching_trusted_root_verifies() {
    // Given: evidence created from a caller-selected absolute source root.
    let sandbox = TempDir::new().expect("sandbox is created");
    let trusted = sandbox.path().join("trusted");
    std::fs::create_dir_all(&trusted).expect("trusted root is created");
    write(trusted.join("doc.txt"), b"needle\n").expect("source is written");
    let trusted = std::fs::canonicalize(trusted).expect("trusted root canonicalizes");
    let bundle = search(sandbox.path(), &trusted).expect("search succeeds");
    let bundle_path = sandbox.path().join("bundle.json");
    store(&bundle_path, &bundle).expect("bundle is stored");

    // When: the same root is explicitly supplied as verification authority.
    let output = run(
        sandbox.path(),
        [
            "find".as_ref(),
            "verify".as_ref(),
            bundle_path.as_os_str(),
            "--root".as_ref(),
            trusted.as_os_str(),
        ],
    )
    .expect("verification process runs");

    // Then: the evidence verifies through the trusted capability.
    assert!(
        output.status.success(),
        "verify stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("verification stdout is JSON");
    assert_eq!(report.get("status"), Some(&serde_json::json!("verified")));
}

#[cfg(unix)]
#[test]
fn unreadable_source_is_an_io_failure_not_stale_evidence() {
    use std::os::unix::fs::PermissionsExt as _;

    // Given: valid evidence whose source becomes unreadable.
    let sandbox = TempDir::new().expect("sandbox is created");
    let source = sandbox.path().join("doc.txt");
    write(&source, b"needle\n").expect("source is written");
    let bundle = search(sandbox.path(), Path::new(".")).expect("search succeeds");
    let bundle_path = sandbox.path().join("bundle.json");
    store(&bundle_path, &bundle).expect("bundle is stored");
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o000))
        .expect("source permissions are removed");

    // When: the real CLI attempts live verification.
    let output = run(
        sandbox.path(),
        ["find".as_ref(), "verify".as_ref(), bundle_path.as_os_str()],
    )
    .expect("verification process runs");
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o600))
        .expect("source permissions are restored");

    // Then: permission denial is typed exit 5 with no stale JSON report.
    assert_eq!(output.status.code(), Some(5));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("verification source"));
}

#[cfg(unix)]
#[test]
fn final_symlink_trusted_root_is_rejected() {
    use std::os::unix::fs::symlink;

    // Given: evidence metadata and CLI authority both name a symlink to a real root.
    let sandbox = TempDir::new().expect("sandbox is created");
    let real = sandbox.path().join("real");
    let linked = sandbox.path().join("linked");
    std::fs::create_dir_all(&real).expect("real root is created");
    write(real.join("doc.txt"), b"needle\n").expect("source is written");
    let canonical_real = std::fs::canonicalize(&real).expect("real root canonicalizes");
    let mut bundle = search(sandbox.path(), &canonical_real).expect("search succeeds");
    symlink(&real, &linked).expect("root symlink is created");
    bundle.root = linked.to_str().expect("linked path is UTF-8").to_owned();
    let bundle_path = sandbox.path().join("linked-root.json");
    store(&bundle_path, &bundle).expect("bundle is stored");

    // When: the symlink path is supplied as the trusted verification root.
    let output = run(
        sandbox.path(),
        [
            "find".as_ref(),
            "verify".as_ref(),
            bundle_path.as_os_str(),
            "--root".as_ref(),
            linked.as_os_str(),
        ],
    )
    .expect("verification process runs");

    // Then: root acquisition fails closed with exit 5 before source reads.
    assert_eq!(output.status.code(), Some(5));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("verification root"));
}
