# CLAUDE.md — vgi-perf

Guidance for working in this repo. It's a **VGI worker** (a standalone binary
DuckDB attaches over Apache Arrow IPC) that decodes the Linux `perf.data` binary
format into SQL tables under the catalog `perf`, schema `main`.

## Layout

- **`crates/perf-core`** — the pure, Arrow-free decoder. `decode(&[u8]) ->
  Decoded` turns a `perf.data` byte image into five row collections (`SampleRow`,
  `MmapRow`, `CommRow`, `EventRow`, `MetaRow`) + captured `errors`. Powered by
  `linux-perf-data` / `linux-perf-event-reader`. `fixture::synthetic_callchain()`
  builds a deterministic, spec-valid `perf.data` (a `cycles` capture with call
  chains + inline build-ids) used by tests and the SQL E2E.
- **`crates/perf-worker`** — the worker binary. Thin Arrow adapters over the core:
  - `source.rs` — the overloaded `src` (VARCHAR path | BLOB bytes) → `Decoded`.
  - `arrow_build.rs` — the five table schemas (with column comments) + batch
    builders. The `Table` enum drives schema/build dispatch.
  - `table/` — the five `TableFunction`s (`samples`/`mmaps`/`comms`/`events`/
    `meta`), all one `PerfTableFn` struct parameterized by a `Table`, plus
    `scan.rs` — the shared `PerfScan` producer with the externalized scan cursor.
  - `scalar/version.rs` — `perf_version()`.
  - `main.rs` — `Worker` setup + catalog/schema discovery metadata.

The dependency points one way: `perf-worker` → `perf-core`. Keep all `perf.data`
parsing in `perf-core` (Arrow-free, unit-testable); keep Arrow/RPC in the worker.

## Conventions (don't regress)

- **Raw IPs only.** `samples.ip` and `callchain` frames are unsymbolicated.
  Symbolication is `vgi-symbols`' job, fed by `mmaps.build_id` + the IP. Do not
  add symbol resolution here.
- **Scope:** binary `perf.data` decode only. **No** folded/collapsed-stack text
  path (committee directive — DuckDB `read_csv` covers it), no symbolication, no
  `perf.data` writing.
- **Graceful degradation:** omitted `HEADER_*` feature sections → NULL columns,
  never an error. Truncated/malformed tail records → captured in `Decoded.errors`,
  the good rows still return; the decoder never panics (proptest enforces this).
- **License is MIT** (fleet convention), in `Cargo.toml`, `LICENSE`, the catalog
  `vgi.license` tag, and per-file copyright headers where present.
- **`perf_version()`** is the conventional `<catalog>_version()` scalar.

## Platform facts

- A scalar's output type is fixed at **bind** time (`on_bind`), not per row.
- Table-function arguments are **constants** — `src` is read via
  `args.const_str(0)` (path) / `args.const_bytes(0)` (BLOB). Table functions
  reject correlated/`LATERAL`/column args; tests pass literal paths or
  `from_hex(...)` / `read_blob(...)` BLOBs, never a correlated column.
- Logs go to **stderr** (`VGI_LOG`); stdout is the Arrow-IPC channel.

## Gates (all must stay green)

```bash
cargo build --release --bin perf-worker
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features
uvx vgi-lint-check@0.37.0          # static metadata, --fail-on info (see vgi-lint.toml)
make test-sql                       # DuckDB sqllogictest E2E (subprocess/http/unix)
```

`make lint` / `make test` run the local subsets. CI (`.github/workflows/ci.yml`)
runs the same on every push/PR across an `os × transport` matrix, plus the
`vgi-lint` metadata gate.

## Tests & fixtures

- `crates/perf-core/tests/golden.rs` — golden assertions against the synthetic
  capture (in-memory) and the real `data/sleep.data`, plus the no-panic proptest.
- Worker unit tests live next to the code (`arrow_build.rs`, `table/scan.rs`):
  Arrow-boundary batch shape, projection narrowing, and scan-cursor resume.
- `test/sql/*.test` — haybarn sqllogictest E2E. They `LOAD vgi;` (not `require
  vgi`), `require-env VGI_PERF_WORKER` and `VGI_PERF_DATA`, ATTACH, and assert the
  spec examples including the `ip ↔ mmap` range join and the BLOB overload.
- Regenerate `data/callchain.data` after touching `fixture.rs`:
  `cargo run -p perf-core --example gen_fixture`.

Copyright 2026 Query Farm LLC — https://query.farm
