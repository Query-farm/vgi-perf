<p align="center">
  <a href="https://query.farm"><img src="https://img.shields.io/badge/Query.Farm-VGI%20worker-2b6cb0" alt="Query.Farm VGI worker"></a>
  <img src="https://img.shields.io/badge/license-MIT-blue" alt="MIT licensed">
  <img src="https://img.shields.io/badge/rust-1.90%2B-orange" alt="Rust 1.90+">
</p>

# vgi-perf — decode Linux `perf.data` profiles in SQL

A [VGI](https://query.farm) worker that decodes the Linux **`perf.data`** binary
profiling format into DuckDB tables — samples (with raw call chains), memory
mappings (with build-ids), comm records, event attributes, and capture metadata —
so SRE / observability teams can run **CI performance-regression gates** and bulk
profile analysis in SQL.

`perf script` and `perf report` are interactive, single-file CLIs; there's no way
to load a *fleet* of `perf.data` files and diff them relationally. This worker
fills that gap. The `mmaps` + `build_id` records it emits also feed
[`vgi-symbols`](https://query.farm) for native symbolication — the piece that
collapsed-stack text files have already thrown away.

```sql
INSTALL vgi FROM community; LOAD vgi;
ATTACH 'perf' (TYPE vgi, LOCATION '/path/to/perf-worker');
SET search_path = 'perf.main';

-- hottest instruction pointers (pre-symbolication), by mapped file
SELECT m.filename, count(*) AS hits
FROM perf.samples('/data/perf.data') s
JOIN perf.mmaps('/data/perf.data') m
  ON s.pid = m.pid AND s.ip BETWEEN m.addr AND m.addr + m.len
GROUP BY 1 ORDER BY 2 DESC;
```

## Functions

Every table function takes the same overloaded **`src`** argument: a **VARCHAR
path** to a `perf.data` file, or a **BLOB** of raw `perf.data` bytes (e.g. a file
already loaded into a column).

| Function | Returns |
| --- | --- |
| `samples(src)` | `time` UBIGINT, `pid` INTEGER, `tid` INTEGER, `cpu` UINTEGER, `ip` UBIGINT, `period` UBIGINT, `event` VARCHAR, `callchain` LIST(UBIGINT) |
| `mmaps(src)` | `pid` INTEGER, `tid` INTEGER, `addr` UBIGINT, `len` UBIGINT, `pgoff` UBIGINT, `filename` VARCHAR, `build_id` VARCHAR |
| `comms(src)` | `pid` INTEGER, `tid` INTEGER, `comm` VARCHAR, `time` UBIGINT |
| `events(src)` | `event_id` UBIGINT, `name` VARCHAR, `type` VARCHAR, `config` UBIGINT, `sample_period` UBIGINT, `sample_freq` UBIGINT |
| `meta(src)` | one row: `hostname` VARCHAR, `arch` VARCHAR, `nrcpus` UINTEGER, `perf_version` VARCHAR, `cmdline` LIST(VARCHAR), `total_events` UBIGINT |
| `perf_version()` | scalar VARCHAR — the worker version |

### Example queries

```sql
-- capture-wide metadata (one row)
SELECT * FROM perf.main.meta('/data/perf.data');

-- what was sampled, and how
SELECT name, type, sample_period, sample_freq FROM perf.main.events('/data/perf.data');

-- the first ten samples with their raw call chains
SELECT time, pid, ip, callchain FROM perf.main.samples('/data/perf.data') LIMIT 10;

-- every mapped DSO and its build-id (the input to vgi-symbols)
SELECT filename, build_id FROM perf.main.mmaps('/data/perf.data') WHERE build_id IS NOT NULL;

-- explode call chains into individual frames
SELECT s.time, f.frame
FROM perf.main.samples('/data/perf.data') s, unnest(s.callchain) AS f(frame);

-- decode straight from bytes (BLOB overload)
SELECT count(*) FROM perf.main.samples(read_blob('/data/perf.data'));
```

## Conventions

- **`callchain` is a LIST of raw IPs** (kernel + user frames, innermost first, in
  capture order). **Resolution is not done here** — join `mmaps.build_id` + the
  IP to `vgi-symbols`, mirroring the pprof / minidump → symbols loop.
- **`mmaps` + `comms` are first-class** because they make samples interpretable:
  which DSO an `ip` fell in (`ip BETWEEN addr AND addr + len`), and which process
  a `pid` is. Collapsed-stack text files discard them; the binary path keeps them.
- **Graceful degradation.** `perf.data` varies by kernel and `perf` build. The
  worker reads the `HEADER_*` feature sections and returns **NULL** columns for
  any a given capture omits (e.g. no `meta` host on a minimal capture).
- **Resilient decode.** A malformed or truncated tail record is captured, not
  fatal — the already-decoded rows are still returned.
- **Externalized scan state.** `perf.data` is a sequential event stream; the scan
  decodes it once and streams rows out in batches with an externalized cursor, so
  the HTTP transport resumes at batch boundaries.

## Scope

**v1** decodes the **binary `perf.data` structure only**: samples / mmaps / comms
/ events / meta. **Non-goals** (committee directive): the folded/collapsed-stack
text path (DuckDB's `read_csv` already covers it), symbolication (→ `vgi-symbols`),
and `perf.data` *writing*.

## Build & test

```bash
cargo build --release --bin perf-worker      # the worker binary (a DuckDB vgi LOCATION)
cargo test --workspace --all-features         # pure-Rust golden + proptest + Arrow-boundary tests
make test-sql                                 # DuckDB sqllogictest E2E over all transports
```

The release binary is a self-contained stdio worker DuckDB spawns; it also serves
`--http` and `--unix` transports (see [`ci/README.md`](ci/README.md)).

## Dependencies & licensing

Decoding is powered by [`linux-perf-data`](https://crates.io/crates/linux-perf-data)
(+ `linux-perf-event-reader`) by Markus Stange — the same parser behind the
Firefox profiler — both **MIT/Apache-2.0**. The worker itself is **MIT** licensed
(see [`LICENSE`](LICENSE)). The committed test fixture `data/sleep.data` is a real
`perf record sleep 1` capture vendored from the MIT-licensed `linux-perf-data`
crate (see `data/sleep.data.LICENSE-MIT`); `data/callchain.data` is a synthetic
fixture generated by `cargo run -p perf-core --example gen_fixture`.

---

The `perf` worker is part of the [Query.Farm](https://query.farm) VGI ecosystem of
DuckDB workers. Source: <https://github.com/Query-farm/vgi-perf>.

Copyright 2026 Query Farm LLC — <https://query.farm>
