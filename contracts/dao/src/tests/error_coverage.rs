//! Error variant coverage: each test reaches its named `Error` variant
//! through the public contract API.
extern crate std;

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, BytesN, String, Vec};

use crate::error::Error;
use crate::privacy::compute_commitment;
use crate::types::LoanPolicy;
use crate::{OurDao, OurDaoClient};

use super::common::{
    advance, policy, setup, whitelist_second_asset, DAOBuilder, Setup, EDITING, FEE, LOAN_DURATION,
    VOTING_PERIOD,
};

/// A private treasury proposal by member 0, with member 1's commitment to a
/// `true` vote already recorded. Returns `(proposal_id, voter, salt)`.
fn committed_private_vote(s: &Setup) -> (u32, Address, BytesN<32>) {
    let proposer = s.members.get(0).unwrap();
    let voter = s.members.get(1).unwrap();
    let pid = s.client.propose_treasury_withdrawal(
        &proposer,
        &100,
        &Address::generate(&s.env),
        &String::from_str(&s.env, "private grant"),
        &true,
    );
    let salt = BytesN::from_array(&s.env, &[7u8; 32]);
    let commitment = compute_commitment(&s.env, &voter, pid, true, &salt);
    s.client.commit_treasury_vote(&voter, &pid, &commitment);
    (pid, voter, salt)
}

/// A disbursed, active loan of 100 to member 0, approved by members 1 and 2
/// (two of three votes meet the 51% threshold). Returns `(borrower, loan_id)`.
fn active_loan(s: &Setup) -> (Address, u32) {
    let borrower = s.members.get(0).unwrap();
    let loan_id = s.client.request_loan(&borrower, &100i128, &None);
    advance(&s.env, EDITING + 1);
    s.client
        .vote_on_loan_proposal(&s.members.get(1).unwrap(), &loan_id, &true);
    s.client
        .vote_on_loan_proposal(&s.members.get(2).unwrap(), &loan_id, &true);
    (borrower, loan_id)
}

// AlreadyInitialized = 1
#[test]
fn error_already_initialized() {
    let s = setup(1);
    let token = s.token.address;
    let admin = s.admin.clone();

    let mut admins = Vec::new(&s.env);
    admins.push_back(admin);

    let result = s
        .client
        .try_initialize(&admins, &5_100u32, &FEE, &token, &policy());
    assert_eq!(result, Err(Ok(Error::AlreadyInitialized)));
}

// NotInitialized = 2
#[test]
fn error_not_initialized() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();

    let contract_id = env.register(OurDao, ());
    let client = OurDaoClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    let result = client.try_register_member(&admin);
    assert_eq!(result, Err(Ok(Error::NotInitialized)));
}

// InvalidThreshold = 3
#[test]
fn error_invalid_threshold() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();

    let token_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_admin);
    let token_id = sac.address();

    let admin = Address::generate(&env);
    let contract_id = env.register(OurDao, ());
    let client = OurDaoClient::new(&env, &contract_id);

    let mut admins = Vec::new(&env);
    admins.push_back(admin);

    let pol = policy();
    let result = client.try_initialize(&admins, &0u32, &FEE, &token_id, &pol);
    assert_eq!(result, Err(Ok(Error::InvalidThreshold)));
}

// InvalidAmount = 4
#[test]
fn error_invalid_amount() {
    let s = setup(1);
    let result = s
        .client
        .try_request_loan(&s.members.get(0).unwrap(), &0i128, &None);
    assert_eq!(result, Err(Ok(Error::InvalidAmount)));
}

// InvalidLoanPolicy = 5
#[test]
fn error_invalid_loan_policy() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();

    let token_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_admin);
    let token_id = sac.address();

    let admin = Address::generate(&env);
    let contract_id = env.register(OurDao, ());
    let client = OurDaoClient::new(&env, &contract_id);

    let mut admins = Vec::new(&env);
    admins.push_back(admin);

    let mut pol = policy();
    pol.min_interest_rate = 3_000;
    pol.max_interest_rate = 1_000;

    let result = client.try_initialize(&admins, &5_100u32, &FEE, &token_id, &pol);
    assert_eq!(result, Err(Ok(Error::InvalidLoanPolicy)));
}

// Paused = 6
#[test]
fn error_paused() {
    let s = setup(1);
    s.client.pause(&s.admin);

    let result = s.client.try_register_member(&Address::generate(&s.env));
    assert_eq!(result, Err(Ok(Error::Paused)));
}

