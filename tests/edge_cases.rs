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
fn crlf_blank_lines_split_alignment_groups_in_minimal() {
    // A blank line in CRLF input is `\r\n\r\n` in the decor. The blank-line
    // predicates only looked for `\n`-shaped patterns, so the two groups
    // below merged into one alignment run and `a` was padded to align with
    // `bb` — `tofu fmt` aligns the blank-separated groups independently.
    let input = "locals {\r\n  a = 1\r\n\r\n  bb = 2\r\n}\r\n";
    let out = format_hcl_with(input, &FormatOptions::minimal())
        .unwrap_or_else(|e| panic!("minimal format failed: {e}"));
    assert!(
        out.contains("  a = 1\n"),
        "`a` was padded across a CRLF blank line: {out:?}"
    );
    assert!(out.contains("  bb = 2\n"), "second group altered: {out:?}");
    let twice = format_hcl_with(&out, &FormatOptions::minimal())
        .unwrap_or_else(|e| panic!("second minimal format failed: {e}"));
    assert_eq!(twice, out, "must be idempotent");
}

#[test]
fn minimal_no_blank_inserted_between_block_and_top_level_attr() {
    // `tofu fmt` preserves the author's blank-line count exactly — zero
    // blanks between a block and a following top-level attribute stay zero.
    // The top-level attr-run handling used to force one blank in.
    let input = "locals {\n  a = 1\n}\nfoo = \"bar\"\n";
    let out = format_hcl_with(input, &FormatOptions::minimal())
        .unwrap_or_else(|e| panic!("minimal format failed: {e}"));
    assert_eq!(out, input, "minimal must be a no-op here (tofu parity)");

    // An author-written blank is still preserved, not collapsed.
    let input_blank = "locals {\n  a = 1\n}\n\nfoo = \"bar\"\n";
    let out_blank = format_hcl_with(input_blank, &FormatOptions::minimal())
        .unwrap_or_else(|e| panic!("minimal format failed: {e}"));
    assert_eq!(out_blank, input_blank, "author blank must be preserved");
}

#[test]
fn non_ascii_keys_round_trip_idempotently() {
    // Multibyte identifiers/keys must survive formatting and be idempotent.
    // (Padding correctness — rune-count alignment matching `tofu fmt` — is
    // pinned by the `multibyte_key_alignment` minimal fixture.)
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
fn heredoc_lookalike_inside_block_comment_is_ignored() {
    // A `x = <<EOT` line inside a `/* … */` block comment is comment text,
    // not a heredoc opener. The text scanners used to take it literally:
    // post_process treated the following lines as a heredoc body (keeping
    // trailing whitespace — Rule 8 violation) and the marker scan desynced,
    // restoring `<<-` markers onto the wrong heredoc.
    let input = "locals {\n  /*\n  x = <<EOT\n  */\n  a = 1   \n  b = 22\n}\n";
    let out = fmt(input);
    assert!(
        !out.contains("a = 1   "),
        "trailing whitespace kept after fake heredoc opener: {out:?}"
    );
    assert!(
        out.contains("x = <<EOT"),
        "comment content altered: {out:?}"
    );
    assert_eq!(fmt(&out), out, "must be idempotent");

    // And the marker pairing stays in sync: the real `<<-EOT` after the
    // commented-out opener keeps its `-` (zero-indent body would otherwise
    // be downgraded when paired with the fake opener's marker).
    let input = "/*\nfake = <<EOT\n*/\nlocals {\n  b = <<-EOT\nzero\nEOT\n}\n";
    for opts in [FormatOptions::opinionated(), FormatOptions::minimal()] {
        let out = format_hcl_with(input, &opts)
            .unwrap_or_else(|e| panic!("format failed ({:?}): {e}", opts.style));
        assert!(
            out.contains("<<-EOT"),
            "`<<-` marker lost after commented-out opener ({:?}): {out:?}",
            opts.style
        );
    }
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
fn opinionated_normalizes_stale_heredoc_padding() {
    // Opinionated mode places heredoc attributes in the multi-line tier,
    // outside any `=` alignment run — so source padding like
    // `foo       = <<EOT` used to survive verbatim, making the output
    // depend on the input's formatting. It must normalize to one space.
    let input = "resource \"a\" \"b\" {\n  foo       = <<EOT\nhi\nEOT\n  bar = 1\n}\n";
    let out = fmt(input);
    assert!(
        out.contains("foo = <<EOT"),
        "stale heredoc `=` padding survived: {out:?}"
    );
    assert_eq!(fmt(&out), out, "must be idempotent");
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
fn benign_whitespace_normalization_does_not_refuse() {
    // Regression: the data-loss guard used to byte-compare the re-encoded body
    // against the input, so any whitespace hcl-edit normalizes on parse (it has
    // no decor slot for spaces around `.` in a multi-segment traversal, tabs
    // around operators, or newlines in a boolean chain) tripped the guard and
    // refused VALID files with a misleading "duplicate keys" error. These must
    // now format successfully (no objects, no duplicate keys, no data loss).
    for input in [
        "a = data . x . y\n",
        "a = b[0] . c\n",
        "output \"x\" {\n  value = module . vpc . id\n}\n",
        "a = 1\t+\t2\n",
        "a = (x &&\n  y &&\n  z)\n",
    ] {
        for opts in [FormatOptions::opinionated(), FormatOptions::minimal()] {
            let out = match format_hcl_with(input, &opts) {
                Ok(out) => out,
                Err(e) => panic!("{:?}: refused valid input {input:?}: {e:?}", opts.style),
            };
            // And the result must be a fixpoint (no oscillation introduced by
            // removing the refusal).
            let twice = match format_hcl_with(&out, &opts) {
                Ok(twice) => twice,
                Err(e) => panic!(
                    "{:?}: second format failed for {input:?}: {e:?}",
                    opts.style
                ),
            };
            assert_eq!(twice, out, "{:?}: not idempotent for {input:?}", opts.style);
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
