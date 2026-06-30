//! Arrow schema definitions and `RecordBatch` builders for the five `perf`
//! tables. Each schema carries per-column comments (surfaced via
//! `duckdb_columns().comment`) so every column is documented wherever it appears.
//!
//! The builders take a *slice* of the decoded rows (`start..start+len`) so the
//! table producer can stream a large capture one batch at a time rather than
//! materializing every sample's Arrow array at once.

use std::collections::HashMap;
use std::sync::Arc;

use arrow_array::builder::{
    Int32Builder, ListBuilder, StringBuilder, UInt32Builder, UInt64Builder,
};
use arrow_array::{ArrayRef, RecordBatch};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use perf_core::Decoded;
use vgi_rpc::{Result, RpcError};

/// The five table surfaces this worker exposes. Used by the shared producer to
/// dispatch schema + batch construction.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Table {
    Samples,
    Mmaps,
    Comms,
    Events,
    Meta,
}

/// A field carrying a column `comment`.
fn commented(name: &str, ty: DataType, nullable: bool, comment: &str) -> Field {
    Field::new(name, ty, nullable).with_metadata(HashMap::from([(
        "comment".to_string(),
        comment.to_string(),
    )]))
}

/// `LIST(UBIGINT)` element field — DuckDB's list child field is conventionally
/// named `item`.
fn u64_list(name: &str, comment: &str) -> Field {
    let item = Field::new("item", DataType::UInt64, true);
    commented(name, DataType::List(Arc::new(item)), true, comment)
}

fn utf8_list(name: &str, comment: &str) -> Field {
    let item = Field::new("item", DataType::Utf8, true);
    commented(name, DataType::List(Arc::new(item)), true, comment)
}

impl Table {
    /// The fixed output schema for this table.
    pub fn schema(self) -> SchemaRef {
        let fields = match self {
            Table::Samples => vec![
                commented("time", DataType::UInt64, true, "Capture timestamp in perf's clock (nanoseconds), or NULL if the capture omits PERF_SAMPLE_TIME."),
                commented("pid", DataType::Int32, true, "Process id the sample was taken in."),
                commented("tid", DataType::Int32, true, "Thread id the sample was taken in."),
                commented("cpu", DataType::UInt32, true, "CPU the sample was taken on, or NULL if the capture omits PERF_SAMPLE_CPU."),
                commented("ip", DataType::UInt64, true, "The raw (unsymbolicated) sampled instruction pointer. Join to mmaps + vgi-symbols to resolve a function."),
                commented("period", DataType::UInt64, true, "The sample period at the time of the sample (events per sample)."),
                commented("event", DataType::Utf8, true, "The event name this sample belongs to, e.g. 'cycles', resolved from the attribute table."),
                u64_list("callchain", "The call chain: raw instruction pointers from innermost to outermost frame (kernel + user), in capture order. Unsymbolicated. NULL if the capture has no call chains."),
            ],
            Table::Mmaps => vec![
                commented("pid", DataType::Int32, true, "Process id the mapping belongs to."),
                commented("tid", DataType::Int32, true, "Thread id the mapping belongs to."),
                commented("addr", DataType::UInt64, true, "Start address of the memory mapping."),
                commented("len", DataType::UInt64, true, "Length of the mapping in bytes. The mapping covers [addr, addr+len)."),
                commented("pgoff", DataType::UInt64, true, "Offset into the mapped file at which the mapping starts."),
                commented("filename", DataType::Utf8, true, "The mapped file path, e.g. '/usr/lib/libc.so.6' or '[vdso]'."),
                commented("build_id", DataType::Utf8, true, "The DSO build-id as lowercase hex, if known (inline MMAP2 build-id or HEADER_BUILD_ID). Feeds vgi-symbols. NULL when unknown."),
            ],
            Table::Comms => vec![
                commented("pid", DataType::Int32, true, "Process id this name applies to."),
                commented("tid", DataType::Int32, true, "Thread id this name applies to."),
                commented("comm", DataType::Utf8, true, "The command / thread name, e.g. 'chrome'."),
                commented("time", DataType::UInt64, true, "Timestamp of the COMM record, or NULL if the capture records no sample-id times."),
            ],
            Table::Events => vec![
                commented("event_id", DataType::UInt64, true, "The primary event id, or the attribute index when the capture records no ids."),
                commented("name", DataType::Utf8, true, "The event name, e.g. 'cycles', 'instructions', if recorded."),
                commented("type", DataType::Utf8, true, "Event kind: 'hardware', 'software', 'tracepoint', 'hw_cache', 'breakpoint', or 'dynamic_pmu'."),
                commented("config", DataType::UInt64, true, "The integer config value for tracepoint / dynamic-PMU events; NULL for structured-enum kinds (hardware/software/hw_cache)."),
                commented("sample_period", DataType::UInt64, true, "Fixed sampling period, if the event uses period-based sampling; else NULL."),
                commented("sample_freq", DataType::UInt64, true, "Target sampling frequency (samples/sec), if frequency-based sampling; else NULL."),
            ],
            Table::Meta => vec![
                commented("hostname", DataType::Utf8, true, "Capture host name (HEADER_HOSTNAME), or NULL if absent."),
                commented("arch", DataType::Utf8, true, "Capture machine architecture, e.g. 'x86_64' (HEADER_ARCH), or NULL if absent."),
                commented("nrcpus", DataType::UInt32, true, "Number of CPUs available on the capture host (HEADER_NRCPUS), or NULL if absent."),
                commented("perf_version", DataType::Utf8, true, "The perf tool version that produced the capture (HEADER_VERSION), or NULL if absent."),
                utf8_list("cmdline", "The 'perf record' command line, split into arguments (HEADER_CMDLINE), or NULL if absent."),
                commented("total_events", DataType::UInt64, true, "Total number of event records decoded from the data section."),
            ],
        };
        Arc::new(Schema::new(fields))
    }

