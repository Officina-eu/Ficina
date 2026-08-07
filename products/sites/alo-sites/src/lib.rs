//! alo Sites public serving (ADR 0036).
//!
//! This crate is a **library first**: the pure [`render`] module turns a
//! page's typed sections plus a site's theme into one complete static HTML
//! document, and [`stylesheet`] turns the same theme into the one CSS file
//! that document links. The [`serve`] module is the public `alo-sites`
//! service (the Host-resolving axum server, built by `src/main.rs`) serving
//! that output from published snapshots; `alo-jmap` reuses the same library
//! for the authenticated draft preview — one renderer, so preview and
//! production HTML cannot drift (`docs/design/sites.md`).
//!
//! What this crate owns: rendering and anonymous public serving of published
//! sites. What it does not own: editing, storage, or anything authenticated —
//! that is `alo-jmap` over `alo-store`. It talks to `platform/alo-store` only
//! (types, and the read-only `SitePublicStore` door) and never to another
//! product.
//!
//! ## Public-path contract of a rendered site
//!
//! Rendered documents reference exactly three server paths, all site-relative
//! (same-origin — a published page performs **zero cross-origin requests**,
//! which is part of the product's privacy promise):
//!
//! - `/assets/site.css` — the one stylesheet, generated from the site's theme
//!   tokens.
//! - `/assets/img/<blob_id>` — image blobs of the resolved site's tenant.
//! - `/f/<form_id>` — contact-form submission target.
//!
//! These are a contract: the service must serve them, and changing them means
//! re-rendering every published snapshot. The page's only JavaScript (menu
//! toggle + form submit) is a static block **inlined** in the document, not a
//! fourth path — see `render::script`.

pub mod render;
pub mod serve;
pub mod stylesheet;
