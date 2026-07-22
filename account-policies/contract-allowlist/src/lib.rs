//! A contract-allowlist policy for Soroban smart accounts.
//!
//! Enforces: the delegated signer may only invoke functions on a pre-approved set of
//! contract addresses. Optionally, specific function names can be restricted
//! per contract. Designed to be attached as a policy on an OpenZeppelin
//! `stellar-accounts` smart account.
//!
//! Use case: a delegated signer should only be able to call your DEX contract's
//! `swap` function and your vault's `deposit` function — nothing else.

#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, Symbol, Vec};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    /// Set of contract addresses that are allowed.
    AllowedContracts,
    /// Per-contract function allowlist: contract address -> Vec<Symbol>.
    /// If empty for a given contract, all functions on that contract are
    /// allowed. If non-empty, only listed functions may be called.
    AllowedFunctions(Address),
}

#[contract]
pub struct ContractAllowlistPolicy;

#[contractimpl]
impl ContractAllowlistPolicy {
    /// One-time setup: set the admin.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);

        let empty: Vec<Address> = Vec::new(&env);
        env.storage()
            .persistent()
            .set(&DataKey::AllowedContracts, &empty);
    }

    /// Admin-only: add a contract address to the allowlist.
    /// If `allowed_fns` is empty, all functions on that contract are allowed.
    /// If non-empty, only the listed function names may be called.
    pub fn allow_contract(env: Env, contract_addr: Address, allowed_fns: Vec<Symbol>) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");
        admin.require_auth();

        let mut contracts: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::AllowedContracts)
            .unwrap_or_else(|| Vec::new(&env));

        // Add contract if not already present.
        let mut found = false;
        for c in contracts.iter() {
            if c == contract_addr {
                found = true;
                break;
            }
        }
        if !found {
            contracts.push_back(contract_addr.clone());
            env.storage()
                .persistent()
                .set(&DataKey::AllowedContracts, &contracts);
        }

        // Store the function allowlist for this contract.
        env.storage()
            .persistent()
            .set(&DataKey::AllowedFunctions(contract_addr), &allowed_fns);
    }

    /// Admin-only: remove a contract address from the allowlist.
    pub fn remove_contract(env: Env, contract_addr: Address) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");
        admin.require_auth();

        let contracts: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::AllowedContracts)
            .unwrap_or_else(|| Vec::new(&env));

        let mut updated: Vec<Address> = Vec::new(&env);
        for c in contracts.iter() {
            if c != contract_addr {
                updated.push_back(c);
            }
        }

        env.storage()
            .persistent()
            .set(&DataKey::AllowedContracts, &updated);
        env.storage()
            .persistent()
            .remove(&DataKey::AllowedFunctions(contract_addr));
    }

    /// Policy entrypoint: verify that `target_contract` and `function_name`
    /// are on the allowlist. Panics (denying the tx) if not.
    pub fn check(env: Env, target_contract: Address, function_name: Symbol) {
        let contracts: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::AllowedContracts)
            .unwrap_or_else(|| Vec::new(&env));

        // Check the contract is in the allowlist.
        let mut contract_allowed = false;
        for c in contracts.iter() {
            if c == target_contract {
                contract_allowed = true;
                break;
            }
        }
        if !contract_allowed {
            panic!("contract not in allowlist");
        }

        // Check function-level restrictions (if any).
        let allowed_fns: Vec<Symbol> = env
            .storage()
            .persistent()
            .get(&DataKey::AllowedFunctions(target_contract))
            .unwrap_or_else(|| Vec::new(&env));

        // If the function list is empty, all functions are allowed.
        if allowed_fns.is_empty() {
            return;
        }

        let mut fn_allowed = false;
        for f in allowed_fns.iter() {
            if f == function_name {
                fn_allowed = true;
                break;
            }
        }
        if !fn_allowed {
            panic!("function not in allowlist");
        }
    }

    /// Query: check if a contract is in the allowlist.
    pub fn is_allowed(env: Env, target_contract: Address) -> bool {
        let contracts: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::AllowedContracts)
            .unwrap_or_else(|| Vec::new(&env));

        for c in contracts.iter() {
            if c == target_contract {
                return true;
            }
        }
        false
    }

    /// Query: get the allowed functions for a given contract.
    /// Returns an empty vec if all functions are allowed or the contract
    /// is not in the allowlist.
    pub fn get_allowed_functions(env: Env, target_contract: Address) -> Vec<Symbol> {
        env.storage()
            .persistent()
            .get(&DataKey::AllowedFunctions(target_contract))
            .unwrap_or_else(|| Vec::new(&env))
    }

    pub fn policy_name(_env: Env) -> Symbol {
        Symbol::short("allow_ls")
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn allows_listed_contract() {
        let env = Env::default();
        let contract_id = env.register(ContractAllowlistPolicy, ());
        let client = ContractAllowlistPolicyClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let target = Address::generate(&env);

        env.mock_all_auths();
        client.initialize(&admin);

        // Allow the target contract with no function restrictions.
        let empty_fns: Vec<Symbol> = Vec::new(&env);
        client.allow_contract(&target, &empty_fns);

        // Any function name should be allowed.
        client.check(&target, &Symbol::short("swap"));
        client.check(&target, &Symbol::short("deposit"));
    }

    #[test]
    #[should_panic(expected = "contract not in allowlist")]
    fn rejects_unlisted_contract() {
        let env = Env::default();
        let contract_id = env.register(ContractAllowlistPolicy, ());
        let client = ContractAllowlistPolicyClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let target = Address::generate(&env);

        env.mock_all_auths();
        client.initialize(&admin);

        // Don't add target to allowlist — should panic.
        client.check(&target, &Symbol::short("swap"));
    }

    #[test]
    fn allows_listed_function() {
        let env = Env::default();
        let contract_id = env.register(ContractAllowlistPolicy, ());
        let client = ContractAllowlistPolicyClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let target = Address::generate(&env);

        env.mock_all_auths();
        client.initialize(&admin);

        // Allow only "swap" on this contract.
        let mut fns: Vec<Symbol> = Vec::new(&env);
        fns.push_back(Symbol::short("swap"));
        client.allow_contract(&target, &fns);

        client.check(&target, &Symbol::short("swap"));
    }

    #[test]
    #[should_panic(expected = "function not in allowlist")]
    fn rejects_unlisted_function() {
        let env = Env::default();
        let contract_id = env.register(ContractAllowlistPolicy, ());
        let client = ContractAllowlistPolicyClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let target = Address::generate(&env);

        env.mock_all_auths();
        client.initialize(&admin);

        // Allow only "swap", then try "withdraw".
        let mut fns: Vec<Symbol> = Vec::new(&env);
        fns.push_back(Symbol::short("swap"));
        client.allow_contract(&target, &fns);

        client.check(&target, &Symbol::short("withdraw"));
    }

    #[test]
    fn remove_contract_works() {
        let env = Env::default();
        let contract_id = env.register(ContractAllowlistPolicy, ());
        let client = ContractAllowlistPolicyClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let target = Address::generate(&env);

        env.mock_all_auths();
        client.initialize(&admin);

        let empty_fns: Vec<Symbol> = Vec::new(&env);
        client.allow_contract(&target, &empty_fns);
        assert!(client.is_allowed(&target));

        client.remove_contract(&target);
        assert!(!client.is_allowed(&target));
    }
}
