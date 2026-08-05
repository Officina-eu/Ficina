// The alo workspace product surface: the full suite. Adds the suite modules
// (Docs under Drive, plus the coming-soon Agenda/Chat/Meet), the multi-tenant
// control-plane console, and the Docs-powered equation/code inserts for the
// compose editor.
//
// This is the ONE file that imports the suite-only areas (`../control`,
// `../authoring`). alomails ships the `mail` surface instead and deletes this
// file together with those areas — nothing else in the web app references them.
import { Suspense, lazy } from "react";
import { Building2, Code2, MessagesSquare, Sigma, Video } from "lucide-react";

import { strings } from "../i18n";
import { ControlConsole } from "../control";
import type { ComposeInsert, ProductModule, ProductSurface } from "./types";
import { adminConsole, defaultPath, sharedModules } from "./shared";

// The technical-authoring editor pulls in KaTeX + Prism, so it is code-split:
// those libraries load only when a user inserts an equation/code block, never on
// mail. (The full Docs surface now lives inside Drive as a file type.)
const AuthoringInsertModal = lazy(() =>
  import("../authoring").then((m) => ({ default: m.AuthoringInsertModal })),
);

/** A compose-insert Modal that reuses the shared authoring modal for `kind`. */
function insertModal(kind: "equation" | "code"): ComposeInsert["Modal"] {
  return function Insert(props: { onInsert: (html: string) => void; onClose: () => void }) {
    return (
      <Suspense fallback={null}>
        <AuthoringInsertModal kind={kind} onInsert={props.onInsert} onClose={props.onClose} />
      </Suspense>
    );
  };
}

const suiteModules: ProductModule[] = [
  // Drive (with its file-hosted documents) comes from sharedModules, alongside
  // Home/Mail/Agenda/Tasks. The suite adds the not-yet-built Chat and Meet.
  ...sharedModules,
  { id: "chat", path: "/chat", label: strings.moduleChat, Icon: MessagesSquare, enabled: false },
  { id: "meet", path: "/meet", label: strings.moduleMeet, Icon: Video, enabled: false },
];

export const surface: ProductSurface = {
  modules: suiteModules,
  consoles: [
    adminConsole,
    {
      path: "/control/*",
      element: () => <ControlConsole />,
      menu: { to: "/control", label: strings.controlOpen, Icon: Building2, requires: "operator" },
    },
  ],
  composeInserts: [
    { id: "equation", label: strings.composeInsertEquation, Icon: Sigma, Modal: insertModal("equation") },
    { id: "code", label: strings.composeInsertCode, Icon: Code2, Modal: insertModal("code") },
  ],
  defaultPath,
  brand: {
    headline: () => strings.brandHeadline,
    subtitle: () => strings.brandSubtitle,
    euBadge: () => strings.brandEuBadge,
  },
  // Business/suite product: SSO for company IdPs, bring-your-domain placeholder.
  login: {
    sso: true,
    emailPlaceholder: () => strings.emailPlaceholder,
  },
};
