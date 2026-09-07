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

    // Each output line paired with whether it is heredoc-body content (or the
    // terminator line) — such lines are raw string data and must never be
    // rewritten by the comment-alignment pass below.
    let mut lines_out: Vec<(String, bool)> = Vec::new();
    let mut heredoc_delim: Option<String> = None;
    let mut in_block_comment = false;
    // Blank lines seen outside a heredoc, held back until we know whether the
    // next non-blank line is a closing bracket (in which case they're dropped).
    let mut pending_blanks: Vec<(String, bool)> = Vec::new();

    for line in output.lines() {
        match &heredoc_delim {
            // Inside a heredoc body: emit lines verbatim. The terminator line
            // (whose trimmed content equals the delimiter) closes the heredoc;
            // it is emitted verbatim too, since its indentation is significant
            // for the `<<-` form.
            Some(delim) => {
                lines_out.append(&mut pending_blanks);
                lines_out.push((line.to_string(), true));
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
                    pending_blanks.push((String::new(), false));
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
                    lines_out.push((trimmed.to_string(), false));
                    heredoc_delim = heredoc_open_delimiter(&masked);
                }
            }
        }
    }
    lines_out.append(&mut pending_blanks);

    let lines_out = apply_bracket_stack_indent(lines_out);
    let lines_out = align_trailing_comments(lines_out);

    // Collapse any trailing blank lines so the file ends with exactly one
    // newline (Rule #8). A trailing blank line can never be inside a heredoc
    // (the heredoc is already closed by end of file), so this is safe.
    let mut result = lines_out.join("\n");
    let trimmed_len = result.trim_end_matches('\n').len();
    result.truncate(trimmed_len);
    result.push('\n');
    result
}

/// Lexer mode for [`bracket_net`]'s scan of a quoted string's contents:
/// either plain string literal text, or inside a `${ … }` / `%{ … }`
/// template interpolation (whose own bracket-nesting `depth` tracks when the
/// interpolation's closing `}` — as opposed to some nested bracket's — is
/// reached).
#[derive(Clone, Copy)]
enum StrMode {
    Str,
    Interp(u32),
}

