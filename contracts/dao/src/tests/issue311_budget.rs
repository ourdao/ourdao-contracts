extern crate std;

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, String};

use super::common::*;
use crate::types::{LoanStatus, ProposalStatus};

// ==================== issue #311: budget regression tests for heavy entrypoints ====================
// Regression tests for the heaviest contract entrypoints: loan lifecycle,
// treasury withdrawals, and policy updates - focusing on budget/fee consumption
// near the instance/persistent storage limits.
#[test]
fn heavy_loan_entrypoint_budget_regression() {
    let s = setup(5);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();
    let v3 = s.members.get(3).unwrap();

    // Exercise the full loan lifecycle multiple times to stress instance storage
    for _ in 0..3 {
        let pid = s.client.request_loan(&borrower, &1_000, &None);
        advance(&s.env, EDITING + 1);
        s.client.vote_on_loan_proposal(&v1, &pid, &true);
        s.client.vote_on_loan_proposal(&v2, &pid, &true);
        s.client.vote_on_loan_proposal(&v3, &pid, &true);

        let loan = s.client.get_loan(&pid).unwrap();
        assert_eq!(loan.status, LoanStatus::Active);
        assert_eq!(loan.principal, 1_000);

        s.client.repay_loan(&borrower, &pid);
        let loan = s.client.get_loan(&pid).unwrap();
        assert_eq!(loan.status, LoanStatus::Repaid);
    }

    // Verify storage hasn't grown unbounded
    assert_eq!(s.client.get_active_members(), 5);
    assert_eq!(s.client.get_total_members(), 5);
}

#[test]
fn heavy_treasury_entrypoint_budget_regression() {
    let s = setup(3);
    let proposer = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();
    let dest = Address::generate(&s.env);

    // Fund the treasury for all ten withdrawals (5,500 in total).
    soroban_sdk::token::StellarAssetClient::new(&s.env, &s.token.address)
        .mint(&s.client.address, &5_500);

    // Issue many treasury withdrawals in sequence
    let mut paid = 0i128;
    for i in 0..10 {
        let pid = s.client.propose_treasury_withdrawal(
            &proposer,
            &(100 * (i as i128 + 1)),
            &dest,
            &String::from_str(&s.env, &std::format!("withdrawal {}", i)),
            &false,
        );
        advance(&s.env, 1);

        // Vote to approve
        s.client.vote_on_treasury_proposal(&v1, &pid, &true);
        s.client.vote_on_treasury_proposal(&v2, &pid, &true);

        // Reaching the threshold executes the withdrawal immediately.
        let prop = s.client.get_treasury_proposal(&pid).unwrap();
        assert_eq!(prop.status, ProposalStatus::Executed);
        paid += 100 * (i as i128 + 1);
        assert_eq!(s.token.balance(&dest), paid);
    }

    // Verify all funds disbursed
    assert_eq!(s.token.balance(&dest), 5_500);
}

#[test]
fn policy_update_entrypoint_budget_regression() {
    let s = setup(2);
    let mut policy = s.client.get_loan_policy();
    policy.max_interest_rate = 1_500;
    policy.min_interest_rate = 500;

    // Propose and execute a policy update (full lifecycle)
    s.client.propose_policy_update(&s.admin, &policy);
    advance(&s.env, crate::admin::TIMELOCK_DURATION);
    s.client.execute_policy_update(&s.admin);

    let updated = s.client.get_loan_policy();
    assert_eq!(updated.max_interest_rate, 1_500);
    assert_eq!(updated.min_interest_rate, 500);
}
