//! Golden-HTML pinning of the renderer: one golden per section type plus a
//! full-page golden of a themed site carrying all twelve sections. Run with
//! `UPDATE_GOLDENS=1` to re-bless after a deliberate markup change, then
//! review the diff like any code change.
#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::PathBuf;

use alo_sites::render::{EN, PageRenderContext, SiteRenderContext, render_page};
use alo_store::id::BlobId;
use alo_store::site_model::{
    ContactFormSection, CtaSection, FaqItem, FaqSection, FeatureItem, FeaturesSection,
    FooterSection, GallerySection, HeroSection, ImageSide, Link, NavSection, PricingSection,
    PricingTier, SECTIONS_SCHEMA_VERSION, Section, SectionsEnvelope, SiteImage, TeamMember,
    TeamSection, Testimonial, TestimonialsSection, TextImageSection,
};
use alo_store::site_theme::SiteTheme;
use serde_json::json;

const SITE_NAME: &str = "Nordwind Coffee Roasters";
const BASE_URL: &str = "https://nordwind.alosites.com";

/// One fully-populated instance of every section variant, mirroring the
/// store's schema-test corpus — deterministic content, deterministic ids.
fn full_sections() -> Vec<Section> {
    let image = SiteImage {
        blob_id: BlobId::new("9hK3vQ2mR8pT1xWz4bC5dg"),
        alt: "Roasting drum mid-batch".to_owned(),
    };
    let link = |label: &str, href: &str| Link {
        label: label.to_owned(),
        href: href.to_owned(),
    };
    vec![
        Section::Nav(NavSection {
            links: vec![link("Home", "/"), link("Pricing", "/pricing")],
            cta: Some(link("Order beans", "/order")),
        }),
        Section::Hero(HeroSection {
            heading: "Coffee roasted the morning it ships".to_owned(),
            subheading: Some("Small-batch roastery on the harbour".to_owned()),
            image: Some(image.clone()),
            primary_cta: Some(link("Shop roasts", "/shop")),
            secondary_cta: Some(link("Our story", "/about")),
        }),
        Section::Features(FeaturesSection {
            heading: Some("Why Nordwind".to_owned()),
            intro: Some("Three promises on every bag.".to_owned()),
            items: vec![FeatureItem {
                title: "Roasted to order".to_owned(),
                body: "Your batch goes in the drum after you order.".to_owned(),
                icon: Some("flame".to_owned()),
            }],
        }),
        Section::TextImage(TextImageSection {
            heading: Some("The roastery".to_owned()),
            body: "A 1962 Probat drum, rebuilt by hand.".to_owned(),
            image: image.clone(),
            image_side: ImageSide::Left,
        }),
        Section::Gallery(GallerySection {
            heading: Some("Inside the roastery".to_owned()),
            images: vec![image.clone()],
        }),
        Section::Testimonials(TestimonialsSection {
            heading: Some("What cafés say".to_owned()),
            items: vec![Testimonial {
                quote: "The freshest beans we've ever pulled shots with.".to_owned(),
                author: "Mara Lindqvist".to_owned(),
                role: Some("Head barista, Kaffebaren".to_owned()),
            }],
        }),
        Section::Pricing(PricingSection {
            heading: Some("Subscriptions".to_owned()),
            intro: Some("Pause or cancel any time.".to_owned()),
            tiers: vec![PricingTier {
                name: "Weekly".to_owned(),
                price: "€18/week".to_owned(),
                period: Some("billed weekly".to_owned()),
                description: Some("Two 250g bags every week.".to_owned()),
                features: vec!["Free shipping".to_owned(), "Roast-day dispatch".to_owned()],
                cta: Some(link("Start weekly", "/subscribe/weekly")),
                highlighted: true,
            }],
        }),
        Section::Team(TeamSection {
            heading: Some("The roasters".to_owned()),
            members: vec![TeamMember {
                name: "Jonas Meer".to_owned(),
                role: Some("Founder & head roaster".to_owned()),
                photo: Some(image.clone()),
                bio: Some("Twenty years at the drum.".to_owned()),
            }],
        }),
        Section::Faq(FaqSection {
            heading: Some("Questions".to_owned()),
            items: vec![FaqItem {
                question: "How fresh is the coffee?".to_owned(),
                answer: "It ships the day it is roasted.".to_owned(),
            }],
        }),
        Section::Cta(CtaSection {
            heading: "Taste the difference".to_owned(),
            body: Some("First bag ships free.".to_owned()),
            button: link("Order now", "/order"),
        }),
        Section::ContactForm(ContactFormSection {
            heading: Some("Wholesale enquiries".to_owned()),
            body: Some("We answer within one business day.".to_owned()),
            form_id: Some("f4K9sL2wN7qR5tYx8vB1cA".to_owned()),
            success_message: Some("Thanks — talk soon.".to_owned()),
        }),
        Section::Footer(FooterSection {
            text: Some("© Nordwind Coffee Roasters".to_owned()),
            links: vec![link("Imprint", "/imprint"), link("Privacy", "/privacy")],
        }),
    ]
}

