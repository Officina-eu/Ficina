//! Page JSON + theme → one complete HTML document (pure, infallible).
//!
//! The write gate (`alo_store::site_model`) guarantees everything stored is
//! valid, so rendering has no failure path: a section this build cannot parse
//! is **skipped with a `tracing` warning**, never a 500 — an old renderer
//! must survive a newer snapshot mid-deploy. Independently of write-side
//! validation, every text and attribute value is escaped and every href is
//! re-checked here (defense in depth): even a hostile stored value renders as
//! inert text.
//!
//! Landmark rule: `nav` sections render as `<header>` blocks before the one
//! `<main>`, `footer` sections as `<footer>` blocks after it, and everything
//! else inside `<main>` in author order. A nav authored mid-page therefore
//! still lands in the header region — the document stays valid and navigable
//! for assistive technology, which outranks literal ordering.

mod html;
mod sections;
mod strings;

pub use strings::{EN, UiStrings};

use alo_store::site_model::{SECTIONS_SCHEMA_VERSION, Section};
use alo_store::site_theme::SiteTheme;

use html::{esc, img_src};

/// Site-level inputs of a render: everything that is true for every page.
#[derive(Debug, Clone, Copy)]
pub struct SiteRenderContext<'a> {
    /// The site's display name (nav brand fallback, `og:site_name`, title
    /// suffix).
    pub name: &'a str,
    /// Absolute origin the site is served on, no trailing slash
    /// (e.g. `https://nordwind.alosites.com`); used for canonical/OG URLs.
    pub base_url: &'a str,
    /// The site's theme (logo, favicon; the preset drives the stylesheet).
    pub theme: &'a SiteTheme,
    /// Visitor-facing chrome strings ([`EN`] until more locales ship).
    pub strings: &'a UiStrings,
}

/// Page-level inputs of a render.
#[derive(Debug, Clone, Copy)]
pub struct PageRenderContext<'a> {
    /// Site-relative path of this page, starting with `/` (home is `/`).
    pub path: &'a str,
    /// The page's title.
    pub title: &'a str,
    /// SEO title override; when absent the title is
    /// `<page title> — <site name>`.
    pub seo_title: Option<&'a str>,
    /// SEO meta description; absent means no description/OG-description tags.
    pub seo_description: Option<&'a str>,
    /// The stored sections envelope (`{ "schema_version": …, "sections": … }`).
    pub sections: &'a serde_json::Value,
}

/// Reads a stored sections envelope leniently: entries that parse as a known
/// [`Section`] render; anything else is skipped with a warning. This is the
/// read-side tolerance the design note requires — the strict counterpart
/// (`SectionsEnvelope::from_value`) guards writes, never reads.
pub fn sections_lenient(stored: &serde_json::Value) -> Vec<Section> {
    if let Some(version) = stored
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        && version != SECTIONS_SCHEMA_VERSION
    {
        tracing::warn!(
            version,
            speaks = SECTIONS_SCHEMA_VERSION,
            "rendering a sections envelope from a different schema version best-effort"
        );
    }
    let Some(entries) = stored.get("sections").and_then(serde_json::Value::as_array) else {
        tracing::warn!("stored sections value has no sections array; rendering an empty page");
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| match serde_json::from_value(entry.clone()) {
            Ok(section) => Some(section),
            Err(error) => {
                let kind = entry
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("<untagged>");
                tracing::warn!(section = kind, %error, "skipping unrenderable section");
                None
            }
        })
        .collect()
}

/// Renders one page to a complete HTML document.
pub fn render_page(site: &SiteRenderContext<'_>, page: &PageRenderContext<'_>) -> String {
    let parsed = sections_lenient(page.sections);

    let mut header = String::new();
    let mut main = String::new();
    let mut footer = String::new();
    for (index, section) in parsed.iter().enumerate() {
        match section {
            Section::Nav(nav) => sections::nav(&mut header, site, nav, index),
            Section::Footer(f) => sections::footer(&mut footer, site, f),
            other => sections::body_section(&mut main, site, other, index),
        }
    }

    let mut out = String::with_capacity(16 * 1024);
    out.push_str("<!doctype html>\n");
    out.push_str(&format!("<html lang=\"{}\">\n", esc(site.strings.lang)));
    push_head(&mut out, site, page, &parsed);
    out.push_str("<body>\n");
    out.push_str(&format!(
        "<a class=\"skip-link\" href=\"#main\">{}</a>\n",
        esc(site.strings.skip_to_content)
    ));
    out.push_str(&header);
    out.push_str("<main id=\"main\">\n");
    out.push_str(&main);
    out.push_str("</main>\n");
    out.push_str(&footer);
    out.push_str("</body>\n</html>\n");
    out
}

/// The `<head>`: charset/viewport, title, description, canonical, OG tags,
/// favicon, and the one stylesheet.
fn push_head(
    out: &mut String,
    site: &SiteRenderContext<'_>,
    page: &PageRenderContext<'_>,
    parsed: &[Section],
) {
    let title = match page.seo_title {
        Some(seo) => seo.to_owned(),
        None => format!("{} — {}", page.title, site.name),
    };
    let canonical = format!("{}{}", site.base_url, page.path);

    out.push_str("<head>\n<meta charset=\"utf-8\">\n");
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    out.push_str(&format!("<title>{}</title>\n", esc(&title)));
    if let Some(description) = page.seo_description {
        out.push_str(&format!(
            "<meta name=\"description\" content=\"{}\">\n",
            esc(description)
        ));
    }
    out.push_str(&format!(
        "<link rel=\"canonical\" href=\"{}\">\n",
        esc(&canonical)
    ));
    out.push_str("<meta property=\"og:type\" content=\"website\">\n");
    out.push_str(&format!(
        "<meta property=\"og:site_name\" content=\"{}\">\n",
        esc(site.name)
    ));
    out.push_str(&format!(
        "<meta property=\"og:title\" content=\"{}\">\n",
        esc(&title)
    ));
    if let Some(description) = page.seo_description {
        out.push_str(&format!(
            "<meta property=\"og:description\" content=\"{}\">\n",
            esc(description)
        ));
    }
    out.push_str(&format!(
        "<meta property=\"og:url\" content=\"{}\">\n",
        esc(&canonical)
    ));
    if let Some(blob) = first_hero_image(parsed) {
        out.push_str(&format!(
            "<meta property=\"og:image\" content=\"{}{}\">\n",
            esc(site.base_url),
            img_src(blob)
        ));
    }
    if let Some(favicon) = &site.theme.favicon {
        out.push_str(&format!(
            "<link rel=\"icon\" href=\"{}\">\n",
            img_src(favicon.as_str())
        ));
    }
    out.push_str("<link rel=\"stylesheet\" href=\"/assets/site.css\">\n</head>\n");
}

/// The page's OG image source: the first hero section that carries an image.
fn first_hero_image(sections: &[Section]) -> Option<&str> {
    sections.iter().find_map(|section| match section {
        Section::Hero(hero) => hero.image.as_ref().map(|image| image.blob_id.as_str()),
        _ => None,
    })
}
