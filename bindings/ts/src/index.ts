import { Buffer } from "buffer";
import { Address } from "@stellar/stellar-sdk";
import {
  AssembledTransaction,
  Client as ContractClient,
  ClientOptions as ContractClientOptions,
  MethodOptions,
  Result,
  Spec as ContractSpec,
} from "@stellar/stellar-sdk/contract";
import type {
  u32,
  i32,
  u64,
  i64,
  u128,
  i128,
  u256,
  i256,
  Option,
  Timepoint,
  Duration,
} from "@stellar/stellar-sdk/contract";
export * from "@stellar/stellar-sdk";
export * as contract from "@stellar/stellar-sdk/contract";
export * as rpc from "@stellar/stellar-sdk/rpc";

if (typeof window !== "undefined") {
  //@ts-ignore Buffer exists
  window.Buffer = window.Buffer || Buffer;
}




/**
 * Every failure mode the DAO can return. Numeric codes are stable and part of
 * the contract's public ABI, so append new variants rather than renumbering.
 */
export const Errors = {
  1: {message:"AlreadyInitialized"},
  2: {message:"NotInitialized"},
  3: {message:"InvalidThreshold"},
  4: {message:"InvalidAmount"},
  5: {message:"InvalidLoanPolicy"},
  6: {message:"Paused"},
  7: {message:"NotPaused"},
  10: {message:"NotAuthorized"},
  11: {message:"NotAdmin"},
  12: {message:"NotMember"},
  13: {message:"AlreadyAdmin"},
  14: {message:"AlreadyMember"},
  15: {message:"CannotRemoveLastAdmin"},
  20: {message:"MemberNotActive"},
  21: {message:"HasActiveLoan"},
  30: {message:"ProposalNotFound"},
  31: {message:"NotBorrower"},
  32: {message:"NotInEditingPhase"},
  33: {message:"NotInVotingPhase"},
  34: {message:"VotingEnded"},
  35: {message:"AlreadyVoted"},
  36: {message:"NotEligibleForLoan"},
  37: {message:"CooldownActive"},
  38: {message:"LoanNotFound"},
  39: {message:"LoanNotActive"},
  40: {message:"ExceedsTreasuryRatio"},
  41: {message:"InsufficientTreasury"},
  42: {message:"LoanNotOverdue"},
  50: {message:"TreasuryProposalNotFound"},
  60: {message:"NameTaken"},
  /**
   * Reserved: never returned. No contract code path produces it; kept
   * because codes are append-only public ABI (#252).
   */
  61: {message:"NameNotFound"},
  62: {message:"NoStake"},
  63: {message:"InsufficientStake"},
  64: {message:"NoCommitment"},
  65: {message:"CommitmentMismatch"},
  66: {message:"AlreadyRevealed"},
  67: {message:"NothingToClaim"},
  70: {message:"ProposalNotExpired"},
  71: {message:"InvalidName"},
  72: {message:"NotYetRevealed"},
  /**
   * Caller is not the proposal's proposer/borrower and may not modify its
   * attached document (#21).
   */
  73: {message:"NotProposalOwner"},
  74: {message:"DocumentTooLarge"},
  /**
   * The token passed to `initialize` is not a contract implementing the
   * token interface (#115).
   */
  75: {message:"InvalidToken"},
  /**
   * Timelock delay has not expired yet (#192).
   */
  76: {message:"TimelockNotExpired"},
  /**
   * No policy update is currently pending (#192).
   */
  77: {message:"NoPendingPolicy"},
  /**
   * Proposal metadata CID is invalid (#194).
   */
  78: {message:"InvalidMetadataCid"},
  /**
   * Invalid delegation target (e.g. self-delegation) (#188).
   */
  79: {message:"InvalidDelegation"},
  80: {message:"InvalidProposal"},
  81: {message:"AlreadyPauser"},
  82: {message:"NotPauser"},
  /**
   * The asset is not on the treasury whitelist (#236).
   */
  83: {message:"TokenNotWhitelisted"},
  /**
   * The asset is already on the treasury whitelist (#236).
   */
  84: {message:"TokenAlreadyWhitelisted"},
  /**
   * No addition is pending for this asset (#236).
   */
  85: {message:"NoPendingTokenAddition"},
  /**
   * The asset does not match the one the loan was issued in (#236).
   */
  86: {message:"AssetMismatch"},
  /**
   * The base token set at initialization can't be removed (#236).
   */
  87: {message:"CannotRemoveBaseToken"},
  /**
   * Seeding is locked; `lock_seeding` has already been called (#332).
   */
  88: {message:"SeedingLocked"},
  /**
   * Seeding is already locked; calling `lock_seeding` again is a no-op error (#333).
   */
  89: {message:"SeedingAlreadyLocked"},
  /**
   * Member already has the maximum number of simultaneously open proposals.
   */
  90: {message:"TooManyOpenProposals"},
  /**
   * A borrower cannot vote on their own loan proposal (#327).
   */
  91: {message:"BorrowerCannotVote"},
  /**
   * A whitelisted token with treasury funds cannot be removed (#272).
   */
  92: {message:"TokenHasBalance"},
  /**
   * A token with active loans cannot be removed (#273).
   */
  93: {message:"TokenHasActiveLoans"},
  /**
   * A returning member still has a loan outstanding or a defaulted loan on
   * record, so they may not rejoin and reset their standing (#168).
   */
  94: {message:"DefaultedLoanOutstanding"},
  /**
   * The treasury asset whitelist has reached its bounded maximum (#282).
   */
  95: {message:"WhitelistFull"},
  /**
   * An inbound token transfer delivered less than the requested amount (#283).
   */
  96: {message:"TransferAmountMismatch"},
  /**
   * The voter joined (or rejoined) the DAO after the proposal was created,
   * so they are outside the electorate its quorum was fixed against (#73).
   */
  97: {message:"JoinedAfterProposal"},
  /**
   * Admin list has reached MAX_ADMINS; remove one before adding another (#68).
   */
  98: {message:"TooManyAdmins"},
  /**
   * Batch vote call exceeded the maximum number of votes allowed per call.
   */
  99: {message:"TooManyVotesInBatch"},
  /**
   * Checked `i128` arithmetic overflowed; the operation was rejected.
   */
  100: {message:"ArithmeticOverflow"}
}


export interface Loan {
  amount_repaid: i128;
  borrower: string;
  /**
 * `LoanPolicy.default_grace_period` in force when this loan was
 * disbursed. Default terms are part of the obligation the borrower
 * accepted, so a later policy change must not alter them (#70).
 */
default_grace_period: u64;
  /**
 * `LoanPolicy.default_penalty_bps` in force when this loan was disbursed.
 * See `default_grace_period`.
 */
default_penalty_bps: u32;
  due_time: u64;
  id: u32;
  interest_rate: u32;
  principal: i128;
  start_time: u64;
  status: LoanStatus;
  total_repayment: i128;
}


export interface Member {
  /**
 * Loans currently outstanding (disbursed and not yet repaid or defaulted).
 */
active_loans: u32;
  address: string;
  contribution: i128;
  has_active_loan: boolean;
  join_time: u64;
  last_loan_time: u64;
  /**
 * Lifetime count of this member's loans that reached full repayment.
 */
repaid_loans: u32;
  share_balance: i128;
  status: MemberStatus;
  /**
 * Lifetime count of loans disbursed to this member.
 */
total_loans: u32;
}


/**
 * Computed loan terms returned by the read-only quote helper.
 * 
 * A quote is only honourable when `amount <= max_loan`. Above that,
 * `request_loan` rejects the amount with `ExceedsTreasuryRatio`, and the
 * rate shown is not one anyone can borrow at (#80).
 */
export interface LoanTerms {
  duration: u64;
  interest_rate: u32;
  /**
 * Largest principal `request_loan` would accept right now:
 * `treasury_balance × max_loan_to_treasury_ratio / 10_000`, or 0 when the
 * treasury is empty or negative (staked funds exceeding the token
 * balance).
 */
max_loan: i128;
  total_repayment: i128;
}


/**
 * Headline DAO state returned by `get_dao_summary` (#339).
 */
export interface DaoSummary {
  active_members: u32;
  consensus_threshold: u32;
  paused: boolean;
  total_members: u32;
  total_staked: i128;
  /**
 * Spendable treasury in the base token; same value as `get_treasury_balance`.
 */
treasury_balance: i128;
  /**
 * Spendable treasury in every whitelisted asset, base token first.
 */
treasury_balances: Array<AssetBalance>;
  version: string;
}


/**
 * Tunable lending parameters. Durations are in ledger seconds; rates in bps.
 */
export interface LoanPolicy {
  cooldown_period: u64;
  /**
 * Extra time past `Loan.due_time` before a loan becomes markable as defaulted.
 */
default_grace_period: u64;
  /**
 * Basis-points of the defaulting borrower's `contribution` slashed on default.
 */
default_penalty_bps: u32;
  editing_period: u64;
  max_interest_rate: u32;
  max_loan_duration: u64;
  max_loan_to_treasury_ratio: u32;
  membership_contribution: i128;
  min_interest_rate: u32;
  min_membership_duration: u64;
  quorum_bps: u32;
  treasury_threshold: u32;
  voting_period: u64;
}

export type LoanStatus = {tag: "Active", values: void} | {tag: "Repaid", values: void} | {tag: "Defaulted", values: void};


/**
 * Spendable treasury balance in one whitelisted asset (#339).
 */
export interface AssetBalance {
  asset: string;
  balance: i128;
}


export interface LoanProposal {
  /**
 * Active-member count when the proposal was created. Quorum and the
 * early-rejection bound are computed from this, not the live count, so
 * membership churn during the vote cannot move the bar (#73).
 */
active_members: u32;
  against_votes: i128;
  amount: i128;
  borrower: string;
  created_at: u64;
  duration: u64;
  editing_period_end: u64;
  for_votes: i128;
  id: u32;
  interest_rate: u32;
  /**
 * Timestamp of the last `edit_loan_proposal` call (`None` if never edited).
 */
last_edited_at: Option<u64>;
  metadata_cid: Option<string>;
  phase: ProposalPhase;
  status: ProposalStatus;
  total_repayment: i128;
  votes_cast: u32;
  voting_period: u64;
}

export type MemberStatus = {tag: "ActiveMember", values: void} | {tag: "Inactive", values: void};


export interface ProposalVote {
  proposal_id: u32;
  vote: boolean;
}


export interface SeedDocEntry {
  content_hash: Buffer;
  kind: ProposalKind;
  proposal_id: u32;
}

export type ProposalPhase = {tag: "Editing", values: void} | {tag: "Voting", values: void} | {tag: "Executed", values: void} | {tag: "Expired", values: void};


export interface SeedNameEntry {
  name: string;
  owner: string;
}


/**
 * Everything the DAO knows about one member, returned by
 * `get_member_overview` in a single call.
 * 
 * Convenience aggregation view combining independently queryable fields:
 * - `member`: general member status and metadata
 * - `stake`: matches `get_stake`
 * - `pending_yield`: matches `get_pending_yield`
 * - `loan_stats`: matches `get_member_loan_stats`
 * - `name`: matches `name_of`
 * - `delegate`: matches `get_delegate`
 * 
 * Maintenance obligation: Every future addition to any of the underlying
 * individual member views must have a matching field added here to ensure
 * the aggregate view remains in sync.
 */
export interface MemberOverview {
  /**
 * Address this member delegated their vote to, if any.
 */
delegate: Option<string>;
  loan_stats: MemberLoanStats;
  member: Member;
  /**
 * Name registered to this address, if any.
 */
name: Option<string>;
  /**
 * Interest yield accrued and not yet claimed via `claim_rewards`.
 */
pending_yield: i128;
  /**
 * Tokens the member has staked for a voting-weight bonus.
 */
stake: i128;
}

export type ProposalStatus = {tag: "Pending", values: void} | {tag: "ApprovedPendingDisbursement", values: void} | {tag: "Approved", values: void} | {tag: "Rejected", values: void} | {tag: "Executed", values: void} | {tag: "Expired", values: void} | {tag: "Cancelled", values: void};


export interface SeedStakeEntry {
  address: string;
  amount: i128;
}


/**
 * Pending yield for a member. `amount` is stored as the negative yield snapshot
 * so the existing `get_pending_yield` formula `(accumulator - snapshot).max(0)`
 * reconstructs the correct value from a fresh accumulator of 0 (#332).
 */
export interface SeedYieldEntry {
  address: string;
  amount: i128;
}


/**
 * Structured event emitted when a borrower edits loan proposal terms.
 * Captures prior and updated terms plus the edit timestamp.
 */
export interface LoanTermsEdited {
  borrower: string;
  edited_at: u64;
  new_amount: i128;
  prev_amount: i128;
  prev_total_repayment: i128;
  proposal_id: u32;
  total_repayment: i128;
}


/**
 * On-chain credit track record for a member. Returned by
 * `get_member_loan_stats` as an O(1) read of the member record.
 */
export interface MemberLoanStats {
  active_loans: u32;
  repaid_loans: u32;
  total_loans: u32;
}


export interface TreasuryProposal {
  /**
 * Active-member count when the proposal was created. See
 * `LoanProposal.active_members` (#73).
 */
active_members: u32;
  against_votes: i128;
  amount: i128;
  created_at: u64;
  destination: string;
  for_votes: i128;
  id: u32;
  /**
 * When true, votes must be committed then revealed (commit-reveal privacy).
 */
private: boolean;
  proposer: string;
  reason: string;
  status: ProposalStatus;
  treasury_threshold: u32;
  votes_cast: u32;
  voting_period: u64;
}


/**
 * Per-asset reconciliation figures returned by `get_treasury_accounting`
 * (#288), so auditors can compare what the contract accounts for against
 * what it actually holds.
 */
export interface TreasuryAccounting {
  /**
 * Principal still owed on `Active` loans in the asset. Lent out, so not
 * part of `token_balance`.
 */
outstanding_principal: i128;
  /**
 * Portion of `token_balance` held as member stake. Staking only happens
 * in the base token, so this is zero for every other asset.
 */
staked: i128;
  /**
 * The contract's actual balance of the asset, as reported by its token.
 */
token_balance: i128;
}


/**
 * Pending policy update undergoing timelock delay (#192).
 */
export interface PendingPolicyUpdate {
  execution_time: u64;
  policy: LoanPolicy;
  proposed_at: u64;
}


/**
 * Structured event emitted upon staking reward claim (#193).
 */
export interface StakingRewardClaimed {
  amount: i128;
  member: string;
  timestamp: u64;
}

/**
 * Distinguishes which proposal family a shared reference (docs, commits) targets.
 */
export type ProposalKind = {tag: "Loan", values: void} | {tag: "Treasury", values: void};

