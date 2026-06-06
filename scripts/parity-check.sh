#!/usr/bin/env bash
# Parity check: tf-format --style minimal must match `tofu fmt`.
#
# For each .tf/.tofu file in a directory: canonicalise with `tofu fmt`, then
# run `tf-format --style minimal` on that canonical output. tofu fmt is
# idempotent, so a correct minimal formatter is a no-op here — any diff is a
# parity bug. Files tofu can't parse are skipped (not our concern).
#
# Usage: parity-check.sh <dir> [<tf-format-binary>]
#   TF_FORMAT  env overrides the binary path.
#   TOFU       env overrides the tofu binary (default: tofu on PATH).
set -uo pipefail

dir="${1:?usage: parity-check.sh <dir> [tf-format-binary]}"
TF_FORMAT="${TF_FORMAT:-${2:-tf-format}}"
TOFU="${TOFU:-tofu}"

mismatches=0
checked=0
skipped=0

while IFS= read -r -d '' f; do
  canon=$("$TOFU" fmt - < "$f" 2>/dev/null)
  if [ $? -ne 0 ] || [ -z "$canon" ]; then
    skipped=$((skipped+1)); continue
  fi
  checked=$((checked+1))
  out=$(printf '%s' "$canon" | "$TF_FORMAT" --stdin --style minimal 2>/dev/null)
  if [ "$canon" != "$out" ]; then
    mismatches=$((mismatches+1))
    echo "### MISMATCH: $f"
    diff <(printf '%s' "$canon") <(printf '%s' "$out") | head -40
    echo
  fi
done < <(find "$dir" -type f \( -name '*.tf' -o -name '*.tofu' \) ! -path '*/.terraform/*' -print0)

echo "SUMMARY dir=$dir checked=$checked mismatches=$mismatches skipped=$skipped"
[ "$mismatches" -eq 0 ]
