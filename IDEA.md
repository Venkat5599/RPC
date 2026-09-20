# RIPCORD — the idea

> Why this and not something else. Positioning, thesis, and the graveyard of what we rejected.
> Companion docs: [PRD.md](./PRD.md) (what we build), [ARCHITECTURE.md](./ARCHITECTURE.md) (how), [RIPCORD.md](./RIPCORD.md) (frozen submission spec).

---

## The novel primitive — lead with this

# The first stop-loss that triggers on intent, not price.

**Seven years of DeFi automation — DeFi Saver since 2019, Summer.fi, Instadapp, every trading bot on Solana — triggers on one thing: price.** A collateral ratio. A stop-loss level. A liquidation threshold.

Price is a *lagging* signal. By the time price has moved, the transaction that moved it has already executed. **Every automated defence in DeFi fires after the damage.**

RIPCORD triggers on **the transaction itself, before it executes** — read from a pre-execution stream, with its effects already simulated. Not "SOL fell 14%, get me out." Instead:

- *"Someone is changing this protocol's upgrade authority — get me out."* Price has not moved yet.
- *"This pending transaction would remove 5% of the protocol's reserves — get me out."* Price has not moved yet.
- *"The oracle my position depends on has stopped updating — get me out."* Price cannot move; that is the problem.
- *"This transaction would push my health factor below 1.1 — repay before it lands."* Not after.

**That is a new class of trigger.** Price-triggered defence is a mature, seven-year-old category with a market leader. **Intent-triggered defence has no prior art we could find, on any chain.**

And it only became buildable this year, because a decoded, ALT-resolved, *pre-simulated* transaction stream became something you can buy rather than something you must be a validator to produce.

### The general form

Once a user can write a policy over **pending, simulated state** instead of over price, the primitive stops being a feature and becomes a surface. Any condition expressible over "what is about to happen to accounts I care about" becomes an automated defence. Three policies ship in the MVP; the primitive admits arbitrarily many.

**Programmable, user-owned defence over pre-execution state.** That is the invention. Everything else in this document is a consequence of it.

## The traction fix — the engine is useful before anyone trusts us

**The problem with every security product built in three weeks: nobody deposits real money into an unaudited vault from an unknown team.** So the vault, on its own, produces zero traction at judging no matter how good the engineering is.

The fix is that **the trigger engine is valuable with zero deposits and zero delegation.** Three artifacts, none of which require a user to trust us with a lamport:

### 1. The backtest — available day 5, not day 21

Point the policy engine at **Solana's history**. Every authority change, every oracle death, every anomalous outflow across the last twelve months, evaluated against the policies we ship.

Output: *"Across N real incidents, our policies fired a median of X minutes before the loss completed, covering $Y of depositor funds."*

That number is computed from public on-chain data, is independently recomputable by any judge, requires no users, and exists **before we have written the UI**. It is the single most persuasive artifact in the entire submission, and it is also exactly how you would validate a defence system in the real world — you backtest it.

### 2. The public feed — zero trust, real users on day one

The live engine publishes every detection publicly and for free. Anyone can watch it. Anyone can subscribe. No deposit, no delegation, no signature.

Traction becomes *"N wallets and protocols consuming the feed"* — a number achievable in three weeks because the ask is a webhook, not a bank transfer.

**Scoping discipline:** this is a *feed*, not a dashboard. The graveyard in this document is full of Solana analytics dashboards that won nothing. What we publish is a detection stream with a backtested hit rate, not charts.

### 3. The vault — demonstrated on mainnet with our own money

The vault ships and works on mainnet, proven with our own capital. It is the monetization path and the demo's climax. It is **not** the traction metric, and we stop pretending it could be in three weeks.

**This ordering is also honest about what a hackathon can prove.** We prove the trigger works (backtest), that people want it (feed subscribers), and that acting on it works end-to-end (mainnet vault). What we do not claim is TVL we could not have earned.

## The simple mental model on top

# An airbag for your money.

