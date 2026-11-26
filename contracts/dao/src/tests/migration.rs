use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env, Vec};

use super::common::*;
use crate::types::{
    Loan, LoanStatus, Member, MemberStatus, SeedDocEntry, SeedNameEntry, SeedStakeEntry,
    SeedYieldEntry,
};
use crate::{storage::ProposalKind, Error};

#[test]
fn seed_entrypoints_check_admin_before_seeding_lock() {
    let s = setup(1);
    let outsider = soroban_sdk::Address::generate(&s.env);

    // Lock the seeding window as the admin.
    s.client.lock_seeding(&s.admin);

    let members: Vec<Member> = Vec::new(&s.env);
    let stakes: Vec<SeedStakeEntry> = Vec::new(&s.env);
    let yields: Vec<SeedYieldEntry> = Vec::new(&s.env);
    let loans: Vec<Loan> = Vec::new(&s.env);
    let names: Vec<SeedNameEntry> = Vec::new(&s.env);
    let docs: Vec<SeedDocEntry> = Vec::new(&s.env);

    // Authorization must be checked first, even when seeding is locked.
    assert_eq!(
        s.client.try_seed_members(&outsider, &members),
        Err(Ok(Error::NotAdmin))
    );
    assert_eq!(
        s.client.try_seed_stakes(&outsider, &stakes),
        Err(Ok(Error::NotAdmin))
    );
    assert_eq!(
        s.client.try_seed_pending_yield(&outsider, &yields),
        Err(Ok(Error::NotAdmin))
    );
    assert_eq!(
        s.client.try_seed_loans(&outsider, &loans),
        Err(Ok(Error::NotAdmin))
    );
    assert_eq!(
        s.client.try_seed_names(&outsider, &names),
        Err(Ok(Error::NotAdmin))
    );
    assert_eq!(
        s.client.try_seed_docs(&outsider, &docs),
        Err(Ok(Error::NotAdmin))
    );

    // Once authorized, the lock error is independently reachable.
    assert_eq!(
        s.client.try_seed_members(&s.admin, &members),
        Err(Ok(Error::SeedingLocked))
    );
    assert_eq!(
        s.client.try_seed_stakes(&s.admin, &stakes),
        Err(Ok(Error::SeedingLocked))
    );
    assert_eq!(
        s.client.try_seed_pending_yield(&s.admin, &yields),
        Err(Ok(Error::SeedingLocked))
    );
    assert_eq!(
        s.client.try_seed_loans(&s.admin, &loans),
        Err(Ok(Error::SeedingLocked))
    );
    assert_eq!(
        s.client.try_seed_names(&s.admin, &names),
        Err(Ok(Error::SeedingLocked))
    );
    assert_eq!(
        s.client.try_seed_docs(&s.admin, &docs),
        Err(Ok(Error::SeedingLocked))
    );

    // Keep ProposalKind referenced here to make it easy to add a populated
    // docs case to this test later.
    let _ = ProposalKind::Loan;
}

// ── seed_loans cross-checks the borrower's has_active_loan flag (#455) ─────

fn seeded_member(env: &Env, address: &Address, has_active_loan: bool) -> Member {
    Member {
        address: address.clone(),
        status: MemberStatus::ActiveMember,
        join_time: env.ledger().timestamp(),
        contribution: FEE,
        share_balance: FEE,
        has_active_loan,
        last_loan_time: 0,
        total_loans: has_active_loan as u32,
        repaid_loans: 0,
        active_loans: has_active_loan as u32,
    }
}

fn seeded_loan(env: &Env, id: u32, borrower: &Address, status: LoanStatus) -> Loan {
    let now = env.ledger().timestamp();
    Loan {
        id,
        borrower: borrower.clone(),
        principal: 1_000,
        interest_rate: 500,
        total_repayment: 1_050,
        start_time: now,
        due_time: now + LOAN_DURATION,
        status,
        amount_repaid: 0,
        default_grace_period: 0,
        default_penalty_bps: 0,
    }
}

#[test]
fn seed_loans_rejects_active_loan_for_member_without_flag() {
    let s = setup(0);
    let borrower = Address::generate(&s.env);
    s.client.seed_members(
        &s.admin,
        &Vec::from_array(&s.env, [seeded_member(&s.env, &borrower, false)]),
    );

    let batch = Vec::from_array(
        &s.env,
        [seeded_loan(&s.env, 3, &borrower, LoanStatus::Active)],
    );
    assert_eq!(
        s.client.try_seed_loans(&s.admin, &batch),
        Err(Ok(Error::SeedLoanMemberMismatch))
    );
    // Caught before lock_seeding: nothing was persisted.
    assert_eq!(s.client.get_loan(&3), None);
}

