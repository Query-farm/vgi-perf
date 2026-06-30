//! Regenerate the committed synthetic `perf.data` fixture used by the SQL E2E
//! suite. Deterministic — run from the repo root:
//!
//! ```bash
//! cargo run -p perf-core --example gen_fixture
//! ```
//!
//! It writes `data/callchain.data` (a tiny, spec-valid `cycles` capture with
//! known call chains, inline build-ids, and an empty feature bitmap so `meta`
//! columns are NULL). The bytes are produced by
//! [`perf_core::fixture::synthetic_callchain`], which the core golden tests also
//! exercise in-memory, so the committed file always matches the tested bytes.

fn main() -> std::io::Result<()> {
    let bytes = perf_core::fixture::synthetic_callchain();
    let out = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/callchain.data");
    std::fs::write(out, &bytes)?;
    println!("wrote {} ({} bytes)", out, bytes.len());
    Ok(())
}
