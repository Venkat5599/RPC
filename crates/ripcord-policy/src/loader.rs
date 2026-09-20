//! BPF Loader Upgradeable instruction decoding.
//!
//! The loader is not an Anchor program: its instructions are a bincode-encoded
//! u32 enum tag. `SetAuthority` and `SetAuthorityChecked` are the two that
//! change who may upgrade a deployed program, which is the whole of the
//! authority-change policy's on-chain surface.

pub const BPF_LOADER_UPGRADEABLE: &str = "BPFLoaderUpgradeab1e11111111111111111111111";

/// The loader instructions we care about. Tags are the on-chain enum order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoaderIx {
    InitializeBuffer,
    Write,
    DeployWithMaxDataLen,
    Upgrade,
    SetAuthority,
    Close,
    ExtendProgram,
    SetAuthorityChecked,
}

impl LoaderIx {
    /// Whether this instruction transfers upgrade authority.
    pub fn is_authority_change(self) -> bool {
        matches!(self, LoaderIx::SetAuthority | LoaderIx::SetAuthorityChecked)
    }
}

/// Decode the leading u32 tag. Returns `None` for data that is too short or
/// carries a tag outside the known set, which is how a loader upgrade that
/// adds instructions degrades: unknown, not misread.
pub fn decode(data: &[u8]) -> Option<LoaderIx> {
    if data.len() < 4 {
        return None;
    }
    let tag = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    Some(match tag {
        0 => LoaderIx::InitializeBuffer,
        1 => LoaderIx::Write,
        2 => LoaderIx::DeployWithMaxDataLen,
        3 => LoaderIx::Upgrade,
        4 => LoaderIx::SetAuthority,
        5 => LoaderIx::Close,
        6 => LoaderIx::ExtendProgram,
        7 => LoaderIx::SetAuthorityChecked,
        _ => return None,
    })
}

/// The account whose authority is being changed: a ProgramData account, or a
/// buffer. Account 0 in both `SetAuthority` and `SetAuthorityChecked`.
pub fn target_account(accounts: &[String]) -> Option<&String> {
    accounts.first()
}

/// The incoming authority, when the instruction supplies one. `SetAuthority`
/// omits it to make a program immutable; `SetAuthorityChecked` requires it as
/// a signer at index 2.
pub fn new_authority(ix: LoaderIx, accounts: &[String]) -> Option<&String> {
    match ix {
        LoaderIx::SetAuthority => accounts.get(2),
        LoaderIx::SetAuthorityChecked => accounts.get(2),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_authority_tags_decode() {
        assert_eq!(decode(&[4, 0, 0, 0]), Some(LoaderIx::SetAuthority));
        assert_eq!(decode(&[7, 0, 0, 0]), Some(LoaderIx::SetAuthorityChecked));
        assert!(decode(&[4, 0, 0, 0]).unwrap().is_authority_change());
        assert!(!decode(&[3, 0, 0, 0]).unwrap().is_authority_change());
    }

    #[test]
    fn unknown_and_short_tags_are_none() {
        assert_eq!(decode(&[99, 0, 0, 0]), None);
        assert_eq!(decode(&[4, 0]), None);
    }

    #[test]
    fn making_a_program_immutable_has_no_new_authority() {
        let accounts = vec!["programdata".to_string(), "current".to_string()];
        assert_eq!(new_authority(LoaderIx::SetAuthority, &accounts), None);
    }
}
