use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// Wire-shape relationship vocabulary.
///
/// Five values for ergonomics on the per-issue read/write endpoints, even
/// though storage is normalized to three (`Blocking`, `Related`,
/// `HasDuplicate`). The two extra values are the inverse-perspective
/// labels the API uses when projecting a row from the queried issue's POV:
///
/// | Stored row direction (issue_id → related_issue_id) | Outbound (queried side is `issue_id`) | Inbound (queried side is `related_issue_id`) |
/// |---|---|---|
/// | `Blocking`     | `Blocking`      | `BlockedBy`    |
/// | `HasDuplicate` | `HasDuplicate`  | `DuplicateOf`  |
/// | `Related`      | `Related`       | `Related`      |
///
/// The per-issue list endpoint (`GET /issue-relationships?issue_id=…`)
/// always swaps fields so the queried issue is in `issue_id` and the
/// other side is in `related_issue_id`, then rewrites the type via the
/// table above. The project-scoped list (`?project_id=…`) returns raw
/// rows since there is no single perspective.
///
/// On create, all five values are accepted. `BlockedBy` and `DuplicateOf`
/// are normalized at the route layer (fields swapped, type rewritten to
/// the canonical stored form) before reaching the storage layer; the DB
/// CHECK constraint only permits the three stored values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "issue_relationship_type", rename_all = "snake_case")
)]
#[serde(rename_all = "snake_case")]
pub enum IssueRelationshipType {
    Blocking,
    BlockedBy,
    Related,
    HasDuplicate,
    DuplicateOf,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct IssueRelationship {
    pub id: Uuid,
    pub issue_id: Uuid,
    pub related_issue_id: Uuid,
    pub relationship_type: IssueRelationshipType,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateIssueRelationshipRequest {
    /// Optional client-generated ID. If not provided, server generates one.
    /// Using client-generated IDs enables stable optimistic updates.
    #[ts(optional)]
    pub id: Option<Uuid>,
    pub issue_id: Uuid,
    pub related_issue_id: Uuid,
    pub relationship_type: IssueRelationshipType,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListIssueRelationshipsQuery {
    pub issue_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ListIssueRelationshipsResponse {
    pub issue_relationships: Vec<IssueRelationship>,
}
