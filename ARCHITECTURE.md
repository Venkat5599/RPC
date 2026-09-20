# RIPCORD — Architecture

> How it is built, and the constraints that shaped it.
> Companions: [IDEA.md](./IDEA.md) (why), [PRD.md](./PRD.md) (what), [RIPCORD.md](./RIPCORD.md) (frozen submission spec).

---

## 1. System

```
Depositor
  |  signs one policy - revocable, capped, withdraw-to-self only
  v
Web app  ........ position | armed policies | live trigger log | KILL SWITCH
  |
  +--> ripcord_vault (Anchor, mainnet)
  |      PDA holds position receipts
  |      guardian authority = withdraw-to-OWNER only
  |
  v
Policy engine (Rust, single process)
  |    per-user matcher on the hot path
  |
  +<-- aperture.Aperture / SubscribeTransactions   [RPC FAST TxStream]
  |       decoded + ALT-resolved + SIMULATED pending transactions
  |
  +<-- Yellowstone gRPC                            [RPC FAST]
  |       confirmed state, fork reconciliation, vote-derived fork verdicts
  v
Executor  --> Solana mainnet
                exit lands slot N+1
                emits Proof-of-Exit receipt
```

## 2. The dependency, stated so it survives a hostile engineer

`aperture-grpc-client` **v0.6.1**, published 2026-09-17, Apache-2.0.
Endpoint `aperture-txstream.rpcfast.com:443` · service `aperture.Aperture` · methods `SubscribeTransactions`, `SubscribeTransactionBatches` · auth `x-token` metadata.

| Need | Field |
|---|---|
| See a pending Kamino transaction at all | `account_include` matched against **static *and* ALT-loaded** accounts |
| Know what it will do | `include_simulation` → `SimulationConfig`: **token balance deltas, account deltas**, compute units, logs, inner instructions, return data |
| Refuse bad data | `alt_resolution` tri-state: `FULL` / `PARTIAL` / `None` |

**Why Yellowstone at `processed` is not a substitute.** It hands you the transaction; it does not hand you its predicted effect. Computing that effect yourself means running a simulation cluster against live account state for all of Solana.

**Why ALT resolution is load-bearing, not a convenience.** Roughly **two-thirds of a Kamino transaction's addresses arrive via lookup tables**. A stream filtering on static keys alone is structurally blind to the traffic we exist to watch. Precedent for getting this wrong silently: Yellowstone shipped production streams with permanently empty `loaded_addresses` for months because `deshred_transaction_alt_resolution_enabled` defaulted to `false`, fixed in a 2026-07-08 changelog entry.

**Why simulation being *slower* is correct here.** RPC Fast's own benchmarks: simulation adds **791µs median** (~95% claimed accuracy, vendor-unvalidated) against a **284µs** median lead over Jito ShredStream. Net, TxStream-with-simulation is behind a raw feed. Disqualifying for a latency product; it is the thesis for this one. Our opponent is a human incident-response loop.

## 3. Constraint that shapes the whole engine

**`account_include` caps at 10 pubkeys.** For comparison: Shyft allows 150,000 per transaction filter, Helius Preprocessed 5,000, Chainstack 200.

Consequence: **RIPCORD cannot filter per user position.** The subscription is at **program level** — Kamino lend, the oracle programs, BPF Loader Upgradeable — and per-user matching happens **locally, on the critical path**.

This is not a workaround, it is better: one subscription serves every user and cost does not scale with user count. But the local matcher is now latency-critical and must be written as such — pre-indexed by account, no allocation in the hot loop, bounded work per transaction.

## 3b. Backtest harness and public feed

Two components that exist so the product has value before anyone trusts it. Both reuse the policy engine unchanged — that is the point: **the same evaluator runs over historical transactions, live transactions, and simulated pending transactions.** One implementation, three input sources.

```
                     +--> historical tx replay  -> BACKTEST  (day 1-4, no Aperture needed)
Policy engine (one)  +--> Yellowstone confirmed -> PUBLIC FEED
                     +--> TxStream pre-exec sim -> LIVE DEFENCE -> vault exit
```

**Backtest harness.** Replays historical Solana transactions for the watched programs through the evaluator, emitting for each matched incident: detection slot, loss-completion slot, lead time, and dollars in scope at detection. Historical transactions are fetched via `getSignaturesForAddress` + `getTransaction` over bounded incident windows — no Aperture entitlement required, which is why this is day one work. Output is a committed report plus the query, so a judge can recompute it.

**Public detection feed.** The live evaluator publishing every detection free — webhook and a public endpoint. No auth, no deposit, no delegation. Deliberately scoped as a **feed, not a dashboard**: a detection stream with a published hit rate. The graveyard section of [IDEA.md](./IDEA.md) exists because Solana analytics dashboards have won nothing at four consecutive hackathons.

