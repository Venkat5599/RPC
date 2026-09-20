# The demo, run on devnet

> Every signature below is real and on devnet. Open them. Nothing here is a mock, a diagram, or a
> screenshot of something that happened locally.
>
> Run: 20 September 2026. Program: `84h2juN2WaFhZtGbaqmeWVVe7HQCUXnDRRtbGZQeAeTu`.

The claim this run exists to test is not "the vault works". It is the narrower, harder one:

**A correctly-signed exit, from the correct guardian, aimed at the correct owner, fails — because
the chain checked the condition itself and disagreed.**

That is the difference between RIPCORD and every automation vault that has lost money. The
guardian is not trusted to be right. It is not trusted at all.

## The run

| # | Step | Result | Signature |
|---|---|---|---|
| 1 | Create vault | `DeDSGexktmbutBAjKSYedwMqngp5XZ4xyxUuNgkFxHHd` | [`4qxSLsx8…`](https://solscan.io/tx/4qxSLsx84MVMWmHERDMDhDU8ZxhP4QpX2VzDzDsSriYV8g1eeSExvzgYfYSUQkPerBqZ7QqzQP69HEQSSampsUnC?cluster=devnet) |
| 2 | Deposit 0.2 SOL | vault holds 200,939,800 lamports | [`3aPWngDa…`](https://solscan.io/tx/3aPWngDacuDNcBsEie38nPp44sfWFZGsizNoeLT3eK9rteKj57b4WjnNmVvdfAxHdjB6W9YCThyFCAMteoe1rMR6?cluster=devnet) |
| 3 | Set policy on a real program | target `84h2juN2…`, program data `BU47HJU3…` | [`5UCrNvqU…`](https://solscan.io/tx/5UCrNvqUbs6vc5Ro1rnSUiDTeZNt7EBiXDTBvAmSiBTLYYkd9nT6YJ7nPxiBHYwKaDEBDiemJcqUh4oHEgeX6MAd?cluster=devnet) |
| 4 | Arm | baseline authority read **by the program, from the chain** | [`wcww5ZiY…`](https://solscan.io/tx/wcww5ZiY6TJPzVvvtrFH6xvMp6YBhaauVVtKyU7fzP2ZbhEmZKjeJvY2dUcX3DXG9Pgbofwy2Gv4knhRJGn9LCn?cluster=devnet) |
| 5 | **Exit, nothing changed** | **REFUSED — predicate false** | no transaction: the program aborted |
| 6 | **Seize the program** — transfer its upgrade authority | the trigger | [`67P2Ckhd…`](https://solscan.io/tx/67P2Ckhdppm8fdWsyoteErvd15GoosCgme89SJnFo4PdNS2SCBfkCfmhyJmmyCiY77GbP2LkSPRdSn12YJstb5z9?cluster=devnet) |
| 7 | **Exit, authority moved** | **LANDED** — 0.05 SOL to the owner | [`3sHWWsLy…`](https://solscan.io/tx/3sHWWsLybQuERz6G7wkkhotVzZo2HX5xkkgsTAkWUNqoaSdBC6X1j5mHq3RpsCCcpo3dDFZysu42Ge4BfnMbdPZj?cluster=devnet) |
| 8 | Exit to a third party, predicate still true | **REFUSED — destination locked to owner** | no transaction: the program aborted |

Owner balance, measured either side of step 7: **1.61380236 → 1.66379236 SOL**. The difference is
the 0.05 SOL exit, less fees.

## Why steps 5 and 7 together are the whole product

The two exits are *identical transactions*. Same guardian, same signature scheme, same
destination, same amount, same policy. The only thing that changed between them is a fact on
chain, and the program read that fact itself.

- At step 5 the keeper was wrong, or lying, or compromised. It did not matter which, because the
  chain refused it.
- At step 7 the keeper was right, and the chain agreed, and the money moved to the owner.

An off-chain keeper that decides for itself cannot produce step 5. That is the failure mode behind
Summer.fi's automated vaults losing $6M in July, and it is the first thing a judge should ask
about.

## Why step 8 matters separately

At step 8 the predicate was **genuinely true** — the authority really had moved, so the policy
really had fired. The only thing standing between a guardian and someone else's wallet was the
delegation invariant. It held.

This is PRD **A2**, and it is also covered by a test that runs against the compiled program in
LiteSVM: `the_guardian_cannot_send_funds_to_a_third_party`.

## Reproducing it

```bash
anchor build
cargo test --workspace          # 37 tests, including the invariants above

PROG=84h2juN2WaFhZtGbaqmeWVVe7HQCUXnDRRtbGZQeAeTu
ME=$(solana-keygen pubkey ~/.config/solana/id.json)

cargo run -p ripcord-keeper -- vault-init
cargo run -p ripcord-keeper -- deposit --lamports 200000000
cargo run -p ripcord-keeper -- policy  --id 1 --target $PROG
cargo run -p ripcord-keeper -- arm     --id 1 --target $PROG --guardian $ME --cap 100000000

cargo run -p ripcord-keeper -- status  --id 1 --target $PROG   # predicate: false
cargo run -p ripcord-keeper -- exit    --id 1 --target $PROG --lamports 50000000   # refused

cargo run -p ripcord-keeper -- seize   --target $PROG --to <another key you hold>
cargo run -p ripcord-keeper -- status  --id 1 --target $PROG   # predicate: TRUE
cargo run -p ripcord-keeper -- exit    --id 1 --target $PROG --lamports 50000000   # lands
```

`seize` is the ordinary upgradeable-loader `SetAuthority` instruction. It is fired against a
program we deployed and own, and in the recorded demo that is said out loud. No part of the
detection path is simulated or staged.

## What this run does not show

Stated here so it is not discovered in Q&A.

- **This is devnet, not mainnet.** `A1` is not met until the program is deployed to mainnet.
- **This is the on-chain half only.** The exit here was proposed by a human running a CLI. The live
  engine that watches a pre-execution stream and proposes automatically is Phase 3, and it needs
  Aperture access that does not exist yet.
- **No slot-timing claim is made.** The pitch's "exit lands in slot N+1" is not demonstrated by
  this run and is not claimed by it.
- **The policy here is authority change only.** Oracle staleness and outflow rate are roadmap.
