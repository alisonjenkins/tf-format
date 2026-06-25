#![deny(clippy::unwrap_used, clippy::expect_used)]

//! Parity tests: ensure tf-format's `=` alignment matches `tofu fmt`.
//!
//! Each input below is already canonical for tf-format in every respect *other
//! than* `=` alignment (attribute order, single-line vs multi-line grouping,
//! indentation, etc.). That isolates alignment as the only thing both
//! formatters will change, so their outputs must agree.
//!
//! If `tofu` is not on PATH, the test prints a warning and passes — CI
//! runs through the nix devshell which provides it.

use std::io::Write;
use std::process::{Command, Stdio};

fn tofu_fmt(input: &str) -> Result<String, String> {
    let mut child = Command::new("tofu")
        .args(["fmt", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn tofu: {e}"))?;

    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| "no stdin on tofu child".to_string())?;
        stdin
            .write_all(input.as_bytes())
            .map_err(|e| format!("write stdin: {e}"))?;
    }

    let out = child
        .wait_with_output()
        .map_err(|e| format!("wait tofu: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "tofu fmt failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    String::from_utf8(out.stdout).map_err(|e| format!("utf8: {e}"))
}

fn tofu_available() -> bool {
    Command::new("tofu")
        .arg("version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// When `TF_FORMAT_REQUIRE_TOFU` is set (e.g. in CI), a missing `tofu`
/// binary turns the parity SKIP into a hard failure, so the single source of
/// truth for real-formatter parity can't silently become a green no-op.
fn require_tofu() -> bool {
    std::env::var_os("TF_FORMAT_REQUIRE_TOFU").is_some()
}

fn check_parity(name: &str, input: &str) {
    if !tofu_available() {
        assert!(
            !require_tofu(),
            "{name}: tofu is not on PATH but TF_FORMAT_REQUIRE_TOFU is set — \
             parity cannot be silently skipped"
        );
        eprintln!("SKIP {name}: tofu not on PATH");
        return;
    }

    let ours = match tf_format::format_hcl(input) {
        Ok(s) => s,
        Err(e) => panic!("{name}: format_hcl failed: {e}"),
    };
    let theirs = match tofu_fmt(input) {
        Ok(s) => s,
        Err(e) => panic!("{name}: tofu fmt failed: {e}"),
    };

    pretty_assertions::assert_eq!(
        ours,
        theirs,
        "{name}: tf-format output differs from `tofu fmt`"
    );

    // Idempotency: re-formatting our output should be a no-op.
    let twice = match tf_format::format_hcl(&ours) {
        Ok(s) => s,
        Err(e) => panic!("{name}: second format_hcl failed: {e}"),
    };
    pretty_assertions::assert_eq!(twice, ours, "{name}: tf-format is not idempotent");
}

/// Minimal-mode variant of `check_parity` for inputs whose
/// expected tofu output relies on tf-format NOT applying its
/// opinionated transforms (e.g. inputs that contain `:` object
/// separators, which the opinionated path rewrites to `=`).
fn check_parity_minimal(name: &str, input: &str) {
    if !tofu_available() {
        assert!(
            !require_tofu(),
            "{name}: tofu is not on PATH but TF_FORMAT_REQUIRE_TOFU is set — \
             parity cannot be silently skipped"
        );
        eprintln!("SKIP {name}: tofu not on PATH");
        return;
    }

    let opts = tf_format::FormatOptions::minimal();
    let ours = match tf_format::format_hcl_with(input, &opts) {
        Ok(s) => s,
        Err(e) => panic!("{name}: format_hcl_with failed: {e}"),
    };
    let theirs = match tofu_fmt(input) {
        Ok(s) => s,
        Err(e) => panic!("{name}: tofu fmt failed: {e}"),
    };
    pretty_assertions::assert_eq!(
        ours,
        theirs,
        "{name}: tf-format minimal output differs from `tofu fmt`"
    );
    let twice = match tf_format::format_hcl_with(&ours, &opts) {
        Ok(s) => s,
        Err(e) => panic!("{name}: second format_hcl_with failed: {e}"),
    };
    pretty_assertions::assert_eq!(twice, ours, "{name}: tf-format minimal is not idempotent");
}

#[test]
fn parity_body_varying_key_lengths() {
    let input = r#"resource "aws_instance" "example" {
  ami           =      "ami-12345678"
  instance_type=    "t2.micro"
  subnet_id  =  "subnet-abc123"
}
"#;
    check_parity("body_varying_key_lengths", input);
}

#[test]
fn parity_body_equal_key_lengths() {
    let input = r#"resource "aws_instance" "example" {
  bar    = "a"
  baz =       "b"
  foo   =  "c"
}
"#;
    check_parity("body_equal_key_lengths", input);
}

#[test]
fn parity_object_keys() {
    let input = r#"resource "aws_instance" "example" {
  tags = {
    CostCenter  =    "12345"
    Environment=  "dev"
    Name      =       "example"
  }
}
"#;
    check_parity("object_keys", input);
}

#[test]
fn parity_single_attribute() {
    let input = r#"resource "aws_instance" "example" {
  ami       =     "ami-12345678"
}
"#;
    check_parity("single_attribute", input);
}

