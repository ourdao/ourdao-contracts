#!/bin/bash
set -euo pipefail

# Reproducible build script for Soroban WASM contracts
# Uses official stellar/soroban-cli container for deterministic builds

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
CONTRACT_NAME="ourdao_contract"
WASM_OUTPUT_DIR="$PROJECT_ROOT/target/wasm32-unknown-unknown/release"
WASM_FILE="$WASM_OUTPUT_DIR/$CONTRACT_NAME.wasm"
CONTAINER_WASM_PATH="/tmp/$CONTRACT_NAME.wasm"

# Ensure output directory exists
mkdir -p "$WASM_OUTPUT_DIR"

# Build inside Soroban container for reproducibility
docker run --rm \
  -v "$PROJECT_ROOT:/src" \
  -w "/src" \
  stellar/soroban-cli:latest \
  sh -c "cargo build --release --target wasm32-unknown-unknown && \
          cp /src/$WASM_OUTPUT_DIR/$CONTRACT_NAME.wasm $CONTAINER_WASM_PATH"

# Copy WASM from container to host
cp "$CONTAINER_WASM_PATH" "$WASM_FILE"

# Generate SHA-256 hash for verification
sha256sum "$WASM_FILE" | awk '{print $1}' > "$WASM_FILE.sha256"

echo "Reproducible build complete. WASM file: $WASM_FILE"
echo "SHA-256 hash saved to $WASM_FILE.sha256"

# Output hash for CI verification
cat "$WASM_FILE.sha256"
