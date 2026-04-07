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

fn check_parity(name: &str, input: &str) {
    if !tofu_available() {
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
