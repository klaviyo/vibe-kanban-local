use api_types::{
    self as wire,
    issue::{CreateIssueRequest, UpdateIssueRequest},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, SqlitePool, Type};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type, Serialize, Deserialize)]
#[sqlx(type_name = "issue_priority", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum IssuePriority {
    Urgent,
    High,
    Medium,
    Low,
}

impl From<IssuePriority> for wire::IssuePriority {
    fn from(value: IssuePriority) -> Self {
        match value {
            IssuePriority::Urgent => wire::IssuePriority::Urgent,
            IssuePriority::High => wire::IssuePriority::High,
            IssuePriority::Medium => wire::IssuePriority::Medium,
            IssuePriority::Low => wire::IssuePriority::Low,
        }
    }
}

impl From<wire::IssuePriority> for IssuePriority {
    fn from(value: wire::IssuePriority) -> Self {
        match value {
            wire::IssuePriority::Urgent => IssuePriority::Urgent,
            wire::IssuePriority::High => IssuePriority::High,
            wire::IssuePriority::Medium => IssuePriority::Medium,
            wire::IssuePriority::Low => IssuePriority::Low,
        }
    }
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Issue {
    pub id: Uuid,
    pub project_id: Uuid,
    pub issue_number: i64,
    pub simple_id: String,
    pub status_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<IssuePriority>,
    pub start_date: Option<DateTime<Utc>>,
    pub target_date: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub sort_order: f64,
    pub parent_issue_id: Option<Uuid>,
    pub parent_issue_sort_order: Option<f64>,
    pub creator_user_id: Option<Uuid>,
    pub extension_metadata: sqlx::types::Json<Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateIssue {
    pub id: Uuid,
    pub creator_user_id: Option<Uuid>,
    pub request: CreateIssueRequest,
}

impl Issue {
    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Issue,
            r#"SELECT id                       as "id!: Uuid",
                      project_id               as "project_id!: Uuid",
                      issue_number,
                      simple_id,
                      status_id                as "status_id!: Uuid",
                      title,
                      description,
                      priority                 as "priority: IssuePriority",
                      start_date               as "start_date: DateTime<Utc>",
                      target_date              as "target_date: DateTime<Utc>",
                      completed_at             as "completed_at: DateTime<Utc>",
                      sort_order               as "sort_order!: f64",
                      parent_issue_id          as "parent_issue_id: Uuid",
                      parent_issue_sort_order  as "parent_issue_sort_order: f64",
                      creator_user_id          as "creator_user_id: Uuid",
                      extension_metadata       as "extension_metadata!: sqlx::types::Json<Value>",
                      created_at               as "created_at!: DateTime<Utc>",
                      updated_at               as "updated_at!: DateTime<Utc>"
               FROM issues
               WHERE id = $1"#,
            id,
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_simple_id(
        pool: &SqlitePool,
        project_id: Uuid,
        simple_id: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Issue,
            r#"SELECT id                       as "id!: Uuid",
                      project_id               as "project_id!: Uuid",
                      issue_number,
                      simple_id,
                      status_id                as "status_id!: Uuid",
                      title,
                      description,
                      priority                 as "priority: IssuePriority",
                      start_date               as "start_date: DateTime<Utc>",
                      target_date              as "target_date: DateTime<Utc>",
                      completed_at             as "completed_at: DateTime<Utc>",
                      sort_order               as "sort_order!: f64",
                      parent_issue_id          as "parent_issue_id: Uuid",
                      parent_issue_sort_order  as "parent_issue_sort_order: f64",
                      creator_user_id          as "creator_user_id: Uuid",
                      extension_metadata       as "extension_metadata!: sqlx::types::Json<Value>",
                      created_at               as "created_at!: DateTime<Utc>",
                      updated_at               as "updated_at!: DateTime<Utc>"
               FROM issues
               WHERE project_id = $1 AND simple_id = $2"#,
            project_id,
            simple_id,
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_project(
        pool: &SqlitePool,
        project_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Issue,
            r#"SELECT id                       as "id!: Uuid",
                      project_id               as "project_id!: Uuid",
                      issue_number,
                      simple_id,
                      status_id                as "status_id!: Uuid",
                      title,
                      description,
                      priority                 as "priority: IssuePriority",
                      start_date               as "start_date: DateTime<Utc>",
                      target_date              as "target_date: DateTime<Utc>",
                      completed_at             as "completed_at: DateTime<Utc>",
                      sort_order               as "sort_order!: f64",
                      parent_issue_id          as "parent_issue_id: Uuid",
                      parent_issue_sort_order  as "parent_issue_sort_order: f64",
                      creator_user_id          as "creator_user_id: Uuid",
                      extension_metadata       as "extension_metadata!: sqlx::types::Json<Value>",
                      created_at               as "created_at!: DateTime<Utc>",
                      updated_at               as "updated_at!: DateTime<Utc>"
               FROM issues
               WHERE project_id = $1
               ORDER BY sort_order ASC, created_at ASC"#,
            project_id,
        )
        .fetch_all(pool)
        .await
    }

    /// Atomically allocates the next per-organization issue identifier and
    /// inserts the issue in a single `BEGIN IMMEDIATE` transaction. The
    /// counter bump and the insert succeed or roll back together, so a failed
    /// insert leaves `organizations.issue_counter` at its pre-call value.
    ///
    /// `BEGIN IMMEDIATE` is required: sqlx's default deferred transactions
    /// upgrade to a writer lazily, which is a TOCTOU window between the
    /// counter read and the insert.
    pub async fn create(pool: &SqlitePool, data: &CreateIssue) -> Result<Self, sqlx::Error> {
        let req = &data.request;
        let priority: Option<IssuePriority> = req.priority.map(IssuePriority::from);
        let extension_metadata = sqlx::types::Json(req.extension_metadata.clone());

        let mut conn = pool.acquire().await?;
        sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await?;

        let result: Result<Issue, sqlx::Error> = async {
            let allocated = sqlx::query!(
                r#"UPDATE organizations
                   SET issue_counter = issue_counter + 1,
                       updated_at    = datetime('now', 'subsec')
                   WHERE id = (SELECT organization_id FROM projects WHERE id = $1)
                   RETURNING issue_counter as "issue_counter!: i64",
                             issue_prefix"#,
                req.project_id,
            )
            .fetch_one(&mut *conn)
            .await?;

            let issue_number = allocated.issue_counter;
            let simple_id = format!("{}-{}", allocated.issue_prefix, issue_number);

            sqlx::query_as!(
                Issue,
                r#"INSERT INTO issues (
                       id, project_id, issue_number, simple_id, status_id, title,
                       description, priority, start_date, target_date, completed_at,
                       sort_order, parent_issue_id, parent_issue_sort_order,
                       creator_user_id, extension_metadata
                   )
                   VALUES (
                       $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
                       $12, $13, $14, $15, $16
                   )
                   RETURNING id                       as "id!: Uuid",
                             project_id               as "project_id!: Uuid",
                             issue_number,
                             simple_id,
                             status_id                as "status_id!: Uuid",
                             title,
                             description,
                             priority                 as "priority: IssuePriority",
                             start_date               as "start_date: DateTime<Utc>",
                             target_date              as "target_date: DateTime<Utc>",
                             completed_at             as "completed_at: DateTime<Utc>",
                             sort_order               as "sort_order!: f64",
                             parent_issue_id          as "parent_issue_id: Uuid",
                             parent_issue_sort_order  as "parent_issue_sort_order: f64",
                             creator_user_id          as "creator_user_id: Uuid",
                             extension_metadata       as "extension_metadata!: sqlx::types::Json<Value>",
                             created_at               as "created_at!: DateTime<Utc>",
                             updated_at               as "updated_at!: DateTime<Utc>""#,
                data.id,
                req.project_id,
                issue_number,
                simple_id,
                req.status_id,
                req.title,
                req.description,
                priority,
                req.start_date,
                req.target_date,
                req.completed_at,
                req.sort_order,
                req.parent_issue_id,
                req.parent_issue_sort_order,
                data.creator_user_id,
                extension_metadata,
            )
            .fetch_one(&mut *conn)
            .await
        }
        .await;

        match result {
            Ok(issue) => {
                sqlx::query("COMMIT").execute(&mut *conn).await?;
                Ok(issue)
            }
            Err(err) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                Err(err)
            }
        }
    }

