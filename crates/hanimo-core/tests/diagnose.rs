//! Integration coverage for the public static diagnosis API.

use std::{
    io,
    num::NonZeroUsize,
    path::{Path, PathBuf},
};

use hanimo_core::diagnose::{
    DiagnoseBudget, DiagnoseError, DiagnoseLimit, RuleId, diagnose, diagnose_with_budget,
    render_markdown,
};
use tempfile::TempDir;

fn fixture_root(name: &str) -> io::Result<PathBuf> {
    canonical_root(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures/diagnose")
            .join(name),
    )
}

fn canonical_root(path: &Path) -> io::Result<PathBuf> {
    std::fs::canonicalize(path)
}

#[test]
fn diagnose_reports_exact_rules_and_real_lines_when_rag_patterns_are_present() {
    // Given: a synthetic repository containing eight inspectable RAG risks.
    let root = fixture_root("positive-repo").expect("fixture root canonicalizes");

    // When: the static diagnosis scans source text without importing it.
    let diagnosis = diagnose(&root).expect("positive fixture diagnosis must succeed");

    // Then: every frozen rule is reported once in stable order with a real line.
    let actual: Vec<RuleId> = diagnosis
        .findings
        .iter()
        .map(|finding| finding.rule_id)
        .collect();
    assert_eq!(
        actual,
        vec![
            RuleId::VectorStoreDependency,
            RuleId::EmbeddingCall,
            RuleId::FixedChunking,
            RuleId::TopKRetriever,
            RuleId::Reranker,
            RuleId::MissingLineCitations,
            RuleId::MissingFreshnessValidation,
            RuleId::MissingExactSearchFallback,
        ]
    );
    for finding in &diagnosis.findings {
        assert_eq!(finding.citations.len(), 1);
        let citation = finding
            .citations
            .first()
            .expect("one citation was asserted above");
        assert!(root.join(&citation.path).is_file());
        assert!(line_exists(&root.join(&citation.path), citation.line));
        assert!(finding.message.starts_with(finding.rule_id.as_str()));
    }
}

#[test]
fn diagnose_returns_no_findings_when_only_exact_verified_search_is_present() {
    // Given: a synthetic repository with literal retrieval and line-addressed evidence.
    let root = fixture_root("negative-repo").expect("fixture root canonicalizes");

    // When: the same static diagnosis scans it.
    let diagnosis = diagnose(&root).expect("negative fixture diagnosis must succeed");

    // Then: no unsupported finding is invented.
    assert!(diagnosis.findings.is_empty());
}

#[test]
fn diagnose_markdown_is_a_pure_render_of_authoritative_json() {
    // Given: a typed diagnosis produced from the positive fixture.
    let root = fixture_root("positive-repo").expect("fixture root canonicalizes");
    let diagnosis = diagnose(&root).expect("fixture diagnosis must succeed");

    // When: consumers request JSON and Markdown representations.
    let json = serde_json::to_value(&diagnosis).expect("diagnosis must serialize");
    let markdown = render_markdown(&diagnosis);

    // Then: Markdown contains only facts present in the authoritative JSON.
    assert_eq!(
        json.get("schema_version")
            .and_then(serde_json::Value::as_str),
        Some("0.1.0")
    );
    assert_eq!(
        json.get("findings")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(8)
    );
    for finding in &diagnosis.findings {
        assert!(markdown.contains(&finding.message));
        for citation in &finding.citations {
            assert!(markdown.contains(&format!("{}:{}", citation.path, citation.line)));
        }
    }
    assert!(markdown.contains(&diagnosis.bundle_sha256));
    assert!(markdown.contains(&diagnosis.summary));
}

#[test]
fn diagnose_rejects_a_source_over_the_default_per_file_budget() {
    // Given: one regular source exceeds the documented one-MiB file envelope.
    let sandbox = TempDir::new().expect("sandbox is created");
    let oversized = vec![b'x'; 1_048_577];
    std::fs::write(sandbox.path().join("oversized.py"), oversized)
        .expect("oversized fixture is written");

    // When: diagnosis scans the hostile repository.
    let root = canonical_root(sandbox.path()).expect("sandbox root canonicalizes");
    let result = diagnose(&root);

    // Then: the source is rejected instead of being retained without a bound.
    assert!(
        result.is_err(),
        "oversized diagnosis source must fail closed"
    );
}

#[test]
fn diagnose_reports_the_candidate_file_budget_deterministically() {
    // Given: three regular sources and a two-candidate diagnosis budget.
    let sandbox = TempDir::new().expect("sandbox is created");
    for name in ["c.py", "a.py", "b.py"] {
        std::fs::write(sandbox.path().join(name), b"import chromadb\n")
            .expect("candidate fixture is written");
    }

    // When: diagnosis crosses the explicit candidate boundary.
    let root = canonical_root(sandbox.path()).expect("sandbox root canonicalizes");
    let result = diagnose_with_budget(&root, budget(2, 64, 128));

    // Then: the typed candidate limit is stable regardless of creation order.
    assert!(matches!(
        result,
        Err(DiagnoseError::BudgetExceeded(DiagnoseLimit::CandidateFiles))
    ));
}

