#![no_std]
// A panic on-chain gives the caller no error code, so contract code returns
// an `Error` instead of unwrapping (#253). Tests may still unwrap.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
//! OurDAO — a member-owned lending DAO for Stellar Soroban.
//!
//! All value moves through a single configurable token set at initialization
//! (USDC, XLM via the Stellar Asset Contract, or any Stellar asset). Four
//! additional Soroban-native modules extend the core lending/treasury flow:
//!
//! * [`registry`] name registry
//! * [`docs`] content-hash proposal metadata
//! * [`privacy`] commit-reveal voting
//! * [`staking`] voting-weight staking

mod admin;
mod docs;
mod error;
mod events;
mod loans;
mod membership;
mod migration;
mod privacy;
mod registry;
mod staking;
mod storage;
mod treasury;
mod types;
mod util;

#[cfg(test)]
mod tests;

#[cfg(all(test, feature = "bench-budget"))]
mod bench;

use soroban_sdk::{contract, contractimpl, Address, Bytes, BytesN, Env, Map, String, Vec};
use types::{
    AssetBalance, DaoSummary, SeedDocEntry, SeedNameEntry, SeedStakeEntry, SeedYieldEntry,
    TreasuryAccounting,
};

pub use error::Error;
pub use storage::ProposalKind;
pub use types::{
    Loan, LoanPolicy, LoanProposal, LoanTerms, Member, MemberLoanStats, MemberOverview,
    PendingPolicyUpdate, StakingRewardClaimed, TreasuryProposal,
};

#[contract]
pub struct OurDao;

#[contractimpl]
impl OurDao {
    // ==================== lifecycle / governance ====================

    /// One-time setup. `admins` bootstrap governance, `consensus_threshold` is
    /// in basis points (e.g. 5100 = 51%), and `token` is the asset all DAO
    /// value flows through.
    pub fn initialize(
        env: Env,
        admins: Vec<Address>,
        consensus_threshold: u32,
        membership_fee: i128,
        token: Address,
        policy: LoanPolicy,
    ) -> Result<(), Error> {
        admin::initialize(
            &env,
            admins,
            consensus_threshold,
            membership_fee,
            token,
            policy,
        )
    }

    /// Adds an admin. The caller must be an existing admin and authorize the
    /// call. May emit an admin-change event and returns an authorization or
    /// configuration error when the change is not allowed.
    pub fn add_admin(env: Env, caller: Address, admin: Address) -> Result<(), Error> {
        admin::add_admin(&env, caller, admin)
    }

    /// Removes an admin subject to the remaining-admin and signer rules.
    /// The caller must be an admin and authorize the call.
    pub fn remove_admin(env: Env, caller: Address, admin: Address) -> Result<(), Error> {
        admin::remove_admin(&env, caller, admin)
    }

    /// Assigns the pauser role. The caller must authorize and be an admin.
    /// May return an authorization or role-state error.
    pub fn set_pauser(env: Env, caller: Address, pauser: Address) -> Result<(), Error> {
        admin::set_pauser(&env, caller, pauser)
    }

    /// Revokes the current pauser role. The caller must be an admin and
    /// authorize the call.
    pub fn revoke_pauser(env: Env, caller: Address) -> Result<(), Error> {
        admin::revoke_pauser(&env, caller)
    }

    /// Updates the governance consensus threshold. The caller must be an admin
    /// and authorize the call; invalid thresholds return `InvalidThreshold`.
    pub fn set_consensus_threshold(env: Env, caller: Address, threshold: u32) -> Result<(), Error> {
        admin::set_consensus_threshold(&env, caller, threshold)
    }

    /// Submits a timelocked loan-policy update. Requires admin authorization
    /// and emits a policy proposal event.
    pub fn propose_policy_update(
        env: Env,
        caller: Address,
        policy: LoanPolicy,
    ) -> Result<(), Error> {
        admin::propose_policy_update(&env, caller, policy)
    }

    /// Executes a mature pending policy update. Requires admin authorization
    /// and returns timelock or policy errors when not eligible.
    pub fn execute_policy_update(env: Env, caller: Address) -> Result<(), Error> {
        admin::execute_policy_update(&env, caller)
    }

    /// Cancels a pending policy update. Requires admin authorization and emits
    /// a policy cancellation event.
    pub fn cancel_policy_update(env: Env, caller: Address) -> Result<(), Error> {
        admin::cancel_policy_update(&env, caller)
    }

