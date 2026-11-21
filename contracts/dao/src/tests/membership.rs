use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, String};

use super::common::*;
use crate::types::MemberStatus;
use crate::Error;

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

// ==================== issue #341: get_delegate view ====================
#[test]
fn get_delegate_reports_delegation_and_none_otherwise() {
    let s = setup(2);
    let a = s.members.get(0).unwrap();
    let b = s.members.get(1).unwrap();

    assert_eq!(s.client.get_delegate(&a), None);

    s.client.delegate_vote(&a, &b);
    assert_eq!(s.client.get_delegate(&a), Some(b.clone()));
    // Delegation is one-way: the delegatee has not delegated anything.
    assert_eq!(s.client.get_delegate(&b), None);
}

// ==================== issue #4: yield accumulator ====================
#[test]
fn yield_accumulator_join_claim_exit_rejoin() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    // Get a loan approved and repaid so interest is distributed.
    let pid = s.client.request_loan(&borrower, &1_000, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);
    s.client.repay_loan(&borrower, &0);

    let loan = s.client.get_loan(&0).unwrap();
    let interest = loan.total_repayment - loan.principal;
    let per_member = interest / 3;

    // Both non-borrower members can claim their share.
    assert_eq!(s.client.get_pending_yield(&v1), per_member);
    assert_eq!(s.client.get_pending_yield(&v2), per_member);
    s.client.claim_rewards(&v1);
    assert_eq!(s.client.get_pending_yield(&v1), 0);

    // v1 exits â€” their snapshot is settled so they don't double-claim.
    s.client.exit_dao(&v1);
    assert_eq!(s.client.get_pending_yield(&v1), 0);

    // v1 rejoins â€” snapshot is set to current accumulator, earning nothing
    // from interest that accrued before they rejoined.
    s.client.register_member(&v1);
    assert_eq!(s.client.get_pending_yield(&v1), 0);
}

// ==================== issue: total_contributions ====================
#[test]
fn ten_join_five_exit_pays_remaining_members_full_share() {
    let s = setup(10);

    for i in 0..5u32 {
        let m = s.members.get(i).unwrap();
        assert_eq!(s.client.calculate_exit_share(&m), FEE);
        s.client.exit_dao(&m);
    }

    assert_eq!(s.client.get_total_members(), 10);
    assert_eq!(s.client.get_active_members(), 5);
    assert_eq!(s.client.get_treasury_balance(), 5 * FEE);

    for i in 5..10u32 {
        let m = s.members.get(i).unwrap();
        assert_eq!(s.client.calculate_exit_share(&m), FEE);
    }
}

#[test]
fn rejoin_after_exit_is_counted_once_not_twice() {
    let s = setup(3);
    let m = s.members.get(0).unwrap();

    s.client.exit_dao(&m);
    assert_eq!(s.client.get_active_members(), 2);

    s.client.register_member(&m);
    assert_eq!(s.client.get_total_members(), 3);
    assert_eq!(s.client.get_active_members(), 3);
    assert_eq!(s.client.calculate_exit_share(&m), FEE);
}

#[test]
fn mixed_joins_exits_defaults_never_strand_value() {
    let s = setup(3);
    let a = s.members.get(0).unwrap();
    let b = s.members.get(1).unwrap();
    let c = s.members.get(2).unwrap();

    let pid = s.client.request_loan(&a, &500, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&b, &pid, &true);
    s.client.vote_on_loan_proposal(&c, &pid, &true);

    s.client.exit_dao(&c);
    let b_share_before_default = s.client.calculate_exit_share(&b);

    advance(&s.env, LOAN_DURATION + 1);
    s.client.mark_loan_defaulted(&pid);

    let b_share_after_default = s.client.calculate_exit_share(&b);
    assert!(b_share_after_default > b_share_before_default);

    s.client.exit_dao(&b);
    s.client.register_member(&c);

    assert_eq!(s.client.get_total_members(), 3);
    assert_eq!(s.client.get_active_members(), 2);

    let mut sum = 0i128;
    for i in 0..3u32 {
        let m = s.members.get(i).unwrap();
        if s.client.is_member(&m) {
            sum += s.client.calculate_exit_share(&m);
        }
    }
    assert!(sum <= s.client.get_treasury_balance());
}

