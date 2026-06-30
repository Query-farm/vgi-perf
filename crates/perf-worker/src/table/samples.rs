//! `perf.samples(src)` — the profiling samples (PERF_RECORD_SAMPLE) of a
//! `perf.data` capture, one row per sample, with the raw call chain.

use crate::arrow_build::Table;

use super::PerfTableFn;

pub fn def() -> PerfTableFn {
    let mut tags = crate::meta::object_tags(
        "perf.data Samples",
        "Decode the profiling samples (PERF_RECORD_SAMPLE) of a Linux perf.data capture into rows \
         — one per sampled event (e.g. a CPU `cycles` overflow). `src` is a VARCHAR path to a \
         perf.data file or a BLOB of perf.data bytes. Columns: `time` (perf-clock nanoseconds), \
         `pid`, `tid`, `cpu`, `ip` (the raw, unsymbolicated instruction pointer as UBIGINT), \
         `period`, `event` (the event name, e.g. 'cycles'), and `callchain` (a LIST(UBIGINT) of \
         raw instruction pointers from innermost to outermost frame, in capture order). \
         Symbolication is NOT done here — join `ip` (and each callchain frame) to `mmaps` to find \
         the backing DSO, then to vgi-symbols via the build-id. Any column the capture omits \
         (no PERF_SAMPLE_CPU, no call chains, …) is NULL. Use it to find hot instruction pointers, \
         build flame data, or diff captures for CI performance-regression gates.",
        "Decode perf.data profiling samples into rows: `time`, `pid`, `tid`, `cpu`, `ip` \
         (raw UBIGINT), `period`, `event`, and `callchain` (LIST(UBIGINT) of raw IPs). `src` is a \
         path or a BLOB. IPs are unsymbolicated — join to `mmaps` + vgi-symbols to resolve. \
         Example: `SELECT ip, count(*) FROM perf.main.samples('/data/perf.data') GROUP BY ip`.",
        "perf, perf.data, perf record, samples, profiling, profiler, instruction pointer, ip, \
         callchain, call stack, stack, cycles, hotspot, flamegraph, CPU profile, performance, \
         linux perf",
        "table/samples.rs",
    );
    tags.push((
        "vgi.result_columns_md".into(),
        "| column | type | description |\n\
         |---|---|---|\n\
         | `time` | UBIGINT | Capture timestamp (perf clock, ns); NULL if not recorded. |\n\
         | `pid` | INTEGER | Process id. |\n\
         | `tid` | INTEGER | Thread id. |\n\
         | `cpu` | UINTEGER | CPU the sample was taken on; NULL if not recorded. |\n\
         | `ip` | UBIGINT | Raw (unsymbolicated) instruction pointer. |\n\
         | `period` | UBIGINT | Sample period at the time of the sample. |\n\
         | `event` | VARCHAR | Event name, e.g. `cycles`. |\n\
         | `callchain` | LIST(UBIGINT) | Raw IP frames, innermost→outermost; NULL if no call chains. |"
            .into(),
    ));
    let examples = r#"[
  {
    "description": "Decode the first ten samples of a perf.data file, with their raw call chains.",
    "sql": "SELECT time, pid, ip, callchain FROM perf.main.samples('/data/perf.data') LIMIT 10"
  },
  {
    "description": "Count samples per process to find the busiest pids.",
    "sql": "SELECT pid, count(*) AS n FROM perf.main.samples('/data/perf.data') GROUP BY pid ORDER BY n DESC"
  }
]"#;
    tags.push(("vgi.executable_examples".into(), examples.into()));
    tags.push(("vgi.example_queries".into(), examples.into()));
    PerfTableFn {
        table: Table::Samples,
        name: "samples",
        description: "Decode perf.data profiling samples (with raw call chains) into rows",
        tags,
    }
}
