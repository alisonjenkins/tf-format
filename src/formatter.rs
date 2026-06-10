use hcl_edit::Decorate;
use hcl_edit::expr::{
    Array, Expression, Object, ObjectKey, ObjectValueAssignment, ObjectValueTerminator,
};
use hcl_edit::structure::{Body, Structure};

use crate::classify::is_multiline;

/// Selects between tf-format's full opinionated style and a
/// minimal `terraform fmt` / `tofu fmt`-parity mode.
///
/// The opinionated style sorts top-level blocks alphabetically,
/// hoists meta-arguments to the top of every block, alphabetises
/// attributes and object keys, expands wide single-line objects,
/// and adds trailing commas to multi-line arrays. The minimal
/// style turns all of those off and applies only spacing /
/// alignment changes — `=` alignment, 2-space indent, single
/// trailing newline, and whitespace cleanup.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FormatStyle {
    /// Apply every transform tf-format knows about. Default.
    #[default]
    Opinionated,
    /// Apply only spacing + alignment transforms; preserve source
    /// order. Mirrors `terraform fmt` / `tofu fmt`.
    Minimal,
}

impl FormatStyle {
    /// True when the style permits structural rewrites like
    /// reordering, alphabetisation, hoisting, single-line→multi
    /// expansion, and trailing-comma insertion.
    fn is_opinionated(self) -> bool {
        matches!(self, FormatStyle::Opinionated)
    }
}

/// Per-block-type priority lists. Hoisting is block-type-aware: an attribute
/// is only treated as a priority (meta-argument) inside the block types where
/// it is actually a meta-argument. Nested blocks — anything below a top-level
/// block — get no hoisting at all (`None`).
///
/// See <https://github.com/alisonjenkins/tf-format/issues/30>.
fn priorities_for_block(ident: &str) -> (&'static [&'static str], &'static [&'static str]) {
    match ident {
        // resource / data / ephemeral / action share the same meta-args.
        "resource" | "data" | "ephemeral" | "action" => (
            &["count", "for_each", "provider", "depends_on"],
            &["lifecycle"],
        ),
        "module" => (
            &[
                "source",
                "version",
                "providers",
                "count",
                "for_each",
                "depends_on",
            ],
            &["lifecycle"],
        ),
        "import" => (&["for_each", "provider"], &[]),
        "output" => (&["depends_on"], &[]),
        "removed" => (&[], &["lifecycle"]),
        // OpenTofu allows `for_each` on `provider`; Terraform does not. Hoisting
        // it under both is harmless — `for_each` won't appear in a TF provider
        // block in valid config.
        "provider" => (&["for_each"], &[]),
        _ => (&[], &[]),
    }
}

/// Returns the priority index for a structure if it's a priority item, or None.
///
/// `parent_ident` is the identifier of the block whose body this structure
/// lives directly inside (`Some("resource")`, `Some("module")`, ...). `None`
/// means "no hoisting applies" — used for nested blocks at depth ≥ 1 and for
/// top-level attribute runs (tfvars-style files).
fn priority_index(structure: &Structure, parent_ident: Option<&str>) -> Option<usize> {
    let ident = parent_ident?;
    let (attrs, blocks) = priorities_for_block(ident);
    match structure {
        Structure::Attribute(attr) => {
            let key = attr.key.as_str();
            attrs.iter().position(|&k| k == key)
        }
        Structure::Block(block) => {
            let bident = block.ident.as_str();
            blocks
                .iter()
                .position(|&k| k == bident)
                .map(|i| attrs.len() + i)
        }
    }
}

/// Extract a sort key from a structure. For attributes this is the key name,
/// for blocks it is the ident followed by labels separated by null bytes.
fn sort_key(structure: &Structure) -> String {
    match structure {
        Structure::Attribute(attr) => attr.key.as_str().to_string(),
        Structure::Block(block) => {
            let mut key = block.ident.as_str().to_string();
            for label in &block.labels {
                key.push('\0');
                key.push_str(label.as_str());
            }
            key
        }
    }
}

/// Extract a sort key string from an ObjectKey.
///
/// For `ObjectKey::Expression`, `to_string()` would include the expression's
/// decor (surrounding whitespace from the source), so we clone and clear the
/// decor first to get just the bare expression. This is critical for `=`
/// alignment, which uses the returned string's length to compute padding.
fn object_key_str(key: &ObjectKey) -> String {
    match key {
        ObjectKey::Ident(ident) => ident.as_str().to_string(),
        ObjectKey::Expression(expr) => {
            let mut bare = expr.clone();
            bare.decor_mut().set_prefix("");
            bare.decor_mut().set_suffix("");
            bare.to_string()
        }
    }
}

/// Check whether a decor prefix string contains a blank line, which acts as
/// an alignment group separator in `terraform fmt` / `tofu fmt`.
///
/// When the previous entry's terminator is `Newline`, the `\n` ending the
/// previous line comes from the terminator — so a single leading `\n` in the
/// prefix represents a blank line. When the terminator is `Comma` (or None),
/// there is no automatic newline, so a single leading `\n` is just the
/// line-break and a blank line requires `\n\n`.
fn has_blank_line_after_newline_terminator(prefix: &str) -> bool {
    // CRLF input stores `\r\n` in decor; strip the `\r` so a blank line is
    // recognised either way (output is LF-normalized downstream regardless).
    let prefix = prefix.replace('\r', "");
    prefix.starts_with('\n') || prefix.contains("\n\n")
}

/// Detect a blank line in a prefix when the previous entry may use a
/// non-Newline terminator (e.g. Comma). Only `\n\n` counts.
fn has_blank_line_after_other_terminator(prefix: &str) -> bool {
    prefix.replace('\r', "").contains("\n\n")
}

/// Count the leading newlines a decor prefix encodes, treating whitespace-only
/// lines as blank. Stops at the first non-whitespace character (e.g. a comment
/// `#` or the structure's own indent has no following content in a prefix).
///
/// Used by the minimal (`tofu fmt` parity) paths to reproduce the *exact*
/// number of blank lines a user wrote, rather than collapsing runs to one.
fn count_leading_newlines(prefix: &str) -> usize {
    let mut count = 0;
    for ch in prefix.chars() {
        match ch {
            '\n' => count += 1,
            ' ' | '\t' | '\r' => continue,
            _ => break,
        }
    }
    count
}

/// Extract comments from a decor prefix string.
///
/// Each returned entry is one logical comment and may span multiple lines (a
/// `/* … */` block comment). The first line of every comment is left-trimmed;
/// continuation lines of a block comment keep their indentation *relative to*
/// the opening `/*`, so the conventional ` * ` star alignment survives when the
/// block is re-indented by [`push_comment`].
///
/// A line-by-line filter (the previous implementation) dropped interior and
/// closing lines of a multi-line block comment — e.g. `line */` — leaving an
/// unterminated `/*` and producing unparseable output. This span-based walk
/// captures the whole block verbatim instead.
fn extract_comments(prefix: &str) -> Vec<String> {
    let mut comments: Vec<String> = Vec::new();
    let mut lines = prefix.lines();

    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();

        if trimmed.starts_with('#') || trimmed.starts_with("//") {
            // Single-line comment.
            comments.push(line.trim().to_string());
        } else if trimmed.starts_with("/*") {
            // Block comment: capture until the line that closes it with `*/`.
            // Indentation of continuation lines is preserved relative to the
            // opening `/*` so star alignment is kept on re-emit.
            let base_indent = leading_ws_len(line);
            let mut block = trimmed.trim_end().to_string();
            if !trimmed.contains("*/") {
                for cont in lines.by_ref() {
                    block.push('\n');
                    block.push_str(dedent(cont, base_indent).trim_end());
                    if cont.contains("*/") {
                        break;
                    }
                }
            }
            comments.push(block);
        }
        // Anything else (blank lines, the trailing indent-only line) is skipped.
    }

    comments
}