    /// Directly sets the loan policy. Requires admin authorization and returns
    /// `InvalidLoanPolicy` for invalid values.
    pub fn set_loan_policy(env: Env, caller: Address, policy: LoanPolicy) -> Result<(), Error> {
        admin::set_policy(&env, caller, policy)
    }

    /// Pauses contract state changes. The pauser or an allowed caller must
    /// authorize the call and emit the pause event on success.
    pub fn pause(env: Env, caller: Address) -> Result<(), Error> {
        admin::pause(&env, caller)
    }

    /// Resumes contract state changes. The pauser or an allowed caller must
    /// authorize the call.
    pub fn unpause(env: Env, caller: Address) -> Result<(), Error> {
        admin::unpause(&env, caller)
    }

    // ==================== membership ====================

    /// Registers `member`, collects the membership fee, and grants active
    /// member status. `member` must authorize; returns `AlreadyMember`,
    /// `InsufficientTreasury`, or transfer/configuration errors when applicable
    /// and emits the member-joined event.
    pub fn register_member(env: Env, member: Address) -> Result<(), Error> {
        membership::register_member(&env, member)
    }

    /// Exits `member`, settling stake and accrued yield and reducing their
    /// contribution share. `member` must authorize and have no active loan.
    /// Returns membership, balance, or arithmetic errors and emits exit events.
    pub fn exit_dao(env: Env, member: Address) -> Result<(), Error> {
        membership::exit_dao(&env, member)
    }

    /// Withdraw accrued loan-interest yield for the caller.
    pub fn claim_rewards(env: Env, member: Address) -> Result<i128, Error> {
        membership::claim_rewards(&env, member)
    }

    /// Withdraws accrued yield in the specified asset without affecting other
    /// asset snapshots.
    pub fn claim_rewards_in_asset(
        env: Env,
        member: Address,
        asset: Address,
    ) -> Result<i128, Error> {
        membership::claim_rewards_in_asset(&env, member, asset)
    }

    /// Delegates `delegator`'s voting power to `delegatee`. `delegator` must
    /// authorize; invalid targets and authorization failures return errors.
    pub fn delegate_vote(env: Env, delegator: Address, delegatee: Address) -> Result<(), Error> {
        membership::delegate_vote(&env, delegator, delegatee)
    }

    // ==================== loans ====================

    /// Creates a loan proposal in the base treasury asset. `borrower` must
    /// authorize and satisfy member, loan-policy, and treasury eligibility;
    /// emits a loan proposal event on success.
    pub fn request_loan(
        env: Env,
        borrower: Address,
        amount: i128,
        metadata_cid: Option<String>,
    ) -> Result<u32, Error> {
        loans::request_loan(&env, borrower, amount, metadata_cid)
    }

    /// Requests a loan denominated in a specific whitelisted asset.
    pub fn request_loan_in_asset(
        env: Env,
        borrower: Address,
        amount: i128,
        asset: Address,
        metadata_cid: Option<String>,
    ) -> Result<u32, Error> {
        loans::request_loan_in_asset(&env, borrower, amount, asset, metadata_cid)
    }

    /// Changes the amount or metadata of a borrower's loan proposal while it
    /// remains in its editing window. `borrower` must authorize.
    pub fn edit_loan_proposal(
        env: Env,
        borrower: Address,
        proposal_id: u32,
        new_amount: i128,
        metadata_cid: Option<String>,
    ) -> Result<(), Error> {
        loans::edit_loan_proposal(&env, borrower, proposal_id, new_amount, metadata_cid)
    }

    /// Withdraws a loan proposal while it is still in its editing period.
    /// See `loans::cancel_loan_proposal`.
    pub fn cancel_loan_proposal(
        env: Env,
        borrower: Address,
        proposal_id: u32,
    ) -> Result<(), Error> {
        loans::cancel_loan_proposal(&env, borrower, proposal_id)
    }

    /// Casts `voter`'s support or rejection vote on a loan proposal.
    /// `voter` must authorize and be eligible; duplicate, ended, or invalid
    /// votes return errors and emit the loan-vote event on success.
    pub fn vote_on_loan_proposal(
        env: Env,
        voter: Address,
        proposal_id: u32,
        support: bool,
    ) -> Result<(), Error> {
        loans::vote_on_loan_proposal(&env, voter, proposal_id, support)
    }

    /// Records multiple loan proposal votes atomically for `voter`. `voter`
    /// must authorize; any invalid vote aborts the batch and emits no partial
    /// vote events.
    pub fn vote_batch(
        env: Env,
        voter: Address,
        votes: Vec<crate::types::ProposalVote>,
    ) -> Result<(), Error> {
        loans::vote_batch(&env, voter, votes)
    }

