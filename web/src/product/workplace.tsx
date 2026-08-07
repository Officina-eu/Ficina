// The alo workspace product surface: the full suite. Adds the suite modules
// (Docs under Drive, plus the coming-soon Agenda/Chat/Meet), the multi-tenant
// control-plane console, and the Docs-powered equation/code inserts for the
// compose editor.
//
// This is the ONE file that imports the suite-only areas (`../control`,
// `../authoring`). alomails ships the `mail` surface instead and deletes this
// file together with those areas — nothing else in the web app references them.
import { Suspense, lazy } from "react";
import { Building2, Code2, Globe, HardDrive, MessagesSquare, Receipt, Sigma, Video } from "lucide-react";

import { strings } from "../i18n";
import { BillingModule } from "../billing";
import { SitesModule } from "../sites";
import { ControlConsole } from "../control";
import { DriveModule } from "../drive";
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
  // The alomails products (Home/Mail/Agenda/Tasks) come from sharedModules; the
  // workspace adds Drive (alodrives, with its file-hosted documents) and the
  // not-yet-built Chat and Meet. This is why Drive shows on aloworkplace.com but
  // not on the standalone alomails app.
  ...sharedModules,
  {
    id: "drive",
    path: "/drive",
    label: strings.moduleDrive,
    Icon: HardDrive,
    enabled: true,
    element: () => <DriveModule />,
  },
  // Billing is a workspace module only (ADR 0035): the business suite is what
  // aloworkplace.com sells, and it has no place in the standalone mail app.
  {
    id: "billing",
    path: "/billing",
    label: strings.moduleBilling,
    Icon: Receipt,
    enabled: true,
    element: () => <BillingModule />,
  },
  // Sites is a workspace module only (ADR 0036): the public website a business
  // publishes belongs to the suite, not to the standalone mail app.
  {
    id: "sites",
    path: "/sites",
    label: strings.moduleSites,
    Icon: Globe,
    enabled: true,
    element: () => <SitesModule />,
  },
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
