use api_types::{self as wire, project::UpdateProjectRequest};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub default_agent_working_dir: Option<String>,
    pub remote_project_id: Option<Uuid>,
    #[ts(type = "Date")]
    pub created_at: DateTime<Utc>,
    #[ts(type = "Date")]
    pub updated_at: DateTime<Utc>,
}

/// Cloud-shape projection of `projects`. Includes the additive columns
/// (`organization_id`, `color`, `sort_order`) introduced by the issue-domain
/// migrations and excludes local-only fields. `organization_id` remains
/// nullable until the synthetic-organization seeder backfills pre-cutover rows.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ProjectRow {
    pub id: Uuid,
    pub organization_id: Option<Uuid>,
    pub name: String,
    pub color: String,
    pub sort_order: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateProject {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub color: String,
}

impl Project {
    pub async fn find_all(pool: &SqlitePool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Project,
            r#"SELECT id as "id!: Uuid",
                      name,
                      default_agent_working_dir,
                      remote_project_id as "remote_project_id: Uuid",
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM projects
               ORDER BY created_at DESC"#
        )
        .fetch_all(pool)
        .await
    }

    pub async fn set_remote_project_id(
        pool: &SqlitePool,
        id: Uuid,
        remote_project_id: Option<Uuid>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"UPDATE projects
               SET remote_project_id = $2
               WHERE id = $1"#,
            id,
            remote_project_id
        )
        .execute(pool)
        .await?;

        Ok(())
    }
}

impl ProjectRow {
    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            ProjectRow,
            r#"SELECT id              as "id!: Uuid",
                      organization_id as "organization_id: Uuid",
                      name,
                      color,
                      sort_order,
                      created_at      as "created_at!: DateTime<Utc>",
                      updated_at      as "updated_at!: DateTime<Utc>"
               FROM projects
               WHERE id = $1"#,
            id,
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_organization(
        pool: &SqlitePool,
        organization_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            ProjectRow,
            r#"SELECT id              as "id!: Uuid",
                      organization_id as "organization_id: Uuid",
                      name,
                      color,
                      sort_order,
                      created_at      as "created_at!: DateTime<Utc>",
                      updated_at      as "updated_at!: DateTime<Utc>"
               FROM projects
               WHERE organization_id = $1
               ORDER BY sort_order ASC, created_at ASC"#,
            organization_id,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn create(pool: &SqlitePool, data: &CreateProject) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            ProjectRow,
            r#"INSERT INTO projects (id, organization_id, name, color)
               VALUES ($1, $2, $3, $4)
               RETURNING id              as "id!: Uuid",
                         organization_id as "organization_id: Uuid",
                         name,
                         color,
                         sort_order,
                         created_at      as "created_at!: DateTime<Utc>",
                         updated_at      as "updated_at!: DateTime<Utc>""#,
            data.id,
            data.organization_id,
            data.name,
            data.color,
        )
        .fetch_one(pool)
        .await
    }

    pub async fn update(
        pool: &SqlitePool,
        id: Uuid,
        data: &UpdateProjectRequest,
    ) -> Result<Self, sqlx::Error> {
        let update_name = data.name.is_some();
        let name_value = data.name.clone();
        let update_color = data.color.is_some();
        let color_value = data.color.clone();
        let update_sort_order = data.sort_order.is_some();
        let sort_order_value = data.sort_order.map(i64::from);

        sqlx::query_as!(
            ProjectRow,
            r#"UPDATE projects
               SET name       = CASE WHEN $2 THEN $3 ELSE name END,
                   color      = CASE WHEN $4 THEN $5 ELSE color END,
                   sort_order = CASE WHEN $6 THEN $7 ELSE sort_order END,
                   updated_at = datetime('now', 'subsec')
               WHERE id = $1
               RETURNING id              as "id!: Uuid",
                         organization_id as "organization_id: Uuid",
                         name,
                         color,
                         sort_order,
                         created_at      as "created_at!: DateTime<Utc>",
                         updated_at      as "updated_at!: DateTime<Utc>""#,
            id,
            update_name,
            name_value,
            update_color,
            color_value,
            update_sort_order,
            sort_order_value,
        )
        .fetch_one(pool)
        .await
    }

    pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!("DELETE FROM projects WHERE id = $1", id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }
}

/// Returned when a `ProjectRow` cannot be projected to the cloud-shape wire
/// `Project` because its `organization_id` is still `NULL` (i.e. the
/// synthetic-organization backfill has not run for that row yet).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissingOrganizationId {
    pub project_id: Uuid,
}

impl std::fmt::Display for MissingOrganizationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "project {} has no organization_id; backfill required before exposing on the cloud surface",
            self.project_id
        )
    }
}

impl std::error::Error for MissingOrganizationId {}

impl TryFrom<ProjectRow> for wire::Project {
    type Error = MissingOrganizationId;

    fn try_from(value: ProjectRow) -> Result<Self, Self::Error> {
        let organization_id = value.organization_id.ok_or(MissingOrganizationId {
            project_id: value.id,
        })?;
        Ok(Self {
            id: value.id,
            organization_id,
            name: value.name,
            color: value.color,
            sort_order: value.sort_order as i32,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}
