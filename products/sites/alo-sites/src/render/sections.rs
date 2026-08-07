//! One HTML fragment builder per section type.
//!
//! Markup is semantic and CSS-free: landmarks and heading levels carry the
//! structure (`h1` only in a hero, `h2` per section, `h3` per item), classes
//! are stable `s-<kind>` hooks for the generated stylesheet, and every
//! `<img>` carries `alt` (empty means decorative, straight from the model).
//! All text goes through [`esc`], every link target through [`safe_href`].

use alo_store::site_model::{
    ContactFormSection, CtaSection, FaqSection, FeaturesSection, FooterSection, GallerySection,
    HeroSection, ImageSide, Link, NavSection, PricingSection, Section, SiteImage, TeamSection,
    TestimonialsSection, TextImageSection,
};

use super::SiteRenderContext;
use super::html::{esc, img_src, safe_href};

/// A `nav` section, rendered as a `<header>` landmark. The brand link shows
/// the theme logo when one is set, the site name otherwise; the toggle
/// button is inert markup until the stylesheet slice ships its script.
pub(super) fn nav(out: &mut String, site: &SiteRenderContext<'_>, s: &NavSection, index: usize) {
    let menu_id = format!("nav-menu-{index}");
    out.push_str("<header class=\"s-nav\">\n");
    out.push_str(&format!(
        "<nav aria-label=\"{}\">\n",
        esc(site.strings.nav_label)
    ));
    match &site.theme.logo {
        Some(logo) => out.push_str(&format!(
            "<a class=\"brand\" href=\"/\"><img class=\"logo\" src=\"{}\" alt=\"{}\"></a>\n",
            img_src(logo.as_str()),
            esc(site.name)
        )),
        None => out.push_str(&format!(
            "<a class=\"brand\" href=\"/\">{}</a>\n",
            esc(site.name)
        )),
    }
    out.push_str(&format!(
        "<button class=\"nav-toggle\" type=\"button\" aria-expanded=\"false\" aria-controls=\"{menu_id}\">{}</button>\n",
        esc(site.strings.menu)
    ));
    out.push_str(&format!("<ul id=\"{menu_id}\">\n"));
    for link in &s.links {
        out.push_str("<li>");
        push_link(out, link, "");
        out.push_str("</li>\n");
    }
    if let Some(cta) = &s.cta {
        out.push_str("<li>");
        push_link(out, cta, "button");
        out.push_str("</li>\n");
    }
    out.push_str("</ul>\n</nav>\n</header>\n");
}

/// A `footer` section, rendered as a `<footer>` landmark.
pub(super) fn footer(out: &mut String, site: &SiteRenderContext<'_>, s: &FooterSection) {
    out.push_str("<footer class=\"s-footer\">\n");
    if !s.links.is_empty() {
        out.push_str(&format!(
            "<nav aria-label=\"{}\">\n<ul>\n",
            esc(site.strings.footer_nav_label)
        ));
        for link in &s.links {
            out.push_str("<li>");
            push_link(out, link, "");
            out.push_str("</li>\n");
        }
        out.push_str("</ul>\n</nav>\n");
    }
    if let Some(text) = &s.text {
        out.push_str(&format!("<p>{}</p>\n", esc(text)));
    }
    out.push_str("</footer>\n");
}

/// Every section that lives inside `<main>`. Nav/footer never reach here
/// (the document assembler routes them to their landmarks); if one ever
/// does, it renders in place rather than vanish.
pub(super) fn body_section(
    out: &mut String,
    site: &SiteRenderContext<'_>,
    section: &Section,
    index: usize,
) {
    match section {
        Section::Nav(s) => nav(out, site, s, index),
        Section::Footer(s) => footer(out, site, s),
        Section::Hero(s) => hero(out, s),
        Section::Features(s) => features(out, s),
        Section::TextImage(s) => text_image(out, s),
        Section::Gallery(s) => gallery(out, s),
        Section::Testimonials(s) => testimonials(out, s),
        Section::Pricing(s) => pricing(out, s),
        Section::Team(s) => team(out, s),
        Section::Faq(s) => faq(out, s),
        Section::Cta(s) => cta(out, s),
        Section::ContactForm(s) => contact_form(out, site, s, index),
    }
}

fn hero(out: &mut String, s: &HeroSection) {
    out.push_str("<section class=\"s-hero\">\n");
    out.push_str(&format!("<h1>{}</h1>\n", esc(&s.heading)));
    if let Some(subheading) = &s.subheading {
        out.push_str(&format!(
            "<p class=\"subheading\">{}</p>\n",
            esc(subheading)
        ));
    }
    if s.primary_cta.is_some() || s.secondary_cta.is_some() {
        out.push_str("<p class=\"actions\">");
        if let Some(link) = &s.primary_cta {
            push_link(out, link, "button");
        }
        if let Some(link) = &s.secondary_cta {
            push_link(out, link, "button secondary");
        }
        out.push_str("</p>\n");
    }
    if let Some(image) = &s.image {
        push_figure(out, image);
    }
    out.push_str("</section>\n");
}