    /// Pays out an approved loan that was waiting on treasury funds.
    /// Permissionless keeper call — see `loans::disburse_approved_loan`.
    pub fn disburse_approved_loan(env: Env, proposal_id: u32) -> Result<(), Error> {
        loans::disburse_approved_loan(&env, proposal_id)
    }

    /// Repays the outstanding balance of a base-asset loan. `borrower` must
    /// authorize; invalid, inactive, or insufficient-payment conditions return
    /// errors and emit a loan-repayment event on success.
    pub fn repay_loan(env: Env, borrower: Address, loan_id: u32) -> Result<(), Error> {
        loans::repay_loan(&env, borrower, loan_id)
    }

    /// Repays a loan using the asset the loan was denominated in.
    pub fn repay_loan_in_asset(
        env: Env,
        borrower: Address,
        loan_id: u32,
        asset: Address,
    ) -> Result<(), Error> {
        loans::repay_loan_in_asset(&env, borrower, loan_id, asset)
    }

    /// Repays up to `amount` of a loan's outstanding balance. See
    /// `loans::repay_loan_partial` for the interest/principal split and why
    /// this is a separate entrypoint from `repay_loan`.
    pub fn repay_loan_partial(
        env: Env,
        borrower: Address,
        loan_id: u32,
        amount: i128,
    ) -> Result<(), Error> {
        loans::repay_loan_partial(&env, borrower, loan_id, amount)
    }

    /// Marks an overdue loan as defaulted. Permissionless — see `loans::mark_loan_defaulted`.
    pub fn mark_loan_defaulted(env: Env, loan_id: u32) -> Result<(), Error> {
        loans::mark_loan_defaulted(&env, loan_id)
    }

    /// Permissionless keeper call: persists the expired/rejected transition for
    /// a loan proposal whose voting window has passed without reaching quorum.
    pub fn expire_loan_proposal(env: Env, proposal_id: u32) -> Result<(), Error> {
        loans::expire_loan_proposal(&env, proposal_id)
    }

    // ==================== treasury ====================

    /// Creates a treasury-withdrawal proposal in the base asset. `proposer`
    /// must authorize and emit the proposal event; invalid amount or state
    /// returns an error.
    pub fn propose_treasury_withdrawal(
        env: Env,
        proposer: Address,
        amount: i128,
        destination: Address,
        reason: String,
        private: bool,
    ) -> Result<u32, Error> {
        treasury::propose_withdrawal(&env, proposer, amount, destination, reason, private)
    }

    /// Proposes a treasury withdrawal denominated in a specific whitelisted asset.
    /// Named without the `treasury_` prefix because Soroban caps entrypoint
    /// names at 32 characters.
    pub fn propose_withdrawal_in_asset(
        env: Env,
        proposer: Address,
        amount: i128,
        asset: Address,
        destination: Address,
        reason: String,
        private: bool,
    ) -> Result<u32, Error> {
        treasury::propose_withdrawal_in_asset(
            &env,
            proposer,
            amount,
            asset,
            destination,
            reason,
            private,
        )
    }

    /// Casts `voter`'s support or rejection vote on a treasury proposal.
    /// `voter` must authorize and satisfy electorate rules; duplicate or ended
    /// votes return errors and emit a treasury-vote event.
    pub fn vote_on_treasury_proposal(
        env: Env,
        voter: Address,
        proposal_id: u32,
        support: bool,
    ) -> Result<(), Error> {
        treasury::vote(&env, voter, proposal_id, support)
    }

    /// Withdraws a pending treasury proposal within the policy's editing
    /// period. See `treasury::cancel_treasury_proposal`.
    pub fn cancel_treasury_proposal(
        env: Env,
        proposer: Address,
        proposal_id: u32,
    ) -> Result<(), Error> {
        treasury::cancel_treasury_proposal(&env, proposer, proposal_id)
    }

    /// Permissionlessly expires a treasury proposal after its voting window.
    /// Returns `NotInVotingPhase` for an active proposal and emits an expiry
    /// event on transition.
    pub fn expire_treasury_proposal(env: Env, proposal_id: u32) -> Result<(), Error> {
        treasury::expire_treasury_proposal(&env, proposal_id)
    }

    /// Executes an approved treasury withdrawal that was waiting on treasury
    /// funds. Permissionless keeper call — see `treasury::execute_approved`.
    pub fn execute_treasury_proposal(env: Env, proposal_id: u32) -> Result<(), Error> {
        treasury::execute_approved(&env, proposal_id)
    }

