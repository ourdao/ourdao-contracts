use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Bytes, String};

use super::common::*;
use crate::storage::ProposalKind;
use crate::Error;

#[test]
fn content_hash_document() {
    let s = setup(1);
    let member = s.members.get(0).unwrap();
    let pid = s.client.request_loan(&member, &500, &None);

    let cid = Bytes::from_array(&s.env, b"QmExampleCid1234567890");
    s.client
        .attach_document(&member, &ProposalKind::Loan, &pid, &cid);
    assert_eq!(s.client.get_document(&ProposalKind::Loan, &pid), Some(cid));
}

// ==================== issue #302: document hash size limit ====================
#[test]
fn document_hash_exactly_64_bytes_accepted() {
    let s = setup(1);
    let member = s.members.get(0).unwrap();
    let pid = s.client.request_loan(&member, &500, &None);

    // Create exactly 64 byte hash.
    let hash_64 = Bytes::from_slice(
        &s.env,
        &[
            0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45,
            46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63,
        ],
    );
    assert_eq!(hash_64.len(), 64);

    let res = s
        .client
        .try_attach_document(&member, &ProposalKind::Loan, &pid, &hash_64);
    assert!(res.is_ok());
    assert_eq!(
        s.client.get_document(&ProposalKind::Loan, &pid),
        Some(hash_64)
    );
}

#[test]
fn document_hash_65_bytes_rejected() {
    let s = setup(1);
    let member = s.members.get(0).unwrap();
    let pid = s.client.request_loan(&member, &500, &None);

    // Create 65 byte hash.
    let hash_65 = Bytes::from_slice(
        &s.env,
        &[
            0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45,
            46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64,
        ],
    );
    assert_eq!(hash_65.len(), 65);

    let res = s
        .client
        .try_attach_document(&member, &ProposalKind::Loan, &pid, &hash_65);
    assert_eq!(res, Err(Ok(Error::DocumentTooLarge)));
}

// Issue #513: attach_document tests for treasury proposals specifically
#[test]
fn attach_document_treasury_proposal_proposer_and_non_proposer() {
    let s = setup(2);
    let proposer = s.members.get(0).unwrap();
    let stranger = s.members.get(1).unwrap();
    let dest = soroban_sdk::Address::generate(&s.env);
    let reason = soroban_sdk::String::from_str(&s.env, "grant for community event");

    let pid = s
        .client
        .propose_treasury_withdrawal(&proposer, &500, &dest, &reason, &false);

    let cid = Bytes::from_array(&s.env, b"QmTreasuryProposalCid123");

    // Stranger cannot attach document (NotProposalOwner)
    let res = s
        .client
        .try_attach_document(&stranger, &ProposalKind::Treasury, &pid, &cid);
    assert_eq!(res, Err(Ok(Error::NotProposalOwner)));

    // Proposer can attach document
    let res = s
        .client
        .try_attach_document(&proposer, &ProposalKind::Treasury, &pid, &cid);
    assert!(res.is_ok());
    assert_eq!(
        s.client.get_document(&ProposalKind::Treasury, &pid),
        Some(cid.clone())
    );

    // It may still be replaced before anybody has voted.
    let cid2 = Bytes::from_array(&s.env, b"QmTreasuryProposalCid456");
    let res2 = s
        .client
        .try_attach_document(&proposer, &ProposalKind::Treasury, &pid, &cid2);
    assert!(res2.is_ok());
    assert_eq!(
        s.client.get_document(&ProposalKind::Treasury, &pid),
        Some(cid2.clone())
    );

    // Once voting begins, the evidence becomes immutable just like a loan
    // document after its editing phase.
    s.client
        .vote_on_treasury_proposal(&stranger, &pid, &true);
    let cid3 = Bytes::from_array(&s.env, b"QmTreasuryProposalCid789");
    assert_eq!(
        s.client
            .try_attach_document(&proposer, &ProposalKind::Treasury, &pid, &cid3),
        Err(Ok(Error::NotInEditingPhase))
    );
    assert_eq!(
        s.client.get_document(&ProposalKind::Treasury, &pid),
        Some(cid2)
    );
}

// ==================== issue #459: ProposalKind extension contract ====================

/// Whether `kind` accepts documents. Deliberately an exhaustive `match`: a new
/// `ProposalKind` variant stops this compiling until its author decides
/// between the two options in the extension contract on
/// `storage::ProposalKind` and records the choice here.
fn accepts_documents(kind: ProposalKind) -> bool {
    match kind {
        ProposalKind::Loan | ProposalKind::Treasury => true,
    }
}

#[test]
fn every_proposal_kind_follows_the_document_extension_contract() {
    let s = setup(1);
    let member = s.members.get(0).unwrap();
    let loan_id = s.client.request_loan(&member, &500, &None);
    let treasury_id = s.client.propose_treasury_withdrawal(
        &member,
        &100,
        &Address::generate(&s.env),
        &String::from_str(&s.env, "grant"),
        &false,
    );
    let cid = Bytes::from_array(&s.env, b"QmExampleCid1234567890");

    for (kind, id) in [
        (ProposalKind::Loan, loan_id),
        (ProposalKind::Treasury, treasury_id),
    ] {
        let res = s.client.try_attach_document(&member, &kind, &id, &cid);
        if accepts_documents(kind) {
            // Both `proposal_exists` and `proposal_owner` must have an arm
            // for the kind, or the attach would fail here.
            assert_eq!(res, Ok(Ok(())));
            assert_eq!(s.client.get_document(&kind, &id), Some(cid.clone()));
        } else {
            assert_eq!(res, Err(Ok(Error::InvalidProposal)));
        }
    }
}
