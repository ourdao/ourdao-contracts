use soroban_sdk::{Address, Env, String, Vec};

use crate::error::Error;
use crate::storage;
use crate::types::{
    Loan, LoanPolicy, LoanProposal, LoanStatus, LoanTerms, MemberStatus, ProposalPhase,
    ProposalStatus, BASIS_POINTS, ONE,
};
use crate::util;

/// Largest principal the treasury-ratio cap allows against `treasury`. Never
/// negative: `treasury_balance` subtracts total staked and can legitimately go
/// below zero, and an empty or negative treasury can fund nothing (#80).
/// Floor division rounds down to favour the treasury.
pub fn max_loan_for(policy: &LoanPolicy, treasury: i128) -> i128 {
    if treasury <= 0 {
        return 0;
    }
    treasury
        .checked_mul(policy.max_loan_to_treasury_ratio as i128)
        .and_then(|v| v.checked_div(BASIS_POINTS))
        .unwrap_or(0)
}

/// Quote the terms a loan of `amount` would carry right now. Interest scales
/// linearly with the loan's size relative to the treasury, clamped to policy.
///
/// This always returns a rate, even for a loan that cannot be issued: when
/// the treasury is empty or negative the rate is the policy maximum, and an
/// amount above the treasury-ratio cap gets a rate too. Check
/// `amount <= terms.max_loan` before showing a quote as available (#80).
pub fn calculate_loan_terms(env: &Env, amount: i128) -> Result<LoanTerms, Error> {
    quote_terms(env, amount, util::treasury_balance(env))
}

/// [`calculate_loan_terms`] against an explicit treasury balance, so loans in
/// a non-base asset are priced against the treasury held in that asset.
fn quote_terms(env: &Env, amount: i128, treasury: i128) -> Result<LoanTerms, Error> {
    let policy = storage::get_policy(env)?;

    let loan_ratio = if treasury > 0 {
        amount
            .checked_mul(BASIS_POINTS)
            .and_then(|v| v.checked_add(treasury - ONE))
            .and_then(|v| v.checked_div(treasury))
            .ok_or(Error::ArithmeticOverflow)?
            .min(BASIS_POINTS)
    } else {
        BASIS_POINTS
    };
    let spread = (policy.max_interest_rate - policy.min_interest_rate) as i128;
    let added_rate = loan_ratio
        .checked_mul(spread)
        .and_then(|v| v.checked_add(BASIS_POINTS - ONE))
        .and_then(|v| v.checked_div(BASIS_POINTS))
        .ok_or(Error::ArithmeticOverflow)?;
    let rate = (policy.min_interest_rate as i128)
        .checked_add(added_rate)
        .ok_or(Error::ArithmeticOverflow)?
        .min(policy.max_interest_rate as i128);

    // Member debt: ceil-division rounds up to favour the treasury.
    let interest = if amount > 0 && rate > 0 {
        amount
            .checked_mul(rate)
            .and_then(|v| v.checked_add(BASIS_POINTS - ONE))
            .and_then(|v| v.checked_div(BASIS_POINTS))
            .ok_or(Error::ArithmeticOverflow)?
    } else {
        0
    };
    let total_repayment = amount
        .checked_add(interest)
        .ok_or(Error::ArithmeticOverflow)?;

    Ok(LoanTerms {
        interest_rate: rate as u32,
        total_repayment,
        duration: policy.max_loan_duration,
        max_loan: max_loan_for(&policy, treasury),
    })
}

pub fn is_eligible_for_loan(env: &Env, member: &Address) -> Result<(), Error> {
    let record = match storage::get_member(env, member) {
        Some(m) if m.status == MemberStatus::ActiveMember => m,
        _ => return Err(Error::MemberNotActive),
    };
    if record.has_active_loan {
        return Err(Error::HasActiveLoan);
    }
    let policy = storage::get_policy(env)?;
    let now = env.ledger().timestamp();
    if now.saturating_sub(record.join_time) < policy.min_membership_duration {
        return Err(Error::NotEligibleForLoan);
    }
    if record.last_loan_time != 0
        && now.saturating_sub(record.last_loan_time) < policy.cooldown_period
    {
        return Err(Error::CooldownActive);
    }
    Ok(())
}

