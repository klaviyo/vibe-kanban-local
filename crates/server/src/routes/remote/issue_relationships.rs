use api_types::{
    CreateIssueRelationshipRequest, DeleteResponse, IssueRelationship,
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
    let rows = match (query.issue_id, query.project_id) {
        (Some(issue_id), None) => IssueRelationshipRow::find_by_issue(pool, issue_id).await?,
        (None, Some(project_id)) => IssueRelationshipRow::find_by_project(pool, project_id).await?,
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
    let issue_relationships: Vec<IssueRelationship> =
        rows.into_iter().map(IssueRelationship::from).collect();
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
    use api_types::{IssueRelationship, ListIssueRelationshipsResponse};
    use serde_json::json;
    use utils::response::ApiResponse;

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
}
