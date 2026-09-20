# RIPCORD

# The first stop-loss that triggers on intent, not price.

Seven years of DeFi automation fires on price — a lagging signal, which means it fires **after** the damage. RIPCORD fires on the transaction that causes the damage, **before it executes**.

*Simple version, for the first ten seconds of any pitch:* **an airbag for your money.**

**Protocols got bodyguards in April. Depositors are still reading Discord.**

> Frozen spec — Colosseum "Crypto World's Fair", 14 Sep – 12 Oct 2026.
> Entry: Solana track ($100k / 10 × $10k) + RPC Fast Superteam Infrastructure sidetrack + Superteam regional sidetracks.
> Status: committed. Day-one blocker is an email, see §10.

---

## 1. One line

**An airbag for your money.**

Sits there costing you nothing. Deploys faster than you can react. You don't need to understand the crash sensor to want one.

Mechanically: a non-custodial vault that watches what Solana is *about to do* and pulls your money out of a protocol before the block lands — because every security product on Solana sells to the protocol, and nobody moves the depositor's funds.

**The structural constraint we remove:** DeFi's risk model assumes the depositor is a passive holder who cannot react at machine speed. That is why you cannot buy a guaranteed maximum loss, why insurance pays out after instead of preventing, why leverage stays conservative, and why mandated capital stays out. Remove that assumption and underwritable insurance, safe leverage and institutional allocation all become possible. Full argument in [IDEA.md](./IDEA.md).

## 2. The problem

- **THE STAT — marginfi, 11 Apr 2024**: the CEO resigned. **No hack, no exploit, no funds at risk — just news.** Depositors pulled **over $130M out in the next 24 hours**, cutting TVL by **25%**. The trigger was information, and humans responded by manually racing for the exit. That race is the thing we automate.
- **Drift, 1 Apr 2026**: ~$285M across 18 tokens. Two pre-signed durable-nonce transactions at **16:05:18 and 16:05:19 UTC** handed the attacker full admin control one second apart; the last confirmed drain was **18:31 UTC** — a **~2.5 hour** window. Funds bridged off Solana within 23 minutes. Insurance fund ~$20M. **Drift then paused deposits and withdrawals — depositors never got the chance to run at all.** TVL still bled a further ~9.5% over the following six days, as people left the moment they could.
- **Kamino, Oct 2025**: **9,372 liquidation events (+309% month-over-month), 1,895 unique wallets liquidated, $25.5M collateral seized.** SOL fell 14% in under an hour, $207 → $177, on tariff news. Kamino's own governance report calls it *"sharp, volatility-driven liquidations."* Separately and importantly, Kamino's documented position on premature liquidation: *"TWAP is meant to protect the platform from bad debt, not to protect users from premature liquidation."*
- **Switchboard**: announced shutdown with six days' notice across 550+ feeds, with Kamino, Jito, marginfi and Drift named as integrators. A scheduled, foreseeable, multi-day risk event that no depositor has tooling to act on automatically.

Since April 2026 the Solana Foundation funds **STRIDE** (free 24/7 monitoring above $10M TVL, formal verification above $100M) and **SIRN** (Asymmetric, OtterSec, Neodyme, Squads, Zeroshadow). Hypernative, Hexagate, Range and Riverguard all sell to protocols. Every layer of that stack is protocol-side **by business model, sales motion and liability posture**.

## 3. The insight

Every other team reaching for this sponsor's stack will sell milliseconds. The measured shred advantage over Yellowstone at `processed` is **~5–12ms p50**, and RPC Fast's own numbers show simulation costs **791µs against a 284µs lead** — TxStream-with-simulation is *slower* than a raw feed. For a latency product that is disqualifying.

**So don't sell latency. Sell the outcome prediction, and pick a fight where the opponent is a human.**

TxStream's real gift is that somebody else already simulated every pending transaction. Doing that yourself means running a simulation cluster against live account state for all of Solana. That is an economic and engineering dependency, and unlike a timing claim it survives a hostile engineer.

