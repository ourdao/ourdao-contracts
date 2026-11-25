use soroban_sdk::{contracttype, Address, BytesN, Env, IntoVal, String, Val, Vec};

use crate::error::Error;
use crate::types::{Loan, LoanPolicy, LoanProposal, Member, PendingPolicyUpdate, TreasuryProposal};

// Soroban produces one ledger every ~5 seconds.
const DAY_IN_LEDGERS: u32 = 17_280;
const INSTANCE_BUMP: u32 = 30 * DAY_IN_LEDGERS;
const INSTANCE_THRESHOLD: u32 = INSTANCE_BUMP - DAY_IN_LEDGERS;
const PERSISTENT_BUMP: u32 = 90 * DAY_IN_LEDGERS;
const PERSISTENT_THRESHOLD: u32 = PERSISTENT_BUMP - DAY_IN_LEDGERS;

/// Distinguishes which proposal family a shared reference (docs, commits) targets.
///
/// Extension contract (#459): every new proposal-like feature must either
/// (a) append a variant here and give it a real arm in each `match` in
/// `docs.rs` (`proposal_exists`, `proposal_owner`, and the editing-phase check
/// in `attach_document`), or (b) be documented on its variant or in its ADR as
/// deliberately excluded from document attachment, with `attach_document`
/// rejecting that kind with `InvalidProposal` (as ADR-0004 plans for
/// `LoanExtension`). The compiler also flags the matches in `has_voted` and
/// `migration::seed_docs`. Never rename or remove a variant: they encode by
/// name in stored keys and events.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProposalKind {
    Loan,
    Treasury,
}

/// Storage keys. Crate-private on purpose: `#[contracttype]` only writes
/// `pub` types into the contract spec, and these keys are an internal storage
/// layout, not part of the callable interface. Keeping them out of the spec
/// saves ~1.5 KB of wasm (#357). Variants still encode by name on the ledger, so
/// renaming one orphans every entry already stored under it.
#[contracttype]
#[derive(Clone)]
pub(crate) enum DataKey {
    // ---- singletons (instance storage) ----
    Admins,
    Threshold,
    MembershipFee,
    Token,
    Policy,
    Paused,
    Pauser,
    Members,
    TotalMembers,
    ActiveMembers,
    NextProposalId,
    NextTreasuryId,
    TotalStaked,
    PendingPolicyUpdate,
    /// Monotonic id for token-whitelist governance proposals/events (#284).
    NextTokenProposalId,

    // ---- per-entity (persistent storage) ----
    Member(Address),
    LoanProposal(u32),
    Loan(u32),
    TreasuryProposal(u32),
    LoanVoted(u32, Address),
    StakeTime(Address),
    TreasuryVoted(u32, Address),
    PendingYield(Address),
    Stake(Address),
    /// Cached count, refreshed from live proposal state before quota checks.
    OpenProposals(Address),
    // ---- pull-based yield accrual ----
    // Keep these original base-token keys for lazy reads during migration.
    YieldAccumulator,
    YieldRemainder,
    MemberYieldSnapshot(Address),
    // staking, registry, and document modules
    Doc(ProposalKind, u32),
    Name(String),
    NameOf(Address),
    Commit(u32, Address),
    TotalContributions,
    /// Running sum of `contribution` over members whose status is
    /// `ActiveMember` (#353). Kept in sync by register / exit / default.
    ActiveContributions,
    Delegation(Address),
    // ---- multi-token treasury (#236) ----
    /// Present (and `true`) while an extra asset is whitelisted. The base
    /// token is implicitly whitelisted and never stored here.
    TokenWhitelisted(Address),
    /// Extra whitelisted assets, in the order they were added.
    WhitelistedTokens,
    /// Assets that have ever accrued member yield, including removed assets.
    YieldAssets,
    /// Earliest timestamp a proposed asset addition may be executed.
    PendingTokenAddition(Address),
    /// Stable proposal id assigned when an extra token is first proposed.
    TokenProposalId(Address),
    /// Asset a loan proposal (and the loan it becomes, which shares its id)
    /// is denominated in. Absent means the base token.
    LoanAsset(u32),
    /// Asset a treasury proposal pays out in. Absent means the base token.
    TreasuryAsset(u32),
    /// Set (to `true`) once by `lock_seeding`; never cleared. Seed entrypoints
    /// reject any call made after this flag is present (#332/#333).
    MigrationSeedingLocked,
    // ---- treasury reconciliation (#288) ----
    /// Principal still owed on `Active` loans denominated in this asset.
    /// Absent means zero.
    OutstandingPrincipal(Address),
    /// Number of active loans denominated in this asset. Absent means zero.
    ActiveLoansInAsset(Address),
    /// Addresses with indexed pending token additions, in proposal order.
    PendingTokenAdditions,
}

