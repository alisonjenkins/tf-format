#![deny(clippy::unwrap_used, clippy::expect_used)]

mod classify;
pub mod error;
mod formatter;

use error::FormatError;
use hcl_edit::structure::Body;

pub use formatter::FormatStyle;

/// Configuration for [`format_hcl_with`]. Use the [`Default`] impl
/// (or [`format_hcl`]) for tf-format's opinionated style; switch
/// to [`FormatOptions::minimal`] when you want only the alignment
/// + spacing transforms that `terraform fmt` / `tofu fmt` apply.
#[derive(Debug, Clone, Default)]
pub struct FormatOptions {
    pub style: FormatStyle,
}

impl FormatOptions {
    /// `terraform fmt` / `tofu fmt` parity: alignment + spacing only.
    /// No alphabetisation, no meta-arg hoisting, no opinionated
    /// rewrites. Source order is preserved.
    pub fn minimal() -> Self {
        Self {
            style: FormatStyle::Minimal,
        }
    }

    /// tf-format's full opinionated style — alphabetises blocks,
    /// hoists meta-arguments, sorts attributes, expands wide
    /// objects. Equivalent to constructing with [`Default`].
    pub fn opinionated() -> Self {
        Self {
            style: FormatStyle::Opinionated,
        }
    }
}

/// Format an HCL string with tf-format's full opinionated style.
/// Equivalent to [`format_hcl_with`] called with the default
/// [`FormatOptions`]. Kept as a stable, zero-config entry point
/// for callers that want the original behaviour.
pub fn format_hcl(input: &str) -> Result<String, FormatError> {
    format_hcl_with(input, &FormatOptions::default())
}

/// Format an HCL string with caller-chosen options.
///
/// With [`FormatStyle::Opinionated`] (the default), behaves
/// exactly like the historical [`format_hcl`]: sorts top-level
/// blocks alphabetically, hoists meta-arguments, sorts attributes
/// and object keys, expands wide single-line objects, etc.
///
/// With [`FormatStyle::Minimal`], applies only the alignment and
/// spacing transforms that `terraform fmt` / `tofu fmt` apply —
/// so source-order is preserved and no opinionated rewrites fire.
/// This is the right choice when you can't impose tf-format's
/// canonicalisation on a repo (e.g. when integrating with a
/// language server that needs to match `terraform fmt` output).
pub fn format_hcl_with(input: &str, opts: &FormatOptions) -> Result<String, FormatError> {
    // hcl-edit rejects a leading UTF-8 BOM, but `terraform fmt` tolerates it
    // and editors (notably on Windows) routinely emit one. Strip it before
    // parsing so a BOM-prefixed file formats instead of erroring.
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);

    // An empty or whitespace-only file has no content to format. Collapse it to
    // a stable empty result: this keeps an empty file byte-identical (so it is
    // a no-op under `--check`, matching `terraform fmt`) and makes a
    // whitespace-only file idempotent rather than oscillating between blank
    // lines and a single newline across passes.
    if input.trim().is_empty() {
        return Ok(String::new());
    }

    // Record each heredoc opener's marker (`<<` vs `<<-`) in source order
    // *before* parsing: hcl-edit drops the `-` on a `<<-EOT` whose body has a
    // zero-indent line, and the marker can't be recovered from the AST alone.
    let heredoc_markers = scan_heredoc_markers(input);

    let mut body: Body = input.parse()?;

    // Restore any `<<-` marker hcl-edit dropped, before sorting can reorder the
    // heredocs out of source order (issue #43).
    formatter::restore_heredoc_indent_markers(&mut body, &heredoc_markers);

    // hcl-edit can lose data its model cannot represent: an object with
    // duplicate keys is silently collapsed at parse time (`Object` is a map),
    // which would make formatting delete one of the user's entries. The
    // heredoc `<<-` marker drop is the one other known structural-lossy case,
    // and it was just restored above — so any remaining *structural* mismatch
    // means formatting would corrupt data. Refuse instead. (Duplicate object
    // keys are invalid Terraform anyway: `terraform validate` rejects them.)
    //
    // The comparison normalizes the two benign ways hcl-edit's round-trip
    // rewrites tokens without losing data, so only a *structural* mismatch
    // remains (a blunt comparison here used to refuse valid files with a
    // misleading duplicate-keys error):
    //
    //  1. Whitespace: hcl-edit normalizes interior whitespace it has no decor
    //     slot for (e.g. `a . b . c` → `a.b.c`, tabs/newlines around
    //     operators) and does not preserve CRLF. Both sides are compared with
    //     all whitespace stripped.
    //  2. String escape sequences: the parser decodes escapes (`\u00e9` → `é`,
    //     `\/` → `/`) and the encoder re-escapes only control characters, `\"`
    //     and `\\` — so a valid escape's raw token changes on round-trip with
    //     no data loss (issue #80). Both sides are compared with escapes
    //     decoded the way hcl-edit's parser decodes them.
    //
    // Heredoc bodies and comments are preserved verbatim on both sides, so
    // applying the same normalization to both cannot mask a structural change
    // (a dropped entry leaves its non-whitespace tokens missing).
    if normalize_for_loss_check(&body.to_string()) != normalize_for_loss_check(input) {
        return Err(FormatError::LossyParse);
    }

    // sort_top_level handles both block ordering and top-level attribute
    // formatting (as in `.tfvars` files), recursing into nested bodies.
    formatter::sort_top_level(&mut body, opts.style);

    Ok(post_process(&body.to_string(), opts.style))
}

