//! `perf.comms(src)` — the process/thread name records (PERF_RECORD_COMM) of a
//! `perf.data` capture.

use crate::arrow_build::Table;

use super::PerfTableFn;

pub fn def() -> PerfTableFn {
    let mut tags = crate::meta::object_tags(
        "perf.data Comm Records",
        "Decode the command/thread-name records (PERF_RECORD_COMM) of a Linux perf.data capture \
         into rows — what each pid/tid is called (e.g. 'chrome', 'postgres'). `src` is a VARCHAR \
         path or a BLOB of perf.data bytes. Columns: `pid`, `tid`, `comm` (the name), and `time` \
         (the record timestamp, or NULL if the capture records no sample-id times). Join `pid` to \
         `samples.pid` to turn a numeric pid into a human-readable process name when reporting \
         hotspots.",
        "Decode perf.data COMM records into rows: `pid`, `tid`, `comm` (name), `time`. Join \
         `pid` to `samples.pid` to label samples with a process name. `src` is a path or BLOB.",
        "perf, perf.data, comm, process name, thread name, command, pid, tid, exec, process \
         resolution",
        "table/comms.rs",
        crate::meta::CAT_CONTEXT,
    );
    tags.push((
        "vgi.result_columns_schema".into(),
        crate::meta::result_columns_schema_json(&[
            ("pid", "INTEGER", "Process id this name applies to."),
            ("tid", "INTEGER", "Thread id this name applies to."),
            ("comm", "VARCHAR", "The command / thread name."),
            ("time", "UBIGINT", "Record timestamp; NULL if not recorded."),
        ]),
    ));
    let blob = crate::meta::fixture_blob_src();
    let examples = crate::meta::example_queries_json(&[(
        "The distinct process/thread names in a capture, one row per pid — the lookup that turns a numeric pid from `samples` into a human-readable command.",
        &format!("SELECT DISTINCT pid, comm FROM perf.main.comms({blob}) ORDER BY pid"),
    )]);
    tags.push(("vgi.executable_examples".into(), examples.clone()));
    tags.push(("vgi.example_queries".into(), examples));
    PerfTableFn {
        table: Table::Comms,
        name: "comms",
        description: "Decode perf.data process/thread name (COMM) records into rows",
        tags,
    }
}
