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
                "Decode Linux perf.data profiling captures into relational tables. Exposes \
                 table functions samples(src), mmaps(src), comms(src), events(src), and meta(src) \
                 — where src is a VARCHAR path to a perf.data file or a BLOB of perf.data bytes. \
                 samples gives time/pid/tid/cpu/ip/period/event plus a callchain LIST(UBIGINT) of \
                 raw instruction pointers; mmaps gives the memory mappings (addr/len/filename/ \
                 build_id) that make a raw ip interpretable; comms maps pid→process name; events \
                 lists the sampled event attributes; meta is one row of capture-wide metadata. \
                 Instruction pointers are RAW — join ip to mmaps by address range and feed \
                 build_id to vgi-symbols for symbolication. Use for CI performance-regression \
                 gates and bulk relational analysis of a fleet of perf.data files."
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
                 **Function surface.** Five table functions, each taking a `src` that is a VARCHAR \
                 path to a `perf.data` file or a BLOB of its bytes: `samples(src)` \
                 (time, pid, tid, cpu, `ip`, period, event, and `callchain` — a LIST(UBIGINT) of \
                 raw IP frames), `mmaps(src)` (pid, tid, addr, len, pgoff, filename, `build_id`), \
                 `comms(src)` (pid, tid, comm, time), `events(src)` (event_id, name, type, config, \
                 sample_period, sample_freq), and `meta(src)` (one row: hostname, arch, nrcpus, \
                 perf_version, cmdline, total_events). Plus a `perf_version()` scalar.\n\n\
                 **Raw IPs, by design.** Instruction pointers and call-chain frames are emitted \
                 **unsymbolicated**. The `mmaps` records — with their `build_id`s — are what make \
                 a raw `ip` interpretable: range-join `samples.ip BETWEEN m.addr AND m.addr + \
                 m.len` to find the backing DSO, then feed `build_id` + the IP to **`vgi-symbols`** \
                 for native symbolication, mirroring the pprof / minidump → symbols loop. \
                 Collapsed-stack text files throw the mmap + build-id information away; the binary \
                 `perf.data` path keeps it.\n\n\
                 **Scope.** v1 decodes the **binary** `perf.data` structure only (samples / mmaps \
                 / comms / events / meta). The folded/collapsed-stack text path is intentionally \
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
                    "perf.data decode functions: samples(src), mmaps(src), comms(src), \
                     events(src), and meta(src). src is a perf.data path or BLOB. Raw IPs join to \
                     mmaps by address range and to vgi-symbols by build-id."
                        .to_string(),
                ),
                (
                    "vgi.doc_md".to_string(),
                    "The single schema for the `perf` worker. It holds the `perf.data` decode \
                     table functions — `samples`, `mmaps`, `comms`, `events`, `meta` — plus the \
                     `perf_version` scalar. Each table function takes a `src` perf.data path or \
                     BLOB."
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
