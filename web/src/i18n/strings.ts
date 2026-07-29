// All user-facing strings live here (CLAUDE.md: externalized from day one —
// hardcoded English in components is a bug in a European product). This module
// is the translation catalog's source; the extraction/locale tooling lands
// with the i18n pass. Components read strings from here via `strings.*` — never
// inline English.
export const strings = {
  // brand
  appName: "Ficina",
  tagline: "The sovereign, AI-native workspace for Europe.",

  // modules (rail labels + titles)
  moduleMail: "Mail",
  moduleAgenda: "Agenda",
  moduleChat: "Chat",
  moduleMeet: "Meet",
  moduleDrive: "Drive",
  moduleDocs: "Docs",
  moduleAi: "Ask AI",

  // shell
  newButton: "New",
  userMenu: "Account",
  signOut: "Sign out",
  signedInAs: "Signed in as",
  comingSoonTitle: "Coming soon",
  comingSoonBody: "This part of your workspace is on the way. Mail is ready now.",

  // auth
  loginTitle: "Sign in to Ficina",
  loginSubtitle: "Your sovereign European workspace.",
  emailLabel: "Email",
  passwordLabel: "Password",
  otpLabel: "Authentication code",
  otpHint: "Enter the 6-digit code from your authenticator app.",
  signInButton: "Sign in",
  signingIn: "Signing in…",
  errorBadCredentials: "That email or password is not right. Please try again.",
  errorSecondFactor: "Enter your authentication code to continue.",
  errorRateLimited: "Too many attempts. Please wait a moment and try again.",
  errorGeneric: "Something went wrong signing in. Please try again.",
  errorNetwork: "Cannot reach the server. Check your connection and try again.",
  signingOut: "Signing out…",

  // mail
  mailLoading: "Loading your mail…",
  mailFolders: "Folders",
  mailEmpty: "No messages here yet.",
  mailSelectPrompt: "Select a message to read it.",
  mailListError: "Could not load messages.",
  mailFolderError: "Could not load your folders.",
  mailRetry: "Try again",
  mailFrom: "From",
  mailTo: "To",
  mailNoSubject: "(no subject)",
  mailUnknownSender: "Unknown sender",
} as const;

export type StringKey = keyof typeof strings;