// NotPaused = 7
#[test]
fn error_not_paused() {
    let s = setup(1);
    let result = s.client.try_unpause(&s.admin);
    assert_eq!(result, Err(Ok(Error::NotPaused)));
}

// NotAuthorized = 10
#[test]
fn error_not_authorized() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let token_id = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    let contract_id = env.register(OurDao, ());
    let client = OurDaoClient::new(&env, &contract_id);

    // An empty admin list is refused.
    let result = client.try_initialize(&Vec::new(&env), &5_100u32, &FEE, &token_id, &policy());
    assert_eq!(result, Err(Ok(Error::NotAuthorized)));
}

// NotAdmin = 11
#[test]
fn error_not_admin() {
    let s = setup(2);
    let member = s.members.get(0).unwrap().clone();

    let result = s.client.try_pause(&member);
    assert_eq!(result, Err(Ok(Error::NotAdmin)));
}

// NotMember = 12
#[test]
fn error_not_member() {
    let s = setup(0);
    let non_member = Address::generate(&s.env);

    let result = s.client.try_request_loan(&non_member, &100i128, &None);
    assert_eq!(result, Err(Ok(Error::NotMember)));
}

// AlreadyAdmin = 13
#[test]
fn error_already_admin() {
    let s = setup(1);
    let result = s.client.try_add_admin(&s.admin, &s.admin);
    assert_eq!(result, Err(Ok(Error::AlreadyAdmin)));
}

// AlreadyMember = 14
#[test]
fn error_already_member() {
    let s = setup(1);
    let member = s.members.get(0).unwrap().clone();
    let result = s.client.try_register_member(&member);
    assert_eq!(result, Err(Ok(Error::AlreadyMember)));
}

// CannotRemoveLastAdmin = 15
#[test]
fn error_cannot_remove_last_admin() {
    let s = setup(0);
    let result = s.client.try_remove_admin(&s.admin, &s.admin);
    assert_eq!(result, Err(Ok(Error::CannotRemoveLastAdmin)));
}

// MemberNotActive = 20
#[test]
fn error_member_not_active() {
    let s = setup(1);
    let member = s.members.get(0).unwrap().clone();

    s.client.exit_dao(&member);

    let result = s.client.try_request_loan(&member, &100i128, &None);
    assert_eq!(result, Err(Ok(Error::MemberNotActive)));
}

// HasActiveLoan = 21
#[test]
fn error_has_active_loan() {
    let s = setup(3);
    let (borrower, _) = active_loan(&s);

    let result = s.client.try_request_loan(&borrower, &100i128, &None);
    assert_eq!(result, Err(Ok(Error::HasActiveLoan)));
}

// ProposalNotFound = 30
#[test]
fn error_proposal_not_found() {
    let s = setup(1);
    let result = s
        .client
        .try_vote_on_loan_proposal(&s.members.get(0).unwrap(), &9999u32, &true);
    assert_eq!(result, Err(Ok(Error::ProposalNotFound)));
}

// NotBorrower = 31
#[test]
fn error_not_borrower() {
    let s = setup(2);
    let borrower = s.members.get(0).unwrap().clone();
    let non_borrower = s.members.get(1).unwrap().clone();

    let proposal_id = s.client.request_loan(&borrower, &100i128, &None);

    let result = s
        .client
        .try_cancel_loan_proposal(&non_borrower, &proposal_id);
    assert_eq!(result, Err(Ok(Error::NotBorrower)));
}

// NotInEditingPhase = 32
#[test]
fn error_not_in_editing_phase() {
    let s = setup(1);
    let borrower = s.members.get(0).unwrap().clone();

    let proposal_id = s.client.request_loan(&borrower, &100i128, &None);

    advance(&s.env, EDITING + 1);
    let result = s.client.try_edit_loan_proposal(
        &borrower,
        &proposal_id,
        &500,
        &Some(String::from_str(&s.env, "new_cid")),
    );
    assert_eq!(result, Err(Ok(Error::NotInEditingPhase)));
}

// NotInVotingPhase = 33
#[test]
fn error_not_in_voting_phase() {
    let s = setup(2);
    let borrower = s.members.get(0).unwrap();
    let voter = s.members.get(1).unwrap();

    // Still inside the editing period.
    let proposal_id = s.client.request_loan(&borrower, &100i128, &None);

    let result = s
        .client
        .try_vote_on_loan_proposal(&voter, &proposal_id, &true);
    assert_eq!(result, Err(Ok(Error::NotInVotingPhase)));
}

