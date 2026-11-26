use soroban_sdk::{Address, Env, String};

use crate::error::Error;
use crate::privacy;
use crate::storage;
use crate::types::{ProposalStatus, TreasuryProposal, ONE};
use crate::util;

// `env.events().publish` is deprecated in soroban-sdk in favour of
// `#[contractevent]`, but migration is a coordinated, breaking wire-format
// change (#85).  Suppress per-function so unrelated deprecations still surface.
pub fn propose_withdrawal(
    env: &Env,
    proposer: Address,
    amount: i128,
    destination: Address,
    reason: String,
    private: bool,
) -> Result<u32, Error> {
    util::require_initialized(env)?;
    let base = storage::get_token(env)?;
    propose_withdrawal_in_asset(env, proposer, amount, base, destination, reason, private)
}

/// Proposes paying `amount` of `asset` (which must be whitelisted) out of the
/// treasury. Funds reserved for approved-but-undisbursed loans in the same
/// asset are not available (#172).
#[allow(deprecated)]
pub fn propose_withdrawal_in_asset(
    env: &Env,
    proposer: Address,
    amount: i128,
    asset: Address,
    destination: Address,
    reason: String,
    private: bool,
) -> Result<u32, Error> {
    util::require_initialized(env)?;
    util::require_not_paused(env)?;
    util::auth_member(env, &proposer)?;

    if amount <= 0 {
        return Err(Error::InvalidAmount);
    }
    if destination == env.current_contract_address() || reason.is_empty() || reason.len() > 256 {
        return Err(Error::InvalidProposal);
    }
    if storage::get_open_proposals(env, &proposer) >= storage::MAX_OPEN_PROPOSALS_PER_MEMBER {
        return Err(Error::TooManyOpenProposals);
    }
    if !storage::is_token_whitelisted(env, &asset) {
        return Err(Error::TokenNotWhitelisted);
    }
    let available =
        util::treasury_balance_in_asset(env, &asset) - util::reserved_loan_commitments(env, &asset);
    if amount > available {
        return Err(Error::InsufficientTreasury);
    }

    let id = storage::next_id(env, storage::DataKey::NextTreasuryId);
    let policy = storage::get_policy(env)?;
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
        votes_cast: 0,
        voting_period: policy.voting_period,
        treasury_threshold: policy.treasury_threshold,
        private,
        active_members: storage::get_active_members(env),
    };
    storage::set_treasury_proposal(env, &proposal);
    // Only non-base assets are recorded, so base-token proposals keep their
    // existing storage footprint and events.
    if asset != storage::get_token(env)? {
        storage::set_treasury_asset(env, id, &asset);
        env.events()
            .publish((crate::events::TRE_ASSET,), (id, asset));
    }
    storage::extend_instance(env);

    env.events().publish(
        (crate::events::TRE_PROP,),
        (id, amount, destination, private),
    );
    Ok(id)
}

