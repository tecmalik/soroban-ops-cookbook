//! A minimal per-transaction spend-limit policy for Soroban smart accounts.
//!
//! Enforces: a single transaction may move at most `max_amount` of a given
//! token. Designed to be attached as a policy on an OpenZeppelin
//! `stellar-accounts` smart account so a delegated signer (or any automated workflow)
//! can be scoped without holding the account's main key.
//!
//! This is intentionally simple — one limit, one token — so it's easy to
//! read, audit, and compose with other policies (see `../rate-limit` for a
//! time-windowed complement).

#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    MaxAmount(Address), // token -> max amount per tx
    Admin,
}

#[contract]
pub struct SpendLimitPolicy;

#[contractimpl]
impl SpendLimitPolicy {
    /// One-time setup: set the admin allowed to configure limits.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    /// Set (or update) the max amount allowed per transaction for `token`.
    /// Only callable by the admin set in `initialize`.
    pub fn set_limit(env: Env, token: Address, max_amount: i128) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");
        admin.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::MaxAmount(token), &max_amount);
    }

    /// Policy entrypoint: called by the smart account's auth flow before a
    /// transfer is allowed. Returns without panicking if `amount` is within
    /// the configured limit for `token`; panics (denying the tx) otherwise.
    ///
    /// In a real `stellar-accounts` integration this maps to the `Policy`
    /// trait's `enforce` hook — wire this function up as that hook.
    pub fn check(env: Env, token: Address, amount: i128) {
        let max_amount: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::MaxAmount(token))
            .unwrap_or(0);

        if amount > max_amount {
            panic!("amount exceeds per-transaction spend limit");
        }
    }

    pub fn get_limit(env: Env, token: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::MaxAmount(token))
            .unwrap_or(0)
    }

    pub fn policy_name(_env: Env) -> Symbol {
        Symbol::short("spend_lim")
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn allows_amount_under_limit() {
        let env = Env::default();
        let contract_id = env.register(SpendLimitPolicy, ());
        let client = SpendLimitPolicyClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let token = Address::generate(&env);

        env.mock_all_auths();
        client.initialize(&admin);
        client.set_limit(&token, &50_000_000i128); // e.g. 50 USDC at 6dp

        // Should not panic.
        client.check(&token, &10_000_000i128);
    }

    #[test]
    #[should_panic(expected = "amount exceeds per-transaction spend limit")]
    fn rejects_amount_over_limit() {
        let env = Env::default();
        let contract_id = env.register(SpendLimitPolicy, ());
        let client = SpendLimitPolicyClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let token = Address::generate(&env);

        env.mock_all_auths();
        client.initialize(&admin);
        client.set_limit(&token, &50_000_000i128);

        client.check(&token, &60_000_000i128); // should panic
    }
}
