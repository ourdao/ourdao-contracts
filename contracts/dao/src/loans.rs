use soroban_sdk::{symbol_short, Address, Env};

use crate::error::Error;
use crate::storage;
use crate::types::{
    Loan, LoanProposal, LoanStatus, LoanTerms, MemberStatus, ProposalPhase, ProposalStatus,
    BASIS_POINTS, PROPOSAL_EDITING_PERIOD, VOTING_PERIOD,
};
use crate::util;

/// Quote the terms a loan of `amount` would carry right now. Interest scales
/// linearly with the loan's size relative to the treasury, clamped to policy.
pub fn calculate_loan_terms(env: &Env, amount: i128) -> LoanTerms {
    let policy = storage::get_policy(env);
    let treasury = util::treasury_balance(env);

    let loan_ratio = if treasury > 0 {
        (amount * BASIS_POINTS / treasury).min(BASIS_POINTS)
    } else {
        BASIS_POINTS
    };
    let spread = (policy.max_interest_rate - policy.min_interest_rate) as i128;
    let mut rate = policy.min_interest_rate as i128 + (loan_ratio * spread / BASIS_POINTS);
    if rate > policy.max_interest_rate as i128 {
        rate = policy.max_interest_rate as i128;
    }
    let total_repayment = amount + (amount * rate / BASIS_POINTS);
    LoanTerms {
        interest_rate: rate as u32,
        total_repayment,
        duration: policy.max_loan_duration,
    }
}

pub fn is_eligible_for_loan(env: &Env, member: &Address) -> bool {
    let record = match storage::get_member(env, member) {
        Some(m) if m.status == MemberStatus::ActiveMember => m,
        _ => return false,
    };
    if record.has_active_loan {
        return false;
    }
    let policy = storage::get_policy(env);
    let now = env.ledger().timestamp();
    if now.saturating_sub(record.join_ledger) < policy.min_membership_duration {
        return false;
    }
    if record.last_loan_time != 0
        && now.saturating_sub(record.last_loan_time) < policy.cooldown_period
    {
        return false;
    }
    true
}

pub fn request_loan(env: &Env, borrower: Address, amount: i128) -> Result<u32, Error> {
    util::require_initialized(env)?;
    util::require_not_paused(env)?;
    util::require_active_member(env, &borrower)?;

    if amount <= 0 {
        return Err(Error::InvalidAmount);
    }
    if !is_eligible_for_loan(env, &borrower) {
        return Err(Error::NotEligibleForLoan);
    }

    let policy = storage::get_policy(env);
    let treasury = util::treasury_balance(env);
    let max_loan = treasury * policy.max_loan_to_treasury_ratio as i128 / BASIS_POINTS;
    if amount > max_loan {
        return Err(Error::ExceedsTreasuryRatio);
    }

    let terms = calculate_loan_terms(env, amount);
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
        editing_period_end: now + PROPOSAL_EDITING_PERIOD,
        phase: ProposalPhase::Editing,
        status: ProposalStatus::Pending,
        for_votes: 0,
        against_votes: 0,
    };
    storage::set_loan_proposal(env, &proposal);
    storage::extend_instance(env);

    env.events().publish(
        (symbol_short!("loan_req"),),
        (id, borrower, amount, terms.total_repayment),
    );
    Ok(id)
}

pub fn edit_loan_proposal(
    env: &Env,
    borrower: Address,
    proposal_id: u32,
    new_amount: i128,
) -> Result<(), Error> {
    util::require_active_member(env, &borrower)?;
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

    let terms = calculate_loan_terms(env, new_amount);
    proposal.amount = new_amount;
    proposal.interest_rate = terms.interest_rate;
    proposal.duration = terms.duration;
    proposal.total_repayment = terms.total_repayment;
    storage::set_loan_proposal(env, &proposal);

    env.events().publish(
        (symbol_short!("loan_edit"),),
        (proposal_id, borrower, new_amount, terms.total_repayment),
    );
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
        && now > proposal.editing_period_end + VOTING_PERIOD
        && proposal.status == ProposalStatus::Pending
    {
        proposal.phase = ProposalPhase::Expired;
        proposal.status = ProposalStatus::Rejected;
    }
    proposal
}

