//! The `perf` VGI worker (library).
//!
//! Function registration and catalog metadata live here so both entrypoints
//! share them verbatim: `main.rs` (the native binary, stdio/HTTP transport) and
//! the `perf-wasm` crate (the browser build, which serves the same `Worker`
//! over a SharedArrayBuffer byte channel instead).
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
//! The pure structural decoder lives in the `perf-core` crate; the `table/`
//! modules here are thin Arrow adapters over it. Call-chain IPs are emitted raw
//! — symbolication is the job of `vgi-symbols`, fed by `mmaps.build_id` + the
//! raw `ip`.

mod arrow_build;
mod meta;
mod source;
mod table;

use vgi::catalog::{CatSchema, CatView, CatalogModel};
use vgi::Worker;

/// Worker build version, published as the catalog `implementation_version` (an
/// agent reads it from `duckdb_databases().tags['vgi_resolved_implementation_version']`
/// without spending a query, and it can never drift from the running build).
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// The `vgi.agent_test_tasks` suite (VGI152/VGI920), self-contained via the
/// embedded [`crate::meta::FIXTURE_HEX`] BLOB so `vgi-lint simulate` can execute
/// every task against the attached worker without an external `perf.data` file.
/// Each prompt hands the analyst the same `from_hex(...)` capture and asks for a
/// single, unambiguous result with named columns. The grader `reference_sql` for
/// each task is private and lives in `vgi-agent-tests.yaml` at the repo root,
/// keyed by task name (VGI416); its results are pinned by the committed synthetic
/// fixture (see `data/callchain.data`).
fn agent_tasks_json() -> String {
    let blob = crate::meta::fixture_blob_src();
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
    crate::meta::agent_test_tasks_json(&[
        ("sample_count", &p_samples),
        ("event_policy", &p_events),
        ("event_record_total", &p_meta),
        ("mapped_files_build_ids", &p_mmaps),
        ("process_name", &p_comms),
        (
            "browse_decode_functions",
            "Without decoding any capture, which decode table functions does this worker expose \
             for reading a perf.data file? There is a browsable reference table listing them. \
             Return two columns, object_name and result_columns, one row per table function, \
             ordered by object_name.",
        ),
    ])
}