    /// The total number of rows this table has for the decoded capture.
    pub fn row_count(self, d: &Decoded) -> usize {
        match self {
            Table::Samples => d.samples.len(),
            Table::Mmaps => d.mmaps.len(),
            Table::Comms => d.comms.len(),
            Table::Events => d.events.len(),
            Table::Meta => 1,
        }
    }

    /// Build the rows `[start, start+len)` of this table into a `RecordBatch`
    /// under `schema` (the possibly projection-narrowed output schema).
    pub fn build(
        self,
        d: &Decoded,
        schema: &SchemaRef,
        start: usize,
        len: usize,
    ) -> Result<RecordBatch> {
        let cols: Vec<ArrayRef> = match self {
            Table::Samples => build_samples(d, start, len),
            Table::Mmaps => build_mmaps(d, start, len),
            Table::Comms => build_comms(d, start, len),
            Table::Events => build_events(d, start, len),
            Table::Meta => build_meta(d),
        };
        // Apply projection narrowing: the producer's schema may be a subset.
        let projected = project(schema, full_schema(self), cols)?;
        RecordBatch::try_new(schema.clone(), projected)
            .map_err(|e| RpcError::runtime_error(e.to_string()))
    }
}

/// The full (unprojected) schema for a table, used to map projected field names
/// back to their column index.
fn full_schema(t: Table) -> SchemaRef {
    t.schema()
}

/// Reorder/narrow `cols` (built in full-schema order) to match `out_schema`,
/// which may be a projection subset (the framework also applies projection, but
/// matching here keeps `RecordBatch::try_new` happy when DuckDB narrows columns).
fn project(out_schema: &SchemaRef, full: SchemaRef, cols: Vec<ArrayRef>) -> Result<Vec<ArrayRef>> {
    if out_schema.fields().len() == full.fields().len() {
        return Ok(cols);
    }
    let mut out = Vec::with_capacity(out_schema.fields().len());
    for f in out_schema.fields() {
        let idx = full
            .index_of(f.name())
            .map_err(|e| RpcError::runtime_error(e.to_string()))?;
        out.push(cols[idx].clone());
    }
    Ok(out)
}

