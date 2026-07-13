//! Regression tests for literal occurrence budgets and Markdown headings.

use std::{fs, num::NonZeroUsize};

use hanimo_core::{
    QueryPlan, SkipReason,
    model::{Budget, MAX_QUERY_LITERALS},
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

#[test]
fn search_fails_closed_when_one_line_exceeds_the_occurrence_budget() {
    // Given: one line with three non-overlapping occurrences and a one-match budget.
    let sandbox = TempDir::new().expect("sandbox is created");
    fs::write(
        sandbox.path().join("repeated.txt"),
        b"needle needle needle\n",
    )
    .expect("fixture is written");
    let mut query = plan("needle");
    query.budget.max_matches = NonZeroUsize::new(1).expect("one is nonzero");

    // When: bounded literal search scans the repeated line.
    let root = std::fs::canonicalize(sandbox.path()).expect("sandbox root canonicalizes");
    let result = search(&root, &query).expect("budget exhaustion is reported in-band");

    // Then: no over-budget evidence is returned and the omission is explicit.
    assert!(result.blocks.is_empty());
    assert_eq!(
        result.skipped.first().map(|gap| &gap.reason),
        Some(&SkipReason::Budget)
    );
}

#[test]
fn search_counts_overlapping_occurrences_at_distinct_byte_offsets() {
    // Given: `aa` occurs at byte offsets zero and one with a one-match budget.
    let sandbox = TempDir::new().expect("sandbox is created");
    fs::write(sandbox.path().join("overlap.txt"), b"aaa\n").expect("fixture is written");
    let mut query = plan("aa");
    query.budget.max_matches = NonZeroUsize::new(1).expect("one is nonzero");

    // When: bounded literal search scans the overlapping occurrences.
    let root = std::fs::canonicalize(sandbox.path()).expect("sandbox root canonicalizes");
    let result = search(&root, &query).expect("budget exhaustion is reported in-band");

    // Then: both distinct start offsets consume the budget, so the file is omitted explicitly.
    assert!(result.blocks.is_empty());
    assert_eq!(
        result.skipped.first().map(|gap| &gap.reason),
        Some(&SkipReason::Budget)
    );
}

#[test]
fn search_recognizes_markdown_h1_through_h6_for_context_and_score() {
    // Given: one matching document beneath each valid ATX heading level.
    let sandbox = TempDir::new().expect("sandbox is created");
    for level in 1..=6 {
        let content = format!("{} Heading {level}\nneedle\n", "#".repeat(level));
        fs::write(sandbox.path().join(format!("h{level}.md")), content)
            .expect("fixture is written");
    }
    let mut query = plan("needle");
    query.budget.context_lines = 0;

    // When: the Markdown files are searched.
    let root = std::fs::canonicalize(sandbox.path()).expect("sandbox root canonicalizes");
    let result = search(&root, &query).expect("search succeeds");

    // Then: every level supplies heading context and the frozen heading score.
    assert_eq!(result.blocks.len(), 6);
    assert!(
        result
            .blocks
            .iter()
            .all(|block| block.line_start.get() == 1)
    );
    assert!(
        result
            .blocks
            .iter()
            .all(|block| block.score_components.heading == 75)
    );
}

#[test]
fn search_rejects_over_limit_plan_before_opening_the_root() {
    // Given: a direct core plan with one literal above the hard limit and a missing root.
    let sandbox = TempDir::new().expect("sandbox is created");
    let mut query = plan("needle");
    query.terms = vec!["needle".to_owned(); MAX_QUERY_LITERALS + 1];

    // When: the public core search boundary receives the untrusted plan.
    let error =
        search(&sandbox.path().join("missing"), &query).expect_err("over-limit query is rejected");

    // Then: typed query admission fails before root filesystem access.
    assert_eq!(error.to_string(), "query exceeds 64 literals");
}

#[test]
fn search_does_not_silently_omit_the_last_allowed_literal() {
    // Given: the last of exactly 64 literals is the only one present in the source.
    let sandbox = TempDir::new().expect("sandbox is created");
    fs::write(sandbox.path().join("last.txt"), b"q63x\n").expect("fixture is written");
    let mut query = plan("q63x");
    query.terms = (0..MAX_QUERY_LITERALS)
        .map(|index| format!("q{index}x"))
        .collect();

    // When: the bounded literal set is searched.
    let root = std::fs::canonicalize(sandbox.path()).expect("sandbox root canonicalizes");
    let result = search(&root, &query).expect("boundary query is admitted");

    // Then: the final admitted literal participates in exact matching.
    assert_eq!(result.blocks.len(), 1);
    assert!(
        result
            .blocks
            .iter()
            .all(|block| block.matched_terms == ["q63x"])
    );
}
