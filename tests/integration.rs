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

#[test]
fn fixture_array_of_objects() {
    run_fixture("array_of_objects");
}

#[test]
fn fixture_trailing_comma() {
    run_fixture("trailing_comma");
}

#[test]
fn fixture_list_order_preserved() {
    run_fixture("list_order_preserved");
}

#[test]
fn fixture_expand_wide_object() {
    run_fixture("expand_wide_object");
}

#[test]
fn fixture_func_call_object() {
    run_fixture("func_call_object");
}

#[test]
fn fixture_blank_line_groups() {
    run_fixture("blank_line_groups");
}

#[test]
fn fixture_tfvars_singles_only() {
    run_fixture("tfvars_singles_only");
}

#[test]
fn fixture_tfvars_multiline_sorted() {
    run_fixture("tfvars_multiline_sorted");
}

#[test]
fn fixture_tfvars_blank_line_groups() {
    run_fixture("tfvars_blank_line_groups");
}

#[test]
fn fixture_tfvars_object_recursion() {
    run_fixture("tfvars_object_recursion");
}

#[test]
fn fixture_tfvars_with_top_level_comment() {
    run_fixture("tfvars_with_top_level_comment");
}

#[test]
fn fixture_colon_assignment_rewrite() {
    // Issue #18: opinionated mode should rewrite `:` object
    // separators to `=` (canonical form) and then column-align
    // uniformly. Reporter's exact repro pinned here.
    run_fixture("colon_assignment_rewrite");
}

#[test]
fn fixture_funccall_multiline_object_arg() {
    // Regression: a multi-line FuncCall whose arg is a multi-
    // line object literal must indent the object's keys at
    // call_depth + 2 (one inside the call's `(`, one inside
    // the object's `{`). The bug had keys at call_depth + 1.
    run_fixture("funccall_multiline_object_arg");
}

#[test]
fn fixture_for_expr_value_indent() {
    // Regression: the value position of an object for-expression
    // (`{ for x in C : K => { ... } }`) must indent its inner
    // members one level DEEPER than the for-line, not at the
    // same column. Recurse into for_expr.value_expr at depth+1.
    run_fixture("for_expr_value_indent");
}

#[test]
fn fixture_opinionated_collapses_blank_groups() {
    // Regression: opinionated mode must IGNORE author blank
    // lines inside a block. All single-line attrs collapse into
    // one tier sorted alphabetically; multi-line attrs collapse
    // into the next tier sorted alphabetically. Blank-line
    // group preservation is a Minimal-mode-only behaviour.
    run_fixture("opinionated_collapses_blank_groups");
}
