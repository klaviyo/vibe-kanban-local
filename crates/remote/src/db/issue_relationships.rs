use api_types::{DeleteResponse, IssueRelationship, IssueRelationshipType, MutationResponse};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

use super::get_txid;

#[derive(Debug, Error)]
pub enum IssueRelationshipError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub struct IssueRelationshipRepository;

impl IssueRelationshipRepository {
    pub async fn find_by_id(
        pool: &PgPool,
        id: Uuid,
    ) -> Result<Option<IssueRelationship>, IssueRelationshipError> {
        let record = sqlx::query_as!(
            IssueRelationship,
            r#"
            SELECT
                id                AS "id!: Uuid",
                issue_id          AS "issue_id!: Uuid",
                related_issue_id  AS "related_issue_id!: Uuid",
                relationship_type AS "relationship_type!: IssueRelationshipType",
                created_at        AS "created_at!: DateTime<Utc>"
            FROM issue_relationships
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await?;

        Ok(record)
    }

    /// Per-issue relationship read. Returns every row in which `issue_id`
    /// participates as either the source side (`issue_id`) or the target
    /// side (`related_issue_id`), ordered by creation time. Self-rows are
    /// impossible at the storage layer (the migration's CHECK constraint
    /// rejects `issue_id = related_issue_id`), so the OR predicate cannot
    /// double-count a single row. Direction-aware projection happens at
    /// the MCP layer; storage returns the raw row shape unchanged.
    ///
    /// MUST stay in lockstep with the SQLite mirror in
    /// `crates/db/src/models/issue_relationship.rs::find_by_issue` —
    /// no CI gate enforces parity.
    pub async fn list_by_issue(
        pool: &PgPool,
        issue_id: Uuid,
    ) -> Result<Vec<IssueRelationship>, IssueRelationshipError> {
        let records = sqlx::query_as!(
            IssueRelationship,
            r#"
            SELECT
                id                AS "id!: Uuid",
                issue_id          AS "issue_id!: Uuid",
                related_issue_id  AS "related_issue_id!: Uuid",
                relationship_type AS "relationship_type!: IssueRelationshipType",
                created_at        AS "created_at!: DateTime<Utc>"
            FROM issue_relationships
            WHERE issue_id = $1 OR related_issue_id = $1
            ORDER BY created_at ASC
            "#,
            issue_id
        )
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    pub async fn list_by_project(
        pool: &PgPool,
        project_id: Uuid,
    ) -> Result<Vec<IssueRelationship>, IssueRelationshipError> {
        let records = sqlx::query_as!(
            IssueRelationship,
            r#"
            SELECT
                id                AS "id!: Uuid",
                issue_id          AS "issue_id!: Uuid",
                related_issue_id  AS "related_issue_id!: Uuid",
                relationship_type AS "relationship_type!: IssueRelationshipType",
                created_at        AS "created_at!: DateTime<Utc>"
            FROM issue_relationships
            WHERE issue_id IN (SELECT id FROM issues WHERE project_id = $1)
            "#,
            project_id
        )
        .fetch_all(pool)
        .await?;
        Ok(records)
    }

    pub async fn create(
        pool: &PgPool,
        id: Option<Uuid>,
        issue_id: Uuid,
        related_issue_id: Uuid,
        relationship_type: IssueRelationshipType,
    ) -> Result<MutationResponse<IssueRelationship>, IssueRelationshipError> {
        let id = id.unwrap_or_else(Uuid::new_v4);
        let mut tx = super::begin_tx(pool).await?;
        let data = sqlx::query_as!(
            IssueRelationship,
            r#"
            INSERT INTO issue_relationships (id, issue_id, related_issue_id, relationship_type)
            VALUES ($1, $2, $3, $4)
            RETURNING
                id                AS "id!: Uuid",
                issue_id          AS "issue_id!: Uuid",
                related_issue_id  AS "related_issue_id!: Uuid",
                relationship_type AS "relationship_type!: IssueRelationshipType",
                created_at        AS "created_at!: DateTime<Utc>"
            "#,
            id,
            issue_id,
            related_issue_id,
            relationship_type as IssueRelationshipType
        )
        .fetch_one(&mut *tx)
        .await?;
        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(MutationResponse { data, txid })
    }

    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<DeleteResponse, IssueRelationshipError> {
        let mut tx = super::begin_tx(pool).await?;
        sqlx::query!("DELETE FROM issue_relationships WHERE id = $1", id)
            .execute(&mut *tx)
            .await?;
        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(DeleteResponse { txid })
    }
}

#[cfg(test)]
mod tests {
    /// Source-level parity check for the per-issue read SQL between the
    /// Postgres mirror in this file and the SQLite primary in
    /// `crates/db/src/models/issue_relationship.rs`.
    ///
    /// No live-Postgres test harness exists in this crate (offline `.sqlx`
    /// metadata is the only compile-time verification), so the four
    /// inbound/outbound/mixed/zero behavioural cases are covered on the
    /// SQLite side. This unit test guards the Postgres mirror by asserting
    /// the bidirectional predicate is present in source — sufficient to
    /// catch a regression that drifted one layer back to the original
    /// `WHERE issue_id = $1` shape.
    #[test]
    fn list_by_issue_uses_bidirectional_predicate() {
        let source = include_str!("issue_relationships.rs");
        // Locate the `pub async fn list_by_issue` body and assert the
        // OR-joined predicate plus creation-time ordering are present
        // inside it. Anchored to this function so unrelated SQL elsewhere
        // can't accidentally satisfy the assertion.
        let function_start = source
            .find("pub async fn list_by_issue")
            .expect("list_by_issue function should exist");
        let function_end = source[function_start..]
            .find("pub async fn ")
            .map(|offset| function_start + offset + "pub async fn ".len())
            .and_then(|after_first| {
                source[after_first..]
                    .find("pub async fn ")
                    .map(|offset| after_first + offset)
            })
            .unwrap_or(source.len());
        let body = &source[function_start..function_end];
        assert!(
            body.contains("WHERE issue_id = $1 OR related_issue_id = $1"),
            "Postgres list_by_issue must use the bidirectional predicate; \
             SQL drifted out of parity with the SQLite mirror"
        );
        assert!(
            body.contains("ORDER BY created_at ASC"),
            "Postgres list_by_issue must order by creation time"
        );
    }
}
