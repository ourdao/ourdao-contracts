//! Loan due-date extension requests (ADR-0004, #437).

extern crate std;

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{token, Address, Bytes, Vec};

use super::common::*;
use crate::storage::ProposalKind;
use crate::types::{LoanPolicy, LoanStatus, ProposalStatus, SeedDocEntry};
use crate::Error;

const DAY: u64 = 24 * 60 * 60;

/// Four members; `members[0]` borrows 1,000 (25% of the 4,000 treasury, so
/// 875 bps and a `total_repayment` of 1,088) and the other three approve it.
/// Returns the loan id.
fn active_loan(s: &Setup) -> u32 {
    let borrower = s.members.get(0).unwrap();
    let id = s.client.request_loan(&borrower, &1_000, &None);
    advance(&s.env, EDITING + 1);
    for i in 1..4 {
        s.client
            .vote_on_loan_proposal(&s.members.get(i).unwrap(), &id, &true);
    }
    let loan = s.client.get_loan(&id).unwrap();
    assert_eq!(loan.status, LoanStatus::Active);
    assert_eq!(loan.total_repayment, 1_088);
    id
}

fn now(s: &Setup) -> u64 {
    s.env.ledger().timestamp()
}

#[test]
fn request_quotes_the_fee_and_records_a_pending_request() {
    let s = setup(4);
    let borrower = s.members.get(0).unwrap();
    let id = active_loan(&s);
    assert_eq!(s.client.get_loan_extension(&id), None);

    s.client.request_loan_extension(&borrower, &id, &(10 * DAY));
    assert!(emitted(&s.env, "ext_req"));
    // Only the borrower's authorization was required.
    let auths = s.env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, borrower);

    let ext = s.client.get_loan_extension(&id).unwrap();
    assert_eq!(ext.loan_id, id);
    assert_eq!(ext.extra_time, 10 * DAY);
    // ceil(1_088 * 875 * 10 / (10_000 * 30)) = ceil(31.73)
    assert_eq!(ext.fee, 32);
    assert_eq!(ext.created_at, now(&s));
    assert_eq!(ext.voting_period, VOTING_PERIOD);
    assert_eq!(ext.status, ProposalStatus::Pending);
    assert_eq!(
        (ext.for_votes, ext.against_votes, ext.votes_cast),
        (0, 0, 0)
    );

    // The loan itself is untouched until members approve.
    let loan = s.client.get_loan(&id).unwrap();
    assert_eq!(loan.total_repayment, 1_088);
}

#[test]
fn fee_on_a_partly_repaid_loan_uses_what_is_still_owed() {
    let s = setup(4);
    let borrower = s.members.get(0).unwrap();
    let id = active_loan(&s);
    s.client.repay_loan_partial(&borrower, &id, &500);

    s.client.request_loan_extension(&borrower, &id, &(10 * DAY));
    // ceil(588 * 875 * 10 / (10_000 * 30)) = ceil(17.15), rounded up for the treasury.
    assert_eq!(s.client.get_loan_extension(&id).unwrap().fee, 18);
}

#[test]
fn approval_extends_the_loan_and_the_fee_is_paid_out_as_yield() {
    let s = setup(4);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();
    let id = active_loan(&s);
    let due = s.client.get_loan(&id).unwrap().due_time;

    s.client.request_loan_extension(&borrower, &id, &(10 * DAY));
    s.client.vote_on_loan_extension(&v1, &id, &true);
    assert!(emitted(&s.env, "ext_vote"));
    assert_eq!(s.env.auths()[0].0, v1);
    // Three voters at 51% need two votes, so one is not enough.
    assert_eq!(
        s.client.get_loan_extension(&id).unwrap().status,
        ProposalStatus::Pending
    );

    s.client.vote_on_loan_extension(&v2, &id, &true);
    assert!(emitted(&s.env, "ext_appr"));
    let ext = s.client.get_loan_extension(&id).unwrap();
    assert_eq!(ext.status, ProposalStatus::Approved);
    assert_eq!((ext.for_votes, ext.votes_cast), (2, 2));

    let loan = s.client.get_loan(&id).unwrap();
    assert_eq!(loan.due_time, due + 10 * DAY);
    assert_eq!(loan.total_repayment, 1_088 + 32);

    // A decided request takes no more votes.
    assert_eq!(
        s.client
            .try_vote_on_loan_extension(&s.members.get(3).unwrap(), &id, &true),
        Err(Ok(Error::VotingEnded))
    );

    // Repaying the extended loan pays the fee out as interest: 120 split
    // across four active members, against 88 / 4 without the extension.
    s.client.repay_loan(&borrower, &id);
    assert_eq!(s.client.get_loan(&id).unwrap().status, LoanStatus::Repaid);
    assert_eq!(s.client.get_pending_yield(&v1), 30);
}

