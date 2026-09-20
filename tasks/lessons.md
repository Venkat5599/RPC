# lessons.md — rules written from corrections

> Appended after ANY user correction. Each entry: what went wrong, the rule that prevents it.
> Reviewed at the start of every session before touching code.

## Standing rules (pre-seeded from project docs, not from corrections)

- **L1 — No unverified numbers.** PRD N4: no metric simulated, extrapolated or staged without being labelled as such. A backtest number that cannot be recomputed from public data does not go in the report.
- **L2 — Never mark done without a probe.** Every PRD acceptance criterion (A1–A16) has a named probe. "It compiles" is not a probe.
- **L3 — One evaluator, three input sources.** Backtest, live feed and TxStream must call the same policy code. A separate backtest codebase is a lie about the product (ARCHITECTURE §3b).
- **L4 — Refuse partial data.** `alt_resolution != FULL` → log, alert, stop. Never evaluate (N2).
- **L5 — Below the CUT LINE stays unbuilt** until everything above demos. TODO.md is binding.

## Corrections log

### 2026-09-20 — the engine failed safe, and was blind

**What happened.** The first smoke run refused 39 of 40 Kamino transactions. The refusal path
worked exactly as designed, so nothing looked broken. It was broken: `alt_resolution` read
`meta.loadedAddresses`, which `jsonParsed` never populates. The resolved lookup addresses were
present the whole time, merged into `accountKeys` with `source: "lookupTable"` (17 of 32 keys on a
real transaction, matching the table's 9 writable + 8 readonly exactly).

**Rule — L6: a safe failure is still a failure, and it hides better than a crash.**
A refusal rate that is not near zero on ordinary traffic is a bug until proven otherwise. Check the
actual bytes the source returns before trusting a field name; never infer a wire format from another
encoding's shape. This is the precedent ARCHITECTURE.md already names — Yellowstone shipped empty
`loaded_addresses` for months because a flag defaulted to false. We reproduced it in a day.

**Rule — L7: write the file in UTF-8, explicitly.**
Bulk-editing a source file through Python on Windows without `encoding='utf-8'` rewrote it as
cp1252 and broke the build with `stream did not contain valid UTF-8`. Always pass the encoding.

**Rule — L8: compute timestamps, never hand-write them.**
Four of seven hand-written Unix timestamps in `incidents.rs` were wrong, one by a full year. They
are now derived and checked.
