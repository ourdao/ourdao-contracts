#![allow(unused_imports)]

use soroban_sdk::testutils::{Address as _, Events as _};
use soroban_sdk::xdr::{ContractEventBody, ScVal};
use soroban_sdk::{Address, Env, Vec};

use super::super::common::*;
use crate::types::ProposalStatus;
use crate::util::{
    isqrt, stake_boost, voting_weight, BASE_VOTE_WEIGHT, MAX_STAKE_BONUS, STAKE_WEIGHT_UNIT,
};
use crate::Error;

#[test]
fn staking_boosts_voting_weight() {
    let s = setup(2); // required for loan = ceil(2*51%) = 2
    let borrower = s.members.get(0).unwrap();
    let staker = s.members.get(1).unwrap();

    // Quadratic curve: 2 bonus votes cost 2^2 * 100 = 400 staked tokens
    // (#182). One staked yes-vote = weight 3 >= 2.
    s.client.stake(&staker, &400);
    assert_eq!(s.client.get_stake(&staker), 400);

    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&staker, &pid, &true);

    let prop = s.client.get_loan_proposal(&pid).unwrap();
    assert_eq!(prop.for_votes, 3);
    assert_eq!(prop.status, ProposalStatus::Approved);

    // Unstake returns tokens.
    let before = s.token.balance(&staker);
    s.client.unstake(&staker, &400);
    assert_eq!(s.token.balance(&staker), before + 400);
    assert_eq!(s.client.get_stake(&staker), 0);
}

#[test]
fn rejected_stake_transfer_leaves_stake_storage_unchanged() {
    let s = rejecting_setup(1);
    let member = s.members.get(0).unwrap();
    let dao_balance_before = s.token.balance(&s.client.address);

    s.token.set_reject_transfers(&true);
    let result = s.client.try_stake(&member, &500);
    assert!(result.is_err());

    assert_eq!(s.client.get_stake(&member), 0);
    let total_staked = s.env.as_contract(&s.client.address, || {
        crate::storage::get_total_staked(&s.env)
    });
    assert_eq!(total_staked, 0);
    assert_eq!(s.token.balance(&s.client.address), dao_balance_before);
    let has_stake_time = s.env.as_contract(&s.client.address, || {
        s.env
            .storage()
            .persistent()
            .has(&crate::storage::DataKey::StakeTime(member.clone()))
    });
    assert!(
        !has_stake_time,
        "stake timestamp must roll back with the rejected transfer"
    );
}

#[test]
fn rejected_unstake_transfer_leaves_stake_storage_unchanged() {
    let s = rejecting_setup(1);
    let member = s.members.get(0).unwrap();

    // Stake succeeds while transfers are allowed.
    s.client.stake(&member, &500);
    assert_eq!(s.client.get_stake(&member), 500);

    // Reject the payout leg: counters must stay in sync with the vault.
    s.token.set_reject_transfers(&true);
    let result = s.client.try_unstake(&member, &500);
    assert!(result.is_err());

    assert_eq!(s.client.get_stake(&member), 500);
    let total_staked = s.env.as_contract(&s.client.address, || {
        crate::storage::get_total_staked(&s.env)
    });
    assert_eq!(total_staked, 500);
}

// Issue #193: StakingRewardClaimed event on yield distribution
#[test]
fn claim_rewards_emits_staking_reward_claimed_event_and_updates_snapshot() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    let pid = s.client.request_loan(&borrower, &1_000, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);
    s.client.repay_loan(&borrower, &0);

    let loan = s.client.get_loan(&0).unwrap();
    let interest = loan.total_repayment - loan.principal;
    let expected_share = interest / 3;
    assert!(expected_share > 0);

    assert_eq!(s.client.get_pending_yield(&v1), expected_share);

    // Claim rewards
    let claimed = s.client.claim_rewards(&v1);
    assert_eq!(claimed, expected_share);

    // Verify event payload and topics
    assert!(emitted(&s.env, "claimed"));

    // Find the claimed event in event log
    let all_events = s.env.events().all();
    let events_vec = all_events.events();
    let event = events_vec
        .iter()
        .rev()
        .find(|e| {
            let ContractEventBody::V0(body) = &e.body;
            matches!(body.topics.first(), Some(ScVal::Symbol(sym)) if sym.0.to_utf8_string_lossy() == "claimed")
        })
        .expect("claimed event not found");

    let ContractEventBody::V0(body) = &event.body;
    assert_eq!(body.topics.len(), 3);
    // Topic 0: symbol "claimed"
    match &body.topics[0] {
        ScVal::Symbol(sym) => assert_eq!(sym.0.to_utf8_string_lossy(), "claimed"),
        _ => panic!("unexpected topic 0"),
    }
    // Verify topic 2 has the claimed amount
    match &body.topics[2] {
        ScVal::I128(amount) => {
            let val = ((amount.hi as i128) << 64) | (amount.lo as i128);
            assert_eq!(val, expected_share);
        }
        _ => panic!("unexpected topic 2"),
    }

    // Verify accumulator snapshot updated on member record
    assert_eq!(s.client.get_pending_yield(&v1), 0);
    assert_eq!(
        s.client.try_claim_rewards(&v1),
        Err(Ok(Error::NothingToClaim))
    );
}

