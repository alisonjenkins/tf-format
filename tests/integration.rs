#![deny(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::Path;

fn run_fixture(name: &str) {
    let dir = Path::new("tests/fixtures").join(name);
    let input = fs::read_to_string(dir.join("input.tf"))
        .unwrap_or_else(|e| panic!("failed to read input.tf for fixture '{name}': {e}"));
    let expected = fs::read_to_string(dir.join("expected.tf"))
        .unwrap_or_else(|e| panic!("failed to read expected.tf for fixture '{name}': {e}"));

    let actual = tf_format::format_hcl(&input)
        .unwrap_or_else(|e| panic!("format_hcl failed for fixture '{name}': {e}"));

    pretty_assertions::assert_eq!(actual, expected, "Fixture '{name}' output mismatch");

    // Verify idempotency: formatting twice should produce the same result
    let double_formatted = tf_format::format_hcl(&actual)
        .unwrap_or_else(|e| panic!("second format_hcl failed for fixture '{name}': {e}"));
    pretty_assertions::assert_eq!(
        double_formatted,
        actual,
        "Fixture '{name}' is not idempotent"
    );
}

#[test]
fn fixture_simple_resource() {
    run_fixture("simple_resource");
}

#[test]
fn fixture_single_multi_ordering() {
    run_fixture("single_multi_ordering");
}

#[test]
fn fixture_nested_blocks() {
    run_fixture("nested_blocks");
}

#[test]
fn fixture_comments_preserved() {
    run_fixture("comments_preserved");
}

#[test]
fn fixture_variables_sorted() {
    run_fixture("variables_sorted");
}

#[test]
fn fixture_idempotency() {
    run_fixture("idempotency");
}

#[test]
fn fixture_alignment() {
    run_fixture("alignment");
}

#[test]
fn fixture_meta_arguments() {
    run_fixture("meta_arguments");
}
