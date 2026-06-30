//! Resolve a table function's `src` argument — the overloaded **path | bytes**
//! source — into decoded `perf.data` rows.
//!
//! Every `perf.*(src)` table function takes the same first positional argument:
//! a VARCHAR file path (read from disk) or a BLOB of raw `perf.data` bytes
//! (decoded in place, e.g. when the file is already a column value). This module
//! centralizes that overload plus the file-existence check used at bind time.

use std::sync::Arc;

use perf_core::Decoded;
use vgi::arguments::Arguments;
use vgi_rpc::{Result, RpcError};

/// Read the `src` (positional 0) as raw `perf.data` bytes: a VARCHAR path is read
/// from disk; a BLOB is taken as-is. Errors if `src` is absent or the wrong type,
/// or (for a path) if the file cannot be read.
fn source_bytes(args: &Arguments) -> Result<Vec<u8>> {
    if let Some(path) = args.const_str(0) {
        return std::fs::read(&path).map_err(|e| {
            RpcError::value_error(format!("perf.data not readable at '{path}': {e}"))
        });
    }
    if let Some(bytes) = args.const_bytes(0) {
        return Ok(bytes);
    }
    Err(RpcError::value_error(
        "src must be a VARCHAR path to a perf.data file or a BLOB of perf.data bytes",
    ))
}

/// Resolve `src` and decode it into the five row collections.
///
/// Decoding is resilient (per-record errors are captured in [`Decoded::errors`],
/// not propagated); this only returns `Err` when `src` is missing/mistyped, the
/// path is unreadable, or the bytes are not a `perf.data` file at all. The result
/// is wrapped in an `Arc` so a producer can hold it cheaply across batches.
pub fn decoded(args: &Arguments) -> Result<Arc<Decoded>> {
    let bytes = source_bytes(args)?;
    let decoded = perf_core::decode(&bytes).map_err(|e| RpcError::value_error(format!("{e}")))?;
    Ok(Arc::new(decoded))
}

/// Bind-time validation: if `src` is a local path, fail early (a clear binder
/// error) when the file does not exist, mirroring DuckDB's native "File not
/// found". A BLOB `src`, or a path that exists, passes — the full decode happens
/// at producer time.
pub fn check_bind(args: &Arguments) -> Result<()> {
    if let Some(path) = args.const_str(0) {
        if !std::path::Path::new(&path).exists() {
            return Err(RpcError::value_error(format!("File not found: {path}")));
        }
    } else if args.const_bytes(0).is_none() {
        return Err(RpcError::value_error(
            "src must be a VARCHAR path to a perf.data file or a BLOB of perf.data bytes",
        ));
    }
    Ok(())
}