fn envelope_value(sections: Vec<Section>) -> serde_json::Value {
    SectionsEnvelope {
        schema_version: SECTIONS_SCHEMA_VERSION,
        sections,
    }
    .to_value()
    .unwrap()
}

/// Renders `sections` as the home page of an untheme'd (default-preset) site.
fn render_default(sections: Vec<Section>) -> String {
    let theme = SiteTheme::new();
    let value = envelope_value(sections);
    let site = SiteRenderContext {
        name: SITE_NAME,
        base_url: BASE_URL,
        theme: &theme,
        strings: &EN,
    };
    let page = PageRenderContext {
        path: "/",
        title: "Home",
        seo_title: None,
        seo_description: None,
        sections: &value,
    };
    render_page(&site, &page)
}

fn assert_golden(name: &str, actual: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name);
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, actual).unwrap();
    }
    let expected = fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("missing golden {name}; run once with UPDATE_GOLDENS=1"));
    assert_eq!(
        expected, actual,
        "golden {name} drifted — if deliberate, re-bless with UPDATE_GOLDENS=1 and review the diff"
    );
}

#[test]
fn one_golden_per_section_type() {
    let sections = full_sections();
    assert_eq!(sections.len(), 12, "corpus must cover every variant");
    for section in sections {
        let kind = section.kind();
        let html = render_default(vec![section]);
        assert_golden(&format!("section_{kind}.html"), &html);
    }
}

#[test]
fn full_page_golden_with_theme_logo_and_seo() {
    let theme = SiteTheme::from_value(json!({
        "schema_version": 1,
        "preset": "terra",
        "logo": "L0g0aaaaaaaaaaaaaaaaaa",
        "favicon": "Fav1conaaaaaaaaaaaaaaa",
    }))
    .unwrap();
    let value = envelope_value(full_sections());
    let site = SiteRenderContext {
        name: SITE_NAME,
        base_url: BASE_URL,
        theme: &theme,
        strings: &EN,
    };
    let page = PageRenderContext {
        path: "/",
        title: "Home",
        seo_title: Some("Coffee roasted the morning it ships — Nordwind"),
        seo_description: Some("Small-batch coffee roastery on the harbour, shipping on roast day."),
        sections: &value,
    };
    assert_golden("full_page.html", &render_page(&site, &page));
}

#[test]
fn every_img_on_the_full_corpus_carries_alt() {
    let html = render_default(full_sections());
    let mut imgs = 0;
    for tag in html.split("<img").skip(1) {
        imgs += 1;
        let tag = tag.split('>').next().unwrap();
        assert!(
            tag.contains(" alt=\""),
            "an <img> is missing its alt attribute: <img{tag}>"
        );
    }
    assert!(
        imgs >= 4,
        "corpus should exercise several images, saw {imgs}"
    );
}
