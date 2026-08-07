//! Behavioral rules of the renderer: head metadata, landmark structure,
//! lenient reads (skip-with-log), and the escaping/href defenses that must
//! hold even for stored values the write gate would never have admitted.
#![allow(clippy::unwrap_used)]

use alo_sites::render::{EN, PageRenderContext, SiteRenderContext, render_page, sections_lenient};
use alo_store::site_theme::SiteTheme;
use serde_json::json;

/// Renders arbitrary stored-sections JSON on a default-theme site.
fn render(sections: &serde_json::Value) -> String {
    let theme = SiteTheme::new();
    render_with(&theme, sections, None, None)
}

fn render_with(
    theme: &SiteTheme,
    sections: &serde_json::Value,
    seo_title: Option<&str>,
    seo_description: Option<&str>,
) -> String {
    let site = SiteRenderContext {
        name: "Nordwind Coffee Roasters",
        base_url: "https://nordwind.alosites.com",
        theme,
        strings: &EN,
    };
    let page = PageRenderContext {
        path: "/about",
        title: "About",
        seo_title,
        seo_description,
        sections,
    };
    render_page(&site, &page)
}

fn hero_page() -> serde_json::Value {
    json!({
        "schema_version": 1,
        "sections": [
            {"type": "nav", "links": [{"label": "Home", "href": "/"}]},
            {"type": "hero", "heading": "Hello", "image":
                {"blob_id": "9hK3vQ2mR8pT1xWz4bC5dg", "alt": "The drum"}},
            {"type": "footer", "text": "© Nordwind", "links": []},
        ]
    })
}

#[test]
fn head_carries_title_description_canonical_and_og() {
    let html = render_with(
        &SiteTheme::new(),
        &hero_page(),
        Some("About Nordwind"),
        Some("Who we are."),
    );
    for expected in [
        "<title>About Nordwind</title>",
        "<meta name=\"description\" content=\"Who we are.\">",
        "<link rel=\"canonical\" href=\"https://nordwind.alosites.com/about\">",
        "<meta property=\"og:type\" content=\"website\">",
        "<meta property=\"og:site_name\" content=\"Nordwind Coffee Roasters\">",
        "<meta property=\"og:title\" content=\"About Nordwind\">",
        "<meta property=\"og:description\" content=\"Who we are.\">",
        "<meta property=\"og:url\" content=\"https://nordwind.alosites.com/about\">",
        "<meta property=\"og:image\" content=\"https://nordwind.alosites.com/assets/img/9hK3vQ2mR8pT1xWz4bC5dg\">",
        "<link rel=\"stylesheet\" href=\"/assets/site.css\">",
        "<html lang=\"en\">",
    ] {
        assert!(html.contains(expected), "missing {expected} in:\n{html}");
    }
}

#[test]
fn default_title_joins_page_and_site_name_and_description_is_omitted() {
    let html = render(&hero_page());
    assert!(html.contains("<title>About — Nordwind Coffee Roasters</title>"));
    assert!(!html.contains("name=\"description\""));
    assert!(!html.contains("og:description"));
}

#[test]
fn landmarks_wrap_main_in_order_with_skip_link_first() {
    let html = render(&hero_page());
    let skip = html.find("class=\"skip-link\"").unwrap();
    let header = html.find("<header class=\"s-nav\">").unwrap();
    let main = html.find("<main id=\"main\">").unwrap();
    let footer = html.find("<footer class=\"s-footer\">").unwrap();
    assert!(skip < header && header < main && main < footer);
    // The hero lives inside main, not before it.
    assert!(html.find("<section class=\"s-hero\">").unwrap() > main);
}

#[test]
fn unknown_sections_and_newer_versions_render_best_effort() {
    let stored = json!({
        "schema_version": 2,
        "sections": [
            {"type": "carousel", "speed": "fast"},
            {"type": "cta", "heading": "Still here",
             "button": {"label": "Go", "href": "/go"}},
            {"not even": "tagged"},
        ]
    });
    let parsed = sections_lenient(&stored);
    assert_eq!(parsed.len(), 1, "only the cta parses");
    let html = render(&stored);
    assert!(html.contains("Still here"));
    assert!(!html.contains("carousel"));
}

#[test]
fn envelope_without_a_sections_array_renders_an_empty_document() {
    let html = render(&json!({"schema_version": 1}));
    assert!(html.contains("<main id=\"main\">\n</main>"));
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.ends_with("</html>\n"));
}

#[test]
fn script_content_renders_escaped_never_live() {
    let stored = json!({
        "schema_version": 1,
        "sections": [{"type": "hero", "heading": "<script>alert(1)</script>"}]
    });
    let html = render(&stored);
    assert!(!html.contains("<script"));
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
}