#[test]
fn the_new_due_time_moves_the_default_boundary() {
    let s = setup(4);
    let borrower = s.members.get(0).unwrap();
    let id = active_loan(&s);
    let due = s.client.get_loan(&id).unwrap().due_time;

    s.client.request_loan_extension(&borrower, &id, &(10 * DAY));
    s.client
        .vote_on_loan_extension(&s.members.get(1).unwrap(), &id, &true);
    s.client
        .vote_on_loan_extension(&s.members.get(2).unwrap(), &id, &true);

    advance(&s.env, due - now(&s) + 1);
    assert_eq!(
        s.client.try_mark_loan_defaulted(&id),
        Err(Ok(Error::LoanNotOverdue))
    );
    advance(&s.env, 10 * DAY);
    s.client.mark_loan_defaulted(&id);
}

#[test]
fn request_filed_exactly_at_the_deadline_resolves_before_default() {
    let s = setup(4);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let id = active_loan(&s);
    let due = s.client.get_loan(&id).unwrap().due_time;

    // Grace period is 0, so the deadline is `now + voting_period == due_time`.
    advance(&s.env, due - VOTING_PERIOD - now(&s));
    s.client.request_loan_extension(&borrower, &id, &DAY);

    // One second before the vote closes the loan can't be defaulted yet.
    advance(&s.env, VOTING_PERIOD - 1);
    assert_eq!(
        s.client.try_mark_loan_defaulted(&id),
        Err(Ok(Error::LoanNotOverdue))
    );
    s.client.vote_on_loan_extension(&v1, &id, &true);

    // At the deadline voting is closed and only then is default allowed.
    advance(&s.env, 1);
    assert_eq!(
        s.client
            .try_vote_on_loan_extension(&s.members.get(2).unwrap(), &id, &true),
        Err(Ok(Error::VotingEnded))
    );
    assert_eq!(
        s.client.get_loan_extension(&id).unwrap().status,
        ProposalStatus::Expired
    );
    s.client.mark_loan_defaulted(&id);
}

#[test]
fn request_one_second_past_the_deadline_is_rejected() {
    let s = setup(4);
    let borrower = s.members.get(0).unwrap();
    let id = active_loan(&s);
    let due = s.client.get_loan(&id).unwrap().due_time;

    advance(&s.env, due - VOTING_PERIOD - now(&s) + 1);
    assert_eq!(
        s.client.try_request_loan_extension(&borrower, &id, &DAY),
        Err(Ok(Error::ExtensionWindowClosed))
    );
    assert_eq!(s.client.get_loan_extension(&id), None);
}

#[test]
fn deadline_uses_the_grace_period_snapshotted_on_the_loan() {
    let s = DAOBuilder::new()
        .with_members(4)
        .with_policy(LoanPolicy {
            default_grace_period: 2 * DAY,
            ..policy()
        })
        .build();
    let borrower = s.members.get(0).unwrap();
    let id = active_loan(&s);
    let due = s.client.get_loan(&id).unwrap().due_time;

    // A later policy change to a zero grace period doesn't shorten this
    // loan's window, because `mark_loan_defaulted` still uses the loan's own.
    s.client.propose_policy_update(&s.admin, &policy());
    advance(&s.env, crate::admin::TIMELOCK_DURATION);
    s.client.execute_policy_update(&s.admin);

    advance(&s.env, due + 2 * DAY - VOTING_PERIOD - now(&s));
    s.client.request_loan_extension(&borrower, &id, &DAY);
}

