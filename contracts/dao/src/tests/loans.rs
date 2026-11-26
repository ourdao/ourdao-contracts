extern crate std;

use soroban_sdk::testutils::{Address as _, Events as _, Ledger as _};
use soroban_sdk::xdr::{ContractEventBody, ScAddress, ScVal};
use soroban_sdk::{Address, Env, String};

use super::common::*;
use crate::admin::TIMELOCK_DURATION;
use crate::storage::ProposalKind;
use crate::types::{LoanStatus, ProposalPhase, ProposalStatus, ProposalVote};
use crate::util::BASE_VOTE_WEIGHT;
use crate::Error;

#[test]
fn full_loan_lifecycle() {
    proposal.votes_cast = proposal.votes_cast.saturating_add(1);
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    let terms = s.client.calculate_loan_terms(&1_000);
    assert!(terms.interest_rate >= 500 && terms.interest_rate <= 2_000);

    let pid = s.client.request_loan(&borrower, &1_000, &None);
    let prop = s.client.get_loan_proposal(&pid).unwrap();
    assert_eq!(prop.total_repayment, terms.total_repayment);

    // Cannot vote during the editing phase.
    let early = s.client.try_vote_on_loan_proposal(&v1, &pid, &true);
    assert_eq!(early, Err(Ok(Error::NotInVotingPhase)));

    advance(&s.env, EDITING + 1);

    let treasury_before = s.client.get_treasury_balance();
    let bal_before = s.token.balance(&borrower);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true); // 2/3 >= ceil(51%) => approved

    let prop = s.client.get_loan_proposal(&pid).unwrap();
    assert_eq!(prop.status, ProposalStatus::Approved);
    assert_eq!(s.token.balance(&borrower), bal_before + 1_000);
    assert_eq!(s.client.get_treasury_balance(), treasury_before - 1_000);

    let loan = s.client.get_loan(&0).unwrap();
    assert_eq!(loan.status, LoanStatus::Active);
    assert!(s.client.get_member(&borrower).unwrap().has_active_loan);

    // Repay and verify interest becomes claimable yield for active members.
    s.client.repay_loan(&borrower, &loan.id);
    let loan = s.client.get_loan(&0).unwrap();
    assert_eq!(loan.status, LoanStatus::Repaid);
    assert!(!s.client.get_member(&borrower).unwrap().has_active_loan);

    let interest = loan.total_repayment - loan.principal;
    let per = interest / 3;
    assert!(per > 0);
    assert_eq!(s.client.get_pending_yield(&v1), per);

    let claim_before = s.token.balance(&v1);
    let claimed = s.client.claim_rewards(&v1);
    assert_eq!(claimed, per);
    assert_eq!(s.token.balance(&v1), claim_before + per);
    assert_eq!(s.client.get_pending_yield(&v1), 0);
}

#[test]
fn loan_rejected_when_ineligible_active_loan() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);

    // Borrower now has an active loan; a second request must fail.
    let res = s.client.try_request_loan(&borrower, &200, &None);
    assert_eq!(res, Err(Ok(Error::HasActiveLoan)));
}

#[test]
fn loan_exceeds_treasury_ratio() {
    let s = setup(3); // treasury = 3000, max ratio 50% => max loan 1500
    let borrower = s.members.get(0).unwrap();
    let res = s.client.try_request_loan(&borrower, &2_000, &None);
    assert_eq!(res, Err(Ok(Error::ExceedsTreasuryRatio)));
}

#[test]
fn loan_default_before_due_rejected() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);

    // Loan is Active and not yet overdue.
    let res = s.client.try_mark_loan_defaulted(&0);
    assert_eq!(res, Err(Ok(Error::LoanNotOverdue)));

    // A loan that doesn't exist can't be defaulted either.
    let missing = s.client.try_mark_loan_defaulted(&99);
    assert_eq!(missing, Err(Ok(Error::LoanNotFound)));
}

#[test]
fn loan_default_applies_penalty_and_frees_borrower() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);
    assert!(s.client.get_member(&borrower).unwrap().has_active_loan);

    advance(&s.env, LOAN_DURATION + 1); // past due_time, grace period is 0

    let contribution_before = s.client.get_member(&borrower).unwrap().contribution;
    s.client.mark_loan_defaulted(&0);

    let loan = s.client.get_loan(&0).unwrap();
    assert_eq!(loan.status, LoanStatus::Defaulted);

    let member = s.client.get_member(&borrower).unwrap();
    assert!(!member.has_active_loan);
    let expected_penalty = contribution_before * 2_000 / 10_000; // 20% per policy()
    assert_eq!(member.contribution, contribution_before - expected_penalty);
}

#[test]
fn defaulted_loan_is_terminal() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);
    advance(&s.env, LOAN_DURATION + 1);
    s.client.mark_loan_defaulted(&0);

    // Can't default it twice.
    let again = s.client.try_mark_loan_defaulted(&0);
    assert_eq!(again, Err(Ok(Error::LoanNotActive)));

    // Can't repay a defaulted loan.
    let repay = s.client.try_repay_loan(&borrower, &0);
    assert_eq!(repay, Err(Ok(Error::LoanNotActive)));
}

#[test]
fn defaulted_borrower_can_exit() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);
    advance(&s.env, LOAN_DURATION + 1);

    // Before default, has_active_loan blocks exit.
    let blocked = s.client.try_exit_dao(&borrower);
    assert_eq!(blocked, Err(Ok(Error::HasActiveLoan)));

    s.client.mark_loan_defaulted(&0);

    // After default, has_active_loan is cleared and exit succeeds (with the
    // already-slashed, reduced share).
    s.client.exit_dao(&borrower);
    assert!(!s.client.is_member(&borrower));
}

#[test]
fn loan_id_matches_its_originating_proposal_id() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    // First proposal (id 0) never reaches approval — consumes a proposal id
    // without ever producing a loan, so a separate loan-id counter would lag
    // behind the proposal-id counter from here on.
    let pid0 = s.client.request_loan(&borrower, &200, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid0, &false);
    assert_eq!(
        s.client.get_loan_proposal(&pid0).unwrap().status,
        ProposalStatus::Pending
    );

    // Second proposal (id 1) is approved. Its loan must carry id 1 too, not
    // whatever a separate counter would have handed out (0).
    let pid1 = s.client.request_loan(&borrower, &300, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid1, &true);
    s.client.vote_on_loan_proposal(&v2, &pid1, &true);

    assert_eq!(pid1, 1);
    let loan = s.client.get_loan(&pid1).unwrap();
    assert_eq!(loan.id, pid1);
    assert_eq!(loan.borrower, borrower);
}

// ==================== issue #1: expire_loan_proposal ====================
#[test]
fn expired_proposal_shows_expired_phase_in_view() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let pid = s.client.request_loan(&borrower, &500, &None);

    // Before voting window expires, view returns the live phase.
    let prop = s.client.get_loan_proposal(&pid).unwrap();
    assert_eq!(prop.phase, ProposalPhase::Editing);

    advance(&s.env, EDITING + 1);
    let prop = s.client.get_loan_proposal(&pid).unwrap();
    assert_eq!(prop.phase, ProposalPhase::Voting);

    // After voting window passes without reaching quorum, view auto-expires.
    advance(&s.env, VOTING_PERIOD + 1);
    let prop = s.client.get_loan_proposal(&pid).unwrap();
    assert_eq!(prop.phase, ProposalPhase::Expired);
    assert_eq!(prop.status, ProposalStatus::Expired);
}