#[test]
fn bench_exit_dao_scaling() {
    // #138: Measure cost of exit_dao at different sizes
    for size in [10, 100, 1000] {
        let s = setup(size);
        let m = s.members.get(0).unwrap();

        s.env.cost_estimate().budget().reset_unlimited();
        let cpu_before = s.env.cost_estimate().budget().cpu_instruction_cost();
        s.client.exit_dao(&m);
        let cpu_after = s.env.cost_estimate().budget().cpu_instruction_cost();

        let cost = cpu_after - cpu_before;

        // Fail if cost scales poorly (O(n) check). Keep this no_std-compatible
        // instead of printing from the contract crate.
        assert!(
            size != 1000 || cost <= 50_000_000,
            "exit_dao cost exceeds O(1) bound"
        );
    }
}

// ==================== issue #304: rejoin resets state ====================
#[test]
fn rejoin_after_exit_resets_share_stake_yield_snapshot_and_name() {
    let s = setup(3);
    let m = s.members.get(0).unwrap();
    let borrower = s.members.get(1).unwrap();
    let voter = s.members.get(2).unwrap();

    // Register a name for the member.
    let name = String::from_str(&s.env, "alice");
    s.client.register_name(&m, &name);
    assert_eq!(s.client.name_of(&m), Some(name.clone()));

    // Member stakes.
    s.client.stake(&m, &500);
    assert_eq!(s.client.get_stake(&m), 500);

    // Generate yield by creating a loan and having it repaid.
    let pid = s.client.request_loan(&borrower, &1_000, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&voter, &pid, &true);
    s.client.vote_on_loan_proposal(&m, &pid, &true);
    s.client.repay_loan(&borrower, &0);

    let loan = s.client.get_loan(&0).unwrap();
    let interest = loan.total_repayment - loan.principal;
    let per_member_yield = interest / 3;
    assert!(per_member_yield > 0);

    // Member claims their yield.
    s.client.claim_rewards(&m);

    let pending_before_exit = s.client.get_pending_yield(&m);
    assert_eq!(pending_before_exit, 0);

    // Member exits the DAO.
    s.client.exit_dao(&m);
    assert!(!s.client.is_member(&m));
    assert_eq!(s.client.get_stake(&m), 0);
    assert_eq!(s.client.get_pending_yield(&m), 0);
    // After exiting, the name should be freed.
    assert_eq!(s.client.name_of(&m), None);
    assert_eq!(s.client.resolve_name(&name), None);

    // Member rejoins.
    s.client.register_member(&m);
    assert!(s.client.is_member(&m));

    // Verify all state is reset:
    // - Stake is zero.
    assert_eq!(s.client.get_stake(&m), 0);
    // - Yield snapshot is reset to current accumulator (so pending is zero).
    assert_eq!(s.client.get_pending_yield(&m), 0);
    // - Name is freed and can be re-registered.
    assert_eq!(s.client.name_of(&m), None);
    s.client.register_name(&m, &name);
    assert_eq!(s.client.name_of(&m), Some(name));
}

#[test]
fn rejected_register_member_transfer_rolls_back_all_membership_state() {
    let s = rejecting_setup(0);
    let member = Address::generate(&s.env);
    s.token.mint(&member, &MINT);
    s.token.set_reject_transfers(&true);

    let result = s.client.try_register_member(&member);
    assert!(result.is_err());

    assert_eq!(s.client.get_total_members(), 0);
    assert_eq!(s.client.get_active_members(), 0);
    assert!(!s.client.is_member(&member));
    assert!(s.client.get_member(&member).is_none());
    assert_eq!(s.token.balance(&s.client.address), 0);
}

// ===========================================================================
// Issue #176: Handle token transfer failure cleanly in exit_dao
// ===========================================================================

#[test]
fn exit_dao_returns_error_when_contract_balance_insufficient() {
    let s = rejecting_setup(2);
    let member = s.members.get(0).unwrap();

    // Reject all outgoing transfers to simulate a depleted/frozen treasury.
    s.token.set_reject_transfers(&true);

    let res = s.client.try_exit_dao(&member);
    assert!(
        res.is_err(),
        "exit_dao must fail cleanly when contract cannot pay out"
    );
}

#[test]
fn exit_dao_succeeds_when_treasury_funded() {
    let s = setup(2);
    let member = s.members.get(0).unwrap();

    let share = s.client.calculate_exit_share(&member);
    assert!(share > 0);
    s.client.exit_dao(&member);
    assert!(!s.client.is_member(&member));
}

