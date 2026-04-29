# tf-format

An opinionated Terraform/OpenTofu HCL formatter that goes beyond `terraform fmt` by enforcing consistent attribute ordering, block sorting, and vertical alignment.

## What it does

```hcl
# Before                                    # After
resource "aws_instance" "web" {             resource "aws_instance" "web" {
  ami = "ami-123"                             count = 2
  count = 2
  instance_type = "t2.micro"                  lifecycle {
  lifecycle {                                   create_before_destroy = true
    create_before_destroy = true              }
  }
  tags = {                                    ami           = "ami-123"
    Name = "web"                              instance_type = "t2.micro"
    Environment = "prod"
  }                                           tags = {
}                                               Environment = "prod"
                                                Name        = "web"
variable "zone" { ... }                       }
variable "ami" { ... }                      }

                                            variable "ami" { ... }
                                            variable "zone" { ... }
```

Key improvements over `terraform fmt`:

- **Sorts `variable` and `output` blocks alphabetically** by name — makes large files easy to navigate
- **Sorts `resource` and `data` blocks** by type and name within consecutive groups
- **Hoists meta-arguments** (`count`, `for_each`, `lifecycle`, etc.) to the top of each block
- **Sorts attributes alphabetically** within blocks and objects
- **Aligns `=` signs** vertically for readability

## Why these rules matter

### Meta-arguments at the top

Attributes like `count`, `for_each`, `provider`, `lifecycle`, and `depends_on` can fundamentally change how a resource behaves. A `lifecycle` block with an unexpected `ignore_changes` can silently mask drift. A `for_each` with a subtle condition can create or destroy resources in ways that aren't obvious from the rest of the block. Keeping these at the top of every resource ensures they're the first thing a reviewer sees, not buried between `tags` and `subnet_id`.

### Single-line attributes above multi-line

When single-line attributes are scattered between multi-line blocks and maps, they're easy to miss. You might add a duplicate attribute because you didn't spot it hiding after a 20-line `ingress` block. Grouping all single-line attributes together at the top of a section increases information density — you can see every simple property at a glance, then read the complex nested structures below.

### Alphabetical sorting over "logical grouping"

Some style guides suggest grouping attributes by purpose — network settings together, storage settings together, and so on. In practice this creates endless bikeshedding over which group an attribute belongs to, produces inconsistent results across teams, and makes it impossible to enforce automatically. Does `associate_public_ip_address` belong with "network" or "instance"? Every developer has a different answer. Alphabetical sorting removes the debate entirely: there is exactly one correct place for every attribute, it's trivially verifiable, and anyone reading the code can find what they're looking for with the same strategy they'd use in a dictionary. It also produces minimal diffs when attributes are added or removed, since a new attribute only affects its immediate neighbours rather than reshuffling an entire "group".

### Variable and output sorting

Sorting `variable` and `output` blocks alphabetically makes them easy to find in large files without relying on editor search. It also encourages consistent naming conventions and nudges teams toward using structured object variables for related configuration rather than dozens of loosely-named scalar variables, since the alphabetical grouping makes related variables naturally cluster together.

## Installation

### Nix flake (recommended)

```sh
# Run directly
nix run github:alisonjenkins/tf-format -- --check .

# Add to your flake inputs
inputs.tf-format.url = "github:alisonjenkins/tf-format";
```

### Cargo

```sh
cargo install --git https://github.com/alisonjenkins/tf-format
```

### Dev shell

```sh
nix develop  # includes rust-analyzer, clippy, rustfmt
```

## Usage

```sh
# Format files in-place
tf-format main.tf variables.tf

# Format all .tf/.tofu/.tfvars files in a directory (recursive)
tf-format .

# Check if files are formatted (for CI pipelines, exits 1 if changes needed)
tf-format --check .

# Print diff without writing
tf-format --diff .

# Read from stdin, write to stdout
cat main.tf | tf-format --stdin
```

### GitHub Action

Add tf-format to your CI pipeline with the included GitHub Action:

```yaml
- uses: alisonjenkins/tf-format@v1
```

This downloads the correct binary for the runner's platform, verifies its SHA256 checksum, and runs `tf-format --check .`. The step fails if any files need formatting.

Options:

```yaml
- uses: alisonjenkins/tf-format@v1
  with:
    version: 'v0.1.0'  # pin a specific version (default: latest)
    directory: 'infra/' # directory to check (default: .)
    args: '--diff'      # override arguments (default: --check)
```

### Supported file types

`.tf`, `.tofu`, `.tfvars` — all discovered automatically when scanning directories.

## Formatting Rules

### 1. Top-level block sorting

Consecutive blocks of the same type are sorted alphabetically by their labels:

- `variable` blocks sorted by variable name
- `output` blocks sorted by output name
- `resource` blocks sorted by type + name (e.g., `aws_instance.web`)
- `data` blocks sorted by type + name

Other block types (`locals`, `provider`, `terraform`, `module`) are left in their original order.

