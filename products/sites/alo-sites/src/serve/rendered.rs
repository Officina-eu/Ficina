//! One publish, rendered: the exact bytes the service serves for a site —
//! every page document, the stylesheet, and the site's not-found page —
//! built once from the frozen snapshots and shared immutably from the cache.

use std::collections::HashMap;

use alo_store::site_theme::SiteTheme;
use alo_store::{PublishedSite, SitePageSnapshot, SitePublishId};

use crate::render::{self, EN, PageRenderContext, SiteRenderContext, UiStrings};
use crate::stylesheet;

/// The servable output of one publish of one site.
pub struct RenderedSite {
    /// The publish these bytes were rendered from — the cache-validity key
    /// and the substance of the pages' `ETag`s.
    pub publish: SitePublishId,
    /// Complete HTML documents by site-relative path (`/`, `/about`, …).
    pages: HashMap<String, String>,
    /// The one stylesheet, served at `/assets/site.css`.
    pub css: String,
    /// The site's themed not-found document (status 404, any unknown path).
    pub not_found: String,
}

impl RenderedSite {
    /// Renders every frozen page of `site`'s current publish. `subdomain` and
    /// `sites_domain` form the canonical origin (`https://<sub>.<apex>`) used
    /// for canonical/OG URLs — public sites are always addressed on HTTPS,
    /// whatever local socket served them.
    #[must_use]
    pub fn build(
        subdomain: &str,
        sites_domain: &str,
        site: &PublishedSite,
        snapshots: &[SitePageSnapshot],
    ) -> Self {
        let theme = SiteTheme::from_stored(site.theme.clone());
        let base_url = format!("https://{subdomain}.{sites_domain}");
        let ctx = SiteRenderContext {
            name: &site.name,
            base_url: &base_url,
            theme: &theme,
            strings: &EN,
        };
        let mut pages = HashMap::with_capacity(snapshots.len());
        for snapshot in snapshots {
            let path = if snapshot.is_home {
                "/".to_owned()
            } else {
                format!("/{}", snapshot.slug)
            };
            let page = PageRenderContext {
                path: &path,
                title: &snapshot.title,
                seo_title: snapshot.seo_title.as_deref(),
                seo_description: snapshot.seo_description.as_deref(),
                sections: &snapshot.sections,
            };
            pages.insert(path.clone(), render::render_page(&ctx, &page));
        }
        Self {
            publish: site.publish.clone(),
            pages,
            css: stylesheet::stylesheet(&theme),
            not_found: render::render_not_found(&ctx),
        }
    }

    /// The rendered document at `path`, if the publish has a page there.
    #[must_use]
    pub fn page(&self, path: &str) -> Option<&str> {
        self.pages.get(path).map(String::as_str)
    }
}

/// The generic not-found document for hosts that resolve to no live site.
/// One body for every miss — unknown subdomain, never-published site,
/// unpublished site, foreign domain — so the response can not leak whether
/// a tenant or site exists. Self-contained (no stylesheet path resolves
/// here), and built once at startup.
#[must_use]
pub fn unknown_host_not_found(strings: &UiStrings) -> String {
    format!(
        "<!doctype html>\n<html lang=\"{lang}\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{title}</title>\n<meta name=\"robots\" content=\"noindex\">\n\
         <style>body{{font-family:system-ui,sans-serif;display:grid;min-height:100vh;\
         margin:0;place-items:center;background:#fafafa;color:#1a1a1a}}\
         main{{text-align:center;padding:2rem}}h1{{font-size:1.5rem}}</style>\n\
         </head>\n<body>\n<main>\n<h1>{title}</h1>\n<p>{text}</p>\n</main>\n</body>\n</html>\n",
        lang = strings.lang,
        title = strings.not_found_title,
        text = strings.not_found_text,
    )
}
