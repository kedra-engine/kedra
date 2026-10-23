//! Public source-editing contracts: atomic changes, history, freshness and save receipts.
#[path = "contracts/component_owners.rs"]
mod component_owners;
#[path = "contracts/freshness.rs"]
mod freshness;
#[path = "contracts/inspection_snapshot.rs"]
mod inspection_snapshot;
#[path = "contracts/native_paths.rs"]
mod native_paths;
#[path = "contracts/override_contracts.rs"]
mod override_contracts;
#[path = "contracts/persistence_guards.rs"]
mod persistence_guards;
#[path = "contracts/persistence_protocol.rs"]
mod persistence_protocol;
mod support;
#[path = "contracts/transactions.rs"]
mod transactions;
#[path = "contracts/unpack_semantics.rs"]
mod unpack_semantics;
