//! `ripcord-keeper` — the off-chain half.
//!
//! Everything here is a *proposal*. The keeper builds transactions, signs
//! them, and sends them; the program decides whether any of it means anything.
//! That asymmetry is the product, so this binary deliberately has no special
//! access: it holds a guardian key, and a guardian key buys the right to ask.
//!
//! It also drives the demo, end to end, against a real cluster:
//!
//! ```text
//! ripcord-keeper vault-init
//! ripcord-keeper deposit   --lamports 200000000
//! ripcord-keeper policy    --id 1 --target <program>
//! ripcord-keeper arm       --id 1 --guardian <pubkey> --cap 100000000
//! ripcord-keeper exit      --id 1 --lamports 50000000   # fails: nothing changed
//! ripcord-keeper seize     --target <program> --to <pubkey>
//! ripcord-keeper exit      --id 1 --lamports 50000000   # lands: authority moved
//! ```

use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{anyhow, bail, Result};
use sha2::{Digest, Sha256};
use solana_client::rpc_client::RpcClient;
use solana_sdk::bpf_loader_upgradeable;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{read_keypair_file, Keypair, Signature, Signer};
use solana_sdk::system_program;
use solana_sdk::transaction::Transaction;

/// Deployed on devnet; the same ID is used everywhere until mainnet.
const PROGRAM_ID: &str = "84h2juN2WaFhZtGbaqmeWVVe7HQCUXnDRRtbGZQeAeTu";
const DEFAULT_RPC: &str = "https://api.devnet.solana.com";

fn program_id() -> Pubkey {
    Pubkey::from_str(PROGRAM_ID).expect("the compiled-in program ID must be valid")
}

/// Anchor's instruction discriminator: `sha256("global:<name>")[0..8]`.
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

fn vault_address(owner: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"vault", owner.as_ref()], &program_id()).0
}

fn policy_address(vault: &Pubkey, policy_id: u64) -> Pubkey {
    Pubkey::find_program_address(
        &[b"policy", vault.as_ref(), &policy_id.to_le_bytes()],
        &program_id(),
    )
    .0
}

/// The ProgramData account of an upgradeable program.
fn program_data_address(program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

/// Everything a command needs: an endpoint, a signer, and the explorer
/// suffix so printed links point at the right cluster.
struct Session {
    client: RpcClient,
    payer: Keypair,
    explorer_suffix: &'static str,
}

impl Session {
    fn new(url: &str, keypair_path: &PathBuf) -> Result<Self> {
        let payer = read_keypair_file(keypair_path)
            .map_err(|error| anyhow!("reading {}: {error}", keypair_path.display()))?;
        let explorer_suffix = if url.contains("devnet") {
            "?cluster=devnet"
        } else {
            ""
        };
        Ok(Self {
            client: RpcClient::new_with_commitment(url.to_string(), CommitmentConfig::confirmed()),
            payer,
            explorer_suffix,
        })
    }

    fn send(&self, instruction: Instruction, extra_signers: &[&Keypair]) -> Result<Signature> {
        let blockhash = self.client.get_latest_blockhash()?;
        let mut signers: Vec<&Keypair> = vec![&self.payer];
        signers.extend_from_slice(extra_signers);

        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.payer.pubkey()),
            &signers,
            blockhash,
        );
        let signature = self.client.send_and_confirm_transaction(&transaction)?;
        println!("  signature: {signature}");
        println!(
            "  https://solscan.io/tx/{signature}{}",
            self.explorer_suffix
        );
        Ok(signature)
    }
}

fn main() -> Result<()> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let command = arguments.first().map(String::as_str).unwrap_or("help");
    let options = parse(&arguments[arguments.len().min(1)..])?;

    let session = Session::new(&options.url, &options.keypair)?;
    let owner = options.owner.unwrap_or_else(|| session.payer.pubkey());
    let vault = vault_address(&owner);

    match command {
        "vault-init" => vault_init(&session, &vault),
        "deposit" => deposit(&session, &vault, &options),
        "policy" => policy(&session, &vault, &options),
        "arm" => arm(&session, &vault, &options),
        "exit" => execute_exit(&session, &vault, &owner, &options),
        "revoke" => revoke(&session, &vault, &options),
        "withdraw" => withdraw(&session, &vault, &options),
        "seize" => seize(&session, &options),
        "status" => status(&session, &vault, &owner, &options),
        _ => {
            usage();
            Ok(())
        }
    }
}

