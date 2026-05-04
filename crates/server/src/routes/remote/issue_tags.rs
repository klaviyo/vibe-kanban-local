use api_types::{
    CreateIssueTagRequest, DeleteResponse, IssueTag, ListIssueTagsResponse, MutationResponse,
};
use axum::{
    Router,
    extract::{Json, Path, Query, State},
    response::Json as ResponseJson,
    routing::get,
};
use db::models::issue_tag::IssueTag as IssueTagRow;
use deployment::Deployment;
use serde::Deserialize;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

/// Issue-tags list query — accepts either `issue_id` (per-issue scope)
/// or `project_id` (project-scope, used by the kanban frontend to
/// populate tag-links across every visible issue at once). Exactly one
/// must be present; supplying both is rejected with 400. Mirrors
/// `issue_followers::ListIssueFollowersQuery`.
#[derive(Debug, Deserialize)]
pub(super) struct ListIssueTagsQuery {
    #[serde(default)]
    pub issue_id: Option<Uuid>,
    #[serde(default)]
    pub project_id: Option<Uuid>,
}

pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/issue-tags", get(list_issue_tags).post(create_issue_tag))
        .route(
            "/issue-tags/{issue_tag_id}",
            get(get_issue_tag).delete(delete_issue_tag),
        )
}

async fn list_issue_tags(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListIssueTagsQuery>,
) -> Result<ResponseJson<ApiResponse<ListIssueTagsResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let rows = match (query.issue_id, query.project_id) {
        (Some(issue_id), None) => IssueTagRow::find_by_issue(pool, issue_id).await?,
        (None, Some(project_id)) => IssueTagRow::find_by_project(pool, project_id).await?,
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
    let issue_tags: Vec<IssueTag> = rows.into_iter().map(IssueTag::from).collect();
    Ok(ResponseJson(ApiResponse::success(ListIssueTagsResponse {
        issue_tags,
    })))
}

async fn get_issue_tag(
    State(deployment): State<DeploymentImpl>,
    Path(issue_tag_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<IssueTag>>, ApiError> {
    let pool = &deployment.db().pool;
    let row = IssueTagRow::find_by_id(pool, issue_tag_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Issue tag not found".to_string()))?;
    Ok(ResponseJson(ApiResponse::success(IssueTag::from(row))))
}

async fn create_issue_tag(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<CreateIssueTagRequest>,
) -> Result<ResponseJson<ApiResponse<MutationResponse<IssueTag>>>, ApiError> {
    let pool = &deployment.db().pool;
    let id = request.id.unwrap_or_else(Uuid::new_v4);
    let response = IssueTagRow::create(pool, id, &request).await?;
    Ok(ResponseJson(ApiResponse::success(MutationResponse {
        data: response.data.into(),
        txid: response.txid,
    })))
}

async fn delete_issue_tag(
    State(deployment): State<DeploymentImpl>,
    Path(issue_tag_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<DeleteResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let response = IssueTagRow::delete(pool, issue_tag_id).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

#[cfg(test)]
mod tests {
    use api_types::{IssueTag, ListIssueTagsResponse};
    use serde_json::json;
    use utils::response::ApiResponse;

    #[test]
    fn list_envelope_is_table_keyed() {
        let envelope: ApiResponse<ListIssueTagsResponse> =
            ApiResponse::success(ListIssueTagsResponse {
                issue_tags: Vec::<IssueTag>::new(),
            });
        let body = serde_json::to_value(&envelope).expect("serialize envelope");
        assert_eq!(
            body,
            json!({
                "success": true,
                "data": { "issue_tags": [] },
                "error_data": null,
                "message": null,
            }),
        );
    }
}
