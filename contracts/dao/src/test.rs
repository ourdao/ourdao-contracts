#![cfg(test)]

use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{token, Address, Bytes, BytesN, Env, String, Vec};

use crate::privacy::compute_commitment;
use crate::storage::ProposalKind;
use crate::types::{LoanPolicy, LoanStatus, MemberStatus, ProposalStatus};
use crate::{Error, OurDao, OurDaoClient};

const FEE: i128 = 1_000;
const MINT: i128 = 1_000_000;
const EDITING: u64 = 3 * 24 * 60 * 60;

struct Setup<'a> {
    env: Env,
    client: OurDaoClient<'a>,
    token: token::Client<'a>,
    admin: Address,
    members: Vec<Address>,
}

fn policy() -> LoanPolicy {
    LoanPolicy {
        min_membership_duration: 0,
        membership_contribution: FEE,
        max_loan_duration: 30 * 24 * 60 * 60,
        min_interest_rate: 500,   // 5%
        max_interest_rate: 2_000, // 20%
        cooldown_period: 0,
        max_loan_to_treasury_ratio: 5_000, // 50%
    }
}

fn setup(num_members: u32) -> Setup<'static> {
    let env = Env::default();
    env.mock_all_auths();

    let token_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_id = sac.address();
    let token = token::Client::new(&env, &token_id);
    let token_mint = token::StellarAssetClient::new(&env, &token_id);

    let admin = Address::generate(&env);
    let contract_id = env.register(OurDao, ());
    let client = OurDaoClient::new(&env, &contract_id);

    let mut admins = Vec::new(&env);
    admins.push_back(admin.clone());
    client.initialize(&admins, &5_100u32, &FEE, &token_id, &policy());

    let mut members = Vec::new(&env);
    for _ in 0..num_members {
        let m = Address::generate(&env);
        token_mint.mint(&m, &MINT);
        client.register_member(&m);
        members.push_back(m);
    }

    Setup {
        env,
        client,
        token,
        admin,
        members,
    }
}

fn advance(env: &Env, secs: u64) {
    env.ledger().with_mut(|li| li.timestamp += secs);
}

// ---------------------------------------------------------------------------

#[test]
fn init_and_membership() {
    let s = setup(3);
    assert_eq!(s.client.get_total_members(), 3);
    assert_eq!(s.client.get_active_members(), 3);
    assert_eq!(s.client.get_treasury_balance(), 3 * FEE);
    assert!(s.client.is_member(&s.members.get(0).unwrap()));
    assert!(s.client.is_admin(&s.admin));
    assert_eq!(s.client.get_consensus_threshold(), 5_100);

    let m = s.client.get_member(&s.members.get(0).unwrap()).unwrap();
    assert_eq!(m.status, MemberStatus::ActiveMember);
    assert_eq!(m.contribution, FEE);
}

#[test]
fn double_join_rejected() {
    let s = setup(1);
    let m = s.members.get(0).unwrap();
    let res = s.client.try_register_member(&m);
    assert_eq!(res, Err(Ok(Error::AlreadyMember)));
}

#[test]
fn exit_returns_share() {
    let s = setup(2);
    let m = s.members.get(0).unwrap();
    let before = s.token.balance(&m);
    let share = s.client.calculate_exit_share(&m);
    assert!(share > 0);
    s.client.exit_dao(&m);
    assert_eq!(s.token.balance(&m), before + share);
    assert_eq!(s.client.get_active_members(), 1);
    assert!(!s.client.is_member(&m));
}

#[test]
fn full_loan_lifecycle() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    let terms = s.client.calculate_loan_terms(&1_000);
    assert!(terms.interest_rate >= 500 && terms.interest_rate <= 2_000);

    let pid = s.client.request_loan(&borrower, &1_000);
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

    let pid = s.client.request_loan(&borrower, &500);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);

    // Borrower now has an active loan; a second request must fail.
    let res = s.client.try_request_loan(&borrower, &200);
    assert_eq!(res, Err(Ok(Error::NotEligibleForLoan)));
}

#[test]
fn loan_exceeds_treasury_ratio() {
    let s = setup(3); // treasury = 3000, max ratio 50% => max loan 1500
    let borrower = s.members.get(0).unwrap();
    let res = s.client.try_request_loan(&borrower, &2_000);
    assert_eq!(res, Err(Ok(Error::ExceedsTreasuryRatio)));
}

#[test]
fn treasury_withdrawal_open_vote() {
    let s = setup(3);
    let proposer = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();
    let dest = Address::generate(&s.env);

    let reason = String::from_str(&s.env, "grant");
    let pid = s
        .client
        .propose_treasury_withdrawal(&proposer, &600, &dest, &reason, &false);

    s.client.vote_on_treasury_proposal(&v1, &pid, &true);
    let mid = s.client.get_treasury_proposal(&pid).unwrap();
    assert_eq!(mid.status, ProposalStatus::Pending); // 1 vote not enough (needs 2)

    s.client.vote_on_treasury_proposal(&v2, &pid, &true);
    let done = s.client.get_treasury_proposal(&pid).unwrap();
    assert_eq!(done.status, ProposalStatus::Executed);
    assert_eq!(s.token.balance(&dest), 600);
}