#[test]
fn seed_loans_rejects_active_loan_seeded_before_its_member() {
    let s = setup(0);
    let borrower = Address::generate(&s.env);
    let batch = Vec::from_array(
        &s.env,
        [seeded_loan(&s.env, 3, &borrower, LoanStatus::Active)],
    );
    assert_eq!(
        s.client.try_seed_loans(&s.admin, &batch),
        Err(Ok(Error::SeedLoanMemberMismatch))
    );

    // Seeding the member first makes the same batch acceptable.
    s.client.seed_members(
        &s.admin,
        &Vec::from_array(&s.env, [seeded_member(&s.env, &borrower, true)]),
    );
    s.client.seed_loans(&s.admin, &batch);
    assert_eq!(s.client.get_loan(&3).unwrap().status, LoanStatus::Active);
}

#[test]
fn seed_loans_rejects_whole_batch_on_one_mismatch() {
    let s = setup(0);
    let good = Address::generate(&s.env);
    let bad = Address::generate(&s.env);
    s.client.seed_members(
        &s.admin,
        &Vec::from_array(
            &s.env,
            [
                seeded_member(&s.env, &good, true),
                seeded_member(&s.env, &bad, false),
            ],
        ),
    );
    let batch = Vec::from_array(
        &s.env,
        [
            seeded_loan(&s.env, 1, &good, LoanStatus::Active),
            seeded_loan(&s.env, 2, &bad, LoanStatus::Active),
        ],
    );
    assert_eq!(
        s.client.try_seed_loans(&s.admin, &batch),
        Err(Ok(Error::SeedLoanMemberMismatch))
    );
    assert_eq!(s.client.get_loan(&1), None);
}

#[test]
fn seed_loans_accepts_closed_loans_without_flag() {
    let s = setup(0);
    let borrower = Address::generate(&s.env);
    s.client.seed_members(
        &s.admin,
        &Vec::from_array(&s.env, [seeded_member(&s.env, &borrower, false)]),
    );
    let mut repaid = seeded_loan(&s.env, 4, &borrower, LoanStatus::Repaid);
    repaid.amount_repaid = repaid.total_repayment;
    let batch = Vec::from_array(
        &s.env,
        [
            repaid,
            seeded_loan(&s.env, 5, &borrower, LoanStatus::Defaulted),
        ],
    );
    s.client.seed_loans(&s.admin, &batch);
    s.client.lock_seeding(&s.admin);
    assert_eq!(s.client.get_loan(&4).unwrap().status, LoanStatus::Repaid);
}

#[test]
fn seed_stakes_yield_and_docs_restore_state_until_locked() {
    let s = setup(1);
    let member = s.members.get(0).unwrap();
    assert!(!s.client.is_seeding_locked());

    let stakes = Vec::from_array(
        &s.env,
        [SeedStakeEntry {
            address: member.clone(),
            amount: 300,
        }],
    );
    s.client.seed_stakes(&s.admin, &stakes);
    assert_eq!(s.client.get_stake(&member), 300);
    assert_eq!(s.client.get_dao_summary().total_staked, 300);

    let yields = Vec::from_array(
        &s.env,
        [SeedYieldEntry {
            address: member.clone(),
            amount: 40,
        }],
    );
    s.client.seed_pending_yield(&s.admin, &yields);
    assert_eq!(s.client.get_pending_yield(&member), 40);

    let hash = soroban_sdk::Bytes::from_array(&s.env, b"QmSeededDocument");
    let docs = Vec::from_array(
        &s.env,
        [
            SeedDocEntry {
                kind: ProposalKind::Loan,
                proposal_id: 4,
                content_hash: hash.clone(),
            },
            SeedDocEntry {
                kind: ProposalKind::Treasury,
                proposal_id: 2,
                content_hash: hash.clone(),
            },
        ],
    );
    s.client.seed_docs(&s.admin, &docs);
    assert_eq!(
        s.client.get_document(&ProposalKind::Loan, &4),
        Some(hash.clone())
    );
    assert_eq!(
        s.client.get_document(&ProposalKind::Treasury, &2),
        Some(hash)
    );
    // Seeded ids are reserved, so new proposals can't collide with them.
    assert_eq!(s.client.get_loan_proposal_count(), 5);
    assert_eq!(s.client.get_treasury_proposal_count(), 3);

    s.client.lock_seeding(&s.admin);
    assert!(s.client.is_seeding_locked());
}