**Design consequence worth stating in the technical demo:** because the evaluator is source-agnostic, the backtest is not a separate codebase written to impress judges — it is the production policy engine running against a different input. That is why the backtested numbers are trustworthy, and it is also the cheapest possible way to get them.

## 3c. On-chain trigger verification — the core technical artifact

**The problem with every automation vault ever built, including the one that lost $6M in July:** an off-chain keeper decides when to act, and the on-chain program does what it is told. The user trusts the keeper's *judgement*, not just its *reach*. Constraining the keeper to withdraw-to-owner limits the damage; it does not make the keeper honest.

**RIPCORD removes the judgement from the keeper.**

`execute_exit` does not accept an assertion that a policy fired. It accepts **evidence**, and re-evaluates the policy predicate **on-chain** against live account state. If the predicate returns false, the instruction fails and no funds move.

```
off-chain engine          on-chain program
--------------------      -----------------------------
detects candidate    -->  execute_exit(policy_id, evidence_accounts)
builds evidence set         |
submits                     +-> load predicate for policy_id
                            +-> read supplied accounts
                            +-> evaluate predicate
                            +-> FALSE -> abort, no movement
                            +-> TRUE  -> withdraw to owner
```

The keeper degrades from a **trusted actor** to an **untrusted relayer**. It can propose. It cannot lie, and it cannot act early. A fully compromised keeper can neither steal (destination is locked to the owner) nor trigger spuriously (the chain checks the condition itself).

### The predicate set

Deliberately **not** a general VM — that is the scope trap. Three parameterised predicates, each a bounded, constant-cost check over supplied accounts:

| Policy | On-chain predicate | Guarantee |
|---|---|---|
| **Authority change** | Read the target program's `ProgramData` upgrade authority; compare against the value recorded at `arm` time | **Fully trustless.** The chain sees the authority directly |
| **Oracle staleness / feed death** | Read the oracle account; compare `publish_slot` against `Clock::slot` and the user's threshold | **Fully trustless.** Slot age is on-chain and self-evident |
| **Anomalous outflow rate** | Compare protocol reserve balances against a watermark written at `arm` time, over a bounded slot window | **Partially trustless.** Reserve deltas are on-chain, but the *window* is keeper-supplied. Documented as a weaker guarantee rather than hidden |

**Two of three are fully verifiable with no trust in us at all.** The third is honest about its weaker property. That distinction goes in the technical demo — a team that says "this one is weaker and here is exactly why" is more credible than one claiming uniform guarantees.

### Why this is the right kind of hard

It converts a **trust assumption into a verification**, which is the only move in this space that actually answers Summer.fi. It costs one extra account read and a comparison in the exit transaction — negligible compute, no latency impact on a next-slot exit.

And it makes the policy set **extensible in the honest direction**: new policies are new predicates, each of which must be expressible as a bounded on-chain check. A policy that cannot be verified on-chain does not ship. That constraint is a feature — it is what keeps the product trustless as it grows.

**Stretch, explicitly not MVP:** a small opcode set (load account field, compare, boolean combine) so users compose their own verifiable predicates without a program upgrade. Do not attempt this in three weeks.

## 4. Program surface — `ripcord_vault` (Anchor)

| Instruction | Signer | Notes |
|---|---|---|
| `deposit` | owner | position receipts into the PDA |
| `set_policy` | owner | policy stored on-chain, publicly auditable |
| `arm` | owner | activates guardian authority, capped and time-bounded |
| `execute_exit` | **guardian** | **withdraw-only; destination hard-locked to the owner** |
| `revoke` | owner | single transaction, no delay, no quorum |
| `emergency_owner_withdraw` | owner | always available, bypasses everything |

### The delegation invariant

**The guardian can only ever move funds to the owner.** It cannot redirect, cannot spend, cannot swap to an arbitrary mint. Enforced in the program, not in the client.

This is the first slide of the pitch, not a footnote. It is the only reason delegating withdrawal authority to a third party is defensible at all.

## 5. Policy engine

Three primitives in the MVP. Each maps to fields the simulation actually returns.

| Policy | Trigger signal | Simulated evidence used |
|---|---|---|
| **Authority change** | Pending tx to BPF Loader Upgradeable or a protocol admin instruction altering upgrade/admin authority | decoded instruction + resolved accounts |
| **Oracle staleness / feed death** | Oracle account age past threshold, or a pending tx that deactivates/retires a feed | account deltas |
| **Anomalous outflow** | Aggregate simulated token balance deltas out of a protocol's reserves exceeding a rolling baseline within a window | token balance deltas |

**Evaluation order per transaction:**

