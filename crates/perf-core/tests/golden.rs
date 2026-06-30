//! Golden-fixture tests for the `perf.data` structural decoder.
//!
//! Two fixtures:
//! - the in-memory **synthetic** capture ([`perf_core::fixture::synthetic_callchain`])
//!   — a `cycles` capture with known call chains, inline build-ids, and an empty
//!   feature bitmap (so `meta` degrades to NULL);
//! - the **real** `data/sleep.data` — an uncompressed `perf record sleep 1`
//!   capture (cycles) with real build-ids and populated `HEADER_*` metadata,
//!   vendored from the MIT-licensed linux-perf-data crate.
//!
//! Plus a proptest asserting the decoder never panics on arbitrary / truncated
//! input (per-record error capture, truncated tail → error row not a panic).

use std::path::PathBuf;

use perf_core::{decode, fixture};

fn data_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../data")
        .join(name)
}

#[test]
fn synthetic_callchain_golden() {
    let bytes = fixture::synthetic_callchain();
    let d = decode(&bytes).expect("synthetic perf.data decodes");
    assert!(d.errors.is_empty(), "no decode errors: {:?}", d.errors);

    // Three samples, in capture (time) order, each carrying its known callchain.
    assert_eq!(d.samples.len(), 3);
    let s0 = &d.samples[0];
    assert_eq!(s0.ip, Some(0x40_0100));
    assert_eq!(s0.cpu, Some(2));
    assert_eq!(s0.time, Some(1000));
    assert_eq!(s0.period, Some(4000));
    assert_eq!(s0.callchain, Some(vec![0x40_0100, 0x40_0050]));
    assert_eq!(
        d.samples[1].callchain,
        Some(vec![0x7f00_0000_0100, 0x40_0100])
    );
    assert_eq!(d.samples[2].callchain, Some(vec![0x40_0100]));

    // Two mappings with inline build-ids.
    assert_eq!(d.mmaps.len(), 2);
    let loop_map = d
        .mmaps
        .iter()
        .find(|m| m.filename == "/usr/bin/loop")
        .expect("loop mapping");
    assert_eq!(loop_map.addr, 0x40_0000);
    assert_eq!(loop_map.len, 0x1000);
    assert_eq!(loop_map.build_id.as_deref(), Some(&"aa".repeat(20)[..]));
    let libc = d
        .mmaps
        .iter()
        .find(|m| m.filename == "/usr/lib/libc.so.6")
        .expect("libc mapping");
    assert_eq!(libc.build_id.as_deref(), Some(&"bb".repeat(20)[..]));

    // One COMM record.
    assert_eq!(d.comms.len(), 1);
    assert_eq!(d.comms[0].comm, "loop");
    assert_eq!(d.comms[0].pid, 1234);

    // One cycles hardware event, period-sampled at 4000.
    assert_eq!(d.events.len(), 1);
    assert_eq!(d.events[0].r#type, "hardware");
    assert_eq!(d.events[0].sample_period, Some(4000));
    assert_eq!(d.events[0].sample_freq, None);

    // Empty feature bitmap → every meta column NULL (graceful degradation), but
    // total_events still counts every event record (1 comm + 2 mmap2 + 3 sample).
    assert_eq!(d.meta.hostname, None);
    assert_eq!(d.meta.arch, None);
    assert_eq!(d.meta.nrcpus, None);
    assert_eq!(d.meta.cmdline, None);
    assert_eq!(d.meta.total_events, 6);
}

#[test]
fn synthetic_ip_mmap_range_join() {
    // The relational core of the marquee query: every sample ip falls inside
    // exactly one mapping's [addr, addr+len) range.
    let d = decode(&fixture::synthetic_callchain()).unwrap();
    for s in &d.samples {
        let ip = s.ip.unwrap();
        let hit = d
            .mmaps
            .iter()
            .find(|m| ip >= m.addr && ip < m.addr + m.len)
            .expect("each ip falls in a mapping");
        if ip < 0x1_0000_0000 {
            assert_eq!(hit.filename, "/usr/bin/loop");
        } else {
            assert_eq!(hit.filename, "/usr/lib/libc.so.6");
        }
    }
}

#[test]
fn real_sleep_capture_golden() {
    let bytes = std::fs::read(data_path("sleep.data")).expect("read data/sleep.data");
    let d = decode(&bytes).expect("sleep.data decodes");
    assert!(d.errors.is_empty(), "no decode errors: {:?}", d.errors);

    // Populated HEADER_* metadata.
    assert_eq!(d.meta.hostname.as_deref(), Some("arthur-des"));
    assert_eq!(d.meta.arch.as_deref(), Some("x86_64"));
    assert_eq!(d.meta.nrcpus, Some(16));
    assert!(d.meta.perf_version.is_some());
    let cmdline = d.meta.cmdline.as_ref().expect("cmdline present");
    assert_eq!(cmdline.first().map(String::as_str), Some("/usr/bin/perf"));
    assert!(cmdline.iter().any(|a| a == "record"));

    // A real cycles capture: samples, mmaps (some with build-ids), comms.
    assert_eq!(d.samples.len(), 7);
    assert!(d.samples.iter().all(|s| s.ip.is_some()));
    assert_eq!(d.mmaps.len(), 4);
    assert!(
        d.mmaps.iter().filter(|m| m.build_id.is_some()).count() >= 2,
        "at least two mappings carry a build-id from HEADER_BUILD_ID"
    );
    // build-ids are lowercase hex.
    for m in d.mmaps.iter().filter_map(|m| m.build_id.as_ref()) {
        assert!(m
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }
    assert!(!d.comms.is_empty());

    // One event: cycles.
    assert_eq!(d.events.len(), 1);
    assert!(d.events[0].name.as_deref().unwrap_or("").contains("cycles"));
    assert_eq!(d.events[0].r#type, "hardware");
}

#[test]
fn fatal_on_non_perf_input() {
    // Not a perf.data file at all → a fatal error, not a panic.
    assert!(decode(b"this is not perf.data").is_err());
    assert!(decode(&[]).is_err());
}

#[cfg(test)]
mod prop {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        // Arbitrary bytes never panic the decoder — they either fatally error
        // (not a perf.data) or decode to some rows + captured per-record errors.
        #[test]
        fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..4096)) {
            let _ = decode(&bytes);
        }

        // A valid header followed by arbitrary garbage, and every truncation of a
        // real capture, must never panic — the truncated tail becomes an error
        // row (or fatal), never an abort.
        #[test]
        fn truncated_synthetic_never_panics(cut in 0usize..600) {
            let full = fixture::synthetic_callchain();
            let n = cut.min(full.len());
            let _ = decode(&full[..n]);
        }
    }

    #[test]
    fn truncating_real_capture_never_panics() {
        let full = std::fs::read(data_path("sleep.data")).unwrap();
        // Every prefix length — exhaustive truncation of a real file.
        for n in 0..=full.len() {
            let _ = decode(&full[..n]);
        }
    }
}
