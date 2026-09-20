//! The predicate, as pure functions over bytes.
//!
//! Separated from the instruction handlers on purpose. This is the code that
//! decides whether money moves, and it is the code most likely to be wrong in
//! a way nothing notices: an off-by-one into an account buffer still parses,
//! still returns a pubkey, and still compares — it just compares the wrong
//! thirty-two bytes. Keeping it free of `AccountInfo` means it can be tested
//! exhaustively on the host, against buffers laid out by hand.

use anchor_lang::prelude::Pubkey;

/// Layout of an upgradeable `ProgramData` account, as the loader writes it:
/// a 4-byte bincode enum tag (3 = ProgramData), the slot of the last
/// deployment, then an `Option<Pubkey>` upgrade authority.
pub const AUTHORITY_OPTION_OFFSET: usize = 4 + 8;
pub const AUTHORITY_OFFSET: usize = AUTHORITY_OPTION_OFFSET + 1;
pub const MINIMUM_LEN: usize = AUTHORITY_OFFSET + 32;
pub const PROGRAM_DATA_TAG: u32 = 3;

/// Why a buffer could not be read as `ProgramData`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseFailure {
    /// Shorter than a `ProgramData` account can possibly be.
    TooShort,
    /// The bincode variant tag is not `ProgramData`.
    WrongVariant(u32),
    /// The `Option` discriminant is neither 0 nor 1, so the account is not
    /// what it claims to be.
    MalformedOption(u8),
}

/// The upgrade authority of a program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Authority {
    /// The current authority. Meaningless when `is_immutable`.
    pub key: Pubkey,
    /// Whether the program can no longer be upgraded by anyone.
    pub is_immutable: bool,
}

impl Authority {
    pub fn immutable() -> Self {
        Self {
            key: Pubkey::default(),
            is_immutable: true,
        }
    }

    pub fn held_by(key: Pubkey) -> Self {
        Self {
            key,
            is_immutable: false,
        }
    }
}

/// Parse the upgrade authority out of a `ProgramData` buffer.
///
/// Every failure is explicit. In particular a malformed buffer never returns
/// the default pubkey, because the default pubkey is exactly what an unset
/// baseline holds, and "parsed as garbage" must never compare equal to
/// "nothing recorded yet".
pub fn parse_upgrade_authority(data: &[u8]) -> Result<Authority, ParseFailure> {
    if data.len() < MINIMUM_LEN {
        return Err(ParseFailure::TooShort);
    }

    let tag = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    if tag != PROGRAM_DATA_TAG {
        return Err(ParseFailure::WrongVariant(tag));
    }

    match data[AUTHORITY_OPTION_OFFSET] {
        0 => Ok(Authority::immutable()),
        1 => {
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(&data[AUTHORITY_OFFSET..AUTHORITY_OFFSET + 32]);
            Ok(Authority::held_by(Pubkey::new_from_array(bytes)))
        }
        other => Err(ParseFailure::MalformedOption(other)),
    }
}

/// Whether authority moved between arming and now.
///
/// Handing a program to a new key is the obvious case. Renouncing upgrade
/// authority entirely is the less obvious one, and it counts: a depositor who
/// armed against "this team controls the program" has had that assumption
/// changed either way, and an attacker who takes an authority and then burns
/// it must not be able to launder the transition into silence.
pub fn authority_changed(baseline: Authority, observed: Authority) -> bool {
    baseline.is_immutable != observed.is_immutable
        || (!observed.is_immutable && baseline.key != observed.key)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `ProgramData` buffer with the given authority option.
    fn program_data(tag: u32, option: u8, authority: [u8; 32], trailing: usize) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&tag.to_le_bytes());
        data.extend_from_slice(&7_000_000u64.to_le_bytes()); // last deploy slot
        data.push(option);
        data.extend_from_slice(&authority);
        data.extend(std::iter::repeat(0u8).take(trailing)); // the program bytes
        data
    }

    fn key(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    #[test]
    fn reads_the_authority_at_the_documented_offset() {
        let data = program_data(PROGRAM_DATA_TAG, 1, key(7), 4096);
        let parsed = parse_upgrade_authority(&data).unwrap();
        assert!(!parsed.is_immutable);
        assert_eq!(parsed.key, Pubkey::new_from_array(key(7)));
    }

    #[test]
    fn an_immutable_program_is_reported_as_immutable_not_as_a_default_key() {
        let data = program_data(PROGRAM_DATA_TAG, 0, [0u8; 32], 0);
        let parsed = parse_upgrade_authority(&data).unwrap();
        assert!(parsed.is_immutable);
    }

    #[test]
    fn a_short_buffer_fails_rather_than_parsing() {
        let mut data = program_data(PROGRAM_DATA_TAG, 1, key(3), 0);
        data.truncate(MINIMUM_LEN - 1);
        assert_eq!(parse_upgrade_authority(&data), Err(ParseFailure::TooShort));
    }

    #[test]
    fn a_non_program_data_variant_fails() {
        // Tag 2 is Program, which points at ProgramData rather than holding
        // the authority. Reading it as ProgramData would return arbitrary
        // bytes as an authority.
        let data = program_data(2, 1, key(9), 64);
        assert_eq!(
            parse_upgrade_authority(&data),
            Err(ParseFailure::WrongVariant(2))
        );
    }

    #[test]
    fn a_malformed_option_discriminant_fails() {
        let data = program_data(PROGRAM_DATA_TAG, 2, key(1), 64);
        assert_eq!(
            parse_upgrade_authority(&data),
            Err(ParseFailure::MalformedOption(2))
        );
    }

    #[test]
    fn a_handover_to_a_new_key_is_a_change() {
        let baseline = Authority::held_by(Pubkey::new_from_array(key(1)));
        let observed = Authority::held_by(Pubkey::new_from_array(key(2)));
        assert!(authority_changed(baseline, observed));
    }

    #[test]
    fn an_unchanged_authority_is_not_a_change() {
        let authority = Authority::held_by(Pubkey::new_from_array(key(1)));
        assert!(
            !authority_changed(authority, authority),
            "an unchanged authority must leave the predicate false, or the guardian can fire at will"
        );
    }

    #[test]
    fn renouncing_authority_is_a_change() {
        let baseline = Authority::held_by(Pubkey::new_from_array(key(1)));
        assert!(
            authority_changed(baseline, Authority::immutable()),
            "burning the authority changes the assumption the depositor armed against"
        );
    }

    #[test]
    fn gaining_an_authority_on_an_immutable_program_is_a_change() {
        // Not currently possible on-chain, but the predicate should not be the
        // thing that assumes so.
        let observed = Authority::held_by(Pubkey::new_from_array(key(4)));
        assert!(authority_changed(Authority::immutable(), observed));
    }

    #[test]
    fn two_immutable_observations_are_not_a_change() {
        assert!(!authority_changed(
            Authority::immutable(),
            Authority::immutable()
        ));
    }

    #[test]
    fn the_key_is_ignored_while_immutable() {
        // An immutable authority carries no meaningful key, so a stale key in
        // the baseline must not fire the predicate on its own.
        let baseline = Authority {
            key: Pubkey::new_from_array(key(8)),
            is_immutable: true,
        };
        assert!(!authority_changed(baseline, Authority::immutable()));
    }
}
