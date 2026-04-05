#![deny(clippy::unwrap_used, clippy::expect_used)]

mod classify;
pub mod error;
mod formatter;

use error::FormatError;
use hcl_edit::Decorate;
use hcl_edit::structure::{Body, Structure};

/// Format an HCL string by sorting attributes and blocks according to the
/// tf-format rules.
pub fn format_hcl(input: &str) -> Result<String, FormatError> {
    let mut body: Body = input.parse()?;

    // Sort top-level blocks (variable, resource, data, output by name)
    formatter::sort_top_level(&mut body);

    // Format each top-level block's contents by draining, processing, rebuilding
    let body_decor = body.decor().clone();
    let prefer_oneline = body.prefer_oneline();
    let prefer_omit_trailing_newline = body.prefer_omit_trailing_newline();

    let old_body = std::mem::take(&mut body);
    let mut structures: Vec<Structure> = old_body.into_iter().collect();

    for structure in &mut structures {
        if let Structure::Block(block) = structure {
            formatter::format_body(&mut block.body, 0);
        }
    }

    for s in structures {
        body.push(s);
    }

    *body.decor_mut() = body_decor;
    body.set_prefer_oneline(prefer_oneline);
    body.set_prefer_omit_trailing_newline(prefer_omit_trailing_newline);

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
