# ADR 0027 — The Drive file model: one location, access follows location

Status: accepted. The first module to attach to the Space object (ADR 0026).
Defines how files, folders, and documents are stored, scoped, moved, and
versioned. Builds on the content-addressed blob store (ADR 0006).

## Context

alo Drive replaces OneDrive **and** SharePoint with **one** coherent layer:
every file lives in exactly one location — the user's personal "My Files" or a
Space — and there is no second, parallel "team files" concept with its own
rules. The permission maze users hate comes from files that carry their own
sharing state; we refuse that. A file's access is simply its location's access.

## Decision

### The tree

`drive_nodes` — one row per folder, file, or document:

- `tenant_id → tenants ON DELETE CASCADE`, `id`, `parent_id` (nullable; null =
  a location root).
- `location_kind ∈ {personal, space}` + `location_id` (the owning user id, or
  the `space_id`). This pair **is** the access scope.
- `kind ∈ {folder, file, doc, sheet, slides}`. `doc/sheet/slides` are files
  whose bytes are an office document edited in Collabora (ADR 0010) — they live
  in the tree exactly like any other file.
- `name`, `blob_id` (null for folders), `size`, `content_type`, `trashed`,
  `created_by`, `created_at`, `updated_at`.

Indexes on `(tenant_id, location_kind, location_id, parent_id)` (list a folder)
and `(tenant_id, location_kind, location_id, trashed)` (the trash view).

### Access follows location (the whole model)

- `personal:<user>` → visible and writable only to that user.
- `space:<id>` → **read** for any Space member, **write** for `editor`+
  (ADR 0026). No per-node permission exists or can be set.
- A non-member / wrong tenant gets `404`.

This makes sharing a file a matter of **where it is**, not a dialog of
checkboxes. "Who can see this?" always has the same answer: whoever is in its
location — and that membership is always visible (Law: trust is visible).

### Move = re-scope (stated plainly because it is a security boundary)

Moving a node changes its `location_*`, and therefore its access. Moving a file
**into** a Space grants every member access to it; moving it **out** revokes
them. This is intended and is the model's core simplification — but it is a real
grant/revoke, so: a move requires `editor`+ in **both** the source and the
destination location, folders move with their whole subtree, and the move is
one transaction (no half-moved subtree, no orphan).

### Copy

Copy duplicates the node (and, for a file, references the same content-addressed
blob — dedup is free) into the destination, subject to the same both-locations
permission check. The copy is independent thereafter.

### Versioning

`drive_node_versions` — `(tenant_id, node_id, version_no, blob_id, size,
created_by, created_at)`. Every upload or document save appends a version; the
node's current `blob_id` is the latest. **Restore** appends a *new* version
pointing at an old blob — history is never rewritten, so restore is itself
undoable. Blobs are content-addressed and left in place on delete (a blob may be
shared across versions/copies), consistent with task attachments.

### Storage

Bytes go to Garage through the existing tenant-scoped blob API (`put_blob` /
`blob_bytes_for_send`): content-addressed, deduplicated per tenant, quota-
enforced (ADR 0012), never re-implemented. Drive owns the *tree and versions*;
the blob store owns the *bytes*. One responsibility each (Law 3).

### Trash / restore-from-trash

`trashed = true` is a soft delete (subtree-wide for a folder); a trash view
lists trashed nodes per location; restore clears the flag; purge is a permanent
delete from trash. Nothing is hard-deleted from a normal file action.

### Source links (workspace-native, ADR 0029)

A node may carry an optional `source_kind` / `source_id` (`email` / `task` /
`event`) — the same jump-back pattern tasks use. Saving a mail attachment to
Drive keeps its link to the message. Additive columns; unused for plain uploads.

## Rejected alternatives

- **OneDrive + SharePoint split (two file systems).** The thing we are
  replacing; two models, two mental maps, endless "which one is it in?".
- **Per-file ACLs / share-with-person.** Reintroduces the maze; incompatible
  with access-follows-location. Space membership is the sharing primitive.
- **A separate document service/subdomain for Docs/Sheets/Slides.** They are
  file types in the drive, not a separate app; keeping them as `drive_nodes`
  means they inherit location, permission, versioning, and trash for free.
- **Building storage over raw Garage from scratch.** Doctrine: engines are
  integrated behind our API. The blob store already is that API.

## Consequences

- "Where is it?" and "who can see it?" are the same question — the calm,
  un-SharePoint model the product promises.
- Docs/Sheets/Slides, previews, search, and AI all operate over one node table,
  so each is built once, not per-surface.
- The move-re-scopes rule is powerful and must be taught in the UI (a visible
  "this will share with the Space" confirmation), not hidden.
- Isolation is testable identically to tasks: a permanent wrong-tenant +
  non-member suite over every read/write/move/version path.
