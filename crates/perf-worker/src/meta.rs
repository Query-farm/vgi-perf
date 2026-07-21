//! Shared helpers for the per-object discovery/description metadata that the
//! `vgi-lint` strict profile expects on **every** function and table.
//!
//! Each function/table surfaces these in its `FunctionMetadata.tags`:
//! - `vgi.title` (VGI124)        — human-friendly display name
//! - `vgi.doc_llm` (VGI112) — concise prose aimed at LLMs
//! - `vgi.doc_md` (VGI113)  — short Markdown description
//! - `vgi.keywords` (VGI126/VGI138) — a JSON array of search terms/synonyms
//!
//! Per-object `vgi.source_url` is intentionally NOT emitted here: `vgi.source_url`
//! belongs on the catalog object only (VGI139). The catalog's `source_url` field
//! already points at the repo.

/// Hex of the committed synthetic `data/callchain.data` fixture (a tiny, valid
/// `cycles` perf.data capture: 3 samples with call chains, 2 mmap2 records with
/// inline build-ids, 1 COMM, 1 event attr, empty feature bitmap). Embedded so the
/// `vgi.agent_test_tasks` suite (VGI152/VGI920) AND every object's executable /
/// example queries are self-contained: the analyst and the `vgi-lint --execute`
/// sandbox decode it via `from_hex(...)` as a BLOB `src`, with no external
/// `perf.data` file needed. Built by `perf_core::fixture::synthetic_callchain`;
/// see `data/callchain.data`.
pub const FIXTURE_HEX: &str = "50455246494c45326800000000000000400000000000000068000000000000004000000000000000a800000000000000a00100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000400000000000000000000000a00f000000000000a70100000000000000000000000000000000000000000000000000000000000000000000000000000300000002001800d2040000d20400006c6f6f70000000000a00000002405800d2040000d204000000004000000000000010000000000000000000000000000014000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa00000000000000002f7573722f62696e2f6c6f6f700000000a00000002406000d2040000d204000000000000007f00000000200000000000000000000000000014000000bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb00000000000000002f7573722f6c69622f6c6962632e736f2e3600000000000009000000020048000001400000000000d2040000d2040000e8030000000000000200000000000000a00f000000000000020000000000000000014000000000005000400000000000090000000200480000010000007f0000d2040000d2040000d0070000000000000200000000000000a00f000000000000020000000000000000010000007f0000000140000000000009000000020040000001400000000000d2040000d2040000b80b0000000000000300000000000000a00f00000000000001000000000000000001400000000000";

/// A `from_hex('…')` SQL expression that decodes the embedded [`FIXTURE_HEX`]
/// capture as a BLOB `src`. Used by every object's example / executable-example
/// queries so they run credential- and file-free under `vgi-lint --execute`.
pub fn fixture_blob_src() -> String {
    format!("from_hex('{FIXTURE_HEX}')")
}