fn build_samples(d: &Decoded, start: usize, len: usize) -> Vec<ArrayRef> {
    let rows = &d.samples[start..start + len];
    let mut time = UInt64Builder::new();
    let mut pid = Int32Builder::new();
    let mut tid = Int32Builder::new();
    let mut cpu = UInt32Builder::new();
    let mut ip = UInt64Builder::new();
    let mut period = UInt64Builder::new();
    let mut event = StringBuilder::new();
    let mut callchain = ListBuilder::new(UInt64Builder::new());
    for r in rows {
        time.append_option(r.time);
        pid.append_option(r.pid);
        tid.append_option(r.tid);
        cpu.append_option(r.cpu);
        ip.append_option(r.ip);
        period.append_option(r.period);
        event.append_option(r.event.as_deref());
        match &r.callchain {
            Some(cc) => {
                for v in cc {
                    callchain.values().append_value(*v);
                }
                callchain.append(true);
            }
            None => callchain.append(false),
        }
    }
    vec![
        Arc::new(time.finish()),
        Arc::new(pid.finish()),
        Arc::new(tid.finish()),
        Arc::new(cpu.finish()),
        Arc::new(ip.finish()),
        Arc::new(period.finish()),
        Arc::new(event.finish()),
        Arc::new(callchain.finish()),
    ]
}

fn build_mmaps(d: &Decoded, start: usize, len: usize) -> Vec<ArrayRef> {
    let rows = &d.mmaps[start..start + len];
    let mut pid = Int32Builder::new();
    let mut tid = Int32Builder::new();
    let mut addr = UInt64Builder::new();
    let mut length = UInt64Builder::new();
    let mut pgoff = UInt64Builder::new();
    let mut filename = StringBuilder::new();
    let mut build_id = StringBuilder::new();
    for r in rows {
        pid.append_value(r.pid);
        tid.append_value(r.tid);
        addr.append_value(r.addr);
        length.append_value(r.len);
        pgoff.append_value(r.pgoff);
        filename.append_value(&r.filename);
        build_id.append_option(r.build_id.as_deref());
    }
    vec![
        Arc::new(pid.finish()),
        Arc::new(tid.finish()),
        Arc::new(addr.finish()),
        Arc::new(length.finish()),
        Arc::new(pgoff.finish()),
        Arc::new(filename.finish()),
        Arc::new(build_id.finish()),
    ]
}

fn build_comms(d: &Decoded, start: usize, len: usize) -> Vec<ArrayRef> {
    let rows = &d.comms[start..start + len];
    let mut pid = Int32Builder::new();
    let mut tid = Int32Builder::new();
    let mut comm = StringBuilder::new();
    let mut time = UInt64Builder::new();
    for r in rows {
        pid.append_value(r.pid);
        tid.append_value(r.tid);
        comm.append_value(&r.comm);
        time.append_option(r.time);
    }
    vec![
        Arc::new(pid.finish()),
        Arc::new(tid.finish()),
        Arc::new(comm.finish()),
        Arc::new(time.finish()),
    ]
}