pub fn vote(env: &Env, voter: Address, proposal_id: u32, support: bool) -> Result<(), Error> {
    util::require_initialized(env)?;
    util::require_not_paused(env)?;
    util::auth_member(env, &voter)?;

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
#[allow(deprecated)]
pub fn tally(
    env: &Env,
    mut proposal: TreasuryProposal,
    voter: &Address,
    support: bool,
) -> Result<(), Error> {
    if proposal.status != ProposalStatus::Pending {
        return Err(Error::NotInVotingPhase);
    }
    // Treasury proposals never expired before this check (#17) — a Pending
    // proposal could sit indefinitely and still be voted through/executed
    // long after the intended voting window. Mirror the loan proposal
    // lifecycle: once VOTING_PERIOD has elapsed since creation, persist the
    // Expired status and reject further votes.
    let voting_end = if proposal.private {
        privacy::commit_reveal_window(&proposal).1
    } else {
        proposal.created_at + proposal.voting_period
    };
    if env.ledger().timestamp() > voting_end {
        return Err(Error::NotInVotingPhase);
    }
    if storage::has_treasury_voted(env, proposal.id, voter) {
        return Err(Error::AlreadyVoted);
    }
    // Callers have already checked `voter` is an active member; this adds the
    // electorate rule for both open votes and commit-reveal reveals (#73).
    let member = storage::get_member(env, voter).ok_or(Error::NotMember)?;
    util::require_in_electorate(&member, proposal.created_at)?;
    proposal.votes_cast = proposal.votes_cast.saturating_add(1);

    let weight = util::voting_weight(env, voter);
    if support {
        proposal.for_votes += weight;
    } else {
        proposal.against_votes += weight;
    }
    proposal.votes_cast += 1;
    storage::set_treasury_voted(env, proposal.id, voter);
    env.events().publish(
        (crate::events::TRE_VOTE,),
        (proposal.id, voter.clone(), support),
    );

    // Quorum is fixed against the electorate snapshotted at creation (#73).
    let required = util::required_votes(proposal.active_members, proposal.treasury_threshold);
    if proposal.for_votes >= required {
        // Persisted once, below: `execute` reads nothing back from storage,
        // and a failed transfer traps the whole transaction anyway.
        proposal.status = ProposalStatus::ApprovedPendingDisbursement;
        if execute(env, &mut proposal).is_err() {
            env.events()
                .publish((crate::events::TRE_WAIT,), (proposal.id, proposal.amount));
        }
    } else {
        let remaining = proposal.active_members.saturating_sub(proposal.votes_cast);
        let vote_cap = ONE
            .checked_add(util::MAX_STAKE_BONUS)
            .ok_or(Error::ArithmeticOverflow)?;
        let max_remaining = (remaining as i128)
            .checked_mul(vote_cap)
            .ok_or(Error::ArithmeticOverflow)?;
        let max_possible = proposal
            .for_votes
            .checked_add(max_remaining)
            .ok_or(Error::ArithmeticOverflow)?;
        if max_possible < required {
            proposal.status = ProposalStatus::Rejected;
            env.events().publish(
                (crate::events::TRE_REJ,),
                (proposal.id, proposal.for_votes, proposal.against_votes),
            );
        }
    }
    storage::set_treasury_proposal(env, &proposal);
    Ok(())
}

/// Executes a treasury withdrawal whose proposal already passed its vote but
/// could not be paid out at the time because the treasury was short (status
/// `ApprovedPendingDisbursement`, see `tre_wait`). Exposed as
/// `execute_treasury_proposal`. Permissionless and callable by anyone, member
/// or not — a keeper call, like `loans::mark_loan_defaulted`, not a
/// privileged action, so there's nothing to authorize.
///
/// Why no auth is needed: the members already approved this exact transfer.
/// The `amount` and `destination` are fixed on the stored proposal, which the
/// vote approved as-is. The caller passes only `proposal_id` and cannot
/// change who is paid or how much.
///
/// What the caller can influence: only *when* the transfer happens, and only
/// once the treasury can cover it. The caller pays the transaction fee and
/// receives nothing, so there is no incentive to grief.
///
/// Failure mode: if the treasury still can't cover the amount this returns
/// `InsufficientTreasury` and nothing changes. Repeated calls are safe — once
/// executed, the status is `Executed` and the status guard below rejects every
/// further call, so a withdrawal can never be paid twice.
///
/// Do not add a `require_auth` here: requiring an admin or the proposer would
/// let a single absent party block an outcome the DAO already voted for. The
/// `non_member_can_execute_approved_treasury_proposal_without_auth` test pins
/// this.
pub fn execute_approved(env: &Env, proposal_id: u32) -> Result<(), Error> {
    util::require_initialized(env)?;
    util::require_not_paused(env)?;
    let mut proposal =
        storage::get_treasury_proposal(env, proposal_id).ok_or(Error::TreasuryProposalNotFound)?;
    if proposal.status != ProposalStatus::ApprovedPendingDisbursement {
        return Err(Error::NotInVotingPhase);
    }
    execute(env, &mut proposal)?;
    storage::set_treasury_proposal(env, &proposal);
    Ok(())
}

#[allow(deprecated)]
fn execute(env: &Env, proposal: &mut TreasuryProposal) -> Result<(), Error> {
    let asset = storage::get_treasury_asset(env, proposal.id)?;
    if util::treasury_balance_in_asset(env, &asset) < proposal.amount {
        return Err(Error::InsufficientTreasury);
    }
    proposal.status = ProposalStatus::Executed;
    // Commit the effect before interacting with the external asset contract.
    storage::set_treasury_proposal(env, proposal);
    util::token_client_for(env, &asset).transfer(
        &util::contract_address(env),
        &proposal.destination,
        &proposal.amount,
    );
    env.events().publish(
        (crate::events::TRE_EXEC,),
        (proposal.id, proposal.amount, proposal.destination.clone()),
    );
    Ok(())
}

/// Cancel a treasury proposal during its editing period. Only the original
/// proposer may cancel, only while the proposal is still Pending and within
/// the editing window. Emits a dedicated `ProposalCancelled` event so
/// off-chain indexers can track cancellations cleanly.
#[allow(deprecated)]
pub fn cancel_treasury_proposal(
    env: &Env,
    proposer: Address,
    proposal_id: u32,
) -> Result<(), Error> {
    util::require_initialized(env)?;
    util::require_not_paused(env)?;
    util::auth_member(env, &proposer)?;
    let mut proposal =
        storage::get_treasury_proposal(env, proposal_id).ok_or(Error::TreasuryProposalNotFound)?;
    if proposal.proposer != proposer {
        return Err(Error::NotAuthorized);
    }
    if proposal.status != ProposalStatus::Pending {
        return Err(Error::NotInVotingPhase);
    }
    let now = env.ledger().timestamp();
    // Treasury proposals don't carry an explicit editing window; treat the
    // editing period as the policy's editing_period from creation.
    let policy = storage::get_policy(env)?;
    if now > proposal.created_at + policy.editing_period {
        return Err(Error::NotInEditingPhase);
    }

    proposal.status = ProposalStatus::Cancelled;
    storage::set_treasury_proposal(env, &proposal);

    env.events()
        .publish((crate::events::PROP_CANC,), (proposal_id, now));
    Ok(())
}

// `env.events().publish` is deprecated in soroban-sdk in favour of
// `#[contractevent]`, but migration is a coordinated, breaking wire-format
// change (#85).  Suppress per-function so unrelated deprecations still surface.
#[allow(deprecated)]
pub fn expire_treasury_proposal(env: &Env, proposal_id: u32) -> Result<(), Error> {
    util::require_initialized(env)?;
    util::require_not_paused(env)?;
    let mut proposal =
        storage::get_treasury_proposal(env, proposal_id).ok_or(Error::TreasuryProposalNotFound)?;

    // Keep keeper retries idempotent: a proposal already persisted as Expired
    // is a no-op and must not emit a second event.
    if proposal.status == ProposalStatus::Expired {
        return Ok(());
    }
    if proposal.status != ProposalStatus::Pending {
        return Err(Error::NotInVotingPhase); // Re-using error
    }

    // A private proposal's voting window is its full commit + reveal window,
    // the same boundary `tally` enforces. Expiring at `voting_period` alone
    // would lock committed voters out of revealing (#434).
    let voting_end = if proposal.private {
        privacy::commit_reveal_window(&proposal).1
    } else {
        proposal.created_at + proposal.voting_period
    };
    if env.ledger().timestamp() > voting_end {
        proposal.status = ProposalStatus::Expired;
        storage::set_treasury_proposal(env, &proposal);
        env.events()
            .publish((crate::events::TRE_EXP,), (proposal_id, proposal.proposer));
        Ok(())
    } else {
        Err(Error::NotInVotingPhase) // Re-using error
    }
}