// VotingEnded = 34
#[test]
fn error_voting_ended() {
    let s = setup(2);
    let borrower = s.members.get(0).unwrap().clone();
    let voter = s.members.get(1).unwrap().clone();

    let proposal_id = s.client.request_loan(&borrower, &100i128, &None);

    advance(&s.env, VOTING_PERIOD + EDITING + 1);

    let result = s
        .client
        .try_vote_on_loan_proposal(&voter, &proposal_id, &true);
    assert_eq!(result, Err(Ok(Error::VotingEnded)));
}

// AlreadyVoted = 35
#[test]
fn error_already_voted() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap().clone();
    let voter = s.members.get(1).unwrap().clone();

    let proposal_id = s.client.request_loan(&borrower, &100i128, &None);

    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&voter, &proposal_id, &true);

    let result = s
        .client
        .try_vote_on_loan_proposal(&voter, &proposal_id, &false);
    assert_eq!(result, Err(Ok(Error::AlreadyVoted)));
}

// NotEligibleForLoan = 36
#[test]
fn error_not_eligible_for_loan() {
    let policy_with_min_duration = LoanPolicy {
        min_membership_duration: 1000u64,
        ..policy()
    };

    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let s = DAOBuilder::new()
        .with_members(1)
        .with_policy(policy_with_min_duration)
        .with_env(env)
        .build();

    let member = s.members.get(0).unwrap().clone();
    let result = s.client.try_request_loan(&member, &100i128, &None);
    assert_eq!(result, Err(Ok(Error::NotEligibleForLoan)));
}

// CooldownActive = 37
#[test]
fn error_cooldown_active() {
    let s = DAOBuilder::new()
        .with_members(3)
        .with_policy(LoanPolicy {
            cooldown_period: 1000u64,
            ..policy()
        })
        .build();
    let (borrower, loan_id) = active_loan(&s);
    s.client.repay_loan(&borrower, &loan_id);

    let result = s.client.try_request_loan(&borrower, &100i128, &None);
    assert_eq!(result, Err(Ok(Error::CooldownActive)));
}

// LoanNotFound = 38
#[test]
fn error_loan_not_found() {
    let s = setup(1);
    let result = s
        .client
        .try_repay_loan(&s.members.get(0).unwrap(), &9999u32);
    assert_eq!(result, Err(Ok(Error::LoanNotFound)));
}

// LoanNotActive = 39
#[test]
fn error_loan_not_active() {
    let s = setup(3);
    let (borrower, loan_id) = active_loan(&s);
    s.client.repay_loan(&borrower, &loan_id);

    let result = s.client.try_repay_loan(&borrower, &loan_id);
    assert_eq!(result, Err(Ok(Error::LoanNotActive)));
}

// ExceedsTreasuryRatio = 40
#[test]
fn error_exceeds_treasury_ratio() {
    let s = setup(1);
    let borrower = s.members.get(0).unwrap().clone();

    let result = s
        .client
        .try_request_loan(&borrower, &1_000_000_000i128, &None);
    assert_eq!(result, Err(Ok(Error::ExceedsTreasuryRatio)));
}

// InsufficientTreasury = 41
#[test]
fn error_insufficient_treasury() {
    // Four members: treasury 4,000, so a 1,500 loan passes the 50% ratio cap.
    let s = setup(4);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();
    let v3 = s.members.get(3).unwrap();
    let proposal_id = s.client.request_loan(&borrower, &1_500, &None);

    // A withdrawal drains the treasury to 1,000 before the loan vote passes.
    let tid = s.client.propose_treasury_withdrawal(
        &v1,
        &3_000,
        &Address::generate(&s.env),
        &String::from_str(&s.env, "drain"),
        &false,
    );
    for voter in [&v1, &v2, &v3] {
        s.client.vote_on_treasury_proposal(voter, &tid, &true);
    }

    advance(&s.env, EDITING + 1);
    for voter in [v1, v2, v3] {
        s.client.vote_on_loan_proposal(&voter, &proposal_id, &true);
    }

    let result = s.client.try_disburse_approved_loan(&proposal_id);
    assert_eq!(result, Err(Ok(Error::InsufficientTreasury)));
}