fn usage() {
    eprintln!(
        "ripcord-keeper — proposes exits that the chain then verifies

  vault-init                       create the vault PDA for the owner
  deposit   --lamports <n>         fund the vault
  policy    --id <n> --target <program>
                                   store a policy against a program's upgrade authority
  arm       --id <n> --guardian <pubkey> [--cap <lamports>] [--expires-in <slots>]
                                   record the baseline authority and grant the guardian
  exit      --id <n> --lamports <n> [--guardian-keypair <path>] [--to <pubkey>]
                                   propose an exit; the chain re-checks the predicate
  revoke    --id <n>               disarm, one transaction
  withdraw  --lamports <n>         the owner's escape hatch
  seize     --target <program> --to <pubkey>
                                   transfer a program's upgrade authority (demo trigger)
  status    --id <n>               show the vault, the policy and the live authority

options:
  --url <rpc>                      default https://api.devnet.solana.com
  --keypair <path>                 default ~/.config/solana/id.json
  --owner <pubkey>                 act on another owner's vault (read-only commands)
  --to <pubkey>                    destination override, to demonstrate it is refused"
    );
}

#[derive(Default)]
struct Options {
    url: String,
    keypair: PathBuf,
    id: u64,
    lamports: u64,
    cap: u64,
    expires_in: u64,
    target: Option<Pubkey>,
    guardian: Option<Pubkey>,
    guardian_keypair: Option<PathBuf>,
    to: Option<Pubkey>,
    owner: Option<Pubkey>,
}

fn parse(arguments: &[String]) -> Result<Options> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let mut options = Options {
        url: std::env::var("RIPCORD_RPC_URL").unwrap_or_else(|_| DEFAULT_RPC.to_string()),
        keypair: PathBuf::from(format!("{home}/.config/solana/id.json")),
        id: 1,
        cap: 100_000_000,
        expires_in: 432_000, // roughly a day of slots
        ..Default::default()
    };

    let mut index = 0;
    while index < arguments.len() {
        let flag = arguments[index].as_str();
        let mut value = || -> Result<String> {
            index += 1;
            arguments
                .get(index)
                .cloned()
                .ok_or_else(|| anyhow!("{flag} needs a value"))
        };
        match flag {
            "--url" => options.url = value()?,
            "--keypair" => options.keypair = PathBuf::from(value()?),
            "--id" => options.id = value()?.parse()?,
            "--lamports" => options.lamports = value()?.parse()?,
            "--cap" => options.cap = value()?.parse()?,
            "--expires-in" => options.expires_in = value()?.parse()?,
            "--target" => options.target = Some(Pubkey::from_str(&value()?)?),
            "--guardian" => options.guardian = Some(Pubkey::from_str(&value()?)?),
            "--guardian-keypair" => options.guardian_keypair = Some(PathBuf::from(value()?)),
            "--to" => options.to = Some(Pubkey::from_str(&value()?)?),
            "--owner" => options.owner = Some(Pubkey::from_str(&value()?)?),
            other if other.starts_with("--") => bail!("unknown option {other}"),
            _ => {}
        }
        index += 1;
    }
    Ok(options)
}

fn vault_init(session: &Session, vault: &Pubkey) -> Result<()> {
    println!("creating vault {vault}");
    session.send(
        Instruction {
            program_id: program_id(),
            accounts: vec![
                AccountMeta::new(session.payer.pubkey(), true),
                AccountMeta::new(*vault, false),
                AccountMeta::new_readonly(system_program::ID, false),
            ],
            data: instruction_data("initialize_vault", &[]),
        },
        &[],
    )?;
    Ok(())
}

fn deposit(session: &Session, vault: &Pubkey, options: &Options) -> Result<()> {
    require_amount(options.lamports)?;
    println!("depositing {} lamports into {vault}", options.lamports);
    session.send(
        Instruction {
            program_id: program_id(),
            accounts: vec![
                AccountMeta::new(session.payer.pubkey(), true),
                AccountMeta::new(*vault, false),
                AccountMeta::new_readonly(system_program::ID, false),
            ],
            data: instruction_data("deposit", &options.lamports.to_le_bytes()),
        },
        &[],
    )?;
    Ok(())
}

