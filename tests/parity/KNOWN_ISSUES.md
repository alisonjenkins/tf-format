# tf-format minimal ⟷ `tofu fmt` parity — status

Found by `scripts/parity-check.sh` across 8 public TF/OpenTofu module repos
(terraform-aws-modules eks/vpc/rds/lambda/iam, cloudposse eks, google GKE,
gruntwork). Each case below is already `tofu fmt`-canonical, so a correct
`--style minimal` is a no-op; cases that still diverge are listed under
**Upstream (hcl-edit)** — they are not fixable in tf-format alone.

## Fixed

Regression fixtures live in `tests/parity/fixtures/` and are gated by
`.github/workflows/parity.yml`.

### P0 — leading comments + blank line deleted (DATA LOSS) — FIXED
`--style minimal` dropped a file's leading (pre-first-block) comments and the
blank line after them (also affected the terraform-ls-rs LSP formatter — header
comments vanished on format). The top-level flattener's first-structure branch
unconditionally cleared the prefix where those comments live; it now preserves
them (and re-emits one separating blank line). Fixture: `leading_comments.tf`.

### inline comment after object-open brace — FIXED
`key = { # comment` had the comment lifted to its own line. `tofu fmt` keeps a
comment written on the same line as `{` inline; tf-format now does too, while
still moving comments that were on their own lines. Fixture:
`object_inline_comment.tf`.

### P1 — nested object under-indented by one level — FIXED (earlier)
`{ for k, v in … : k => { … } if … }` inner body + closer, and object literals
passed to `merge(...)` / function args, were emitted 2 spaces too shallow. Now
correct. Fixture: `for_expr_nested.tf`.

### P2 — `[{…}]` closer indentation — FIXED (earlier)
Regression-guarded by `bracket_object.tf`.

## Upstream (hcl-edit) — not fixable in tf-format

These survive a *raw* hcl-edit parse→encode round-trip with no tf-format logic
involved, so the divergence is in the library's decor model, not our formatter.

### P2 — value-grouping ellipsis loses the space before `}` (`k... }` → `k...}`)
In an object for-expression with grouping (`{ for k in v : k => v... }`), the
`ForExpr` encoder writes `value_expr`, then `...`, then `}` with **no decor slot
between `...` and `}`**. hcl-edit parses the original space into the for-expr's
*trailing* decor (after `}`), which the formatter then trims. No AST slot can
carry the space; a string post-process would risk corrupting `"...}"` string
literals, so this is left to upstream.

### P2 — multi-line `&&` / boolean chain collapsed onto one line
`BinaryOp` operands are encoded with `BOTH_SPACE_DECOR` defaults and hcl-edit's
parser discards the inter-operand newlines, so a parenthesised `a &&\n b &&\n c`
round-trips to `a && b && c` before tf-format runs. The line breaks are never
represented in the AST and cannot be recovered.

### P2 — inline single-line nested block spacing (`allow {  protocol }`)
Only reproduced on one real-world google-GKE shape; synthetic single-attr inline
blocks round-trip cleanly. Tracking as a suspected hcl-edit interior-spacing
case pending a minimal repro.
