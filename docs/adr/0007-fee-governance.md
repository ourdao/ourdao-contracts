# ADR-0007: Membership fee governance via timelock

- **Status:** Proposed
- **Date:** 2026-11-26

## Context

The membership fee is written once. `initialize` stores it under
`DataKey::MembershipFee` (instance storage), and `register_member` reads it
on every join: the joiner transfers `fee` to the contract and their `Member`
record starts with `contribution = share_balance = fee`. No entrypoint can
change it afterwards, and none returns it either, so clients learn the fee
from the `init` event or the `joined` events that followed.

A fee fixed for the life of the contract does not survive contact with the
real world. The base token's value moves, the treasury grows, and members
may want a higher price of entry once the DAO holds more, or a lower one to
grow. The open question from #362 and `LOAN_POLICY_GUIDE.md` is whether a
fee change should go through the policy timelock.

What already exists for policy changes (`admin.rs`):

- `propose_policy_update(caller, policy)` validates a full `LoanPolicy` and
  stores it as a `PendingPolicyUpdate { policy, proposed_at, execution_time }`
  with `execution_time = now + TIMELOCK_DURATION` (48 hours). Proposing again
  overwrites the pending update.
- `execute_policy_update(caller)` applies it once the delay has passed, and
  `cancel_policy_update(caller)` drops it. `set_loan_policy` is the same call
  as `propose_policy_update`. All four are admin-only and emit `pol_prop`,
  `policy` and `pol_canc`.

The constraints:

- **Storage compatibility.** `LoanPolicy` and `PendingPolicyUpdate` are
  stored `#[contracttype]` structs. ADR-0005 notes that a new field on
  `LoanPolicy` changes its encoding and breaks decoding of the entry already
  on the ledger. `PendingPolicyUpdate` embeds a `LoanPolicy`, so it has the
  same problem.
- **`LoanPolicy.membership_contribution` is not the fee.** It is a separate,
  validated value that loan code does not read; the fee lives only in
  `MembershipFee`. The two must not be conflated by a fee change.
- **Existing members.** `Member.contribution` is each member's pro-rata claim
  in `calculate_exit_share`. A fee change must never rewrite it.

## Decision

Fee changes **reuse the policy timelock's rules but get their own pending
slot and entrypoints.** They are admin-only, wait the same
`TIMELOCK_DURATION`, and only change what the next `register_member` call
charges.

### Entrypoints

| Entrypoint | Auth | Effect |
| --- | --- | --- |
| `propose_fee_update(caller, fee) -> Result<(), Error>` | admin | Stores a pending fee, executable after `TIMELOCK_DURATION`. Replaces any pending fee. |
| `execute_fee_update(caller) -> Result<(), Error>` | admin | Writes the pending fee to `MembershipFee` once the delay has passed. |
| `cancel_fee_update(caller) -> Result<(), Error>` | admin | Drops the pending fee. |
| `get_membership_fee() -> Result<i128, Error>` | none | The fee `register_member` charges now. `NotInitialized` before setup. |
| `get_pending_fee_update() -> Option<PendingFeeUpdate>` | none | The pending change, if any. |

### State

```rust
pub struct PendingFeeUpdate {
    pub fee: i128,
    pub proposed_at: u64,
    pub execution_time: u64,
}
```

Stored under a new `DataKey::PendingFeeUpdate` in instance storage, next to
`MembershipFee` and `PendingPolicyUpdate`, with the usual
`extend_instance` on write. `LoanPolicy`, `PendingPolicyUpdate` and `Member`
keep their shape.

### Rules

- `propose_fee_update` requires `fee > 0` (`InvalidAmount`), the same check
  `initialize` applies. It does not compare the fee with
  `membership_contribution`.
- `execute_fee_update` returns `NoPendingFeeUpdate` with nothing pending and
  `TimelockNotExpired` while `now < execution_time`, the same boundary
  `execute_policy_update` uses.
- None of the three is pause-gated, matching the policy entrypoints, so
  admins can still schedule or cancel a change while the DAO is paused.

### Why its own slot, not `propose_policy_update`

