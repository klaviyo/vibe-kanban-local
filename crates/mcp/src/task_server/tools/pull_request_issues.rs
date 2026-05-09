use api_types::{
    CreatePullRequestIssueRequest, MutationResponse, PullRequestIssue, PullRequestStatus,
};
use db::models::merge::MergeStatus;
use git_host::PullRequestDetail;
use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, schemars, tool,
    tool_router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{McpServer, ToolError};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct McpLinkPrToIssueRequest {
    #[schemars(description = "The issue to attach the pull request to.")]
    issue_id: Uuid,
    #[schemars(
        description = "Full pull request URL (e.g. 'https://github.com/owner/repo/pull/123'). Server fetches PR metadata via `gh` (or the equivalent provider CLI) and persists the link with the canonical state."
    )]
    pr_url: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct McpLinkPrToIssueResponse {
    #[schemars(
        description = "ID of the `pull_request_issues` junction row that records this link. Stable across re-links — calling again with the same (issue, pr_url) pair returns the same id."
    )]
    pull_request_issue_id: String,
    #[schemars(description = "PR number, as fetched from the provider.")]
    pr_number: i32,
    #[schemars(
        description = "PR status at link time: 'open', 'merged', or 'closed'. The pr-monitor service will refresh this on its 60s tick."
    )]
    status: String,
    #[schemars(description = "PR base branch (e.g. 'main'), as fetched from the provider.")]
    target_branch_name: String,
}

fn merge_status_to_pr_status(status: MergeStatus) -> PullRequestStatus {
    match status {
        MergeStatus::Merged => PullRequestStatus::Merged,
        MergeStatus::Closed => PullRequestStatus::Closed,
        MergeStatus::Open | MergeStatus::Unknown => PullRequestStatus::Open,
    }
}

fn pr_status_label(status: &PullRequestStatus) -> &'static str {
    match status {
        PullRequestStatus::Open => "open",
        PullRequestStatus::Merged => "merged",
        PullRequestStatus::Closed => "closed",
    }
}

#[tool_router(router = pull_request_issues_tools_router, vis = "pub")]
impl McpServer {
    #[tool(
        description = "Link a pull request to an issue. Server fetches PR metadata from the provider (`gh pr view` for GitHub) using `pr_url`, then writes the link via the same `/api/remote/pull-request-issues` endpoint the kanban dialog uses. Idempotent on (pull_request_id, issue_id) — re-linking returns the existing junction row."
    )]
    async fn link_pr_to_issue(
        &self,
        Parameters(McpLinkPrToIssueRequest { issue_id, pr_url }): Parameters<
            McpLinkPrToIssueRequest,
        >,
    ) -> Result<CallToolResult, ErrorData> {
        // Fetch PR metadata first. Same endpoint the dialog uses; resolves
        // number / status / target_branch_name from the provider so the
        // caller only needs the URL. `query` handles percent-encoding so
        // arbitrary URL shapes (with `&`, `=`, etc.) round-trip safely.
        let pr_info_url = self.url("/api/repos/pr-info");
        let pr_info: PullRequestDetail = match self
            .send_json(self.client.get(&pr_info_url).query(&[("url", &pr_url)]))
            .await
        {
            Ok(info) => info,
            Err(e) => {
                return Ok(Self::tool_error(ToolError::new(
                    format!("Failed to fetch PR info for {pr_url}"),
                    Some(e.to_string()),
                )));
            }
        };

        let pr_number = i32::try_from(pr_info.number).unwrap_or(i32::MAX);
        let status = merge_status_to_pr_status(pr_info.status);

        let payload = CreatePullRequestIssueRequest {
            id: None,
            issue_id,
            url: pr_info.url.clone(),
            number: pr_number,
            status,
            merged_at: pr_info.merged_at,
            merge_commit_sha: pr_info.merge_commit_sha.clone(),
            target_branch_name: pr_info.base_branch.clone(),
        };

        let link_url = self.url("/api/remote/pull-request-issues");
        let response: MutationResponse<PullRequestIssue> =
            match self.send_json(self.client.post(&link_url).json(&payload)).await {
                Ok(r) => r,
                Err(e) => return Ok(Self::tool_error(e)),
            };

        McpServer::success(&McpLinkPrToIssueResponse {
            pull_request_issue_id: response.data.id.to_string(),
            pr_number,
            status: pr_status_label(&payload.status).to_string(),
            target_branch_name: payload.target_branch_name,
        })
    }
}
