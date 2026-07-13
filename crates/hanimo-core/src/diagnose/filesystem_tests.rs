use std::{cell::Cell, fs, os::unix::fs::symlink};

use tempfile::TempDir;

use super::{DiagnoseBudget, DiagnoseError, ScanCallbacks, scan_sources_with_hook};

#[test]
fn scan_rejects_a_regular_file_swapped_to_an_external_symlink_before_open() {
    // Given: discovery sees a regular file and an external source holds a sentinel.
    let sandbox = TempDir::new().expect("sandbox is created");
    let root = sandbox.path().join("root");
    fs::create_dir(&root).expect("root is created");
    let victim = root.join("victim.py");
    let outside = sandbox.path().join("outside.py");
    fs::write(&victim, b"safe\n").expect("victim is written");
    fs::write(&outside, b"OUTSIDE_SENTINEL\n").expect("outside source is written");
    let swapped = Cell::new(false);
    let mut observed = Vec::new();

    // When: the deterministic seam swaps the candidate immediately before capability open.
    let canonical_root = fs::canonicalize(&root).expect("diagnosis root canonicalizes");
    let result = scan_sources_with_hook(
        &canonical_root,
        DiagnoseBudget::default(),
        ScanCallbacks {
            visit: |_: &str, bytes: &[u8]| observed.extend_from_slice(bytes),
            before_open: |_: &std::path::Path| {
                if !swapped.replace(true) {
                    fs::remove_file(&victim).expect("victim is removed before open");
                    symlink(&outside, &victim).expect("external symlink replaces victim");
                }
            },
        },
    );

    // Then: no-follow reopening fails closed and no external bytes reach diagnosis state.
    assert!(matches!(result, Err(DiagnoseError::Read(_))));
    assert!(
        !observed
            .windows(16)
            .any(|bytes| bytes == b"OUTSIDE_SENTINEL")
    );
}