// #86: a timed-out proposal and an early-rejected one are told apart by state.
#[test]
fn timed_out_proposal_is_stored_expired_with_loan_exp() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + VOTING_PERIOD + 2);
    s.client.expire_loan_proposal(&pid);
    // Events only reflect the latest invocation, so check before any view call.
    assert!(emitted(&s.env, "loan_exp"));
    assert!(!emitted(&s.env, "loan_rej"));

    let prop = s.client.get_loan_proposal(&pid).unwrap();
    assert_eq!(prop.phase, ProposalPhase::Expired);
    assert_eq!(prop.status, ProposalStatus::Expired);
}

#[test]
fn early_rejected_proposal_is_stored_rejected_with_loan_rej() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();
    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &false);
    s.client.vote_on_loan_proposal(&v2, &pid, &false);
    s.client.vote_on_loan_proposal(&borrower, &pid, &false);
    assert!(emitted(&s.env, "loan_rej"));
    assert!(!emitted(&s.env, "loan_exp"));

    let prop = s.client.get_loan_proposal(&pid).unwrap();
    assert_eq!(prop.phase, ProposalPhase::Expired);
    assert_eq!(prop.status, ProposalStatus::Rejected);
}

// #89: a defaulter forfeits yield accrued but unclaimed at the moment of default.
#[test]
fn defaulter_forfeits_unclaimed_yield() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    // v1 borrows and repays, so every member accrues equal yield.
    let pid = s.client.request_loan(&v1, &1_000, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);
    s.client.vote_on_loan_proposal(&borrower, &pid, &true);
    s.client.repay_loan(&v1, &0);
    let accrued = s.client.get_pending_yield(&borrower);
    assert!(accrued > 0);
    assert_eq!(s.client.get_pending_yield(&v2), accrued);

    // The borrower takes a loan and defaults: unclaimed yield is forfeited,
    // everyone else's is untouched.
    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);
    advance(&s.env, LOAN_DURATION + 1);
    s.client.mark_loan_defaulted(&1);

    assert_eq!(s.client.get_pending_yield(&borrower), 0);
    assert_eq!(s.client.get_pending_yield(&v2), accrued);
}

#[test]
fn expire_before_deadline_rejected() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let pid = s.client.request_loan(&borrower, &500, &None);

    // Proposal is still in editing phase — not expired yet.
    let res = s.client.try_expire_loan_proposal(&pid);
    assert_eq!(res, Err(Ok(Error::ProposalNotExpired)));
}

#[test]
fn double_expire_is_noop() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let pid = s.client.request_loan(&borrower, &500, &None);

    advance(&s.env, EDITING + VOTING_PERIOD + 2);

    // First call persists the expired state.
    s.client.expire_loan_proposal(&pid);
    let prop = s.client.get_loan_proposal(&pid).unwrap();
    assert_eq!(prop.phase, ProposalPhase::Expired);

    // Second call is a no-op (already persisted).
    s.client.expire_loan_proposal(&pid);
    let prop = s.client.get_loan_proposal(&pid).unwrap();
    assert_eq!(prop.phase, ProposalPhase::Expired);
}

// ==================== issue #2: has_voted view ====================
#[test]
fn has_voted_loan_before_and_after() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);

    assert!(!s.client.has_voted(&ProposalKind::Loan, &pid, &v1));
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    assert!(s.client.has_voted(&ProposalKind::Loan, &pid, &v1));
}

// ==================== issue #5: partial loan repayment ====================
#[test]
fn partial_repayment_then_full() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    let pid = s.client.request_loan(&borrower, &1_000, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);

    let loan = s.client.get_loan(&pid).unwrap();
    let outstanding = loan.total_repayment - loan.amount_repaid;
    let half = outstanding / 2;
    assert!(half > 0);

    let bal_before = s.token.balance(&borrower);
    s.client.repay_loan_partial(&borrower, &pid, &half);

    let loan = s.client.get_loan(&pid).unwrap();
    assert_eq!(loan.status, LoanStatus::Active);
    assert_eq!(loan.amount_repaid, half);
    assert_eq!(s.token.balance(&borrower), bal_before - half);
    assert!(s.client.get_member(&borrower).unwrap().has_active_loan);

    // Interest-first: whatever portion of `half` falls inside the loan's
    // total interest is claimable right away, exactly as a full repayment.
    let interest_total = loan.total_repayment - loan.principal;
    let interest_component = half.min(interest_total);
    let per = interest_component / 3;
    assert_eq!(s.client.get_pending_yield(&v1), per);

    // Repaying the rest via the full-repayment entrypoint clears the loan.
    s.client.repay_loan(&borrower, &pid);
    let loan = s.client.get_loan(&pid).unwrap();
    assert_eq!(loan.status, LoanStatus::Repaid);
    assert_eq!(loan.amount_repaid, loan.total_repayment);
    assert!(!s.client.get_member(&borrower).unwrap().has_active_loan);
}

#[test]
fn partial_repayment_overpay_rejected() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);

    let loan = s.client.get_loan(&pid).unwrap();
    let outstanding = loan.total_repayment - loan.amount_repaid;
    let bal_before = s.token.balance(&borrower);

    let over = s
        .client
        .try_repay_loan_partial(&borrower, &pid, &(outstanding + 1));
    assert_eq!(over, Err(Ok(Error::InvalidAmount)));

    let zero = s.client.try_repay_loan_partial(&borrower, &pid, &0);
    assert_eq!(zero, Err(Ok(Error::InvalidAmount)));

    let negative = s.client.try_repay_loan_partial(&borrower, &pid, &-10);
    assert_eq!(negative, Err(Ok(Error::InvalidAmount)));

    // None of the rejected attempts moved any funds or touched the loan.
    assert_eq!(s.token.balance(&borrower), bal_before);
    assert_eq!(s.client.get_loan(&pid).unwrap().amount_repaid, 0);
}

#[test]
fn partial_repayments_sum_to_exact_total_marks_repaid() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    let pid = s.client.request_loan(&borrower, &1_000, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);

    let total = s.client.get_loan(&pid).unwrap().total_repayment;
    let a = total / 3;
    let b = total / 3;
    let c = total - a - b; // remainder, so a + b + c == total exactly

    s.client.repay_loan_partial(&borrower, &pid, &a);
    assert_eq!(s.client.get_loan(&pid).unwrap().status, LoanStatus::Active);
    s.client.repay_loan_partial(&borrower, &pid, &b);
    assert_eq!(s.client.get_loan(&pid).unwrap().status, LoanStatus::Active);
    s.client.repay_loan_partial(&borrower, &pid, &c);

    let loan = s.client.get_loan(&pid).unwrap();
    assert_eq!(loan.status, LoanStatus::Repaid);
    assert_eq!(loan.amount_repaid, total);
    assert!(!s.client.get_member(&borrower).unwrap().has_active_loan);
}

#[test]
fn partial_payment_overdue_loan_still_defaultable() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);

    let outstanding = s.client.get_loan(&pid).unwrap().total_repayment;
    let partial = outstanding / 4;
    assert!(partial > 0);
    s.client.repay_loan_partial(&borrower, &pid, &partial);

    advance(&s.env, LOAN_DURATION + 1); // past due_time, grace period is 0

    let contribution_before = s.client.get_member(&borrower).unwrap().contribution;
    s.client.mark_loan_defaulted(&pid);

    let loan = s.client.get_loan(&pid).unwrap();
    assert_eq!(loan.status, LoanStatus::Defaulted);
    let member = s.client.get_member(&borrower).unwrap();
    assert!(!member.has_active_loan);
    let expected_penalty = contribution_before * 2_000 / 10_000; // 20% per policy()
    assert_eq!(member.contribution, contribution_before - expected_penalty);

    // Defaulted loans are terminal — no further repayment, partial or full.
    let res = s.client.try_repay_loan_partial(&borrower, &pid, &1);
    assert_eq!(res, Err(Ok(Error::LoanNotActive)));
}

