# Design note — the alo web shell (product foundation)

Status: building · 2026-07 · ROADMAP Phase 2, "Web app shell" (first item)

The web shell is the **one-product frame** every module renders inside:
Mail first, then Agenda, Chat, Meet, Drive, Docs. It owns the visual identity,
the navigation rail, the login flow, and the identity/session context — the
shared foundation, not a mail-only screen. ADR 0005 (one TypeScript web app)
and ADR 0008 (identity/OIDC) are the governing decisions; this note records
*how* the shell is structured so later modules plug in without reshaping it.

## Surface

- **Inputs:** a browser session. On first load with no session, the shell
  shows the login screen; after login it shows the active module (default
  Mail).
- **Outputs:** the rendered workspace — left rail (logo, ＋New, module icons,
  ✦AI, avatar) plus the active module's panels.
- **Who calls it:** end users in a browser (PWA later). It talks to two live
  backends at the same origin: `alo-identity` (OIDC, for login) and
  `alo-jmap` (JMAP, for mail data).

### Module plug-in seam

`shell/moduleRegistry.ts` is a list of `{ id, path, labelKey, icon,
element, enabled }`. The rail renders one button per entry; the router mounts
each entry's `element` at its `path` inside the shell layout. **Adding
Agenda/Chat/Drive/Docs later is one registry entry + one area folder** — the
shell, rail, auth, and layout do not change. Modules not yet built are
registered with `enabled: false` and render a "coming soon" placeholder, so
the rail already shows the whole suite (the one-product promise) while only
Mail is live.

### Areas (one responsibility per folder, per new-component skill)

```
web/src/
├── ds/          design system: tokens.css, global.css, one primitive per file
├── shell/       AppShell layout, Rail, module registry, user menu
├── auth/        OIDC-PKCE client, AuthProvider/useAuth, LoginPage, RequireAuth
├── jmap/        shared JMAP client (session + request), typed
├── mail/        the Mail module: api/ components/ state/ index.ts
└── i18n/        externalized strings (translation catalog source)
```

Cross-area imports go through each area's `index.ts`. Shared UI lives in `ds/`,
never copy-pasted.

## Login flow (first-party OIDC + PKCE)

`alo-identity` exposes `authorization_code` + **PKCE S256** and treats our
web app as a **public client** (no secret). Its `/oauth/authorize` accepts the
username/password/OTP as a POST form — i.e. **the web app renders the login
form itself** (first-party pattern; the IdP has no its-own login page). Flow:

1. User enters email + password (+ TOTP code if enrolled) in our LoginPage.
2. We generate a PKCE `verifier`/`challenge` (Web Crypto, S256) and `state`.
3. POST them with the credentials to `/oauth/authorize`.
   - `401 access_denied "second factor required"` → reveal the OTP field and
     let the user submit the code; re-POST.
   - `401 access_denied "invalid credentials"` → inline error.
4. Success returns a 302 to our redirect URI with `?code=…`; we exchange the
   code at `/oauth/token` with the PKCE `verifier` for tokens.
5. The ID token (EdDSA) identifies the user; the opaque access token
   authorizes JMAP calls (`Authorization: Bearer`).

Redirect URI: `https://<domain>/auth/callback` (a client-side route).
Registered once with `identityctl register-client web "alo Web"
https://<domain>/auth/callback`.

**Token storage (v1, with a documented hardening path):** access token in
memory; refresh token in `sessionStorage`; a strict CSP; PKCE throughout; the
server rotates refresh tokens with replay-chain revocation (a stolen refresh
token is detected on reuse). This is proportionate for a first-party SPA. The
named next step is a **backend-for-frontend** that holds tokens in an httpOnly
cookie so no token is reachable from JS — recorded here, promoted to an ADR
when built.

## Errors

- **No session / expired token:** the shell routes to LoginPage; in-flight
  JMAP `401` triggers one refresh attempt, then a clean re-login (never a
  spinner that hangs).
- **Bad credentials / 2FA required:** shown inline on the login form (mapped
  from the OIDC error), never a raw server message.
- **Backend unreachable:** each module surface shows a retryable error state,
  not a blank screen. No message body, password, or token is ever logged.

## Tenancy

The client holds **no cross-tenant data by construction**: every JMAP request
carries the user's bearer token, and `alo-jmap` scopes every read/write to
that token's `(tenant, user)` at the store (the isolation is enforced and
tested server-side — ADR 0008, the wrong-tenant suite). The shell only ever
renders what the authenticated account's own JMAP responses return; there is
no client-side tenant selection to get wrong.

## Out of scope (this pass — recorded, not forgotten)

- Offline/PWA service worker, push notifications (ROADMAP Phase 2 later item).
- The backend-for-frontend token handler (hardening path above).
- Agenda/Chat/Meet/Drive/Docs module bodies — registered as placeholders; the
  seam is proven by Mail being the first real tenant of it.
- Full mail UX (compose richness, drag-drop, snooze, rule builder) — this pass
  delivers read: folders → message list → reading pane. Compose/organize are
  the next ROADMAP items, built on this foundation.
- Real i18n tooling (ICU/extraction). Strings are externalized in `i18n/`
  from day one; the tooling that turns the catalog into locales lands with the
  translation pass.

**Rejected alternative:** a mail-only screen now, generalized "later." Rejected
because the roadmap item is explicitly the *shared frame*, and retrofitting a
rail/auth/layout around a mail-shaped app is exactly the untangling the
one-file-one-responsibility rule exists to avoid. The frame is cheaper to
build first than to extract later.
