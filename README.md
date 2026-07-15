# OurDAO — Stellar Soroban Lending DAO

[![CI](https://github.com/ourdao/ourdao-contracts/actions/workflows/ci.yml/badge.svg)](https://github.com/ourdao/ourdao-contracts/actions/workflows/ci.yml)

A member-owned lending DAO implemented as a [Soroban](https://developers.stellar.org/docs/build/smart-contracts) smart contract in Rust.

This is a ground-up reimplementation of the original EVM `UnifiedLendingDAO` (Solidity) for the Stellar network. It is **not** a line-by-line translation — the execution model, storage, authorization, and value transfer are all Soroban-native. All DAO value flows through a single configurable token set at initialization (USDC, XLM via the Stellar Asset Contract, or any Stellar asset).

## What the DAO does

- **Governance** — a set of admins, a basis-points consensus threshold (default 51%), and tunable loan policy.
- **Membership** — anyone can join by paying a membership fee in the DAO token; the fee becomes their share in the treasury. Members can exit and withdraw their pro-rata share.
- **Lending** — members request loans that go through an editable draft phase, then member voting; on approval the principal is disbursed from the treasury. Repayment returns principal to the treasury and distributes interest to members as claimable yield.
- **Treasury** — members propose withdrawals to any destination; execution requires a higher (60%) consensus.
- **Safety** — admin pause/unpause and extensive view functions.

## Stellar-native replacements for the EVM extensions

The original contract carried four Ethereum-ecosystem integrations with no Soroban equivalent. Each is replaced by a real, working Stellar-native feature:

| Original EVM extension | Soroban-native replacement | Module |
|---|---|---|
| ENS governance (naming) | On-chain **name registry** (name ⇄ address) | `registry.rs` |
| Filecoin storage | **Content-hash metadata** — anchor an IPFS CID / digest to a proposal | `docs.rs` |
| FHE encrypted voting | **Commit-reveal voting** for private treasury proposals | `privacy.rs` |
| Symbiotic restaking | **Staking** for a capped voting-weight boost | `staking.rs` |

## Layout

```
contracts/dao/
  src/
    lib.rs         # contract entrypoints (the public ABI) + views
    admin.rs       # init, admins, threshold, policy, pause
    membership.rs  # join, exit, claim yield, exit-share math
    loans.rs       # request -> edit -> vote -> disburse -> repay -> interest
    treasury.rs    # propose -> vote -> execute (shared tally)
    registry.rs    # name registry (ENS analog)
    docs.rs        # content-hash proposal metadata (Filecoin analog)
    privacy.rs     # commit-reveal voting (FHE analog)
    staking.rs     # voting-weight staking (Symbiotic analog)
    storage.rs     # typed storage keys + TTL management
    types.rs       # data model + constants
    error.rs       # contract error codes
    util.rs        # token client, auth guards, vote math
    test.rs        # full test suite
```

## Build & test

Requires the Rust `wasm32v1-none` target (Rust 1.84+) and the [`stellar` CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli).

```bash
# Native unit tests
cargo test

# Release wasm
cargo build --target wasm32v1-none --release

# Optimized, deployment-ready wasm
stellar contract build --optimize
```

> **Note on dependencies:** `Cargo.lock` pins `ed25519-dalek` to `2.2.0`. A newer
> transitive release (`3.0.0`) is incompatible with the pinned `rand_core` used by
> `soroban-env-host` and breaks the test build if allowed to float. The lockfile is
> committed to keep builds reproducible.

## Deploy (example)

```bash
stellar contract deploy \
  --wasm target/wasm32v1-none/release/ourdao_dao.optimized.wasm \
  --network testnet \
  --source <your-identity>
```

Then initialize with your admin set, consensus threshold (bps), membership fee, DAO token address, and loan policy.

## License

MIT
