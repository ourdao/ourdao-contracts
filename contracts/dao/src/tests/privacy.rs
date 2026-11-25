use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{Address, BytesN, String};

use super::common::*;
use crate::privacy::compute_commitment;
use crate::storage::ProposalKind;
use crate::types::ProposalStatus;
use crate::Error;

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
    let c1 = compute_commitment(&s.env, &v1, pid, true, &salt1);
    let c2 = compute_commitment(&s.env, &v2, pid, true, &salt2);

    s.client.commit_treasury_vote(&v1, &pid, &c1);
    s.client.commit_treasury_vote(&v2, &pid, &c2);

    // Reveals are valid only after the commitment phase.
    advance(&s.env, VOTING_PERIOD + 1);

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
fn commit_vote_cannot_be_overwritten() {
    let s = setup(2);
    let proposer = s.members.get(0).unwrap();
    let voter = s.members.get(1).unwrap();
    let dest = Address::generate(&s.env);
    let reason = String::from_str(&s.env, "secret grant");
    let pid = s
        .client
        .propose_treasury_withdrawal(&proposer, &600, &dest, &reason, &true);

    let salt = BytesN::from_array(&s.env, &[7u8; 32]);
    let other = BytesN::from_array(&s.env, &[9u8; 32]);
    let c1 = compute_commitment(&s.env, &voter, pid, true, &salt);
    let c2 = compute_commitment(&s.env, &voter, pid, false, &other);

    s.client.commit_treasury_vote(&voter, &pid, &c1);
    let again = s.client.try_commit_treasury_vote(&voter, &pid, &c2);
    assert_eq!(again, Err(Ok(Error::AlreadyVoted)));

    // The original commitment is still the one that reveals successfully.
    advance(&s.env, VOTING_PERIOD + 1);
    s.client.reveal_treasury_vote(&voter, &pid, &true, &salt);
}

#[test]
fn has_voted_treasury_commit_reveal() {
    let s = setup(3);
    let proposer = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let dest = Address::generate(&s.env);
    let reason = String::from_str(&s.env, "grant");

    let pid = s
        .client
        .propose_treasury_withdrawal(&proposer, &100, &dest, &reason, &true);

    // Before commit: not voted.
    assert!(!s.client.has_voted(&ProposalKind::Treasury, &pid, &v1));

    // After commit: voted (commitment counts as a vote for dedup).
    let salt = BytesN::from_array(&s.env, &[1u8; 32]);
    let commitment = compute_commitment(&s.env, &v1, pid, true, &salt);
    s.client.commit_treasury_vote(&v1, &pid, &commitment);
    assert!(s.client.has_voted(&ProposalKind::Treasury, &pid, &v1));
}

// ===========================================================================
// Issue #174: Verify reveal salt / commitment hash validation
// ===========================================================================

#[test]
fn reveal_with_correct_salt_tallies_vote() {
    let s = setup(3);
    let proposer = s.members.get(0).unwrap();
    let voter = s.members.get(1).unwrap();
    let dest = Address::generate(&s.env);

    let reason = String::from_str(&s.env, "private grant");
    let pid = s
        .client
        .propose_treasury_withdrawal(&proposer, &300, &dest, &reason, &true);

    let salt = BytesN::from_array(&s.env, &[42u8; 32]);
    let commitment = compute_commitment(&s.env, &voter, pid, true, &salt);
    s.client.commit_treasury_vote(&voter, &pid, &commitment);

    advance(&s.env, VOTING_PERIOD + 1);

    // Correct salt reveals cleanly.
    s.client.reveal_treasury_vote(&voter, &pid, &true, &salt);
    assert!(s.client.has_voted(&ProposalKind::Treasury, &pid, &voter));
}