pub fn vote_on_loan_proposal(
    env: &Env,
    voter: Address,
    proposal_id: u32,
    support: bool,
) -> Result<(), Error> {
    util::require_initialized(env)?;
    util::require_not_paused(env)?;
    util::require_active_member(env, &voter)?;

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
    if now > proposal.editing_period_end + VOTING_PERIOD {
        return Err(Error::VotingEnded);
    }
    if storage::has_loan_voted(env, proposal_id, &voter) {
        return Err(Error::AlreadyVoted);
    }

    let weight = util::voting_weight(env, &voter);
    if support {
        proposal.for_votes += weight;
    } else {
        proposal.against_votes += weight;
    }
    storage::set_loan_voted(env, proposal_id, &voter);
    env.events()
        .publish((symbol_short!("loan_vote"),), (proposal_id, voter, support));

    let required = util::required_votes(
        storage::get_active_members(env),
        storage::get_threshold(env),
    );
    if proposal.for_votes >= required && proposal.status == ProposalStatus::Pending {
        proposal.status = ProposalStatus::Approved;
        proposal.phase = ProposalPhase::Executed;
        approve_and_disburse(env, &proposal)?;
    }
    storage::set_loan_proposal(env, &proposal);
    Ok(())
}

fn approve_and_disburse(env: &Env, proposal: &LoanProposal) -> Result<(), Error> {
    if util::treasury_balance(env) < proposal.amount {
        return Err(Error::InsufficientTreasury);
    }
    let now = env.ledger().timestamp();
    let id = storage::next_id(env, storage::DataKey::NextLoanId);
    let loan = Loan {
        id,
        borrower: proposal.borrower.clone(),
        principal: proposal.amount,
        interest_rate: proposal.interest_rate,
        total_repayment: proposal.total_repayment,
        start_time: now,
        due_time: now + proposal.duration,
        status: LoanStatus::Active,
        amount_repaid: 0,
    };
    storage::set_loan(env, &loan);

    let mut borrower = storage::get_member(env, &proposal.borrower).ok_or(Error::NotMember)?;
    borrower.has_active_loan = true;
    borrower.last_loan_time = now;
    storage::set_member(env, &borrower);

    util::token_client(env).transfer(
        &util::contract_address(env),
        &proposal.borrower,
        &proposal.amount,
    );

    env.events().publish(
        (symbol_short!("loan_appr"),),
        (id, proposal.borrower.clone(), proposal.amount),
    );
    Ok(())
}

pub fn repay_loan(env: &Env, borrower: Address, loan_id: u32) -> Result<(), Error> {
    util::require_initialized(env)?;
    borrower.require_auth();

    let mut loan = storage::get_loan(env, loan_id).ok_or(Error::LoanNotFound)?;
    if loan.borrower != borrower {
        return Err(Error::NotBorrower);
    }
    if loan.status != LoanStatus::Active {
        return Err(Error::LoanNotActive);
    }

    let outstanding = loan.total_repayment - loan.amount_repaid;
    util::token_client(env).transfer(&borrower, &util::contract_address(env), &outstanding);

    loan.amount_repaid = loan.total_repayment;
    loan.status = LoanStatus::Repaid;
    storage::set_loan(env, &loan);

    if let Some(mut member) = storage::get_member(env, &borrower) {
        member.has_active_loan = false;
        storage::set_member(env, &member);
    }

    let interest = loan.total_repayment - loan.principal;
    distribute_interest(env, interest);

    env.events().publish(
        (symbol_short!("loan_rpy"),),
        (loan_id, borrower, outstanding),
    );
    Ok(())
}

/// Splits repaid interest equally across active members as claimable yield.
/// Any indivisible remainder is retained by the treasury.
fn distribute_interest(env: &Env, interest: i128) {
    let active = storage::get_active_members(env) as i128;
    if interest <= 0 || active == 0 {
        return;
    }
    let per_member = interest / active;
    if per_member == 0 {
        return;
    }
    let members = storage::get_members(env);
    for addr in members.iter() {
        if let Some(m) = storage::get_member(env, &addr) {
            if m.status == MemberStatus::ActiveMember {
                let current = storage::get_pending_yield(env, &addr);
                storage::set_pending_yield(env, &addr, current + per_member);
            }
        }
    }
    env.events()
        .publish((symbol_short!("interest"),), (interest, active));
}