#[contracttype]
#[derive(Clone)]
pub(crate) enum YieldKey {
    YieldAccumulator(Address),
    YieldRemainder(Address),
    MemberYieldSnapshot(Address, Address),
}

pub fn extend_instance(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_THRESHOLD, INSTANCE_BUMP);
}

fn extend_persistent<K>(env: &Env, key: &K)
where
    K: IntoVal<Env, Val> + Sized,
{
    env.storage()
        .persistent()
        .extend_ttl(key, PERSISTENT_THRESHOLD, PERSISTENT_BUMP);
}

// ---------- singleton config accessors ----------

pub fn is_initialized(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Admins)
}

pub fn get_admins(env: &Env) -> Result<Vec<Address>, Error> {
    env.storage()
        .instance()
        .get(&DataKey::Admins)
        .ok_or(Error::NotInitialized)
}

pub fn set_admins(env: &Env, admins: &Vec<Address>) {
    env.storage().instance().set(&DataKey::Admins, admins);
}

pub fn get_pauser(env: &Env) -> Option<Address> {
    env.storage().instance().get(&DataKey::Pauser)
}

pub fn set_pauser(env: &Env, pauser: &Address) {
    env.storage().instance().set(&DataKey::Pauser, pauser);
}

pub fn remove_pauser(env: &Env) {
    env.storage().instance().remove(&DataKey::Pauser);
}

pub fn get_threshold(env: &Env) -> Result<u32, Error> {
    env.storage()
        .instance()
        .get(&DataKey::Threshold)
        .ok_or(Error::NotInitialized)
}

pub fn set_threshold(env: &Env, t: u32) {
    env.storage().instance().set(&DataKey::Threshold, &t);
}

pub fn get_membership_fee(env: &Env) -> Result<i128, Error> {
    env.storage()
        .instance()
        .get(&DataKey::MembershipFee)
        .ok_or(Error::NotInitialized)
}

pub fn set_membership_fee(env: &Env, fee: i128) {
    env.storage().instance().set(&DataKey::MembershipFee, &fee);
}

pub fn get_token(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .get(&DataKey::Token)
        .ok_or(Error::NotInitialized)
}

pub fn set_token(env: &Env, token: &Address) {
    env.storage().instance().set(&DataKey::Token, token);
}

pub fn get_policy(env: &Env) -> Result<LoanPolicy, Error> {
    env.storage()
        .instance()
        .get(&DataKey::Policy)
        .ok_or(Error::NotInitialized)
}

pub fn set_policy(env: &Env, policy: &LoanPolicy) {
    env.storage().instance().set(&DataKey::Policy, policy);
}

pub fn get_pending_policy_update(env: &Env) -> Option<PendingPolicyUpdate> {
    env.storage().instance().get(&DataKey::PendingPolicyUpdate)
}

pub fn set_pending_policy_update(env: &Env, update: &PendingPolicyUpdate) {
    env.storage()
        .instance()
        .set(&DataKey::PendingPolicyUpdate, update);
}

pub fn remove_pending_policy_update(env: &Env) {
    env.storage()
        .instance()
        .remove(&DataKey::PendingPolicyUpdate);
}

pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false)
}

pub fn set_paused(env: &Env, paused: bool) {
    env.storage().instance().set(&DataKey::Paused, &paused);
}

pub fn get_members(env: &Env) -> Vec<Address> {
    env.storage()
        .instance()
        .get(&DataKey::Members)
        .unwrap_or_else(|| Vec::new(env))
}

pub fn set_members(env: &Env, members: &Vec<Address>) {
    env.storage().instance().set(&DataKey::Members, members);
}

pub fn get_total_members(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::TotalMembers)
        .unwrap_or(0)
}

