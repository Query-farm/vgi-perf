//! The row structs the decoder produces — one Rust struct per SQL surface
//! (`samples`, `mmaps`, `comms`, `events`, `meta`). These are pure data: the
//! `perf-worker` crate maps them onto Arrow arrays. Keeping them Arrow-free lets
//! the core crate be tested directly against golden fixtures.

/// One row of `perf.samples(src)`. A PERF_RECORD_SAMPLE — a profiling event
/// (e.g. a CPU `cycles` overflow). Every field is `Option` because which fields
/// a sample carries is governed by the recording's `sample_type` bitmask: a
/// capture without `PERF_SAMPLE_CPU` yields `cpu = None` → SQL `NULL`.
#[derive(Debug, Clone, PartialEq)]
pub struct SampleRow {
    /// Capture timestamp in perf's clock (nanoseconds), if `PERF_SAMPLE_TIME`.
    pub time: Option<u64>,
    /// Process id, if `PERF_SAMPLE_TID`.
    pub pid: Option<i32>,
    /// Thread id, if `PERF_SAMPLE_TID`.
    pub tid: Option<i32>,
    /// CPU the sample was taken on, if `PERF_SAMPLE_CPU`.
    pub cpu: Option<u32>,
    /// The sampled instruction pointer, if `PERF_SAMPLE_IP`. Raw (unsymbolicated)
    /// — join it against `mmaps` + `vgi-symbols` to resolve a function.
    pub ip: Option<u64>,
    /// The sample period at the time of the sample, if `PERF_SAMPLE_PERIOD`.
    pub period: Option<u64>,
    /// The event name this sample belongs to (e.g. `cycles`), resolved from the
    /// recording's attribute table. `None` if the capture omits event names.
    pub event: Option<String>,
    /// The call chain: raw instruction pointers from innermost to outermost
    /// frame (kernel + user), in capture order, if `PERF_SAMPLE_CALLCHAIN`.
    /// `None` if the capture has no call chains; resolution is **not** done here.
    pub callchain: Option<Vec<u64>>,
}

/// One row of `perf.mmaps(src)`. A PERF_RECORD_MMAP / MMAP2 — a memory mapping
/// (which DSO/shared-object backs a range of a process's address space). These
/// are what make a sample `ip` interpretable: the `[addr, addr+len)` range a
/// raw IP falls in identifies the mapped file. `build_id` feeds `vgi-symbols`.
#[derive(Debug, Clone, PartialEq)]
pub struct MmapRow {
    pub pid: i32,
    pub tid: i32,
    /// Start address of the mapping.
    pub addr: u64,
    /// Length of the mapping in bytes.
    pub len: u64,
    /// Offset into the mapped file at which the mapping starts.
    pub pgoff: u64,
    /// The mapped file path (e.g. `/usr/lib/libc.so.6`, `[vdso]`).
    pub filename: String,
    /// The DSO build-id as a lowercase hex string, if known — either carried
    /// inline by an MMAP2 build-id record, or resolved from the file's
    /// `HEADER_BUILD_ID` feature section by path. `None` when the capture has no
    /// build-id for this mapping.
    pub build_id: Option<String>,
}

/// One row of `perf.comms(src)`. A PERF_RECORD_COMM — a process/thread name
/// assignment (what `pid`/`tid` is called, e.g. `chrome`). Resolves a numeric
/// pid in `samples` to a human-readable process name.
#[derive(Debug, Clone, PartialEq)]
pub struct CommRow {
    pub pid: i32,
    pub tid: i32,
    /// The command / thread name.
    pub comm: String,
    /// Timestamp of the COMM record, if the capture records sample ids/times.
    pub time: Option<u64>,
}

/// One row of `perf.events(src)`. A recorded event attribute — describes one of
/// the perf events the capture sampled (its name, kind, and sampling policy).
#[derive(Debug, Clone, PartialEq)]
pub struct EventRow {
    /// The primary event id (the first id in the attribute's id list), or the
    /// attribute index when the capture records no ids.
    pub event_id: u64,
    /// The event name (e.g. `cycles`, `instructions`), if recorded.
    pub name: Option<String>,
    /// The event kind: `hardware`, `software`, `tracepoint`, `hw_cache`,
    /// `breakpoint`, `dynamic_pmu`, or `raw`.
    pub r#type: String,
    /// The event config value, when it is a plain integer in the source
    /// (tracepoint id / dynamic-PMU config). `None` for kinds whose config is a
    /// structured enum (hardware/software/hw_cache) — the `name`/`type` identify
    /// those — so the column degrades gracefully rather than guessing.
    pub config: Option<u64>,
    /// Fixed sampling period, if the event uses period-based sampling.
    pub sample_period: Option<u64>,
    /// Target sampling frequency (samples/sec), if frequency-based sampling.
    pub sample_freq: Option<u64>,
}

/// The single row of `perf.meta(src)`. Capture-wide metadata read from the
/// `HEADER_*` feature sections. Every field is `Option` because a given kernel /
/// `perf` build may omit any feature section — the column degrades to `NULL`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MetaRow {
    pub hostname: Option<String>,
    pub arch: Option<String>,
    /// Number of CPUs available (from `HEADER_NRCPUS`).
    pub nrcpus: Option<u32>,
    pub perf_version: Option<String>,
    /// The `perf record` command line, split into arguments.
    pub cmdline: Option<Vec<String>>,
    /// Total number of event records decoded from the data section.
    pub total_events: u64,
}

/// A per-record decode error. The decoder captures these instead of aborting the
/// whole scan: a malformed or truncated tail record yields an error here and the
/// already-decoded rows are still returned (per-record error capture).
#[derive(Debug, Clone, PartialEq)]
pub struct DecodeError {
    /// The ordinal of the record in the sequential event stream where decoding
    /// stopped or a record was skipped (a position in the stream, not a byte
    /// offset — perf records are variable-length and the parser is not
    /// byte-seekable).
    pub record_index: u64,
    /// A human-readable description of what went wrong.
    pub message: String,
}

/// Everything decoded from one `perf.data` source: the five row collections plus
/// any per-record errors captured along the way.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Decoded {
    pub samples: Vec<SampleRow>,
    pub mmaps: Vec<MmapRow>,
    pub comms: Vec<CommRow>,
    pub events: Vec<EventRow>,
    pub meta: MetaRow,
    pub errors: Vec<DecodeError>,
}