That is the pitch. Not "a non-custodial vault with a pre-execution policy engine" — that is the machinery, and the machinery goes in [ARCHITECTURE.md](./ARCHITECTURE.md).

Everyone understands an airbag in under a second: it sits there doing nothing, it costs you nothing to carry, and in the one moment that matters it deploys faster than you can react. You do not need to understand the crash sensor to want one.

Supporting line, once they've got it: **protocols got bodyguards in April — depositors are still reading Discord.**

## The structural constraint we remove

DeFi's entire risk model assumes the depositor is a **passive holder who cannot react at machine speed**. Everything downstream is shaped by that assumption: you cannot buy a guaranteed maximum loss on a position, insurance has to pay out after the fact instead of preventing the loss, leverage has to be conservative because the tail is unmanaged, and institutional capital stays out because "we were asleep" is not an acceptable risk control.

That is not a cosmetic gap. It is a constraint that prevents an entire category of product from existing.

**If a depositor can be given a reliable, automated exit, a whole class of things becomes possible:** underwritable DeFi insurance priced on actual prevented loss rather than actuarial guesswork, safe leverage with a real floor, mandate-compliant institutional allocation, and risk policies that are portable across protocols instead of re-implemented by each one.

We are not building a 20%-better monitoring tool. We are removing the assumption that the user cannot act.

## Why Solana specifically

On Ethereum this product is easier and therefore already commoditised — a public mempool means anyone can see pending transactions, and BlockSec-style automated pausing exists.

Solana is the chain where it is simultaneously **necessary and possible**:

- **Necessary** — no public mempool, so there is no way for a user to see what is coming. The only view into a block being built is the shred layer, which no consumer product touches.
- **Possible** — 350ms slots (400ms until 22 Aug 2026, heading to 200ms) mean "react in the next block" is a real defence rather than a rounding error. On a 12-second chain, next-block is not fast enough to matter; on a 3-second chain there is no leader-side stream to read.

Without Solana's block cadence this product does not work. Without Solana's missing mempool, nobody needed to build it this way.

## Why the ecosystem is better off if we win

- **Capital that currently will not enter Solana DeFi becomes allocatable.** The blocker for treasuries and funds is not yield, it is the absence of an automated downside control. A guaranteed exit is a risk control a mandate can actually reference.
- **It completes the security story the Foundation started.** STRIDE covers protocols above $10M TVL. That leaves the entire long tail, and it leaves every depositor, uncovered. We are the other half, and we require no protocol to integrate anything.
- **It makes leverage safer without making it smaller**, which is the thing that actually grows lending and perp volume.
- **Proof-of-Exit receipts become public risk data** — a shared, auditable record of what actually threatened Solana DeFi and when, which today exists only inside private vendor dashboards.

## The enormous vision behind the narrow MVP

The MVP is one protocol and three policies. The vision is **the risk layer for DeFi**: user-owned, programmable, portable risk policies that travel with capital across protocols and eventually across chains, with the prevented-loss record underwriting a real insurance market on top.

Today every protocol re-implements risk in isolation and every user bears the tail alone. RIPCORD is the layer where that stops being true.

## The asymmetry nobody is exploiting

Every security product on Solana sells to the protocol. Not one of them moves a depositor's funds.

- **Hypernative, Chainalysis Hexagate, Range, Neodyme Riverguard** — detection sold to protocols; output is an alert and a webhook.
- **STRIDE** (Solana Foundation, from 7 Apr 2026) — free 24/7 monitoring above $10M TVL, funded formal verification above $100M.
- **SIRN** — Asymmetric Research, OtterSec, Neodyme, Squads, Zeroshadow coordinating incident response.

That entire stack is protocol-side **by business model, sales motion and legal posture**. Moving retail funds automatically is a different product with a different liability surface and a different distribution channel. Incumbents are not structurally able to pivot into it quickly, and they have no incentive to.

The depositor's toolkit in 2026 is a Discord notification and their own thumbs.

## What it costs, in numbers we can cite

