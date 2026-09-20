//! Minimal Solana JSON-RPC client.
//!
//! Two methods only, because the backtest needs exactly two:
//! `getSignaturesForAddress` to enumerate a bounded incident window, and
//! `getTransaction` to fetch each one. Public mainnet RPC rate-limits hard, so
//! every call backs off and retries rather than dropping transactions — a
//! silently short window would understate the incident and make the headline
//! number wrong.

use std::thread::sleep;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};

pub const PUBLIC_MAINNET: &str = "https://api.mainnet-beta.solana.com";

/// Highest transaction version this client will accept.
///
/// Not cosmetic: `getBlock` rejects the entire block if it contains a
/// transaction newer than this, so a stale ceiling does not degrade the scan,
/// it stops it. Mainnet began producing version-1 transactions before this was
/// written, and none of the project docs mention them.
pub const MAX_TRANSACTION_VERSION: u8 = 1;

pub struct RpcClient {
    endpoint: String,
    agent: ureq::Agent,
    /// Pause between successful calls. Public RPC tolerates roughly this.
    throttle: Duration,
    max_retries: u32,
}

/// One entry from `getSignaturesForAddress`.
#[derive(Debug, Clone)]
pub struct SignatureRecord {
    pub signature: String,
    pub slot: u64,
    pub block_time: Option<i64>,
    pub failed: bool,
}

impl RpcClient {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            agent: ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(30))
                .build(),
            throttle: Duration::from_millis(250),
            max_retries: 8,
        }
    }

    /// Endpoint from `RIPCORD_RPC_URL`, falling back to public mainnet.
    pub fn from_env() -> Self {
        Self::new(std::env::var("RIPCORD_RPC_URL").unwrap_or_else(|_| PUBLIC_MAINNET.to_string()))
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Set the pause between calls. A paid endpoint can drop this to zero.
    pub fn with_throttle(mut self, throttle: Duration) -> Self {
        self.throttle = throttle;
        self
    }

    fn call(&self, method: &str, params: Value) -> Result<Value> {
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });

        let mut backoff = Duration::from_millis(500);
        let mut last_error = None;

        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                sleep(backoff);
                backoff = (backoff * 2).min(Duration::from_secs(20));
            }

            match self.agent.post(&self.endpoint).send_json(body.clone()) {
                Ok(response) => {
                    let parsed: Value = response
                        .into_json()
                        .with_context(|| format!("{method}: response was not JSON"))?;
                    if let Some(error) = parsed.get("error") {
                        // A rate-limit error is worth retrying; a malformed
                        // request is not, and retrying it wastes the budget.
                        let code = error.get("code").and_then(Value::as_i64).unwrap_or(0);
                        if code == 429 || code == -32005 {
                            last_error = Some(anyhow!("{method}: rate limited: {error}"));
                            continue;
                        }
                        bail!("{method}: rpc error: {error}");
                    }
                    sleep(self.throttle);
                    return parsed
                        .get("result")
                        .cloned()
                        .ok_or_else(|| anyhow!("{method}: response had no result field"));
                }
                Err(ureq::Error::Status(429, _)) => {
                    last_error = Some(anyhow!("{method}: HTTP 429"));
                }
                Err(ureq::Error::Status(status, _)) if (500..600).contains(&status) => {
                    last_error = Some(anyhow!("{method}: HTTP {status}"));
                }
                // A connection reset is how a rate limiter often says no: the
                // socket dies rather than returning 429. Treating it as fatal
                // ends a long scan on a condition that clears by waiting.
                Err(error @ ureq::Error::Transport(_)) => {
                    last_error = Some(anyhow!("{method}: {error}"));
                }
                Err(other) => return Err(anyhow!("{method}: {other}")),
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("{method}: exhausted retries")))
    }

    /// Every signature touching `address` whose block time falls inside
    /// `[start_unix, end_unix]`.
    ///
    /// Time rather than slot is the unit on purpose: the incident windows come
    /// from published UTC timings, and converting those to slot numbers
    /// off-chain would be inventing precision the sources do not have. The
    /// chain reports `blockTime` per signature, so the window is applied to
    /// the value the chain itself gives.
    pub fn signatures_in_time_window(
        &self,
        address: &str,
        start_unix: i64,
        end_unix: i64,
        page_limit: usize,
        mut on_page: impl FnMut(usize, usize),
    ) -> Result<Vec<SignatureRecord>> {
        let mut collected = Vec::new();
        let mut before: Option<String> = None;
        let mut pages = 0usize;

        loop {
            let params = json!([
                address,
                { "limit": page_limit, "before": before, "commitment": "confirmed" }
            ]);
            let page = self.call("getSignaturesForAddress", params)?;
            let entries = page.as_array().cloned().unwrap_or_default();
            if entries.is_empty() {
                break;
            }
            pages += 1;

            let mut oldest_time = i64::MAX;
            let mut last_signature = None;

            for entry in &entries {
                let signature = entry
                    .get("signature")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let block_time = entry.get("blockTime").and_then(Value::as_i64);
                last_signature = Some(signature.clone());
                if let Some(time) = block_time {
                    oldest_time = oldest_time.min(time);
                    if time >= start_unix && time <= end_unix {
                        collected.push(SignatureRecord {
                            signature,
                            slot: entry.get("slot").and_then(Value::as_u64).unwrap_or(0),
                            block_time,
                            failed: entry.get("err").map(|e| !e.is_null()).unwrap_or(false),
                        });
                    }
                }
            }

            on_page(pages, collected.len());

            // Paged back past the start of the window, or ran out of history.
            if oldest_time < start_unix || entries.len() < page_limit {
                break;
            }
            before = last_signature;
        }

        Ok(collected)
    }

    /// One transaction, parsed, with version-0 support enabled so lookup-table
    /// addresses come back resolved in `meta.loadedAddresses`.
    pub fn transaction(&self, signature: &str) -> Result<Value> {
        self.call(
            "getTransaction",
            json!([
                signature,
                {
                    "encoding": "jsonParsed",
                    "commitment": "confirmed",
                    // Mainnet carries version-1 transactions as of 2026. Asking
                    // for 0 makes the RPC reject the whole block rather than
                    // skip the transaction, so the ceiling tracks the chain.
                    "maxSupportedTransactionVersion": MAX_TRANSACTION_VERSION
                }
            ]),
        )
    }
}

