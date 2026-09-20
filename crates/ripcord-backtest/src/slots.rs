//! Resolving a time window to a slot range.
//!
//! Signature pagination walks backwards from the present, so reaching a window
//! in 2024 means paging through millions of signatures — it does not work at
//! any level of patience. The block scanner needs slot bounds instead, and
//! those come from the chain: binary search `getBlockTime` until the window's
//! edges are located.
//!
//! Skipped slots have no block and therefore no time, so the search probes
//! outwards from a missed slot rather than treating the gap as an answer.

use anyhow::{bail, Result};

use crate::rpc::RpcClient;

/// A slot range covering a time window, with the times actually observed at
/// each end so a report can show what was really scanned rather than what was
/// requested.
#[derive(Debug, Clone, Copy)]
pub struct SlotWindow {
    pub start_slot: u64,
    pub end_slot: u64,
    pub start_time: i64,
    pub end_time: i64,
}

impl SlotWindow {
    pub fn slots(&self) -> u64 {
        self.end_slot.saturating_sub(self.start_slot) + 1
    }
}

/// How far to probe either side of a skipped slot before giving up on it.
const SKIP_PROBE_RADIUS: u64 = 256;

/// Block time for `slot`, or for the nearest slot within the probe radius that
/// actually produced a block.
fn nearby_block_time(client: &RpcClient, slot: u64, ceiling: u64) -> Result<Option<(u64, i64)>> {
    if let Some(time) = client.block_time(slot)? {
        return Ok(Some((slot, time)));
    }
    for offset in 1..=SKIP_PROBE_RADIUS {
        let forward = slot.saturating_add(offset);
        if forward <= ceiling {
            if let Some(time) = client.block_time(forward)? {
                return Ok(Some((forward, time)));
            }
        }
        let backward = slot.saturating_sub(offset);
        if backward > 0 {
            if let Some(time) = client.block_time(backward)? {
                return Ok(Some((backward, time)));
            }
        }
    }
    Ok(None)
}

/// The first slot at or after `target_unix`.
///
/// Binary search over the slot axis. The endpoint must serve block times for
/// the era being searched; an endpoint without history fails here loudly
/// rather than silently returning a recent slot and producing an empty window.
pub fn slot_at_or_after(client: &RpcClient, target_unix: i64, ceiling: u64) -> Result<(u64, i64)> {
    let mut low = 1u64;
    let mut high = ceiling;
    let mut best: Option<(u64, i64)> = None;

    while low <= high {
        let middle = low + (high - low) / 2;
        let Some((probed_slot, probed_time)) = nearby_block_time(client, middle, ceiling)? else {
            // A long stretch with no blocks at all. Move up and keep going.
            low = middle + SKIP_PROBE_RADIUS + 1;
            continue;
        };

        if probed_time >= target_unix {
            best = Some((probed_slot, probed_time));
            if probed_slot == 0 {
                break;
            }
            high = probed_slot.saturating_sub(1);
        } else {
            low = probed_slot + 1;
        }
    }

    best.ok_or_else(|| {
        anyhow::anyhow!(
            "no block found at or after {target_unix}: the endpoint does not serve history for \
             this window"
        )
    })
}

/// Resolve a time window to the slot range that covers it.
pub fn resolve(client: &RpcClient, start_unix: i64, end_unix: i64) -> Result<SlotWindow> {
    if start_unix >= end_unix {
        bail!("window must run forwards: {start_unix} to {end_unix}");
    }

    let tip = client.current_slot()?;
    let tip_time = nearby_block_time(client, tip, tip)?
        .map(|(_, time)| time)
        .unwrap_or(i64::MAX);
    if tip_time < start_unix {
        bail!(
            "the window starts after the chain tip: requested {start_unix}, tip is at {tip_time}"
        );
    }

    let (start_slot, start_time) = slot_at_or_after(client, start_unix, tip)?;
    // The end of the window is the slot before the first slot after it.
    let (end_slot, end_time) = match slot_at_or_after(client, end_unix + 1, tip) {
        Ok((slot, _)) => {
            let end_slot = slot.saturating_sub(1).max(start_slot);
            let end_time = nearby_block_time(client, end_slot, tip)?
                .map(|(_, time)| time)
                .unwrap_or(end_unix);
            (end_slot, end_time)
        }
        // The window runs to the tip.
        Err(_) => (tip, tip_time),
    };

    Ok(SlotWindow {
        start_slot,
        end_slot,
        start_time,
        end_time,
    })
}
