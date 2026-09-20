//! Slot-bounded block scanning.
//!
//! The replay path that actually reaches a historical window. Rather than
//! paging signatures backwards from the present, the window is resolved to a
//! slot range and those blocks are read directly, which is bounded work
//! proportional to the window rather than to how long ago it happened.
//!
//! Only transactions touching a watched program are kept, and only those are
//! cached, because caching whole blocks for a multi-hour window would store
//! tens of gigabytes to read a few thousand transactions.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use anyhow::Result;
use ripcord_policy::{evaluate, Decision, Detection, Policy, Refusal};
use serde_json::{json, Value};

use crate::cache::Cache;
use crate::engine::{ScanMode, WindowResult};
use crate::incidents::Incident;
use crate::normalize;
use crate::rpc::RpcClient;
use crate::slots::{self, SlotWindow};

/// The `getBlocks` span limit.
const MAX_BLOCK_SPAN: u64 = 500_000;

/// Scan one incident window by reading its blocks.
pub fn scan(
    client: &RpcClient,
    cache: &Cache,
    incident: &Incident,
    policy: &(dyn Policy + Sync),
    concurrency: usize,
    max_transactions: Option<usize>,
    progress: &(dyn Fn(&str) + Sync),
) -> Result<(WindowResult, SlotWindow)> {
    progress(&format!("  resolving {} to a slot range", incident.id));
    let window = slots::resolve(client, incident.start_unix, incident.end_unix)?;
    progress(&format!(
        "    slots {}..={} ({} slots), observed {} to {}",
        window.start_slot,
        window.end_slot,
        window.slots(),
        window.start_time,
        window.end_time
    ));

    // Which slots actually produced a block.
    let mut produced = Vec::new();
    let mut chunk_start = window.start_slot;
    while chunk_start <= window.end_slot {
        let chunk_end = (chunk_start + MAX_BLOCK_SPAN - 1).min(window.end_slot);
        produced.extend(client.blocks(chunk_start, chunk_end)?);
        chunk_start = chunk_end + 1;
    }
    progress(&format!("    {} blocks produced in range", produced.len()));

    let total_blocks = produced.len();
    // Report often enough on a small window to show it is alive, rarely enough
    // on a large one to stay readable.
    let progress_every = (total_blocks / 20).max(1);
    let scanned = AtomicUsize::new(0);
    let collected = Mutex::new(Collected::default());
    let workers = concurrency.max(1).min(produced.len().max(1));

    std::thread::scope(|scope| -> Result<()> {
        let mut handles = Vec::new();
        for worker in 0..workers {
            let slice: Vec<u64> = produced
                .iter()
                .skip(worker)
                .step_by(workers)
                .copied()
                .collect();
            let scanned = &scanned;
            let collected = &collected;
            handles.push(scope.spawn(move || -> Result<()> {
                for slot in slice {
                    // Stop early once the transaction budget is met. Checked
                    // per slot rather than per transaction so every block that
                    // is started is also finished.
                    if let Some(limit) = max_transactions {
                        if collected.lock().unwrap().examined >= limit {
                            return Ok(());
                        }
                    }

                    let (matched, served_from_cache) =
                        match cached_slot(client, cache, incident, slot)? {
                            Some(result) => result,
                            None => continue,
                        };
                    {
                        let mut guard = collected.lock().unwrap();
                        if served_from_cache {
                            guard.from_cache += 1;
                        } else {
                            guard.fetched += 1;
                        }
                    }

                    let done = scanned.fetch_add(1, Ordering::Relaxed) + 1;
                    if done % progress_every == 0 || done == total_blocks {
                        progress(&format!("    {done}/{total_blocks} blocks scanned"));
                    }

                    for raw in matched {
                        let signature = raw
                            .get("__signature")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string();

                        let view = match normalize::tx_view(&signature, &raw) {
                            Ok(view) => view,
                            Err(error) => {
                                let mut guard = collected.lock().unwrap();
                                guard.examined += 1;
                                guard.skipped.push((signature, error.to_string()));
                                continue;
                            }
                        };

                        let decision = evaluate(policy, &view);
                        let mut guard = collected.lock().unwrap();
                        guard.examined += 1;
                        match decision {
                            Decision::Fire(detection) => guard.detections.push(*detection),
                            Decision::Refuse(Refusal::IncompleteAccounts(_)) => {
                                guard.refusals += 1
                            }
                            Decision::NoMatch => {}
                        }
                    }
                }
                Ok(())
            }));
        }
        for handle in handles {
            handle.join().expect("scan worker panicked")?;
        }
        Ok(())
    })?;

    let Collected {
        examined,
        refusals,
        skipped,
        mut detections,
        fetched,
        from_cache,
    } = collected.into_inner().unwrap();

    detections.sort_by_key(|detection| (detection.block_time.unwrap_or(i64::MAX), detection.slot));

    Ok((
        WindowResult {
            incident_id: incident.id.to_string(),
            protocol: incident.protocol.to_string(),
            examined,
            fetched,
            from_cache,
            skipped,
            refusals,
            detections,
            scan_mode: ScanMode::Blocks,
            slot_range: Some((window.start_slot, window.end_slot)),
        },
        window,
    ))
}

