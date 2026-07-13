//! Structured search-gap behavior.

use std::{
    fs, io,
    num::NonZeroUsize,
    path::{Path, PathBuf},
};

use hanimo_core::{
    CriticVerdict, EncodedBytes, QueryPlan, SkipReason, assemble_bundle,
    model::{Budget, MAX_CANDIDATE_FILES, MAX_DISCOVERY_DEPTH, MAX_DISCOVERY_ENTRIES},
    search,
};
use tempfile::TempDir;

fn plan(term: &str) -> QueryPlan {
    QueryPlan {
        schema_version: "0.1.0".to_owned(),
        query: term.to_owned(),
        root: ".".to_owned(),
        quoted_phrases: Vec::new(),
        identifiers: Vec::new(),
        terms: vec![term.to_owned()],
        budget: Budget::default(),
    }
}

fn write(root: &Path, relative: &str, content: &[u8]) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        assert!(
            fs::create_dir_all(parent).is_ok(),
            "test directory is created"
        );
    }
    assert!(fs::write(path, content).is_ok(), "test file is written");
}

fn path_text(path: &EncodedBytes) -> Option<&str> {
    match path {
        EncodedBytes::Utf8 { text } => Some(text),
        EncodedBytes::Base64 { .. } => None,
    }
}

fn canonical_root(path: &Path) -> io::Result<PathBuf> {
    fs::canonicalize(path)
}

#[test]
fn search_reports_discovered_secret_and_oversized_sources() {
    // Given: one public match plus discovered secret-like and oversized files.
    let sandbox = TempDir::new().expect("sandbox is created");
    write(sandbox.path(), "public.txt", b"needle\n");
    write(sandbox.path(), "service-token.txt", b"needle\n");
    write(sandbox.path(), "z-large.txt", b"0123456789abcdefg");
    let mut query = plan("needle");
    query.budget.max_file_bytes = NonZeroUsize::new(16).expect("sixteen is nonzero");

    // When: the bounded search scans the root.
    let root = canonical_root(sandbox.path()).expect("test root canonicalizes");
    let result = search(&root, &query).expect("search succeeds");

    // Then: policy gaps are typed, path-addressed, and deterministically ordered.
    assert_eq!(result.blocks.len(), 1);
    let actual: Vec<_> = result
        .skipped
        .iter()
        .map(|gap| (path_text(&gap.path), gap.reason))
        .collect();
    assert_eq!(
        actual,
        [
            (Some("service-token.txt"), SkipReason::Secret),
            (Some("z-large.txt"), SkipReason::Oversized),
        ]
    );
}

#[test]
fn search_returns_a_partial_result_with_total_byte_budget_gaps() {
    // Given: two ordered matches but only enough total-byte budget for the first.
    let sandbox = TempDir::new().expect("sandbox is created");
    write(sandbox.path(), "a.txt", b"needle\n");
    write(sandbox.path(), "b.txt", b"needle\n");
    let mut query = plan("needle");
    query.budget.max_total_bytes = NonZeroUsize::new(7).expect("seven is nonzero");

    // When: the total-byte budget reaches its deterministic boundary.
    let root = canonical_root(sandbox.path()).expect("test root canonicalizes");
    let result = search(&root, &query).expect("partial search is returned");

    // Then: prior evidence remains and the omitted path is an explicit budget gap.
    assert_eq!(result.blocks.len(), 1);
    assert_eq!(
        result
            .skipped
            .first()
            .map(|gap| (path_text(&gap.path), gap.reason)),
        Some((Some("b.txt"), SkipReason::Budget))
    );
}