#[test]
fn exit_blocked_while_partial_balance_remains() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);

    let outstanding = s.client.get_loan(&pid).unwrap().total_repayment;
    let partial = outstanding / 2;
    assert!(partial > 0 && partial < outstanding);
    s.client.repay_loan_partial(&borrower, &pid, &partial);

    let blocked = s.client.try_exit_dao(&borrower);
    assert_eq!(blocked, Err(Ok(Error::HasActiveLoan)));

    s.client.repay_loan(&borrower, &pid);
    s.client.exit_dao(&borrower);
    assert!(!s.client.is_member(&borrower));
}

#[test]
fn approved_but_unfundable_loan_waits_then_disburses_after_refill() {
    // 4 members => treasury 4000, max loan 2000 (50%), 3 of 4 votes required.
    let s = setup(4);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();
    let v3 = s.members.get(3).unwrap();

    // Request passes the pre-vote ratio check against the full treasury...
    let pid = s.client.request_loan(&borrower, &1_500, &None);

    // ...then the treasury is drained by a passed withdrawal before the loan vote closes.
    let dest = Address::generate(&s.env);
    let reason = String::from_str(&s.env, "drain");
    let tid = s
        .client
        .propose_treasury_withdrawal(&v1, &3_000, &dest, &reason, &false);
    s.client.vote_on_treasury_proposal(&v1, &tid, &true);
    s.client.vote_on_treasury_proposal(&v2, &tid, &true);
    s.client.vote_on_treasury_proposal(&v3, &tid, &true);
    assert_eq!(s.client.get_treasury_balance(), 1_000);

    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);
    let borrower_bal = s.token.balance(&borrower);
    s.client.vote_on_loan_proposal(&v3, &pid, &true); // reaches the threshold; treasury (1000) < 1500
    assert!(emitted(&s.env, "loan_wait"));

    let prop = s.client.get_loan_proposal(&pid).unwrap();
    assert_eq!(prop.status, ProposalStatus::ApprovedPendingDisbursement);
    // No tokens moved, and no loan/active-loan state was created.
    assert_eq!(s.token.balance(&borrower), borrower_bal);
    assert_eq!(s.client.get_treasury_balance(), 1_000);
    assert!(s.client.get_loan(&pid).is_none());
    assert!(!s.client.get_member(&borrower).unwrap().has_active_loan);

    // Still unfundable until the treasury is refilled.
    assert_eq!(
        s.client.try_disburse_approved_loan(&pid),
        Err(Ok(Error::InsufficientTreasury))
    );

    // Refill, then the very same proposal can be disbursed.
    refill_treasury(&s);
    refill_treasury(&s);
    assert!(s.client.get_treasury_balance() >= 1_500);
    s.client.disburse_approved_loan(&pid);

    let prop = s.client.get_loan_proposal(&pid).unwrap();
    assert_eq!(prop.status, ProposalStatus::Approved);
    assert_eq!(s.token.balance(&borrower), borrower_bal + 1_500);
    assert!(s.client.get_member(&borrower).unwrap().has_active_loan);
    assert_eq!(s.client.get_loan(&pid).unwrap().status, LoanStatus::Active);
}

#[test]
fn disbursement_rechecks_max_loan_to_treasury_ratio() {
    let s = setup(4); // treasury = 4000, ratio = 50%
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();
    let v3 = s.members.get(3).unwrap();

    // The request is valid while the treasury is 4000 (1500 <= 2000).
    let pid = s.client.request_loan(&borrower, &1_500, &None);

    // Leave enough cash to pay the loan, but below the 50% exposure cap.
    let destination = Address::generate(&s.env);
    let reason = String::from_str(&s.env, "shrink treasury");
    let withdrawal =
        s.client
            .propose_treasury_withdrawal(&v1, &2_200, &destination, &reason, &false);
    s.client.vote_on_treasury_proposal(&v1, &withdrawal, &true);
    s.client.vote_on_treasury_proposal(&v2, &withdrawal, &true);
    s.client.vote_on_treasury_proposal(&v3, &withdrawal, &true);
    assert_eq!(s.client.get_treasury_balance(), 1_800);

    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);
    s.client.vote_on_loan_proposal(&v3, &pid, &true);

    assert_eq!(
        s.client.try_disburse_approved_loan(&pid),
        Err(Ok(Error::ExceedsTreasuryRatio))
    );
    assert!(s.client.get_loan(&pid).is_none());
    assert_eq!(s.client.get_treasury_balance(), 1_800);
}

#[test]
fn loan_proposal_uses_dynamic_quorum_threshold() {
    // 4 members. Threshold set to 5100 (needs 3 votes normally).
    // If policy has quorum_bps = 2500 (needs ceil(4 * 25%) = 1 vote), 1 vote is enough to approve!
    let s = setup(4);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();

    let mut low_quorum_policy = policy();
    low_quorum_policy.quorum_bps = 2_500; // 25% => 1 vote required for 4 members

    // Propose and execute policy update after timelock
    s.client.propose_policy_update(&s.admin, &low_quorum_policy);
    advance(&s.env, TIMELOCK_DURATION + 1);
    s.client.execute_policy_update(&s.admin);
    assert_eq!(s.client.get_loan_policy().quorum_bps, 2_500);

    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);

    // Single vote is enough with 25% quorum!
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    let prop = s.client.get_loan_proposal(&pid).unwrap();
    assert_eq!(prop.status, ProposalStatus::Approved);
}

#[test]
fn loan_proposal_quorum_higher_threshold_requires_more_votes() {
    // 5 members. Threshold set to 8000 (needs ceil(5 * 80%) = 4 votes).
    let s = setup(5);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();
    let v3 = s.members.get(3).unwrap();
    let v4 = s.members.get(4).unwrap();

    let mut high_quorum_policy = policy();
    high_quorum_policy.quorum_bps = 8_000; // 80% => 4 votes required

    s.client
        .propose_policy_update(&s.admin, &high_quorum_policy);
    advance(&s.env, TIMELOCK_DURATION + 1);
    s.client.execute_policy_update(&s.admin);

    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);

    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);
    assert_eq!(
        s.client.get_loan_proposal(&pid).unwrap().status,
        ProposalStatus::Pending
    );

    s.client.vote_on_loan_proposal(&v3, &pid, &true);
    assert_eq!(
        s.client.get_loan_proposal(&pid).unwrap().status,
        ProposalStatus::Pending
    );

    // Borrower cannot vote on their own loan proposal
    let err = s.client.try_vote_on_loan_proposal(&borrower, &pid, &true);
    assert_eq!(err, Err(Ok(Error::BorrowerCannotVote)));

    // 4th non-borrower voter votes yes => 4/4 required approved!
    s.client.vote_on_loan_proposal(&v4, &pid, &true);
    assert_eq!(
        s.client.get_loan_proposal(&pid).unwrap().status,
        ProposalStatus::Approved
    );
}

#[test]
fn borrower_cannot_vote_on_own_loan_proposal() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);
    let res = s.client.try_vote_on_loan_proposal(&borrower, &pid, &true);
    assert_eq!(res, Err(Ok(Error::BorrowerCannotVote)));
}