#[test]
fn rejection_leaves_the_loan_alone_and_it_can_still_default() {
    let s = setup(4);
    let borrower = s.members.get(0).unwrap();
    let id = active_loan(&s);
    let due = s.client.get_loan(&id).unwrap().due_time;

    s.client.request_loan_extension(&borrower, &id, &(10 * DAY));
    for i in 1..3 {
        s.client
            .vote_on_loan_extension(&s.members.get(i).unwrap(), &id, &false);
        assert_eq!(
            s.client.get_loan_extension(&id).unwrap().status,
            ProposalStatus::Pending
        );
    }
    // The last voter could no longer lift it to two votes.
    s.client
        .vote_on_loan_extension(&s.members.get(3).unwrap(), &id, &false);
    assert!(emitted(&s.env, "ext_rej"));
    let ext = s.client.get_loan_extension(&id).unwrap();
    assert_eq!(ext.status, ProposalStatus::Rejected);
    assert_eq!(
        (ext.for_votes, ext.against_votes, ext.votes_cast),
        (0, 3, 3)
    );

    let loan = s.client.get_loan(&id).unwrap();
    assert_eq!((loan.due_time, loan.total_repayment), (due, 1_088));

    // The rejected request used up the loan's only one.
    assert_eq!(
        s.client.try_request_loan_extension(&borrower, &id, &DAY),
        Err(Ok(Error::ExtensionAlreadyRequested))
    );

    advance(&s.env, due - now(&s));
    s.client.mark_loan_defaulted(&id);
    assert_eq!(
        s.client.get_loan(&id).unwrap().status,
        LoanStatus::Defaulted
    );
}

#[test]
fn an_expired_request_changes_nothing_and_uses_up_the_request() {
    let s = setup(4);
    let borrower = s.members.get(0).unwrap();
    let id = active_loan(&s);
    let before = s.client.get_loan(&id).unwrap();

    s.client.request_loan_extension(&borrower, &id, &DAY);
    advance(&s.env, VOTING_PERIOD);
    assert_eq!(
        s.client.get_loan_extension(&id).unwrap().status,
        ProposalStatus::Expired
    );
    assert_eq!(
        s.client
            .try_vote_on_loan_extension(&s.members.get(1).unwrap(), &id, &true),
        Err(Ok(Error::VotingEnded))
    );
    assert_eq!(s.client.get_loan(&id).unwrap(), before);
    assert_eq!(
        s.client.try_request_loan_extension(&borrower, &id, &DAY),
        Err(Ok(Error::ExtensionAlreadyRequested))
    );
}

#[test]
fn request_validation() {
    let s = setup(4);
    let borrower = s.members.get(0).unwrap();
    let other = s.members.get(1).unwrap();
    let id = active_loan(&s);
    let max = policy().max_loan_duration;

    assert_eq!(
        s.client.try_request_loan_extension(&borrower, &99, &DAY),
        Err(Ok(Error::LoanNotFound))
    );
    assert_eq!(
        s.client.try_request_loan_extension(&other, &id, &DAY),
        Err(Ok(Error::NotBorrower))
    );
    assert_eq!(
        s.client
            .try_request_loan_extension(&Address::generate(&s.env), &id, &DAY),
        Err(Ok(Error::NotMember))
    );
    assert_eq!(
        s.client.try_request_loan_extension(&borrower, &id, &0),
        Err(Ok(Error::InvalidExtension))
    );
    assert_eq!(
        s.client
            .try_request_loan_extension(&borrower, &id, &(max + 1)),
        Err(Ok(Error::InvalidExtension))
    );
    s.client.request_loan_extension(&borrower, &id, &max);

    // A repaid loan has nothing to extend.
    let s = setup(4);
    let id = active_loan(&s);
    s.client.repay_loan(&borrower_of(&s), &id);
    assert_eq!(
        s.client
            .try_request_loan_extension(&borrower_of(&s), &id, &DAY),
        Err(Ok(Error::LoanNotActive))
    );

    s.client.pause(&s.admin);
    assert_eq!(
        s.client
            .try_request_loan_extension(&borrower_of(&s), &id, &DAY),
        Err(Ok(Error::Paused))
    );
}