// `env.events().publish` is deprecated in soroban-sdk in favour of
// `#[contractevent]`, but migration is a coordinated, breaking wire-format
// change (#85).  Suppress per-function so unrelated deprecations still surface.
pub fn request_loan(
    env: &Env,
    borrower: Address,
    amount: i128,
    metadata_cid: Option<String>,
) -> Result<u32, Error> {
    util::require_initialized(env)?;
    let base = storage::get_token(env)?;
    request_loan_in_asset(env, borrower, amount, base, metadata_cid)
}

/// Requests a loan of `amount` denominated in `asset`, which must be
/// whitelisted. Size limits and pricing use the treasury held in that asset.
#[allow(deprecated)]
pub fn request_loan_in_asset(
    env: &Env,
    borrower: Address,
    amount: i128,
    asset: Address,
    metadata_cid: Option<String>,
) -> Result<u32, Error> {
    util::require_initialized(env)?;
    util::require_not_paused(env)?;
    util::auth_member(env, &borrower)?;
    if !storage::is_token_whitelisted(env, &asset) {
        return Err(Error::TokenNotWhitelisted);
    }

    if amount <= 0 {
        return Err(Error::InvalidAmount);
    }
    if let Some(ref cid) = metadata_cid {
        if cid.is_empty() {
            return Err(Error::InvalidMetadataCid);
        }
        if cid.len() > 64 {
            return Err(Error::DocumentTooLarge);
        }
    }
    is_eligible_for_loan(env, &borrower)?;
    if storage::get_open_proposals(env, &borrower) >= storage::MAX_OPEN_PROPOSALS_PER_MEMBER {
        return Err(Error::TooManyOpenProposals);
    }

    let policy = storage::get_policy(env)?;
    let treasury = util::treasury_balance_in_asset(env, &asset);
    // Same cap the quote reports as `max_loan` (#80).
    let max_loan = max_loan_for(&policy, treasury);
    if amount > max_loan {
        return Err(Error::ExceedsTreasuryRatio);
    }

    let terms = quote_terms(env, amount, treasury)?;
    let now = env.ledger().timestamp();
    let id = storage::next_id(env, storage::DataKey::NextProposalId);
    let proposal = LoanProposal {
        id,
        borrower: borrower.clone(),
        amount,
        interest_rate: terms.interest_rate,
        duration: terms.duration,
        total_repayment: terms.total_repayment,
        created_at: now,
        editing_period_end: now + policy.editing_period,
        phase: ProposalPhase::Editing,
        status: ProposalStatus::Pending,
        for_votes: 0,
        against_votes: 0,
        votes_cast: 0,
        voting_period: policy.voting_period,
        metadata_cid,
        last_edited_at: None,
        active_members: storage::get_active_members(env),
    };
    storage::set_loan_proposal(env, &proposal);
    // Only non-base assets are recorded, so base-token loans keep their
    // existing storage footprint and events.
    if asset != storage::get_token(env)? {
        storage::set_loan_asset(env, id, &asset);
        env.events()
            .publish((crate::events::LOAN_ASST,), (id, asset.clone()));
    }
    storage::extend_instance(env);

    env.events().publish(
        (crate::events::LOAN_REQ,),
        (id, borrower, amount, terms.total_repayment, asset),
    );
    Ok(id)
}

#[allow(deprecated)]
pub fn edit_loan_proposal(
    env: &Env,
    borrower: Address,
    proposal_id: u32,
    new_amount: i128,
    metadata_cid: Option<String>,
) -> Result<(), Error> {
    util::require_initialized(env)?;
    util::require_not_paused(env)?;
    util::auth_member(env, &borrower)?;
    let mut proposal =
        storage::get_loan_proposal(env, proposal_id).ok_or(Error::ProposalNotFound)?;
    if proposal.borrower != borrower {
        return Err(Error::NotBorrower);
    }
    let now = env.ledger().timestamp();
    if proposal.phase != ProposalPhase::Editing || now >= proposal.editing_period_end {
        return Err(Error::NotInEditingPhase);
    }
    if new_amount <= 0 {
        return Err(Error::InvalidAmount);
    }
    if let Some(ref cid) = metadata_cid {
        if cid.is_empty() {
            return Err(Error::InvalidMetadataCid);
        }
        if cid.len() > 64 {
            return Err(Error::DocumentTooLarge);
        }
    }

    let asset = storage::get_loan_asset(env, proposal_id)?;
    let terms = quote_terms(
        env,
        new_amount,
        util::treasury_balance_in_asset(env, &asset),
    )?;
    proposal.amount = new_amount;
    proposal.interest_rate = terms.interest_rate;
    proposal.duration = terms.duration;
    proposal.total_repayment = terms.total_repayment;
    proposal.metadata_cid = metadata_cid;
    proposal.last_edited_at = Some(now);
    storage::set_loan_proposal(env, &proposal);

    env.events().publish(
        (crate::events::LOAN_EDIT,),
        (proposal_id, borrower, new_amount, terms.total_repayment),
    );
    Ok(())
}

