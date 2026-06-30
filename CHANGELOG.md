# Changelog

All notable changes to the `vgi-perf` worker are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
