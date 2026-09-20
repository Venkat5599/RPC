//! `ripcord_vault` — a vault whose guardian can propose, but cannot lie.
//!
//! Every automation vault built so far, including the one that lost $6M in
//! July, works the same way: an off-chain keeper decides when to act and the
//! on-chain program does as it is told. Locking the destination to the owner
//! bounds the damage, but the user is still trusting the keeper's *judgement*.
//!
//! This program removes the judgement from the keeper. [`execute_exit`] does
//! not accept an assertion that a policy fired. It accepts **evidence** — the
//! actual accounts — and re-evaluates the predicate on-chain against live
//! state. A false predicate aborts and nothing moves.
//!
//! Two invariants carry the design, and both are enforced here rather than in
//! any client:
//!
//! 1. **The guardian can only ever move funds to the owner.** Not redirect,
//!    not spend, not swap. The destination is derived from the vault, never
//!    supplied.
//! 2. **The guardian cannot act early.** The chain checks the condition
//!    itself, so a fully compromised guardian can neither steal nor fire
//!    spuriously.
//!
//! What remains is a keeper that is an untrusted relayer: it proposes, the
//! chain verifies.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::bpf_loader_upgradeable;

declare_id!("84h2juN2WaFhZtGbaqmeWVVe7HQCUXnDRRtbGZQeAeTu");

pub mod predicate;

use predicate::{authority_changed, parse_upgrade_authority, Authority};

#[program]
pub mod ripcord_vault {
    use super::*;

    /// Create the caller's vault. One per owner.
    pub fn initialize_vault(context: Context<InitializeVault>) -> Result<()> {
        let vault = &mut context.accounts.vault;
        vault.owner = context.accounts.owner.key();
        vault.bump = context.bumps.vault;
        vault.policy_count = 0;
        vault.exits_executed = 0;
        Ok(())
    }

    /// Move lamports into the vault.
    ///
    /// The vault holds them at its own PDA, so there is no custodian and no
    /// pool: the only address that can ever receive them back is the owner
    /// recorded above.
    pub fn deposit(context: Context<Deposit>, amount: u64) -> Result<()> {
        require!(amount > 0, RipcordError::ZeroAmount);

        anchor_lang::system_program::transfer(
            CpiContext::new(
                context.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: context.accounts.owner.to_account_info(),
                    to: context.accounts.vault.to_account_info(),
                },
            ),
            amount,
        )?;