/// Cancel a loan proposal during its editing period. Only the original
/// borrower may cancel, on,y while the proposal is still in the editing
/// phase and pending. Emits a dedicated `ProposalCancelled` event so
/// off-chain indexers can track cancellations cleanly.
#[allow(deprecated)]
pub fn cancel_loan_proposal(env: &Env, borrower: Address, proposal_id: u32) -> Result<(), Error> {
    util::require_initialized(env)?;
    util::require_not_paused(env)?;
    util::auth_member(env, &borrower)?;
    let mut proposal =
        storage::get_loan_proposal(env, proposal_id).ok_or(Error::ProposalNotFound)?;
    if proposal.borrower != borrower {
        return Err(Error::NotBorrower);
    }
    let now = env.ledger().timestamp();
    if proposal.phase != ProposalPhase::Editing || now >= proposal.editing_period_end {
        return Err(Error::NotInEditingPhase);
    }
    if proposal.status != ProposalStatus::Pending {
        return Err(Error::NotInEditingPhase);
    }

    proposal.status = ProposalStatus::Cancelled;
    proposal.phase = ProposalPhase::Expired;
    storage::set_loan_proposal(env, &proposal);

    env.events()
        .publish((crate::events::PROP_CANC,), (proposal_id, now));
    Ok(())
}

/// Advances a proposal's phase based on the clock. Returns the (possibly
/// mutated) proposal; the caller is responsible for persisting it.
pub fn refresh_phase(env: &Env, mut proposal: LoanProposal) -> LoanProposal {
    let now = env.ledger().timestamp();
    if proposal.phase == ProposalPhase::Editing && now >= proposal.editing_period_end {
        proposal.phase = ProposalPhase::Voting;
    }
    if proposal.phase == ProposalPhase::Voting
        && now > proposal.editing_period_end + proposal.voting_period
        && proposal.status == ProposalStatus::Pending
    {
        // Timed out without quorum: `Expired` status, distinct from the
        // `Rejected` status written by the early-rejection path.
        proposal.phase = ProposalPhase::Expired;
        proposal.status = ProposalStatus::Expired;
    }
    proposal
}

#[allow(deprecated)]
pub fn vote_on_loan_proposal(
    env: &Env,
    voter: Address,
    proposal_id: u32,
    support: bool,
) -> Result<(), Error> {
    util::require_initialized(env)?;
    util::require_not_paused(env)?;
    util::auth_member(env, &voter)?;
    vote_on_loan_proposal_internal(env, &voter, proposal_id, support)
}

pub fn vote_batch(
    env: &Env,
    voter: Address,
    votes: Vec<crate::types::ProposalVote>,
) -> Result<(), Error> {
    util::require_initialized(env)?;
    util::require_not_paused(env)?;
    util::auth_member(env, &voter)?;

    if votes.len() > storage::MAX_VOTES_PER_BATCH {
        return Err(Error::TooManyVotesInBatch);
    }

    for v in votes.iter() {
        vote_on_loan_proposal_internal(env, &voter, v.proposal_id, v.vote)?;
    }
    Ok(())
}

