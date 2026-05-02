use api_types::{CreateTagRequest, ListTagsResponse, MutationResponse, Tag, UpdateTagRequest};
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

use crate::{DeploymentImpl, error::ApiError, runtime::synthetic};

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
    let row = ProjectTag::create(pool, id, &request).await?;
    Ok(ResponseJson(ApiResponse::success(MutationResponse {
        data: Tag::from(row),
        txid: synthetic::txid(),
    })))
}

async fn update_tag(
    State(deployment): State<DeploymentImpl>,
    Path(tag_id): Path<Uuid>,
    Json(request): Json<UpdateTagRequest>,
) -> Result<ResponseJson<ApiResponse<MutationResponse<Tag>>>, ApiError> {
    let pool = &deployment.db().pool;
    let row = ProjectTag::update(pool, tag_id, &request).await?;
    Ok(ResponseJson(ApiResponse::success(MutationResponse {
        data: Tag::from(row),
        txid: synthetic::txid(),
    })))
}

async fn delete_tag(
    State(deployment): State<DeploymentImpl>,
    Path(tag_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let pool = &deployment.db().pool;
    ProjectTag::delete(pool, tag_id).await?;
    Ok(ResponseJson(ApiResponse::success(())))
}
