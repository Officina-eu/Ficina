// Root route of the Ficina web application. Placeholder at Phase 0:
// the real shell (design system, auth flow, navigation) is the first
// "Webmail & mail UX" item of ROADMAP.md Phase 2.
import { strings } from "./i18n/strings";

export function App() {
  return (
    <main>
      <h1>{strings.appName}</h1>
      <p>{strings.tagline}</p>
    </main>
  );
}
