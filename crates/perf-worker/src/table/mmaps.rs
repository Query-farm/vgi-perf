//! `perf.mmaps(src)` — the memory mappings (PERF_RECORD_MMAP / MMAP2) of a
//! `perf.data` capture, with build-ids for native symbolication.

use crate::arrow_build::Table;

use super::PerfTableFn;

pub fn def() -> PerfTableFn {
    let mut tags = crate::meta::object_tags(
        "perf.data Memory Mappings",
        "Decode the memory-mapping records (PERF_RECORD_MMAP / MMAP2) of a Linux perf.data capture \
         into rows — which shared object / DSO backs each range of a process's address space. \
         `src` is a VARCHAR path or a BLOB of perf.data bytes. Columns: `pid`, `tid`, `addr` \
         (start address), `len` (length in bytes — the mapping covers [addr, addr+len)), `pgoff` \
         (offset into the file), `filename` (e.g. '/usr/lib/libc.so.6', '[vdso]'), and `build_id` \
         (the DSO build-id as lowercase hex, from an inline MMAP2 build-id record or the \
         HEADER_BUILD_ID feature section; NULL when unknown). These are what make a sample `ip` \
         interpretable: join `samples.ip BETWEEN m.addr AND m.addr + m.len` to find which DSO an \
         IP fell in, and feed `build_id` + the IP to vgi-symbols for native symbolication. \
         Collapsed-stack text files throw this information away; the binary perf.data keeps it.",
        "Decode perf.data memory mappings into rows: `pid`, `tid`, `addr`, `len`, `pgoff`, \
         `filename`, `build_id` (lowercase hex, or NULL). Range-join `samples.ip BETWEEN addr AND \
         addr+len` to attribute an IP to a DSO; `build_id` feeds vgi-symbols. `src` is a path or \
         BLOB.",
        "perf, perf.data, mmap, mmap2, memory mapping, dso, shared object, build id, build_id, \
         symbolication, symbols, address range, module, library, vdso",
        "table/mmaps.rs",
        crate::meta::CAT_MAPPINGS,
    );
    tags.push((
        "vgi.result_columns_schema".into(),
        crate::meta::result_columns_schema_json(&[
            ("pid", "INTEGER", "Process id the mapping belongs to."),
            ("tid", "INTEGER", "Thread id the mapping belongs to."),
            ("addr", "UBIGINT", "Start address of the mapping."),
            ("len", "UBIGINT", "Length in bytes; the mapping covers [addr, addr+len)."),
            ("pgoff", "UBIGINT", "Offset into the mapped file."),
            ("filename", "VARCHAR", "Mapped file path, e.g. '/usr/lib/libc.so.6'."),
            ("build_id", "VARCHAR", "DSO build-id (lowercase hex); NULL if unknown."),
        ]),
    ));
    let blob = crate::meta::fixture_blob_src();
    let examples = crate::meta::example_queries_json(&[
        (
            "Every mapped DSO with a known build-id (the input to vgi-symbols), ordered by file.",
            &format!(
                "SELECT filename, build_id FROM perf.main.mmaps({blob}) WHERE build_id IS NOT NULL ORDER BY filename"
            ),
        ),
        (
            "The hottest mapped files: attribute each sample ip to the DSO it fell in via an address-range join, then rank by hit count.",
            &format!(
                "SELECT m.filename, count(*) AS hits FROM perf.main.samples({blob}) s JOIN perf.main.mmaps({blob}) m ON s.pid = m.pid AND s.ip BETWEEN m.addr AND m.addr + m.len GROUP BY m.filename ORDER BY hits DESC"
            ),
        ),
    ]);
    tags.push(("vgi.executable_examples".into(), examples.clone()));
    tags.push(("vgi.example_queries".into(), examples));
    PerfTableFn {
        table: Table::Mmaps,
        name: "mmaps",
        description: "Decode perf.data memory mappings (with build-ids) into rows",
        tags,
    }
}
