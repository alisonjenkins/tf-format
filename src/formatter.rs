use hcl_edit::Decorate;
use hcl_edit::expr::{Expression, Object, ObjectKey, ObjectValueTerminator};
use hcl_edit::structure::{Body, Structure};

use crate::classify::is_multiline;

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
pub fn format_body(body: &mut Body, depth: usize) {
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
                format_body(&mut block.body, depth + 1);
            }
            Structure::Attribute(attr) => {
                format_expression(&mut attr.value, depth + 1);
            }
        }
    }

    // 4-way partition: priority single/multi, then normal single/multi
    let mut priority_single: Vec<Structure> = Vec::new();
    let mut priority_multi: Vec<Structure> = Vec::new();
    let mut normal_single: Vec<Structure> = Vec::new();
    let mut normal_multi: Vec<Structure> = Vec::new();

    for s in structures {
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

    // Priority items sort by their predefined order
    priority_single.sort_by_key(|s| priority_index(s).unwrap_or(usize::MAX));
    priority_multi.sort_by_key(|s| priority_index(s).unwrap_or(usize::MAX));

    // Normal items sort alphabetically
    normal_single.sort_by_key(sort_key);
    normal_multi.sort_by_key(sort_key);

    // Align `=` signs independently within each single-line group
    align_body_attributes(&mut priority_single);
    align_body_attributes(&mut normal_single);

    // Rebuild body: priority first, then normal, with appropriate spacing
    let has_priority = !priority_single.is_empty() || !priority_multi.is_empty();
    let has_priority_single = !priority_single.is_empty();

    // 1. Priority single-line attrs (no blank lines between)
    for mut s in priority_single {
        adjust_structure_prefix(&mut s, false, &indent);
        body.push(s);
    }

    // 2. Priority multi-line blocks (blank line before each)
    for (i, mut s) in priority_multi.into_iter().enumerate() {
        let want_blank = i > 0 || has_priority_single;
        adjust_structure_prefix(&mut s, want_blank, &indent);
        body.push(s);
    }

    // 3. Normal single-line attrs (blank line before first if priority existed)
    let has_normal_single = !normal_single.is_empty();
    for (i, mut s) in normal_single.into_iter().enumerate() {
        let want_blank = i == 0 && has_priority;
        adjust_structure_prefix(&mut s, want_blank, &indent);
        body.push(s);
    }

    // 4. Normal multi-line attrs/blocks (blank line before each)
    for (i, mut s) in normal_multi.into_iter().enumerate() {
        let want_blank = i > 0 || has_normal_single || has_priority;
        adjust_structure_prefix(&mut s, want_blank, &indent);
        body.push(s);
    }

    // Restore body-level metadata
    *body.decor_mut() = body_decor;
    body.set_prefer_oneline(prefer_oneline);
    body.set_prefer_omit_trailing_newline(prefer_omit_trailing_newline);
}

/// Recursively format an expression in-place. Sorts object keys and recurses
/// into nested objects, arrays, function call arguments, and other compound
/// expressions.
fn format_expression(expr: &mut Expression, depth: usize) {
    match expr {
        Expression::Object(obj) => {
            // Format multi-line objects, and also expand any single-line object
            // whose rendered width exceeds the line-length budget so we don't
            // emit huge unreadable one-liners.
            if is_multiline_object(obj) || should_expand_single_line_object(obj, depth) {
                format_object(obj, depth);
            }
        }
        Expression::Array(arr) => {
            // Multi-line arrays should always have a trailing comma so that
            // adding a new entry only changes one line in the diff.
            if is_multiline_array(arr) && !arr.is_empty() {
                // When the input has no trailing comma, the parser stores the
                // whitespace before `]` in the last element's suffix. Move it
                // to the array's trailing so the comma lands on the right line.
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
                    // If the element shares a line with the array's `[` (no
                    // newline in its prefix), the inline form `[{ ... }]` makes
                    // the object behave as if it were a direct attribute value
                    // at the array's own depth. Otherwise the canonical multi-
                    // line form puts each element one level deeper than the
                    // array.
                    let elem_inline = elem
                        .decor()
                        .prefix()
                        .is_none_or(|p| !p.to_string().contains('\n'));
                    let elem_depth = if elem_inline { depth } else { depth + 1 };
                    format_expression(elem, elem_depth);
                }
            }
        }
        Expression::FuncCall(call) => {
            for arg in call.args.iter_mut() {
                format_expression(arg, depth);
            }
        }
        Expression::Parenthesis(paren) => {
            format_expression(paren.inner_mut(), depth);
        }
        Expression::Conditional(cond) => {
            format_expression(&mut cond.cond_expr, depth);
            format_expression(&mut cond.true_expr, depth);
            format_expression(&mut cond.false_expr, depth);
        }
        Expression::Traversal(trav) => {
            format_expression(&mut trav.expr, depth);
        }
        Expression::ForExpr(for_expr) => {
            format_expression(&mut for_expr.intro.collection_expr, depth);
            if let Some(key_expr) = &mut for_expr.key_expr {
                format_expression(key_expr, depth);
            }
            format_expression(&mut for_expr.value_expr, depth);
        }
        Expression::UnaryOp(op) => {
            format_expression(&mut op.expr, depth);
        }
        Expression::BinaryOp(op) => {
            format_expression(&mut op.lhs_expr, depth);
            format_expression(&mut op.rhs_expr, depth);
        }
        // Leaf expressions (Null, Bool, Number, String, Variable, etc.)
        _ => {}
    }
}

