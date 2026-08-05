// The alodrives product surface: the standalone Drive app, on its own domain
// (app.alodrives.com). Only the Drive module — nothing from mail/calendar/tasks
// ships here, so it is defined self-contained (it deliberately does NOT import
// `./shared`, which would pull those modules in). Same shared alo-jmap backend,
// same login. ADR 0019 (independent products).
import { HardDrive, Shield } from "lucide-react";

import { strings } from "../i18n";
import { DriveModule } from "../drive";
import { AdminConsole } from "../admin";
import type { ProductConsole, ProductModule, ProductSurface } from "./types";

const driveModules: ProductModule[] = [
  {
    id: "drive",
    path: "/drive",
    label: strings.moduleDrive,
    Icon: HardDrive,
    enabled: true,
    element: () => <DriveModule />,
  },
];

/** Tenant admin (users, domains, security) — reachable from the account menu. */
const adminConsole: ProductConsole = {
  path: "/admin/*",
  element: () => <AdminConsole />,
  menu: { to: "/admin", label: strings.adminOpen, Icon: Shield, requires: "admin" },
};

export const surface: ProductSurface = {
  modules: driveModules,
  consoles: [adminConsole],
  composeInserts: [],
  defaultPath: "/drive",
  brand: {
    headline: () => strings.brandHeadlineDrive,
    subtitle: () => strings.brandSubtitleDrive,
    euBadge: () => strings.brandEuBadgeDrive,
  },
  // Business product: company SSO, bring-your-domain placeholder.
  login: {
    sso: true,
    emailPlaceholder: () => strings.emailPlaceholder,
  },
};
