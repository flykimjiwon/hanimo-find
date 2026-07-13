//! Frozen Todo 1 search conformance exercised through the real filesystem.

use std::path::PathBuf;

use hanimo_core::{
    QueryPlan, SearchResult, assemble_bundle,
    model::{Budget, MAX_QUERY_BYTES, MAX_QUERY_LITERALS, QueryLimitError},
    search,
};

#[test]
fn search_matches_frozen_bundle_when_multilingual_fixture_is_scanned() {
    // Given: the frozen plan and an isolated copy of only the multilingual fixtures.
    let product_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let sandbox = tempfile::tempdir().expect("sandbox must be available");
    for relative in [
        "fixtures/multilingual/config/feature-flags.txt",
        "fixtures/multilingual/docs/deploy.md",
    ] {
        let target = sandbox.path().join(relative);
        std::fs::create_dir_all(target.parent().expect("fixture has parent"))
            .expect("fixture directory must be created");
        std::fs::copy(product_root.join(relative), target).expect("fixture copy must succeed");
    }
    let plan_bytes = std::fs::read(product_root.join("conformance/query-plan.valid.json"))
        .expect("query plan fixture must be readable");
    let plan: QueryPlan =
        serde_json::from_slice(&plan_bytes).expect("query plan fixture must match the model");

    // When: the real in-process core searches the fixture root.
    let root = std::fs::canonicalize(sandbox.path()).expect("sandbox root canonicalizes");
    let result = search(&root, &plan).expect("frozen fixture search must succeed");
    let actual = assemble_bundle(&plan, result).expect("bundle assembly must succeed");

    // Then: its authoritative JSON equals the frozen accepted bundle.
    let expected_bytes =
        std::fs::read(product_root.join("conformance/evidence-bundle.accepted.json"))
            .expect("evidence fixture must be readable");
    let expected: serde_json::Value =
        serde_json::from_slice(&expected_bytes).expect("evidence fixture must be valid JSON");
    assert_eq!(
        serde_json::to_value(actual).expect("bundle serializes"),
        expected
    );
}

#[test]
fn rejected_bundle_attestation_matches_frozen_vector() {
    // Given: the frozen rejected query with no selected evidence blocks.
    let plan = QueryPlan {
        schema_version: "0.1.0".to_owned(),
        query: "Find \"missing phrase\" MISSING_IDENTIFIER".to_owned(),
        root: ".".to_owned(),
        quoted_phrases: vec!["missing phrase".to_owned()],
        identifiers: vec!["MISSING_IDENTIFIER".to_owned()],
        terms: Vec::new(),
        budget: Budget::default(),
    };

    // When: the deterministic rejected bundle is assembled.
    let bundle = assemble_bundle(
        &plan,
        SearchResult {
            blocks: Vec::new(),
            skipped: Vec::new(),
        },
    )
    .expect("rejected bundle assembly succeeds");

    // Then: its immutable payload attestation matches the frozen vector.
    assert_eq!(
        bundle.bundle_sha256,
        "fc5b89c1a56505ff5a2b1e2a464571dd0d77a2f1a51f34d8dda13ee19ebb11f8"
    );
}

#[test]
fn budget_rejected_bundle_attestation_matches_frozen_vector() {
    // Given: the frozen bundle whose only insufficiency is an explicit budget gap.
    let json = include_str!("../../../conformance/evidence-bundle.budget-rejected.json");
    let bundle: hanimo_core::EvidenceBundle =
        serde_json::from_str(json).expect("frozen bundle parses");

    // When: the immutable payload is independently attested.
    let digest = hanimo_core::bundle_sha256(&bundle).expect("bundle attestation succeeds");

    // Then: the digest and deterministic rejection state remain frozen together.
    assert_eq!(digest, bundle.bundle_sha256);
    assert_eq!(bundle.critic.verdict, hanimo_core::CriticVerdict::Rejected);
    assert!(
        bundle
            .skipped
            .iter()
            .any(|skip| skip.reason == hanimo_core::SkipReason::Budget)
    );
}

#[test]
fn query_plan_schema_exposes_runtime_query_ceilings() {
    // Given: the public QueryPlan schema.
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../schema/query-plan.schema.json"))
            .expect("query schema parses");
    let constants: serde_json::Value =
        serde_json::from_str(include_str!("../../../conformance/v0.constants.json"))
            .expect("conformance constants parse");

    // When: query and literal constraints are inspected.
    let query = schema
        .pointer("/properties/query")
        .expect("query schema exists");
    let arrays = ["quoted_phrases", "identifiers", "terms"].map(|name| {
        schema
            .pointer(&format!("/properties/{name}"))
            .expect("array schema exists")
    });

    // Then: JSON character limits and explicit UTF-8/runtime extensions are frozen together.
    assert_eq!(
        query.get("maxLength"),
        Some(&serde_json::json!(MAX_QUERY_BYTES))
    );
    assert_eq!(
        query.get("x-maxUtf8Bytes"),
        Some(&serde_json::json!(MAX_QUERY_BYTES))
    );
    assert!(arrays.iter().all(|array| {
        array.get("maxItems") == Some(&serde_json::json!(MAX_QUERY_LITERALS))
            && array.pointer("/items/x-maxUtf8Bytes") == Some(&serde_json::json!(MAX_QUERY_BYTES))
    }));
    assert_eq!(
        schema.pointer("/x-hanimo-find-contract/query_limits/runtime_aggregate_literals"),
        Some(&serde_json::json!(MAX_QUERY_LITERALS))
    );
    assert_eq!(
        constants.pointer("/search_limits/query_utf8_bytes"),
        Some(&serde_json::json!(MAX_QUERY_BYTES))
    );
    assert_eq!(
        constants.pointer("/search_limits/aggregate_literals"),
        Some(&serde_json::json!(MAX_QUERY_LITERALS))
    );
}

#[test]
fn query_plan_conformance_enforces_multibyte_utf8_bytes() {
    // Given: a frozen valid plan cloned at exactly 4,096 UTF-8 bytes and max+3.
    let mut plan: QueryPlan =
        serde_json::from_str(include_str!("../../../conformance/query-plan.valid.json"))
            .expect("query fixture parses");
    plan.query = format!("{}a", "가".repeat(1_365));
    assert_eq!(plan.query.len(), MAX_QUERY_BYTES);

    // When: runtime conformance validates exact max and then one multibyte scalar above it.
    let accepted = plan.validate_limits();
    plan.query.push('가');
    let rejected = plan.validate_limits();

    // Then: bytes, not JSON Schema character count, control the runtime invariant.
    assert_eq!(accepted, Ok(()));
    assert_eq!(
        rejected,
        Err(QueryLimitError::Bytes {
            maximum: MAX_QUERY_BYTES
        })
    );
}
