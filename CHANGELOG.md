# Changelog

User- and operator-visible changes, written when the knowledge is
fresh (release skill). Versions follow SemVer against public
contracts.

## Unreleased

- **alo Sites: your websites now live in the workspace.** The rail has a new
  **Websites** area: every site you have, with its address and whether it is
  live. Create one by picking a name and claiming an address — the form checks
  availability as you type and tells you in plain words when an address is
  free, taken, or not allowed. Open a site to see its pages with the home page
  marked, and add a page with a title and path — the first page you add is
  offered as the home page. Every rule is the server's: a refusal always
  names the exact rule that was broken. The visual page editor is next.

- **alo Sites: build and publish your website through the workspace API.**
  Everything a site is made of can now be managed while signed in: create a
  site by claiming a free subdomain (with a live taken/free check), add and
  arrange pages, stack typed sections on each page (add, edit, reorder,
  remove), pick a theme, and publish — or unpublish — with one call. Every
  input is checked before it lands: unsafe links, unknown section types,
  reserved names, and duplicate slugs are refused with a message naming the
  exact rule, and publishing tells you what is missing (a page, a home page)
  instead of failing silently. Nothing you edit reaches the public site until
  you publish. This is the API the visual editor ships on next.

- **alo Sites: published sites are now served on the web.** The new
  `alo-sites` service answers for `<your-subdomain>.<sites domain>`: it looks
  up the site by the address it was asked for and serves exactly what you
  published — the frozen pages, the theme's stylesheet, and a styled
  "page not found" in your site's own look. Edits after publishing change
  nothing on the public site until you publish again, and a republish shows up
  on the very next request. Visitors' browsers are told to re-check pages
  every minute and get a compact "not modified" answer when nothing changed.
  One site's address can never show another site's content — that isolation
  is a tested guarantee, not a hope. (Self-hosted deployments: run the new
  `alo-sites` container with `SITES_DOMAIN` and wildcard DNS pointed at it;
  nothing else changes.)

- **alo Billing: invoice in another currency, and keep your books in yours.**
  Your billing details now name the **currency you keep books in** — euro unless
  you say otherwise — and you can invoice a customer in any currency alongside
  it. Under that setting you keep the **exchange rates**: paste the European
  Central Bank's published rate file (the daily one, or its whole history), or
  type a single rate by hand. Nothing is fetched on your behalf, so the rates
  your books are converted at are a file you chose, and a file with one bad value
  changes nothing at all rather than importing half of itself.
  When you issue a foreign-currency invoice, the rate of that day is **frozen on
  the document** — the day's published rate, or the last one published before it,
  which is what the VAT rules ask for. The document then prints its VAT a second
  time in your own currency with the rate beside it, which is what makes it a
  valid invoice outside the euro; the same figures appear on screen and in the
  PDF. A credit note converts at the rate of the invoice it corrects, so the two
  cancel exactly in your books. An invoice in a currency you have no rate for is
  **not** issued: it says so and stays a draft, rather than being numbered at a
  rate nobody published.
  The **VAT report** now ends with the whole period in your accounting currency —
  every document at the rate frozen on it — which is the figure a return is
  copied from. Each currency still gets its own table above it, and if anything
  in the period could not be converted the report says how many documents that
  is, on screen and in the CSV, instead of quietly leaving them out.

- **alo Billing: the VAT figures for a period, in one screen.** Billing has a
  new **VAT report** tab: pick two dates — or click **This quarter** or **Last
  quarter** — and see what you billed at each VAT rate between them, with the
  tax on it and the totals underneath. It counts the documents that actually
  stand: issued and paid invoices, dated by the day they were **issued**, with
  credit notes subtracted; drafts and cancelled documents are not in it, because
  they charged nobody anything. The tax shown is the sum of the tax on your
  documents, not the rate re-applied to a total — so the figures agree with the
  invoices your customers are holding, to the cent. Amounts in different
  currencies are reported separately and never added together. **Download CSV**
  saves the same figures as a file for your accountant; it carries rates,
  amounts and counts, and names no customer.

- **alo Billing: record what your customers have actually paid.** An issued
  invoice now has a **Payments** section: enter what arrived, the day your bank
  shows it, how it came and the reference on the statement line. Part payments
  are the ordinary case — a customer settling a large bill in instalments — so
  the invoice shows what has been received and what is **still owed** after each
  one, and only flips to **Settled** when the whole amount is in. Nothing about
  that state is typed by anyone: it is worked out from the payments themselves,
  so what the invoice says and what the ledger under it holds can never
  disagree. A payment keyed wrongly is **removed** and entered again, which puts
  the invoice back to owed. The invoice list gains a **Still owed** column and
  an **Overdue** view — issued, past its date, not yet settled — judged against
  the server's date, so no clock but ours decides who is late. Two refusals
  worth knowing: an invoice that money has been received against can no longer
  be **voided** (correct it with a credit note, so both movements stay
  visible), and a credit note takes no payments at all, because it is money
  owed the other way.

- **alo Billing: email an invoice to your customer, without leaving the
  invoice.** An issued invoice can now be sent to the customer it names: alo
  writes the email for you — addressed to that customer, with the PDF attached
  and a short note stating the number, the total and when it is payable — and
  puts it in your **Drafts**. It does not send it. You open it, change a word
  if you want to, and send it yourself like any other message, so nothing ever
  leaves your mailbox without you seeing it first. A draft invoice cannot be
  sent (it has no number yet — issue it first), nor can a voided one, and a
  customer with no email address is told so plainly. Sending the same invoice
  twice simply writes a second draft: nothing about the invoice changes.

- **alo Billing: an invoice as a PDF you can send.** Any invoice can now be
  fetched as a **PDF file** — the same document as the Print view, laid out for
  A4, with the pages numbered when there is more than one so nobody can mislay
  half a bill. It is produced entirely by alo, on your own server: no browser,
  no external service, and nothing about your customers leaves the machine to
  make it. The file is named after the document inside it
  (`Invoice-INV-2026-00001.pdf`), it downloads rather than opening in the
  browser, and it is never cached. Emailing it to the customer arrives next.
  One limitation, until the next release: the PDF is set in a font that covers
  Western Europe, so Polish, Czech, Hungarian, Romanian, Baltic, Greek and
  Cyrillic letters are simplified to their nearest Latin form on the **PDF**
  (`Łukasz` prints as `Lukasz`). The Print view and everything on screen are
  unaffected.

- **alo Billing: the document your customer actually receives.** Every invoice,
  credit note and quote now has a **Print** button that puts a proper A4
  document in front of you: your name and address at the top, theirs beside it,
  the lines, the VAT broken out per rate, the total, and — on an invoice — what
  is payable by when and the account it goes to. A draft prints as a **draft**
  and carries no number, because it has none; a voided invoice prints as
  **void**; a credit note is titled as one and names the invoice it corrects,
  and neither a credit note nor a quote shows your bank details, since nothing
  is payable on them. The page is the same one the PDF and the emailed
  attachment will be made from, so what you see is what your customer gets.
  Fill in **Your details** first (a new tab in Billing): the name you invoice
  under, your VAT and company numbers, how customers reach you, and where the
  money goes. The **IBAN is checked before it is saved** — against your
  country's length and its check digits — because a mistyped account number is
  only ever discovered by the payment that never arrives.

- **alo Billing: issue an invoice, and quote for the work first.** A draft
  invoice now has an **Issue** button. It asks first, and says exactly what it
  is about to do: take the next number in your series, date the document, and
  freeze it for good. After that the invoice is a record — correct it with a
  **credit note**, which raises a draft mirroring every line for you to trim
  down to a partial credit, or **void** one nobody has seen (it keeps its
  number, because a number that vanished is a hole in your books). Nothing is
  emailed to anyone yet.
  **Quotes** are the same screen, one step earlier: raise a draft for a
  customer, put the same kind of lines on it, and mark it **sent** — which
  takes a quote number of its own and freezes the prices you offered. When the
  customer says yes, **Accepted** closes the offer and hands you a draft
  invoice with the identical lines at the identical prices, ready to issue;
  **Declined** and **Give up on it** close it without business. An offer past
  its date is flagged, not blocked — honouring one a few days late is your
  call. Every document says where it came from and what it became, so a quote,
  its invoice and any credit note are one click from each other. Printing and
  PDFs come next (ADR 0035).

- **alo Billing: your invoices, on screen.** Billing now opens on the
  **invoice list** — number, customer, dates, what it is worth — with a chip
  for where each document stands (draft, issued, paid, void) and a plain red
  row for anything **overdue**, judged by the server's date and not your
  browser's. Filter by status, or search by number, customer or their own
  reference. **New invoice** raises a draft for the customer you pick, and the
  **draft editor** is where you fill it in: add lines by hand or pick them
  straight from your price list (which copies the price and VAT rate as they
  are today, so changing your price list never rewrites a document). Quantities
  take three decimals — half an hour is `0.5`, a third of a kilo is `0.333` —
  and prices take whichever notation you normally type. The draft **saves
  itself** a moment after you stop typing, and the net, the VAT per rate and
  the total you see are the ones the server sent back; while an edit is still
  on its way they dim rather than pretend. A line without a description holds
  the save instead of quietly disappearing from the document. A draft can be
  deleted (it carries no number, so nothing is left behind); a document that
  has been issued shows as a frozen record. Issuing, credit notes and printing
  come next (ADR 0035).

