// The alo workspace product surface: the full suite. Adds the suite modules
// (Docs under Drive, plus the coming-soon Agenda/Chat/Meet), the multi-tenant
// control-plane console, and the Docs-powered equation/code inserts for the
// compose editor.
//
// This is the ONE file that imports the suite-only areas (`../control`,
// `../authoring`). alomails ships the `mail` surface instead and deletes this
// file together with those areas — nothing else in the web app references them.
import { Suspense, lazy } from "react";
import { Building2, Calendar, Code2, HardDrive, MessagesSquare, Sigma, Video } from "lucide-react";

import { strings } from "../i18n";
import { Spinner } from "../ds";
import { ControlConsole } from "../control";
import type { ComposeInsert, ProductModule, ProductSurface } from "./types";
import { adminConsole, defaultPath, sharedModules } from "./shared";

// Docs pulls in KaTeX + Prism, so it is code-split: those libraries load only
// when a user opens Docs or inserts an equation/code block, never on mail.
const DocsModule = lazy(() => import("../authoring").then((m) => ({ default: m.DocsModule })));
const AuthoringInsertModal = lazy(() =>
  import("../authoring").then((m) => ({ default: m.AuthoringInsertModal })),
);

function DriveSurface() {
  return (
    <Suspense
      fallback={
        <div style={{ display: "flex", justifyContent: "center", padding: "4rem" }}>
          <Spinner size={24} />
        </div>
      }
    >
      <DocsModule />
    </Suspense>
  );
}

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
  ...sharedModules,
  { id: "agenda", path: "/agenda", label: strings.moduleAgenda, Icon: Calendar, enabled: false },
  { id: "chat", path: "/chat", label: strings.moduleChat, Icon: MessagesSquare, enabled: false },
  // Drive proper is not built; its surface hosts the Docs technical-authoring
  // preview (ADR 0015), which lives under Drive (ADR 0010).
  {
    id: "drive",
    path: "/drive",
    label: strings.moduleDrive,
    Icon: HardDrive,
    enabled: true,
    element: () => <DriveSurface />,
  },
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
};
