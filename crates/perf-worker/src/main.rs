//! The `perf` VGI worker.
//!
//! A standalone binary that DuckDB launches and talks to over Apache Arrow IPC
//! (`ATTACH 'perf' (TYPE vgi, LOCATION '…')`). It decodes the Linux `perf.data`
//! binary format into SQL tables under the catalog `perf`, schema `main`:
//!
//! ```sql
//! ATTACH 'perf' (TYPE vgi, LOCATION './target/release/perf-worker');
//! SET search_path = 'perf.main';
//!
//! -- hottest instruction pointers (pre-symbolication), by mapped file
//! SELECT m.filename, count(*) AS hits
//! FROM perf.samples('/data/perf.data') s
//! JOIN perf.mmaps('/data/perf.data') m
//!   ON s.pid = m.pid AND s.ip BETWEEN m.addr AND m.addr + m.len
//! GROUP BY 1 ORDER BY 2 DESC;
//! ```
//!
//! The pure structural decoder lives in the `perf-core` crate; the `scalar/` and
//! `table/` modules here are thin Arrow adapters over it. Call-chain IPs are
//! emitted raw — symbolication is the job of `vgi-symbols`, fed by
//! `mmaps.build_id` + the raw `ip`.

mod arrow_build;
mod meta;
mod scalar;
mod source;
mod table;

use vgi::catalog::{CatSchema, CatalogModel};
use vgi::Worker;

/// Worker version string, surfaced by `perf_version()`.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Hex of the committed synthetic `data/callchain.data` fixture (a tiny, valid
/// `cycles` perf.data capture: 3 samples with call chains, 2 mmap2 records with
/// inline build-ids, 1 COMM, 1 event attr, empty feature bitmap). Embedded so the
/// `vgi.agent_test_tasks` suite (VGI152/VGI920) is self-contained — the analyst
/// decodes it via `from_hex(...)` as a BLOB `src`, no external file needed.
const FIXTURE_HEX: &str = "50455246494c45326800000000000000400000000000000068000000000000004000000000000000a800000000000000a00100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000400000000000000000000000a00f000000000000a70100000000000000000000000000000000000000000000000000000000000000000000000000000300000002001800d2040000d20400006c6f6f70000000000a00000002405800d2040000d204000000004000000000000010000000000000000000000000000014000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa00000000000000002f7573722f62696e2f6c6f6f700000000a00000002406000d2040000d204000000000000007f00000000200000000000000000000000000014000000bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb00000000000000002f7573722f6c69622f6c6962632e736f2e3600000000000009000000020048000001400000000000d2040000d2040000e8030000000000000200000000000000a00f000000000000020000000000000000014000000000005000400000000000090000000200480000010000007f0000d2040000d2040000d0070000000000000200000000000000a00f000000000000020000000000000000010000007f0000000140000000000009000000020040000001400000000000d2040000d2040000b80b0000000000000300000000000000a00f00000000000001000000000000000001400000000000";