    // ==================== maintenance ====================

    #[allow(deprecated)]
    /// Extends the contract instance TTL. Permissionless; emits `TTL_BUMPED`.
    pub fn bump_dao_ttl(env: Env) {
        storage::extend_instance(&env);
        env.events().publish((events::TTL_BUMPED,), ());
    }

    // ==================== staking ====================

    /// Stakes base treasury assets to increase `member`'s voting weight.
    /// `member` must authorize and have sufficient spendable balance; returns
    /// stake/amount errors and emits the stake event.
    pub fn stake(env: Env, member: Address, amount: i128) -> Result<(), Error> {
        staking::stake(&env, member, amount)
    }

    /// Releases staked base treasury assets from `member`. `member` must
    /// authorize and have enough stake; emits the unstake event.
    pub fn unstake(env: Env, member: Address, amount: i128) -> Result<(), Error> {
        staking::unstake(&env, member, amount)
    }

    // ==================== name registry ====================

    /// Registers or updates `name` for `owner`. `owner` must authorize and the
    /// name must be valid and available; emits a name-registration event.
    pub fn register_name(env: Env, owner: Address, name: String) -> Result<(), Error> {
        registry::register_name(&env, owner, name)
    }

    /// Releases `owner`'s registered name so they go nameless without
    /// claiming a replacement or exiting (#458). `owner` must authorize;
    /// returns `NameNotFound` if they hold no name and emits `name_rel`.
    pub fn release_name(env: Env, owner: Address) -> Result<(), Error> {
        registry::release_name(&env, owner)
    }

    /// Resolves a registered name to its owner address. Read-only; returns
    /// `None` when unregistered and `NotInitialized` if the DAO is uninitialized.
    pub fn resolve_name(env: Env, name: String) -> Result<Option<Address>, Error> {
        registry::resolve_name(&env, name)
    }

    /// Returns the name registered to `owner`, or `None` when none is
    /// registered. Read-only; may return `NotInitialized`.
    pub fn name_of(env: Env, owner: Address) -> Result<Option<String>, Error> {
        registry::name_of(&env, owner)
    }

    // ==================== commit-reveal voting ====================

    /// Stores a hidden treasury-vote commitment. `voter` must authorize and
    /// commit during the configured window; emits a commitment event.
    pub fn commit_treasury_vote(
        env: Env,
        voter: Address,
        proposal_id: u32,
        commitment: BytesN<32>,
    ) -> Result<(), Error> {
        privacy::commit_vote(&env, voter, proposal_id, commitment)
    }

    /// Reveals a previously committed treasury vote. `voter` must authorize
    /// and provide the matching commitment inputs; emits the treasury-vote
    /// event after tallying.
    pub fn reveal_treasury_vote(
        env: Env,
        voter: Address,
        proposal_id: u32,
        support: bool,
        salt: BytesN<32>,
    ) -> Result<(), Error> {
        privacy::reveal_vote(&env, voter, proposal_id, support, salt)
    }

    /// Convenience pre-check: `true` only when the stored commitment matches
    /// the supplied reveal inputs.
    pub fn verify_commitment(
        env: Env,
        voter: Address,
        proposal_id: u32,
        support: bool,
        salt: BytesN<32>,
    ) -> bool {
        privacy::verify_commitment(&env, voter, proposal_id, support, &salt)
    }

    /// Commit and reveal boundary timestamps for a private treasury proposal.
    pub fn get_commit_reveal_window(env: Env, proposal_id: u32) -> Result<(u64, u64), Error> {
        privacy::get_commit_reveal_window(&env, proposal_id)
    }

    // ==================== content-hash documents ====================

    /// Attaches a content hash to a proposal. The caller must be the
    /// authorized proposer/borrower and the document payload must be valid;
    /// emits the document event.
    pub fn attach_document(
        env: Env,
        caller: Address,
        kind: ProposalKind,
        proposal_id: u32,
        content_hash: Bytes,
    ) -> Result<(), Error> {
        docs::attach_document(&env, caller, kind, proposal_id, content_hash)
    }

    /// Returns the content hash attached to a proposal, or `None` if absent.
    /// Read-only.
    pub fn get_document(
        env: Env,
        kind: ProposalKind,
        proposal_id: u32,
    ) -> Result<Option<Bytes>, Error> {
        docs::get_document(&env, kind, proposal_id)
    }

