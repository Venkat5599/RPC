//! Anchor instruction discriminators.
//!
//! Anchor prefixes every instruction with `sha256("global:<ix_name>")[0..8]`.
//! Decoding by name means the backtest can match admin instructions on
//! protocols whose IDL we do not ship.

use sha2::{Digest, Sha256};

/// The 8-byte discriminator Anchor emits for a global instruction.
pub fn discriminator(ix_name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{ix_name}").as_bytes());
    let digest = hasher.finalize();
    let mut out = [0u8; 8];
    out.copy_from_slice(&digest[..8]);
    out
}

/// Whether instruction data carries the discriminator for `ix_name`.
pub fn matches(data: &[u8], ix_name: &str) -> bool {
    data.len() >= 8 && data[..8] == discriminator(ix_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discriminator_is_the_documented_sha256_prefix() {
        // sha256("global:initialize") begins afaf6d1f0d989bed.
        assert_eq!(
            discriminator("initialize"),
            [0xaf, 0xaf, 0x6d, 0x1f, 0x0d, 0x98, 0x9b, 0xed]
        );
    }

    #[test]
    fn short_data_never_matches() {
        assert!(!matches(&[0xaf, 0xaf], "initialize"));
    }
}