#[test]
fn loan_proposal_backwards_compatible_with_zero_quorum_bps() {
    // With quorum_bps == 0, it falls back to global consensus_threshold
    let s = setup(3); // 51% of 3 members is 2 votes
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    assert_eq!(s.client.get_loan_policy().quorum_bps, 0);

    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);

    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    assert_eq!(
        s.client.get_loan_proposal(&pid).unwrap().status,
        ProposalStatus::Pending
    );

    s.client.vote_on_loan_proposal(&v2, &pid, &true);
    assert_eq!(
        s.client.get_loan_proposal(&pid).unwrap().status,
        ProposalStatus::Approved
    );
}

// Issue #194: Custom metadata CID attachment to loan proposals
#[test]
fn proposal_creation_with_and_without_cid() {
    let s = setup(2);
    let borrower = s.members.get(0).unwrap();

    // 1. Without CID
    let pid_none = s.client.request_loan(&borrower, &500, &None);
    let prop_none = s.client.get_loan_proposal(&pid_none).unwrap();
    assert_eq!(prop_none.metadata_cid, None);

    // 2. With valid CID (IPFS CIDv0: 46 chars)
    let cid_str = String::from_str(&s.env, "QmXoypizjW3WknFiJnKLwHCnL72vedxjQkDDP1mXWo6uco");
    let pid_some = s
        .client
        .request_loan(&borrower, &500, &Some(cid_str.clone()));
    let prop_some = s.client.get_loan_proposal(&pid_some).unwrap();
    assert_eq!(prop_some.metadata_cid, Some(cid_str));

    // 3. With valid CIDv1 (59 chars)
    let cid_v1 = String::from_str(
        &s.env,
        "bafybeicg2abbmanlpdgahgah744vyqeifqgndq7x2pzg7kmd3p7w4h2bfe",
    );
    let pid_v1 = s
        .client
        .request_loan(&borrower, &500, &Some(cid_v1.clone()));
    let prop_v1 = s.client.get_loan_proposal(&pid_v1).unwrap();
    assert_eq!(prop_v1.metadata_cid, Some(cid_v1));
}

#[test]
fn proposal_creation_rejects_invalid_cid() {
    let s = setup(2);
    let borrower = s.members.get(0).unwrap();

    // Empty CID string
    let empty_cid = String::from_str(&s.env, "");
    let err_empty = s.client.try_request_loan(&borrower, &500, &Some(empty_cid));
    assert_eq!(err_empty, Err(Ok(Error::InvalidMetadataCid)));

    // Too long CID (> 64 chars)
    let long_cid = String::from_str(
        &s.env,
        "QmXoypizjW3WknFiJnKLwHCnL72vedxjQkDDP1mXWo6ucoExtraLongContentHashExceeding64Characters",
    );
    let err_long = s.client.try_request_loan(&borrower, &500, &Some(long_cid));
    assert_eq!(err_long, Err(Ok(Error::DocumentTooLarge)));
}

#[test]
fn edit_loan_proposal_emits_loan_edit_event() {
    let s = setup(2);
    let borrower = s.members.get(0).unwrap();
    let pid = s.client.request_loan(&borrower, &500, &None);
    // Drain the loan_req event so only edit-related events remain.
    let _ = s.env.events().all();
    advance(&s.env, 10);
    let edit_ts = s.env.ledger().timestamp();
    s.client.edit_loan_proposal(&borrower, &pid, &600, &None);
    // Capture events before any further contract call (all() drains).
    let events = s.env.events().all();
    let mut found = false;
    for e in events.events().iter() {
        let ContractEventBody::V0(body) = &e.body;
        let is_topic = matches!(body.topics.first(), Some(ScVal::Symbol(sym)) if sym.0.to_utf8_string_lossy() == "loan_edit");
        if !is_topic {
            continue;
        }
        let data_str = std::format!("{:?}", body.data);
        if data_str.contains("600") {
            found = true;
        }
    }
    assert!(found, "loan_edit event with new_amount 600 not found");
    // Verify storage separately, after event capture.
    let after = s.client.get_loan_proposal(&pid).unwrap();
    assert_eq!(after.amount, 600);
    assert_eq!(after.last_edited_at, Some(edit_ts));
    let expected = s.client.calculate_loan_terms(&600);
    assert_eq!(after.total_repayment, expected.total_repayment);
}

#[test]
fn edit_loan_proposal_updates_and_clears_metadata_cid() {
    let s = setup(2);
    let borrower = s.members.get(0).unwrap();
    let initial = String::from_str(&s.env, "cid-before");
    let replacement = String::from_str(&s.env, "cid-after");
    let pid = s.client.request_loan(&borrower, &500, &Some(initial));

    s.client
        .edit_loan_proposal(&borrower, &pid, &600, &Some(replacement.clone()));
    assert_eq!(
        s.client.get_loan_proposal(&pid).unwrap().metadata_cid,
        Some(replacement)
    );

    s.client.edit_loan_proposal(&borrower, &pid, &700, &None);
    assert_eq!(s.client.get_loan_proposal(&pid).unwrap().metadata_cid, None);
}

// ===========================================================================
// Issue #173: Off-by-one in voting period deadline comparison
// ===========================================================================

#[test]
fn voting_closed_exactly_at_deadline() {
    // At now == deadline a vote must be rejected (VotingEnded), not accepted.
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let voter = s.members.get(1).unwrap();

    let pid = s.client.request_loan(&borrower, &500, &None);
    // Advance to exactly the voting deadline (editing_period_end + voting_period).
    advance(&s.env, EDITING + VOTING_PERIOD);

    let res = s.client.try_vote_on_loan_proposal(&voter, &pid, &true);
    assert_eq!(
        res,
        Err(Ok(crate::Error::VotingEnded)),
        "vote at exact deadline must be rejected"
    );
}

#[test]
fn voting_open_one_second_before_deadline() {
    // One second before deadline a vote must still be accepted.
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let voter = s.members.get(1).unwrap();

    let pid = s.client.request_loan(&borrower, &500, &None);
    // Advance to one second before the voting deadline.
    advance(&s.env, EDITING + VOTING_PERIOD - 1);

    s.client.vote_on_loan_proposal(&voter, &pid, &true);
    let prop = s.client.get_loan_proposal(&pid).unwrap();
    assert!(prop.for_votes > 0, "vote should have been recorded");
}

// Issue #171: Reject zero-amount loan proposals
#[test]
fn zero_amount_loan_request_rejected() {
    let s = setup(1);
    let borrower = s.members.get(0).unwrap();

    let err = s.client.try_request_loan(&borrower, &0, &None);
    assert_eq!(err, Err(Ok(Error::InvalidAmount)));

    let err_neg = s.client.try_request_loan(&borrower, &-100, &None);
    assert_eq!(err_neg, Err(Ok(Error::InvalidAmount)));
}