/// The `vgi.agent_test_tasks` suite (VGI152/VGI920), self-contained via the
/// embedded [`FIXTURE_HEX`] BLOB so `vgi-lint simulate` can execute every task's
/// `reference_sql` against the attached worker without an external `perf.data`
/// file. Each prompt hands the analyst the same `from_hex(...)` capture and asks
/// for a single, unambiguous result with named columns; the reference results are
/// pinned by the committed synthetic fixture (see `data/callchain.data`).
fn agent_tasks_json() -> String {
    let blob = format!("from_hex('{FIXTURE_HEX}')");
    let p_samples = format!(
        "A Linux perf.data capture is available as the BLOB {blob}. How many profiling samples \
         does it contain? Return a single row with one column named n."
    );
    let p_events = format!(
        "A Linux perf.data capture is available as the BLOB {blob}. Which event kind was sampled \
         and at what fixed sampling period? Return a single row with columns type and \
         sample_period."
    );
    let p_meta = format!(
        "A Linux perf.data capture is available as the BLOB {blob}. Its capture-wide metadata \
         reports a single grand total of every event record in the data section (samples, mmap, \
         and comm records combined) — this is NOT the number of sampled event *types*. Read that \
         metadata total. Return a single row with one column named total_events."
    );
    let p_mmaps = format!(
        "A Linux perf.data capture is available as the BLOB {blob}. List every memory-mapped file \
         and its build-id, ordered by filename ascending. Return columns filename and build_id."
    );
    let p_comms = format!(
        "A Linux perf.data capture is available as the BLOB {blob}. What command (process) name \
         was recorded for the process in this capture? Return a single row with one column named \
         comm."
    );
    let r_samples = format!("SELECT count(*) AS n FROM perf.main.samples({blob})");
    let r_events = format!("SELECT type, sample_period FROM perf.main.events({blob})");
    let r_meta = format!("SELECT total_events FROM perf.main.meta({blob})");
    let r_mmaps =
        format!("SELECT filename, build_id FROM perf.main.mmaps({blob}) ORDER BY filename");
    let r_comms = format!("SELECT comm FROM perf.main.comms({blob})");
    crate::meta::agent_test_tasks_json(&[
        (
            "worker_version",
            "What version of the perf worker is running? Return a single row with one column \
             named version.",
            "SELECT perf.main.perf_version() AS version",
        ),
        ("sample_count", &p_samples, &r_samples),
        ("event_policy", &p_events, &r_events),
        ("event_record_total", &p_meta, &r_meta),
        ("mapped_files_build_ids", &p_mmaps, &r_mmaps),
        ("process_name", &p_comms, &r_comms),
    ])
}