#[allow(deprecated)]
fn vote_on_loan_proposal_internal(
    env: &Env,
    voter: &Address,
    proposal_id: u32,
    support: bool,
) -> Result<(), Error> {
    let mut proposal = storage::get_loan_proposal(env, proposal_id)
        .ok_or(Error::ProposalNotFound)
        .map(|p| refresh_phase(env, p))?;

    if proposal.phase == ProposalPhase::Editing {
        return Err(Error::NotInVotingPhase);
    }
    if proposal.phase != ProposalPhase::Voting {
        return Err(Error::VotingEnded);
    }
    let now = env.ledger().timestamp();
    // Issue #173: use >= so voting is closed at exactly the deadline second,
    // making voting and post-voting execution strictly mutually exclusive.
    if now >= proposal.editing_period_end + proposal.voting_period {
        return Err(Error::VotingEnded);
    }
    if *voter == proposal.borrower {
        return Err(Error::BorrowerCannotVote);
    }
    if storage::has_loan_voted(env, proposal_id, voter) {
        return Err(Error::AlreadyVoted);
    }
    // Callers have already checked `voter` is an active member; this adds the
    // electorate rule for single and batch votes alike (#73).
    let member = storage::get_member(env, voter).ok_or(Error::NotMember)?;
    util::require_in_electorate(&member, proposal.created_at)?;

    let weight = util::voting_weight(env, voter);
    if support {
        proposal.for_votes += weight;
    } else {
        proposal.against_votes += weight;
    }
    proposal.votes_cast += 1;
    storage::set_loan_voted(env, proposal_id, voter);
    env.events().publish(
        (crate::events::LOAN_VOTE,),
        (proposal_id, voter.clone(), support),
    );

    // Read the policy and the active-member count once per call (#355).
    let policy = storage::get_policy(env)?;
    let threshold = if policy.quorum_bps > 0 {
        policy.quorum_bps
    } else {
        storage::get_threshold(env)?
    };
    // Quorum is fixed against the electorate snapshotted at creation (#73).
    let required = util::required_votes(proposal.active_members, threshold);
    if proposal.for_votes >= required && proposal.status == ProposalStatus::Pending {
        proposal.status = ProposalStatus::ApprovedPendingDisbursement;
        proposal.phase = ProposalPhase::Executed;
        storage::set_loan_proposal(env, &proposal);
        if approve_and_disburse(env, &proposal).is_ok() {
            proposal.status = ProposalStatus::Approved;
        } else {
            env.events()
                .publish((crate::events::LOAN_WAIT,), (proposal.id, proposal.amount));
        }
    } else if proposal.status == ProposalStatus::Pending {
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
            proposal.phase = ProposalPhase::Expired;
            env.events().publish(
                (crate::events::LOAN_REJ,),
                (proposal.id, proposal.for_votes, proposal.against_votes),
            );
        }
    }
    storage::set_loan_proposal(env, &proposal);
    Ok(())
}

/// Disburses a loan whose proposal already passed its vote but could not be
/// paid out at the time because the treasury was short (status
/// `ApprovedPendingDisbursement`, see `loan_wait`). Permissionless and
/// callable by anyone, member or not — this is a keeper call, like
/// [`mark_loan_defaulted`], not a privileged action, so there's nothing to
/// authorize.
///
/// Why no auth is needed: the decision was already made by the members'
/// vote. The borrower and principal are fixed on the stored proposal, and the
/// terms are computed on-chain from the current policy and treasury. The
/// caller passes only `proposal_id` and cannot redirect funds, change the
/// amount, or pick the terms.
///
/// What the caller can influence: only *when* disbursement happens, and only
/// once the treasury can cover it. The caller pays the transaction fee and
/// receives nothing, so there is no incentive to grief; at worst an outsider
/// does the borrower a favour by paying out early.
///
/// Failure mode: if the treasury still can't cover the principal this returns
/// `InsufficientTreasury` and nothing changes. Repeated calls are safe — once
/// disbursed, the status is `Approved` and the status guard below rejects
/// every further call, so a loan can never be paid out twice.
///
/// Do not add a `require_auth` here: requiring an admin or the borrower would
/// let a single absent party block an outcome the DAO already voted for. The
/// `non_member_can_disburse_approved_loan_without_auth` test pins this.
pub fn disburse_approved_loan(env: &Env, proposal_id: u32) -> Result<(), Error> {
    util::require_initialized(env)?;
    util::require_not_paused(env)?;
    let proposal = storage::get_loan_proposal(env, proposal_id).ok_or(Error::ProposalNotFound)?;
    if proposal.status != ProposalStatus::ApprovedPendingDisbursement {
        return Err(Error::NotInVotingPhase);
    }
    approve_and_disburse(env, &proposal)?;
    let mut proposal = proposal;
    proposal.status = ProposalStatus::Approved;
    storage::set_loan_proposal(env, &proposal);
    Ok(())
}

