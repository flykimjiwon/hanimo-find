//! Deterministic critic and Markdown projection contract tests.

use std::fs;

use hanimo_core::{
    CriticVerdict, EncodedBytes, QueryPlan, SkipReason, SkippedEvidence, assemble_bundle,
    model::Budget, render_markdown, search,
};
use tempfile::TempDir;

fn plan() -> QueryPlan {
    QueryPlan {
        schema_version: "0.1.0".to_owned(),
        query: "present missing IDENT NOPE".to_owned(),
        root: ".".to_owned(),
        quoted_phrases: vec!["present".to_owned(), "missing".to_owned()],
        identifiers: vec!["IDENT".to_owned(), "NOPE".to_owned()],
        terms: Vec::new(),
        budget: Budget::default(),
    }
}

#[test]
fn critic_reports_coverage_gaps_and_sorted_skips_deterministically() {
    // Given: partial required coverage and skips supplied out of path order.
    let sandbox = TempDir::new().expect("sandbox is created");
    fs::write(sandbox.path().join("doc.txt"), b"present IDENT\n").expect("fixture is written");
    let query = plan();
    let root = std::fs::canonicalize(sandbox.path()).expect("sandbox root canonicalizes");
    let result = search(&root, &query).expect("search succeeds");
    let skips = vec![
        SkippedEvidence {
            path: EncodedBytes::Utf8 {
                text: "z-secret".to_owned(),
            },
            reason: SkipReason::Secret,
        },
        SkippedEvidence {
            path: EncodedBytes::Utf8 {
                text: "a-hidden".to_owned(),
            },
            reason: SkipReason::Hidden,
        },
    ];

    // When: the authoritative typed bundle is assembled.
    let mut result = result;
    result.skipped = skips;
    let bundle = assemble_bundle(&query, result).expect("bundle is assembled");

    // Then: required coverage, gaps, and skips follow frozen deterministic order.
    assert_eq!(bundle.critic.verdict, CriticVerdict::Rejected);
    assert_eq!(bundle.critic.covered_quoted_phrases, ["present"]);
    assert_eq!(bundle.critic.covered_identifiers, ["IDENT"]);
    assert_eq!(bundle.critic.uncovered, ["missing", "NOPE"]);
    assert_eq!(
        bundle.skipped.first().map(|skip| skip.reason),
        Some(SkipReason::Hidden)
    );
    assert_eq!(
        bundle.skipped.get(1).map(|skip| skip.reason),
        Some(SkipReason::Secret)
    );
}

#[test]
fn markdown_embeds_authoritative_json_and_references_block_ids_in_order() {
    // Given: a multi-block accepted bundle.
    let sandbox = TempDir::new().expect("sandbox is created");
    fs::write(
        sandbox.path().join("b.txt"),
        b"present missing IDENT NOPE\n",
    )
    .expect("fixture is written");
    fs::write(
        sandbox.path().join("a.txt"),
        b"present missing IDENT NOPE\n",
    )
    .expect("fixture is written");
    let query = plan();
    let root = std::fs::canonicalize(sandbox.path()).expect("sandbox root canonicalizes");
    let result = search(&root, &query).expect("search succeeds");
    let bundle = assemble_bundle(&query, result).expect("bundle is assembled");
    let authoritative = serde_json::to_string_pretty(&bundle).expect("bundle serializes");

    // When: Markdown is rendered solely from the typed bundle.
    let markdown = render_markdown(&bundle).expect("Markdown renders");

    // Then: exact JSON is embedded and every block ID appears in bundle order.
    assert!(markdown.contains(&authoritative));
    let positions: Vec<_> = bundle
        .blocks
        .iter()
        .map(|block| {
            markdown
                .find(&block.block_id)
                .expect("block ID is rendered")
        })
        .collect();
    assert!(
        positions
            .windows(2)
            .all(|pair| matches!(pair, [left, right] if left < right))
    );
    assert_eq!(bundle.critic.verdict, CriticVerdict::Accepted);
}

#[test]
fn critic_rejects_when_budget_truncation_leaves_the_scan_incomplete() {
    // Given: complete required-literal coverage but an explicit budget gap.
    let sandbox = TempDir::new().expect("sandbox is created");
    fs::write(
        sandbox.path().join("a.txt"),
        b"present missing IDENT NOPE\n",
    )
    .expect("fixture is written");
    let query = plan();
    let root = std::fs::canonicalize(sandbox.path()).expect("sandbox root canonicalizes");
    let result = search(&root, &query).expect("search succeeds");
    let gaps = vec![SkippedEvidence {
        path: EncodedBytes::Utf8 {
            text: "b.txt".to_owned(),
        },
        reason: SkipReason::Budget,
    }];

    // When: the bundle critic evaluates the incomplete scan.
    let mut result = result;
    result.skipped = gaps;
    let bundle = assemble_bundle(&query, result).expect("bundle is assembled");

    // Then: covered literals cannot make a budget-truncated scan accepted.
    assert_eq!(bundle.critic.uncovered, Vec::<String>::new());
    assert_eq!(bundle.critic.verdict, CriticVerdict::Rejected);
}
