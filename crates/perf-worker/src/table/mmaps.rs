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
    );
    tags.push((
        "vgi.result_columns_md".into(),
        "| column | type | description |\n\
         |---|---|---|\n\
         | `pid` | INTEGER | Process id the mapping belongs to. |\n\
         | `tid` | INTEGER | Thread id the mapping belongs to. |\n\
         | `addr` | UBIGINT | Start address of the mapping. |\n\
         | `len` | UBIGINT | Length in bytes; the mapping covers `[addr, addr+len)`. |\n\
         | `pgoff` | UBIGINT | Offset into the mapped file. |\n\
         | `filename` | VARCHAR | Mapped file path, e.g. `/usr/lib/libc.so.6`. |\n\
         | `build_id` | VARCHAR | DSO build-id (lowercase hex); NULL if unknown. |"
            .into(),
    ));
    let examples = r#"[
  {
    "description": "List every mapped DSO and its build-id (the input to vgi-symbols).",
    "sql": "SELECT filename, build_id FROM perf.main.mmaps('/data/perf.data') WHERE build_id IS NOT NULL"
  },
  {
    "description": "Attribute each sample ip to the DSO it fell in, via an address-range join.",
    "sql": "SELECT m.filename, count(*) AS hits FROM perf.main.samples('/data/perf.data') s JOIN perf.main.mmaps('/data/perf.data') m ON s.pid = m.pid AND s.ip BETWEEN m.addr AND m.addr + m.len GROUP BY 1 ORDER BY 2 DESC"
  }
]"#;
    tags.push(("vgi.executable_examples".into(), examples.into()));
    tags.push(("vgi.example_queries".into(), examples.into()));
    PerfTableFn {
        table: Table::Mmaps,
        name: "mmaps",
        description: "Decode perf.data memory mappings (with build-ids) into rows",
        tags,
    }
}
