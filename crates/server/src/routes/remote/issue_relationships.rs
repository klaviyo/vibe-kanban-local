use api_types::{
    CreateIssueRelationshipRequest, DeleteResponse, IssueRelationship, IssueRelationshipType,
    ListIssueRelationshipsResponse, MutationResponse,
};
use axum::{
    Router,
    extract::{Json, Path, Query, State},
    response::Json as ResponseJson,
    routing::get,
};
use db::models::issue_relationship::IssueRelationship as IssueRelationshipRow;
use deployment::Deployment;
use serde::Deserialize;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

/// Project a stored relationship row from `queried_issue_id`'s point of
/// view. When the queried issue is the row's `issue_id` the row is
/// returned unchanged; when it is the `related_issue_id` side the fields
/// are swapped and asymmetric types are rewritten to their inverse label
/// (`Blocking` → `BlockedBy`, `HasDuplicate` → `DuplicateOf`). `Related`
/// is symmetric and unchanged. The result has the queried issue in
/// `issue_id` and the other side in `related_issue_id`, regardless of
/// which slot they occupy in storage.
fn project_relationship(queried_issue_id: Uuid, row: IssueRelationship) -> IssueRelationship {
    if row.issue_id == queried_issue_id {
        return row;
    }
    let projected_type = match row.relationship_type {
        IssueRelationshipType::Blocking => IssueRelationshipType::BlockedBy,
        IssueRelationshipType::HasDuplicate => IssueRelationshipType::DuplicateOf,
        IssueRelationshipType::Related => IssueRelationshipType::Related,
        // Stored rows are always one of the three canonical types — the
        // DB CHECK constraint enforces that. Reaching this arm means the
        // From<storage>::for wire conversion changed in a way that can
        // emit an inverse label, which is a contract violation.
        IssueRelationshipType::BlockedBy | IssueRelationshipType::DuplicateOf => {
            unreachable!(
                "stored row had inverse-label type {:?}",
                row.relationship_type
            )
        }
    };
    IssueRelationship {
        id: row.id,
        issue_id: row.related_issue_id,
        related_issue_id: row.issue_id,
        relationship_type: projected_type,
        created_at: row.created_at,
    }
}

/// Normalize a create request to the canonical stored form. When the
/// caller asserts the relationship from the inverse side
/// (`relationship_type` is `BlockedBy` or `DuplicateOf`), swap the two
/// issue ids so storage sees the canonical perspective. Returns the
/// normalized request; the type is guaranteed to be one of the three
/// stored variants on return.
fn normalize_create_request(
    mut request: CreateIssueRelationshipRequest,
) -> CreateIssueRelationshipRequest {
    match request.relationship_type {
        IssueRelationshipType::BlockedBy => {
            std::mem::swap(&mut request.issue_id, &mut request.related_issue_id);
            request.relationship_type = IssueRelationshipType::Blocking;
        }
        IssueRelationshipType::DuplicateOf => {
            std::mem::swap(&mut request.issue_id, &mut request.related_issue_id);
            request.relationship_type = IssueRelationshipType::HasDuplicate;
        }
        _ => {}
    }
    request
}

/// Issue-relationships list query — accepts either `issue_id` (per-issue
/// scope) or `project_id` (project-scope, used by the kanban frontend to
/// populate relationships across every visible issue at once). Exactly
/// one must be present; supplying both is rejected with 400. Defined
/// locally rather than reusing `api_types::ListIssueRelationshipsQuery`
/// because the wire type is a single-required-field shape that doesn't
/// model the project-scoped variant.
#[derive(Debug, Deserialize)]
pub(super) struct ListIssueRelationshipsQuery {
    #[serde(default)]
    pub issue_id: Option<Uuid>,
    #[serde(default)]
    pub project_id: Option<Uuid>,
}

pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route(
            "/issue-relationships",
            get(list_issue_relationships).post(create_issue_relationship),
        )
        .route(
            "/issue-relationships/{relationship_id}",
            axum::routing::delete(delete_issue_relationship),
        )
}

async fn list_issue_relationships(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListIssueRelationshipsQuery>,
) -> Result<ResponseJson<ApiResponse<ListIssueRelationshipsResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    // Per-issue queries project rows from the queried issue's POV (queried
    // side always in `issue_id`, asymmetric types rewritten to their
    // inverse label when the queried issue is the inbound side). Project-
    // scoped queries return raw rows since there is no single perspective.
    let issue_relationships: Vec<IssueRelationship> = match (query.issue_id, query.project_id) {
        (Some(issue_id), None) => IssueRelationshipRow::find_by_issue(pool, issue_id)
            .await?
            .into_iter()
            .map(IssueRelationship::from)
            .map(|row| project_relationship(issue_id, row))
            .collect(),
        (None, Some(project_id)) => IssueRelationshipRow::find_by_project(pool, project_id)
            .await?
            .into_iter()
            .map(IssueRelationship::from)
            .collect(),
        (Some(_), Some(_)) => {
            return Err(ApiError::BadRequest(
                "issue_id and project_id are mutually exclusive".to_string(),
            ));
        }
        (None, None) => {
            return Err(ApiError::BadRequest(
                "issue_id or project_id is required".to_string(),
            ));
        }
    };
    Ok(ResponseJson(ApiResponse::success(
        ListIssueRelationshipsResponse {
            issue_relationships,
        },
    )))
}

async fn create_issue_relationship(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<CreateIssueRelationshipRequest>,
) -> Result<ResponseJson<ApiResponse<MutationResponse<IssueRelationship>>>, ApiError> {
    let pool = &deployment.db().pool;
    // Accept inverse labels (`BlockedBy`, `DuplicateOf`) as a write-side
    // ergonomic — the dialog can post a row from the blockee/duplicate's
    // POV without first inverting it. Storage stays canonical.
    let request = normalize_create_request(request);
    let id = request.id.unwrap_or_else(Uuid::new_v4);
    let response = IssueRelationshipRow::create(pool, id, &request).await?;
    Ok(ResponseJson(ApiResponse::success(MutationResponse {
        data: response.data.into(),
        txid: response.txid,
    })))
}

