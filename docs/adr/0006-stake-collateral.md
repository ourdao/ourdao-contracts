# ADR-0006: Stake-as-collateral loans

- **Status:** Proposed
- **Date:** 2026-11-26

## Context

Every loan today is unsecured. The only thing a defaulting borrower loses is
`default_penalty_bps` of their `contribution`, which `mark_loan_defaulted`
slashes once the grace period has passed. On a loan that is large next to
the membership fee, that slash recovers very little of what the treasury
lent.

Many members already hold tokens in the contract as stake. `stake` moves base
tokens into the DAO, records them under `DataKey::Stake(member)` and in
`TotalStaked`, and buys a capped quadratic boost to voting weight
(`util::stake_boost`: `isqrt(stake / STAKE_WEIGHT_UNIT)`, at most
`MAX_STAKE_BONUS`). Staked funds are excluded from the spendable treasury
(`util::treasury_balance_in_asset` subtracts `TotalStaked` for the base
token), so they are never lent out or paid as yield. A member willing to put
that stake behind a loan gives the DAO something to recover from, and the
DAO can reasonably charge a lower rate for it.

The constraints:

- **Storage compatibility.** `Loan`, `LoanProposal`, `LoanPolicy` and `Member`
  are stored `#[contracttype]` structs. Adding a field changes their encoding
  (see ADR-0004), so collateral has to live under its own keys.