#[test]
fn reveal_with_mismatched_salt_returns_commitment_mismatch() {
    let s = setup(3);
    let proposer = s.members.get(0).unwrap();
    let voter = s.members.get(1).unwrap();
    let dest = Address::generate(&s.env);

    let reason = String::from_str(&s.env, "private grant");
    let pid = s
        .client
        .propose_treasury_withdrawal(&proposer, &300, &dest, &reason, &true);

    let real_salt = BytesN::from_array(&s.env, &[1u8; 32]);
    let wrong_salt = BytesN::from_array(&s.env, &[2u8; 32]);
    let commitment = compute_commitment(&s.env, &voter, pid, true, &real_salt);
    s.client.commit_treasury_vote(&voter, &pid, &commitment);

    advance(&s.env, VOTING_PERIOD + 1);

    let res = s
        .client
        .try_reveal_treasury_vote(&voter, &pid, &true, &wrong_salt);
    assert_eq!(res, Err(Ok(Error::CommitmentMismatch)));
}

#[test]
fn reveal_with_wrong_support_returns_commitment_mismatch() {
    let s = setup(3);
    let proposer = s.members.get(0).unwrap();
    let voter = s.members.get(1).unwrap();
    let dest = Address::generate(&s.env);

    let reason = String::from_str(&s.env, "private grant");
    let pid = s
        .client
        .propose_treasury_withdrawal(&proposer, &300, &dest, &reason, &true);

    let salt = BytesN::from_array(&s.env, &[7u8; 32]);
    let commitment = compute_commitment(&s.env, &voter, pid, true, &salt);
    s.client.commit_treasury_vote(&voter, &pid, &commitment);

    advance(&s.env, VOTING_PERIOD + 1);

    // Committed true but reveals false — hash mismatch.
    let res = s
        .client
        .try_reveal_treasury_vote(&voter, &pid, &false, &salt);
    assert_eq!(res, Err(Ok(Error::CommitmentMismatch)));
}

#[test]
fn commitments_bind_voter_and_proposal() {
    let env = soroban_sdk::Env::default();
    let voter = Address::generate(&env);
    let copier = Address::generate(&env);
    let salt = BytesN::from_array(&env, &[7u8; 32]);
    let c = compute_commitment(&env, &voter, 3, true, &salt);
    assert_eq!(c, compute_commitment(&env, &voter, 3, true, &salt));
    assert_ne!(c, compute_commitment(&env, &copier, 3, true, &salt));
    assert_ne!(c, compute_commitment(&env, &voter, 4, true, &salt));
    assert_ne!(c, compute_commitment(&env, &voter, 3, false, &salt));
}

#[test]
fn copied_commitment_cannot_be_revealed_by_another_voter() {
    let s = setup(3);
    let proposer = s.members.get(0).unwrap();
    let first = s.members.get(1).unwrap();
    let copier = s.members.get(2).unwrap();
    let destination = Address::generate(&s.env);
    let pid = s.client.propose_treasury_withdrawal(
        &proposer,
        &300,
        &destination,
        &String::from_str(&s.env, "private"),
        &true,
    );
    let salt = BytesN::from_array(&s.env, &[13u8; 32]);
    let commitment = compute_commitment(&s.env, &first, pid, true, &salt);
    s.client.commit_treasury_vote(&first, &pid, &commitment);
    s.client.commit_treasury_vote(&copier, &pid, &commitment);
    advance(&s.env, VOTING_PERIOD + 1);
    assert_eq!(
        s.client
            .try_reveal_treasury_vote(&copier, &pid, &true, &salt),
        Err(Ok(Error::CommitmentMismatch)),
    );
    s.client.reveal_treasury_vote(&first, &pid, &true, &salt);
}

// ===========================================================================
// Issues #475 and #476: reveal pre-check and shared window boundaries
// ===========================================================================

