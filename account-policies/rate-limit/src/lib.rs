//! A rolling-window rate-limit policy for Soroban smart accounts.
//!
//! Enforces: at most `max_count` transactions within any rolling window of
//! `window_ledgers` ledger sequence numbers. Designed to be attached as a
//! policy on an OpenZeppelin `stellar-accounts` smart account alongside
//! spend-limit or contract-allowlist policies.
//!
//! The window is ledger-based (not wall-clock) because ledger sequence is
//! the only monotonic timestamp available inside a Soroban contract.

#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol, Vec};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    /// Maximum number of calls allowed in the window.
    MaxCount,
    /// Window size in ledger sequence numbers.
    WindowLedgers,
    /// Ring buffer of ledger sequence numbers when `record_call` was invoked.
    CallLedgers,
}

#[contract]
pub struct RateLimitPolicy;

#[contractimpl]
impl RateLimitPolicy {
    /// One-time setup: set the admin and the rate-limit parameters.
    ///
    /// * `max_count` — max transactions allowed within the window.
    /// * `window_ledgers` — window size in ledger sequence numbers (e.g. 720
    ///   ≈ 1 hour at ~5 s/ledger).
    pub fn initialize(env: Env, admin: Address, max_count: u32, window_ledgers: u32) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .persistent()
            .set(&DataKey::MaxCount, &max_count);
        env.storage()
            .persistent()
            .set(&DataKey::WindowLedgers, &window_ledgers);

        let empty: Vec<u32> = Vec::new(&env);
        env.storage()
            .persistent()
            .set(&DataKey::CallLedgers, &empty);
    }

    /// Admin-only: update the rate-limit parameters after initialization.
    pub fn set_limits(env: Env, max_count: u32, window_ledgers: u32) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");
        admin.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::MaxCount, &max_count);
        env.storage()
            .persistent()
            .set(&DataKey::WindowLedgers, &window_ledgers);
    }

    /// Policy entrypoint: record a call and enforce the rate limit.
    ///
    /// Call this from the smart account's auth flow before allowing the
    /// transaction to proceed. It records the current ledger sequence and
    /// panics if the number of calls within the window exceeds `max_count`.
    pub fn check(env: Env) {
        let max_count: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::MaxCount)
            .expect("not initialized");
        let window_ledgers: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::WindowLedgers)
            .expect("not initialized");
        let mut call_ledgers: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::CallLedgers)
            .unwrap_or_else(|| Vec::new(&env));

        let current_ledger = env.ledger().sequence();
        let cutoff = current_ledger.saturating_sub(window_ledgers);

        // Prune entries outside the window.
        let mut pruned: Vec<u32> = Vec::new(&env);
        for ledger in call_ledgers.iter() {
            if ledger >= cutoff {
                pruned.push_back(ledger);
            }
        }

        // Check rate limit before recording the new call.
        if pruned.len() >= max_count {
            panic!("rate limit exceeded");
        }

        // Record this call.
        pruned.push_back(current_ledger);
        call_ledgers = pruned;

        env.storage()
            .persistent()
            .set(&DataKey::CallLedgers, &call_ledgers);
    }

    pub fn get_limits(env: Env) -> (u32, u32) {
        let max_count: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::MaxCount)
            .unwrap_or(0);
        let window_ledgers: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::WindowLedgers)
            .unwrap_or(0);
        (max_count, window_ledgers)
    }

    pub fn get_call_count(env: Env) -> u32 {
        let call_ledgers: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::CallLedgers)
            .unwrap_or_else(|| Vec::new(&env));
        let window_ledgers: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::WindowLedgers)
            .unwrap_or(0);
        let current_ledger = env.ledger().sequence();
        let cutoff = current_ledger.saturating_sub(window_ledgers);

        let mut count: u32 = 0;
        for ledger in call_ledgers.iter() {
            if ledger >= cutoff {
                count += 1;
            }
        }
        count
    }

    pub fn policy_name(_env: Env) -> Symbol {
        Symbol::short("rate_lim")
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger};

    #[test]
    fn allows_calls_within_limit() {
        let env = Env::default();
        let contract_id = env.register(RateLimitPolicy, ());
        let client = RateLimitPolicyClient::new(&env, &contract_id);

        let admin = Address::generate(&env);

        env.mock_all_auths();
        // Allow 3 calls per 100-ledger window.
        client.initialize(&admin, &3u32, &100u32);

        // First three should succeed.
        client.check();

        env.ledger().with_mut(|li| li.sequence_number += 1);
        client.check();

        env.ledger().with_mut(|li| li.sequence_number += 1);
        client.check();
    }

    #[test]
    #[should_panic(expected = "rate limit exceeded")]
    fn rejects_calls_over_limit() {
        let env = Env::default();
        let contract_id = env.register(RateLimitPolicy, ());
        let client = RateLimitPolicyClient::new(&env, &contract_id);

        let admin = Address::generate(&env);

        env.mock_all_auths();
        // Allow 2 calls per 100-ledger window.
        client.initialize(&admin, &2u32, &100u32);

        client.check();
        env.ledger().with_mut(|li| li.sequence_number += 1);
        client.check();
        env.ledger().with_mut(|li| li.sequence_number += 1);
        // Third call should panic.
        client.check();
    }

    #[test]
    fn window_expiry_allows_new_calls() {
        let env = Env::default();
        let contract_id = env.register(RateLimitPolicy, ());
        let client = RateLimitPolicyClient::new(&env, &contract_id);

        let admin = Address::generate(&env);

        env.mock_all_auths();
        // Allow 2 calls per 10-ledger window.
        client.initialize(&admin, &2u32, &10u32);

        client.check();
        env.ledger().with_mut(|li| li.sequence_number += 1);
        client.check();

        // Jump forward past the window.
        env.ledger().with_mut(|li| li.sequence_number += 20);

        // Should succeed — old calls fell out of the window.
        client.check();
    }

    #[test]
    fn get_call_count_reflects_window() {
        let env = Env::default();
        let contract_id = env.register(RateLimitPolicy, ());
        let client = RateLimitPolicyClient::new(&env, &contract_id);

        let admin = Address::generate(&env);

        env.mock_all_auths();
        client.initialize(&admin, &5u32, &10u32);

        assert_eq!(client.get_call_count(), 0);

        client.check();
        assert_eq!(client.get_call_count(), 1);

        env.ledger().with_mut(|li| li.sequence_number += 1);
        client.check();
        assert_eq!(client.get_call_count(), 2);

        // Jump past window.
        env.ledger().with_mut(|li| li.sequence_number += 20);
        assert_eq!(client.get_call_count(), 0);
    }
}
