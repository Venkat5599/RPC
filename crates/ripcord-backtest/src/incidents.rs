//! The incident windows, as data.
//!
//! Every window is defined by UTC timestamps taken from the project docs and
//! carries the source it came from. Slots are deliberately **not** hardcoded:
//! a slot number invented to look precise is exactly the kind of unverifiable
//! figure PRD N4 forbids, so windows are matched against the `blockTime` the
//! chain itself reports.
//!
//! Where the docs give a date but not a time, `precision` says so, and the
//! report prints it. An imprecise window widens the search; it never silently
//! sharpens a lead-time number.

/// How exactly the window is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    /// Start and end are known to the minute.
    ToTheMinute,
    /// Only the day (or span of days) is known; the window is the whole day.
    ToTheDay,
}

#[derive(Debug, Clone)]
pub struct Incident {
    /// Stable identifier, used for cache paths and report sections.
    pub id: &'static str,
    pub protocol: &'static str,
    /// Inclusive window start, Unix seconds UTC.
    pub start_unix: i64,
    /// Inclusive window end, Unix seconds UTC.
    pub end_unix: i64,
    pub precision: Precision,
    /// The moment the loss finished, Unix seconds UTC, when it is known.
    /// Lead time is measured against this, so an unknown value means the
    /// incident contributes detections but no lead-time figure.
    pub loss_complete_unix: Option<i64>,
    /// Programs to enumerate for this window. These are the addresses whose
    /// signature history is walked.
    pub programs: &'static [&'static str],
    /// Protocol programs whose authority is being watched. A detection only
    /// fires for one of these, so an empty list means the window is
    /// enumerated but nothing can match — stated rather than hidden.
    pub watch_programs: &'static [&'static str],
    /// Where the timings came from. Printed in the report so a reader can
    /// check the window itself, not just the result.
    pub source: &'static str,
    /// What is not yet verified about this entry.
    pub caveat: Option<&'static str>,
}

impl Incident {
    /// Seconds per slot during this window.
    ///
    /// Slot time moved from 400ms to 350ms on 22 Aug 2026 under SIMD-0525, so
    /// converting slots to minutes with one global constant silently misstates
    /// every pre-2026 window. The conversion is per-incident for that reason.
    pub fn slot_seconds(&self) -> f64 {
        const SIMD_0525_ACTIVATION_UNIX: i64 = 1_787_356_800; // 2026-08-22 00:00 UTC
        if self.start_unix >= SIMD_0525_ACTIVATION_UNIX {
            0.350
        } else {
            0.400
        }
    }
}

/// Kamino Lend, mainnet.
pub const KAMINO_LEND: &str = "KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD";
/// The upgradeable loader, where upgrade authority actually changes.
pub const BPF_LOADER_UPGRADEABLE: &str = "BPFLoaderUpgradeab1e11111111111111111111111";

/// The windows the backtest replays.
///
/// Scoped to the three incidents the brief names. Kamino is first because it
/// is the only protocol with an adapter in the MVP, and because a liquidation
/// cascade is the case where lead time is most arguable — running the weakest
/// case first is the honest order.
pub fn incidents() -> Vec<Incident> {
    vec![
        Incident {
            id: "drift-2026-04-01",
            protocol: "Drift",
            // 1 Apr 2026 16:05 UTC to 18:31 UTC.
            start_unix: 1_775_059_500,
            end_unix: 1_775_068_260,
            precision: Precision::ToTheMinute,
            loss_complete_unix: Some(1_775_068_260),
            programs: &[BPF_LOADER_UPGRADEABLE],
            watch_programs: &[],
            source: "BRIEF.md: ~$285M over ~2.5 hours; TODO.md gives 16:05-18:31 UTC",
            caveat: Some(
                "the pause time is unpublished, so the window ends at the last drain, not at the pause; Drift's program ID is not yet verified, so nothing is watched and this window currently measures enumeration only",
            ),
        },
        Incident {
            id: "kamino-2025-10",
            protocol: "Kamino",
            // October 2025, day unknown: the whole month is searched.
            start_unix: 1_759_276_800, // 2025-10-01 00:00 UTC
            end_unix: 1_761_955_199,   // 2025-10-31 23:59:59 UTC
            precision: Precision::ToTheDay,
            loss_complete_unix: None,
            programs: &[KAMINO_LEND, BPF_LOADER_UPGRADEABLE],
            watch_programs: &[KAMINO_LEND],
            source: "BRIEF.md: 9,372 liquidations, 1,895 wallets, $25.5M seized",
            caveat: Some("exact day and loss-completion time not established in the docs yet"),
        },
        Incident {
            id: "marginfi-2024-04-11",
            protocol: "marginfi",
            // 11-13 Apr 2024, days only.
            start_unix: 1_712_793_600, // 2024-04-11 00:00 UTC
            end_unix: 1_713_052_799,   // 2024-04-13 23:59:59 UTC
            precision: Precision::ToTheDay,
            loss_complete_unix: None,
            programs: &[BPF_LOADER_UPGRADEABLE],
            watch_programs: &[],
            source: "BRIEF.md: CEO resigned, >$130M withdrawn in 24h, TVL -25%",
            caveat: Some(
                "not an exploit: there is no loss to lead, so this window measures withdrawal behaviour, not detection lead time; marginfi's program ID is not yet verified, so nothing is watched",
            ),
        },
    ]
}

/// A short window ending now, over Kamino Lend.
///
/// Not an incident: a self-test. It proves the whole path — enumerate, fetch,
/// normalise, evaluate, report — against live mainnet data that any endpoint
/// can serve, without needing archival history. It is generated rather than
/// stored because its window is relative to the moment it runs.
pub fn smoke_window(now_unix: i64, minutes: i64) -> Incident {
    Incident {
        id: "smoke-recent",
        protocol: "Kamino",
        start_unix: now_unix - minutes * 60,
        end_unix: now_unix,
        precision: Precision::ToTheMinute,
        loss_complete_unix: None,
        programs: &[KAMINO_LEND],
        watch_programs: &[KAMINO_LEND],
        source: "not an incident: a live self-test of the replay path",
        caveat: Some(
            "ordinary recent traffic, so zero detections is the expected and correct result",
        ),
    }
}

/// One incident by id.
pub fn by_id(id: &str) -> Option<Incident> {
    incidents().into_iter().find(|i| i.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_are_ordered_and_non_empty() {
        for incident in incidents() {
            assert!(
                incident.start_unix < incident.end_unix,
                "{}: window must run forwards",
                incident.id
            );
            assert!(
                !incident.programs.is_empty(),
                "{}: a window with no programs enumerates nothing",
                incident.id
            );
        }
    }

    #[test]
    fn slot_duration_follows_the_simd_0525_change() {
        let drift = by_id("drift-2026-04-01").unwrap();
        let marginfi = by_id("marginfi-2024-04-11").unwrap();
        // Both windows predate 22 Aug 2026, so both are still 400ms.
        assert_eq!(drift.slot_seconds(), 0.400);
        assert_eq!(marginfi.slot_seconds(), 0.400);
    }
}