- **alo Billing has a home in the workspace.** A **Billing** entry now sits in
  the rail (in alo workplace only — the standalone mail app is unchanged), with
  the two lists everything else is built from: your **customers** and your
  **price list**. Add a customer with their address, VAT id, invoice email,
  payment terms and currency, and the server tells you straight away if a VAT
  id does not add up. Add the things you sell once — name, unit, price, VAT
  rate — and pick them later instead of retyping them. Type a price the way you
  normally would (`1 234,56` or `1,234.56`, both work); what is stored is exact
  whole cents, and nothing about money is ever worked out in the browser.
  Neither list has a delete: you **archive** an entry, so it leaves the pickers
  while every document already raised still names it, and "show archived"
  brings it back into view. Invoices, quotes and the rest of the screens
  follow (ADR 0035).

- **alo Billing: an accepted quote becomes the invoice for it.** Marking an
  offer as accepted now also raises the **draft invoice** for it, in one move:
  every line copied at the price it was offered at, in the same order, worth
  exactly what the customer agreed to — down to the VAT per rate. The draft is
  an ordinary draft, so you can add the line you forgot before issuing it, and
  issuing it takes the next number in your invoice series as always. The two
  documents point at each other: the invoice names the quote it came from, and
  the quote names the invoice it produced. An offer that was declined or that
  lapsed is never billed, an offer can only be accepted once, and the whole
  thing is a single step — you will never find an accepted quote with nothing
  to bill it by. The quote surface itself is now live over the API
  (`/billing/quotes`), with the screens to follow (ADR 0035).

- **alo Billing: quotes, the offer before the invoice.** You can now draft a
  quote for a customer with exactly the same lines an invoice takes, and the
  server totals it the same way, to the cent. A draft is yours to change or
  throw away; **sending** it takes the next number in your quote series
  (`QUO-2026-00001` — a series of its own, so an offer nobody accepted never
  leaves a hole in your invoice numbering), stamps the day it went out and the
  day it stands until, and freezes it. An open offer is then **accepted**,
  **declined**, or marked **expired**, each recorded with the day it was
  decided; a quote list can be filtered by any of those. Nothing closes an
  offer behind your back — a quote past its date is shown as lapsed, and it is
  still yours to honour if you want to. Turning an accepted quote into a draft
  invoice, and the screens for all of this, arrive shortly (ADR 0035).

- **alo Billing: invoices, from draft to issued to credited.** The document
  itself is now live on the server. You raise a **draft** for a customer — lines
  with a description, a quantity, a unit price and a VAT rate — and the server
  works out the net, the VAT per rate, and the gross, every time, in whole
  cents; nothing about what a document is worth is ever computed in the browser
  or sent in by a client. A draft is yours to change or discard. **Issuing** it
  is the moment it becomes a legal document: it takes the next number in your
  unbroken series (`INV-2026-00001`), is stamped with the day it was issued and
  the day it is due from the payment terms it was raised with, and is frozen —
  an issued invoice is never edited afterwards. From there you either **void**
  it (it keeps its number and stops being owed, so your series stays gapless) or
  **credit** it: one click raises a mirrored credit note, drawing on the same
  series, that you can trim to a partial credit before issuing. The two
  documents together sum to exactly zero, so a corrected invoice reconciles
  against the customer's copy to the cent. The invoice list can be filtered by
  status and flags anything past its due date as **overdue**. The screens for
  all of this arrive shortly (`/billing/invoices`; ADR 0035).

- **alo Billing: your customers and your price list, over the API.** The first
  working part of alo Billing is live on the server: a tenant-wide list of the
  companies you invoice — address, country, VAT id, payment terms, currency,
  optionally linked to a contact in your address book — and a price list of the
  things you sell, each with a unit, a price, and a VAT rate. VAT ids are checked
  against the rules of the country they name, so a typo is caught when it is
  entered rather than on an invoice. Nothing is ever deleted: an item you stop
  selling, or a customer you stop working with, is **archived** — out of the
  pickers, still there to explain last year's books. Prices are held in whole
  cents from end to end, so nothing rounds behind your back. The screens for all
  of this arrive shortly (`/billing/customers` and `/billing/products`; ADR 0035).

- **alo Sheet ribbon: borders, rotation, wrapping, merge, and cell styles.** The
  Home ribbon now covers cell **borders**, **text rotation**, **wrapping**
  (overflow / wrap / clip), **merging** (all, across, vertically, unmerge), and a
  **cell styles** gallery (Default, Heading 1–3, and more) — the everyday
  formatting an Excel hand expects, all on alo's own ribbon.

- **alo Sheet is a complete Excel replacement.** You can now create a spreadsheet
  (**New → Sheet**), edit it, **open a real `.xlsx`** — which imports straight into
  alo Sheet, no third-party editor — and **download any sheet back as `.xlsx`**
  (a button in the sheet toolbar) to send a genuine Excel file to anyone. Values,
  numbers, and multiple sheets round-trip; complex styling and charts are
  best-effort. Imports never touch your original file — it stays in Drive. The
  redundant "New → Excel" (Collabora) entry is gone; "Sheet" is the one way to
  make a spreadsheet. First format fully moved onto alo's own editors (ADR 0033).

- **Equations in documents.** In an alo Doc, type `/equation` (or `/formula`,
  `/math`) to add a math formula written in LaTeX — `E = mc^2`, `\frac{a}{b}`,
  and so on — rendered cleanly on the page. Click a formula to edit it. Code
  blocks are already there via `/code`.

- **AI in documents — propose, then approve.** In an alo Doc, **Ask AI** lets you
  tell the AI what to write or change ("draft an intro about…", "summarise this").
  The AI comes back with a **proposal you review** — nothing is added to your
  document until you click **Insert** (or **Discard** to throw it away). The AI
  never edits your document silently; that's the promise. ADR 0029.

- **Ask your workspace (AI).** The search box (Ctrl/Cmd-K) now has an **Ask AI**
  option: ask a question in plain language — "where's the Acme proposal?", "what
  did we decide about pricing?" — and get an answer drawn from **your own files,
  tasks, and email**, with the sources it used listed and clickable underneath.
  The AI only ever sees what you could already open — it can't widen your access —
  and it answers *from your sources*, citing them, rather than making things up.
  If no AI model is set up yet (an admin configures one), you still get the
  matching files/tasks/email, just without the written answer. ADR 0029.

- **You see exactly what Ask AI will do before it does it.** Every action the
  agent proposes now shows a **preview card**: a draft or reply shows the
  recipient, subject and the full text; a move shows the target folder; a snooze
  shows the wake time. **Sending** — the one step that can't be undone — carries
  its own caution note and a distinct **Send** button. Nothing runs until you
  press Approve. ADR 0034.

- **Ask AI can tidy your inbox — with your approval.** Beyond answering, **Ask AI**
  can now act on your email: ask it to "archive the Acme newsletter", "delete the
  spam from billing", "snooze the invoice until Monday 9am", "flag the contract",
  or "mark the release note as read", and it finds the message you mean and shows
  you the single action it proposes. Nothing happens until you press **Approve** —
  then it archives, moves to Trash, snoozes (the email slips out of the inbox and
  comes back at the time you chose), flags/unflags, or marks read/unread that one
  email. It only ever touches your own mailbox, and it will say so plainly when it
  can't do what you asked yet. ADR 0034.

- **Ask AI can draft an email — new or a reply — for you.** Ask it to "email
  bob@acme.com asking to move our meeting to Friday", or "reply to the Globex
  invoice saying I'll pay Monday", and it writes the message and — once you
  **Approve** — saves it to your **Drafts** to review and send yourself. A reply
  is addressed to the original sender and stays in the same conversation thread.
  It never sends on its own, and the sender is always your own address (it can't
  write as anyone else). ADR 0034.

- **Ask AI can send a draft — only when you approve, and only a draft.** After you
  have a draft (one you wrote, or one Ask AI drafted for you), you can ask Ask AI
  to send it; it shows you the send as a proposal and delivers it **only after you
  press Approve**. It will only ever send a message that is already in your Drafts
  — never an arbitrary email — and it goes out through the normal signed-sending
  path, moving to Sent just as if you had clicked Send yourself. ADR 0034.

