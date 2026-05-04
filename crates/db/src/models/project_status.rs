use api_types::{
    self as wire, DeleteResponse, MutationResponse,
    project_status::{CreateProjectStatusRequest, UpdateProjectStatusRequest},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use super::mutation_log;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ProjectStatus {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub color: String,
    pub sort_order: i64,
    pub hidden: bool,
    pub created_at: DateTime<Utc>,
}

impl ProjectStatus {
    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            ProjectStatus,
            r#"SELECT id         as "id!: Uuid",
                      project_id as "project_id!: Uuid",
                      name,
                      color,
                      sort_order,
                      hidden     as "hidden!: bool",
                      created_at as "created_at!: DateTime<Utc>"
               FROM project_statuses
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
            ProjectStatus,
            r#"SELECT id         as "id!: Uuid",
                      project_id as "project_id!: Uuid",
                      name,
                      color,
                      sort_order,
                      hidden     as "hidden!: bool",
                      created_at as "created_at!: DateTime<Utc>"
               FROM project_statuses
               WHERE project_id = $1
               ORDER BY sort_order ASC, created_at ASC"#,
            project_id,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn create(
        pool: &SqlitePool,
        id: Uuid,
        data: &CreateProjectStatusRequest,
    ) -> Result<MutationResponse<Self>, sqlx::Error> {
        let sort_order = i64::from(data.sort_order);
        let mut tx = pool.begin().await?;
        let row = sqlx::query_as!(
            ProjectStatus,
            r#"INSERT INTO project_statuses (id, project_id, name, color, sort_order, hidden)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING id         as "id!: Uuid",
                         project_id as "project_id!: Uuid",
                         name,
                         color,
                         sort_order,
                         hidden     as "hidden!: bool",
                         created_at as "created_at!: DateTime<Utc>""#,
            id,
            data.project_id,
            data.name,
            data.color,
            sort_order,
            data.hidden,
        )
        .fetch_one(&mut *tx)
        .await?;
        let txid = mutation_log::next_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(MutationResponse { data: row, txid })
    }

    pub async fn update(
        pool: &SqlitePool,
        id: Uuid,
        data: &UpdateProjectStatusRequest,
    ) -> Result<MutationResponse<Self>, sqlx::Error> {
        let update_name = data.name.is_some();
        let name_value = data.name.clone();
        let update_color = data.color.is_some();
        let color_value = data.color.clone();
        let update_sort_order = data.sort_order.is_some();
        let sort_order_value = data.sort_order.map(i64::from);
        let update_hidden = data.hidden.is_some();
        let hidden_value = data.hidden;

        let mut tx = pool.begin().await?;
        let row = sqlx::query_as!(
            ProjectStatus,
            r#"UPDATE project_statuses
               SET name       = CASE WHEN $2 THEN $3 ELSE name END,
                   color      = CASE WHEN $4 THEN $5 ELSE color END,
                   sort_order = CASE WHEN $6 THEN $7 ELSE sort_order END,
                   hidden     = CASE WHEN $8 THEN $9 ELSE hidden END
               WHERE id = $1
               RETURNING id         as "id!: Uuid",
                         project_id as "project_id!: Uuid",
                         name,
                         color,
                         sort_order,
                         hidden     as "hidden!: bool",
                         created_at as "created_at!: DateTime<Utc>""#,
            id,
            update_name,
            name_value,
            update_color,
            color_value,
            update_sort_order,
            sort_order_value,
            update_hidden,
            hidden_value,
        )
        .fetch_one(&mut *tx)
        .await?;
        let txid = mutation_log::next_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(MutationResponse { data: row, txid })
    }

    pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<DeleteResponse, sqlx::Error> {
        let mut tx = pool.begin().await?;
        sqlx::query!("DELETE FROM project_statuses WHERE id = $1", id)
            .execute(&mut *tx)
            .await?;
        let txid = mutation_log::next_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(DeleteResponse { txid })
    }
}

impl From<ProjectStatus> for wire::ProjectStatus {
    fn from(value: ProjectStatus) -> Self {
        Self {
            id: value.id,
            project_id: value.project_id,
            name: value.name,
            color: value.color,
            sort_order: value.sort_order as i32,
            hidden: value.hidden,
            created_at: value.created_at,
        }
    }
}