/// What the chain says about a deployed program.
#[derive(Debug, Clone)]
pub struct ProgramInfo {
    pub executable: bool,
    /// The ProgramData account, which is what a loader `SetAuthority` names.
    pub program_data: String,
    /// Current upgrade authority, or `None` when the program is immutable.
    pub upgrade_authority: Option<String>,
}

impl RpcClient {
    /// Resolve a program to its ProgramData account and current upgrade
    /// authority, or `None` when the account does not exist.
    ///
    /// This is the guard against watching an address that was typed from
    /// memory: an ID that does not resolve, or resolves to something that is
    /// not an upgradeable program, can never produce a real detection.
    pub fn program_authority(&self, program_id: &str) -> Result<Option<ProgramInfo>> {
        let account = self.account_info(program_id)?;
        let Some(account) = account else {
            return Ok(None);
        };

        let executable = account
            .get("executable")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let program_data = account
            .get("data")
            .and_then(|d| d.get("parsed"))
            .and_then(|p| p.get("info"))
            .and_then(|i| i.get("programData"))
            .and_then(Value::as_str)
            .map(str::to_string);

        let Some(program_data) = program_data else {
            // Not an upgradeable program: a native program, a non-loader
            // account, or a loader-v2 program with no authority at all.
            return Ok(Some(ProgramInfo {
                executable,
                program_data: String::new(),
                upgrade_authority: None,
            }));
        };

        let upgrade_authority = self
            .account_info(&program_data)?
            .and_then(|data_account| {
                data_account
                    .get("data")
                    .and_then(|d| d.get("parsed"))
                    .and_then(|p| p.get("info"))
                    .and_then(|i| i.get("authority"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            });

        Ok(Some(ProgramInfo {
            executable,
            program_data,
            upgrade_authority,
        }))
    }

    fn account_info(&self, address: &str) -> Result<Option<Value>> {
        let result = self.call(
            "getAccountInfo",
            json!([address, { "encoding": "jsonParsed", "commitment": "confirmed" }]),
        )?;
        Ok(result.get("value").filter(|v| !v.is_null()).cloned())
    }
}

impl RpcClient {
    /// Current confirmed slot.
    pub fn current_slot(&self) -> Result<u64> {
        let result = self.call("getSlot", json!([{ "commitment": "confirmed" }]))?;
        result
            .as_u64()
            .ok_or_else(|| anyhow!("getSlot returned a non-integer"))
    }

    /// Block time for a slot, or `None` when the slot was skipped.
    ///
    /// A skipped slot is a normal, frequent condition, not an error: it must
    /// not abort a binary search. An endpoint that has pruned the slot reports
    /// the same way, which is why the caller checks that the window it got
    /// back is the window it asked for.
    pub fn block_time(&self, slot: u64) -> Result<Option<i64>> {
        match self.call("getBlockTime", json!([slot])) {
            Ok(value) => Ok(value.as_i64()),
            Err(error) => {
                let text = error.to_string();
                if text.contains("-32009") || text.contains("-32007") || text.contains("skipped") {
                    Ok(None)
                } else {
                    Err(error)
                }
            }
        }
    }

    /// Confirmed block slots in `[start, end]`.
    ///
    /// The RPC caps the span at 500,000 slots per call, so longer windows are
    /// requested in chunks by the caller.
    pub fn blocks(&self, start: u64, end: u64) -> Result<Vec<u64>> {
        let result = self.call("getBlocks", json!([start, end, { "commitment": "confirmed" }]))?;
        Ok(result
            .as_array()
            .map(|slots| slots.iter().filter_map(Value::as_u64).collect())
            .unwrap_or_default())
    }

    /// A full block, parsed, with version-0 transactions resolved.
    ///
    /// Rewards are excluded because nothing here reads them and they are a
    /// large share of the payload.
    pub fn block(&self, slot: u64) -> Result<Option<Value>> {
        match self.call(
            "getBlock",
            json!([
                slot,
                {
                    "encoding": "jsonParsed",
                    "transactionDetails": "full",
                    "rewards": false,
                    "commitment": "confirmed",
                    "maxSupportedTransactionVersion": MAX_TRANSACTION_VERSION
                }
            ]),
        ) {
            Ok(value) if value.is_null() => Ok(None),
            Ok(value) => Ok(Some(value)),
            Err(error) => {
                let text = error.to_string();
                if text.contains("-32009") || text.contains("-32007") || text.contains("skipped") {
                    Ok(None)
                } else {
                    Err(error)
                }
            }
        }
    }
}