pub fn set_total_members(env: &Env, n: u32) {
    env.storage().instance().set(&DataKey::TotalMembers, &n);
}

#[allow(dead_code)]
pub fn get_total_contributions(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::TotalContributions)
        .unwrap_or(0)
}

#[allow(dead_code)]
pub fn set_total_contributions(env: &Env, amount: i128) {
    env.storage()
        .instance()
        .set(&DataKey::TotalContributions, &amount);
}

pub fn get_active_contributions(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::ActiveContributions)
        .unwrap_or(0)
}

pub fn set_active_contributions(env: &Env, amount: i128) {
    env.storage()
        .instance()
        .set(&DataKey::ActiveContributions, &amount);
}

pub fn get_active_members(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::ActiveMembers)
        .unwrap_or(0)
}

pub fn set_active_members(env: &Env, n: u32) {
    env.storage().instance().set(&DataKey::ActiveMembers, &n);
}

/// Atomically fetches and increments one of the id counters, returning the id to use.
pub fn next_id(env: &Env, key: DataKey) -> u32 {
    let current: u32 = env.storage().instance().get(&key).unwrap_or(0);
    env.storage().instance().set(&key, &(current + 1));
    current
}

pub fn get_proposal_count(env: &Env, key: DataKey) -> u32 {
    env.storage().instance().get(&key).unwrap_or(0)
}

pub fn get_total_staked(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::TotalStaked)
        .unwrap_or(0)
}

pub fn set_total_staked(env: &Env, amount: i128) {
    env.storage().instance().set(&DataKey::TotalStaked, &amount);
}

// ---------- per-entity accessors ----------

pub fn get_member(env: &Env, addr: &Address) -> Option<Member> {
    let key = DataKey::Member(addr.clone());
    let m = env.storage().persistent().get(&key);
    if m.is_some() {
        extend_persistent(env, &key);
    }
    m
}

pub fn set_member(env: &Env, member: &Member) {
    let key = DataKey::Member(member.address.clone());
    env.storage().persistent().set(&key, member);
    extend_persistent(env, &key);
}

/// Bound how many unclosed proposals one member may have at a time.
pub const MAX_OPEN_PROPOSALS_PER_MEMBER: u32 = 3;
/// Maximum number of loan proposals accepted by one batch vote.
pub const MAX_VOTES_PER_BATCH: u32 = 10;

/// Refresh the member's quota from authoritative proposal states and cache it (#502).
///
/// NOTE on storage write side effect: Although named `get_open_proposals`, this
/// function intentionally scans authoritative proposals and unconditionally writes
/// the recalculated open-proposal count into `DataKey::OpenProposals(member)` persistent
/// storage. This ensures the quota cache is refreshed whenever proposal state
/// transitions occur, allowing any subsequent quota check to rely on an up-to-date
/// cached value without stale reads.
/// Expired pending records do not consume quota even when no keeper has finalized them.
pub fn get_open_proposals(env: &Env, member: &Address) -> u32 {
    let now = env.ledger().timestamp();
    let mut count: u32 = 0;
    for id in 0..get_proposal_count(env, DataKey::NextProposalId) {
        if let Some(p) = get_loan_proposal(env, id) {
            if &p.borrower == member
                && p.status == crate::types::ProposalStatus::Pending
                && now <= p.editing_period_end.saturating_add(p.voting_period)
            {
                count = count.saturating_add(1);
            }
        }
    }
    for id in 0..get_proposal_count(env, DataKey::NextTreasuryId) {
        if let Some(p) = get_treasury_proposal(env, id) {
            if &p.proposer == member
                && p.status == crate::types::ProposalStatus::Pending
                && now
                    <= p.created_at
                        .saturating_add(p.voting_period.saturating_mul(if p.private {
                            2
                        } else {
                            1
                        }))
            {
                count = count.saturating_add(1);
            }
        }
    }
    let key = DataKey::OpenProposals(member.clone());
    env.storage().persistent().set(&key, &count);
    extend_persistent(env, &key);
    count
}

pub fn get_loan_proposal(env: &Env, id: u32) -> Option<LoanProposal> {
    let key = DataKey::LoanProposal(id);
    let p = env.storage().persistent().get(&key);
    if p.is_some() {
        extend_persistent(env, &key);
    }
    p
}

