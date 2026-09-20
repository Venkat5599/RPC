//! The normalised transaction view.
//!
//! Every input source produces this and nothing else: historical RPC replay
//! (backtest), Yellowstone confirmed (public feed), and Aperture TxStream
//! pre-execution (live defence). Policies never see an RPC type, so the same
//! evaluator runs over all three. See ARCHITECTURE.md §3b.

use serde::{Deserialize, Serialize};

/// Completeness of address-lookup-table resolution for this transaction.
///
/// Roughly two thirds of a Kamino transaction's addresses arrive via lookup
/// tables. Evaluating a policy against an incomplete account set is how a
/// defence silently goes blind, so the tri-state is carried all the way to the
/// evaluator rather than being flattened at the edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum AltResolution {
    /// Every lookup-table address was resolved.
    Full,
    /// Some lookup tables could not be resolved. Never evaluate against this.
    Partial,
    /// The source did not report resolution status at all.
    #[serde(rename = "NONE")]
    Unknown,
}

impl AltResolution {
    /// A transaction with no lookup tables at all is trivially fully resolved.
    pub fn for_legacy_transaction() -> Self {
        AltResolution::Full
    }
}

/// One instruction, with its accounts already resolved to base58 pubkeys.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IxView {
    pub program_id: String,
    /// Account keys in instruction order, statics and ALT-loaded alike.
    pub accounts: Vec<String>,
    /// Raw instruction data. Anchor discriminator is the first 8 bytes.
    #[serde(with = "hex_bytes")]
    pub data: Vec<u8>,
    /// Index of the outer instruction this belongs to; inner instructions
    /// share their parent's index.
    pub outer_index: usize,
    pub is_inner: bool,
}

/// A token balance change attributable to one account, in raw base units.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenDelta {
    pub account: String,
    pub mint: String,
    pub owner: Option<String>,
    pub decimals: u8,
    /// Post minus pre, in raw units. Negative is an outflow.
    pub delta: i128,
}

/// One transaction, normalised. Source-agnostic by construction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxView {
    pub signature: String,
    pub slot: u64,
    /// Unix seconds. Absent on pre-execution transactions, which have not
    /// landed in a block yet.
    pub block_time: Option<i64>,
    pub alt_resolution: AltResolution,
    /// Whether the transaction succeeded. For a simulated pending transaction
    /// this is the simulation's verdict.
    pub success: bool,
    pub instructions: Vec<IxView>,
    pub token_deltas: Vec<TokenDelta>,
}

impl TxView {
    /// Instructions issued to a given program, outer and inner.
    pub fn instructions_for<'a>(&'a self, program_id: &'a str) -> impl Iterator<Item = (usize, &'a IxView)> {
        self.instructions
            .iter()
            .enumerate()
            .filter(move |(_, ix)| ix.program_id == program_id)
    }
}

/// Hex serialisation for instruction data, so committed fixtures stay readable
/// and diffable instead of being a wall of base64.
mod hex_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        let mut out = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            out.push_str(&format!("{b:02x}"));
        }
        s.serialize_str(&out)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        if s.len() % 2 != 0 {
            return Err(serde::de::Error::custom("odd-length hex string"));
        }
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(serde::de::Error::custom))
            .collect()
    }
}