#[allow(deprecated)]
fn approve_and_disburse(env: &Env, proposal: &LoanProposal) -> Result<(), Error> {
    let asset = storage::get_loan_asset(env, proposal.id)?;
    let treasury = util::treasury_balance_in_asset(env, &asset);
    if treasury < proposal.amount {
        return Err(Error::InsufficientTreasury);
    }

    // Treasury exposure can change after approval. Re-apply the policy cap at
    // disbursement so an approved proposal cannot exceed the current ratio.
    let policy = storage::get_policy(env)?;
    let max_loan = treasury
        .checked_mul(policy.max_loan_to_treasury_ratio as i128)
        .ok_or(Error::ExceedsTreasuryRatio)?
        / crate::types::BASIS_POINTS;
    if proposal.amount > max_loan {
        return Err(Error::ExceedsTreasuryRatio);
    }

    // Issue 61: re-quote at disbursement so rate reflects current treasury balance
    let terms = quote_terms(env, proposal.amount, treasury)?;

    let now = env.ledger().timestamp();
    // Reuse the proposal's own id rather than a separate counter: a proposal
    // produces at most one loan, so this keeps loan_id == proposal_id as an
    // invariant instead of two sequences that silently diverge the moment
    // any proposal is rejected or expires without disbursing (the off-chain
    // indexer has no other way to correlate a loan back to its proposal).
    let id = proposal.id;
    let loan = Loan {
        id,
        borrower: proposal.borrower.clone(),
        principal: proposal.amount,
        interest_rate: terms.interest_rate,
        total_repayment: terms.total_repayment,
        start_time: now,
        due_time: now + terms.duration,
        status: LoanStatus::Active,
        amount_repaid: 0,
        default_grace_period: policy.default_grace_period,
        default_penalty_bps: policy.default_penalty_bps,
    };
    storage::set_loan(env, &loan);
    storage::adjust_outstanding_principal(env, &asset, principal_outstanding(&loan));
    storage::increment_active_loans_in_asset(env, &asset);

    let mut borrower = storage::get_member(env, &proposal.borrower).ok_or(Error::NotMember)?;
    borrower.has_active_loan = true;
    borrower.last_loan_time = now;
    borrower.total_loans += 1;
    borrower.active_loans += 1;
    storage::set_member(env, &borrower);

    util::token_client_for(env, &asset).transfer(
        &util::contract_address(env),
        &proposal.borrower,
        &proposal.amount,
    );

    env.events().publish(
        (crate::events::LOAN_APPR,),
        (
            id,
            proposal.borrower.clone(),
            proposal.amount,
            loan.due_time,
            asset,
        ),
    );
    Ok(())
}

/// Repays a loan's entire remaining balance in one transaction. A thin
/// wrapper over the same path [`repay_loan_partial`] uses, so existing
/// integrations built against this zero-amount-argument entrypoint keep
/// working unchanged — see [`repay_loan_partial`] for why a breaking ABI
/// change to this entrypoint was avoided instead.
pub fn repay_loan(env: &Env, borrower: Address, loan_id: u32) -> Result<(), Error> {
    repay_loan_internal(env, borrower, loan_id, None)
}

/// [`repay_loan`] with the caller naming the asset they repay in, so a client
/// can't silently pay in the wrong token: `asset` must be the loan's own asset.
pub fn repay_loan_in_asset(
    env: &Env,
    borrower: Address,
    loan_id: u32,
    asset: Address,
) -> Result<(), Error> {
    util::require_initialized(env)?;
    if storage::get_loan(env, loan_id).is_none() {
        return Err(Error::LoanNotFound);
    }
    if storage::get_loan_asset(env, loan_id)? != asset {
        return Err(Error::AssetMismatch);
    }
    repay_loan_internal(env, borrower, loan_id, None)
}

/// Repays up to `amount` of a loan's outstanding balance. `amount` must be
/// positive and may not exceed what's currently outstanding.
///
/// Payments apply to accrued interest first, then principal: whatever
/// portion of `amount` falls within the loan's still-unpaid interest is
/// distributed to active members as yield immediately, exactly as a full
/// repayment always has. The remainder (principal) needs no separate
/// bookkeeping — it's already sitting in the contract's token balance the
/// moment it's transferred in, so it's counted in the treasury right away.
///
/// The loan only flips to `Repaid` (and `has_active_loan` only clears) once
/// the outstanding balance reaches exactly zero.
///
/// This is a new entrypoint rather than an added parameter on `repay_loan`:
/// the ABI isn't upgradeable once deployed, and a new entrypoint leaves
/// every existing caller of `repay_loan(borrower, loan_id)` — including
/// `ourdao-backend` and any already-deployed clients — untouched, at the
/// cost of two entrypoints sharing one code path instead of one.
pub fn repay_loan_partial(
    env: &Env,
    borrower: Address,
    loan_id: u32,
    amount: i128,
) -> Result<(), Error> {
    repay_loan_internal(env, borrower, loan_id, Some(amount))
}

