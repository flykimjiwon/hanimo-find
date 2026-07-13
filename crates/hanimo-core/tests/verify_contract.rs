//! Live evidence verification contract tests.

use std::{error::Error, fs, io, num::NonZeroUsize, path::Path};

use hanimo_core::{
    CriticVerdict, EncodedBytes, QueryPlan, SkipReason, SkippedEvidence, VerificationStatus,
    assemble_bundle, bundle_sha256, identity::source_sha256, model::Budget, search, verify,
};
use tempfile::TempDir;

fn plan() -> QueryPlan {
    QueryPlan {
        schema_version: "0.1.0".to_owned(),
        query: "needle IDENT".to_owned(),
        root: ".".to_owned(),
        quoted_phrases: vec!["needle".to_owned()],
        identifiers: vec!["IDENT".to_owned()],
        terms: Vec::new(),
        budget: Budget::default(),
    }
}

fn write(path: &Path, content: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)
}

fn bundle(root: &Path) -> Result<hanimo_core::EvidenceBundle, Box<dyn Error>> {
    let query = plan();
    let canonical = fs::canonicalize(root)?;
    let result = search(&canonical, &query)?;
    Ok(assemble_bundle(&query, result)?)
}

#[test]
fn untouched_bundle_verifies() {
    // Given: a bundle compiled from an unchanged source.
    let sandbox = TempDir::new().expect("sandbox is created");
    write(&sandbox.path().join("doc.txt"), b"needle IDENT\n").expect("fixture is written");
    let evidence = bundle(sandbox.path()).expect("bundle is assembled");

    // When: live verification re-reads the source.
    let report = verify(sandbox.path(), &evidence).expect("verification runs");

    // Then: the whole bundle and every block are verified in one attempt.
    assert_eq!(report.status, VerificationStatus::Verified);
    assert_eq!(report.attempts, 1);
    assert!(
        report
            .blocks
            .iter()
            .all(|block| block.status == VerificationStatus::Verified)
    );
}

#[test]
fn accepted_bundle_with_budget_gap_is_forged_even_when_digest_is_self_consistent() {
    // Given: a self-consistent bundle whose accepted critic contradicts a budget gap.
    let sandbox = TempDir::new().expect("sandbox is created");
    write(&sandbox.path().join("doc.txt"), b"needle IDENT\n").expect("fixture is written");
    let mut evidence = bundle(sandbox.path()).expect("bundle is assembled");
    assert_eq!(evidence.critic.verdict, CriticVerdict::Accepted);
    evidence.skipped.push(SkippedEvidence {
        path: EncodedBytes::Utf8 {
            text: "omitted.txt".to_owned(),
        },
        reason: SkipReason::Budget,
    });
    evidence.bundle_sha256 = bundle_sha256(&evidence).expect("bundle digest is recomputed");

    // When: live verification receives the semantically contradictory artifact.
    let report = verify(sandbox.path(), &evidence).expect("verification runs");

    // Then: self-consistency alone cannot promote the invalid critic to verified.
    assert_eq!(report.status, VerificationStatus::Forged);
}

#[test]
fn verify_rejects_artifact_owned_budget_above_the_product_ceiling() {
    // Given: an otherwise valid bundle with a verifier-controlled numeric limit exceeded.
    let sandbox = TempDir::new().expect("sandbox is created");
    write(&sandbox.path().join("doc.txt"), b"needle IDENT\n").expect("fixture is written");
    let mut evidence = bundle(sandbox.path()).expect("bundle is assembled");
    evidence.budget.max_file_bytes = NonZeroUsize::new(16_777_217).expect("test limit is nonzero");

    // When: the artifact attempts to enlarge a live-read allowance.
    let error = verify(sandbox.path(), &evidence).expect_err("oversized budget must fail");

    // Then: the artifact is rejected before any live source verification.
    assert_eq!(
        error.to_string(),
        "invalid evidence bundle: max_file_bytes exceeds verification limit"
    );
}

#[test]
fn verify_rejects_more_blocks_than_the_product_ceiling() {
    // Given: a self-consistent artifact containing one block beyond the fixed array ceiling.
    let sandbox = TempDir::new().expect("sandbox is created");
    write(&sandbox.path().join("doc.txt"), b"needle IDENT\n").expect("fixture is written");
    let mut evidence = bundle(sandbox.path()).expect("bundle is assembled");
    let block = evidence
        .blocks
        .first()
        .expect("fixture has a block")
        .clone();
    evidence.blocks = vec![block; 65];
    evidence.budget.max_blocks = NonZeroUsize::new(64).expect("test limit is nonzero");
    evidence.bundle_sha256 = bundle_sha256(&evidence).expect("bundle digest is recomputed");

    // When: verification receives the over-limit array.
    let error = verify(sandbox.path(), &evidence).expect_err("oversized block array must fail");

    // Then: validation rejects it before repeated live reads.
    assert_eq!(
        error.to_string(),
        "invalid evidence bundle: blocks exceed verification limit"
    );
}

