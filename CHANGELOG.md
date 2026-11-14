# Changelog

All notable changes to the `vgi-perf` worker are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Composed example queries combining the decode functions (published on
  `samples` / `mmaps` and the schema): hot files labelled by process name
  (samples + mmaps + comms), self vs inclusive samples per DSO (unnested call
  chains + mmaps), and the (build_id, file offset) keys for vgi-symbols. New
  agent test task `loop_frames_outside_leaf`.

### Fixed

- The ip-to-mapping range join in docs, examples, and tests used
  `ip BETWEEN addr AND addr + len`, which also claims the first byte past a
  mapping (double-attributing an ip on the boundary of adjacent mappings).
  Mappings are half-open: `ip >= addr AND ip < addr + len`.
- README listed a `perf_version()` scalar that the worker no longer exposes
  (the version is the catalog `implementation_version`).
- Schema-level example queries no longer use a bare `SELECT *` and have a
  stable `ORDER BY`.

## [0.4.0] - 2026-11-12

### Changed

- Ported to VGI protocol 3.3 (vgi.v3 3.3.0): vgi 0.51.0, vgi-rpc 0.32.4.
  Requires the current community `vgi` extension (3.3); 3.2 extensions are
  incompatible. No worker-visible change (the worker has no catalog tables).

## [0.3.0] - 2026-11-09

### Changed

- Ported to VGI protocol 3.2 (vgi.v3 3.2.0, filtered scan planning): vgi 0.50.0,
  vgi-rpc 0.32.3. Requires the haybarn 2.0 rc11+ community `vgi` extension (3.2).

### Fixed

- Catalog metadata now passes vgi-lint-check 2.2: added a one-sentence
  `vgi.subtitle` (VGI148) and shortened the `main` schema title from the
  headline-style "perf — main" to "perf.data Decoders" (VGI147).

## [0.2.0] - 2026-11-06

### Changed

- Ported to VGI protocol 3.1 (vgi.v3): vgi 0.49.2, vgi-rpc 0.32.3, arrow 59.2.
  Requires haybarn 2.0 (the community `vgi` extension that speaks vgi.v3).

## [0.1.0]

### Added

- Initial release. Decodes the Linux `perf.data` binary format into SQL under the
  `perf` catalog, schema `main`, over Apache Arrow:
  - `samples(src)` — profiling samples with raw `callchain` (`LIST(UBIGINT)`).
  - `mmaps(src)` — memory mappings with `build_id` (feeds `vgi-symbols`).
  - `comms(src)` — process/thread name records.
  - `events(src)` — recorded event attributes.
  - `meta(src)` — one row of capture-wide metadata (graceful `NULL` degradation).
  - `perf_version()` — scalar version string.
- Overloaded `src` argument: a VARCHAR path or a BLOB of `perf.data` bytes.
- Externalized byte-stream scan cursor (batch-boundary HTTP resume).