| Event | Damage | What the depositor had |
|---|---|---|
| **marginfi, 11 Apr 2024** | CEO resigned. **No hack, no exploit, no funds at risk — just news.** Depositors pulled **>$130M in 24 hours**, TVL **−25%** | Their own thumbs |
| Drift exploit, 1 Apr 2026 | ~$285M across 18 tokens. First malicious tx **16:05:18 UTC**, last drain **18:31 UTC** — a **~2.5 hour** window. Insurance fund ~$20M | Nothing — withdrawals were paused |
| Kamino, Oct 2025 | **9,372 liquidation events (+309% MoM), 1,895 unique wallets, $25.5M collateral seized.** SOL fell 14% in under an hour, $207 → $177 | Nothing |
| Switchboard shutdown | 550+ feeds, six days' notice, Kamino/Jito/marginfi/Drift named | Nothing automated |

Kamino's own position on the premature liquidations: *"TWAP is meant to protect the platform from bad debt, not to protect users from premature liquidation."* That sentence is the product brief.

Forty-seven minutes is an eternity for an automated system and a coin-flip for a human. **Our opponent is a human incident-response loop, not a colocated bot.** That single fact determines everything below.

## Proof the thesis exists outside our heads

Colosseum winners all carry at least one of four proofs. We have all four, and the behavioural one is the strongest thing in this document.

### Behavioural proof — people already do this by hand, and it is measurable

> **On 11 April 2024, marginfi's CEO resigned. No hack, no exploit, no funds at risk — just news. Depositors pulled over $130 million out of the protocol in the next 24 hours, cutting TVL by 25%.**

The trigger was **information**, and hundreds of humans responded by manually racing for the exit, at whatever hour it reached them. That race is exactly what RIPCORD automates. marginfi was still bleeding ~$43M in net withdrawals over the last 30 days as of this writing.