#[test]
fn one_byte_source_change_is_source_drift() {
    // Given: a valid bundle whose selected source byte changes afterward.
    let sandbox = TempDir::new().expect("sandbox is created");
    let path = sandbox.path().join("doc.txt");
    write(&path, b"needle IDENT\n").expect("fixture is written");
    let evidence = bundle(sandbox.path()).expect("bundle is assembled");
    write(&path, b"needle IDENX\n").expect("fixture is mutated");

    // When: the original bundle is verified against the changed source.
    let report = verify(sandbox.path(), &evidence).expect("verification runs");

    // Then: live source drift is distinguished from bundle forgery.
    assert_eq!(report.status, VerificationStatus::SourceDrift);
}

#[test]
fn inserted_line_is_source_drift() {
    // Given: a valid bundle whose source gains a leading line.
    let sandbox = TempDir::new().expect("sandbox is created");
    let path = sandbox.path().join("doc.txt");
    write(&path, b"needle IDENT\n").expect("fixture is written");
    let evidence = bundle(sandbox.path()).expect("bundle is assembled");
    write(&path, b"inserted\nneedle IDENT\n").expect("fixture is mutated");

    // When: the old line-addressed bundle is verified.
    let report = verify(sandbox.path(), &evidence).expect("verification runs");

    // Then: changed live bytes are surfaced as source drift.
    assert_eq!(report.status, VerificationStatus::SourceDrift);
}

#[test]
fn missing_or_renamed_path_is_stale() {
    // Given: a valid bundle whose cited path is renamed.
    let sandbox = TempDir::new().expect("sandbox is created");
    let original = sandbox.path().join("doc.txt");
    write(&original, b"needle IDENT\n").expect("fixture is written");
    let evidence = bundle(sandbox.path()).expect("bundle is assembled");
    fs::rename(original, sandbox.path().join("renamed.txt")).expect("fixture is renamed");

    // When: verification opens the recorded path without fallback discovery.
    let report = verify(sandbox.path(), &evidence).expect("verification runs");

    // Then: the missing recorded citation is stale.
    assert_eq!(report.status, VerificationStatus::Stale);
}

#[test]
fn tampered_bundle_fields_are_forged_before_live_comparison() {
    // Given: valid bundles with a tampered ID, path, or canonical text.
    let sandbox = TempDir::new().expect("sandbox is created");
    write(&sandbox.path().join("doc.txt"), b"needle IDENT\n").expect("fixture is written");
    let original = bundle(sandbox.path()).expect("bundle is assembled");
    let mut cases = Vec::new();
    let mut identifier = original.clone();
    identifier
        .blocks
        .first_mut()
        .expect("fixture has one block")
        .block_id = format!("sha256:{}", "0".repeat(64));
    cases.push(identifier);
    let mut path = original.clone();
    path.blocks.first_mut().expect("fixture has one block").path = EncodedBytes::Utf8 {
        text: "other.txt".to_owned(),
    };
    cases.push(path);
    let mut text = original;
    text.blocks
        .first_mut()
        .expect("fixture has one block")
        .content = EncodedBytes::Utf8 {
        text: "forged content".to_owned(),
    };
    cases.push(text);

    // When: each internally inconsistent bundle is verified.
    let reports: Vec<_> = cases
        .iter()
        .map(|case| verify(sandbox.path(), case).expect("verification runs"))
        .collect();

    // Then: tampering is classified as forgery, not source drift.
    assert!(
        reports
            .iter()
            .all(|report| report.status == VerificationStatus::Forged)
    );
}

#[test]
fn tampered_source_hash_is_forged_by_bundle_attestation() {
    // Given: a valid bundle whose recorded exact-source digest is replaced.
    let sandbox = TempDir::new().expect("sandbox is created");
    write(&sandbox.path().join("doc.txt"), b"needle IDENT\n").expect("fixture is written");
    let mut evidence = bundle(sandbox.path()).expect("bundle is assembled");
    evidence
        .blocks
        .first_mut()
        .expect("fixture has one block")
        .source_sha256 = "0".repeat(64);

    // When: verification checks immutable bundle integrity before reading live source.
    let report = verify(sandbox.path(), &evidence).expect("verification runs");

    // Then: a bundle-only source-hash rewrite is classified as internal forgery.
    assert_eq!(report.status, VerificationStatus::Forged);
}

