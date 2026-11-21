# OurDAO Contract Deployments

This document tracks the OurDAO DAO contract deployments across networks. Each deployment is linked to a specific GitHub release.

## Network Deployments

### Stellar Testnet

| Release | Contract ID | Status | Deployed |
|---------|---|---|---|
| Latest | TBD | Preparing | - |

The testnet DAO also holds a second test asset, `OURTESTB`, so multi-token flows can be exercised. Its contract ID is printed by `scripts/deploy-testnet.sh`, and it is recorded here once the script has run:

| Asset | Contract ID | Whitelisted | Recorded |
|-------|---|---|---|
| `OURTESTB` | TBD | Proposed, executes 48 hours after `propose_token_addition` | - |

### Stellar Public Network

| Release | Contract ID | Status | Deployed |
|---------|---|---|---|
| Latest | TBD | Not yet deployed | - |

## Deployment record template

Copy this row for each deployment and replace every placeholder. Record the initialization arguments passed to the contract, including all initial admins, consensus threshold, membership fee, base token, and loan policy.

| Network | Release/tag | Contract ID | WASM SHA-256 | Git commit | Initialization parameters | Deployed (UTC) | Deployment transaction |
|---|---|---|---|---|---|---|---|
| `<network>` | `vX.Y.Z` | `<contract-id>` | `<64-character hash>` | `<commit SHA>` | `admins=...; threshold=...; membership_fee=...; base_token=...; policy=...` | `YYYY-MM-DD HH:MM` | `<transaction hash or URL>` |

Use the hash printed by `scripts/build-reproducible.sh`, not a hash from an unverified local build. The script reports both build hashes and ends with `reproducible build verified: <hash>` only when they match; record that hash in the table.

## Post-deployment checklist

- [ ] Confirm the release tag and commit are the intended versions for the target network.
- [ ] Run `./scripts/build-reproducible.sh`; confirm build A and build B hashes match and copy the verified SHA-256 into the deployment record.
- [ ] Confirm the initialization arguments (admins, threshold, membership fee, base token, and policy) are correct before submitting the deployment.
- [ ] Save the successful deployment transaction and contract ID, then query the contract on the same network (for example, `stellar contract info --id <CONTRACT_ID> --network testnet`).
- [ ] Read back the contract version and configuration using the available contract views; confirm they match the release and recorded initialization parameters.
- [ ] Add or update the deployment row with the network, release, contract ID, verified WASM hash, commit, initialization parameters, UTC timestamp, and transaction reference.

## Verification

To verify a deployed contract matches a GitHub release:

1. **Find the release** — Go to [Releases](https://github.com/ourdao/ourdao-contracts/releases) and select the version.

2. **Download the wasm** — Each release includes `ourdao_dao.optimized.wasm`.

3. **Verify the checksum** — Compare the SHA-256 hash in the release notes against your copy:
   ```bash
   sha256sum ourdao_dao.optimized.wasm
   ```

4. **Confirm the contract ID** — Query the network for the contract you deployed:
   ```bash
   stellar contract info --id <CONTRACT_ID> --network testnet
   ```

## Reproducible build verification

The repository includes `scripts/build-reproducible.sh`, which builds
`ourdao-dao` inside a pinned Rust/Stellar CLI build container and prints the
SHA-256 hash of the resulting WASM.

Run the verification with:

```bash
./scripts/build-reproducible.sh
```

The script builds the contract twice in separate instances of the pinned build
container. Each invocation uses an isolated Cargo target directory, then the
script compares the SHA-256 hashes of the two resulting WASM files. It exits
non-zero if the bytes differ. CI runs this verification on every pull request.

The reproducible image is built from `Dockerfile.reproducible`, pinned to
Rust 1.98.1 on Debian Trixie and the official Stellar CLI 28.0.0 binary. The
local image tag defaults to
`ourdao-reproducible:rust-1.98.1-stellar-28.0.0`; override only the tag with
`REPRO_IMAGE` if required:

```bash
REPRO_IMAGE=ourdao-reproducible:verification \
  ./scripts/build-reproducible.sh
```

For release verification, record the printed SHA-256 in the release notes and
compare it with the downloaded release artifact before deployment.

## Deployment Process

To deploy a new release:

1. **Check the release** — Open the [latest release](https://github.com/ourdao/ourdao-contracts/releases) and verify the checksum.

2. **Download or build locally** — Either download the wasm from the release, or build it yourself:
   ```bash
   git checkout v<VERSION>
   stellar contract build --optimize
   ```

3. **Deploy** — Use the deployment script (for testnet) or a key-holding CI job (for production):
   ```bash
   ./scripts/deploy-testnet.sh
   ```

4. **Record the contract ID** — Update this file with the new deployment, including the release version and timestamp.

## Release History

- **v1.0.0** (not yet released) — Initial release candidate

## Notes

- Each release produces a reproducible, verifiable wasm artifact with a documented SHA-256.
- Contract IDs are recorded per network to enable members to verify they have the correct code.
- There is currently no upgrade path; a new deployment is a fresh contract with a new ID.