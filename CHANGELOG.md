# Changelog

All notable changes to `ourdao-contracts` are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to follow [Semantic Versioning](https://semver.org/). Update the workspace version in `Cargo.toml` alongside each dated release heading.

This file records **what has changed**. For **what is deployed** (tagged releases, contract IDs per network, and verified wasm hashes), see [DEPLOYMENTS.md](DEPLOYMENTS.md). A version heading here does not mean that version has been tagged or deployed.

## [Unreleased]

### Changed
- **Storage layout:** `TreasuryProposal` gains an `editing_period_end` field (#452). Records written before this change do not decode under the new type. This is accepted rather than handled with a fallback: the contract is immutable with no in-place upgrade path (see `docs/MIGRATION.md`), and treasury proposals are not carried over by migration seeding, so no deployment running this code can hold a record without the field. Off-chain decoders of `get_treasury_proposal` must add the field.

### Added
- `Error::SeedLoanMemberMismatch` (101), returned by `seed_loans` (#455).
- Property test `total_staked_equals_sum_of_member_stakes` covering invariant L4 across random multi-member `stake` / `unstake` sequences; `docs/INVARIANTS.md` L4 is now ✅ (#454).

### Fixed
- `cancel_treasury_proposal` checks the cancellation window snapshotted at creation (`editing_period_end`) instead of reading `editing_period` from the live policy, so a policy update can no longer shorten or lengthen an in-flight treasury proposal's window (#452).
- `stake` now rejects (`HasActiveLoan`, as `unstake` does for its vote lock) while the staker, or the delegatee it backs, holds an unrevealed commitment on a pending private treasury proposal. Weight is read live at reveal, so a voter could previously commit, watch other reveals move the public tally, then stake up before revealing (#453).
- `seed_loans` rejects the whole batch with `SeedLoanMemberMismatch` when an `Active` loan's borrower has not been seeded or was seeded with `has_active_loan == false`, so invariant L2 can no longer be broken by out-of-sync migration batches; `docs/MIGRATION.md` documents the members-before-loans ordering and the reverse-direction checks that remain off-chain (#455).
- `tests/migration.rs` is now registered in `tests/mod.rs` and runs.
- The test suite compiles and passes again. Split-out test files gained their missing imports, `error_coverage.rs` calls the real entrypoints (`unstake`, `commit_treasury_vote` / `reveal_treasury_vote`, `claim_rewards`, `seed_members`), two tests truncated by a merge in `tests/treasury.rs` are closed, and tests written against since-changed rules are updated. Three tests whose scenario the contract now forbids are removed: removing a whitelisted token that still has an active loan or a treasury balance (#272, #273), and a pause table that expected `repay_loan` to stay callable while paused (#52 made it pause-gated). New tests cover `get_member_overview`, `list_members`, `list_treasury_proposals`, `list_loans`, `vote_batch`, `set_loan_policy`, the pending token addition views and the `seed_stakes` / `seed_pending_yield` / `seed_docs` entrypoints, which had lost their tests in the split and left line coverage below the 95% CI floor.
- `tally` counted every treasury vote twice in `votes_cast`, so the early-rejection bound could reject a withdrawal that could still pass.
- Loan early rejection no longer counts the borrower as a remaining vote, since they may not vote on their own proposal (#327). A loan that can no longer reach its threshold is now rejected instead of waiting out the voting period.
- `initialize` checks for duplicate admins before collecting their auth, so a repeated address returns `AlreadyAdmin` instead of trapping, and `remove_admin` no longer asks a self-removing admin to authorize twice (which trapped before `CannotRemoveLastAdmin` could be returned).
- `propose_token_addition`, `execute_token_addition` and `remove_token` are pause-gated, as `docs/PAUSE_FUNCTIONALITY.md` already stated (#527).
- Removed the unused `util::ceil_div_nonnegative` helper and unused test helpers that failed `clippy -D warnings`.
- The committed ABI and TypeScript bindings are regenerated; they were missing `release_name`, `TreasuryProposal.editing_period_end` and `SeedLoanMemberMismatch`.
### Added
- Documented the `ProposalKind` extension contract: a new proposal-like feature either appends a variant and gives it a real arm in every `docs.rs` match, or is documented as excluded from document attachment and rejected with `InvalidProposal`. The note lives on `ProposalKind`, and `docs.rs` and `INVARIANTS.md` point to it. A test with an exhaustive match stops compiling when a variant is added (#459).
- A test proving every `get_dao_summary().treasury_balances` entry matches `get_treasury_balance_in_asset` after funds move in three assets (stake in the base token, a loan in a second asset, a treasury withdrawal in a third), with the base token first in both the whitelist and the summary (#457).
- `release_name(owner)` lets a member give up their registered name without claiming a replacement or exiting the DAO. It requires `owner` auth, is pause-gated, emits the existing `name_rel` event, and returns `NameNotFound` (61, previously reserved and never returned) when `owner` holds no name (#458).
- Regression tests pinning `expire_treasury_proposal` to literal deadlines: a private proposal is not expirable anywhere in `(voting_period, voting_period * 2]` and expires after `voting_period * 2`, while a public proposal still expires after one `voting_period` (#456).

### Fixed
- The contract library compiles again: concurrent merges had each numbered new error variants 92–97 and a later merge dropped `WhitelistFull`, `TransferAmountMismatch`, `JoinedAfterProposal`, `TooManyAdmins`, `TooManyVotesInBatch` and `ArithmeticOverflow` (and `types::ONE`) while their call sites remained. They are restored as append-only codes 95–100, so no existing code is renumbered; `default_loan` propagates `get_loan_asset`'s `Result` (#441, #442).
- `register_member` now rejects a returning member who defaulted on a loan (or still has one outstanding) with the new `Error::DefaultedLoanOutstanding` (94), so exiting and rejoining can no longer reset a defaulter's standing (#168).
- `contracts/dao/src/admin.rs` compiles again: `remove_token` regains its closing brace and token-removal event payload, the token-whitelist events use the shared `events` constants, and the unused, malformed duplicate `try_execute_policy_update` is removed (the public entrypoint is `execute_policy_update`) (#433).
- `validate_policy` now caps `max_loan_duration` at `admin::MAX_LOAN_DURATION` (365 days), rejecting longer terms with `InvalidLoanPolicy` at `initialize` and on `propose_policy_update`, like the 30-day cap on the other policy durations (#449).
- `propose_policy_update` accepts `quorum_bps == 0`, the fallback-to-`consensus_threshold` sentinel that `initialize` already accepts; both paths now share `validate_policy` alone (#450).
- Treasury withdrawals parked in `ApprovedPendingDisbursement` now reserve their amount: `propose_withdrawal_in_asset` and the loan-request treasury-ratio cap subtract them (and approved-but-undisbursed loans) via the new `util::available_treasury_in_asset`, so the same funds can't be committed twice (#451).
- Decided and documented that `execute_policy_update` is the only way to apply a pending policy update: the removed `try_execute_policy_update` is not coming back (a contract fn by that name would also collide with the SDK-generated `try_` client method). README no longer lists it, and a test pins the `(old_policy, new_policy)` event payload (#447).
- `expire_treasury_proposal` now waits for a private proposal's full commit + reveal window (`created_at + voting_period * 2`), the same boundary `tally` enforces, so committed votes can no longer be locked out of revealing (#434).

### Added
- `rust-toolchain.toml` pins Rust 1.100.0 with `rustfmt`, `clippy` and the `wasm32v1-none` target. CI (`ci.yml`, `release.yml`, `integration.yml`) installs from it instead of floating `stable`, `Dockerfile.reproducible` uses the same version, and the MSRV job calls `cargo +1.84` explicitly (#240).
- `contract/contract-interface.json` and `contract/interface-summary.txt`, generated by `scripts/generate-abi.sh` and linked from the README ABI section as the source of truth. `generate-abi.sh` now reads the workspace `target/` wasm, uses `stellar contract info interface`, and is executable (#442).
- The generated TypeScript client in `bindings/ts/` (package `ourdao-dao`, depending on `@stellar/stellar-sdk`). `generate-bindings.sh` builds the optimized wasm, uses `stellar contract bindings typescript`, patches the strict-mode `private` argument so `tsc` passes, and no longer silently writes a stub when the CLI is missing; `make abi` / `make bindings` run the generators (#441).
- CI `generated-artifacts` job: installs a pinned, checksum-verified stellar CLI, runs `generate-abi.sh --check` and `generate-bindings.sh --check`, and runs the new `tests/repo_consistency.rs` checks (#240, #441, #442, #445).

### Documentation
- `ADMIN_GUIDE.md` no longer claims `AlreadyPauser` and `NotPauser` are missing from `error.rs`; both exist as codes 81 and 82 and are now listed in its error-code table (#440).
- `DEPLOYMENTS.md`'s release history now states that no version has been tagged yet despite the functionality on `main` (tag cut tracked in #265), and `DEPLOYMENTS.md` and `CHANGELOG.md` cross-link as "what's deployed" and "what's changed" (#445).
- `CONTRIBUTING.md` documents installing the pinned toolchain and regenerating the committed ABI and bindings on any interface change (#240, #441).

## [0.2.0] - 2026-11-21

### Changed
- The README ABI section now lists each public contract method from `lib.rs`; `scripts/generate-abi.sh --markdown` prints the table for regeneration (#258).
- Added a regression test proving non-base loans use the loan asset's treasury balance for ratio checks and pricing (#277).
- Added a regression test ensuring wrong-asset loan repayment fails before moving any tokens (#278).
- Added a regression test proving treasury proposals execute strictly in their recorded asset even after whitelist removal (#279).
- Staking now escrows/returns the configured base token, and regression coverage proves membership fees and staking ignore additional whitelisted treasury assets (#280).
- Documented and regression-tested that voting weight is independent of a member's contribution (#509).
- Locked treasury proposal documents after the first vote so governance evidence cannot be swapped mid-vote (#514).
- Added a worked example and regression tests explaining how partial repayment, overdue repayment fees, and default contribution slashing interact (#515).
- CI now enforces append-only, unique error-code discriminants and requires comments for intentional numeric gaps (#528).
- Explicitly enforce and document that pausers cannot unpause (`admin::unpause` requires admin authorization and rejects non-admin pausers with `Error::NotAdmin`), separating emergency halt and resume privileges (#319).
- `loan_req`, `loan_appr`, `loan_rpy` and `interest` events now carry the loan's `asset` as a trailing field (#343). Existing positional fields are unchanged, so indexers that read by position keep working; `interest` is only emitted for base-token repayments.
- `exit_dao` / `calculate_exit_share` are now O(1): a maintained `ActiveContributions` total replaces the per-member scan (#353).
- `pause` and `unpause` events now include the caller (#346).
- `set_consensus_threshold` and `execute_policy_update` events now emit old and new values (#347).
- `staked` and `unstaked` events now include the resulting voting weight (#348).

### Added
- Per-asset yield accumulators, remainders and member snapshots. Loan interest is credited and claimed in the loan's asset; `exit_dao` settles accrued yield in every tracked asset while preserving the existing base-token claim API (#271).
- `scripts/deploy-testnet.sh` deploys a second test asset (`OURTESTB`) on testnet and proposes it for the treasury whitelist; `DEPLOYMENTS.md` tracks it (#342).
- `get_member_overview(member)` view returning the member record, stake, pending yield, loan stats, registered name and delegate in one call (#340).
- `get_delegate(delegator)` view returning the address a member delegated their vote to, or `None` (#341).
- Members can delegate voting weight to another active member; the delegate view exposes that assignment (#188, #341, #510).
- `docs/events.schema.json`: machine-readable event schema covering all contract event topics (#344).
- `treasury::tally` and `loans::vote_on_loan_proposal` read the active-member count (and policy/threshold) once per call (#355).
- `bench-cost.sh` now runs a real `test_budget_scaling` benchmark that also covers `request_loan_in_asset`, `repay_loan_in_asset` and `propose_withdrawal_in_asset` (#354).
- `get_treasury_accounting(asset)` reconciliation view returning `TreasuryAccounting { token_balance, staked, outstanding_principal }`. It is backed by a new per-asset `OutstandingPrincipal` storage entry, kept in step by disbursement, repayment (including overdue penalties), default and `seed_loans` (#288).
- Equivalence tests proving `request_loan`, `repay_loan` and `propose_treasury_withdrawal` leave the same ledger state and emit the same events as their `*_in_asset(base)` counterparts (#285).
- `loan_lifecycle_in_second_asset` test: request → edit → vote → disburse → partial repay → full repay → yield claim in a non-base asset, checking base and asset-B balances at every step (#286).
- `default_in_second_asset` test. Default slashing for loans in a non-base asset is documented as invariant A6 in `docs/INVARIANTS.md` (#287).
- `verify_commitment(voter, proposal_id, support, salt)` view and `get_commit_reveal_window(proposal_id)` view, backed by the same commitment and deadline helpers used by reveal/tally (#475, #476).
- `tre_exp` event on successful treasury proposal expiry, documented in the README and `docs/events.schema.json` (#477).

### Fixed
- `remove_token` now rejects an asset while any loan denominated in it is active (#273).
- `remove_token` now rejects removal while the treasury still holds any balance of that asset (#272).
- Audit all eight DAO token-transfer call sites and align state effects before external token interactions (#317).
- Enforce and regression-test admin-only `unpause`: the dedicated pauser may pause but cannot resume the DAO (#319).
- Fixed `vote_batch` to authenticate the voter once for the batch instead of re-authorizing per proposal vote, preventing auth collisions (#308).
- Bind private vote commitments to voter and proposal to prevent copied-hash reveals (#324).
- Enforce a three-open-proposal quota per member across treasury and loan proposals (#326).
- Prevent unstaking during open loan/treasury ballots and private commitments (#330).
- Apply treasury-favouring rounding for debt and interest calculations; document every division (#325).
- Added regression coverage that `reveal_vote` removes the stored commitment (#356).
- Validate consensus/treasury thresholds and quorum_bps are within 1..=10000: reject zero quorum_bps in proposed policy updates and verify set_consensus_threshold bounds (#320).
- `expire_treasury_proposal` is now idempotent: repeated keeper calls on an already-`Expired` proposal succeed without a duplicate event (#478).


### Added
- `make costs` and `tests/costs.rs`: per-entrypoint CPU, memory, ledger I/O, event size and fee estimates measured against the optimized wasm, plus how votes and `exit_dao` scale with membership; `make size` reports raw and optimized wasm size (#357, #358).
- Multi-token treasury (#236): `request_loan_in_asset`, `repay_loan_in_asset`, `propose_withdrawal_in_asset`, `get_treasury_balance_in_asset`, and the token whitelist (`propose_token_addition` / `execute_token_addition` behind the 48h timelock, `remove_token`, `is_token_whitelisted`, `get_whitelisted_tokens`). The base token is always whitelisted. New append-only error codes 83–87 and events `loan_asst`, `tre_asset`, `tok_prop`, `tok_add`, `tok_rem`.
- `cancel_loan_proposal` and `cancel_treasury_proposal` entrypoints for the existing #185 cancel logic.
- CI: a fast `check` job (`cargo check --locked --all-targets`) that `build-and-test` and `reproducible-build` depend on, so a broken merge fails within minutes (#238).
- Regression coverage ensuring `get_version` stays aligned with the Cargo package version (#372).
- `repay_loan_partial` for partial loan repayment.
- Loan and proposal lifecycle expiry, a `has_voted` view, name validation and an O(1) pull-based yield accumulator.
- `expire_treasury_proposal`, exposed in `lib.rs`; treasury proposals now expire after `VOTING_PERIOD`.
- Commit-reveal phase guards for private proposals (#57).
- `NotEditingPhase` and `DocumentTooLarge` errors; document hashes are limited to 64 bytes (#58, #59).
- `YieldRemainder` storage: `distribute_interest` carries rounding remainders and emits an event (#60).
- Loan terms are re-quoted at disbursement (#61).
- `bench-cost.sh` and a budget scaling benchmark; resource costs and the membership ceiling documented (#138).
- `generate-abi.sh` for a wasm-derived interface (#135).
- Property tests for exit-share, interest and staking-weight math.
- `util::isqrt`: an overflow-free integer square root over the whole `i128` range, backing the quadratic staking curve (#182).
- CI: `cargo clippy`, `cargo audit` and a `cargo-llvm-cov` coverage floor of 95% (#62).
- CI: MSRV job tests minimum supported Rust version (1.84) across all targets (#381).
- CI: markdown link checker validates all `.md` files for broken references (#383).
- PR template: checkboxes for "single issue only" and "ABI/event changes" (#382).

### Changed
- Pending yield has a single definition (`membership::compute_pending_yield`) shared by `get_pending_yield`, `claim_rewards` and `exit_dao` (#92).
- Loan proposals that time out without quorum are now stored as `status = Expired` (previously `Rejected`), matching `phase` and the `loan_exp` event; early rejection stays `Rejected` / `loan_rej` (#86).
- Defaulting a loan forfeits the borrower's unclaimed yield; yield stays equal-split and exit share stays contribution-weighted, now documented in the README (#89).
- **ABI:** config and registry views (`get_token`, `get_admins`, `is_admin`, `get_loan_policy`, `get_consensus_threshold`, `get_treasury_balance`, `calculate_loan_terms`, `calculate_exit_share`, `resolve_name`, `name_of`, `get_document`) now return `Result` and yield `NotInitialized` on an uninitialized contract; new `is_initialized` view (#88).
- `Member.join_ledger` renamed to `join_time` (#55).
- Staking boost is quadratic instead of linear: `voting_weight` is now `1 + min(isqrt(stake / 100), 5)`, so the *k*-th bonus vote costs `k² × 100` staked tokens and a whale can no longer buy proportional control (#182).
- Admin policy bounds tightened (#53, #54); loan eligibility checks and the `is_eligible_for_loan` signature corrected (#52, #56).
- Proposal document attachment is restricted to the proposal's own owner, and rejected after the editing phase (#59).

### Fixed
- Release wasm size: `crate-type = ["cdylib", "rlib"]` made `cargo build` skip `-C lto`, so `lto = true` never applied to `make build`, CI or `release.yml` builds. With `cdylib` only, plus `DataKey` kept out of the contract spec and a few host-side `Vec` lookups, the release wasm shrinks from 110,988 to 68,329 bytes and the optimized wasm from 85,833 to 58,372, and CPU per call on the optimized wasm drops by 4-14%. No entrypoint, event, error or public type changed (#357).
- `main` compiles again after the corrupted merge of #216 (#235); `cargo fmt` and `cargo clippy -D warnings` pass again.
- `total_members` is now decremented on exit, so exit shares no longer underpay (#11).
- Stake locking and `due_time` on loan approval (#46, #51); reveal period end (#45).
- CI workflow and formatting violations.

### Documentation
- README "Resource costs" table, with the membership ceiling corrected from the measured footprint: `exit_dao` exceeds the SDK's 100-entry mainnet snapshot at 86 members and votes at 90 (#358).
- ADR-0004 (loan due-date extension proposals) and ADR-0005 (interest discount from member loan history) (#359, #360).
- Removed the orphaned, never-compiled `src/test.rs`; its four unique tests were ported to `src/tests/` (#237).
- `SECURITY.md` with a coordinated disclosure policy.
- `docs/MIGRATION.md`, `docs/INVARIANTS.md`, `docs/LOAN_POLICY_GUIDE.md` and the pause/unpause incident runbook.
- `docs/THREAT_MODEL.md`: actors, trust assumptions and known gaps (#133).