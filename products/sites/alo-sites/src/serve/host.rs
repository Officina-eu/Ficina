//! Host header → site subdomain, the resolution step that scopes a public
//! request to one tenant's site (`docs/design/sites.md`, Tenancy). Strict on
//! purpose: only `<label>.<SITES_DOMAIN>` where the label passes the store's
//! subdomain rules resolves — nested labels, the apex itself, IP literals,
//! and every other host fall through to the generic not-found. Custom
//! domains are a later slice (S1.25) and land here as a second lookup.

/// Extracts the site subdomain from a request's Host header value, given the
/// configured apex (already lowercase). Ports and a trailing FQDN dot are
/// ignored; matching is case-insensitive; anything that is not exactly one
/// valid subdomain label under the apex is `None`.
pub fn subdomain(host: &str, sites_domain: &str) -> Option<String> {
    let host = host.trim();
    // An IPv6 authority (`[::1]:8081`) is never a site host.
    if host.starts_with('[') {
        return None;
    }
    let host = match host.rsplit_once(':') {
        Some((name, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => name,
        _ => host,
    };
    let host = host.strip_suffix('.').unwrap_or(host).to_ascii_lowercase();
    let label = host.strip_suffix(sites_domain)?.strip_suffix('.')?;
    // The store's rules also exclude `.`, so `a.b.<apex>` can never match.
    alo_store::validate_subdomain(label).ok()?;
    Some(label.to_owned())
}

#[cfg(test)]
mod tests {
    use super::subdomain;

    const APEX: &str = "alosites.test";

    #[test]
    fn resolves_exactly_one_valid_label_under_the_apex() {
        assert_eq!(
            subdomain("acme.alosites.test", APEX).as_deref(),
            Some("acme")
        );
        assert_eq!(
            subdomain("ACME.AloSites.Test", APEX).as_deref(),
            Some("acme"),
            "host matching is case-insensitive"
        );
        assert_eq!(
            subdomain("acme.alosites.test:8081", APEX).as_deref(),
            Some("acme"),
            "a port is ignored"
        );
        assert_eq!(
            subdomain("acme.alosites.test.", APEX).as_deref(),
            Some("acme"),
            "a trailing FQDN dot is ignored"
        );
    }

    #[test]
    fn everything_else_falls_through() {
        // The apex itself, nested labels, other domains, lookalike suffixes.
        assert_eq!(subdomain("alosites.test", APEX), None);
        assert_eq!(subdomain("a.b.alosites.test", APEX), None);
        assert_eq!(subdomain("acme.example.com", APEX), None);
        assert_eq!(subdomain("acme.evilalosites.test", APEX), None);
        // Labels the store would never have admitted.
        assert_eq!(subdomain("-x-.alosites.test", APEX), None);
        assert_eq!(
            subdomain("ab.alosites.test", APEX),
            None,
            "below min length"
        );
        // Degenerate authorities.
        assert_eq!(subdomain("", APEX), None);
        assert_eq!(subdomain("[::1]:8081", APEX), None);
        assert_eq!(subdomain("127.0.0.1:8081", APEX), None);
        assert_eq!(subdomain(".alosites.test", APEX), None);
    }
}