// Not wired to an entrypoint; kept unchanged from the pre-merge version.
#[allow(dead_code, deprecated)]
pub fn repay_partial(env: &Env, borrower: Address, amount: i128) -> Result<(), Error> {
    let next_id = storage::next_id(env, storage::DataKey::NextProposalId);
    let mut loan_id_opt = None;
    for i in 1..next_id {
        if let Some(loan) = storage::get_loan(env, i) {
            if loan.borrower == borrower && loan.status == LoanStatus::Active {
                loan_id_opt = Some(i);
                break;
            }
        }
    }
    let loan_id = loan_id_opt.ok_or(Error::LoanNotFound)?;
    repay_loan_internal(env, borrower.clone(), loan_id, Some(amount))?;
    env.events().publish(
        (soroban_sdk::String::from_str(env, "loan_partial_repaid"),),
        (borrower, amount),
    );
    Ok(())
}

#[allow(deprecated)]
fn repay_loan_internal(
    env: &Env,
    borrower: Address,
    loan_id: u32,
    amount: Option<i128>,
) -> Result<(), Error> {
    util::require_initialized(env)?;
    util::require_not_paused(env)?;
    borrower.require_auth();

    let mut loan = storage::get_loan(env, loan_id).ok_or(Error::LoanNotFound)?;
    if loan.borrower != borrower {
        return Err(Error::NotBorrower);
    }
    if loan.status != LoanStatus::Active {
        return Err(Error::LoanNotActive);
    }
    let principal_before = principal_outstanding(&loan);

    let mut outstanding = loan
        .total_repayment
        .checked_sub(loan.amount_repaid)
        .ok_or(Error::ArithmeticOverflow)?;

    let now = env.ledger().timestamp();
    if now > loan.due_time {
        let policy = storage::get_policy(env)?;
        // Overdue penalty (member debt): ceil-division rounds up to favour the treasury.
        let penalty = if outstanding > 0 && policy.default_penalty_bps > 0 {
            outstanding
                .checked_mul(policy.default_penalty_bps as i128)
                .and_then(|v| v.checked_add(BASIS_POINTS - ONE))
                .and_then(|v| v.checked_div(crate::types::BASIS_POINTS))
                .ok_or(Error::ArithmeticOverflow)?
        } else {
            0
        };
        outstanding = outstanding
            .checked_add(penalty)
            .ok_or(Error::ArithmeticOverflow)?;
        loan.total_repayment = loan
            .total_repayment
            .checked_add(penalty)
            .ok_or(Error::ArithmeticOverflow)?;
        loan.principal = loan
            .principal
            .checked_add(penalty)
            .ok_or(Error::ArithmeticOverflow)?; // Keep penalty in treasury, don't distribute as interest
    }

    let amount = amount.unwrap_or(outstanding);
    if amount <= 0 || amount > outstanding {
        return Err(Error::InvalidAmount);
    }

    let asset = storage::get_loan_asset(env, loan_id)?;

    // Interest-first split: how much of the loan's total interest was
    // already covered before this payment vs. after it. The difference is
    // the interest component of *this* payment; everything else is principal.
    let interest_total = loan
        .total_repayment
        .checked_sub(loan.principal)
        .ok_or(Error::ArithmeticOverflow)?;
    let interest_before = loan.amount_repaid.min(interest_total);
    loan.amount_repaid = loan
        .amount_repaid
        .checked_add(amount)
        .ok_or(Error::ArithmeticOverflow)?;
    let interest_after = loan.amount_repaid.min(interest_total);
    let interest_component = interest_after - interest_before;

    let remaining = loan
        .total_repayment
        .checked_sub(loan.amount_repaid)
        .ok_or(Error::ArithmeticOverflow)?;
    if remaining == 0 {
        loan.status = LoanStatus::Repaid;
        storage::decrement_active_loans_in_asset(env, &asset);
        if let Some(mut member) = storage::get_member(env, &borrower) {
            member.has_active_loan = false;
            member.repaid_loans += 1;
            member.active_loans = member.active_loans.saturating_sub(1);
            storage::set_member(env, &member);
        }
    }
    storage::set_loan(env, &loan);
    // Net of any overdue penalty booked as principal above.
    let principal_delta = principal_outstanding(&loan)
        .checked_sub(principal_before)
        .ok_or(Error::ArithmeticOverflow)?;
    storage::adjust_outstanding_principal(env, &asset, principal_delta);

    distribute_interest(env, interest_component, &asset)?;
    // External token interaction last; failed transfers revert all accounting effects.
    // Measure the actual amount received so fee-on-transfer/non-conforming
    // assets cannot make accounting claim more repayment than the treasury got.
    let contract = util::contract_address(env);
    let tk = util::token_client_for(env, &asset);
    let balance_before = tk.balance(&contract);
    tk.transfer(&borrower, &contract, &amount);
    let balance_after = tk.balance(&contract);
    let received = balance_after
        .checked_sub(balance_before)
        .ok_or(Error::TransferAmountMismatch)?;
    if received != amount {
        return Err(Error::TransferAmountMismatch);
    }

    env.events().publish(
        (crate::events::LOAN_RPY,),
        (loan_id, borrower, remaining, asset),
    );
    Ok(())
}

