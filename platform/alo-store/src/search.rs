//! Workspace search (ADR 0029) — one query across the modules, scoped to exactly
//! what the caller may already see (their personal items, the Spaces they belong
//! to, their visible task projects, their own mailbox). Files and tasks match by
//! name/title; mail matches by full content — the message body is in the mail
//! full-text index, so this searches *inside* the email, not just its subject.
//! Access is applied in SQL, never widened — the same predicates the modules use.
//! Still to come: content search inside Drive file bytes (needs per-format text
//! extraction) and cross-module relevance ranking.

use crate::account::AccountStore;
use crate::error::{Result, StoreError};

/// One search result, enough to render a row and open the item.
#[derive(Debug, Clone)]
pub struct SearchHit {
    /// `folder` | `file` | `doc` | `base` (a Drive node kind), `task`, or
    /// `message` (a mail message).
    pub kind: String,
    pub id: String,
    pub title: String,
    /// Where it lives — a Space id for a Space file, else `None` (personal /
    /// task). Lets the UI open it in the right place.
    pub space: Option<String>,
}

impl AccountStore {
    /// Searches the workspace. Returns up to `limit` Drive nodes (in the caller's
    /// personal files or member Spaces) and up to `limit` visible active tasks
    /// whose name/title matches — a substring, case-insensitive — plus up to
    /// `limit` of the caller's own messages whose subject, participants, or
    /// **body** match, via the mail full-text index (content search).
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn workspace_search(&self, query: &str, limit: i64) -> Result<Vec<SearchHit>> {
        let q = query.trim();
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let mut hits = Vec::new();

        // Drive nodes: name match, in a location the caller can read.
        let drive = sqlx::query_as::<_, (String, String, String, Option<String>)>(
            "SELECT id, kind, name, \
                    CASE WHEN location_kind = 'space' THEN location_id ELSE NULL END AS space \
             FROM drive_nodes \
             WHERE tenant_id = $1 AND trashed = false \
               AND strpos(lower(name), lower($3)) > 0 \
               AND ( (location_kind = 'personal' AND location_id = $2) \
                  OR (location_kind = 'space' AND location_id IN ( \
                        SELECT space_id FROM space_members \
                        WHERE tenant_id = $1 AND user_id = $2)) ) \
             ORDER BY updated_at DESC LIMIT $4",
        )
        .bind(self.tenant.as_str())
        .bind(self.user.as_str())
        .bind(q)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(StoreError::Db)?;
        for (id, kind, name, space) in drive {
            hits.push(SearchHit { kind, id, title: name, space });
        }

        // Tasks: title match, on a project visible to the caller (team, or their
        // own personal) — the same predicate the task module uses.
        let tasks = sqlx::query_as::<_, (String, String)>(
            "SELECT t.id, t.title FROM tasks t \
             WHERE t.tenant_id = $1 AND t.state = 'active' \
               AND strpos(lower(t.title), lower($3)) > 0 \
               AND t.project_id IN ( \
                     SELECT p.id FROM task_projects p WHERE p.tenant_id = $1 \
                       AND p.archived = false \
                       AND (p.kind = 'team' OR (p.kind = 'personal' AND p.owner_user_id = $2))) \
             ORDER BY t.updated_at DESC LIMIT $4",
        )
        .bind(self.tenant.as_str())
        .bind(self.user.as_str())
        .bind(q)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(StoreError::Db)?;
        for (id, title) in tasks {
            hits.push(SearchHit {
                kind: "task".to_owned(),
                id,
                title,
                space: None,
            });
        }

        // Mail: full-text over the message's subject, participants, AND body —
        // the `search` tsvector the mail module builds and queries. Scoped to the
        // caller's own mail (`user_id`), exactly as `AccountStore::search` is.
        // This is the content-search half: a term only in the body still matches.
        let mail = sqlx::query_as::<_, (String, String)>(
            "SELECT id, subject FROM messages \
             WHERE tenant_id = $1 AND user_id = $2 \
               AND search @@ plainto_tsquery('simple', $3) \
             ORDER BY received_at DESC LIMIT $4",
        )
        .bind(self.tenant.as_str())
        .bind(self.user.as_str())
        .bind(q)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(StoreError::Db)?;
        for (id, subject) in mail {
            let title = if subject.trim().is_empty() {
                "(no subject)".to_owned()
            } else {
                subject
            };
            hits.push(SearchHit {
                kind: "message".to_owned(),
                id,
                title,
                space: None,
            });
        }

        Ok(hits)
    }
}