### 2. Attribute ordering within blocks

Inside each `resource`, `data`, `module`, or nested block, attributes are organized into four tiers:

```
1. Priority single-line    count, for_each, source, version, provider, depends_on
2. Priority blocks         lifecycle
   ── blank line ──
3. Normal single-line      all other attributes, sorted alphabetically
4. Normal multi-line       multi-line attributes and nested blocks, sorted alphabetically
```

Priority items appear in a fixed order (not alphabetical). Normal items are sorted alphabetically. Blank lines separate the tiers and between multi-line items.

### 3. Vertical `=` alignment

Within each group of consecutive single-line attributes, `=` signs are padded to align vertically:

```hcl
ami           = "ami-12345678"
instance_type = "t2.micro"
subnet_id     = "subnet-abc123"
```

Priority and normal attribute groups are aligned independently.

### 4. Object and map sorting

Multi-line objects have their keys sorted alphabetically with the same single-line-first, multi-line-after rules. Inline objects (single-line) are left untouched:

```hcl
tags = {
  CostCenter  = "12345"
  Environment = "dev"
  Name        = "example"
}
```

### 5. List order is never changed

Array/list element order is always preserved. Attributes and object keys are unordered in HCL and safe to sort, but list elements are ordered and their position can be semantically meaningful. Reordering a list used in a `for_each` would cause Terraform to destroy and recreate resources, which could result in downtime or data loss. tf-format will never do this.

### 6. Array formatting

Objects inside arrays are recursively formatted with correct indentation. Multi-line arrays always get a trailing comma so that adding a new entry only changes one line in the diff, making peer review easier:

```hcl
overrides = [
  {
    instance_type = "c7g.xlarge"
  },
  {
    instance_type = "m7g.xlarge"
  },
]
```

### 7. Comment preservation

Comments are never stripped. They travel with their associated attribute or block through sorting:

```hcl
# The AMI to launch
ami           = "ami-12345678"
# The instance type to use
instance_type = "t2.micro"
```

### 8. Whitespace normalization

- 2-space indentation at every nesting level
- Trailing whitespace stripped from all lines
- File always ends with exactly one newline

### 9. Author blank lines

In **opinionated** style, author-placed blank lines inside a
block / object are NOT preserved as alignment-group boundaries.
Every single-line attribute collapses into the priority/normal
single tier (sorted alphabetically) and every multi-line
attribute or nested block falls into the multi tier (also
sorted) — regardless of where the author put blank lines.
tf-format's tier layout still inserts a single blank line
between priority and normal tiers, and between single-line and
multi-line tiers, so the output stays readable.

In **minimal** style (the `terraform fmt` / `tofu fmt` parity
mode), author blank lines DO act as alignment-group boundaries:
each blank-line-separated group sorts/aligns independently and
source order is preserved. Use minimal when you want to keep
intentional visual grouping the author wrote.

## Styles

tf-format ships two styles:

- `opinionated` (default) — every transform documented in
  "Formatting Rules" above: alphabetisation, hoisting, alignment,
  expansion. Ideal for greenfield repos and teams who want one
  canonical style.
- `minimal` — alignment + spacing only, mirroring
  `terraform fmt` / `tofu fmt`. No reordering, no hoisting, no
  trailing-comma insertion, no single-line-object expansion. Use
  this when you want the spacing benefits of an in-process
  formatter (e.g. inside a language server) without imposing
  tf-format's opinions on a codebase that hasn't adopted them.

CLI:

```sh
tf-format --style opinionated path/to/dir   # default
tf-format --style minimal     path/to/dir   # terraform fmt parity
```

## Library usage

```rust
use tf_format::{FormatOptions, format_hcl, format_hcl_with};

let input = r#"
resource "aws_instance" "web" {
  instance_type = "t2.micro"
  ami = "ami-123"
}
"#;

// Default opinionated style:
let formatted = format_hcl(input).expect("valid HCL");

// Minimal (terraform fmt) style:
let minimal = format_hcl_with(input, &FormatOptions::minimal()).expect("valid HCL");
```

## Design

- **Fast** — tf-format is a single native binary with no runtime dependencies. It parses HCL into an AST via [hcl-edit](https://crates.io/crates/hcl-edit) and manipulates it in-memory rather than doing string processing, so formatting even large Terraform codebases completes in milliseconds. There is no JVM startup, no interpreter overhead, and no shelling out to `terraform fmt` — just parse, sort, emit.
- **Reliable** — every formatting rule is covered by fixture tests that verify both correctness and idempotency (formatting twice always produces identical output). The formatter never modifies semantics: list order is preserved, comments travel with their attributes, and inline expressions are left untouched. Typed error handling with full context means failures report exactly what went wrong and where.
- **Deterministic** — same input always produces same output, regardless of the original formatting
- **Two styles, opt-in** — `opinionated` (full rule set) or `minimal` (alignment-only). Each is a single fixed style with no further knobs.