/// Number of leading ASCII-whitespace bytes in `line`.
fn leading_ws_len(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Strip up to `n` leading whitespace characters from `line`, preserving any
/// indentation beyond that (so a block comment's relative star alignment is
/// retained).
fn dedent(line: &str, n: usize) -> &str {
    let prefix_len = line
        .char_indices()
        .take(n)
        .take_while(|&(_, c)| c.is_whitespace())
        .map(|(i, c)| i + c.len_utf8())
        .last()
        .unwrap_or(0);
    &line[prefix_len..]
}

/// Append a (possibly multi-line) comment to `prefix`, prepending `indent` to
/// every line so the whole block sits at the target indentation while keeping
/// its internal relative alignment. Each comment is followed by a newline.
fn push_comment(prefix: &mut String, comment: &str, indent: &str) {
    for line in comment.lines() {
        if !line.is_empty() {
            prefix.push_str(indent);
            prefix.push_str(line);
        }
        prefix.push('\n');
    }
}

/// Whether `prefix` holds at least one blank line *after* its comments (i.e. a
/// separating blank between header comments and the following block). Compares
/// the newline count of the raw prefix against what re-emitting just the
/// comments would produce — any surplus is a blank line. `tofu fmt` keeps one
/// such blank, so the caller re-emits a single `\n` when this is true.
fn blank_after_comments(prefix: &str) -> bool {
    let comments = extract_comments(prefix);
    let mut rendered = String::new();
    for comment in &comments {
        push_comment(&mut rendered, comment, "");
    }
    let raw_newlines = prefix.matches('\n').count();
    let comment_newlines = rendered.matches('\n').count();
    raw_newlines > comment_newlines
}

/// Build a prefix for a body structure. The Body/Block encoding adds `\n`
/// between structures, so the prefix only needs indent (and optionally an
/// extra `\n` for a blank line separator).
fn build_body_prefix(blank_lines: usize, comments: &[String], indent: &str) -> String {
    let mut prefix = String::new();
    for _ in 0..blank_lines {
        prefix.push('\n');
    }
    for comment in comments {
        push_comment(&mut prefix, comment, indent);
    }
    prefix.push_str(indent);
    prefix
}

/// Build a prefix for an object key. In objects, `\n` comes from the previous
/// entry's ObjectValueTerminator::Newline, except for the first entry where
/// we need a `\n` after the opening `{`.
fn build_object_key_prefix(
    add_structural_newline: bool,
    blank_lines: usize,
    inline_comment: Option<&str>,
    comments: &[String],
    indent: &str,
) -> String {
    let mut prefix = String::new();
    if let Some(comment) = inline_comment {
        // A comment that hugged the opening `{` on the same line stays inline
        // (`{ # comment`), matching `tofu fmt`. The newline it owns replaces the
        // structural newline that would otherwise precede the first entry.
        prefix.push(' ');
        prefix.push_str(comment);
        prefix.push('\n');
    } else if add_structural_newline {
        prefix.push('\n');
    }
    for _ in 0..blank_lines {
        prefix.push('\n');
    }
    for comment in comments {
        push_comment(&mut prefix, comment, indent);
    }
    prefix.push_str(indent);
    prefix
}

/// Split a first-entry prefix into (inline comment hugging `{`, blank lines
/// following that comment, remaining comments). `tofu fmt` keeps a comment
/// written on the same line as the object opening brace inline; any comments
/// on their own lines below it move to their own lines as usual. Only the
/// leading single-line comment qualifies as inline.
///
/// The blank-line count is recovered from the text after the comment's own
/// line break: the raw prefix starts with the comment text, so the caller's
/// leading-newline count sees zero and would otherwise drop an author blank
/// between `{ # note` and the first entry.
fn split_leading_inline_comment(prefix: &str) -> (Option<String>, usize, Vec<String>) {
    let first_line = prefix.lines().next().unwrap_or("").trim();
    let is_inline = first_line.starts_with('#')
        || first_line.starts_with("//")
        || (first_line.starts_with("/*") && first_line.ends_with("*/"));
    if !is_inline {
        return (None, 0, extract_comments(prefix));
    }
    let rest = prefix.split_once('\n').map(|(_, r)| r).unwrap_or("");
    (
        Some(first_line.to_string()),
        count_leading_newlines(rest),
        extract_comments(rest),
    )
}

/// Number of blank lines to emit before an object entry.
///
/// Opinionated mode follows the group-driven `want_blank` decision (0 or 1).
/// Minimal mode mirrors `tofu fmt`: reproduce the *exact* number of blank lines
/// the user wrote, recovered from the entry's original prefix. `add_structural`
/// indicates whether the prefix builder will emit the structural newline (the
/// `\n` after `{`, or the line-break following a comma-terminated entry) — that
/// newline is not itself a blank line, so it is excluded from the count.
fn object_entry_blank_lines(
    style: FormatStyle,
    key: &ObjectKey,
    add_structural: bool,
    want_blank: bool,
) -> usize {
    if style.is_opinionated() {
        return want_blank as usize;
    }
    let prefix = key
        .decor()
        .prefix()
        .map(|p| p.to_string())
        .unwrap_or_default();
    let leading = count_leading_newlines(&prefix);
    let blanks = if add_structural {
        leading.saturating_sub(1)
    } else {
        leading
    };
    if blanks == 0 && want_blank { 1 } else { blanks }
}

/// Adjust the prefix decoration on a body structure, emitting `blank_lines`
/// blank lines before it. When `oneline` is set the body stays on a single line
/// (hcl-edit emits no inter-structure `\n`), so the prefix is just the single
/// space that separates `{` from the first structure — no newline, indent, or
/// comment handling (one-line bodies carry none).
fn adjust_structure_prefix(
    structure: &mut Structure,
    blank_lines: usize,
    indent: &str,
    oneline: bool,
) {
    let decor = structure.decor_mut();
    if oneline {
        decor.set_prefix(" ");
        return;
    }
    let existing_prefix = decor.prefix().map(|p| p.to_string()).unwrap_or_default();

    let comments = extract_comments(&existing_prefix);
    let new_prefix = build_body_prefix(blank_lines, &comments, indent);
    decor.set_prefix(new_prefix);
}

/// Sort and format the contents of a Body in-place. Applies the rules:
/// 1. Single-line attributes first (sorted alphabetically)
/// 2. Multi-line attributes and blocks mixed together (sorted alphabetically,
///    blank line between each)
/// 3. Recurse into nested blocks and object expressions.
/// 4. Blank-line-separated groups in the original source are preserved and
///    each group is sorted/aligned independently.
///
/// Under [`FormatStyle::Minimal`] the partitioning + sorting is
/// suppressed; only `=` alignment within blank-line groups runs.
pub fn format_body(body: &mut Body, depth: usize, parent_ident: Option<&str>, style: FormatStyle) {
    let indent = "  ".repeat(depth + 1);

    // Preserve body-level metadata
    let body_decor = body.decor().clone();
    let prefer_oneline = body.prefer_oneline();
    let prefer_omit_trailing_newline = body.prefer_omit_trailing_newline();

    // Drain all structures from the body
    let old_body = std::mem::take(body);
    let mut structures: Vec<Structure> = old_body.into_iter().collect();

    // Recurse into nested blocks and expressions. Nested blocks always pass
    // `None` for parent_ident — hoisting is suppressed below the top level.
    for structure in &mut structures {
        match structure {
            Structure::Block(block) => {
                format_body(&mut block.body, depth + 1, None, style);
            }
            Structure::Attribute(attr) => {
                let prefix_width = attr.key.as_str().len() + 3; // `key = `
                format_expression(&mut attr.value, depth + 1, style, prefix_width);
            }
        }
    }

    // Split into blank-line-separated groups under minimal style.
    // Opinionated style ignores author blank lines: the whole body
    // is one logical group so all single-line attrs collapse into
    // the priority/normal-single tier (sorted alphabetically) and
    // multi-line attrs/blocks fall into the multi tiers (also
    // sorted) — no blank-line-driven sub-grouping.
    let groups = if style.is_opinionated() {
        vec![structures]
    } else {
        split_body_groups(structures)
    };
    let mut any_emitted = false;

    for (group_idx, group_structures) in groups.into_iter().enumerate() {
        let want_group_blank = any_emitted && group_idx > 0;
        any_emitted = format_structure_group(
            body,
            group_structures,
            &indent,
            want_group_blank,
            any_emitted,
            parent_ident,
            style,
            prefer_oneline,
        );
    }

    // Restore body-level metadata
    *body.decor_mut() = body_decor;
    body.set_prefer_oneline(prefer_oneline);
    body.set_prefer_omit_trailing_newline(prefer_omit_trailing_newline);
}

/// Apply the 4-tier partition (priority single/multi, normal single/multi) to a
/// group of structures, sort each tier, align `=` signs on the single-line
/// tiers, and push the result onto `body`. `indent` is the indentation string
/// for each structure; `want_group_blank` asks for a blank line before the
/// first emitted structure; `any_emitted_before` is the running "anything
/// already pushed?" flag. `oneline` flags a `prefer_oneline` body so emitted
/// structures get a single-space prefix instead of newline+indent. Returns the
/// updated flag.
#[allow(clippy::too_many_arguments)]
fn format_structure_group(
    body: &mut Body,
    group: Vec<Structure>,
    indent: &str,
    want_group_blank: bool,
    any_emitted_before: bool,
    parent_ident: Option<&str>,
    style: FormatStyle,
    oneline: bool,
) -> bool {
    if !style.is_opinionated() {
        return format_structure_group_minimal(
            body,
            group,
            indent,
            want_group_blank,
            any_emitted_before,
            oneline,
        );
    }

    let mut priority_single: Vec<Structure> = Vec::new();
    let mut priority_multi: Vec<Structure> = Vec::new();
    let mut normal_single: Vec<Structure> = Vec::new();
    let mut normal_multi: Vec<Structure> = Vec::new();

    for s in group {
        if priority_index(&s, parent_ident).is_some() {
            if is_multiline(&s) {
                priority_multi.push(s);
            } else {
                priority_single.push(s);
            }
        } else if is_multiline(&s) {
            normal_multi.push(s);
        } else {
            normal_single.push(s);
        }
    }

    priority_single.sort_by_key(|s| priority_index(s, parent_ident).unwrap_or(usize::MAX));
    priority_multi.sort_by_key(|s| priority_index(s, parent_ident).unwrap_or(usize::MAX));
    normal_single.sort_by_key(sort_key);
    normal_multi.sort_by_key(sort_key);

    align_body_attributes(&mut priority_single);
    align_body_attributes(&mut normal_single);

    // Multi-line attributes break the `=` run and are never column-padded;
    // strip any stale alignment so output is idempotent (single→multi-line
    // edits don't leave a once-aligned `=` behind).
    for s in priority_multi.iter_mut().chain(normal_multi.iter_mut()) {
        normalize_unaligned_attribute(s);
    }

    let has_priority = !priority_single.is_empty() || !priority_multi.is_empty();
    let has_priority_single = !priority_single.is_empty();

    let mut any_emitted = any_emitted_before;

    for (i, mut s) in priority_single.into_iter().enumerate() {
        let want_blank = if i == 0 { want_group_blank } else { false };
        adjust_structure_prefix(&mut s, want_blank as usize, indent, oneline);
        body.push(s);
        any_emitted = true;
    }

    for (i, mut s) in priority_multi.into_iter().enumerate() {
        let want_blank = i > 0 || has_priority_single || (i == 0 && want_group_blank);
        adjust_structure_prefix(&mut s, want_blank as usize, indent, oneline);
        body.push(s);
        any_emitted = true;
    }

    let has_normal_single = !normal_single.is_empty();

    for (i, mut s) in normal_single.into_iter().enumerate() {
        let want_blank = i == 0 && (has_priority || want_group_blank);
        adjust_structure_prefix(&mut s, want_blank as usize, indent, oneline);
        body.push(s);
        any_emitted = true;
    }

    for (i, mut s) in normal_multi.into_iter().enumerate() {
        let want_blank = i > 0 || has_normal_single || has_priority || (i == 0 && want_group_blank);
        adjust_structure_prefix(&mut s, want_blank as usize, indent, oneline);
        body.push(s);
        any_emitted = true;
    }

    any_emitted
}

/// `terraform fmt` / `tofu fmt` parity path: keep source order
/// intact, run only `=` alignment over the consecutive runs of
/// single-line attributes, and re-emit. No partitioning, no
/// hoisting, no alphabetisation.
fn format_structure_group_minimal(
    body: &mut Body,
    mut group: Vec<Structure>,
    indent: &str,
    want_group_blank: bool,
    any_emitted_before: bool,
    oneline: bool,
) -> bool {
    align_body_attributes_in_place(&mut group);

    let mut any_emitted = any_emitted_before;
    for (i, mut s) in group.into_iter().enumerate() {
        // Minimal mode mirrors `tofu fmt`: preserve the exact number of blank
        // lines the user wrote. A body structure encodes each blank line as a
        // leading `\n` in its prefix (the Body adds the line-break between
        // structures itself). `want_group_blank` carries the blank that caused
        // this group to split off from the previous one, but only for the very
        // first body group boundary where the count would otherwise be lost.
        let existing_prefix = s
            .decor()
            .prefix()
            .map(|p| p.to_string())
            .unwrap_or_default();
        let mut blank_lines = count_leading_newlines(&existing_prefix);
        if i == 0 && want_group_blank && blank_lines == 0 {
            blank_lines = 1;
        }
        adjust_structure_prefix(&mut s, blank_lines, indent, oneline);
        body.push(s);
        any_emitted = true;
    }
    any_emitted
}

/// Align `=` signs across each contiguous run of single-line
/// attributes inside `structures`, preserving overall order.
/// Multi-line attributes and blocks split a run; comments do too
/// (matching `terraform fmt`).
fn align_body_attributes_in_place(structures: &mut [Structure]) {
    let mut i = 0;
    while i < structures.len() {
        // Advance past anything that isn't a single-line attribute. Multi-line
        // attributes break the alignment run, so they are never column-padded —
        // but any stale padding (e.g. left over from when the value was a
        // single-line expression) must be stripped, or formatting is not
        // idempotent and a once-aligned `=` survives a single→multi-line edit.
        while i < structures.len() && !is_single_line_attribute(&structures[i]) {
            normalize_unaligned_attribute(&mut structures[i]);
            i += 1;
        }
        let run_start = i;
        while i < structures.len()
            && is_single_line_attribute(&structures[i])
            && (i == run_start
                || extract_comments(
                    &structures[i]
                        .decor()
                        .prefix()
                        .map(|p| p.to_string())
                        .unwrap_or_default(),
                )
                .is_empty())
        {
            i += 1;
        }
        if i > run_start {
            align_body_attribute_group(&mut structures[run_start..i]);
        } else {
            // No progress — shouldn't happen because the outer
            // loop advances on non-attributes, but break to be
            // defensive against pathological inputs.
            break;
        }
    }
}

fn is_single_line_attribute(s: &Structure) -> bool {
    matches!(s, Structure::Attribute(attr) if !is_multiline(s) || is_heredoc_expr(&attr.value))
}

/// True if an expression is a heredoc template (`<<EOT` / `<<-EOT`). Although a
/// heredoc spans multiple lines, its `=` sits on the opening line, so
/// `terraform fmt` / `tofu fmt` keep it inside the surrounding `=` alignment
/// group — unlike a multi-line object or array value, which break the group.
fn is_heredoc_expr(expr: &Expression) -> bool {
    matches!(expr, Expression::HeredocTemplate(_))
}

/// Restore the indented-heredoc marker (`<<-`) that hcl-edit drops on parse.
///
/// When a `<<-EOT` body contains a line with no leading whitespace, hcl-edit's
/// `dedent()` computes a strip amount of "nothing" and stores `indent = None`,
/// which re-encodes as a plain `<<EOT`. The rendered value is identical (there
/// was nothing to strip), but `terraform fmt` / `tofu fmt` preserve the literal
/// `<<-` the user wrote, so we restore it for parity (issue #43).
///
/// The original marker can't be recovered from the parsed AST alone (a genuine
/// `<<EOT` is indistinguishable post-parse), so the caller scans the source for
/// each opener's marker in document order. `markers[i]` is `true` if the i-th
/// heredoc in source order used `<<-`. This walk visits heredocs in that same
/// order and sets `indent = Some(0)` on any that lost their `-`.
pub fn restore_heredoc_indent_markers(body: &mut Body, markers: &[bool]) {
    let mut idx = 0;
    restore_heredoc_in_body(body, markers, &mut idx);
}

fn restore_heredoc_in_body(body: &mut Body, markers: &[bool], idx: &mut usize) {
    for mut structure in body.iter_mut() {
        if let Some(mut attr) = structure.as_attribute_mut() {
            restore_heredoc_in_expr(attr.value_mut(), markers, idx);
        } else if let Some(block) = structure.as_block_mut() {
            restore_heredoc_in_body(&mut block.body, markers, idx);
        }
    }
}

fn restore_heredoc_in_expr(expr: &mut Expression, markers: &[bool], idx: &mut usize) {
    match expr {
        Expression::HeredocTemplate(heredoc) => {
            let was_indented = markers.get(*idx).copied().unwrap_or(false);
            *idx += 1;
            if was_indented && heredoc.indent().is_none() {
                heredoc.set_indent(0);
            }
            // Do not descend into the heredoc body: the source scan treats a
            // heredoc body as opaque (skipping to the delimiter), so any nested
            // opener inside an interpolation is invisible to it. Skipping it
            // here too keeps the marker indices aligned.
        }
        Expression::Object(obj) => {
            // Key expressions are not visited: `ObjectKeyMut` exposes no
            // mutable access to the inner expression, and a heredoc in key
            // position requires a parenthesized multi-line key — not valid
            // in practice.
            for (_, value) in obj.iter_mut() {
                restore_heredoc_in_expr(value.expr_mut(), markers, idx);
            }
        }
        Expression::Array(arr) => {
            for i in 0..arr.len() {
                if let Some(elem) = arr.get_mut(i) {
                    restore_heredoc_in_expr(elem, markers, idx);
                }
            }
        }
        Expression::FuncCall(call) => {
            for arg in call.args.iter_mut() {
                restore_heredoc_in_expr(arg, markers, idx);
            }
        }
        Expression::Parenthesis(paren) => restore_heredoc_in_expr(paren.inner_mut(), markers, idx),
        Expression::Conditional(cond) => {
            restore_heredoc_in_expr(&mut cond.cond_expr, markers, idx);
            restore_heredoc_in_expr(&mut cond.true_expr, markers, idx);
            restore_heredoc_in_expr(&mut cond.false_expr, markers, idx);
        }
        Expression::Traversal(trav) => {
            restore_heredoc_in_expr(&mut trav.expr, markers, idx);
            // An index operator (`x[<<EOT ... ]`) can hold a heredoc; skipping
            // it would desync the marker indices for every later heredoc.
            for op in trav.operators.iter_mut() {
                if let hcl_edit::expr::TraversalOperator::Index(index_expr) = op.value_mut() {
                    restore_heredoc_in_expr(index_expr, markers, idx);
                }
            }
        }
        Expression::ForExpr(for_expr) => {
            restore_heredoc_in_expr(&mut for_expr.intro.collection_expr, markers, idx);
            if let Some(key_expr) = &mut for_expr.key_expr {
                restore_heredoc_in_expr(key_expr, markers, idx);
            }
            restore_heredoc_in_expr(&mut for_expr.value_expr, markers, idx);
            // The `if` filter clause is an expression position too; missing it
            // desynced the markers (a heredoc in the cond shifted every later
            // heredoc's marker by one).
            if let Some(cond) = &mut for_expr.cond {
                restore_heredoc_in_expr(&mut cond.expr, markers, idx);
            }
        }
        Expression::UnaryOp(op) => restore_heredoc_in_expr(&mut op.expr, markers, idx),
        Expression::BinaryOp(op) => {
            restore_heredoc_in_expr(&mut op.lhs_expr, markers, idx);
            restore_heredoc_in_expr(&mut op.rhs_expr, markers, idx);
        }
        _ => {}
    }
}

/// True if an object value spans multiple lines in a way that breaks an `=`
/// alignment run. A heredoc value spans lines but keeps its `=` on the opening
/// line, so it does NOT break the run.
fn object_value_breaks_alignment(value: &hcl_edit::expr::ObjectValue) -> bool {
    value.expr().to_string().contains('\n') && !is_heredoc_expr(value.expr())
}

/// Split body structures into groups separated by blank lines. The Body
/// encoding adds `\n` between structures (like a Newline terminator), so a
/// blank line shows up as a leading `\n` in the structure's prefix.
fn split_body_groups(structures: Vec<Structure>) -> Vec<Vec<Structure>> {
    let mut groups: Vec<Vec<Structure>> = Vec::new();
    let mut current: Vec<Structure> = Vec::new();

    for (i, s) in structures.into_iter().enumerate() {
        if i > 0 && !current.is_empty() {
            let prefix = s
                .decor()
                .prefix()
                .map(|p| p.to_string())
                .unwrap_or_default();
            if has_blank_line_after_newline_terminator(&prefix) {
                groups.push(std::mem::take(&mut current));
            }
        }
        current.push(s);
    }
    if !current.is_empty() {
        groups.push(current);
    }
    groups
}

/// Opinionated: remove every blank line from a multi-line array — both between
/// elements and immediately before the closing `]` (issue #35). Each element's
/// prefix is rebuilt as a single newline + indent (comments preserved), and the
/// array's trailing whitespace is collapsed so `]` sits on its own line with no
/// preceding blank line. `terraform fmt` / `tofu fmt` preserve these blanks, so
/// this never runs under [`FormatStyle::Minimal`].
fn normalize_array_blank_lines(arr: &mut Array, depth: usize) {
    let inner_indent = "  ".repeat(depth + 1);
    let closing_indent = "  ".repeat(depth);

    for i in 0..arr.len() {
        if let Some(elem) = arr.get_mut(i) {
            let prefix = elem
                .decor()
                .prefix()
                .map(|p| p.to_string())
                .unwrap_or_default();
            // Only rewrite elements that start their own line; leave the rare
            // inline element untouched.
            if prefix.contains('\n') {
                let comments = extract_comments(&prefix);
                let mut new_prefix = String::from("\n");
                for comment in &comments {
                    push_comment(&mut new_prefix, comment, &inner_indent);
                }
                new_prefix.push_str(&inner_indent);
                elem.decor_mut().set_prefix(new_prefix);
            }
            // A whitespace-only suffix can carry stray blank lines before the
            // next comma; drop it. Suffixes holding comments are preserved.
            let suffix = elem
                .decor()
                .suffix()
                .map(|s| s.to_string())
                .unwrap_or_default();
            if suffix.contains('\n') && extract_comments(&suffix).is_empty() {
                elem.decor_mut().set_suffix("");
            }
        }
    }

    let trailing = arr.trailing().to_string();
    let comments = extract_comments(&trailing);
    let mut new_trailing = String::from("\n");
    for comment in &comments {
        push_comment(&mut new_trailing, comment, &inner_indent);
    }
    new_trailing.push_str(&closing_indent);
    arr.set_trailing(new_trailing);
}

/// Recursively format an expression in-place. Sorts object keys and recurses
/// into nested objects, arrays, function call arguments, and other compound
/// expressions.
///
/// Under [`FormatStyle::Minimal`] the object-key sort and the
/// single-line-object expansion are skipped, and trailing-comma
/// insertion on multi-line arrays is suppressed.
/// Recursively format an expression in-place. `prefix_width` is the width of
/// any text that precedes the expression on its own line (e.g. `key = ` for an
/// attribute or object value), so the single-line-object expansion check can
/// measure the *whole* line rather than just the object literal. It is 0 for
/// sub-expressions that don't start a line (array elements, call arguments,
/// operands).
fn format_expression(expr: &mut Expression, depth: usize, style: FormatStyle, prefix_width: usize) {
    match expr {
        Expression::Object(obj) => {
            // Format multi-line objects, and also expand any single-line object
            // whose rendered width exceeds the line-length budget so we don't
            // emit huge unreadable one-liners. The expansion is opinionated
            // — it changes the source layout — so we skip it under
            // FormatStyle::Minimal.
            let should_expand = style.is_opinionated()
                && should_expand_single_line_object(obj, depth, prefix_width);
            if is_multiline_object(obj) || should_expand {
                format_object(obj, depth, style);
            }
        }
        Expression::Array(arr) => {
            // Trailing-comma insertion is opinionated — `terraform fmt`
            // preserves the user's original layout — so it only fires
            // under FormatStyle::Opinionated.
            if style.is_opinionated() && is_multiline_array(arr) && !arr.is_empty() {
                let last_idx = arr.len() - 1;
                if let Some(last) = arr.get_mut(last_idx) {
                    let suffix = last
                        .decor()
                        .suffix()
                        .map(|s| s.to_string())
                        .unwrap_or_default();
                    if suffix.contains('\n') {
                        last.decor_mut().set_suffix("");
                        arr.set_trailing(suffix);
                    }
                }
                arr.set_trailing_comma(true);
                normalize_array_blank_lines(arr, depth);
            }
            for i in 0..arr.len() {
                if let Some(elem) = arr.get_mut(i) {
                    let elem_inline = elem
                        .decor()
                        .prefix()
                        .is_none_or(|p| !p.to_string().contains('\n'));
                    let elem_depth = if elem_inline { depth } else { depth + 1 };
                    format_expression(elem, elem_depth, style, 0);
                }
            }
        }
        Expression::FuncCall(call) => {
            // Multi-line FuncCalls put each arg on its own line
            // one level deeper than the call itself; single-line
            // calls keep all args on the call's line at the same
            // depth. Detect per-arg via the prefix-newline trick
            // already used by Array elements above so that
            // recursing into a multi-line Object / Array arg
            // hands `format_object` / inner indent calculations
            // the right depth.
            for arg in call.args.iter_mut() {
                let arg_inline = arg
                    .decor()
                    .prefix()
                    .is_none_or(|p| !p.to_string().contains('\n'));
                let arg_depth = if arg_inline { depth } else { depth + 1 };
                format_expression(arg, arg_depth, style, 0);
            }
        }
        Expression::Parenthesis(paren) => {
            format_expression(paren.inner_mut(), depth, style, 0);
        }
        Expression::Conditional(cond) => {
            format_expression(&mut cond.cond_expr, depth, style, 0);
            format_expression(&mut cond.true_expr, depth, style, 0);
            format_expression(&mut cond.false_expr, depth, style, 0);
        }
        Expression::Traversal(trav) => {
            format_expression(&mut trav.expr, depth, style, 0);
        }
        Expression::ForExpr(for_expr) => {
            // The for-expression's `for ... in C : K => V` line lives
            // one level inside the for-expression's outer `[`/`{`.
            // Sub-expressions whose CONTENT spans multiple lines
            // (most commonly `value_expr` when it's an object or
            // array) need their inner depth bumped by one so the
            // keys/elements line up at the right column. Without the
            // bump, an inner `{ name = ..., type = ... }` lands at
            // the same indent as the for-line itself.
            format_expression(&mut for_expr.intro.collection_expr, depth + 1, style, 0);
            if let Some(key_expr) = &mut for_expr.key_expr {
                format_expression(key_expr, depth + 1, style, 0);
            }
            format_expression(&mut for_expr.value_expr, depth + 1, style, 0);
            if let Some(cond) = &mut for_expr.cond {
                format_expression(&mut cond.expr, depth + 1, style, 0);
            }
        }
        Expression::UnaryOp(op) => {
            format_expression(&mut op.expr, depth, style, 0);
        }
        Expression::BinaryOp(op) => {
            format_expression(&mut op.lhs_expr, depth, style, 0);
            format_expression(&mut op.rhs_expr, depth, style, 0);
        }
        // Leaf expressions (Null, Bool, Number, String, Variable, etc.)
        _ => {}
    }
}

/// Vertically align the `=` signs of consecutive single-line attributes in a
/// body by padding the key's decor suffix.
///
/// Matches `terraform fmt` / `tofu fmt` semantics: a comment line attached to
/// an attribute breaks the alignment group. Blank-line groups are handled
/// upstream by `split_body_groups`.
fn align_body_attributes(structures: &mut [Structure]) {
    let mut start = 0;
    while start < structures.len() {
        let mut end = start + 1;
        while end < structures.len() {
            let prefix = structures[end]
                .decor()
                .prefix()
                .map(|p| p.to_string())
                .unwrap_or_default();
            if !extract_comments(&prefix).is_empty() {
                break;
            }
            end += 1;
        }
        align_body_attribute_group(&mut structures[start..end]);
        start = end;
    }
}

/// Normalize the `=` spacing of an attribute that is *not* part of an `=`
/// alignment run — a multi-line value (object/array/func-call spanning lines)
/// breaks the run, so its `=` gets a single space on each side rather than
/// column padding, matching `terraform fmt` / `tofu fmt`. No-op on blocks.
///
/// Heredoc attributes reach this only on the opinionated multi-tier path
/// (minimal mode keeps them inside alignment runs and never routes them
/// here); they get the same single-space normalization so stale padding
/// from the source (`foo       = <<EOT`) doesn't survive.
fn normalize_unaligned_attribute(s: &mut Structure) {
    if let Structure::Attribute(attr) = s {
        attr.key.decor_mut().set_suffix(" ");
        attr.value.decor_mut().set_prefix(" ");
    }
}

/// Align a single contiguous group of attributes (no comments between them).
fn align_body_attribute_group(structures: &mut [Structure]) {
    let max_key_len = structures
        .iter()
        .filter_map(|s| s.as_attribute().map(|a| a.key.as_str().len()))
        .max()
        .unwrap_or(0);

    for s in structures.iter_mut() {
        if let Structure::Attribute(attr) = s {
            let padding = max_key_len - attr.key.as_str().len() + 1;
            attr.key.decor_mut().set_suffix(" ".repeat(padding));
            // Normalize whitespace after `=` to a single space, matching
            // `terraform fmt` / `tofu fmt`. The value's prefix decor holds
            // the whitespace between `=` and the value.
            attr.value.decor_mut().set_prefix(" ");
        }
    }
}

/// Vertically align the `=` signs of object key entries by padding the key's
/// decor suffix. A comment attached to an entry breaks the alignment group,
/// matching `terraform fmt` / `tofu fmt`. Blank-line groups are already
/// handled by `split_object_groups`.
fn align_object_keys(entries: &mut [(ObjectKey, hcl_edit::expr::ObjectValue)]) {
    let mut start = 0;
    while start < entries.len() {
        let mut end = start + 1;
        while end < entries.len() {
            if !extract_key_comments(&entries[end].0).is_empty() {
                break;
            }
            end += 1;
        }
        align_object_key_group(&mut entries[start..end]);
        start = end;
    }
}

fn align_object_key_group(entries: &mut [(ObjectKey, hcl_edit::expr::ObjectValue)]) {
    // tofu fmt aligns `=` runs but never `:` runs, and treats a
    // `:` entry as a hard break for `=` alignment. Two `=` runs
    // separated by a `:` align INDEPENDENTLY of each other.
    //
    // Walk the slice in consecutive-same-assignment runs. Equals
    // runs get column-aligned (longest key sets the column for
    // its own run). Colon runs get single-space padding —
    // matching `tofu fmt`'s render of the JSON-like object form.
    let mut start = 0;
    while start < entries.len() {
        let kind = entries[start].1.assignment();
        let mut end = start + 1;
        while end < entries.len() && entries[end].1.assignment() == kind {
            end += 1;
        }
        match kind {
            ObjectValueAssignment::Equals => {
                align_equals_run(&mut entries[start..end]);
            }
            ObjectValueAssignment::Colon => {
                for (key, value) in entries[start..end].iter_mut() {
                    key.decor_mut().set_suffix(" ");
                    value.expr_mut().decor_mut().set_prefix(" ");
                }
            }
        }
        start = end;
    }
}

/// Column-align a contiguous run of single-line `=` assignments
/// — pad each key's suffix to bring every `=` to the same
/// column, normalise the value's prefix to a single space.
/// Matches `terraform fmt` / `tofu fmt` exactly.
fn align_equals_run(entries: &mut [(ObjectKey, hcl_edit::expr::ObjectValue)]) {
    let max_key_len = entries
        .iter()
        .map(|(k, _)| object_key_str(k).len())
        .max()
        .unwrap_or(0);

    for (key, value) in entries.iter_mut() {
        let padding = max_key_len - object_key_str(key).len() + 1;
        key.decor_mut().set_suffix(" ".repeat(padding));
        value.expr_mut().decor_mut().set_prefix(" ");
    }
}

/// Check if an array is multi-line by looking at whether any element's prefix
/// or the array's trailing contains a newline.
fn is_multiline_array(arr: &hcl_edit::expr::Array) -> bool {
    arr.trailing().to_string().contains('\n')
        || arr.iter().any(|elem| {
            elem.decor()
                .prefix()
                .is_some_and(|p| p.to_string().contains('\n'))
        })
}

/// Maximum line width before a single-line object literal gets expanded
/// onto multiple lines. Matches the conventional Terraform/HCL line budget.
const MAX_LINE_WIDTH: usize = 80;

/// Decide whether a currently single-line object should be expanded onto
/// multiple lines. Triggers when there's more than one entry and the rendered
/// single-line form — including the leading indent and any `key = ` prefix
/// (`prefix_width`) — would exceed `MAX_LINE_WIDTH`.
fn should_expand_single_line_object(obj: &Object, depth: usize, prefix_width: usize) -> bool {
    if is_multiline_object(obj) {
        return false;
    }
    if obj.iter().count() < 2 {
        return false;
    }
    // Object doesn't implement Display directly; wrap it in an Expression
    // (which does) to render the single-line form for measurement.
    let rendered = Expression::Object(obj.clone()).to_string();
    let line_width = depth * 2 + prefix_width + rendered.len();
    line_width > MAX_LINE_WIDTH
}

/// Check if an object is multi-line by looking at whether any key's prefix
/// contains a newline (indicating the object spans multiple lines).
fn is_multiline_object(obj: &Object) -> bool {
    obj.iter().any(|(key, _)| {
        key.decor()
            .prefix()
            .is_some_and(|p| p.to_string().contains('\n'))
    })
}

/// Format an HCL object in-place. Under
/// [`FormatStyle::Opinionated`], applies the single-line-first /
/// multi-line-second tiering with alphabetical sort within each
/// tier (matching the body-level rule). Under
/// [`FormatStyle::Minimal`], preserves source order and only
/// aligns `=` signs.
///
/// Blank-line-separated groups in the original source are
/// preserved and each group is sorted/aligned independently.
fn format_object(obj: &mut Object, depth: usize, style: FormatStyle) {
    let indent = "  ".repeat(depth + 1);

    // Preserve object-level decor
    let obj_decor = obj.decor().clone();

    // Drain all entries
    let old_obj = std::mem::take(obj);
    // Capture the original trailing (whitespace between the last entry and the
    // closing `}`) before consuming the object — minimal mode preserves any
    // blank lines it holds, matching `tofu fmt`.
    let old_trailing = old_obj.trailing().to_string();
    let mut entries: Vec<(ObjectKey, hcl_edit::expr::ObjectValue)> = old_obj.into_iter().collect();

    // Decide the canonical terminator before sorting can shuffle a no-comma
    // entry into the middle of comma-terminated ones. `tofu fmt` preserves the
    // object's comma style, so we mirror it: if the object used commas at all,
    // normalize every entry to a comma; otherwise newline-terminate them.
    let use_commas = entries
        .iter()
        .any(|(_, v)| matches!(v.terminator(), ObjectValueTerminator::Comma));

    // Recurse into nested values
    for (key, value) in &mut entries {
        let prefix_width = object_key_str(key).len() + 3; // `key = `
        format_expression(value.expr_mut(), depth + 1, style, prefix_width);
    }

    // Opinionated style: rewrite every `:` separator to `=`
    // BEFORE alignment, so the whole object renders in the
    // canonical equals-form and lands in a single uniformly-
    // aligned column. Minimal style preserves the user's
    // separator choice (matching `tofu fmt` exactly).
    if style.is_opinionated() {
        for (_, value) in &mut entries {
            if matches!(value.assignment(), ObjectValueAssignment::Colon) {
                value.set_assignment(ObjectValueAssignment::Equals);
            }
        }
    }

    // Split entries into blank-line-separated groups under
    // minimal style. Opinionated style ignores author blank
    // lines: the whole object is one logical group so all
    // single-line keys collapse and sort together.
    let groups = if style.is_opinionated() {
        vec![entries]
    } else {
        split_object_groups(entries)
    };

    // Process each group: partition single/multi, sort, align, re-insert.
    let mut is_first = true;
    let mut last_terminator = ObjectValueTerminator::Newline;

    for (group_idx, group_entries) in groups.into_iter().enumerate() {
        // Whether this group needs a blank line before its first entry.
        let need_group_blank = !is_first && group_idx > 0;
        let mut group_blank_emitted = false;

        // Partition into single-line and multi-line; under Minimal we
        // skip the partition entirely so the source order survives.
        let (mut single, mut multi): (Vec<_>, Vec<_>) = if style.is_opinionated() {
            group_entries
                .into_iter()
                .partition(|(_, v)| !v.expr().to_string().contains('\n'))
        } else {
            (group_entries, Vec::new())
        };

        if style.is_opinionated() {
            single.sort_by_key(|(a, _)| object_key_str(a));
            multi.sort_by_key(|(a, _)| object_key_str(a));
        }

        // Align `=` signs only within consecutive runs of single-line
        // entries. Under Opinionated everything in `single` qualifies;
        // under Minimal we have to walk `single` (which holds the
        // original order, mixed single + multi) and align run-by-run.
        if style.is_opinionated() {
            align_object_keys(&mut single);
            for (key, value) in multi.iter_mut() {
                key.decor_mut().set_suffix(" ");
                value.expr_mut().decor_mut().set_prefix(" ");
            }
        } else {
            align_object_entries_in_place(&mut single);
        }

        let has_single = !single.is_empty();

        for (mut key, mut value) in single {
            // After a Newline-terminated entry the line break is already
            // emitted by that terminator, so this entry sits on its own line
            // regardless of its own prefix. After a comma (or no terminator)
            // the layout depends on what the author wrote: `tofu fmt` preserves
            // an object's line layout exactly — comma-separated entries sharing
            // a physical line stay together, it never reflows them one-per-line.
            let prev_was_newline = matches!(last_terminator, ObjectValueTerminator::Newline);
            let had_source_newline = count_leading_newlines(
                &key.decor()
                    .prefix()
                    .map(|p| p.to_string())
                    .unwrap_or_default(),
            ) > 0;
            let needs_leading_newline = if style.is_opinionated() {
                // Opinionated: one entry per line (newline unless the previous
                // terminator already supplied it).
                !is_first && !prev_was_newline
            } else {
                // Minimal: only emit our own newline when the author wrote one
                // after a comma; an own-line entry already has its newline from
                // the previous terminator.
                !is_first && !prev_was_newline && had_source_newline
            };
            let add_structural = is_first || needs_leading_newline;
            // Minimal: an entry the author kept on the previous entry's line
            // (comma terminator, no newline before it) gets a single space
            // after the comma — no newline/indent.
            let same_line =
                !style.is_opinionated() && !is_first && !prev_was_newline && !had_source_newline;
            let blank_lines = object_entry_blank_lines(
                style,
                &key,
                add_structural,
                need_group_blank && !group_blank_emitted,
            );
            // Minimal style preserves a comment that hugged the opening `{` on
            // the same line as the first entry (`tofu fmt` keeps it inline).
            let (inline_comment, inline_blanks, comments) = if is_first && !style.is_opinionated()
            {
                let raw = key
                    .decor()
                    .prefix()
                    .map(|p| p.to_string())
                    .unwrap_or_default();
                split_leading_inline_comment(&raw)
            } else {
                (None, 0, extract_key_comments(&key))
            };
            // With an inline comment the raw prefix starts with the comment
            // text, so the blank count above came back 0; use the count
            // recovered from after the comment line.
            let blank_lines = if inline_comment.is_some() {
                inline_blanks
            } else {
                blank_lines
            };
            let prefix = if same_line {
                String::from(" ")
            } else {
                build_object_key_prefix(
                    add_structural,
                    blank_lines,
                    inline_comment.as_deref(),
                    &comments,
                    &indent,
                )
            };
            key.decor_mut().set_prefix(prefix);
            normalize_terminator(&mut value, style, use_commas);
            last_terminator = value.terminator();
            obj.insert(key, value);
            is_first = false;
            group_blank_emitted = true;
        }
        for (i, (mut key, mut value)) in multi.into_iter().enumerate() {
            let want_blank = (i > 0 || has_single) || (need_group_blank && !group_blank_emitted);
            let blank_lines = object_entry_blank_lines(style, &key, is_first, want_blank);
            let (inline_comment, inline_blanks, comments) = if is_first && !style.is_opinionated()
            {
                let raw = key
                    .decor()
                    .prefix()
                    .map(|p| p.to_string())
                    .unwrap_or_default();
                split_leading_inline_comment(&raw)
            } else {
                (None, 0, extract_key_comments(&key))
            };
            let blank_lines = if inline_comment.is_some() {
                inline_blanks
            } else {
                blank_lines
            };
            let prefix = build_object_key_prefix(
                is_first,
                blank_lines,
                inline_comment.as_deref(),
                &comments,
                &indent,
            );
            key.decor_mut().set_prefix(prefix);
            normalize_terminator(&mut value, style, use_commas);
            last_terminator = value.terminator();
            obj.insert(key, value);
            is_first = false;
            group_blank_emitted = true;
        }
    }

    // Restore object-level decor and normalize trailing indent (controls `}` position).
    // If the last entry's terminator is Newline, it already produces the
    // newline before the closing `}`; otherwise (Comma or None) we have to
    // prepend one ourselves so `}` doesn't end up on the same line as the
    // last value.
    *obj.decor_mut() = obj_decor;
    let closing_indent = "  ".repeat(depth);
    let trailing = if style.is_opinionated() {
        // Opinionated: drop any blank lines before `}`; keep just the newline
        // (added here when the last terminator didn't already supply one).
        match last_terminator {
            ObjectValueTerminator::Newline => closing_indent,
            _ => format!("\n{closing_indent}"),
        }
    } else {
        // Minimal (`tofu fmt` parity): preserve the original blank lines before
        // `}`, re-indenting only the final line that the `}` sits on.
        match old_trailing.rfind('\n') {
            Some(idx) => format!("{}{closing_indent}", &old_trailing[..=idx]),
            None => closing_indent,
        }
    };
    obj.set_trailing(trailing);
}

/// Normalize a multi-line object entry's terminator to a single canonical
/// form. Sorting an object can move a no-comma entry into the middle of
/// comma-terminated entries, leaving inconsistent separators (`a = 1,` /
/// `b = 2` / `c = 3,`). Under the opinionated style we normalize every entry
/// to the object's dominant separator — commas if the object used any
/// (`use_commas`), otherwise newlines — which keeps the output uniform and
/// idempotent while matching `tofu fmt`'s preservation of the comma style.
/// Minimal style is left untouched to preserve `tofu fmt` parity exactly.
fn normalize_terminator(
    value: &mut hcl_edit::expr::ObjectValue,
    style: FormatStyle,
    use_commas: bool,
) {
    if !style.is_opinionated() {
        return;
    }
    let suffix = value
        .expr()
        .decor()
        .suffix()
        .map(|s| s.to_string())
        .unwrap_or_default();
    let comments = extract_comments(&suffix);
    if comments.is_empty() {
        // Clear any trailing whitespace decor on the value (e.g. the space a
        // just-expanded inline object carried before its closing `}`), so a
        // comma terminator renders as `4,` rather than `4 ,`.
        value.expr_mut().decor_mut().set_suffix("");
        value.set_terminator(if use_commas {
            ObjectValueTerminator::Comma
        } else {
            ObjectValueTerminator::Newline
        });
    } else {
        // The suffix carries an inline comment (`a = 1 # note`). Keep the
        // comment — clearing the suffix deleted it (Rule 7 violation). The
        // entry is newline-terminated even in a comma-style object: a comma
        // emitted after a line comment would be swallowed into the comment.
        let mut kept = String::new();
        for comment in &comments {
            kept.push(' ');
            kept.push_str(comment);
        }
        value.expr_mut().decor_mut().set_suffix(kept);
        value.set_terminator(ObjectValueTerminator::Newline);
    }
}

/// Align `=` across each contiguous run of single-line object
/// entries inside `entries`, leaving multi-line entries with a
/// plain single-space `=` on either side. Used in Minimal mode
/// where the entries vector still holds the original mixed order.
fn align_object_entries_in_place(entries: &mut [(ObjectKey, hcl_edit::expr::ObjectValue)]) {
    let mut i = 0;
    while i < entries.len() {
        // Skip multi-line entries — give them the canonical single
        // space on either side of `=` and advance. Heredoc values are NOT
        // skipped: their `=` is on the opening line, so they align with their
        // single-line neighbours (matching `tofu fmt`).
        while i < entries.len() && object_value_breaks_alignment(&entries[i].1) {
            entries[i].0.decor_mut().set_suffix(" ");
            entries[i].1.expr_mut().decor_mut().set_prefix(" ");
            i += 1;
        }
        let run_start = i;
        while i < entries.len() && !object_value_breaks_alignment(&entries[i].1) {
            // Comments attached to a key break the alignment run.
            if i > run_start && !extract_key_comments(&entries[i].0).is_empty() {
                break;
            }
            i += 1;
        }
        if i > run_start {
            align_object_key_group(&mut entries[run_start..i]);
        }
    }
}

/// Split object entries into groups separated by blank lines. Uses the
/// previous entry's terminator to determine whether a single `\n` in the
/// prefix is a line-break or a blank line.
fn split_object_groups(
    entries: Vec<(ObjectKey, hcl_edit::expr::ObjectValue)>,
) -> Vec<Vec<(ObjectKey, hcl_edit::expr::ObjectValue)>> {
    let mut groups: Vec<Vec<(ObjectKey, hcl_edit::expr::ObjectValue)>> = Vec::new();
    let mut current: Vec<(ObjectKey, hcl_edit::expr::ObjectValue)> = Vec::new();

    for (i, entry) in entries.into_iter().enumerate() {
        if i > 0 && !current.is_empty() {
            let prefix = entry
                .0
                .decor()
                .prefix()
                .map(|p| p.to_string())
                .unwrap_or_default();
            let prev_terminator = current.last().map(|(_, v)| v.terminator());
            let is_blank = match prev_terminator {
                Some(ObjectValueTerminator::Newline) => {
                    has_blank_line_after_newline_terminator(&prefix)
                }
                _ => has_blank_line_after_other_terminator(&prefix),
            };
            if is_blank {
                groups.push(std::mem::take(&mut current));
            }
        }
        current.push(entry);
    }
    if !current.is_empty() {
        groups.push(current);
    }
    groups
}

/// Extract comment lines from an object key's prefix decor.
fn extract_key_comments(key: &ObjectKey) -> Vec<String> {
    let prefix = key
        .decor()
        .prefix()
        .map(|p| p.to_string())
        .unwrap_or_default();
    extract_comments(&prefix)
}

/// Top-level "run": either a contiguous span of attributes, or a contiguous
/// span of blocks sharing the same `ident`.
#[derive(PartialEq, Eq)]
enum TopLevelRunKind {
    Attr,
    Block(String),
}

/// Format the top-level of a `Body`. Recurses into each structure first, then
/// groups the structures into runs:
///   - `Attr`: consecutive attribute assignments (as in a `.tfvars` file).
///     Within a run, user-authored blank-line groups are preserved; each group
///     sorts/aligns independently via `format_structure_group`.
///   - `Block(ident)`: consecutive blocks of the same ident. Sortable idents
///     (`variable` / `resource` / `data` / `output`) sort alphabetically by
///     label; others keep their order. A blank line is emitted between each
///     block in the run.
///
/// Runs are separated by a blank line.
pub fn sort_top_level(body: &mut Body, style: FormatStyle) {
    let body_decor = body.decor().clone();
    let prefer_oneline = body.prefer_oneline();
    let prefer_omit_trailing_newline = body.prefer_omit_trailing_newline();

    let old_body = std::mem::take(body);
    let mut structures: Vec<Structure> = old_body.into_iter().collect();

    // Recurse into each structure first so nested bodies and object values are
    // formatted before we reorder the top level.
    for structure in &mut structures {
        match structure {
            Structure::Block(block) => {
                let ident = block.ident.as_str().to_string();
                format_body(&mut block.body, 0, Some(&ident), style);
            }
            Structure::Attribute(attr) => {
                let prefix_width = attr.key.as_str().len() + 3; // `key = `
                format_expression(&mut attr.value, 0, style, prefix_width);
            }
        }
    }

    // Group into runs.
    let mut runs: Vec<(TopLevelRunKind, Vec<Structure>)> = Vec::new();
    for s in structures {
        let kind = match &s {
            Structure::Attribute(_) => TopLevelRunKind::Attr,
            Structure::Block(b) => TopLevelRunKind::Block(b.ident.as_str().to_string()),
        };
        match runs.last_mut() {
            Some((last_kind, group)) if *last_kind == kind => {
                group.push(s);
            }
            _ => {
                runs.push((kind, vec![s]));
            }
        }
    }

    // Sort sortable block runs by label — only under the
    // opinionated style. Minimal mode preserves source order.
    if style.is_opinionated() {
        for (kind, group) in &mut runs {
            if let TopLevelRunKind::Block(ident) = kind
                && matches!(ident.as_str(), "variable" | "resource" | "data" | "output")
            {
                group.sort_by_key(label_sort_key);
            }
        }
    }

    // Flatten runs back into the body.
    let mut any_emitted = false;
    for (kind, group) in runs {
        match kind {
            TopLevelRunKind::Attr => {
                // Minimal: preserve user-authored blank-line groups
                // within the run, format each independently.
                // Opinionated: ignore blank-line groups so all
                // single-line attrs collapse + sort across the
                // entire run (matches the in-block semantic).
                let sub_groups = if style.is_opinionated() {
                    vec![group]
                } else {
                    split_body_groups(group)
                };
                for (sub_idx, sub_group) in sub_groups.into_iter().enumerate() {
                    // Minimal mode never forces a blank here: each structure's
                    // prefix already carries the author's exact blank-line
                    // count (`tofu fmt` doesn't insert a blank between a block
                    // and a following top-level attribute). Opinionated mode
                    // normalizes run/group boundaries to one blank line.
                    let want_group_blank = if style.is_opinionated() {
                        (any_emitted && sub_idx == 0) || sub_idx > 0
                    } else {
                        false
                    };
                    any_emitted = format_structure_group(
                        body,
                        sub_group,
                        "",
                        want_group_blank,
                        any_emitted,
                        None,
                        style,
                        false,
                    );
                }
            }
            TopLevelRunKind::Block(_) => {
                for mut s in group {
                    if !any_emitted {
                        // First top-level structure: no preceding block, so no
                        // blank-line separator — but the file's leading comments
                        // live in THIS structure's prefix. Preserve them instead
                        // of blindly clearing (was data loss). `tofu fmt` keeps a
                        // single blank line between header comments and the first
                        // block, so re-emit one if the source had any.
                        let existing = s
                            .decor()
                            .prefix()
                            .map(|p| p.to_string())
                            .unwrap_or_default();
                        let comments = extract_comments(&existing);
                        if comments.is_empty() {
                            s.decor_mut().set_prefix("");
                        } else {
                            let mut prefix = String::new();
                            for comment in &comments {
                                push_comment(&mut prefix, comment, "");
                            }
                            if blank_after_comments(&existing) {
                                prefix.push('\n');
                            }
                            s.decor_mut().set_prefix(prefix);
                        }
                    } else {
                        // Preserve comments; set the blank-line separator.
                        // Opinionated normalizes spacing between top-level blocks
                        // to one blank line. Minimal mirrors `tofu fmt`: keep the
                        // author's exact blank-line count (which may be zero for
                        // adjacent blocks) — forcing a blank here breaks parity.
                        let existing = s
                            .decor()
                            .prefix()
                            .map(|p| p.to_string())
                            .unwrap_or_default();
                        let comments = extract_comments(&existing);
                        let blank_lines = if style.is_opinionated() {
                            1
                        } else {
                            count_leading_newlines(&existing)
                        };
                        s.decor_mut()
                            .set_prefix(build_body_prefix(blank_lines, &comments, ""));
                    }
                    body.push(s);
                    any_emitted = true;
                }
            }
        }
    }

    *body.decor_mut() = body_decor;
    body.set_prefer_oneline(prefer_oneline);
    body.set_prefer_omit_trailing_newline(prefer_omit_trailing_newline);
}

/// Build a sort key from a block's labels (used for top-level sorting).
fn label_sort_key(structure: &Structure) -> String {
    match structure {
        Structure::Block(block) => block
            .labels
            .iter()
            .map(|l| l.as_str().to_string())
            .collect::<Vec<_>>()
            .join("\0"),
        Structure::Attribute(attr) => attr.key.as_str().to_string(),
    }
}