- **One asset.** Stake only exists in the base token. Loans in other
  whitelisted assets (#236) would need a price to value base-token
  collateral against them, and the contract has no oracle.
- **Members' floor.** `min_interest_rate` is the least members earn on any
  loan. A collateral discount must not go below it, the same rule ADR-0005
  applies to its reputation discount.
- **Unstaking rules.** `unstake` already enforces `cooldown_period` since the
  last stake and refuses while the member has an open loan proposal or a
  ballot still open on a proposal they voted on. Locked collateral needs one
  more check of the same kind.

## Decision

A borrower may **pledge part of their stake as collateral** when requesting a
base-token loan. The pledge lowers the quoted rate, is locked while the loan
is active, is released on repayment, and is seized first on default.

### Entrypoints

| Entrypoint | Auth | Effect |
| --- | --- | --- |
| `request_loan_with_collateral(borrower, amount, collateral, metadata_cid) -> Result<u32, Error>` | borrower | `request_loan` in the base token with `collateral` pledged. |
| `get_loan_collateral(loan_id) -> i128` | none | Collateral pledged for a proposal or loan; 0 if none. |
| `get_locked_stake(member) -> i128` | none | Stake currently locked behind the member's active loan; 0 if none. |

`request_loan` and `request_loan_in_asset` keep their behaviour: no
collateral. There is no way to add collateral to a non-base asset loan, so
no new error is needed for that case.

### State

| Key | Storage | Value |
| --- | --- | --- |
| `DataKey::LoanCollateral(u32)` | persistent | Pledged amount, keyed by proposal id (and so by loan id, since `loan.id == proposal.id`). Written at request, removed when the loan closes. |
| `DataKey::LockedStake(Address)` | persistent | Stake locked behind the member's one active loan. Written at disbursement, removed on repayment or default. |

Both follow the usual TTL extension on read and write. `Member` records no
loan id, which is why the lock is keyed by member as well: `unstake` needs it
in a single read. `has_active_loan` already limits a member to one active
loan, so one lock per member is enough.

### Rules

**At request.** In addition to every `request_loan` check:

1. `0 < collateral <= amount`, else `InvalidAmount`. Pledging more than the
   principal locks stake without lowering the rate any further.
2. `collateral <= get_stake(borrower)`, else `InsufficientCollateral`.

Nothing is locked yet. While the proposal is open, `unstake` already refuses
for the borrower (#175), so the pledged stake cannot leave before the vote
ends.

**Rate.** The pledge discounts the premium above `min_interest_rate` by the
share of the principal it covers:

```text
coverage_bps  = floor(collateral * BASIS_POINTS / amount)          (at most BASIS_POINTS)
premium       = rate - min_interest_rate
discount      = floor(premium * coverage_bps / BASIS_POINTS)
borrower_rate = rate - discount
```

Both roundings go down, which keeps the rate higher and so favours members. A
fully covered loan pays exactly `min_interest_rate`, never less. If
ADR-0005 is implemented, its reputation discount is applied first and this
one discounts the premium that remains, so the two can never stack below the
floor. `edit_loan_proposal` re-quotes with the stored collateral against the
new amount, and rejects the edit with `InvalidAmount` if the new amount is
below the collateral.

**At disbursement.** `approve_and_disburse` re-checks
`collateral <= get_stake(borrower)` and, when it holds, writes
`LockedStake(borrower) = collateral`. A borrower can unstake between the
vote passing and a delayed disbursement (`ApprovedPendingDisbursement` is not
`Pending`), so this check is needed. If it fails the loan is not disbursed,
exactly as when the treasury is short: the proposal stays
`ApprovedPendingDisbursement` and a later `disburse_approved_loan` can retry
once the stake is back.

**While active.** `unstake` adds one rule:
`amount <= stake - locked_stake`, else `CollateralLocked`. `exit_dao` already
refuses while `has_active_loan` is set, so locked stake cannot leave through
an exit either.

**On repayment.** When `repay_loan_internal` flips the loan to `Repaid`, it
removes `LockedStake(borrower)` and `LoanCollateral(loan_id)`. Partial
payments release nothing; the lock is all or nothing.

**On default.** `mark_loan_defaulted` seizes collateral before it applies the
contribution slash:

```text
owed     = total_repayment - amount_repaid
seized   = min(collateral, owed)
released = collateral - seized
```

The seized amount is taken from `Stake(borrower)` and from `TotalStaked`.
The tokens are already in the contract, so no transfer happens: lowering
`TotalStaked` is what moves them into the spendable treasury. The seizure is
applied to the debt the same way a repayment would be, interest first, and
its interest share goes to `distribute_interest` as yield. Whatever is left
of the pledge (`released`) stays as ordinary stake, and the lock is removed.

The existing contribution slash then **supplements** the seizure only when
the seizure left part of the debt unrecovered (`seized < owed`). When the
collateral covers the whole debt the treasury has lost nothing, so
`default_penalty_bps` is not applied to `contribution`. The loan is still
marked `Defaulted` and still counts as a default in `MemberLoanStats`, so a
borrower cannot use a fully covered loan as a free way out: they lose the
stake, the stake's voting boost and their repayment history.

### Voting weight

Collateralized stake **keeps its voting weight**. `util::voting_weight` is
unchanged and does not read `LockedStake`.

- The stake is still the member's and still at risk, more so than before.
  Taking its weight away would penalise the members who give the DAO the
  most protection.
- The conflict of interest that matters is already handled: a borrower may
  not vote on their own loan (`BorrowerCannotVote`) or on their own
  extension (ADR-0004).
- Reading the lock inside `voting_weight` would add a ledger read per
  delegating member on every vote, against the cost ceiling the README
  "Resource costs" section describes.

Locked stake is, however, set aside from the treasury the same way all stake
already is: it stays in `TotalStaked` and so outside
`treasury_balance_in_asset`. It can never be lent out while it backs a loan.
Only seizure moves it into the treasury, and that also removes its boost.

### Interaction with ADR-0004 and ADR-0008

- **Extensions (ADR-0004).** The lock simply stays in place until the new
  `due_time`. The extension fee is priced at the loan's own, already
  discounted, rate, so a secured loan's extension is cheaper too.
- **Early repayment (ADR-0008).** A payoff before `due_time` releases the
  collateral like any other full repayment. The rebate is computed from the
  loan's `interest_total`, which the collateral discount has already lowered;
  the two compose without either one reading the other's state.

### Worked example

`min_interest_rate` 5%, `max_interest_rate` 20%, a loan of 1,000 that is 25%
of the treasury (undiscounted rate 875 bps, premium 375 bps):

| Collateral | 0 | 250 | 500 | 1,000 |
| --- | --- | --- | --- | --- |
| Coverage (bps) | 0 | 2,500 | 5,000 | 10,000 |
| Rate (bps) | 875 | 782 | 688 | 500 |
| Interest | 88 | 79 | 69 | 50 |
| `total_repayment` | 1,088 | 1,079 | 1,069 | 1,050 |

Default after 300 has been repaid:

| | Collateral 500 | Collateral 1,000 |
| --- | --- | --- |
| `owed` | 1,069 - 300 = 769 | 1,050 - 300 = 750 |
| `seized` | 500 | 750 |
| `released` | 0 | 250 |
| Interest in the seizure | 0 (all 69 already paid) | 0 (all 50 already paid) |
| Contribution slash | applied, 269 unrecovered | not applied |

### Events

| Topic | Data |
| --- | --- |
| `coll_lock` | `(loan_id, borrower, collateral)`, at disbursement |
| `coll_rel` | `(loan_id, borrower, collateral)`, on repayment |
| `coll_seiz` | `(loan_id, borrower, seized, released)`, on default, before `loan_dflt` |

`loan_req` is unchanged; indexers read the pledge from `coll_lock` or
`get_loan_collateral`.

### Errors

Appended after the last code in use when this is implemented, never
renumbered:

- `InsufficientCollateral`: the pledge is larger than the borrower's stake at
  request or at disbursement.
- `CollateralLocked`: an `unstake` would take the stake below the locked
  amount.

## Alternatives considered

- **Collateral in any whitelisted asset.** Needs a price for each pair and
  so an oracle, which adds a trusted party. Deferred until the multi-asset
  work (#236) has one.
- **Higher loan limit instead of a lower rate.** Raising
  `max_loan_to_treasury_ratio` for secured loans would let one borrower tie
  up more of the treasury, the risk that cap exists to bound. A rate discount
  rewards the pledge without touching treasury exposure.
- **Collateral replaces the contribution slash outright.** A borrower
  pledging a token amount could then default on a large loan and lose only
  that token amount. Applying the slash whenever debt is left over keeps a
  partly secured loan at least as costly to default on as an unsecured one.
- **Slash both, always.** Charges a fully secured defaulter twice for a loss
  the treasury never took. Rejected.
- **Strip voting weight from locked stake.** Discussed under Voting weight:
  it penalises the safest borrowers and costs a read per delegator per vote.
- **Lock at request time.** Needs a release path for every way a proposal
  can end without a loan (rejection, expiry, cancellation). The existing
  unstake block (#175) already covers the open-proposal window, so locking at
  disbursement is enough.
- **New fields on `Loan` or `LoanProposal`.** Breaks decoding of stored
  entries. Rejected for the same reason as in ADR-0004.

## Consequences

- **ABI additions:** three entrypoints, three events and two errors. Nothing
  existing is renamed or renumbered; the backend indexer needs the new
  events.
- **Storage:** two new persistent key families, at most one entry per
  secured loan and one per borrowing member.
- **Cost:** `unstake` reads one more key; disbursement, full repayment and
  default each read and write the two collateral keys. `voting_weight` is
  unchanged.
- **Treasury accounting:** a seizure lowers `TotalStaked` without a
  transfer. `get_treasury_accounting` reports `staked` from `TotalStaked`, so
  it stays consistent, but invariant tests on `token_balance == staked +
  spendable` must include the seizure path.
- **Risk:** the pledge is valued in the same token it secures, so there is
  no price risk, but the borrower keeps the borrowed funds on default. The
  DAO recovers at most the pledge.
- **Follow-up:** implementation with tests for each rule above, including
  the worked examples, the floor at `min_interest_rate`, a disbursement that
  fails because the borrower unstaked after approval, `CollateralLocked` on
  unstake, release on payoff, seizure capped at `owed`, the slash applied
  only when debt is left over, and the voting weight of a locked stake.
