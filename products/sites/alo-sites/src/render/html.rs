//! HTML escaping and attribute-safety primitives for the renderer.
//!
//! Everything user-authored passes through [`esc`]; every link target passes
//! through [`safe_href`]. The write gate already enforces these rules, but
//! the renderer re-checks — a snapshot predating a rule, or a value that
//! somehow bypassed the gate, must still render inert.

/// Escapes a string for use in HTML text nodes **and** double-quoted
/// attribute values (`&`, `<`, `>`, `"`, `'`).
pub(crate) fn esc(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Escapes a link target for an `href` attribute, re-checking the write
/// gate's allowlist (site paths, fragments, http(s)/mailto/tel). Anything
/// else — a scriptable scheme, a protocol-relative URL — renders as inert
/// `#` with a warning, never as a live link.
pub(crate) fn safe_href(href: &str) -> String {
    if is_allowed_href(href) {
        esc(href)
    } else {
        tracing::warn!("stored href failed the render-side allowlist; rendering inert");
        "#".to_owned()
    }
}

/// The same allowlist as the write gate (`alo_store::site_model`): a stored
/// href is safe in an `href` attribute iff it is a site path, a fragment, or
/// an http(s)/mailto/tel URL.
fn is_allowed_href(href: &str) -> bool {
    if href.is_empty() || href.starts_with("//") {
        return false;
    }
    if href.starts_with('/') || href.starts_with('#') {
        return true;
    }
    let lower = href.to_ascii_lowercase();
    ["http://", "https://", "mailto:", "tel:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
}

/// The public image path for a tenant blob (`/assets/img/<blob_id>` — the
/// crate-level path contract), attribute-escaped.
pub(crate) fn img_src(blob_id: &str) -> String {
    format!("/assets/img/{}", esc(blob_id))
}
