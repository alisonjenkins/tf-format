#![deny(clippy::unwrap_used, clippy::expect_used)]

mod classify;
pub mod error;
mod formatter;

use error::FormatError;
use hcl_edit::structure::Body;

/// Format an HCL string by sorting attributes and blocks according to the
/// tf-format rules.
pub fn format_hcl(input: &str) -> Result<String, FormatError> {
    let mut body: Body = input.parse()?;

    // sort_top_level handles both block ordering and top-level attribute
    // formatting (as in `.tfvars` files), recursing into nested bodies.
    formatter::sort_top_level(&mut body);

    Ok(post_process(&body.to_string()))
}

/// Post-process the formatted output: strip trailing whitespace from each line
/// and ensure the file ends with exactly one newline.
fn post_process(output: &str) -> String {
    let mut result: String = output
        .lines()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n");
    if !result.ends_with('\n') {
        result.push('\n');
    }
    result
}
