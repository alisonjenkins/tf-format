//! Library-level edge-case tests for degenerate or unusual inputs:
//! empty / whitespace-only files, BOM prefixes, and heredoc/CRLF
//! handling. These pin invariants (idempotence, no-op on empty,
//! semantics never modified) that the fixture suites don't cover.

#![deny(clippy::unwrap_used, clippy::expect_used)]

use tf_format::error::FormatError;
use tf_format::{FormatOptions, format_hcl, format_hcl_with};

fn fmt(input: &str) -> String {
    format_hcl(input).unwrap_or_else(|e| panic!("format_hcl failed: {e}"))
}

#[test]
fn empty_input_stays_empty() {
    // An empty file must be a byte-identical no-op (matching terraform fmt),
    // not rewritten to a lone newline.
    assert_eq!(fmt(""), "");
}

#[test]
fn whitespace_only_input_is_idempotent() {
    let once = fmt("   \n\n  \t\n");
    let twice = fmt(&once);
    assert_eq!(once, twice, "whitespace-only formatting must be idempotent");
    assert_eq!(once, "", "whitespace-only input collapses to empty");
}

#[test]
fn bom_prefix_is_stripped() {
    // A leading UTF-8 BOM must be tolerated (terraform fmt does) and removed.
    let out = fmt("\u{feff}variable \"a\" {}\n");
    assert_eq!(out, "variable \"a\" {}\n");
    assert!(!out.starts_with('\u{feff}'));
}

#[test]
fn heredoc_trailing_whitespace_preserved() {
    // Heredoc bodies are literal string data: trailing whitespace and tabs
    // inside them must survive formatting unchanged.
    let input = "locals {\n  s = <<-EOT\n    line with spaces   \n    \ttab\t\n  EOT\n}\n";
    let out = fmt(input);
    assert!(
        out.contains("line with spaces   \n"),
        "trailing spaces in heredoc body were stripped: {out:?}"
    );
    assert!(
        out.contains("\ttab\t\n"),
        "trailing tab in heredoc body was stripped: {out:?}"
    );
    // And idempotent.
    assert_eq!(fmt(&out), out);
}

#[test]
fn crlf_is_normalized_to_lf_and_idempotent() {
    let out = fmt("variable \"b\" {}\r\nvariable \"a\" {}\r\n");
    assert!(
        !out.contains('\r'),
        "CRLF should be normalized to LF: {out:?}"
    );
    assert_eq!(
        fmt(&out),
        out,
        "post-normalization output must be idempotent"
    );
}

#[test]
fn non_ascii_keys_round_trip_idempotently() {
    // Multibyte identifiers/keys must survive formatting and be idempotent.
    // (Column alignment of multibyte keys is measured in bytes today; this
    // test pins round-tripping, not the exact padding.)
    let input = "locals {\n  obj = {\n    \u{e9}_key = 1\n    a_key  = 2\n  }\n}\n";
    let once = fmt(input);
    assert!(once.contains('\u{e9}'), "non-ASCII key was lost: {once:?}");
    assert_eq!(
        fmt(&once),
        once,
        "non-ASCII key formatting must be idempotent"
    );
}

#[test]
fn opinionated_drops_array_blank_lines_but_keeps_comments() {
    // issue #35: opinionated mode removes every blank line inside a multi-line
    // array (between elements and before `]`), but comments must survive.
    let input = "x = [\n  \"a\",\n\n  # keep me\n  \"b\",\n\n\n]\n";
    let out = fmt(input);
    assert_eq!(out, "x = [\n  \"a\",\n  # keep me\n  \"b\",\n]\n");
    assert_eq!(
        fmt(&out),
        out,
        "array blank-line cleanup must be idempotent"
    );
}

#[test]
fn opinionated_strips_blank_lines_before_closing_brace() {
    // issue #35: trailing blank lines before a block's closing `}` are removed
    // in opinionated mode, at every nesting level.
    let input = "resource \"a\" \"b\" {\n  nested {\n    x = 1\n\n\n  }\n\n\n}\n";
    let out = fmt(input);
    assert_eq!(
        out,
        "resource \"a\" \"b\" {\n  nested {\n    x = 1\n  }\n}\n"
    );
    assert_eq!(fmt(&out), out);
}

#[test]
fn opinionated_does_not_strip_blank_lines_inside_heredoc() {
    // A `}` line or blank line *inside* a heredoc body is literal data and must
    // not be touched by the closing-brace blank-line cleanup.
    let input = "locals {\n  x = <<EOT\n    foo\n\n}\n  EOT\n\n\n}\n";
    let out = fmt(input);
    assert!(
        out.contains("    foo\n\n}\n"),
        "heredoc body blank line / brace line was altered: {out:?}"
    );
    // The blank lines AFTER the heredoc closes, before the block `}`, are gone.
    assert!(
        out.ends_with("  EOT\n}\n"),
        "trailing blanks not stripped: {out:?}"
    );
    assert_eq!(fmt(&out), out);
}