// Note on grouping: tf-format sorts single-line attributes alphabetically and
// removes any user-inserted blank lines between them, but a comment attached
// to an attribute is preserved (and travels with that attribute through the
// sort). For `=` alignment to agree with `tofu fmt`, the alignment must break
// at any comment line, since `tofu fmt` treats comments as alignment-group
// boundaries.

#[test]
fn parity_comment_breaks_alignment_group() {
    // Already alphabetically sorted; the comment attached to `instance_type`
    // must break the alignment group so `ami` aligns alone and
    // `instance_type`/`subnet_id` align as their own group — same as
    // `tofu fmt`.
    let input = r#"resource "aws_instance" "example" {
  ami =  "ami-12345678"
  # network config below
  instance_type =     "t2.micro"
  subnet_id    =   "subnet-abc123"
}
"#;
    check_parity("comment_breaks_alignment_group", input);
}

#[test]
fn parity_array_of_inline_object_with_nested_objects() {
    // Regression: `rules = [{ ... }]` (object's `{` on the array's `[` line)
    // used to over-indent every line inside the object by 2sp, because the
    // array branch unconditionally added a depth level for elements. The
    // inline form should behave as if `rules = { ... }` — same depth as the
    // array itself.
    let input = r#"resource "cloudflare_ruleset" "x" {
  rules = [{
    action      = "rewrite"
    description = "Prepend /file/x to URI"
    enabled     = true
    expression  = "(http.host eq \"example.com\")"

    action_parameters = {
      uri = {
        path = {
          expression = "concat(\"/file/x\", http.request.uri.path)"
        }
      }
    }
  }]
}
"#;
    check_parity("array_of_inline_object_with_nested_objects", input);
}

#[test]
fn parity_array_of_objects_multiline_form() {
    // Smoke test: the canonical multi-line array-of-objects form should
    // still receive an extra depth level (each element on its own line).
    let input = r#"resource "x" "y" {
  rules = [
    {
      bar = "b"
      foo = "a"
    },
    {
      bar = "d"
      foo = "c"
    },
  ]
}
"#;
    check_parity("array_of_objects_multiline_form", input);
}

#[test]
fn parity_object_of_objects_with_trailing_commas() {
    // Regression: when each entry of an object is itself a multi-line
    // object terminated with a comma, the closing `}` of the outer object
    // used to be glued onto the same line as the last inner `}` (e.g.
    // `},  }`) because the trailing decoration was just whitespace, with
    // no newline. The terminator-aware fix now prepends `\n` when the last
    // entry's terminator isn't already a newline.
    //
    // Checked under MINIMAL style: opinionated mode intentionally strips the
    // trailing commas from multi-line-value objects and blank-line-separates
    // the entries (issue #54), so it no longer matches `tofu fmt` here. Minimal
    // preserves the comma style and must still match — guarding the
    // closing-brace placement in the comma-keeping mode. The opinionated
    // reformatting is covered by `fixture_map_trailing_commas_normalized`.
    let input = r#"locals {
  github_actions_roles = {
    aaa = {
      bar = "2"
      foo = "1"
    },
    bbb = {
      bar = "4"
      foo = "3"
    },
    ccc = {
      bar = "6"
      foo = "5"
    },
  }
}
"#;
    check_parity_minimal("object_of_objects_with_trailing_commas", input);
}