async fn delete_issue_relationship(
    State(deployment): State<DeploymentImpl>,
    Path(relationship_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<DeleteResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let response = IssueRelationshipRow::delete(pool, relationship_id).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

#[cfg(test)]
mod tests {
    use api_types::{
        CreateIssueRelationshipRequest, IssueRelationship, IssueRelationshipType,
        ListIssueRelationshipsResponse,
    };
    use chrono::Utc;
    use serde_json::json;
    use utils::response::ApiResponse;
    use uuid::Uuid;

    use super::{normalize_create_request, project_relationship};

    #[test]
    fn list_envelope_is_table_keyed() {
        let envelope: ApiResponse<ListIssueRelationshipsResponse> =
            ApiResponse::success(ListIssueRelationshipsResponse {
                issue_relationships: Vec::<IssueRelationship>::new(),
            });
        let body = serde_json::to_value(&envelope).expect("serialize envelope");
        assert_eq!(
            body,
            json!({
                "success": true,
                "data": { "issue_relationships": [] },
                "error_data": null,
                "message": null,
            }),
        );
    }

    fn raw_row(
        issue_id: Uuid,
        related_issue_id: Uuid,
        relationship_type: IssueRelationshipType,
    ) -> IssueRelationship {
        IssueRelationship {
            id: Uuid::new_v4(),
            issue_id,
            related_issue_id,
            relationship_type,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn project_outbound_row_returns_unchanged() {
        let queried = Uuid::new_v4();
        let other = Uuid::new_v4();
        let row = raw_row(queried, other, IssueRelationshipType::Blocking);
        let projected = project_relationship(queried, row.clone());
        assert_eq!(projected.issue_id, queried);
        assert_eq!(projected.related_issue_id, other);
        assert!(matches!(
            projected.relationship_type,
            IssueRelationshipType::Blocking
        ));
    }

    #[test]
    fn project_inbound_blocking_becomes_blocked_by() {
        let queried = Uuid::new_v4();
        let blocker = Uuid::new_v4();
        // Stored as `(blocker -> queried, blocking)`. Queried's POV is
        // "I am blocked by `blocker`", so the row must invert.
        let row = raw_row(blocker, queried, IssueRelationshipType::Blocking);
        let projected = project_relationship(queried, row);
        assert_eq!(projected.issue_id, queried);
        assert_eq!(projected.related_issue_id, blocker);
        assert!(matches!(
            projected.relationship_type,
            IssueRelationshipType::BlockedBy
        ));
    }

    #[test]
    fn project_inbound_has_duplicate_becomes_duplicate_of() {
        let queried = Uuid::new_v4();
        let canonical = Uuid::new_v4();
        let row = raw_row(canonical, queried, IssueRelationshipType::HasDuplicate);
        let projected = project_relationship(queried, row);
        assert_eq!(projected.issue_id, queried);
        assert_eq!(projected.related_issue_id, canonical);
        assert!(matches!(
            projected.relationship_type,
            IssueRelationshipType::DuplicateOf
        ));
    }

    #[test]
    fn project_inbound_related_stays_related() {
        let queried = Uuid::new_v4();
        let other = Uuid::new_v4();
        let row = raw_row(other, queried, IssueRelationshipType::Related);
        let projected = project_relationship(queried, row);
        assert_eq!(projected.issue_id, queried);
        assert_eq!(projected.related_issue_id, other);
        assert!(matches!(
            projected.relationship_type,
            IssueRelationshipType::Related
        ));
    }

    #[test]
    fn normalize_blocked_by_swaps_and_rewrites_to_blocking() {
        let blockee = Uuid::new_v4();
        let blocker = Uuid::new_v4();
        let request = CreateIssueRelationshipRequest {
            id: None,
            issue_id: blockee,
            related_issue_id: blocker,
            relationship_type: IssueRelationshipType::BlockedBy,
        };
        let normalized = normalize_create_request(request);
        assert_eq!(normalized.issue_id, blocker);
        assert_eq!(normalized.related_issue_id, blockee);
        assert!(matches!(
            normalized.relationship_type,
            IssueRelationshipType::Blocking
        ));
    }

    #[test]
    fn normalize_duplicate_of_swaps_and_rewrites_to_has_duplicate() {
        let duplicate = Uuid::new_v4();
        let canonical = Uuid::new_v4();
        let request = CreateIssueRelationshipRequest {
            id: None,
            issue_id: duplicate,
            related_issue_id: canonical,
            relationship_type: IssueRelationshipType::DuplicateOf,
        };
        let normalized = normalize_create_request(request);
        assert_eq!(normalized.issue_id, canonical);
        assert_eq!(normalized.related_issue_id, duplicate);
        assert!(matches!(
            normalized.relationship_type,
            IssueRelationshipType::HasDuplicate
        ));
    }

    #[test]
    fn normalize_canonical_types_pass_through_unchanged() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        for ty in [
            IssueRelationshipType::Blocking,
            IssueRelationshipType::Related,
            IssueRelationshipType::HasDuplicate,
        ] {
            let request = CreateIssueRelationshipRequest {
                id: None,
                issue_id: a,
                related_issue_id: b,
                relationship_type: ty,
            };
            let normalized = normalize_create_request(request);
            assert_eq!(normalized.issue_id, a);
            assert_eq!(normalized.related_issue_id, b);
            assert!(
                matches!(normalized.relationship_type, t if std::mem::discriminant(&t) == std::mem::discriminant(&ty))
            );
        }
    }
}
