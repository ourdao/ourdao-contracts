#![no_std]
// Events use the classic `env.events().publish` API, which is deprecated in
// favor of `#[contractevent]` but still fully supported in soroban-sdk 26.
#![allow(deprecated)]
//! OurDAO — a member-owned lending DAO for Stellar Soroban.
//!
//! Ported from the original EVM `UnifiedLendingDAO` (Solidity). All value moves
//! through a single configurable token set at initialization (USDC, XLM via the
//! Stellar Asset Contract, or any Stellar asset). The four EVM-ecosystem
//! extensions are replaced with Stellar-native equivalents:
//!
//! * ENS governance  -> [`registry`] name registry
//! * Filecoin storage -> [`docs`] content-hash proposal metadata
//! * FHE encrypted votes -> [`privacy`] commit-reveal voting
//! * Symbiotic restaking -> [`staking`] voting-weight staking

mod admin;
mod docs;
mod error;
mod loans;
mod membership;
mod privacy;
mod registry;
mod staking;
mod storage;
mod treasury;
mod types;
mod util;

#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, Address, Bytes, BytesN, Env, String, Vec};

pub use error::Error;
pub use storage::ProposalKind;
pub use types::{Loan, LoanPolicy, LoanProposal, LoanTerms, Member, TreasuryProposal};

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

    pub fn add_admin(env: Env, caller: Address, admin: Address) -> Result<(), Error> {
        admin::add_admin(&env, caller, admin)
    }

    pub fn remove_admin(env: Env, caller: Address, admin: Address) -> Result<(), Error> {
        admin::remove_admin(&env, caller, admin)
    }

    pub fn set_consensus_threshold(env: Env, caller: Address, threshold: u32) -> Result<(), Error> {
        admin::set_consensus_threshold(&env, caller, threshold)
    }

    pub fn set_loan_policy(env: Env, caller: Address, policy: LoanPolicy) -> Result<(), Error> {
        admin::set_policy(&env, caller, policy)
    }

    pub fn pause(env: Env, caller: Address) -> Result<(), Error> {
        admin::pause(&env, caller)
    }

    pub fn unpause(env: Env, caller: Address) -> Result<(), Error> {
        admin::unpause(&env, caller)
    }

    // ==================== membership ====================

    pub fn register_member(env: Env, member: Address) -> Result<(), Error> {
        membership::register_member(&env, member)
    }

    pub fn exit_dao(env: Env, member: Address) -> Result<(), Error> {
        membership::exit_dao(&env, member)
    }

    /// Withdraw accrued loan-interest yield for the caller.
    pub fn claim_rewards(env: Env, member: Address) -> Result<i128, Error> {
        membership::claim_rewards(&env, member)
    }

    // ==================== loans ====================

    pub fn request_loan(env: Env, borrower: Address, amount: i128) -> Result<u32, Error> {
        loans::request_loan(&env, borrower, amount)
    }

    pub fn edit_loan_proposal(
        env: Env,
        borrower: Address,
        proposal_id: u32,
        new_amount: i128,
    ) -> Result<(), Error> {
        loans::edit_loan_proposal(&env, borrower, proposal_id, new_amount)
    }

    pub fn vote_on_loan_proposal(
        env: Env,
        voter: Address,
        proposal_id: u32,
        support: bool,
    ) -> Result<(), Error> {
        loans::vote_on_loan_proposal(&env, voter, proposal_id, support)
    }

    pub fn repay_loan(env: Env, borrower: Address, loan_id: u32) -> Result<(), Error> {
        loans::repay_loan(&env, borrower, loan_id)
    }

    // ==================== treasury ====================

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

    pub fn vote_on_treasury_proposal(
        env: Env,
        voter: Address,
        proposal_id: u32,
        support: bool,
    ) -> Result<(), Error> {
        treasury::vote(&env, voter, proposal_id, support)
    }

    // ==================== native swap: staking ====================

    pub fn stake(env: Env, member: Address, amount: i128) -> Result<(), Error> {
        staking::stake(&env, member, amount)
    }

    pub fn unstake(env: Env, member: Address, amount: i128) -> Result<(), Error> {
        staking::unstake(&env, member, amount)
    }

    // ==================== native swap: name registry ====================

    pub fn register_name(env: Env, owner: Address, name: String) -> Result<(), Error> {
        registry::register_name(&env, owner, name)
    }

    pub fn resolve_name(env: Env, name: String) -> Option<Address> {
        registry::resolve_name(&env, name)
    }

    pub fn name_of(env: Env, owner: Address) -> Option<String> {
        registry::name_of(&env, owner)
    }

    // ==================== native swap: commit-reveal voting ====================

    pub fn commit_treasury_vote(
        env: Env,
        voter: Address,
        proposal_id: u32,
        commitment: BytesN<32>,
    ) -> Result<(), Error> {
        privacy::commit_vote(&env, voter, proposal_id, commitment)
    }

    pub fn reveal_treasury_vote(
        env: Env,
        voter: Address,
        proposal_id: u32,
        support: bool,
        salt: BytesN<32>,
    ) -> Result<(), Error> {
        privacy::reveal_vote(&env, voter, proposal_id, support, salt)
    }

    // ==================== native swap: content-hash docs ====================

    pub fn attach_document(
        env: Env,
        caller: Address,
        kind: ProposalKind,
        proposal_id: u32,
        content_hash: Bytes,
    ) -> Result<(), Error> {
        docs::attach_document(&env, caller, kind, proposal_id, content_hash)
    }

    pub fn get_document(env: Env, kind: ProposalKind, proposal_id: u32) -> Option<Bytes> {
        docs::get_document(&env, kind, proposal_id)
    }

    // ==================== views ====================

    pub fn get_member(env: Env, address: Address) -> Option<Member> {
        storage::get_member(&env, &address)
    }

    pub fn get_loan(env: Env, loan_id: u32) -> Option<Loan> {
        storage::get_loan(&env, loan_id)
    }

    pub fn get_loan_proposal(env: Env, proposal_id: u32) -> Option<LoanProposal> {
        storage::get_loan_proposal(&env, proposal_id)
    }

    pub fn get_treasury_proposal(env: Env, proposal_id: u32) -> Option<TreasuryProposal> {
        storage::get_treasury_proposal(&env, proposal_id)
    }

    pub fn get_loan_policy(env: Env) -> LoanPolicy {
        storage::get_policy(&env)
    }

    pub fn get_admins(env: Env) -> Vec<Address> {
        storage::get_admins(&env)
    }

    pub fn is_admin(env: Env, address: Address) -> bool {
        util::is_admin(&env, &address)
    }

    pub fn is_member(env: Env, address: Address) -> bool {
        matches!(
            storage::get_member(&env, &address),
            Some(m) if m.status == types::MemberStatus::ActiveMember
        )
    }

    pub fn is_eligible_for_loan(env: Env, member: Address) -> bool {
        loans::is_eligible_for_loan(&env, &member)
    }

    pub fn get_treasury_balance(env: Env) -> i128 {
        util::treasury_balance(&env)
    }

    pub fn get_total_members(env: Env) -> u32 {
        storage::get_total_members(&env)
    }

    pub fn get_active_members(env: Env) -> u32 {
        storage::get_active_members(&env)
    }

    pub fn get_consensus_threshold(env: Env) -> u32 {
        storage::get_threshold(&env)
    }

    pub fn get_token(env: Env) -> Address {
        storage::get_token(&env)
    }

    pub fn is_paused(env: Env) -> bool {
        storage::is_paused(&env)
    }

    pub fn get_stake(env: Env, member: Address) -> i128 {
        storage::get_stake(&env, &member)
    }

    pub fn get_pending_yield(env: Env, member: Address) -> i128 {
        storage::get_pending_yield(&env, &member)
    }

    pub fn calculate_loan_terms(env: Env, amount: i128) -> LoanTerms {
        loans::calculate_loan_terms(&env, amount)
    }

    pub fn calculate_exit_share(env: Env, member: Address) -> i128 {
        membership::calculate_exit_share(&env, &member)
    }
}