fn build_events(d: &Decoded, start: usize, len: usize) -> Vec<ArrayRef> {
    let rows = &d.events[start..start + len];
    let mut event_id = UInt64Builder::new();
    let mut name = StringBuilder::new();
    let mut type_ = StringBuilder::new();
    let mut config = UInt64Builder::new();
    let mut sample_period = UInt64Builder::new();
    let mut sample_freq = UInt64Builder::new();
    for r in rows {
        event_id.append_value(r.event_id);
        name.append_option(r.name.as_deref());
        type_.append_value(&r.r#type);
        config.append_option(r.config);
        sample_period.append_option(r.sample_period);
        sample_freq.append_option(r.sample_freq);
    }
    vec![
        Arc::new(event_id.finish()),
        Arc::new(name.finish()),
        Arc::new(type_.finish()),
        Arc::new(config.finish()),
        Arc::new(sample_period.finish()),
        Arc::new(sample_freq.finish()),
    ]
}

fn build_meta(d: &Decoded) -> Vec<ArrayRef> {
    let m = &d.meta;
    let mut hostname = StringBuilder::new();
    hostname.append_option(m.hostname.as_deref());
    let mut arch = StringBuilder::new();
    arch.append_option(m.arch.as_deref());
    let mut nrcpus = UInt32Builder::new();
    nrcpus.append_option(m.nrcpus);
    let mut perf_version = StringBuilder::new();
    perf_version.append_option(m.perf_version.as_deref());
    let mut cmdline = ListBuilder::new(StringBuilder::new());
    match &m.cmdline {
        Some(args) => {
            for a in args {
                cmdline.values().append_value(a);
            }
            cmdline.append(true);
        }
        None => cmdline.append(false),
    }
    let mut total_events = UInt64Builder::new();
    total_events.append_value(m.total_events);
    vec![
        Arc::new(hostname.finish()),
        Arc::new(arch.finish()),
        Arc::new(nrcpus.finish()),
        Arc::new(perf_version.finish()),
        Arc::new(cmdline.finish()),
        Arc::new(total_events.finish()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::cast::AsArray;
    use arrow_array::types::{UInt32Type, UInt64Type};
    use arrow_array::Array;

    fn decoded() -> Decoded {
        perf_core::decode(&perf_core::fixture::synthetic_callchain()).unwrap()
    }

    #[test]
    fn samples_batch_has_callchain_list() {
        let d = decoded();
        let schema = Table::Samples.schema();
        let batch = Table::Samples.build(&d, &schema, 0, 3).unwrap();
        assert_eq!(batch.num_rows(), 3);

        let ip = batch.column(4).as_primitive::<UInt64Type>();
        assert_eq!(ip.value(0), 0x40_0100);

        // callchain is LIST(UBIGINT); row 0 = [0x400100, 0x400050].
        let cc = batch.column(7).as_list::<i32>();
        let frames = cc.value(0);
        let frames = frames.as_primitive::<UInt64Type>();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames.value(0), 0x40_0100);
        assert_eq!(frames.value(1), 0x40_0050);
    }

    #[test]
    fn mmaps_batch_has_build_ids() {
        let d = decoded();
        let schema = Table::Mmaps.schema();
        let batch = Table::Mmaps.build(&d, &schema, 0, 2).unwrap();
        let build_id = batch.column(6).as_string::<i32>();
        assert!((0..2).any(|i| build_id.value(i) == "aa".repeat(20)));
    }

    #[test]
    fn meta_batch_degrades_to_nulls() {
        let d = decoded();
        let schema = Table::Meta.schema();
        let batch = Table::Meta.build(&d, &schema, 0, 1).unwrap();
        assert_eq!(batch.num_rows(), 1);
        // Empty feature bitmap → hostname/arch/nrcpus/cmdline all NULL.
        assert!(batch.column(0).is_null(0)); // hostname
        assert!(batch.column(2).is_null(0)); // nrcpus
        assert!(batch.column(4).is_null(0)); // cmdline LIST
                                             // total_events is always present.
        let total = batch.column(5).as_primitive::<UInt64Type>();
        assert_eq!(total.value(0), 6);
    }

    #[test]
    fn projection_narrows_columns() {
        let d = decoded();
        // Project just (ip, callchain) — fields 4 and 7 of the full schema.
        let full = Table::Samples.schema();
        let narrowed = Arc::new(Schema::new(vec![
            full.field(4).clone(),
            full.field(7).clone(),
        ]));
        let batch = Table::Samples.build(&d, &narrowed, 0, 3).unwrap();
        assert_eq!(batch.num_columns(), 2);
        assert_eq!(batch.schema().field(0).name(), "ip");
        let ip = batch.column(0).as_primitive::<UInt64Type>();
        assert_eq!(ip.value(0), 0x40_0100);
    }

    #[test]
    fn cpu_column_is_uint32() {
        let d = decoded();
        let schema = Table::Samples.schema();
        let batch = Table::Samples.build(&d, &schema, 0, 3).unwrap();
        let cpu = batch.column(3).as_primitive::<UInt32Type>();
        assert_eq!(cpu.value(0), 2);
    }
}
