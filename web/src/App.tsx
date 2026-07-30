// The application router. Public route: the login screen. Everything else is
// behind RequireAuth and rendered inside the shell frame; the module set comes
// from the registry, so adding a module is a registry entry, not a router
// change. Only Mail has a real surface this pass; the rest show "coming soon".
import { Suspense, lazy } from "react";
import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";

import { AuthProvider, LoginPage, RequireAuth } from "./auth";
import { AppShell, ComingSoon, defaultModulePath, modules } from "./shell";
import { Spinner } from "./ds";
import { MailModule } from "./mail";
import { AdminConsole } from "./admin";
import { ControlConsole } from "./control";

// The technical-authoring surface pulls in KaTeX + Prism (and every Prism
// language grammar), so it is code-split: those libraries load only when a user
// opens Docs, never on the mail path (ADR 0015).
const AuthoringWorkspace = lazy(() =>
  import("./authoring").then((m) => ({ default: m.AuthoringWorkspace })),
);

function ModuleLoading() {
  return (
    <div style={{ display: "flex", justifyContent: "center", padding: "4rem" }}>
      <Spinner size={24} />
    </div>
  );
}

/** The real surface for a module, or a "coming soon" placeholder. Docs'
 * technical-authoring surface (ADR 0015) lives under Drive (ADR 0010). */
function moduleElement(id: string, label: string, Icon: (typeof modules)[number]["Icon"]) {
  if (id === "mail") return <MailModule />;
  if (id === "drive") {
    return (
      <Suspense fallback={<ModuleLoading />}>
        <AuthoringWorkspace />
      </Suspense>
    );
  }
  return <ComingSoon title={label} Icon={Icon} />;
}

export function App() {
  return (
    <BrowserRouter>
      <AuthProvider>
        <Routes>
          <Route path="/login" element={<LoginPage />} />
          {/* The OIDC redirect target; the login flow reads the code inline, so
              a stray navigation here just returns to the app. */}
          <Route path="/auth/callback" element={<Navigate to={defaultModulePath} replace />} />

          <Route element={<RequireAuth />}>
            {/* The admin console has its own full-screen shell (not the mail
                rail); it gates to tenant admins internally. */}
            <Route path="/admin/*" element={<AdminConsole />} />
            {/* The platform control plane (ADR 0012): its own full-screen
                shell, gated to platform operators internally. */}
            <Route path="/control/*" element={<ControlConsole />} />
            <Route element={<AppShell />}>
              <Route index element={<Navigate to={defaultModulePath} replace />} />
              {modules.map((m) => (
                <Route
                  key={m.id}
                  path={`${m.path}/*`}
                  element={moduleElement(m.id, m.label, m.Icon)}
                />
              ))}
              <Route path="*" element={<Navigate to={defaultModulePath} replace />} />
            </Route>
          </Route>
        </Routes>
      </AuthProvider>
    </BrowserRouter>
  );
}