pub fn set_loan_proposal(env: &Env, p: &LoanProposal) {
    let key = DataKey::LoanProposal(p.id);
    env.storage().persistent().set(&key, p);
    extend_persistent(env, &key);
    get_open_proposals(env, &p.borrower);
}

pub fn get_loan(env: &Env, id: u32) -> Option<Loan> {
    let key = DataKey::Loan(id);
    let l = env.storage().persistent().get(&key);
    if l.is_some() {
        extend_persistent(env, &key);
    }
    l
}

pub fn set_loan(env: &Env, l: &Loan) {
    let key = DataKey::Loan(l.id);
    env.storage().persistent().set(&key, l);
    extend_persistent(env, &key);
}

pub fn get_treasury_proposal(env: &Env, id: u32) -> Option<TreasuryProposal> {
    let key = DataKey::TreasuryProposal(id);
    let p = env.storage().persistent().get(&key);
    if p.is_some() {
        extend_persistent(env, &key);
    }
    p
}

pub fn set_treasury_proposal(env: &Env, p: &TreasuryProposal) {
    let key = DataKey::TreasuryProposal(p.id);
    env.storage().persistent().set(&key, p);
    extend_persistent(env, &key);
    get_open_proposals(env, &p.proposer);
}

pub fn has_loan_voted(env: &Env, id: u32, voter: &Address) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::LoanVoted(id, voter.clone()))
}

pub fn set_loan_voted(env: &Env, id: u32, voter: &Address) {
    let key = DataKey::LoanVoted(id, voter.clone());
    env.storage().persistent().set(&key, &true);
    extend_persistent(env, &key);
}

pub fn has_treasury_voted(env: &Env, id: u32, voter: &Address) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::TreasuryVoted(id, voter.clone()))
}

pub fn set_treasury_voted(env: &Env, id: u32, voter: &Address) {
    let key = DataKey::TreasuryVoted(id, voter.clone());
    env.storage().persistent().set(&key, &true);
    extend_persistent(env, &key);
}

pub fn get_stake(env: &Env, addr: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::Stake(addr.clone()))
        .unwrap_or(0)
}

pub fn set_stake(env: &Env, addr: &Address, amount: i128) {
    let key = DataKey::Stake(addr.clone());
    env.storage().persistent().set(&key, &amount);
    extend_persistent(env, &key);
}

// ---------- pull-based yield accrual ----------

pub fn get_yield_accumulator(env: &Env, asset: &Address) -> i128 {
    let key = YieldKey::YieldAccumulator(asset.clone());
    if let Some(value) = env.storage().instance().get(&key) {
        extend_instance(env);
        value
    } else if get_token(env).is_ok_and(|token| &token == asset) {
        let legacy_value = env
            .storage()
            .instance()
            .get(&DataKey::YieldAccumulator)
            .unwrap_or(0);
        extend_instance(env);
        legacy_value
    } else {
        0
    }
}

pub fn set_yield_accumulator(env: &Env, asset: &Address, value: i128) {
    env.storage()
        .instance()
        .set(&YieldKey::YieldAccumulator(asset.clone()), &value);
    extend_instance(env);
}

pub fn get_yield_snapshot(env: &Env, addr: &Address, asset: &Address) -> i128 {
    let key = YieldKey::MemberYieldSnapshot(addr.clone(), asset.clone());
    if let Some(value) = env.storage().persistent().get(&key) {
        extend_persistent(env, &key);
        value
    } else if get_token(env).is_ok_and(|token| &token == asset) {
        let legacy_key = DataKey::MemberYieldSnapshot(addr.clone());
        match env.storage().persistent().get(&legacy_key) {
            Some(value) => {
                extend_persistent(env, &legacy_key);
                value
            }
            None => 0,
        }
    } else {
        0
    }
}

pub fn set_yield_snapshot(env: &Env, addr: &Address, asset: &Address, value: i128) {
    let key = YieldKey::MemberYieldSnapshot(addr.clone(), asset.clone());
    env.storage().persistent().set(&key, &value);
    extend_persistent(env, &key);
}

// ---------- staking, registry, and document module accessors ----------

pub fn get_doc(env: &Env, kind: ProposalKind, id: u32) -> Option<soroban_sdk::Bytes> {
    let key = DataKey::Doc(kind, id);
    let v = env.storage().persistent().get(&key);
    if v.is_some() {
        extend_persistent(env, &key);
    }
    v
}

