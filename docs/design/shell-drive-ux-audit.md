# Shell and Drive UX constitution audit

Date: 2026-08-08  
Scope: launcher rail, Drive document menus, and the page-mode two-row toolbar.  
Domain references: Google Workspace app launcher; Microsoft Word and Google
Docs page editors; Notion canvas editing.

This audit applies the menu test from `ux-principles.md`: mentally remove every
menu and confirm that the screen's core task remains possible through visible
controls. Each defect below is intentionally a separate implementation slice.

## Launcher rail

Top actions and click counts:

1. Open Home: one click.
2. Open any of the six chosen favorite apps: one click.
3. Open a less-used app: Apps catalog, then app — two clicks.

Result: **pass**. The six user-chosen primary destinations remain visible and
one click away. The labeled Apps control opens a navigation catalog for depth;
it does not hide the user's primary destinations. Editing the favorite set is
also a rare configuration action, so it may remain in the catalog. The rail
continues to work fully with AI disabled.

## Document editor — page mode

Top actions and click counts:

1. Write and format text: direct manipulation; formatting is one click.
2. Insert a link, image, table, code block, equation, or comment: one click from
   the visible toolbar.
3. Find text or print: one click from the visible toolbar.

The File, Edit, Insert, and Format menus duplicate visible controls. Page setup,
page breaks, and explicit PDF export are depth actions consistent with Word and
Google Docs; they do not gatekeep the core writing flow.

### Confirmed defects and fix slices

1. **D-UX-01 — Lists are hidden behind recall controls.** Bulleted, numbered,
   and checklist formatting exist only inside the paragraph-style selector and
   slash suggestions. Surface three visible list buttons beside alignment and
   indentation. Keep the selector entries as duplicates. Verify each changes
   the selected block in one click and reflects the active block type.

2. **D-UX-02 — Narrow page toolbars hide primary controls.** The formatting row
   horizontally scrolls, so insertion, find, and print can leave the visible
   surface without any indication. Establish a responsive primary cluster that
   stays visible, then let depth controls overflow or collapse only when their
   visible menu duplicate remains. Verify at desktop, tablet, and phone widths.

3. **D-UX-03 — Toolbar targets are below the constitution minimum.** Several
   controls are 30px high although interactive targets must be at least 40px.
   Add role-based control and icon tokens in a dedicated token commit, then
   migrate the two toolbar rows without increasing visual clutter.

4. **D-UX-04 — Recent editor CSS contains unlegislated dimensions and colors.**
   The color picker and toolbar use hardcoded pixel and color values. Introduce
   the missing role tokens in isolated, reasoned token commits and migrate one
   component group at a time: toolbar geometry, picker geometry, then picker
   color primitives.

## Document editor — canvas mode

Result: **pass for the core task**. A user can type immediately, create the next
block with Enter, and use the visible block affordance. Slash suggestions and
the block catalog deepen insertion in the Notion convention; neither is needed
to begin or continue writing. AI is optional and has direct deterministic
siblings in page mode.

## Implementation order

1. D-UX-01 visible list controls.
2. D-UX-03 role tokens and 40px targets.
3. D-UX-02 responsive primary toolbar cluster.
4. D-UX-04 token cleanup in three mechanical slices.