fn features(out: &mut String, s: &FeaturesSection) {
    out.push_str("<section class=\"s-features\">\n");
    push_opt_heading(out, s.heading.as_deref());
    if let Some(intro) = &s.intro {
        out.push_str(&format!("<p class=\"intro\">{}</p>\n", esc(intro)));
    }
    out.push_str("<ul class=\"grid\">\n");
    for item in &s.items {
        out.push_str(&format!(
            "<li>\n<h3>{}</h3>\n<p>{}</p>\n</li>\n",
            esc(&item.title),
            esc(&item.body)
        ));
    }
    out.push_str("</ul>\n</section>\n");
}

fn text_image(out: &mut String, s: &TextImageSection) {
    let side = match s.image_side {
        ImageSide::Left => "image-left",
        ImageSide::Right => "image-right",
    };
    out.push_str(&format!("<section class=\"s-text-image {side}\">\n"));
    push_figure(out, &s.image);
    out.push_str("<div class=\"text\">\n");
    push_opt_heading(out, s.heading.as_deref());
    out.push_str(&format!("<p>{}</p>\n", esc(&s.body)));
    out.push_str("</div>\n</section>\n");
}

fn gallery(out: &mut String, s: &GallerySection) {
    out.push_str("<section class=\"s-gallery\">\n");
    push_opt_heading(out, s.heading.as_deref());
    out.push_str("<ul class=\"grid\">\n");
    for image in &s.images {
        out.push_str("<li>");
        push_figure(out, image);
        out.push_str("</li>\n");
    }
    out.push_str("</ul>\n</section>\n");
}

fn testimonials(out: &mut String, s: &TestimonialsSection) {
    out.push_str("<section class=\"s-testimonials\">\n");
    push_opt_heading(out, s.heading.as_deref());
    out.push_str("<ul>\n");
    for item in &s.items {
        out.push_str("<li>\n<figure class=\"testimonial\">\n");
        out.push_str(&format!(
            "<blockquote><p>{}</p></blockquote>\n",
            esc(&item.quote)
        ));
        out.push_str(&format!("<figcaption>{}", esc(&item.author)));
        if let Some(role) = &item.role {
            out.push_str(&format!(" <span class=\"role\">{}</span>", esc(role)));
        }
        out.push_str("</figcaption>\n</figure>\n</li>\n");
    }
    out.push_str("</ul>\n</section>\n");
}

fn pricing(out: &mut String, s: &PricingSection) {
    out.push_str("<section class=\"s-pricing\">\n");
    push_opt_heading(out, s.heading.as_deref());
    if let Some(intro) = &s.intro {
        out.push_str(&format!("<p class=\"intro\">{}</p>\n", esc(intro)));
    }
    out.push_str("<ul class=\"tiers\">\n");
    for tier in &s.tiers {
        let class = if tier.highlighted {
            "tier highlighted"
        } else {
            "tier"
        };
        out.push_str(&format!("<li class=\"{class}\">\n"));
        out.push_str(&format!("<h3>{}</h3>\n", esc(&tier.name)));
        out.push_str(&format!("<p class=\"price\">{}", esc(&tier.price)));
        if let Some(period) = &tier.period {
            out.push_str(&format!(" <span class=\"period\">{}</span>", esc(period)));
        }
        out.push_str("</p>\n");
        if let Some(description) = &tier.description {
            out.push_str(&format!(
                "<p class=\"description\">{}</p>\n",
                esc(description)
            ));
        }
        if !tier.features.is_empty() {
            out.push_str("<ul class=\"tier-features\">\n");
            for feature in &tier.features {
                out.push_str(&format!("<li>{}</li>\n", esc(feature)));
            }
            out.push_str("</ul>\n");
        }
        if let Some(link) = &tier.cta {
            push_link(out, link, "button");
            out.push('\n');
        }
        out.push_str("</li>\n");
    }
    out.push_str("</ul>\n</section>\n");
}

fn team(out: &mut String, s: &TeamSection) {
    out.push_str("<section class=\"s-team\">\n");
    push_opt_heading(out, s.heading.as_deref());
    out.push_str("<ul class=\"grid\">\n");
    for member in &s.members {
        out.push_str("<li>\n");
        if let Some(photo) = &member.photo {
            push_figure(out, photo);
        }
        out.push_str(&format!("<h3>{}</h3>\n", esc(&member.name)));
        if let Some(role) = &member.role {
            out.push_str(&format!("<p class=\"role\">{}</p>\n", esc(role)));
        }
        if let Some(bio) = &member.bio {
            out.push_str(&format!("<p class=\"bio\">{}</p>\n", esc(bio)));
        }
        out.push_str("</li>\n");
    }
    out.push_str("</ul>\n</section>\n");
}

