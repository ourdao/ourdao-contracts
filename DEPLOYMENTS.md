# Deployment Guide

## Prerequisites
- Docker
- Rust toolchain
- Soroban CLI

## Reproducible Build Verification

To ensure the WASM bytecode matches the source code exactly:

1. **Local Verification**
   ```bash
   chmod +x ./scripts/build-reproducible.sh
   ./scripts/build-reproducible.sh
   ```
   This generates:
   - WASM file in `target/wasm32-unknown-unknown/release/`
   - SHA-256 hash file with `.sha256` extension

2. **CI Verification**
   The GitHub Actions workflow `.github/workflows/reproducible-build.yml` automatically:
   - Builds the contract in the official Soroban container
   - Compares the SHA-256 hash against the stored hash (on main branch)
   - Fails if hashes don't match

3. **Audit Verification**
   To verify a deployment:
   ```bash
   # Generate local hash
   ./scripts/build-reproducible.sh > local_hash.txt
   
   # Compare with on-chain or published hash
   diff local_hash.txt <published_hash_file>
   ```

## Deployment Steps
1. Build the contract:
   ```bash
   ./scripts/build-reproducible.sh
   ```
2. Verify the hash matches expectations
3. Deploy using Soroban CLI:
   ```bash
   soroban contract deploy --wasm target/wasm32-unknown-unknown/release/ourdao_contract.wasm
   ```

## Notes
- Always verify the WASM hash before deployment
- The reproducible build ensures bytecode matches source exactly
- Critical for security audits and on-chain verification
