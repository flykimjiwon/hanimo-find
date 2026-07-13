//! Real-filesystem behavior tests for deterministic literal search.

use std::{fs, num::NonZeroUsize, path::Path};

#[cfg(unix)]
use hanimo_core::EncodedBytes;
use hanimo_core::{QueryPlan, SkipReason, model::Budget, search};
use proptest::prelude::*;
use sha2::{Digest as _, Sha256};
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

#[test]
fn search_merges_overlap_and_includes_heading_when_contexts_touch() {
    // Given: two literal hits with overlapping context below a Markdown heading.
    let sandbox = TempDir::new().expect("sandbox is created");
    write(
        sandbox.path(),
        "docs/page.md",
        b"# Heading\nlead\nneedle\nbridge\nneedle\ntail\n",
    );
    let mut query = plan("needle");
    query.budget.context_lines = 1;

    // When: the file is searched.
    let root = fs::canonicalize(sandbox.path()).expect("sandbox root canonicalizes");
    let result = search(&root, &query).expect("search succeeds");

    // Then: one merged block begins at the heading and ends after the second hit.
    assert_eq!(result.blocks.len(), 1);
    let block = result.blocks.first().expect("one block exists");
    assert_eq!(block.line_start.get(), 1);
    assert_eq!(block.line_end.get(), 6);
}

#[test]
fn search_is_byte_exact_when_unicode_has_canonical_equivalents() {
    // Given: decomposed source bytes and a composed query scalar.
    let sandbox = TempDir::new().expect("sandbox is created");
    write(sandbox.path(), "unicode.txt", "e\u{301}\n".as_bytes());
    let query = plan("é");

    // When: exact literal search runs without normalization.
    let root = fs::canonicalize(sandbox.path()).expect("sandbox root canonicalizes");
    let result = search(&root, &query).expect("search succeeds");

    // Then: canonically equivalent but byte-distinct text does not match.
    assert!(result.blocks.is_empty());
}

#[cfg(unix)]
#[test]
fn search_skips_hidden_ignored_secret_and_external_symlink_content() {
    use std::os::unix::fs::symlink;

    // Given: public and forbidden files plus a symlink escaping the scan root.
    let sandbox = TempDir::new().expect("sandbox is created");
    let root = sandbox.path().join("repo");
    fs::create_dir_all(&root).expect("root is created");
    write(
        &root,
        ".gitignore",
        b".hidden/\nsecrets/\n*.key\nlink-outside\n",
    );
    write(&root, "public.txt", b"needle PUBLIC_SENTINEL\n");
    write(&root, ".hidden/hidden.txt", b"needle HIDDEN_SENTINEL\n");
    write(&root, "secrets/.env", b"needle SECRET_SENTINEL\n");
    write(&root, "private.key", b"needle KEY_SENTINEL\n");
    write(sandbox.path(), "outside.txt", b"needle OUTSIDE_SENTINEL\n");
    symlink("../outside.txt", root.join("link-outside")).expect("symlink is created");

    // When: the ignore-aware no-follow scanner searches the root.
    let root = fs::canonicalize(root).expect("search root canonicalizes");
    let result = search(&root, &plan("needle")).expect("search succeeds");
    let json = serde_json::to_string(&result).expect("result serializes");

    // Then: only public content appears anywhere in the result.
    assert_eq!(result.blocks.len(), 1);
    assert!(json.contains("PUBLIC_SENTINEL"));
    for forbidden in [
        "HIDDEN_SENTINEL",
        "SECRET_SENTINEL",
        "KEY_SENTINEL",
        "OUTSIDE_SENTINEL",
    ] {
        assert!(!json.contains(forbidden));
    }
}

#[cfg(unix)]
#[test]
fn search_rejects_a_symlink_root_before_reading_its_target() {
    use std::os::unix::fs::symlink;

    // Given: the supplied search root is a symlink to an outside sentinel tree.
    let sandbox = TempDir::new().expect("sandbox is created");
    let outside = sandbox.path().join("outside");
    fs::create_dir(&outside).expect("outside directory is created");
    write(&outside, "sentinel.txt", b"needle ROOT_SYMLINK_SENTINEL\n");
    let linked_root = sandbox.path().join("linked-root");
    symlink(&outside, &linked_root).expect("root symlink is created");

    // When: search is asked to acquire the linked root.
    let error = search(&linked_root, &plan("needle")).expect_err("root symlink is rejected");

    // Then: authority acquisition fails with the typed root-symlink error.
    assert_eq!(error.to_string(), "search root must not be a symbolic link");
}

