//! Synthesizes the `txid` field of `MutationResponse<T>` from a SQLite
//! `mutation_log` table -- a Postgres-free replacement for `pg_current_xact_id`.
//!
//! A mutating route opens a transaction, performs its data write, calls
//! [`next_txid`] against the same transaction, and commits. The returned id
//! becomes the wire txid. AUTOINCREMENT guarantees the id is strictly greater
//! than any previously committed value, and because the insert is part of the
//! surrounding transaction, a rollback also rolls back the id allocation: no
//! client ever observes a txid for a mutation that did not commit.

use sqlx::{Executor, Sqlite};

/// Allocates the next mutation_log id and returns it. Pass the same executor
/// (a `&mut SqliteConnection` borrowed from a transaction) used for the data
/// write so the id allocation participates in the same transaction.
pub async fn next_txid<'e, E>(executor: E) -> Result<i64, sqlx::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_scalar!(r#"INSERT INTO mutation_log DEFAULT VALUES RETURNING id as "id!: i64""#)
        .fetch_one(executor)
        .await
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use sqlx::{
        SqlitePool,
        sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    };

    use super::next_txid;

    async fn make_pool() -> SqlitePool {
        let opts = SqliteConnectOptions::from_str("sqlite::memory:")
            .unwrap()
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Delete)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn successive_calls_are_strictly_increasing_and_nonzero() {
        let pool = make_pool().await;

        let first = next_txid(&pool).await.unwrap();
        let second = next_txid(&pool).await.unwrap();
        let third = next_txid(&pool).await.unwrap();

        assert!(first > 0, "first txid must be non-zero, got {first}");
        assert!(
            second > first,
            "txid must be strictly increasing: {second} <= {first}"
        );
        assert!(
            third > second,
            "txid must be strictly increasing: {third} <= {second}"
        );
    }

    #[tokio::test]
    async fn rolled_back_write_does_not_produce_visible_txid() {
        let pool = make_pool().await;

        let committed_txid = {
            let mut tx = pool.begin().await.unwrap();
            let txid = next_txid(&mut *tx).await.unwrap();
            tx.commit().await.unwrap();
            txid
        };

        let rolled_back_txid = {
            let mut tx = pool.begin().await.unwrap();
            let txid = next_txid(&mut *tx).await.unwrap();
            tx.rollback().await.unwrap();
            txid
        };

        // The rolled-back transaction held an id at allocation time, but it
        // never became visible to any reader.
        assert!(rolled_back_txid > committed_txid);

        let visible_max: Option<i64> = sqlx::query_scalar("SELECT MAX(id) FROM mutation_log")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            visible_max,
            Some(committed_txid),
            "rolled-back write must not advance the visible mutation_log sequence"
        );

        // The next committed write produces a txid strictly greater than the
        // last committed txid, preserving the monotonic-per-mutation contract
        // observable by clients.
        let next_committed = {
            let mut tx = pool.begin().await.unwrap();
            let txid = next_txid(&mut *tx).await.unwrap();
            tx.commit().await.unwrap();
            txid
        };
        assert!(
            next_committed > committed_txid,
            "next committed txid {next_committed} must exceed prior committed {committed_txid}"
        );
    }

    #[tokio::test]
    async fn participates_in_caller_transaction() {
        // Demonstrates the intended call site shape: the helper accepts the
        // same executor used by the data write, so allocation rolls back with
        // the surrounding transaction.
        let pool = make_pool().await;

        let mut tx = pool.begin().await.unwrap();
        let _txid = next_txid(&mut *tx).await.unwrap();
        tx.rollback().await.unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mutation_log")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            count, 0,
            "rollback must remove the inserted mutation_log row"
        );
    }
}
