//! The `perf.data` structural decoder.
//!
//! [`decode`] reads the whole binary `perf.data` byte image once and returns the
//! five row collections ([`Decoded`]). It is **resilient**: a malformed or
//! truncated tail record is captured as a [`DecodeError`] and the already-decoded
//! rows are still returned — decoding never panics and never aborts the scan on a
//! bad record (per-record error capture). Only a fatally unreadable header (not a
//! `perf.data` file at all) returns `Err`.
//!
//! Scope (committee directive): the BINARY structural decode only. Call-chain IPs
//! are emitted **raw** — symbol resolution is not done here; it is the job of
//! `vgi-symbols`, fed by `mmaps.build_id` + the raw `ip`.

use std::collections::HashMap;
use std::io::Cursor;

use linux_perf_data::linux_perf_event_reader::{
    EventRecord, Mmap2FileId, PerfEventType, SamplingPolicy,
};
use linux_perf_data::{AttributeDescription, PerfFileReader, PerfFileRecord};

use crate::rows::{CommRow, DecodeError, Decoded, EventRow, MetaRow, MmapRow, SampleRow};

/// A fatal decode error: the source is not a readable `perf.data` file. Per-record
/// problems do **not** surface here — they are captured in [`Decoded::errors`].
#[derive(Debug)]
pub struct FatalError(pub String);

impl std::fmt::Display for FatalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "not a readable perf.data file: {}", self.0)
    }
}

impl std::error::Error for FatalError {}

/// Lowercase-hex encode a build-id byte string (e.g. `b"\xab\xcd"` → `"abcd"`).
fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        s.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
    }
    s
}

/// The event-kind tag string for `events.type`.
fn type_name(t: &PerfEventType) -> &'static str {
    match t {
        PerfEventType::Hardware(..) => "hardware",
        PerfEventType::Software(..) => "software",
        PerfEventType::Tracepoint(..) => "tracepoint",
        PerfEventType::HwCache(..) => "hw_cache",
        PerfEventType::Breakpoint(..) => "breakpoint",
        PerfEventType::DynamicPmu(..) => "dynamic_pmu",
    }
}

/// The plain-integer config for an event, when the source carries one directly
/// (tracepoint id / dynamic-PMU config). Structured-enum kinds return `None` so
/// the column degrades to `NULL` rather than fabricating a number.
fn config_value(t: &PerfEventType) -> Option<u64> {
    match t {
        PerfEventType::Tracepoint(c) => Some(*c),
        PerfEventType::DynamicPmu(_, config, _, _) => Some(*config),
        _ => None,
    }
}

/// Build the `events` rows + the parallel `attr_index → name` lookup used to
/// label each sample, from the recording's attribute table.
fn decode_events(attrs: &[AttributeDescription]) -> (Vec<EventRow>, Vec<Option<String>>) {
    let mut events = Vec::with_capacity(attrs.len());
    let mut names = Vec::with_capacity(attrs.len());
    for (idx, desc) in attrs.iter().enumerate() {
        let name = desc.name().map(|s| s.to_string());
        names.push(name.clone());
        let attr = desc.attributes();
        let (sample_period, sample_freq) = match attr.sampling_policy {
            SamplingPolicy::NoSampling => (None, None),
            SamplingPolicy::Period(p) => (Some(p.get()), None),
            SamplingPolicy::Frequency(f) => (None, Some(f)),
        };
        let event_id = desc.ids().first().copied().unwrap_or(idx as u64);
        events.push(EventRow {
            event_id,
            name,
            r#type: type_name(&attr.type_).to_string(),
            config: config_value(&attr.type_),
            sample_period,
            sample_freq,
        });
    }
    (events, names)
}

/// Read the capture-wide metadata from the `HEADER_*` feature sections, degrading
/// any omitted section to `None`.
fn decode_meta(perf_file: &linux_perf_data::PerfFile) -> MetaRow {
    MetaRow {
        hostname: perf_file.hostname().ok().flatten().map(str::to_string),
        arch: perf_file.arch().ok().flatten().map(str::to_string),
        nrcpus: perf_file
            .nr_cpus()
            .ok()
            .flatten()
            .map(|n| n.nr_cpus_available),
        perf_version: perf_file.perf_version().ok().flatten().map(str::to_string),
        cmdline: perf_file
            .cmdline()
            .ok()
            .flatten()
            .map(|v| v.into_iter().map(str::to_string).collect()),
        total_events: 0,
    }
}

