//! Integration tests for `FormatStyle::Minimal` — `terraform fmt`
//! / `tofu fmt` parity mode. Each fixture pair under
//! `tests/fixtures-minimal/<name>/{input.tf,expected.tf}` captures
//! a behaviour we want to PIN: alignment fires, source order is
//! preserved, and no opinionated rewrite leaks through.

#![deny(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::Path;

use tf_format::{FormatOptions, format_hcl_with};

fn run_minimal_fixture(name: &str) {
    let dir = Path::new("tests/fixtures-minimal").join(name);
    let input = fs::read_to_string(dir.join("input.tf"))
        .unwrap_or_else(|e| panic!("failed to read input.tf for fixture '{name}': {e}"));
    let expected = fs::read_to_string(dir.join("expected.tf"))
        .unwrap_or_else(|e| panic!("failed to read expected.tf for fixture '{name}': {e}"));

    let opts = FormatOptions::minimal();
    let actual = format_hcl_with(&input, &opts)
        .unwrap_or_else(|e| panic!("format_hcl_with failed for fixture '{name}': {e}"));

    pretty_assertions::assert_eq!(actual, expected, "Fixture '{name}' output mismatch");

    // Idempotency.
    let double_formatted = format_hcl_with(&actual, &opts)
        .unwrap_or_else(|e| panic!("second format_hcl_with failed for fixture '{name}': {e}"));
    pretty_assertions::assert_eq!(
        double_formatted,
        actual,
        "Fixture '{name}' is not idempotent under minimal style"
    );
}

#[test]
fn fixture_order_preserved() {
    // Three resources written z, b, a — minimal style must NOT
    // reorder them.
    run_minimal_fixture("order_preserved");
}

#[test]
fn fixture_no_meta_hoisting() {
    // `count` written AFTER `ami` — opinionated would hoist
    // `count` to the top of the block; minimal preserves the
    // user's order.
    run_minimal_fixture("no_meta_hoisting");
}

#[test]
fn fixture_equals_alignment() {
    // Mis-aligned `=` signs become column-aligned in minimal mode
    // (this is the spacing transform tofu fmt also applies).
    run_minimal_fixture("equals_alignment");
}

#[test]
fn fixture_object_keys_unsorted() {
    // Object keys written z, a, m — minimal style must NOT
    // alphabetise.
    run_minimal_fixture("object_keys_unsorted");
}

#[test]
fn fixture_no_trailing_comma() {
    // Multi-line array without a trailing comma — minimal style
    // must NOT add one (opinionated would).
    run_minimal_fixture("no_trailing_comma");
}

#[test]
fn fixture_blank_line_groups() {
    // Author-placed blank lines split alignment groups; each
    // group aligns independently.
    run_minimal_fixture("blank_line_groups");
}

#[test]
fn fixture_comments_preserved() {
    // Comments break alignment runs and travel with their
    // associated attribute. Source order preserved.
    run_minimal_fixture("comments_preserved");
}

#[test]
fn fixture_nested_blocks() {
    // Nested block bodies get the same minimal treatment —
    // alignment yes, sorting no.
    run_minimal_fixture("nested_blocks");
}

#[test]
fn fixture_wide_object_not_expanded() {
    // Single-line object that would exceed 80 columns — minimal
    // mode leaves it on one line (opinionated would expand).
    run_minimal_fixture("wide_object_not_expanded");
}