// ==================== issue #190: member loan stats view ====================
#[test]
fn member_loan_stats_track_lifecycle() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    let stats = s.client.get_member_loan_stats(&borrower);
    assert_eq!(stats.total_loans, 0);
    assert_eq!(stats.repaid_loans, 0);
    assert_eq!(stats.active_loans, 0);

    let pid = s.client.request_loan(&borrower, &1_000, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);

    let stats = s.client.get_member_loan_stats(&borrower);
    assert_eq!(stats.total_loans, 1);
    assert_eq!(stats.repaid_loans, 0);
    assert_eq!(stats.active_loans, 1);

    // Partial repayment leaves the loan outstanding: stats must not move yet.
    let loan = s.client.get_loan(&pid).unwrap();
    let outstanding = loan.total_repayment - loan.amount_repaid;
    s.client
        .repay_loan_partial(&borrower, &pid, &(outstanding / 2));
    let stats = s.client.get_member_loan_stats(&borrower);
    assert_eq!(stats.total_loans, 1);
    assert_eq!(stats.repaid_loans, 0);
    assert_eq!(stats.active_loans, 1);

    // Full repayment settles the loan.
    s.client.repay_loan(&borrower, &pid);
    let stats = s.client.get_member_loan_stats(&borrower);
    assert_eq!(stats.total_loans, 1);
    assert_eq!(stats.repaid_loans, 1);
    assert_eq!(stats.active_loans, 0);

    // A second loan keeps accumulating on top of the first.
    let pid2 = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid2, &true);
    s.client.vote_on_loan_proposal(&v2, &pid2, &true);
    let stats = s.client.get_member_loan_stats(&borrower);
    assert_eq!(stats.total_loans, 2);
    assert_eq!(stats.repaid_loans, 1);
    assert_eq!(stats.active_loans, 1);
}

#[test]
fn member_loan_stats_default_is_not_repaid() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);
    advance(&s.env, LOAN_DURATION + 1);
    s.client.mark_loan_defaulted(&pid);

    let stats = s.client.get_member_loan_stats(&borrower);
    assert_eq!(stats.total_loans, 1);
    assert_eq!(stats.repaid_loans, 0);
    assert_eq!(stats.active_loans, 0);
}

#[test]
fn member_loan_stats_preserved_across_rejoin() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);
    s.client.repay_loan(&borrower, &pid);

    let before = s.client.get_member_loan_stats(&borrower);
    assert_eq!(before.total_loans, 1);
    assert_eq!(before.repaid_loans, 1);

    s.client.exit_dao(&borrower);
    s.client.register_member(&borrower);

    assert_eq!(s.client.get_member_loan_stats(&borrower), before);
}

#[test]
fn member_loan_stats_unknown_member_rejected() {
    let s = setup(1);
    let stranger = Address::generate(&s.env);
    assert_eq!(
        s.client.try_get_member_loan_stats(&stranger),
        Err(Ok(Error::NotMember))
    );
}

// ==================== issue #236: loans in a second asset ====================

#[test]
fn loan_in_second_asset_disburses_and_repays_in_that_asset() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();
    let asset = whitelist_second_asset(&s, 10_000);
    let base_treasury = s.client.get_treasury_balance();
    let base_pending_before = s.client.get_pending_yield(&v1);

    // Sized against the second asset's treasury, not the base token's 3_000.
    let pid = s
        .client
        .request_loan_in_asset(&borrower, &4_000, &asset.address, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);

    let loan = s.client.get_loan(&pid).unwrap();
    assert_eq!(loan.status, LoanStatus::Active);
    assert_eq!(asset.balance(&borrower), 4_000);
    assert_eq!(
        s.client.get_treasury_balance_in_asset(&asset.address),
        6_000
    );
    assert_eq!(s.client.get_treasury_balance(), base_treasury);

    // Repaying names the asset, and only the loan's own asset is accepted.
    let wrong = s
        .client
        .try_repay_loan_in_asset(&borrower, &pid, &s.token.address);
    assert_eq!(wrong, Err(Ok(Error::AssetMismatch)));

    mint_asset(&s, &asset.address, &borrower, loan.total_repayment);
    s.client
        .repay_loan_in_asset(&borrower, &pid, &asset.address);

    assert_eq!(s.client.get_loan(&pid).unwrap().status, LoanStatus::Repaid);
    assert_eq!(
        s.client.get_treasury_balance_in_asset(&asset.address),
        6_000 + loan.total_repayment
    );
    // Interest is credited only to the asset it was repaid in.
    assert_eq!(s.client.get_treasury_balance(), base_treasury);
    assert_eq!(s.client.get_pending_yield(&v1), base_pending_before);
    let interest = loan.total_repayment - loan.principal;
    assert_eq!(
        s.client.get_pending_yield_in_asset(&v1, &asset.address),
        interest / 3
    );
    assert!(!s.client.get_member(&borrower).unwrap().has_active_loan);
}

#[test]
fn loan_in_non_whitelisted_asset_rejected() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let issuer = Address::generate(&s.env);
    let unknown = s.env.register_stellar_asset_contract_v2(issuer).address();

    let res = s
        .client
        .try_request_loan_in_asset(&borrower, &100, &unknown, &None);
    assert_eq!(res, Err(Ok(Error::TokenNotWhitelisted)));
}

#[test]
fn base_token_loan_repays_through_repay_loan_in_asset() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    let pid = s
        .client
        .request_loan_in_asset(&borrower, &1_000, &s.token.address, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);

    assert_eq!(
        s.client
            .try_repay_loan_in_asset(&borrower, &99, &s.token.address),
        Err(Ok(Error::LoanNotFound))
    );
    s.client
        .repay_loan_in_asset(&borrower, &pid, &s.token.address);
    assert_eq!(s.client.get_loan(&pid).unwrap().status, LoanStatus::Repaid);
}

#[test]
fn loan_in_removed_asset_can_still_be_repaid() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();
    let asset = whitelist_second_asset(&s, 10_000);

    let pid = s
        .client
        .request_loan_in_asset(&borrower, &1_000, &asset.address, &None);
    // Editing re-quotes against the same asset's treasury.
    s.client.edit_loan_proposal(&borrower, &pid, &2_000, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);
    assert_eq!(asset.balance(&borrower), 2_000);

    s.client.remove_token(&s.admin, &asset.address);
    assert_eq!(
        s.client
            .try_request_loan_in_asset(&v1, &100, &asset.address, &None),
        Err(Ok(Error::TokenNotWhitelisted))
    );

    let loan = s.client.get_loan(&pid).unwrap();
    mint_asset(&s, &asset.address, &borrower, loan.total_repayment);
    s.client.repay_loan(&borrower, &pid);
    assert_eq!(s.client.get_loan(&pid).unwrap().status, LoanStatus::Repaid);
}

// ==================== issue #185: cancel during editing (migrated from test.rs) ====================

#[test]
fn cancel_loan_proposal_emits_event_and_sets_cancelled_status() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();

    let pid = s.client.request_loan(&borrower, &500, &None);
    let prop = s.client.get_loan_proposal(&pid).unwrap();
    assert_eq!(prop.status, ProposalStatus::Pending);
    assert_eq!(prop.phase, ProposalPhase::Editing);

    s.client.cancel_loan_proposal(&borrower, &pid);
    // Events only cover the latest invocation, so check before reading state.
    assert_prop_cancelled_event(&s.env, pid);

    let prop = s.client.get_loan_proposal(&pid).unwrap();
    assert_eq!(prop.status, ProposalStatus::Cancelled);
    assert_eq!(prop.phase, ProposalPhase::Expired);
}

#[test]
fn cancel_loan_proposal_after_editing_period_rejected() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();

    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);

    let res = s.client.try_cancel_loan_proposal(&borrower, &pid);
    assert_eq!(res, Err(Ok(Error::NotInEditingPhase)));
}

#[test]
fn cancel_loan_proposal_by_non_proposer_rejected() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let other = s.members.get(1).unwrap();

    let pid = s.client.request_loan(&borrower, &500, &None);

    let res = s.client.try_cancel_loan_proposal(&other, &pid);
    assert_eq!(res, Err(Ok(Error::NotBorrower)));
}