/// Normalize a source text for the data-loss comparison in
/// [`format_hcl_with`]: decode HCL string escape sequences the way hcl-edit's
/// parser does, and drop all whitespace. Applied identically to both sides of
/// the comparison, so hcl-edit's escape normalization (decode-on-parse,
/// minimal re-escape-on-encode) no longer registers as a token difference,
/// while real data loss (dropped tokens) still does.
///
/// The decoder mirrors hcl-edit's `escaped_char` parser: `\n \r \t \\ \" \/
/// \b \f`, `\u` + exactly 4 hex digits, `\U` + exactly 8 hex digits. An
/// invalid escape is kept verbatim — inside a quoted string it would have
/// been a parse error before we got here, so it can only occur in comments
/// and heredoc bodies, which hcl-edit round-trips verbatim.
fn normalize_for_loss_check(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            continue;
        }
        if c != '\\' {
            out.push(c);
            continue;
        }
        let Some(&next) = chars.peek() else {
            out.push('\\');
            continue;
        };
        match next {
            // Decoded char is whitespace — dropped like any other whitespace.
            'n' | 'r' | 't' => {
                chars.next();
            }
            // NB: consuming the pair here is what keeps `\\u0041` (escaped
            // backslash, then literal text) from being read as a `\u` escape.
            '\\' | '"' | '/' => {
                chars.next();
                out.push(next);
            }
            'b' => {
                chars.next();
                out.push('\u{08}');
            }
            'f' => {
                chars.next();
                out.push('\u{0C}');
            }
            'u' | 'U' => {
                let len = if next == 'u' { 4 } else { 8 };
                // Parse on a lookahead clone so an invalid sequence consumes
                // nothing and falls through verbatim.
                let mut lookahead = chars.clone();
                lookahead.next(); // the `u` / `U`
                let mut hex = String::with_capacity(len);
                while hex.len() < len {
                    match lookahead.peek() {
                        Some(&h) if h.is_ascii_hexdigit() => {
                            hex.push(h);
                            lookahead.next();
                        }
                        _ => break,
                    }
                }
                let decoded = (hex.len() == len)
                    .then(|| u32::from_str_radix(&hex, 16).ok())
                    .flatten()
                    .and_then(char::from_u32);
                match decoded {
                    Some(ch) => {
                        chars = lookahead;
                        if !ch.is_whitespace() {
                            out.push(ch);
                        }
                    }
                    None => out.push('\\'),
                }
            }
            _ => out.push('\\'),
        }
    }
    out
}

/// Post-process the formatted output: strip trailing whitespace from each line
/// and ensure the file ends with exactly one newline.
///
/// Lines inside a heredoc body (`<<EOT` / `<<-EOT` … `EOT`) are literal string
/// data, so their trailing whitespace must be preserved — trimming it would
/// silently alter the rendered Terraform value. We track heredoc spans while
/// walking the rendered text and skip trimming inside them.
fn post_process(output: &str, style: FormatStyle) -> String {
    // Opinionated mode drops any blank line immediately preceding a closing
    // `}` / `]` (issue #35). `terraform fmt` / `tofu fmt` preserve those blanks,
    // so minimal mode leaves them alone. Array-interior blanks are already
    // removed at the AST level; this catches the block-body case (the blank
    // sits in a decor suffix whose owner varies by structure type) and acts as
    // a backstop for arrays.
    let strip_before_close = matches!(style, FormatStyle::Opinionated);

    let mut lines_out: Vec<String> = Vec::new();
    let mut heredoc_delim: Option<String> = None;
    let mut in_block_comment = false;
    // Blank lines seen outside a heredoc, held back until we know whether the
    // next non-blank line is a closing bracket (in which case they're dropped).
    let mut pending_blanks: Vec<String> = Vec::new();

    for line in output.lines() {
        match &heredoc_delim {
            // Inside a heredoc body: emit lines verbatim. The terminator line
            // (whose trimmed content equals the delimiter) closes the heredoc;
            // it is emitted verbatim too, since its indentation is significant
            // for the `<<-` form.
            Some(delim) => {
                lines_out.append(&mut pending_blanks);
                lines_out.push(line.to_string());
                if line.trim() == delim.as_str() {
                    heredoc_delim = None;
                }
            }
            // Outside a heredoc: strip trailing whitespace, then check whether
            // this line opens a heredoc. Detection runs on the comment-masked
            // line so `<<` / `}` inside comments never match.
            None => {
                let masked = mask_comments(line, &mut in_block_comment);
                let trimmed = line.trim_end();
                if trimmed.is_empty() {
                    pending_blanks.push(String::new());
                } else {
                    let starts_close = {
                        let t = masked.trim_start();
                        t.starts_with('}') || t.starts_with(']')
                    };
                    if strip_before_close && starts_close {
                        pending_blanks.clear();
                    } else {
                        lines_out.append(&mut pending_blanks);
                    }
                    lines_out.push(trimmed.to_string());
                    heredoc_delim = heredoc_open_delimiter(&masked);
                }
            }
        }
    }
    lines_out.append(&mut pending_blanks);

    // Collapse any trailing blank lines so the file ends with exactly one
    // newline (Rule #8). A trailing blank line can never be inside a heredoc
    // (the heredoc is already closed by end of file), so this is safe.
    let mut result = lines_out.join("\n");
    let trimmed_len = result.trim_end_matches('\n').len();
    result.truncate(trimmed_len);
    result.push('\n');
    result
}

