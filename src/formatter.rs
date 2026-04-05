use hcl_edit::Decorate;
use hcl_edit::expr::{Expression, Object, ObjectKey};
use hcl_edit::structure::{Body, Structure};

use crate::classify::is_multiline;

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
fn object_key_str(key: &ObjectKey) -> String {
    match key {
        ObjectKey::Ident(ident) => ident.as_str().to_string(),
        ObjectKey::Expression(expr) => expr.to_string(),
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

    // Partition into single-line and multi-line groups
    let (mut single_line, mut multi_line): (Vec<_>, Vec<_>) =
        structures.into_iter().partition(|s| !is_multiline(s));

    // Sort each group alphabetically
    single_line.sort_by_key(sort_key);
    multi_line.sort_by_key(sort_key);

    // Align `=` signs within the single-line attribute group
    align_body_attributes(&mut single_line);

    // Rebuild body with correct spacing
    let has_single = !single_line.is_empty();
    for mut s in single_line {
        adjust_structure_prefix(&mut s, false, &indent);
        body.push(s);
    }
    for (i, mut s) in multi_line.into_iter().enumerate() {
        let want_blank = i > 0 || has_single;
        adjust_structure_prefix(&mut s, want_blank, &indent);
        body.push(s);
    }

    // Restore body-level metadata
    *body.decor_mut() = body_decor;
    body.set_prefer_oneline(prefer_oneline);
    body.set_prefer_omit_trailing_newline(prefer_omit_trailing_newline);
}

/// Recursively format an expression in-place. Sorts object keys and recurses
/// into nested objects within arrays.
fn format_expression(expr: &mut Expression, depth: usize) {
    if let Some(obj) = expr.as_object_mut() {
        // Only format multi-line objects; leave inline objects untouched
        if is_multiline_object(obj) {
            format_object(obj, depth);
        }
    } else if let Some(arr) = expr.as_array_mut() {
        for i in 0..arr.len() {
            if let Some(elem) = arr.get_mut(i) {
                format_expression(elem, depth);
            }
        }
    }
}

/// Vertically align the `=` signs of consecutive single-line attributes in a
/// body by padding the key's decor suffix.
fn align_body_attributes(structures: &mut [Structure]) {
    let max_key_len = structures
        .iter()
        .filter_map(|s| s.as_attribute().map(|a| a.key.as_str().len()))
        .max()
        .unwrap_or(0);

    for s in structures.iter_mut() {
        if let Structure::Attribute(attr) = s {
            let padding = max_key_len - attr.key.as_str().len() + 1;
            attr.key.decor_mut().set_suffix(" ".repeat(padding));
        }
    }
}

/// Vertically align the `=` signs of object key entries by padding the key's
/// decor suffix.
fn align_object_keys(entries: &mut [(ObjectKey, hcl_edit::expr::ObjectValue)]) {
    let max_key_len = entries
        .iter()
        .map(|(k, _)| object_key_str(k).len())
        .max()
        .unwrap_or(0);

    for (key, _) in entries.iter_mut() {
        let padding = max_key_len - object_key_str(key).len() + 1;
        key.decor_mut().set_suffix(" ".repeat(padding));
    }
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
    let obj_trailing = obj.trailing().clone();

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

    // Align `=` signs within each group
    align_object_keys(&mut single);
    align_object_keys(&mut multi);

    // Re-insert in order: single-line first, then multi-line
    let has_single = !single.is_empty();
    let mut is_first = true;

    for (mut key, value) in single {
        let comments = extract_key_comments(&key);
        let prefix = build_object_key_prefix(is_first, false, &comments, &indent);
        key.decor_mut().set_prefix(prefix);
        obj.insert(key, value);
        is_first = false;
    }
    for (i, (mut key, value)) in multi.into_iter().enumerate() {
        let want_blank = i > 0 || has_single;
        let comments = extract_key_comments(&key);
        let prefix = build_object_key_prefix(is_first, want_blank, &comments, &indent);
        key.decor_mut().set_prefix(prefix);
        obj.insert(key, value);
        is_first = false;
    }

    // Restore object-level decor
    *obj.decor_mut() = obj_decor;
    obj.set_trailing(obj_trailing);
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