fn policy(session: &Session, vault: &Pubkey, options: &Options) -> Result<()> {
    let target = options.target.ok_or_else(|| anyhow!("--target required"))?;
    let target_data = program_data_address(&target);
    let policy = policy_address(vault, options.id);

    println!("policy {} on {target}", options.id);
    println!("  program data: {target_data}");

    let mut arguments = options.id.to_le_bytes().to_vec();
    arguments.extend_from_slice(target.as_ref());
    arguments.extend_from_slice(target_data.as_ref());

    session.send(
        Instruction {
            program_id: program_id(),
            accounts: vec![
                AccountMeta::new(session.payer.pubkey(), true),
                AccountMeta::new(*vault, false),
                AccountMeta::new(policy, false),
                AccountMeta::new_readonly(system_program::ID, false),
            ],
            data: instruction_data("set_policy", &arguments),
        },
        &[],
    )?;
    Ok(())
}

fn arm(session: &Session, vault: &Pubkey, options: &Options) -> Result<()> {
    let guardian = options
        .guardian
        .ok_or_else(|| anyhow!("--guardian required"))?;
    let target = options.target.ok_or_else(|| anyhow!("--target required"))?;
    let target_data = program_data_address(&target);
    let policy = policy_address(vault, options.id);
    let expires_at = session.client.get_slot()? + options.expires_in;

    println!("arming policy {} for guardian {guardian}", options.id);
    println!("  cap {} lamports, expires at slot {expires_at}", options.cap);
    println!("  the baseline authority is read from the chain by the program, not supplied here");

    let mut arguments = guardian.as_ref().to_vec();
    arguments.extend_from_slice(&expires_at.to_le_bytes());
    arguments.extend_from_slice(&options.cap.to_le_bytes());

    session.send(
        Instruction {
            program_id: program_id(),
            accounts: vec![
                AccountMeta::new_readonly(session.payer.pubkey(), true),
                AccountMeta::new_readonly(*vault, false),
                AccountMeta::new(policy, false),
                AccountMeta::new_readonly(target_data, false),
            ],
            data: instruction_data("arm", &arguments),
        },
        &[],
    )?;
    Ok(())
}

fn execute_exit(
    session: &Session,
    vault: &Pubkey,
    owner: &Pubkey,
    options: &Options,
) -> Result<()> {
    require_amount(options.lamports)?;
    let target = options.target.ok_or_else(|| anyhow!("--target required"))?;
    let target_data = program_data_address(&target);
    let policy = policy_address(vault, options.id);

    // The destination can be overridden only to demonstrate that the program
    // refuses it. There is no legitimate reason to pass --to.
    let destination = options.to.unwrap_or(*owner);
    if destination != *owner {
        println!("  destination overridden to {destination}, which the program should refuse");
    }

    let guardian_keypair = match &options.guardian_keypair {
        Some(path) => Some(
            read_keypair_file(path)
                .map_err(|error| anyhow!("reading {}: {error}", path.display()))?,
        ),
        None => None,
    };
    let guardian_pubkey = guardian_keypair
        .as_ref()
        .map(|keypair| keypair.pubkey())
        .unwrap_or_else(|| session.payer.pubkey());

    println!("proposing exit of {} lamports", options.lamports);
    println!("  guardian:    {guardian_pubkey}");
    println!("  destination: {destination}");

    let instruction = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new_readonly(guardian_pubkey, true),
            AccountMeta::new(destination, false),
            AccountMeta::new(*vault, false),
            AccountMeta::new(policy, false),
            AccountMeta::new_readonly(target_data, false),
        ],
        data: instruction_data("execute_exit", &options.lamports.to_le_bytes()),
    };

    let extra: Vec<&Keypair> = guardian_keypair.iter().collect();
    match session.send(instruction, &extra) {
        Ok(_) => {
            println!("  the chain agreed: the authority changed, and the funds are with the owner");
            Ok(())
        }
        Err(error) => {
            let text = error.to_string();
            if text.contains("PredicateFalse") || text.contains("predicate is false") {
                println!("  refused: the chain re-read the authority and it has not changed.");
                println!("  this is the product working, not a failure.");
                Ok(())
            } else if text.contains("DestinationNotOwner") || text.contains("ConstraintAddress") {
                println!("  refused: the guardian cannot name a destination. Only the owner.");
                Ok(())
            } else {
                Err(error)
            }
        }
    }
}

fn revoke(session: &Session, vault: &Pubkey, options: &Options) -> Result<()> {
    let policy = policy_address(vault, options.id);
    println!("revoking policy {}", options.id);
    session.send(
        Instruction {
            program_id: program_id(),
            accounts: vec![
                AccountMeta::new_readonly(session.payer.pubkey(), true),
                AccountMeta::new_readonly(*vault, false),
                AccountMeta::new(policy, false),
            ],
            data: instruction_data("revoke", &[]),
        },
        &[],
    )?;
    Ok(())
}