pub fn set_doc(env: &Env, kind: ProposalKind, id: u32, hash: &soroban_sdk::Bytes) {
    let key = DataKey::Doc(kind, id);
    env.storage().persistent().set(&key, hash);
    extend_persistent(env, &key);
}

pub fn get_name_owner(env: &Env, name: &String) -> Option<Address> {
    let key = DataKey::Name(name.clone());
    let v = env.storage().persistent().get(&key);
    if v.is_some() {
        extend_persistent(env, &key);
    }
    v
}

pub fn set_name(env: &Env, name: &String, owner: &Address) {
    let nkey = DataKey::Name(name.clone());
    let okey = DataKey::NameOf(owner.clone());
    env.storage().persistent().set(&nkey, owner);
    env.storage().persistent().set(&okey, name);
    extend_persistent(env, &nkey);
    extend_persistent(env, &okey);
}

pub fn get_name_of(env: &Env, owner: &Address) -> Option<String> {
    let key = DataKey::NameOf(owner.clone());
    let v = env.storage().persistent().get(&key);
    if v.is_some() {
        extend_persistent(env, &key);
    }
    v
}

pub fn get_commit(env: &Env, id: u32, voter: &Address) -> Option<BytesN<32>> {
    let key = DataKey::Commit(id, voter.clone());
    let v = env.storage().persistent().get(&key);
    if v.is_some() {
        extend_persistent(env, &key);
    }
    v
}

pub fn set_commit(env: &Env, id: u32, voter: &Address, commitment: &BytesN<32>) {
    let key = DataKey::Commit(id, voter.clone());
    env.storage().persistent().set(&key, commitment);
    extend_persistent(env, &key);
}

pub fn remove_commit(env: &Env, id: u32, voter: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::Commit(id, voter.clone()));
}

pub fn get_yield_remainder(env: &Env, asset: &Address) -> i128 {
    let key = YieldKey::YieldRemainder(asset.clone());
    if let Some(value) = env.storage().persistent().get(&key) {
        extend_persistent(env, &key);
        value
    } else if get_token(env).is_ok_and(|token| &token == asset) {
        let legacy_key = DataKey::YieldRemainder;
        match env.storage().persistent().get(&legacy_key) {
            Some(value) => {
                extend_persistent(env, &legacy_key);
                value
            }
            None => 0,
        }
    } else {
        0
    }
}

pub fn set_yield_remainder(env: &Env, asset: &Address, value: i128) {
    let key = YieldKey::YieldRemainder(asset.clone());
    env.storage().persistent().set(&key, &value);
    extend_persistent(env, &key);
}

pub fn record_yield_asset(env: &Env, asset: &Address) {
    let key = DataKey::YieldAssets;
    let mut assets = env
        .storage()
        .persistent()
        .get::<_, Vec<Address>>(&key)
        .unwrap_or_else(|| Vec::new(env));
    if !assets.contains(asset) {
        assets.push_back(asset.clone());
        env.storage().persistent().set(&key, &assets);
    }
    extend_persistent(env, &key);
}

pub fn get_yield_assets(env: &Env) -> Vec<Address> {
    let mut assets = get_whitelisted_tokens(env);
    let key = DataKey::YieldAssets;
    if let Some(recorded) = env.storage().persistent().get::<_, Vec<Address>>(&key) {
        extend_persistent(env, &key);
        for asset in recorded.iter() {
            if !assets.contains(&asset) {
                assets.push_back(asset);
            }
        }
    }
    assets
}

pub fn get_delegation(env: &Env, member: &Address) -> Option<Address> {
    let key = DataKey::Delegation(member.clone());
    let res = env.storage().persistent().get(&key);
    if res.is_some() {
        extend_persistent(env, &key);
    }
    res
}

pub fn set_delegation(env: &Env, member: &Address, delegatee: &Address) {
    let key = DataKey::Delegation(member.clone());
    env.storage().persistent().set(&key, delegatee);
    extend_persistent(env, &key);
}

// ---------- multi-token treasury (#236) ----------

