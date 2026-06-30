//! The five table functions exposed by the perf worker under `perf.main`:
//! `samples`, `mmaps`, `comms`, `events`, and `meta`. Each takes the same
//! overloaded `src` argument (a VARCHAR path or a BLOB of `perf.data` bytes) and
//! streams the decoded rows out via the shared [`scan::PerfScan`] producer.

mod comms;
mod events;
mod meta;
mod mmaps;
mod samples;
mod scan;

use vgi::table_function::{TableFunction, TableProducer};
use vgi::{ArgSpec, BindParams, BindResponse, FunctionMetadata, ProcessParams};
use vgi_rpc::Result;

use crate::arrow_build::Table;
use crate::source;

/// Register every table function on the worker.
pub fn register(worker: &mut vgi::Worker) {
    worker.register_table(samples::def());
    worker.register_table(mmaps::def());
    worker.register_table(comms::def());
    worker.register_table(events::def());
    worker.register_table(meta::def());
}

/// A `perf.data` table function. All five share identical plumbing — only the
/// SQL name, the decoded [`Table`] they project, and their discovery metadata
/// differ — so they are one struct parameterized by data.
pub struct PerfTableFn {
    table: Table,
    name: &'static str,
    description: &'static str,
    tags: Vec<(String, String)>,
}

impl TableFunction for PerfTableFn {
    fn name(&self) -> &str {
        self.name
    }

    fn metadata(&self) -> FunctionMetadata {
        FunctionMetadata {
            description: self.description.into(),
            tags: self.tags.clone(),
            ..Default::default()
        }
    }

    fn argument_specs(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::const_arg(
            "src",
            0,
            "any",
            "The perf.data source: a VARCHAR path to a perf.data file on disk, or a BLOB \
             containing the raw perf.data bytes (e.g. a file already loaded into a column). \
             The file is decoded as a sequential binary event stream.",
        )]
    }

    fn on_bind(&self, params: &BindParams) -> Result<BindResponse> {
        source::check_bind(&params.arguments)?;
        Ok(BindResponse {
            output_schema: self.table.schema(),
            opaque_data: Vec::new(),
        })
    }

    fn producer(&self, params: &ProcessParams) -> Result<Box<dyn TableProducer>> {
        let decoded = source::decoded(&params.arguments)?;
        Ok(Box::new(scan::PerfScan::new(
            decoded,
            self.table,
            params.output_schema.clone(),
        )))
    }
}
