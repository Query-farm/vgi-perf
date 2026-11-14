# Self samples and inclusive stacks

[01 · One sample can appear in several inclusive totals](01_self_samples_and_inclusive_stacks.ipynb)
decodes an explicitly **synthetic** binary capture and distinguishes self samples,
distinct samples containing an object, and frame occurrences. Returned names,
event attributes and the function reference establish the population and units
before a stack-depth experiment removes caller context.
[Open in Colab](https://colab.research.google.com/github/Query-farm/vgi-perf/blob/main/notebooks/01_self_samples_and_inclusive_stacks.ipynb).

Choose **Runtime → Run all** in Colab. The base shared package preserves compatible
preloaded kernel/data/plotting libraries and retrieves immutable SHA256-verified
companions. Local execution uses a separate lock with the `[runner]` extra:

```sh
uv sync --locked
uv run --locked vgi-notebooks
uv run --locked vgi-notebooks 01_self_samples_and_inclusive_stacks.ipynb --parameters '{"MAX_FRAMES":1}' --output-dir artifacts/depth-1
```

Run from `notebooks/` without the worker checkout. For Jupyter, select
`.venv/bin/python`. The runner uses a temporary kernel, keeps successful defaults
intact on failure, and rejects unknown parameters or parameter overrides without
a separate destination. If an older bootstrap replaced kernel libraries, save
your work and open the current notebook in a fresh runtime.

Visible SQL starts the public native release:

```sql
INSTALL vgi FROM community;
LOAD vgi;
ATTACH 'perf' (TYPE vgi, LOCATION 'github-auto://Query-farm/vgi-perf@v0.4.0');
```

Haybarn/VGI downloads the native platform artifact, verifies its SHA256 and caches
it. No Rust compiler, worker build, local source checkout or credentials are
needed. Initial extension, package, binary and companion setup needs network
access. The local lock uses Haybarn **2.0.0rc11**, tested on macOS arm64. The tag
resolves to `75c9774e39bc9df0e7c54d793963d3cb3a71e776`. To upgrade, change the
literal tag and revalidate its catalog, default, experiment and invariants.

The [shared v0.3.14 standard](https://github.com/Query-farm/vgi-example-notebooks/blob/v0.3.14/docs/standard.md) supplies catalog-derived worker title/subtitle,
branding, isolated SQL secrets, companion verification, SVG rendering and runner
protections. Keep `charts/` beside saved notebooks: content-addressed public SVG
image links display on GitHub, while interactive runs retain native SVG. Review
PNGs, experiment copies and execution logs are ignored.

Coverage is **6 of 6 public named surfaces**: `samples`, `mmaps`, `comms`, `events`,
`meta` and the `functions` view. The actual complete catalog contains five table
functions and one view. Path and BLOB inputs are forms of the same named function;
no APIs are excluded. `coverage.json` binds actual released/current catalogs,
complete output columns, successful execution receipts and reviewed SQL/narrative
mappings. The notebook executes the release. Compatible current mappings do not
claim current-main notebook execution. Current source changes metadata and composed
examples only; decoder code, output schemas and dependency locks are unchanged.

The reference view is joined to actual materialized populations to explain the
five relations' roles. Three sample records form the sample denominator; two
mapping and one name record complete the six capture records. The event attribute
and capture metadata rows supply configuration rather than extra sampled
instructions. The recorded name `loop` labels PID/TID 1234/1234. All sample periods
match the event attribute's value 4,000, while the decoded event name/configuration
remain unknown. These values cannot be converted to wall-clock seconds.

`fixtures/callchain.data` is copied byte-for-byte from the worker's deterministic
MIT-licensed fixture at source revision
`312b748d5453cda08e4e5a20db2fba2ea4e63207`. Its generator is
`crates/perf-core/src/fixture.rs::synthetic_callchain`; provenance records its
SHA256, size and source. It is generated test data, not a recorded workload. Its
three samples, two static mappings, one name record, timestamps and build IDs are
authored. No API requests or live capture occur. The notebook verifies its hash
before the worker reads it.

Mapping uses PID and half-open intervals **`addr <= ip < addr + len`** with wide
endpoint arithmetic. The released descriptive example's older `BETWEEN` form
includes the upper endpoint; the notebook deliberately uses the correct half-open
form, matching current source documentation. Nonoverlap and one name per PID are
checked for the fixture. Mapping timestamps are unavailable and the name timestamp
is NULL: these static joins cannot resolve address/PID reuse or command changes
in a general real-world capture. Unmapped addresses remain visible.

`MAX_FRAMES=2` retains every recorded frame. The experiment **1** keeps only the
first frame of each stack, without changing the source bytes or sampled IPs.
Retained frames fall from **5 to 3**, inclusive sample–object pairs from **4 to 3**,
and self counts stay **2 loop / 1 libc**. Grey cells show omitted caller evidence;
a separate empty cell marks a frame that was never recorded. Inclusive counts can
overlap across objects; frame counts additionally weight repeated appearances.
They are not interchangeable percentages or timing measurements. Repeated frames
in one object do not establish recursion, and raw addresses are not symbol names.

Twelve explicit SQL relationship checks complement the existing uniqueness,
nonoverlap, frame-reconciliation and raw-fingerprint assertions. They establish the
fixture transformation, not real capture completeness or profiler uncertainty.
The decoder can retain good rows after a malformed tail, and its internal error
collection is not exposed through these tables. Successful SQL is therefore not
evidence of a complete, error-free real capture.

Validation includes the executed default, a separate notebooks-only depth-1
experiment, two reviewed SVG figures, runner guards and a fresh preloaded
compatible kernel for this companion/launcher path. Hosted Colab was not manually
operated. Existing green [source CI at 2c8ae07](https://github.com/Query-farm/vgi-perf/actions/runs/43580055619)
is reused; no source rebuild, full-suite repetition or new worker CI is triggered.
Prior source checks passed 16 Rust tests, 121 stdio / 135 HTTP / 121 Unix SQL
assertions, clippy, format and rustdoc. The earlier `vgi-lint-check==2.5.0` metadata
run reported 98/100, no errors and three VGI417 warnings for long composed
`vgi.example_queries` on comms, mmaps and samples; configured `--fail-on info`
exited 2. That unchanged local baseline is separate from the successful source CI.