// ===========================================================================
// Issue #182: quadratic (square-root) staking boost
// ===========================================================================

#[test]
fn unstake_rejected_while_proposal_in_voting() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();

    s.client.stake(&borrower, &200);
    let pid = s.client.request_loan(&borrower, &500, &None);

    advance(&s.env, EDITING + 1);
    let prop = s.client.get_loan_proposal(&pid).unwrap();
    assert_eq!(prop.phase, crate::types::ProposalPhase::Voting);

    let res = s.client.try_unstake(&borrower, &200);
    assert_eq!(
        res,
        Err(Ok(crate::Error::HasActiveLoan)),
        "should not be able to unstake with an active pending proposal"
    );
}

#[test]
fn unstake_rejected_while_proposal_in_editing() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();

    s.client.stake(&borrower, &200);
    let _pid = s.client.request_loan(&borrower, &500, &None);

    let res = s.client.try_unstake(&borrower, &200);
    assert_eq!(
        res,
        Err(Ok(crate::Error::HasActiveLoan)),
        "should not be able to unstake with a proposal in editing phase"
    );
}

#[test]
fn unstake_allowed_after_proposal_approved() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    s.client.stake(&borrower, &200);
    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);

    let prop = s.client.get_loan_proposal(&pid).unwrap();
    assert_eq!(prop.status, ProposalStatus::Approved);

    // Advance past the stake cooldown (voting_period).
    advance(&s.env, VOTING_PERIOD + 1);

    s.client.unstake(&borrower, &200);
    assert_eq!(s.client.get_stake(&borrower), 0);
}

#[test]
fn unstake_allowed_after_proposal_expired() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();

    s.client.stake(&borrower, &200);
    let _pid = s.client.request_loan(&borrower, &500, &None);

    // Advance well past the voting deadline so the proposal expires.
    advance(&s.env, EDITING + VOTING_PERIOD + 1);

    s.client.unstake(&borrower, &200);
    assert_eq!(s.client.get_stake(&borrower), 0);
}

#[test]
fn unstaking_during_open_loan_ballot_is_rejected() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let voter = s.members.get(1).unwrap();
    s.client.stake(&voter, &200);
    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&voter, &pid, &false);
    advance(&s.env, VOTING_PERIOD / 2);
    assert_eq!(
        s.client.try_unstake(&voter, &200),
        Err(Ok(Error::HasActiveLoan))
    );
}

#[test]
fn unstaking_during_open_treasury_ballot_is_rejected() {
    let s = setup(4);
    let proposer = s.members.get(0).unwrap();
    let voter = s.members.get(1).unwrap();
    s.client.stake(&voter, &200);
    let pid = s.client.propose_treasury_withdrawal(
        &proposer,
        &100,
        &Address::generate(&s.env),
        &soroban_sdk::String::from_str(&s.env, "vote"),
        &false,
    );
    s.client.vote_on_treasury_proposal(&voter, &pid, &false);
    advance(&s.env, VOTING_PERIOD / 2);
    assert_eq!(
        s.client.try_unstake(&voter, &200),
        Err(Ok(Error::HasActiveLoan))
    );
}

#[test]
fn unstaking_during_unrevealed_private_commitment_is_rejected() {
    let s = setup(3);
    let proposer = s.members.get(0).unwrap();
    let voter = s.members.get(1).unwrap();
    s.client.stake(&voter, &200);
    let pid = s.client.propose_treasury_withdrawal(
        &proposer,
        &100,
        &Address::generate(&s.env),
        &soroban_sdk::String::from_str(&s.env, "private"),
        &true,
    );
    let salt = soroban_sdk::BytesN::from_array(&s.env, &[1u8; 32]);
    let hash = crate::privacy::compute_commitment(&s.env, &voter, pid, true, &salt);
    s.client.commit_treasury_vote(&voter, &pid, &hash);
    advance(&s.env, VOTING_PERIOD / 2);
    assert_eq!(
        s.client.try_unstake(&voter, &200),
        Err(Ok(Error::HasActiveLoan))
    );
}

// Issue #280: staking always transfers the configured base token.
#[test]
fn staking_ignores_whitelisted_non_base_assets() {
    let s = setup(1);
    let member = s.members.get(0).unwrap();
    let second = whitelist_second_asset(&s, 0);
    mint_asset(&s, &second.address, &member, 10_000);

    let base_before = s.token.balance(&member);
    let second_before = second.balance(&member);
    s.client.stake(&member, &400);

    assert_eq!(s.token.balance(&member), base_before - 400);
    assert_eq!(second.balance(&member), second_before);
}
