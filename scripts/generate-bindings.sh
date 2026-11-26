#!/usr/bin/env bash
# generate-bindings.sh — Issues #350, #441
#
# Generates the TypeScript client for the DAO contract from the optimized wasm
# and writes it to bindings/ts/. The output is committed so consumers
# (ourdao-frontend, ourdao-backend) can depend on it without the stellar CLI.
#
# Usage:
#   ./scripts/generate-bindings.sh          # builds and regenerates bindings/ts
#   ./scripts/generate-bindings.sh --check  # exits non-zero if bindings/ts differs
#                                           # from a fresh generation (used in CI)
#
# Prerequisites:
#   - Rust + wasm32v1-none target (pinned in rust-toolchain.toml)
#   - stellar CLI: https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli

set -euo pipefail

cd "$(dirname "$0")/.."

# Same optimized wasm as `make costs` / `make size`.
OPTIMIZED_DIR="target/optimized"
WASM_PATH="${OPTIMIZED_DIR}/ourdao_dao.wasm"
BINDINGS_DIR="bindings/ts"
# The CLI names the npm package after the output directory's basename.
PACKAGE_NAME="ourdao-dao"
CHECK_MODE=false

for arg in "$@"; do
  case $arg in
    --check) CHECK_MODE=true ;;
    *) echo "Unknown argument: $arg" >&2; exit 1 ;;
  esac
done

if ! command -v stellar >/dev/null 2>&1; then
  echo "Error: the stellar CLI is required to generate bindings." >&2
  echo "Install it: https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli" >&2
  exit 1
fi

echo "Building optimized contract wasm..."
stellar contract build --locked --package ourdao-dao --out-dir "$OPTIMIZED_DIR" 2>&1 | tail -5

if [ ! -f "$WASM_PATH" ]; then
  echo "Error: wasm not found at $WASM_PATH after build" >&2
  exit 1
fi

FRESH_DIR="$(mktemp -d)"
trap 'rm -rf "$FRESH_DIR"' EXIT

stellar contract bindings typescript \
  --wasm "$WASM_PATH" \
  --output-dir "$FRESH_DIR/$PACKAGE_NAME" \
  --overwrite

# The CLI emits each method signature's arguments as a destructuring pattern,
# and the treasury proposals' `private` argument is a reserved word in
# strict-mode TypeScript, so `tsc` rejects the output. Name that parameter
# `args` instead: callers still pass `{ ..., private: ... }` and the on-chain
# argument is unchanged.
perl -pi -e 's/\(\{[^}]*\bprivate\}: \{/(args: {/g' "$FRESH_DIR/$PACKAGE_NAME/src/index.ts"

# Installed dependencies and build output are never committed.
DIFF_EXCLUDES=(--exclude node_modules --exclude dist)

if $CHECK_MODE; then
  if ! diff -r "${DIFF_EXCLUDES[@]}" "$BINDINGS_DIR" "$FRESH_DIR/$PACKAGE_NAME" >&2; then
    echo "" >&2
    echo "CI drift check FAILED: $BINDINGS_DIR is stale." >&2
    echo "Run ./scripts/generate-bindings.sh and commit the result." >&2
    exit 1
  fi
  echo "CI drift check PASSED: $BINDINGS_DIR matches a fresh generation."
else
  rm -rf "$BINDINGS_DIR"
  mkdir -p "$(dirname "$BINDINGS_DIR")"
  cp -R "$FRESH_DIR/$PACKAGE_NAME" "$BINDINGS_DIR"
  echo "TypeScript bindings generated at $BINDINGS_DIR"
fi