    pub async fn update(
        pool: &SqlitePool,
        id: Uuid,
        data: &UpdateIssueRequest,
    ) -> Result<Self, sqlx::Error> {
        let update_status_id = data.status_id.is_some();
        let status_id_value = data.status_id;
        let update_title = data.title.is_some();
        let title_value = data.title.clone();
        let update_description = data.description.is_some();
        let description_value = data.description.clone().flatten();
        let update_priority = data.priority.is_some();
        let priority_value = data.priority.flatten().map(IssuePriority::from);
        let update_start_date = data.start_date.is_some();
        let start_date_value = data.start_date.flatten();
        let update_target_date = data.target_date.is_some();
        let target_date_value = data.target_date.flatten();
        let update_completed_at = data.completed_at.is_some();
        let completed_at_value = data.completed_at.flatten();
        let update_sort_order = data.sort_order.is_some();
        let sort_order_value = data.sort_order;
        let update_parent_issue_id = data.parent_issue_id.is_some();
        let parent_issue_id_value = data.parent_issue_id.flatten();
        let update_parent_issue_sort_order = data.parent_issue_sort_order.is_some();
        let parent_issue_sort_order_value = data.parent_issue_sort_order.flatten();
        let update_extension_metadata = data.extension_metadata.is_some();
        let extension_metadata_value = data.extension_metadata.clone().map(sqlx::types::Json);

        sqlx::query_as!(
            Issue,
            r#"UPDATE issues
               SET status_id                = CASE WHEN $2  THEN $3  ELSE status_id END,
                   title                    = CASE WHEN $4  THEN $5  ELSE title END,
                   description              = CASE WHEN $6  THEN $7  ELSE description END,
                   priority                 = CASE WHEN $8  THEN $9  ELSE priority END,
                   start_date               = CASE WHEN $10 THEN $11 ELSE start_date END,
                   target_date              = CASE WHEN $12 THEN $13 ELSE target_date END,
                   completed_at             = CASE WHEN $14 THEN $15 ELSE completed_at END,
                   sort_order               = CASE WHEN $16 THEN $17 ELSE sort_order END,
                   parent_issue_id          = CASE WHEN $18 THEN $19 ELSE parent_issue_id END,
                   parent_issue_sort_order  = CASE WHEN $20 THEN $21 ELSE parent_issue_sort_order END,
                   extension_metadata       = CASE WHEN $22 THEN $23 ELSE extension_metadata END,
                   updated_at               = datetime('now', 'subsec')
               WHERE id = $1
               RETURNING id                       as "id!: Uuid",
                         project_id               as "project_id!: Uuid",
                         issue_number,
                         simple_id,
                         status_id                as "status_id!: Uuid",
                         title,
                         description,
                         priority                 as "priority: IssuePriority",
                         start_date               as "start_date: DateTime<Utc>",
                         target_date              as "target_date: DateTime<Utc>",
                         completed_at             as "completed_at: DateTime<Utc>",
                         sort_order               as "sort_order!: f64",
                         parent_issue_id          as "parent_issue_id: Uuid",
                         parent_issue_sort_order  as "parent_issue_sort_order: f64",
                         creator_user_id          as "creator_user_id: Uuid",
                         extension_metadata       as "extension_metadata!: sqlx::types::Json<Value>",
                         created_at               as "created_at!: DateTime<Utc>",
                         updated_at               as "updated_at!: DateTime<Utc>""#,
            id,
            update_status_id,
            status_id_value,
            update_title,
            title_value,
            update_description,
            description_value,
            update_priority,
            priority_value,
            update_start_date,
            start_date_value,
            update_target_date,
            target_date_value,
            update_completed_at,
            completed_at_value,
            update_sort_order,
            sort_order_value,
            update_parent_issue_id,
            parent_issue_id_value,
            update_parent_issue_sort_order,
            parent_issue_sort_order_value,
            update_extension_metadata,
            extension_metadata_value,
        )
        .fetch_one(pool)
        .await
    }

    pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!("DELETE FROM issues WHERE id = $1", id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }
}

impl From<Issue> for wire::Issue {
    fn from(value: Issue) -> Self {
        Self {
            id: value.id,
            project_id: value.project_id,
            issue_number: value.issue_number,
            simple_id: value.simple_id,
            status_id: value.status_id,
            title: value.title,
            description: value.description,
            priority: value.priority.map(wire::IssuePriority::from),
            start_date: value.start_date,
            target_date: value.target_date,
            completed_at: value.completed_at,
            sort_order: value.sort_order,
            parent_issue_id: value.parent_issue_id,
            parent_issue_sort_order: value.parent_issue_sort_order,
            extension_metadata: value.extension_metadata.0,
            creator_user_id: value.creator_user_id,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}
