//! The authority-change policy.
//!
//! Fires when the upgrade authority over a watched program is transferred, or
//! when the protocol's own admin instruction hands control to a new key. It is
//! first because it is the one policy that is fully trustless on-chain later:
//! `execute_exit` can read the target's `ProgramData` authority itself and
//! compare it against the value recorded at `arm` time.

use crate::loader::{self, BPF_LOADER_UPGRADEABLE};
use crate::policy::{Decision, Detection, Policy};
use crate::txview::TxView;
use crate::anchor;

/// One watched program and the accounts that govern it.
#[derive(Debug, Clone)]
pub struct WatchedProgram {
    /// Program ID, for matching protocol-level admin instructions.
    pub program_id: String,
    /// The program's ProgramData account, which is what the loader's
    /// `SetAuthority` actually names. Derived off-chain once at `arm` time.
    pub program_data: String,
    /// Label used in reports, e.g. "kamino-lend".
    pub label: String,
    /// Protocol-level Anchor instruction names that transfer administrative
    /// control. Matched by discriminator, so no IDL is required.
    pub admin_ix_names: Vec<String>,
}

pub struct AuthorityChange {
    watched: Vec<WatchedProgram>,
}

impl AuthorityChange {
    pub fn new(watched: Vec<WatchedProgram>) -> Self {
        Self { watched }
    }

    fn watched_by_program_data(&self, account: &str) -> Option<&WatchedProgram> {
        self.watched.iter().find(|w| w.program_data == account)
    }

    fn watched_by_program_id(&self, program_id: &str) -> Option<&WatchedProgram> {
        self.watched.iter().find(|w| w.program_id == program_id)
    }
}

impl Policy for AuthorityChange {
    fn id(&self) -> &str {
        "authority-change"
    }

    fn evaluate_resolved(&self, tx: &TxView) -> Decision {
        for (index, ix) in tx.instructions.iter().enumerate() {
            // Path 1: the loader itself reassigns upgrade authority.
            if ix.program_id == BPF_LOADER_UPGRADEABLE {
                let Some(decoded) = loader::decode(&ix.data) else {
                    continue;
                };
                if !decoded.is_authority_change() {
                    continue;
                }
                let Some(target) = loader::target_account(&ix.accounts) else {
                    continue;
                };
                let Some(watched) = self.watched_by_program_data(target) else {
                    continue;
                };
                let new_authority = loader::new_authority(decoded, &ix.accounts)
                    .map(String::as_str)
                    .unwrap_or("none (program made immutable)");
                return Decision::Fire(Box::new(Detection {
                    policy_id: self.id().to_string(),
                    signature: tx.signature.clone(),
                    slot: tx.slot,
                    block_time: tx.block_time,
                    subject: target.clone(),
                    reason: format!(
                        "{:?} on {} ({}) transfers upgrade authority to {}",
                        decoded, watched.label, target, new_authority
                    ),
                    evidence_ix: vec![index],
                }));
            }

            // Path 2: the protocol's own admin instruction moves control.
            if let Some(watched) = self.watched_by_program_id(&ix.program_id) {
                for name in &watched.admin_ix_names {
                    if anchor::matches(&ix.data, name) {
                        return Decision::Fire(Box::new(Detection {
                            policy_id: self.id().to_string(),
                            signature: tx.signature.clone(),
                            slot: tx.slot,
                            block_time: tx.block_time,
                            subject: watched.program_id.clone(),
                            reason: format!(
                                "protocol admin instruction `{}` on {}",
                                name, watched.label
                            ),
                            evidence_ix: vec![index],
                        }));
                    }
                }
            }
        }

        Decision::NoMatch
    }
}
