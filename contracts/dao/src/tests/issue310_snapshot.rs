// ==================== issue #310: snapshot tests for exact event topics and payloads ====================
// Snapshot tests verifying the exact event topics and payload data for key events.
use soroban_sdk::testutils::{Address as _, Events as _};
use soroban_sdk::xdr::{ContractEventBody, ScVal};
use soroban_sdk::{Address, Env, IntoVal, String, TryFromVal, Val};

use super::common::*;
use crate::admin::TIMELOCK_DURATION;

/// Data payload of the most recent event whose first topic is `name`, checked
/// to carry no topic besides that name.
fn event_data(env: &Env, name: &str) -> ScVal {
    let all = env.events().all();
    let event = all
        .events()
        .iter()
        .rev()
        .find(|e| {
            let ContractEventBody::V0(body) = &e.body;
            matches!(body.topics.first(), Some(ScVal::Symbol(sym)) if sym.0.to_utf8_string_lossy() == name)
        })
        .unwrap_or_else(|| panic!("{name} event not found"))
        .clone();
    let ContractEventBody::V0(body) = event.body;
    assert_eq!(body.topics.len(), 1);
    body.data
}

/// `value` encoded the way the host encodes event data.
fn scval<T: IntoVal<Env, Val>>(env: &Env, value: T) -> ScVal {
    ScVal::try_from_val(env, &value.into_val(env)).unwrap()
}

#[test]
fn loan_request_event_exact_payload() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let pid = s.client.request_loan(&borrower, &1_000, &None);
    // Events only cover the latest invocation, so read them first.
    let data = event_data(&s.env, "loan_req");
    let total = s.client.get_loan_proposal(&pid).unwrap().total_repayment;

    // (id, borrower, amount, total_repayment, asset)
    assert_eq!(
        data,
        scval(
            &s.env,
            (pid, borrower, 1_000i128, total, s.token.address.clone())
        )
    );
}

#[test]
fn treasury_propose_event_exact_payload() {
    let s = setup(3);
    let proposer = s.members.get(0).unwrap();
    let dest = Address::generate(&s.env);
    let reason = String::from_str(&s.env, "test");
    let pid = s
        .client
        .propose_treasury_withdrawal(&proposer, &600, &dest, &reason, &false);

    // (id, amount, destination, private)
    assert_eq!(
        event_data(&s.env, "tre_prop"),
        scval(&s.env, (pid, 600i128, dest, false))
    );
}

#[test]
fn policy_update_proposal_event_exact_payload() {
    let s = setup(2);
    let mut policy = s.client.get_loan_policy();
    policy.max_interest_rate = 1_200;

    s.client.propose_policy_update(&s.admin, &policy);

    // (proposer, execution_time)
    let execution_time = s.env.ledger().timestamp() + TIMELOCK_DURATION;
    assert_eq!(
        event_data(&s.env, "pol_prop"),
        scval(&s.env, (s.admin.clone(), execution_time))
    );
}
