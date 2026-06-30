//! `perf-core` — a pure-Rust structural decoder for the Linux `perf.data` binary
//! format.
//!
//! It turns the sequential `perf.data` event stream into five plain row
//! collections — [`SampleRow`], [`MmapRow`], [`CommRow`], [`EventRow`], and a
//! single [`MetaRow`] — with no Arrow or RPC dependency, so it can be tested
//! directly against golden fixtures. The `perf-worker` crate maps these onto
//! Apache Arrow and serves them to DuckDB under the `perf` catalog.
//!
//! Scope (committee directive): the **binary** `perf.data` structural decode
//! only — the folded/collapsed-stack text path is intentionally dropped (DuckDB
//! `read_csv` covers it). Call-chain instruction pointers are emitted **raw**;
//! symbolication is the job of `vgi-symbols`, fed by `mmaps.build_id` + the raw
//! `ip`.
//!
//! ```no_run
//! let bytes = std::fs::read("perf.data").unwrap();
//! let decoded = perf_core::decode(&bytes).unwrap();
//! println!("{} samples, {} mmaps", decoded.samples.len(), decoded.mmaps.len());
//! ```

mod decode;
pub mod fixture;
mod rows;

pub use decode::{decode, FatalError};
pub use rows::{CommRow, DecodeError, Decoded, EventRow, MetaRow, MmapRow, SampleRow};