        emit!(Deposited {
            vault: context.accounts.vault.key(),
            amount,
        });
        Ok(())
    }

    /// Store a policy. Public and auditable: anyone can read what a vault will
    /// act on before deciding whether the design is trustworthy.
    ///
    /// Storing a policy grants nothing. Authority is granted by [`arm`], and
    /// only then.
    pub fn set_policy(
        context: Context<SetPolicy>,
        policy_id: u64,
        target_program: Pubkey,
        target_program_data: Pubkey,
    ) -> Result<()> {
        let policy = &mut context.accounts.policy;
        policy.vault = context.accounts.vault.key();
        policy.policy_id = policy_id;
        policy.kind = PolicyKind::AuthorityChange;
        policy.target_program = target_program;
        policy.target_program_data = target_program_data;
        policy.baseline_authority = Pubkey::default();
        policy.baseline_is_immutable = false;
        policy.guardian = Pubkey::default();
        policy.armed = false;
        policy.armed_at_slot = 0;
        policy.expires_at_slot = 0;
        policy.max_exit_lamports = 0;
        policy.bump = context.bumps.policy;

        let vault = &mut context.accounts.vault;
        vault.policy_count = vault.policy_count.saturating_add(1);
        Ok(())
    }

    /// Arm a policy: name the guardian, cap the exit, bound it in time, and
    /// record the baseline the predicate will later be compared against.
    ///
    /// The baseline is read **from the chain, here, now** — not supplied by
    /// the caller. That is what makes the later comparison meaningful: both
    /// sides of it are observed by the program.
    pub fn arm(
        context: Context<Arm>,
        guardian: Pubkey,
        expires_at_slot: u64,
        max_exit_lamports: u64,
    ) -> Result<()> {
        let clock = Clock::get()?;
        require!(
            expires_at_slot > clock.slot,
            RipcordError::ExpiryInThePast
        );
        require!(max_exit_lamports > 0, RipcordError::ZeroAmount);

        let policy = &mut context.accounts.policy;
        require_keys_eq!(
            context.accounts.target_program_data.key(),
            policy.target_program_data,
            RipcordError::WrongEvidenceAccount
        );

        let baseline = read_upgrade_authority(&context.accounts.target_program_data)?;

        policy.guardian = guardian;
        policy.armed = true;
        policy.armed_at_slot = clock.slot;
        policy.expires_at_slot = expires_at_slot;
        policy.max_exit_lamports = max_exit_lamports;
        policy.baseline_authority = baseline.key;
        policy.baseline_is_immutable = baseline.is_immutable;

        emit!(Armed {
            vault: policy.vault,
            policy_id: policy.policy_id,
            guardian,
            baseline_authority: baseline.key,
            baseline_is_immutable: baseline.is_immutable,
            armed_at_slot: clock.slot,
            expires_at_slot,
        });
        Ok(())
    }

    /// Withdraw to the owner, if and only if the chain agrees the policy fired.
    ///
    /// Signed by the guardian, but the guardian's signature only buys the
    /// right to *ask*. The program re-reads the target's upgrade authority and
    /// compares it against the baseline recorded at [`arm`]. Unchanged
    /// authority means the predicate is false, the instruction fails, and no
    /// lamports move.
    ///
    /// The destination is not a parameter. It is the owner recorded in the
    /// vault, which is why a fully compromised guardian cannot steal.
    pub fn execute_exit(context: Context<ExecuteExit>, amount: u64) -> Result<()> {
        let clock = Clock::get()?;
        let policy = &context.accounts.policy;

        require!(policy.armed, RipcordError::PolicyNotArmed);
        require!(
            clock.slot <= policy.expires_at_slot,
            RipcordError::PolicyExpired
        );
        require!(amount > 0, RipcordError::ZeroAmount);
        require!(
            amount <= policy.max_exit_lamports,
            RipcordError::ExceedsExitCap
        );
        require_keys_eq!(
            context.accounts.guardian.key(),
            policy.guardian,
            RipcordError::WrongGuardian
        );
        require_keys_eq!(
            context.accounts.target_program_data.key(),
            policy.target_program_data,
            RipcordError::WrongEvidenceAccount
        );

        // The predicate. Everything above is authorisation; this is the part
        // that makes the keeper untrusted.
        let observed = read_upgrade_authority(&context.accounts.target_program_data)?;
        let baseline = Authority {
            key: policy.baseline_authority,
            is_immutable: policy.baseline_is_immutable,
        };
        require!(
            authority_changed(baseline, observed),
            RipcordError::PredicateFalse
        );

        let vault = &context.accounts.vault;
        let rent_floor = Rent::get()?.minimum_balance(Vault::LEN);
        let vault_info = vault.to_account_info();
        let available = vault_info.lamports().saturating_sub(rent_floor);
        require!(amount <= available, RipcordError::InsufficientVaultBalance);

        // Direct lamport movement: the vault PDA carries data, so the system
        // program will not transfer from it.
        **vault_info.try_borrow_mut_lamports()? -= amount;
        **context
            .accounts
            .owner
            .to_account_info()
            .try_borrow_mut_lamports()? += amount;

        let policy = &mut context.accounts.policy;
        // One exit per arming. Re-firing requires the owner to arm again,
        // which is the on-chain half of the rate limit.
        policy.armed = false;

        let vault = &mut context.accounts.vault;
        vault.exits_executed = vault.exits_executed.saturating_add(1);

        emit!(ExitExecuted {
            vault: vault.key(),
            policy_id: policy.policy_id,
            amount,
            slot: clock.slot,
            baseline_authority: policy.baseline_authority,
            observed_authority: observed.key,
            observed_is_immutable: observed.is_immutable,
        });
        Ok(())
    }

    /// Revoke a policy. One transaction, owner only, no delay and no quorum.
    pub fn revoke(context: Context<Revoke>) -> Result<()> {
        let policy = &mut context.accounts.policy;
        policy.armed = false;
        policy.guardian = Pubkey::default();
        policy.max_exit_lamports = 0;

        emit!(Revoked {
            vault: policy.vault,
            policy_id: policy.policy_id,
        });
        Ok(())
    }

    /// The owner's own funds, always available, bypassing every policy.
    pub fn emergency_owner_withdraw(
        context: Context<EmergencyOwnerWithdraw>,
        amount: u64,
    ) -> Result<()> {
        require!(amount > 0, RipcordError::ZeroAmount);

        let vault_info = context.accounts.vault.to_account_info();
        let rent_floor = Rent::get()?.minimum_balance(Vault::LEN);
        let available = vault_info.lamports().saturating_sub(rent_floor);
        require!(amount <= available, RipcordError::InsufficientVaultBalance);

        **vault_info.try_borrow_mut_lamports()? -= amount;
        **context
            .accounts
            .owner
            .to_account_info()
            .try_borrow_mut_lamports()? += amount;
        Ok(())
    }
}

