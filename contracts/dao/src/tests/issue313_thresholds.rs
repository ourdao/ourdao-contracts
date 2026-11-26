use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, String};

use super::common::*;
use crate::types::{LoanPolicy, ProposalStatus};

// ==================== issue #313: loan threshold vs treasury_threshold ====================
// Tests that the loan proposal threshold and treasury threshold are applied to the
// correct proposal types. Loan proposals use `quorum_bps` for approval, while
// treasury withdrawals use `treasury_threshold`.
#[test]
fn loan_proposal_uses_quorum_bps_not_treasury_threshold() {
    let s = setup(3); // consensus 51% of 3 members = 2 votes
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    let pid = s.client.request_loan(&borrower, &1_000, &None);
    advance(&s.env, EDITING + 1);

    // A single vote (1/3) should NOT approve a loan
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    let prop = s.client.get_loan_proposal(&pid).unwrap();
    assert_eq!(prop.status, ProposalStatus::Pending);

    // Two votes (2/3 = 66% > 51%) should approve
    s.client.vote_on_loan_proposal(&v2, &pid, &true);
    let prop = s.client.get_loan_proposal(&pid).unwrap();
    assert_eq!(prop.status, ProposalStatus::Approved);
}

#[test]
fn treasury_withdrawal_uses_treasury_threshold_not_quorum_bps() {
    // Loans would need every vote (quorum 90%); withdrawals need 30%, which is
    // one of three members.
    let s = DAOBuilder::new()
        .with_members(3)
        .with_policy(LoanPolicy {
            quorum_bps: 9_000,
            treasury_threshold: 3_000,
            ..policy()
        })
        .build();
    let proposer = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let dest = Address::generate(&s.env);
    let reason = String::from_str(&s.env, "test");

    let pid = s
        .client
        .propose_treasury_withdrawal(&proposer, &1_000, &dest, &reason, &false);

    // One vote meets treasury_threshold and the withdrawal executes at once.
    s.client.vote_on_treasury_proposal(&v1, &pid, &true);
    let prop = s.client.get_treasury_proposal(&pid).unwrap();
    assert_eq!(prop.status, ProposalStatus::Executed);
    assert_eq!(s.token.balance(&dest), 1_000);
}

#[test]
fn treasury_withdrawal_above_treasury_threshold_requires_quorum() {
    // 75% of four members is three votes.
    let s = DAOBuilder::new()
        .with_members(4)
        .with_policy(LoanPolicy {
            treasury_threshold: 7_500,
            ..policy()
        })
        .build();
    let proposer = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();
    let v3 = s.members.get(3).unwrap();
    let dest = Address::generate(&s.env);
    let reason = String::from_str(&s.env, "test");

    let pid = s
        .client
        .propose_treasury_withdrawal(&proposer, &2_500, &dest, &reason, &false);

    // Two of four votes (50%) fall short of 75%.
    s.client.vote_on_treasury_proposal(&v1, &pid, &true);
    s.client.vote_on_treasury_proposal(&v2, &pid, &true);
    let prop = s.client.get_treasury_proposal(&pid).unwrap();
    assert_eq!(prop.status, ProposalStatus::Pending);

    // The third vote reaches it and the withdrawal executes.
    s.client.vote_on_treasury_proposal(&v3, &pid, &true);
    let prop = s.client.get_treasury_proposal(&pid).unwrap();
    assert_eq!(prop.status, ProposalStatus::Executed);
    assert_eq!(s.token.balance(&dest), 2_500);
}