// ==================== issue #325: rounding direction ====================

#[test]
fn loan_interest_rounds_up_to_favour_treasury() {
    let s = setup(3);
    // Loan terms for an amount where (amount * rate) is not a multiple of BASIS_POINTS (10_000).
    // Minimum interest rate is 500 bps (5%).
    // For amount = 1: 1 * 500 / 10_000 = 0.05. Floor would yield 0, but ceil gives 1.
    let terms = s.client.calculate_loan_terms(&1);
    assert!(terms.interest_rate >= 500);
    assert!(terms.total_repayment >= 2);
    let interest = terms.total_repayment - 1;
    assert_eq!(
        interest, 1,
        "interest must round up to 1 instead of flooring to 0"
    );
}

#[test]
fn overdue_penalty_rounds_up_to_favour_treasury() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);

    let loan = s.client.get_loan(&pid).unwrap();
    // Repay all but 1 stroop of the loan so outstanding is 1.
    let partial = loan.total_repayment - 1;
    s.client.repay_loan_partial(&borrower, &pid, &partial);

    // Advance beyond due time so default penalty applies on repayment.
    advance(&s.env, LOAN_DURATION + 1);
    let loan_overdue = s.client.get_loan(&pid).unwrap();
    let outstanding_before = loan_overdue.total_repayment - loan_overdue.amount_repaid;
    assert_eq!(outstanding_before, 1);

    // Overdue penalty: default_penalty_bps = 2_000 (20%).
    // 1 * 2000 / 10_000 = 0.2. Floor would give 0 penalty, but ceil gives 1.
    s.client.repay_loan(&borrower, &pid);
    let loan_after = s.client.get_loan(&pid).unwrap();
    assert_eq!(loan_after.status, LoanStatus::Repaid);
    // Total repayment was bumped by 1 (penalty ceil-rounded to 1).
    assert_eq!(loan_after.total_repayment, loan.total_repayment + 1);
}

#[test]
fn default_penalty_rounds_up_to_favour_treasury() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);

    // Advance beyond due_time + default_grace_period to mark defaulted.
    advance(&s.env, LOAN_DURATION + LOAN_DURATION + 1);

    let contribution_before = s.client.get_member(&borrower).unwrap().contribution;
    s.client.mark_loan_defaulted(&pid);

    let member = s.client.get_member(&borrower).unwrap();
    // default_penalty_bps = 2_000 (20%).
    // Ceil rounding: (contribution_before * 2000 + 10000 - 1) / 10000.
    let expected_penalty = (contribution_before * 2_000 + 10_000 - 1) / 10_000;
    assert_eq!(member.contribution, contribution_before - expected_penalty);
}

// ==================== issue #285: single-token loan entrypoints ====================

/// Request → vote → disburse → repay → claim through one entrypoint family,
/// observing after every call. `overdue` repays past `due_time`, so the
/// penalty path is compared too.
fn run_loan_path(path: EntryPath, overdue: bool) -> std::vec::Vec<Observation> {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();
    let base = s.token.address.clone();
    let mut steps = std::vec::Vec::new();

    let pid = match path {
        EntryPath::Legacy => s.client.request_loan(&borrower, &1_000, &None),
        EntryPath::InBaseAsset => s
            .client
            .request_loan_in_asset(&borrower, &1_000, &base, &None),
    };
    steps.push(observe(&s.env));

    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    steps.push(observe(&s.env));
    s.client.vote_on_loan_proposal(&v2, &pid, &true); // approves and disburses
    steps.push(observe(&s.env));

    if overdue {
        advance(&s.env, LOAN_DURATION + 1);
    }
    match path {
        EntryPath::Legacy => s.client.repay_loan(&borrower, &pid),
        EntryPath::InBaseAsset => s.client.repay_loan_in_asset(&borrower, &pid, &base),
    }
    steps.push(observe(&s.env));
    assert_eq!(s.client.get_loan(&pid).unwrap().status, LoanStatus::Repaid);

    // The yield the repayment produced is the same either way.
    s.client.claim_rewards(&v1);
    steps.push(observe(&s.env));
    steps
}

#[test]
fn single_token_loan_entrypoints_match_base_asset_counterparts() {
    for overdue in [false, true] {
        let legacy = run_loan_path(EntryPath::Legacy, overdue);
        let in_base = run_loan_path(EntryPath::InBaseAsset, overdue);

        // Every step did real work, so the comparison below isn't vacuous.
        let topics: std::vec::Vec<_> = legacy.iter().map(|o| event_topics(&o.0)).collect();
        for (step, expected) in ["loan_req", "loan_vote", "loan_appr", "loan_rpy", "claimed"]
            .iter()
            .enumerate()
        {
            assert!(
                topics[step].iter().any(|t| t == expected),
                "step {step} emitted {:?}, expected {expected}",
                topics[step]
            );
        }
        // Naming the base token records no loan asset, like the legacy path.
        assert!(!topics[0].iter().any(|t| t == "loan_asst"));

        assert_runs_identical(&legacy, &in_base);
    }
}

// ==================== issue #287: default of a loan in a non-base asset ====================

