// The alo workspace product surface: the full suite. Adds the suite modules
// (Docs under Drive, plus the coming-soon Agenda/Chat/Meet), the multi-tenant
// control-plane console, and the Docs-powered equation/code inserts for the
// compose editor.
//
// This is the ONE file that imports the suite-only areas (`../control`,
// `../authoring`). alomails ships the `mail` surface instead and deletes this
// file together with those areas — nothing else in the web app references them.
import { Suspense, lazy } from "react";
import { BarChart3, Briefcase, Building2, Code2, Globe, HardDrive, Handshake, MessagesSquare, Receipt, Sigma, Video } from "lucide-react";

import { strings } from "../i18n";
import { BillingModule } from "../billing";
import { CrmModule } from "../crm";
import { InsightsModule } from "../insights";
import { ProjectsModule, TimerWidget } from "../projects";
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
  // CRM is a workspace module only (ADR 0035), beside Billing and for the same
  // reason: the business suite is what aloworkplace.com sells. It sits next to
  // Billing because a won deal becomes a quote there.
  {
    id: "crm",
    path: "/crm",
    label: strings.moduleCrm,
    Icon: Handshake,
    enabled: true,
    element: () => <CrmModule />,
  },
  // Projects is a workspace module only (ADR 0035, wave B3), beside Billing
  // and CRM: client work, the hours worked on it, and the invoice those hours
  // become. It sits after CRM because a won deal becomes an engagement, and
  // before Insights because Insights reads what the three of them record.
  //
  // Its boards are the SAME rows Tasks shows — a client project is a task
  // project with client facts beside it (docs/design/projects.md). Two tabs
  // saying "project" is the honest cost of one project list.
  {
    id: "projects",
    path: "/projects",
    label: strings.moduleProjects,
    Icon: Briefcase,
    enabled: true,
    element: () => <ProjectsModule />,
  },
  // Insights is a workspace module only (ADR 0037), and it sits after the
  // modules whose records it reads: a chart here is billing's and CRM's own
  // figures, computed by the server and only drawn in the browser.
  {
    id: "insights",
    path: "/insights",
    label: strings.moduleInsights,
    Icon: BarChart3,
    enabled: true,
    element: () => <InsightsModule />,
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
  // The running timer, visible from every module: a clock you cannot see from
  // your inbox is a clock you forget to stop. It draws nothing when none runs,
  // which is the ordinary state of a workspace.
  railWidgets: [{ id: "timer", Widget: TimerWidget }],
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