/// The base token is always whitelisted; any other asset only while its
/// `TokenWhitelisted` entry exists.
pub fn is_token_whitelisted(env: &Env, token: &Address) -> bool {
    let base: Option<Address> = env.storage().instance().get(&DataKey::Token);
    if base.as_ref() == Some(token) {
        return true;
    }
    let key = DataKey::TokenWhitelisted(token.clone());
    let listed = env.storage().persistent().has(&key);
    if listed {
        extend_persistent(env, &key);
    }
    listed
}

/// Adds (`true`) or removes (`false`) an extra asset, keeping the
/// `WhitelistedTokens` list in step. Idempotent in both directions.
pub fn set_token_whitelisted(env: &Env, token: &Address, whitelisted: bool) {
    let key = DataKey::TokenWhitelisted(token.clone());
    let mut tokens: Vec<Address> = env
        .storage()
        .persistent()
        .get(&DataKey::WhitelistedTokens)
        .unwrap_or_else(|| Vec::new(env));
    if whitelisted {
        env.storage().persistent().set(&key, &true);
        extend_persistent(env, &key);
        if !tokens.contains(token) {
            tokens.push_back(token.clone());
        }
    } else {
        env.storage().persistent().remove(&key);
        if let Some(i) = tokens.first_index_of(token) {
            tokens.remove(i);
        }
    }
    env.storage()
        .persistent()
        .set(&DataKey::WhitelistedTokens, &tokens);
    extend_persistent(env, &DataKey::WhitelistedTokens);
}

/// Every accepted asset: the base token first, then extras in insertion order.
pub fn get_whitelisted_tokens(env: &Env) -> Vec<Address> {
    let mut all = Vec::new(env);
    if let Some(base) = env.storage().instance().get::<_, Address>(&DataKey::Token) {
        all.push_back(base);
    }
    let key = DataKey::WhitelistedTokens;
    if let Some(extra) = env.storage().persistent().get::<_, Vec<Address>>(&key) {
        extend_persistent(env, &key);
        all.append(&extra);
    }
    all
}

pub fn get_pending_token_addition(env: &Env, token: &Address) -> Option<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::PendingTokenAddition(token.clone()))
}

fn pending_token_addresses(env: &Env) -> Vec<Address> {
    let key = DataKey::PendingTokenAdditions;
    match env.storage().persistent().get(&key) {
        Some(tokens) => {
            extend_persistent(env, &key);
            tokens
        }
        None => Vec::new(env),
    }
}

pub fn get_pending_token_additions(env: &Env) -> Vec<(Address, u64)> {
    let mut pending = Vec::new(env);
    for token in pending_token_addresses(env) {
        if let Some(execution_time) = get_pending_token_addition(env, &token) {
            pending.push_back((token, execution_time));
        }
    }
    pending
}

pub fn set_pending_token_addition(env: &Env, token: &Address, execution_time: u64) {
    let key = DataKey::PendingTokenAddition(token.clone());
    env.storage().persistent().set(&key, &execution_time);
    extend_persistent(env, &key);

    let mut tokens = pending_token_addresses(env);
    if !tokens.contains(token) {
        tokens.push_back(token.clone());
    }
    env.storage()
        .persistent()
        .set(&DataKey::PendingTokenAdditions, &tokens);
    extend_persistent(env, &DataKey::PendingTokenAdditions);
}

pub fn remove_pending_token_addition(env: &Env, token: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::PendingTokenAddition(token.clone()));

    let mut tokens = pending_token_addresses(env);
    if let Some(index) = tokens.first_index_of(token) {
        tokens.remove(index);
        let key = DataKey::PendingTokenAdditions;
        if tokens.is_empty() {
            env.storage().persistent().remove(&key);
        } else {
            env.storage().persistent().set(&key, &tokens);
            extend_persistent(env, &key);
        }
    }
}

pub fn next_token_proposal_id(env: &Env) -> u32 {
    let current: u32 = env
        .storage()
        .instance()
        .get(&DataKey::NextTokenProposalId)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::NextTokenProposalId, &current.saturating_add(1));
    current
}

pub fn set_token_proposal_id(env: &Env, token: &Address, id: u32) {
    let key = DataKey::TokenProposalId(token.clone());
    env.storage().persistent().set(&key, &id);
    extend_persistent(env, &key);
}

