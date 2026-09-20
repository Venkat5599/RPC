//! The two claims this program exists to make, tested against the real
//! compiled program.
//!
//! 1. **The guardian cannot lie.** With the target's upgrade authority
//!    unchanged, a correctly-signed exit from the correct guardian fails. The
//!    chain checks the condition itself.
//! 2. **The guardian cannot steal.** With the predicate genuinely true, an
//!    exit aimed at any address other than the vault owner fails.
//!
//! These run against `target/deploy/ripcord_vault.so` in LiteSVM, which lets
//! the test rewrite a `ProgramData` account's authority bytes directly — the
//! on-chain equivalent of an attacker seizing a protocol, reproduced exactly
//! and deterministically.
//!
//! Build the program before running these:
//!
//! ```text
//! anchor build && cargo test -p ripcord_vault
//! ```

#![cfg(test)]
#![allow(deprecated)] // solana-sdk re-exports are mid-migration; the replacements are not yet in this version

use litesvm::LiteSVM;
use sha2::{Digest, Sha256};
use solana_sdk::account::Account;
use solana_sdk::bpf_loader_upgradeable;
#[allow(deprecated)]
use solana_sdk::system_program;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::Transaction;

/// The program under test, as declared in `lib.rs`.
fn program_id() -> Pubkey {
    "84h2juN2WaFhZtGbaqmeWVVe7HQCUXnDRRtbGZQeAeTu"
        .parse()
        .unwrap()
}

/// Anchor's instruction discriminator.
fn discriminator(name: &str) -> [u8; 8] {
    let digest = Sha256::digest(format!("global:{name}").as_bytes());
    let mut out = [0u8; 8];
    out.copy_from_slice(&digest[..8]);
    out
}

fn instruction_data(name: &str, arguments: &[u8]) -> Vec<u8> {
    let mut data = discriminator(name).to_vec();
    data.extend_from_slice(arguments);
    data
}

/// A `ProgramData` account holding `authority`, laid out the way the
/// upgradeable loader writes it.
fn program_data_account(authority: Option<Pubkey>) -> Account {
    let mut data = Vec::new();
    data.extend_from_slice(&3u32.to_le_bytes()); // ProgramData variant
    data.extend_from_slice(&1u64.to_le_bytes()); // last deploy slot
    match authority {
        Some(key) => {
            data.push(1);
            data.extend_from_slice(key.as_ref());
        }
        None => {
            data.push(0);
            data.extend_from_slice(&[0u8; 32]);
        }
    }
    data.extend(std::iter::repeat(0u8).take(128)); // program bytes

    Account {
        lamports: 1_000_000_000,
        data,
        owner: bpf_loader_upgradeable::ID,
        executable: false,
        rent_epoch: 0,
    }
}

struct Fixture {
    svm: LiteSVM,
    owner: Keypair,
    guardian: Keypair,
    attacker: Keypair,
    vault: Pubkey,
    policy: Pubkey,
    target_program: Pubkey,
    target_program_data: Pubkey,
}

const POLICY_ID: u64 = 1;
const DEPOSIT_LAMPORTS: u64 = 500_000_000;
const EXIT_CAP: u64 = 400_000_000;

fn setup() -> Fixture {
    let mut svm = LiteSVM::new();

    let program_bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/deploy/ripcord_vault.so"),
    )
    .expect("run `anchor build` before these tests: target/deploy/ripcord_vault.so is missing");
    svm.add_program(program_id(), &program_bytes)
        .expect("the compiled program must load");

    let owner = Keypair::new();
    let guardian = Keypair::new();
    let attacker = Keypair::new();
    for account in [&owner, &guardian, &attacker] {
        svm.airdrop(&account.pubkey(), 10_000_000_000).unwrap();
    }

    let target_program = Pubkey::new_unique();
    let target_program_data = Pubkey::new_unique();
    let original_authority = Pubkey::new_unique();
    svm.set_account(
        target_program_data,
        program_data_account(Some(original_authority)),
    )
    .unwrap();

    let (vault, _) = Pubkey::find_program_address(
        &[b"vault", owner.pubkey().as_ref()],
        &program_id(),
    );
    let (policy, _) = Pubkey::find_program_address(
        &[b"policy", vault.as_ref(), &POLICY_ID.to_le_bytes()],
        &program_id(),
    );

    Fixture {
        svm,
        owner,
        guardian,
        attacker,
        vault,
        policy,
        target_program,
        target_program_data,
    }
}