fn borrower_of(s: &Setup) -> Address {
    s.members.get(0).unwrap()
}

#[test]
fn vote_validation() {
    let s = setup(4);
    let borrower = borrower_of(&s);
    let v1 = s.members.get(1).unwrap();
    let id = active_loan(&s);

    assert_eq!(
        s.client.try_vote_on_loan_extension(&v1, &id, &true),
        Err(Ok(Error::ExtensionNotFound))
    );
    s.client.request_loan_extension(&borrower, &id, &DAY);

    assert_eq!(
        s.client.try_vote_on_loan_extension(&borrower, &id, &true),
        Err(Ok(Error::NotAuthorized))
    );
    assert_eq!(
        s.client
            .try_vote_on_loan_extension(&Address::generate(&s.env), &id, &true),
        Err(Ok(Error::NotMember))
    );

    assert!(!s.client.has_voted(&ProposalKind::LoanExtension, &id, &v1));
    s.client.vote_on_loan_extension(&v1, &id, &false);
    assert!(s.client.has_voted(&ProposalKind::LoanExtension, &id, &v1));
    // Votes on the loan proposal and on its extension are tracked apart.
    let v3 = s.members.get(3).unwrap();
    assert!(s.client.has_voted(&ProposalKind::Loan, &id, &v3));
    assert!(!s.client.has_voted(&ProposalKind::LoanExtension, &id, &v3));
    assert_eq!(
        s.client.try_vote_on_loan_extension(&v1, &id, &true),
        Err(Ok(Error::AlreadyVoted))
    );

    // Members who join after the request was filed are outside its electorate.
    advance(&s.env, 1);
    let late = Address::generate(&s.env);
    token::StellarAssetClient::new(&s.env, &s.token.address).mint(&late, &MINT);
    s.client.register_member(&late);
    assert_eq!(
        s.client.try_vote_on_loan_extension(&late, &id, &true),
        Err(Ok(Error::JoinedAfterProposal))
    );

    // Repaid mid-vote: the request can't be approved onto a closed loan.
    s.client.repay_loan(&borrower, &id);
    assert_eq!(
        s.client
            .try_vote_on_loan_extension(&s.members.get(2).unwrap(), &id, &true),
        Err(Ok(Error::LoanNotActive))
    );

    s.client.pause(&s.admin);
    assert_eq!(
        s.client
            .try_vote_on_loan_extension(&s.members.get(2).unwrap(), &id, &true),
        Err(Ok(Error::Paused))
    );
}

#[test]
fn extension_votes_keep_the_voters_stake_locked_until_the_vote_ends() {
    let s = setup(4);
    let borrower = borrower_of(&s);
    let v1 = s.members.get(1).unwrap();
    let id = active_loan(&s);
    s.client.stake(&v1, &50);

    s.client.request_loan_extension(&borrower, &id, &DAY);
    s.client.vote_on_loan_extension(&v1, &id, &true);
    assert_eq!(
        s.client.try_unstake(&v1, &50),
        Err(Ok(Error::HasActiveLoan))
    );

    advance(&s.env, VOTING_PERIOD);
    s.client.unstake(&v1, &50);
}

#[test]
fn extension_requests_take_no_documents() {
    let s = setup(4);
    let borrower = borrower_of(&s);
    let id = active_loan(&s);
    s.client.request_loan_extension(&borrower, &id, &DAY);
    let cid = Bytes::from_array(&s.env, b"QmExampleCid1234567890");

    assert_eq!(
        s.client
            .try_attach_document(&borrower, &ProposalKind::LoanExtension, &id, &cid),
        Err(Ok(Error::InvalidProposal))
    );
    assert_eq!(
        s.client.get_document(&ProposalKind::LoanExtension, &id),
        None
    );

    let mut docs = Vec::new(&s.env);
    docs.push_back(SeedDocEntry {
        kind: ProposalKind::LoanExtension,
        proposal_id: id,
        content_hash: cid,
    });
    assert_eq!(
        s.client.try_seed_docs(&s.admin, &docs),
        Err(Ok(Error::InvalidProposal))
    );
}
