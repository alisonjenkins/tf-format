use hcl_edit::structure::{Attribute, Structure};

/// Returns true if the structure is multi-line (a block, or an attribute whose
/// rendered value spans multiple lines).
pub fn is_multiline(structure: &Structure) -> bool {
    match structure {
        Structure::Block(_) => true,
        Structure::Attribute(attr) => is_multiline_attribute(attr),
    }
}

fn is_multiline_attribute(attr: &Attribute) -> bool {
    attr.value.to_string().contains('\n')
}