pub fn get_token_proposal_id(env: &Env, token: &Address) -> Option<u32> {
    let key = DataKey::TokenProposalId(token.clone());
    let id = env.storage().persistent().get(&key);
    if id.is_some() {
        extend_persistent(env, &key);
    }
    id
}

/// Asset of loan proposal / loan `id`; the base token unless recorded.
pub fn get_loan_asset(env: &Env, id: u32) -> Result<Address, Error> {
    let key = DataKey::LoanAsset(id);
    match env.storage().persistent().get(&key) {
        Some(asset) => {
            extend_persistent(env, &key);
            Ok(asset)
        }
        None => get_token(env),
    }
}

pub fn set_loan_asset(env: &Env, id: u32, asset: &Address) {
    let key = DataKey::LoanAsset(id);
    env.storage().persistent().set(&key, asset);
    extend_persistent(env, &key);
}

/// Asset of treasury proposal `id`; the base token unless recorded.
pub fn get_treasury_asset(env: &Env, id: u32) -> Result<Address, Error> {
    let key = DataKey::TreasuryAsset(id);
    match env.storage().persistent().get(&key) {
        Some(asset) => {
            extend_persistent(env, &key);
            Ok(asset)
        }
        None => get_token(env),
    }
}

pub fn set_treasury_asset(env: &Env, id: u32, asset: &Address) {
    let key = DataKey::TreasuryAsset(id);
    env.storage().persistent().set(&key, asset);
    extend_persistent(env, &key);
}

// ---------- treasury reconciliation (#288) ----------

/// Principal still owed to the treasury on `Active` loans in `asset`.
pub fn get_outstanding_principal(env: &Env, asset: &Address) -> i128 {
    let key = DataKey::OutstandingPrincipal(asset.clone());
    match env.storage().persistent().get(&key) {
        Some(amount) => {
            extend_persistent(env, &key);
            amount
        }
        None => 0,
    }
}

/// Moves `asset`'s outstanding principal by `delta`; a zero delta writes nothing.
pub fn adjust_outstanding_principal(env: &Env, asset: &Address, delta: i128) {
    if delta == 0 {
        return;
    }
    let key = DataKey::OutstandingPrincipal(asset.clone());
    let current: i128 = env.storage().persistent().get(&key).unwrap_or(0);
    env.storage().persistent().set(&key, &(current + delta));
    extend_persistent(env, &key);
}

/// Number of active loans denominated in `asset`.
pub fn get_active_loans_in_asset(env: &Env, asset: &Address) -> u32 {
    let key = DataKey::ActiveLoansInAsset(asset.clone());
    match env.storage().persistent().get(&key) {
        Some(count) => {
            extend_persistent(env, &key);
            count
        }
        None => 0,
    }
}

pub fn increment_active_loans_in_asset(env: &Env, asset: &Address) {
    let key = DataKey::ActiveLoansInAsset(asset.clone());
    let current = get_active_loans_in_asset(env, asset);
    env.storage()
        .persistent()
        .set(&key, &current.saturating_add(1));
    extend_persistent(env, &key);
}

pub fn decrement_active_loans_in_asset(env: &Env, asset: &Address) {
    let key = DataKey::ActiveLoansInAsset(asset.clone());
    let current = get_active_loans_in_asset(env, asset);
    env.storage()
        .persistent()
        .set(&key, &current.saturating_sub(1));
    extend_persistent(env, &key);
}

// ---------- migration seeding lock (#332 / #333) ----------

pub fn is_seeding_locked(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::MigrationSeedingLocked)
        .unwrap_or(false)
}

pub fn lock_seeding(env: &Env) {
    env.storage()
        .instance()
        .set(&DataKey::MigrationSeedingLocked, &true);
}

pub fn set_next_proposal_id_at_least(env: &Env, min: u32) {
    let current: u32 = env
        .storage()
        .instance()
        .get(&DataKey::NextProposalId)
        .unwrap_or(0);
    if min > current {
        env.storage().instance().set(&DataKey::NextProposalId, &min);
    }
}

pub fn set_next_treasury_id_at_least(env: &Env, min: u32) {
    let current: u32 = env
        .storage()
        .instance()
        .get(&DataKey::NextTreasuryId)
        .unwrap_or(0);
    if min > current {
        env.storage().instance().set(&DataKey::NextTreasuryId, &min);
    }
}
