use api_types::{
    self as wire, CreateIssueRequest, DeleteResponse, Issue, IssueSortField, ListIssuesQuery,
    ListIssuesResponse, MutationResponse, SearchIssuesRequest, SortDirection, UpdateIssueRequest,
};
use axum::{
    Router,
    extract::{DefaultBodyLimit, Json, Path, Query, State},
    response::Json as ResponseJson,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use db::models::issue::{Issue as IssueRow, IssuePriority};
use deployment::Deployment;
use serde_json::Value;
use sqlx::SqlitePool;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError, runtime::synthetic};

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
    let pool = &deployment.db().pool;
    let issues = IssueRow::find_by_project(pool, query.project_id).await?;
    let total = issues.len();
    let issues: Vec<Issue> = issues.into_iter().map(Issue::from).collect();
    Ok(ResponseJson(ApiResponse::success(ListIssuesResponse {
        issues,
        total_count: total,
        limit: total,
        offset: 0,
    })))
}

async fn search_issues(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<SearchIssuesRequest>,
) -> Result<ResponseJson<ApiResponse<ListIssuesResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let response = run_search(pool, &request).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

async fn get_issue(
    State(deployment): State<DeploymentImpl>,
    Path(issue_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<Issue>>, ApiError> {
    let pool = &deployment.db().pool;
    let issue = IssueRow::find_by_id(pool, issue_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Issue not found".to_string()))?;
    Ok(ResponseJson(ApiResponse::success(Issue::from(issue))))
}

async fn create_issue(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<CreateIssueRequest>,
) -> Result<ResponseJson<ApiResponse<MutationResponse<Issue>>>, ApiError> {
    let pool = &deployment.db().pool;
    let id = request.id.unwrap_or_else(Uuid::new_v4);
    let user = synthetic::local_user(&deployment).await?;

    // Cloud contract: `simple_id` is org-scoped via `organizations.issue_prefix`
    // + `organizations.issue_counter`, atomically incremented in the same
    // transaction as the insert. The model handles the lookup, increment, and
    // insert so a concurrent second project in the same org cannot emit a
    // duplicate `issue_number` or the wrong prefix.
    let response = IssueRow::create_with_org_short_id(pool, id, &request, Some(user.id)).await?;
    Ok(ResponseJson(ApiResponse::success(MutationResponse {
        data: response.data.into(),
        txid: response.txid,
    })))
}

async fn update_issue(
    State(deployment): State<DeploymentImpl>,
    Path(issue_id): Path<Uuid>,
    Json(request): Json<UpdateIssueRequest>,
) -> Result<ResponseJson<ApiResponse<MutationResponse<Issue>>>, ApiError> {
    let pool = &deployment.db().pool;
    let response = IssueRow::update(pool, issue_id, &request).await?;
    Ok(ResponseJson(ApiResponse::success(MutationResponse {
        data: response.data.into(),
        txid: response.txid,
    })))
}

async fn delete_issue(
    State(deployment): State<DeploymentImpl>,
    Path(issue_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<DeleteResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let response = IssueRow::delete(pool, issue_id).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

/// Inline replacement for the cloud's search endpoint. Filters are applied
/// after fetching the project's issue rows; this is acceptable for local mode
/// (single-user, low cardinality) and keeps the implementation free of dynamic
/// SQL builders.
async fn run_search(
    pool: &SqlitePool,
    request: &SearchIssuesRequest,
) -> Result<ListIssuesResponse, sqlx::Error> {
    let mut issues = IssueRow::find_by_project(pool, request.project_id).await?;

    if let Some(status_id) = request.status_id {
        issues.retain(|i| i.status_id == status_id);
    }
    if let Some(status_ids) = &request.status_ids
        && !status_ids.is_empty()
    {
        issues.retain(|i| status_ids.contains(&i.status_id));
    }
    if let Some(priority) = request.priority {
        let p = IssuePriority::from(priority);
        issues.retain(|i| i.priority == Some(p));
    }
    if let Some(parent_issue_id) = request.parent_issue_id {
        issues.retain(|i| i.parent_issue_id == Some(parent_issue_id));
    }
    if let Some(simple_id) = &request.simple_id {
        issues.retain(|i| i.simple_id.eq_ignore_ascii_case(simple_id));
    }
    if let Some(search) = &request.search {
        let needle = search.to_lowercase();
        issues.retain(|i| {
            i.title.to_lowercase().contains(&needle)
                || i.description
                    .as_ref()
                    .map(|d| d.to_lowercase().contains(&needle))
                    .unwrap_or(false)
        });
    }
    if let Some(assignee_user_id) = request.assignee_user_id {
        let mut filtered: Vec<IssueRow> = Vec::with_capacity(issues.len());
        for issue in issues {
            let assignees =
                db::models::issue_assignee::IssueAssignee::find_by_issue(pool, issue.id).await?;
            if assignees.iter().any(|a| a.user_id == assignee_user_id) {
                filtered.push(issue);
            }
        }
        issues = filtered;
    }
    if let Some(tag_id) = request.tag_id {
        let mut filtered: Vec<IssueRow> = Vec::with_capacity(issues.len());
        for issue in issues {
            let tags = db::models::issue_tag::IssueTag::find_by_issue(pool, issue.id).await?;
            if tags.iter().any(|t| t.tag_id == tag_id) {
                filtered.push(issue);
            }
        }
        issues = filtered;
    }
    if let Some(tag_ids) = &request.tag_ids
        && !tag_ids.is_empty()
    {
        let mut filtered: Vec<IssueRow> = Vec::with_capacity(issues.len());
        for issue in issues {
            let tags = db::models::issue_tag::IssueTag::find_by_issue(pool, issue.id).await?;
            if tags.iter().any(|t| tag_ids.contains(&t.tag_id)) {
                filtered.push(issue);
            }
        }
        issues = filtered;
    }

    sort_issues(
        &mut issues,
        request.sort_field.unwrap_or(IssueSortField::SortOrder),
        request.sort_direction.unwrap_or(SortDirection::Asc),
    );

    let total_count = issues.len();
    let offset = request.offset.unwrap_or(0).max(0) as usize;
    let limit = request
        .limit
        .map(|l| l.max(0) as usize)
        .unwrap_or(total_count);

    let page: Vec<Issue> = issues
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(Issue::from)
        .collect();

    Ok(ListIssuesResponse {
        issues: page,
        total_count,
        limit,
        offset,
    })
}

fn sort_issues(issues: &mut [IssueRow], field: IssueSortField, dir: SortDirection) {
    let cmp_priority = |p: Option<IssuePriority>| -> u8 {
        match p {
            Some(IssuePriority::Urgent) => 0,
            Some(IssuePriority::High) => 1,
            Some(IssuePriority::Medium) => 2,
            Some(IssuePriority::Low) => 3,
            None => 4,
        }
    };

    issues.sort_by(|a, b| {
        let ord = match field {
            IssueSortField::SortOrder => a
                .sort_order
                .partial_cmp(&b.sort_order)
                .unwrap_or(std::cmp::Ordering::Equal),
            IssueSortField::Priority => cmp_priority(a.priority).cmp(&cmp_priority(b.priority)),
            IssueSortField::CreatedAt => a.created_at.cmp(&b.created_at),
            IssueSortField::UpdatedAt => a.updated_at.cmp(&b.updated_at),
            IssueSortField::Title => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
        };
        match dir {
            SortDirection::Asc => ord,
            SortDirection::Desc => ord.reverse(),
        }
    });
}

// Suppress unused-import warnings when only some of the wire types are referenced
// through trait conversions.
#[allow(dead_code)]
fn _wire_assert(_: wire::Issue, _: DateTime<Utc>, _: Value) {}

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
