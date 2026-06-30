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
    );
    tags.push((
        "vgi.result_columns_md".into(),
        "| column | type | description |\n\
         |---|---|---|\n\
         | `pid` | INTEGER | Process id this name applies to. |\n\
         | `tid` | INTEGER | Thread id this name applies to. |\n\
         | `comm` | VARCHAR | The command / thread name. |\n\
         | `time` | UBIGINT | Record timestamp; NULL if not recorded. |"
            .into(),
    ));
    tags.push((
        "vgi.executable_examples".into(),
        r#"[
  {
    "description": "Map each process id to its command name.",
    "sql": "SELECT DISTINCT pid, comm FROM perf.main.comms('/data/perf.data') ORDER BY pid"
  }
]"#
        .into(),
    ));
    PerfTableFn {
        table: Table::Comms,
        name: "comms",
        description: "Decode perf.data process/thread name (COMM) records into rows",
        tags,
    }
}
