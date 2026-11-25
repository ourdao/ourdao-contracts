//! Staking module. Members lock tokens to signal commitment and gain a
//! capped boost to their voting weight (see `util::voting_weight`). Staked
//! funds are tracked separately from the treasury and are never lent out or
//! distributed as yield.

use soroban_sdk::{Address, Env};

use crate::error::Error;
use crate::privacy;
use crate::storage;
use crate::types::{ProposalPhase, ProposalStatus};
use crate::util;

// `env.events().publish` is deprecated in soroban-sdk in favour of
// `#[contractevent]`, but migration is a coordinated, breaking wire-format
// change (#85).  Suppress per-function so unrelated deprecations still surface.
#[allow(deprecated)]
pub fn stake(env: &Env, member: Address, amount: i128) -> Result<(), Error> {
    util::require_initialized(env)?;
    util::require_not_paused(env)?;
    util::auth_member(env, &member)?;
    if amount <= 0 {
        return Err(Error::InvalidAmount);
    }
    if weight_locked_by_commitment(env, &member) {
        return Err(Error::HasActiveLoan);
    }

    // Commit stake accounting first (CEI); a failed transfer reverts these writes.
    let new_stake = storage::get_stake(env, &member) + amount;
    storage::set_stake(env, &member, new_stake);
    storage::set_total_staked(env, storage::get_total_staked(env) + amount);
    env.storage().persistent().set(
        &crate::storage::DataKey::StakeTime(member.clone()),
        &env.ledger().timestamp(),
    );
    storage::extend_instance(env);

    // Staking is always denominated in the DAO's configured base token.
    // Move the principal into the DAO contract after accounting writes; a
    // failed transfer reverts the whole Soroban transaction.
    util::token_client(env)?.transfer(&member, util::contract_address(env), &amount);

    let voting_weight = util::voting_weight(env, &member);
    env.events().publish(
        (crate::events::STAKED,),
        (member, amount, new_stake, voting_weight),
    );
    Ok(())
}

/// True while a stake increase by `member` would raise the weight of a
/// still-unrevealed private vote (#453). `tally` reads voting weight live at
/// reveal, and reveals land one at a time with the running tally public, so
/// without this a voter could commit, watch the others reveal, then stake up
/// before revealing. That covers the member's own commitment and, because a
/// delegator's stake counts toward its delegatee's weight, a commitment held
/// by the member's delegatee. The mirror of the `unstake` lock (#330).
fn weight_locked_by_commitment(env: &Env, member: &Address) -> bool {
    let delegatee = storage::get_delegation(env, member);
    let now = env.ledger().timestamp();
    let treasury_count = storage::get_proposal_count(env, storage::DataKey::NextTreasuryId);
    for id in 0..treasury_count {
        if let Some(p) = storage::get_treasury_proposal(env, id) {
            if !p.private
                || p.status != ProposalStatus::Pending
                || now > privacy::commit_reveal_window(&p).1
            {
                continue;
            }
            if storage::get_commit(env, id, member).is_some() {
                return true;
            }
            if let Some(ref d) = delegatee {
                if storage::get_commit(env, id, d).is_some() {
                    return true;
                }
            }
        }
    }
    false
}

#[allow(deprecated)]
pub fn unstake(env: &Env, member: Address, amount: i128) -> Result<(), Error> {
    util::require_initialized(env)?;
    util::require_not_paused(env)?;
    member.require_auth();
    if amount <= 0 {
        return Err(Error::InvalidAmount);
    }

    let current = storage::get_stake(env, &member);
    if current == 0 {
        return Err(Error::NoStake);
    }
    if amount > current {
        return Err(Error::InsufficientStake);
    }

    let last_stake: u64 = env
        .storage()
        .persistent()
        .get(&crate::storage::DataKey::StakeTime(member.clone()))
        .unwrap_or(0);
    let policy = storage::get_policy(env)?;
    if env.ledger().timestamp() < last_stake + policy.cooldown_period {
        return Err(Error::CooldownActive);
    }

    // Issue #175: disallow unstaking while the member has an active pending
    // loan proposal still in the voting phase (collateral must remain locked
    // until the proposal reaches a terminal state).
    let proposal_count = storage::get_proposal_count(env, storage::DataKey::NextProposalId);
    for id in 0..proposal_count {
        if let Some(p) = storage::get_loan_proposal(env, id) {
            if p.borrower == member
                && (p.phase == ProposalPhase::Editing || p.phase == ProposalPhase::Voting)
                && p.status == ProposalStatus::Pending
                && env.ledger().timestamp() <= p.editing_period_end.saturating_add(p.voting_period)
            {
                return Err(Error::HasActiveLoan);
            }
        }
    }

    // A vote cast with stake-derived weight must remain backed until that
    // proposal closes. This includes loan ballots, treasury ballots, and
    // unrevealed private commitments, not just the borrower's own request.
    let treasury_count = storage::get_proposal_count(env, storage::DataKey::NextTreasuryId);
    for id in 0..treasury_count {
        if let Some(p) = storage::get_treasury_proposal(env, id) {
            let voting_end =
                p.created_at
                    .saturating_add(
                        p.voting_period
                            .saturating_mul(if p.private { 2 } else { 1 }),
                    );
            if p.status == ProposalStatus::Pending
                && env.ledger().timestamp() <= voting_end
                && (storage::has_treasury_voted(env, id, &member)
                    || storage::get_commit(env, id, &member).is_some())
            {
                return Err(Error::HasActiveLoan);
            }
        }
    }
    for id in 0..proposal_count {
        if let Some(p) = storage::get_loan_proposal(env, id) {
            if p.status == ProposalStatus::Pending
                && env.ledger().timestamp() <= p.editing_period_end.saturating_add(p.voting_period)
                && storage::has_loan_voted(env, id, &member)
            {
                return Err(Error::HasActiveLoan);
            }
        }
    }

    let new_stake = current - amount;
    storage::set_stake(env, &member, new_stake);
    storage::set_total_staked(env, storage::get_total_staked(env) - amount);
    storage::extend_instance(env);

    // Return staked principal in the same configured base token. Whitelisted
    // treasury assets never participate in staking.
    util::token_client(env)?.transfer(&util::contract_address(env), &member, &amount);

    let voting_weight = util::voting_weight(env, &member);
    env.events().publish(
        (crate::events::UNSTAKED,),
        (member, amount, new_stake, voting_weight),
    );
    Ok(())
}
