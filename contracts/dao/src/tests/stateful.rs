extern crate std;

use proptest::prelude::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, String};

use super::common::*;
use crate::types::MemberStatus;
use crate::OurDaoClient;

#[derive(Clone, Copy, Debug)]
enum Action {
    Join,
    Exit,
    Stake,
    Unstake,
    RequestLoan,
    VoteLoan,
    Repay,
    Default,
    ProposeTreasury,
    VoteTreasury,
    ExecuteTreasury,
}

fn action() -> impl Strategy<Value = Action> {
    prop::sample::select(std::vec![
        Action::Join,
        Action::Exit,
        Action::Stake,
        Action::Unstake,
        Action::RequestLoan,
        Action::VoteLoan,
        Action::Repay,
        Action::Default,
        Action::ProposeTreasury,
        Action::VoteTreasury,
        Action::ExecuteTreasury,
    ])
}

fn check_invariants(env: &soroban_sdk::Env, client: &OurDaoClient<'_>) {
    let contract_id = client.address.clone();

    let (total_members, active_members, active_contributions) =
        env.as_contract(&contract_id, || {
            (
                crate::storage::get_total_members(env),
                crate::storage::get_active_members(env),
                crate::storage::get_active_contributions(env),
            )
        });

    assert_eq!(
        active_members,
        env.as_contract(&contract_id, || {
            let mut counted = 0u32;
            for address in crate::storage::get_members(env).iter() {
                if matches!(
                    crate::storage::get_member(env, &address),
                    Some(member) if member.status == MemberStatus::ActiveMember
                ) {
                    counted += 1;
                }
            }
            counted
        })
    );

    assert_eq!(
        total_members,
        env.as_contract(&contract_id, || { crate::storage::get_members(env).len() })
    );

    let sum_contributions = env.as_contract(&contract_id, || {
        let mut sum = 0i128;
        for address in crate::storage::get_members(env).iter() {
            if let Some(member) = crate::storage::get_member(env, &address) {
                if member.status == MemberStatus::ActiveMember {
                    sum = sum.saturating_add(member.contribution);
                }
            }
        }
        sum
    });
    assert_eq!(active_contributions, sum_contributions);

    let mut sum_stake = 0i128;
    for address in client.list_members(&0, &1_000).iter() {
        sum_stake = sum_stake.saturating_add(client.get_stake(&address.address));
    }
    assert_eq!(sum_stake, client.get_dao_summary().total_staked);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    #[test]
    fn stateful_actions_preserve_invariants(
        steps in prop::collection::vec(action(), 12..=24),
    ) {
        let s = setup(4);
        let mut newcomers: soroban_sdk::Vec<Address> = soroban_sdk::Vec::new(&s.env);

        for step in steps {
            let actor = s.members
                .get((step as u8 as u32) % s.members.len())
                .unwrap_or_else(|| s.members.get(0).unwrap());

            match step {
                Action::Join => {
                    let member = Address::generate(&s.env);
                    soroban_sdk::token::StellarAssetClient::new(
                        &s.env,
                        &s.client.get_token(),
                    ).mint(&member, &MINT);
                    let _ = s.client.try_register_member(&member);
                    if s.client.is_member(&member) {
                        newcomers.push_back(member);
                    }
                }
                Action::Exit => {
                    if s.client.is_member(&actor) {
                        let _ = s.client.try_exit_dao(&actor);
                    }
                }
                Action::Stake => {
                    if s.client.is_member(&actor) {
                        let _ = s.client.try_stake(&actor, &100);
                    }
                }
                Action::Unstake => {
                    if s.client.is_member(&actor) {
                        let _ = s.client.try_unstake(&actor, &100);
                    }
                }
                Action::RequestLoan => {
                    if s.client.is_member(&actor) {
                        let _ = s.client.try_request_loan(&actor, &100, &None);
                    }
                }
                Action::VoteLoan => {
                    let count = s.client.get_loan_proposal_count();
                    if count > 0 {
                        let id = count - 1;
                        advance(&s.env, EDITING + 1);
                        let _ = s.client.try_vote_on_loan_proposal(&actor, &id, &true);
                    }
                }
                Action::Repay => {
                    let count = s.client.get_loan_proposal_count();
                    for id in 0..count {
                        if let Some(loan) = s.client.get_loan(&id) {
                            if loan.status == crate::types::LoanStatus::Active {
                                let _ = s.client.try_repay_loan_partial(&actor, &id, &10);
                            }
                        }
                    }
                }
                Action::Default => {
                    advance(&s.env, LOAN_DURATION + 1);
                    let count = s.client.get_loan_proposal_count();
                    for id in 0..count {
                        if let Some(loan) = s.client.get_loan(&id) {
                            if loan.status == crate::types::LoanStatus::Active {
                                let _ = s.client.try_mark_loan_defaulted(&id);
                            }
                        }
                    }
                }
                Action::ProposeTreasury => {
                    if s.client.is_member(&actor) {
                        let reason = String::from_str(&s.env, "stateful");
                        let _ = s.client.try_propose_treasury_withdrawal(
                            &actor,
                            &10,
                            &Address::generate(&s.env),
                            &reason,
                            &false,
                        );
                    }
                }
                Action::VoteTreasury => {
                    let count = s.client.get_treasury_proposal_count();
                    if count > 0 {
                        let id = count - 1;
                        for member in s.members.iter() {
                            let _ = s.client.try_vote_on_treasury_proposal(
                                &member,
                                &id,
                                &true,
                            );
                        }
                    }
                }
                Action::ExecuteTreasury => {
                    let count = s.client.get_treasury_proposal_count();
                    if count > 0 {
                        let id = count - 1;
                        let _ = s.client.try_execute_treasury_proposal(&id);
                    }
                }
            }

            check_invariants(&s.env, &s.client);
        }
    }
}
