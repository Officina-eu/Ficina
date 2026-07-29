// The Tenant Admin console shell: a full-screen surface with its own left nav,
// gated to tenant admins. Only the pages with a real backend are in the nav
// today (AI providers); more are added as their backends land — no dead links.
import { useEffect, useState } from "react";
import { Link, Navigate, NavLink, Route, Routes } from "react-router-dom";
import { ArrowLeft, Sparkles } from "lucide-react";

import { strings } from "../i18n";
import { Spinner, cx } from "../ds";
import { useJmapClient } from "../jmap";
import { AiProvidersPage } from "./AiProvidersPage";
import styles from "./admin.module.css";

export function AdminConsole() {
  const client = useJmapClient();
  const [status, setStatus] = useState<"loading" | "admin" | "denied">("loading");

  useEffect(() => {
    let live = true;
    client
      .isAdmin()
      .then((ok) => {
        if (live) setStatus(ok ? "admin" : "denied");
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
  if (status === "denied") return <Navigate to="/mail" replace />;

  return (
    <div className={styles.console}>
      <aside className={styles.sidebar}>
        <Link to="/mail" className={styles.back}>
          <ArrowLeft size={16} />
          <span>{strings.adminBackToFicina}</span>
        </Link>
        <div className={styles.brand}>{strings.adminTitle}</div>
        <nav className={styles.sideNav}>
          <NavLink
            to="ai"
            className={({ isActive }) => cx(styles.navItem, isActive && styles.navActive)}
          >
            <Sparkles size={17} strokeWidth={1.75} />
            <span>{strings.adminAiProviders}</span>
          </NavLink>
        </nav>
      </aside>
      <main className={styles.content}>
        <Routes>
          <Route index element={<Navigate to="ai" replace />} />
          <Route path="ai" element={<AiProvidersPage />} />
          <Route path="*" element={<Navigate to="ai" replace />} />
        </Routes>
      </main>
    </div>
  );
}
