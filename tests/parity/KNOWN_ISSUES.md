# tf-format minimal ⟷ `tofu fmt` parity — known divergences

Found by `scripts/parity-check.sh` across 8 public TF/OpenTofu module repos
(terraform-aws-modules eks/vpc/rds/lambda/iam, cloudposse eks, google GKE,
gruntwork). Each case below is already `tofu fmt`-canonical, so a correct
`--style minimal` is a no-op; tf-format currently changes it.

## P0 — leading comments + blank lines deleted (DATA LOSS)
`--style minimal` drops a file's leading (pre-first-block) comments and the
blank line after them. Also affects the terraform-ls-rs LSP formatter (header
comments vanish on format).
```hcl
# banner comment
# second line

locals {
  a = 1
}
```
tofu fmt: unchanged. tf-format minimal: deletes the two `#` lines + the blank.

## P1 — nested object under-indented by one level (2 spaces)
Inside a `{ for k, v in { ... } : k => v if ... }` for-expression, the inner
object body + the `} : k => v if …` closer are emitted 2 spaces too shallow.
Repos: terraform-aws-eks, cloudposse eks, terraform-aws-iam.

## P1 — object literal as `merge()` / function arg under-indented by 2 spaces
The whole nested object passed into `merge(...)` is dedented one level.
Repos: cloudposse eks, terraform-aws-eks.

## P2 — multi-line `&&` boolean chain collapsed onto one line
tofu fmt preserves author line breaks in a parenthesised `&&` chain; tf-format
joins them. Repo: cloudposse eks.

## P2 — inline single-line nested block spacing: `allow {    protocol }` squashed
Repo: google GKE.

## P2 — missing space before `}` when value ends in `...` ellipsis (`id...}` vs `id... }`)
Repo: gruntwork.

## P2 — inline comment after object-open brace not pushed to its own line
`key = { # comment` → tofu moves the comment to the next line; tf-format keeps it inline.
Repos: terraform-aws-lambda, terraform-aws-eks.

## P2 — `[{…}]` / `}] : [{` / `] }` closer indentation + inner alignment-space squashing
Repos: google GKE, terraform-aws-lambda.
