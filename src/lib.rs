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

    // hcl-edit round-trips losslessly (parse → to_string is byte-identical)
    // except where its data model cannot represent the input: an object with
    // duplicate keys is silently collapsed at parse time (`Object` is a map),
    // which would make formatting delete one of the user's entries. The
    // heredoc `<<-` marker drop is the one other known structural-lossy case,
    // and it was just restored above — so any remaining mismatch means
    // formatting would corrupt data. Refuse instead. (Duplicate object keys
    // are invalid Terraform anyway: `terraform validate` rejects them.)
    //
    // The comparison ignores `\r` because hcl-edit does not round-trip CRLF
    // byte-identically; tf-format normalizes line endings to LF regardless,
    // so a `\r`-only difference is never data loss.
    if body.to_string().replace('\r', "") != input.replace('\r', "") {
        return Err(FormatError::LossyParse);
    }

    // sort_top_level handles both block ordering and top-level attribute
    // formatting (as in `.tfvars` files), recursing into nested bodies.
    formatter::sort_top_level(&mut body, opts.style);

    Ok(post_process(&body.to_string(), opts.style))
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
            // this line opens a heredoc.
            None => {
                let trimmed = line.trim_end();
                if trimmed.is_empty() {
                    pending_blanks.push(String::new());
                } else {
                    let starts_close = {
                        let t = trimmed.trim_start();
                        t.starts_with('}') || t.starts_with(']')
                    };
                    if strip_before_close && starts_close {
                        pending_blanks.clear();
                    } else {
                        lines_out.append(&mut pending_blanks);
                    }
                    lines_out.push(trimmed.to_string());
                    heredoc_delim = heredoc_open_delimiter(line);
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

/// If `line` opens a heredoc, return its delimiter (the identifier after
/// `<<` / `<<-`). Returns `None` otherwise.
///
/// A heredoc opener has nothing but whitespace after the delimiter on the
/// opening line, so we require that to avoid false positives like a `<<` that
/// appears inside a string literal. `<<` occurring after a line comment marker
/// is also ignored.
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

    for line in input.lines() {
        match &heredoc_delim {
            Some(delim) => {
                if line.trim() == delim.as_str() {
                    heredoc_delim = None;
                }
            }
            None => {
                if let Some(delim) = heredoc_open_delimiter(line) {
                    if let Some(idx) = line.find("<<") {
                        markers.push(line[idx + 2..].starts_with('-'));
                    }
                    heredoc_delim = Some(delim);
                }
            }
        }
    }

    markers
}
