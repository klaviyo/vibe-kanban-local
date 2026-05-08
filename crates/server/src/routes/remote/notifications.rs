//! Notifications endpoint — **deliberate no-op** in single-user local mode.
//!
//! The cloud product surfaces inbox notifications generated when *other*
//! members of an organization act on issues you follow (status changes,
//! comments, mentions, reactions, …). Single-user local mode has only the
//! synthetic user as a member of the personal organization, so no
//! actor-other-than-self ever exists; there is nothing to deliver. The
//! kanban frontend resolves this entity through `localRouteResolver` and
//! reads the list via `extractRows`, which looks up `data[<table>]`
//! on the envelope — so this handler returns
//! `ApiResponse<NotificationsResponse>` where `NotificationsResponse`
//! exposes a single `notifications: Vec<Notification>` field, always
//! empty in single-user mode. This matches the table-keyed convention
//! used by the real-CRUD modules in this directory (e.g.
//! `ListIssueFollowersResponse { issue_followers: Vec<…> }`) and
//! returns an empty list rather than 404 (which the side-panel would
//! surface as broken state).
//!
//! This is **not** a TODO. There is no local notifications table, no
//! producer for local notifications, and no consumer story that would
//! make populating the list meaningful in single-user mode. If/when
//! local mode grows multi-user semantics, this handler will need to be
//! replaced with a real list backed by a `notifications` table — the
//! envelope shape, however, is already correct and would not change.

use api_types::Notification;
use axum::{Router, extract::Query, response::Json as ResponseJson, routing::get};
use serde::{Deserialize, Serialize};
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::DeploymentImpl;

/// Table-keyed list envelope for `GET /notifications`. The wire field
/// name (`notifications`) matches the snake-case table name the kanban
/// frontend's `extractRows` looks up via `data[table]`. Defined
/// locally rather than in `api-types` because no `ListNotificationsResponse`
/// wire type is shared with the cloud surface — this is a local-only
/// single-user shape contract whose payload is always empty.
#[derive(Debug, Serialize)]
pub(super) struct NotificationsResponse {
    pub notifications: Vec<Notification>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ListNotificationsQuery {
    /// Accepted for cloud-shape compatibility; ignored because the local
    /// surface has no notifications regardless of which user is asking.
    #[allow(dead_code)]
    pub user_id: Uuid,
}

pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new().route("/notifications", get(list_notifications))
}

/// Returns an empty notification list. See module docs — this is a
/// deliberate no-op for single-user local mode, not a stub awaiting
/// implementation.
async fn list_notifications(
    Query(_query): Query<ListNotificationsQuery>,
) -> ResponseJson<ApiResponse<NotificationsResponse>> {
    ResponseJson(ApiResponse::success(NotificationsResponse {
        notifications: Vec::new(),
    }))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn empty_notifications_envelope_shape() {
        let envelope: ApiResponse<NotificationsResponse> =
            ApiResponse::success(NotificationsResponse {
                notifications: Vec::new(),
            });
        let body = serde_json::to_value(&envelope).expect("serialize envelope");
        assert_eq!(
            body,
            json!({
                "success": true,
                "data": { "notifications": [] },
                "error_data": null,
                "message": null,
            }),
            "single-user local mode must return an empty notifications list \
             under the table-keyed envelope; extractRows reads \
             data[\"notifications\"] and the side-panel relies on the \
             ApiResponse envelope rather than 404"
        );
    }
}
