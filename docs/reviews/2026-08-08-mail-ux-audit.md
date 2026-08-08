# Mail UX constitution audit

Date: 2026-08-08  
Scope: folder navigation, message list and search, reading pane, conversation
actions, compose, recipients, attachments, categories, snooze, invitations,
spam handling, and responsive behavior.  
Domain references: Gmail and Microsoft Outlook.

This review applies the menu test from `docs/design/ux-principles.md`: remove
every menu mentally and verify that reading, writing, replying, searching, and
organising mail remain understandable and usable.

## What passes

- Compose, recipients, subject, body, attachment, formatting, and Send are
  visible. Schedule send is legitimate depth because it requires choosing a
  time; the menu does not gate ordinary sending.
- Reply, Reply all, Forward, Archive, Move, Flag, Categorise, Delete, and Snooze
  are visible in the reading pane. Move, Categorise, and Snooze require a
  destination or value, so their pickers are appropriate.
- Folder navigation, Compose, search, conversation/message view, bulk
  selection, and message opening remain visible without context menus.
- Spam and invitation cards expose their deterministic actions directly. AI
  summaries and suggested replies are optional enhancements with direct mail
  actions alongside them.
- Folder/category rename, colour, and delete may remain in overflow/context
  menus: navigation and creation remain visible, while these are infrequent
  management actions. Irreversible deletion and external unsubscribe may keep
  confirmation; ordinary archive/delete already use undo-oriented flows.

## Confirmed defects and fix slices

1. **M-UX-01 — Empty states do not always teach one next step.** The empty
   message list is text-only, and the unselected reading pane has no action.
   Surface Compose in both; a no-result search instead surfaces Clear search.

2. **M-UX-02 — Structural loading uses spinners.** Folder navigation, message
   results, and the reading pane show indeterminate spinners although their
   layout is known. Replace them with stable, accessible skeletons. Keep small
   in-action progress indicators for attachment upload and Send, where a
   spinner describes a bounded operation rather than page loading.

3. **M-UX-03 — Mark unread is gatekept.** The open conversation exposes Mark
   unread only in More actions. Add it to the visible reading toolbar and keep
   the overflow entry as a duplicate.

4. **M-UX-04 — Hover and undersized targets hide row organisation.** Checkbox,
   flag, row quick actions, Reply, Reply all, and Forward are below the 40px
   interaction role. Row quick actions disappear on devices without hover.
   Migrate targets to the shared role and keep quick actions visible on touch
   and keyboard focus.

5. **M-UX-05 — Errors discard the useful reason.** Compose send and attachment
   failures replace server errors with generic text. Present what happened,
   preserve the server reason verbatim when available, and retain the visible
   retry path.

6. **M-UX-06 — Mail styling predates the role scale.** Mail component styles
   still contain component-born dimensions and colour literals. Migrate by
   bounded component group after the behavioral defects: list/reading chrome,
   compose/recipient controls, then folder/category depth surfaces.

## Top-action click count

| Surface | Action | Current | Required |
| --- | --- | ---: | ---: |
| Folder/list | Compose | 1 | 1 |
| List | Open conversation | 1 | 1 |
| List | Archive on touch | 2+ | 1 |
| Reading | Reply | 1 | 1 |
| Reading | Mark unread | 2 | 1 |
| Compose | Send | 1 | 1 |

## Implementation order

M-UX-01, M-UX-02, M-UX-03, M-UX-04, M-UX-05, then M-UX-06 as three
mechanical token migrations.