#[test]
fn verify_commitment_precheck_matches_reveal_outcome() {
    let s = setup(3);
    let proposer = s.members.get(0).unwrap();
    let voter = s.members.get(1).unwrap();
    let other = s.members.get(2).unwrap();
    let destination = Address::generate(&s.env);
    let reason = String::from_str(&s.env, "private grant");
    let pid = s
        .client
        .propose_treasury_withdrawal(&proposer, &300, &destination, &reason, &true);

    let salt = BytesN::from_array(&s.env, &[7u8; 32]);
    let wrong_salt = BytesN::from_array(&s.env, &[9u8; 32]);
    assert!(!s.client.verify_commitment(&voter, &pid, &true, &salt));

    let commitment = compute_commitment(&s.env, &voter, pid, true, &salt);
    s.client.commit_treasury_vote(&voter, &pid, &commitment);

    assert!(s.client.verify_commitment(&voter, &pid, &true, &salt));
    assert!(!s.client.verify_commitment(&voter, &pid, &false, &salt));
    assert!(!s.client.verify_commitment(&voter, &pid, &true, &wrong_salt));
    assert!(!s.client.verify_commitment(&other, &pid, &true, &salt));

    advance(&s.env, VOTING_PERIOD + 1);

    assert_eq!(
        s.client
            .try_reveal_treasury_vote(&voter, &pid, &false, &salt),
        Err(Ok(Error::CommitmentMismatch)),
    );
    assert!(!s.client.verify_commitment(&voter, &pid, &false, &salt));

    assert_eq!(
        s.client
            .try_reveal_treasury_vote(&other, &pid, &true, &salt),
        Err(Ok(Error::NoCommitment)),
    );
    assert!(!s.client.verify_commitment(&other, &pid, &true, &salt));

    s.client.reveal_treasury_vote(&voter, &pid, &true, &salt);
    assert!(!s.client.verify_commitment(&voter, &pid, &true, &salt));
    assert_eq!(
        s.client
            .try_reveal_treasury_vote(&voter, &pid, &true, &salt),
        Err(Ok(Error::AlreadyRevealed)),
    );
}

#[test]
fn get_commit_reveal_window_matches_enforced_boundaries() {
    let s = setup(5);
    let proposer = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();
    let too_late = s.members.get(3).unwrap();
    let destination = Address::generate(&s.env);
    let reason = String::from_str(&s.env, "private grant");
    let pid = s
        .client
        .propose_treasury_withdrawal(&proposer, &300, &destination, &reason, &true);

    let (commit_end, reveal_end) = s.client.get_commit_reveal_window(&pid);
    let created_at = s.client.get_treasury_proposal(&pid).unwrap().created_at;
    assert_eq!(commit_end, created_at + VOTING_PERIOD);
    assert_eq!(reveal_end, commit_end + VOTING_PERIOD);

    let salt1 = BytesN::from_array(&s.env, &[1u8; 32]);
    let salt2 = BytesN::from_array(&s.env, &[2u8; 32]);
    let salt3 = BytesN::from_array(&s.env, &[3u8; 32]);
    let c1 = compute_commitment(&s.env, &v1, pid, true, &salt1);
    let c2 = compute_commitment(&s.env, &v2, pid, true, &salt2);
    let c3 = compute_commitment(&s.env, &too_late, pid, true, &salt3);

    s.env.ledger().with_mut(|li| li.timestamp = commit_end);
    s.client.commit_treasury_vote(&v1, &pid, &c1);
    s.client.commit_treasury_vote(&v2, &pid, &c2);
    assert_eq!(
        s.client.try_reveal_treasury_vote(&v1, &pid, &true, &salt1),
        Err(Ok(Error::NotYetRevealed)),
    );

    s.env.ledger().with_mut(|li| li.timestamp = commit_end + 1);
    assert_eq!(
        s.client.try_commit_treasury_vote(&too_late, &pid, &c3),
        Err(Ok(Error::VotingEnded)),
    );
    s.client.reveal_treasury_vote(&v1, &pid, &true, &salt1);

    s.env.ledger().with_mut(|li| li.timestamp = reveal_end + 1);
    assert_eq!(
        s.client.try_reveal_treasury_vote(&v2, &pid, &true, &salt2),
        Err(Ok(Error::VotingEnded)),
    );
}

#[test]
fn get_commit_reveal_window_rejects_public_proposal() {
    let s = setup(2);
    let proposer = s.members.get(0).unwrap();
    let destination = Address::generate(&s.env);
    let reason = String::from_str(&s.env, "public grant");
    let pid = s
        .client
        .propose_treasury_withdrawal(&proposer, &300, &destination, &reason, &false);

    assert_eq!(
        s.client.try_get_commit_reveal_window(&pid),
        Err(Ok(Error::NotAuthorized)),
    );
}

