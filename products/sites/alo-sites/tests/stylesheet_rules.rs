//! Rules of the generated stylesheet: golden pinning of the default sheet,
//! per-preset token wiring, self-containment (zero external requests), the
//! selectors the markup and behavior script rely on, and the byte budget.
#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::PathBuf;

use alo_sites::stylesheet::stylesheet;
use alo_store::site_theme::{SiteTheme, THEME_PRESETS};
use serde_json::json;

fn theme_for(preset: &str) -> SiteTheme {
    SiteTheme::from_value(json!({"schema_version": 1, "preset": preset})).unwrap()
}

#[test]
fn default_stylesheet_matches_golden() {
    let css = stylesheet(&SiteTheme::new());
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/site.css");
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        fs::write(&path, &css).unwrap();
    }
    let expected = fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("missing golden site.css; run once with UPDATE_GOLDENS=1"));
    assert_eq!(
        expected, css,
        "site.css golden drifted — if deliberate, re-bless with UPDATE_GOLDENS=1 and review"
    );
}

#[test]
fn every_preset_sheet_carries_its_tokens_and_stays_in_budget() {
    for preset in THEME_PRESETS {
        let css = stylesheet(&theme_for(preset.id));
        assert!(
            css.len() < 50 * 1024,
            "{}: stylesheet is {} bytes, budget is 50KB",
            preset.id,
            css.len()
        );
        let p = preset.palette;
        for (token, value) in [
            ("--bg", p.background),
            ("--surface", p.surface),
            ("--text", p.text),
            ("--muted", p.muted_text),
            ("--primary", p.primary),
            ("--on-primary", p.on_primary),
            ("--border", p.border),
            ("--font-heading", preset.typography.heading_family),
            ("--font-body", preset.typography.body_family),
        ] {
            assert!(
                css.contains(&format!("{token}: {value};")),
                "{}: missing {token}",
                preset.id
            );
        }
        assert!(css.contains(&format!(
            "--weight-heading: {};",
            preset.typography.heading_weight
        )));
        assert_eq!(
            css.matches('{').count(),
            css.matches('}').count(),
            "{}: unbalanced braces",
            preset.id
        );
    }
}

/// The privacy promise, mechanically: the sheet triggers zero requests —
/// no imports, no fonts, no url() at all (there are no icon or background
/// assets), and no absolute URL anywhere.
#[test]
fn stylesheet_is_fully_self_contained() {
    for preset in THEME_PRESETS {
        let css = stylesheet(&theme_for(preset.id)).to_ascii_lowercase();
        for forbidden in ["@import", "@font-face", "url(", "http:", "https:", "//"] {
            assert!(
                !css.contains(forbidden),
                "{}: stylesheet contains {forbidden}",
                preset.id
            );
        }
    }
}

/// Selectors the rendered markup and the behavior script depend on; renaming
/// either side must fail loudly here, not silently unstyle published sites.
#[test]
fn contract_selectors_are_styled() {
    let css = stylesheet(&SiteTheme::new());
    for selector in [
        ".skip-link:focus",
        ".hp",
        ".s-nav",
        ".js .nav-toggle",
        ".js .s-nav .nav-toggle[aria-expanded=\"true\"] + ul",
        ".s-hero",
        ".s-features .grid li",
        ".s-text-image.image-right figure",
        ".s-gallery img",
        ".s-testimonials ul",
        ".tier.highlighted",
        ".s-team .grid li",
        ".s-faq details",
        "main > .s-cta",
        ".s-contact-form textarea",
        ".form-success",
        ".s-footer",
    ] {
        assert!(css.contains(selector), "missing selector {selector}");
    }
    // Responsive and no-JS guarantees: a breakpoint exists, images are
    // contained, and the mobile menu only collapses under the script's class.
    assert!(css.contains("@media (max-width:"));
    assert!(css.contains("@media (min-width:"));
    assert!(css.contains("img { max-width: 100%;"));
    assert!(!css.contains("\n.s-nav ul { display: none"));
}

/// A theme the read path could only produce defensively (pristine `{}`)
/// still yields the default sheet — stylesheet generation never fails.
#[test]
fn pristine_theme_reads_as_the_default_sheet() {
    let pristine = SiteTheme::from_stored(json!({}));
    assert_eq!(stylesheet(&pristine), stylesheet(&SiteTheme::new()));
    assert!(
        stylesheet(&pristine).starts_with("/* alo Sites stylesheet — theme preset \"north\" */")
    );
}