#[test]
fn parity_object_multiline_values_not_aligned() {
    // Regression: multi-line object entries used to be aligned together,
    // padding their keys to the longest key in the group. `tofu fmt` does
    // not do this — each multi-line entry just gets a single space on
    // either side of `=`. Keys here are intentionally varying length.
    let input = r#"locals {
  lambdas = {
    lambda-hello-world = {
      lambda = true
    }

    lambda-manage-dns = {
      lambda = true
    }

    lambda-redwood-guild-servers = {
      lambda = true
    }

    portal-alison-jenkins-com-api = {
      lambda = true
    }
  }
}
"#;
    check_parity("object_multiline_values_not_aligned", input);
}

#[test]
fn parity_object_quoted_string_keys() {
    // Regression: `ObjectKey::Expression` (quoted-string keys) used to be
    // measured *with* their decor whitespace included, which made alignment
    // padding completely wrong for any object that used quoted keys.
    let input = r#"variable "regions" {
  default = {
    "eu-west-1" = {
      "cidr_block"  =   "10.0.0.0/16"
      "enabled" =     true
    }
  }
}
"#;
    check_parity("object_quoted_string_keys", input);
}

#[test]
fn parity_object_comment_breaks_alignment_group() {
    let input = r#"resource "aws_instance" "example" {
  tags = {
    AAA  =   "1"
    # divider
    BBBBB  =     "2"
    CCC =   "3"
  }
}
"#;
    check_parity("object_comment_breaks_alignment_group", input);
}

#[test]
fn parity_nested_block_sibling_groups() {
    let input = r#"resource "aws_instance" "example" {
  ami           =   "ami-12345678"
  instance_type =  "t2.micro"

  root_block_device {
    volume_size =   8
    volume_type =     "gp3"
  }
}
"#;
    check_parity("nested_block_sibling_groups", input);
}

#[test]
fn parity_multiline_value_between_singles() {
    // Single-line attrs are alphabetically sorted and precede the multi-line
    // `tags` block, which is tf-format's canonical order. tofu fmt won't
    // re-order, so the only difference between input and either formatter's
    // output is `=` alignment plus the blank line tf-format inserts before
    // the multi-line block.
    let input = r#"resource "aws_instance" "example" {
  ami           =   "ami-12345678"
  instance_type =  "t2.micro"
  key_name      =      "mykey"
  subnet_id     =   "subnet-abc123"

  tags = {
    Environment =  "dev"
    Name        =       "example"
  }
}
"#;
    check_parity("multiline_value_between_singles", input);
}

#[test]
fn parity_object_colon_separator() {
    // Issue #18 regression: `tofu fmt` does NOT column-align
    // object entries that use `:` as the assignment operator.
    // tf-format minimal must match exactly.
    let input = r#"locals {
  m = {
    "attribute.inline_very_long" : "assertion.inline_very_long_name"
    (local.names["short"]) : "assertion.short"
    (local.names["very_long"]) : "assertion.very_long"
  }
}
"#;
    check_parity_minimal("object_colon_separator", input);
}

#[test]
fn parity_object_mixed_colon_and_equals() {
    // Issue #18 follow-up: `:` is a hard break for `=`
    // alignment runs. Two `=` runs separated by a `:` align
    // INDEPENDENTLY. tf-format minimal must mirror tofu fmt.
    let input = r#"locals {
  m = {
    "a" = "1"
    "long_key" = "2"
    "x" : "3"
    "another_eq" = "4"
    "z" = "5"
  }
}
"#;
    check_parity_minimal("object_mixed_colon_and_equals", input);
}

#[test]
fn parity_body_trailing_comment_alignment() {
    // `tofu fmt` column-aligns trailing inline comments across a run of
    // consecutive comment-bearing attributes. A comment-less line breaks the
    // comment run (but not the `=` run), and a blank line breaks both.
    let input = r#"locals {
  a = 1 # one
  bb = 22 # two
  ccc = 333
  dddd = 4 # four
  e = 5
}
"#;
    check_parity("body_trailing_comment_alignment", input);
}