Putting the fee inside the policy flow would mean adding a field to
`LoanPolicy`, or wrapping `PendingPolicyUpdate` in a new type. Either
changes a stored encoding. It would also couple two unrelated decisions:
proposing a fee change would require resubmitting every loan parameter, and
because a new proposal overwrites the pending one, a fee proposal would
silently cancel a policy change already counting down, or the other way
around. A separate slot keeps each change reviewable on its own and leaves
the existing four policy entrypoints, their events and their errors
untouched.

What *is* shared is the part that matters for safety: the same admin gate,
the same `TIMELOCK_DURATION` constant (changing it moves both delays
together) and the same `TimelockNotExpired` error.

### Existing members

A fee change applies **only to joins after it executes.**

- `register_member` already reads `MembershipFee` at call time, so nothing
  in it changes. No `Member` record is rewritten, and `ActiveContributions`
  only moves when someone joins, exits or defaults, as today.
- A member who exits and later rejoins pays the fee in force on the day they
  rejoin, and their `contribution` restarts at that amount. This is already
  how rejoining works; it is listed here because a fee change makes it
  visible.
- Exit shares stay proportional to `contribution`. After an increase a new
  member's claim on the treasury is larger than an earlier member's; after a
  decrease it is smaller. That matches the capital each one put in.
- Yield is split one share per active member (`distribute_interest`), not by
  contribution. A lower fee therefore lets new members earn the same yield
  for less capital. That trade-off is the main thing members should weigh
  during the 48-hour window.

Worked example: fee 1,000, three members, treasury 3,000. The fee is raised
to 2,000 and a fourth member joins, so the treasury is 5,000 and active
contributions total 5,000.

| Member | Contribution | Exit share |
| --- | --- | --- |
| A, B, C (joined before) | 1,000 each | 1,000 each |
| D (joined after) | 2,000 | 2,000 |

Each of the four receives a quarter of every later interest payment.

### Events

| Topic | Data |
| --- | --- |
| `fee_prop` | `(caller, fee, execution_time)` |
| `fee_set` | `(old_fee, new_fee)` |
| `fee_canc` | `caller` |

`joined` already carries the fee each member paid, so indexers can check
every join against the fee history.

### Errors

Appended after the last code in use when this is implemented, never
renumbered:

- `NoPendingFeeUpdate`: `execute_fee_update` or `cancel_fee_update` with
  nothing pending.

Existing errors cover the rest: `NotAdmin`, `InvalidAmount`,
`TimelockNotExpired`, `NotInitialized`.

## Alternatives considered

- **Add the fee to `LoanPolicy` and use `propose_policy_update`.** One flow
  for every parameter, but it breaks decoding of the stored policy and
  couples fee and policy changes as described above. Rejected.
- **Immediate admin setter, no timelock.** Simplest, but one admin key could
  reprice entry with no notice. Joiners are partly protected already, because
  the fee transfer is inside their signed authorization and a changed amount
  fails it, yet existing members would still get no chance to react to a
  change in yield dilution. Rejected for the same reason the policy has a
  timelock.
- **A member vote, like treasury proposals.** Puts the decision with the
  people it affects, but no other configuration value is voted on today, and
  it would need its own proposal type, quorum rules and `ProposalKind`
  variant. Deferred. If parameter voting is ever added it should cover the
  policy and the fee together.
- **Keep the fee immutable and redeploy to change it.** ADR-0001 makes a
  redeploy a full migration. Far too heavy for one number.
- **Recompute existing contributions on a fee change.** Would move exit
  shares between members who did nothing. Rejected.

## Consequences

- **ABI additions:** five entrypoints, one type, three events and one error.
  Nothing existing is renamed or renumbered; the backend indexer needs the
  new events.
- **Storage:** one new instance key, present only while a change is
  pending.
- **Clients** can read the fee directly instead of replaying events, and
  should show a pending change with its `execution_time` to prospective
  members.
- **Security:** fee changes gain the same 48-hour window policy changes
  have. They remain admin decisions, so the admin trust assumptions in
  `docs/THREAT_MODEL.md` apply unchanged.
- **Follow-up:** implementation with tests for the timelock boundary
  (executable exactly at `execution_time`, not one second before), cancel
  and re-propose, a pending fee change and a pending policy change that do
  not affect each other, a join before and after execution paying the old
  and new fee, existing `contribution` values unchanged, a rejoin paying the
  new fee, and the new events.