#[test]
fn indented_heredoc_marker_preserved_when_body_has_zero_indent_line() {
    // issue #43: hcl-edit drops the `-` from `<<-EOT` when a body line has no
    // leading whitespace (nothing to dedent). `terraform fmt` / `tofu fmt`
    // preserve the literal marker, so tf-format must too — in both styles.
    let input = "locals {\n  x = <<-EOT\n    foo\n}\n  EOT\n}\n";

    let out = fmt(input);
    assert!(
        out.contains("<<-EOT"),
        "opinionated dropped the `<<-` marker: {out:?}"
    );
    assert_eq!(fmt(&out), out, "marker preservation must be idempotent");

    let out_min = format_hcl_with(input, &FormatOptions::minimal())
        .unwrap_or_else(|e| panic!("minimal format failed: {e}"));
    assert!(
        out_min.contains("<<-EOT"),
        "minimal dropped the `<<-` marker: {out_min:?}"
    );
    // Body content is unchanged — the marker change is cosmetic, never lossy.
    assert!(
        out_min.contains("    foo\n}\n"),
        "heredoc body altered: {out_min:?}"
    );
}

#[test]
fn heredoc_marker_restore_visits_for_cond_and_index_positions() {
    // Marker restoration pairs a text scan of `<<` / `<<-` openers with a
    // depth-first AST walk. Expression positions the walk skipped (a for
    // expression's `if` clause, a traversal index) desynced the pairing, so
    // a later `<<-EOT` whose `-` hcl-edit dropped (zero-indent body line)
    // was restored from the WRONG marker and downgraded to `<<EOT`.
    let input = "a = [for x in y : x if x != <<EOT\nv\nEOT\n]\nb = <<-EOT\nzero\nEOT\n";
    for opts in [FormatOptions::opinionated(), FormatOptions::minimal()] {
        let out = format_hcl_with(input, &opts)
            .unwrap_or_else(|e| panic!("format failed ({:?}): {e}", opts.style));
        assert!(
            out.contains("b = <<-EOT"),
            "`<<-` marker lost after for-cond heredoc ({:?}): {out:?}",
            opts.style
        );
    }

    let input = "a = x[<<EOT\nv\nEOT\n]\nb = <<-EOT\nzero\nEOT\n";
    for opts in [FormatOptions::opinionated(), FormatOptions::minimal()] {
        let out = format_hcl_with(input, &opts)
            .unwrap_or_else(|e| panic!("format failed ({:?}): {e}", opts.style));
        assert!(
            out.contains("b = <<-EOT"),
            "`<<-` marker lost after index heredoc ({:?}): {out:?}",
            opts.style
        );
    }
}

#[test]
fn plain_heredoc_marker_not_promoted_to_indented() {
    // The inverse must hold: a plain `<<EOT` must never gain a `-`.
    let input = "locals {\n  x = <<EOT\nfoo\n  EOT\n}\n";
    let out = fmt(input);
    assert!(out.contains("<<EOT"), "marker changed: {out:?}");
    assert!(!out.contains("<<-EOT"), "plain heredoc gained `-`: {out:?}");
}

#[test]
fn duplicate_object_keys_refuse_to_format() {
    // hcl-edit's object model is a map, so the PARSER collapses duplicate
    // keys — formatting used to silently emit `x = {  a = 2\n}`, deleting
    // the user's first entry. The duplicate can't be preserved (it's gone
    // before we see the AST), so formatting must refuse with a typed error
    // instead of corrupting the file. (Duplicate object keys are invalid
    // Terraform anyway — `terraform validate` rejects them.)
    let input = "x = {\n  a = 1\n  a = 2\n}\n";
    for opts in [FormatOptions::opinionated(), FormatOptions::minimal()] {
        match format_hcl_with(input, &opts) {
            Ok(out) => panic!(
                "expected LossyParse error ({:?}), got output: {out:?}",
                opts.style
            ),
            Err(e) => assert!(
                matches!(e, FormatError::LossyParse),
                "expected FormatError::LossyParse ({:?}), got: {e:?}",
                opts.style
            ),
        }
    }
}

#[test]
fn invalid_hcl_returns_typed_parse_error() {
    // An unparseable input must surface a typed error, not panic.
    let err = match format_hcl("variable \"a\" {") {
        Ok(out) => panic!("expected a parse error, got output: {out:?}"),
        Err(e) => e,
    };
    assert!(
        matches!(err, FormatError::ParseHcl(_)),
        "expected FormatError::ParseHcl, got: {err:?}"
    );
    // The Display impl should mention parsing.
    assert!(format!("{err}").contains("parse"), "error display: {err}");
}

#[test]
fn empty_input_idempotent_in_minimal_style() {
    let opts = FormatOptions::minimal();
    let once = format_hcl_with("", &opts).unwrap_or_else(|e| panic!("minimal format failed: {e}"));
    assert_eq!(once, "");
}