#[test]
fn attribute_injection_renders_escaped() {
    let stored = json!({
        "schema_version": 1,
        "sections": [{"type": "gallery", "images": [
            {"blob_id": "9hK3vQ2mR8pT1xWz4bC5dg",
             "alt": "\" onmouseover=\"alert(1)"}
        ]}]
    });
    let html = render(&stored);
    assert!(!html.contains("\" onmouseover=\""));
    assert!(html.contains("&quot; onmouseover=&quot;"));
}

#[test]
fn unsafe_stored_href_renders_inert() {
    // The write gate rejects these; a hostile or pre-rule stored value must
    // still come out inert.
    let stored = json!({
        "schema_version": 1,
        "sections": [{"type": "cta", "heading": "Click",
            "button": {"label": "Go", "href": "javascript:alert(1)"}}]
    });
    let html = render(&stored);
    assert!(!html.to_ascii_lowercase().contains("javascript:"));
    assert!(html.contains("<a class=\"button\" href=\"#\">Go</a>"));
}

#[test]
fn contact_form_without_form_id_renders_text_only() {
    let stored = json!({
        "schema_version": 1,
        "sections": [{"type": "contact_form", "heading": "Write us"}]
    });
    let html = render(&stored);
    assert!(html.contains("<h2>Write us</h2>"));
    assert!(!html.contains("<form"));
    // A form with no submit is nothing for the script to do.
    assert!(!html.contains("<script"));
}

#[test]
fn contact_form_without_custom_message_gets_the_default_data_success() {
    let stored = json!({
        "schema_version": 1,
        "sections": [{"type": "contact_form", "form_id": "f4K9sL2wN7qR5tYx8vB1cA"}]
    });
    let html = render(&stored);
    assert!(html.contains("data-success=\"Thanks — your message has been sent.\""));
}

/// The page's entire JavaScript is one static inline block, present only
/// when there is a menu to toggle or a form to submit — a page with neither
/// ships zero JS.
#[test]
fn behavior_script_is_included_exactly_when_needed() {
    let with_nav = render(&hero_page());
    assert_eq!(with_nav.matches("<script>").count(), 1);
    let script_at = with_nav.find("<script>").unwrap();
    assert!(script_at > with_nav.find("</footer>").unwrap());
    assert!(script_at < with_nav.find("</body>").unwrap());
    for wired in [
        "classList.add(\"js\")",
        "aria-expanded",
        "form[action^=\"/f/\"]",
    ] {
        assert!(with_nav.contains(wired), "script lost its {wired} wiring");
    }

    let with_form = render(&json!({
        "schema_version": 1,
        "sections": [{"type": "contact_form", "form_id": "f4K9sL2wN7qR5tYx8vB1cA"}]
    }));
    assert_eq!(with_form.matches("<script>").count(), 1);

    let static_only = render(&json!({
        "schema_version": 1,
        "sections": [{"type": "hero", "heading": "Quiet"}]
    }));
    assert!(!static_only.contains("<script"));
}

#[test]
fn contact_form_posts_to_the_form_path_with_honeypot_and_fixed_fields() {
    let stored = json!({
        "schema_version": 1,
        "sections": [{"type": "contact_form",
            "form_id": "f4K9sL2wN7qR5tYx8vB1cA",
            "success_message": "Thanks!"}]
    });
    let html = render(&stored);
    assert!(html.contains(
        "<form action=\"/f/f4K9sL2wN7qR5tYx8vB1cA\" method=\"post\" data-success=\"Thanks!\">"
    ));
    // The fixed v1 field contract the forms backend will accept.
    for field in ["name=\"name\"", "name=\"email\"", "name=\"message\""] {
        assert!(html.contains(field), "missing {field}");
    }
    // The honeypot: present, hidden from assistive tech, out of tab order.
    assert!(html.contains(
        "<p class=\"hp\" aria-hidden=\"true\"><label for=\"form-0-website\">Website</label>"
    ));
    assert!(html.contains("name=\"website\" type=\"text\" tabindex=\"-1\" autocomplete=\"off\""));
}

#[test]
fn theme_logo_and_favicon_reach_nav_and_head() {
    let theme = SiteTheme::from_value(json!({
        "schema_version": 1,
        "preset": "terra",
        "logo": "L0g0aaaaaaaaaaaaaaaaaa",
        "favicon": "Fav1conaaaaaaaaaaaaaaa",
    }))
    .unwrap();
    let html = render_with(&theme, &hero_page(), None, None);
    assert!(html.contains("<link rel=\"icon\" href=\"/assets/img/Fav1conaaaaaaaaaaaaaaa\">"));
    assert!(html.contains(
        "<a class=\"brand\" href=\"/\"><img class=\"logo\" src=\"/assets/img/L0g0aaaaaaaaaaaaaaaaaa\" alt=\"Nordwind Coffee Roasters\"></a>"
    ));
    // Without a logo, the brand link is the site name as text.
    let bare = render(&hero_page());
    assert!(bare.contains("<a class=\"brand\" href=\"/\">Nordwind Coffee Roasters</a>"));
    assert!(!bare.contains("rel=\"icon\""));
}