#[test]
fn changed_outside_span_with_rewritten_source_hash_is_forged() {
    // Given: a bundle whose cited span is unchanged while uncited source and its recorded hash are rewritten.
    let sandbox = TempDir::new().expect("sandbox is created");
    let source = sandbox.path().join("doc.txt");
    let original = b"needle IDENT\nold outside span\n";
    let changed = b"needle IDENT\nnew outside span\n";
    write(&source, original).expect("fixture is written");
    let mut query = plan();
    query.budget.context_lines = 0;
    let root = fs::canonicalize(sandbox.path()).expect("sandbox root canonicalizes");
    let result = search(&root, &query).expect("search succeeds");
    let mut evidence = assemble_bundle(&query, result).expect("bundle is assembled");
    write(&source, changed).expect("uncited source is changed");
    evidence
        .blocks
        .first_mut()
        .expect("fixture has one block")
        .source_sha256 = source_sha256(changed);

    // When: the combined source and metadata rewrite is verified without recalculating bundle integrity.
    let report = verify(sandbox.path(), &evidence).expect("verification runs");

    // Then: unchanged cited text cannot hide the forged bundle metadata.
    assert_eq!(report.status, VerificationStatus::Forged);
}

#[test]
fn create_verify_tamper_stale_restore_roundtrip_uses_the_public_api() {
    // Given: newly created source evidence that verifies untouched.
    let sandbox = TempDir::new().expect("sandbox is created");
    let source = sandbox.path().join("doc.txt");
    let parked = sandbox.path().join("doc.parked");
    write(&source, b"needle IDENT\n").expect("fixture is written");
    let evidence = bundle(sandbox.path()).expect("bundle is assembled");
    let initial = verify(sandbox.path(), &evidence).expect("initial verification runs");

    // When: content is tampered, then restored before the cited path is parked and restored.
    write(&source, b"needle IDENX\n").expect("fixture is tampered");
    let drift = verify(sandbox.path(), &evidence).expect("drift verification runs");
    write(&source, b"needle IDENT\n").expect("fixture content is restored");
    fs::rename(&source, &parked).expect("fixture is parked");
    let stale = verify(sandbox.path(), &evidence).expect("stale verification runs");
    fs::rename(&parked, &source).expect("fixture is restored");
    let restored = verify(sandbox.path(), &evidence).expect("restored verification runs");

    // Then: the public report distinguishes drift and staleness before restoration.
    assert_eq!(initial.status, VerificationStatus::Verified);
    assert_eq!(drift.status, VerificationStatus::SourceDrift);
    assert_eq!(stale.status, VerificationStatus::Stale);
    assert_eq!(restored.status, VerificationStatus::Verified);
}

#[cfg(unix)]
#[test]
fn final_and_intermediate_symlinks_are_io_failures_without_following() {
    use std::os::unix::fs::symlink;

    // Given: two bundles whose final file and intermediate directory become symlinks.
    let sandbox = TempDir::new().expect("sandbox is created");
    let final_root = sandbox.path().join("final-root");
    let middle_root = sandbox.path().join("middle-root");
    write(&final_root.join("doc.txt"), b"needle IDENT\n").expect("fixture is written");
    write(&middle_root.join("dir/doc.txt"), b"needle IDENT\n").expect("fixture is written");
    let final_bundle = bundle(&final_root).expect("bundle is assembled");
    let middle_bundle = bundle(&middle_root).expect("bundle is assembled");
    let outside = sandbox.path().join("outside");
    write(&outside.join("doc.txt"), b"needle IDENT OUTSIDE\n").expect("fixture is written");
    fs::remove_file(final_root.join("doc.txt")).expect("final fixture is removed");
    symlink(outside.join("doc.txt"), final_root.join("doc.txt")).expect("file link is created");
    fs::rename(middle_root.join("dir"), middle_root.join("real-dir"))
        .expect("directory is renamed");
    symlink(&outside, middle_root.join("dir")).expect("directory link is created");

    // When: both bundles are verified through capability-relative opens.
    let final_error = verify(&final_root, &final_bundle).expect_err("final symlink must fail");
    let middle_error = verify(&middle_root, &middle_bundle).expect_err("middle symlink must fail");

    // Then: neither symlink is followed or misclassified as missing evidence.
    assert_eq!(final_error.to_string(), "cannot read verification source");
    assert_eq!(middle_error.to_string(), "cannot read verification source");
}
