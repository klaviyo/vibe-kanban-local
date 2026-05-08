//! Cloud `/remote/workspaces/by-local-id/...` was a lookup against the cloud
//! workspace mirror. Local mode has no such mirror — the canonical mapping
//! between a local workspace and an issue lives in `workspace_issue_links`.
//!
//! We synthesize an `api_types::Workspace` shape from `workspace_issue_links`
//! and the synthetic local user so existing callers (e.g. `routes::workspaces::git`)
//! still receive a useful payload when probing this URL. If no link exists we
//! return a `BadRequest`, which callers already tolerate.
//!
//! `GET /workspaces` (the kanban frontend's project- or owner-scoped list
//! shape) is also served from local SQLite: `project_id` enumerates every
//! `workspace_issue_link` whose project matches and synthesizes one wire
//! `Workspace` row per link; `owner_user_id` falls back to "every local
//! workspace" because in single-user mode there is exactly one user and
//! all workspaces are owned by them — see the deliberate single-user-mode
//! framing already established by `users.rs` and `notifications.rs`.

use api_types::Workspace as WireWorkspace;
use axum::{
    Router,
    extract::{Path, Query, State},
    response::Json as ResponseJson,
    routing::get,
};
use db::models::{workspace::Workspace, workspace_issue_link::WorkspaceIssueLink};
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError, runtime::synthetic};

/// Table-keyed list envelope for `GET /workspaces`. The wire field name
/// (`workspaces`) matches the snake-case table name the kanban
/// frontend's `extractRows` looks up via `data[table]`. Defined locally
/// rather than in `api-types` because no `ListWorkspacesResponse` wire
/// type is shared with the cloud surface — the cloud's `Workspace`
/// table has no direct local mirror, so this is a local-only synthesis
/// shape contract.
#[derive(Debug, Serialize)]
pub(super) struct WorkspacesResponse {
    pub workspaces: Vec<WireWorkspace>,
}

/// Workspaces list query — accepts either `project_id` (project scope)
/// or `owner_user_id` (single-user-mode fallback). Exactly one must be
/// present; supplying both is rejected with 400. The two variants
/// exist because the kanban defines two distinct shapes against the
/// same table (see `localRouteResolver.LOCAL_ROUTES_BY_TABLE.workspaces`).
#[derive(Debug, Deserialize)]
pub(super) struct ListWorkspacesQuery {
    #[serde(default)]
    pub project_id: Option<Uuid>,
    #[serde(default)]
    pub owner_user_id: Option<Uuid>,
}

pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/workspaces", get(list_workspaces))
        .route(
            "/workspaces/by-local-id/{local_workspace_id}",
            get(get_workspace_by_local_id),
        )
}

async fn list_workspaces(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListWorkspacesQuery>,
) -> Result<ResponseJson<ApiResponse<WorkspacesResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let user = synthetic::local_user(&deployment).await?;

    let workspaces = match (query.project_id, query.owner_user_id) {
        (Some(project_id), None) => list_for_project(pool, project_id, user.id).await?,
        (None, Some(owner_user_id)) => {
            // Single-user-mode simplification (mirrors the deliberate
            // pattern established by `users.rs` / `notifications.rs`):
            // requests for the synthetic user surface every local
            // workspace; any other owner id surfaces nothing because
            // there is no second user to own them.
            if owner_user_id == user.id {
                list_for_owner(pool, user.id).await?
            } else {
                Vec::new()
            }
        }
        (Some(_), Some(_)) => {
            return Err(ApiError::BadRequest(
                "project_id and owner_user_id are mutually exclusive".to_string(),
            ));
        }
        (None, None) => {
            return Err(ApiError::BadRequest(
                "project_id or owner_user_id is required".to_string(),
            ));
        }
    };

    Ok(ResponseJson(ApiResponse::success(WorkspacesResponse {
        workspaces,
    })))
}

