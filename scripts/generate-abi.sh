#!/usr/bin/env bash
# generate-abi.sh — Issue #135
#
# Extracts the contract's XDR spec from the compiled wasm and writes it to
# contract/contract-interface.json alongside a human-readable summary
# (contract/interface-summary.txt). Both files are committed (#442) and CI runs
# --check so they cannot drift from lib.rs.
#
# Usage:
#   ./scripts/generate-abi.sh          # builds and generates
#   ./scripts/generate-abi.sh --markdown # prints the README method table
#   ./scripts/generate-abi.sh --check  # exits non-zero if output differs from
#                                      # what a fresh generation would produce
#                                      # (used in CI to detect drift)
#
# Prerequisites:
#   - Rust + wasm32v1-none target (see CONTRIBUTING.md)
#   - stellar CLI: https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli

set -euo pipefail

cd "$(dirname "$0")/.."

# Workspace builds land in the root target/ dir, not the crate's own.
WASM_PATH="target/wasm32v1-none/release/ourdao_dao.wasm"
OUT_DIR="contract"
OUT_JSON="${OUT_DIR}/contract-interface.json"
OUT_SUMMARY="${OUT_DIR}/interface-summary.txt"
CHECK_MODE=false
MARKDOWN_MODE=false

for arg in "$@"; do
  case $arg in
    --check) CHECK_MODE=true ;;
    --markdown) MARKDOWN_MODE=true ;;
    *) echo "Unknown argument: $arg" >&2; exit 1 ;;
  esac
done

if $MARKDOWN_MODE; then
  echo "| Method |"
  echo "|---|"
  awk '/^[[:space:]]*pub fn [[:alnum:]_]+\(/ {
    name = $0
    sub(/^[[:space:]]*pub fn /, "", name)
    sub(/\(.*/, "", name)
    printf "| `%s` |\n", name
  }' contracts/dao/src/lib.rs
  exit 0
fi

echo "Building contract wasm..."
cargo build --locked --target wasm32v1-none --release -p ourdao-dao 2>&1 | tail -5

if [ ! -f "$WASM_PATH" ]; then
  echo "Error: wasm not found at $WASM_PATH after build" >&2
  exit 1
fi

mkdir -p "$OUT_DIR"
FRESH_JSON="$(mktemp)"
FRESH_SUMMARY="$(mktemp)"
trap 'rm -f "$FRESH_JSON" "$FRESH_SUMMARY"' EXIT

echo "Extracting contract spec from wasm..."
# Formatted JSON keeps the committed file reviewable and its diffs readable.
stellar contract info interface --wasm "$WASM_PATH" --output json-formatted > "$FRESH_JSON"
stellar contract info interface --wasm "$WASM_PATH" --output rust > "$FRESH_SUMMARY"

if $CHECK_MODE; then
  if [ ! -f "$OUT_JSON" ]; then
    echo "CI drift check FAILED: $OUT_JSON does not exist." >&2
    echo "Run ./scripts/generate-abi.sh and commit the result." >&2
    exit 1
  fi
  if ! diff -q "$OUT_JSON" "$FRESH_JSON" > /dev/null 2>&1; then
    echo "CI drift check FAILED: $OUT_JSON is stale." >&2
    echo "Diff:" >&2
    diff "$OUT_JSON" "$FRESH_JSON" >&2 || true
    echo "" >&2
    echo "Run ./scripts/generate-abi.sh and commit the result." >&2
    exit 1
  fi
  echo "CI drift check PASSED: $OUT_JSON matches a fresh generation."
else
  cp "$FRESH_JSON" "$OUT_JSON"
  # Human-readable summary of the same spec, as Rust signatures.
  cp "$FRESH_SUMMARY" "$OUT_SUMMARY"
  echo "Generated: $OUT_JSON"
  echo "Generated: $OUT_SUMMARY"
  echo ""
  echo "Remember to commit both files. README.md links to"
  echo "contract/contract-interface.json as the generated source of truth."
fi
