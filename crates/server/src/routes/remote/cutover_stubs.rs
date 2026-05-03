//! Read-side stubs for entity shapes the kanban frontend resolves through
//! [`localRouteResolver`] but for which the local-cutover SQLite backend has
//! not yet wired full CRUD routes. Without these, reads fall through to
//! `/v1/fallback/*` (no handler) and return HTTP 404 — which the kanban
//! side-panel surfaces as broken state for comments / followers / reactions
//! and the org-member listing.
//!
//! The local backend stores these entities (see `db::models::issue_follower`,
//! `db::models::issue_comment`, `db::models::issue_comment_reaction`,
//! `db::models::user`); a Round 2 follow-up will replace these stubs with
//! real CRUD routes. For now:
//!   - `GET` returns an empty JSON array so the side-panel renders an empty
//!     state instead of crashing on a 404
//!   - mutations are not yet supported and fall through to axum's default
//!     `404 Not Found` until the follow-up wires them; the frontend already
//!     guards mutations behind explicit user intent
//!
//! This unblocks Parent 3 DoD ("kanban side-panel comments/followers/
//! reactions and org-member listings render post-cutover") at the read seam.

use axum::{Router, response::Json as ResponseJson, routing::get};
use serde_json::Value;
use utils::response::ApiResponse;

use crate::DeploymentImpl;

async fn empty_list() -> ResponseJson<ApiResponse<Vec<Value>>> {
    ResponseJson(ApiResponse::success(Vec::new()))
}

pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/notifications", get(empty_list))
        .route("/organization-member-metadata", get(empty_list))
        .route("/users", get(empty_list))
        .route("/issue-followers", get(empty_list))
        .route("/issue-comments", get(empty_list))
        .route("/issue-comment-reactions", get(empty_list))
}
