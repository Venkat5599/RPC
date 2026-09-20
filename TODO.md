# TODO — solo build, 23 days to 12 Oct 2026

> Scoped for **one person**. This is not the 3-person plan in PRD.md — it is that plan with ~60% cut.
> Rule: **anything below the CUT LINE does not get built unless everything above it is done and demoing.**

---

## DAY 0 — today, ~45 minutes, zero code

- [ ] **Email RPC Fast for Aperture access.** Draft is in [RIPCORD.md §10](./RIPCORD.md). Colosseum's free plan is Focus ($45/mo) with **no gRPC at all**; TxStream is Aperture-tier. Nothing downstream works without this.
- [ ] **Ask Panta `#dev-chat` two questions:**
  1. *"Is the primary market parimutuel, a bonding curve, or dynamically priced with a secondary CLOB? Is my payout determined at purchase or at settlement?"*
  2. *"Can the resolution source be a published methodology + on-chain metric I define at creation?"*
- [ ] **Register for Colosseum Crypto World's Fair** if not already.
- [ ] **Write the founder-market-fit paragraph.** Two sentences: why you, why this. It is the clearest separator for accelerator selection and it cannot be written later under pressure.

---

# PROJECT A — RIPCORD (main submission + RPC Fast sidetrack)

## Phase 1 — Backtest (days 1–4) · NO EXTERNAL DEPENDENCY

Highest value per hour in the whole plan. Start here even if RPC Fast hasn't replied.

- [ ] Pick incident windows: Drift (1 Apr 2026, 16:05–18:31 UTC), Kamino (Oct 2025), marginfi (11–13 Apr 2024)
- [ ] Fetch historical txs: `getSignaturesForAddress` + `getTransaction` over bounded windows, paid RPC (public will rate-limit you out)
- [ ] Decode Anchor instruction discriminators — `sha256("global:<ix_name>")[0..8]`
- [ ] Implement **one** policy evaluator: **authority change** (simplest, and the one that is fully trustless on-chain later)
- [ ] Run it. Output: incidents matched · median lead time before loss completion · dollars in scope
- [ ] **Publish the report + the query** so a judge can recompute it
- [ ] Post the finding publicly — this is also your first traction artifact

**Done = a real number you can say out loud.** *"Across N incidents our policy fired a median of X minutes before the loss completed."*

## Phase 2 — Vault (days 5–9)

- [ ] `ripcord_vault` Anchor program: `deposit`, `set_policy`, `arm`, `execute_exit`, `revoke`, `emergency_owner_withdraw`
- [ ] **Destination hard-locked to owner.** Negative test: guardian-signed exit to a third party **must fail**
- [ ] **On-chain predicate for the authority-change policy** — program reads the target's upgrade authority and compares to the value stored at `arm`. This is the technical-depth differentiator; do not skip it
- [ ] Devnet → mainnet, program ID public
- [ ] Negative tests committed (they are the proof, not decoration)

## Phase 3 — Live engine (days 10–14) · NEEDS APERTURE

- [ ] TxStream gRPC client via `aperture-grpc-client` v0.6.1
- [ ] Program-level filter (**`account_include` caps at 10 pubkeys** — filter on programs, match users locally)
- [ ] Simulated-delta parser
- [ ] **Refuse to act on `alt_resolution = PARTIAL`** — log and alert
- [ ] Executor: submit evidence → on-chain verify → exit in slot N+1
- [ ] Idempotent under reorg — re-check against `confirmed` before retry
- [ ] **Public detection feed** — free endpoint + webhook. This is where traction comes from; the ask is a webhook, not a deposit

## Phase 4 — Demo + submit (days 15–21)

- [ ] Deploy a throwaway protocol on mainnet to trigger against
- [ ] One screen: position · armed policy · trigger log · kill switch
- [ ] Rehearse the live trigger until boring. **The moment: exit confirms while the trigger's own Solscan tab is still empty**
- [ ] Pitch video **under 3:00** — problem, who it's for, traction, working demo
- [ ] Separate technical demo — architecture + the Solana/RPC Fast integration specifically
- [ ] 2–3 RPC Fast posts (sidetrack requirement)
- [ ] Submit: Colosseum + Solana track + RPC Fast sidetrack + Superteam regionals
- [ ] **Keep committing after submission** — judges ask about post-deadline progress

---

# PROJECT B — PREMIUM (Panta sidetrack)

**Solo reality: build the factory only.** It is a cron plus REST calls, it runs unattended, and its value compounds daily. Everything else is cut.

## Phase 1 — Market factory (days 5–7, alongside vault work)

- [ ] Panta auth: `POST /auth/register/`, `POST /auth/token/`
- [ ] Market factory: quote fee → build unsigned tx → sign → `POST /register`
- [ ] **Idempotent per `(protocol, epoch)`** — deterministic slug, creating twice is a no-op
- [ ] **One** question type: *"Will Kamino liquidate > $2M collateral between slot A and slot B?"*
- [ ] Resolution source = published methodology + metric (set at creation)
- [ ] Publish methodology **before** the first epoch opens
- [ ] Fund the wallet for **all** planned epochs now — running out of USDC in week three is the dumbest possible failure
- [ ] **External watchdog + alert to your phone.** Separate process. A watchdog inside the thing it watches is decoration
- [ ] Seed each market with your own USDC, publicly labelled as yours

**Then leave it running.** Every day it runs is another resolved epoch on your submission page.

## Phase 2 — Minimum UI (days 18–20, only if RIPCORD is demoing)

- [ ] One-click **underwrite (NO)** — the yield side, the easier sell, where volume comes from
- [ ] Position + claim view
- [ ] Attribution wired from the **first** trade — retrofitting loses the history
- [ ] Submit to Panta Sidetrack on **Superteam Earn** (separate from the Colosseum submission)

---

# ⛔ CUT LINE — do not build these

- [ ] ~~Three policies~~ → **one** (authority change). Oracle-staleness and outflow-rate are roadmap
- [ ] ~~Drift adapter~~ → Kamino only
- [ ] ~~Shadow Mode~~ → the backtest already covers "what would it have done", for free
- [ ] ~~Proof-of-Exit receipts~~ → the trigger log is enough for a demo
- [ ] ~~Risk Market Factory breadth~~ (utilization, oracle deviation, bad debt) → one question type
- [ ] ~~Position-graph UI~~ → roadmap slide, not code
- [ ] ~~One-click hedge (YES)~~ → underwrite side only if time is tight
- [ ] ~~Predicate VM~~ → roadmap. Named in the pitch, not built
- [ ] ~~Multi-region anything~~
- [ ] ~~Mobile~~

---

# Weekly checkpoints — be honest at each

| Date | Must be true, or cut deeper |
|---|---|
| **26 Sep** | Backtest number exists and is published. Aperture answered |
| **3 Oct** | Vault on mainnet with the on-chain predicate. Factory running unattended for a week |
| **8 Oct** | Live trigger works end-to-end on mainnet. Videos started |
| **12 Oct** | Submitted to all tracks |

**If 3 Oct arrives and the vault is not on mainnet: drop PREMIUM entirely, drop the live engine, and submit the backtest plus the feed as the product.** That is still a real, honest, defensible submission. A broken vault is not.
