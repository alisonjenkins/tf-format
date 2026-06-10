# tf-format — Audit Backlog

Generated from a docs-grounded code audit: documented formatting rules and invariants
(from `README.md`, `CHANGELOG.md`, `.pre-commit-hooks.yaml`) were extracted, then each
source area was deep-reviewed and every finding adversarially verified against the code.
38 findings confirmed → 24 deduped, prioritized items below.

**Priority:** P0 = silent data loss / correctness corruption · P1 = robustness/correctness
on real runs · P2 = CLI papercuts & coverage · P3 = cosmetic / nice-to-have.
**Effort:** S / M / L.

The dominant theme: the formatter has **no binary-level, edge-case, or block-comment test
coverage** — which is exactly where the two P0 data-loss bugs hid.

---

## Resolution status (all items addressed)

| Item | Status | Commit |
|------|--------|--------|
| BUG-1 block comments dropped | ✅ fixed | `9bdc468` |
| BUG-2 heredoc whitespace stripped | ✅ fixed | `9bdc468` |
| BUG-3 symlink re-traversal | ✅ fixed | `693626c` |
| BUG-4 batch abort | ✅ fixed | `d5a3506` |
| BUG-5 object comma terminators | ✅ fixed | `32f3b20` |
| BUG-6 `.terraform` recursion | ✅ fixed | `693626c` |
| BUG-7 non-atomic write | ✅ fixed | `fd12295` |
| BUG-8 stdin ignores check/diff | ✅ fixed | `158d442` |
| BUG-9 lossy `--diff` | ✅ fixed | `158d442` |
| BUG-10 nonexistent path false-pass | ✅ fixed | `693626c` |
| BUG-11 BOM parse failure | ✅ fixed | `158d442` |
| BUG-12 empty/whitespace idempotence | ✅ fixed | `97c8f9d` |
| BUG-13 block-comment indentation | ✅ fixed (with BUG-1) | `9bdc468` |
| IMP-1 path dedup/canonicalize | ✅ done | `693626c` |
| IMP-2 expansion width prefix | ✅ done | `397d33d` |
| IMP-3 multiline re-render perf | ⏸️ deferred — structural rewrite risks correctness for marginal gain | — |
| TEST-1..8 coverage gaps | ✅ done | `9bdc468`, `b624d5d` |

