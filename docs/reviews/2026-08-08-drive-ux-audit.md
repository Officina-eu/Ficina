# Drive UX constitution audit

Date: 2026-08-08  
Scope: file and space navigation, folders, list and icon views, selection,
upload and creation, drag and drop, Trash, version history, members, previews,
Base views, document editing, spreadsheet editing, Office editing, loading,
empty, error, and responsive states.  
Domain references: Windows File Explorer, macOS Finder, Google Drive,
Microsoft Excel, Microsoft Word, Google Docs, Airtable, and Notion.

This review applies the menu test from `docs/design/ux-principles.md`: remove
every menu mentally and verify that a user can still find, open, create,
upload, select, organise, and recover Drive content. Editor findings from
`2026-08-08-shell-drive-ux-audit.md` are incorporated here rather than
duplicated.

## What passes

- My Files, Spaces, and Trash are visible destinations. Folder disclosure is
  inline, so browsing preserves context instead of navigating through a chain
  of otherwise identical screens.
- Upload and New are visible at the top level. The New picker chooses a file
  type and therefore provides legitimate depth; it does not conceal the fact
  that content can be created.
- Sort and View expose current state and match Explorer/Finder conventions.
  File extensions, compact density, navigation pane, and preview depth may
  remain in View.
- Root empty folders teach two concrete next steps: Upload and New folder.
- Spreadsheet and document editors surface their primary formatting actions.
  Their menus duplicate visible controls and retain only setup or export depth.
- Permanent deletion and member removal may retain confirmation because they
  are irreversible or revoke another person’s access.

## Confirmed defects and fix slices

1. **DRV-UX-01 — Structural loading and failed loading are indistinguishable.**
   The file list, nested folders, editor boot, version history, Base, document,
   sheet, and Office loading use indeterminate spinners despite having known
   structure. Some failed requests become an empty list or an endless spinner.
   Replace known structures with accessible skeletons and give failed surfaces
   a human explanation plus Retry.

2. **DRV-UX-02 — Errors are routinely discarded.** Creation, upload, download,
   rename, move, copy, trash, restore, version restore, membership, and editor
   loading contain empty catches or generic messages. Preserve the server
   reason verbatim, state what failed, and keep the failed action available.

3. **DRV-UX-03 — Reversible trashing uses ceremony instead of Undo.** Moving an
   item to Trash asks for confirmation and then provides no recovery feedback.
   Move immediately, show a visible success notice with Undo, and reserve
   confirmation for Delete forever.

4. **DRV-UX-04 — Explorer/Finder selection is absent.** Users cannot select one
   or several nodes, see selection state, or operate on a selection. Add visible
   item checkboxes, keyboard-safe selection, a selection toolbar, and bulk
   move/copy/trash/restore where the API supports the same single-node actions.
   Keep row overflow as a duplicate for low-frequency actions.

5. **DRV-UX-05 — Important row actions rely on an overflow menu.** Download,
   rename, and Trash/Restore are invisible until a menu is opened. Once an item
   is selected, surface the safe, relevant actions in the selection toolbar so
   the menu no longer gatekeeps ordinary organisation.

6. **DRV-UX-06 — Nested and secondary empty states do not onboard.** An expanded
   empty folder is text-only, version history collapses load failure into “no
   versions,” and several Base states only describe a missing configuration.
   Give each actionable empty state one sentence and one next-step control.

7. **DRV-UX-07 — Base calendar breaks its domain reflex.** Dates are not
   interactive even though calendar users expect a date click to create a
   record for that day. The previous/next labels are hardcoded and vague.
   Make each date a one-click creation surface and externalise descriptive
   navigation labels.

8. **DRV-UX-08 — Interaction roles and tokens are inconsistent.** File rows,
   dialog actions, Base controls, and parts of the editors contain component-
   born dimensions, colour literals, inline indentation, and targets below the
   40px role. Migrate in bounded groups: file manager/dialogs, Base, document,
   then sheet/ribbon. Keep editor-specific density only where the token scale
   explicitly legislates it.

9. **DRV-UX-09 — Responsive behavior hides context and actions.** Narrow file
   manager and editor surfaces can overflow without preserving primary actions.
   Keep location, New/Upload, selection feedback, and editor primary clusters
   visible; let only legitimate depth scroll or collapse.

## Top-action click count

| Surface | Action | Current | Required |
| --- | --- | ---: | ---: |
| File manager | Open file or folder | 1 | 1 |
| File manager | Upload | 1 | 1 |
| File manager | Create chosen type | 2 | 2 |
| File manager | Select one item | unavailable | 1 |
| File manager | Rename selected item | 2 | 1 |
| File manager | Move selected items to Trash | 3 | 1 + Undo |
| Trash | Restore selected items | 2 | 1 |
| Base calendar | Create on a date | unavailable | 1 |

## Implementation order

DRV-UX-01 and DRV-UX-02; DRV-UX-03; DRV-UX-04 and DRV-UX-05;
DRV-UX-06 and DRV-UX-07; then DRV-UX-08 and DRV-UX-09 as bounded surface
migrations. Every slice is gated and pushed independently.