// LoanNotOverdue = 42
#[test]
fn error_loan_not_overdue() {
    let s = setup(3);
    let (_, loan_id) = active_loan(&s);

    let result = s.client.try_mark_loan_defaulted(&loan_id);
    assert_eq!(result, Err(Ok(Error::LoanNotOverdue)));
}

// TreasuryProposalNotFound = 50
#[test]
fn error_treasury_proposal_not_found() {
    let s = setup(1);
    let result =
        s.client
            .try_vote_on_treasury_proposal(&s.members.get(0).unwrap(), &9999u32, &true);
    assert_eq!(result, Err(Ok(Error::TreasuryProposalNotFound)));
}

// NameTaken = 60
#[test]
fn error_name_taken() {
    let s = setup(2);
    let name = String::from_str(&s.env, "test_name");
    s.client.register_name(&s.members.get(0).unwrap(), &name);

    let result = s
        .client
        .try_register_name(&s.members.get(1).unwrap(), &name);
    assert_eq!(result, Err(Ok(Error::NameTaken)));
}

// NameNotFound = 61
#[test]
fn error_name_not_found() {
    let s = setup(1);
    let member = s.members.get(0).unwrap().clone();
    let result = s.client.try_release_name(&member);
    assert_eq!(result, Err(Ok(Error::NameNotFound)));
}

// NoStake = 62
#[test]
fn error_no_stake() {
    let s = setup(1);
    let member = s.members.get(0).unwrap();

    let result = s.client.try_unstake(&member, &1);
    assert_eq!(result, Err(Ok(Error::NoStake)));
}

// InsufficientStake = 63
#[test]
fn error_insufficient_stake() {
    let s = setup(1);
    let member = s.members.get(0).unwrap();

    s.client.stake(&member, &1);

    let result = s.client.try_unstake(&member, &2);
    assert_eq!(result, Err(Ok(Error::InsufficientStake)));
}

// NoCommitment = 64
#[test]
fn error_no_commitment() {
    let s = setup(3);
    let (pid, _, salt) = committed_private_vote(&s);
    let uncommitted = s.members.get(2).unwrap();
    advance(&s.env, VOTING_PERIOD + 1);

    let result = s
        .client
        .try_reveal_treasury_vote(&uncommitted, &pid, &true, &salt);
    assert_eq!(result, Err(Ok(Error::NoCommitment)));
}

// CommitmentMismatch = 65
#[test]
fn error_commitment_mismatch() {
    let s = setup(3);
    let (pid, voter, salt) = committed_private_vote(&s);
    advance(&s.env, VOTING_PERIOD + 1);

    let result = s
        .client
        .try_reveal_treasury_vote(&voter, &pid, &false, &salt);
    assert_eq!(result, Err(Ok(Error::CommitmentMismatch)));
}

// AlreadyRevealed = 66
#[test]
fn error_already_revealed() {
    let s = setup(3);
    let (pid, voter, salt) = committed_private_vote(&s);
    advance(&s.env, VOTING_PERIOD + 1);
    s.client.reveal_treasury_vote(&voter, &pid, &true, &salt);

    let result = s
        .client
        .try_reveal_treasury_vote(&voter, &pid, &true, &salt);
    assert_eq!(result, Err(Ok(Error::AlreadyRevealed)));
}

// NothingToClaim = 67
#[test]
fn error_nothing_to_claim() {
    let s = setup(1);
    let result = s.client.try_claim_rewards(&s.members.get(0).unwrap());
    assert_eq!(result, Err(Ok(Error::NothingToClaim)));
}

// ProposalNotExpired = 70
#[test]
fn error_proposal_not_expired() {
    let s = setup(1);
    let borrower = s.members.get(0).unwrap().clone();

    let proposal_id = s.client.request_loan(&borrower, &100i128, &None);

    let result = s.client.try_expire_loan_proposal(&proposal_id);
    assert_eq!(result, Err(Ok(Error::ProposalNotExpired)));
}

// InvalidName = 71
#[test]
fn error_invalid_name() {
    let s = setup(1);
    let result = s
        .client
        .try_register_name(&s.members.get(0).unwrap(), &String::from_str(&s.env, ""));
    assert_eq!(result, Err(Ok(Error::InvalidName)));
}

