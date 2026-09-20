//! RIPCORD policy engine.
//!
//! One evaluator, three input sources: historical replay (backtest),
//! Yellowstone confirmed (public feed), and Aperture TxStream pre-execution
//! (live defence). The backtest is not a separate codebase written to impress
//! anyone — it is this crate running against a different input, which is why
//! its numbers mean something.

pub mod anchor;
pub mod authority_change;
pub mod loader;
pub mod policy;
pub mod txview;

pub use policy::{evaluate, Decision, Detection, Policy, Refusal};
pub use txview::{AltResolution, IxView, TokenDelta, TxView};
