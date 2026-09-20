# RIPCORD — Product Requirements

> What we build, for whom, and how we know it works.
> Companions: [IDEA.md](./IDEA.md) (why), [ARCHITECTURE.md](./ARCHITECTURE.md) (how), [RIPCORD.md](./RIPCORD.md) (frozen submission spec).

| | |
|---|---|
| **Deadline** | 12 Oct 2026, Colosseum Crypto World's Fair |
| **Entries** | Solana track + RPC Fast Superteam Infrastructure sidetrack + Superteam regionals |
| **Target network** | Solana **mainnet** (devnet is the published bar; mainnet is the differentiator) |
| **Assumed team** | 2–3. Do not enter solo — see §9 |
| **Status** | Committed. Blocked on one email, §8 |

---

## 1. Users

**Primary — the protected depositor.** Holds a position in a Solana lending market. Cannot watch Discord at 3am. Has been liquidated or drained before, or has read about someone who was.

*Cold-start cohort, named and enumerable:* the **1,895 unique wallets liquidated on Kamino in October 2025** (9,372 events, +309% month-over-month, $25.5M collateral seized — Kamino's own governance risk report). Publicly identifiable on-chain, provably harmed, reachable. Narrow to the 10 Oct window — SOL fell 14% in under an hour, $207 → $177 — using on-chain timestamps rather than the monthly aggregate.

**Secondary — the treasury / DAO operator.** Same problem, larger balance, an existing multisig, and a mandate that makes "we were asleep" an unacceptable answer.

**Explicit non-user — the protocol.** We do not sell to protocols, do not need their integration, and cannot be switched off by them. That is the entire positioning. See [IDEA.md](./IDEA.md).

## 2. Jobs to be done

| # | As a… | I want… | So that… |
|---|---|---|---|
| J1 | depositor | my position to exit automatically when a protocol shows a risk event | I don't lose funds while asleep |
| J2 | depositor | to see what the system *would* have done, before I trust it with money | I can evaluate it at zero risk |
| J3 | depositor | to revoke everything instantly | I am never locked in |
| J4 | depositor | proof of what fired and why | I can audit the system and dispute it |
| J5 | operator | to know the false-positive rate | I can price the risk of automation |

## 3. Scope

### In — MVP, nothing else ships

| ID | Requirement | Job |
|---|---|---|
| F1 | `ripcord_vault` Anchor program live on **mainnet** | J1, J3 |
| F2 | Guardian authority that is **withdraw-to-owner only** — cannot redirect, spend, or swap | J1, J3 |
| F3 | Owner revoke in a **single transaction**, no delay, no quorum | J3 |
| F4 | TxStream subscription with simulation enabled, filtered at **program level** | J1 |
| F5 | Three policy primitives: **authority/upgrade-authority change**, **oracle staleness or feed death**, **anomalous outflow rate** | J1 |
| F6 | One protocol adapter: **Kamino** | J1 |
| F7 | Exit lands in **slot N+1** relative to the trigger, proven by signature | J1 |
| F8 | **Shadow Mode** — arm any policy with zero delegation and zero deposit; system reports what it would have done | J2 |
| F9 | **Proof-of-Exit** on-chain receipt: trigger sig + slot, simulated deltas that fired, exit sig + slot | J4, J5 |
| F10 | Single screen: position, armed policies, live trigger log, kill switch | J1–J4 |
| F11 | Escalating exit ladder: repay → partial de-risk → full withdraw | J1 |
| F12 | Rate limits (per user per epoch) and a global per-protocol trigger budget | J5 |
| F13 | **Panic-withdrawal study** — quantified, cited evidence that depositors already attempt this manually and too slowly | V1 |

### Out — stretch, clearly separated, does not block submission

Multi-protocol adapters (Drift, marginfi, Save); health-factor policies; policy marketplace; insurance priced on protected TVL; cross-ecosystem deployment; mobile.

### Explicitly never

A shred pipeline. An indexer. An analytics dashboard. A transaction debugger. A fee optimizer. A protocol-side pause button. Anything whose pitch contains the word "milliseconds." Every one of these is in the graveyard in [IDEA.md](./IDEA.md) with the evidence that killed it.

## 4. Acceptance criteria

Binary, each verifiable by one probe. Nothing ships marked done without the evidence.

| ID | Criterion | Probe |
|---|---|---|
| A1 | `ripcord_vault` deployed to mainnet | program ID resolves on Solscan |
| A2 | Guardian cannot move funds to any address but the owner | negative test: signed exit to a third-party destination **fails** |
| A3 | Revoke completes in one transaction | signature + subsequent exit attempt fails |
| A4 | TxStream subscription receives simulated deltas | logged `SimulationConfig` payload with non-empty token balance deltas |
| A5 | `PARTIAL` ALT resolution never triggers a policy | synthetic `PARTIAL` input → refusal logged, zero exits |
| A6 | Each of the three policies fires on a crafted trigger | three trigger signatures, three exit signatures |
| A7 | Median trigger-to-exit delta ≤ 1 slot | measured over ≥20 runs |
| A8 | Exit is idempotent under fork reversal | replayed trigger produces no second withdrawal |
| A9 | Shadow Mode produces a report with zero on-chain authority granted | screenshot + absence of a delegation account |
| A10 | Proof-of-Exit receipt is readable on-chain and links both signatures | Solscan, by a third party |
| A11 | Rate limit blocks a second exit inside the same epoch | negative test |
| A12 | False-positive rate published against replayed history | committed report file in repo |
| A13 | Five real users contacted, conversations documented | transcript on the submission page |
| A14 | Pitch video **under 3 minutes**; separate technical demo explaining the Solana integration | two URLs on the submission |
| A15 | Panic-withdrawal study published with a citable headline number | committed report + source URLs |
| A16 | Founder-market-fit paragraph on the submission page | submission page text |

**Anti-criteria — what must NOT happen:**

- **N1** RIPCORD must never move funds to an address other than the vault owner.
- **N2** RIPCORD must never act on a transaction whose `alt_resolution` is `PARTIAL` or absent.
- **N3** RIPCORD must never exceed the per-protocol trigger budget in a single epoch, regardless of signal volume.
- **N4** No metric on the submission may be simulated, extrapolated or staged without being labelled as such out loud in the demo.

## 5. Metrics

**Primary — achievable in three weeks, verifiable by a judge, requiring zero user trust:**

1. **Backtest coverage** — number of real historical Solana incidents our policies fire on, median lead time before loss completion, and dollars of depositor funds that were in scope. Computed from public on-chain data; independently recomputable.
2. **Feed consumers** — wallets and protocols subscribed to the free public detection stream. The ask is a webhook, not a deposit.
3. **Median trigger-to-exit slot delta** — proven on mainnet with our own capital.

**Secondary:** dollars under armed policy. **Explicitly not the headline metric.** Nobody deposits meaningful money into an unaudited three-week-old vault, and claiming otherwise invites a judge to ask how much — to which the honest answer is embarrassing. We report it if it is real and we do not lead with it.

**Secondary:** Shadow Mode users, trigger count, published false-positive rate, exits that preserved a position without full withdrawal.

**Vanity metrics we will not report:** page views, waitlist signups, Discord members.

## 6. The three risks and their mitigations

| Risk | Mitigation | Requirement |
|---|---|---|
| False positives move real money and RIPCORD becomes the bank run | Escalating ladder, rate limits, per-protocol budget, owner kill switch, published FP rate | F11, F12, A12 |
| Nobody delegates withdrawal authority to a stranger | Withdraw-to-owner-only guardian as the first slide; Shadow Mode requires no delegation at all | F2, F8 |
| "Why isn't Geyser at `processed` enough?" | Never claim latency. The answer is ALT resolution + edge simulation. Rehearse until reflexive | see [ARCHITECTURE.md §2](./ARCHITECTURE.md) |

## 7. Milestones

| Days | Deliverable | Gates |
|---|---|---|
| 1–2 | `ripcord_vault` full instruction set, devnet → mainnet | A1, A2, A3 |
| 3–4 | TxStream client, program filter, simulated-delta parser, `PARTIAL` refusal | A4, A5 |
| 5–7 | Kamino adapter, executor, idempotent fork-safe retry | A6, A7, A8 |
| 8–10 | Shadow Mode + the one screen | A9 |
| 11–14 | Kamino cohort outreach; RPC Fast writeup; Proof-of-Exit live | A10, A13 |
| 15–18 | Test protocol deployed; live trigger rehearsed until boring | A11, A12 |
| 19–21 | Videos, repo access, every optional field filled, submit to all tracks | A14 |

## 8. Blocker — resolve today

The Colosseum offer is the **Focus** plan at $45/mo, which carries **no gRPC streams at all**. Yellowstone starts at **Stream ($249)**. **TxStream is Aperture-tier only ($499)** — the sidetrack's own top prize is one month of Aperture, which confirms where it sits.

RPC Fast's published pricing contradicts itself in two places (homepage lists TxStream under Stream; their blog says Aperture is "from $399"), so the entitlement must be **confirmed in writing**, not inferred. Email draft is in [RIPCORD.md §10](./RIPCORD.md).

**If Aperture access is refused, the project as specified does not run.** Fallback is not "use Yellowstone" — that removes the product. Fallback is a different sponsor tier negotiation or a different project.

## 8b. Validation — evidence the thesis exists outside our heads

Study of Colosseum winners shows every strong submission carried at least one of four proofs. Ours, ranked by how cheap they are to obtain:

| Proof type | What we have | Cost to get |
|---|---|---|
| **Market proof** | Hypernative, Hexagate, Range and Riverguard are funded businesses; the Solana Foundation funds STRIDE and SIRN outright. Capital and demand for automated on-chain defence provably exist | Already have it |
| **Comparable proof** | BlockSec couples monitoring to automated on-chain pausing on EVM. The mechanism is proven; nobody has pointed it at the user | Already have it |
| **Behavioural proof** | **Depositors already do this by hand, badly and too late.** During every major incident, TVL leaves within hours as users manually withdraw. That behaviour is public on-chain data and can be quantified — see F13 | ~1 day of analysis |
| **Traction proof** | Shadow Mode users + the Kamino cohort outreach (A13) | 2 weeks |

**Behavioural proof is the highest-leverage item on this list.** "N wallets tried to do manually, and too slowly, exactly what RIPCORD does automatically" is a stronger submission-page line than any architecture diagram, and it costs a day.

Measurement approach: DefiLlama per-protocol TVL timeseries around each incident window, cross-checked against withdraw-instruction counts derived from the protocol program ID. Target incidents: the Drift exploit, Kamino's 10 Oct 2025 liquidation hour, and the marginfi withdrawal crisis.

## 9. Team

### Founder-market fit — the accelerator gate

Winning submissions answer *"why should we trust you with capital"* and *"why are you the best team to build this"* explicitly, in the pitch, in one paragraph. Observed examples from prior champions: *"we previously exited our computer vision company and scaled a DeFi app to $4B and 100 MAUs"*; *"our cofounder has a PhD in computational astrophysics, was previously a researcher at ImmuneFi, KeeperDAO, Ethereum."*

**This paragraph is currently unwritten and it is the largest open risk to accelerator qualification.** It cannot be fabricated and should not be written around. If the honest version is thin, the correct response is to recruit a third teammate with a security, DeFi-risk or infrastructure track record — which also fixes the team-size problem below.



Colosseum's average winning team is now above three. Every prior attempt at this exact idea — Tossbounty (Renaissance), SecureVault (Cypherpunk), plug (Frontier) — was **one person with no technical demo**, and all three lost.

Minimum viable split: one Anchor/Rust engineer on the program and executor, one Rust engineer on the TxStream client and policy engine, one on frontend plus user outreach. Outreach is not optional — it is the criterion Colosseum weights hardest.

## 10. Submission checklist

- [ ] Aperture entitlement confirmed in writing
- [ ] Program deployed to mainnet, ID public
- [ ] Repo access granted to judges
- [ ] Pitch video < 3:00
- [ ] Separate technical demo explaining the Solana integration
- [ ] Five user conversations documented on the submission page
- [ ] False-positive report committed
- [ ] 2–3 RPC Fast posts published (sidetrack requirement)
- [ ] Entered: Solana track, RPC Fast sidetrack, Superteam regionals
- [ ] Commits visible after the deadline — judges ask about post-submission progress