#[test]
fn staking_boosts_voting_weight() {
    let s = setup(2); // required for loan = ceil(2*51%) = 2
    let borrower = s.members.get(0).unwrap();
    let staker = s.members.get(1).unwrap();

    // Stake enough for +2 weight (200 / 100). One staked yes-vote = weight 3 >= 2.
    s.client.stake(&staker, &200);
    assert_eq!(s.client.get_stake(&staker), 200);

    let pid = s.client.request_loan(&borrower, &500);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&staker, &pid, &true);

    let prop = s.client.get_loan_proposal(&pid).unwrap();
    assert_eq!(prop.for_votes, 3);
    assert_eq!(prop.status, ProposalStatus::Approved);

    // Unstake returns tokens.
    let before = s.token.balance(&staker);
    s.client.unstake(&staker, &200);
    assert_eq!(s.token.balance(&staker), before + 200);
    assert_eq!(s.client.get_stake(&staker), 0);
}

#[test]
fn name_registry() {
    let s = setup(1);
    let owner = s.members.get(0).unwrap();
    let name = String::from_str(&s.env, "alice.dao");
    s.client.register_name(&owner, &name);
    assert_eq!(s.client.resolve_name(&name), Some(owner.clone()));
    assert_eq!(s.client.name_of(&owner), Some(name.clone()));

    // A different owner cannot claim the same name.
    let other = Address::generate(&s.env);
    let res = s.client.try_register_name(&other, &name);
    assert_eq!(res, Err(Ok(Error::NameTaken)));
}

#[test]
fn commit_reveal_private_treasury_vote() {
    let s = setup(3);
    let proposer = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();
    let dest = Address::generate(&s.env);

    let reason = String::from_str(&s.env, "secret grant");
    let pid = s
        .client
        .propose_treasury_withdrawal(&proposer, &600, &dest, &reason, &true);

    // Open voting is refused on a private proposal.
    let open = s.client.try_vote_on_treasury_proposal(&v1, &pid, &true);
    assert_eq!(open, Err(Ok(Error::NotAuthorized)));

    let salt1 = BytesN::from_array(&s.env, &[7u8; 32]);
    let salt2 = BytesN::from_array(&s.env, &[9u8; 32]);
    let c1 = compute_commitment(&s.env, true, &salt1);
    let c2 = compute_commitment(&s.env, true, &salt2);

    s.client.commit_treasury_vote(&v1, &pid, &c1);
    s.client.commit_treasury_vote(&v2, &pid, &c2);

    // A reveal that doesn't match the commitment is rejected.
    let bad = s.client.try_reveal_treasury_vote(&v1, &pid, &false, &salt1);
    assert_eq!(bad, Err(Ok(Error::CommitmentMismatch)));

    s.client.reveal_treasury_vote(&v1, &pid, &true, &salt1);
    assert_eq!(
        s.client.get_treasury_proposal(&pid).unwrap().status,
        ProposalStatus::Pending
    );
    s.client.reveal_treasury_vote(&v2, &pid, &true, &salt2);
    assert_eq!(
        s.client.get_treasury_proposal(&pid).unwrap().status,
        ProposalStatus::Executed
    );
    assert_eq!(s.token.balance(&dest), 600);
}

#[test]
fn content_hash_document() {
    let s = setup(1);
    let member = s.members.get(0).unwrap();
    let pid = s.client.request_loan(&member, &500);

    let cid = Bytes::from_array(&s.env, b"QmExampleCid1234567890");
    s.client
        .attach_document(&member, &ProposalKind::Loan, &pid, &cid);
    assert_eq!(s.client.get_document(&ProposalKind::Loan, &pid), Some(cid));
}

#[test]
fn pause_blocks_state_changes() {
    let s = setup(1);
    s.client.pause(&s.admin);
    assert!(s.client.is_paused());

    let newcomer = Address::generate(&s.env);
    let res = s.client.try_register_member(&newcomer);
    assert_eq!(res, Err(Ok(Error::Paused)));

    s.client.unpause(&s.admin);
    assert!(!s.client.is_paused());
}

#[test]
fn only_admin_governs() {
    let s = setup(1);
    let intruder = s.members.get(0).unwrap();
    let res = s.client.try_set_consensus_threshold(&intruder, &7_000);
    assert_eq!(res, Err(Ok(Error::NotAdmin)));

    s.client.set_consensus_threshold(&s.admin, &7_000);
    assert_eq!(s.client.get_consensus_threshold(), 7_000);
}

#[test]
fn cannot_remove_last_admin() {
    let s = setup(0);
    let res = s.client.try_remove_admin(&s.admin, &s.admin);
    assert_eq!(res, Err(Ok(Error::CannotRemoveLastAdmin)));
}
