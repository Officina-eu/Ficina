//! Workspace search (ADR 0029) — one query across the modules, scoped to exactly
//! what the caller may already see (their personal items, the Spaces they belong
//! to, their visible task projects). Names/titles first (this slice); content
//! (full-text over file bytes + mail bodies) and cross-module ranking come next.
//! Access is applied in SQL, never widened — the same predicates the modules use.

use crate::account::AccountStore;
use crate::error::{Result, StoreError};

/// One search result, enough to render a row and open the item.
#[derive(Debug, Clone)]
pub struct SearchHit {
    /// `folder` | `file` | `doc` | `base` (a Drive node kind) or `task`.
    pub kind: String,
    pub id: String,
    pub title: String,
    /// Where it lives — a Space id for a Space file, else `None` (personal /
    /// task). Lets the UI open it in the right place.
    pub space: Option<String>,
}

impl AccountStore {
    /// Searches the workspace by name/title. Returns up to `limit` Drive nodes
    /// (in the caller's personal files or member Spaces) and up to `limit`
    /// visible active tasks whose title matches — a substring, case-insensitive.
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

        Ok(hits)
    }
}