#[test]
fn default_in_second_asset() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();
    let asset = whitelist_second_asset(&s, 10_000);

    // Give every member unclaimed yield in both assets so the forfeiture
    // on default is observable.
    let base_pid = s.client.request_loan(&v1, &1_000, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v2, &base_pid, &true);
    s.client.vote_on_loan_proposal(&borrower, &base_pid, &true);
    s.client.repay_loan(&v1, &base_pid);

    let asset_yield_pid = s
        .client
        .request_loan_in_asset(&v1, &1_000, &asset.address, &None);
    advance(&s.env, EDITING + 1);
    s.client
        .vote_on_loan_proposal(&borrower, &asset_yield_pid, &true);
    s.client.vote_on_loan_proposal(&v2, &asset_yield_pid, &true);
    let asset_yield_loan = s.client.get_loan(&asset_yield_pid).unwrap();
    mint_asset(&s, &asset.address, &v1, asset_yield_loan.total_repayment);
    s.client
        .repay_loan_in_asset(&v1, &asset_yield_pid, &asset.address);
    let accrued_base = s.client.get_pending_yield(&borrower);
    let accrued_asset = s
        .client
        .get_pending_yield_in_asset(&borrower, &asset.address);
    assert!(accrued_base > 0);
    assert!(accrued_asset > 0);

    let pid = s
        .client
        .request_loan_in_asset(&borrower, &4_000, &asset.address, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);
    let loan = s.client.get_loan(&pid).unwrap();
    assert_eq!(loan.principal, 4_000);

    let base_treasury = s.client.get_treasury_balance();
    let asset_treasury = s.client.get_treasury_balance_in_asset(&asset.address);
    let borrower_asset = asset.balance(&borrower);
    let contribution_before = s.client.get_member(&borrower).unwrap().contribution;
    assert_eq!(
        s.client
            .get_treasury_accounting(&asset.address)
            .outstanding_principal,
        4_000
    );

    advance(&s.env, LOAN_DURATION + 1); // past due_time, grace period is 0
    s.client.mark_loan_defaulted(&pid);
    let default_events = s.env.events().all();
    assert_eq!(
        s.client.get_loan(&pid).unwrap().status,
        LoanStatus::Defaulted
    );

    // The slashed claim is the borrower's base-token `contribution` (their
    // share of the base treasury at exit), cut by `default_penalty_bps`
    // exactly as for a base-token default. Nothing in asset B is slashed.
    let expected_penalty = (contribution_before * 2_000 + 10_000 - 1) / 10_000;
    let member = s.client.get_member(&borrower).unwrap();
    assert_eq!(member.contribution, contribution_before - expected_penalty);
    assert!(!member.has_active_loan);
    assert_eq!(member.active_loans, 0);
    let dflt = default_events
        .events()
        .iter()
        .find(|e| event_topics(core::slice::from_ref(*e)) == ["loan_dflt"])
        .expect("loan_dflt event not found");
    let ContractEventBody::V0(body) = &dflt.body;
    let ScVal::Vec(Some(data)) = &body.data else {
        panic!("unexpected loan_dflt data: {:?}", body.data);
    };
    assert_eq!(data[0], ScVal::U32(pid));
    assert_eq!(
        data[2],
        ScVal::I128(soroban_sdk::xdr::Int128Parts {
            hi: 0,
            lo: expected_penalty as u64,
        })
    );

    // No tokens move on default. The borrower keeps the asset-B principal and
    // the asset-B treasury absorbs the loss: the outstanding principal is
    // written off, not recovered from the base token.
    assert_eq!(asset.balance(&borrower), borrower_asset);
    assert_eq!(
        s.client.get_treasury_balance_in_asset(&asset.address),
        asset_treasury
    );
    assert_eq!(s.client.get_treasury_balance(), base_treasury);
    assert_eq!(
        s.client
            .get_treasury_accounting(&asset.address)
            .outstanding_principal,
        0
    );

    // Unclaimed base-token yield is forfeited; everyone else keeps theirs.
    assert_eq!(s.client.get_pending_yield(&borrower), 0);
    assert_eq!(
        s.client
            .get_pending_yield_in_asset(&borrower, &asset.address),
        0
    );
    assert_eq!(s.client.get_pending_yield(&v2), accrued_base);
    assert_eq!(
        s.client.get_pending_yield_in_asset(&v2, &asset.address),
        accrued_asset
    );

    // The reduced contribution shrinks the base-token exit share. The capital
    // share never includes asset B; the forfeited B yield was settled above.
    let expected_share = base_treasury * member.contribution / (3 * FEE - expected_penalty);
    assert_eq!(s.client.calculate_exit_share(&borrower), expected_share);
    let borrower_base = s.token.balance(&borrower);
    s.client.exit_dao(&borrower);
    assert_eq!(s.token.balance(&borrower), borrower_base + expected_share);
    assert_eq!(asset.balance(&borrower), borrower_asset);
    assert_eq!(
        s.client.get_treasury_balance_in_asset(&asset.address),
        asset_treasury
    );
}

// ==================== issue #286: full lifecycle of a loan in a non-base asset ====================

#[test]
fn loan_lifecycle_in_second_asset() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();
    let base = s.token.address.clone();
    let b = whitelist_second_asset(&s, 10_000);
    let dao = s.client.address.clone();

    // Base-token yield from an earlier base loan, so the claim at the end
    // pays something and shows the asset-B loan never changed it.
    let base_pid = s.client.request_loan(&v1, &1_000, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v2, &base_pid, &true);
    s.client.vote_on_loan_proposal(&borrower, &base_pid, &true);
    s.client.repay_loan(&v1, &base_pid);
    let base_yield = s.client.get_pending_yield(&v2);
    assert!(base_yield > 0);

    let base_treasury = s.client.get_treasury_balance();
    let borrower_base = s.token.balance(&borrower);
    let assert_base_untouched = || {
        assert_eq!(s.client.get_treasury_balance(), base_treasury);
        assert_eq!(s.token.balance(&borrower), borrower_base);
        assert_eq!(s.client.get_pending_yield(&v2), base_yield);
        assert_eq!(
            s.client
                .get_treasury_accounting(&base)
                .outstanding_principal,
            0
        );
    };
    let outstanding_b = || {
        s.client
            .get_treasury_accounting(&b.address)
            .outstanding_principal
    };

    // 1. Request: sized and priced against asset B's 10_000 treasury.
    // ratio 2_000 bps → rate 500 + 300 = 800 → interest 160.
    let pid = s
        .client
        .request_loan_in_asset(&borrower, &2_000, &b.address, &None);
    let quoted = s.client.get_loan_proposal(&pid).unwrap();
    assert_eq!(quoted.interest_rate, 800);
    assert_eq!(quoted.total_repayment, 2_160);
    assert_eq!(b.balance(&dao), 10_000);
    assert_eq!(b.balance(&borrower), 0);
    assert_eq!(outstanding_b(), 0);
    assert_base_untouched();

    // 2. Edit: re-quoted against asset B, not the much smaller base treasury
    // (which would price 3_000 at the 20% cap).
    // ratio 3_000 bps → rate 500 + 450 = 950 → interest 285.
    s.client.edit_loan_proposal(&borrower, &pid, &3_000, &None);
    let edited = s.client.get_loan_proposal(&pid).unwrap();
    assert_eq!(edited.amount, 3_000);
    assert_eq!(edited.interest_rate, 950);
    assert_eq!(edited.total_repayment, 3_285);
    assert_ne!(
        s.client.calculate_loan_terms(&3_000).interest_rate,
        edited.interest_rate
    );
    assert_eq!(b.balance(&dao), 10_000);
    assert_eq!(b.balance(&borrower), 0);
    assert_base_untouched();

    // 3. Vote: an asset-B withdrawal leaves too little B to fund the loan,
    // so the approved proposal waits for disbursement instead of paying out.
    let dest = Address::generate(&s.env);
    let reason = String::from_str(&s.env, "drain asset b");
    let tid = s
        .client
        .propose_withdrawal_in_asset(&v1, &8_000, &b.address, &dest, &reason, &false);
    s.client.vote_on_treasury_proposal(&v1, &tid, &true);
    s.client.vote_on_treasury_proposal(&v2, &tid, &true);
    assert_eq!(b.balance(&dest), 8_000);
    assert_eq!(b.balance(&dao), 2_000);

    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);
    assert!(emitted(&s.env, "loan_wait"));
    assert_eq!(
        s.client.get_loan_proposal(&pid).unwrap().status,
        ProposalStatus::ApprovedPendingDisbursement
    );
    assert!(s.client.get_loan(&pid).is_none());
    assert_eq!(b.balance(&dao), 2_000);
    assert_eq!(b.balance(&borrower), 0);
    assert_eq!(outstanding_b(), 0);
    assert_base_untouched();

    // 4. Disburse: once asset B is topped up, the same proposal pays out in B.
    mint_asset(&s, &b.address, &dao, 8_000);
    s.client.disburse_approved_loan(&pid);
    let loan = s.client.get_loan(&pid).unwrap();
    assert_eq!(loan.status, LoanStatus::Active);
    assert_eq!(loan.principal, 3_000);
    assert_eq!(loan.total_repayment, 3_285);
    assert_eq!(b.balance(&borrower), 3_000);
    assert_eq!(b.balance(&dao), 7_000);
    assert_eq!(
        s.client.get_treasury_accounting(&b.address),
        crate::types::TreasuryAccounting {
            token_balance: 7_000,
            staked: 0,
            outstanding_principal: 3_000,
        }
    );
    assert_base_untouched();

    // 5. Partial repay in B: interest (285) first, then 1_000 of principal.
    mint_asset(&s, &b.address, &borrower, 285);
    s.client.repay_loan_partial(&borrower, &pid, &1_285);
    let loan = s.client.get_loan(&pid).unwrap();
    assert_eq!(loan.status, LoanStatus::Active);
    assert_eq!(loan.amount_repaid, 1_285);
    assert_eq!(b.balance(&borrower), 2_000);
    assert_eq!(b.balance(&dao), 8_285);
    assert_eq!(outstanding_b(), 2_000);
    // Interest paid in asset B is not distributed as base-token yield.
    assert_base_untouched();

    // 6. Full repay, naming asset B.
    s.client.repay_loan_in_asset(&borrower, &pid, &b.address);
    assert_eq!(s.client.get_loan(&pid).unwrap().status, LoanStatus::Repaid);
    assert!(!s.client.get_member(&borrower).unwrap().has_active_loan);
    assert_eq!(b.balance(&borrower), 0);
    assert_eq!(b.balance(&dao), 10_285);
    assert_eq!(outstanding_b(), 0);
    assert_base_untouched();

    // 7. Yield claim: pays exactly the base-token yield accrued before the
    // asset-B loan, in the base token. The asset-B interest (285) is not
    // distributed and stays in the asset-B treasury; per-asset yield is #271.
    let v2_base = s.token.balance(&v2);
    assert_eq!(s.client.claim_rewards(&v2), base_yield);
    assert_eq!(s.token.balance(&v2), v2_base + base_yield);
    assert_eq!(b.balance(&v2), 0);
    assert_eq!(s.client.get_pending_yield(&v2), 0);
    assert_eq!(b.balance(&dao), 10_285);
    assert_eq!(s.client.get_treasury_balance(), base_treasury - base_yield);
}