// ── expire_treasury_proposal uses the full commit + reveal window (#434) ────

#[test]
fn private_proposal_cannot_expire_inside_its_reveal_window() {
    let s = setup(3);
    let proposer = s.members.get(0).unwrap();
    let voter = s.members.get(1).unwrap();
    let destination = Address::generate(&s.env);
    let reason = String::from_str(&s.env, "private grant");
    let pid = s
        .client
        .propose_treasury_withdrawal(&proposer, &300, &destination, &reason, &true);
    let (commit_end, reveal_end) = s.client.get_commit_reveal_window(&pid);

    let salt = BytesN::from_array(&s.env, &[5u8; 32]);
    let commitment = compute_commitment(&s.env, &voter, pid, true, &salt);
    s.client.commit_treasury_vote(&voter, &pid, &commitment);

    // Past the commit window but inside the reveal window: expiry is refused
    // and the committed voter can still reveal.
    s.env.ledger().with_mut(|li| li.timestamp = commit_end + 1);
    assert_eq!(
        s.client.try_expire_treasury_proposal(&pid),
        Err(Ok(Error::NotInVotingPhase)),
    );
    assert_eq!(
        s.client.get_treasury_proposal(&pid).unwrap().status,
        ProposalStatus::Pending
    );
    s.client.reveal_treasury_vote(&voter, &pid, &true, &salt);

    // The last second of the reveal window is still not expirable.
    s.env.ledger().with_mut(|li| li.timestamp = reveal_end);
    assert_eq!(
        s.client.try_expire_treasury_proposal(&pid),
        Err(Ok(Error::NotInVotingPhase)),
    );
}

#[test]
fn private_proposal_expires_once_the_reveal_window_has_closed() {
    let s = setup(3);
    let proposer = s.members.get(0).unwrap();
    let destination = Address::generate(&s.env);
    let reason = String::from_str(&s.env, "private grant");
    let pid = s
        .client
        .propose_treasury_withdrawal(&proposer, &300, &destination, &reason, &true);
    let (_commit_end, reveal_end) = s.client.get_commit_reveal_window(&pid);

    s.env.ledger().with_mut(|li| li.timestamp = reveal_end + 1);
    s.client.expire_treasury_proposal(&pid);
    assert_eq!(
        s.client.get_treasury_proposal(&pid).unwrap().status,
        ProposalStatus::Expired
    );
}

#[test]
fn public_proposal_still_expires_after_one_voting_period() {
    let s = setup(3);
    let proposer = s.members.get(0).unwrap();
    let destination = Address::generate(&s.env);
    let reason = String::from_str(&s.env, "open grant");
    let pid = s
        .client
        .propose_treasury_withdrawal(&proposer, &300, &destination, &reason, &false);
    let created_at = s.client.get_treasury_proposal(&pid).unwrap().created_at;

    s.env
        .ledger()
        .with_mut(|li| li.timestamp = created_at + VOTING_PERIOD);
    assert_eq!(
        s.client.try_expire_treasury_proposal(&pid),
        Err(Ok(Error::NotInVotingPhase)),
    );
    s.env
        .ledger()
        .with_mut(|li| li.timestamp = created_at + VOTING_PERIOD + 1);
    s.client.expire_treasury_proposal(&pid);
    assert_eq!(
        s.client.get_treasury_proposal(&pid).unwrap().status,
        ProposalStatus::Expired
    );
}

// ── a revealed vote's weight cannot be raised after commit (#453) ──────────

