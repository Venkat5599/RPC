# RIPCORD — one-page brief

> **FINAL. Locked 19 Sep 2026.** Colosseum Crypto World's Fair, submissions close **12 Oct 2026**.
> Detail: [IDEA.md](./IDEA.md) · [PRD.md](./PRD.md) · [ARCHITECTURE.md](./ARCHITECTURE.md) · [RIPCORD.md](./RIPCORD.md) · [Panta Sidetrack/](./Panta%20Sidetrack/)

---

## What it is

**The first stop-loss that triggers on intent, not price.**

Simple version: **an airbag for your money.**

Seven years of DeFi automation — DeFi Saver since 2019, Summer.fi, every Solana trading bot — fires on **price**. Price is a lagging signal, so every automated defence in DeFi fires *after* the damage. RIPCORD fires on **the transaction that causes the damage, before it executes**, read from a pre-execution stream with its effects already simulated.

## Why it exists

Every security product on Solana protects **the protocol**. Hypernative, Hexagate, Range, Riverguard; the Foundation funds STRIDE (free 24/7 monitoring above $10M TVL) and SIRN. Not one of them moves a **depositor's** funds.

- **marginfi, 11 Apr 2024** — CEO resigned. No hack, no exploit, no funds at risk, just news. Depositors pulled **>$130M in 24 hours**, TVL −25%. Humans manually racing for the exit. That race is what we automate.
- **Drift, 1 Apr 2026** — ~$285M over ~2.5 hours. Depositors never got to run; withdrawals were paused.
- **Kamino, Oct 2025** — 9,372 liquidations, 1,895 wallets, $25.5M seized.

## How it works

1. Your funds sit in a PDA vault **you own**.
2. You set a policy once — *"if this protocol's upgrade authority changes, get me out."*
3. An off-chain engine watches RPC Fast **TxStream**: decoded, ALT-resolved, **pre-simulated** pending transactions.
4. On a match it submits **evidence**, and the program **re-evaluates the policy on-chain**. False predicate → nothing moves.
5. Exit lands in **slot N+1**, into **your** wallet. Revocable in one transaction.

## The two things that make it defensible

**Sponsor lock-in is structural.** ~⅔ of a Kamino transaction's addresses arrive via lookup tables, so a feed without ALT resolution is blind to the traffic we watch; `include_simulation` returns the token and account deltas that *are* the policy input. Remove RPC Fast and the product stops — it does not degrade. And we never claim latency: simulation costs 791µs against a 284µs lead, deliberately, because **our opponent is a human incident-response loop, not a colocated bot.**

**The keeper cannot lie.** `execute_exit` re-evaluates the predicate on-chain. Authority-change and oracle-staleness are **fully trustless**; outflow-rate is documented as weaker rather than hidden. A compromised keeper can neither steal (destination locked to owner) nor fire spuriously. **This is the answer to Summer.fi losing $6M through automated vaults in July, and it leads the pitch.**

## Traction that needs no trust

1. **Backtest** — policies replayed across Solana's last 12 months: incidents matched, median lead time before loss completion, dollars in scope. Public data, judge-recomputable, **no Aperture dependency — day 1 work**.
2. **Public detection feed** — free, no deposit, no delegation. The ask is a webhook. A *feed, not a dashboard*.
3. **Mainnet vault** with our own capital. **TVL is explicitly not the headline metric.**

## Demo

Backtest → a live position → the pre-execution panel → fire a real authority change on our own mainnet protocol (said out loud as ours) → **the exit confirms while the trigger's own Solscan tab is still empty.** Judge verifies both signatures on their phone.

## The sidetracks — one submission, three entries

Colosseum allows **one product per team**; sidetracks stack. RIPCORD enters the **Solana track**, the **RPC Fast** sidetrack, and — via **PREMIUM**, weekly two-sided risk markets on Panta resolved by our own detections — the **Panta** sidetrack. PREMIUM is a *consequence* of the trigger primitive, not a co-equal half. Pitch the trigger first, always.

## Scores, honest

| | |
|---|---|
| Novelty | 7 — intent-triggered defence has no prior art on any chain; the components (DeFi Saver, Y2K) are mature. We cite them ourselves |
| Technical depth | 9 — on-chain trigger verification, exactly-once money movement under reorg, hot-path matcher forced by the 10-pubkey filter cap, source-agnostic evaluator |
| Real pain | 9 · Hackathon fit | 8 · Sponsor integration | 9 · Demo | 9 · Traction | 9 · Startup potential | 8 |

**Idea: 9/10.** Not 10 — novelty is capped by prior art that exists whether or not we acknowledge it, and the 10-on-depth version (a composable verifiable-predicate VM) is a scope trap three weeks out. **It ships as the stated roadmap, not as MVP.**

## Decisions locked

- Build the **9**, not the 10. Predicate VM is roadmap.
- Backtest before UI. Feed before vault. Vault before PREMIUM.
- PREMIUM only after RIPCORD's core demos cleanly.
- Never pitch milliseconds.

## Blockers — only you can clear these

1. **Email RPC Fast for Aperture access.** The Colosseum plan is Focus ($45/mo) with **no gRPC at all**; TxStream is Aperture-tier. Draft in [RIPCORD.md §10](./RIPCORD.md).
2. **Ask Panta's Discord who resolves a market.** Not in their public API.
3. **Team size and founder-market-fit paragraph.** Average winning team is above three; all three prior attempts at this idea were solo with no technical demo, and all three lost.
