//! Lookup-table completeness, tested against the shapes mainnet actually
//! returns.
//!
//! This is the regression guard for a bug that already happened here: the
//! first implementation read `meta.loadedAddresses`, which `jsonParsed` does
//! not populate, so every Kamino transaction was marked incomplete and the
//! policy refused all of them. The engine failed safe, but it was blind — the
//! exact failure ARCHITECTURE.md warns about, where a stream silently sees
//! only the static third of a transaction.

use ripcord_backtest::normalize::tx_view;
use ripcord_policy::AltResolution;
use serde_json::{json, Value};

/// A version-0 transaction whose lookup addresses are merged into
/// `accountKeys`, which is what `jsonParsed` returns.
fn parsed_shape(writable_indexes: usize, readonly_indexes: usize, resolved: usize) -> Value {
    let mut account_keys: Vec<Value> = vec![json!({
        "pubkey": "BGEwJDzDktAsFHo2h2bBEXKUyfQGUV8eN87EceqPkmEy",
        "signer": true,
        "writable": true,
        "source": "transaction"
    })];
    for index in 0..resolved {
        account_keys.push(json!({
            "pubkey": format!("LookupResolved{index}"),
            "signer": false,
            "writable": false,
            "source": "lookupTable"
        }));
    }

    json!({
        "slot": 372_000_000u64,
        "blockTime": 1_789_000_000i64,
        "version": 0,
        "meta": { "err": null },
        "transaction": {
            "message": {
                "accountKeys": account_keys,
                "instructions": [],
                "addressTableLookups": [{
                    "accountKey": "A2mdcXEdWTNjYL33eDstwfsUR5rmJeyWxMKYryBeExxK",
                    "writableIndexes": vec![0u8; writable_indexes],
                    "readonlyIndexes": vec![0u8; readonly_indexes]
                }]
            }
        }
    })
}

#[test]
fn json_parsed_lookup_addresses_count_as_fully_resolved() {
    // The real proportions from a live Kamino transaction: 9 writable and 8
    // readonly lookup addresses, all 17 present in accountKeys.
    let raw = parsed_shape(9, 8, 17);
    let view = tx_view("sig", &raw).unwrap();
    assert_eq!(
        view.alt_resolution,
        AltResolution::Full,
        "addresses merged into accountKeys are resolved; refusing them blinds the engine"
    );
}

#[test]
fn missing_lookup_addresses_are_partial() {
    // The table names 17 addresses and only 10 came back.
    let raw = parsed_shape(9, 8, 10);
    let view = tx_view("sig", &raw).unwrap();
    assert_eq!(
        view.alt_resolution,
        AltResolution::Partial,
        "an incomplete account set must never be treated as complete"
    );
}

#[test]
fn binary_encoding_loaded_addresses_also_count() {
    // Base64/base58 encodings put the resolved set in meta.loadedAddresses
    // instead, with no lookupTable entries in accountKeys at all.
    let mut raw = parsed_shape(2, 1, 0);
    raw["meta"]["loadedAddresses"] = json!({
        "writable": ["W1", "W2"],
        "readonly": ["R1"]
    });
    let view = tx_view("sig", &raw).unwrap();
    assert_eq!(view.alt_resolution, AltResolution::Full);
}

#[test]
fn a_transaction_using_no_lookup_tables_is_complete() {
    let raw = json!({
        "slot": 372_000_000u64,
        "blockTime": 1_789_000_000i64,
        "version": 0,
        "meta": { "err": null },
        "transaction": { "message": {
            "accountKeys": [],
            "instructions": [],
            "addressTableLookups": []
        }}
    });
    let view = tx_view("sig", &raw).unwrap();
    assert_eq!(view.alt_resolution, AltResolution::Full);
}

#[test]
fn a_legacy_transaction_is_complete() {
    let raw = json!({
        "slot": 200_000_000u64,
        "blockTime": 1_700_000_000i64,
        "meta": { "err": null },
        "transaction": { "message": { "accountKeys": [], "instructions": [] } }
    });
    let view = tx_view("sig", &raw).unwrap();
    assert_eq!(view.alt_resolution, AltResolution::Full);
}