#[test]
fn search_marks_current_and_remaining_paths_when_match_budget_is_exhausted() {
    // Given: sorted files whose second file would exceed the match budget.
    let sandbox = TempDir::new().expect("sandbox is created");
    write(sandbox.path(), "a.txt", b"needle\n");
    write(sandbox.path(), "b.txt", b"needle needle\n");
    write(sandbox.path(), "c.txt", b"needle\n");
    let mut query = plan("needle");
    query.budget.max_matches = NonZeroUsize::new(1).expect("one is nonzero");

    // When: accepting the second file would exceed the match budget.
    let root = canonical_root(sandbox.path()).expect("test root canonicalizes");
    let result = search(&root, &query).expect("partial search is returned");

    // Then: no over-budget block leaks and the truncated suffix is explicit.
    assert_eq!(result.blocks.len(), 1);
    let actual: Vec<_> = result
        .skipped
        .iter()
        .map(|gap| (path_text(&gap.path), gap.reason))
        .collect();
    assert_eq!(
        actual,
        [
            (Some("b.txt"), SkipReason::Budget),
            (Some("c.txt"), SkipReason::Budget),
        ]
    );
}

#[test]
fn search_stops_discovery_at_the_candidate_limit_and_rejects_the_gap() {
    // Given: 64 eligible files followed by an oversized 65th path.
    let sandbox = TempDir::new().expect("sandbox is created");
    for index in (0..MAX_CANDIDATE_FILES).rev() {
        write(
            sandbox.path(),
            &format!("{index:03}.txt"),
            b"ordinary content\n",
        );
    }
    write(sandbox.path(), "064-boundary.txt", &vec![b'x'; 1_048_577]);
    let query = plan("needle");

    // When: search selects the deterministic candidate prefix.
    let root = canonical_root(sandbox.path()).expect("test root canonicalizes");
    let result = search(&root, &query).expect("candidate prefix is bounded");
    let bundle = assemble_bundle(&query, result).expect("partial bundle is assembled");

    // Then: the first unscanned eligible path is a budget gap, never content-read as oversized.
    assert_eq!(bundle.blocks.len(), 0);
    assert_eq!(bundle.critic.verdict, CriticVerdict::Rejected);
    let actual: Vec<_> = bundle
        .skipped
        .iter()
        .map(|gap| (path_text(&gap.path), gap.reason))
        .collect();
    assert_eq!(actual, [(Some("064-boundary.txt"), SkipReason::Budget)]);
}

#[test]
fn search_selects_candidate_prefix_by_global_raw_path_order() {
    // Given: depth-first order puts 64 nested files before a matching root file.
    let sandbox = TempDir::new().expect("sandbox is created");
    write(sandbox.path(), "a.txt", b"needle\n");
    for index in 0..MAX_CANDIDATE_FILES {
        write(
            sandbox.path(),
            &format!("a/{index:03}.txt"),
            b"ordinary content\n",
        );
    }

    // When: search selects the 64-file prefix.
    let root = canonical_root(sandbox.path()).expect("test root canonicalizes");
    let result = search(&root, &plan("needle")).expect("search succeeds");

    // Then: root-relative raw bytes place a.txt first and a/063.txt at the gap.
    assert_eq!(
        result
            .blocks
            .first()
            .and_then(|block| path_text(&block.path)),
        Some("a.txt")
    );
    assert_eq!(
        result
            .skipped
            .iter()
            .find(|gap| gap.reason == SkipReason::Budget)
            .and_then(|gap| path_text(&gap.path)),
        Some("a/063.txt")
    );
}

