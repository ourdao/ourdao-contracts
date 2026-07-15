//! Name registry — the Stellar-native stand-in for the EVM contract's ENS
//! governance hooks. Maps human-readable names to addresses (and back), so
//! members and the DAO itself can be referenced by name on-chain.

use soroban_sdk::{symbol_short, Address, Env, String};

use crate::error::Error;
use crate::storage::{self, DataKey};

pub fn register_name(env: &Env, owner: Address, name: String) -> Result<(), Error> {
    util_owner_auth(&owner);

    if let Some(existing) = storage::get_name_owner(env, &name) {
        if existing != owner {
            return Err(Error::NameTaken);
        }
    }

    // Free any name this owner held previously so lookups stay 1:1.
    if let Some(old) = storage::get_name_of(env, &owner) {
        if old != name {
            env.storage().persistent().remove(&DataKey::Name(old));
        }
    }

    storage::set_name(env, &name, &owner);
    env.events()
        .publish((symbol_short!("name_reg"),), (name, owner));
    Ok(())
}

pub fn resolve_name(env: &Env, name: String) -> Option<Address> {
    storage::get_name_owner(env, &name)
}

pub fn name_of(env: &Env, owner: Address) -> Option<String> {
    storage::get_name_of(env, &owner)
}

fn util_owner_auth(owner: &Address) {
    owner.require_auth();
}
