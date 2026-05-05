use api_types::{
    CreateIssueRequest, Issue, ListIssuesQuery, ListIssuesResponse, MutationResponse,
    SearchIssuesRequest, UpdateIssueRequest,
};
use axum::{
    Router,
    extract::{DefaultBodyLimit, Json, Path, Query, State},
    response::Json as ResponseJson,
    routing::{get, post},
};
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

/// Maximum request body size accepted by `/api/remote/issues/*` PATCH/POST handlers.
///
/// The daemon writes pipeline state into issue descriptions; runaway pipelines can
/// produce large bodies. 2 MiB is the documented headroom — the daemon must produce
/// payloads within this cap, and the route rejects anything larger with HTTP 413.
pub const ISSUE_BODY_LIMIT_BYTES: usize = 2 * 1024 * 1024;

pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/issues", get(list_issues).post(create_issue))
        .route("/issues/search", post(search_issues))
        .route(
            "/issues/{issue_id}",
            get(get_issue).patch(update_issue).delete(delete_issue),
        )
        .layer(DefaultBodyLimit::max(ISSUE_BODY_LIMIT_BYTES))
}

async fn list_issues(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListIssuesQuery>,
) -> Result<ResponseJson<ApiResponse<ListIssuesResponse>>, ApiError> {
    let client = deployment.remote_client()?;
    let response = client.list_issues(query.project_id).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

async fn search_issues(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<SearchIssuesRequest>,
) -> Result<ResponseJson<ApiResponse<ListIssuesResponse>>, ApiError> {
    let client = deployment.remote_client()?;
    let response = client.search_issues(&request).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

async fn get_issue(
    State(deployment): State<DeploymentImpl>,
    Path(issue_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<Issue>>, ApiError> {
    let client = deployment.remote_client()?;
    let response = client.get_issue(issue_id).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

async fn create_issue(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<CreateIssueRequest>,
) -> Result<ResponseJson<ApiResponse<MutationResponse<Issue>>>, ApiError> {
    let client = deployment.remote_client()?;
    let response = client.create_issue(&request).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

async fn update_issue(
    State(deployment): State<DeploymentImpl>,
    Path(issue_id): Path<Uuid>,
    Json(request): Json<UpdateIssueRequest>,
) -> Result<ResponseJson<ApiResponse<MutationResponse<Issue>>>, ApiError> {
    let client = deployment.remote_client()?;
    let response = client.update_issue(issue_id, &request).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

async fn delete_issue(
    State(deployment): State<DeploymentImpl>,
    Path(issue_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let client = deployment.remote_client()?;
    client.delete_issue(issue_id).await?;
    Ok(ResponseJson(ApiResponse::success(())))
}

#[cfg(test)]
mod tests {
    use axum::{
        Router,
        body::Body,
        extract::{DefaultBodyLimit, Json},
        http::{Request, StatusCode},
        routing::{patch, post},
    };
    use tower::ServiceExt;

    use super::ISSUE_BODY_LIMIT_BYTES;

    #[test]
    fn issue_body_limit_is_two_mib() {
        assert_eq!(ISSUE_BODY_LIMIT_BYTES, 2 * 1024 * 1024);
    }

    /// Mirrors the body-limit layer applied to `/api/remote/issues/*` so that we can
    /// exercise the cap without standing up the full deployment-backed router. The
    /// production router applies the same `DefaultBodyLimit::max(ISSUE_BODY_LIMIT_BYTES)`.
    fn body_limited_router() -> Router {
        async fn echo_ok(Json(value): Json<serde_json::Value>) -> Json<serde_json::Value> {
            Json(value)
        }
        Router::new()
            .route("/issues", post(echo_ok))
            .route("/issues/{issue_id}", patch(echo_ok))
            .layer(DefaultBodyLimit::max(ISSUE_BODY_LIMIT_BYTES))
    }

    /// Builds a JSON body of exactly `len` bytes by padding the `description` string.
    fn json_body_of_size(len: usize) -> Vec<u8> {
        let prefix = br#"{"description":""#;
        let suffix = br#""}"#;
        assert!(
            len > prefix.len() + suffix.len(),
            "len must exceed JSON envelope"
        );
        let mut buf = Vec::with_capacity(len);
        buf.extend_from_slice(prefix);
        buf.resize(len - suffix.len(), b'a');
        buf.extend_from_slice(suffix);
        debug_assert_eq!(buf.len(), len);
        buf
    }

    fn json_request(method: &str, path: &str, body: Vec<u8>) -> Request<Body> {
        let len = body.len();
        Request::builder()
            .method(method)
            .uri(path)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .header(axum::http::header::CONTENT_LENGTH, len)
            .body(Body::from(body))
            .unwrap()
    }

    #[tokio::test]
    async fn create_at_cap_is_accepted() {
        let app = body_limited_router();
        let body = json_body_of_size(ISSUE_BODY_LIMIT_BYTES);
        let response = app
            .oneshot(json_request("POST", "/issues", body))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn create_above_cap_returns_413() {
        let app = body_limited_router();
        let body = json_body_of_size(ISSUE_BODY_LIMIT_BYTES + 1);
        let response = app
            .oneshot(json_request("POST", "/issues", body))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn update_at_cap_is_accepted() {
        let app = body_limited_router();
        let body = json_body_of_size(ISSUE_BODY_LIMIT_BYTES);
        let response = app
            .oneshot(json_request(
                "PATCH",
                "/issues/00000000-0000-0000-0000-000000000000",
                body,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn update_above_cap_returns_413() {
        let app = body_limited_router();
        let body = json_body_of_size(ISSUE_BODY_LIMIT_BYTES + 1);
        let response = app
            .oneshot(json_request(
                "PATCH",
                "/issues/00000000-0000-0000-0000-000000000000",
                body,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn round_trip_within_cap_preserves_body() {
        // Round-trip integrity: the echo handler returns the exact JSON it received,
        // proving the bytes reached the handler intact (within the 2 MiB cap).
        let app = body_limited_router();
        let sizes = [
            1024,
            64 * 1024,
            512 * 1024,
            ISSUE_BODY_LIMIT_BYTES - 1024,
            ISSUE_BODY_LIMIT_BYTES,
        ];
        for size in sizes {
            let body = json_body_of_size(size);
            let response = app
                .clone()
                .oneshot(json_request("POST", "/issues", body.clone()))
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "size {size} should pass the cap"
            );
            let response_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            assert_eq!(
                response_bytes.as_ref(),
                body.as_slice(),
                "size {size} body must round-trip without modification"
            );
        }
    }
}
