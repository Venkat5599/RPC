//! The predicate, against a real mainnet account.
//!
//! Hand-built buffers prove the parser is self-consistent. They cannot prove
//! the layout is right, because a wrong offset tested against a buffer laid
//! out at the same wrong offset agrees with itself perfectly.
//!
//! This test closes that gap: the fixture is the first 45 bytes of Kamino
//! Lend's actual `ProgramData` account, and the expected authority is what
//! Solana's own `jsonParsed` decoder reports for it. Two independent decoders
//! agreeing on real bytes is the evidence that matters, and re-fetching the
//! account reproduces it.

use anchor_lang::prelude::Pubkey;
use ripcord_vault::predicate::{parse_upgrade_authority, MINIMUM_LEN};
use std::str::FromStr;

/// Minimal base64 decoder, so the fixture stays readable in the repo without
/// pulling a dependency into a program crate for one test.
fn base64_decode(input: &str) -> Vec<u8> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut accumulator = 0u32;
    let mut bits = 0u32;
    let mut out = Vec::new();
    for byte in input.bytes() {
        if byte == b'=' {
            break;
        }
        let Some(value) = ALPHABET.iter().position(|c| *c == byte) else {
            continue; // whitespace and newlines
        };
        accumulator = (accumulator << 6) | value as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((accumulator >> bits) as u8);
        }
    }
    out
}

#[test]
fn parses_the_authority_of_a_real_mainnet_program() {
    let fixture = include_str!("../fixtures/kamino_lend_programdata_head.json");

    let head_base64 = json_string_field(fixture, "head_base64");
    let expected = json_string_field(fixture, "expected_authority");
    let owner = json_string_field(fixture, "owner");

    assert_eq!(
        owner, "BPFLoaderUpgradeab1e11111111111111111111111",
        "the fixture must be an upgradeable-loader account, or it proves nothing"
    );

    let data = base64_decode(&head_base64);
    assert!(
        data.len() >= MINIMUM_LEN,
        "fixture is {} bytes, needs at least {MINIMUM_LEN}",
        data.len()
    );

    let parsed = parse_upgrade_authority(&data).expect("real ProgramData must parse");
    assert!(!parsed.is_immutable, "Kamino Lend is upgradeable");
    assert_eq!(
        parsed.key,
        Pubkey::from_str(&expected).unwrap(),
        "the on-chain parser and Solana's own jsonParsed decoder must agree on the same bytes"
    );
}

/// Pull one string field out of the fixture without a JSON dependency.
fn json_string_field(document: &str, field: &str) -> String {
    let needle = format!("\"{field}\"");
    let start = document
        .find(&needle)
        .unwrap_or_else(|| panic!("fixture has no field {field}"));
    let after_colon = document[start + needle.len()..]
        .find(':')
        .map(|offset| start + needle.len() + offset + 1)
        .expect("field has no value");
    let rest = &document[after_colon..];
    let open = rest.find('"').expect("value is not a string");
    let remainder = &rest[open + 1..];
    let close = remainder.find('"').expect("unterminated string");
    remainder[..close].to_string()
}
