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
pub const CAT_DIAGNOSTICS: &str = "Diagnostics";

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
            CAT_DIAGNOSTICS,
            "Operational introspection of the running worker, such as its version string.",
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