impl Fixture {
    fn send(&mut self, instruction: Instruction, signers: &[&Keypair]) -> Result<(), String> {
        let blockhash = self.svm.latest_blockhash();
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&signers[0].pubkey()),
            signers,
            blockhash,
        );
        self.svm
            .send_transaction(transaction)
            .map(|_| ())
            .map_err(|failure| format!("{:?}", failure.meta.logs))
    }

    /// Everything up to and including a live, armed policy.
    fn arm_everything(&mut self) {
        let initialize = Instruction {
            program_id: program_id(),
            accounts: vec![
                AccountMeta::new(self.owner.pubkey(), true),
                AccountMeta::new(self.vault, false),
                AccountMeta::new_readonly(system_program::ID, false),
            ],
            data: instruction_data("initialize_vault", &[]),
        };
        let owner = self.owner.insecure_clone();
        self.send(initialize, &[&owner]).expect("initialize_vault");

        let deposit = Instruction {
            program_id: program_id(),
            accounts: vec![
                AccountMeta::new(self.owner.pubkey(), true),
                AccountMeta::new(self.vault, false),
                AccountMeta::new_readonly(system_program::ID, false),
            ],
            data: instruction_data("deposit", &DEPOSIT_LAMPORTS.to_le_bytes()),
        };
        self.send(deposit, &[&owner]).expect("deposit");

        let mut set_policy_args = POLICY_ID.to_le_bytes().to_vec();
        set_policy_args.extend_from_slice(self.target_program.as_ref());
        set_policy_args.extend_from_slice(self.target_program_data.as_ref());
        let set_policy = Instruction {
            program_id: program_id(),
            accounts: vec![
                AccountMeta::new(self.owner.pubkey(), true),
                AccountMeta::new(self.vault, false),
                AccountMeta::new(self.policy, false),
                AccountMeta::new_readonly(system_program::ID, false),
            ],
            data: instruction_data("set_policy", &set_policy_args),
        };
        self.send(set_policy, &[&owner]).expect("set_policy");

        let mut arm_args = self.guardian.pubkey().as_ref().to_vec();
        arm_args.extend_from_slice(&u64::MAX.to_le_bytes()); // expires_at_slot
        arm_args.extend_from_slice(&EXIT_CAP.to_le_bytes());
        let arm = Instruction {
            program_id: program_id(),
            accounts: vec![
                AccountMeta::new_readonly(self.owner.pubkey(), true),
                AccountMeta::new_readonly(self.vault, false),
                AccountMeta::new(self.policy, false),
                AccountMeta::new_readonly(self.target_program_data, false),
            ],
            data: instruction_data("arm", &arm_args),
        };
        self.send(arm, &[&owner]).expect("arm");
    }

    /// An exit attempt, with the destination account chosen by the caller.
    fn exit_to(&self, destination: Pubkey, amount: u64) -> Instruction {
        Instruction {
            program_id: program_id(),
            accounts: vec![
                AccountMeta::new_readonly(self.guardian.pubkey(), true),
                AccountMeta::new(destination, false),
                AccountMeta::new(self.vault, false),
                AccountMeta::new(self.policy, false),
                AccountMeta::new_readonly(self.target_program_data, false),
            ],
            data: instruction_data("execute_exit", &amount.to_le_bytes()),
        }
    }

    /// Seize the target program: rewrite its upgrade authority, exactly as a
    /// `SetAuthority` transaction would.
    fn seize_target(&mut self) -> Pubkey {
        let new_authority = Pubkey::new_unique();
        self.svm
            .set_account(
                self.target_program_data,
                program_data_account(Some(new_authority)),
            )
            .unwrap();
        new_authority
    }

    fn balance(&self, key: &Pubkey) -> u64 {
        self.svm.get_balance(key).unwrap_or(0)
    }
}

#[test]
fn the_guardian_cannot_fire_while_the_predicate_is_false() {
    let mut fixture = setup();
    fixture.arm_everything();

    let guardian = fixture.guardian.insecure_clone();
    let owner_before = fixture.balance(&fixture.owner.pubkey());

    let exit = fixture.exit_to(fixture.owner.pubkey(), 100_000_000);
    let failure = fixture
        .send(exit, &[&guardian])
        .expect_err("an unchanged authority must not permit an exit");

    assert!(
        failure.contains("PredicateFalse") || failure.contains("predicate is false"),
        "expected the predicate to reject the exit, got: {failure}"
    );
    assert_eq!(
        fixture.balance(&fixture.owner.pubkey()),
        owner_before,
        "no lamports may move when the predicate is false"
    );
}

