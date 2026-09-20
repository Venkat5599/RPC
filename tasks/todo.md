# todo.md — active work plan

> Operating rules: plan before non-trivial work · verify with a probe before marking done ·
> subagents for research and parallel analysis · lessons.md updated after every correction.
> Source of truth for scope: [TODO.md](../TODO.md). This file tracks the CURRENT slice only.

## Current slice — Phase 1: Backtest engine (TODO.md days 1-4, no Aperture dependency)

Goal, stated as the sentence it must produce:
**"Across N real incidents our authority-change policy fired a median of X minutes before the loss completed, covering $Y of depositor funds."**

### Plan

- [x] 1. Workspace scaffold — cargo workspace, `ripcord-policy` (evaluator) + `ripcord-backtest` (historical source)
- [x] 2. RPC client — `getSignaturesForAddress` + `getTransaction` (jsonParsed, maxSupportedTransactionVersion=0), bounded by slot window, retry + rate-limit aware, endpoint from `RIPCORD_RPC_URL`
- [x] 3. Local tx cache on disk — fetch once, replay free (runs get re-run dozens of times; paid RPC credits are finite)
- [x] 4. Anchor discriminator decode — `sha256("global:<ix_name>")[0..8]`, plus BPF Loader Upgradeable `SetAuthority`/`SetAuthorityChecked` raw-enum decode
- [x] 5. `Policy` trait + `AuthorityChange` evaluator — input is a normalised `TxView` (resolved accounts + deltas), NOT an RPC type, so TxStream can feed the same code later (L3)
- [x] 6. Incident windows as data — Drift 1 Apr 2026 16:05-18:31 UTC, Kamino Oct 2025, marginfi 11-13 Apr 2024; slots resolved from timestamps and committed
- [ ] 7. Lead-time metric — detection slot vs loss-completion slot, in slots AND minutes (NB: 400ms -> 350ms slot change 22 Aug 2026, SIMD-0525; convert per-window, not globally)
- [ ] 8. Dollars-in-scope at detection
- [x] 9. Report writer — `reports/backtest-YYYY-MM-DD.md` + the exact query/command that reproduces it
- [x] 10. Unit tests on fixtures: known authority-change tx fires, an ordinary tx does not, a `PARTIAL` account set refuses

### Verification (probe per item, nothing marked done without it)

| Item | Probe |
|---|---|
| 2, 3 | Cached fixture count > 0 for each incident window; second run does zero network calls |
| 4 | Discriminator of a known Anchor ix matches the on-chain bytes in a committed fixture |
| 5, 10 | `cargo test` — positive, negative and refusal cases all green |
| 7 | Lead time recomputed by hand from two Solscan timestamps matches the tool's output |
| 9 | Fresh clone + documented command reproduces the committed report byte-for-byte |

### Blocked / needs a decision

- **B1 — Paid RPC endpoint.** Public RPC rate-limits out of a multi-hour window fetch. Need a Helius/Triton/QuickNode key in `RIPCORD_RPC_URL`.
- **B2 — Aperture access** (RPC Fast). Blocks Phase 3 only. Email draft in RIPCORD.md §10.
- **B3 — solana CLI + anchor not installed.** Blocks Phase 2 (vault). Install before day 5.

## Review — 20 Sep 2026

**Proven, with the probe that proved it.**

- Workspace builds and 17/17 tests pass (`cargo test`). Toolchain is WSL Ubuntu: Windows-native
  Rust cannot link here (mingw `ld` 116, no MSVC build tools), and WSL already had solana-cli
  3.1.15 and anchor-cli 0.32.1.
- Anchor discriminator decode matches the documented `sha256("global:initialize")` prefix
  `afaf6d1f0d989bed`.
- Policy fires on a loader `SetAuthority`, fires on a protocol admin instruction by discriminator,
  stays silent on ordinary traffic and on a code-only `Upgrade`, and refuses an incomplete account
  set. Five behavioural tests.
- Program IDs are verified against mainnet rather than trusted from memory. Kamino Lend
  `KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD` resolves: executable, ProgramData
  `9uSbGW1y9H5Av6H5TKxQ1wnFApSq2t3oEpfF2YfjDQGA`, upgrade authority
  `GzFgdRJXmawPhGeBsyRCDLx4jAKPsvbUqoqitzppkzkW`.
