//! Organization-member metadata endpoint — **deliberate no-op** in
//! single-user local mode.
//!
//! In the cloud product, `organization_member_metadata` carries
//! per-(org, user) UI state that has no equivalent in the local
//! single-user product (e.g. invitation acceptance bookkeeping,
//! cross-member presence hints). The kanban frontend resolves this
//! entity through `localRouteResolver` and expects an
//! `ApiResponse<Vec<Value>>`-shaped envelope, so we return an empty
//! list rather than 404 (which the side-panel would surface as broken
//! state).
//!
//! This is **not** a TODO. There is no local table, no producer, and
//! no consumer story for member metadata in single-user mode. The
//! response is intentionally `Vec<serde_json::Value>` (rather than a
//! typed wire shape) because no `OrganizationMemberMetadata` wire
//! type exists in `api-types` — the local surface has nothing to
//! describe.

use axum::{
    Router,
    extract::Query,
    response::Json as ResponseJson,
    routing::get,
};
use serde::Deserialize;
use serde_json::Value;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::DeploymentImpl;

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
) -> ResponseJson<ApiResponse<Vec<Value>>> {
    ResponseJson(ApiResponse::success(Vec::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn empty_organization_member_metadata_envelope_shape() {
        let envelope: ApiResponse<Vec<Value>> = ApiResponse::success(Vec::new());
        let body = serde_json::to_value(&envelope).expect("serialize envelope");
        assert_eq!(
            body,
            json!({
                "success": true,
                "data": [],
                "error_data": null,
                "message": null,
            }),
            "single-user local mode must return an empty member-metadata list, \
             not a 404 — the kanban side-panel relies on the ApiResponse envelope"
        );
    }
}
