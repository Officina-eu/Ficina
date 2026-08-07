//! Visitor-facing chrome strings of a rendered site, per locale.
//!
//! Section content is the tenant's own words; these are the few strings the
//! *renderer* contributes (skip link, menu button, form labels). They are
//! externalized here — never inline in the markup builders — so more locales
//! are a new const, not a code hunt. English ships now; fr/nl land at the
//! wave review, and site-level locale selection arrives with them.

/// The renderer-contributed strings for one locale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiStrings {
    /// BCP 47 tag for `<html lang>`.
    pub lang: &'static str,
    /// Skip-navigation link text (first focusable element).
    pub skip_to_content: &'static str,
    /// `aria-label` of the top navigation landmark.
    pub nav_label: &'static str,
    /// `aria-label` of the footer links landmark.
    pub footer_nav_label: &'static str,
    /// Mobile menu toggle button text.
    pub menu: &'static str,
    /// Contact form: name field label.
    pub form_name: &'static str,
    /// Contact form: email field label.
    pub form_email: &'static str,
    /// Contact form: message field label.
    pub form_message: &'static str,
    /// Contact form: honeypot field label (visually hidden; a real visitor
    /// never sees it, but it must read as a plausible field to a bot).
    pub form_website: &'static str,
    /// Contact form: submit button text.
    pub form_send: &'static str,
    /// Contact form: confirmation shown after a successful submission when
    /// the section sets no custom `success_message`.
    pub form_success: &'static str,
}

/// English chrome strings — the v1 default.
pub const EN: UiStrings = UiStrings {
    lang: "en",
    skip_to_content: "Skip to content",
    nav_label: "Main",
    footer_nav_label: "Footer",
    menu: "Menu",
    form_name: "Name",
    form_email: "Email",
    form_message: "Message",
    form_website: "Website",
    form_send: "Send",
    form_success: "Thanks — your message has been sent.",
};