    // ==================== views ====================
    //
    // Rule: a view that reads DAO configuration or the registry returns
    // `Err(Error::NotInitialized)` on an uninitialized contract. Per-key
    // lookups (`get_member`, `get_loan`, ...) keep returning `None`; use
    // `is_initialized` to tell a missing record from a wrong address.

    /// Returns the member record for `address`, or `None` if unregistered.
    pub fn get_member(env: Env, address: Address) -> Option<Member> {
        storage::get_member(&env, &address)
    }

    /// Returns `member`'s lifetime loan track record: total loans taken, loans
    /// fully repaid, and loans currently outstanding. Single O(1) storage read;
    /// returns `NotMember` if the address has never registered.
    pub fn get_member_loan_stats(env: Env, member: Address) -> Result<MemberLoanStats, Error> {
        storage::get_member(&env, &member)
            .map(|m| MemberLoanStats {
                total_loans: m.total_loans,
                repaid_loans: m.repaid_loans,
                active_loans: m.active_loans,
            })
            .ok_or(Error::NotMember)
    }

    /// Returns everything about `member` in one call: their record, stake,
    /// unclaimed yield, loan track record, registered name and delegate.
    /// Returns `NotMember` if the address has never registered.
    pub fn get_member_overview(env: Env, member: Address) -> Result<MemberOverview, Error> {
        membership::member_overview(&env, &member)
    }

    /// Returns a loan by id, or `None` if it does not exist.
    pub fn get_loan(env: Env, loan_id: u32) -> Option<Loan> {
        storage::get_loan(&env, loan_id)
    }

    /// Returns a loan proposal with any expired phase refreshed from ledger
    /// time, or `None` if absent.
    pub fn get_loan_proposal(env: Env, proposal_id: u32) -> Option<LoanProposal> {
        storage::get_loan_proposal(&env, proposal_id).map(|p| loans::refresh_phase(&env, p))
    }

    /// Returns a treasury proposal by id, or `None` if absent.
    pub fn get_treasury_proposal(env: Env, proposal_id: u32) -> Option<TreasuryProposal> {
        storage::get_treasury_proposal(&env, proposal_id)
    }

    /// Returns the next loan-proposal sequence value as the proposal count.
    pub fn get_loan_proposal_count(env: Env) -> u32 {
        storage::get_proposal_count(&env, storage::DataKey::NextProposalId)
    }

    /// Returns the next treasury-proposal sequence value as the proposal count.
    pub fn get_treasury_proposal_count(env: Env) -> u32 {
        storage::get_proposal_count(&env, storage::DataKey::NextTreasuryId)
    }

    /// Returns the active loan policy. Returns `NotInitialized` before setup.
    pub fn get_loan_policy(env: Env) -> Result<LoanPolicy, Error> {
        util::require_initialized(&env)?;
        storage::get_policy(&env)
    }

    /// Returns the pending timelocked policy update, or `None` if none exists.
    pub fn get_pending_policy_update(env: Env) -> Option<PendingPolicyUpdate> {
        admin::get_pending_policy_update(&env)
    }

    /// Returns the current admin set. Returns `NotInitialized` before setup.
    pub fn get_admins(env: Env) -> Result<Vec<Address>, Error> {
        util::require_initialized(&env)?;
        storage::get_admins(&env)
    }

    /// Reports whether `address` is an admin. Returns `NotInitialized` before setup.
    pub fn is_admin(env: Env, address: Address) -> Result<bool, Error> {
        util::require_initialized(&env)?;
        Ok(util::is_admin(&env, &address))
    }

    /// Reports whether `address` is an active registered member.
    pub fn is_member(env: Env, address: Address) -> bool {
        matches!(
            storage::get_member(&env, &address),
            Some(m) if m.status == types::MemberStatus::ActiveMember
        )
    }

    /// Reports the first failing eligibility check. Checking again after
    /// resolving that error can reveal another eligibility failure.
    pub fn is_eligible_for_loan(env: Env, member: Address) -> Result<(), Error> {
        loans::is_eligible_for_loan(&env, &member)
    }

    /// Returns the spendable base treasury balance. Returns
    /// `NotInitialized` before setup.
    pub fn get_treasury_balance(env: Env) -> Result<i128, Error> {
        util::require_initialized(&env)?;
        Ok(util::treasury_balance(&env))
    }

    /// Returns the treasury balance held in a specific whitelisted asset.
    pub fn get_treasury_balance_in_asset(env: Env, asset: Address) -> i128 {
        util::treasury_balance_in_asset(&env, &asset)
    }