/// Blank out the comment portions of `line` so heredoc-opener detection never
/// matches a `<<` inside a comment. `/* … */` block comments span lines, so
/// the caller threads `in_block_comment` through consecutive calls; line
/// comments (`#`, `//`) mask to end of line. Quoted strings are honoured (a
/// comment marker inside `"…"` is content, not a comment) — strings cannot
/// span lines outside heredocs, so per-line string state is sound. Characters
/// outside comments are copied verbatim, so indices into the masked line are
/// valid for the non-comment content.
///
/// Without this, a heredoc-looking line inside a block comment desynced the
/// marker scan (wrong `<<-` restoration) and made `post_process` treat the
/// following lines as a heredoc body (trailing whitespace kept, Rule 8
/// violation).
fn mask_comments(line: &str, in_block_comment: &mut bool) -> String {
    let mut out = String::with_capacity(line.len());
    let mut iter = line.chars().peekable();
    let mut in_string = false;

    while let Some(c) = iter.next() {
        if *in_block_comment {
            if c == '*' && iter.peek() == Some(&'/') {
                iter.next();
                *in_block_comment = false;
                out.push_str("  ");
            } else {
                out.push(' ');
            }
            continue;
        }
        if in_string {
            out.push(c);
            match c {
                '\\' => {
                    if let Some(escaped) = iter.next() {
                        out.push(escaped);
                    }
                }
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push(c);
            }
            '#' => {
                out.push(' ');
                for _ in iter.by_ref() {
                    out.push(' ');
                }
            }
            '/' if iter.peek() == Some(&'/') => {
                out.push(' ');
                for _ in iter.by_ref() {
                    out.push(' ');
                }
            }
            '/' if iter.peek() == Some(&'*') => {
                iter.next();
                *in_block_comment = true;
                out.push_str("  ");
            }
            _ => out.push(c),
        }
    }

    out
}

/// If `line` opens a heredoc, return its delimiter (the identifier after
/// `<<` / `<<-`). Returns `None` otherwise.
///
/// A heredoc opener has nothing but whitespace after the delimiter on the
/// opening line, so we require that to avoid false positives like a `<<` that
/// appears inside a string literal. `<<` occurring after a line comment marker
/// is also ignored. Callers pass a line pre-masked by [`mask_comments`] so
/// block-comment content never matches.
fn heredoc_open_delimiter(line: &str) -> Option<String> {
    let idx = line.find("<<")?;

    // Ignore a `<<` that sits inside a line comment.
    let before = &line[..idx];
    if before.contains('#') || before.contains("//") {
        return None;
    }

    let rest = &line[idx + 2..];
    let rest = rest.strip_prefix('-').unwrap_or(rest);
    let ident: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if ident.is_empty() {
        return None;
    }

    // Only whitespace may follow the delimiter on the opening line.
    if rest[ident.len()..].trim().is_empty() {
        Some(ident)
    } else {
        None
    }
}

/// Scan `input` for heredoc openers in document order, returning `true` for
/// each one that used the indented form `<<-` and `false` for plain `<<`.
///
/// Heredoc bodies are treated as opaque (we skip to the closing delimiter), so
/// a `<<` appearing inside a heredoc body is not mistaken for a new opener.
/// The resulting order matches a depth-first walk of the parsed AST, which is
/// how [`formatter::restore_heredoc_indent_markers`] pairs the two.
fn scan_heredoc_markers(input: &str) -> Vec<bool> {
    let mut markers = Vec::new();
    let mut heredoc_delim: Option<String> = None;
    let mut in_block_comment = false;

    for line in input.lines() {
        match &heredoc_delim {
            Some(delim) => {
                if line.trim() == delim.as_str() {
                    heredoc_delim = None;
                }
            }
            None => {
                let masked = mask_comments(line, &mut in_block_comment);
                if let Some(delim) = heredoc_open_delimiter(&masked) {
                    if let Some(idx) = masked.find("<<") {
                        markers.push(masked[idx + 2..].starts_with('-'));
                    }
                    heredoc_delim = Some(delim);
                }
            }
        }
    }

    markers
}
