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
    let mut body: Body = input.parse()?;

    // sort_top_level handles both block ordering and top-level attribute
    // formatting (as in `.tfvars` files), recursing into nested bodies.
    formatter::sort_top_level(&mut body, opts.style);

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
