// The module registry — the one place that declares the workspace's modules.
// The rail renders a button per entry; the router (see App.tsx) mounts each
// enabled module's element at its path and falls back to a "coming soon"
// placeholder for the rest. Adding Agenda/Chat/Drive/Docs later is ONE entry
// here plus one area folder — the shell, rail, auth, and layout do not change.
import { Calendar, HardDrive, Mail, MessagesSquare, Video } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { strings } from "../i18n";

export interface ModuleDef {
  /** Stable id (also the rail key). */
  id: string;
  /** Route path under the shell, e.g. "/mail". */
  path: string;
  /** Rail + header label (already-resolved string from the i18n catalog). */
  label: string;
  /** Rail icon. */
  Icon: LucideIcon;
  /** False until the module is built — renders the "coming soon" placeholder. */
  enabled: boolean;
}

// Order and set match the Figma app-shell rail (Docs lives inside Drive per
// ADR 0010, so it is not a separate rail item).
export const modules: ModuleDef[] = [
  { id: "mail", path: "/mail", label: strings.moduleMail, Icon: Mail, enabled: true },
  { id: "agenda", path: "/agenda", label: strings.moduleAgenda, Icon: Calendar, enabled: false },
  { id: "chat", path: "/chat", label: strings.moduleChat, Icon: MessagesSquare, enabled: false },
  // Drive proper is not built; its surface currently hosts the Docs
  // technical-authoring preview (ADR 0015), which lives under Drive (ADR 0010).
  { id: "drive", path: "/drive", label: strings.moduleDrive, Icon: HardDrive, enabled: true },
  { id: "meet", path: "/meet", label: strings.moduleMeet, Icon: Video, enabled: false },
];

/** The module a bare "/" should open. */
export const defaultModulePath = "/mail";