/// Build a `vgi.example_queries` / `vgi.executable_examples` value: a JSON array
/// of `{"description","sql"}` objects (VGI502/VGI503/VGI507).
pub fn example_queries_json(examples: &[(&str, &str)]) -> String {
    fn esc(s: &str) -> String {
        s.replace('\\', "\\\\").replace('"', "\\\"")
    }
    let items: Vec<String> = examples
        .iter()
        .map(|(description, sql)| {
            format!(
                "{{\"description\":\"{}\",\"sql\":\"{}\"}}",
                esc(description),
                esc(sql)
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

/// Build a `vgi.result_columns_schema` value: a JSON array of
/// `{"name","type","description"}` objects (VGI307/VGI321/VGI322/VGI323). Types
/// must be real DuckDB type names as `DESCRIBE` reports them (e.g. `UBIGINT`,
/// `UBIGINT[]`) so the declared schema matches the live output (VGI910).
pub fn result_columns_schema_json(cols: &[(&str, &str, &str)]) -> String {
    fn esc(s: &str) -> String {
        s.replace('\\', "\\\\").replace('"', "\\\"")
    }
    let items: Vec<String> = cols
        .iter()
        .map(|(name, ty, description)| {
            format!(
                "{{\"name\":\"{}\",\"type\":\"{}\",\"description\":\"{}\"}}",
                esc(name),
                esc(ty),
                esc(description)
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

/// Encode comma-separated keywords as the JSON array of strings that
/// `vgi.keywords` requires (VGI138). Each term is trimmed and empty terms are
/// dropped; the result is e.g. `["units","convert","length"]`.
pub fn keywords_json(keywords: &str) -> String {
    let items: Vec<String> = keywords
        .split(',')
        .map(str::trim)
        .filter(|k| !k.is_empty())
        // JSON-escape each keyword (covers quotes/backslashes) by emitting a
        // one-element array and stripping the surrounding brackets.
        .map(|k| {
            let escaped = k.replace('\\', "\\\\").replace('"', "\\\"");
            format!("\"{escaped}\"")
        })
        .collect();
    format!("[{}]", items.join(","))
}

/// Category names for the schema's `vgi.categories` registry (VGI413). Each
/// object carries a `vgi.category` naming exactly one of these; the schema
/// declares the ordered registry via [`categories_json`].
pub const CAT_SAMPLES: &str = "Profile Samples";
pub const CAT_MAPPINGS: &str = "Address Space";
pub const CAT_CONTEXT: &str = "Capture Context";
pub const CAT_REFERENCE: &str = "Reference";

/// The ordered `vgi.categories` registry declared on schema `main` (VGI413): a
/// JSON array of `{"name","description"}` objects, one per `CAT_*` constant.
/// Order is the navigation / listing order.
pub fn categories_json() -> String {
    fn esc(s: &str) -> String {
        s.replace('\\', "\\\\").replace('"', "\\\"")
    }
    let cats = [
        (
            CAT_SAMPLES,
            "The profiling samples themselves — the per-event rows with their raw, \
             unsymbolicated instruction pointers and call chains that a hotspot or flame analysis \
             starts from.",
        ),
        (
            CAT_MAPPINGS,
            "The memory mappings and build-ids that make a raw instruction pointer interpretable: \
             range-join an address to the shared object it fell in, then hand the build-id to \
             vgi-symbols for native symbolication.",
        ),
        (
            CAT_CONTEXT,
            "The surrounding capture context used to label and interpret samples — process and \
             thread names, the sampled event attributes and their policy, and the capture-wide \
             host / tool metadata.",
        ),
        (
            CAT_REFERENCE,
            "Browsable reference tables for discovery — a no-argument catalog of the decode \
             functions this worker exposes and the columns each returns, so an agent can learn \
             the surface before it has a capture in hand.",
        ),
    ];
    let items: Vec<String> = cats
        .iter()
        .map(|(name, desc)| {
            format!(
                "{{\"name\":\"{}\",\"description\":\"{}\"}}",
                esc(name),
                esc(desc)
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

/// Build the `vgi.agent_test_tasks` JSON value run by `vgi-lint simulate`
/// (VGI152/VGI920). Each task is `(name, prompt, reference_sql)`; only the prompt
/// reaches the analyst, the reference_sql is grader-only.
pub fn agent_test_tasks_json(tasks: &[(&str, &str, &str)]) -> String {
    fn esc(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    }
    let items: Vec<String> = tasks
        .iter()
        .map(|(name, prompt, reference_sql)| {
            format!(
                "{{\"name\":\"{}\",\"prompt\":\"{}\",\"reference_sql\":\"{}\"}}",
                esc(name),
                esc(prompt),
                esc(reference_sql)
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

/// Build the standard per-object discovery/description tags (title, doc_llm,
/// doc_md, keywords, category). `category` must name one of the schema's
/// `vgi.categories` (VGI413); use a `CAT_*` constant. `_relative_path` is
/// retained for call-site documentation only — it is no longer emitted as a
/// per-object `vgi.source_url` (that tag is catalog-only — VGI139).
pub fn object_tags(
    title: &str,
    description_llm: &str,
    description_md: &str,
    keywords: &str,
    _relative_path: &str,
    category: &str,
) -> Vec<(String, String)> {
    vec![
        ("vgi.title".to_string(), title.to_string()),
        ("vgi.doc_llm".to_string(), description_llm.to_string()),
        ("vgi.doc_md".to_string(), description_md.to_string()),
        ("vgi.keywords".to_string(), keywords_json(keywords)),
        ("vgi.category".to_string(), category.to_string()),
    ]
}
