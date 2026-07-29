// The application router. Public route: the login screen. Everything else is
// behind RequireAuth and rendered inside the shell frame; the module set comes
// from the registry, so adding a module is a registry entry, not a router
// change. Only Mail has a real surface this pass; the rest show "coming soon".
import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";

import { AuthProvider, LoginPage, RequireAuth } from "./auth";
import { AppShell, ComingSoon, defaultModulePath, modules } from "./shell";
import { MailModule } from "./mail";
import { AdminConsole } from "./admin";

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
            <Route element={<AppShell />}>
              <Route index element={<Navigate to={defaultModulePath} replace />} />
              {modules.map((m) => (
                <Route
                  key={m.id}
                  path={`${m.path}/*`}
                  element={
                    m.id === "mail" ? <MailModule /> : <ComingSoon title={m.label} Icon={m.Icon} />
                  }
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
