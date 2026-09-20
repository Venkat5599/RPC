//! Policy evaluation.
//!
//! A policy answers one question about one transaction: does this, if it
//! executes, mean the depositor should be out? Evaluation is pure — no I/O, no
//! clock, no network — so the same call is valid over a historical
//! transaction, a confirmed one, and a simulated pending one.

use serde::{Deserialize, Serialize};

use crate::txview::{AltResolution, TxView};

/// Why an evaluation refused to produce a verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Refusal {
    /// The account set was incomplete. PRD N2: never act on this.
    IncompleteAccounts(AltResolution),
}

/// What fired, and the evidence a human (or the on-chain predicate) can check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Detection {
    pub policy_id: String,
    pub signature: String,
    pub slot: u64,
    pub block_time: Option<i64>,
    /// The account the policy fired on: a ProgramData account for an
    /// authority change, a reserve for an outflow.
    pub subject: String,
    /// Human-readable, quotable in a report without further processing.
    pub reason: String,
    /// Instruction indices that carried the evidence.
    pub evidence_ix: Vec<usize>,
}

/// The verdict for one transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Decision {
    /// Nothing in this transaction concerns the policy.
    NoMatch,
    /// The policy fired.
    Fire(Box<Detection>),
    /// The policy declined to evaluate. This is not a no-match: it means we
    /// could not see enough to answer, and it must be logged and alerted.
    Refuse(Refusal),
}

/// A policy. Deliberately not a general VM — every policy must also be
/// expressible as a bounded on-chain predicate, or it does not ship.
pub trait Policy {
    fn id(&self) -> &str;

    /// Evaluate against an account set already known to be complete.
    /// Call [`evaluate`] instead; it applies the refusal guard first.
    fn evaluate_resolved(&self, tx: &TxView) -> Decision;
}

/// Evaluate a policy with the completeness guard applied.
///
/// Every input source routes through here, so the refusal path cannot be
/// forgotten at one call site: a `PARTIAL` or absent resolution refuses before
/// the policy ever sees the transaction.
pub fn evaluate(policy: &dyn Policy, tx: &TxView) -> Decision {
    match tx.alt_resolution {
        AltResolution::Full => policy.evaluate_resolved(tx),
        other => Decision::Refuse(Refusal::IncompleteAccounts(other)),
    }
}