// Issue #277: non-base loan sizing must use that asset's treasury, never base-token liquidity.
#[test]
fn second_asset_loan_ratio_uses_second_asset_balance() {
    let s = setup(10); // base treasury = 10_000
    let borrower = s.members.get(0).unwrap();
    let asset = whitelist_second_asset(&s, 1_000);

    // Policy cap is 50%: 501 exceeds half of asset-B's 1_000 even though
    // the much larger base-token treasury could cover it.
    let before_base = s.client.get_treasury_balance();
    let before_asset = s.client.get_treasury_balance_in_asset(&asset.address);
    let res = s
        .client
        .try_request_loan_in_asset(&borrower, &501, &asset.address, &None);
    assert_eq!(res, Err(Ok(Error::ExceedsTreasuryRatio)));
    assert_eq!(s.client.get_treasury_balance(), before_base);
    assert_eq!(
        s.client.get_treasury_balance_in_asset(&asset.address),
        before_asset
    );
}

#[test]
fn token_cannot_be_removed_until_its_loan_is_repaid() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();
    let asset = whitelist_second_asset(&s, 1_000);
    let proposal_id = s
        .client
        .request_loan_in_asset(&borrower, &500, &asset.address, &None);

    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &proposal_id, &true);
    s.client.vote_on_loan_proposal(&v2, &proposal_id, &true);

    let loan = s.client.get_loan(&proposal_id).unwrap();
    let treasury_balance = asset.balance(&s.client.address);
    soroban_sdk::token::StellarAssetClient::new(&s.env, &asset.address)
        .burn(&s.client.address, &treasury_balance);
    assert_eq!(
        s.client.try_remove_token(&s.admin, &asset.address),
        Err(Ok(Error::TokenHasActiveLoans))
    );

    mint_asset(
        &s,
        &asset.address,
        &borrower,
        loan.total_repayment - loan.principal,
    );
    s.client
        .repay_loan_in_asset(&borrower, &proposal_id, &asset.address);

    let treasury_balance = asset.balance(&s.client.address);
    soroban_sdk::token::StellarAssetClient::new(&s.env, &asset.address)
        .burn(&s.client.address, &treasury_balance);
    s.client.remove_token(&s.admin, &asset.address);
    assert!(!s.client.is_token_whitelisted(&asset.address));
}

// Issue #278: naming the wrong repayment asset must fail before any transfer.
#[test]
fn repay_loan_in_asset_wrong_asset_moves_no_tokens() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();
    let asset = whitelist_second_asset(&s, 10_000);

    let pid = s
        .client
        .request_loan_in_asset(&borrower, &1_000, &asset.address, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);

    let loan = s.client.get_loan(&pid).unwrap();
    mint_asset(&s, &asset.address, &borrower, loan.total_repayment);
    let borrower_asset_before = asset.balance(&borrower);
    let dao_asset_before = asset.balance(&s.client.address);
    let borrower_base_before = s.token.balance(&borrower);
    let dao_base_before = s.token.balance(&s.client.address);

    let res = s
        .client
        .try_repay_loan_in_asset(&borrower, &pid, &s.token.address);
    assert_eq!(res, Err(Ok(Error::AssetMismatch)));
    assert_eq!(asset.balance(&borrower), borrower_asset_before);
    assert_eq!(asset.balance(&s.client.address), dao_asset_before);
    assert_eq!(s.token.balance(&borrower), borrower_base_before);
    assert_eq!(s.token.balance(&s.client.address), dao_base_before);
    assert_eq!(s.client.get_loan(&pid).unwrap().amount_repaid, 0);
}

// Issue #509: voting power is deliberately independent of economic contribution.
#[test]
fn heavily_slashed_member_keeps_full_base_voting_weight() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let voter = s.members.get(1).unwrap();

    s.env.as_contract(&s.client.address, || {
        let mut member = crate::storage::get_member(&s.env, &voter).unwrap();
        member.contribution = 1;
        crate::storage::set_member(&s.env, &member);
    });
    assert_eq!(s.client.get_member(&voter).unwrap().contribution, 1);
    assert_eq!(s.client.get_voting_weight(&voter), BASE_VOTE_WEIGHT);

    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&voter, &pid, &true);

    let proposal = s.client.get_loan_proposal(&pid).unwrap();
    assert_eq!(proposal.for_votes, BASE_VOTE_WEIGHT);
}

// Issue #515: partial repayment and default penalties are independent mechanisms.
#[test]
fn partial_repayment_before_due_then_default_only_slashes_contribution() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);

    let original = s.client.get_loan(&pid).unwrap();
    let partial = original.total_repayment * 30 / 100;
    s.client.repay_loan_partial(&borrower, &pid, &partial);

    let after_partial = s.client.get_loan(&pid).unwrap();
    assert_eq!(after_partial.amount_repaid, partial);
    assert_eq!(after_partial.total_repayment, original.total_repayment);

    let contribution_before = s.client.get_member(&borrower).unwrap().contribution;
    advance(&s.env, LOAN_DURATION + 1);
    s.client.mark_loan_defaulted(&pid);

    let defaulted = s.client.get_loan(&pid).unwrap();
    assert_eq!(defaulted.total_repayment, original.total_repayment);
    let expected_slash = (contribution_before * 2_000 + 10_000 - 1) / 10_000;
    assert_eq!(
        s.client.get_member(&borrower).unwrap().contribution,
        contribution_before - expected_slash
    );
}

#[test]
fn post_due_partial_repayment_adds_late_fee_before_payment() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);

    let before = s.client.get_loan(&pid).unwrap();
    advance(&s.env, LOAN_DURATION + 1);
    s.client.repay_loan_partial(&borrower, &pid, &100);

    let after = s.client.get_loan(&pid).unwrap();
    let expected_fee = (before.total_repayment * 2_000 + 10_000 - 1) / 10_000;
    assert_eq!(after.total_repayment, before.total_repayment + expected_fee);
    assert_eq!(after.amount_repaid, 100);
}