- End-to-end replay runs against live mainnet: enumerate, fetch, cache, normalise, evaluate,
  report. `--incident smoke-recent` is the self-test and needs no archival history.
- ALT completeness is handled for both wire shapes and guarded by five tests. See lessons L6.

**Not proven, stated plainly.**

- **No real incident number exists yet.** The report says so itself rather than estimating one.
- **The archival blocker (B1) is now the critical path, and it is worse than "rate limits".**
  `getSignaturesForAddress` pages backwards from the present, so reaching October 2025 means
  walking millions of signatures. Public mainnet RPC cannot do it at any patience. The fix is
  slot-bounded scanning: resolve the window's slot range by binary search on `getBlockTime`, then
  read it with `getBlocks` + `getBlock`. That needs an archival endpoint.
- The cache-reuse probe has not been run: the smoke window is relative to now, so each run
  enumerates different signatures. Needs a fixed window, which needs B1.
- Drift and marginfi program IDs are not established, so those windows currently enumerate the
  loader and watch nothing. The report prints that rather than hiding it.

## Slice 2 — 20 Sep 2026, same day

**Done, with the probe.**

- [x] Slot-bounded window scanning. Binary search over `getBlockTime` resolved a one-minute
      window to slots 448691655..=448691885 with the observed times matching. Block reading works;
      public RPC then reset the connection under load, which is the endpoint, not the scanner.
- [x] `ripcord_vault` Anchor program builds to SBF (282KB) with IDL. Program ID
      `84h2juN2WaFhZtGbaqmeWVVe7HQCUXnDRRtbGZQeAeTu` (local keypair, not yet deployed).
- [x] **PRD A2 proven.** `the_guardian_cannot_send_funds_to_a_third_party`: predicate genuinely
      true, guardian correctly signed, destination anywhere but the owner → rejected, zero lamports
      moved.
- [x] **The predicate proven false-safe.** `the_guardian_cannot_fire_while_the_predicate_is_false`:
      the chain re-read the authority, found it unchanged, and refused.
- [x] Byte layout verified against mainnet: our parser and Solana's `jsonParsed` decoder
      independently produce `GzFgdRJXmawPhGeBsyRCDLx4jAKPsvbUqoqitzppkzkW` for Kamino Lend.
      Committed as a fixture so it stays proven.
- [x] Transport errors now retry. A connection reset is how a rate limiter often says no, and
      treating it as fatal ended a long scan on a condition that clears by waiting.

**Found on the way.**

- Mainnet carries **version-1 transactions**. `getBlock` rejects the whole block if the client asks
  for a lower ceiling, so this stops a scan rather than degrading it. No project doc mentions v1.
- Anchor requires `overflow-checks`. Enabled, and correct regardless for code that moves money.
- Windows-native Rust cannot link here at all (mingw `ld` 116, no MSVC build tools). WSL is the
  build environment, and it already had solana-cli 3.1.15 and anchor-cli 0.32.1.

## Slice 3 — devnet, same day

- [x] **Deployed to devnet.** `84h2juN2WaFhZtGbaqmeWVVe7HQCUXnDRRtbGZQeAeTu`, deploy signature
      `2vV6LrQbaRsHThSywaJBZWt3CfF255JJx33NWoqZsqHHPY4WnkjWwmWDmPWJYJtaRSrD31cN5fdAdm4Kewdxc6c5`.
      First attempt failed with 12 write transactions dropped; public devnet needs
      `--with-compute-unit-price 50000 --max-sign-attempts 200`.
- [x] `ripcord-keeper` CLI: vault-init, deposit, policy, arm, exit, revoke, withdraw, seize, status.
- [x] **The demo beat, proven on a real cluster.** See [DEMO.md](../DEMO.md). The same exit was
      refused before the authority moved and landed after it. Owner balance 1.61380236 →
      1.66379236 SOL.
- [x] **A2 on a live cluster.** With the predicate genuinely true, an exit aimed at a third party
      was refused by the program.
- [x] Upgrade authority restored to the deployer after the demo.

## Next

- [ ] Deploy to mainnet (A1). Needs real SOL: ~2 SOL for a 282KB program
- [ ] Establish and verify Drift and marginfi program IDs; pin Kamino's incident day
- [ ] 7. Lead-time metric — wired, but unexercised until a real window runs
- [ ] 8. Dollars in scope at detection
- [ ] Day 0 items from TODO.md: RPC Fast application form, Panta questions, founder-market fit