#[test]
fn the_guardian_cannot_send_funds_to_a_third_party() {
    let mut fixture = setup();
    fixture.arm_everything();
    // Make the predicate genuinely true, so the only thing left to stop this
    // is the delegation invariant itself.
    fixture.seize_target();

    let guardian = fixture.guardian.insecure_clone();
    let attacker_destination = fixture.attacker.pubkey();
    let attacker_before = fixture.balance(&attacker_destination);

    let exit = fixture.exit_to(attacker_destination, 100_000_000);
    let failure = fixture
        .send(exit, &[&guardian])
        .expect_err("the guardian must never be able to name a destination");

    assert!(
        failure.contains("DestinationNotOwner")
            || failure.contains("destination must be the vault owner")
            || failure.contains("ConstraintAddress"),
        "expected the destination constraint to reject this, got: {failure}"
    );
    assert_eq!(
        fixture.balance(&attacker_destination),
        attacker_before,
        "not one lamport may reach a third party"
    );
}

#[test]
fn a_real_authority_change_lets_the_owner_out() {
    let mut fixture = setup();
    fixture.arm_everything();
    fixture.seize_target();

    let guardian = fixture.guardian.insecure_clone();
    let owner_key = fixture.owner.pubkey();
    let owner_before = fixture.balance(&owner_key);

    let exit = fixture.exit_to(owner_key, 100_000_000);
    fixture
        .send(exit, &[&guardian])
        .expect("a genuine authority change must permit the exit");

    assert_eq!(
        fixture.balance(&owner_key),
        owner_before + 100_000_000,
        "the exit must land in the owner's wallet"
    );
}

#[test]
fn an_exit_cannot_exceed_the_cap_set_at_arming() {
    let mut fixture = setup();
    fixture.arm_everything();
    fixture.seize_target();

    let guardian = fixture.guardian.insecure_clone();
    let owner_key = fixture.owner.pubkey();

    let exit = fixture.exit_to(owner_key, EXIT_CAP + 1);
    let failure = fixture
        .send(exit, &[&guardian])
        .expect_err("the cap is part of what the owner agreed to");
    assert!(
        failure.contains("ExceedsExitCap") || failure.contains("exceeds the cap"),
        "expected the cap to reject this, got: {failure}"
    );
}

#[test]
fn a_stranger_cannot_fire_a_policy_armed_for_someone_else() {
    let mut fixture = setup();
    fixture.arm_everything();
    fixture.seize_target();

    let attacker = fixture.attacker.insecure_clone();
    let owner_key = fixture.owner.pubkey();

    // The attacker signs in the guardian position, and even aims the funds at
    // the legitimate owner, so only the guardian check can stop it.
    let exit = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new_readonly(attacker.pubkey(), true),
            AccountMeta::new(owner_key, false),
            AccountMeta::new(fixture.vault, false),
            AccountMeta::new(fixture.policy, false),
            AccountMeta::new_readonly(fixture.target_program_data, false),
        ],
        data: instruction_data("execute_exit", &100_000_000u64.to_le_bytes()),
    };
    let failure = fixture
        .send(exit, &[&attacker])
        .expect_err("only the armed guardian may propose");
    assert!(
        failure.contains("WrongGuardian") || failure.contains("not the armed guardian"),
        "expected the guardian check to reject this, got: {failure}"
    );
}

#[test]
fn revoking_disarms_the_policy_in_one_transaction() {
    let mut fixture = setup();
    fixture.arm_everything();
    fixture.seize_target();

    let owner = fixture.owner.insecure_clone();
    let revoke = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new_readonly(owner.pubkey(), true),
            AccountMeta::new_readonly(fixture.vault, false),
            AccountMeta::new(fixture.policy, false),
        ],
        data: instruction_data("revoke", &[]),
    };
    fixture.send(revoke, &[&owner]).expect("revoke");

    // The predicate is true and the guardian is the one that was armed, so
    // only the revocation stands between it and the funds.
    let guardian = fixture.guardian.insecure_clone();
    let exit = fixture.exit_to(owner.pubkey(), 100_000_000);
    let failure = fixture
        .send(exit, &[&guardian])
        .expect_err("a revoked policy must not fire");
    assert!(
        failure.contains("PolicyNotArmed")
            || failure.contains("not armed")
            || failure.contains("WrongGuardian"),
        "expected the revocation to reject this, got: {failure}"
    );
}

#[test]
fn the_owner_can_always_withdraw_regardless_of_any_policy() {
    let mut fixture = setup();
    fixture.arm_everything();

    let owner = fixture.owner.insecure_clone();
    let before = fixture.balance(&owner.pubkey());

    let withdraw = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(owner.pubkey(), true),
            AccountMeta::new(fixture.vault, false),
        ],
        data: instruction_data("emergency_owner_withdraw", &200_000_000u64.to_le_bytes()),
    };
    fixture
        .send(withdraw, &[&owner])
        .expect("the owner's escape hatch must never be blocked");

    assert!(
        fixture.balance(&owner.pubkey()) > before,
        "the owner's funds must come back"
    );
}