#[derive(Default)]
struct Collected {
    examined: usize,
    refusals: usize,
    skipped: Vec<(String, String)>,
    detections: Vec<Detection>,
    fetched: usize,
    from_cache: usize,
}

/// Transactions in `slot` that touch a watched program, from cache when
/// possible.
///
/// The cache entry is the filtered list, not the block, and an empty list is
/// cached too — "this slot had nothing for us" is a result worth keeping, or
/// every re-run re-reads every empty block in the window.
fn cached_slot(
    client: &RpcClient,
    cache: &Cache,
    incident: &Incident,
    slot: u64,
) -> Result<Option<(Vec<Value>, bool)>> {
    let key = format!("slot-{}-{}", incident.id, slot);

    if let Some(cached) = cache.get(&key) {
        return Ok(cached.as_array().cloned().map(|list| (list, true)));
    }

    let Some(block) = client.block(slot)? else {
        cache.put(&key, &json!([]))?;
        return Ok(None);
    };

    let block_time = block.get("blockTime").and_then(Value::as_i64);
    let mut matched = Vec::new();

    if let Some(transactions) = block.get("transactions").and_then(Value::as_array) {
        for entry in transactions {
            if !touches_watched_program(entry, incident) {
                continue;
            }
            let signature = entry
                .get("transaction")
                .and_then(|t| t.get("signatures"))
                .and_then(Value::as_array)
                .and_then(|signatures| signatures.first())
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();

            // A block transaction carries no slot or block time of its own;
            // both come from the block, and the normaliser needs them.
            let mut raw = entry.clone();
            raw["slot"] = json!(slot);
            raw["blockTime"] = json!(block_time);
            raw["__signature"] = json!(signature);
            matched.push(raw);
        }
    }

    cache.put(&key, &Value::Array(matched.clone()))?;
    Ok(Some((matched, false)))
}

/// Whether a transaction involves any program this window watches.
///
/// Checked against the resolved account keys, which includes lookup-table
/// addresses — a filter on static keys alone would miss most of the traffic,
/// which is the same blindness the policy guards against.
fn touches_watched_program(entry: &Value, incident: &Incident) -> bool {
    let message = entry.get("transaction").and_then(|t| t.get("message"));

    let in_account_keys = message
        .and_then(|m| m.get("accountKeys"))
        .and_then(Value::as_array)
        .map(|keys| {
            keys.iter().any(|key| {
                key.get("pubkey")
                    .and_then(Value::as_str)
                    .map(|pubkey| incident.programs.contains(&pubkey))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);

    if in_account_keys {
        return true;
    }

    // Inner instructions can invoke a program that never appears as a
    // top-level account key.
    entry
        .get("meta")
        .and_then(|m| m.get("innerInstructions"))
        .and_then(Value::as_array)
        .map(|groups| {
            groups.iter().any(|group| {
                group
                    .get("instructions")
                    .and_then(Value::as_array)
                    .map(|list| {
                        list.iter().any(|ix| {
                            ix.get("programId")
                                .and_then(Value::as_str)
                                .map(|program| incident.programs.contains(&program))
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}
