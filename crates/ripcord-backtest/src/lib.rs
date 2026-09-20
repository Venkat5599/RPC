//! RIPCORD backtest harness.
//!
//! Replays historical Solana transactions through the production policy
//! engine. No Aperture entitlement is required, which is why this is day-one
//! work and why its output is independently recomputable.

pub mod block_scan;
pub mod cache;
pub mod engine;
pub mod incidents;
pub mod report;
pub mod normalize;
pub mod rpc;
pub mod slots;