/// Net count of open (`{`/`[`/`(`) minus close (`}`/`]`/`)`) bracket tokens on
/// `line`, for the bracket-stack indent pass in [`apply_bracket_stack_indent`]
/// — mirrors the token stream `hclwrite`'s `formatIndent` walks. Brackets
/// inside a `#`/`//`/`/* … */` comment, or inside a quoted string's literal
/// text, don't count. A `${ … }` interpolation's or `%{ … }` directive's own
/// delimiters — and any brackets inside them — are real tokens and do count;
/// a `"…"` string nested inside such an interpolation is literal text again,
/// recursively. `mode_stack` and `in_block_comment` carry lexer state across
/// lines, since both a string (spanning an interpolation whose own content
/// spans lines) and a block comment can cross physical lines.
fn bracket_net(line: &str, mode_stack: &mut Vec<StrMode>, in_block_comment: &mut bool) -> i32 {
    let chars: Vec<char> = line.chars().collect();
    let mut net: i32 = 0;
    let mut i = 0;
    while i < chars.len() {
        if *in_block_comment {
            if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                *in_block_comment = false;
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        match mode_stack.last().copied() {
            None => match chars[i] {
                '#' => break,
                '/' if chars.get(i + 1) == Some(&'/') => break,
                '/' if chars.get(i + 1) == Some(&'*') => {
                    *in_block_comment = true;
                    i += 2;
                }
                '"' => {
                    mode_stack.push(StrMode::Str);
                    i += 1;
                }
                '{' | '(' | '[' => {
                    net += 1;
                    i += 1;
                }
                '}' | ')' | ']' => {
                    net -= 1;
                    i += 1;
                }
                _ => i += 1,
            },
            Some(StrMode::Str) => match chars[i] {
                '\\' => i += 2,
                '"' => {
                    mode_stack.pop();
                    i += 1;
                }
                // `$${` / `%%{` is HCL's escape for a literal `${` / `%{`:
                // plain text, not an interpolation opener.
                c @ ('$' | '%')
                    if chars.get(i + 1) == Some(&c) && chars.get(i + 2) == Some(&'{') =>
                {
                    i += 3;
                }
                '$' | '%' if chars.get(i + 1) == Some(&'{') => {
                    mode_stack.push(StrMode::Interp(0));
                    net += 1;
                    i += 2;
                }
                _ => i += 1,
            },
            Some(StrMode::Interp(depth)) => match chars[i] {
                '#' => break,
                '/' if chars.get(i + 1) == Some(&'/') => break,
                '/' if chars.get(i + 1) == Some(&'*') => {
                    *in_block_comment = true;
                    i += 2;
                }
                '"' => {
                    mode_stack.push(StrMode::Str);
                    i += 1;
                }
                '{' | '(' | '[' => {
                    if let Some(StrMode::Interp(d)) = mode_stack.last_mut() {
                        *d += 1;
                    }
                    net += 1;
                    i += 1;
                }
                '}' if depth == 0 => {
                    mode_stack.pop();
                    net -= 1;
                    i += 1;
                }
                '}' | ')' | ']' => {
                    if let Some(StrMode::Interp(d)) = mode_stack.last_mut() {
                        *d = d.saturating_sub(1);
                    }
                    net -= 1;
                    i += 1;
                }
                _ => i += 1,
            },
        }
    }
    net
}

/// Re-indent every non-blank, non-heredoc-body line by a per-line bracket
/// stack, matching `terraform fmt` / `tofu fmt` (`hclwrite`'s `formatIndent`):
/// a line's own indent is `2 * stack.len()` *before* any push, a line whose
/// net bracket delta ([`bracket_net`]) is positive then pushes that delta, and
/// a line whose delta is negative consumes entries from the top — an entry
/// larger than what is left to close is only *reduced*, not popped — before
/// computing that line's indent from the resulting depth.
/// This one mechanism replaces the AST-level per-node re-indentation
/// (`reindent_bracketed`, `reindent_parenthesis`, etc.): those approximated
/// `hclwrite`'s bracket stack node by node and got every net-zero line
/// (`}, {`, `[for x in l : {`) wrong (PAR-8).
///
/// Blank lines and heredoc-body/terminator lines (flagged by the caller) are
/// left untouched — a heredoc's opener line is re-indented normally, but its
/// body is literal string data with significant leading whitespace.
fn apply_bracket_stack_indent(lines: Vec<(String, bool)>) -> Vec<(String, bool)> {
    let mut stack: Vec<i32> = Vec::new();
    let mut mode_stack: Vec<StrMode> = Vec::new();
    let mut in_block_comment = false;

    lines
        .into_iter()
        .map(|(line, is_heredoc)| {
            if is_heredoc || line.trim().is_empty() {
                return (line, is_heredoc);
            }
            // A line that starts already inside a `/* … */` block comment is
            // continuation content, not a formattable line — like a heredoc
            // body it keeps its original indentation verbatim. Its bracket
            // net (should the comment end mid-line) still updates the stack
            // so lines *after* it are indented correctly.
            let continues_block_comment = in_block_comment;
            let net = bracket_net(&line, &mut mode_stack, &mut in_block_comment);
            let depth = update_stack(&mut stack, net);
            if continues_block_comment {
                return (line, false);
            }
            (
                format!("{}{}", "  ".repeat(depth), line.trim_start()),
                false,
            )
        })
        .collect()
}

/// Apply one line's bracket net delta to the indent-depth stack and return
/// the depth to indent *that* line at (measured before any push, per
/// `hclwrite`'s `formatIndent`): a positive net pushes itself after; a
/// negative net first consumes the stack from the top — an entry smaller
/// than or equal to what is left to close is popped, a larger one is only
/// reduced by that amount and stays (so `}, { … })` closing two of a
/// three-opener line keeps the interior depth); a zero net leaves the stack
/// alone.
fn update_stack(stack: &mut Vec<i32>, net: i32) -> usize {
    match net.cmp(&0) {
        std::cmp::Ordering::Greater => {
            let depth = stack.len();
            stack.push(net);
            depth
        }
        std::cmp::Ordering::Less => {
            let mut remaining = -net;
            while remaining > 0 {
                let Some(top) = stack.last_mut() else {
                    break;
                };
                if *top > remaining {
                    *top -= remaining;
                    remaining = 0;
                } else {
                    remaining -= *top;
                    stack.pop();
                }
            }
            stack.len()
        }
        std::cmp::Ordering::Equal => stack.len(),
    }
}

/// Vertically align trailing `#` / `//` comments across every maximal run of
/// consecutive lines that each carry one, matching `terraform fmt` /
/// `tofu fmt`'s `formatCells` pass. Unlike the AST-level aligners this is a
/// pure text pass over the fully rendered, fully indented output, so it joins
/// a run across structural boundaries an AST walk would treat as separate
/// scopes — an attribute line, a `b = {` opener, a nested block header, a `}`
/// closer, an array element all belong to the same run as long as they are
/// physically consecutive and each has a trailing comment. A line with no
/// trailing comment, a comment-only line, a blank line, or heredoc-body
/// content ends the run and is left untouched. Widths are measured in chars
/// (rune count), matching `tofu fmt`'s multibyte-safe alignment.
fn align_trailing_comments(lines: Vec<(String, bool)>) -> Vec<String> {
    let mut in_block_comment = false;
    let comment_at: Vec<Option<usize>> = lines
        .iter()
        .map(|(line, is_heredoc)| {
            if *is_heredoc {
                None
            } else {
                find_trailing_comment(line, &mut in_block_comment)
            }
        })
        .collect();

    let mut result: Vec<String> = lines.into_iter().map(|(line, _)| line).collect();

    let content_width = |line: &str, idx: usize| -> usize {
        line.chars()
            .take(idx)
            .collect::<String>()
            .trim_end()
            .chars()
            .count()
    };

    let mut start = 0;
    while start < comment_at.len() {
        let Some(_) = comment_at[start] else {
            start += 1;
            continue;
        };
        let mut end = start + 1;
        while end < comment_at.len() && comment_at[end].is_some() {
            end += 1;
        }
        let max_width = (start..end)
            .map(|i| content_width(&result[i], comment_at[i].unwrap_or_default()))
            .max()
            .unwrap_or(0);
        for i in start..end {
            let idx = match comment_at[i] {
                Some(idx) => idx,
                None => continue,
            };
            let chars: Vec<char> = result[i].chars().collect();
            let content: String = chars[..idx].iter().collect();
            let content = content.trim_end();
            let comment: String = chars[idx..].iter().collect();
            let padding = max_width - content_width(&result[i], idx) + 1;
            result[i] = format!("{content}{}{comment}", " ".repeat(padding));
        }
        start = end;
    }

    result
}

/// Locate the start (as a char index) of a genuine trailing `#`/`//` comment
/// on `line`, if any. Returns `None` when the line has no such comment, when
/// the line is comment-only (no non-whitespace content before the marker —
/// an own-line comment breaks an alignment run rather than joining it), or
/// when the only comment marker found is a `/* … */` block comment — `tofu
/// fmt` does not treat a block comment as an alignment cell (verified: `a = 1
/// /* c */` next to a `#`-commented line does not join its column). Threads
/// `in_block_comment` across calls the same way [`mask_comments`] does, so a
/// block comment spanning multiple lines is tracked correctly.
fn find_trailing_comment(line: &str, in_block_comment: &mut bool) -> Option<usize> {
    let chars: Vec<char> = line.chars().collect();
    let mut in_string = false;
    let mut saw_block_comment = false;
    let mut i = 0;
    while i < chars.len() {
        if *in_block_comment {
            if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                *in_block_comment = false;
                saw_block_comment = true;
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        if in_string {
            match chars[i] {
                '\\' => i += 2,
                '"' => {
                    in_string = false;
                    i += 1;
                }
                _ => i += 1,
            }
            continue;
        }
        match chars[i] {
            '"' => {
                in_string = true;
                i += 1;
            }
            '#' => return finish_trailing_comment(&chars, i, saw_block_comment),
            '/' if chars.get(i + 1) == Some(&'/') => {
                return finish_trailing_comment(&chars, i, saw_block_comment);
            }
            '/' if chars.get(i + 1) == Some(&'*') => {
                saw_block_comment = true;
                *in_block_comment = true;
                i += 2;
            }
            _ => i += 1,
        }
    }
    None
}

/// Shared tail of [`find_trailing_comment`]: given the char index of a `#`/`//`
/// marker, decide whether it is a genuine trailing comment.
fn finish_trailing_comment(chars: &[char], idx: usize, saw_block_comment: bool) -> Option<usize> {
    if saw_block_comment {
        // A block comment already appeared earlier on this line; treating a
        // second, `#`/`//` comment after it as the alignment cell is an
        // unverified edge case, so conservatively don't align it.
        return None;
    }
    let prefix: String = chars[..idx].iter().collect();
    if prefix.trim().is_empty() {
        None
    } else {
        Some(idx)
    }
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