// NotYetRevealed = 72
#[test]
fn error_not_yet_revealed() {
    let s = setup(3);
    let (pid, voter, salt) = committed_private_vote(&s);

    let result = s
        .client
        .try_reveal_treasury_vote(&voter, &pid, &true, &salt);
    assert_eq!(result, Err(Ok(Error::NotYetRevealed)));
}

// NotProposalOwner = 73
#[test]
fn error_not_proposal_owner() {
    let s = setup(2);
    let borrower = s.members.get(0).unwrap();
    let other = s.members.get(1).unwrap();

    let proposal_id = s.client.request_loan(&borrower, &100i128, &None);

    let result = s.client.try_attach_document(
        &other,
        &crate::storage::ProposalKind::Loan,
        &proposal_id,
        &soroban_sdk::Bytes::from_array(&s.env, b"QmExampleCid"),
    );
    assert_eq!(result, Err(Ok(Error::NotProposalOwner)));
}

// DocumentTooLarge = 74
#[test]
fn error_document_too_large() {
    let s = setup(1);
    let member = s.members.get(0).unwrap().clone();

    let large_doc = String::from_str(&s.env, &"x".repeat(100_000));

    let result = s
        .client
        .try_request_loan(&member, &100i128, &Some(large_doc));
    assert_eq!(result, Err(Ok(Error::DocumentTooLarge)));
}

// InvalidToken = 75
#[test]
fn error_invalid_token() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let contract_id = env.register(OurDao, ());
    let client = OurDaoClient::new(&env, &contract_id);

    let invalid_token = Address::generate(&env);

    let mut admins = Vec::new(&env);
    admins.push_back(admin);

    let result = client.try_initialize(&admins, &5_100u32, &FEE, &invalid_token, &policy());
    assert_eq!(result, Err(Ok(Error::InvalidToken)));
}

// TimelockNotExpired = 76
#[test]
fn error_timelock_not_expired() {
    let s = setup(0);

    let new_policy = policy();
    s.client.propose_policy_update(&s.admin, &new_policy);

    let result = s.client.try_execute_policy_update(&s.admin);
    assert_eq!(result, Err(Ok(Error::TimelockNotExpired)));
}

// NoPendingPolicy = 77
#[test]
fn error_no_pending_policy() {
    let s = setup(0);

    let result = s.client.try_execute_policy_update(&s.admin);
    assert_eq!(result, Err(Ok(Error::NoPendingPolicy)));
}

// InvalidMetadataCid = 78
#[test]
fn error_invalid_metadata_cid() {
    let s = setup(1);
    let result = s.client.try_request_loan(
        &s.members.get(0).unwrap(),
        &100i128,
        &Some(String::from_str(&s.env, "")),
    );
    assert_eq!(result, Err(Ok(Error::InvalidMetadataCid)));
}

// InvalidDelegation = 79
#[test]
fn error_invalid_delegation() {
    let s = setup(1);
    let member = s.members.get(0).unwrap().clone();

    let result = s.client.try_delegate_vote(&member, &member);
    assert_eq!(result, Err(Ok(Error::InvalidDelegation)));
}

// InvalidProposal = 80
#[test]
fn error_invalid_proposal() {
    let s = setup(1);
    // A withdrawal must give a reason.
    let result = s.client.try_propose_treasury_withdrawal(
        &s.members.get(0).unwrap(),
        &100,
        &Address::generate(&s.env),
        &String::from_str(&s.env, ""),
        &false,
    );
    assert_eq!(result, Err(Ok(Error::InvalidProposal)));
}

// AlreadyPauser = 81
#[test]
fn error_already_pauser() {
    let s = setup(0);
    s.client.set_pauser(&s.admin, &s.admin);

    let result = s.client.try_set_pauser(&s.admin, &s.admin);
    assert_eq!(result, Err(Ok(Error::AlreadyPauser)));
}

// NotPauser = 82
#[test]
fn error_not_pauser() {
    let s = setup(1);

    let result = s.client.try_revoke_pauser(&s.admin);
    assert_eq!(result, Err(Ok(Error::NotPauser)));
}

// TokenNotWhitelisted = 83
#[test]
fn error_token_not_whitelisted() {
    let s = setup(1);
    let member = s.members.get(0).unwrap();
    let unwhitelisted_token = Address::generate(&s.env);

    let result = s
        .client
        .try_request_loan_in_asset(&member, &100, &unwhitelisted_token, &None);
    assert_eq!(result, Err(Ok(Error::TokenNotWhitelisted)));
}