#[test]
fn diagnose_reports_the_total_byte_budget_without_retaining_the_corpus() {
    // Given: two individually valid files exceed the combined byte envelope.
    let sandbox = TempDir::new().expect("sandbox is created");
    std::fs::write(sandbox.path().join("a.py"), b"1234").expect("first fixture is written");
    std::fs::write(sandbox.path().join("b.py"), b"5678").expect("second fixture is written");

    // When: diagnosis reaches the second sorted source.
    let root = canonical_root(sandbox.path()).expect("sandbox root canonicalizes");
    let result = diagnose_with_budget(&root, budget(2, 4, 7));

    // Then: total bytes fail through the typed limit rather than an allocation failure.
    assert!(matches!(
        result,
        Err(DiagnoseError::BudgetExceeded(DiagnoseLimit::TotalBytes))
    ));
}

#[test]
fn diagnose_findings_and_digest_ignore_filesystem_creation_order() {
    // Given: equivalent repositories created in opposite order.
    let left = TempDir::new().expect("left sandbox is created");
    let right = TempDir::new().expect("right sandbox is created");
    let sources = [
        ("a.py", b"import chromadb\n".as_slice()),
        (
            "z.py",
            b"retriever = vectorstore.as_retriever()\n".as_slice(),
        ),
    ];
    for (path, bytes) in sources {
        std::fs::write(left.path().join(path), bytes).expect("left source is written");
    }
    for (path, bytes) in sources.into_iter().rev() {
        std::fs::write(right.path().join(path), bytes).expect("right source is written");
    }

    // When: both repositories are diagnosed.
    let left_root = canonical_root(left.path()).expect("left root canonicalizes");
    let right_root = canonical_root(right.path()).expect("right root canonicalizes");
    let left_diagnosis = diagnose(&left_root).expect("left diagnosis succeeds");
    let right_diagnosis = diagnose(&right_root).expect("right diagnosis succeeds");

    // Then: source order, findings, and digest are deterministic.
    assert_eq!(left_diagnosis, right_diagnosis);
}

#[test]
fn diagnose_findings_and_digest_ignore_ancestor_and_repository_context() {
    // Given: identical sources scanned outside any git repository and inside a
    // git repository whose ancestor ignore file excludes every Python source.
    let plain = TempDir::new().expect("plain sandbox is created");
    let nested = TempDir::new().expect("nested sandbox is created");
    let nested_root = nested.path().join("child");
    std::fs::create_dir(&nested_root).expect("nested child root is created");
    std::fs::create_dir(nested.path().join(".git")).expect("repository marker is created");
    std::fs::write(nested.path().join(".gitignore"), b"*.py\n")
        .expect("ancestor ignore file is written");
    for root in [plain.path(), nested_root.as_path()] {
        std::fs::write(root.join("a.py"), b"import chromadb\n").expect("source is written");
        std::fs::write(root.join("ignored.py"), b"import chromadb\n")
            .expect("ignored source is written");
        std::fs::write(root.join(".gitignore"), b"ignored.py\n")
            .expect("root ignore file is written");
    }

    // When: both roots are diagnosed.
    let plain_root = canonical_root(plain.path()).expect("plain root canonicalizes");
    let nested_root = canonical_root(&nested_root).expect("nested root canonicalizes");
    let plain_diagnosis = diagnose(&plain_root).expect("plain diagnosis succeeds");
    let nested_diagnosis = diagnose(&nested_root).expect("nested diagnosis succeeds");

    // Then: ancestor ignore context and repository detection change nothing,
    // and the root-level ignore file is honored without any git repository.
    assert_eq!(plain_diagnosis, nested_diagnosis);
    assert!(
        plain_diagnosis
            .findings
            .iter()
            .flat_map(|finding| &finding.citations)
            .all(|citation| citation.path != "ignored.py")
    );
    assert!(
        plain_diagnosis
            .findings
            .iter()
            .flat_map(|finding| &finding.citations)
            .any(|citation| citation.path == "a.py")
    );
}

#[test]
fn diagnose_visits_sources_in_root_relative_raw_byte_order() {
    // Given: sibling paths whose component order and raw-byte order disagree:
    // raw bytes place "a-b.py" (0x2D) before "a/b.py" (0x2F).
    let sandbox = TempDir::new().expect("sandbox is created");
    std::fs::create_dir(sandbox.path().join("a")).expect("nested directory is created");
    for path in ["a/b.py", "a-b.py"] {
        std::fs::write(sandbox.path().join(path), b"import chromadb\n")
            .expect("sibling source is written");
    }

    // When: the repository is diagnosed.
    let root = canonical_root(sandbox.path()).expect("sandbox root canonicalizes");
    let diagnosis = diagnose(&root).expect("sibling diagnosis succeeds");

    // Then: the first citation follows the canonical raw-byte source order.
    assert_eq!(
        diagnosis
            .findings
            .iter()
            .find(|finding| finding.rule_id == RuleId::VectorStoreDependency)
            .and_then(|finding| finding.citations.first())
            .map(|citation| citation.path.as_str()),
        Some("a-b.py")
    );
}

const fn budget(candidate_files: usize, file_bytes: usize, total_bytes: usize) -> DiagnoseBudget {
    DiagnoseBudget {
        max_candidate_files: nonzero(candidate_files),
        max_file_bytes: nonzero(file_bytes),
        max_total_bytes: nonzero(total_bytes),
    }
}

const fn nonzero(value: usize) -> NonZeroUsize {
    match NonZeroUsize::new(value) {
        Some(value) => value,
        None => NonZeroUsize::MIN,
    }
}

fn line_exists(path: &Path, line: usize) -> bool {
    std::fs::read_to_string(path).is_ok_and(|text| line > 0 && line <= text.lines().count())
}
