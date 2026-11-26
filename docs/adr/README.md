# Architecture Decision Records

ADRs capture decisions that affect the contract’s architecture, public ABI,
security model, or operational lifecycle. Accepted decisions are immutable
history: supersede an old record with a new ADR rather than rewriting it.

## Template

Copy [`TEMPLATE.md`](./TEMPLATE.md), choose the next number, and add the
record to the index below. Keep the status (`Proposed`, `Accepted`,
`Superseded`, or `Deprecated`) and link the superseding ADR when applicable.

## Choosing the next ADR number (#443)

ADR numbers are **append-only**, following the same discipline
[`CONTRIBUTING.md`](../../CONTRIBUTING.md#contract-specific-rules) applies to
error codes:

- The next ADR takes **one more than the highest number ever used or
  reserved** — the highest of the index table and the
  [unused numbers](#unused-and-reserved-numbers) table below — never the
  lowest free gap.
- **Never reuse a skipped number.** A gap stays a gap forever, because other
  issues, PRs, and commits may already cite that number.
- **Never renumber** an existing ADR, even to close a gap.
- If a number is abandoned, add it to the unused numbers table with the
  reason, rather than leaving a silent hole in the index.

## Review Gate for ADR Status Changes (#520)

Per ADR-0001, accepted decisions are immutable architecture history. Moving an ADR's status from `Proposed` to `Accepted` requires explicit sign-off from at least two project maintainers recorded in the approving PR review. Changes to `docs/adr/` are protected via `.github/CODEOWNERS`.

## Index

| ADR | Title | Status |
| --- | --- | --- |
| [0001](./0001-immutable-contract-vs-upgradeable.md) | Immutable contract vs upgradeable contract | Accepted |
| [0004](./0004-loan-extension.md) | Loan due-date extension proposals | Proposed |
| [0005](./0005-reputation-rate.md) | Interest discount from member loan history | Proposed |
| [0008](./0008-early-repayment.md) | Early repayment discount | Proposed |
| [0009](./0009-max-open-proposals-policy.md) | Evaluation of MAX_OPEN_PROPOSALS_PER_MEMBER in LoanPolicy | Proposed |
| [0010](./0010-multi-asset-exit.md) | Multi-asset member exit payouts | Proposed |
| [0011](./0011-per-asset-yield.md) | Per-asset yield accounting | Proposed |

## Unused and reserved numbers

Every gap between 0001 and the highest numbered ADR is listed here, so a
missing number is never ambiguous. None of these numbers may be reassigned to
a different decision.

| ADR | State | Reason |
| --- | --- | --- |
| 0002 | Unused (retired) | Never created. No issue, PR, or commit in this repository references an ADR-0002; the number was skipped when later ADRs were numbered. Retired permanently — do not reuse. |
| 0003 | Unused (retired) | Never created. No issue, PR, or commit in this repository references an ADR-0003; the number was skipped when later ADRs were numbered. Retired permanently — do not reuse. |
| 0006 | Reserved | Stake-as-collateral loans. Cited by the closed design issue #361 but never written; tracked by #435. Only that ADR may use this number. |
| 0007 | Reserved | Membership fee governance via timelock. Cited by the closed design issue #362 but never written; tracked by #436. Only that ADR may use this number. |

