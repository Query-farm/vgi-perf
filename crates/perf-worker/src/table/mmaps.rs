//! `perf.mmaps(src)` — the memory mappings (PERF_RECORD_MMAP / MMAP2) of a
//! `perf.data` capture, with build-ids for native symbolication.

use crate::arrow_build::Table;

use super::PerfTableFn;

pub fn def() -> PerfTableFn {
    let mut tags = crate::meta::object_tags(
        "perf.data Memory Mappings",
        "Decode the memory-mapping records (PERF_RECORD_MMAP / MMAP2) of a Linux perf.data capture \
         into rows — which shared object / DSO backs each range of a process's address space. \
         `src` is a `VARCHAR` path or a `BLOB` of perf.data bytes. Columns: `pid`, `tid`, `addr` \
         (start address), `len` (length in bytes — the mapping covers [addr, addr+len)), `pgoff` \
         (offset into the file), `filename` (e.g. '/usr/lib/libc.so.6', '[vdso]'), and `build_id` \
         (the DSO build-id as lowercase hex, from an inline MMAP2 build-id record or the \
         HEADER_BUILD_ID feature section; NULL when unknown). These are what make a sample `ip` \
         interpretable: join `samples.ip >= m.addr AND samples.ip < m.addr + m.len` (half-open) to \
         find which DSO an IP fell in, and feed `build_id` + the IP to vgi-symbols for native symbolication. \
         Collapsed-stack text files throw this information away; the binary perf.data keeps it.",
        "Decode perf.data memory mappings into rows: `pid`, `tid`, `addr`, `len`, `pgoff`, \
         `filename`, `build_id` (lowercase hex, or NULL). Range-join `samples.ip >= addr AND \
         samples.ip < addr+len` to attribute an IP to a DSO; `build_id` feeds vgi-symbols. `src` is a path or \
         `BLOB`.",
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
            (
                "len",
                "UBIGINT",
                "Length in bytes; the mapping covers [addr, addr+len).",
            ),
            ("pgoff", "UBIGINT", "Offset into the mapped file."),
            (
                "filename",
                "VARCHAR",
                "Mapped file path, e.g. '/usr/lib/libc.so.6'.",
            ),
            (
                "build_id",
                "VARCHAR",
                "DSO build-id (lowercase hex); NULL if unknown.",
            ),
        ]),
    ));
    let blob = crate::meta::fixture_blob_src();
    let build_ids = format!(
        "SELECT filename, build_id FROM perf.main.mmaps({blob}) WHERE build_id IS NOT NULL ORDER BY filename"
    );
    let hot_files = crate::meta::hot_files_sql(&blob);
    let symbol_keys = crate::meta::symbolication_keys_sql(&blob);
    let executable = [
        (
            "Mapped DSOs with a known build-id (for vgi-symbols).",
            build_ids.as_str(),
        ),
        (crate::meta::HOT_FILES_DESC, hot_files.as_str()),
    ];
    tags.push((
        "vgi.executable_examples".into(),
        crate::meta::example_queries_json(&executable),
    ));
    // The symbolication-key query adds two more fixture-BLOB references, past the
    // 4,000-char executable budget (VGI417): `vgi.example_queries` (8,000) only.
    let mut examples = executable.to_vec();
    examples.push((crate::meta::SYMBOLICATION_KEYS_DESC, symbol_keys.as_str()));
    tags.push((
        "vgi.example_queries".into(),
        crate::meta::example_queries_json(&examples),
    ));
    PerfTableFn {
        table: Table::Mmaps,
        name: "mmaps",
        description: "Decode perf.data memory mappings (with build-ids) into rows",
        tags,
    }
}