/// Synthesize one wire-shape `Workspace` per `workspace_issue_link` in
/// the project, joining the underlying `workspaces` row for the
/// human-meaningful `name` / `archived` / timestamps. The cloud's
/// `Workspace` table had a stable `(workspace_id, project_id, issue_id)`
/// triple; locally that triple lives on the link row, so the link is
/// the canonical source of truth.
async fn list_for_project(
    pool: &sqlx::SqlitePool,
    project_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<WireWorkspace>, ApiError> {
    let links = WorkspaceIssueLink::find_by_project(pool, project_id).await?;
    let mut out = Vec::with_capacity(links.len());
    for link in links {
        if let Some(ws) = Workspace::find_by_id(pool, link.workspace_id).await? {
            out.push(WireWorkspace {
                id: ws.id,
                project_id: link.project_id,
                owner_user_id: user_id,
                issue_id: Some(link.issue_id),
                local_workspace_id: Some(ws.id),
                name: ws.name,
                archived: ws.archived,
                files_changed: None,
                lines_added: None,
                lines_removed: None,
                created_at: ws.created_at,
                updated_at: ws.updated_at,
            });
        }
    }
    Ok(out)
}

/// Owner-scoped fallback: enumerate every workspace in the local
/// database. Each row's `project_id`/`issue_id` is derived from its
/// first link (if any); workspaces with no link surface the synthetic
/// user as owner and `Uuid::nil()` for the project, mirroring the
/// `get_workspace_by_local_id` failure mode rather than excluding the
/// row outright.
async fn list_for_owner(
    pool: &sqlx::SqlitePool,
    user_id: Uuid,
) -> Result<Vec<WireWorkspace>, ApiError> {
    let workspaces = Workspace::fetch_all(pool).await?;
    let mut out = Vec::with_capacity(workspaces.len());
    for ws in workspaces {
        let links = WorkspaceIssueLink::find_by_workspace(pool, ws.id).await?;
        let (project_id, issue_id) = links
            .into_iter()
            .next()
            .map(|l| (l.project_id, Some(l.issue_id)))
            .unwrap_or((Uuid::nil(), None));
        out.push(WireWorkspace {
            id: ws.id,
            project_id,
            owner_user_id: user_id,
            issue_id,
            local_workspace_id: Some(ws.id),
            name: ws.name,
            archived: ws.archived,
            files_changed: None,
            lines_added: None,
            lines_removed: None,
            created_at: ws.created_at,
            updated_at: ws.updated_at,
        });
    }
    Ok(out)
}

async fn get_workspace_by_local_id(
    State(deployment): State<DeploymentImpl>,
    Path(local_workspace_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<WireWorkspace>>, ApiError> {
    let pool = &deployment.db().pool;

    let workspace = Workspace::find_by_id(pool, local_workspace_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Workspace not found".to_string()))?;

    let links = WorkspaceIssueLink::find_by_workspace(pool, workspace.id).await?;
    let link = links
        .into_iter()
        .next()
        .ok_or_else(|| ApiError::BadRequest("Workspace is not linked to an issue".to_string()))?;

    let user = synthetic::local_user(&deployment).await?;

    let wire_workspace = WireWorkspace {
        id: workspace.id,
        project_id: link.project_id,
        owner_user_id: user.id,
        issue_id: Some(link.issue_id),
        local_workspace_id: Some(workspace.id),
        name: workspace.name,
        archived: workspace.archived,
        files_changed: None,
        lines_added: None,
        lines_removed: None,
        created_at: workspace.created_at,
        updated_at: workspace.updated_at,
    };

    Ok(ResponseJson(ApiResponse::success(wire_workspace)))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn list_envelope_is_table_keyed() {
        let envelope: ApiResponse<WorkspacesResponse> = ApiResponse::success(WorkspacesResponse {
            workspaces: Vec::new(),
        });
        let body = serde_json::to_value(&envelope).expect("serialize envelope");
        assert_eq!(
            body,
            json!({
                "success": true,
                "data": { "workspaces": [] },
                "error_data": null,
                "message": null,
            }),
            "workspaces list must use the table-keyed ApiResponse envelope; \
             extractRows reads data[\"workspaces\"] and an empty list must \
             still surface that field, not a 404"
        );
    }
}