/// Vertically align the `=` signs of consecutive single-line attributes in a
/// body by padding the key's decor suffix.
///
/// Matches `terraform fmt` / `tofu fmt` semantics: a comment line attached to
/// an attribute breaks the alignment group, so attributes above and below the
/// comment are aligned independently. (tf-format never inserts blank lines
/// within a single-line attribute sequence, so blank lines are not a concern
/// here.)
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
/// matching `terraform fmt` / `tofu fmt`.
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

/// Sort the keys of an HCL object in-place, applying the same single-line-first
/// then multi-line rules.
fn format_object(obj: &mut Object, depth: usize) {
    let indent = "  ".repeat(depth + 1);

    // Preserve object-level decor
    let obj_decor = obj.decor().clone();

    // Drain all entries
    let old_obj = std::mem::take(obj);
    let mut entries: Vec<(ObjectKey, hcl_edit::expr::ObjectValue)> = old_obj.into_iter().collect();

    // Recurse into nested values
    for (_, value) in &mut entries {
        format_expression(value.expr_mut(), depth + 1);
    }

    // Partition into single-line and multi-line
    let (mut single, mut multi): (Vec<_>, Vec<_>) = entries
        .into_iter()
        .partition(|(_, v)| !v.expr().to_string().contains('\n'));

    // Sort each group
    single.sort_by(|(a, _), (b, _)| object_key_str(a).cmp(&object_key_str(b)));
    multi.sort_by(|(a, _), (b, _)| object_key_str(a).cmp(&object_key_str(b)));

    // Align `=` signs only within the single-line group. `tofu fmt` does not
    // align `=` for multi-line values; each multi-line entry just gets a
    // single space on either side of `=`.
    align_object_keys(&mut single);
    for (key, value) in multi.iter_mut() {
        key.decor_mut().set_suffix(" ");
        value.expr_mut().decor_mut().set_prefix(" ");
    }

    // Re-insert in order: single-line first, then multi-line
    let has_single = !single.is_empty();
    let mut is_first = true;
    let mut last_terminator = ObjectValueTerminator::Newline;

    for (mut key, value) in single {
        // If the previous entry's terminator wasn't a newline (e.g. an
        // expanded one-liner whose entries used `,` or had no terminator at
        // all), we have to inject the line break ourselves via the prefix.
        let needs_leading_newline =
            !is_first && !matches!(last_terminator, ObjectValueTerminator::Newline);
        let comments = extract_key_comments(&key);
        let prefix =
            build_object_key_prefix(is_first || needs_leading_newline, false, &comments, &indent);
        key.decor_mut().set_prefix(prefix);
        last_terminator = value.terminator();
        obj.insert(key, value);
        is_first = false;
    }
    for (i, (mut key, value)) in multi.into_iter().enumerate() {
        let want_blank = i > 0 || has_single;
        let comments = extract_key_comments(&key);
        let prefix = build_object_key_prefix(is_first, want_blank, &comments, &indent);
        key.decor_mut().set_prefix(prefix);
        last_terminator = value.terminator();
        obj.insert(key, value);
        is_first = false;
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

/// Extract comment lines from an object key's prefix decor.
fn extract_key_comments(key: &ObjectKey) -> Vec<String> {
    let prefix = key
        .decor()
        .prefix()
        .map(|p| p.to_string())
        .unwrap_or_default();
    extract_comments(&prefix)
}

/// Sort top-level blocks in a body. Groups consecutive blocks of the same type
/// and sorts within each group for sortable block types (variable, resource,
/// data, output).
pub fn sort_top_level(body: &mut Body) {
    let body_decor = body.decor().clone();
    let prefer_oneline = body.prefer_oneline();
    let prefer_omit_trailing_newline = body.prefer_omit_trailing_newline();

    let old_body = std::mem::take(body);
    let structures: Vec<Structure> = old_body.into_iter().collect();

    // Group consecutive blocks of the same ident
    let mut groups: Vec<Vec<Structure>> = Vec::new();
    let mut current_group: Vec<Structure> = Vec::new();
    let mut current_ident: Option<String> = None;

    for s in structures {
        let ident = match &s {
            Structure::Block(b) => Some(b.ident.as_str().to_string()),
            Structure::Attribute(_) => None,
        };

        if ident == current_ident && ident.is_some() {
            current_group.push(s);
        } else {
            if !current_group.is_empty() {
                groups.push(std::mem::take(&mut current_group));
            }
            current_ident = ident;
            current_group.push(s);
        }
    }
    if !current_group.is_empty() {
        groups.push(current_group);
    }

    // Sort within sortable groups
    for group in &mut groups {
        let should_sort = group.first().is_some_and(|s| {
            matches!(
                s,
                Structure::Block(b) if matches!(b.ident.as_str(), "variable" | "resource" | "data" | "output")
            )
        });

        if should_sort {
            group.sort_by(|a, b| {
                let a_key = label_sort_key(a);
                let b_key = label_sort_key(b);
                a_key.cmp(&b_key)
            });
        }
    }

    // Flatten back into body, adjusting top-level prefixes after sort
    let mut is_first_structure = true;
    for group in groups {
        for mut s in group {
            if is_first_structure {
                // First structure in file: no leading whitespace
                s.decor_mut().set_prefix("");
                is_first_structure = false;
            } else {
                // Preserve comments but normalize spacing between top-level blocks
                let existing = s
                    .decor()
                    .prefix()
                    .map(|p| p.to_string())
                    .unwrap_or_default();
                let comments = extract_comments(&existing);
                // Top-level blocks are separated by blank lines (the Body encoding
                // adds \n after each structure, so one extra \n = one blank line)
                let mut prefix = String::from("\n");
                for comment in &comments {
                    prefix.push_str(comment.trim());
                    prefix.push('\n');
                }
                s.decor_mut().set_prefix(prefix);
            }
            body.push(s);
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
