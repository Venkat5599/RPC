//! Historical RPC transaction to [`TxView`].
//!
//! This is the seam that makes the backtest meaningful: everything
//! source-specific lives here, and what comes out the other side is the same
//! structure the live TxStream path will produce. The policy code cannot tell
//! which source it is reading, which is why a backtested number says something
//! about the production engine (ARCHITECTURE.md §3b).
//!
//! Two RPC details shape this module.
//!
//! **`jsonParsed` pre-decodes some programs.** BPF Loader Upgradeable is one
//! of them, so a `SetAuthority` arrives as `{"parsed": {"type": "setAuthority",
//! ...}}` with no raw data or account list at all. Reading only the raw form
//! would miss every authority change on mainnet, so both shapes are handled
//! and the parsed one is re-encoded back into the raw layout the decoder
//! expects.
//!
//! **Lookup-table resolution is not guaranteed.** A version-0 transaction that
//! used lookup tables but came back without `meta.loadedAddresses` is not
//! "probably fine" — it is an incomplete account set, and it is marked as such
//! so the policy refuses instead of quietly evaluating against two thirds of
//! the accounts.

use anyhow::{anyhow, Result};
use ripcord_policy::loader::{LoaderIx, BPF_LOADER_UPGRADEABLE};
use ripcord_policy::{AltResolution, IxView, TokenDelta, TxView};
use serde_json::Value;

/// Build a [`TxView`] from one `getTransaction` result.
pub fn tx_view(signature: &str, raw: &Value) -> Result<TxView> {
    let slot = raw
        .get("slot")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("{signature}: transaction had no slot"))?;
    let block_time = raw.get("blockTime").and_then(Value::as_i64);
    let meta = raw.get("meta");
    let success = meta
        .and_then(|m| m.get("err"))
        .map(Value::is_null)
        .unwrap_or(false);

    let message = raw
        .get("transaction")
        .and_then(|t| t.get("message"))
        .ok_or_else(|| anyhow!("{signature}: transaction had no message"))?;

    let mut instructions = Vec::new();
    if let Some(outer) = message.get("instructions").and_then(Value::as_array) {
        for (index, ix) in outer.iter().enumerate() {
            if let Some(view) = instruction_view(ix, index, false) {
                instructions.push(view);
            }
        }
    }
    if let Some(inner_groups) = meta
        .and_then(|m| m.get("innerInstructions"))
        .and_then(Value::as_array)
    {
        for group in inner_groups {
            let parent = group
                .get("index")
                .and_then(Value::as_u64)
                .unwrap_or_default() as usize;
            let Some(list) = group.get("instructions").and_then(Value::as_array) else {
                continue;
            };
            for ix in list {
                if let Some(view) = instruction_view(ix, parent, true) {
                    instructions.push(view);
                }
            }
        }
    }

    Ok(TxView {
        signature: signature.to_string(),
        slot,
        block_time,
        alt_resolution: alt_resolution(raw),
        success,
        instructions,
        token_deltas: token_deltas(meta),
    })
}

/// How complete the account set is.
///
/// A legacy transaction has no lookup tables, so it is trivially complete. A
/// version-0 transaction is complete only when the RPC actually returned the
/// resolved addresses; anything else is unknown, never assumed.
fn alt_resolution(raw: &Value) -> AltResolution {
    let message = raw.get("transaction").and_then(|t| t.get("message"));

    let is_legacy = match raw.get("version") {
        None => true,
        Some(Value::String(version)) => version == "legacy",
        _ => false,
    };
    if is_legacy {
        return AltResolution::for_legacy_transaction();
    }

    // How many addresses the transaction says it pulled from lookup tables.
    let expected: usize = message
        .and_then(|m| m.get("addressTableLookups"))
        .and_then(Value::as_array)
        .map(|lookups| {
            lookups
                .iter()
                .map(|lookup| {
                    let count = |field: &str| {
                        lookup
                            .get(field)
                            .and_then(Value::as_array)
                            .map(Vec::len)
                            .unwrap_or(0)
                    };
                    count("writableIndexes") + count("readonlyIndexes")
                })
                .sum()
        })
        .unwrap_or(0);

    if expected == 0 {
        return AltResolution::Full;
    }

    // How many actually came back resolved. `jsonParsed` merges them into
    // `accountKeys` tagged `source: "lookupTable"`; the binary encodings put
    // them in `meta.loadedAddresses` instead. Both are checked, because
    // reading only one of them is exactly how a feed goes silently blind to
    // two thirds of a Kamino transaction.
    let from_account_keys = message
        .and_then(|m| m.get("accountKeys"))
        .and_then(Value::as_array)
        .map(|keys| {
            keys.iter()
                .filter(|key| key.get("source").and_then(Value::as_str) == Some("lookupTable"))
                .count()
        })
        .unwrap_or(0);

    let from_loaded_addresses = raw
        .get("meta")
        .and_then(|m| m.get("loadedAddresses"))
        .map(|loaded| {
            let count = |field: &str| {
                loaded
                    .get(field)
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0)
            };
            count("writable") + count("readonly")
        })
        .unwrap_or(0);

    let resolved = from_account_keys.max(from_loaded_addresses);

    if resolved >= expected {
        AltResolution::Full
    } else {
        // Some lookup addresses are missing. Never "probably fine".
        AltResolution::Partial
    }
}