fn faq(out: &mut String, s: &FaqSection) {
    out.push_str("<section class=\"s-faq\">\n");
    push_opt_heading(out, s.heading.as_deref());
    for item in &s.items {
        // <details>/<summary> is a native, scriptless accordion.
        out.push_str(&format!(
            "<details>\n<summary>{}</summary>\n<p>{}</p>\n</details>\n",
            esc(&item.question),
            esc(&item.answer)
        ));
    }
    out.push_str("</section>\n");
}

fn cta(out: &mut String, s: &CtaSection) {
    out.push_str("<section class=\"s-cta\">\n");
    out.push_str(&format!("<h2>{}</h2>\n", esc(&s.heading)));
    if let Some(body) = &s.body {
        out.push_str(&format!("<p>{}</p>\n", esc(body)));
    }
    out.push_str("<p class=\"actions\">");
    push_link(out, &s.button, "button");
    out.push_str("</p>\n</section>\n");
}

/// The contact form. Without a `form_id` the section renders its text only —
/// "the section without a working submit". With one, the form posts to
/// `/f/<form_id>` and carries the fixed v1 field contract — `name`, `email`,
/// `message`, plus the visually-hidden `website` honeypot (a submission
/// filling it is bot traffic, silently dropped by the forms backend).
fn contact_form(
    out: &mut String,
    site: &SiteRenderContext<'_>,
    s: &ContactFormSection,
    index: usize,
) {
    out.push_str("<section class=\"s-contact-form\">\n");
    push_opt_heading(out, s.heading.as_deref());
    if let Some(body) = &s.body {
        out.push_str(&format!("<p>{}</p>\n", esc(body)));
    }
    if let Some(form_id) = &s.form_id {
        let t = site.strings;
        out.push_str(&format!(
            "<form action=\"/f/{}\" method=\"post\"",
            esc(form_id)
        ));
        if let Some(success) = &s.success_message {
            out.push_str(&format!(" data-success=\"{}\"", esc(success)));
        }
        out.push_str(">\n");
        out.push_str(&format!(
            "<p class=\"hp\" aria-hidden=\"true\"><label for=\"form-{index}-website\">{}</label><input id=\"form-{index}-website\" name=\"website\" type=\"text\" tabindex=\"-1\" autocomplete=\"off\"></p>\n",
            esc(t.form_website)
        ));
        out.push_str(&format!(
            "<p><label for=\"form-{index}-name\">{}</label><input id=\"form-{index}-name\" name=\"name\" type=\"text\" required maxlength=\"300\"></p>\n",
            esc(t.form_name)
        ));
        out.push_str(&format!(
            "<p><label for=\"form-{index}-email\">{}</label><input id=\"form-{index}-email\" name=\"email\" type=\"email\" required maxlength=\"320\"></p>\n",
            esc(t.form_email)
        ));
        out.push_str(&format!(
            "<p><label for=\"form-{index}-message\">{}</label><textarea id=\"form-{index}-message\" name=\"message\" required maxlength=\"5000\"></textarea></p>\n",
            esc(t.form_message)
        ));
        out.push_str(&format!(
            "<p><button type=\"submit\">{}</button></p>\n",
            esc(t.form_send)
        ));
        out.push_str("</form>\n");
    }
    out.push_str("</section>\n");
}

// ---- shared fragments -------------------------------------------------------

/// `<a>` with a safe href; `class` may be empty.
fn push_link(out: &mut String, link: &Link, class: &str) {
    if class.is_empty() {
        out.push_str(&format!(
            "<a href=\"{}\">{}</a>",
            safe_href(&link.href),
            esc(&link.label)
        ));
    } else {
        out.push_str(&format!(
            "<a class=\"{class}\" href=\"{}\">{}</a>",
            safe_href(&link.href),
            esc(&link.label)
        ));
    }
}

/// `<figure><img></figure>` — `alt` is always present; empty means the model
/// marked the image decorative.
fn push_figure(out: &mut String, image: &SiteImage) {
    out.push_str(&format!(
        "<figure><img src=\"{}\" alt=\"{}\"></figure>\n",
        img_src(image.blob_id.as_str()),
        esc(&image.alt)
    ));
}

/// The section's optional `<h2>`.
fn push_opt_heading(out: &mut String, heading: Option<&str>) {
    if let Some(heading) = heading {
        out.push_str(&format!("<h2>{}</h2>\n", esc(heading)));
    }
}