/// Permissionless keeper call: persists the expired/rejected transition for a
/// loan proposal whose voting window has passed without reaching quorum.
/// Succeeds exactly once per proposal — subsequent calls are a no-op (no
/// double event).
#[allow(deprecated)]
pub fn expire_loan_proposal(env: &Env, proposal_id: u32) -> Result<(), Error> {
    util::require_initialized(env)?;
    util::require_not_paused(env)?;
    let mut proposal =
        storage::get_loan_proposal(env, proposal_id).ok_or(Error::ProposalNotFound)?;
    proposal = refresh_phase(env, proposal);
    if proposal.phase != ProposalPhase::Expired {
        return Err(Error::ProposalNotExpired);
    }
    // Check if already persisted (no-op for repeat calls).
    // Re-read from storage to compare: if already Expired, skip.
    if let Some(original) = storage::get_loan_proposal(env, proposal_id) {
        if original.phase == ProposalPhase::Expired {
            return Ok(());
        }
    }
    storage::set_loan_proposal(env, &proposal);
    storage::extend_instance(env);

    env.events()
        .publish((crate::events::LOAN_EXP,), (proposal_id, proposal.borrower));
    Ok(())
}

/// Marks an overdue loan as defaulted. Permissionless and callable by anyone
/// once `due_time + loan.default_grace_period` has passed — this is an
/// objective, time-based state transition (a keeper call), not an admin
/// action, so there's nothing to authorize.
///
/// Consequence: the borrower's `contribution` (their pro-rata claim on the
/// treasury via `calculate_exit_share`) is slashed by `loan.default_penalty_bps`,
/// and `has_active_loan` is cleared. Clearing the flag is deliberate: it lets
/// a defaulted borrower still exit the DAO with their reduced share rather
/// than being trapped (exit is blocked while `has_active_loan` is true), and
/// lets them request a new loan again after the normal cooldown. Like
/// `Repaid`, `Defaulted` is terminal — a defaulted loan can't later be repaid.
///
/// Both default terms come from the loan itself, snapshotted from the policy
/// at disbursement, never from the current policy (#70): a borrower who took a
/// loan under a 5% penalty can't be slashed at 50% by a later admin change.
/// There is no fallback to the live policy because there is nothing to fall
/// back for — the contract has no in-place upgrade path, so every `Loan` this
/// code can read was written by this code. See `docs/MIGRATION.md` for loans
/// carried over from an older deployment.
#[allow(deprecated)]
pub fn mark_loan_defaulted(env: &Env, loan_id: u32) -> Result<(), Error> {
    util::require_initialized(env)?;
    util::require_not_paused(env)?;
    let mut loan = storage::get_loan(env, loan_id).ok_or(Error::LoanNotFound)?;
    if loan.status != LoanStatus::Active {
        return Err(Error::LoanNotActive);
    }

    let now = env.ledger().timestamp();
    if now < loan.due_time + loan.default_grace_period {
        return Err(Error::LoanNotOverdue);
    }

    // Whatever principal is still owed is written off, not repaid.
    let written_off = principal_outstanding(&loan);
    loan.status = LoanStatus::Defaulted;
    storage::set_loan(env, &loan);
    let asset = storage::get_loan_asset(env, loan_id)?;
    storage::decrement_active_loans_in_asset(env, &asset);
    storage::adjust_outstanding_principal(env, &asset, -written_off);

    let mut penalty: i128 = 0;
    if let Some(mut member) = storage::get_member(env, &loan.borrower) {
        // Default slashing penalty (member debt to treasury): ceil-division rounds up to favour the treasury.
        penalty = if member.contribution > 0 && loan.default_penalty_bps > 0 {
            member
                .contribution
                .checked_mul(loan.default_penalty_bps as i128)
                .and_then(|v| v.checked_add(BASIS_POINTS - ONE))
                .and_then(|v| v.checked_div(BASIS_POINTS))
                .unwrap_or(member.contribution)
                .min(member.contribution)
        } else {
            0
        };
        member.contribution -= penalty;
        member.has_active_loan = false;
        // Defaulted loans are terminal but were never repaid, so only the
        // outstanding count changes — `repaid_loans` deliberately stays put.
        member.active_loans = member.active_loans.saturating_sub(1);
        storage::set_member(env, &member);
        if penalty > 0 {
            storage::set_active_contributions(
                env,
                storage::get_active_contributions(env) - penalty,
            );
        }
        // Yield is split equally (see `distribute_interest`), so a defaulter
        // keeps earning from here on like any active member, but forfeits
        // whatever yield they had accrued and not yet claimed at the moment
        // of default. Advancing the snapshots leaves the amounts in their
        // respective treasuries.
        for asset in storage::get_yield_assets(env).iter() {
            let acc = storage::get_yield_accumulator(env, &asset);
            storage::set_yield_snapshot(env, &loan.borrower, &asset, acc);
        }
    }
    storage::extend_instance(env);

    env.events().publish(
        (crate::events::LOAN_DFLT,),
        (loan_id, loan.borrower.clone(), penalty),
    );
    Ok(())
}