/// Read the upgrade authority out of a `ProgramData` account.
///
/// The account checks live here; the byte parsing lives in [`predicate`],
/// where it is tested against hand-laid buffers.
fn read_upgrade_authority(account: &AccountInfo) -> Result<Authority> {
    require_keys_eq!(
        *account.owner,
        bpf_loader_upgradeable::ID,
        RipcordError::NotProgramData
    );

    let data = account.try_borrow_data()?;
    parse_upgrade_authority(&data).map_err(|_| error!(RipcordError::NotProgramData))
}

// ---------------------------------------------------------------- state

#[account]
pub struct Vault {
    pub owner: Pubkey,
    pub bump: u8,
    pub policy_count: u64,
    pub exits_executed: u64,
}

impl Vault {
    pub const LEN: usize = 8 + 32 + 1 + 8 + 8;
}

/// Which predicate a policy carries.
///
/// Deliberately not a general VM: every variant must be expressible as a
/// bounded, constant-cost check over supplied accounts. A policy that cannot
/// be verified on-chain does not ship, and that constraint is what keeps the
/// product trustless as the set grows.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum PolicyKind {
    /// The target program's upgrade authority is no longer what it was at
    /// arming. Fully trustless: the chain reads the authority directly.
    AuthorityChange,
}

#[account]
pub struct Policy {
    pub vault: Pubkey,
    pub policy_id: u64,
    pub kind: PolicyKind,
    pub target_program: Pubkey,
    pub target_program_data: Pubkey,
    /// The authority observed at arming. Half of the comparison the predicate
    /// makes; the other half is read live at exit.
    pub baseline_authority: Pubkey,
    /// Whether the target was already immutable at arming. Tracked separately
    /// because an immutable program's authority reads as the default pubkey,
    /// and "immutable" must not be confused with "unset".
    pub baseline_is_immutable: bool,
    pub guardian: Pubkey,
    pub armed: bool,
    pub armed_at_slot: u64,
    pub expires_at_slot: u64,
    pub max_exit_lamports: u64,
    pub bump: u8,
}

impl Policy {
    pub const LEN: usize = 8 + 32 + 8 + 1 + 32 + 32 + 32 + 1 + 32 + 1 + 8 + 8 + 8 + 1;
}

// ---------------------------------------------------------------- accounts

#[derive(Accounts)]
pub struct InitializeVault<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        init,
        payer = owner,
        space = Vault::LEN,
        seeds = [b"vault", owner.key().as_ref()],
        bump
    )]
    pub vault: Account<'info, Vault>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        mut,
        seeds = [b"vault", owner.key().as_ref()],
        bump = vault.bump,
        has_one = owner @ RipcordError::NotOwner
    )]
    pub vault: Account<'info, Vault>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(policy_id: u64)]