/// A no-argument, browsable reference of the decode functions this worker exposes
/// (VGI146): one row per decode table function, with the columns it returns.
/// Backed by a `VALUES` literal, so it scans entirely inside DuckDB with no
/// `perf.data` file, network, or credential (clears VGI911 for free), and — being
/// a real view rather than a table-function wrapper — is the shape VGI145/VGI146
/// want. It lets an agent discover the worker's surface before it has a capture.
fn functions_view() -> CatView {
    let mut tags = crate::meta::object_tags(
        "perf Decode Functions",
        "A browsable, no-argument reference of every decode table function the perf worker \
         exposes: one row per function with its name, kind, the category it belongs to, a \
         one-line summary, and the comma-separated list of columns it returns. Query it to \
         discover what to call and what each call returns before you have a perf.data capture in \
         hand — it takes no arguments and always returns the same rows. To actually decode a \
         capture, call the named function with a perf.data path or the raw bytes.",
        "A no-argument catalog of the perf worker's decode functions (`object_name`, \
         `object_kind`, `category`, `summary`, `result_columns`) — browse \
         `perf.main.functions` to discover what each function returns before decoding a capture.",
        "functions, catalog, reference, discovery, decode functions, samples, mmaps, comms, \
         events, meta, browsable, result columns, schema",
        "main.rs",
        crate::meta::CAT_REFERENCE,
    );
    tags.push(("domain".to_string(), "observability".to_string()));
    tags.push(("topic".to_string(), "perf-data-decode".to_string()));
    tags.push((
        "vgi.example_queries".to_string(),
        crate::meta::example_queries_json(&[(
            "List the decode table functions and the columns each one returns.",
            "SELECT object_name, result_columns FROM perf.main.functions \
             WHERE object_kind = 'table function' ORDER BY object_name",
        )]),
    ));
    CatView {
        name: "functions".to_string(),
        definition: "SELECT * FROM (VALUES \
             ('samples', 'table function', 'Profile Samples', 'One row per profiling sample (PERF_RECORD_SAMPLE) with its raw call chain.', 'time, pid, tid, cpu, ip, period, event, callchain'), \
             ('mmaps', 'table function', 'Address Space', 'One row per memory mapping (MMAP/MMAP2) with the build-id that makes a raw ip interpretable.', 'pid, tid, addr, len, pgoff, filename, build_id'), \
             ('comms', 'table function', 'Capture Context', 'One row per command/thread-name record (COMM).', 'pid, tid, comm, time'), \
             ('events', 'table function', 'Capture Context', 'One row per recorded event attribute — what was sampled and how.', 'event_id, name, type, config, sample_period, sample_freq'), \
             ('meta', 'table function', 'Capture Context', 'A single row of capture-wide HEADER_* metadata.', 'hostname, arch, nrcpus, perf_version, cmdline, total_events')) \
             AS t(object_name, object_kind, category, summary, result_columns)"
            .to_string(),
        comment: Some(
            "Browsable reference of the perf worker's decode functions — one row per object, with \
             the columns it returns. Browse it to discover the surface before decoding a capture."
                .to_string(),
        ),
        tags,
        column_comments: vec![
            (
                "object_name".to_string(),
                "The function name to call, e.g. `samples`, `mmaps`, `meta`.".to_string(),
            ),
            (
                "object_kind".to_string(),
                "The kind of object — always `table function` here (each decodes a capture into \
                 rows)."
                    .to_string(),
            ),
            (
                "category".to_string(),
                "The navigation category the object belongs to (Profile Samples, Address Space, \
                 Capture Context)."
                    .to_string(),
            ),
            (
                "summary".to_string(),
                "A one-line description of what the function decodes / returns.".to_string(),
            ),
            (
                "result_columns".to_string(),
                "Comma-separated list of the columns the function returns.".to_string(),
            ),
        ],
    }
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
                 into a column — and there is one per record kind: samples, memory mappings, comm \
                 records, event attributes, and capture metadata. Reach for it for bulk, \
                 relational analysis of Linux perf profiles rather than single-file interactive \
                 inspection."
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
                 Attach the worker, then point `search_path` at its schema:\n\n\
                 ```sql\n\
                 ATTACH 'perf' (TYPE vgi, LOCATION '/path/to/perf-worker');\n\
                 SET search_path = 'perf.main';\n\
                 ```\n\n\
                 You then call a decode function with a `perf.data` path (or the raw capture \
                 bytes) and analyze the rows in ordinary SQL. The marquee move is an \
                 address-range join of each sample's raw `ip` to the memory mapping it fell in, \
                 ranking the hottest modules — that query, and others, are shipped ready-to-run in \
                 this worker's example queries and per-function documentation. Browse the \
                 `perf.main.functions` reference table to see every function and the columns it \
                 returns before you have a capture in hand.\n\n\
                 **What you get.** Each decode function takes one source argument — a path to a \
                 `perf.data` file, or the raw capture bytes already loaded into a column — and \
                 streams the decoded records out as ordinary typed rows. The profiling samples \
                 (with their raw call chains), the process address-space mappings, the recorded \
                 event attributes, the process/thread names, and the capture-wide host metadata \
                 are each a separate function. Ranges, IDs, and timestamps come through as real \
                 integer columns so they join and aggregate directly in SQL.\n\n\
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
                 rather than failing the query."
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
        // Publish the worker build version as the catalog implementation_version
        // (VGI328). An agent reads it from
        // `duckdb_databases().tags['vgi_resolved_implementation_version']` without
        // spending a query, and it can never drift from the running binary — which
        // is why there is no `perf_version()` scalar.
        implementation_version: Some(version().to_string()),
        schemas: vec![CatSchema {
            name: "main".to_string(),
            path: Vec::new(),
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
                     by address and to vgi-symbols by build-id."
                        .to_string(),
                ),
                (
                    "vgi.doc_md".to_string(),
                    "## perf — main\n\n\
                     The single schema for the `perf` worker: the decode functions that turn a \
                     Linux profiling capture into typed SQL rows. They are grouped into three \
                     areas:\n\n\
                     - **Profile samples** — the per-event rows with their raw, unsymbolicated \
                     call chains.\n\
                     - **Address space** — the process memory mappings and build-ids that make a \
                     raw instruction pointer interpretable.\n\
                     - **Capture context** — process/thread names, the sampled event attributes, \
                     and the capture-wide host metadata.\n\n\
                     Every decode function takes a `perf.data` path or the raw capture bytes."
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
  {"description": "Hottest mapped files via an ip-to-mmap address-range join.", "sql": "SELECT m.filename, count(*) AS hits FROM perf.main.samples('/data/perf.data') s JOIN perf.main.mmaps('/data/perf.data') m ON s.pid = m.pid AND s.ip BETWEEN m.addr AND m.addr + m.len GROUP BY 1 ORDER BY 2 DESC"}
]"#
                        .to_string(),
                ),
            ],
            views: vec![functions_view()],
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

/// The catalog name DuckDB sees in `ATTACH 'perf' (TYPE vgi, …)`. Defaults to
/// `perf`, but honors an explicit override so a test harness can rename it.
/// Also exports the variable so downstream SDK code observes the same default.
pub fn catalog_name() -> String {
    if std::env::var_os("VGI_WORKER_CATALOG_NAME").is_none() {
        std::env::set_var("VGI_WORKER_CATALOG_NAME", "perf");
    }
    std::env::var("VGI_WORKER_CATALOG_NAME").unwrap_or_else(|_| "perf".to_string())
}

/// Build a fully-registered worker: every table function plus the catalog
/// metadata. Callers choose the transport — `run()` natively,
/// `serve_reader_writer()` in the browser.
pub fn build_worker() -> Worker {
    let name = catalog_name();
    let mut worker = Worker::new();
    table::register(&mut worker);
    worker.set_catalog(catalog_metadata(&name));
    worker
}
