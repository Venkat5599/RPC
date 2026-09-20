//! Behavioural tests for the authority-change policy.
//!
//! The three cases that matter are a real firing, an ordinary transaction that
//! must stay silent, and an incomplete account set that must refuse rather
//! than guess (PRD N2).

use ripcord_policy::authority_change::{AuthorityChange, WatchedProgram};
use ripcord_policy::loader::BPF_LOADER_UPGRADEABLE;
use ripcord_policy::{evaluate, AltResolution, Decision, IxView, Refusal, TxView};

const KAMINO_LEND: &str = "KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD";
const KAMINO_PROGRAM_DATA: &str = "DxNvQfPzWDBpRBrPMdgSr2nKgPKJbeiRWM6XHQ6Xq3rV";

fn watched() -> Vec<WatchedProgram> {
    vec![WatchedProgram {
        program_id: KAMINO_LEND.to_string(),
        program_data: KAMINO_PROGRAM_DATA.to_string(),
        label: "kamino-lend".to_string(),
        admin_ix_names: vec!["update_lending_market_owner".to_string()],
    }]
}

fn tx(instructions: Vec<IxView>, alt: AltResolution) -> TxView {
    TxView {
        signature: "5xSig".to_string(),
        slot: 300_000_000,
        block_time: Some(1_759_000_000),
        alt_resolution: alt,
        success: true,
        instructions,
        token_deltas: vec![],
    }
}

fn ix(program_id: &str, accounts: &[&str], data: Vec<u8>) -> IxView {
    IxView {
        program_id: program_id.to_string(),
        accounts: accounts.iter().map(|a| a.to_string()).collect(),
        data,
        outer_index: 0,
        is_inner: false,
    }
}

#[test]
fn fires_when_upgrade_authority_moves_on_a_watched_program() {
    let policy = AuthorityChange::new(watched());
    // SetAuthority: tag 4, accounts [programdata, current authority, new authority]
    let transaction = tx(
        vec![ix(
            BPF_LOADER_UPGRADEABLE,
            &[KAMINO_PROGRAM_DATA, "CurrentAuth111", "AttackerAuth111"],
            vec![4, 0, 0, 0],
        )],
        AltResolution::Full,
    );

    let Decision::Fire(detection) = evaluate(&policy, &transaction) else {
        panic!("authority transfer on a watched program must fire");
    };
    assert_eq!(detection.policy_id, "authority-change");
    assert_eq!(detection.subject, KAMINO_PROGRAM_DATA);
    assert_eq!(detection.slot, 300_000_000);
    assert_eq!(detection.evidence_ix, vec![0]);
    assert!(
        detection.reason.contains("AttackerAuth111"),
        "the incoming authority belongs in the evidence: {}",
        detection.reason
    );
}

#[test]
fn fires_on_a_protocol_admin_instruction_matched_by_discriminator() {
    let policy = AuthorityChange::new(watched());
    let mut data = ripcord_policy::anchor::discriminator("update_lending_market_owner").to_vec();
    data.extend_from_slice(&[0u8; 8]); // trailing args, ignored

    let transaction = tx(
        vec![ix(KAMINO_LEND, &["market", "new_owner"], data)],
        AltResolution::Full,
    );

    let Decision::Fire(detection) = evaluate(&policy, &transaction) else {
        panic!("protocol admin handover must fire");
    };
    assert_eq!(detection.subject, KAMINO_LEND);
    assert!(detection.reason.contains("update_lending_market_owner"));
}

#[test]
fn stays_silent_on_an_ordinary_transaction() {
    let policy = AuthorityChange::new(watched());

    // A deposit into the same program, and a loader Upgrade of an unwatched
    // program. Neither is an authority change on anything we watch.
    let transaction = tx(
        vec![
            ix(
                KAMINO_LEND,
                &["market", "user"],
                ripcord_policy::anchor::discriminator("deposit_reserve_liquidity").to_vec(),
            ),
            ix(
                BPF_LOADER_UPGRADEABLE,
                &["SomeOtherProgramData", "auth", "newauth"],
                vec![4, 0, 0, 0],
            ),
        ],
        AltResolution::Full,
    );

    assert_eq!(evaluate(&policy, &transaction), Decision::NoMatch);
}

#[test]
fn upgrade_without_authority_change_stays_silent() {
    let policy = AuthorityChange::new(watched());
    // Tag 3 is Upgrade: the code changes, the authority does not.
    let transaction = tx(
        vec![ix(
            BPF_LOADER_UPGRADEABLE,
            &[KAMINO_PROGRAM_DATA, "buffer", "auth"],
            vec![3, 0, 0, 0],
        )],
        AltResolution::Full,
    );

    assert_eq!(evaluate(&policy, &transaction), Decision::NoMatch);
}

#[test]
fn refuses_rather_than_evaluates_when_accounts_are_incomplete() {
    let policy = AuthorityChange::new(watched());
    // The identical firing transaction, but the account set is not complete.
    let firing_ix = ix(
        BPF_LOADER_UPGRADEABLE,
        &[KAMINO_PROGRAM_DATA, "CurrentAuth111", "AttackerAuth111"],
        vec![4, 0, 0, 0],
    );

    for incomplete in [AltResolution::Partial, AltResolution::Unknown] {
        let transaction = tx(vec![firing_ix.clone()], incomplete);
        assert_eq!(
            evaluate(&policy, &transaction),
            Decision::Refuse(Refusal::IncompleteAccounts(incomplete)),
            "an incomplete account set must refuse, never fire and never silently pass"
        );
    }
}
