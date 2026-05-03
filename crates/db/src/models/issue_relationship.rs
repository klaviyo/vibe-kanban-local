use api_types::{self as wire, issue_relationship::CreateIssueRelationshipRequest};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool, Type};
use uuid::Uuid;

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
    fn from(value: wire::IssueRelationshipType) -> Self {
        match value {
            wire::IssueRelationshipType::Blocking => IssueRelationshipType::Blocking,
            wire::IssueRelationshipType::Related => IssueRelationshipType::Related,
            wire::IssueRelationshipType::HasDuplicate => IssueRelationshipType::HasDuplicate,
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
               WHERE issue_id = $1
               ORDER BY created_at ASC"#,
            issue_id,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn create(
        pool: &SqlitePool,
        id: Uuid,
        data: &CreateIssueRelationshipRequest,
    ) -> Result<Self, sqlx::Error> {
        let relationship_type = IssueRelationshipType::from(data.relationship_type);
        sqlx::query_as!(
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
        .fetch_one(pool)
        .await
    }

    pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!("DELETE FROM issue_relationships WHERE id = $1", id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
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
