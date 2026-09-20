# RIPCORD

**The first stop-loss that triggers on intent, not price.**

Seven years of DeFi automation fires on price. Price is a lagging signal, so every automated
defence in DeFi fires *after* the damage. RIPCORD fires on the transaction that causes the damage,
before it executes, read from a pre-execution stream with its effects already simulated.

Every security product on Solana protects the *protocol*. None of them moves a *depositor's* funds.

- **marginfi, Apr 2024** — CEO resigned. No hack. Depositors pulled >$130M in 24 hours, by hand.
  That race is what this automates.
- **Drift, Apr 2026** — ~$285M over ~2.5 hours. Depositors never got to run; withdrawals paused.
- **Kamino, Oct 2025** — 9,372 liquidations, 1,895 wallets, $25.5M seized.

Built for the **Colosseum Crypto World's Fair** (submissions close 12 Oct 2026), entering the
Solana track, the **RPC Fast** infrastructure sidetrack, and the **Panta** sidetrack.

## The two things that make it defensible

**The keeper cannot lie.** `execute_exit` does not accept an assertion that a policy fired. It
accepts evidence, and the program re-evaluates the predicate **on-chain** against live account
state. A false predicate aborts and nothing moves. The keeper degrades from a trusted actor to an
untrusted relayer: it can propose, it cannot act early, and it cannot steal, because the
destination is hard-locked to the vault owner.

**The sponsor dependency is structural, not a preference.** Roughly two thirds of a Kamino
transaction's addresses arrive via lookup tables, so a feed without ALT resolution is blind to the
traffic this watches. Pre-execution simulation is what turns a pending transaction into a policy
input. Remove that stream and the product stops; it does not degrade.

We never claim latency. Simulation costs ~791µs against a ~284µs lead — deliberately. The opponent
is a human incident-response loop, not a colocated bot.

## Status

Day 1 of 23. **Phase 1 (backtest) is in progress. There is no backtested number yet, and the
generated report says so rather than estimating one.**

| Component | State |
|---|---|
| `ripcord-policy` — evaluator, authority-change policy | 10 tests passing |
| `ripcord-backtest` — historical replay, slot-bounded block scanner | 7 tests; runs end to end against mainnet |
| `ripcord_vault` — Anchor program | builds to SBF; 20 tests, including the delegation invariant |
| Live TxStream engine | not started (Phase 3, needs Aperture access) |

**37 tests pass across the workspace.** The ones worth naming:

- `the_guardian_cannot_fire_while_the_predicate_is_false` — a correctly-signed exit from the
  correct guardian is rejected because the chain checked the condition and disagreed.
- `the_guardian_cannot_send_funds_to_a_third_party` — with the predicate genuinely true, an exit
  aimed anywhere but the owner still fails. This is PRD A2.
- `parses_the_authority_of_a_real_mainnet_program` — the on-chain parser and Solana's own
  `jsonParsed` decoder agree on the authority of Kamino Lend's real `ProgramData` account. Two
  independent decoders on real bytes, not a buffer agreeing with itself.

These run in LiteSVM against the compiled `.so`, which lets the test rewrite a `ProgramData`
account's authority bytes — an attacker seizing a protocol, reproduced deterministically.

### One evaluator, three input sources

The backtest is not a separate codebase written to look good in a submission. It is the production
policy engine reading a different input:

```
                     +--> historical replay     -> BACKTEST
Policy engine (one)  +--> Yellowstone confirmed -> PUBLIC FEED
                     +--> TxStream pre-exec sim -> LIVE DEFENCE -> vault exit
```

That is why a backtested number would say something about the live engine, and it is the cheapest
honest way to get one.

## Running it

Requires a Rust toolchain. An archival RPC endpoint is required for the historical windows;
`RIPCORD_RPC_URL` selects it, and it falls back to public mainnet.

```bash
# The program: build, then run the invariant tests against the compiled .so
anchor build
cargo test --workspace

# Check every configured program ID against the chain before trusting it
cargo run -p ripcord-backtest -- verify

# Live self-test of the whole replay path — needs no archival history
cargo run -p ripcord-backtest -- run --incident smoke-recent --smoke-minutes 2 --limit 40

# Replay the incident windows and write a report
export RIPCORD_RPC_URL="https://your-archival-endpoint"
cargo run -p ripcord-backtest -- run --out reports/backtest.md

cargo test
```

`verify` exists because a program ID typed from memory that happens to look plausible is the
easiest way to publish a confidently wrong report.

## Known limits, stated here rather than discovered in Q&A

- **If a protocol pauses withdrawals, RIPCORD cannot exit either.** The window is between the first
  malicious transaction and the pause. On Drift that was hours wide, but it is a real ceiling.
- The slot-bounded block scanner is built and resolves a window correctly, but public mainnet RPC
  resets the connection under `getBlock` load. Reaching a historical window needs an archival
  endpoint; this is the one thing standing between here and a real backtest number.
- `ripcord_vault` is not deployed. It builds, and its invariants are tested against the compiled
  program, but no mainnet program ID exists yet.
- Drift and marginfi program IDs are not yet established, so those windows currently enumerate
  without watching anything. The report prints that.
- Only the authority-change policy is implemented. Oracle staleness and outflow rate are roadmap.

## Documents

| | |
|---|---|
| [BRIEF.md](./BRIEF.md) | one page, start here |
| [IDEA.md](./IDEA.md) | why, including the prior art we name ourselves |
| [PRD.md](./PRD.md) | what ships, with a probe per acceptance criterion |
| [ARCHITECTURE.md](./ARCHITECTURE.md) | how, and the constraints that shaped it |
| [RIPCORD.md](./RIPCORD.md) | frozen submission spec |
| [TODO.md](./TODO.md) | the solo plan, with an explicit cut line |
| [tasks/](./tasks/) | active plan and the lessons log |

## Licence

Apache-2.0.
