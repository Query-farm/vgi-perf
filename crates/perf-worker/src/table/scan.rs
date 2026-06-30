//! The shared per-execution producer behind every `perf.*(src)` table function.
//!
//! `perf.data` is a **sequential** binary event stream, so a scan decodes the
//! whole source once (in [`crate::source::decoded`]) and then streams the decoded
//! rows of one table out in batches. The scan position — the cursor into that
//! sequential stream — is **externalized** via the SDK's resume hooks
//! ([`encode_resume`] / [`restore_resume`]), so the HTTP transport can return one
//! batch per response and resume at the next batch boundary without holding the
//! whole result set in memory. On resume the producer is rebuilt from the same
//! bind params (the decode is deterministic), then re-seeded to the saved cursor.
//!
//! [`encode_resume`]: vgi::table_function::TableProducer::encode_resume
//! [`restore_resume`]: vgi::table_function::TableProducer::restore_resume

use std::sync::Arc;

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use perf_core::Decoded;
use vgi::table_function::{resume, TableProducer};
use vgi_rpc::{OutputCollector, Result};

use crate::arrow_build::Table;

/// Rows per emitted batch. `meta` (one row) and `events` (a handful) fit in one
/// batch; `samples` of a large capture stream out in chunks of this size.
const BATCH_ROWS: usize = 8192;

/// A producer streaming one decoded `perf` table out in row batches, with an
/// externalized byte-stream scan cursor.
pub struct PerfScan {
    decoded: Arc<Decoded>,
    table: Table,
    schema: SchemaRef,
    /// Cursor: the next row index into the sequential decoded stream to emit.
    cursor: usize,
    total: usize,
}

impl PerfScan {
    pub fn new(decoded: Arc<Decoded>, table: Table, schema: SchemaRef) -> Self {
        let total = table.row_count(&decoded);
        PerfScan {
            decoded,
            table,
            schema,
            cursor: 0,
            total,
        }
    }
}

impl TableProducer for PerfScan {
    fn next_batch(&mut self, _out: &mut OutputCollector) -> Result<Option<RecordBatch>> {
        if self.cursor >= self.total {
            return Ok(None);
        }
        let len = BATCH_ROWS.min(self.total - self.cursor);
        let batch = self
            .table
            .build(&self.decoded, &self.schema, self.cursor, len)?;
        self.cursor += len;
        Ok(Some(batch))
    }

    fn resume_supported(&self) -> bool {
        true
    }

    /// Externalize the scan position: just the cursor into the sequential event
    /// stream. Everything else is regenerated from the bind params on resume.
    fn encode_resume(&self) -> Vec<u8> {
        resume::pack(&[self.cursor as i64])
    }

    /// Restore the scan position produced by [`Self::encode_resume`]; a
    /// corrupt/empty token degrades to a fresh start at cursor 0.
    fn restore_resume(&mut self, bytes: &[u8]) {
        if let Some(vals) = resume::unpack(bytes, 1) {
            self.cursor = (vals[0].max(0) as usize).min(self.total);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan() -> PerfScan {
        let d = Arc::new(perf_core::decode(&perf_core::fixture::synthetic_callchain()).unwrap());
        let schema = Table::Samples.schema();
        PerfScan::new(d, Table::Samples, schema)
    }

    #[test]
    fn resume_round_trips_the_cursor() {
        let mut s = scan();
        assert_eq!(s.total, 3);
        assert!(s.resume_supported());
        // Advance the cursor as a mid-scan batch boundary would.
        s.cursor = 2;
        let token = s.encode_resume();

        // A fresh producer (rebuilt from the same params) re-seeds to the saved
        // position.
        let mut resumed = scan();
        resumed.restore_resume(&token);
        assert_eq!(resumed.cursor, 2);
    }

    #[test]
    fn corrupt_token_degrades_to_fresh_start() {
        let mut s = scan();
        s.restore_resume(&[]);
        assert_eq!(s.cursor, 0);
        s.restore_resume(&[1, 2, 3]); // wrong length
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn restore_clamps_to_total() {
        let mut s = scan();
        s.restore_resume(&resume::pack(&[999]));
        assert_eq!(s.cursor, s.total);
    }
}