pub struct SetPolicy<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        mut,
        seeds = [b"vault", owner.key().as_ref()],
        bump = vault.bump,
        has_one = owner @ RipcordError::NotOwner
    )]
    pub vault: Account<'info, Vault>,
    #[account(
        init,
        payer = owner,
        space = Policy::LEN,
        seeds = [b"policy", vault.key().as_ref(), &policy_id.to_le_bytes()],
        bump
    )]
    pub policy: Account<'info, Policy>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Arm<'info> {
    pub owner: Signer<'info>,
    #[account(
        seeds = [b"vault", owner.key().as_ref()],
        bump = vault.bump,
        has_one = owner @ RipcordError::NotOwner
    )]
    pub vault: Account<'info, Vault>,
    #[account(
        mut,
        seeds = [b"policy", vault.key().as_ref(), &policy.policy_id.to_le_bytes()],
        bump = policy.bump,
        constraint = policy.vault == vault.key() @ RipcordError::PolicyVaultMismatch
    )]
    pub policy: Account<'info, Policy>,
    /// CHECK: validated by `read_upgrade_authority`, which requires the
    /// upgradeable loader as owner and the ProgramData variant tag, and by the
    /// key check against the policy's recorded address.
    pub target_program_data: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct ExecuteExit<'info> {
    /// The keeper. Its signature buys the right to ask, nothing more.
    pub guardian: Signer<'info>,
    /// CHECK: not a signer and never chosen by the caller — it is required to
    /// equal the owner recorded in the vault, which is the whole delegation
    /// invariant.
    #[account(mut, address = vault.owner @ RipcordError::DestinationNotOwner)]
    pub owner: AccountInfo<'info>,
    #[account(
        mut,
        seeds = [b"vault", vault.owner.as_ref()],
        bump = vault.bump
    )]
    pub vault: Account<'info, Vault>,
    #[account(
        mut,
        seeds = [b"policy", vault.key().as_ref(), &policy.policy_id.to_le_bytes()],
        bump = policy.bump,
        constraint = policy.vault == vault.key() @ RipcordError::PolicyVaultMismatch
    )]
    pub policy: Account<'info, Policy>,
    /// CHECK: the evidence. Validated by `read_upgrade_authority` and by the
    /// key check against the policy's recorded address.
    pub target_program_data: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct Revoke<'info> {
    pub owner: Signer<'info>,
    #[account(
        seeds = [b"vault", owner.key().as_ref()],
        bump = vault.bump,
        has_one = owner @ RipcordError::NotOwner
    )]
    pub vault: Account<'info, Vault>,
    #[account(
        mut,
        seeds = [b"policy", vault.key().as_ref(), &policy.policy_id.to_le_bytes()],
        bump = policy.bump,
        constraint = policy.vault == vault.key() @ RipcordError::PolicyVaultMismatch
    )]
    pub policy: Account<'info, Policy>,
}

#[derive(Accounts)]
pub struct EmergencyOwnerWithdraw<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        mut,
        seeds = [b"vault", owner.key().as_ref()],
        bump = vault.bump,
        has_one = owner @ RipcordError::NotOwner
    )]
    pub vault: Account<'info, Vault>,
}

// ---------------------------------------------------------------- events

#[event]
pub struct Deposited {
    pub vault: Pubkey,
    pub amount: u64,
}

#[event]
pub struct Armed {
    pub vault: Pubkey,
    pub policy_id: u64,
    pub guardian: Pubkey,
    pub baseline_authority: Pubkey,
    pub baseline_is_immutable: bool,
    pub armed_at_slot: u64,
    pub expires_at_slot: u64,
}

/// The on-chain half of a Proof-of-Exit receipt: what the policy expected,
/// what the chain actually saw, and what moved as a result.
#[event]
pub struct ExitExecuted {
    pub vault: Pubkey,
    pub policy_id: u64,
    pub amount: u64,
    pub slot: u64,
    pub baseline_authority: Pubkey,
    pub observed_authority: Pubkey,
    pub observed_is_immutable: bool,
}

#[event]
pub struct Revoked {
    pub vault: Pubkey,
    pub policy_id: u64,
}

// ---------------------------------------------------------------- errors

#[error_code]
pub enum RipcordError {
    #[msg("only the vault owner may do this")]
    NotOwner,
    #[msg("amount must be greater than zero")]
    ZeroAmount,
    #[msg("policy is not armed")]
    PolicyNotArmed,
    #[msg("policy has expired")]
    PolicyExpired,
    #[msg("expiry slot is not in the future")]
    ExpiryInThePast,
    #[msg("signer is not the armed guardian")]
    WrongGuardian,
    #[msg("destination must be the vault owner")]
    DestinationNotOwner,
    #[msg("evidence account does not match the policy target")]
    WrongEvidenceAccount,
    #[msg("account is not an upgradeable ProgramData account")]
    NotProgramData,
    #[msg("the policy predicate is false: the authority has not changed, so nothing moves")]
    PredicateFalse,
    #[msg("amount exceeds the cap set when the policy was armed")]
    ExceedsExitCap,
    #[msg("vault balance is insufficient once rent is reserved")]
    InsufficientVaultBalance,
    #[msg("policy does not belong to this vault")]
    PolicyVaultMismatch,
}
