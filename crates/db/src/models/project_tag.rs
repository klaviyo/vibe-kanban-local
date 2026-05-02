use api_types::{
    self as wire,
    tag::{CreateTagRequest, UpdateTagRequest},
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

/// Cloud's `tags` table renamed locally to `project_tags` because the
/// existing `tags` table already serves the task-template tag domain.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ProjectTag {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub color: String,
}

impl ProjectTag {
    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            ProjectTag,
            r#"SELECT id         as "id!: Uuid",
                      project_id as "project_id!: Uuid",
                      name,
                      color
               FROM project_tags
               WHERE id = $1"#,
            id,
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_project(
        pool: &SqlitePool,
        project_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            ProjectTag,
            r#"SELECT id         as "id!: Uuid",
                      project_id as "project_id!: Uuid",
                      name,
                      color
               FROM project_tags
               WHERE project_id = $1
               ORDER BY name ASC"#,
            project_id,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn create(
        pool: &SqlitePool,
        id: Uuid,
        data: &CreateTagRequest,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            ProjectTag,
            r#"INSERT INTO project_tags (id, project_id, name, color)
               VALUES ($1, $2, $3, $4)
               RETURNING id         as "id!: Uuid",
                         project_id as "project_id!: Uuid",
                         name,
                         color"#,
            id,
            data.project_id,
            data.name,
            data.color,
        )
        .fetch_one(pool)
        .await
    }

    pub async fn update(
        pool: &SqlitePool,
        id: Uuid,
        data: &UpdateTagRequest,
    ) -> Result<Self, sqlx::Error> {
        let update_name = data.name.is_some();
        let name_value = data.name.clone();
        let update_color = data.color.is_some();
        let color_value = data.color.clone();

        sqlx::query_as!(
            ProjectTag,
            r#"UPDATE project_tags
               SET name  = CASE WHEN $2 THEN $3 ELSE name END,
                   color = CASE WHEN $4 THEN $5 ELSE color END
               WHERE id = $1
               RETURNING id         as "id!: Uuid",
                         project_id as "project_id!: Uuid",
                         name,
                         color"#,
            id,
            update_name,
            name_value,
            update_color,
            color_value,
        )
        .fetch_one(pool)
        .await
    }

    pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!("DELETE FROM project_tags WHERE id = $1", id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }
}

impl From<ProjectTag> for wire::Tag {
    fn from(value: ProjectTag) -> Self {
        Self {
            id: value.id,
            project_id: value.project_id,
            name: value.name,
            color: value.color,
        }
    }
}
