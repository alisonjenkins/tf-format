use hcl_edit::Decorate;
use hcl_edit::expr::{Expression, Object, ObjectKey, ObjectValueTerminator};
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

/// Attributes that are meta-arguments or module-specific and should appear at
/// the top of a block, in this fixed order.
const PRIORITY_ATTRS: &[&str] = &[
    "source",
    "version",
    "count",
    "for_each",
    "provider",
    "depends_on",
];

/// Blocks that are meta-arguments and should appear at the top of a block.
const PRIORITY_BLOCKS: &[&str] = &["lifecycle"];

/// Returns the priority index for a structure if it's a priority item, or None.
fn priority_index(structure: &Structure) -> Option<usize> {
    match structure {
        Structure::Attribute(attr) => {
            let key = attr.key.as_str();
            PRIORITY_ATTRS.iter().position(|&k| k == key)
        }
        Structure::Block(block) => {
            let ident = block.ident.as_str();
            PRIORITY_BLOCKS
                .iter()
                .position(|&k| k == ident)
                .map(|i| PRIORITY_ATTRS.len() + i)
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
    prefix.starts_with('\n') || prefix.contains("\n\n")
}

/// Detect a blank line in a prefix when the previous entry may use a
/// non-Newline terminator (e.g. Comma). Only `\n\n` counts.
fn has_blank_line_after_other_terminator(prefix: &str) -> bool {
    prefix.contains("\n\n")
}

/// Extract comment lines from a decor prefix string.
fn extract_comments(prefix: &str) -> Vec<String> {
    prefix
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with('#')
                || trimmed.starts_with("//")
                || trimmed.starts_with("/*")
                || trimmed.starts_with('*')
                || trimmed.starts_with("*/")
        })
        .map(|s| s.to_string())
        .collect()
}

/// Build a prefix for a body structure. The Body/Block encoding adds `\n`
/// between structures, so the prefix only needs indent (and optionally an
/// extra `\n` for a blank line separator).
fn build_body_prefix(want_blank_line: bool, comments: &[String], indent: &str) -> String {
    let mut prefix = String::new();
    if want_blank_line {
        prefix.push('\n');
    }
    for comment in comments {
        prefix.push_str(indent);
        prefix.push_str(comment.trim());
        prefix.push('\n');
    }
    prefix.push_str(indent);
    prefix
}

/// Build a prefix for an object key. In objects, `\n` comes from the previous
/// entry's ObjectValueTerminator::Newline, except for the first entry where
/// we need a `\n` after the opening `{`.
fn build_object_key_prefix(
    is_first: bool,
    want_blank_line: bool,
    comments: &[String],
    indent: &str,
) -> String {
    let mut prefix = String::new();
    if is_first {
        prefix.push('\n');
    }
    if want_blank_line {
        prefix.push('\n');
    }
    for comment in comments {
        prefix.push_str(indent);
        prefix.push_str(comment.trim());
        prefix.push('\n');
    }
    prefix.push_str(indent);
    prefix
}