    /// Reconciliation view for `asset` (#288): the contract's actual token
    /// balance, how much of it is member stake, and the principal still owed
    /// on active loans in that asset. `token_balance - staked` equals
    /// `get_treasury_balance_in_asset`; comparing these figures against the
    /// indexed event history exposes drift or direct donations. Also answers
    /// for a removed asset, whose loans may still be outstanding.
    pub fn get_treasury_accounting(env: Env, asset: Address) -> Result<TreasuryAccounting, Error> {
        util::require_initialized(&env)?;
        Ok(TreasuryAccounting {
            token_balance: util::treasury_balance_for(&env, &asset),
            staked: util::staked_in_asset(&env, &asset),
            outstanding_principal: storage::get_outstanding_principal(&env, &asset),
        })
    }

    /// Returns every accepted treasury asset and its current contract balance.
    ///
    /// The base token is always included first by `get_whitelisted_tokens`,
    /// followed by every currently whitelisted extra asset.
    pub fn get_treasury_balances(env: Env) -> Map<Address, i128> {
        let mut balances = Map::new(&env);
        for asset in storage::get_whitelisted_tokens(&env).iter() {
            balances.set(
                asset.clone(),
                util::treasury_balance_in_asset(&env, &asset),
            );
        }
        balances
    }

    pub fn get_total_members(env: Env) -> u32 {
        storage::get_total_members(&env)
    }

    /// Returns the current number of active members.
    pub fn get_active_members(env: Env) -> u32 {
        storage::get_active_members(&env)
    }

    /// Returns the governance consensus threshold in basis points.
    /// Returns `NotInitialized` before setup.
    pub fn get_consensus_threshold(env: Env) -> Result<u32, Error> {
        util::require_initialized(&env)?;
        storage::get_threshold(&env)
    }

    /// Returns the base treasury asset. Returns `NotInitialized` before setup.
    pub fn get_token(env: Env) -> Result<Address, Error> {
        util::require_initialized(&env)?;
        storage::get_token(&env)
    }

    /// Whether `initialize` has run. Clients should check this before relying
    /// on any other view: every state-reading view returns
    /// `Error::NotInitialized` on an uninitialized contract rather than
    /// trapping or returning an ambiguous empty value.
    pub fn is_initialized(env: Env) -> bool {
        storage::is_initialized(&env)
    }

    /// Headline DAO state in one call (#339): membership counts, spendable
    /// treasury per whitelisted asset, total staked, threshold, pause flag and
    /// version. Each field matches the individual view it summarises.
    pub fn get_dao_summary(env: Env) -> DaoSummary {
        let mut treasury_balances = Vec::new(&env);
        for asset in storage::get_whitelisted_tokens(&env).iter() {
            let balance = util::treasury_balance_in_asset(&env, &asset);
            treasury_balances.push_back(AssetBalance { asset, balance });
        }
        DaoSummary {
            total_members: storage::get_total_members(&env),
            active_members: storage::get_active_members(&env),
            treasury_balance: util::treasury_balance(&env),
            treasury_balances,
            total_staked: storage::get_total_staked(&env),
            // 0 before initialisation, matching the other zeroed counters.
            consensus_threshold: storage::get_threshold(&env).unwrap_or(0),
            paused: storage::is_paused(&env),
            version: String::from_str(&env, env!("CARGO_PKG_VERSION")),
        }
    }

    // ==================== token whitelist ====================

    /// Proposes adding `token` to the treasury's supported-asset whitelist.
    /// Requires admin consensus; see `admin::propose_token_addition`.
    pub fn propose_token_addition(env: Env, caller: Address, token: Address) -> Result<(), Error> {
        admin::propose_token_addition(&env, caller, token)
    }

    /// Executes a previously proposed token addition once consensus is met.
    pub fn execute_token_addition(env: Env, caller: Address, token: Address) -> Result<(), Error> {
        admin::execute_token_addition(&env, caller, token)
    }

    /// Removes `token` from the supported-asset whitelist.
    pub fn remove_token(env: Env, caller: Address, token: Address) -> Result<(), Error> {
        admin::remove_token(&env, caller, token)
    }

    /// Returns whether `token` is currently an approved treasury asset.
    pub fn is_token_whitelisted(env: Env, token: Address) -> bool {
        storage::is_token_whitelisted(&env, &token)
    }

    /// Returns the full list of approved treasury assets.
    pub fn get_whitelisted_tokens(env: Env) -> Vec<Address> {
        storage::get_whitelisted_tokens(&env)
    }

