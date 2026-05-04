use api_types::{
    CreateIssueCommentRequest, DeleteResponse, IssueComment, ListIssueCommentsResponse,
    MutationResponse, UpdateIssueCommentRequest,
};
use axum::{
    Router,
    extract::{Json, Path, Query, State},
    response::Json as ResponseJson,
    routing::get,
};
use db::models::issue_comment::{CreateIssueComment, IssueComment as IssueCommentRow};
use deployment::Deployment;
use serde::Deserialize;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError, runtime::synthetic};

#[derive(Debug, Deserialize)]
pub(super) struct ListIssueCommentsQuery {
    pub issue_id: Uuid,
}

pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route(
            "/issue-comments",
            get(list_issue_comments).post(create_issue_comment),
        )
        .route(
            "/issue-comments/{issue_comment_id}",
            axum::routing::patch(update_issue_comment).delete(delete_issue_comment),
        )
}

async fn list_issue_comments(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListIssueCommentsQuery>,
) -> Result<ResponseJson<ApiResponse<ListIssueCommentsResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let rows = IssueCommentRow::find_by_issue(pool, query.issue_id).await?;
    let issue_comments: Vec<IssueComment> = rows.into_iter().map(IssueComment::from).collect();
    Ok(ResponseJson(ApiResponse::success(
        ListIssueCommentsResponse { issue_comments },
    )))
}

async fn create_issue_comment(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<CreateIssueCommentRequest>,
) -> Result<ResponseJson<ApiResponse<MutationResponse<IssueComment>>>, ApiError> {
    let pool = &deployment.db().pool;
    let id = request.id.unwrap_or_else(Uuid::new_v4);
    // Local mode attributes the comment to the synthetic user so the
    // wire shape carries a valid `author_id`. The cloud contract allows
    // null author_id (system comments); the local product currently
    // never produces those.
    let user = synthetic::local_user(&deployment).await?;
    let response = IssueCommentRow::create(
        pool,
        &CreateIssueComment {
            id,
            author_id: Some(user.id),
            request,
        },
    )
    .await?;
    Ok(ResponseJson(ApiResponse::success(MutationResponse {
        data: response.data.into(),
        txid: response.txid,
    })))
}

async fn update_issue_comment(
    State(deployment): State<DeploymentImpl>,
    Path(issue_comment_id): Path<Uuid>,
    Json(request): Json<UpdateIssueCommentRequest>,
) -> Result<ResponseJson<ApiResponse<MutationResponse<IssueComment>>>, ApiError> {
    let pool = &deployment.db().pool;
    let response = IssueCommentRow::update(pool, issue_comment_id, &request).await?;
    Ok(ResponseJson(ApiResponse::success(MutationResponse {
        data: response.data.into(),
        txid: response.txid,
    })))
}

async fn delete_issue_comment(
    State(deployment): State<DeploymentImpl>,
    Path(issue_comment_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<DeleteResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let response = IssueCommentRow::delete(pool, issue_comment_id).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

#[cfg(test)]
mod tests {
    use api_types::{DeleteResponse, IssueComment, MutationResponse};
    use chrono::{TimeZone, Utc};
    use serde_json::json;
    use utils::response::ApiResponse;
    use uuid::Uuid;

    #[test]
    fn create_envelope_preserves_txid_on_the_wire() {
        let id = Uuid::nil();
        let created_at = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let envelope: ApiResponse<MutationResponse<IssueComment>> =
            ApiResponse::success(MutationResponse {
                data: IssueComment {
                    id,
                    issue_id: id,
                    author_id: Some(id),
                    parent_id: None,
                    message: "hi".to_string(),
                    created_at,
                    updated_at: created_at,
                },
                txid: 9,
            });
        let body = serde_json::to_value(&envelope).expect("serialize envelope");
        assert_eq!(body["data"]["txid"], json!(9));
        assert_eq!(body["data"]["data"]["message"], json!("hi"));
        assert_eq!(body["success"], json!(true));
    }

    #[test]
    fn delete_envelope_preserves_txid_on_the_wire() {
        let envelope: ApiResponse<DeleteResponse> =
            ApiResponse::success(DeleteResponse { txid: 13 });
        let body = serde_json::to_value(&envelope).expect("serialize envelope");
        assert_eq!(
            body,
            json!({
                "success": true,
                "data": { "txid": 13 },
                "error_data": null,
                "message": null,
            }),
        );
    }
}
