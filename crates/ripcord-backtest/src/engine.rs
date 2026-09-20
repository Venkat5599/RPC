//! The replay runner.
//!
//! Enumerates an incident window, fetches each transaction once, normalises it
//! and hands it to the policy. Every number the report prints is produced
//! here, and every one of them is derived from a transaction that is sitting
//! in the cache and can be re-read by hand.

use std::collections::BTreeSet;

use anyhow::Result;
use ripcord_policy::authority_change::{AuthorityChange, WatchedProgram};
use ripcord_policy::{evaluate, Decision, Detection, Policy, Refusal};

use crate::cache::Cache;
use crate::incidents::Incident;
use crate::normalize;
use crate::rpc::RpcClient;

/// What one incident window produced.
#[derive(Debug, Clone)]
pub struct WindowResult {
    pub incident_id: String,
    pub protocol: String,
    /// Signatures the window enumerated.
    pub examined: usize,
    /// Transactions fetched over the network on this run. Zero on a second
    /// run is the probe that the cache works.
    pub fetched: usize,
    /// Transactions read from cache.
    pub from_cache: usize,
    /// Transactions that could not be normalised, with the reason. Counted
    /// rather than swallowed: a window that silently drops half its
    /// transactions produces a confident, wrong number.
    pub skipped: Vec<(String, String)>,
    /// Refusals on incomplete account sets. A non-zero count here is a
    /// finding, not an error.
    pub refusals: usize,
    pub detections: Vec<Detection>,
    /// How the window was read: by paging signatures, or by scanning blocks.
    /// The two count `fetched` in different units, so the report says which.
    pub scan_mode: ScanMode,
    /// The slot range actually scanned, when the window was resolved to one.
    pub slot_range: Option<(u64, u64)>,
}

/// How a window was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanMode {
    /// `getSignaturesForAddress`, paging backwards from the present. Only
    /// usable for recent windows.
    Signatures,
    /// `getBlocks` + `getBlock` over a resolved slot range. The only approach
    /// that reaches a historical window.
    Blocks,
}

impl ScanMode {
    /// What one unit of `fetched` means in this mode.
    pub fn unit(self) -> &'static str {
        match self {
            ScanMode::Signatures => "transactions",
            ScanMode::Blocks => "blocks",
        }
    }
}

impl WindowResult {
    /// Earliest detection in the window, by block time.
    pub fn first_detection(&self) -> Option<&Detection> {
        self.detections
            .iter()
            .filter(|d| d.block_time.is_some())
            .min_by_key(|d| d.block_time.unwrap_or(i64::MAX))
    }

    /// Seconds between the first detection and the moment the loss completed.
    ///
    /// `None` when either end is unknown — an incident whose completion time
    /// is not established contributes detections but no lead time, rather than
    /// a guessed one.
    pub fn lead_seconds(&self, incident: &Incident) -> Option<i64> {
        let detection = self.first_detection()?.block_time?;
        let complete = incident.loss_complete_unix?;
        Some(complete - detection)
    }
}

/// The policy under test, configured for one incident.
///
/// The watched set is built from the incident's own program list, so the same
/// evaluator instance is never quietly reused across windows with the wrong
/// targets.
pub fn policy_for(incident: &Incident, program_data: &[(String, String)]) -> AuthorityChange {
    let watched = incident
        .programs
        .iter()
        .map(|program_id| {
            let data_account = program_data
                .iter()
                .find(|(id, _)| id == program_id)
                .map(|(_, data)| data.clone())
                .unwrap_or_default();
            WatchedProgram {
                program_id: (*program_id).to_string(),
                program_data: data_account,
                label: format!("{}:{}", incident.protocol, short(program_id)),
                admin_ix_names: admin_instruction_names(),
            }
        })
        .collect();
    AuthorityChange::new(watched)
}

/// Protocol-level instruction names that hand over administrative control.
///
/// Matched by Anchor discriminator, so no IDL is shipped. The list is
/// deliberately short and named: a wide net here would inflate the detection
/// count with instructions that are not handovers.
pub fn admin_instruction_names() -> Vec<String> {
    [
        "update_lending_market_owner",
        "set_lending_market_owner",
        "update_market_owner",
        "transfer_authority",
        "set_admin",
        "update_admin",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn short(pubkey: &str) -> String {
    pubkey.chars().take(8).collect()
}

/// Replay one incident window.
pub fn run_window(
    client: &RpcClient,
    cache: &Cache,
    incident: &Incident,
    policy: &dyn Policy,
    page_limit: usize,
    max_transactions: Option<usize>,
    mut progress: impl FnMut(&str),
) -> Result<WindowResult> {
    let mut signatures = BTreeSet::new();
    for program in incident.programs {
        progress(&format!(
            "  enumerating {} over {}",
            short(program),
            incident.id
        ));
        let found = client.signatures_in_time_window(
            program,
            incident.start_unix,
            incident.end_unix,
            page_limit,
            |page, kept| {
                if page % 5 == 0 {
                    eprintln!("    page {page}, {kept} in window so far");
                }
            },
        )?;
        progress(&format!("    {} signatures in window", found.len()));
        for record in found {
            signatures.insert((record.block_time.unwrap_or_default(), record.signature));
        }
    }

    let mut result = WindowResult {
        incident_id: incident.id.to_string(),
        protocol: incident.protocol.to_string(),
        examined: signatures.len(),
        fetched: 0,
        from_cache: 0,
        skipped: Vec::new(),
        refusals: 0,
        detections: Vec::new(),
        scan_mode: ScanMode::Signatures,
        slot_range: None,
    };

    for (index, (_, signature)) in signatures.iter().enumerate() {
        if let Some(limit) = max_transactions {
            if index >= limit {
                progress(&format!("    stopping at the {limit}-transaction limit"));
                break;
            }
        }

        let raw = match cache.get(signature) {
            Some(cached) => {
                result.from_cache += 1;
                cached
            }
            None => {
                let fetched = client.transaction(signature)?;
                cache.put(signature, &fetched)?;
                result.fetched += 1;
                fetched
            }
        };

        if raw.is_null() {
            result
                .skipped
                .push((signature.clone(), "rpc returned null".to_string()));
            continue;
        }

        let view = match normalize::tx_view(signature, &raw) {
            Ok(view) => view,
            Err(error) => {
                result.skipped.push((signature.clone(), error.to_string()));
                continue;
            }
        };

        match evaluate(policy, &view) {
            Decision::Fire(detection) => result.detections.push(*detection),
            Decision::Refuse(Refusal::IncompleteAccounts(_)) => result.refusals += 1,
            Decision::NoMatch => {}
        }
    }

    Ok(result)
}
