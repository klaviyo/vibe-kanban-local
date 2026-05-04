use api_types::{
    CreateIssueRelationshipRequest, DeleteResponse, IssueRelationship, ListIssueRelationshipsQuery,
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
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

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
    let rows = IssueRelationshipRow::find_by_issue(pool, query.issue_id).await?;
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