/// One instruction, from either the raw or the pre-parsed `jsonParsed` shape.
fn instruction_view(ix: &Value, outer_index: usize, is_inner: bool) -> Option<IxView> {
    let program_id = ix.get("programId").and_then(Value::as_str)?.to_string();

    // Raw shape: accounts as base58 strings, data as a base58 blob.
    if let (Some(accounts), Some(data)) = (
        ix.get("accounts").and_then(Value::as_array),
        ix.get("data").and_then(Value::as_str),
    ) {
        return Some(IxView {
            program_id,
            accounts: accounts
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect(),
            data: bs58::decode(data).into_vec().unwrap_or_default(),
            outer_index,
            is_inner,
        });
    }

    // Parsed shape: reconstruct the layout the loader decoder expects.
    let parsed = ix.get("parsed")?;
    if program_id == BPF_LOADER_UPGRADEABLE {
        return parsed_loader_instruction(parsed, program_id, outer_index, is_inner);
    }

    // Any other pre-parsed program (system, spl-token) carries no authority
    // change we act on. Keep it with an empty account list rather than
    // dropping it, so instruction indices stay meaningful as evidence.
    Some(IxView {
        program_id,
        accounts: Vec::new(),
        data: Vec::new(),
        outer_index,
        is_inner,
    })
}

/// Re-encode a parsed BPF Loader Upgradeable instruction into the raw form.
///
/// `jsonParsed` throws away both the tag and the account order, so both are
/// rebuilt here: the tag from the instruction type, and the accounts in the
/// order the on-chain instruction defines, which is what the decoder indexes
/// into.
fn parsed_loader_instruction(
    parsed: &Value,
    program_id: String,
    outer_index: usize,
    is_inner: bool,
) -> Option<IxView> {
    let kind = parsed.get("type").and_then(Value::as_str)?;
    let info = parsed.get("info");

    let (tag, accounts) = match kind {
        "setAuthority" | "setAuthorityChecked" => {
            let info = info?;
            let account = field(info, &["account", "programData", "buffer"])?;
            let authority = field(info, &["authority", "currentAuthority"]).unwrap_or_default();
            let mut accounts = vec![account, authority];
            if let Some(new_authority) = field(info, &["newAuthority"]) {
                accounts.push(new_authority);
            }
            let tag = if kind == "setAuthority" {
                LoaderIx::SetAuthority
            } else {
                LoaderIx::SetAuthorityChecked
            };
            (tag, accounts)
        }
        "upgrade" => {
            let info = info?;
            let account = field(info, &["programData", "account"]).unwrap_or_default();
            (LoaderIx::Upgrade, vec![account])
        }
        // Deploys, writes and closes are not authority changes; they are kept
        // only so the instruction index still lines up.
        _ => return None,
    };

    Some(IxView {
        program_id,
        accounts,
        data: (tag as u32).to_le_bytes().to_vec(),
        outer_index,
        is_inner,
    })
}

/// First present string field among several candidate names, because the
/// parser has renamed these fields across Agave versions.
fn field(info: &Value, names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| info.get(*name).and_then(Value::as_str))
        .map(str::to_string)
}

/// Token balance changes, post minus pre, keyed by the account index the
/// balances refer to.
fn token_deltas(meta: Option<&Value>) -> Vec<TokenDelta> {
    let Some(meta) = meta else {
        return Vec::new();
    };
    let pre = balances(meta.get("preTokenBalances"));
    let post = balances(meta.get("postTokenBalances"));

    let mut deltas = Vec::new();
    for (index, (mint, owner, decimals, post_amount)) in &post {
        let pre_amount = pre
            .get(index)
            .map(|(_, _, _, amount)| *amount)
            .unwrap_or(0);
        if post_amount != &pre_amount {
            deltas.push(TokenDelta {
                account: format!("index:{index}"),
                mint: mint.clone(),
                owner: owner.clone(),
                decimals: *decimals,
                delta: post_amount - pre_amount,
            });
        }
    }
    deltas
}

type Balance = (String, Option<String>, u8, i128);

fn balances(value: Option<&Value>) -> std::collections::BTreeMap<u64, Balance> {
    let mut out = std::collections::BTreeMap::new();
    let Some(list) = value.and_then(Value::as_array) else {
        return out;
    };
    for entry in list {
        let Some(index) = entry.get("accountIndex").and_then(Value::as_u64) else {
            continue;
        };
        let mint = entry
            .get("mint")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let owner = entry
            .get("owner")
            .and_then(Value::as_str)
            .map(str::to_string);
        let amount_info = entry.get("uiTokenAmount");
        let decimals = amount_info
            .and_then(|a| a.get("decimals"))
            .and_then(Value::as_u64)
            .unwrap_or(0) as u8;
        let amount = amount_info
            .and_then(|a| a.get("amount"))
            .and_then(Value::as_str)
            .and_then(|a| a.parse::<i128>().ok())
            .unwrap_or(0);
        out.insert(index, (mint, owner, decimals, amount));
    }
    out
}