// ==================== #92: view and payout agree ====================
#[test]
fn pending_yield_view_matches_claim_rewards_payout() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    let pid = s.client.request_loan(&borrower, &1_000, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);
    s.client.repay_loan(&borrower, &0);

    let shown = s.client.get_pending_yield(&v1);
    assert!(shown > 0);
    let before = s.token.balance(&v1);
    let claimed = s.client.claim_rewards(&v1);
    assert_eq!(claimed, shown);
    assert_eq!(s.token.balance(&v1) - before, shown);

    // exit_dao pays share + stake + the same displayed pending yield.
    let shown = s.client.get_pending_yield(&v2);
    let share = s.client.calculate_exit_share(&v2);
    let before = s.token.balance(&v2);
    s.client.exit_dao(&v2);
    assert_eq!(s.token.balance(&v2) - before, share + shown);
}

// Issue #510: Two-hop delegation chain is single-level / non-transitive
#[test]
fn two_hop_delegation_chain_is_non_transitive() {
    let s = setup(3);
    let a = s.members.get(0).unwrap();
    let b = s.members.get(1).unwrap();
    let c = s.members.get(2).unwrap();

    // A delegates to B, B delegates to C
    s.client.delegate_vote(&a, &b);
    s.client.delegate_vote(&b, &c);

    // B delegated away, so B has 0 voting weight
    s.env.as_contract(&s.client.address, || {
        assert_eq!(crate::util::voting_weight(&s.env, &b), 0);
    });

    // C only directly receives B's vote, not A's transitive vote (single-level)
    s.env.as_contract(&s.client.address, || {
        // Base vote 100 + B's base vote 100 = 200
        assert_eq!(crate::util::voting_weight(&s.env, &c), 200);
    });
}

// Issue #511: Delegation state across exit and rejoin
#[test]
fn delegation_state_across_exit_and_rejoin() {
    let s = setup(2);
    let a = s.members.get(0).unwrap();
    let b = s.members.get(1).unwrap();

    s.client.delegate_vote(&a, &b);
    assert_eq!(s.client.get_delegate(&a), Some(b.clone()));

    // A exits
    s.client.exit_dao(&a);

    // Rejoin
    s.token.mint(&a, &FEE);
    s.client.register_member(&a);

    // Check delegate status after rejoin
    let d = s.client.get_delegate(&a);
    assert_eq!(d, Some(b.clone()));
}

// Issue #512: Voting weight ignores delegations from inactive (exited) members
#[test]
fn voting_weight_ignores_delegations_from_inactive_members() {
    let s = setup(2);
    let a = s.members.get(0).unwrap();
    let b = s.members.get(1).unwrap();

    s.client.delegate_vote(&a, &b);

    // Before exit: B has weight 200 (100 base + 100 from A)
    s.env.as_contract(&s.client.address, || {
        assert_eq!(crate::util::voting_weight(&s.env, &b), 200);
    });

    // A exits
    s.client.exit_dao(&a);

    // After exit: B has weight 100 (A's delegation ignored because A is inactive)
    s.env.as_contract(&s.client.address, || {
        assert_eq!(crate::util::voting_weight(&s.env, &b), 100);
    });
}

// Issue #525: calculate_exit_share for an address that has never registered
#[test]
fn calculate_exit_share_for_unregistered_address() {
    let s = setup(1);
    let unknown = Address::generate(&s.env);

    // Unregistered address has None in get_member
    assert_eq!(s.client.get_member(&unknown), None);

    // calculate_exit_share returns 0 without erroring or trapping
    assert_eq!(s.client.calculate_exit_share(&unknown), 0);
}
}


// Issue #280: membership contribution always uses the configured base token.
#[test]
fn membership_fee_ignores_whitelisted_non_base_assets() {
    let s = setup(1);
    let second = whitelist_second_asset(&s, 0);
    let member = Address::generate(&s.env);
    soroban_sdk::token::StellarAssetClient::new(&s.env, &s.token.address)
        .mint(&member, &MINT);
    mint_asset(&s, &second.address, &member, MINT);

    let base_before = s.token.balance(&member);
    let second_before = second.balance(&member);
    s.client.register_member(&member);

    assert_eq!(s.token.balance(&member), base_before - FEE);
    assert_eq!(second.balance(&member), second_before);
}
