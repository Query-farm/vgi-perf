//! `perf.events(src)` — the recorded event attributes of a `perf.data` capture
//! (what was sampled and how).

use crate::arrow_build::Table;

use super::PerfTableFn;

pub fn def() -> PerfTableFn {
    let mut tags = crate::meta::object_tags(
        "perf.data Event Attributes",
        "Decode the recorded event attributes of a Linux perf.data capture into rows — which perf \
         events were sampled, and with what sampling policy. `src` is a VARCHAR path or a BLOB of \
         perf.data bytes. Columns: `event_id` (the primary event id, or the attribute index when \
         no ids are recorded), `name` (e.g. 'cycles', 'instructions'), `type` (the event kind: \
         'hardware', 'software', 'tracepoint', 'hw_cache', 'breakpoint', or 'dynamic_pmu'), \
         `config` (the integer config for tracepoint / dynamic-PMU events; NULL for structured \
         kinds whose `name`/`type` already identify them), `sample_period` (fixed period, if \
         period-sampled), and `sample_freq` (target samples/sec, if frequency-sampled). Use it to \
         see what a capture measured and to label samples by event.",
        "Decode perf.data event attributes into rows: `event_id`, `name`, `type` \
         (hardware/software/…), `config`, `sample_period`, `sample_freq`. `src` is a path or BLOB. \
         Example: `SELECT name, type, sample_freq FROM perf.main.events('/data/perf.data')`.",
        "perf, perf.data, event, perf_event_attr, attributes, cycles, instructions, hardware \
         event, software event, tracepoint, sample period, sample frequency, pmu",
        "table/events.rs",
    );
    tags.push((
        "vgi.result_columns_md".into(),
        "| column | type | description |\n\
         |---|---|---|\n\
         | `event_id` | UBIGINT | Primary event id, or the attribute index. |\n\
         | `name` | VARCHAR | Event name, e.g. `cycles`. |\n\
         | `type` | VARCHAR | `hardware`/`software`/`tracepoint`/`hw_cache`/`breakpoint`/`dynamic_pmu`. |\n\
         | `config` | UBIGINT | Tracepoint/dynamic-PMU config; NULL for structured kinds. |\n\
         | `sample_period` | UBIGINT | Fixed sampling period; NULL if frequency-sampled. |\n\
         | `sample_freq` | UBIGINT | Target samples/sec; NULL if period-sampled. |"
            .into(),
    ));
    tags.push((
        "vgi.executable_examples".into(),
        r#"[
  {
    "description": "List the events a capture sampled, with their kind and sampling policy.",
    "sql": "SELECT name, type, sample_period, sample_freq FROM perf.main.events('/data/perf.data')"
  }
]"#
        .into(),
    ));
    PerfTableFn {
        table: Table::Events,
        name: "events",
        description: "Decode perf.data event attributes (name, type, sampling policy) into rows",
        tags,
    }
}
