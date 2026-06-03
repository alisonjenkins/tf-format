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

/// Unparseable input (unterminated block).
const BROKEN: &str = "variable \"a\" {\n";

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

#[test]
fn check_continues_past_broken_file() {
    let dir = tmpdir();
    write(&dir.path().join("broken.tf"), BROKEN);
    write(&dir.path().join("good.tf"), DIRTY);

    let assert = tf().arg("--check").arg(dir.path()).assert().failure();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();

    // The broken file is reported, the good file is still checked and listed.
    assert!(stderr.contains("broken.tf"), "stderr: {stderr}");
    assert!(stderr.contains("good.tf"), "stderr: {stderr}");
    assert!(stderr.contains("failed to process"), "stderr: {stderr}");
}

#[test]
fn write_formats_good_files_despite_broken_file() {
    let dir = tmpdir();
    write(&dir.path().join("broken.tf"), BROKEN);
    let good = dir.path().join("good.tf");
    write(&good, DIRTY);

    // Exit non-zero because one file failed...
    tf().arg(dir.path()).assert().failure();

    // ...but the healthy file was still formatted in place.
    let formatted =
        fs::read_to_string(&good).unwrap_or_else(|e| panic!("failed to read back good.tf: {e}"));
    assert_eq!(formatted, "variable \"a\" {}\n\nvariable \"b\" {}\n");
}

#[test]
fn stdin_check_dirty_exits_nonzero_without_body() {
    let assert = tf()
        .arg("--stdin")
        .arg("--check")
        .write_stdin(DIRTY)
        .assert()
        .failure();
    // Must not print the formatted body on the output channel.
    assert!(
        assert.get_output().stdout.is_empty(),
        "stdout should be empty in --stdin --check mode"
    );
}

#[test]
fn stdin_check_clean_exits_zero() {
    tf().arg("--stdin")
        .arg("--check")
        .write_stdin("variable \"a\" {}\n")
        .assert()
        .success();
}

#[test]
fn stdin_diff_reports_inserted_lines() {
    let assert = tf()
        .arg("--stdin")
        .arg("--diff")
        .write_stdin(DIRTY)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();

    // A real unified diff: headers, a hunk, and the inserted blank line that
    // the old line-zipping implementation dropped.
    assert!(stdout.contains("@@"), "stdout: {stdout}");
    assert!(stdout.contains("-variable \"b\" {}"), "stdout: {stdout}");
    assert!(stdout.contains("+variable \"b\" {}"), "stdout: {stdout}");
}

#[test]
fn bom_prefixed_file_is_formatted_not_errored() {
    let dir = tmpdir();
    let file = dir.path().join("main.tf");
    // A leading UTF-8 BOM used to make hcl-edit's parser error out, so the
    // file was never formatted. It must now format cleanly (BOM stripped).
    write(&file, "\u{feff}variable \"a\" {}\n");

    tf().arg(&file).assert().success();

    let formatted =
        fs::read_to_string(&file).unwrap_or_else(|e| panic!("failed to read back main.tf: {e}"));
    assert_eq!(formatted, "variable \"a\" {}\n");
    assert!(
        !formatted.starts_with('\u{feff}'),
        "BOM should have been stripped"
    );
}

#[cfg(unix)]
#[test]
fn in_place_write_preserves_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tmpdir();
    let file = dir.path().join("main.tf");
    write(&file, DIRTY);
    fs::set_permissions(&file, fs::Permissions::from_mode(0o640))
        .unwrap_or_else(|e| panic!("failed to set perms: {e}"));

    tf().arg(&file).assert().success();

    let mode = fs::metadata(&file)
        .unwrap_or_else(|e| panic!("failed to stat: {e}"))
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o640, "atomic write should preserve file permissions");

    let formatted =
        fs::read_to_string(&file).unwrap_or_else(|e| panic!("failed to read back: {e}"));
    assert_eq!(formatted, "variable \"a\" {}\n\nvariable \"b\" {}\n");
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