export interface Client {
  /**
   * Construct and simulate a pause transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Pauses contract state changes. The pauser or an allowed caller must
   * authorize the call and emit the pause event on success.
   */
  pause: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a stake transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Stakes base treasury assets to increase `member`'s voting weight.
   * `member` must authorize and have sufficient spendable balance; returns
   * stake/amount errors and emits the stake event.
   */
  stake: ({member, amount}: {member: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a name_of transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the name registered to `owner`, or `None` when none is
   * registered. Read-only; may return `NotInitialized`.
   */
  name_of: ({owner}: {owner: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Option<string>>>>

  /**
   * Construct and simulate a unpause transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Resumes contract state changes. The pauser or an allowed caller must
   * authorize the call.
   */
  unpause: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a unstake transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Releases staked base treasury assets from `member`. `member` must
   * authorize and have enough stake; emits the unstake event.
   */
  unstake: ({member, amount}: {member: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a exit_dao transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Exits `member`, settling stake and accrued yield and reducing their
   * contribution share. `member` must authorize and have no active loan.
   * Returns membership, balance, or arithmetic errors and emits exit events.
   */
  exit_dao: ({member}: {member: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_loan transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns a loan by id, or `None` if it does not exist.
   */
  get_loan: ({loan_id}: {loan_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Option<Loan>>>

  /**
   * Construct and simulate a is_admin transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Reports whether `address` is an admin. Returns `NotInitialized` before setup.
   */
  is_admin: ({address}: {address: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<boolean>>>

  /**
   * Construct and simulate a add_admin transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Adds an admin. The caller must be an existing admin and authorize the
   * call. May emit an admin-change event and returns an authorization or
   * configuration error when the change is not allowed.
   */
  add_admin: ({caller, admin}: {caller: string, admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_stake transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns `member`'s current base-asset stake balance.
   */
  get_stake: ({member}: {member: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a get_token transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the base treasury asset. Returns `NotInitialized` before setup.
   */
  get_token: (options?: MethodOptions) => Promise<AssembledTransaction<Result<string>>>

  /**
   * Construct and simulate a has_voted transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns whether `voter` has cast a (possibly not-yet-revealed) vote
   * on the given proposal. For public (non-private) proposals this is a
   * simple check against the vote record. For commit-reveal treasury
   * proposals it checks whether the voter has committed (which is the
   * earliest point at which a double-vote is prevented).
   */
  has_voted: ({kind, proposal_id, voter}: {kind: ProposalKind, proposal_id: u32, voter: string}, options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a is_member transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Reports whether `address` is an active registered member.
   */
  is_member: ({address}: {address: string}, options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a is_paused transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Reports whether contract state changes are currently paused.
   */
  is_paused: (options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a seed_docs transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Seeds historical content-hash documents. Admin-only before
   * `lock_seeding`; returns `SeedingLocked` after migration closes.
   */
  seed_docs: ({caller, docs}: {caller: string, docs: Array<SeedDocEntry>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_admins transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the current admin set. Returns `NotInitialized` before setup.
   */
  get_admins: (options?: MethodOptions) => Promise<AssembledTransaction<Result<Array<string>>>>

  /**
   * Construct and simulate a get_member transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the member record for `address`, or `None` if unregistered.
   */
  get_member: ({address}: {address: string}, options?: MethodOptions) => Promise<AssembledTransaction<Option<Member>>>

  /**
   * Construct and simulate a initialize transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * One-time setup. `admins` bootstrap governance, `consensus_threshold` is
   * in basis points (e.g. 5100 = 51%), and `token` is the asset all DAO
   * value flows through.
   */
  initialize: ({admins, consensus_threshold, membership_fee, token, policy}: {admins: Array<string>, consensus_threshold: u32, membership_fee: i128, token: string, policy: LoanPolicy}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a list_loans transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Lists up to `limit` loans within the proposal-ID window starting at
   * `start`. Missing loans are skipped and do not extend the window.
   */
  list_loans: ({start, limit}: {start: u32, limit: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Array<Loan>>>

  /**
   * Construct and simulate a repay_loan transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Repays the outstanding balance of a base-asset loan. `borrower` must
   * authorize; invalid, inactive, or insufficient-payment conditions return
   * errors and emit a loan-repayment event on success.
   */
  repay_loan: ({borrower, loan_id}: {borrower: string, loan_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a seed_loans transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Seeds historical loan records. Admin-only before `lock_seeding`;
   * returns `SeedingLocked` after migration closes.
   */
  seed_loans: ({caller, loans}: {caller: string, loans: Array<Loan>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a seed_names transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Seeds historical name-registry records. Admin-only before
   * `lock_seeding`; returns `SeedingLocked` after migration closes.
   */
  seed_names: ({caller, names}: {caller: string, names: Array<SeedNameEntry>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a set_pauser transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Assigns the pauser role. The caller must authorize and be an admin.
   * May return an authorization or role-state error.
   */
  set_pauser: ({caller, pauser}: {caller: string, pauser: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a vote_batch transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Records multiple loan proposal votes atomically for `voter`. `voter`
   * must authorize; any invalid vote aborts the batch and emits no partial
   * vote events.
   */
  vote_batch: ({voter, votes}: {voter: string, votes: Array<ProposalVote>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_version transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns this contract's semver, read from CARGO_PKG_VERSION at build
   * time (#197) — lets off-chain tooling / indexers detect which
   * contract build a given deployment is running without relying on the
   * WASM hash alone.
   */
  get_version: (options?: MethodOptions) => Promise<AssembledTransaction<string>>

  /**
   * Construct and simulate a seed_stakes transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Seeds historical stake records. Admin-only before `lock_seeding`;
   * returns `SeedingLocked` after migration closes and emits no event.
   */
  seed_stakes: ({caller, stakes}: {caller: string, stakes: Array<SeedStakeEntry>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a bump_dao_ttl transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Extends the contract instance TTL. Permissionless; emits `TTL_BUMPED`.
   */
  bump_dao_ttl: (options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_delegate transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the address `delegator` has delegated their vote to, or `None`
   * if they have never called `delegate_vote`.
   */
  get_delegate: ({delegator}: {delegator: string}, options?: MethodOptions) => Promise<AssembledTransaction<Option<string>>>

  /**
   * Construct and simulate a get_document transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the content hash attached to a proposal, or `None` if absent.
   * Read-only.
   */
  get_document: ({kind, proposal_id}: {kind: ProposalKind, proposal_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Option<Buffer>>>>

  /**
   * Construct and simulate a list_members transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns up to `limit` member records starting at offset `start` in the
   * internal member-address list. Returns an empty vec when `start` is past
   * the end.  Skips addresses whose persistent storage entry has expired.
   */
  list_members: ({start, limit}: {start: u32, limit: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Array<Member>>>

  /**
   * Construct and simulate a lock_seeding transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Irreversibly closes the seeding window. All `seed_*` entrypoints reject
   * calls after this. Admin-only. Returns `SeedingAlreadyLocked` if already
   * locked.
   */
  lock_seeding: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a remove_admin transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Removes an admin subject to the remaining-admin and signer rules.
   * The caller must be an admin and authorize the call.
   */
  remove_admin: ({caller, admin}: {caller: string, admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a remove_token transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Removes `token` from the supported-asset whitelist.
   */
  remove_token: ({caller, token}: {caller: string, token: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a request_loan transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Creates a loan proposal in the base treasury asset. `borrower` must
   * authorize and satisfy member, loan-policy, and treasury eligibility;
   * emits a loan proposal event on success.
   */
  request_loan: ({borrower, amount, metadata_cid}: {borrower: string, amount: i128, metadata_cid: Option<string>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u32>>>

  /**
   * Construct and simulate a resolve_name transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Resolves a registered name to its owner address. Read-only; returns
   * `None` when unregistered and `NotInitialized` if the DAO is uninitialized.
   */
  resolve_name: ({name}: {name: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Option<string>>>>

  /**
   * Construct and simulate a seed_members transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Seeds member records from a prior deployment. Admin-only; rejected after
   * `lock_seeding()` is called. See `docs/MIGRATION.md` for the full procedure.
   */
  seed_members: ({caller, members}: {caller: string, members: Array<Member>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a claim_rewards transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Withdraw accrued loan-interest yield for the caller.
   */
  claim_rewards: ({member}: {member: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a delegate_vote transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Delegates `delegator`'s voting power to `delegatee`. `delegator` must
   * authorize; invalid targets and authorization failures return errors.
   */
  delegate_vote: ({delegator, delegatee}: {delegator: string, delegatee: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a register_name transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Registers or updates `name` for `owner`. `owner` must authorize and the
   * name must be valid and available; emits a name-registration event.
   */
  register_name: ({owner, name}: {owner: string, name: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a revoke_pauser transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Revokes the current pauser role. The caller must be an admin and
   * authorize the call.
   */
  revoke_pauser: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a is_initialized transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Whether `initialize` has run. Clients should check this before relying
   * on any other view: every state-reading view returns
   * `Error::NotInitialized` on an uninitialized contract rather than
   * trapping or returning an ambiguous empty value.
   */
  is_initialized: (options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a attach_document transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Attaches a content hash to a proposal. The caller must be the
   * authorized proposer/borrower and the document payload must be valid;
   * emits the document event.
   */
  attach_document: ({caller, kind, proposal_id, content_hash}: {caller: string, kind: ProposalKind, proposal_id: u32, content_hash: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_dao_summary transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Headline DAO state in one call (#339): membership counts, spendable
   * treasury per whitelisted asset, total staked, threshold, pause flag and
   * version. Each field matches the individual view it summarises.
   */
  get_dao_summary: (options?: MethodOptions) => Promise<AssembledTransaction<DaoSummary>>

  /**
   * Construct and simulate a get_loan_policy transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the active loan policy. Returns `NotInitialized` before setup.
   */
  get_loan_policy: (options?: MethodOptions) => Promise<AssembledTransaction<Result<LoanPolicy>>>

  /**
   * Construct and simulate a register_member transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Registers `member`, collects the membership fee, and grants active
   * member status. `member` must authorize; returns `AlreadyMember`,
   * `InsufficientTreasury`, or transfer/configuration errors when applicable
   * and emits the member-joined event.
   */
  register_member: ({member}: {member: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a set_loan_policy transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Directly sets the loan policy. Requires admin authorization and returns
   * `InvalidLoanPolicy` for invalid values.
   */
  set_loan_policy: ({caller, policy}: {caller: string, policy: LoanPolicy}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_loan_proposal transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns a loan proposal with any expired phase refreshed from ledger
   * time, or `None` if absent.
   */
  get_loan_proposal: ({proposal_id}: {proposal_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Option<LoanProposal>>>

  /**
   * Construct and simulate a get_pending_yield transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns `member`'s unclaimed base-asset yield. No authorization is
   * required for this read; it may return `ArithmeticOverflow`.
   */
  get_pending_yield: ({member}: {member: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a get_total_members transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_total_members: (options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a get_voting_weight transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns `member`'s current voting weight (base vote + staking bonus).
   * Equivalent to the weight applied when they cast a loan or treasury vote.
   */
  get_voting_weight: ({member}: {member: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a is_seeding_locked transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns `true` while the seeding window is open (before `lock_seeding`
   * is called).
   */
  is_seeding_locked: (options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a verify_commitment transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Convenience pre-check: `true` only when the stored commitment matches
   * the supplied reveal inputs.
   */
  verify_commitment: ({voter, proposal_id, support, salt}: {voter: string, proposal_id: u32, support: boolean, salt: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a edit_loan_proposal transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Changes the amount or metadata of a borrower's loan proposal while it
   * remains in its editing window. `borrower` must authorize.
   */
  edit_loan_proposal: ({borrower, proposal_id, new_amount, metadata_cid}: {borrower: string, proposal_id: u32, new_amount: i128, metadata_cid: Option<string>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_active_members transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the current number of active members.
   */
  get_active_members: (options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a repay_loan_partial transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Repays up to `amount` of a loan's outstanding balance. See
   * `loans::repay_loan_partial` for the interest/principal split and why
   * this is a separate entrypoint from `repay_loan`.
   */
  repay_loan_partial: ({borrower, loan_id, amount}: {borrower: string, loan_id: u32, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a seed_pending_yield transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Seeds historical pending-yield accumulators. Admin-only before
   * `lock_seeding`; returns `SeedingLocked` after migration closes.
   */
  seed_pending_yield: ({caller, yields}: {caller: string, yields: Array<SeedYieldEntry>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_max_stake_bonus transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Maximum additional votes a member can earn through staking.
   */
  get_max_stake_bonus: (options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a get_member_overview transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns everything about `member` in one call: their record, stake,
   * unclaimed yield, loan track record, registered name and delegate.
   * Returns `NotMember` if the address has never registered.
   */
  get_member_overview: ({member}: {member: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<MemberOverview>>>

  /**
   * Construct and simulate a mark_loan_defaulted transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Marks an overdue loan as defaulted. Permissionless — see `loans::mark_loan_defaulted`.
   */
  mark_loan_defaulted: ({loan_id}: {loan_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a repay_loan_in_asset transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Repays a loan using the asset the loan was denominated in.
   */
  repay_loan_in_asset: ({borrower, loan_id, asset}: {borrower: string, loan_id: u32, asset: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a calculate_exit_share transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Calculates `member`'s contribution-proportional share of the base
   * treasury. Returns zero for no active record and `ArithmeticOverflow`
   * if the proportional calculation cannot be represented.
   */
  calculate_exit_share: ({member}: {member: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a calculate_loan_terms transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Quotes the terms a loan of `amount` would carry right now. The quote
   * can only be honoured when `amount <= max_loan`: with an empty or
   * negative treasury `max_loan` is 0 and the rate shown is the policy
   * maximum for a loan that cannot be issued. See
   * `loans::calculate_loan_terms`.
   */
  calculate_loan_terms: ({amount}: {amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<LoanTerms>>>

  /**
   * Construct and simulate a cancel_loan_proposal transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Withdraws a loan proposal while it is still in its editing period.
   * See `loans::cancel_loan_proposal`.
   */
  cancel_loan_proposal: ({borrower, proposal_id}: {borrower: string, proposal_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a cancel_policy_update transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Cancels a pending policy update. Requires admin authorization and emits
   * a policy cancellation event.
   */
  cancel_policy_update: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a commit_treasury_vote transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Stores a hidden treasury-vote commitment. `voter` must authorize and
   * commit during the configured window; emits a commitment event.
   */
  commit_treasury_vote: ({voter, proposal_id, commitment}: {voter: string, proposal_id: u32, commitment: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a expire_loan_proposal transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Permissionless keeper call: persists the expired/rejected transition for
   * a loan proposal whose voting window has passed without reaching quorum.
   */
  expire_loan_proposal: ({proposal_id}: {proposal_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_treasury_balance transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the spendable base treasury balance. Returns
   * `NotInitialized` before setup.
   */
  get_treasury_balance: (options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a is_eligible_for_loan transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Reports the first failing eligibility check. Checking again after
   * resolving that error can reveal another eligibility failure.
   */
  is_eligible_for_loan: ({member}: {member: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a is_token_whitelisted transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns whether `token` is currently an approved treasury asset.
   */
  is_token_whitelisted: ({token}: {token: string}, options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a reveal_treasury_vote transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Reveals a previously committed treasury vote. `voter` must authorize
   * and provide the matching commitment inputs; emits the treasury-vote
   * event after tallying.
   */
  reveal_treasury_vote: ({voter, proposal_id, support, salt}: {voter: string, proposal_id: u32, support: boolean, salt: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a execute_policy_update transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Executes a mature pending policy update. Requires admin authorization
   * and returns timelock or policy errors when not eligible.
   */
  execute_policy_update: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_member_loan_stats transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns `member`'s lifetime loan track record: total loans taken, loans
   * fully repaid, and loans currently outstanding. Single O(1) storage read;
   * returns `NotMember` if the address has never registered.
   */
  get_member_loan_stats: ({member}: {member: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<MemberLoanStats>>>

  /**
   * Construct and simulate a get_stake_weight_unit transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Stake tokens required per additional unit of voting bonus.
   * Every `stake_weight_unit` tokens staked grants +1 vote, up to the cap.
   */
  get_stake_weight_unit: (options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a get_treasury_balances transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns every accepted treasury asset and its current contract balance.
   * 
   * The base token is always included first by `get_whitelisted_tokens`,
   * followed by every currently whitelisted extra asset.
   */
  get_treasury_balances: (options?: MethodOptions) => Promise<AssembledTransaction<Map<string, i128>>>

  /**
   * Construct and simulate a get_treasury_proposal transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns a treasury proposal by id, or `None` if absent.
   */
  get_treasury_proposal: ({proposal_id}: {proposal_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Option<TreasuryProposal>>>

  /**
   * Construct and simulate a propose_policy_update transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Submits a timelocked loan-policy update. Requires admin authorization
   * and emits a policy proposal event.
   */
  propose_policy_update: ({caller, policy}: {caller: string, policy: LoanPolicy}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a request_loan_in_asset transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Requests a loan denominated in a specific whitelisted asset.
   */
  request_loan_in_asset: ({borrower, amount, asset, metadata_cid}: {borrower: string, amount: i128, asset: string, metadata_cid: Option<string>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u32>>>

  /**
   * Construct and simulate a vote_on_loan_proposal transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Casts `voter`'s support or rejection vote on a loan proposal.
   * `voter` must authorize and be eligible; duplicate, ended, or invalid
   * votes return errors and emit the loan-vote event on success.
   */
  vote_on_loan_proposal: ({voter, proposal_id, support}: {voter: string, proposal_id: u32, support: boolean}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a claim_rewards_in_asset transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Withdraws accrued yield in the specified asset without affecting other
   * asset snapshots.
   */
  claim_rewards_in_asset: ({member, asset}: {member: string, asset: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a disburse_approved_loan transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Pays out an approved loan that was waiting on treasury funds.
   * Permissionless keeper call — see `loans::disburse_approved_loan`.
   */
  disburse_approved_loan: ({proposal_id}: {proposal_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a execute_token_addition transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Executes a previously proposed token addition once consensus is met.
   */
  execute_token_addition: ({caller, token}: {caller: string, token: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_whitelisted_tokens transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the full list of approved treasury assets.
   */
  get_whitelisted_tokens: (options?: MethodOptions) => Promise<AssembledTransaction<Array<string>>>

  /**
   * Construct and simulate a propose_token_addition transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Proposes adding `token` to the treasury's supported-asset whitelist.
   * Requires admin consensus; see `admin::propose_token_addition`.
   */
  propose_token_addition: ({caller, token}: {caller: string, token: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_consensus_threshold transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the governance consensus threshold in basis points.
   * Returns `NotInitialized` before setup.
   */
  get_consensus_threshold: (options?: MethodOptions) => Promise<AssembledTransaction<Result<u32>>>

  /**
   * Construct and simulate a get_loan_proposal_count transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the next loan-proposal sequence value as the proposal count.
   */
  get_loan_proposal_count: (options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a get_treasury_accounting transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Reconciliation view for `asset` (#288): the contract's actual token
   * balance, how much of it is member stake, and the principal still owed
   * on active loans in that asset. `token_balance - staked` equals
   * `get_treasury_balance_in_asset`; comparing these figures against the
   * indexed event history exposes drift or direct donations. Also answers
   * for a removed asset, whose loans may still be outstanding.
   */
  get_treasury_accounting: ({asset}: {asset: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<TreasuryAccounting>>>

  /**
   * Construct and simulate a list_treasury_proposals transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns up to `limit` treasury proposals starting at proposal id `start`.
   * Ids are sequential from 0; absent entries (expired from storage) are
   * skipped.  Returns an empty vec when `start` is past the last proposal.
   */
  list_treasury_proposals: ({start, limit}: {start: u32, limit: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Array<TreasuryProposal>>>

  /**
   * Construct and simulate a set_consensus_threshold transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Updates the governance consensus threshold. The caller must be an admin
   * and authorize the call; invalid thresholds return `InvalidThreshold`.
   */
  set_consensus_threshold: ({caller, threshold}: {caller: string, threshold: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a cancel_treasury_proposal transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Withdraws a pending treasury proposal within the policy's editing
   * period. See `treasury::cancel_treasury_proposal`.
   */
  cancel_treasury_proposal: ({proposer, proposal_id}: {proposer: string, proposal_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a expire_treasury_proposal transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Permissionlessly expires a treasury proposal after its voting window.
   * Returns `NotInVotingPhase` for an active proposal and emits an expiry
   * event on transition.
   */
  expire_treasury_proposal: ({proposal_id}: {proposal_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_commit_reveal_window transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Commit and reveal boundary timestamps for a private treasury proposal.
   */
  get_commit_reveal_window: ({proposal_id}: {proposal_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<readonly [u64, u64]>>>

  /**
   * Construct and simulate a execute_treasury_proposal transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Executes an approved treasury withdrawal that was waiting on treasury
   * funds. Permissionless keeper call — see `treasury::execute_approved`.
   */
  execute_treasury_proposal: ({proposal_id}: {proposal_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_pending_policy_update transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the pending timelocked policy update, or `None` if none exists.
   */
  get_pending_policy_update: (options?: MethodOptions) => Promise<AssembledTransaction<Option<PendingPolicyUpdate>>>

  /**
   * Construct and simulate a vote_on_treasury_proposal transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Casts `voter`'s support or rejection vote on a treasury proposal.
   * `voter` must authorize and satisfy electorate rules; duplicate or ended
   * votes return errors and emit a treasury-vote event.
   */
  vote_on_treasury_proposal: ({voter, proposal_id, support}: {voter: string, proposal_id: u32, support: boolean}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_pending_token_addition transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the ledger time at which `token`'s pending addition can be
   * executed, or `None` if no addition is pending for it.
   */
  get_pending_token_addition: ({token}: {token: string}, options?: MethodOptions) => Promise<AssembledTransaction<Option<u64>>>

  /**
   * Construct and simulate a get_pending_yield_in_asset transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns `member`'s unclaimed yield denominated in `asset`.
   */
  get_pending_yield_in_asset: ({member, asset}: {member: string, asset: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a get_pending_token_additions transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Lists indexed pending asset additions with their earliest execution times.
   */
  get_pending_token_additions: (options?: MethodOptions) => Promise<AssembledTransaction<Array<readonly [string, u64]>>>

  /**
   * Construct and simulate a get_treasury_proposal_count transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the next treasury-proposal sequence value as the proposal count.
   */
  get_treasury_proposal_count: (options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a propose_treasury_withdrawal transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Creates a treasury-withdrawal proposal in the base asset. `proposer`
   * must authorize and emit the proposal event; invalid amount or state
   * returns an error.
   */
  propose_treasury_withdrawal: (args: {proposer: string, amount: i128, destination: string, reason: string, private: boolean}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u32>>>

  /**
   * Construct and simulate a propose_withdrawal_in_asset transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Proposes a treasury withdrawal denominated in a specific whitelisted asset.
   * Named without the `treasury_` prefix because Soroban caps entrypoint
   * names at 32 characters.
   */
  propose_withdrawal_in_asset: (args: {proposer: string, amount: i128, asset: string, destination: string, reason: string, private: boolean}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u32>>>

  /**
   * Construct and simulate a get_treasury_balance_in_asset transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the treasury balance held in a specific whitelisted asset.
   */
  get_treasury_balance_in_asset: ({asset}: {asset: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

}
export class Client extends ContractClient {
  static async deploy<T = Client>(
    /** Options for initializing a Client as well as for calling a method, with extras specific to deploying. */
    options: MethodOptions &
      Omit<ContractClientOptions, "contractId"> & {
        /** The hash of the Wasm blob, which must already be installed on-chain. */
        wasmHash: Buffer | string;
        /** Salt used to generate the contract's ID. Passed through to {@link Operation.createCustomContract}. Default: random. */
        salt?: Buffer | Uint8Array;
        /** The format used to decode `wasmHash`, if it's provided as a string. */
        format?: "hex" | "base64";
      }
  ): Promise<AssembledTransaction<T>> {
    return ContractClient.deploy(null, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "AAAAAAAAAHtQYXVzZXMgY29udHJhY3Qgc3RhdGUgY2hhbmdlcy4gVGhlIHBhdXNlciBvciBhbiBhbGxvd2VkIGNhbGxlciBtdXN0CmF1dGhvcml6ZSB0aGUgY2FsbCBhbmQgZW1pdCB0aGUgcGF1c2UgZXZlbnQgb24gc3VjY2Vzcy4AAAAABXBhdXNlAAAAAAAAAQAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAALdTdGFrZXMgYmFzZSB0cmVhc3VyeSBhc3NldHMgdG8gaW5jcmVhc2UgYG1lbWJlcmAncyB2b3Rpbmcgd2VpZ2h0LgpgbWVtYmVyYCBtdXN0IGF1dGhvcml6ZSBhbmQgaGF2ZSBzdWZmaWNpZW50IHNwZW5kYWJsZSBiYWxhbmNlOyByZXR1cm5zCnN0YWtlL2Ftb3VudCBlcnJvcnMgYW5kIGVtaXRzIHRoZSBzdGFrZSBldmVudC4AAAAABXN0YWtlAAAAAAAAAgAAAAAAAAAGbWVtYmVyAAAAAAATAAAAAAAAAAZhbW91bnQAAAAAAAsAAAABAAAD6QAAAAIAAAAD",
        "AAAAAAAAAHJSZXR1cm5zIHRoZSBuYW1lIHJlZ2lzdGVyZWQgdG8gYG93bmVyYCwgb3IgYE5vbmVgIHdoZW4gbm9uZSBpcwpyZWdpc3RlcmVkLiBSZWFkLW9ubHk7IG1heSByZXR1cm4gYE5vdEluaXRpYWxpemVkYC4AAAAAAAduYW1lX29mAAAAAAEAAAAAAAAABW93bmVyAAAAAAAAEwAAAAEAAAPpAAAD6AAAABAAAAAD",
        "AAAAAAAAAFhSZXN1bWVzIGNvbnRyYWN0IHN0YXRlIGNoYW5nZXMuIFRoZSBwYXVzZXIgb3IgYW4gYWxsb3dlZCBjYWxsZXIgbXVzdAphdXRob3JpemUgdGhlIGNhbGwuAAAAB3VucGF1c2UAAAAAAQAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAAHtSZWxlYXNlcyBzdGFrZWQgYmFzZSB0cmVhc3VyeSBhc3NldHMgZnJvbSBgbWVtYmVyYC4gYG1lbWJlcmAgbXVzdAphdXRob3JpemUgYW5kIGhhdmUgZW5vdWdoIHN0YWtlOyBlbWl0cyB0aGUgdW5zdGFrZSBldmVudC4AAAAAB3Vuc3Rha2UAAAAAAgAAAAAAAAAGbWVtYmVyAAAAAAATAAAAAAAAAAZhbW91bnQAAAAAAAsAAAABAAAD6QAAAAIAAAAD",
        "AAAAAAAAANFFeGl0cyBgbWVtYmVyYCwgc2V0dGxpbmcgc3Rha2UgYW5kIGFjY3J1ZWQgeWllbGQgYW5kIHJlZHVjaW5nIHRoZWlyCmNvbnRyaWJ1dGlvbiBzaGFyZS4gYG1lbWJlcmAgbXVzdCBhdXRob3JpemUgYW5kIGhhdmUgbm8gYWN0aXZlIGxvYW4uClJldHVybnMgbWVtYmVyc2hpcCwgYmFsYW5jZSwgb3IgYXJpdGhtZXRpYyBlcnJvcnMgYW5kIGVtaXRzIGV4aXQgZXZlbnRzLgAAAAAAAAhleGl0X2RhbwAAAAEAAAAAAAAABm1lbWJlcgAAAAAAEwAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAADVSZXR1cm5zIGEgbG9hbiBieSBpZCwgb3IgYE5vbmVgIGlmIGl0IGRvZXMgbm90IGV4aXN0LgAAAAAAAAhnZXRfbG9hbgAAAAEAAAAAAAAAB2xvYW5faWQAAAAABAAAAAEAAAPoAAAH0AAAAARMb2Fu",
        "AAAAAAAAAE1SZXBvcnRzIHdoZXRoZXIgYGFkZHJlc3NgIGlzIGFuIGFkbWluLiBSZXR1cm5zIGBOb3RJbml0aWFsaXplZGAgYmVmb3JlIHNldHVwLgAAAAAAAAhpc19hZG1pbgAAAAEAAAAAAAAAB2FkZHJlc3MAAAAAEwAAAAEAAAPpAAAAAQAAAAM=",
        "AAAAAAAAAL5BZGRzIGFuIGFkbWluLiBUaGUgY2FsbGVyIG11c3QgYmUgYW4gZXhpc3RpbmcgYWRtaW4gYW5kIGF1dGhvcml6ZSB0aGUKY2FsbC4gTWF5IGVtaXQgYW4gYWRtaW4tY2hhbmdlIGV2ZW50IGFuZCByZXR1cm5zIGFuIGF1dGhvcml6YXRpb24gb3IKY29uZmlndXJhdGlvbiBlcnJvciB3aGVuIHRoZSBjaGFuZ2UgaXMgbm90IGFsbG93ZWQuAAAAAAAJYWRkX2FkbWluAAAAAAAAAgAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAAVhZG1pbgAAAAAAABMAAAABAAAD6QAAAAIAAAAD",
        "AAAAAAAAADRSZXR1cm5zIGBtZW1iZXJgJ3MgY3VycmVudCBiYXNlLWFzc2V0IHN0YWtlIGJhbGFuY2UuAAAACWdldF9zdGFrZQAAAAAAAAEAAAAAAAAABm1lbWJlcgAAAAAAEwAAAAEAAAAL",
        "AAAAAAAAAEdSZXR1cm5zIHRoZSBiYXNlIHRyZWFzdXJ5IGFzc2V0LiBSZXR1cm5zIGBOb3RJbml0aWFsaXplZGAgYmVmb3JlIHNldHVwLgAAAAAJZ2V0X3Rva2VuAAAAAAAAAAAAAAEAAAPpAAAAEwAAAAM=",
        "AAAAAAAAAT9SZXR1cm5zIHdoZXRoZXIgYHZvdGVyYCBoYXMgY2FzdCBhIChwb3NzaWJseSBub3QteWV0LXJldmVhbGVkKSB2b3RlCm9uIHRoZSBnaXZlbiBwcm9wb3NhbC4gRm9yIHB1YmxpYyAobm9uLXByaXZhdGUpIHByb3Bvc2FscyB0aGlzIGlzIGEKc2ltcGxlIGNoZWNrIGFnYWluc3QgdGhlIHZvdGUgcmVjb3JkLiBGb3IgY29tbWl0LXJldmVhbCB0cmVhc3VyeQpwcm9wb3NhbHMgaXQgY2hlY2tzIHdoZXRoZXIgdGhlIHZvdGVyIGhhcyBjb21taXR0ZWQgKHdoaWNoIGlzIHRoZQplYXJsaWVzdCBwb2ludCBhdCB3aGljaCBhIGRvdWJsZS12b3RlIGlzIHByZXZlbnRlZCkuAAAAAAloYXNfdm90ZWQAAAAAAAADAAAAAAAAAARraW5kAAAH0AAAAAxQcm9wb3NhbEtpbmQAAAAAAAAAC3Byb3Bvc2FsX2lkAAAAAAQAAAAAAAAABXZvdGVyAAAAAAAAEwAAAAEAAAAB",
        "AAAAAAAAADlSZXBvcnRzIHdoZXRoZXIgYGFkZHJlc3NgIGlzIGFuIGFjdGl2ZSByZWdpc3RlcmVkIG1lbWJlci4AAAAAAAAJaXNfbWVtYmVyAAAAAAAAAQAAAAAAAAAHYWRkcmVzcwAAAAATAAAAAQAAAAE=",
        "AAAAAAAAADxSZXBvcnRzIHdoZXRoZXIgY29udHJhY3Qgc3RhdGUgY2hhbmdlcyBhcmUgY3VycmVudGx5IHBhdXNlZC4AAAAJaXNfcGF1c2VkAAAAAAAAAAAAAAEAAAAB",
        "AAAAAAAAAHpTZWVkcyBoaXN0b3JpY2FsIGNvbnRlbnQtaGFzaCBkb2N1bWVudHMuIEFkbWluLW9ubHkgYmVmb3JlCmBsb2NrX3NlZWRpbmdgOyByZXR1cm5zIGBTZWVkaW5nTG9ja2VkYCBhZnRlciBtaWdyYXRpb24gY2xvc2VzLgAAAAAACXNlZWRfZG9jcwAAAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAEZG9jcwAAA+oAAAfQAAAADFNlZWREb2NFbnRyeQAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAAEVSZXR1cm5zIHRoZSBjdXJyZW50IGFkbWluIHNldC4gUmV0dXJucyBgTm90SW5pdGlhbGl6ZWRgIGJlZm9yZSBzZXR1cC4AAAAAAAAKZ2V0X2FkbWlucwAAAAAAAAAAAAEAAAPpAAAD6gAAABMAAAAD",
        "AAAAAAAAAENSZXR1cm5zIHRoZSBtZW1iZXIgcmVjb3JkIGZvciBgYWRkcmVzc2AsIG9yIGBOb25lYCBpZiB1bnJlZ2lzdGVyZWQuAAAAAApnZXRfbWVtYmVyAAAAAAABAAAAAAAAAAdhZGRyZXNzAAAAABMAAAABAAAD6AAAB9AAAAAGTWVtYmVyAAA=",
        "AAAAAAAAAKBPbmUtdGltZSBzZXR1cC4gYGFkbWluc2AgYm9vdHN0cmFwIGdvdmVybmFuY2UsIGBjb25zZW5zdXNfdGhyZXNob2xkYCBpcwppbiBiYXNpcyBwb2ludHMgKGUuZy4gNTEwMCA9IDUxJSksIGFuZCBgdG9rZW5gIGlzIHRoZSBhc3NldCBhbGwgREFPCnZhbHVlIGZsb3dzIHRocm91Z2guAAAACmluaXRpYWxpemUAAAAAAAUAAAAAAAAABmFkbWlucwAAAAAD6gAAABMAAAAAAAAAE2NvbnNlbnN1c190aHJlc2hvbGQAAAAABAAAAAAAAAAObWVtYmVyc2hpcF9mZWUAAAAAAAsAAAAAAAAABXRva2VuAAAAAAAAEwAAAAAAAAAGcG9saWN5AAAAAAfQAAAACkxvYW5Qb2xpY3kAAAAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAAIRMaXN0cyB1cCB0byBgbGltaXRgIGxvYW5zIHdpdGhpbiB0aGUgcHJvcG9zYWwtSUQgd2luZG93IHN0YXJ0aW5nIGF0CmBzdGFydGAuIE1pc3NpbmcgbG9hbnMgYXJlIHNraXBwZWQgYW5kIGRvIG5vdCBleHRlbmQgdGhlIHdpbmRvdy4AAAAKbGlzdF9sb2FucwAAAAAAAgAAAAAAAAAFc3RhcnQAAAAAAAAEAAAAAAAAAAVsaW1pdAAAAAAAAAQAAAABAAAD6gAAB9AAAAAETG9hbg==",
        "AAAAAAAAAL9SZXBheXMgdGhlIG91dHN0YW5kaW5nIGJhbGFuY2Ugb2YgYSBiYXNlLWFzc2V0IGxvYW4uIGBib3Jyb3dlcmAgbXVzdAphdXRob3JpemU7IGludmFsaWQsIGluYWN0aXZlLCBvciBpbnN1ZmZpY2llbnQtcGF5bWVudCBjb25kaXRpb25zIHJldHVybgplcnJvcnMgYW5kIGVtaXQgYSBsb2FuLXJlcGF5bWVudCBldmVudCBvbiBzdWNjZXNzLgAAAAAKcmVwYXlfbG9hbgAAAAAAAgAAAAAAAAAIYm9ycm93ZXIAAAATAAAAAAAAAAdsb2FuX2lkAAAAAAQAAAABAAAD6QAAAAIAAAAD",
        "AAAAAAAAAHBTZWVkcyBoaXN0b3JpY2FsIGxvYW4gcmVjb3Jkcy4gQWRtaW4tb25seSBiZWZvcmUgYGxvY2tfc2VlZGluZ2A7CnJldHVybnMgYFNlZWRpbmdMb2NrZWRgIGFmdGVyIG1pZ3JhdGlvbiBjbG9zZXMuAAAACnNlZWRfbG9hbnMAAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAFbG9hbnMAAAAAAAPqAAAH0AAAAARMb2FuAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAAHlTZWVkcyBoaXN0b3JpY2FsIG5hbWUtcmVnaXN0cnkgcmVjb3Jkcy4gQWRtaW4tb25seSBiZWZvcmUKYGxvY2tfc2VlZGluZ2A7IHJldHVybnMgYFNlZWRpbmdMb2NrZWRgIGFmdGVyIG1pZ3JhdGlvbiBjbG9zZXMuAAAAAAAACnNlZWRfbmFtZXMAAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAFbmFtZXMAAAAAAAPqAAAH0AAAAA1TZWVkTmFtZUVudHJ5AAAAAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAAHRBc3NpZ25zIHRoZSBwYXVzZXIgcm9sZS4gVGhlIGNhbGxlciBtdXN0IGF1dGhvcml6ZSBhbmQgYmUgYW4gYWRtaW4uCk1heSByZXR1cm4gYW4gYXV0aG9yaXphdGlvbiBvciByb2xlLXN0YXRlIGVycm9yLgAAAApzZXRfcGF1c2VyAAAAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAABnBhdXNlcgAAAAAAEwAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAAJhSZWNvcmRzIG11bHRpcGxlIGxvYW4gcHJvcG9zYWwgdm90ZXMgYXRvbWljYWxseSBmb3IgYHZvdGVyYC4gYHZvdGVyYAptdXN0IGF1dGhvcml6ZTsgYW55IGludmFsaWQgdm90ZSBhYm9ydHMgdGhlIGJhdGNoIGFuZCBlbWl0cyBubyBwYXJ0aWFsCnZvdGUgZXZlbnRzLgAAAAp2b3RlX2JhdGNoAAAAAAACAAAAAAAAAAV2b3RlcgAAAAAAABMAAAAAAAAABXZvdGVzAAAAAAAD6gAAB9AAAAAMUHJvcG9zYWxWb3RlAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAANhSZXR1cm5zIHRoaXMgY29udHJhY3QncyBzZW12ZXIsIHJlYWQgZnJvbSBDQVJHT19QS0dfVkVSU0lPTiBhdCBidWlsZAp0aW1lICgjMTk3KSDigJQgbGV0cyBvZmYtY2hhaW4gdG9vbGluZyAvIGluZGV4ZXJzIGRldGVjdCB3aGljaApjb250cmFjdCBidWlsZCBhIGdpdmVuIGRlcGxveW1lbnQgaXMgcnVubmluZyB3aXRob3V0IHJlbHlpbmcgb24gdGhlCldBU00gaGFzaCBhbG9uZS4AAAALZ2V0X3ZlcnNpb24AAAAAAAAAAAEAAAAQ",
        "AAAAAAAAAIRTZWVkcyBoaXN0b3JpY2FsIHN0YWtlIHJlY29yZHMuIEFkbWluLW9ubHkgYmVmb3JlIGBsb2NrX3NlZWRpbmdgOwpyZXR1cm5zIGBTZWVkaW5nTG9ja2VkYCBhZnRlciBtaWdyYXRpb24gY2xvc2VzIGFuZCBlbWl0cyBubyBldmVudC4AAAALc2VlZF9zdGFrZXMAAAAAAgAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAAZzdGFrZXMAAAAAA+oAAAfQAAAADlNlZWRTdGFrZUVudHJ5AAAAAAABAAAD6QAAAAIAAAAD",
        "AAAAAAAAAEZFeHRlbmRzIHRoZSBjb250cmFjdCBpbnN0YW5jZSBUVEwuIFBlcm1pc3Npb25sZXNzOyBlbWl0cyBgVFRMX0JVTVBFRGAuAAAAAAAMYnVtcF9kYW9fdHRsAAAAAAAAAAA=",
        "AAAAAAAAAHFSZXR1cm5zIHRoZSBhZGRyZXNzIGBkZWxlZ2F0b3JgIGhhcyBkZWxlZ2F0ZWQgdGhlaXIgdm90ZSB0bywgb3IgYE5vbmVgCmlmIHRoZXkgaGF2ZSBuZXZlciBjYWxsZWQgYGRlbGVnYXRlX3ZvdGVgLgAAAAAAAAxnZXRfZGVsZWdhdGUAAAABAAAAAAAAAAlkZWxlZ2F0b3IAAAAAAAATAAAAAQAAA+gAAAAT",
        "AAAAAAAAAFBSZXR1cm5zIHRoZSBjb250ZW50IGhhc2ggYXR0YWNoZWQgdG8gYSBwcm9wb3NhbCwgb3IgYE5vbmVgIGlmIGFic2VudC4KUmVhZC1vbmx5LgAAAAxnZXRfZG9jdW1lbnQAAAACAAAAAAAAAARraW5kAAAH0AAAAAxQcm9wb3NhbEtpbmQAAAAAAAAAC3Byb3Bvc2FsX2lkAAAAAAQAAAABAAAD6QAAA+gAAAAOAAAAAw==",
        "AAAAAAAAANRSZXR1cm5zIHVwIHRvIGBsaW1pdGAgbWVtYmVyIHJlY29yZHMgc3RhcnRpbmcgYXQgb2Zmc2V0IGBzdGFydGAgaW4gdGhlCmludGVybmFsIG1lbWJlci1hZGRyZXNzIGxpc3QuIFJldHVybnMgYW4gZW1wdHkgdmVjIHdoZW4gYHN0YXJ0YCBpcyBwYXN0CnRoZSBlbmQuICBTa2lwcyBhZGRyZXNzZXMgd2hvc2UgcGVyc2lzdGVudCBzdG9yYWdlIGVudHJ5IGhhcyBleHBpcmVkLgAAAAxsaXN0X21lbWJlcnMAAAACAAAAAAAAAAVzdGFydAAAAAAAAAQAAAAAAAAABWxpbWl0AAAAAAAABAAAAAEAAAPqAAAH0AAAAAZNZW1iZXIAAA==",
        "AAAAAAAAAJdJcnJldmVyc2libHkgY2xvc2VzIHRoZSBzZWVkaW5nIHdpbmRvdy4gQWxsIGBzZWVkXypgIGVudHJ5cG9pbnRzIHJlamVjdApjYWxscyBhZnRlciB0aGlzLiBBZG1pbi1vbmx5LiBSZXR1cm5zIGBTZWVkaW5nQWxyZWFkeUxvY2tlZGAgaWYgYWxyZWFkeQpsb2NrZWQuAAAAAAxsb2NrX3NlZWRpbmcAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAABAAAD6QAAAAIAAAAD",
        "AAAAAAAAAHVSZW1vdmVzIGFuIGFkbWluIHN1YmplY3QgdG8gdGhlIHJlbWFpbmluZy1hZG1pbiBhbmQgc2lnbmVyIHJ1bGVzLgpUaGUgY2FsbGVyIG11c3QgYmUgYW4gYWRtaW4gYW5kIGF1dGhvcml6ZSB0aGUgY2FsbC4AAAAAAAAMcmVtb3ZlX2FkbWluAAAAAgAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAAVhZG1pbgAAAAAAABMAAAABAAAD6QAAAAIAAAAD",
        "AAAAAAAAADNSZW1vdmVzIGB0b2tlbmAgZnJvbSB0aGUgc3VwcG9ydGVkLWFzc2V0IHdoaXRlbGlzdC4AAAAADHJlbW92ZV90b2tlbgAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAFdG9rZW4AAAAAAAATAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAALBDcmVhdGVzIGEgbG9hbiBwcm9wb3NhbCBpbiB0aGUgYmFzZSB0cmVhc3VyeSBhc3NldC4gYGJvcnJvd2VyYCBtdXN0CmF1dGhvcml6ZSBhbmQgc2F0aXNmeSBtZW1iZXIsIGxvYW4tcG9saWN5LCBhbmQgdHJlYXN1cnkgZWxpZ2liaWxpdHk7CmVtaXRzIGEgbG9hbiBwcm9wb3NhbCBldmVudCBvbiBzdWNjZXNzLgAAAAxyZXF1ZXN0X2xvYW4AAAADAAAAAAAAAAhib3Jyb3dlcgAAABMAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAAMbWV0YWRhdGFfY2lkAAAD6AAAABAAAAABAAAD6QAAAAQAAAAD",
        "AAAAAAAAAI5SZXNvbHZlcyBhIHJlZ2lzdGVyZWQgbmFtZSB0byBpdHMgb3duZXIgYWRkcmVzcy4gUmVhZC1vbmx5OyByZXR1cm5zCmBOb25lYCB3aGVuIHVucmVnaXN0ZXJlZCBhbmQgYE5vdEluaXRpYWxpemVkYCBpZiB0aGUgREFPIGlzIHVuaW5pdGlhbGl6ZWQuAAAAAAAMcmVzb2x2ZV9uYW1lAAAAAQAAAAAAAAAEbmFtZQAAABAAAAABAAAD6QAAA+gAAAATAAAAAw==",
        "AAAAAAAAAJRTZWVkcyBtZW1iZXIgcmVjb3JkcyBmcm9tIGEgcHJpb3IgZGVwbG95bWVudC4gQWRtaW4tb25seTsgcmVqZWN0ZWQgYWZ0ZXIKYGxvY2tfc2VlZGluZygpYCBpcyBjYWxsZWQuIFNlZSBgZG9jcy9NSUdSQVRJT04ubWRgIGZvciB0aGUgZnVsbCBwcm9jZWR1cmUuAAAADHNlZWRfbWVtYmVycwAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAHbWVtYmVycwAAAAPqAAAH0AAAAAZNZW1iZXIAAAAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAADRXaXRoZHJhdyBhY2NydWVkIGxvYW4taW50ZXJlc3QgeWllbGQgZm9yIHRoZSBjYWxsZXIuAAAADWNsYWltX3Jld2FyZHMAAAAAAAABAAAAAAAAAAZtZW1iZXIAAAAAABMAAAABAAAD6QAAAAsAAAAD",
        "AAAAAAAAAIpEZWxlZ2F0ZXMgYGRlbGVnYXRvcmAncyB2b3RpbmcgcG93ZXIgdG8gYGRlbGVnYXRlZWAuIGBkZWxlZ2F0b3JgIG11c3QKYXV0aG9yaXplOyBpbnZhbGlkIHRhcmdldHMgYW5kIGF1dGhvcml6YXRpb24gZmFpbHVyZXMgcmV0dXJuIGVycm9ycy4AAAAAAA1kZWxlZ2F0ZV92b3RlAAAAAAAAAgAAAAAAAAAJZGVsZWdhdG9yAAAAAAAAEwAAAAAAAAAJZGVsZWdhdGVlAAAAAAAAEwAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAAIpSZWdpc3RlcnMgb3IgdXBkYXRlcyBgbmFtZWAgZm9yIGBvd25lcmAuIGBvd25lcmAgbXVzdCBhdXRob3JpemUgYW5kIHRoZQpuYW1lIG11c3QgYmUgdmFsaWQgYW5kIGF2YWlsYWJsZTsgZW1pdHMgYSBuYW1lLXJlZ2lzdHJhdGlvbiBldmVudC4AAAAAAA1yZWdpc3Rlcl9uYW1lAAAAAAAAAgAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAAARuYW1lAAAAEAAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAAFRSZXZva2VzIHRoZSBjdXJyZW50IHBhdXNlciByb2xlLiBUaGUgY2FsbGVyIG11c3QgYmUgYW4gYWRtaW4gYW5kCmF1dGhvcml6ZSB0aGUgY2FsbC4AAAANcmV2b2tlX3BhdXNlcgAAAAAAAAEAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAAOtXaGV0aGVyIGBpbml0aWFsaXplYCBoYXMgcnVuLiBDbGllbnRzIHNob3VsZCBjaGVjayB0aGlzIGJlZm9yZSByZWx5aW5nCm9uIGFueSBvdGhlciB2aWV3OiBldmVyeSBzdGF0ZS1yZWFkaW5nIHZpZXcgcmV0dXJucwpgRXJyb3I6Ok5vdEluaXRpYWxpemVkYCBvbiBhbiB1bmluaXRpYWxpemVkIGNvbnRyYWN0IHJhdGhlciB0aGFuCnRyYXBwaW5nIG9yIHJldHVybmluZyBhbiBhbWJpZ3VvdXMgZW1wdHkgdmFsdWUuAAAAAA5pc19pbml0aWFsaXplZAAAAAAAAAAAAAEAAAAB",
        "AAAAAAAAAJxBdHRhY2hlcyBhIGNvbnRlbnQgaGFzaCB0byBhIHByb3Bvc2FsLiBUaGUgY2FsbGVyIG11c3QgYmUgdGhlCmF1dGhvcml6ZWQgcHJvcG9zZXIvYm9ycm93ZXIgYW5kIHRoZSBkb2N1bWVudCBwYXlsb2FkIG11c3QgYmUgdmFsaWQ7CmVtaXRzIHRoZSBkb2N1bWVudCBldmVudC4AAAAPYXR0YWNoX2RvY3VtZW50AAAAAAQAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAEa2luZAAAB9AAAAAMUHJvcG9zYWxLaW5kAAAAAAAAAAtwcm9wb3NhbF9pZAAAAAAEAAAAAAAAAAxjb250ZW50X2hhc2gAAAAOAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAAMpIZWFkbGluZSBEQU8gc3RhdGUgaW4gb25lIGNhbGwgKCMzMzkpOiBtZW1iZXJzaGlwIGNvdW50cywgc3BlbmRhYmxlCnRyZWFzdXJ5IHBlciB3aGl0ZWxpc3RlZCBhc3NldCwgdG90YWwgc3Rha2VkLCB0aHJlc2hvbGQsIHBhdXNlIGZsYWcgYW5kCnZlcnNpb24uIEVhY2ggZmllbGQgbWF0Y2hlcyB0aGUgaW5kaXZpZHVhbCB2aWV3IGl0IHN1bW1hcmlzZXMuAAAAAAAPZ2V0X2Rhb19zdW1tYXJ5AAAAAAAAAAABAAAH0AAAAApEYW9TdW1tYXJ5AAA=",
        "AAAAAAAAAEZSZXR1cm5zIHRoZSBhY3RpdmUgbG9hbiBwb2xpY3kuIFJldHVybnMgYE5vdEluaXRpYWxpemVkYCBiZWZvcmUgc2V0dXAuAAAAAAAPZ2V0X2xvYW5fcG9saWN5AAAAAAAAAAABAAAD6QAAB9AAAAAKTG9hblBvbGljeQAAAAAAAw==",
        "AAAAAAAAAO9SZWdpc3RlcnMgYG1lbWJlcmAsIGNvbGxlY3RzIHRoZSBtZW1iZXJzaGlwIGZlZSwgYW5kIGdyYW50cyBhY3RpdmUKbWVtYmVyIHN0YXR1cy4gYG1lbWJlcmAgbXVzdCBhdXRob3JpemU7IHJldHVybnMgYEFscmVhZHlNZW1iZXJgLApgSW5zdWZmaWNpZW50VHJlYXN1cnlgLCBvciB0cmFuc2Zlci9jb25maWd1cmF0aW9uIGVycm9ycyB3aGVuIGFwcGxpY2FibGUKYW5kIGVtaXRzIHRoZSBtZW1iZXItam9pbmVkIGV2ZW50LgAAAAAPcmVnaXN0ZXJfbWVtYmVyAAAAAAEAAAAAAAAABm1lbWJlcgAAAAAAEwAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAAG9EaXJlY3RseSBzZXRzIHRoZSBsb2FuIHBvbGljeS4gUmVxdWlyZXMgYWRtaW4gYXV0aG9yaXphdGlvbiBhbmQgcmV0dXJucwpgSW52YWxpZExvYW5Qb2xpY3lgIGZvciBpbnZhbGlkIHZhbHVlcy4AAAAAD3NldF9sb2FuX3BvbGljeQAAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAABnBvbGljeQAAAAAH0AAAAApMb2FuUG9saWN5AAAAAAABAAAD6QAAAAIAAAAD",
        "AAAAAAAAAF9SZXR1cm5zIGEgbG9hbiBwcm9wb3NhbCB3aXRoIGFueSBleHBpcmVkIHBoYXNlIHJlZnJlc2hlZCBmcm9tIGxlZGdlcgp0aW1lLCBvciBgTm9uZWAgaWYgYWJzZW50LgAAAAARZ2V0X2xvYW5fcHJvcG9zYWwAAAAAAAABAAAAAAAAAAtwcm9wb3NhbF9pZAAAAAAEAAAAAQAAA+gAAAfQAAAADExvYW5Qcm9wb3NhbA==",
        "AAAAAAAAAH5SZXR1cm5zIGBtZW1iZXJgJ3MgdW5jbGFpbWVkIGJhc2UtYXNzZXQgeWllbGQuIE5vIGF1dGhvcml6YXRpb24gaXMKcmVxdWlyZWQgZm9yIHRoaXMgcmVhZDsgaXQgbWF5IHJldHVybiBgQXJpdGhtZXRpY092ZXJmbG93YC4AAAAAABFnZXRfcGVuZGluZ195aWVsZAAAAAAAAAEAAAAAAAAABm1lbWJlcgAAAAAAEwAAAAEAAAPpAAAACwAAAAM=",
        "AAAAAAAAAAAAAAARZ2V0X3RvdGFsX21lbWJlcnMAAAAAAAAAAAAAAQAAAAQ=",
        "AAAAAAAAAI5SZXR1cm5zIGBtZW1iZXJgJ3MgY3VycmVudCB2b3Rpbmcgd2VpZ2h0IChiYXNlIHZvdGUgKyBzdGFraW5nIGJvbnVzKS4KRXF1aXZhbGVudCB0byB0aGUgd2VpZ2h0IGFwcGxpZWQgd2hlbiB0aGV5IGNhc3QgYSBsb2FuIG9yIHRyZWFzdXJ5IHZvdGUuAAAAAAARZ2V0X3ZvdGluZ193ZWlnaHQAAAAAAAABAAAAAAAAAAZtZW1iZXIAAAAAABMAAAABAAAACw==",
        "AAAAAAAAAFJSZXR1cm5zIGB0cnVlYCB3aGlsZSB0aGUgc2VlZGluZyB3aW5kb3cgaXMgb3BlbiAoYmVmb3JlIGBsb2NrX3NlZWRpbmdgCmlzIGNhbGxlZCkuAAAAAAARaXNfc2VlZGluZ19sb2NrZWQAAAAAAAAAAAAAAQAAAAE=",
        "AAAAAAAAAGFDb252ZW5pZW5jZSBwcmUtY2hlY2s6IGB0cnVlYCBvbmx5IHdoZW4gdGhlIHN0b3JlZCBjb21taXRtZW50IG1hdGNoZXMKdGhlIHN1cHBsaWVkIHJldmVhbCBpbnB1dHMuAAAAAAAAEXZlcmlmeV9jb21taXRtZW50AAAAAAAABAAAAAAAAAAFdm90ZXIAAAAAAAATAAAAAAAAAAtwcm9wb3NhbF9pZAAAAAAEAAAAAAAAAAdzdXBwb3J0AAAAAAEAAAAAAAAABHNhbHQAAAPuAAAAIAAAAAEAAAAB",
        "AAAAAAAAAH9DaGFuZ2VzIHRoZSBhbW91bnQgb3IgbWV0YWRhdGEgb2YgYSBib3Jyb3dlcidzIGxvYW4gcHJvcG9zYWwgd2hpbGUgaXQKcmVtYWlucyBpbiBpdHMgZWRpdGluZyB3aW5kb3cuIGBib3Jyb3dlcmAgbXVzdCBhdXRob3JpemUuAAAAABJlZGl0X2xvYW5fcHJvcG9zYWwAAAAAAAQAAAAAAAAACGJvcnJvd2VyAAAAEwAAAAAAAAALcHJvcG9zYWxfaWQAAAAABAAAAAAAAAAKbmV3X2Ftb3VudAAAAAAACwAAAAAAAAAMbWV0YWRhdGFfY2lkAAAD6AAAABAAAAABAAAD6QAAAAIAAAAD",
        "AAAAAAAAAC1SZXR1cm5zIHRoZSBjdXJyZW50IG51bWJlciBvZiBhY3RpdmUgbWVtYmVycy4AAAAAAAASZ2V0X2FjdGl2ZV9tZW1iZXJzAAAAAAAAAAAAAQAAAAQ=",
        "AAAAAAAAALBSZXBheXMgdXAgdG8gYGFtb3VudGAgb2YgYSBsb2FuJ3Mgb3V0c3RhbmRpbmcgYmFsYW5jZS4gU2VlCmBsb2Fuczo6cmVwYXlfbG9hbl9wYXJ0aWFsYCBmb3IgdGhlIGludGVyZXN0L3ByaW5jaXBhbCBzcGxpdCBhbmQgd2h5CnRoaXMgaXMgYSBzZXBhcmF0ZSBlbnRyeXBvaW50IGZyb20gYHJlcGF5X2xvYW5gLgAAABJyZXBheV9sb2FuX3BhcnRpYWwAAAAAAAMAAAAAAAAACGJvcnJvd2VyAAAAEwAAAAAAAAAHbG9hbl9pZAAAAAAEAAAAAAAAAAZhbW91bnQAAAAAAAsAAAABAAAD6QAAAAIAAAAD",
        "AAAAAAAAAH5TZWVkcyBoaXN0b3JpY2FsIHBlbmRpbmcteWllbGQgYWNjdW11bGF0b3JzLiBBZG1pbi1vbmx5IGJlZm9yZQpgbG9ja19zZWVkaW5nYDsgcmV0dXJucyBgU2VlZGluZ0xvY2tlZGAgYWZ0ZXIgbWlncmF0aW9uIGNsb3Nlcy4AAAAAABJzZWVkX3BlbmRpbmdfeWllbGQAAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAGeWllbGRzAAAAAAPqAAAH0AAAAA5TZWVkWWllbGRFbnRyeQAAAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAADtNYXhpbXVtIGFkZGl0aW9uYWwgdm90ZXMgYSBtZW1iZXIgY2FuIGVhcm4gdGhyb3VnaCBzdGFraW5nLgAAAAATZ2V0X21heF9zdGFrZV9ib251cwAAAAAAAAAAAQAAAAs=",
        "AAAAAAAAAL5SZXR1cm5zIGV2ZXJ5dGhpbmcgYWJvdXQgYG1lbWJlcmAgaW4gb25lIGNhbGw6IHRoZWlyIHJlY29yZCwgc3Rha2UsCnVuY2xhaW1lZCB5aWVsZCwgbG9hbiB0cmFjayByZWNvcmQsIHJlZ2lzdGVyZWQgbmFtZSBhbmQgZGVsZWdhdGUuClJldHVybnMgYE5vdE1lbWJlcmAgaWYgdGhlIGFkZHJlc3MgaGFzIG5ldmVyIHJlZ2lzdGVyZWQuAAAAAAATZ2V0X21lbWJlcl9vdmVydmlldwAAAAABAAAAAAAAAAZtZW1iZXIAAAAAABMAAAABAAAD6QAAB9AAAAAOTWVtYmVyT3ZlcnZpZXcAAAAAAAM=",
        "AAAAAAAAAFhNYXJrcyBhbiBvdmVyZHVlIGxvYW4gYXMgZGVmYXVsdGVkLiBQZXJtaXNzaW9ubGVzcyDigJQgc2VlIGBsb2Fuczo6bWFya19sb2FuX2RlZmF1bHRlZGAuAAAAE21hcmtfbG9hbl9kZWZhdWx0ZWQAAAAAAQAAAAAAAAAHbG9hbl9pZAAAAAAEAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAADpSZXBheXMgYSBsb2FuIHVzaW5nIHRoZSBhc3NldCB0aGUgbG9hbiB3YXMgZGVub21pbmF0ZWQgaW4uAAAAAAATcmVwYXlfbG9hbl9pbl9hc3NldAAAAAADAAAAAAAAAAhib3Jyb3dlcgAAABMAAAAAAAAAB2xvYW5faWQAAAAABAAAAAAAAAAFYXNzZXQAAAAAAAATAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAAL1DYWxjdWxhdGVzIGBtZW1iZXJgJ3MgY29udHJpYnV0aW9uLXByb3BvcnRpb25hbCBzaGFyZSBvZiB0aGUgYmFzZQp0cmVhc3VyeS4gUmV0dXJucyB6ZXJvIGZvciBubyBhY3RpdmUgcmVjb3JkIGFuZCBgQXJpdGhtZXRpY092ZXJmbG93YAppZiB0aGUgcHJvcG9ydGlvbmFsIGNhbGN1bGF0aW9uIGNhbm5vdCBiZSByZXByZXNlbnRlZC4AAAAAAAAUY2FsY3VsYXRlX2V4aXRfc2hhcmUAAAABAAAAAAAAAAZtZW1iZXIAAAAAABMAAAABAAAD6QAAAAsAAAAD",
        "AAAAAAAAARVRdW90ZXMgdGhlIHRlcm1zIGEgbG9hbiBvZiBgYW1vdW50YCB3b3VsZCBjYXJyeSByaWdodCBub3cuIFRoZSBxdW90ZQpjYW4gb25seSBiZSBob25vdXJlZCB3aGVuIGBhbW91bnQgPD0gbWF4X2xvYW5gOiB3aXRoIGFuIGVtcHR5IG9yCm5lZ2F0aXZlIHRyZWFzdXJ5IGBtYXhfbG9hbmAgaXMgMCBhbmQgdGhlIHJhdGUgc2hvd24gaXMgdGhlIHBvbGljeQptYXhpbXVtIGZvciBhIGxvYW4gdGhhdCBjYW5ub3QgYmUgaXNzdWVkLiBTZWUKYGxvYW5zOjpjYWxjdWxhdGVfbG9hbl90ZXJtc2AuAAAAAAAAFGNhbGN1bGF0ZV9sb2FuX3Rlcm1zAAAAAQAAAAAAAAAGYW1vdW50AAAAAAALAAAAAQAAA+kAAAfQAAAACUxvYW5UZXJtcwAAAAAAAAM=",
        "AAAAAAAAAGVXaXRoZHJhd3MgYSBsb2FuIHByb3Bvc2FsIHdoaWxlIGl0IGlzIHN0aWxsIGluIGl0cyBlZGl0aW5nIHBlcmlvZC4KU2VlIGBsb2Fuczo6Y2FuY2VsX2xvYW5fcHJvcG9zYWxgLgAAAAAAABRjYW5jZWxfbG9hbl9wcm9wb3NhbAAAAAIAAAAAAAAACGJvcnJvd2VyAAAAEwAAAAAAAAALcHJvcG9zYWxfaWQAAAAABAAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAAGRDYW5jZWxzIGEgcGVuZGluZyBwb2xpY3kgdXBkYXRlLiBSZXF1aXJlcyBhZG1pbiBhdXRob3JpemF0aW9uIGFuZCBlbWl0cwphIHBvbGljeSBjYW5jZWxsYXRpb24gZXZlbnQuAAAAFGNhbmNlbF9wb2xpY3lfdXBkYXRlAAAAAQAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAAINTdG9yZXMgYSBoaWRkZW4gdHJlYXN1cnktdm90ZSBjb21taXRtZW50LiBgdm90ZXJgIG11c3QgYXV0aG9yaXplIGFuZApjb21taXQgZHVyaW5nIHRoZSBjb25maWd1cmVkIHdpbmRvdzsgZW1pdHMgYSBjb21taXRtZW50IGV2ZW50LgAAAAAUY29tbWl0X3RyZWFzdXJ5X3ZvdGUAAAADAAAAAAAAAAV2b3RlcgAAAAAAABMAAAAAAAAAC3Byb3Bvc2FsX2lkAAAAAAQAAAAAAAAACmNvbW1pdG1lbnQAAAAAA+4AAAAgAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAAJBQZXJtaXNzaW9ubGVzcyBrZWVwZXIgY2FsbDogcGVyc2lzdHMgdGhlIGV4cGlyZWQvcmVqZWN0ZWQgdHJhbnNpdGlvbiBmb3IKYSBsb2FuIHByb3Bvc2FsIHdob3NlIHZvdGluZyB3aW5kb3cgaGFzIHBhc3NlZCB3aXRob3V0IHJlYWNoaW5nIHF1b3J1bS4AAAAUZXhwaXJlX2xvYW5fcHJvcG9zYWwAAAABAAAAAAAAAAtwcm9wb3NhbF9pZAAAAAAEAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAAFNSZXR1cm5zIHRoZSBzcGVuZGFibGUgYmFzZSB0cmVhc3VyeSBiYWxhbmNlLiBSZXR1cm5zCmBOb3RJbml0aWFsaXplZGAgYmVmb3JlIHNldHVwLgAAAAAUZ2V0X3RyZWFzdXJ5X2JhbGFuY2UAAAAAAAAAAQAAA+kAAAALAAAAAw==",
        "AAAAAAAAAH5SZXBvcnRzIHRoZSBmaXJzdCBmYWlsaW5nIGVsaWdpYmlsaXR5IGNoZWNrLiBDaGVja2luZyBhZ2FpbiBhZnRlcgpyZXNvbHZpbmcgdGhhdCBlcnJvciBjYW4gcmV2ZWFsIGFub3RoZXIgZWxpZ2liaWxpdHkgZmFpbHVyZS4AAAAAABRpc19lbGlnaWJsZV9mb3JfbG9hbgAAAAEAAAAAAAAABm1lbWJlcgAAAAAAEwAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAAEBSZXR1cm5zIHdoZXRoZXIgYHRva2VuYCBpcyBjdXJyZW50bHkgYW4gYXBwcm92ZWQgdHJlYXN1cnkgYXNzZXQuAAAAFGlzX3Rva2VuX3doaXRlbGlzdGVkAAAAAQAAAAAAAAAFdG9rZW4AAAAAAAATAAAAAQAAAAE=",
        "AAAAAAAAAJ5SZXZlYWxzIGEgcHJldmlvdXNseSBjb21taXR0ZWQgdHJlYXN1cnkgdm90ZS4gYHZvdGVyYCBtdXN0IGF1dGhvcml6ZQphbmQgcHJvdmlkZSB0aGUgbWF0Y2hpbmcgY29tbWl0bWVudCBpbnB1dHM7IGVtaXRzIHRoZSB0cmVhc3VyeS12b3RlCmV2ZW50IGFmdGVyIHRhbGx5aW5nLgAAAAAAFHJldmVhbF90cmVhc3VyeV92b3RlAAAABAAAAAAAAAAFdm90ZXIAAAAAAAATAAAAAAAAAAtwcm9wb3NhbF9pZAAAAAAEAAAAAAAAAAdzdXBwb3J0AAAAAAEAAAAAAAAABHNhbHQAAAPuAAAAIAAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAAH5FeGVjdXRlcyBhIG1hdHVyZSBwZW5kaW5nIHBvbGljeSB1cGRhdGUuIFJlcXVpcmVzIGFkbWluIGF1dGhvcml6YXRpb24KYW5kIHJldHVybnMgdGltZWxvY2sgb3IgcG9saWN5IGVycm9ycyB3aGVuIG5vdCBlbGlnaWJsZS4AAAAAABVleGVjdXRlX3BvbGljeV91cGRhdGUAAAAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAABAAAD6QAAAAIAAAAD",
        "AAAAAAAAAMlSZXR1cm5zIGBtZW1iZXJgJ3MgbGlmZXRpbWUgbG9hbiB0cmFjayByZWNvcmQ6IHRvdGFsIGxvYW5zIHRha2VuLCBsb2FucwpmdWxseSByZXBhaWQsIGFuZCBsb2FucyBjdXJyZW50bHkgb3V0c3RhbmRpbmcuIFNpbmdsZSBPKDEpIHN0b3JhZ2UgcmVhZDsKcmV0dXJucyBgTm90TWVtYmVyYCBpZiB0aGUgYWRkcmVzcyBoYXMgbmV2ZXIgcmVnaXN0ZXJlZC4AAAAAAAAVZ2V0X21lbWJlcl9sb2FuX3N0YXRzAAAAAAAAAQAAAAAAAAAGbWVtYmVyAAAAAAATAAAAAQAAA+kAAAfQAAAAD01lbWJlckxvYW5TdGF0cwAAAAAD",
        "AAAAAAAAAIFTdGFrZSB0b2tlbnMgcmVxdWlyZWQgcGVyIGFkZGl0aW9uYWwgdW5pdCBvZiB2b3RpbmcgYm9udXMuCkV2ZXJ5IGBzdGFrZV93ZWlnaHRfdW5pdGAgdG9rZW5zIHN0YWtlZCBncmFudHMgKzEgdm90ZSwgdXAgdG8gdGhlIGNhcC4AAAAAAAAVZ2V0X3N0YWtlX3dlaWdodF91bml0AAAAAAAAAAAAAAEAAAAL",
        "AAAAAAAAAMJSZXR1cm5zIGV2ZXJ5IGFjY2VwdGVkIHRyZWFzdXJ5IGFzc2V0IGFuZCBpdHMgY3VycmVudCBjb250cmFjdCBiYWxhbmNlLgoKVGhlIGJhc2UgdG9rZW4gaXMgYWx3YXlzIGluY2x1ZGVkIGZpcnN0IGJ5IGBnZXRfd2hpdGVsaXN0ZWRfdG9rZW5zYCwKZm9sbG93ZWQgYnkgZXZlcnkgY3VycmVudGx5IHdoaXRlbGlzdGVkIGV4dHJhIGFzc2V0LgAAAAAAFWdldF90cmVhc3VyeV9iYWxhbmNlcwAAAAAAAAAAAAABAAAD7AAAABMAAAAL",
        "AAAAAAAAADdSZXR1cm5zIGEgdHJlYXN1cnkgcHJvcG9zYWwgYnkgaWQsIG9yIGBOb25lYCBpZiBhYnNlbnQuAAAAABVnZXRfdHJlYXN1cnlfcHJvcG9zYWwAAAAAAAABAAAAAAAAAAtwcm9wb3NhbF9pZAAAAAAEAAAAAQAAA+gAAAfQAAAAEFRyZWFzdXJ5UHJvcG9zYWw=",
        "AAAAAAAAAGhTdWJtaXRzIGEgdGltZWxvY2tlZCBsb2FuLXBvbGljeSB1cGRhdGUuIFJlcXVpcmVzIGFkbWluIGF1dGhvcml6YXRpb24KYW5kIGVtaXRzIGEgcG9saWN5IHByb3Bvc2FsIGV2ZW50LgAAABVwcm9wb3NlX3BvbGljeV91cGRhdGUAAAAAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAABnBvbGljeQAAAAAH0AAAAApMb2FuUG9saWN5AAAAAAABAAAD6QAAAAIAAAAD",
        "AAAAAAAAADxSZXF1ZXN0cyBhIGxvYW4gZGVub21pbmF0ZWQgaW4gYSBzcGVjaWZpYyB3aGl0ZWxpc3RlZCBhc3NldC4AAAAVcmVxdWVzdF9sb2FuX2luX2Fzc2V0AAAAAAAABAAAAAAAAAAIYm9ycm93ZXIAAAATAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAABWFzc2V0AAAAAAAAEwAAAAAAAAAMbWV0YWRhdGFfY2lkAAAD6AAAABAAAAABAAAD6QAAAAQAAAAD",
        "AAAAAAAAAL9DYXN0cyBgdm90ZXJgJ3Mgc3VwcG9ydCBvciByZWplY3Rpb24gdm90ZSBvbiBhIGxvYW4gcHJvcG9zYWwuCmB2b3RlcmAgbXVzdCBhdXRob3JpemUgYW5kIGJlIGVsaWdpYmxlOyBkdXBsaWNhdGUsIGVuZGVkLCBvciBpbnZhbGlkCnZvdGVzIHJldHVybiBlcnJvcnMgYW5kIGVtaXQgdGhlIGxvYW4tdm90ZSBldmVudCBvbiBzdWNjZXNzLgAAAAAVdm90ZV9vbl9sb2FuX3Byb3Bvc2FsAAAAAAAAAwAAAAAAAAAFdm90ZXIAAAAAAAATAAAAAAAAAAtwcm9wb3NhbF9pZAAAAAAEAAAAAAAAAAdzdXBwb3J0AAAAAAEAAAABAAAD6QAAAAIAAAAD",
        "AAAAAAAAAFdXaXRoZHJhd3MgYWNjcnVlZCB5aWVsZCBpbiB0aGUgc3BlY2lmaWVkIGFzc2V0IHdpdGhvdXQgYWZmZWN0aW5nIG90aGVyCmFzc2V0IHNuYXBzaG90cy4AAAAAFmNsYWltX3Jld2FyZHNfaW5fYXNzZXQAAAAAAAIAAAAAAAAABm1lbWJlcgAAAAAAEwAAAAAAAAAFYXNzZXQAAAAAAAATAAAAAQAAA+kAAAALAAAAAw==",
        "AAAAAAAAAIFQYXlzIG91dCBhbiBhcHByb3ZlZCBsb2FuIHRoYXQgd2FzIHdhaXRpbmcgb24gdHJlYXN1cnkgZnVuZHMuClBlcm1pc3Npb25sZXNzIGtlZXBlciBjYWxsIOKAlCBzZWUgYGxvYW5zOjpkaXNidXJzZV9hcHByb3ZlZF9sb2FuYC4AAAAAAAAWZGlzYnVyc2VfYXBwcm92ZWRfbG9hbgAAAAAAAQAAAAAAAAALcHJvcG9zYWxfaWQAAAAABAAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAAERFeGVjdXRlcyBhIHByZXZpb3VzbHkgcHJvcG9zZWQgdG9rZW4gYWRkaXRpb24gb25jZSBjb25zZW5zdXMgaXMgbWV0LgAAABZleGVjdXRlX3Rva2VuX2FkZGl0aW9uAAAAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAABXRva2VuAAAAAAAAEwAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAADJSZXR1cm5zIHRoZSBmdWxsIGxpc3Qgb2YgYXBwcm92ZWQgdHJlYXN1cnkgYXNzZXRzLgAAAAAAFmdldF93aGl0ZWxpc3RlZF90b2tlbnMAAAAAAAAAAAABAAAD6gAAABM=",
        "AAAAAAAAAINQcm9wb3NlcyBhZGRpbmcgYHRva2VuYCB0byB0aGUgdHJlYXN1cnkncyBzdXBwb3J0ZWQtYXNzZXQgd2hpdGVsaXN0LgpSZXF1aXJlcyBhZG1pbiBjb25zZW5zdXM7IHNlZSBgYWRtaW46OnByb3Bvc2VfdG9rZW5fYWRkaXRpb25gLgAAAAAWcHJvcG9zZV90b2tlbl9hZGRpdGlvbgAAAAAAAgAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAAV0b2tlbgAAAAAAABMAAAABAAAD6QAAAAIAAAAD",
        "AAAAAAAAAGJSZXR1cm5zIHRoZSBnb3Zlcm5hbmNlIGNvbnNlbnN1cyB0aHJlc2hvbGQgaW4gYmFzaXMgcG9pbnRzLgpSZXR1cm5zIGBOb3RJbml0aWFsaXplZGAgYmVmb3JlIHNldHVwLgAAAAAAF2dldF9jb25zZW5zdXNfdGhyZXNob2xkAAAAAAAAAAABAAAD6QAAAAQAAAAD",
        "AAAAAAAAAERSZXR1cm5zIHRoZSBuZXh0IGxvYW4tcHJvcG9zYWwgc2VxdWVuY2UgdmFsdWUgYXMgdGhlIHByb3Bvc2FsIGNvdW50LgAAABdnZXRfbG9hbl9wcm9wb3NhbF9jb3VudAAAAAAAAAAAAQAAAAQ=",
        "AAAAAAAAAY5SZWNvbmNpbGlhdGlvbiB2aWV3IGZvciBgYXNzZXRgICgjMjg4KTogdGhlIGNvbnRyYWN0J3MgYWN0dWFsIHRva2VuCmJhbGFuY2UsIGhvdyBtdWNoIG9mIGl0IGlzIG1lbWJlciBzdGFrZSwgYW5kIHRoZSBwcmluY2lwYWwgc3RpbGwgb3dlZApvbiBhY3RpdmUgbG9hbnMgaW4gdGhhdCBhc3NldC4gYHRva2VuX2JhbGFuY2UgLSBzdGFrZWRgIGVxdWFscwpgZ2V0X3RyZWFzdXJ5X2JhbGFuY2VfaW5fYXNzZXRgOyBjb21wYXJpbmcgdGhlc2UgZmlndXJlcyBhZ2FpbnN0IHRoZQppbmRleGVkIGV2ZW50IGhpc3RvcnkgZXhwb3NlcyBkcmlmdCBvciBkaXJlY3QgZG9uYXRpb25zLiBBbHNvIGFuc3dlcnMKZm9yIGEgcmVtb3ZlZCBhc3NldCwgd2hvc2UgbG9hbnMgbWF5IHN0aWxsIGJlIG91dHN0YW5kaW5nLgAAAAAAF2dldF90cmVhc3VyeV9hY2NvdW50aW5nAAAAAAEAAAAAAAAABWFzc2V0AAAAAAAAEwAAAAEAAAPpAAAH0AAAABJUcmVhc3VyeUFjY291bnRpbmcAAAAAAAM=",
        "AAAAAAAAANVSZXR1cm5zIHVwIHRvIGBsaW1pdGAgdHJlYXN1cnkgcHJvcG9zYWxzIHN0YXJ0aW5nIGF0IHByb3Bvc2FsIGlkIGBzdGFydGAuCklkcyBhcmUgc2VxdWVudGlhbCBmcm9tIDA7IGFic2VudCBlbnRyaWVzIChleHBpcmVkIGZyb20gc3RvcmFnZSkgYXJlCnNraXBwZWQuICBSZXR1cm5zIGFuIGVtcHR5IHZlYyB3aGVuIGBzdGFydGAgaXMgcGFzdCB0aGUgbGFzdCBwcm9wb3NhbC4AAAAAAAAXbGlzdF90cmVhc3VyeV9wcm9wb3NhbHMAAAAAAgAAAAAAAAAFc3RhcnQAAAAAAAAEAAAAAAAAAAVsaW1pdAAAAAAAAAQAAAABAAAD6gAAB9AAAAAQVHJlYXN1cnlQcm9wb3NhbA==",
        "AAAAAAAAAI1VcGRhdGVzIHRoZSBnb3Zlcm5hbmNlIGNvbnNlbnN1cyB0aHJlc2hvbGQuIFRoZSBjYWxsZXIgbXVzdCBiZSBhbiBhZG1pbgphbmQgYXV0aG9yaXplIHRoZSBjYWxsOyBpbnZhbGlkIHRocmVzaG9sZHMgcmV0dXJuIGBJbnZhbGlkVGhyZXNob2xkYC4AAAAAAAAXc2V0X2NvbnNlbnN1c190aHJlc2hvbGQAAAAAAgAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAAl0aHJlc2hvbGQAAAAAAAAEAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAAHNXaXRoZHJhd3MgYSBwZW5kaW5nIHRyZWFzdXJ5IHByb3Bvc2FsIHdpdGhpbiB0aGUgcG9saWN5J3MgZWRpdGluZwpwZXJpb2QuIFNlZSBgdHJlYXN1cnk6OmNhbmNlbF90cmVhc3VyeV9wcm9wb3NhbGAuAAAAABhjYW5jZWxfdHJlYXN1cnlfcHJvcG9zYWwAAAACAAAAAAAAAAhwcm9wb3NlcgAAABMAAAAAAAAAC3Byb3Bvc2FsX2lkAAAAAAQAAAABAAAD6QAAAAIAAAAD",
        "AAAAAAAAAKBQZXJtaXNzaW9ubGVzc2x5IGV4cGlyZXMgYSB0cmVhc3VyeSBwcm9wb3NhbCBhZnRlciBpdHMgdm90aW5nIHdpbmRvdy4KUmV0dXJucyBgTm90SW5Wb3RpbmdQaGFzZWAgZm9yIGFuIGFjdGl2ZSBwcm9wb3NhbCBhbmQgZW1pdHMgYW4gZXhwaXJ5CmV2ZW50IG9uIHRyYW5zaXRpb24uAAAAGGV4cGlyZV90cmVhc3VyeV9wcm9wb3NhbAAAAAEAAAAAAAAAC3Byb3Bvc2FsX2lkAAAAAAQAAAABAAAD6QAAAAIAAAAD",
        "AAAAAAAAAEZDb21taXQgYW5kIHJldmVhbCBib3VuZGFyeSB0aW1lc3RhbXBzIGZvciBhIHByaXZhdGUgdHJlYXN1cnkgcHJvcG9zYWwuAAAAAAAYZ2V0X2NvbW1pdF9yZXZlYWxfd2luZG93AAAAAQAAAAAAAAALcHJvcG9zYWxfaWQAAAAABAAAAAEAAAPpAAAD7QAAAAIAAAAGAAAABgAAAAM=",
        "AAAAAAAAAI1FeGVjdXRlcyBhbiBhcHByb3ZlZCB0cmVhc3VyeSB3aXRoZHJhd2FsIHRoYXQgd2FzIHdhaXRpbmcgb24gdHJlYXN1cnkKZnVuZHMuIFBlcm1pc3Npb25sZXNzIGtlZXBlciBjYWxsIOKAlCBzZWUgYHRyZWFzdXJ5OjpleGVjdXRlX2FwcHJvdmVkYC4AAAAAAAAZZXhlY3V0ZV90cmVhc3VyeV9wcm9wb3NhbAAAAAAAAAEAAAAAAAAAC3Byb3Bvc2FsX2lkAAAAAAQAAAABAAAD6QAAAAIAAAAD",
        "AAAAAAAAAEdSZXR1cm5zIHRoZSBwZW5kaW5nIHRpbWVsb2NrZWQgcG9saWN5IHVwZGF0ZSwgb3IgYE5vbmVgIGlmIG5vbmUgZXhpc3RzLgAAAAAZZ2V0X3BlbmRpbmdfcG9saWN5X3VwZGF0ZQAAAAAAAAAAAAABAAAD6AAAB9AAAAATUGVuZGluZ1BvbGljeVVwZGF0ZQA=",
        "AAAAAAAAAL1DYXN0cyBgdm90ZXJgJ3Mgc3VwcG9ydCBvciByZWplY3Rpb24gdm90ZSBvbiBhIHRyZWFzdXJ5IHByb3Bvc2FsLgpgdm90ZXJgIG11c3QgYXV0aG9yaXplIGFuZCBzYXRpc2Z5IGVsZWN0b3JhdGUgcnVsZXM7IGR1cGxpY2F0ZSBvciBlbmRlZAp2b3RlcyByZXR1cm4gZXJyb3JzIGFuZCBlbWl0IGEgdHJlYXN1cnktdm90ZSBldmVudC4AAAAAAAAZdm90ZV9vbl90cmVhc3VyeV9wcm9wb3NhbAAAAAAAAAMAAAAAAAAABXZvdGVyAAAAAAAAEwAAAAAAAAALcHJvcG9zYWxfaWQAAAAABAAAAAAAAAAHc3VwcG9ydAAAAAABAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAAHhSZXR1cm5zIHRoZSBsZWRnZXIgdGltZSBhdCB3aGljaCBgdG9rZW5gJ3MgcGVuZGluZyBhZGRpdGlvbiBjYW4gYmUKZXhlY3V0ZWQsIG9yIGBOb25lYCBpZiBubyBhZGRpdGlvbiBpcyBwZW5kaW5nIGZvciBpdC4AAAAaZ2V0X3BlbmRpbmdfdG9rZW5fYWRkaXRpb24AAAAAAAEAAAAAAAAABXRva2VuAAAAAAAAEwAAAAEAAAPoAAAABg==",
        "AAAAAAAAADpSZXR1cm5zIGBtZW1iZXJgJ3MgdW5jbGFpbWVkIHlpZWxkIGRlbm9taW5hdGVkIGluIGBhc3NldGAuAAAAAAAaZ2V0X3BlbmRpbmdfeWllbGRfaW5fYXNzZXQAAAAAAAIAAAAAAAAABm1lbWJlcgAAAAAAEwAAAAAAAAAFYXNzZXQAAAAAAAATAAAAAQAAA+kAAAALAAAAAw==",
        "AAAAAAAAAEpMaXN0cyBpbmRleGVkIHBlbmRpbmcgYXNzZXQgYWRkaXRpb25zIHdpdGggdGhlaXIgZWFybGllc3QgZXhlY3V0aW9uIHRpbWVzLgAAAAAAG2dldF9wZW5kaW5nX3Rva2VuX2FkZGl0aW9ucwAAAAAAAAAAAQAAA+oAAAPtAAAAAgAAABMAAAAG",
        "AAAAAAAAAEhSZXR1cm5zIHRoZSBuZXh0IHRyZWFzdXJ5LXByb3Bvc2FsIHNlcXVlbmNlIHZhbHVlIGFzIHRoZSBwcm9wb3NhbCBjb3VudC4AAAAbZ2V0X3RyZWFzdXJ5X3Byb3Bvc2FsX2NvdW50AAAAAAAAAAABAAAABA==",
        "AAAAAAAAAJpDcmVhdGVzIGEgdHJlYXN1cnktd2l0aGRyYXdhbCBwcm9wb3NhbCBpbiB0aGUgYmFzZSBhc3NldC4gYHByb3Bvc2VyYAptdXN0IGF1dGhvcml6ZSBhbmQgZW1pdCB0aGUgcHJvcG9zYWwgZXZlbnQ7IGludmFsaWQgYW1vdW50IG9yIHN0YXRlCnJldHVybnMgYW4gZXJyb3IuAAAAAAAbcHJvcG9zZV90cmVhc3VyeV93aXRoZHJhd2FsAAAAAAUAAAAAAAAACHByb3Bvc2VyAAAAEwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAAtkZXN0aW5hdGlvbgAAAAATAAAAAAAAAAZyZWFzb24AAAAAABAAAAAAAAAAB3ByaXZhdGUAAAAAAQAAAAEAAAPpAAAABAAAAAM=",
        "AAAAAAAAAKhQcm9wb3NlcyBhIHRyZWFzdXJ5IHdpdGhkcmF3YWwgZGVub21pbmF0ZWQgaW4gYSBzcGVjaWZpYyB3aGl0ZWxpc3RlZCBhc3NldC4KTmFtZWQgd2l0aG91dCB0aGUgYHRyZWFzdXJ5X2AgcHJlZml4IGJlY2F1c2UgU29yb2JhbiBjYXBzIGVudHJ5cG9pbnQKbmFtZXMgYXQgMzIgY2hhcmFjdGVycy4AAAAbcHJvcG9zZV93aXRoZHJhd2FsX2luX2Fzc2V0AAAAAAYAAAAAAAAACHByb3Bvc2VyAAAAEwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAAVhc3NldAAAAAAAABMAAAAAAAAAC2Rlc3RpbmF0aW9uAAAAABMAAAAAAAAABnJlYXNvbgAAAAAAEAAAAAAAAAAHcHJpdmF0ZQAAAAABAAAAAQAAA+kAAAAEAAAAAw==",
        "AAAAAAAAAEJSZXR1cm5zIHRoZSB0cmVhc3VyeSBiYWxhbmNlIGhlbGQgaW4gYSBzcGVjaWZpYyB3aGl0ZWxpc3RlZCBhc3NldC4AAAAAAB1nZXRfdHJlYXN1cnlfYmFsYW5jZV9pbl9hc3NldAAAAAAAAAEAAAAAAAAABWFzc2V0AAAAAAAAEwAAAAEAAAAL",
        "AAAABAAAAJZFdmVyeSBmYWlsdXJlIG1vZGUgdGhlIERBTyBjYW4gcmV0dXJuLiBOdW1lcmljIGNvZGVzIGFyZSBzdGFibGUgYW5kIHBhcnQgb2YKdGhlIGNvbnRyYWN0J3MgcHVibGljIEFCSSwgc28gYXBwZW5kIG5ldyB2YXJpYW50cyByYXRoZXIgdGhhbiByZW51bWJlcmluZy4AAAAAAAAAAAAFRXJyb3IAAAAAAABEAAAAAAAAABJBbHJlYWR5SW5pdGlhbGl6ZWQAAAAAAAEAAAAAAAAADk5vdEluaXRpYWxpemVkAAAAAAACAAAAAAAAABBJbnZhbGlkVGhyZXNob2xkAAAAAwAAAAAAAAANSW52YWxpZEFtb3VudAAAAAAAAAQAAAAAAAAAEUludmFsaWRMb2FuUG9saWN5AAAAAAAABQAAAAAAAAAGUGF1c2VkAAAAAAAGAAAAAAAAAAlOb3RQYXVzZWQAAAAAAAAHAAAAAAAAAA1Ob3RBdXRob3JpemVkAAAAAAAACgAAAAAAAAAITm90QWRtaW4AAAALAAAAAAAAAAlOb3RNZW1iZXIAAAAAAAAMAAAAAAAAAAxBbHJlYWR5QWRtaW4AAAANAAAAAAAAAA1BbHJlYWR5TWVtYmVyAAAAAAAADgAAAAAAAAAVQ2Fubm90UmVtb3ZlTGFzdEFkbWluAAAAAAAADwAAAAAAAAAPTWVtYmVyTm90QWN0aXZlAAAAABQAAAAAAAAADUhhc0FjdGl2ZUxvYW4AAAAAAAAVAAAAAAAAABBQcm9wb3NhbE5vdEZvdW5kAAAAHgAAAAAAAAALTm90Qm9ycm93ZXIAAAAAHwAAAAAAAAARTm90SW5FZGl0aW5nUGhhc2UAAAAAAAAgAAAAAAAAABBOb3RJblZvdGluZ1BoYXNlAAAAIQAAAAAAAAALVm90aW5nRW5kZWQAAAAAIgAAAAAAAAAMQWxyZWFkeVZvdGVkAAAAIwAAAAAAAAASTm90RWxpZ2libGVGb3JMb2FuAAAAAAAkAAAAAAAAAA5Db29sZG93bkFjdGl2ZQAAAAAAJQAAAAAAAAAMTG9hbk5vdEZvdW5kAAAAJgAAAAAAAAANTG9hbk5vdEFjdGl2ZQAAAAAAACcAAAAAAAAAFEV4Y2VlZHNUcmVhc3VyeVJhdGlvAAAAKAAAAAAAAAAUSW5zdWZmaWNpZW50VHJlYXN1cnkAAAApAAAAAAAAAA5Mb2FuTm90T3ZlcmR1ZQAAAAAAKgAAAAAAAAAYVHJlYXN1cnlQcm9wb3NhbE5vdEZvdW5kAAAAMgAAAAAAAAAJTmFtZVRha2VuAAAAAAAAPAAAAHJSZXNlcnZlZDogbmV2ZXIgcmV0dXJuZWQuIE5vIGNvbnRyYWN0IGNvZGUgcGF0aCBwcm9kdWNlcyBpdDsga2VwdApiZWNhdXNlIGNvZGVzIGFyZSBhcHBlbmQtb25seSBwdWJsaWMgQUJJICgjMjUyKS4AAAAAAAxOYW1lTm90Rm91bmQAAAA9AAAAAAAAAAdOb1N0YWtlAAAAAD4AAAAAAAAAEUluc3VmZmljaWVudFN0YWtlAAAAAAAAPwAAAAAAAAAMTm9Db21taXRtZW50AAAAQAAAAAAAAAASQ29tbWl0bWVudE1pc21hdGNoAAAAAABBAAAAAAAAAA9BbHJlYWR5UmV2ZWFsZWQAAAAAQgAAAAAAAAAOTm90aGluZ1RvQ2xhaW0AAAAAAEMAAAAAAAAAElByb3Bvc2FsTm90RXhwaXJlZAAAAAAARgAAAAAAAAALSW52YWxpZE5hbWUAAAAARwAAAAAAAAAOTm90WWV0UmV2ZWFsZWQAAAAAAEgAAABeQ2FsbGVyIGlzIG5vdCB0aGUgcHJvcG9zYWwncyBwcm9wb3Nlci9ib3Jyb3dlciBhbmQgbWF5IG5vdCBtb2RpZnkgaXRzCmF0dGFjaGVkIGRvY3VtZW50ICgjMjEpLgAAAAAAEE5vdFByb3Bvc2FsT3duZXIAAABJAAAAAAAAABBEb2N1bWVudFRvb0xhcmdlAAAASgAAAFtUaGUgdG9rZW4gcGFzc2VkIHRvIGBpbml0aWFsaXplYCBpcyBub3QgYSBjb250cmFjdCBpbXBsZW1lbnRpbmcgdGhlCnRva2VuIGludGVyZmFjZSAoIzExNSkuAAAAAAxJbnZhbGlkVG9rZW4AAABLAAAAKlRpbWVsb2NrIGRlbGF5IGhhcyBub3QgZXhwaXJlZCB5ZXQgKCMxOTIpLgAAAAAAElRpbWVsb2NrTm90RXhwaXJlZAAAAAAATAAAAC1ObyBwb2xpY3kgdXBkYXRlIGlzIGN1cnJlbnRseSBwZW5kaW5nICgjMTkyKS4AAAAAAAAPTm9QZW5kaW5nUG9saWN5AAAAAE0AAAAoUHJvcG9zYWwgbWV0YWRhdGEgQ0lEIGlzIGludmFsaWQgKCMxOTQpLgAAABJJbnZhbGlkTWV0YWRhdGFDaWQAAAAAAE4AAAA4SW52YWxpZCBkZWxlZ2F0aW9uIHRhcmdldCAoZS5nLiBzZWxmLWRlbGVnYXRpb24pICgjMTg4KS4AAAARSW52YWxpZERlbGVnYXRpb24AAAAAAABPAAAAAAAAAA9JbnZhbGlkUHJvcG9zYWwAAAAAUAAAAAAAAAANQWxyZWFkeVBhdXNlcgAAAAAAAFEAAAAAAAAACU5vdFBhdXNlcgAAAAAAAFIAAAAyVGhlIGFzc2V0IGlzIG5vdCBvbiB0aGUgdHJlYXN1cnkgd2hpdGVsaXN0ICgjMjM2KS4AAAAAABNUb2tlbk5vdFdoaXRlbGlzdGVkAAAAAFMAAAA2VGhlIGFzc2V0IGlzIGFscmVhZHkgb24gdGhlIHRyZWFzdXJ5IHdoaXRlbGlzdCAoIzIzNikuAAAAAAAXVG9rZW5BbHJlYWR5V2hpdGVsaXN0ZWQAAAAAVAAAAC1ObyBhZGRpdGlvbiBpcyBwZW5kaW5nIGZvciB0aGlzIGFzc2V0ICgjMjM2KS4AAAAAAAAWTm9QZW5kaW5nVG9rZW5BZGRpdGlvbgAAAAAAVQAAAD9UaGUgYXNzZXQgZG9lcyBub3QgbWF0Y2ggdGhlIG9uZSB0aGUgbG9hbiB3YXMgaXNzdWVkIGluICgjMjM2KS4AAAAADUFzc2V0TWlzbWF0Y2gAAAAAAABWAAAAPVRoZSBiYXNlIHRva2VuIHNldCBhdCBpbml0aWFsaXphdGlvbiBjYW4ndCBiZSByZW1vdmVkICgjMjM2KS4AAAAAAAAVQ2Fubm90UmVtb3ZlQmFzZVRva2VuAAAAAAAAVwAAAEFTZWVkaW5nIGlzIGxvY2tlZDsgYGxvY2tfc2VlZGluZ2AgaGFzIGFscmVhZHkgYmVlbiBjYWxsZWQgKCMzMzIpLgAAAAAAAA1TZWVkaW5nTG9ja2VkAAAAAAAAWAAAAFBTZWVkaW5nIGlzIGFscmVhZHkgbG9ja2VkOyBjYWxsaW5nIGBsb2NrX3NlZWRpbmdgIGFnYWluIGlzIGEgbm8tb3AgZXJyb3IgKCMzMzMpLgAAABRTZWVkaW5nQWxyZWFkeUxvY2tlZAAAAFkAAABHTWVtYmVyIGFscmVhZHkgaGFzIHRoZSBtYXhpbXVtIG51bWJlciBvZiBzaW11bHRhbmVvdXNseSBvcGVuIHByb3Bvc2Fscy4AAAAAFFRvb01hbnlPcGVuUHJvcG9zYWxzAAAAWgAAADlBIGJvcnJvd2VyIGNhbm5vdCB2b3RlIG9uIHRoZWlyIG93biBsb2FuIHByb3Bvc2FsICgjMzI3KS4AAAAAAAASQm9ycm93ZXJDYW5ub3RWb3RlAAAAAABbAAAAQUEgd2hpdGVsaXN0ZWQgdG9rZW4gd2l0aCB0cmVhc3VyeSBmdW5kcyBjYW5ub3QgYmUgcmVtb3ZlZCAoIzI3MikuAAAAAAAAD1Rva2VuSGFzQmFsYW5jZQAAAABcAAAAM0EgdG9rZW4gd2l0aCBhY3RpdmUgbG9hbnMgY2Fubm90IGJlIHJlbW92ZWQgKCMyNzMpLgAAAAATVG9rZW5IYXNBY3RpdmVMb2FucwAAAABdAAAAhkEgcmV0dXJuaW5nIG1lbWJlciBzdGlsbCBoYXMgYSBsb2FuIG91dHN0YW5kaW5nIG9yIGEgZGVmYXVsdGVkIGxvYW4gb24KcmVjb3JkLCBzbyB0aGV5IG1heSBub3QgcmVqb2luIGFuZCByZXNldCB0aGVpciBzdGFuZGluZyAoIzE2OCkuAAAAAAAYRGVmYXVsdGVkTG9hbk91dHN0YW5kaW5nAAAAXgAAAERUaGUgdHJlYXN1cnkgYXNzZXQgd2hpdGVsaXN0IGhhcyByZWFjaGVkIGl0cyBib3VuZGVkIG1heGltdW0gKCMyODIpLgAAAA1XaGl0ZWxpc3RGdWxsAAAAAAAAXwAAAEpBbiBpbmJvdW5kIHRva2VuIHRyYW5zZmVyIGRlbGl2ZXJlZCBsZXNzIHRoYW4gdGhlIHJlcXVlc3RlZCBhbW91bnQgKCMyODMpLgAAAAAAFlRyYW5zZmVyQW1vdW50TWlzbWF0Y2gAAAAAAGAAAACNVGhlIHZvdGVyIGpvaW5lZCAob3IgcmVqb2luZWQpIHRoZSBEQU8gYWZ0ZXIgdGhlIHByb3Bvc2FsIHdhcyBjcmVhdGVkLApzbyB0aGV5IGFyZSBvdXRzaWRlIHRoZSBlbGVjdG9yYXRlIGl0cyBxdW9ydW0gd2FzIGZpeGVkIGFnYWluc3QgKCM3MykuAAAAAAAAE0pvaW5lZEFmdGVyUHJvcG9zYWwAAAAAYQAAAEpBZG1pbiBsaXN0IGhhcyByZWFjaGVkIE1BWF9BRE1JTlM7IHJlbW92ZSBvbmUgYmVmb3JlIGFkZGluZyBhbm90aGVyICgjNjgpLgAAAAAADVRvb01hbnlBZG1pbnMAAAAAAABiAAAARkJhdGNoIHZvdGUgY2FsbCBleGNlZWRlZCB0aGUgbWF4aW11bSBudW1iZXIgb2Ygdm90ZXMgYWxsb3dlZCBwZXIgY2FsbC4AAAAAABNUb29NYW55Vm90ZXNJbkJhdGNoAAAAAGMAAABBQ2hlY2tlZCBgaTEyOGAgYXJpdGhtZXRpYyBvdmVyZmxvd2VkOyB0aGUgb3BlcmF0aW9uIHdhcyByZWplY3RlZC4AAAAAAAASQXJpdGhtZXRpY092ZXJmbG93AAAAAABk",
        "AAAAAQAAAAAAAAAAAAAABExvYW4AAAALAAAAAAAAAA1hbW91bnRfcmVwYWlkAAAAAAAACwAAAAAAAAAIYm9ycm93ZXIAAAATAAAAvGBMb2FuUG9saWN5LmRlZmF1bHRfZ3JhY2VfcGVyaW9kYCBpbiBmb3JjZSB3aGVuIHRoaXMgbG9hbiB3YXMKZGlzYnVyc2VkLiBEZWZhdWx0IHRlcm1zIGFyZSBwYXJ0IG9mIHRoZSBvYmxpZ2F0aW9uIHRoZSBib3Jyb3dlcgphY2NlcHRlZCwgc28gYSBsYXRlciBwb2xpY3kgY2hhbmdlIG11c3Qgbm90IGFsdGVyIHRoZW0gKCM3MCkuAAAAFGRlZmF1bHRfZ3JhY2VfcGVyaW9kAAAABgAAAGNgTG9hblBvbGljeS5kZWZhdWx0X3BlbmFsdHlfYnBzYCBpbiBmb3JjZSB3aGVuIHRoaXMgbG9hbiB3YXMgZGlzYnVyc2VkLgpTZWUgYGRlZmF1bHRfZ3JhY2VfcGVyaW9kYC4AAAAAE2RlZmF1bHRfcGVuYWx0eV9icHMAAAAABAAAAAAAAAAIZHVlX3RpbWUAAAAGAAAAAAAAAAJpZAAAAAAABAAAAAAAAAANaW50ZXJlc3RfcmF0ZQAAAAAAAAQAAAAAAAAACXByaW5jaXBhbAAAAAAAAAsAAAAAAAAACnN0YXJ0X3RpbWUAAAAAAAYAAAAAAAAABnN0YXR1cwAAAAAH0AAAAApMb2FuU3RhdHVzAAAAAAAAAAAAD3RvdGFsX3JlcGF5bWVudAAAAAAL",
        "AAAAAQAAAAAAAAAAAAAABk1lbWJlcgAAAAAACgAAAEhMb2FucyBjdXJyZW50bHkgb3V0c3RhbmRpbmcgKGRpc2J1cnNlZCBhbmQgbm90IHlldCByZXBhaWQgb3IgZGVmYXVsdGVkKS4AAAAMYWN0aXZlX2xvYW5zAAAABAAAAAAAAAAHYWRkcmVzcwAAAAATAAAAAAAAAAxjb250cmlidXRpb24AAAALAAAAAAAAAA9oYXNfYWN0aXZlX2xvYW4AAAAAAQAAAAAAAAAJam9pbl90aW1lAAAAAAAABgAAAAAAAAAObGFzdF9sb2FuX3RpbWUAAAAAAAYAAABCTGlmZXRpbWUgY291bnQgb2YgdGhpcyBtZW1iZXIncyBsb2FucyB0aGF0IHJlYWNoZWQgZnVsbCByZXBheW1lbnQuAAAAAAAMcmVwYWlkX2xvYW5zAAAABAAAAAAAAAANc2hhcmVfYmFsYW5jZQAAAAAAAAsAAAAAAAAABnN0YXR1cwAAAAAH0AAAAAxNZW1iZXJTdGF0dXMAAAAxTGlmZXRpbWUgY291bnQgb2YgbG9hbnMgZGlzYnVyc2VkIHRvIHRoaXMgbWVtYmVyLgAAAAAAAAt0b3RhbF9sb2FucwAAAAAE",
        "AAAAAQAAAPdDb21wdXRlZCBsb2FuIHRlcm1zIHJldHVybmVkIGJ5IHRoZSByZWFkLW9ubHkgcXVvdGUgaGVscGVyLgoKQSBxdW90ZSBpcyBvbmx5IGhvbm91cmFibGUgd2hlbiBgYW1vdW50IDw9IG1heF9sb2FuYC4gQWJvdmUgdGhhdCwKYHJlcXVlc3RfbG9hbmAgcmVqZWN0cyB0aGUgYW1vdW50IHdpdGggYEV4Y2VlZHNUcmVhc3VyeVJhdGlvYCwgYW5kIHRoZQpyYXRlIHNob3duIGlzIG5vdCBvbmUgYW55b25lIGNhbiBib3Jyb3cgYXQgKCM4MCkuAAAAAAAAAAAJTG9hblRlcm1zAAAAAAAABAAAAAAAAAAIZHVyYXRpb24AAAAGAAAAAAAAAA1pbnRlcmVzdF9yYXRlAAAAAAAABAAAAMtMYXJnZXN0IHByaW5jaXBhbCBgcmVxdWVzdF9sb2FuYCB3b3VsZCBhY2NlcHQgcmlnaHQgbm93OgpgdHJlYXN1cnlfYmFsYW5jZSDDlyBtYXhfbG9hbl90b190cmVhc3VyeV9yYXRpbyAvIDEwXzAwMGAsIG9yIDAgd2hlbiB0aGUKdHJlYXN1cnkgaXMgZW1wdHkgb3IgbmVnYXRpdmUgKHN0YWtlZCBmdW5kcyBleGNlZWRpbmcgdGhlIHRva2VuCmJhbGFuY2UpLgAAAAAIbWF4X2xvYW4AAAALAAAAAAAAAA90b3RhbF9yZXBheW1lbnQAAAAACw==",
        "AAAAAQAAADhIZWFkbGluZSBEQU8gc3RhdGUgcmV0dXJuZWQgYnkgYGdldF9kYW9fc3VtbWFyeWAgKCMzMzkpLgAAAAAAAAAKRGFvU3VtbWFyeQAAAAAACAAAAAAAAAAOYWN0aXZlX21lbWJlcnMAAAAAAAQAAAAAAAAAE2NvbnNlbnN1c190aHJlc2hvbGQAAAAABAAAAAAAAAAGcGF1c2VkAAAAAAABAAAAAAAAAA10b3RhbF9tZW1iZXJzAAAAAAAABAAAAAAAAAAMdG90YWxfc3Rha2VkAAAACwAAAEtTcGVuZGFibGUgdHJlYXN1cnkgaW4gdGhlIGJhc2UgdG9rZW47IHNhbWUgdmFsdWUgYXMgYGdldF90cmVhc3VyeV9iYWxhbmNlYC4AAAAAEHRyZWFzdXJ5X2JhbGFuY2UAAAALAAAAQFNwZW5kYWJsZSB0cmVhc3VyeSBpbiBldmVyeSB3aGl0ZWxpc3RlZCBhc3NldCwgYmFzZSB0b2tlbiBmaXJzdC4AAAARdHJlYXN1cnlfYmFsYW5jZXMAAAAAAAPqAAAH0AAAAAxBc3NldEJhbGFuY2UAAAAAAAAAB3ZlcnNpb24AAAAAEA==",
        "AAAAAQAAAEpUdW5hYmxlIGxlbmRpbmcgcGFyYW1ldGVycy4gRHVyYXRpb25zIGFyZSBpbiBsZWRnZXIgc2Vjb25kczsgcmF0ZXMgaW4gYnBzLgAAAAAAAAAAAApMb2FuUG9saWN5AAAAAAANAAAAAAAAAA9jb29sZG93bl9wZXJpb2QAAAAABgAAAExFeHRyYSB0aW1lIHBhc3QgYExvYW4uZHVlX3RpbWVgIGJlZm9yZSBhIGxvYW4gYmVjb21lcyBtYXJrYWJsZSBhcyBkZWZhdWx0ZWQuAAAAFGRlZmF1bHRfZ3JhY2VfcGVyaW9kAAAABgAAAExCYXNpcy1wb2ludHMgb2YgdGhlIGRlZmF1bHRpbmcgYm9ycm93ZXIncyBgY29udHJpYnV0aW9uYCBzbGFzaGVkIG9uIGRlZmF1bHQuAAAAE2RlZmF1bHRfcGVuYWx0eV9icHMAAAAABAAAAAAAAAAOZWRpdGluZ19wZXJpb2QAAAAAAAYAAAAAAAAAEW1heF9pbnRlcmVzdF9yYXRlAAAAAAAABAAAAAAAAAARbWF4X2xvYW5fZHVyYXRpb24AAAAAAAAGAAAAAAAAABptYXhfbG9hbl90b190cmVhc3VyeV9yYXRpbwAAAAAABAAAAAAAAAAXbWVtYmVyc2hpcF9jb250cmlidXRpb24AAAAACwAAAAAAAAARbWluX2ludGVyZXN0X3JhdGUAAAAAAAAEAAAAAAAAABdtaW5fbWVtYmVyc2hpcF9kdXJhdGlvbgAAAAAGAAAAAAAAAApxdW9ydW1fYnBzAAAAAAAEAAAAAAAAABJ0cmVhc3VyeV90aHJlc2hvbGQAAAAAAAQAAAAAAAAADXZvdGluZ19wZXJpb2QAAAAAAAAG",
        "AAAAAgAAAAAAAAAAAAAACkxvYW5TdGF0dXMAAAAAAAMAAAAAAAAAAAAAAAZBY3RpdmUAAAAAAAAAAAAAAAAABlJlcGFpZAAAAAAAAAAAAAAAAAAJRGVmYXVsdGVkAAAA",
        "AAAAAQAAADtTcGVuZGFibGUgdHJlYXN1cnkgYmFsYW5jZSBpbiBvbmUgd2hpdGVsaXN0ZWQgYXNzZXQgKCMzMzkpLgAAAAAAAAAADEFzc2V0QmFsYW5jZQAAAAIAAAAAAAAABWFzc2V0AAAAAAAAEwAAAAAAAAAHYmFsYW5jZQAAAAAL",
        "AAAAAQAAAAAAAAAAAAAADExvYW5Qcm9wb3NhbAAAABEAAADCQWN0aXZlLW1lbWJlciBjb3VudCB3aGVuIHRoZSBwcm9wb3NhbCB3YXMgY3JlYXRlZC4gUXVvcnVtIGFuZCB0aGUKZWFybHktcmVqZWN0aW9uIGJvdW5kIGFyZSBjb21wdXRlZCBmcm9tIHRoaXMsIG5vdCB0aGUgbGl2ZSBjb3VudCwgc28KbWVtYmVyc2hpcCBjaHVybiBkdXJpbmcgdGhlIHZvdGUgY2Fubm90IG1vdmUgdGhlIGJhciAoIzczKS4AAAAAAA5hY3RpdmVfbWVtYmVycwAAAAAABAAAAAAAAAANYWdhaW5zdF92b3RlcwAAAAAAAAsAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAAIYm9ycm93ZXIAAAATAAAAAAAAAApjcmVhdGVkX2F0AAAAAAAGAAAAAAAAAAhkdXJhdGlvbgAAAAYAAAAAAAAAEmVkaXRpbmdfcGVyaW9kX2VuZAAAAAAABgAAAAAAAAAJZm9yX3ZvdGVzAAAAAAAACwAAAAAAAAACaWQAAAAAAAQAAAAAAAAADWludGVyZXN0X3JhdGUAAAAAAAAEAAAASVRpbWVzdGFtcCBvZiB0aGUgbGFzdCBgZWRpdF9sb2FuX3Byb3Bvc2FsYCBjYWxsIChgTm9uZWAgaWYgbmV2ZXIgZWRpdGVkKS4AAAAAAAAObGFzdF9lZGl0ZWRfYXQAAAAAA+gAAAAGAAAAAAAAAAxtZXRhZGF0YV9jaWQAAAPoAAAAEAAAAAAAAAAFcGhhc2UAAAAAAAfQAAAADVByb3Bvc2FsUGhhc2UAAAAAAAAAAAAABnN0YXR1cwAAAAAH0AAAAA5Qcm9wb3NhbFN0YXR1cwAAAAAAAAAAAA90b3RhbF9yZXBheW1lbnQAAAAACwAAAAAAAAAKdm90ZXNfY2FzdAAAAAAABAAAAAAAAAANdm90aW5nX3BlcmlvZAAAAAAAAAY=",
        "AAAAAgAAAAAAAAAAAAAADE1lbWJlclN0YXR1cwAAAAIAAAAAAAAAAAAAAAxBY3RpdmVNZW1iZXIAAAAAAAAAAAAAAAhJbmFjdGl2ZQ==",
        "AAAAAQAAAAAAAAAAAAAADFByb3Bvc2FsVm90ZQAAAAIAAAAAAAAAC3Byb3Bvc2FsX2lkAAAAAAQAAAAAAAAABHZvdGUAAAAB",
        "AAAAAQAAAAAAAAAAAAAADFNlZWREb2NFbnRyeQAAAAMAAAAAAAAADGNvbnRlbnRfaGFzaAAAAA4AAAAAAAAABGtpbmQAAAfQAAAADFByb3Bvc2FsS2luZAAAAAAAAAALcHJvcG9zYWxfaWQAAAAABA==",
        "AAAAAgAAAAAAAAAAAAAADVByb3Bvc2FsUGhhc2UAAAAAAAAEAAAAAAAAAAAAAAAHRWRpdGluZwAAAAAAAAAAAAAAAAZWb3RpbmcAAAAAAAAAAAAAAAAACEV4ZWN1dGVkAAAAAAAAAAAAAAAHRXhwaXJlZAA=",
        "AAAAAQAAAAAAAAAAAAAADVNlZWROYW1lRW50cnkAAAAAAAACAAAAAAAAAARuYW1lAAAAEAAAAAAAAAAFb3duZXIAAAAAAAAT",
        "AAAAAQAAAkhFdmVyeXRoaW5nIHRoZSBEQU8ga25vd3MgYWJvdXQgb25lIG1lbWJlciwgcmV0dXJuZWQgYnkKYGdldF9tZW1iZXJfb3ZlcnZpZXdgIGluIGEgc2luZ2xlIGNhbGwuCgpDb252ZW5pZW5jZSBhZ2dyZWdhdGlvbiB2aWV3IGNvbWJpbmluZyBpbmRlcGVuZGVudGx5IHF1ZXJ5YWJsZSBmaWVsZHM6Ci0gYG1lbWJlcmA6IGdlbmVyYWwgbWVtYmVyIHN0YXR1cyBhbmQgbWV0YWRhdGEKLSBgc3Rha2VgOiBtYXRjaGVzIGBnZXRfc3Rha2VgCi0gYHBlbmRpbmdfeWllbGRgOiBtYXRjaGVzIGBnZXRfcGVuZGluZ195aWVsZGAKLSBgbG9hbl9zdGF0c2A6IG1hdGNoZXMgYGdldF9tZW1iZXJfbG9hbl9zdGF0c2AKLSBgbmFtZWA6IG1hdGNoZXMgYG5hbWVfb2ZgCi0gYGRlbGVnYXRlYDogbWF0Y2hlcyBgZ2V0X2RlbGVnYXRlYAoKTWFpbnRlbmFuY2Ugb2JsaWdhdGlvbjogRXZlcnkgZnV0dXJlIGFkZGl0aW9uIHRvIGFueSBvZiB0aGUgdW5kZXJseWluZwppbmRpdmlkdWFsIG1lbWJlciB2aWV3cyBtdXN0IGhhdmUgYSBtYXRjaGluZyBmaWVsZCBhZGRlZCBoZXJlIHRvIGVuc3VyZQp0aGUgYWdncmVnYXRlIHZpZXcgcmVtYWlucyBpbiBzeW5jLgAAAAAAAAAOTWVtYmVyT3ZlcnZpZXcAAAAAAAYAAAA0QWRkcmVzcyB0aGlzIG1lbWJlciBkZWxlZ2F0ZWQgdGhlaXIgdm90ZSB0bywgaWYgYW55LgAAAAhkZWxlZ2F0ZQAAA+gAAAATAAAAAAAAAApsb2FuX3N0YXRzAAAAAAfQAAAAD01lbWJlckxvYW5TdGF0cwAAAAAAAAAABm1lbWJlcgAAAAAH0AAAAAZNZW1iZXIAAAAAAChOYW1lIHJlZ2lzdGVyZWQgdG8gdGhpcyBhZGRyZXNzLCBpZiBhbnkuAAAABG5hbWUAAAPoAAAAEAAAAD9JbnRlcmVzdCB5aWVsZCBhY2NydWVkIGFuZCBub3QgeWV0IGNsYWltZWQgdmlhIGBjbGFpbV9yZXdhcmRzYC4AAAAADXBlbmRpbmdfeWllbGQAAAAAAAALAAAAN1Rva2VucyB0aGUgbWVtYmVyIGhhcyBzdGFrZWQgZm9yIGEgdm90aW5nLXdlaWdodCBib251cy4AAAAABXN0YWtlAAAAAAAACw==",
        "AAAAAgAAAAAAAAAAAAAADlByb3Bvc2FsU3RhdHVzAAAAAAAHAAAAAAAAAAAAAAAHUGVuZGluZwAAAAAAAAAAAAAAABtBcHByb3ZlZFBlbmRpbmdEaXNidXJzZW1lbnQAAAAAAAAAAAAAAAAIQXBwcm92ZWQAAAAAAAAAAAAAAAhSZWplY3RlZAAAAAAAAAAAAAAACEV4ZWN1dGVkAAAAAAAAAAAAAAAHRXhwaXJlZAAAAAAAAAAAAAAAAAlDYW5jZWxsZWQAAAA=",
        "AAAAAQAAAAAAAAAAAAAADlNlZWRTdGFrZUVudHJ5AAAAAAACAAAAAAAAAAdhZGRyZXNzAAAAABMAAAAAAAAABmFtb3VudAAAAAAACw==",
        "AAAAAQAAAOBQZW5kaW5nIHlpZWxkIGZvciBhIG1lbWJlci4gYGFtb3VudGAgaXMgc3RvcmVkIGFzIHRoZSBuZWdhdGl2ZSB5aWVsZCBzbmFwc2hvdApzbyB0aGUgZXhpc3RpbmcgYGdldF9wZW5kaW5nX3lpZWxkYCBmb3JtdWxhIGAoYWNjdW11bGF0b3IgLSBzbmFwc2hvdCkubWF4KDApYApyZWNvbnN0cnVjdHMgdGhlIGNvcnJlY3QgdmFsdWUgZnJvbSBhIGZyZXNoIGFjY3VtdWxhdG9yIG9mIDAgKCMzMzIpLgAAAAAAAAAOU2VlZFlpZWxkRW50cnkAAAAAAAIAAAAAAAAAB2FkZHJlc3MAAAAAEwAAAAAAAAAGYW1vdW50AAAAAAAL",
        "AAAAAQAAAH1TdHJ1Y3R1cmVkIGV2ZW50IGVtaXR0ZWQgd2hlbiBhIGJvcnJvd2VyIGVkaXRzIGxvYW4gcHJvcG9zYWwgdGVybXMuCkNhcHR1cmVzIHByaW9yIGFuZCB1cGRhdGVkIHRlcm1zIHBsdXMgdGhlIGVkaXQgdGltZXN0YW1wLgAAAAAAAAAAAAAPTG9hblRlcm1zRWRpdGVkAAAAAAcAAAAAAAAACGJvcnJvd2VyAAAAEwAAAAAAAAAJZWRpdGVkX2F0AAAAAAAABgAAAAAAAAAKbmV3X2Ftb3VudAAAAAAACwAAAAAAAAALcHJldl9hbW91bnQAAAAACwAAAAAAAAAUcHJldl90b3RhbF9yZXBheW1lbnQAAAALAAAAAAAAAAtwcm9wb3NhbF9pZAAAAAAEAAAAAAAAAA90b3RhbF9yZXBheW1lbnQAAAAACw==",
        "AAAAAQAAAHRPbi1jaGFpbiBjcmVkaXQgdHJhY2sgcmVjb3JkIGZvciBhIG1lbWJlci4gUmV0dXJuZWQgYnkKYGdldF9tZW1iZXJfbG9hbl9zdGF0c2AgYXMgYW4gTygxKSByZWFkIG9mIHRoZSBtZW1iZXIgcmVjb3JkLgAAAAAAAAAPTWVtYmVyTG9hblN0YXRzAAAAAAMAAAAAAAAADGFjdGl2ZV9sb2FucwAAAAQAAAAAAAAADHJlcGFpZF9sb2FucwAAAAQAAAAAAAAAC3RvdGFsX2xvYW5zAAAAAAQ=",
        "AAAAAQAAAAAAAAAAAAAAEFRyZWFzdXJ5UHJvcG9zYWwAAAAOAAAAW0FjdGl2ZS1tZW1iZXIgY291bnQgd2hlbiB0aGUgcHJvcG9zYWwgd2FzIGNyZWF0ZWQuIFNlZQpgTG9hblByb3Bvc2FsLmFjdGl2ZV9tZW1iZXJzYCAoIzczKS4AAAAADmFjdGl2ZV9tZW1iZXJzAAAAAAAEAAAAAAAAAA1hZ2FpbnN0X3ZvdGVzAAAAAAAACwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAApjcmVhdGVkX2F0AAAAAAAGAAAAAAAAAAtkZXN0aW5hdGlvbgAAAAATAAAAAAAAAAlmb3Jfdm90ZXMAAAAAAAALAAAAAAAAAAJpZAAAAAAABAAAAElXaGVuIHRydWUsIHZvdGVzIG11c3QgYmUgY29tbWl0dGVkIHRoZW4gcmV2ZWFsZWQgKGNvbW1pdC1yZXZlYWwgcHJpdmFjeSkuAAAAAAAAB3ByaXZhdGUAAAAAAQAAAAAAAAAIcHJvcG9zZXIAAAATAAAAAAAAAAZyZWFzb24AAAAAABAAAAAAAAAABnN0YXR1cwAAAAAH0AAAAA5Qcm9wb3NhbFN0YXR1cwAAAAAAAAAAABJ0cmVhc3VyeV90aHJlc2hvbGQAAAAAAAQAAAAAAAAACnZvdGVzX2Nhc3QAAAAAAAQAAAAAAAAADXZvdGluZ19wZXJpb2QAAAAAAAAG",
        "AAAAAQAAAKVQZXItYXNzZXQgcmVjb25jaWxpYXRpb24gZmlndXJlcyByZXR1cm5lZCBieSBgZ2V0X3RyZWFzdXJ5X2FjY291bnRpbmdgCigjMjg4KSwgc28gYXVkaXRvcnMgY2FuIGNvbXBhcmUgd2hhdCB0aGUgY29udHJhY3QgYWNjb3VudHMgZm9yIGFnYWluc3QKd2hhdCBpdCBhY3R1YWxseSBob2xkcy4AAAAAAAAAAAAAElRyZWFzdXJ5QWNjb3VudGluZwAAAAAAAwAAAF5QcmluY2lwYWwgc3RpbGwgb3dlZCBvbiBgQWN0aXZlYCBsb2FucyBpbiB0aGUgYXNzZXQuIExlbnQgb3V0LCBzbyBub3QKcGFydCBvZiBgdG9rZW5fYmFsYW5jZWAuAAAAAAAVb3V0c3RhbmRpbmdfcHJpbmNpcGFsAAAAAAAACwAAAH9Qb3J0aW9uIG9mIGB0b2tlbl9iYWxhbmNlYCBoZWxkIGFzIG1lbWJlciBzdGFrZS4gU3Rha2luZyBvbmx5IGhhcHBlbnMKaW4gdGhlIGJhc2UgdG9rZW4sIHNvIHRoaXMgaXMgemVybyBmb3IgZXZlcnkgb3RoZXIgYXNzZXQuAAAAAAZzdGFrZWQAAAAAAAsAAABFVGhlIGNvbnRyYWN0J3MgYWN0dWFsIGJhbGFuY2Ugb2YgdGhlIGFzc2V0LCBhcyByZXBvcnRlZCBieSBpdHMgdG9rZW4uAAAAAAAADXRva2VuX2JhbGFuY2UAAAAAAAAL",
        "AAAAAQAAADdQZW5kaW5nIHBvbGljeSB1cGRhdGUgdW5kZXJnb2luZyB0aW1lbG9jayBkZWxheSAoIzE5MikuAAAAAAAAAAATUGVuZGluZ1BvbGljeVVwZGF0ZQAAAAADAAAAAAAAAA5leGVjdXRpb25fdGltZQAAAAAABgAAAAAAAAAGcG9saWN5AAAAAAfQAAAACkxvYW5Qb2xpY3kAAAAAAAAAAAALcHJvcG9zZWRfYXQAAAAABg==",
        "AAAAAQAAADpTdHJ1Y3R1cmVkIGV2ZW50IGVtaXR0ZWQgdXBvbiBzdGFraW5nIHJld2FyZCBjbGFpbSAoIzE5MykuAAAAAAAAAAAAFFN0YWtpbmdSZXdhcmRDbGFpbWVkAAAAAwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAAZtZW1iZXIAAAAAABMAAAAAAAAACXRpbWVzdGFtcAAAAAAAAAY=",
        "AAAAAgAAAE9EaXN0aW5ndWlzaGVzIHdoaWNoIHByb3Bvc2FsIGZhbWlseSBhIHNoYXJlZCByZWZlcmVuY2UgKGRvY3MsIGNvbW1pdHMpIHRhcmdldHMuAAAAAAAAAAAMUHJvcG9zYWxLaW5kAAAAAgAAAAAAAAAAAAAABExvYW4AAAAAAAAAAAAAAAhUcmVhc3VyeQ==" ]),
      options
    )
  }
  public readonly fromJSON = {
    pause: this.txFromJSON<Result<void>>,
        stake: this.txFromJSON<Result<void>>,
        name_of: this.txFromJSON<Result<Option<string>>>,
        unpause: this.txFromJSON<Result<void>>,
        unstake: this.txFromJSON<Result<void>>,
        exit_dao: this.txFromJSON<Result<void>>,
        get_loan: this.txFromJSON<Option<Loan>>,
        is_admin: this.txFromJSON<Result<boolean>>,
        add_admin: this.txFromJSON<Result<void>>,
        get_stake: this.txFromJSON<i128>,
        get_token: this.txFromJSON<Result<string>>,
        has_voted: this.txFromJSON<boolean>,
        is_member: this.txFromJSON<boolean>,
        is_paused: this.txFromJSON<boolean>,
        seed_docs: this.txFromJSON<Result<void>>,
        get_admins: this.txFromJSON<Result<Array<string>>>,
        get_member: this.txFromJSON<Option<Member>>,
        initialize: this.txFromJSON<Result<void>>,
        list_loans: this.txFromJSON<Array<Loan>>,
        repay_loan: this.txFromJSON<Result<void>>,
        seed_loans: this.txFromJSON<Result<void>>,
        seed_names: this.txFromJSON<Result<void>>,
        set_pauser: this.txFromJSON<Result<void>>,
        vote_batch: this.txFromJSON<Result<void>>,
        get_version: this.txFromJSON<string>,
        seed_stakes: this.txFromJSON<Result<void>>,
        bump_dao_ttl: this.txFromJSON<null>,
        get_delegate: this.txFromJSON<Option<string>>,
        get_document: this.txFromJSON<Result<Option<Buffer>>>,
        list_members: this.txFromJSON<Array<Member>>,
        lock_seeding: this.txFromJSON<Result<void>>,
        remove_admin: this.txFromJSON<Result<void>>,
        remove_token: this.txFromJSON<Result<void>>,
        request_loan: this.txFromJSON<Result<u32>>,
        resolve_name: this.txFromJSON<Result<Option<string>>>,
        seed_members: this.txFromJSON<Result<void>>,
        claim_rewards: this.txFromJSON<Result<i128>>,
        delegate_vote: this.txFromJSON<Result<void>>,
        register_name: this.txFromJSON<Result<void>>,
        revoke_pauser: this.txFromJSON<Result<void>>,
        is_initialized: this.txFromJSON<boolean>,
        attach_document: this.txFromJSON<Result<void>>,
        get_dao_summary: this.txFromJSON<DaoSummary>,
        get_loan_policy: this.txFromJSON<Result<LoanPolicy>>,
        register_member: this.txFromJSON<Result<void>>,
        set_loan_policy: this.txFromJSON<Result<void>>,
        get_loan_proposal: this.txFromJSON<Option<LoanProposal>>,
        get_pending_yield: this.txFromJSON<Result<i128>>,
        get_total_members: this.txFromJSON<u32>,
        get_voting_weight: this.txFromJSON<i128>,
        is_seeding_locked: this.txFromJSON<boolean>,
        verify_commitment: this.txFromJSON<boolean>,
        edit_loan_proposal: this.txFromJSON<Result<void>>,
        get_active_members: this.txFromJSON<u32>,
        repay_loan_partial: this.txFromJSON<Result<void>>,
        seed_pending_yield: this.txFromJSON<Result<void>>,
        get_max_stake_bonus: this.txFromJSON<i128>,
        get_member_overview: this.txFromJSON<Result<MemberOverview>>,
        mark_loan_defaulted: this.txFromJSON<Result<void>>,
        repay_loan_in_asset: this.txFromJSON<Result<void>>,
        calculate_exit_share: this.txFromJSON<Result<i128>>,
        calculate_loan_terms: this.txFromJSON<Result<LoanTerms>>,
        cancel_loan_proposal: this.txFromJSON<Result<void>>,
        cancel_policy_update: this.txFromJSON<Result<void>>,
        commit_treasury_vote: this.txFromJSON<Result<void>>,
        expire_loan_proposal: this.txFromJSON<Result<void>>,
        get_treasury_balance: this.txFromJSON<Result<i128>>,
        is_eligible_for_loan: this.txFromJSON<Result<void>>,
        is_token_whitelisted: this.txFromJSON<boolean>,
        reveal_treasury_vote: this.txFromJSON<Result<void>>,
        execute_policy_update: this.txFromJSON<Result<void>>,
        get_member_loan_stats: this.txFromJSON<Result<MemberLoanStats>>,
        get_stake_weight_unit: this.txFromJSON<i128>,
        get_treasury_balances: this.txFromJSON<Map<string, i128>>,
        get_treasury_proposal: this.txFromJSON<Option<TreasuryProposal>>,
        propose_policy_update: this.txFromJSON<Result<void>>,
        request_loan_in_asset: this.txFromJSON<Result<u32>>,
        vote_on_loan_proposal: this.txFromJSON<Result<void>>,
        claim_rewards_in_asset: this.txFromJSON<Result<i128>>,
        disburse_approved_loan: this.txFromJSON<Result<void>>,
        execute_token_addition: this.txFromJSON<Result<void>>,
        get_whitelisted_tokens: this.txFromJSON<Array<string>>,
        propose_token_addition: this.txFromJSON<Result<void>>,
        get_consensus_threshold: this.txFromJSON<Result<u32>>,
        get_loan_proposal_count: this.txFromJSON<u32>,
        get_treasury_accounting: this.txFromJSON<Result<TreasuryAccounting>>,
        list_treasury_proposals: this.txFromJSON<Array<TreasuryProposal>>,
        set_consensus_threshold: this.txFromJSON<Result<void>>,
        cancel_treasury_proposal: this.txFromJSON<Result<void>>,
        expire_treasury_proposal: this.txFromJSON<Result<void>>,
        get_commit_reveal_window: this.txFromJSON<Result<readonly [u64, u64]>>,
        execute_treasury_proposal: this.txFromJSON<Result<void>>,
        get_pending_policy_update: this.txFromJSON<Option<PendingPolicyUpdate>>,
        vote_on_treasury_proposal: this.txFromJSON<Result<void>>,
        get_pending_token_addition: this.txFromJSON<Option<u64>>,
        get_pending_yield_in_asset: this.txFromJSON<Result<i128>>,
        get_pending_token_additions: this.txFromJSON<Array<readonly [string, u64]>>,
        get_treasury_proposal_count: this.txFromJSON<u32>,
        propose_treasury_withdrawal: this.txFromJSON<Result<u32>>,
        propose_withdrawal_in_asset: this.txFromJSON<Result<u32>>,
        get_treasury_balance_in_asset: this.txFromJSON<i128>
  }
}