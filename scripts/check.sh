#!/usr/bin/env bash
# Compact lint + test run. Usage: scripts/check.sh [crate]   (no crate = whole workspace)
# Prints only warnings/errors and test failures, then a one-line summary.
set -uo pipefail
cd "$(dirname "$0")/.."

if [[ ! -f Cargo.toml ]]; then
  echo "check: no Cargo.toml yet – skipped"
  exit 0
fi

if [[ $# -gt 0 ]]; then
  scope=(-p "$1"); label="$1"
else
  scope=(--workspace); label="workspace"
fi

status=0

fmt_out=$(cargo fmt --all --check 2>&1) || { echo "fmt: needs formatting – run 'cargo fmt --all'"; echo "$fmt_out" | grep '^Diff in' | head -n 10; status=1; }

clippy_out=$(cargo clippy -q "${scope[@]}" --all-targets --message-format=short -- -D warnings 2>&1) || status=1
[[ -n $clippy_out ]] && echo "$clippy_out" | grep -E '(error|warning)' | head -n 40

test_out=$(cargo test -q "${scope[@]}" 2>&1) || {
  status=1
  echo "$test_out" | grep -E '^(---- |thread .* panicked|failures:|test result: FAILED|error(\[|:))' | head -n 40
}

if [[ $status -eq 0 ]]; then
  echo "check ($label): OK – fmt, clippy, tests"
else
  echo "check ($label): FAILED"
fi
exit $status
