use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct WorkspaceIssueLink {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub issue_id: Uuid,
    pub project_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, TS)]
pub struct CreateWorkspaceIssueLinkRequest {
    /// Optional client-generated ID. If not provided, server generates one.
    /// Using client-generated IDs enables stable optimistic updates.
    #[ts(optional)]
    pub id: Option<Uuid>,
    pub workspace_id: Uuid,
    pub issue_id: Uuid,
    pub project_id: Uuid,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListWorkspaceIssueLinksQuery {
    pub issue_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ListWorkspaceIssueLinksResponse {
    pub workspace_issue_links: Vec<WorkspaceIssueLink>,
}
