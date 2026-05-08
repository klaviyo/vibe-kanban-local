//! Organization-member metadata endpoint — **deliberate no-op** in
//! single-user local mode.
//!
//! In the cloud product, `organization_member_metadata` carries
//! per-(org, user) UI state that has no equivalent in the local
//! single-user product (e.g. invitation acceptance bookkeeping,
//! cross-member presence hints). The kanban frontend resolves this
//! entity through `localRouteResolver` and reads the list via
//! `extractRows`, which looks up `data[<table>]` on the envelope — so
//! this handler returns
//! `ApiResponse<OrganizationMemberMetadataResponse>` where
//! `OrganizationMemberMetadataResponse` exposes a single
//! `organization_member_metadata: Vec<Value>` field, always empty in
//! single-user mode. This matches the table-keyed convention used by
//! the real-CRUD modules in this directory (e.g.
//! `ListIssueFollowersResponse { issue_followers: Vec<…> }`) and
//! returns an empty list rather than 404 (which the side-panel would
//! surface as broken state).
//!
//! This is **not** a TODO. There is no local table, no producer, and
//! no consumer story for member metadata in single-user mode. The
//! row payload is intentionally `serde_json::Value` (rather than a
//! typed wire shape) because no `OrganizationMemberMetadata` wire
//! type exists in `api-types` — the local surface has nothing to
//! describe and the wrapper shape is the only contract that matters
//! to `extractRows`.

use axum::{Router, extract::Query, response::Json as ResponseJson, routing::get};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::DeploymentImpl;

/// Table-keyed list envelope for `GET /organization-member-metadata`.
/// The wire field name (`organization_member_metadata`) matches the
/// snake-case table name the kanban frontend's `extractRows` looks up
/// via `data[table]`. Defined locally rather than in `api-types`
/// because no `ListOrganizationMemberMetadataResponse` wire type is
/// shared with the cloud surface — this is a local-only single-user
/// shape contract whose payload is always empty.
#[derive(Debug, Serialize)]
pub(super) struct OrganizationMemberMetadataResponse {
    pub organization_member_metadata: Vec<Value>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ListOrganizationMemberMetadataQuery {
    /// Accepted for cloud-shape compatibility; ignored because the local
    /// surface has no member metadata regardless of which org is asking.
    #[allow(dead_code)]
    pub organization_id: Uuid,
}

pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new().route(
        "/organization-member-metadata",
        get(list_organization_member_metadata),
    )
}

/// Returns an empty member-metadata list. See module docs — this is a
/// deliberate no-op for single-user local mode, not a stub awaiting
/// implementation.
async fn list_organization_member_metadata(
    Query(_query): Query<ListOrganizationMemberMetadataQuery>,
) -> ResponseJson<ApiResponse<OrganizationMemberMetadataResponse>> {
    ResponseJson(ApiResponse::success(OrganizationMemberMetadataResponse {
        organization_member_metadata: Vec::new(),
    }))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn empty_organization_member_metadata_envelope_shape() {
        let envelope: ApiResponse<OrganizationMemberMetadataResponse> =
            ApiResponse::success(OrganizationMemberMetadataResponse {
                organization_member_metadata: Vec::new(),
            });
        let body = serde_json::to_value(&envelope).expect("serialize envelope");
        assert_eq!(
            body,
            json!({
                "success": true,
                "data": { "organization_member_metadata": [] },
                "error_data": null,
                "message": null,
            }),
            "single-user local mode must return an empty member-metadata list \
             under the table-keyed envelope; extractRows reads \
             data[\"organization_member_metadata\"] and the side-panel relies \
             on the ApiResponse envelope rather than 404"
        );
    }
}
