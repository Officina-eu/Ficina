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
const DEV_API_PATHS = [
  "/jmap", "/ai", "/admin", "/settings", "/docs", "/snooze", "/send-later",
  "/calendar", "/tasks", "/spaces", "/drive", "/wopi", "/search", "/contacts",
  "/import", "/signup", "/reset", "/autodiscover", "/Autodiscover", "/dav",
  "/filters", "/share", "/oauth", "/.well-known", "/auth",
  // Collabora, so Office files open in the local dev app too.
  "/hosting", "/browser", "/cool", "/lool",
];
const devProxy = Object.fromEntries(
  DEV_API_PATHS.map((p) => [
    p,
    { target: DEV_API, changeOrigin: true, secure: true, ws: true },
  ]),
);

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