Say it out loud in the pitch: *we deliberately give up the milliseconds to buy the outcome, because our opponent is an incident-response loop, not a colocated bot.*

## 4. Sponsor integration — verified, not inferred

`aperture-grpc-client` **v0.6.1, published 2026-09-17**, Apache-2.0.
Endpoint `aperture-txstream.rpcfast.com:443`, service `aperture.Aperture`, methods `SubscribeTransactions` / `SubscribeTransactionBatches`, `x-token` auth.

| What RIPCORD needs | Field that provides it |
|---|---|
| See a pending Kamino tx at all | `account_include` matched against **static *and* ALT-loaded** accounts — ~⅔ of a Kamino transaction's addresses arrive via ALTs, so a static-key filter is structurally blind |
| Know what it will do to a position | `include_simulation` → `SimulationConfig` returns **token balance deltas, account deltas**, CUs, logs, inner instructions |
| Refuse to act on bad data | `alt_resolution` tri-state — hard-refuse on `PARTIAL`, log it, never evaluate a policy against an incomplete account set |

**Remove RPC Fast and the product stops, it does not degrade.** Yellowstone hands you the transaction. It does not hand you its predicted effect.

Secondary: Yellowstone gRPC for confirmed-state reconciliation and fork resolution.

## 5. Architecture

```
Depositor
 |  signs one policy - revocable, capped, withdraw-to-self only
RIPCORD app - position, policies, live trigger log, kill switch
 |
ripcord_vault (Anchor) - PDA holds receipts; guardian is withdraw-to-OWNER only
 |
Policy engine (Rust) - local per-user matcher on the hot path
 |
aperture.Aperture / SubscribeTransactions  <- decoded + ALT-resolved + SIMULATED
Yellowstone gRPC                           <- confirmed state, fork reconciliation
 |
Solana mainnet - exit lands in slot N+1, emits a Proof-of-Exit receipt
```

**Hard constraint:** `account_include` caps at **10 pubkeys** (Shyft allows 150,000; Helius 5,000). RIPCORD therefore **cannot filter per user position**. Subscribe on a handful of **program IDs** — Kamino lend, the oracle programs, BPF Loader Upgradeable — and do per-user matching **locally, on the critical path**. One subscription serves every user.

## 6. Program surface

`ripcord_vault` (Anchor):

- `deposit`
- `set_policy`
- `arm`
- `execute_exit` — guardian-signed, **withdraw-only, destination locked to the owner**, and the program **re-evaluates the policy predicate on-chain** against live accounts. The guardian submits *evidence*, not an assertion; a false predicate aborts and nothing moves
- `revoke` — single transaction, owner-only
- `emergency_owner_withdraw`

**The guardian can only ever move funds to the owner.** It cannot redirect, cannot spend, cannot swap to an arbitrary mint.

**And it cannot lie.** The program re-evaluates the policy predicate on-chain before releasing anything, so the guardian is an untrusted relayer rather than a trusted actor — it proposes, the chain verifies. Two of the three MVP policies (authority change, oracle staleness) are **fully trustless**; the third (outflow rate) is documented as a weaker guarantee rather than hidden. See [ARCHITECTURE.md §3c](./ARCHITECTURE.md).

This is the answer to *"Summer.fi's automated vaults lost $6M in July — why are you different?"* — and it goes on the first slide, not in the Q&A.

## 7. The three sharpenings

1. **Shadow Mode** — arm any policy with zero delegation and zero deposit. RIPCORD shows what it *would have* done, live, on a real position. Solves cold-start trust, produces a metric before anyone deposits, generates the user conversations Colosseum weights hardest, and makes the demo safe to run in front of judges.
2. **Proof-of-Exit** — every action emits an on-chain record: trigger signature and slot, the simulated deltas that fired the policy, exit signature and slot. Publish the false-positive rate against replayed history rather than burying it. Seed of the insurance business; the artifact a judge verifies on their own phone.
3. **Bank-run guard, stated proudly** — exits escalate: repay → partial de-risk → full withdraw. Per-user rate limits, global per-protocol trigger budget, owner kill switch in one transaction. A system that automatically empties a protocol on a false signal *is* the risk; naming it before the judge does is the difference between a design and a liability.

