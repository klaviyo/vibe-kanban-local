use api_types::{
    CreateIssueCommentReactionRequest, DeleteResponse, IssueCommentReaction,
    ListIssueCommentReactionsResponse, MutationResponse, UpdateIssueCommentReactionRequest,
};
use axum::{
    Router,
    extract::{Json, Path, Query, State},
    response::Json as ResponseJson,
    routing::get,
};
use db::models::issue_comment_reaction::{
    CreateIssueCommentReaction, IssueCommentReaction as IssueCommentReactionRow,
};
use deployment::Deployment;
use serde::Deserialize;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError, runtime::synthetic};

#[derive(Debug, Deserialize)]
pub(super) struct ListIssueCommentReactionsQuery {
    pub issue_id: Uuid,
}

pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route(
            "/issue-comment-reactions",
            get(list_issue_comment_reactions).post(create_issue_comment_reaction),
        )
        .route(
            "/issue-comment-reactions/{issue_comment_reaction_id}",
            axum::routing::patch(update_issue_comment_reaction)
                .delete(delete_issue_comment_reaction),
        )
}

async fn list_issue_comment_reactions(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListIssueCommentReactionsQuery>,
) -> Result<ResponseJson<ApiResponse<ListIssueCommentReactionsResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    // The kanban frontend lists reactions across every comment on an
    // issue at once (rather than per-comment); see
    // `IssueCommentReaction::find_by_issue` for the JOIN.
    let rows = IssueCommentReactionRow::find_by_issue(pool, query.issue_id).await?;
    let issue_comment_reactions: Vec<IssueCommentReaction> = rows
        .into_iter()
        .map(IssueCommentReaction::from)
        .collect();
    Ok(ResponseJson(ApiResponse::success(
        ListIssueCommentReactionsResponse {
            issue_comment_reactions,
        },
    )))
}

async fn create_issue_comment_reaction(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<CreateIssueCommentReactionRequest>,
) -> Result<ResponseJson<ApiResponse<MutationResponse<IssueCommentReaction>>>, ApiError> {
    let pool = &deployment.db().pool;
    let id = request.id.unwrap_or_else(Uuid::new_v4);
    // Local mode attributes the reaction to the synthetic user.
    let user = synthetic::local_user(&deployment).await?;
    let response = IssueCommentReactionRow::create(
        pool,
        &CreateIssueCommentReaction {
            id,
            user_id: user.id,
            request,
        },
    )
    .await?;
    Ok(ResponseJson(ApiResponse::success(MutationResponse {
        data: response.data.into(),
        txid: response.txid,
    })))
}

async fn update_issue_comment_reaction(
    State(deployment): State<DeploymentImpl>,
    Path(issue_comment_reaction_id): Path<Uuid>,
    Json(request): Json<UpdateIssueCommentReactionRequest>,
) -> Result<ResponseJson<ApiResponse<MutationResponse<IssueCommentReaction>>>, ApiError> {
    let pool = &deployment.db().pool;
    let response =
        IssueCommentReactionRow::update(pool, issue_comment_reaction_id, &request).await?;
    Ok(ResponseJson(ApiResponse::success(MutationResponse {
        data: response.data.into(),
        txid: response.txid,
    })))
}

async fn delete_issue_comment_reaction(
    State(deployment): State<DeploymentImpl>,
    Path(issue_comment_reaction_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<DeleteResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let response = IssueCommentReactionRow::delete(pool, issue_comment_reaction_id).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

#[cfg(test)]
mod tests {
    use api_types::{DeleteResponse, IssueCommentReaction, MutationResponse};
    use chrono::{TimeZone, Utc};
    use serde_json::json;
    use utils::response::ApiResponse;
    use uuid::Uuid;

    #[test]
    fn create_envelope_preserves_txid_on_the_wire() {
        let id = Uuid::nil();
        let created_at = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let envelope: ApiResponse<MutationResponse<IssueCommentReaction>> =
            ApiResponse::success(MutationResponse {
                data: IssueCommentReaction {
                    id,
                    comment_id: id,
                    user_id: id,
                    emoji: "👍".to_string(),
                    created_at,
                },
                txid: 17,
            });
        let body = serde_json::to_value(&envelope).expect("serialize envelope");
        assert_eq!(body["data"]["txid"], json!(17));
        assert_eq!(body["data"]["data"]["emoji"], json!("👍"));
        assert_eq!(body["success"], json!(true));
    }

    #[test]
    fn delete_envelope_preserves_txid_on_the_wire() {
        let envelope: ApiResponse<DeleteResponse> =
            ApiResponse::success(DeleteResponse { txid: 19 });
        let body = serde_json::to_value(&envelope).expect("serialize envelope");
        assert_eq!(
            body,
            json!({
                "success": true,
                "data": { "txid": 19 },
                "error_data": null,
                "message": null,
            }),
        );
    }
}
