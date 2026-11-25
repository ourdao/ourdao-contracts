//! Commit-reveal voting for private treasury proposals. On-chain encrypted
//! voting isn't available on Soroban, so privacy is achieved with a two-phase
//! scheme: members first submit a hash of `(voter, proposal_id, support, salt)`, hiding their
//! choice while voting is open, then reveal it later. The tally only runs on
//! reveal, so no one sees the running result influence how others vote.
//!
//! Weight is read live when a vote is revealed, so it must not be raisable
//! after the reveals start: `staking::stake` refuses while the staker, or the
//! delegatee it backs, holds an unrevealed commitment (#453), mirroring the
//! existing `unstake` lock.
//!
//! Applies to treasury proposals created with `private = true`.

use soroban_sdk::{xdr::ToXdr, Address, Bytes, BytesN, Env};

use crate::error::Error;
use crate::storage;
use crate::treasury;
use crate::types::{ProposalStatus, TreasuryProposal};
use crate::util;

/// The commit phase closes one voting period after creation; the reveal phase
/// closes one more voting period after that. Shared by the phase guards, the
/// shared tally, and the public window view so clients and the contract agree.
pub fn commit_reveal_window(proposal: &TreasuryProposal) -> (u64, u64) {
    let commit_end = proposal.created_at + proposal.voting_period;
    let reveal_end = commit_end + proposal.voting_period;
    (commit_end, reveal_end)
}

/// Read-only pre-check for a reveal. Returns `false` for both a missing
/// commitment and a mismatched `(support, salt)` so callers can avoid paying
/// for a reveal transaction that cannot succeed.
pub fn verify_commitment(
    env: &Env,
    voter: Address,
    proposal_id: u32,
    support: bool,
    salt: &BytesN<32>,
) -> bool {
    let Some(stored) = storage::get_commit(env, proposal_id, &voter) else {
        return false;
    };
    stored == compute_commitment(env, &voter, proposal_id, support, salt)
}

/// Public view of the commit/reveal boundaries for a private proposal.
pub fn get_commit_reveal_window(env: &Env, proposal_id: u32) -> Result<(u64, u64), Error> {
    let proposal =
        storage::get_treasury_proposal(env, proposal_id).ok_or(Error::TreasuryProposalNotFound)?;
    if !proposal.private {
        return Err(Error::NotAuthorized);
    }
    Ok(commit_reveal_window(&proposal))
}

/// Domain-bound commitment: SHA256(XDR(voter) || proposal_id_be || support_byte || salt).
/// The voter and proposal binding prevents commitment copying and cross-proposal reuse.
pub fn compute_commitment(
    env: &Env,
    voter: &Address,
    proposal_id: u32,
    support: bool,
    salt: &BytesN<32>,
) -> BytesN<32> {
    let mut preimage = Bytes::new(env);
    // Use Soroban's canonical serialized Address (a tagged XDR value).
    preimage.append(&voter.to_xdr(env));
    for byte in proposal_id.to_be_bytes() {
        preimage.push_back(byte);
    }
    preimage.push_back(if support { 1 } else { 0 });
    preimage.append(&Bytes::from(salt.clone()));
    env.crypto().sha256(&preimage).to_bytes()
}

// `env.events().publish` is deprecated in soroban-sdk in favour of
// `#[contractevent]`, but migration is a coordinated, breaking wire-format
// change (#85).  Suppress per-function so unrelated deprecations still surface.
#[allow(deprecated)]
pub fn commit_vote(
    env: &Env,
    voter: Address,
    proposal_id: u32,
    commitment: BytesN<32>,
) -> Result<(), Error> {
    util::require_initialized(env)?;
    util::require_not_paused(env)?;
    let member = util::auth_member(env, &voter)?;

    let proposal =
        storage::get_treasury_proposal(env, proposal_id).ok_or(Error::TreasuryProposalNotFound)?;
    if !proposal.private {
        return Err(Error::NotAuthorized);
    }
    // Fail at commit rather than letting the commitment be stored and only
    // rejected at reveal (#73).
    util::require_in_electorate(&member, proposal.created_at)?;
    if proposal.status != ProposalStatus::Pending {
        return Err(Error::NotInVotingPhase);
    }
    if env.ledger().timestamp() > commit_reveal_window(&proposal).0 {
        return Err(Error::VotingEnded);
    }
    if storage::has_treasury_voted(env, proposal_id, &voter) {
        return Err(Error::AlreadyVoted);
    }
    // A commitment is binding: overwriting it would let a member keep changing
    // their hidden vote until the reveal phase (#49).
    if storage::get_commit(env, proposal_id, &voter).is_some() {
        return Err(Error::AlreadyVoted);
    }

    storage::set_commit(env, proposal_id, &voter, &commitment);
    env.events()
        .publish((crate::events::COMMITTED,), (proposal_id, voter));
    Ok(())
}

#[allow(deprecated)]
pub fn reveal_vote(
    env: &Env,
    voter: Address,
    proposal_id: u32,
    support: bool,
    salt: BytesN<32>,
) -> Result<(), Error> {
    util::require_initialized(env)?;
    util::require_not_paused(env)?;
    util::auth_member(env, &voter)?;

    let proposal =
        storage::get_treasury_proposal(env, proposal_id).ok_or(Error::TreasuryProposalNotFound)?;
    if !proposal.private {
        return Err(Error::NotAuthorized);
    }
    let (commit_end, reveal_end) = commit_reveal_window(&proposal);
    if env.ledger().timestamp() <= commit_end {
        return Err(Error::NotYetRevealed);
    }
    if env.ledger().timestamp() > reveal_end {
        return Err(Error::VotingEnded);
    }
    if storage::has_treasury_voted(env, proposal_id, &voter) {
        return Err(Error::AlreadyRevealed);
    }

    let stored = storage::get_commit(env, proposal_id, &voter).ok_or(Error::NoCommitment)?;
    let expected = compute_commitment(env, &voter, proposal_id, support, &salt);
    if stored != expected {
        return Err(Error::CommitmentMismatch);
    }

    // Consume the commitment; the shared tally records the vote and, if the
    // threshold is met, executes the withdrawal.
    storage::remove_commit(env, proposal_id, &voter);
    env.events().publish(
        (crate::events::REVEALED,),
        (proposal_id, voter.clone(), support),
    );
    treasury::tally(env, proposal, &voter, support)
}
