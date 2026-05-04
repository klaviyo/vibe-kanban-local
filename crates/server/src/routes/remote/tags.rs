use api_types::{
    CreateTagRequest, DeleteResponse, ListTagsResponse, MutationResponse, Tag, UpdateTagRequest,
};
use axum::{
    Router,
    extract::{Json, Path, Query, State},
    response::Json as ResponseJson,
    routing::get,
};
use db::models::project_tag::ProjectTag;
use deployment::Deployment;
use serde::Deserialize;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

#[derive(Debug, Deserialize)]
pub(super) struct ListTagsQuery {
    pub project_id: Uuid,
}

pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/tags", get(list_tags).post(create_tag))
        .route(
            "/tags/{tag_id}",
            get(get_tag).patch(update_tag).delete(delete_tag),
        )
}

async fn list_tags(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListTagsQuery>,
) -> Result<ResponseJson<ApiResponse<ListTagsResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let rows = ProjectTag::find_by_project(pool, query.project_id).await?;
    let tags: Vec<Tag> = rows.into_iter().map(Tag::from).collect();
    Ok(ResponseJson(ApiResponse::success(ListTagsResponse {
        tags,
    })))
}

async fn get_tag(
    State(deployment): State<DeploymentImpl>,
    Path(tag_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<Tag>>, ApiError> {
    let pool = &deployment.db().pool;
    let row = ProjectTag::find_by_id(pool, tag_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Tag not found".to_string()))?;
    Ok(ResponseJson(ApiResponse::success(Tag::from(row))))
}

async fn create_tag(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<CreateTagRequest>,
) -> Result<ResponseJson<ApiResponse<MutationResponse<Tag>>>, ApiError> {
    let pool = &deployment.db().pool;
    let id = request.id.unwrap_or_else(Uuid::new_v4);
    let response = ProjectTag::create(pool, id, &request).await?;
    Ok(ResponseJson(ApiResponse::success(MutationResponse {
        data: response.data.into(),
        txid: response.txid,
    })))
}

async fn update_tag(
    State(deployment): State<DeploymentImpl>,
    Path(tag_id): Path<Uuid>,
    Json(request): Json<UpdateTagRequest>,
) -> Result<ResponseJson<ApiResponse<MutationResponse<Tag>>>, ApiError> {
    let pool = &deployment.db().pool;
    let response = ProjectTag::update(pool, tag_id, &request).await?;
    Ok(ResponseJson(ApiResponse::success(MutationResponse {
        data: response.data.into(),
        txid: response.txid,
    })))
}

async fn delete_tag(
    State(deployment): State<DeploymentImpl>,
    Path(tag_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<DeleteResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let response = ProjectTag::delete(pool, tag_id).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

#[cfg(test)]
mod tests {
    use api_types::DeleteResponse;
    use serde_json::json;
    use utils::response::ApiResponse;

    /// Sibling routes (issues, issue_followers, issue_assignees, issue_tags,
    /// issue_relationships, issue_comments, issue_comment_reactions) all
    /// surface `DeleteResponse { txid }` on delete so the kanban's
    /// optimistic-update reconciler can match the optimistic write to the
    /// committed mutation. Returning `ApiResponse<()>` would silently drop
    /// the txid and break that reconciler for tag deletes.
    #[test]
    fn delete_envelope_preserves_txid_on_the_wire() {
        let envelope: ApiResponse<DeleteResponse> =
            ApiResponse::success(DeleteResponse { txid: 17 });
        let body = serde_json::to_value(&envelope).expect("serialize envelope");
        assert_eq!(
            body,
            json!({
                "success": true,
                "data": { "txid": 17 },
                "error_data": null,
                "message": null,
            }),
        );
    }
}