/// Adjust the prefix decoration on a body structure.
fn adjust_structure_prefix(structure: &mut Structure, want_blank_line: bool, indent: &str) {
    let decor = structure.decor_mut();
    let existing_prefix = decor.prefix().map(|p| p.to_string()).unwrap_or_default();

    let comments = extract_comments(&existing_prefix);
    let new_prefix = build_body_prefix(want_blank_line, &comments, indent);
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
pub fn format_body(body: &mut Body, depth: usize, style: FormatStyle) {
    let indent = "  ".repeat(depth + 1);

    // Preserve body-level metadata
    let body_decor = body.decor().clone();
    let prefer_oneline = body.prefer_oneline();
    let prefer_omit_trailing_newline = body.prefer_omit_trailing_newline();

    // Drain all structures from the body
    let old_body = std::mem::take(body);
    let mut structures: Vec<Structure> = old_body.into_iter().collect();

    // Recurse into nested blocks and expressions
    for structure in &mut structures {
        match structure {
            Structure::Block(block) => {
                format_body(&mut block.body, depth + 1, style);
            }
            Structure::Attribute(attr) => {
                format_expression(&mut attr.value, depth + 1, style);
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
            style,
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
/// already pushed?" flag. Returns the updated flag.
fn format_structure_group(
    body: &mut Body,
    group: Vec<Structure>,
    indent: &str,
    want_group_blank: bool,
    any_emitted_before: bool,
    style: FormatStyle,
) -> bool {
    if !style.is_opinionated() {
        return format_structure_group_minimal(
            body,
            group,
            indent,
            want_group_blank,
            any_emitted_before,
        );
    }

    let mut priority_single: Vec<Structure> = Vec::new();
    let mut priority_multi: Vec<Structure> = Vec::new();
    let mut normal_single: Vec<Structure> = Vec::new();
    let mut normal_multi: Vec<Structure> = Vec::new();

    for s in group {
        if priority_index(&s).is_some() {
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

    priority_single.sort_by_key(|s| priority_index(s).unwrap_or(usize::MAX));
    priority_multi.sort_by_key(|s| priority_index(s).unwrap_or(usize::MAX));
    normal_single.sort_by_key(sort_key);
    normal_multi.sort_by_key(sort_key);

    align_body_attributes(&mut priority_single);
    align_body_attributes(&mut normal_single);

    let has_priority = !priority_single.is_empty() || !priority_multi.is_empty();
    let has_priority_single = !priority_single.is_empty();

    let mut any_emitted = any_emitted_before;

    for (i, mut s) in priority_single.into_iter().enumerate() {
        let want_blank = if i == 0 { want_group_blank } else { false };
        adjust_structure_prefix(&mut s, want_blank, indent);
        body.push(s);
        any_emitted = true;
    }

    for (i, mut s) in priority_multi.into_iter().enumerate() {
        let want_blank = i > 0 || has_priority_single || (i == 0 && want_group_blank);
        adjust_structure_prefix(&mut s, want_blank, indent);
        body.push(s);
        any_emitted = true;
    }

    let has_normal_single = !normal_single.is_empty();

    for (i, mut s) in normal_single.into_iter().enumerate() {
        let want_blank = i == 0 && (has_priority || want_group_blank);
        adjust_structure_prefix(&mut s, want_blank, indent);
        body.push(s);
        any_emitted = true;
    }

    for (i, mut s) in normal_multi.into_iter().enumerate() {
        let want_blank = i > 0 || has_normal_single || has_priority || (i == 0 && want_group_blank);
        adjust_structure_prefix(&mut s, want_blank, indent);
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
) -> bool {
    align_body_attributes_in_place(&mut group);

    let mut any_emitted = any_emitted_before;
    for (i, mut s) in group.into_iter().enumerate() {
        let want_blank = i == 0 && want_group_blank;
        adjust_structure_prefix(&mut s, want_blank, indent);
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
        // Advance past anything that isn't a single-line attribute.
        while i < structures.len() && !is_single_line_attribute(&structures[i]) {
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
    matches!(s, Structure::Attribute(_)) && !is_multiline(s)
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

/// Recursively format an expression in-place. Sorts object keys and recurses
/// into nested objects, arrays, function call arguments, and other compound
/// expressions.
///
/// Under [`FormatStyle::Minimal`] the object-key sort and the
/// single-line-object expansion are skipped, and trailing-comma
/// insertion on multi-line arrays is suppressed.
fn format_expression(expr: &mut Expression, depth: usize, style: FormatStyle) {
    match expr {
        Expression::Object(obj) => {
            // Format multi-line objects, and also expand any single-line object
            // whose rendered width exceeds the line-length budget so we don't
            // emit huge unreadable one-liners. The expansion is opinionated
            // — it changes the source layout — so we skip it under
            // FormatStyle::Minimal.
            let should_expand =
                style.is_opinionated() && should_expand_single_line_object(obj, depth);
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
            }
            for i in 0..arr.len() {
                if let Some(elem) = arr.get_mut(i) {
                    let elem_inline = elem
                        .decor()
                        .prefix()
                        .is_none_or(|p| !p.to_string().contains('\n'));
                    let elem_depth = if elem_inline { depth } else { depth + 1 };
                    format_expression(elem, elem_depth, style);
                }
            }
        }
        Expression::FuncCall(call) => {
            for arg in call.args.iter_mut() {
                format_expression(arg, depth, style);
            }
        }
        Expression::Parenthesis(paren) => {
            format_expression(paren.inner_mut(), depth, style);
        }
        Expression::Conditional(cond) => {
            format_expression(&mut cond.cond_expr, depth, style);
            format_expression(&mut cond.true_expr, depth, style);
            format_expression(&mut cond.false_expr, depth, style);
        }
        Expression::Traversal(trav) => {
            format_expression(&mut trav.expr, depth, style);
        }
        Expression::ForExpr(for_expr) => {
            format_expression(&mut for_expr.intro.collection_expr, depth, style);
            if let Some(key_expr) = &mut for_expr.key_expr {
                format_expression(key_expr, depth, style);
            }
            format_expression(&mut for_expr.value_expr, depth, style);
        }
        Expression::UnaryOp(op) => {
            format_expression(&mut op.expr, depth, style);
        }
        Expression::BinaryOp(op) => {
            format_expression(&mut op.lhs_expr, depth, style);
            format_expression(&mut op.rhs_expr, depth, style);
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
    let max_key_len = entries
        .iter()
        .map(|(k, _)| object_key_str(k).len())
        .max()
        .unwrap_or(0);

    for (key, value) in entries.iter_mut() {
        let padding = max_key_len - object_key_str(key).len() + 1;
        key.decor_mut().set_suffix(" ".repeat(padding));
        // Normalize whitespace after `=` to a single space, matching
        // `terraform fmt` / `tofu fmt`.
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
/// multiple lines. Triggers when there's more than one entry and the
/// rendered single-line form (including the leading attribute indent)
/// would exceed `MAX_LINE_WIDTH`.
fn should_expand_single_line_object(obj: &Object, depth: usize) -> bool {
    if is_multiline_object(obj) {
        return false;
    }
    if obj.iter().count() < 2 {
        return false;
    }
    // Object doesn't implement Display directly; wrap it in an Expression
    // (which does) to render the single-line form for measurement.
    let rendered = Expression::Object(obj.clone()).to_string();
    let line_width = depth * 2 + rendered.len();
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
    let mut entries: Vec<(ObjectKey, hcl_edit::expr::ObjectValue)> = old_obj.into_iter().collect();

    // Recurse into nested values
    for (_, value) in &mut entries {
        format_expression(value.expr_mut(), depth + 1, style);
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
            single.sort_by(|(a, _), (b, _)| object_key_str(a).cmp(&object_key_str(b)));
            multi.sort_by(|(a, _), (b, _)| object_key_str(a).cmp(&object_key_str(b)));
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

        for (mut key, value) in single {
            let needs_leading_newline =
                !is_first && !matches!(last_terminator, ObjectValueTerminator::Newline);
            let want_blank = need_group_blank && !group_blank_emitted;
            let comments = extract_key_comments(&key);
            let prefix = build_object_key_prefix(
                is_first || needs_leading_newline,
                want_blank,
                &comments,
                &indent,
            );
            key.decor_mut().set_prefix(prefix);
            last_terminator = value.terminator();
            obj.insert(key, value);
            is_first = false;
            group_blank_emitted = true;
        }
        for (i, (mut key, value)) in multi.into_iter().enumerate() {
            let want_blank = (i > 0 || has_single)
                || (need_group_blank && !group_blank_emitted);
            let comments = extract_key_comments(&key);
            let prefix = build_object_key_prefix(is_first, want_blank, &comments, &indent);
            key.decor_mut().set_prefix(prefix);
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
    let trailing = match last_terminator {
        ObjectValueTerminator::Newline => closing_indent,
        _ => format!("\n{closing_indent}"),
    };
    obj.set_trailing(trailing);
}

/// Align `=` across each contiguous run of single-line object
/// entries inside `entries`, leaving multi-line entries with a
/// plain single-space `=` on either side. Used in Minimal mode
/// where the entries vector still holds the original mixed order.
fn align_object_entries_in_place(entries: &mut [(ObjectKey, hcl_edit::expr::ObjectValue)]) {
    let mut i = 0;
    while i < entries.len() {
        // Skip multi-line entries — give them the canonical single
        // space on either side of `=` and advance.
        while i < entries.len() && entries[i].1.expr().to_string().contains('\n') {
            entries[i].0.decor_mut().set_suffix(" ");
            entries[i].1.expr_mut().decor_mut().set_prefix(" ");
            i += 1;
        }
        let run_start = i;
        while i < entries.len() && !entries[i].1.expr().to_string().contains('\n') {
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
                format_body(&mut block.body, 0, style);
            }
            Structure::Attribute(attr) => {
                format_expression(&mut attr.value, 0, style);
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
                    let want_group_blank = (any_emitted && sub_idx == 0) || sub_idx > 0;
                    any_emitted = format_structure_group(
                        body,
                        sub_group,
                        "",
                        want_group_blank,
                        any_emitted,
                        style,
                    );
                }
            }
            TopLevelRunKind::Block(_) => {
                for mut s in group {
                    if !any_emitted {
                        s.decor_mut().set_prefix("");
                    } else {
                        // Preserve comments, normalize spacing between top-level
                        // blocks to one blank line.
                        let existing = s
                            .decor()
                            .prefix()
                            .map(|p| p.to_string())
                            .unwrap_or_default();
                        let comments = extract_comments(&existing);
                        let mut prefix = String::from("\n");
                        for comment in &comments {
                            prefix.push_str(comment.trim());
                            prefix.push('\n');
                        }
                        s.decor_mut().set_prefix(prefix);
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
