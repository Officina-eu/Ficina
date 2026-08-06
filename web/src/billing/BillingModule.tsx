// The Billing module (alo Billing, ADR 0035, wave B1) — the workspace surface
// over the `/billing` API. This is the skeleton the rest of the wave hangs
// from: a header, a tab per record type, and a nested route each, so an
// invoice list and a quote list (B1.14–B1.15) are new tabs rather than a new
// navigation idea.
//
// It is mounted at `/billing/*` by the product surface, so every path below is
// relative and a deep link (`/billing/products`) survives a page reload.
import { NavLink, Navigate, Route, Routes } from "react-router-dom";

import { strings } from "../i18n";
import { CustomersView } from "./CustomersView";
import { ProductsView } from "./ProductsView";
import styles from "./BillingModule.module.css";

/** The tabs, in the order a tenant meets them: who you bill, then what for. */
const TABS = [
  { path: "customers", label: () => strings.billingCustomers },
  { path: "products", label: () => strings.billingProducts },
] as const;

export function BillingModule() {
  return (
    <div className={styles.billing}>
      <header className={styles.header}>
        <h1 className={styles.title}>{strings.moduleBilling}</h1>
        <nav className={styles.tabs}>
          {TABS.map((t) => (
            <NavLink
              key={t.path}
              to={t.path}
              className={({ isActive }) => (isActive ? `${styles.tab} ${styles.tabActive}` : styles.tab)}
            >
              {t.label()}
            </NavLink>
          ))}
        </nav>
      </header>

      <Routes>
        <Route index element={<Navigate to="customers" replace />} />
        <Route path="customers" element={<CustomersView />} />
        <Route path="products" element={<ProductsView />} />
        {/* An unknown billing path is a stale link, not an error page. */}
        <Route path="*" element={<Navigate to="customers" replace />} />
      </Routes>
    </div>
  );
}