    /// Returns the ledger time at which `token`'s pending addition can be
    /// executed, or `None` if no addition is pending for it.
    pub fn get_pending_token_addition(env: Env, token: Address) -> Option<u64> {
        storage::get_pending_token_addition(&env, &token)
    }

    /// Lists indexed pending asset additions with their earliest execution times.
    pub fn get_pending_token_additions(env: Env) -> Vec<(Address, u64)> {
        storage::get_pending_token_additions(&env)
    }

    /// Reports whether contract state changes are currently paused.
    pub fn is_paused(env: Env) -> bool {
        storage::is_paused(&env)
    }

    /// Returns this contract's semver, read from CARGO_PKG_VERSION at build
    /// time (#197) — lets off-chain tooling / indexers detect which
    /// contract build a given deployment is running without relying on the
    /// WASM hash alone.
    pub fn get_version(env: Env) -> String {
        String::from_str(&env, env!("CARGO_PKG_VERSION"))
    }

    /// Returns `member`'s current base-asset stake balance.
    pub fn get_stake(env: Env, member: Address) -> i128 {
        storage::get_stake(&env, &member)
    }

    /// Returns `member`'s current voting weight (base vote + staking bonus).
    /// Equivalent to the weight applied when they cast a loan or treasury vote.
    pub fn get_voting_weight(env: Env, member: Address) -> i128 {
        util::get_voting_weight(&env, &member)
    }

    /// Stake tokens required per additional unit of voting bonus.
    /// Every `stake_weight_unit` tokens staked grants +1 vote, up to the cap.
    pub fn get_stake_weight_unit(_env: Env) -> i128 {
        util::get_stake_weight_unit()
    }

    /// Maximum additional votes a member can earn through staking.
    pub fn get_max_stake_bonus(_env: Env) -> i128 {
        util::get_max_stake_bonus()
    }

    /// Returns `member`'s unclaimed base-asset yield. No authorization is
    /// required for this read; it may return `ArithmeticOverflow`.
    pub fn get_pending_yield(env: Env, member: Address) -> Result<i128, Error> {
        membership::compute_pending_yield(&env, &member)
    }

    /// Returns `member`'s unclaimed yield denominated in `asset`.
    pub fn get_pending_yield_in_asset(
        env: Env,
        member: Address,
        asset: Address,
    ) -> Result<i128, Error> {
        membership::compute_pending_yield_in_asset(&env, &member, &asset)
    }

    /// Returns the address `delegator` has delegated their vote to, or `None`
    /// if they have never called `delegate_vote`.
    pub fn get_delegate(env: Env, delegator: Address) -> Option<Address> {
        storage::get_delegation(&env, &delegator)
    }

    /// Returns whether `voter` has cast a (possibly not-yet-revealed) vote
    /// on the given proposal. For public (non-private) proposals this is a
    /// simple check against the vote record. For commit-reveal treasury
    /// proposals it checks whether the voter has committed (which is the
    /// earliest point at which a double-vote is prevented).
    pub fn has_voted(env: Env, kind: ProposalKind, proposal_id: u32, voter: Address) -> bool {
        match kind {
            ProposalKind::Loan => storage::has_loan_voted(&env, proposal_id, &voter),
            ProposalKind::Treasury => {
                // For private (commit-reveal) proposals, a commitment is
                // sufficient to report "voted" — the double-vote guard is
                // enforced at commit time, and the client needs to know the
                // member can't vote again regardless of reveal status.
                storage::has_treasury_voted(&env, proposal_id, &voter)
                    || storage::get_commit(&env, proposal_id, &voter).is_some()
            }
        }
    }

    /// Quotes the terms a loan of `amount` would carry right now. The quote
    /// can only be honoured when `amount <= max_loan`: with an empty or
    /// negative treasury `max_loan` is 0 and the rate shown is the policy
    /// maximum for a loan that cannot be issued. See
    /// `loans::calculate_loan_terms`.
    pub fn calculate_loan_terms(env: Env, amount: i128) -> Result<LoanTerms, Error> {
        util::require_initialized(&env)?;
        loans::calculate_loan_terms(&env, amount)
    }

    /// Calculates `member`'s contribution-proportional share of the base
    /// treasury. Returns zero for no active record and `ArithmeticOverflow`
    /// if the proportional calculation cannot be represented.
    pub fn calculate_exit_share(env: Env, member: Address) -> Result<i128, Error> {
        util::require_initialized(&env)?;
        membership::calculate_exit_share(&env, &member)
    }

    // ==================== paginated list views (#334 / #335) ====================