// TokenAlreadyWhitelisted = 84
#[test]
fn error_token_already_whitelisted() {
    let s = setup(0);
    let base_token = s.token.address;

    let result = s.client.try_propose_token_addition(&s.admin, &base_token);
    assert_eq!(result, Err(Ok(Error::TokenAlreadyWhitelisted)));
}

// NoPendingTokenAddition = 85
#[test]
fn error_no_pending_token_addition() {
    let s = setup(0);
    let token = Address::generate(&s.env);

    let result = s.client.try_execute_token_addition(&s.admin, &token);
    assert_eq!(result, Err(Ok(Error::NoPendingTokenAddition)));
}

// AssetMismatch = 86
#[test]
fn error_asset_mismatch() {
    let s = setup(3);
    let (borrower, loan_id) = active_loan(&s);
    let wrong_token = Address::generate(&s.env);

    let result = s
        .client
        .try_repay_loan_in_asset(&borrower, &loan_id, &wrong_token);
    assert_eq!(result, Err(Ok(Error::AssetMismatch)));
}

// CannotRemoveBaseToken = 87
#[test]
fn error_cannot_remove_base_token() {
    let s = setup(0);
    let base_token = s.token.address;

    let result = s.client.try_remove_token(&s.admin, &base_token);
    assert_eq!(result, Err(Ok(Error::CannotRemoveBaseToken)));
}

// TokenHasBalance = 92
#[test]
fn error_token_has_balance() {
    let s = setup(0);
    let funded = whitelist_second_asset(&s, 1);

    let result = s.client.try_remove_token(&s.admin, &funded.address);
    assert_eq!(result, Err(Ok(Error::TokenHasBalance)));
}

// TokenHasActiveLoans = 93
#[test]
fn error_token_has_active_loans() {
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

    let treasury_balance = asset.balance(&s.client.address);
    soroban_sdk::token::StellarAssetClient::new(&s.env, &asset.address)
        .burn(&s.client.address, &treasury_balance);
    let result = s.client.try_remove_token(&s.admin, &asset.address);
    assert_eq!(result, Err(Ok(Error::TokenHasActiveLoans)));
}

// SeedingLocked = 88
#[test]
fn error_seeding_locked() {
    let s = setup(0);
    s.client.lock_seeding(&s.admin);

    let result = s.client.try_seed_members(&s.admin, &Vec::new(&s.env));
    assert_eq!(result, Err(Ok(Error::SeedingLocked)));
}

// SeedingAlreadyLocked = 89
#[test]
fn error_seeding_already_locked() {
    let s = setup(0);
    s.client.lock_seeding(&s.admin);

    let result = s.client.try_lock_seeding(&s.admin);
    assert_eq!(result, Err(Ok(Error::SeedingAlreadyLocked)));
}

// TooManyOpenProposals = 90
#[test]
fn error_too_many_open_proposals() {
    let s = setup(1);
    let borrower = s.members.get(0).unwrap();

    for _ in 0..crate::storage::MAX_OPEN_PROPOSALS_PER_MEMBER {
        s.client.request_loan(&borrower, &100i128, &None);
    }

    let result = s.client.try_request_loan(&borrower, &100i128, &None);
    assert_eq!(result, Err(Ok(Error::TooManyOpenProposals)));
}

// BorrowerCannotVote = 91
#[test]
fn error_borrower_cannot_vote() {
    let s = setup(2);
    let borrower = s.members.get(0).unwrap().clone();

    let proposal_id = s.client.request_loan(&borrower, &100i128, &None);

    advance(&s.env, EDITING + 1);

    let result = s
        .client
        .try_vote_on_loan_proposal(&borrower, &proposal_id, &true);
    assert_eq!(result, Err(Ok(Error::BorrowerCannotVote)));
}

// DefaultedLoanOutstanding = 94
#[test]
fn error_defaulted_loan_outstanding() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();
    let proposal_id = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &proposal_id, &true);
    s.client.vote_on_loan_proposal(&v2, &proposal_id, &true);
    advance(&s.env, LOAN_DURATION + 1);
    s.client.mark_loan_defaulted(&proposal_id);
    s.client.exit_dao(&borrower);

    let result = s.client.try_register_member(&borrower);
    assert_eq!(result, Err(Ok(Error::DefaultedLoanOutstanding)));
}
