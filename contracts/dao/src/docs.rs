//! Content-hash proposal metadata — the Stellar-native stand-in for the EVM
//! contract's Filecoin storage integration. Soroban has no decentralized blob
//! store, so instead of storing documents on-chain we anchor a content hash
//! (e.g. an IPFS CID or SHA-256 digest) against a proposal. The bytes live
//! off-chain; the chain proves which document a proposal referred to.

use soroban_sdk::{symbol_short, Address, Bytes, Env};

use crate::error::Error;
use crate::storage::{self, ProposalKind};
use crate::util;

fn proposal_exists(env: &Env, kind: &ProposalKind, id: u32) -> bool {
    match kind {
        ProposalKind::Loan => storage::get_loan_proposal(env, id).is_some(),
        ProposalKind::Treasury => storage::get_treasury_proposal(env, id).is_some(),
    }
}

pub fn attach_document(
    env: &Env,
    caller: Address,
    kind: ProposalKind,
    proposal_id: u32,
    content_hash: Bytes,
) -> Result<(), Error> {
    util::require_initialized(env)?;
    util::require_active_member(env, &caller)?;
    if !proposal_exists(env, &kind, proposal_id) {
        return Err(Error::ProposalNotFound);
    }
    storage::set_doc(env, kind.clone(), proposal_id, &content_hash);
    env.events()
        .publish((symbol_short!("doc_attn"),), (kind, proposal_id, caller));
    Ok(())
}

pub fn get_document(env: &Env, kind: ProposalKind, proposal_id: u32) -> Option<Bytes> {
    storage::get_doc(env, kind, proposal_id)
}
