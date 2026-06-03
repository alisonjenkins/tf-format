//! Binary-level integration tests driving the compiled `tf-format`
//! CLI: file discovery, `--check` exit codes, and write behaviour.
//! Library-level formatting is covered by the other suites; these
//! pin the `main.rs` invariants that those can't reach.

#![deny(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use tempfile::TempDir;

/// Unformatted input (needs reordering under the opinionated style).
const DIRTY: &str = "variable \"b\" {}\nvariable \"a\" {}\n";

fn tf() -> Command {
    Command::cargo_bin("tf-format")
        .unwrap_or_else(|e| panic!("failed to locate tf-format binary: {e}"))
}

fn tmpdir() -> TempDir {
    TempDir::new().unwrap_or_else(|e| panic!("failed to create temp dir: {e}"))
}

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|e| panic!("failed to create {}: {e}", parent.display()));
    }
    fs::write(path, contents).unwrap_or_else(|e| panic!("failed to write {}: {e}", path.display()));
}

#[test]
fn discovers_all_supported_extensions() {
    let dir = tmpdir();
    for name in ["main.tf", "vars.tfvars", "stack.tofu"] {
        write(&dir.path().join(name), DIRTY);
    }
    // Unrelated file must be ignored.
    write(&dir.path().join("README.md"), "# not terraform\n");

    // --check should flag all three supported files and exit non-zero.
    let assert = tf().arg("--check").arg(dir.path()).assert().failure();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();

    assert!(stderr.contains("main.tf"), "stderr: {stderr}");
    assert!(stderr.contains("vars.tfvars"), "stderr: {stderr}");
    assert!(stderr.contains("stack.tofu"), "stderr: {stderr}");
    assert!(!stderr.contains("README.md"), "stderr: {stderr}");
    assert!(stderr.contains("3 file(s)"), "stderr: {stderr}");
}

#[test]
fn skips_dot_directories() {
    let dir = tmpdir();
    write(&dir.path().join("main.tf"), DIRTY);
    // A vendored module under .terraform must NOT be discovered.
    write(&dir.path().join(".terraform/modules/vendored.tf"), DIRTY);

    let assert = tf().arg("--check").arg(dir.path()).assert().failure();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();

    assert!(stderr.contains("main.tf"), "stderr: {stderr}");
    assert!(!stderr.contains("vendored.tf"), "stderr: {stderr}");
    assert!(stderr.contains("1 file(s)"), "stderr: {stderr}");
}

#[test]
fn deduplicates_repeated_paths() {
    let dir = tmpdir();
    let file = dir.path().join("main.tf");
    write(&file, DIRTY);

    // The same file passed twice must be reported once.
    let assert = tf().arg("--check").arg(&file).arg(&file).assert().failure();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();

    assert!(stderr.contains("1 file(s)"), "stderr: {stderr}");
    assert_eq!(
        stderr.matches("main.tf").count(),
        1,
        "path listed more than once: {stderr}"
    );
}

#[test]
fn nonexistent_literal_path_errors() {
    let dir = tmpdir();
    let missing = dir.path().join("does_not_exist.tf");

    let assert = tf().arg("--check").arg(&missing).assert().failure();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();

    assert!(stderr.contains("does not exist"), "stderr: {stderr}");
}

#[test]
fn clean_file_passes_check() {
    let dir = tmpdir();
    // Already canonical: single var, nothing to reorder or align.
    write(&dir.path().join("main.tf"), "variable \"a\" {}\n");

    tf().arg("--check").arg(dir.path()).assert().success();
}

#[cfg(unix)]
#[test]
fn symlink_cycle_terminates() {
    let dir = tmpdir();
    write(&dir.path().join("main.tf"), DIRTY);
    // A symlink pointing back at its parent would loop a follow-symlinks walk.
    std::os::unix::fs::symlink(dir.path(), dir.path().join("loop"))
        .unwrap_or_else(|e| panic!("failed to create symlink: {e}"));

    // Must terminate and report the single real file exactly once.
    let assert = tf()
        .arg("--check")
        .arg(dir.path())
        .timeout(std::time::Duration::from_secs(10))
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();

    assert!(stderr.contains("1 file(s)"), "stderr: {stderr}");
}
