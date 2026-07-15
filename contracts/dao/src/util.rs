use soroban_sdk::{token, Address, Env};

use crate::error::Error;
use crate::storage;
use crate::types::{Member, MemberStatus};

/// Staked tokens above this unit grant one extra unit of voting weight...
pub const STAKE_WEIGHT_UNIT: i128 = 100;
/// ...up to this cap, so a whale can never fully dominate member consensus.
pub const MAX_STAKE_BONUS: i128 = 5;

pub fn token_client(env: &Env) -> token::Client<'_> {
    token::Client::new(env, &storage::get_token(env))
}

pub fn contract_address(env: &Env) -> Address {
    env.current_contract_address()
}

/// Treasury == the DAO's own token balance minus funds earmarked as stake,
/// so staked principal is never lent out or counted as distributable equity.
pub fn treasury_balance(env: &Env) -> i128 {
    let bal = token_client(env).balance(&contract_address(env));
    bal - storage::get_total_staked(env)
}

pub fn require_initialized(env: &Env) -> Result<(), Error> {
    if storage::is_initialized(env) {
        Ok(())
    } else {
        Err(Error::NotInitialized)
    }
}

pub fn require_not_paused(env: &Env) -> Result<(), Error> {
    if storage::is_paused(env) {
        Err(Error::Paused)
    } else {
        Ok(())
    }
}

pub fn is_admin(env: &Env, who: &Address) -> bool {
    storage::get_admins(env).iter().any(|a| &a == who)
}

/// Authorizes `caller` and asserts admin membership.
pub fn require_admin(env: &Env, caller: &Address) -> Result<(), Error> {
    caller.require_auth();
    if is_admin(env, caller) {
        Ok(())
    } else {
        Err(Error::NotAdmin)
    }
}

/// Authorizes `caller` and returns their active-member record, or errors.
pub fn require_active_member(env: &Env, caller: &Address) -> Result<Member, Error> {
    caller.require_auth();
    match storage::get_member(env, caller) {
        Some(m) if m.status == MemberStatus::ActiveMember => Ok(m),
        Some(_) => Err(Error::MemberNotActive),
        None => Err(Error::NotMember),
    }
}

/// One base vote per active member, plus a capped bonus for staked commitment.
pub fn voting_weight(env: &Env, who: &Address) -> i128 {
    let bonus = (storage::get_stake(env, who) / STAKE_WEIGHT_UNIT).min(MAX_STAKE_BONUS);
    1 + bonus
}

/// Ceil-division consensus bar over the active-member base, in basis points.
/// Mirrors the EVM contract's `(base * threshold + BP - 1) / BP`.
pub fn required_votes(active_members: u32, threshold_bps: u32) -> i128 {
    let base = active_members as i128;
    let bp = crate::types::BASIS_POINTS;
    (base * threshold_bps as i128 + bp - 1) / bp
}
