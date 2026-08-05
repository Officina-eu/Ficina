import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";
import { fileURLToPath } from "node:url";

// Product selection (ADR 0019): the whole app is defined by one product
// surface. `@product` resolves to it — the full workspace by default, the
// mail-only surface when built with ALO_PRODUCT=mail (alomails), or the
// Drive-only surface with ALO_PRODUCT=drive (alodrives).
const ALO_PRODUCT = process.env.ALO_PRODUCT;
const product =
  ALO_PRODUCT === "mail" ? "mail" : ALO_PRODUCT === "drive" ? "drive" : "workplace";

// Browser-tab brand name per product. A proper-noun brand, not translatable
// copy — so it lives here (like the `alo` wordmark) rather than in i18n, and
// is stamped into index.html at build time so the tab is correct before JS
// loads. Keep in step with the marketed product name.
const productTitle: Record<typeof product, string> = {
  workplace: "alo workplace",
  mail: "alomails",
  drive: "alodrives",
};

// Local dev backend. `npm run dev` serves the UI from Vite but the app calls its
// API same-origin, so in dev we proxy the API (and Collabora) path prefixes to a
// real alo server — the live server by default, overridable with VITE_DEV_API
// (e.g. a local jmap on http://localhost:8080). Auth is bearer-token in
// localStorage (no cookies), so changeOrigin is all that's needed.
const DEV_API = process.env.VITE_DEV_API ?? "https://mail.alomails.com";

// Several API prefixes (/drive, /tasks, /admin, …) are ALSO client-side route
// paths. A browser page-load of one (refresh on /drive) sends `Accept: text/html`
// and must be served by Vite's own dev shell — proxying it would return the
// live server's PRODUCTION index.html, whose hashed asset URLs the dev server
// doesn't have → a blank page. So on an HTML navigation we bypass the proxy and
// let Vite serve index.html (the SPA); real API/XHR calls (Accept: */* etc.)
// proxy through as normal.
const spaBypass = (req: { headers: Record<string, string | string[] | undefined> }) => {
  const accept = req.headers.accept;
  if (typeof accept === "string" && accept.includes("text/html")) return "/index.html";
  return undefined;
};

const API_PATHS = [
  "/jmap", "/ai", "/admin", "/settings", "/docs", "/snooze", "/send-later",
  "/calendar", "/tasks", "/spaces", "/drive", "/search", "/contacts",
  "/import", "/signup", "/reset", "/autodiscover", "/Autodiscover", "/dav",
  "/filters", "/share", "/oauth", "/.well-known", "/auth",
];
// Collabora paths are loaded by the editor itself (and its server), never a
// user page navigation — proxy them straight through with no SPA bypass.
const COLLABORA_PATHS = ["/wopi", "/hosting", "/browser", "/cool", "/lool"];

const base = { target: DEV_API, changeOrigin: true, secure: true, ws: true };
const devProxy = {
  ...Object.fromEntries(API_PATHS.map((p) => [p, { ...base, bypass: spaBypass }])),
  ...Object.fromEntries(COLLABORA_PATHS.map((p) => [p, { ...base }])),
};

export default defineConfig({
  plugins: [
    react(),
    {
      name: "alo-product-title",
      transformIndexHtml(html) {
        return html.replace(
          /<title>[^<]*<\/title>/,
          `<title>${productTitle[product]}</title>`,
        );
      },
    },
  ],
  resolve: {
    alias: {
      "@product": fileURLToPath(new URL(`./src/product/${product}.tsx`, import.meta.url)),
    },
  },
  server: {
    proxy: devProxy,
  },
  test: {
    environment: "jsdom",
  },
});