Sources: [Unchained](https://unchainedcrypto.com/130-million-withdrawn-from-marginfi-as-ceo-departs/) · [CoinDesk](https://www.coindesk.com/tech/2024/04/11/marginfi-leader-resigns-on-fiery-day-for-major-solana-lender) · [BeInCrypto](https://beincrypto.com/solana-defi-tvl-decreased/)

### The Drift case, framed correctly

Drift's exploit ran from the first malicious transaction at **16:05:18 UTC** to the last drain at **18:31 UTC** on 1 Apr 2026 — ~2.5 hours, ~$285M across 18 tokens, funds bridged off Solana within 23 minutes.

**Drift then paused deposits and withdrawals. Its depositors never got the chance to run at all.** TVL still bled a further ~9.5% over the following six days, as people left the moment they could.

This is a sharper story than "users were too slow" — the exit door was welded shut. It also names our honest limit, which we say before a judge does: **if a protocol pauses withdrawals, RIPCORD cannot withdraw either. Our window is between the first malicious transaction and the pause.** On Drift that window was hours wide.

### Academic proof — and it is about latency falling on retail

NBER working paper **w31160, *Anatomy of a Run: The Terra Luna Crash*** (Jiageng Liu), using wallet-level Anchor withdrawal data, found that **sophisticated and wealthy wallets ran first, while poorer and less sophisticated wallets ran later and took larger losses.**

That is a measured latency penalty paid by ordinary users. It is the inequality RIPCORD removes: a policy does not sleep, and it does not care how sophisticated its owner is.

[NBER w31160](https://www.nber.org/system/files/working_papers/w31160/w31160.pdf) · see also Saengchote on the [Iron Finance / TITAN run](https://papers.ssrn.com/sol3/papers.cfm?abstract_id=3888089)

### Market and comparable proof

Hypernative, Chainalysis Hexagate, Range and Neodyme Riverguard are funded businesses, and the Solana Foundation funds STRIDE and SIRN outright — capital and demand for automated on-chain defence provably exist. BlockSec already couples monitoring to automated on-chain pausing on EVM, so the mechanism is proven. **Nobody has pointed any of it at the user.**

### How we produce our own number

Flipside `fact_decoded_instructions`, marginfi program `MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA`, 11–13 Apr 2024, `count(distinct tx_signer)` bucketed hourly, filtered to withdraw. That histogram — the shape of a manual bank run, hour by hour — is the single best slide we can put on the submission page, and it costs a day. Note that DefiLlama's free TVL series is **daily only**, so it cannot show a minute-scale run; the on-chain query is required.

## Prior art, stated by us before a judge states it

Know your landscape and say so. A team that cites its own prior art is trusted; a team whose novelty claim dies on the first search is not.

| What exists | Who | What it proves for us |
|---|---|---|
| Automated position management for users | **DeFi Saver**, since 2019 (as CDP Saver, pre-Aave-v1). Stop Loss, Take Profit, trailing stop-loss, Boost/Repay | **Users want this.** Seven years and a market leader validates the demand. All of it is **price-triggered** |
| Same, now consolidated | **Summer.fi Pro** — shut down, users migrated to DeFi Saver | Category consolidation, not category absence |
| Trading-side TP/SL on Solana | Mizar, Banana Gun | Solana users already automate. Nobody automates *defence* |
| Two-sided epoch insurance | **Y2K Finance "Earthquake"** on Arbitrum — Hedge and Risk vaults, epochs, catastrophe-bond structure, Chainlink oracle + Keepers | The insurance structure works. Theirs is for **depegs**, oracle-resolved. Ours is for protocol risk, resolved by a recomputable on-chain metric |
| Protocol-side threat detection | Hypernative, Hexagate, Range, Riverguard; Foundation-funded STRIDE and SIRN | Detection is solved and subsidised — **for protocols**. Nobody moves the user's funds |

**What none of them do: trigger on a pending transaction's simulated effect.** That is the gap, and it is the whole product.

**The honest composition, and we say it in the pitch:** *DeFi Saver proved users want automated position management. Y2K proved two-sided epoch insurance works. Nobody has pointed either at security events, on Solana, using pre-execution data.*

### The question we will be asked, and must answer first

On **6 July 2026 Summer.fi was exploited for roughly $6 million — through its automated vaults.** The post-mortem's conclusion: delegated DeFi automation "depends on layered contract and keeper controls."

So: *"your automation vault is itself an attack surface, and the last one got drained two months ago."*

**Our answer, and it must lead rather than defend:** the guardian key has exactly one verb. It can only move funds **to the owner**. It cannot redirect, cannot swap, cannot spend, cannot choose a destination. Fully compromised, an attacker's only available action is sending your money back to you. Most automation vaults hold broad execution authority — that is why they are worth attacking. Ours is worth nothing to steal.

## The technical thesis

Every other team reaching for RPC Fast's stack will sell milliseconds. They will lose.

- Measured shred advantage over Yellowstone at `processed`: **~5–12ms p50**, 20–45ms p99.
- RPC Fast's own benchmarks: simulation costs **791µs** against a **284µs** median lead. TxStream-with-simulation is *slower* than a raw shred feed.
- RPC Fast is **Frankfurt-only**. A latency product from a single European vantage loses a colocation race against desks in Amsterdam and New York on physics alone.

So a latency pitch is disqualifying here. But that same 791µs buys something a raw feed never provides:

> **TxStream's real gift is that somebody else already simulated every pending transaction.**

Doing that yourself means running a simulation cluster against live account state for all of Solana. That is an economic and engineering dependency, and unlike a timing claim it survives a hostile engineer.

**Say it in the pitch, verbatim:** *we deliberately give up the milliseconds to buy the outcome, because our opponent is an incident-response loop, not a colocated bot.*

## Why the sponsor integration is structural

Two fields, both verified in `aperture-grpc-client` v0.6.1 (published 2026-09-17):

1. `account_include` matched against **static *and* ALT-loaded** accounts. Roughly **two-thirds of a Kamino transaction's addresses arrive via lookup tables** — a static-key filter is structurally blind to the traffic we need.
2. `include_simulation` → **token balance deltas and account deltas**. These are literally the policy input.

**Remove RPC Fast and the product stops, it does not degrade.** Yellowstone hands you the transaction. It does not hand you its predicted effect.

## Why hackathon judges remember it

Colosseum's published bar: *"Make your product work, usable and appealing — in that order."* Devnet is the requirement; mainnet is a differentiator. Judges evaluate each submission as a potential company. Early traction defined as **real user conversations** is the stated strongest differentiator. Average winning team size is now above three.

Grand champions are consistently systems, not screens — Reflect, TapeDrive (a storage network), Unruggable (actual hardware), CrowdBrain (robotics). Accelerator Cohort 5 intake was 14 of 21 infra/fintech.

RIPCORD is infra-shaped, runs on mainnet, and its demo is verifiable on the judge's own phone in 90 seconds: a defensive transaction confirming against a transaction that, on every explorer on earth, has not happened yet.

## Why it is a company, not a hackathon toy

- **Revenue**: fee on protected TVL. Insurance-adjacent — Proof-of-Exit receipts are the underwriting data.
- **Moat compounds**: every trigger fired and every false positive published builds a public track record that a new entrant cannot fabricate.
- **The primitive generalises** to any chain with a sequencer, which is the cross-ecosystem story for a nine-track World's Fair.
- **Nobody is in the seat.** Three prior hackathon attempts at user-side protection — Tossbounty (Renaissance), SecureVault (Cypherpunk), plug (Frontier) — were all **solo builders with no technical demo**, and all three lost. The problem is real and recurring; the missing piece was always the decision layer, which did not exist as a purchasable input until TxStream.

## Where it is genuinely vulnerable

Stated plainly so nobody is surprised in Q&A.

1. **Approximable.** A determined team could run their own simulation against Yellowstone `processed`. That is a cost and engineering barrier, not an impossibility.
2. **Trust.** The whole thing rests on people delegating withdrawal authority. Shadow Mode is the answer; it is not a complete one.
3. **False positives move real money.** A system that automatically empties a protocol on a bad signal *is* the risk. Named, guarded, and published rather than hidden.

## The graveyard — killed on evidence

Recorded so they don't get re-proposed mid-build.

| Killed | Why |
|---|---|
| Protocol-side drain interrupter / auto-pause | Hypernative secures Solana; STRIDE funds it free above $10M TVL; SIRN coordinates response |
| Transaction "flight recorder" / inclusion forensics | Every claimed field already exists in `geyser.proto` — `SLOT_FIRST_SHRED_RECEIVED`, `SLOT_COMPLETED`, `SLOT_DEAD` + `dead_error`, `SubscribeUpdateSlot.parent`, `SubscribeUpdateEntry.index`, `starting_transaction_index`. Blocknative's data APIs shut down 19 Jun 2026 |
| Priority-fee / CU copilot | Since 31 Aug 2026 equal-priority txs order by **arrival time**; leaders rank by `reward × 1e6 ÷ (cost + 1)` and charge on the CU *limit requested*, so the real lever is a static offline optimization. Helius ships fee estimation at 1 credit |
| Another real-time indexer / sub-second analytics | Cypherpunk Infrastructure was a sweep: Ionic (3rd), Pine Analytics (4th), Hyperstack (5th), Chaindex (HM) |
| Transaction debugger | Seer took 1st in Infrastructure at Cypherpunk |
| Shred-stream-as-trading-edge | Eight+ vendors sell it; Frankfurt-only loses the colocation race |
| Neutral provider referee / landing router | Nozomi already ships a landing dashboard; NTP clock skew (±1–10ms) is the same order as the signal being sold |
| Rug/honeypot scanner | Six+ in the corpus, zero wins |
| Inclusion markets / preconfirmation insurance | Self-defeating — once a tx is in a shred its inclusion is near-certain, so there is no uncertainty left to price |
| Validator integrity / propagation network | Technically the strongest runner-up. Killed on demo quality and a ~1,500-validator buyer, not on merit |

## The sentence a judge should be able to repeat

*"That was the one where the money left the burning building by itself."*