#[test]
fn search_fails_closed_before_content_when_discovery_entries_exceed_limit() {
    // Given: otherwise identical wide roots at the metadata maximum and max+1.
    let exact = TempDir::new().expect("exact sandbox is created");
    let exceeded = TempDir::new().expect("exceeded sandbox is created");
    for index in 0..MAX_DISCOVERY_ENTRIES {
        let content = if index == 0 {
            b"needle\n".as_slice()
        } else {
            b"ordinary\n".as_slice()
        };
        write(exact.path(), &format!("{index:03}.txt"), content);
        write(exceeded.path(), &format!("{index:03}.txt"), content);
    }
    write(exceeded.path(), "overflow.txt", b"ordinary\n");

    // When: both roots are searched.
    let query = plan("needle");
    let exact_root = canonical_root(exact.path()).expect("exact root canonicalizes");
    let exceeded_root = canonical_root(exceeded.path()).expect("exceeded root canonicalizes");
    let exact = search(&exact_root, &query).expect("exact-limit search succeeds");
    let exceeded = search(&exceeded_root, &query).expect("over-limit search is in-band");

    // Then: max is scanned, while max+1 reports one root-level gap before content.
    assert_eq!(exact.blocks.len(), 1);
    assert!(exceeded.blocks.is_empty());
    assert_eq!(
        exceeded
            .skipped
            .iter()
            .map(|gap| (path_text(&gap.path), gap.reason))
            .collect::<Vec<_>>(),
        [(Some(""), SkipReason::Budget)]
    );
}

#[test]
fn search_bounds_secret_skip_retention_independent_of_creation_order() {
    // Given: two max+1 secret-only roots created in opposite orders.
    let forward = TempDir::new().expect("forward sandbox is created");
    let reverse = TempDir::new().expect("reverse sandbox is created");
    for index in 0..=MAX_DISCOVERY_ENTRIES {
        write(
            forward.path(),
            &format!("secret-{index:03}.txt"),
            b"needle\n",
        );
    }
    for index in (0..=MAX_DISCOVERY_ENTRIES).rev() {
        write(
            reverse.path(),
            &format!("secret-{index:03}.txt"),
            b"needle\n",
        );
    }

    // When: both roots cross the metadata envelope.
    let query = plan("needle");
    let forward_root = canonical_root(forward.path()).expect("forward root canonicalizes");
    let reverse_root = canonical_root(reverse.path()).expect("reverse root canonicalizes");
    let forward = search(&forward_root, &query).expect("forward search is in-band");
    let reverse = search(&reverse_root, &query).expect("reverse search is in-band");

    // Then: retained skips stay constant and deterministic.
    assert_eq!(forward, reverse);
    assert_eq!(forward.skipped.len(), 1);
    assert_eq!(
        forward.skipped.first().map(|gap| gap.reason),
        Some(SkipReason::Budget)
    );
}

#[test]
fn search_enforces_the_discovery_depth_boundary() {
    // Given: one matching file at exact depth and one at depth max+1.
    let exact = TempDir::new().expect("exact sandbox is created");
    let exceeded = TempDir::new().expect("exceeded sandbox is created");
    write_deep_file(exact.path(), MAX_DISCOVERY_DEPTH - 1, b"needle\n");
    write_deep_file(exceeded.path(), MAX_DISCOVERY_DEPTH, b"needle\n");

    // When: both trees are searched.
    let query = plan("needle");
    let exact_root = canonical_root(exact.path()).expect("exact root canonicalizes");
    let exceeded_root = canonical_root(exceeded.path()).expect("exceeded root canonicalizes");
    let exact = search(&exact_root, &query).expect("exact-depth search succeeds");
    let exceeded = search(&exceeded_root, &query).expect("deep search is in-band");

    // Then: depth max is scanned and max+1 fails closed before content.
    assert_eq!(exact.blocks.len(), 1);
    assert!(exceeded.blocks.is_empty());
    assert_eq!(
        exceeded.skipped.first().map(|gap| gap.reason),
        Some(SkipReason::Budget)
    );
}

fn write_deep_file(root: &Path, directory_count: usize, content: &[u8]) {
    let relative = (0..directory_count)
        .map(|index| format!("d{index:02}"))
        .chain(std::iter::once("file.txt".to_owned()))
        .collect::<PathBuf>();
    let path = root.join(relative);
    assert!(
        path.parent()
            .is_some_and(|parent| fs::create_dir_all(parent).is_ok()),
        "deep directories are created"
    );
    assert!(fs::write(path, content).is_ok(), "deep file is written");
}
