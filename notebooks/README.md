# Perf decoding notebooks

[01 · Self samples and inclusive stacks](01_self_samples_and_inclusive_stacks.ipynb)
uses a small **synthetic** binary capture to distinguish exclusive self samples,
distinct samples containing an object, and frame occurrences. One stack can
contribute to several objects' inclusive counts. A depth experiment removes caller
frames while keeping the sample population fixed.

## Run from this directory

Install [uv](https://docs.astral.sh/uv/) and use Python 3.13 or 3.14:

```sh
cd notebooks
uv sync --locked
uv run --locked python execute.py
uv run --locked python -m ipykernel install --user --name vgi-perf-notebooks --display-name 'Python (vgi-perf notebooks)'
```

Select that kernel in Jupyter. Start the kernel in this directory and Run All. Only
this directory is required; no worker source tree, root environment or local
worker build is used. Initial setup needs internet access for locked dependencies,
the VGI extension and the native worker release. The tested platform is macOS ARM64.

The expanded SQL connects through Haybarn 2.0.0rc11:

```sql
INSTALL vgi FROM community;
LOAD vgi;
ATTACH 'perf' (TYPE vgi, LOCATION 'github-auto://Query-farm/vgi-perf@v0.4.0');
```

The v0.4.0 tag resolves to `75c9774e39bc9df0e7c54d793963d3cb3a71e776`.
The extension downloads and caches the platform release. To upgrade, change the
literal tag and rerun the default, experiment and invariants. Source checks use the
repository's own unchanged dependencies; notebook dependencies have their own lock.

## One-parameter experiment

`MAX_FRAMES=2` retains the fixture's complete callchains. Keep only their first
frame in a separate executed copy:

```sh
uv run --locked python execute.py --parameters '{"MAX_FRAMES":1}' --output-dir validation/depth-1
```

This changes the analysis after decoding, not the source capture. Self counts and
the raw fingerprint must stay unchanged. Inclusive counts and frame occurrences
cannot increase. Reset to 2 for the default. The runner executes in a fresh kernel,
replaces the saved default only after success, and rejects overrides without a
separate destination or unknown parameter names. Experiment copies carry their
fixtures, assets, environment and documentation. `validation/` and `artifacts/`
are ignored; embedded successful notebook outputs are committed.

## Fixture and analytical limits

`fixtures/callchain.data` is copied byte-for-byte from the worker's deterministic
MIT-licensed test fixture at source revision
`312b748d5453cda08e4e5a20db2fba2ea4e63207`; its generator is
`crates/perf-core/src/fixture.rs::synthetic_callchain`. `fixtures/provenance.json`
records the hash, byte size and source. The notebook verifies that hash before
starting the worker. This is deliberately synthetic and has no capture timestamp
or real workload provenance. It contains three samples, two static mappings and
one process-name record. No API requests or live-data mode are needed.

Mapping uses PID plus the half-open address interval `[addr, addr+len)` with wide
endpoint arithmetic. Nonoverlap and one process name per PID are verified for this
fixture. Mapping timestamps are unavailable, so this join cannot resolve address
or PID reuse in a general real-world capture. Unmapped frames are retained.

Raw IPs are not symbolicated functions. Repeated frames in one object do not prove
recursion. Inclusive counts overlap across objects and are not an exclusive
percentage partition. Frame occurrences weight repeated appearances. Period values
are not wall-clock seconds; missing event names/configuration and header metadata
remain unknown. `meta.total_events` counts all capture records. Finally, the
decoder's internal error collection is not exposed in these SQL tables: successful
SQL does not certify a complete, error-free real capture.

Both charts are generated from materialized decoded rows. The notebook checks
mapping ambiguity, population and frame reconciliation, first-frame/self agreement
for this fixture, inclusive bounds and raw-data fingerprint stability. See
[STANDARD.md](STANDARD.md) for presentation conventions.

## Validation and unchanged source baseline

The default and a notebooks-only `MAX_FRAMES=1` experiment execute with the native
published release. Repository tests pass: 16 Rust tests, 121 stdio / 135 HTTP /
121 Unix SQL assertions (seven cases per transport), clippy, format and rustdoc.
The unchanged source's metadata gate with `vgi-lint-check==2.5.0` reports 98/100,
zero errors and three VGI417 warnings: composed `vgi.example_queries` exceed the
4,000-character agent context bound on comms, mmaps and samples (5,560 / 6,825 /
5,917 characters). Behavioral verification runs, but the configured `--fail-on info`
gate exits 2. No worker code or source metadata is changed by these notebooks.