## 8. MVP — nothing else

Build in this order. The first item is the highest-value artifact per hour in the entire plan and does not depend on Aperture access to start.

1. **Backtest engine** — run the three policies against Solana's last twelve months. Output: *"across N real incidents, our policies fired a median of X minutes before the loss completed, covering $Y of depositor funds."* Public data, independently recomputable by a judge, needs zero users. **Day 5, before the UI.**
2. **Public detection feed** — every live detection published free. No deposit, no delegation, no signature. This is where traction comes from: the ask is a webhook, not a bank transfer. **A feed, not a dashboard** — detections with a backtested hit rate, never charts.
3. TxStream subscription, program-level filter, simulation on.
4. Three policies only: **authority/upgrade-authority change**, **oracle staleness or feed death**, **anomalous outflow rate**.
5. `ripcord_vault` on **mainnet**, one adapter: Kamino. Proven with our own capital.
6. Exit landing in slot N+1, proven by signature.
7. One screen. Shadow Mode toggle.

**Explicitly NOT building:** a shred pipeline, an indexer, an analytics dashboard, a transaction debugger, a fee optimizer, a protocol-side pause button, or anything whose pitch contains the word "milliseconds."

**Stretch, clearly separate:** multi-protocol adapters, health-factor policies, partial de-risking, policy marketplace, insurance priced on protected TVL, cross-ecosystem (the primitive generalises to any chain with a sequencer — the World's Fair story).

## 9. Demo — 110 seconds

| Time | Beat |
|---|---|
| 0:00–0:12 | "In April 2024 marginfi's CEO resigned. No hack. No funds at risk. Just news. Depositors pulled 130 million dollars out in 24 hours — by hand." |
| 0:12–0:22 | "Two years later Drift lost 285 million over two and a half hours — and its depositors never even got to run, because withdrawals were paused. Every security product on Solana protects the protocol." |
| 0:22–0:32 | **The backtest.** "We ran our policies against Solana's last twelve months. They fire a median of X minutes before the loss completes, across N real incidents, covering $Y." Public data. Recompute it yourself. |
| 0:32–0:40 | A real mainnet position in a RIPCORD vault. One armed policy, visible on-chain. |
| 0:35–0:52 | Live TxStream panel: pending transactions *with their simulated effects*. "This is the only feed that tells you what a transaction will do before it does it." |
| 0:52–1:15 | Fire a real authority-change transaction against our own deployed mainnet protocol — **stated out loud as ours**. It appears in the pending panel. RIPCORD's exit lands **in the next slot**. |
| 1:15–1:30 | Two Solscan tabs: the trigger's tab is still empty. The exit already confirmed. |
| 1:30–1:45 | Replay the real Drift exploit sequence through the live engine. First trigger fires at **minute 0.4 of 47**. |
| 1:45–2:00 | "Protocols have had bodyguards since April. Your deposits get one today." |

**Holy-shit moment:** the judge watches a defensive transaction confirm against a transaction that, on every explorer on earth, has not happened yet. Reproducible live. Impossible without the sponsor's stream.

**Never fabricate activity.** The trigger is our own deployed protocol, said out loud. The Drift replay uses real historical transactions.

## 10. Day-one blocker

The Colosseum offer is the **Focus** plan at $45/mo with **no gRPC streams at all**. Yellowstone starts at **Stream ($249)**. **TxStream is Aperture-tier only ($499)** — the sidetrack's own top prize is one month of Aperture, which tells you where it lives. RPC Fast's published pricing contradicts itself in two places (homepage lists TxStream under Stream; blog says Aperture is "from $399"), so get the entitlement **confirmed in writing**, not inferred.

### Email draft

> Subject: Colosseum World's Fair — Aperture/TxStream access request (project fit)
>
> Hi RPC Fast team,
>
> We're building RIPCORD for Colosseum's Crypto World's Fair and entering your Superteam Infrastructure sidetrack. RIPCORD is a non-custodial vault that exits a depositor's DeFi position when a protocol-level risk event fires — automated user-side defence, as opposed to the protocol-side monitoring that STRIDE and Hypernative already cover.
>
> The product depends on Aperture TxStream specifically, not on latency:
>
> 1. `account_include` matched against ALT-loaded accounts. Roughly two-thirds of a Kamino transaction's addresses arrive via lookup tables, so a static-key filter cannot see the traffic we need.
> 2. `include_simulation` — we evaluate user policies against `SimulationConfig`'s token balance deltas and account deltas. Without predicted execution effects there is no decision to make.
>
> The Focus plan offered to hackathon participants carries no gRPC streams, so we'd like to request Aperture access under "other plans available upon request based on project fit." We'll be holding a continuous TxStream subscription throughout the hackathon and publishing our integration writeup — including the honest finding that TxStream-with-simulation trades ~791µs of latency for outcome prediction, which is exactly the tradeoff our product wants.
>
> Happy to share the repo and architecture doc.

## 11. Traction

- **First five users are enumerable from public data**: wallets liquidated on Kamino during the hour of 10 Oct 2025. Named, addressable, provably harmed. Reach via Superteam Discord and X; put the transcript on the submission page. Shadow Mode lets them try before trusting.
- **First transactions**: own deposits plus those five vaults, on mainnet.
- **Primary metrics** — achievable in three weeks, judge-verifiable, requiring zero user trust: (1) **backtest coverage** — incidents matched, median lead time before loss completion, dollars in scope, recomputable from public data; (2) **feed consumers** — wallets and protocols subscribed to the free detection stream; (3) **median trigger-to-exit slot delta**, proven on mainnet with our own capital.
- **Not the headline**: dollars under armed policy. Nobody deposits meaningful money into an unaudited three-week-old vault, and leading with it invites "how much?" — to which the honest answer is embarrassing. Report only if real.
- **Sidetrack**: continuous TxStream subscription from day one is exactly the demonstrated-usage bonus RPC Fast paid at Frontier. Publish 2–3 posts as required.

## 12. Three weeks to 12 October

| When | Work |
|---|---|
| **Today** | Email RPC Fast for Aperture. Nothing else unblocks without it. |
| Days 1–2 | Policy definitions + **backtest harness** against historical Solana data. No Aperture dependency — start here regardless of the entitlement. |
| Days 3–4 | **Backtest result published.** `ripcord_vault` instruction set, devnet → mainnet. |
| Days 5–7 | TxStream client, program filter, simulated-delta parser, `PARTIAL` refusal path. **Public detection feed live.** |
| Days 8–10 | Kamino adapter, executor, idempotent fork-safe retry. Next-slot landing proven. Shadow Mode + the one screen. |
| Days 11–14 | Contact the Kamino cohort. Ship the RPC Fast writeup. Proof-of-Exit receipts live. |
| Days 15–18 | Deploy the test protocol. Rehearse the live trigger until it is boring. |
| Days 19–21 | Pitch video **under 3 min**, separate technical demo explaining the Solana integration, repo access confirmed, every optional field filled. Submit to Solana track + RPC Fast sidetrack + Superteam regionals. |

## 13. Failure handling

- **Fork reversal** — every exit idempotent, re-checked against `confirmed` before retry.
- **False positive** — position-preserving first (repay before withdraw), rate-limited per user per epoch, owner kill switch always available.
- **Trigger storm** — per-protocol circuit breaker so RIPCORD cannot itself become the bank run.
- **`PARTIAL` ALT resolution** — refuse to evaluate, log, alert. Never act on an incomplete account set.

## 14. Honest scorecard

| Dimension | /10 | Evidence |
|---|---|---|
| Novelty | 8 | **Intent-triggered defence has no prior art on any chain.** Automated position management (DeFi Saver, 2019) and two-sided epoch insurance (Y2K) both exist — and both trigger on *price*. Triggering on a pending transaction's simulated effect is the new class. Scored honestly: ~5 on the components, 8 on the primitive and the composition |
| Technical depth | 9 | **On-chain trigger verification** — the program re-evaluates the policy predicate against live accounts, so the keeper cannot lie. Plus exactly-once money movement under reorg, a hot-path matcher forced by the 10-pubkey filter cap, and a source-agnostic evaluator running over historical, confirmed and pre-execution inputs |
| Real pain | 9 | marginfi: >$130M pulled in 24h on *news alone*, TVL −25%. Drift: $285M over ~2.5h with withdrawals paused. Kamino Oct 2025: 9,372 liquidations, 1,895 wallets, $25.5M seized. All from primary sources |
| Hackathon fit | 8 | Infra-shaped, mainnet, working product, addressable users |
| Sponsor integration | 9 | Fails closed without ALT resolution + simulation; verified in a live client library |
| Demo | 9 | Judge verifies it on their own phone |
| Traction | 9 | Backtest needs zero users and is recomputable by a judge; free public feed makes the ask a webhook, not a deposit. Vault TVL explicitly not claimed |
| Startup potential | 8 | Fee on protected TVL, insurance-adjacent, generalises to any chain with a sequencer |

**Where it is genuinely vulnerable, stated plainly:**

1. A determined team could approximate the engine by running their own simulation against Yellowstone `processed`. That is a cost and engineering barrier, not an impossibility.
2. The whole thing rests on people delegating withdrawal authority. Shadow Mode is the answer; it is not a complete one.
3. False positives move real money. The bank-run guard and published false-positive rate are the mitigation.

**Team:** do not enter this solo. Colosseum's average winning team is now above three, and every prior attempt at this exact idea — Tossbounty (Renaissance), SecureVault (Cypherpunk), plug (Frontier) — was one person with no technical demo. All three lost.

## 15. Ideas killed on evidence, and why

Recorded so they don't get re-proposed.

| Killed | Why |
|---|---|
| Protocol-side drain interrupter / auto-pause | Hypernative secures Solana; STRIDE funds free 24/7 monitoring above $10M TVL; SIRN coordinates response |
| Transaction "flight recorder" / inclusion forensics | Every claimed field is already in `geyser.proto` (`SLOT_FIRST_SHRED_RECEIVED`, `SLOT_COMPLETED`, `SLOT_DEAD` + `dead_error`, `SubscribeUpdateSlot.parent`, `SubscribeUpdateEntry.index`, `starting_transaction_index`). Blocknative's data APIs shut down 19 Jun 2026 |
| Priority-fee / CU copilot | Since 31 Aug 2026 equal-priority txs order by arrival time; leaders rank by `reward × 1e6 ÷ (cost + 1)` and charge on the CU *limit requested*, so the real lever is a static offline optimization. Helius ships fee estimation at 1 credit |
| Another real-time indexer / sub-second analytics | Cypherpunk Infrastructure was a sweep: Ionic, Pine Analytics, Hyperstack, Chaindex |
| Transaction debugger | Seer took 1st in Infrastructure at Cypherpunk |
| Shred-stream-as-trading-edge | Eight+ vendors; Frankfurt-only loses the colocation race on physics |
| Neutral provider referee / landing router | Nozomi already ships a landing dashboard; NTP clock skew (±1–10ms) is the same order as the signal being sold |
| Rug/honeypot scanner | Six+ in the corpus, zero wins |
| Inclusion markets / preconfirmation insurance | Self-defeating: once a tx is in a shred its inclusion is near-certain |
