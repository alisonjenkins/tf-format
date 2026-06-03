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
