//! `perf.samples(src)` — the profiling samples (PERF_RECORD_SAMPLE) of a
//! `perf.data` capture, one row per sample, with the raw call chain.

use crate::arrow_build::Table;

use super::PerfTableFn;

pub fn def() -> PerfTableFn {
    let mut tags = crate::meta::object_tags(
        "perf.data Samples",
        "Decode the profiling samples (PERF_RECORD_SAMPLE) of a Linux perf.data capture into rows \
         — one per sampled event (e.g. a CPU `cycles` overflow). `src` is a `VARCHAR` path to a \
         perf.data file or a `BLOB` of perf.data bytes. Columns: `time` (perf-clock nanoseconds), \
         `pid`, `tid`, `cpu`, `ip` (the raw, unsymbolicated instruction pointer as `UBIGINT`), \
         `period`, `event` (the event name, e.g. 'cycles'), and `callchain` (a `LIST(UBIGINT)` of \
         raw instruction pointers from innermost to outermost frame, in capture order). \
         Symbolication is NOT done here — join `ip` (and each callchain frame) to `mmaps` to find \
         the backing DSO, then to vgi-symbols via the build-id. Any column the capture omits \
         (no PERF_SAMPLE_CPU, no call chains, …) is NULL. Use it to find hot instruction pointers, \
         build flame data, or diff captures for CI performance-regression gates.",
        "Decode perf.data profiling samples into rows: `time`, `pid`, `tid`, `cpu`, `ip` \
         (raw `UBIGINT`), `period`, `event`, and `callchain` (`LIST(UBIGINT)` of raw IPs). `src` \
         is a path or a `BLOB`. IPs are unsymbolicated — join to `mmaps` + vgi-symbols to resolve. \
         Rank \
         raw IPs by frequency to find the hottest addresses in a capture.",
        "perf, perf.data, perf record, samples, profiling, profiler, instruction pointer, ip, \
         callchain, call stack, stack, cycles, hotspot, flamegraph, CPU profile, performance, \
         linux perf",
        "table/samples.rs",
        crate::meta::CAT_SAMPLES,
    );
    tags.push((
        "vgi.result_columns_schema".into(),
        crate::meta::result_columns_schema_json(&[
            (
                "time",
                "UBIGINT",
                "Capture timestamp (perf clock, nanoseconds); NULL if not recorded.",
            ),
            ("pid", "INTEGER", "Process id the sample was taken in."),
            ("tid", "INTEGER", "Thread id the sample was taken in."),
            (
                "cpu",
                "UINTEGER",
                "CPU the sample was taken on; NULL if not recorded.",
            ),
            ("ip", "UBIGINT", "Raw (unsymbolicated) instruction pointer."),
            (
                "period",
                "UBIGINT",
                "Sample period at the time of the sample.",
            ),
            ("event", "VARCHAR", "Event name, e.g. 'cycles'."),
            (
                "callchain",
                "UBIGINT[]",
                "Raw IP frames, innermost to outermost; NULL if no call chains.",
            ),
        ]),
    ));
    let blob = crate::meta::fixture_blob_src();
    let self_inclusive = crate::meta::self_vs_inclusive_sql(&blob);
    let top_ips = format!(
        "SELECT ip, count(*) AS hits FROM perf.main.samples({blob}) GROUP BY ip ORDER BY hits DESC LIMIT 10"
    );
    let deepest = format!(
        "SELECT ip, length(callchain) AS depth FROM perf.main.samples({blob}) WHERE callchain IS NOT NULL ORDER BY depth DESC, ip LIMIT 10"
    );
    let single = [
        (
            "The ten most frequent raw instruction pointers in a capture — the hottest addresses, pre-symbolication.",
            top_ips.as_str(),
        ),
        (
            "The deepest call chains: each sample's leaf ip and how many frames its stack carries.",
            deepest.as_str(),
        ),
    ];
    // The composed query references the embedded fixture BLOB twice, which would
    // push `vgi.executable_examples` past its 4,000-char agent budget (VGI417), so
    // it is published on `vgi.example_queries` (8,000) only.
    tags.push((
        "vgi.executable_examples".into(),
        crate::meta::example_queries_json(&single),
    ));
    let mut examples = single.to_vec();
    examples.push((crate::meta::SELF_VS_INCLUSIVE_DESC, self_inclusive.as_str()));
    tags.push((
        "vgi.example_queries".into(),
        crate::meta::example_queries_json(&examples),
    ));
    PerfTableFn {
        table: Table::Samples,
        name: "samples",
        description: "Decode perf.data profiling samples (with raw call chains) into rows",
        tags,
    }
}