/// A `filename → build-id hex` map from the `HEADER_BUILD_ID` feature section,
/// used to attach a build-id to an MMAP record that did not carry one inline.
fn build_id_map(perf_file: &linux_perf_data::PerfFile) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Ok(ids) = perf_file.build_ids() {
        for info in ids.into_values() {
            let path = String::from_utf8_lossy(&info.path).into_owned();
            map.insert(path, hex(&info.build_id));
        }
    }
    map
}

/// Decode a whole `perf.data` byte image into the five row collections.
///
/// Resilient by contract: per-record failures are captured in
/// [`Decoded::errors`] and decoding continues / stops gracefully; the function
/// only returns `Err(FatalError)` when the header itself is unreadable.
pub fn decode(bytes: &[u8]) -> Result<Decoded, FatalError> {
    let reader = Cursor::new(bytes);
    let PerfFileReader {
        mut perf_file,
        mut record_iter,
    } = PerfFileReader::parse_file(reader).map_err(|e| FatalError(e.to_string()))?;

    // Compute everything that borrows `perf_file` immutably up front, since the
    // record loop needs `&mut perf_file` for `next_record`.
    let (events, event_names) = decode_events(perf_file.event_attributes());
    let mut meta = decode_meta(&perf_file);
    let builds = build_id_map(&perf_file);

    let mut out = Decoded {
        events,
        meta: meta.clone(),
        ..Default::default()
    };

    let mut index: u64 = 0;
    loop {
        let next = record_iter.next_record(&mut perf_file);
        let record = match next {
            Ok(Some(r)) => r,
            Ok(None) => break,
            // A truncated / malformed tail record: capture it and stop, keeping
            // every row decoded so far (truncated tail → error row, not a failed
            // query).
            Err(e) => {
                out.errors.push(DecodeError {
                    record_index: index,
                    message: e.to_string(),
                });
                break;
            }
        };

        match record {
            PerfFileRecord::EventRecord { attr_index, record } => {
                out.meta.total_events += 1;
                let timestamp = record.timestamp();
                let parsed = match record.parse() {
                    Ok(p) => p,
                    Err(e) => {
                        out.errors.push(DecodeError {
                            record_index: index,
                            message: format!("event record parse failed: {e}"),
                        });
                        index += 1;
                        continue;
                    }
                };
                match parsed {
                    EventRecord::Sample(s) => {
                        let callchain = s.callchain.map(|cc| {
                            (0..cc.len())
                                .filter_map(|i| cc.get(i))
                                .collect::<Vec<u64>>()
                        });
                        let event = event_names.get(attr_index).cloned().flatten();
                        out.samples.push(SampleRow {
                            time: s.timestamp.or(timestamp),
                            pid: s.pid,
                            tid: s.tid,
                            cpu: s.cpu,
                            ip: s.ip,
                            period: s.period,
                            event,
                            callchain,
                        });
                    }
                    EventRecord::Mmap(m) => {
                        let filename = String::from_utf8_lossy(&m.path.as_slice()).into_owned();
                        let build_id = builds.get(&filename).cloned();
                        out.mmaps.push(MmapRow {
                            pid: m.pid,
                            tid: m.tid,
                            addr: m.address,
                            len: m.length,
                            pgoff: m.page_offset,
                            filename,
                            build_id,
                        });
                    }
                    EventRecord::Mmap2(m) => {
                        let filename = String::from_utf8_lossy(&m.path.as_slice()).into_owned();
                        let build_id = match &m.file_id {
                            Mmap2FileId::BuildId(b) => Some(hex(b)),
                            Mmap2FileId::InodeAndVersion(_) => builds.get(&filename).cloned(),
                        };
                        out.mmaps.push(MmapRow {
                            pid: m.pid,
                            tid: m.tid,
                            addr: m.address,
                            len: m.length,
                            pgoff: m.page_offset,
                            filename,
                            build_id,
                        });
                    }
                    EventRecord::Comm(c) => {
                        out.comms.push(CommRow {
                            pid: c.pid,
                            tid: c.tid,
                            comm: String::from_utf8_lossy(&c.name.as_slice()).into_owned(),
                            time: timestamp,
                        });
                    }
                    // Fork/Exit/Lost/Throttle/ContextSwitch/Raw: counted in
                    // total_events but not surfaced as a first-class table in v1.
                    _ => {}
                }
            }
            // User records (build-id, feature, thread map, …) are consumed by the
            // feature-section accessors above; nothing to row-ify here.
            PerfFileRecord::UserRecord(_) => {}
        }
        index += 1;
    }

    // `meta` was snapshotted into `out.meta` before the loop incremented its
    // event counter; the running count on `out.meta` is the authoritative one.
    meta.total_events = out.meta.total_events;
    out.meta = meta;
    Ok(out)
}