#[test]
fn parity_object_trailing_comment_alignment() {
    // Trailing inline comments inside an object literal align by the value's
    // end column across each consecutive comment-bearing run.
    // Minimal mode: opinionated mode would re-sort the keys, diverging from
    // tofu's source order, so the object path is exercised in minimal mode.
    let input = r#"x = {
  short = "x" # s
  longername = "yy" # l
  plain = 1
}
"#;
    check_parity_minimal("object_trailing_comment_alignment", input);
}

#[test]
fn parity_object_colon_trailing_comment_alignment() {
    // For `:` runs the keys are NOT padded, so the comment column folds in the
    // per-key length — comment alignment must measure the value's end column,
    // not just the value width. tf-format minimal must mirror tofu fmt.
    let input = r#"x = {
  k : "v" # colon1
  k2 : "vv" # colon2
}
"#;
    check_parity_minimal("object_colon_trailing_comment_alignment", input);
}

#[test]
fn parity_hash_inside_string_not_a_comment() {
    // A `#` inside a string literal is part of the value, not a trailing
    // comment — it must not perturb alignment.
    let input = r#"x = {
  a = 1
  bb = 2 # c
  ccc = "https://example.com#frag" # url with hash
}
"#;
    check_parity("hash_inside_string_not_a_comment", input);
}

#[test]
fn parity_redundant_interpolation_unwrap_attribute_only() {
    // `tofu fmt` unwraps a whole-string interpolation (`"${foo}"` → `foo`,
    // `"${1 + 2}"` → `1 + 2`) but ONLY at attribute-value position. It is kept
    // inside objects, lists, function arguments and conditional branches, and
    // partial templates (`"pre${foo}"`) are always left alone.
    let input = r#"locals {
  unwrap_var  = "${foo}"
  unwrap_expr = "${1 + 2}"
  keep_obj    = { k = "${foo}" }
  keep_list   = ["${foo}"]
  keep_func   = upper("${foo}")
  keep_cond   = c ? "${foo}" : "${bar}"
  keep_partial = "pre${foo}"
}
"#;
    check_parity_minimal("redundant_interpolation_unwrap_attribute_only", input);
}

#[test]
fn parity_single_line_array_trailing_comma_space() {
    // `tofu fmt` renders a single-line array with a trailing comma as `…, ]`
    // (one space before the bracket), like the object `…, }` form.
    let input = r#"locals {
  a = [1, 2, 3,]
  b = [1,]
  c = [[1, 2,], [3, 4,]]
}
"#;
    check_parity_minimal("single_line_array_trailing_comma_space", input);
}

#[test]
fn parity_array_element_trailing_comment_alignment() {
    // `tofu fmt` column-aligns trailing comments on multi-line array elements
    // across runs of consecutive comment-bearing elements. An array element's
    // comment lives in the *next* element's prefix (the comma is rendered
    // between), and the last element's in the array trailing decor.
    let input = r#"locals {
  l = [
    "a", # first
    "bb", # second
    "ccc",
    1, # x
    22,
    333, # y
  ]
}
"#;
    check_parity_minimal("array_element_trailing_comment_alignment", input);
}

/// Sweep every minimal-mode fixture's `input.tf` through real `tofu fmt`
/// so the minimal style is validated against the actual formatter, not only
/// against hand-written `expected.tf` files. Catches any minimal-mode
/// divergence the static fixtures might mask.
#[test]
fn parity_all_minimal_fixtures() {
    let dir = std::path::Path::new("tests/fixtures-minimal");
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => panic!("failed to read {}: {e}", dir.display()),
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => panic!("failed to read dir entry: {e}"),
        };
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        // Test-file fixtures carry a `.tftest.hcl` suffix; everything else uses
        // the default `.tf`. tofu fmt reads stdin as generic HCL either way.
        let input_path = {
            let tftest = path.join("input.tftest.hcl");
            if tftest.exists() {
                tftest
            } else {
                path.join("input.tf")
            }
        };
        let input = match std::fs::read_to_string(&input_path) {
            Ok(s) => s,
            Err(e) => panic!("failed to read {}: {e}", input_path.display()),
        };
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("<unknown>")
            .to_string();
        check_parity_minimal(&name, &input);
    }
}