/// Principal `loan` still owes the treasury, as tracked per asset for
/// `get_treasury_accounting` (#288): zero unless the loan is `Active`.
/// Payments cover interest first (see [`repay_loan_partial`]), so only what
/// has been paid beyond the loan's interest counts as principal repaid. An
/// overdue penalty is booked as principal, so it is included until paid.
pub(crate) fn principal_outstanding(loan: &Loan) -> i128 {
    if loan.status != LoanStatus::Active {
        return 0;
    }
    let interest_total = loan.total_repayment - loan.principal;
    let principal_repaid = loan.amount_repaid - loan.amount_repaid.min(interest_total);
    loan.principal - principal_repaid
}

/// Splits repaid interest equally across active members as claimable yield.
///
/// Yield is deliberately one-member-one-share while the exit share is
/// proportional to contribution: yield rewards participation, the exit share
/// returns capital. A defaulted borrower forfeits their unclaimed yield at the
/// moment of default (see `mark_loan_defaulted`).
///
/// Any indivisible remainder is retained by the treasury.
///
/// Uses a pull-based accumulator: instead of iterating every member and
/// bumping their `PendingYield` (O(n)), we increment the given asset's
/// accumulator by `interest / active_members`. Each member stores a snapshot
/// per asset at their last interaction (join, claim, or exit).
///
/// `pub(crate)` (rather than private) solely so the property tests in
/// `tests/properties.rs` can drive it directly with arbitrary `interest` values instead
/// of only the ones reachable through a real loan's computed interest.
///
/// `asset` is the asset the interest was paid in. It is carried on the
/// `interest` event so indexers can attribute it to the right treasury.
#[allow(deprecated)]
pub(crate) fn distribute_interest(env: &Env, interest: i128, asset: &Address) -> Result<(), Error> {
    let active = storage::get_active_members(env) as i128;
    if interest <= 0 || active == 0 {
        return Ok(());
    }

    storage::record_yield_asset(env, asset);

    // #60 — Carry the sub-divisible remainder forward instead of silently discarding.
    let total_interest = interest
        .checked_add(storage::get_yield_remainder(env, asset))
        .ok_or(Error::ArithmeticOverflow)?;
    let per_member = total_interest / active;
    let remainder = total_interest % active;

    storage::set_yield_remainder(env, asset, remainder);

    if per_member > 0 {
        let current = storage::get_yield_accumulator(env, asset);
        let updated = current
            .checked_add(per_member)
            .ok_or(Error::ArithmeticOverflow)?;
        storage::set_yield_accumulator(env, asset, updated);
    }

    // Unconditionally publish the event so the indexer sees the interest paid
    env.events().publish(
        (crate::events::INTEREST,),
        (interest, active, asset.clone()),
    );
    Ok(())
}