Plus one bug surfaced *during* the work and fixed: **post_process kept trailing blank
lines** (violated Rule #8, broke idempotence) — fixed in `b624d5d`.

### New findings raised during the sweep (not yet actioned)

- ~~**Minimal mode breaks `=` alignment at a heredoc opener.**~~ ✅ fixed. A heredoc
  attribute's opener (`x = <<EOT`) now stays in the surrounding `=` alignment group in
  minimal mode (body and object contexts), matching `tofu fmt`; multi-line object/array
  values still break the group, also matching tofu. Verified against real `tofu fmt`. The
  `heredoc_preserved` minimal fixture was restored to its richer form and a dedicated
  `heredoc_alignment` fixture added.

- **CRLF line endings are normalized to LF; `tofu fmt` preserves them verbatim.**
  Verified against real `tofu fmt` (2026-06-11): a CRLF file round-trips with CRLF intact
  under tofu, while tf-format (both styles) emits LF. Full preservation would need
  line-ending tracking through `post_process` and every decor rebuild; pinned by
  `crlf_is_normalized_to_lf_and_idempotent` as deliberate behaviour for now. Minor
  minimal-mode parity gap.

- ~~**Multibyte-key `=` alignment is measured in bytes, not display columns.**~~ ✅ fixed.
  `tofu fmt` turns out to pad by RUNE COUNT (`é` = 1, `日本` = 2 — neither bytes nor
  display width; verified empirically 2026-06-11). All alignment/width sites now measure
  `chars().count()`. Pinned byte-for-byte against real tofu output by the
  `multibyte_key_alignment` minimal fixture.
- **Pre-existing clippy `unnecessary_sort_by` warnings** at `formatter.rs:819-820`
  (object key sort) — harmless, could switch to `sort_by_key`.

---

## P0 — data loss / corruption

### BUG-1 — Multi-line `/* */` block comments mangled or dropped during reorder → unparseable output
`src/formatter.rs:120-133` (extract_comments), `138-173`, `889-897`, `1007-1013` · effort: M
Rule #7 (comments never stripped), idempotence, minimal=tofu-fmt parity

`extract_comments()` splits the decor prefix line-by-line and keeps only lines whose trimmed
start matches `#`, `//`, `/*`, `*`, `*/`. For a multi-line C-style comment, interior/closing
lines that don't start with a marker (e.g. `comment */`) are **dropped**, leaving an
unterminated `/* line one`. When the comment precedes a reordered attribute, object key, or
sits between sorted top-level blocks, output loses comment text and a second pass fails to
parse (`invalid block body; expected }`). Reproduced in **both** opinionated and minimal.
Single-line `/* */` survive.
→ Replace the line-by-line heuristic with span-based extraction (capture each comment span
verbatim, `/*` through matching `*/` across lines; only adjust the first line's indent).
Safety net: if a kept set has `/*` with no matching `*/`, preserve the raw prefix verbatim.

### BUG-2 — `post_process` strips trailing whitespace inside heredoc bodies (silent content corruption)
`src/lib.rs:74-84` (post_process), called from `:69` · effort: M
Semantics-never-modified invariant; Rule #8 (layout only, not string contents)

post_process `trim_end()`s **every** line of the rendered document, including literal lines
inside `<<EOT` / `<<-EOT` bodies. hcl-edit round-trips them losslessly, so corruption is
introduced solely here. Heredoc content is literal string data → silently changes Terraform
semantics. No heredoc fixtures exist, so it's untested.
→ Track heredoc spans and skip trimming inside them, or apply trimming only to
formatter-produced layout decor. *(Confirmed by hand: `lib.rs:74-79`.)*

---

## P1 — robustness / correctness on real runs

### BUG-3 — Symlink cycle in recursive discovery → redundant re-traversal of the same file
`src/main.rs:204-231` (collect_tf_files_recursive) · effort: S
`collect_tf_files_recursive` uses `path.is_dir()` (follows symlinks) with no cycle detection.
A symlink to an ancestor re-discovers the same real `.tf` under deepening paths. Self-terminates
near OS PATH_MAX (~40 levels) — not an infinite loop — but ~40× redundant traversal/reads.
→ Use `symlink_metadata`/`entry.file_type()` to not descend symlinked dirs, and/or track
visited canonical paths. Document a symlink policy (skip by default). Pairs with IMP-1.

### BUG-4 — One erroring file aborts the whole batch → codebase left half-formatted
`src/main.rs:104-109` (run loop, `process_file(...)?`) · effort: M
First parse/read/write error propagates out of `run()` and terminates the invocation. Files
before the failure are already written in-place; files after are never touched. Under `--check`
it reports only the first broken file. Re-runnable (idempotent) but undermines the large-codebase goal.
→ Collect per-file results, print every error to stderr, continue. Exit non-zero if any errored
(distinct from CheckFailed) but always attempt every file. Lets `--check` enumerate all in one pass.

### BUG-5 — Sorting a multi-line object with comma terminators → inconsistent comma placement
`src/formatter.rs:704-823` (format_object; terminator read at 794/804/818, never set) · effort: S
format_object sorts and re-inserts entries but never normalizes each `ObjectValue` terminator.
A no-comma final entry sorted into the middle yields `alpha = 2,` / `mid = 3` / `zeta = 1,`.
Valid HCL but non-canonical and diverges from tofu fmt.
→ After sorting, normalize terminators via `value.set_terminator(...)` (mirror array
trailing-comma logic near `:469`).

### BUG-6 — Recursion descends into `.terraform` and other hidden/vendored dirs
`src/main.rs:204-231` · effort: S
Walks every subdir unconditionally incl. dot-dirs. Reformats vendored `.tf` under
`.terraform/modules/`; `terraform fmt` skips `.terraform`. Causes spurious `--check` failures
in CI and (with BUG-1) risks corrupting vendored files.
→ Skip dir entries starting with `.` (at least `.terraform`). Optionally honour `.gitignore`.

### TEST-1 — Add multi-line block-comment fixtures (the gap that hid BUG-1/BUG-13)
`tests/fixtures/*`, `tests/fixtures-minimal/*` (none contain `/*`) · effort: S
Zero fixtures contain a `/* */` block comment; all comment fixtures use single-line `#`.
→ Add opinionated+minimal fixtures: multi-line `/* */` before a reordered attr, before an
object key, between two sortable top-level blocks; an inline `/* */`; a star-aligned comment.
Assert byte-correct output and idempotency. Land with BUG-1.

### TEST-2 — Add CLI/binary integration tests (exit codes, --check/--diff/--stdin, discovery, in-place write)
`tests/*` (all library-level; none spawn the binary) · effort: M
Every `main.rs` invariant is untested: no-op detection, `--check` exit codes, `--diff`,
`--stdin`, glob expansion, dir recursion of `.tf/.tofu/.tfvars`, nonexistent-path, partial
failure, in-place write. This is why BUG-4/8/9/10 went undetected.
→ Add an `assert_cmd` + `tempfile` suite covering the above.

### TEST-3 — Add edge-case fixtures: heredocs, empty/whitespace-only, BOM, CRLF, unicode keys
`tests/fixtures/*` (none matching) · effort: M
Zero coverage for heredoc bodies, empty/whitespace-only files, BOM, CRLF, non-ASCII
identifiers. Hid BUG-2, BUG-11, BUG-12. CRLF is also silently normalized to LF with no fixture.
→ Add fixtures with idempotency assertions for each. Land with BUG-2/11/12.

---

## P2 — CLI papercuts & coverage

### BUG-7 — Non-atomic in-place write risks source corruption on interrupt
`src/main.rs:153` (`std::fs::write`) · effort: S
Truncate-then-write; SIGKILL/OOM/panic/power-loss mid-write leaves a truncated/empty `.tf`
with no backup.
→ Write to a temp file in the same dir, flush (optionally `sync_all`), then `rename` over the
original. Preserve permissions/mode.

### BUG-8 — `--stdin` silently ignores `--check`/`--diff`: formats to stdout, always exits 0
`src/main.rs:80-93` (stdin branch returns Ok before check/diff) · effort: S
`echo 'a=1' | tf-format --stdin --check` prints formatted text and exits 0 → false OK for
editor/CI integrations. clap accepts the contradictory combo silently.
→ Either `conflicts_with_all = ["check","diff"]`, or honour them (stdin+check compares
input==output, exits non-zero without printing the body; stdin+diff prints the diff).

### BUG-9 — `--diff` is lossy when line counts differ (drops added/removed lines)
`src/main.rs:161-173` (print_diff zips line iterators) · effort: S
`zip` stops at the shorter iterator → trailing added lines dropped, shifted content shows as
bogus `-`/`+` pairs. Prints `---`/`+++`/`@@` headers and help advertises "unified diff", so it
misrepresents itself. Presentation-only.
→ Use a real diff (e.g. `similar`'s `TextDiff::unified_diff`), or relabel as naive comparison.

### BUG-10 — Nonexistent literal path silently swallowed → exits 0 (CI false-pass)
`src/main.rs:178-198` (glob fallback), `97-100` (empty-paths) · effort: S
A nonexistent path falls through to glob, matches nothing → "No .tf files found" → exit 0.
`tf-format --check /tmp/does_not_exist.tf` passes green, defeating `--check`.
→ If an arg has no glob metacharacters and doesn't exist, return `PathNotFound`. Consider
exiting non-zero when `--check` discovers zero files.

### BUG-11 — BOM-prefixed files fail to parse and abort formatting for that file
`src/lib.rs:63` (parse), `src/main.rs:129` (read_to_string) · effort: S
A leading UTF-8 BOM makes hcl-edit return a parse error; not stripped, so BOM-saved files
(common from Windows editors) are reported `Format` errors and never formatted. `terraform fmt`
tolerates a leading BOM.
→ Strip a leading U+FEFF before parsing (optionally re-emit it). Add a BOM fixture.

### TEST-4 — Top-level block-sorting fixtures: `output`, `data` (type+name tie-break), negative cases
`tests/fixtures/*`; `src/formatter.rs:958-966`, `1028-1038` · effort: S
No fixture contains `output` or `data` (those match-arms unverified — deleting them passes
all tests); no same-type/different-name pair exercises the name tie-break; nothing pins that
locals/provider/terraform/module are NOT reordered. Bug-masking coverage gaps.
→ Add fixtures for out-of-order outputs; mixed data types/names (type-then-name); same-type
resources (name tie-break); preserved module/provider order; group-breaking interleave. Route
through `run_fixture`.

### TEST-5 — Validate the whole minimal fixture suite against real `tofu`, fail loudly when absent
`tests/terraform_fmt_parity.rs:5-12, 57-61, 90-95, 117-388` · effort: M
Real tofu parity is cross-checked only for alignment-isolated inputs; the bulk of minimal mode
is asserted against hand-written `expected.tf`. Worse, `check_parity*` early-return with an
`eprintln` SKIP when tofu is absent → silent green no-op on any lane without tofu.
→ Drive every `fixtures-minimal/*/input.tf` through `check_parity_minimal`. Gate the SKIP behind
`TF_FORMAT_REQUIRE_TOFU=1` (hard failure in CI), or add a meta-test asserting tofu ran.

### TEST-6 — Tests for error paths / invalid HCL (typed-error invariant)
`tests/*`; `src/lib.rs:62-63`, `src/error.rs` · effort: S
No test feeds malformed HCL to assert `Err(FormatError::ParseHcl)` rather than a panic; the
`ProcessFileError` path-context Display is untested. `#![deny(unwrap_used)]` is the only guard.
→ Add tests passing invalid HCL asserting the typed variant/message, plus a Display test.

---

## P3 — cosmetic / nice-to-have

### BUG-12 — Whitespace-only/empty files not idempotent; empty file rewritten to a newline
`src/lib.rs:74-84`; `src/main.rs:140` · effort: S
`format(format("   \n\n  \t\n"))` differs from one pass (breaks idempotence); `format_hcl("")`
returns `"\n"` so an empty file is reported changed under `--check` and rewritten to one
newline (breaks byte-identical no-op). `terraform fmt` leaves empty files empty.
→ Short-circuit empty/whitespace-only input to a deterministic fixed point. Add fixtures.

### BUG-13 — Multi-line block-comment indentation rewritten even when not dropped
`src/formatter.rs:143-147, 168-172` (per-line `comment.trim()`) · effort: S
Continuation lines starting with `*` (which survive BUG-1's filter) get `trim()`ed and a flat
depth indent, collapsing conventional ` * ` alignment. Minimal-mode fidelity regression.
→ Preserve relative indentation of continuation lines (covered by the BUG-1 span-based fix).

### IMP-1 — Deduplicate and canonicalize discovered file paths
`src/main.rs:175-202`, `102-118` · effort: S
`discover_files` concatenates with no de-dup/canonicalization. `tf-format --check f.tf f.tf`
reports the file twice and counts 2. Safe but wastes I/O and inflates check output.
→ Canonicalize and collect into an ordered set before processing. Pairs with BUG-3.

### IMP-2 — `should_expand_single_line_object` underestimates width (ignores `key = ` prefix)
`src/formatter.rs:671-683`; call sites `438/447` · effort: S
`line_width = depth*2 + rendered.len()` omits the preceding `key = `, so lines slightly over
80 chars escape expansion; threshold becomes key-length dependent. Doc comment also overstates.
→ Include `key = ` width in the measurement (or at least fix the doc comment). Add a boundary fixture.

### IMP-3 — Multi-line detection re-renders expressions to String repeatedly (perf)
`src/classify.rs:12-14`; `src/formatter.rs:753-758, 834, 840` · effort: M
Multi-line-ness via `value.to_string()` (full recursive re-render) scanned for `\n`, done
multiple times per structure per pass — works against the "milliseconds" goal.
→ Compute once and reuse, or detect structurally instead of serializing.

### TEST-7 — Dedicated whitespace-normalization fixture (Rule #8)
`tests/*`; `src/lib.rs:74-84` · effort: S
No input fixture has trailing whitespace, missing final newline, multiple trailing blanks, or
tab/4-space indent, so post_process's logic is only exercised incidentally.
→ Add such fixtures asserting canonical 2-space output + single trailing newline + idempotency.

### TEST-8 — Assert `depends_on` fixed priority ordering as a single-line meta-arg
`tests/fixtures/meta_arguments`; `src/formatter.rs:38-45` (PRIORITY_ATTRS) · effort: S
`depends_on` appears only as a multi-line list (multi tier); its fixed single-line ordering
relative to other priority attrs is never asserted.
→ Extend the meta_arguments fixture with provider + single-line `depends_on` + count out of
order, asserting fixed `PRIORITY_ATTRS` order.

---

## Suggested sequencing

1. **BUG-1 + TEST-1** and **BUG-2 + TEST-3** together — the two P0 data-loss bugs with the
   fixtures that would have caught them. BUG-13 falls out of the BUG-1 fix.
2. **TEST-2** (CLI harness) — unblocks verifying BUG-4, BUG-8, BUG-9, BUG-10.
3. P1 robustness batch: BUG-3, BUG-4, BUG-5, BUG-6 (+ IMP-1 alongside BUG-3).
4. P2 CLI papercuts (BUG-7..11) + coverage (TEST-4..6).
5. P3 cleanups.
