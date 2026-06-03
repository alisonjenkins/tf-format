//! Library-level edge-case tests for degenerate or unusual inputs:
//! empty / whitespace-only files, BOM prefixes, and heredoc/CRLF
//! handling. These pin invariants (idempotence, no-op on empty,
//! semantics never modified) that the fixture suites don't cover.

#![deny(clippy::unwrap_used, clippy::expect_used)]

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
fn empty_input_idempotent_in_minimal_style() {
    let opts = FormatOptions::minimal();
    let once = format_hcl_with("", &opts).unwrap_or_else(|e| panic!("minimal format failed: {e}"));
    assert_eq!(once, "");
}