#[cfg(unix)]
#[test]
fn search_rejects_an_intermediate_symlink_root_before_outside_read() {
    use std::os::unix::fs::symlink;

    // Given: an intermediate root component links to an outside sentinel tree.
    let sandbox = TempDir::new().expect("sandbox is created");
    let base = sandbox.path().join("base");
    let outside_child = sandbox.path().join("outside/child");
    fs::create_dir_all(&base).expect("base directory is created");
    fs::create_dir_all(&outside_child).expect("outside child is created");
    write(
        &outside_child,
        "sentinel.txt",
        b"needle OUTSIDE_INTERMEDIATE_ROOT_SENTINEL\n",
    );
    symlink(sandbox.path().join("outside"), base.join("link"))
        .expect("intermediate symlink is created");

    // When: search acquires an absolute root through the linked component.
    let linked_root = base.join("link/child");
    let error =
        search(&linked_root, &plan("needle")).expect_err("intermediate root symlink is rejected");

    // Then: root authority fails before outside content can be read.
    assert_eq!(error.to_string(), "search root must not be a symbolic link");
}

#[cfg(unix)]
#[test]
fn search_represents_invalid_content_bytes_as_base64() {
    // Given: a non-UTF-8 source containing an ASCII hit.
    let sandbox = TempDir::new().expect("sandbox is created");
    fs::write(sandbox.path().join("invalid.bin"), b"needle\n\xff\x80\n")
        .expect("binary fixture is written");

    // When: exact literal search returns the evidence block.
    let root = fs::canonicalize(sandbox.path()).expect("sandbox root canonicalizes");
    let result = search(&root, &plan("needle")).expect("search succeeds");
    let block = result.blocks.first().expect("one block exists");

    // Then: content uses base64 and the valid path remains UTF-8.
    assert!(matches!(block.path, EncodedBytes::Utf8 { .. }));
    assert!(matches!(block.content, EncodedBytes::Base64 { .. }));
    assert!(
        !serde_json::to_string(block)
            .expect("block serializes")
            .contains('\u{fffd}')
    );
}

#[test]
fn search_json_is_identical_across_thirty_runs() {
    // Given: three equal-score files created in non-lexical order.
    let sandbox = TempDir::new().expect("sandbox is created");
    for relative in ["z.txt", "a.txt", "m.txt"] {
        write(sandbox.path(), relative, b"needle\n");
    }

    // When: the same search is serialized thirty times.
    let outputs: Vec<Vec<u8>> = (0..30)
        .map(|_| {
            let root = fs::canonicalize(sandbox.path()).expect("sandbox root canonicalizes");
            let result = search(&root, &plan("needle")).expect("search succeeds");
            serde_json::to_vec(&result).expect("result serializes")
        })
        .collect();
    let hashes: Vec<String> = outputs
        .iter()
        .map(|output| hex::encode(Sha256::digest(output)))
        .collect();

    // Then: every authoritative byte string is identical.
    let first = hashes.first().expect("thirty hashes exist");
    println!("canonical_sha256={first}");
    assert_eq!(hashes.len(), 30);
    assert!(hashes.iter().all(|hash| hash == first));
}

proptest! {
    #[test]
    fn search_json_is_independent_of_randomized_creation_order(keys in any::<[u16; 3]>()) {
        // Given: identical corpora created in fixed and randomized orders.
        let fixed = TempDir::new().expect("fixed sandbox is created");
        let randomized = TempDir::new().expect("randomized sandbox is created");
        let files = [("z.txt", b"needle z".as_slice()), ("a.txt", b"needle a".as_slice()), ("m.txt", b"needle m".as_slice())];
        for (path, content) in files {
            write(fixed.path(), path, content);
        }
        let mut keyed: Vec<_> = keys.into_iter().zip(files).collect();
        keyed.sort_by_key(|(key, (path, _))| (*key, *path));
        for (_, (path, content)) in keyed {
            write(randomized.path(), path, content);
        }

        // When: both roots are searched with the same bounded plan.
        let query = plan("needle");
        let fixed_root = fs::canonicalize(fixed.path()).expect("fixed root canonicalizes");
        let randomized_root =
            fs::canonicalize(randomized.path()).expect("random root canonicalizes");
        let fixed_json = serde_json::to_vec(&search(&fixed_root, &query).expect("fixed search succeeds"))
            .expect("fixed result serializes");
        let randomized_json =
            serde_json::to_vec(&search(&randomized_root, &query).expect("random search succeeds"))
                .expect("random result serializes");

        // Then: filesystem creation order cannot affect canonical JSON.
        prop_assert_eq!(fixed_json, randomized_json);
    }
}

#[test]
fn search_discloses_the_gap_when_total_byte_budget_is_exceeded() {
    // Given: a corpus larger than the explicit total-byte budget.
    let sandbox = TempDir::new().expect("sandbox is created");
    write(sandbox.path(), "large.txt", b"needle and more bytes\n");
    let mut query = plan("needle");
    query.budget.max_total_bytes = NonZeroUsize::new(4).expect("four is nonzero");

    // When: the bounded search reaches the total-byte limit.
    let root = fs::canonicalize(sandbox.path()).expect("sandbox root canonicalizes");
    let result = search(&root, &query).expect("partial search is returned");

    // Then: no over-budget evidence is returned and the omitted path is explicit.
    assert!(result.blocks.is_empty());
    assert_eq!(result.skipped.len(), 1);
    assert_eq!(
        result.skipped.first().map(|gap| gap.reason),
        Some(SkipReason::Budget)
    );
}
