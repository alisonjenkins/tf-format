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
fn fixture_empty_lines_in_arrays() {
    run_fixture("empty_lines_in_arrays");
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
fn fixture_for_expr_colon_spacing() {
    // Regression: squished for-expression separators (`var.x:x`,
    // `k=>v`) must be normalized to a single space around `:` and
    // `=>`, matching `terraform fmt`. Wrapped (multi-line) bodies and
    // inline comments are left untouched.
    run_fixture("for_expr_colon_spacing");
}

#[test]
fn fixture_sort_output_blocks() {
    // TEST-4: a run of `output` blocks must sort alphabetically by name.
    run_fixture("sort_output_blocks");
}

#[test]
fn fixture_sort_data_type_then_name() {
    // TEST-4: `data` blocks sort by type, then by name within a type.
    run_fixture("sort_data_type_then_name");
}

#[test]
fn fixture_sort_resource_name_tiebreak() {
    // TEST-4: same-type resources sort on the second label (name),
    // exercising label_sort_key's tie-break.
    run_fixture("sort_resource_name_tiebreak");
}

#[test]
fn fixture_preserve_module_order() {
    // TEST-4 (negative): `module` blocks are NOT reordered, while an
    // adjacent `variable` run still sorts.
    run_fixture("preserve_module_order");
}

#[test]
fn fixture_depends_on_priority() {
    // TEST-8: single-line meta-args keep their fixed PRIORITY_ATTRS
    // order (count, provider, depends_on), not alphabetical.
    run_fixture("depends_on_priority");
}

#[test]
fn fixture_whitespace_normalization() {
    // TEST-7: trailing whitespace, tab indentation, and trailing blank
    // lines normalize to 2-space indent + single trailing newline.
    run_fixture("whitespace_normalization");
}

#[test]
fn fixture_expand_object_with_key_prefix() {
    // IMP-2: the single-line-object expansion threshold must count the
    // `key = ` prefix. This object literal is under 80 columns on its
    // own but crosses the budget once the long attribute name is
    // included, so it must expand. The same object under a short key
    // stays inline (see the short-key case in the CLI/edge tests).
    run_fixture("expand_object_with_key_prefix");
}

#[test]
fn fixture_object_comma_normalized() {
    // BUG-5 regression: sorting a multi-line object whose entries mix
    // comma and no-comma terminators must normalize them to one
    // canonical form (newline-terminated, no trailing commas) rather
    // than leaving a stray no-comma entry stranded in the middle.
    run_fixture("object_comma_normalized");
}

#[test]
fn fixture_file_header_stays_on_top() {
    // A file-header comment (blank-separated from the first block) lives in
    // that block's prefix decor, so sorting the run used to drag the header
    // into the middle of the file with its block. The header must stay at
    // the top; a comment hugging its block (no blank) still travels with it.
    run_fixture("file_header_stays_on_top");
}

#[test]
fn fixture_object_inline_comments() {
    // Regression: an inline comment trailing an object entry's value
    // (`a = 1 # note`) lives in the value's suffix decor; terminator
    // normalization used to wipe that suffix, deleting the comment
    // (Rule 7 violation). Comments must survive the sort and stay on
    // their entry's line.
    run_fixture("object_inline_comments");
}

#[test]
fn fixture_block_comments() {
    // BUG-1 regression: multi-line `/* ... */` block comments must
    // survive reordering intact — over a reordered attribute, over a
    // reordered object key, and a star-aligned block — without being
    // truncated into unparseable output. Idempotency is asserted by
    // run_fixture.
    run_fixture("block_comments");
}

#[test]
fn fixture_heredoc_preserved() {
    // BUG-2 regression: trailing whitespace, tabs, and blank lines
    // inside heredoc bodies (`<<EOT` / `<<-EOT`) are literal string
    // content and must NOT be stripped by whitespace post-processing.
    run_fixture("heredoc_preserved");
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

#[test]
fn fixture_nested_block_no_hoist() {
    // Issue #30: priority attribute hoisting is block-type-aware.
    // `version` inside `secret_key_ref` (a nested provider-specific
    // block) is a plain domain attribute, not a meta-argument — it
    // must stay alongside its sibling `secret` and not get hoisted.
    run_fixture("nested_block_no_hoist");
}

#[test]
fn fixture_module_providers_hoisted() {
    // Issue #30: `providers` (plural) is a `module`-only meta-arg
    // and should hoist alongside source/version.
    run_fixture("module_providers_hoisted");
}

#[test]
fn fixture_output_depends_on() {
    // Issue #30: only `depends_on` is a meta-arg on `output` blocks.
    // Other attrs (description/value/sensitive) stay in the normal
    // tier.
    run_fixture("output_depends_on");
}

#[test]
fn fixture_import_meta_args() {
    // Issue #30: `import` block hoists `for_each` and `provider`
    // (no `depends_on`, no `version`).
    run_fixture("import_meta_args");
}

#[test]
fn fixture_provider_no_version_hoist() {
    // Issue #30: `version` inside `provider` blocks (the deprecated
    // form) is no longer hoisted — only `for_each` is a meta-arg
    // here (per OpenTofu). `version` is treated as a normal attr.
    run_fixture("provider_no_version_hoist");
}

#[test]
fn fixture_nested_inline_comments() {
    // Issue #53: an inline `#` comment on an object entry must survive,
    // including when the entry is comma-terminated (the comment then parses
    // into the next entry's prefix, or the object trailing for the last
    // entry) and at multiple nesting levels.
    run_fixture("nested_inline_comments");
}

#[test]
fn fixture_single_line_block() {
    // Single-line block bodies (e.g. `mock_provider "x" { alias = "y" }`,
    // as seen in .tftest files) keep exactly ONE space after `{`, matching
    // `terraform fmt`. Regression: the body prefix used to emit the full
    // indent (2 spaces) → `{  alias`. Also covers empty `{}` and a
    // one-line block nested inside a multi-line body.
    run_fixture("single_line_block");
}

#[test]
fn fixture_dynamic_meta_arguments() {
    // Issue #55: inside a `dynamic` block, `for_each` / `iterator` /
    // `labels` are meta-args and must hoist ahead of `content` — even when
    // `for_each` is multi-line (it used to alphabetise next to `content`,
    // landing after it). Covers single- and multi-line `for_each`, all three
    // iteration args, and a `dynamic` nested inside another's `content`.
    run_fixture("dynamic_meta_arguments");
}

#[test]
fn fixture_map_trailing_commas_normalized() {
    // Issue #54: a multi-line-value map/object is normalised to no trailing
    // commas with a blank line between entries, regardless of whether the
    // author wrote trailing commas — so structurally identical maps format
    // identically. Covers the comma and no-comma forms converging, plus a
    // mixed single-line/multi-line-entry map.
    run_fixture("map_trailing_commas_normalized");
}
