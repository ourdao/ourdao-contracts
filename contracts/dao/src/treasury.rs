use soroban_sdk::{symbol_short, Address, Env, String};

use crate::error::Error;
use crate::storage;
use crate::types::{ProposalStatus, TreasuryProposal, TREASURY_THRESHOLD};
use crate::util;

pub fn propose_withdrawal(
    env: &Env,
    proposer: Address,
    amount: i128,
    destination: Address,
    reason: String,
    private: bool,
) -> Result<u32, Error> {
    util::require_initialized(env)?;
    util::require_not_paused(env)?;
    util::require_active_member(env, &proposer)?;

    if amount <= 0 {
        return Err(Error::InvalidAmount);
    }
    if amount > util::treasury_balance(env) {
        return Err(Error::InsufficientTreasury);
    }

    let id = storage::next_id(env, storage::DataKey::NextTreasuryId);
    let proposal = TreasuryProposal {
        id,
        proposer,
        amount,
        destination: destination.clone(),
        reason,
        created_at: env.ledger().timestamp(),
        status: ProposalStatus::Pending,
        for_votes: 0,
        against_votes: 0,
        private,
    };
    storage::set_treasury_proposal(env, &proposal);
    storage::extend_instance(env);

    env.events().publish(
        (symbol_short!("tre_prop"),),
        (id, amount, destination, private),
    );
    Ok(id)
}

pub fn vote(env: &Env, voter: Address, proposal_id: u32, support: bool) -> Result<(), Error> {
    util::require_initialized(env)?;
    util::require_not_paused(env)?;
    util::require_active_member(env, &voter)?;

    let proposal =
        storage::get_treasury_proposal(env, proposal_id).ok_or(Error::TreasuryProposalNotFound)?;
    if proposal.private {
        // Private proposals must go through commit → reveal, not open voting.
        return Err(Error::NotAuthorized);
    }
    tally(env, proposal, &voter, support)
}

/// Shared vote-recording + execution path. Used by open voting and by the
/// commit-reveal privacy module once a vote is revealed. Assumes the caller has
/// already authorized `voter` and enforced any privacy-mode rules.
pub fn tally(
    env: &Env,
    mut proposal: TreasuryProposal,
    voter: &Address,
    support: bool,
) -> Result<(), Error> {
    if proposal.status != ProposalStatus::Pending {
        return Err(Error::NotInVotingPhase);
    }
    if storage::has_treasury_voted(env, proposal.id, voter) {
        return Err(Error::AlreadyVoted);
    }

    let weight = util::voting_weight(env, voter);
    if support {
        proposal.for_votes += weight;
    } else {
        proposal.against_votes += weight;
    }
    storage::set_treasury_voted(env, proposal.id, voter);
    env.events().publish(
        (symbol_short!("tre_vote"),),
        (proposal.id, voter.clone(), support),
    );

    let required = util::required_votes(storage::get_active_members(env), TREASURY_THRESHOLD);
    if proposal.for_votes >= required {
        execute(env, &mut proposal)?;
    }
    storage::set_treasury_proposal(env, &proposal);
    Ok(())
}

fn execute(env: &Env, proposal: &mut TreasuryProposal) -> Result<(), Error> {
    if util::treasury_balance(env) < proposal.amount {
        return Err(Error::InsufficientTreasury);
    }
    proposal.status = ProposalStatus::Executed;
    util::token_client(env).transfer(
        &util::contract_address(env),
        &proposal.destination,
        &proposal.amount,
    );
    env.events().publish(
        (symbol_short!("tre_exec"),),
        (proposal.id, proposal.amount, proposal.destination.clone()),
    );
    Ok(())
}
