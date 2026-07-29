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

  // auth — brand panel
  brandHeadline: "Your workspace.\nYour servers.\nYour rules.",
  brandSubtitle:
    "Mail, calendar, chat, and files — sovereign, AI-native, and hosted in Europe.",
  brandEuBadge: "Hosted on your infrastructure · EU",

  // auth — sign in
  signInHeading: "Sign in",
  signInSubtitle: "Welcome back. Enter your credentials to continue.",
  emailLabel: "Email",
  emailPlaceholder: "you@yourdomain.com",
  passwordLabel: "Password",
  rememberMe: "Remember me",
  forgotPassword: "Forgot password?",
  forgotPasswordNote: "To reset your password, contact your administrator.",
  signInButton: "Sign in",
  signingIn: "Signing in…",
  orDivider: "or",
  signInWithSso: "Sign in with SSO",
  ssoComingSoon: "Single sign-on is coming soon.",

  // auth — two-factor
  twoFactorTitle: "Two-factor authentication",
  twoFactorSubtitle: "Enter the 6-digit code from your authenticator app",
  twoFactorRecoverySubtitle: "Enter one of your recovery codes",
  twoFactorCodeLabel: "Authentication code",
  recoveryCodeLabel: "Recovery code",
  recoveryPlaceholder: "xxxx-xxxx",
  verify: "Verify",
  verifying: "Verifying…",
  useRecoveryCode: "Use a recovery code instead",
  useAuthenticator: "Use your authenticator app instead",
  backToSignIn: "Back to sign in",

  // auth — errors
  errorBadCredentials: "That email or password is not right. Please try again.",
  errorSecondFactor: "Enter your authentication code to continue.",
  errorBadOtp: "That code is not right. Please try again.",
  errorRateLimited: "Too many attempts. Please wait a moment and try again.",
  errorGeneric: "Something went wrong signing in. Please try again.",
  errorNetwork: "Cannot reach the server. Check your connection and try again.",
  signingOut: "Signing out…",

  // mail
  mailLoading: "Loading your mail…",
  mailFolders: "Folders",
  mailEmpty: "No messages here yet.",
  mailSearchEmpty: "No messages match your search.",
  mailSelectPrompt: "Select a message to read it.",
  mailListError: "Could not load messages.",
  mailFolderError: "Could not load your folders.",
  mailRetry: "Try again",
  mailFrom: "From",
  mailTo: "To",
  mailNoSubject: "(no subject)",
  mailUnknownSender: "Unknown sender",

  // mail — sidebar
  compose: "Compose",
  mailSearchPlaceholder: "Search mail…",

  // mail — reading pane
  reply: "Reply",
  replyAll: "Reply all",
  forward: "Forward",
  archive: "Archive",
  snooze: "Snooze",
  flag: "Flag",
  unflag: "Unflag",
  markUnread: "Mark as unread",
  moreActions: "More actions",
  endOfMessage: "End of message",
  toLabel: "to",
  replyTo: "Reply to",
  draftWithAi: "Draft with AI",

  // mail — compose
  composeTitle: "New message",
  composeReplyTitle: "Reply",
  composeTo: "To",
  composeCc: "Cc",
  composeSubject: "Subject",
  composeRecipientsPlaceholder: "name@example.com, …",
  composeSubjectPlaceholder: "Subject",
  composeBodyPlaceholder: "Write your message…",
  composeSend: "Send",
  composeSending: "Sending…",
  composeDiscard: "Discard",
  composeCcToggle: "Cc",
  composeNoRecipients: "Add at least one recipient.",
  composeSendError: "Could not send your message. Please try again.",
  composeSent: "Message sent.",
  composeWroteOn: "wrote:",
  composeReplyPrefix: "Re: ",

  // mail — not-yet-built (honest placeholders)
  aiComingSoon: "The AI assistant is coming soon.",
  archiveUnavailable: "There's no archive folder to move this to.",
} as const;

export type StringKey = keyof typeof strings;
