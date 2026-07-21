//! `perf.meta(src)` — a single row of capture-wide metadata from the `HEADER_*`
//! feature sections of a `perf.data` capture.

use crate::arrow_build::Table;

use super::PerfTableFn;

pub fn def() -> PerfTableFn {
    let mut tags = crate::meta::object_tags(
        "perf.data Capture Metadata",
        "Decode the capture-wide metadata of a Linux perf.data file into a single row, read from \
         its HEADER_* feature sections. `src` is a `VARCHAR` path or a `BLOB` of perf.data bytes. \
         Columns: `hostname`, `arch` (e.g. 'x86_64'), `nrcpus` (CPUs available on the capture \
         host), `perf_version` (the perf tool version), `cmdline` (the `perf record` command line \
         as a `LIST(VARCHAR)`), and `total_events` (the count of event records in the data \
         section). \
         perf.data varies by kernel and perf build — any feature section a given capture omits \
         comes back NULL (graceful degradation). Use it to label a fleet of captures by host / \
         arch / tool version and to sanity-check event counts.",
        "Decode perf.data capture metadata into one row: `hostname`, `arch`, `nrcpus`, \
         `perf_version`, `cmdline` (`LIST(VARCHAR)`), `total_events`. Omitted HEADER_* sections \
         are NULL. `src` is a path or `BLOB`.",
        "perf, perf.data, metadata, header, hostname, arch, architecture, nrcpus, perf version, \
         cmdline, command line, total events, capture info",
        "table/meta.rs",
        crate::meta::CAT_CONTEXT,
    );
    tags.push((
        "vgi.result_columns_schema".into(),
        crate::meta::result_columns_schema_json(&[
            ("hostname", "VARCHAR", "Capture host name; NULL if absent."),
            (
                "arch",
                "VARCHAR",
                "Machine architecture, e.g. 'x86_64'; NULL if absent.",
            ),
            (
                "nrcpus",
                "UINTEGER",
                "CPUs available on the capture host; NULL if absent.",
            ),
            (
                "perf_version",
                "VARCHAR",
                "perf tool version; NULL if absent.",
            ),
            (
                "cmdline",
                "VARCHAR[]",
                "The 'perf record' command line, split into arguments; NULL if absent.",
            ),
            (
                "total_events",
                "UBIGINT",
                "Count of event records in the data section.",
            ),
        ]),
    ));
    let blob = crate::meta::fixture_blob_src();
    let examples = crate::meta::example_queries_json(&[(
        "Read the capture-wide metadata of a perf.data file (exactly one row): host, arch, tool version, and the total event-record count.",
        &format!(
            "SELECT hostname, arch, nrcpus, perf_version, cmdline, total_events FROM perf.main.meta({blob})"
        ),
    )]);
    tags.push(("vgi.executable_examples".into(), examples.clone()));
    tags.push(("vgi.example_queries".into(), examples));
    PerfTableFn {
        table: Table::Meta,
        name: "meta",
        description: "Decode perf.data capture-wide metadata (host, arch, cmdline, …) into one row",
        tags,
    }
}