    /// Returns up to `limit` member records starting at offset `start` in the
    /// internal member-address list. Returns an empty vec when `start` is past
    /// the end.  Skips addresses whose persistent storage entry has expired.
    pub fn list_members(env: Env, start: u32, limit: u32) -> Vec<Member> {
        let all = storage::get_members(&env);
        let mut out = Vec::new(&env);
        let len = all.len();
        let from = start.min(len);
        let to = (start + limit).min(len);
        for i in from..to {
            // `i < len`, so the index is always present.
            let Some(addr) = all.get(i) else {
                continue;
            };
            if let Some(m) = storage::get_member(&env, &addr) {
                out.push_back(m);
            }
        }
        out
    }

    /// Returns up to `limit` treasury proposals starting at proposal id `start`.
    /// Ids are sequential from 0; absent entries (expired from storage) are
    /// skipped.  Returns an empty vec when `start` is past the last proposal.
    pub fn list_treasury_proposals(env: Env, start: u32, limit: u32) -> Vec<TreasuryProposal> {
        let count = storage::get_proposal_count(&env, storage::DataKey::NextTreasuryId);
        let mut out = Vec::new(&env);
        let from = start.min(count);
        let to = (start + limit).min(count);
        for id in from..to {
            if let Some(p) = storage::get_treasury_proposal(&env, id) {
                out.push_back(p);
            }
        }
        out
    }

    /// Lists up to `limit` loans within the proposal-ID window starting at
    /// `start`. Missing loans are skipped and do not extend the window.
    pub fn list_loans(env: Env, start: u32, limit: u32) -> Vec<Loan> {
        let count = storage::get_proposal_count(&env, storage::DataKey::NextProposalId);
        let mut loans = Vec::new(&env);
        for id in start.min(count)..start.saturating_add(limit).min(count) {
            if let Some(loan) = storage::get_loan(&env, id) {
                loans.push_back(loan);
            }
        }
        loans
    }

    // ==================== migration seed entrypoints (#332 / #333) ====================

    /// Seeds member records from a prior deployment. Admin-only; rejected after
    /// `lock_seeding()` is called. See `docs/MIGRATION.md` for the full procedure.
    pub fn seed_members(env: Env, caller: Address, members: Vec<Member>) -> Result<(), Error> {
        migration::seed_members(&env, caller, members)
    }

    /// Seeds historical stake records. Admin-only before `lock_seeding`;
    /// returns `SeedingLocked` after migration closes and emits no event.
    pub fn seed_stakes(
        env: Env,
        caller: Address,
        stakes: Vec<SeedStakeEntry>,
    ) -> Result<(), Error> {
        migration::seed_stakes(&env, caller, stakes)
    }

    /// Seeds historical pending-yield accumulators. Admin-only before
    /// `lock_seeding`; returns `SeedingLocked` after migration closes.
    pub fn seed_pending_yield(
        env: Env,
        caller: Address,
        yields: Vec<SeedYieldEntry>,
    ) -> Result<(), Error> {
        migration::seed_pending_yield(&env, caller, yields)
    }

    /// Seeds historical loan records. Admin-only before `lock_seeding`;
    /// returns `SeedingLocked` after migration closes.
    pub fn seed_loans(env: Env, caller: Address, loans: Vec<Loan>) -> Result<(), Error> {
        migration::seed_loans(&env, caller, loans)
    }

    /// Seeds historical name-registry records. Admin-only before
    /// `lock_seeding`; returns `SeedingLocked` after migration closes.
    pub fn seed_names(env: Env, caller: Address, names: Vec<SeedNameEntry>) -> Result<(), Error> {
        migration::seed_names(&env, caller, names)
    }

    /// Seeds historical content-hash documents. Admin-only before
    /// `lock_seeding`; returns `SeedingLocked` after migration closes.
    pub fn seed_docs(env: Env, caller: Address, docs: Vec<SeedDocEntry>) -> Result<(), Error> {
        migration::seed_docs(&env, caller, docs)
    }

    /// Irreversibly closes the seeding window. All `seed_*` entrypoints reject
    /// calls after this. Admin-only. Returns `SeedingAlreadyLocked` if already
    /// locked.
    pub fn lock_seeding(env: Env, caller: Address) -> Result<(), Error> {
        migration::lock_seeding(&env, caller)
    }

    /// Returns `true` while the seeding window is open (before `lock_seeding`
    /// is called).
    pub fn is_seeding_locked(env: Env) -> bool {
        storage::is_seeding_locked(&env)
    }
}