#[test]
fn late_stake_before_reveal_is_blocked_and_vote_keeps_commit_time_weight() {
    let s = setup(4);
    let proposer = s.members.get(0).unwrap();
    let late = s.members.get(1).unwrap();
    let early = s.members.get(2).unwrap();
    let dest = Address::generate(&s.env);
    let reason = String::from_str(&s.env, "secret grant");
    let pid = s
        .client
        .propose_treasury_withdrawal(&proposer, &600, &dest, &reason, &true);

    let late_salt = BytesN::from_array(&s.env, &[1u8; 32]);
    let early_salt = BytesN::from_array(&s.env, &[2u8; 32]);
    s.client.commit_treasury_vote(
        &late,
        &pid,
        &compute_commitment(&s.env, &late, pid, true, &late_salt),
    );
    s.client.commit_treasury_vote(
        &early,
        &pid,
        &compute_commitment(&s.env, &early, pid, false, &early_salt),
    );
    let weight_at_commit = s.client.get_voting_weight(&late);
    assert_eq!(weight_at_commit, 1);

    // Blocked during the commit phase...
    assert_eq!(
        s.client.try_stake(&late, &10_000),
        Err(Ok(Error::HasActiveLoan))
    );

    // ...and after watching another member's reveal move the public tally.
    advance(&s.env, VOTING_PERIOD + 1);
    s.client
        .reveal_treasury_vote(&early, &pid, &false, &early_salt);
    assert_eq!(
        s.client.get_treasury_proposal(&pid).unwrap().against_votes,
        1
    );
    assert_eq!(
        s.client.try_stake(&late, &10_000),
        Err(Ok(Error::HasActiveLoan))
    );
    assert_eq!(s.client.get_voting_weight(&late), weight_at_commit);

    s.client
        .reveal_treasury_vote(&late, &pid, &true, &late_salt);
    assert_eq!(
        s.client.get_treasury_proposal(&pid).unwrap().for_votes,
        weight_at_commit
    );

    // Once revealed the commitment is consumed and staking is open again.
    s.client.stake(&late, &10_000);
    assert!(s.client.get_voting_weight(&late) > weight_at_commit);
}

#[test]
fn delegator_cannot_stake_up_a_delegatee_with_an_unrevealed_commitment() {
    let s = setup(3);
    let proposer = s.members.get(0).unwrap();
    let voter = s.members.get(1).unwrap();
    let delegator = s.members.get(2).unwrap();
    let dest = Address::generate(&s.env);
    let reason = String::from_str(&s.env, "secret grant");
    s.client.delegate_vote(&delegator, &voter);
    let pid = s
        .client
        .propose_treasury_withdrawal(&proposer, &600, &dest, &reason, &true);

    let salt = BytesN::from_array(&s.env, &[3u8; 32]);
    s.client.commit_treasury_vote(
        &voter,
        &pid,
        &compute_commitment(&s.env, &voter, pid, true, &salt),
    );
    let weight_at_commit = s.client.get_voting_weight(&voter);

    advance(&s.env, VOTING_PERIOD + 1);
    assert_eq!(
        s.client.try_stake(&delegator, &10_000),
        Err(Ok(Error::HasActiveLoan))
    );
    assert_eq!(s.client.get_voting_weight(&voter), weight_at_commit);
}

#[test]
fn stake_is_not_blocked_without_an_unrevealed_commitment() {
    let s = setup(3);
    let proposer = s.members.get(0).unwrap();
    let voter = s.members.get(1).unwrap();
    let bystander = s.members.get(2).unwrap();
    let dest = Address::generate(&s.env);
    let reason = String::from_str(&s.env, "secret grant");
    let pid = s
        .client
        .propose_treasury_withdrawal(&proposer, &600, &dest, &reason, &true);
    let salt = BytesN::from_array(&s.env, &[4u8; 32]);
    s.client.commit_treasury_vote(
        &voter,
        &pid,
        &compute_commitment(&s.env, &voter, pid, true, &salt),
    );

    // A member with no commitment (and no committed delegatee) may stake.
    s.client.stake(&bystander, &10_000);

    // An abandoned commitment stops blocking once the reveal window closes.
    let (_, reveal_end) = s.client.get_commit_reveal_window(&pid);
    s.env.ledger().with_mut(|li| li.timestamp = reveal_end + 1);
    s.client.stake(&voter, &10_000);
}
