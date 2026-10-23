//! Public source, composition and instance contracts with independent consumer types.
#[path = "contracts/catalog_contract.rs"]
mod catalog_contract;
#[path = "contracts/composition_operations.rs"]
mod composition_operations;
#[path = "contracts/composition_safety.rs"]
mod composition_safety;
#[path = "contracts/dependency_failures.rs"]
mod dependency_failures;
#[path = "contracts/instance_contracts.rs"]
mod instance_contracts;
#[path = "contracts/runtime.rs"]
mod runtime;
#[path = "contracts/startup.rs"]
mod startup;
