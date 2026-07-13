use std::{fs, io};

use tempfile::TempDir;

use super::{VerificationStatus, verify_with_reader};
use crate::{QueryPlan, assemble_bundle, model::Budget, search};

fn evidence() -> crate::EvidenceBundle {
    let sandbox = TempDir::new().expect("sandbox is created");
    fs::write(sandbox.path().join("doc.txt"), b"needle IDENT\n").expect("fixture is written");
    let plan = QueryPlan {
        schema_version: "0.1.0".to_owned(),
        query: "needle IDENT".to_owned(),
        root: ".".to_owned(),
        quoted_phrases: vec!["needle".to_owned()],
        identifiers: vec!["IDENT".to_owned()],
        terms: Vec::new(),
        budget: Budget::default(),
    };
    let root = fs::canonicalize(sandbox.path()).expect("sandbox root canonicalizes");
    let result = search(&root, &plan).expect("search succeeds");
    assemble_bundle(&plan, result).expect("bundle is assembled")
}

#[test]
fn whole_bundle_retries_once_when_source_mutates_during_verification() {
    // Given: a valid bundle and a reader that changes bytes after its first snapshot.
    let sandbox = TempDir::new().expect("sandbox is created");
    fs::write(sandbox.path().join("doc.txt"), b"needle IDENT\n").expect("fixture is written");
    let plan = QueryPlan {
        schema_version: "0.1.0".to_owned(),
        query: "needle IDENT".to_owned(),
        root: ".".to_owned(),
        quoted_phrases: vec!["needle".to_owned()],
        identifiers: vec!["IDENT".to_owned()],
        terms: Vec::new(),
        budget: Budget::default(),
    };
    let root = fs::canonicalize(sandbox.path()).expect("sandbox root canonicalizes");
    let result = search(&root, &plan).expect("search succeeds");
    let bundle = assemble_bundle(&plan, result).expect("bundle is assembled");
    let mut reads = 0_u8;

    // When: the first snapshot is original and every later read is the stable mutation.
    let report = verify_with_reader(&bundle, |_, _| {
        reads = reads.saturating_add(1);
        Ok(if reads == 1 {
            b"needle IDENT\n".to_vec()
        } else {
            b"changed IDENT\n".to_vec()
        })
    })
    .expect("verification runs");

    // Then: the whole bundle is retried once and the stable change is source drift.
    assert_eq!(report.attempts, 2);
    assert_eq!(report.status, VerificationStatus::SourceDrift);
}

#[test]
fn source_reader_classifies_not_found_as_stale() {
    // Given: valid evidence and an injected reader reporting a missing source.
    let bundle = evidence();

    // When: live verification attempts to open the recorded path.
    let report = verify_with_reader(&bundle, |_, _| {
        Err(io::Error::new(io::ErrorKind::NotFound, "missing source"))
    })
    .expect("missing source is an evidence status");

    // Then: absence remains stale rather than becoming an operational failure.
    assert_eq!(report.status, VerificationStatus::Stale);
}

#[test]
fn source_reader_surfaces_non_missing_io_errors() {
    // Given: valid evidence and representative security and device read failures.
    let bundle = evidence();

    for kind in [io::ErrorKind::PermissionDenied, io::ErrorKind::Other] {
        // When: the injected reader fails for a reason other than path absence.
        let error = verify_with_reader(&bundle, |_, _| {
            Err(io::Error::new(kind, "injected source read failure"))
        })
        .expect_err("non-missing source I/O must fail verification");

        // Then: the typed operational error is preserved for exit-5 routing.
        assert_eq!(error.to_string(), "cannot read verification source");
    }
}