fn withdraw(session: &Session, vault: &Pubkey, options: &Options) -> Result<()> {
    require_amount(options.lamports)?;
    println!("owner withdrawing {} lamports", options.lamports);
    session.send(
        Instruction {
            program_id: program_id(),
            accounts: vec![
                AccountMeta::new(session.payer.pubkey(), true),
                AccountMeta::new(*vault, false),
            ],
            data: instruction_data("emergency_owner_withdraw", &options.lamports.to_le_bytes()),
        },
        &[],
    )?;
    Ok(())
}

/// Transfer a program's upgrade authority.
///
/// The demo trigger, and it is a real one: this is the ordinary loader
/// instruction any protocol team would use, fired against a program we own and
/// say out loud that we own. Nothing about the detection path is simulated.
fn seize(session: &Session, options: &Options) -> Result<()> {
    let target = options.target.ok_or_else(|| anyhow!("--target required"))?;
    let new_authority = options
        .to
        .ok_or_else(|| anyhow!("--to <new authority> required"))?;
    let target_data = program_data_address(&target);

    println!("transferring upgrade authority of {target}");
    println!("  program data: {target_data}");
    println!("  new authority: {new_authority}");

    let instruction = bpf_loader_upgradeable::set_upgrade_authority(
        &target,
        &session.payer.pubkey(),
        Some(&new_authority),
    );
    session.send(instruction, &[])?;
    Ok(())
}

fn status(session: &Session, vault: &Pubkey, owner: &Pubkey, options: &Options) -> Result<()> {
    println!("owner:  {owner}");
    println!("vault:  {vault}");
    match session.client.get_account(vault) {
        Ok(account) => println!("  balance: {} lamports", account.lamports),
        Err(_) => {
            println!("  not created yet");
            return Ok(());
        }
    }

    let policy = policy_address(vault, options.id);
    println!("policy {}: {policy}", options.id);
    let Ok(account) = session.client.get_account(&policy) else {
        println!("  not set");
        return Ok(());
    };

    // Layout after the 8-byte account discriminator: vault, policy_id, kind,
    // target_program, target_program_data, baseline_authority,
    // baseline_is_immutable, guardian, armed, ...
    let data = &account.data;
    let read_pubkey = |offset: usize| -> Option<Pubkey> {
        data.get(offset..offset + 32)
            .map(|slice| Pubkey::try_from(slice).ok())
            .flatten()
    };
    let target_program = read_pubkey(8 + 32 + 8 + 1);
    let baseline = read_pubkey(8 + 32 + 8 + 1 + 32 + 32);
    let immutable_offset = 8 + 32 + 8 + 1 + 32 + 32 + 32;
    let baseline_immutable = data.get(immutable_offset).copied().unwrap_or(0) == 1;
    let guardian = read_pubkey(immutable_offset + 1);
    let armed = data.get(immutable_offset + 1 + 32).copied().unwrap_or(0) == 1;

    println!("  armed:    {armed}");
    if let Some(guardian) = guardian {
        println!("  guardian: {guardian}");
    }
    if let Some(target) = target_program {
        println!("  target:   {target}");
        let target_data = program_data_address(&target);
        if let Ok(account) = session.client.get_account(&target_data) {
            let live = live_authority(&account.data);
            println!("  baseline authority: {}", describe(baseline, baseline_immutable));
            println!("  live authority:     {}", describe(live.0, live.1));
            let changed = live.1 != baseline_immutable || (!live.1 && live.0 != baseline);
            println!(
                "  predicate: {}",
                if changed {
                    "TRUE — an exit would be permitted"
                } else {
                    "false — an exit would be refused"
                }
            );
        }
    }
    Ok(())
}

fn describe(key: Option<Pubkey>, immutable: bool) -> String {
    if immutable {
        "none (immutable)".to_string()
    } else {
        key.map(|key| key.to_string())
            .unwrap_or_else(|| "unreadable".to_string())
    }
}

/// Read the upgrade authority from a ProgramData buffer, using the same
/// offsets the program uses.
fn live_authority(data: &[u8]) -> (Option<Pubkey>, bool) {
    if data.len() < 45 {
        return (None, false);
    }
    if data[12] == 0 {
        return (None, true);
    }
    (Pubkey::try_from(&data[13..45]).ok(), false)
}

fn require_amount(lamports: u64) -> Result<()> {
    if lamports == 0 {
        bail!("--lamports must be greater than zero");
    }
    Ok(())
}