- **Ask AI can file an email into one of your folders.** Ask it to "move this to
  Work" or "file the payslip under Payroll" and — once you **Approve** — it takes
  the message out of your inbox and into that folder. It only ever uses folders
  you already have (it won't invent one from a typo), and if you name a folder
  that doesn't exist it says so instead of guessing. ADR 0034.

- **Workspace search (files, tasks + email content).** A search box in the left
  rail (or **Ctrl/Cmd-K** anywhere) searches across your **files, tasks and
  email** at once, and a result jumps you straight to it — opening the file, task
  or message. Files and tasks match by name; **email matches by full content**, so
  a word that appears only in the body of a message still finds it. It only ever
  shows what you can already see (your files, your Spaces, your tasks, your own
  mailbox); a teammate's private items, another person's mail, and other
  organisations never appear — and each app only shows results it can open.
  Search now also looks **inside your files**: text files, alo Docs, and
  **Word, Excel, PowerPoint and PDF** files are read for their text when you save
  them, so a word *inside* the document finds it, not just its name. (Scanned
  PDFs with no text layer, and images, stay findable by name — there's no OCR.)
  ADR 0029.

- **Real Word / Excel / PowerPoint editing in Drive (Collabora).** Open a
  `.docx`, `.xlsx`, `.pptx` (or OpenDocument) file in Drive and it now opens in a
  full editor **inside the workspace** — powered by Collabora, with genuine
  desktop-Office fidelity. Edits save straight back to Drive as new versions, and
  the file stays where it lives (shared with its Space's members). This is the
  *compatibility* type — for a great native document, use an alo Doc or alo Base.
  The editor is embedded same-origin behind our own WOPI host with short-lived,
  signed access tokens; the engine is a memory-capped pinned container so it
  can't disturb mail. ADR 0010. (New-from-blank Office files come next; for now,
  upload one and open it.)

- **alo Base — board, calendar & gallery views + select and link fields.** alo
  Base now feels like Airtable: add **Select** and **Multi-select** fields (with
  coloured chips), **Person** and **Link-to-another-table** fields, then look at
  the same records as a **grid**, a **board** (kanban — drag cards between
  columns to change their status), a **calendar** (records on the day of a date
  field), or a **gallery** (cards). Switching view never changes the data — it's
  the same records, seen differently. Add views with a picker (choose what to
  group or date by). ADR 0032.

- **alo Base — the grid you click (web UI).** "New base" in Drive creates an alo
  Base and opens its **editable grid**: columns are your typed fields, rows are
  records, and you edit **right in the cells** (text, number, date, checkbox).
  Add a row, add a column (choose its type), and add more tables — all saving as
  you go. It opens inside the workspace like the doc editor, and (like everything
  in Drive) it's shared by where it lives. Board/calendar/gallery views over the
  same rows, linked records, and more field types come next. ADR 0032.

- **alo Base — a relational data table (backend).** alo's native "sheet" isn't a
  grid of cells — it's a small database with a spreadsheet face (Airtable-style).
  A Base lives in Drive like any file (in My Files or a Space, auto-shared with
  members); it has tables with **typed fields** (text, number, date, checkbox,
  select, attachment, person, and link-to-record), records, and **multiple views
  over the same records** (grid/board/calendar/gallery — switching view never
  changes data). The engine is live and verified on the server: a Space viewer
  can read but not edit, another organisation gets "not found", and bad field
  types are rejected. The grid you'll click comes next; then linked records and
  the office-file compatibility editors. ADR 0032.

- **alo Doc — a block editor in Drive (first slice).** "New doc" in Drive creates
  an **alo Doc** — a clean, Notion-style block document (headings, lists, tables,
  quotes, images, and more) that opens right inside the workspace. It lives in
  Drive like any file: in your My Files or a Space (auto-shared with that Space),
  and **every change auto-saves as a new version** you can roll back to. It's the
  alo-native document type, distinct from a Word file (which opens in the
  compatibility editor) — ADRs 0030–0032. Coming next on top of this: technical
  authoring (math/code), live data blocks, and propose-then-approve AI.

- **Drive — the file manager (web UI).** Drive is now a module in the app: down
  the side, **My Files** and each **Space** you belong to, plus **Trash**. The
  main area shows the current folder with a breadcrumb, drag-or-click **upload**,
  **New folder**, and per-item actions — open, download, rename, move, make a
  copy, version history, and move-to-trash (restore or delete-forever from
  Trash). A Space shows **Members** (who has access), which managers can change.
  "Move to…" lets you shift a file between My Files and a Space — and because
  access follows location, that changes who can see it. Built for the live app;
  the document types (alo Doc / alo Base / Word-Excel compatibility) come next.

- **Drive — files, in one coherent place (backend).** Every file lives in exactly
  one spot: your private **My Files**, or a **Space** (where it's automatically
  shared with that Space's members). No OneDrive-vs-SharePoint split, no per-file
  permission maze — a file's access is simply its location's access. You get
  folders, upload, download, rename, move, copy, trash/restore, and full version
  history with restore. **Moving a file changes who can see it** (into a Space
  shares it; out of it un-shares it) — verified on the live server, along with:
  a Space viewer can read but not change files (a clean "not allowed"), and
  another organisation gets "not found" on everything. The file manager UI comes
  next. ADR 0027.

- **Spaces — the shared home for team work (foundation).** A Space (e.g. "Acme
  project") is a group with named members and three plain roles — viewer,
  editor, manager. It's the spine the whole suite will plug into: files first,
  then tasks, mailboxes, and more, all inheriting one membership instead of the
  per-item permission maze. Membership is always visible, a manager changes it,
  and a Space always keeps at least one manager. Everything is scoped to your
  organisation: a non-member can't even see a Space exists, and another
  organisation gets a clean "not found". ADRs 0026–0029.

- **Single sign-on for standalone products (token introspection).** The login
  system can now tell a separate product's backend not just *who* signed in but
  *which organisation* they belong to — the piece that lets a genuinely
  standalone app (Drive next) share the one workspace login without copying the
  login code or its database. It's a protected, off-by-default endpoint
  (RFC 7662); nothing changes for existing apps. Groundwork for ADR 0025.

- **Desktop 0.1.10**: bundles everything since 0.1.9 — task attachments, labels,
  followers, and "blocked by" dependencies, the branded date picker, and the
  Timeline dependency arrows. Installed apps auto-update.

- **Task dependencies ("blocked by").** A task can now be marked as blocked by
  another; the task detail lists what's blocking it, with a picker to add or
  remove blockers, and the Timeline draws an arrow from each blocker to the task
  it holds up. A task can't block itself, and — like everything else — you can
  only link tasks you can already see, so a dependency never points across
  organisations or into someone else's private project.

- **Task followers.** You can follow a task to keep an eye on it; whoever
  creates a task follows it automatically, and the assignee and anyone else can
  follow or unfollow from the task detail. The follower list shows each person's
  avatar. Following is scoped to your organisation — you can only follow tasks
  you can already see, and a follow request for another organisation's task is
  refused.

- **Desktop 0.1.9**: bundles the redesigned Home, Calendar, and Tasks (List /
  Board / Timeline / Calendar / Overview, the new-task modal, and the branded
  dialogs). Installed apps auto-update.

- Fixed: **Desktop app (Windows) opened to a blank window.** The "open external
  links in your browser" behaviour was mistaking the app's own local address for
  an outside site on Windows, so it never loaded. The app now opens straight to
  your workspace. Also: the desktop app no longer shows the old File/Edit menu
  bar on Windows/Linux (macOS keeps its standard menu), and interface text is no
  longer selectable like a web page — only message bodies and fields are. Ship
  0.1.8; installs on the auto-update feed refresh automatically.
- Fixed: **Deleting a tenant now removes its tasks too.** Task projects and
  tasks were left behind when a tenant was deleted (they weren't tied to the
  tenant record); they are now purged with it, like the rest of a tenant's data.
- New: **Turn an email into a task.** Open a message, and "Create a task" (in the
  ⋯ menu) makes a task from it — titled with the subject and linked back to the
  message. On the task, "Open the source email" jumps straight to that message.
  Where a tenant has AI configured, "Suggest tasks from this email" reads the
  message and drops candidate to-dos into your task inbox to accept or dismiss —
  it never adds them to your board on its own. The source link stays inside your
  tenant: it can only ever open a message you're already allowed to read.
- New: **Tasks.** A calm, fast task manager — the third leg of mail + calendar +
  tasks. Board (kanban) and list are two views of the *same* tasks: switch
  instantly, drag a card between columns to change its status or reorder within
  one. Each task has an assignee, due date, priority, subtasks, comments, and a
  history; the detail slides in from the side without leaving your board.
  Personal tasks are private; team projects are shared. A task can remember the
  email or event it came from, a due date can surface on the calendar, and the
  AI *suggests* action items you accept or dismiss — it never creates tasks
  silently. In English, French, and Dutch.
- New: **Dutch (Nederlands).** The whole interface is now available in Dutch —
  pick it under Account → Language (it's also auto-selected for Dutch browsers).
  Alongside English and French, for Belgian/Flemish teams.
- New: **Shared calendars sync to your phone.** Every calendar you can see —
  your own and any shared with you — now appears as its own calendar on your
  phone / Apple Calendar / Thunderbird over CalDAV, with its name and colour;
  read-only shared calendars show as read-only. (Your existing personal calendar
  is unchanged.) Times written in a named time zone are handled correctly, and
  clients that ask for a specific date range now get just that range.
- New: **Event reminders.** Set a reminder on any event — from "at the time of
  the event" up to "1 day before" — and it fires natively on your phone / Apple
  Calendar (synced as a calendar alarm), even when the app is closed.
- New: **See when people are free.** When you add guests to an event, **Check
  availability** shows who is busy at that time (within your organization) so you
  can schedule around conflicts — busy/free only, never their event details.
- New: **See who's coming.** When a guest accepts, declines, or answers "maybe"
  to your invitation, their reply is now recorded on the event, so opening it
  shows each guest's status instead of the reply just sitting in your inbox.
- New: **More repeat options.** Events can repeat on specific weekdays
  (e.g. every Mon/Wed/Fri, or every weekday) and on monthly patterns like the
  2nd Tuesday or the last day of the month. A new **Every weekday** preset is in
  the repeat picker.
- New: **Per-occurrence changes reach your phone, and guests.** Editing or
  skipping a single occurrence of a repeating event now syncs that one instance
  to your phone over CalDAV, and — if the event has guests — emails them so just
  that occurrence moves or drops off their calendar too.
- Fixed: **alomails shows only Mail + Calendar again.** The web deploy was
  building the full workplace surface, so the sidebar briefly showed Chat, Drive,
  and Meet — products that aren't part of alomails. The publish step now builds
  the mail surface (`ALO_PRODUCT=mail`), so alomails is Home, Mail, and Agenda.
- New: **Edit a single event in a repeating series.** Opening one occurrence of a
  recurring event now offers **This event** or **All events** on save — move or
  rename just this Tuesday's standup while the rest of the series stays put, or
  apply the change to the whole series. Skipping a single occurrence (delete →
  "This event") already existed; this is its editing counterpart. For now the
  per-occurrence edit shows in the app; it does not yet propagate to phones over
  CalDAV (the series still syncs) — that follow-on is tracked in the calendar
  notes.
- New: **Shared and team calendars.** You can now share a calendar with a
  colleague by email, or with a whole group (team) at once, giving them either
  **view** or **edit** access. Shared calendars appear in everyone's sidebar
  marked with their access level; a view-only calendar opens read-only, while an
  editor can add and change its events. Owners get a **Share** button on any
  calendar they own to add or remove people and groups at any time. Sharing is
  strictly within your organization — a calendar can only ever be shared with
  people in the same tenant, never across the boundary.
- New: **A landing page at alomails.com.** The bare domain now has a proper
  marketing page — what alomails is (private, sovereign email + calendar, hosted
  in Europe) with app downloads — while the app itself stays on
  mail.alomails.com. The apex has its own certificate; the app's TLS and mail
  are untouched. The site itself lives in its own repo
  (`aloworld-org/alomails-website`); only the serving glue is here.
- New: **Skip one occurrence of a repeating event.** Deleting a single instance
  of a recurring series — "cancel *this* Tuesday's standup, keep the rest" —
  now removes just that occurrence while the series stays. Opening a repeating
  event offers **This event** or **All events**. The exclusion rides along to
  your phone and Apple Calendar over CalDAV (an iCalendar `EXDATE`), and
  exclusions made there sync back. (Editing a single occurrence in place is the
  next step.)
- New: **Event cancellations.** When an organizer calls off a meeting, alomails
  removes it from your calendar automatically and shows a clear "Cancelled"
  notice on the email — no stale events left behind. And when you cancel an event
  you organized (delete it), every guest is emailed a cancellation so it drops
  off their calendar too. Works with Gmail, Outlook, and Apple both ways.
- New: **RSVP to invitations you receive.** When a calendar invitation lands in
  your inbox — from anyone on Gmail, Outlook, or Apple — alomails shows an
  **Accept / Maybe / Decline** card right in the reading pane, with the event's
  time, place, and who invited you. Accept (or Maybe) drops the event onto your
  calendar and emails a proper reply back to the organizer so their calendar
  updates too; Decline just sends the reply. Together with sending invitations,
  the full invite loop now works both ways.
- New: **Invite guests to an event.** Add email addresses to an event and, when
  you save, alomails emails each guest a standard invitation (iMIP `REQUEST`)
  from your address — so anyone on Gmail, Outlook, or Apple Calendar gets a real,
  RSVP-able invite in their own calendar, and editing the event re-sends an
  update. (Receiving invitations back as RSVPs in alomails comes next.)
- New: **Recurring events.** When you create or edit an event, pick how it
  repeats — every day, week, month, or year — and it fills the calendar going
  forward (with an optional end). The repeat rides along to your phone/Apple
  Calendar over CalDAV, and events created there with a repeat show up in
  alomails too. Editing or deleting a repeating event changes the whole series;
  per-occurrence exceptions come later.
- New: **Your alomails calendar syncs to your phone and computer (CalDAV).** Add
  your alo account to iPhone/iPad or macOS Calendar, Android (via a CalDAV app),
  or Thunderbird, and the events you create in alomails appear there — and events
  you add on your phone sync back. It rides the same one-account setup as your
  contacts (CardDAV), with incremental sync so only changes move.
- New: **Calendar, built right into alomails.** alomails is now Mail **and**
  Calendar in one app — the Gmail/Outlook shape — with a familiar month and week
  Agenda: a "New event" button, click a day or time slot to add one, and click
  an event to edit or delete it. Events live on your own account, tenant-isolated
  like everything else. (First slice: personal timed and all-day events; syncing
  to your phone / Apple Calendar via CalDAV, and emailed invitations, come next.)
- New: **alomails as a real desktop app.** Download an installable app from
  **mail.alomails.com/download** — its own window and dock/taskbar icon, not a
  browser tab. The full alomails interface is **bundled inside the app** and
  loads locally (instant, works offline until it needs the network), talking to
  your alomails account over a secure connection — an installed program, not a
  window pointed at the website. It's built with Tauri (Rust shell + the existing
  web UI, uses the system webview — no bundled browser), so it's the same app you
  know with nothing rewritten (ADR 0005). Windows ships now; the macOS .dmg
  builds on CI. **The app keeps itself up to date:** on launch it checks a
  signed update feed and, if a newer version is out, downloads it, verifies its
  signature, installs it, and relaunches — silently, in the background, so you
  download it once. (Still unsigned to the OS, so the first install may warn
  about an unidentified developer until code-signing certificates are added —
  that's separate from the update signing, which is already in place.)
- New: **Forgot your password? Reset it yourself.** A "Forgot password?" link on
  the sign-in screen now starts a self-service reset: enter your alo address, get
  a code at the recovery mailbox you set up at signup, and choose a new password —
  no admin needed. The request step always looks the same whether or not the
  address exists, so it never reveals who has an account; the code is short-lived,
  attempt-capped, and rate-limited, exactly like signup. (Applies to accounts
  created from this release on, since it needs the recovery mailbox on file.)
- Fixed: **The alomails sign-in no longer looks like a company login.** The
  standalone mail product now shows a personal email hint (`you@alomails.com`
  instead of `you@yourdomain.com`) and drops the enterprise "Sign in with SSO"
  button — leaving Sign in + "Create a personal account". The workspace build
  keeps SSO and the bring-your-own-domain hint. Both are product-surface
  settings, so each product shows the right login.
- New: **alomails speaks its own language on the sign-in screen.** The standalone
  mail product's login now reads as an email service ("Your mail. Your privacy.
  Your rules.", "Sovereign email · Hosted in Europe") instead of the workspace
  copy. The brand text is part of the product surface, so each product carries
  its own — and it stays fully translatable (English + French shipped).
- Fixed: **Typing the bare mail domain now works.** Visiting `mail.example.com`
  (plain HTTP) previously connected to nothing and errored; it now redirects to
  HTTPS. Caddy serves port 80 for the redirect while the Let's Encrypt renewal
  challenge is served from a shared webroot — so certificate auto-renewal keeps
  working without certbot needing its own public port. Verified with a live
  renewal dry-run.
- New: **alomails — the Mail product as its own app.** Built with
  `ALO_PRODUCT=mail`, the standalone alomails surface ships Home + Mail only
  (no workspace, authoring, or suite-admin modules) and the browser tab now
  reads *alomails* rather than *alo workplace*. This is the trimmed bundle
  served at mail.alomails.com; the full workspace build is unchanged.
- New: **Mail apps set themselves up (autoconfig).** Add your alo address in
  Thunderbird, Apple Mail, or Outlook and the app fills in the servers and
  ports for you — no more typing IMAP/SMTP hostnames by hand. (Requires two
  small DNS records for your mail domain; see the deployment guide.)
- New: **Bring your old mail in (IMAP import).** A new **Import mail** item
  in your account menu pulls recent messages from another mailbox — pick
  **Gmail** or **Outlook** (the server address is filled in for you) or
  enter any IMAP server, sign in, and your mail is copied into alo over a
  verified TLS connection. **All your folders come across** — Sent, Drafts,
  Junk, Trash, Archive and your own folders are recreated, and each
  message keeps its read / starred / answered state. Re-running is safe:
  messages already imported are skipped, not duplicated. For Gmail and
  Outlook, use an app password (their normal password won't work for mail
  apps).
- Improved: **Mail works on a phone.** On a small screen the mailbox now
  shows one pane at a time — your message list, then the conversation
  when you tap it, with a back button to return — and folders slide in
  from a menu button instead of squeezing the layout. The desktop
  three-pane view is unchanged.
- New: **Contacts sync to your phone and computer (CardDAV).** Add your
  alo account to iPhone/iPad, macOS Contacts, Android (via a CardDAV
  app), or Thunderbird, and your address book syncs both ways
  automatically — add a contact on your phone and it's on the web, and
  vice versa. Point the client at your alo server and sign in with your
  normal email and password.
- New: **Address book (contacts).** A new **Contacts** panel in your
  account menu lets you keep an address book — names, multiple emails
  and phone numbers, organization, job title, notes — with search,
  create, edit, and delete. Saved contacts show up first when you're
  picking recipients in compose. **Import and Export** move your whole
  address book in and out as a standard `.vcf` file, so you can bring
  your Gmail/Outlook/Apple contacts straight in (and back them up).
  (Automatic device sync follows.)
- New: **alo now speaks French — and can speak more.** A full,
  native-quality French translation of the whole app, switchable from a
  new **Language** control in your account menu; your choice is
  remembered, and new visitors get their browser's language
  automatically. The translation framework underneath makes adding more
  European languages a matter of dropping in a catalog — Dutch and
  German are next.
- New: **Abuse controls for inbound and outbound mail.** A single
  source IP can no longer monopolise the server — each is capped to a
  fair number of simultaneous connections (excess get a polite "try
  again"), and unknown senders are greylisted (briefly deferred, which
  most spam sources never retry). Outbound, a per-destination send-rate
  limit protects the server's sending reputation if an account is ever
  compromised — a sudden flood is smoothed into a steady trickle rather
  than blasted out. All tunable, and the outbound limiter is off by
  default for single-tenant servers.
- New: **Incoming mail is scanned for malware** (ClamAV, ~3.6 M
  signatures, auto-updating). A message carrying a known threat is
  refused at the door with a clear reason — it never reaches a
  mailbox — and if the scanner is ever down, mail is politely deferred
  rather than let through unscanned. Operators disable by unsetting
  `ALO_SMTP_CLAMAV_ADDR`.
- New: **Marking mail as junk now trains the spam filter.** Moving a
  message into Junk reports it as spam; moving it back out reports it
  as ham — the filter (Rspamd Bayes) learns from your real mail and
  gets sharper over time. Deployments gain a small redis service for
  the learning store; this also fixes Bayes being silently inactive at
  scan time (it had no token backend). Training is best-effort and
  never delays or blocks moving mail.
- New: **Outgoing mail to DANE-protected servers is now
  tamper-proof-encrypted** (RFC 7672). When a destination publishes
  DNSSEC-signed TLSA records, alo validates the DNSSEC chain itself,
  makes TLS mandatory (no downgrade-to-cleartext, ever), and verifies
  the server's certificate against the published records — closing the
  classic STARTTLS-stripping attack for those destinations. Servers
  without TLSA keep today's opportunistic encryption. Operators can
  disable with `ALO_SMTP_DANE=off`.
- New: **DMARC aggregate reports are now sent** (RFC 7489 §7.2). The MX
  records every inbound DMARC evaluation and a daily job mails each
  sender domain's published `rua=` address a gzipped XML report of what
  we saw — source IPs, alignment outcomes, applied dispositions. This
  is the feedback loop other domain owners rely on; external report
  addresses are verified per §7.1 before anything is sent. Operators
  can disable with `ALO_SMTP_DMARC_REPORTS=off` (migration 0033).
- New: **Forwarded mail keeps its proof of authenticity (ARC).** Mail
  forwarded by a filter rule ("redirect") is now ARC-sealed (RFC 8617):
  the receiving server can verify the SPF/DKIM/DMARC results we saw at
  ingress even though forwarding breaks SPF, so forwards stop failing
  DMARC downstream. Sealed with the forwarding domain's own DKIM key;
  operators can disable with `ALO_SMTP_ARC_SEALING=off`.
- New: **alo Transfer — large files as links.** A file too big to attach
  (over 25 MB) uploads once and rides the message as a private, expiring download
  link instead of an inline attachment, so it sidesteps recipient
  attachment-size limits. **No size limit** — the file is streamed straight to
  storage — and **you choose how long the link lives** (1 / 7 / 30 / 90 days).
  In compose it shows as a link chip with an expiry picker; the sent message
  carries a tidy download card. Links are unguessable and served as a forced
  download, never rendered inline (`POST /share/upload`, public streaming
  `GET /share/{token}`, migrations 0026–0027).
- New: **Colored labels.** Custom folders can be color-coded — a colored dot in
  the sidebar, set from a right-click palette (or cleared). Colors round-trip on
  `Mailbox/get`/`Mailbox/set` and are validated to a strict `#rrggbb` (migration
  0025).
- Improved: **Settings redesigned** as a two-pane preferences panel (General ·
  Filters & rules · Organization) with proper section headers and cards, in
  place of the old flat single column.
- New: **Filters & rules + Block sender.** A visual rule builder in Settings:
  match incoming mail on From / To / Cc / Subject (contains / is, all or any)
  and act — move to a folder, mark read, star, or delete. Rules run **on the
  server at delivery**, even when you're offline, and the first match applies.
  **Block sender** is one click in a conversation's ⋯ menu — that address's mail
  goes straight to Junk. Rules compile to a single managed Sieve script that
  also carries any out-of-office auto-reply, so the two coexist
  (`GET`/`PUT /filters`, `POST /filters/block`, migration 0024).
- New: **Recipient autocomplete.** Typing in To / Cc / Bcc drops down matching
  recent correspondents (name + address) for one-click selection — arrow keys
  and Enter, or click. The list is mined from your recent mail, ranked by how
  often and how recently you've corresponded, and your own addresses are left
  out (`GET /contacts`).
- New: **Send later.** Schedule a composed message for a chosen time instead of
  sending now — the Send button has a **▾ menu** (Tomorrow morning / afternoon,
  Monday morning, or a custom date & time). The draft moves to a **Scheduled**
  folder and a background sweeper sends it when due, filing it to Sent; **Cancel
  send** (reading pane) returns it to Drafts. Scheduling runs the same send-from
  validation as an immediate send, so a forbidden send is refused up front; the
  sweeper claims each due message before it hits the wire, so a crash can never
  double-send (`POST /send-later`, migration 0023).
- New: **AI smart replies.** When AI is configured, an open conversation shows up
  to **three short, ready-to-send replies** as pills below the thread (only when
  the newest message is from someone else). Picking one opens a reply
  **pre-filled** with that text, ready to edit or send. Soft-degrades like the
  rest of the AI suite — the pills simply don't appear when AI is off
  (`POST /ai/replies`).
- New: **Gmail-style mail.** The conversation list is now compact two-line rows
  (sender · time / subject — snippet), unread bold, with a star and hover
  **archive / delete / read** actions; **bulk select** (row checkboxes → a
  select-all bar with batch archive/delete/read/snooze). Expanded messages
  collapse the **quoted history behind a "···"** and show a **"to me ▾"**
  recipient expander.
- New: **Snooze.** Hide a conversation until later (Later today / Tomorrow /
  This weekend / Next week) from the reading pane or the bulk bar; it moves to a
  **Snoozed** folder and a background sweeper returns it to the Inbox, unread,
  when due (migration 0021; `POST /snooze`).
- New: **Math & code in emails.** The compose toolbar can insert an **equation**
  (the LaTeX editor with a live preview) and a **code block** (dark, with a
  language picker). Equations are sent as **MathML** and code as a
  self-contained **inline-styled block**, so they render in alo's reading
  pane and other modern clients; the message's plain-text part carries the raw
  LaTeX and fenced code as the universal fallback. KaTeX/Prism are code-split —
  loaded only when a user inserts one, never on the normal mail path (ADR 0015).
- New: **alo Docs — real documents.** Technical authoring is now a working
  editor, not a demo: each user creates, opens, renames, and deletes their own
  **documents** (tenant- and owner-scoped store; a document is reachable only by
  its owner — isolation is tested). The editor is **block-based** — add, reorder,
  and delete headings, prose (with inline `$math$` and `{{cross-references}}`),
  numbered display equations, dark syntax-highlighted code blocks, and editable
  tables — and **autosaves** as you type. New API: `GET/POST /docs`,
  `GET/PUT/DELETE /docs/{id}` (migration 0020, `documents`). Reached via Drive.
- New: **Technical authoring in alo Docs.** Write specs with math and code,
  all rendered **in the browser** (no draft equation or line of code leaves the
  client): an **equation editor** with a LaTeX input, a live **KaTeX** preview, a
  LaTeX/Visual toggle, a common-symbol quick bar, and inline vs numbered display
  equations; **code blocks** with **Prism** highlighting, a searchable language
  picker (explicit, never guessed), copy, and line numbers; and **cross-references
  with auto-numbering** — equations, tables, figures, and sections number
  themselves and reference chips ("Eq. 3", "Table 1", "Section 2.3") stay correct
  when items are reordered or inserted, with an insert-cross-reference picker
  (tabs). Ships as a standalone alo Docs surface (reached via Drive) that will
  dock into the Collabora Docs shell when that lands. KaTeX + Prism are MIT; the
  numbering/reference layer is alo's own (ADR 0015). The libraries are
  code-split, so the mail app never loads them.
- New: **Cc and Bcc.** Compose sends to Cc and Bcc recipients; the reading pane
  shows the full To / Cc / Bcc of each message. Bcc is written into the sender's
  own copy (so Sent records who was blind-copied) but the server **strips the
  Bcc header from the transmitted bytes**, so recipients never see it — while
  Bcc addresses are still delivered via the envelope. A received message's Bcc is
  always empty, and Cc (never Bcc) joins the searchable text.
- New: **AI conversation summary.** Opening a conversation can produce a short
  alo-written summary through the tenant's configured model
  (`POST /ai/summarize`), degrading quietly when AI is off.
- New: **Verified sender badge.** A message whose inbound authentication passed
  (DMARC, or DKIM in DMARC's absence) shows a "Verified" pill in the reading pane.
- New: **Out-of-office auto-reply.** A settings toggle (account menu → Settings)
  with an optional subject and a message; turning it on installs and activates a
  managed `out-of-office` Sieve **vacation** script, so replies go out through
  the existing vacation machinery (one reply per correspondent, suppression
  window). Turning it off removes it. `GET /settings/mail` reports it;
  `POST /settings/out-of-office` sets it (a message is required to enable).
- New: **Real mail search.** The search box now runs a **server-side full-text
  search across the whole account** (JMAP `Email/query` over the message
  tsvector index) instead of filtering only the loaded page; results are
  grouped into conversations and open in the reading pane. Debounced; a cleared
  box returns to the folder view.
- New: **Mail signatures + organization footer.** Each user sets a rich-text
  **signature** (account menu → Settings) inserted into new messages and
  replies; tenant admins set a tenant-wide **organization footer** appended
  after every user's signature. Endpoints: `GET /settings/mail`,
  `POST /settings/signature` (any user), `POST /admin/org-footer` (admin);
  stored per user / per tenant, empty clears.
- New: **Undo send.** A sent message is held for a few seconds with an **Undo**
  action before it actually submits; Undo leaves it in Drafts. A queued send is
  never lost (it flushes on window-elapse or navigation).
- New: **Per-tenant DKIM signing keys.** Verifying a domain now provisions its
  own Ed25519 DKIM key (ADR 0014); outbound mail is signed with the key for the
  message's `From` domain, so each tenant signs as itself (DMARC-aligned). The
  Domains page shows the DKIM record to publish and offers **Rotate** (selector
  rollover — the old record stays valid until removed). The secret key never
  leaves the server or a client response. The existing single deployment key
  (`ALO_SMTP_DKIM_*`) is unchanged and remains the fallback, so single-tenant
  deployments sign exactly as before. New route: `/admin/domains/dkim/rotate`;
  the `/admin/domains` listing gains a `dkim` record per domain. RSA keys are
  not generated in-process (Ed25519 only; operators needing RSA supply it via
  the file key). Groundwork for no-touch rotation once alo serves
  authoritative DNS (ADR 0013, deferred).
- New: **Admin console completed + storage quotas + audit log.** The tenant
  Admin console now opens on an **Overview** dashboard (users, storage,
  deliverability, AI) and adds a **Domains** page (register + DNS-verify the
  tenant's own domains, tenant-scoped) and an **Audit log** (every
  administrative action — who, what, target, when — newest first, including
  platform-operator actions on the tenant). **Per-tenant storage quotas**
  (operator-set; `NULL` = unlimited, the default) are enforced at the
  blob-write choke points: over-quota JMAP upload → **507**, `set` → `overQuota`,
  and inbound mail is deferred with a transient **452**. New operator env
  `ALO_AI_EGRESS` (default `open` for self-hosting; `restricted` on shared
  hosting requires https and blocks loopback/private/link-local AI endpoints —
  an SSRF guard with the vetted IP pinned) and `ALO_ENFORCE_DOMAIN_OWNERSHIP`
  (default `off`). Both deferred findings in
  `docs/design/multi-tenant-trust-boundary.md` are now closed.
- New: **Multi-tenant control plane (`alo-control`).** A dedicated
  platform-operator service (ADR 0012), separate from the tenant API, for
  governing a shared deployment: **tenant lifecycle** (list, provision a
  tenant + its first admin, suspend/resume, delete with an id-echo
  confirmation) and **tenant→domain ownership** (register a domain, verify it
  by a `_alo-verify` DNS TXT proof, list, remove). Operators are a new
  principal — a user carrying `is_platform_admin`, created by `identityctl
  bootstrap-operator`, authenticated by the same opaque token path as everyone
  else; an operator token authorizes `/control/*` governance only and is
  **never** a key into any tenant's mail. Address assignment
  (`create_user`/alias/list) can now be constrained to a tenant's verified
  domains — the fix for the cross-tenant mail-capture risk — behind
  `ALO_ENFORCE_DOMAIN_OWNERSHIP` (default off; flip once domains are
  registered). New service: compose `alo-control` + Caddy `/control/*`
  route. Schema (additive): `users.is_platform_admin`, `tenants.status`, a
  `domains` table. Design + threat model: ADR
  `0012-multi-tenant-control-plane.md`, `docs/design/multi-tenant-trust-boundary.md`.
- New: **Tenant Admin console + AI inference layer.** A full-screen,
  tenant-admin-only console (reached from the user menu, gated on the new
  `alo:isAdmin` session key) with four working pages: **Users & mailboxes**
  (create, reset password, grant/revoke admin with self-lockout protection,
  aliases, delete), **Groups & lists** (groups, membership, and distribution
  **list addresses** that fan inbound mail out to every member's inbox),
  **Security & trust** (live SPF/DKIM/DMARC/MX/reverse-DNS/MTA-STS
  deliverability checks run as real DNS + HTTPS queries against the email
  domain), and **AI providers**. New backend crate **`alo-ai`** speaks the
  OpenAI-compatible Chat Completions contract, so the AI backend is
  *configured, never bundled* — bring your own: local Ollama, a self-hosted
  model, or a hosted provider (OpenAI/Anthropic/custom), per tenant. The web
  Compose **"Improve"** action calls it via a new authenticated, tenant-scoped
  **`POST /ai/improve`** (new `alo:aiEnabled` session key hides the control
  when AI is off). API keys are stored server-side and **never returned to
  clients** (only a `hasKey` flag) or logged; prompts and completions are
  never logged (law #1). New HTTP surface: `/admin/users*`, `/admin/groups*`,
  `/admin/security/checks`, `/admin/ai/*`, `/ai/improve`. New operator env for
  the admin: `bootstrap-admin` marks the first user; the Security page reads
  `ALO_SMTP_LOCAL_DOMAINS` / `ALO_SMTP_DKIM_*`. Design + threat model:
  ADR `0011-ai-inference-layer.md`, `docs/design/multi-tenant-trust-boundary.md`.
- New: **Sending mail** — JMAP **`EmailSubmission/set`** (RFC 8621 §7), so the
  web app's Compose and Reply actually send. A composed message is built as a
  proper RFC 5322 `text/plain` message (all To/Cc, reply threading, and
  European-correct non-ASCII via RFC 2047 encoded-words + base64 body — no
  header injection) and sent through a new **trusted internal SMTP submission
  listener** so it is DKIM-signed, queued, and delivered by the existing
  outbound path, then filed to Sent. **Send-as is enforced on both the SMTP
  envelope and the visible `From:` header** (a token cannot send as another
  identity), only drafts are sendable, and recipients are capped per message.
  The outbound SMTP client is now a shared `alo-smtp-client` crate used by
  both the delivery path and this submission path (no duplication). New config:
  `ALO_SMTP_INTERNAL_SUBMISSION_ADDR` (never publish this port) and
  `ALO_JMAP_SUBMISSION_ADDR`. Design + security review:
  `docs/design/email-submission.md`.
- New: **alo web app** — the one-product workspace shell, web-first
  (`web/`). The "warm workshop" design system (paper / verdigris / copper /
  ink tokens, self-hosted Inter + EB Garamond, shared primitives), the left
  rail + layout frame with a module registry that Agenda/Chat/Drive/Docs plug
  into later, first-party **OIDC + PKCE** sign-in against `alo-identity`
  (2FA field revealed on demand), and a **Mail read surface** — folders,
  message list, and a reading pane that renders plain text in Garamond and
  isolates untrusted HTML in a sandboxed, CSP-locked iframe that blocks remote
  content (no tracking pixels). Served at the same origin as the API behind
  Caddy; sign-in verified end-to-end on the live deployment. Compose/reply,
  PWA/offline, and the other modules are the next items. Design note
  `docs/design/web-shell.md`.
- New: **`alo-identity`** — the credential authority and an **OpenID
  Connect / OAuth 2.0 provider** (alo-as-IdP). It replaces every interim
  auth path: SMTP AUTH, IMAP/POP3 `LOGIN`, and the JMAP bearer now
  authenticate through one crate, and the dev `StaticAuthenticator`, the
  store's interim `auth.rs`, and the SMTP credentials-file loader are
  **deleted**. Passwords are **argon2id** (OWASP-baseline parameters,
  documented as a contract and overridable per deployment); **every secret
  comparison is constant-time** (the `subtle` crate), and an unknown user
  still pays one argon2 hash so *wrong password* and *no such user* are
  indistinguishable in time — closing the timing oracle the M3 TLS audit
  pinned here (proven by a timing test, not asserted: unknown-vs-wrong
  ratio ≈ 1.0). Tokens and recovery codes are stored only as SHA-256
  hashes; secrets never appear in a log, error, or `Debug`. The identity
  model is **tenants → users → aliases + groups**; `account_by_email`
  (inbound routing) is **alias-aware**; a tenant's first admin is created
  by the `identityctl` **CLI**, never a public endpoint. The **OAuth
  provider** offers discovery (RFC 8414), a JWKS, `authorization_code` with
  **mandatory PKCE `S256`** (RFC 6749/7636 — `plain` and challenge-less
  codes refused), and token / userinfo / revocation (RFC 7009). **Access
  tokens are opaque and revocable** (a logout truly invalidates); refresh
  tokens rotate on use and a replayed refresh token **revokes the whole
  token chain**; authorization codes are single-use. **ID tokens are EdDSA
  (Ed25519) JWTs** with `kid` rotation designed in — `sub` is the stable
  opaque user id, never the email (ADR 0008 explains opaque-vs-JWT and
  EdDSA-vs-RS256). **TOTP 2FA** (RFC 6238) adds enrollment (provisioning
  URI), verification with a clock-drift window, and single-use recovery
  codes. **2FA is enforced everywhere it can be:** the OIDC flow prompts for
  the code, and the legacy protocols (IMAP/POP3/SMTP), which cannot prompt,
  **fail closed** for a TOTP-enabled account — a password-only login is
  refused (indistinguishably from a wrong password) so a phished password
  cannot bypass 2FA over IMAP. Credential endpoints — including the legacy
  ones — have per-`(client, )username` exponential backoff (not a lockout,
  which would be a denial-of-service lever). Reviewed + security-audited
  (two independent passes); cross-tenant **and** cross-account isolation is
  tested on every identity operation, and the OAuth flow's negative cases
  (wrong PKCE verifier, code/refresh replay → chain revoke, unregistered
  redirect, bad credentials) are covered. App-specific passwords + `XOAUTH2`
  are the sanctioned follow-up that lets a 2FA user drive a non-OAuth legacy
  client again. See `docs/design/identity.md` and
  `docs/decisions/0008-identity-and-token-model.md`.

- New: **inbound local delivery** — received mail now files into the account
  store with **Sieve at the boundary**, closing the SMTP → mailbox path
  (previously inbound mail terminated at a spool). On the MX role with a
  database configured, each `RCPT TO:` for a hosted domain is resolved against
  the store (`Store::account_by_email`, subaddress-aware): an **unknown local
  user is refused `550 5.1.1` at RCPT** (an honest immediate answer, never a
  silent drop or post-DATA backscatter), while the anti-open-relay guard still
  refuses non-local recipients to unauthenticated senders. At end of `DATA` the
  fully-stamped message (Received + Authentication-Results + body) is delivered
  to **each** resolved recipient through `AccountStore::deliver_sieve` (parse →
  spam score → Sieve → file), isolation inherited per recipient. Sieve
  `redirect`/`vacation` actions are enqueued through the existing outbound queue
  under the rule owner's identity, with all attacker-influenced header strings
  (`subject`/`from`/redirect address) **CR/LF-stripped before any header is
  built**, and the store's redirect-rate budget enforced on the real path.
  Delivery is **per-recipient, try-then-commit**: a transient store/blob fault
  yields a conservative whole-message `4xx` so the sender retries (RFC 5321 §6.1
  — **duplicate delivery is preferred to loss**; blobs dedup by content), and
  **no failure path loses mail**. Delivered bytes go to a **durable on-disk blob
  backend** (`BlobStore::local`, `ALO_SMTP_BLOB_DIR`, default `./blobs`), so a
  body survives a restart on single-node deployments without Garage/S3. The
  inbound **spool is retired as the local sink**: its all-local backlog is
  migrated into the store once at startup (before the queue runner claims), and
  it remains the outbound queue's durable store (unchanged). Reviewed +
  security-audited. See `docs/design/local-delivery.md` and the new inbound
  entries in `docs/interop.md`.

- New: **`alo-sieve`** + delivery-time filtering — user **Sieve** filter
  scripts (RFC 5228, with **vacation** RFC 5230, **subaddress** RFC 5233,
  **imap4flags** RFC 5232) compiled and run on the server at delivery time.
  Sieve scripts are user-supplied programs, so every limit is a security
  control: hard parse caps (script size, nesting depth, test-list length,
  string size) enforced *during* parse, an evaluation instruction budget,
  and `require` enforcement (an un-declared extension is a compile error).
  Actions keep/fileinto/discard/redirect/stop with **implicit keep**, and
  **no script failure ever loses mail** — a compile error, a budget overrun,
  or a `fileinto` to a non-existent folder (auto-create is off) all fall back
  to implicit keep. **Redirect storms are impossible by construction**
  (per-script count cap, per-account rolling rate budget, loop guards,
  self-redirect refusal) and **vacation** carries the full RFC 3834 backscatter
  guards plus per-correspondent `:days` suppression. Wired at the store's
  delivery entry (`AccountStore::deliver_sieve`, after spam scoring and before
  filing); scripts, suppression, and the redirect budget are per-account rows,
  so isolation is inherited (cross-tenant **and** cross-account CRUD and
  execution tested). **Rule management is JMAP for Sieve** (RFC 9661, ADR
  0007): `SieveScript/{get,set,validate}` compile-checked on `set`
  (`invalidScript`), with the sieve capability in the Session resource.
  Reviewed + security-audited. The `deliver_sieve` seam is now exercised on the
  real inbound path (see "inbound local delivery" above). See
  `docs/design/sieve-filtering.md` and `docs/decisions/0007-sieve-rule-management.md`.

- New: **`alo-imap`** — IMAP4rev2 (RFC 9051) / IMAP4rev1 (RFC 3501) and
  POP3 (RFC 1939) **compatibility shims** over the account store, so the
  installed base of mail clients (Thunderbird, Apple Mail, Outlook, phones
  over IMAP) can reach a alo mailbox unchanged. JMAP stays the native
  protocol (ADR 0001); these are thin translators over `AccountStore`, so
  tenant/account isolation is **inherited**, not re-implemented. IMAP on
  implicit TLS (993) and STARTTLS (143), POP3 on implicit TLS (995);
  `LOGIN`/`AUTHENTICATE PLAIN`/`LOGIN` are refused before TLS (no
  credentials in the clear) and both protocols cap failed authentications
  per connection. Full command set: `SELECT`/`EXAMINE`, `LIST`/`LSUB`
  (correct `%`/`*` wildcards + RFC 6154 special-use), `CREATE`/`DELETE`/
  `RENAME`, `STATUS`, `APPEND` (through the **same** ingestion path as
  delivery — no second parser), `FETCH` (`ENVELOPE`, `INTERNALDATE`,
  `RFC822.SIZE`, `FLAGS`, byte-exact `BODY[]`/`[HEADER]`/`[TEXT]`/
  `[HEADER.FIELDS]`/numbered parts with `<partial>`, and a bounded-honest
  `BODYSTRUCTURE`), `STORE`, `SEARCH`, `EXPUNGE`, `COPY`/`MOVE` (RFC 6851,
  with `COPYUID`/`APPENDUID`), every `UID` variant, and `IDLE` (RFC 2177)
  as **account-scoped push** off the per-account change cursor.
  **Stable per-mailbox UIDs and UIDVALIDITY** (schema migration 0006):
  strictly-ascending, never reused within an epoch, stable across
  reconnection; `EXPUNGE` renumbers sequence numbers, never UIDs. Covered
  by a cross-tenant **and** cross-account isolation suite plus UID-
  stability, concurrent-session, malformed/oversized-input, pipelining,
  STARTTLS, and POP3 integration tests over real TLS; reviewed and
  security-audited. `CONDSTORE`/`QRESYNC`, `SORT`/`THREAD`, `ACL`/`QUOTA`/
  `METADATA`, and sub-second IDLE via `LISTEN`/`NOTIFY` are additive
  follow-ups. See `docs/design/imap-pop3-shims.md`.

- Fixed: **account-scoped change visibility** — the JMAP/IMAP state cursor
  is now a **per-account** monotonic modseq (`account_modseq`, migration
  0005), not per-tenant, so a co-tenant user's activity can no longer
  advance another user's state token (closing a coarse activity-volume
  side channel and removing a spurious cross-account push wakeup). The
  change log was already per-account; only the counter was shared. State
  tokens stay opaque; `/changes` resumes unchanged.

- New: **`alo-jmap`** — the JMAP API (RFC 8620 core, RFC 8621 mail),
  an HTTP service over the store and alo's native client protocol.
  **A public contract from merge** (web/desktop/compat adapters speak
  it): the Session resource with honest, enforced limits; the
  Request/Response envelope with ordered method dispatch and result
  references (back-references); `Mailbox`, `Email`, and `Thread`
  `get`/`set`/`query`/`changes` mapped onto the store; blob
  upload/download (blob ids are the store's — one id space; download is
  tenant-scoped, served with the stored Content-Type and `nosniff`); and
  an EventSource push endpoint emitting `StateChange` per tenant with
  heartbeats. `/changes` is backed by a new per-tenant monotonic modseq
  and change log in the store (`alo-store::changes`), with opaque
  state tokens and an honest `cannotCalculateChanges`. **Interim bearer
  auth** (`/auth/token`, argon2 credentials in the store) resolves each
  token to `(tenant, account)` and enters the store only through
  `for_account` — behind a seam the future alo-identity OIDC replaces
  without touching method code. Isolation is **per-account** (accountId =
  user): every by-id read/mutate, `/changes`, `Thread/get`, and blob
  download is scoped to the token's `(tenant, user)`, so a user cannot
  reach another user's mail even within the same tenant. Covered by the
  wrong-tenant AND cross-account isolation suites (CI-gated), plus
  conformance, result-reference, concurrent-`/changes`, `/changes`
  pagination-group, and malformed/oversized-body tests, all against real
  Postgres.
  `EmailSubmission/set` (send), full MIME `bodyStructure`, and
  JMAP-over-WebSocket are follow-ups. See `docs/design/jmap-api.md`.

- New: **`alo-store`** — the account-scoped message store on
  PostgreSQL (system of record, via `sqlx` with compile-checked queries)
  and Garage/S3 (message bytes). **Isolation is structural, enforced by
  the type you hold:** user-owned mail data is reachable only through an
  `AccountStore`, obtained via `Store::for_account(TenantId, UserId)`,
  and every query bakes in its `(tenant, user)` predicate by construction
  — no API takes a `tenant_id` or `user_id` parameter, there is no
  ownership guard in any call path to forget, and a wrong-tenant *or*
  wrong-account lookup returns a clean `NotFound` (no cross-account
  oracle). Tenant-level provisioning (users, credentials) stays on a
  narrow `TenantStore` from `Store::for_tenant(TenantId)`. Entities: tenants, users, hierarchical mailboxes (with
  transactional total/unread counters), messages (with the parsed
  `Authentication-Results` verdict stored queryable), threads (RFC 8621
  §3 References-based), message↔mailbox membership, JMAP keywords/flags,
  and content-addressed blobs (SHA-256, per-tenant key prefix,
  ref-counted for a later GC sweep). Ids are opaque and random — no
  sequential integer crosses the API boundary. Ingestion writes the blob
  before the DB commit, so a crash leaves an invisible orphan (GC'd),
  never a visible message with a missing body. Full-text search
  (Postgres `tsvector`) over subject/addresses/body, updated in the same
  transaction as ingestion. Every list path is bounded by a `Page`. The
  Garage S3 backend is behind the `garage` cargo feature; tests use an
  in-memory backend. A **wrong-tenant and cross-account isolation suite**
  covers every public read and write path — proving two users of the same
  tenant cannot reach each other's rows with no guard in the path — and is
  required by CI, alongside threading
  property tests, concurrent-counter tests, and ingestion crash-safety
  tests (all against real Postgres). JMAP/IMAP endpoints, the Garage
  live-integration test, and the spool-migration tool are follow-ups.

- New: **Rspamd spam scoring** at DATA and **MTA-STS** policy serving
  (Phase 1 M4b), finishing M4's deferrals. On the MX role, after
  SPF/DKIM/DMARC, `alo-smtp` consults Rspamd over `POST /checkv2`
  (`ALO_SMTP_RSPAMD_URL`): a `reject` action refuses with **550**,
  `soft reject`/`greylist` defer with **451**, and otherwise the message
  is accepted with the score recorded as an `x-spam` method in
  `Authentication-Results`. A scanner that is unreachable, slow, or
  answers unparseably **fails closed** (451) — configuring a scanner and
  having it down never silently disables filtering. Scanning is off
  until the URL is set (`ALO_SMTP_RSPAMD_TIMEOUT_SECS` bounds the
  call). **MTA-STS** (RFC 8461): the policy (`mode`/`mx`/`max_age`, with
  a content-derived `id`) is rendered from config and served at
  `GET /.well-known/mta-sts.txt` on `ALO_SMTP_MTA_STS_ADDR` (plaintext
  behind the deploy TLS proxy); knobs `ALO_SMTP_MTA_STS_MODE/MX/
  MAX_AGE/ID`, with the `_mta-sts` and `mta-sts` DNS records documented
  in `docs/interop.md`. ARC, TLS-RPT reporting, and DMARC report
  delivery remain deferred (see ROADMAP).

- New: `alo-auth-mail` — the email-authentication trust stack (Phase
  1 M4), wired into `alo-smtp`. Inbound (MX) at DATA: **SPF** (RFC
  7208 full `check_host` with macro expansion and the 10-DNS-lookup /
  2-void-lookup hard limits), **DKIM** verification (RFC 6376 + Ed25519
  per RFC 8463; relaxed/simple canonicalization, `l=`/`x=`, multiple
  signatures), and **DMARC** (RFC 7489; public-suffix org-domain,
  relaxed/strict alignment, `p=reject` → 550, with `pct=` sampling per
  §6.6.4 so a sender mid-rollout is not enforced at 100%). Every verdict
  is recorded in **`Authentication-Results`** (RFC 8601) — the public
  contract downstream parses — plus a `Received-SPF` header; any
  pre-existing `Authentication-Results` bearing our authserv-id (and any
  `Received-SPF`) is stripped from inbound mail first (RFC 8601 §5) so a
  remote sender cannot forge the verdict. A DKIM signature whose `h=`
  omits `From` is a permfail (RFC 6376 §6.1.1). Outbound
  (submission): **DKIM signing** with RSA-2048 or Ed25519, keys
  addressed by `(domain, selector)` behind a `KeyStore` (file backend
  with permission checks and zeroizing buffers) so rotation is a config
  change. RSA uses `ring` (constant-time), not the `rsa` crate
  (RUSTSEC-2023-0071). New knobs: `ALO_SMTP_DKIM_DOMAIN/SELECTOR/KEY/
  ALGORITHM`. DMARC report delivery, ARC, MTA-STS, TLS-RPT, and Rspamd
  are deferred (see ROADMAP).

- New: `alo-smtp` TLS and authenticated submission (Phase 1 M3).
  **STARTTLS** (RFC 3207) on the MX and submission ports and **implicit
  TLS** (port 465), via rustls with the ring provider — pure Rust, no
  OpenSSL. A PEM certificate/key is loaded from disk
  (`ALO_SMTP_TLS_CERT`/`ALO_SMTP_TLS_KEY`) or a self-signed one is
  generated for development. **AUTH PLAIN and LOGIN** (RFC 4954),
  offered only on a submission port over active TLS; wrong password and
  unknown user are indistinguishable (535, anti-enumeration).
  **Submission listeners** (`ALO_SMTP_SUBMISSION_ADDR` for STARTTLS,
  `ALO_SMTP_IMPLICIT_TLS_ADDR` for 465) require authentication before
  MAIL (530) — closing the open-relay hole ahead of enabling outbound.
  Credentials come from `ALO_SMTP_CREDENTIALS_FILE` (a dev bootstrap;
  alo-identity replaces it in M9). **RFC 6409** submission fixups add
  a `Date:` and `Message-ID:` when absent. EHLO now advertises a
  truthful capability set (STARTTLS/AUTH/SIZE/8BITMIME) reflecting the
  connection's exact state, and MAIL accepts `SIZE=`/`BODY=`/`AUTH=`
  parameters for the advertised extensions. `Received:` records
  `ESMTPS` for TLS-protected sessions (RFC 3848).
- New: `alo-smtp` outbound delivery (Phase 1 M2) — a durable queue
  over the spool relays accepted mail. MX resolution (RFC 5321 §5.1:
  preference order, implicit MX, RFC 7505 null-MX = permanent),
  outbound SMTP client with RFC 5321 §4.5.3.2 timeouts and
  dot-stuffing, exponential backoff with jitter (4xx transient vs 5xx
  permanent), per-recipient durable state so a partial delivery never
  re-sends to already-delivered recipients, and RFC 3464 DSN bounces
  from the null sender (never bouncing a null-sender message, §4.5.5).
  **Relay safety: outbound is OFF by default** — enabled only via
  `ALO_SMTP_OUTBOUND_ENABLED=true`, because open relaying must wait
  for the AUTH gate (M3). `ALO_SMTP_SMARTHOST` routes all mail to
  one host (self-hosted mode). Knobs: `ALO_SMTP_RETRY_BASE_SECS`,
  `ALO_SMTP_RETRY_CAP_SECS`, `ALO_SMTP_MAX_ATTEMPTS`,
  `ALO_SMTP_QUEUE_INTERVAL_SECS`. Domainless recipients (bare
  `postmaster`) are parked pending local delivery (M5), never dropped.
- New: `alo-smtp` receives mail end-to-end (Phase 1 M1) — full
  MAIL FROM / RCPT TO / DATA transactions with RFC 5321 sequencing
  (503 on out-of-order commands), address parsing incl. quoted local
  parts, address literals, source routes, the null sender and
  `<postmaster>`; DATA with dot-unstuffing, the size limit enforced
  during read (552), and bare-line-ending rejection (SMTP-smuggling
  defense); a `Received:` header stamped on every accepted message;
  durable maildir-style spool (`ALO_SMTP_SPOOL_DIR`) with fsync +
  atomic-rename commit. New knobs: `ALO_SMTP_MAX_MESSAGE_SIZE`
  (default 25 MiB), `ALO_SMTP_MAX_RCPT` (default 100). HELO, RSET,
  NOOP, VRFY (252, anti-enumeration), HELP/EXPN → 502.
- New: `alo-smtp` service — accepts TCP connections on port 2525,
  greets with a 220 banner, and answers EHLO and QUIT with
  RFC 5321-correct replies. Enforces the 512-octet command-line limit
  during read, rejects bare-LF line endings (SMTP-smuggling defense),
  and closes idle sessions after 5 minutes with 421. Configuration:
  `ALO_SMTP_ADDR`, `ALO_SMTP_HOSTNAME`. `--healthcheck` flag
  probes a running instance for container health.
- New: `deploy/docker-compose.yml` — the pinned engine set (Synapse
  v1.157.1, LiveKit v1.13.4, Collabora CODE 25.04.9.4.1, Garage
  v2.3.0, PostgreSQL 16.14, Rspamd 4.1.2) plus alo-smtp, with
  healthchecks and `.env.example`.
- New: `scripts/fetch-engines.sh` — clones engine sources into
  `../engines` (read-only reference) at exactly the compose-pinned
  versions.
- New: CI runs the quality gate on every PR; releases build from tags
  only.