/// Catalog + schema metadata (description, provenance, discovery tags) surfaced
/// to DuckDB and the `vgi-lint` metadata-quality linter. The table functions
/// themselves are registered via [`table::register`]; this adds catalog/schema
/// comments and tags.
fn catalog_metadata(name: &str) -> CatalogModel {
    CatalogModel {
        name: name.to_string(),
        comment: Some(
            "Decode the Linux perf.data binary profiling format into SQL tables: samples (with \
             raw call chains), memory mappings (with build-ids), comm records, event attributes, \
             and capture metadata."
                .to_string(),
        ),
        tags: vec![
            (
                "vgi.title".to_string(),
                "perf.data Profile Decoder".to_string(),
            ),
            (
                "vgi.keywords".to_string(),
                crate::meta::keywords_json(
                    "perf, perf.data, linux perf, perf record, profiling, profiler, samples, \
                     call chain, callchain, instruction pointer, mmap, build id, symbolication, \
                     cycles, hotspot, flamegraph, performance regression, observability, SRE",
                ),
            ),
            (
                "vgi.doc_llm".to_string(),
                "Decode Linux perf.data profiling captures into relational tables you can query in \
                 DuckDB. A perf.data file is a binary stream of profiling records; this worker \
                 shreds it into ordinary rows so you can rank hot instruction pointers, diff \
                 captures, and gate CI on performance regressions across a whole fleet of captures \
                 without leaving SQL. Instruction pointers and call-chain frames come out RAW \
                 (unsymbolicated) by design: range-join an address to the memory mapping it fell \
                 in to find the backing shared object, then feed that mapping's build-id to \
                 vgi-symbols for native symbolication. Each decode function takes one source \
                 argument — a path to a perf.data file, or the raw capture bytes already loaded \
                 into a column. List the schema to discover the individual functions. Reach for \
                 it for bulk, relational analysis of Linux perf profiles rather than single-file \
                 interactive inspection."
                    .to_string(),
            ),
            (
                "vgi.doc_md".to_string(),
                "# perf — Decode Linux `perf.data` Profiles in SQL\n\n\
                 **Load a fleet of Linux `perf.data` captures and analyze them relationally in \
                 DuckDB.** `perf script` and `perf report` are interactive, single-file CLIs; this \
                 worker turns the binary `perf.data` format into ordinary SQL tables so SRE and \
                 observability teams can run **CI performance-regression gates** and bulk profile \
                 analysis — diffing captures, ranking hot instruction pointers, and joining \
                 samples to the modules they fell in — without leaving SQL.\n\n\
                 Attach the worker and query a capture by path or by raw bytes:\n\n\
                 ```sql\n\
                 ATTACH 'perf' (TYPE vgi, LOCATION '/path/to/perf-worker');\n\
                 SET search_path = 'perf.main';\n\n\
                 -- hottest instruction pointers (pre-symbolication), by mapped file\n\
                 SELECT m.filename, count(*) AS hits\n\
                 FROM perf.samples('/data/perf.data') s\n\
                 JOIN perf.mmaps('/data/perf.data') m\n\
                 \x20 ON s.pid = m.pid AND s.ip BETWEEN m.addr AND m.addr + m.len\n\
                 GROUP BY 1 ORDER BY 2 DESC;\n\
                 ```\n\n\
                 **What you get.** Each decode function takes one source argument — a path to a \
                 `perf.data` file, or the raw capture bytes already loaded into a column — and \
                 streams the decoded records out as ordinary typed rows. The profiling samples \
                 (with their raw call chains), the process address-space mappings, the recorded \
                 event attributes, the process/thread names, and the capture-wide host metadata \
                 are each a separate function; list the schema to discover them. Ranges, IDs, and \
                 timestamps come through as real integer columns so they join and aggregate \
                 directly in SQL.\n\n\
                 **Raw IPs, by design.** Instruction pointers and call-chain frames are emitted \
                 **unsymbolicated**. The `mmaps` records — with their `build_id`s — are what make \
                 a raw `ip` interpretable: range-join `samples.ip BETWEEN m.addr AND m.addr + \
                 m.len` to find the backing DSO, then feed `build_id` + the IP to **`vgi-symbols`** \
                 for native symbolication, mirroring the pprof / minidump → symbols loop. \
                 Collapsed-stack text files throw the mmap + build-id information away; the binary \
                 `perf.data` path keeps it.\n\n\
                 **Scope.** v1 decodes the **binary** `perf.data` record structure only. The \
                 folded/collapsed-stack text path is intentionally \
                 out of scope (DuckDB's `read_csv` already covers it), as is symbolication \
                 (→ vgi-symbols) and `perf.data` *writing*. `perf.data` varies by kernel and perf \
                 build; any `HEADER_*` feature section a capture omits comes back as NULL columns \
                 rather than failing the query.\n\n\
                 The `perf` worker is open source and part of the [Query.Farm](https://query.farm) \
                 VGI ecosystem of DuckDB workers — see the [source repository on \
                 GitHub](https://github.com/Query-farm/vgi-perf) for the full column reference and \
                 examples."
                    .to_string(),
            ),
            (
                "vgi.agent_test_tasks".to_string(),
                agent_tasks_json(),
            ),
            ("vgi.author".to_string(), "Query.Farm".to_string()),
            (
                "vgi.copyright".to_string(),
                "Copyright 2026 Query Farm LLC - https://query.farm".to_string(),
            ),
            ("vgi.license".to_string(), "MIT".to_string()),
            (
                "vgi.support_contact".to_string(),
                "https://github.com/Query-farm/vgi-perf/issues".to_string(),
            ),
            (
                "vgi.support_policy_url".to_string(),
                "https://github.com/Query-farm/vgi-perf/blob/main/README.md".to_string(),
            ),
        ],
        source_url: Some("https://github.com/Query-farm/vgi-perf".to_string()),
        schemas: vec![CatSchema {
            name: "main".to_string(),
            comment: Some(
                "perf.data decode functions: samples, mmaps, comms, events, meta.".to_string(),
            ),
            tags: vec![
                ("vgi.title".to_string(), "perf — main".to_string()),
                (
                    "vgi.categories".to_string(),
                    crate::meta::categories_json(),
                ),
                (
                    "vgi.keywords".to_string(),
                    crate::meta::keywords_json(
                        "perf, perf.data, samples, mmaps, comms, events, meta, callchain, \
                         instruction pointer, build id, profiling, profiler, performance",
                    ),
                ),
                // VGI123 classifying tags (bare keys: domain/category/topic).
                ("domain".to_string(), "observability".to_string()),
                ("category".to_string(), "profiling".to_string()),
                ("topic".to_string(), "perf-data-decode".to_string()),
                (
                    "vgi.doc_llm".to_string(),
                    "The single schema for the perf worker: the perf.data decode functions. Each \
                     takes a perf.data path or the raw capture bytes and returns typed rows — the \
                     profiling samples with raw call chains, the address-space mappings and their \
                     build-ids, the process/thread names, the sampled event attributes, and the \
                     capture-wide host metadata. Raw instruction pointers range-join to a mapping \
                     by address and to vgi-symbols by build-id. List the schema to see each \
                     function."
                        .to_string(),
                ),
                (
                    "vgi.doc_md".to_string(),
                    "## perf — main\n\n\
                     The single schema for the `perf` worker: the decode functions that turn a \
                     Linux profiling capture into typed SQL rows. They are grouped into four \
                     areas:\n\n\
                     - **Profile samples** — the per-event rows with their raw, unsymbolicated \
                     call chains.\n\
                     - **Address space** — the process memory mappings and build-ids that make a \
                     raw instruction pointer interpretable.\n\
                     - **Capture context** — process/thread names, the sampled event attributes, \
                     and the capture-wide host metadata.\n\
                     - **Diagnostics** — operational introspection of the running worker.\n\n\
                     Every decode function takes a `perf.data` path or the raw capture bytes. List \
                     the schema to discover the individual functions."
                        .to_string(),
                ),
                // VGI506 representative example queries for the schema. These scan
                // an external perf.data file, so they are executed by the haybarn
                // E2E (test/sql/*.test) rather than the lint sandbox (the VGI9xx
                // execution rules are disabled in vgi-lint.toml).
                (
                    "vgi.example_queries".to_string(),
                    r#"[
  {"description": "Read the capture-wide metadata (one row).", "sql": "SELECT * FROM perf.main.meta('/data/perf.data')"},
  {"description": "List the events the capture sampled and how.", "sql": "SELECT name, type, sample_period, sample_freq FROM perf.main.events('/data/perf.data')"},
  {"description": "The first ten samples with their raw call chains.", "sql": "SELECT time, pid, ip, callchain FROM perf.main.samples('/data/perf.data') LIMIT 10"},
  {"description": "Every mapped DSO and its build-id (the vgi-symbols input).", "sql": "SELECT filename, build_id FROM perf.main.mmaps('/data/perf.data') WHERE build_id IS NOT NULL"},
  {"description": "Hottest mapped files via an ip-to-mmap address-range join.", "sql": "SELECT m.filename, count(*) AS hits FROM perf.main.samples('/data/perf.data') s JOIN perf.main.mmaps('/data/perf.data') m ON s.pid = m.pid AND s.ip BETWEEN m.addr AND m.addr + m.len GROUP BY 1 ORDER BY 2 DESC"},
  {"description": "The running worker version.", "sql": "SELECT perf.main.perf_version()"}
]"#
                        .to_string(),
                ),
            ],
            views: Vec::new(),
            macros: Vec::new(),
            // All five table functions take a `src` argument, so none is a
            // parameterless catalog table (VGI311); they are registered as table
            // functions via `table::register` and discovered by their own
            // FunctionMetadata.
            tables: Vec::new(),
        }],
        ..Default::default()
    }
}

fn main() {
    // Logs MUST go to stderr — stdout is the Arrow-IPC channel.
    let _ = env_logger::Builder::from_env(env_logger::Env::default().filter_or("VGI_LOG", "info"))
        .format_timestamp_millis()
        .try_init();

    // The catalog name DuckDB sees in `ATTACH 'perf' (TYPE vgi, …)`. Default to
    // `perf`, but honor an explicit override so a test harness can rename it.
    if std::env::var_os("VGI_WORKER_CATALOG_NAME").is_none() {
        std::env::set_var("VGI_WORKER_CATALOG_NAME", "perf");
    }
    let catalog_name =
        std::env::var("VGI_WORKER_CATALOG_NAME").unwrap_or_else(|_| "perf".to_string());

    let mut worker = Worker::new();
    scalar::register(&mut worker);
    table::register(&mut worker);
    worker.set_catalog(catalog_metadata(&catalog_name));
    worker.run();
}
