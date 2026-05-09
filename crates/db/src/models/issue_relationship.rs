use api_types::{
    self as wire, DeleteResponse, MutationResponse,
    issue_relationship::CreateIssueRelationshipRequest,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool, Type};
use uuid::Uuid;

use super::mutation_log;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type, Serialize, Deserialize)]
#[sqlx(type_name = "issue_relationship_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum IssueRelationshipType {
    Blocking,
    Related,
    HasDuplicate,
}

impl From<IssueRelationshipType> for wire::IssueRelationshipType {
    fn from(value: IssueRelationshipType) -> Self {
        match value {
            IssueRelationshipType::Blocking => wire::IssueRelationshipType::Blocking,
            IssueRelationshipType::Related => wire::IssueRelationshipType::Related,
            IssueRelationshipType::HasDuplicate => wire::IssueRelationshipType::HasDuplicate,
        }
    }
}

impl From<wire::IssueRelationshipType> for IssueRelationshipType {
    /// Wire→storage. Only the three canonical types map; the inverse
    /// labels (`BlockedBy`, `DuplicateOf`) must be normalized to their
    /// canonical form (with field swap) at the route layer before
    /// reaching storage. Reaching this arm with an inverse label is a
    /// caller bug — panic loudly so it surfaces in tests.
    fn from(value: wire::IssueRelationshipType) -> Self {
        match value {
            wire::IssueRelationshipType::Blocking => IssueRelationshipType::Blocking,
            wire::IssueRelationshipType::Related => IssueRelationshipType::Related,
            wire::IssueRelationshipType::HasDuplicate => IssueRelationshipType::HasDuplicate,
            wire::IssueRelationshipType::BlockedBy | wire::IssueRelationshipType::DuplicateOf => {
                panic!(
                    "inverse-label {value:?} reached storage without normalization — \
                     route layer must swap fields and rewrite to the canonical form first"
                )
            }
        }
    }
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct IssueRelationship {
    pub id: Uuid,
    pub issue_id: Uuid,
    pub related_issue_id: Uuid,
    pub relationship_type: IssueRelationshipType,
    pub created_at: DateTime<Utc>,
}

impl IssueRelationship {
    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            IssueRelationship,
            r#"SELECT id                as "id!: Uuid",
                      issue_id          as "issue_id!: Uuid",
                      related_issue_id  as "related_issue_id!: Uuid",
                      relationship_type as "relationship_type!: IssueRelationshipType",
                      created_at        as "created_at!: DateTime<Utc>"
               FROM issue_relationships
               WHERE id = $1"#,
            id,
        )
        .fetch_optional(pool)
        .await
    }

    /// Per-issue relationship read. Returns every row in which `issue_id`
    /// participates as either the source side (`issue_id`) or the target
    /// side (`related_issue_id`), ordered by creation time. Self-rows are
    /// impossible at the storage layer (CHECK constraint
    /// `issue_id != related_issue_id`), so the OR predicate cannot
    /// double-count a single row. Direction-aware projection happens at
    /// the MCP layer; storage returns the raw row shape unchanged.
    ///
    /// MUST stay in lockstep with the Postgres mirror in
    /// `crates/remote/src/db/issue_relationships.rs::list_by_issue` —
    /// no CI gate enforces parity.
    pub async fn find_by_issue(
        pool: &SqlitePool,
        issue_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            IssueRelationship,
            r#"SELECT id                as "id!: Uuid",
                      issue_id          as "issue_id!: Uuid",
                      related_issue_id  as "related_issue_id!: Uuid",
                      relationship_type as "relationship_type!: IssueRelationshipType",
                      created_at        as "created_at!: DateTime<Utc>"
               FROM issue_relationships
               WHERE issue_id = $1 OR related_issue_id = $1
               ORDER BY created_at ASC"#,
            issue_id,
        )
        .fetch_all(pool)
        .await
    }

    /// Lists relationships originating from any issue in the given project.
    /// Used by the kanban frontend's project-scoped relationship shape (it
    /// pulls relationships for all visible issues at once, rather than
    /// fetching per-issue). Filters on `issue_id`'s project — the
    /// `related_issue_id` may belong to a different project for cross-project
    /// dependencies, but the row is anchored to the source-issue's project.
    /// Mirrors `IssueFollower::find_by_project`.
    pub async fn find_by_project(
        pool: &SqlitePool,
        project_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            IssueRelationship,
            r#"SELECT r.id                as "id!: Uuid",
                      r.issue_id          as "issue_id!: Uuid",
                      r.related_issue_id  as "related_issue_id!: Uuid",
                      r.relationship_type as "relationship_type!: IssueRelationshipType",
                      r.created_at        as "created_at!: DateTime<Utc>"
               FROM issue_relationships r
               INNER JOIN issues i ON i.id = r.issue_id
               WHERE i.project_id = $1
               ORDER BY r.created_at ASC"#,
            project_id,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn create(
        pool: &SqlitePool,
        id: Uuid,
        data: &CreateIssueRelationshipRequest,
    ) -> Result<MutationResponse<Self>, sqlx::Error> {
        let relationship_type = IssueRelationshipType::from(data.relationship_type);
        let mut tx = pool.begin().await?;
        let row = sqlx::query_as!(
            IssueRelationship,
            r#"INSERT INTO issue_relationships (id, issue_id, related_issue_id, relationship_type)
               VALUES ($1, $2, $3, $4)
               RETURNING id                as "id!: Uuid",
                         issue_id          as "issue_id!: Uuid",
                         related_issue_id  as "related_issue_id!: Uuid",
                         relationship_type as "relationship_type!: IssueRelationshipType",
                         created_at        as "created_at!: DateTime<Utc>""#,
            id,
            data.issue_id,
            data.related_issue_id,
            relationship_type,
        )
        .fetch_one(&mut *tx)
        .await?;
        let txid = mutation_log::next_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(MutationResponse { data: row, txid })
    }

    pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<DeleteResponse, sqlx::Error> {
        let mut tx = pool.begin().await?;
        sqlx::query!("DELETE FROM issue_relationships WHERE id = $1", id)
            .execute(&mut *tx)
            .await?;
        let txid = mutation_log::next_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(DeleteResponse { txid })
    }
}

impl From<IssueRelationship> for wire::IssueRelationship {
    fn from(value: IssueRelationship) -> Self {
        Self {
            id: value.id,
            issue_id: value.issue_id,
            related_issue_id: value.related_issue_id,
            relationship_type: value.relationship_type.into(),
            created_at: value.created_at,
        }
    }
}