1. `alt_resolution != FULL` → **refuse, log, alert, stop**. Never evaluate a policy against an incomplete account set.
2. Local per-user matcher against resolved accounts.
3. Policy evaluation against simulated deltas.
4. Rate-limit and budget check.
5. Dispatch to executor.

## 6. Exit ladder

Escalating, least-destructive first:

```
repay  ->  partial de-risk  ->  full withdraw
```

A policy resolves to the lowest rung that clears the risk. Full withdrawal is the last resort, not the default. This is both better for the user and the primary defence against becoming a bank-run machine.

## 7. Failure handling

| Failure | Handling |
|---|---|
| **Fork reversal** — shred-observed txs can sit on a branch that dies | Every exit idempotent; re-check against `confirmed` before retry; fork verdicts come from Yellowstone, since votes live in blocks |
| **False positive** | Position-preserving rung first; per-user per-epoch rate limit; owner kill switch always live |
| **Trigger storm** | Global per-protocol trigger budget per epoch — RIPCORD refuses to be the bank run even if the signal says otherwise |
| **`PARTIAL` ALT resolution** | Hard refusal, logged and alerted. Never act |
| **Stream disconnect** | Client reconnects with exponential backoff (100ms → 5s, keepalive-while-idle); gap is reconciled from Yellowstone before policies re-arm |
| **Simulation wrong** (~1 in 20) | Errors correlate with contested state — exactly the transactions that matter. Mitigated by the ladder and rate limits, disclosed in the published FP rate |
| **Protocol pauses withdrawals** | **The honest hard limit. If the protocol halts, RIPCORD cannot exit either.** Our window is between the first malicious transaction and the pause — on Drift that was 16:05:18 UTC to an unpublished pause time, with the last drain at 18:31 UTC, so hours wide. State this before a judge does; do not let it be discovered in Q&A |

## 8. Proof-of-Exit

Every action emits an on-chain receipt:

```
trigger_signature, trigger_slot,
simulated_deltas_that_fired,
policy_id,
exit_signature, exit_slot
```

Three jobs at once: the artifact a judge verifies on their own phone; the audit trail that makes false positives accountable instead of hidden; the underwriting data for the insurance product later.

## 9. Shadow Mode

Arm any policy with **zero delegation and zero deposit**. The engine evaluates against a real position and reports what it *would* have done.

- Solves cold-start trust — try before delegating.
- Produces a metric before anyone deposits.
- Makes the live demo safe to run in front of judges.
- Is the mechanism for the user conversations Colosseum weights hardest.

No on-chain authority is granted in Shadow Mode. That absence is itself an acceptance criterion ([PRD A9](./PRD.md)).

## 10. Build order

| Days | Work | Gate |
|---|---|---|
| 1–2 | Program: full instruction set, delegation invariant, negative tests. Devnet → mainnet | A1–A3 |
| 3–4 | TxStream client, program-level filter, simulated-delta parser, `PARTIAL` refusal path | A4, A5 |
| 5–7 | Kamino adapter, executor, idempotent fork-safe retry, next-slot landing proven by signature | A6–A8 |
| 8–10 | Shadow Mode + the single screen | A9 |
| 11–14 | Proof-of-Exit receipts; Kamino cohort outreach; RPC Fast writeup | A10, A13 |
| 15–18 | Test protocol deployed; live trigger rehearsed until boring | A11, A12 |
| 19–21 | Videos, repo access, submit to all tracks | A14 |

## 11. Deliberate non-goals

No shred pipeline. No indexer. No analytics dashboard. No transaction debugger. No fee optimizer. No protocol-side pause button.

Each is in the [graveyard](./IDEA.md) with the evidence that killed it. Worth repeating one: **Alpenglow's Rotor transmits a single erasure-coded version of each shred, eliminating the data/coding distinction every deshredder is built around.** Any team hand-rolling a shred pipeline this month is buying a maintenance liability with a known expiry. We consume protobuf and let RPC Fast absorb the wire-format churn — which is an additional argument for building *on* TxStream rather than beside it.

## 12. Known-unverified

Carried forward honestly rather than laundered into fact.

- No public `.proto` for `aperture.Aperture` was located — only the Rust client wrapping a generated crate. Confirm the wire contract before hour 24.
- The ~95% simulation accuracy figure is vendor-claimed and independently unvalidated.
- RPC Fast's published pricing contradicts itself on which tier includes TxStream, and on Aperture's price ($399 blog vs $499 pricing page). Their docs also state simulation cost as "791 ns" in one place, a units typo against every other figure they publish.
- Where RPC Fast sources shreds following **Jito ShredStream's shutdown on 5 Sep 2026** is unstated; their homepage copy referencing Jito is at least partially stale.
- Slot time moved 400ms → 350ms on 22 Aug 2026 under SIMD-0525, heading to 200ms. Every pre-Aug-2026 latency benchmark in circulation is against a 400ms slot.
