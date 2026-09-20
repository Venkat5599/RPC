# RIPCORD backtest — authority-change policy

Generated 2026-09-20T09:12:55Z. Endpoint: `https://api.mainnet-beta.solana.com`.

Reproduce with:

```
cargo run -p ripcord-backtest -- run --incident smoke-recent --limit 40 --page-limit 200 --cache cache --out reports/smoke.md
```

The evaluator is the production policy crate (`ripcord-policy`), not a separate implementation written for this report. The only thing that differs from the live engine is the input source.

## Headline

No lead time is measurable yet: 0 detection(s) across 106 transactions in 1 window(s), and no window has both a detection and an established loss-completion time. The headline sentence is deliberately absent rather than estimated.

## Windows

| Incident | Window (UTC) | Precision | Examined | Detections | Refusals | Skipped | Lead |
|---|---|---|---|---|---|---|---|
| smoke-recent | 2026-09-20T09:10:02Z → 2026-09-20T09:12:02Z | minute | 106 | 0 | 0 | 0 | not measurable

### smoke-recent — Kamino

- Source of the window: not an incident: a live self-test of the replay path
- **Caveat:** ordinary recent traffic, so zero detections is the expected and correct result
- Enumerated programs: KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD
- Watched programs: KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD
- Transactions: 106 examined, 40 fetched this run, 0 from cache
- Slot duration used for conversion: 0.350s

**No detections in this window.** Reported as-is; an empty window is a result, not a failure to be tuned away.

## What this does not show

- Only the authority-change policy runs here. Oracle staleness and outflow rate are roadmap, per the cut line in TODO.md.
- Detection is measured against historical *confirmed* transactions. The live engine reads pre-execution simulated transactions, which lands earlier; this report does not claim that improvement, it measures the policy.
- Windows marked `day` precision are searched across whole days because the published sources do not give a time. That widens the search; it does not sharpen the result.
