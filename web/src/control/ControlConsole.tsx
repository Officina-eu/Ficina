// The platform control plane console (ADR 0012): a full-screen surface with
// its own nav, gated to platform operators (not tenant admins). Reached at
// /control; a non-operator sees a clear access-required card. Distinct from the
// tenant Admin console — this governs tenants across the whole deployment.
import { useEffect, useState } from "react";
import { Link, Navigate, NavLink, Route, Routes } from "react-router-dom";
import { ArrowLeft, Building2, Globe, ShieldOff } from "lucide-react";

import { strings } from "../i18n";
import { Spinner, cx } from "../ds";
import { useJmapClient } from "../jmap";
import { DomainsPage } from "./DomainsPage";
import { TenantsPage } from "./TenantsPage";
import styles from "../admin/admin.module.css";

export function ControlConsole() {
  const client = useJmapClient();
  const [status, setStatus] = useState<"loading" | "operator" | "denied">("loading");

  useEffect(() => {
    let live = true;
    client
      .isOperator()
      .then((ok) => {
        if (live) setStatus(ok ? "operator" : "denied");
      })
      .catch(() => {
        if (live) setStatus("denied");
      });
    return () => {
      live = false;
    };
  }, [client]);

  if (status === "loading") {
    return (
      <div className={styles.gate}>
        <Spinner size={24} />
      </div>
    );
  }
  if (status === "denied") {
    return (
      <div className={styles.gate}>
        <div className={styles.denied}>
          <span className={styles.deniedIcon}>
            <ShieldOff size={28} strokeWidth={1.75} />
          </span>
          <h1>{strings.controlDeniedTitle}</h1>
          <p>{strings.controlDeniedBody}</p>
          <Link to="/mail" className={styles.deniedBtn}>
            <ArrowLeft size={16} />
            <span>{strings.adminBackToFicina}</span>
          </Link>
        </div>
      </div>
    );
  }

  return (
    <div className={styles.console}>
      <aside className={styles.sidebar}>
        <Link to="/mail" className={styles.back}>
          <ArrowLeft size={16} />
          <span>{strings.adminBackToFicina}</span>
        </Link>
        <div className={styles.brand}>{strings.controlTitle}</div>
        <nav className={styles.sideNav}>
          <NavLink
            to="/control/tenants"
            className={({ isActive }) => cx(styles.navItem, isActive && styles.navActive)}
          >
            <Building2 size={17} strokeWidth={1.75} />
            <span>{strings.controlTenants}</span>
          </NavLink>
          <NavLink
            to="/control/domains"
            className={({ isActive }) => cx(styles.navItem, isActive && styles.navActive)}
          >
            <Globe size={17} strokeWidth={1.75} />
            <span>{strings.controlDomains}</span>
          </NavLink>
        </nav>
      </aside>
      <main className={styles.content}>
        <Routes>
          <Route index element={<Navigate to="/control/tenants" replace />} />
          <Route path="tenants" element={<TenantsPage />} />
          <Route path="domains" element={<DomainsPage />} />
          <Route path="*" element={<Navigate to="/control/tenants" replace />} />
        </Routes>
      </main>
    </div>
  );
}
