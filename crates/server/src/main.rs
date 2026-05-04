use anyhow::{self, Error as AnyhowError};
use axum::Router;
use deployment::{Deployment, DeploymentError};
use server::{
    DeploymentImpl, middleware::origin::validate_origin, routes,
    startup::perform_cleanup_actions,
};
use services::services::container::ContainerService;
use sqlx::Error as SqlxError;
use strip_ansi_escapes::strip;
use thiserror::Error;
use tokio_util::sync::CancellationToken;
use tower_http::validate_request::ValidateRequestHeaderLayer;
use tracing_subscriber::{EnvFilter, prelude::*};
use utils::{
    assets::{CutoverError, asset_dir, ensure_db_v3},
    port_file::write_port_file_with_proxy,
};

#[derive(Debug, Error)]
pub enum VibeKanbanError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Sqlx(#[from] SqlxError),
    #[error(transparent)]
    Deployment(#[from] DeploymentError),
    #[error(transparent)]
    Cutover(#[from] CutoverError),
    #[error(transparent)]
    Other(#[from] AnyhowError),
}

#[tokio::main]
async fn main() -> Result<(), VibeKanbanError> {
    // Install rustls crypto provider before any TLS operations
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    let filter_string = format!(
        "warn,server={level},services={level},db={level},executors={level},deployment={level},local_deployment={level},utils={level},codex_core=off",
        level = log_level
    );
    let env_filter = EnvFilter::try_new(filter_string).expect("Failed to create tracing filter");
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(env_filter))
        .init();

    // Create asset directory if it doesn't exist
    if !asset_dir().exists() {
        std::fs::create_dir_all(asset_dir())?;
    }

    // First-launch cutover-copy from the v2 database to v3. v2 is left
    // byte-identical as the rollback artifact; the new schema runs only
    // against v3. Refuses if a hot rollback journal sits beside v2.
    ensure_db_v3()?;

    let shutdown_token = CancellationToken::new();

    let deployment = DeploymentImpl::new(shutdown_token.clone()).await?;
    deployment
        .container()
        .cleanup_orphan_executions()
        .await
        .map_err(DeploymentError::from)?;
    deployment
        .container()
        .backfill_before_head_commits()
        .await
        .map_err(DeploymentError::from)?;
    deployment
        .container()
        .backfill_repo_names()
        .await
        .map_err(DeploymentError::from)?;
    // Preload global executor options cache for all executors with DEFAULT presets
    tokio::spawn(async move {
        executors::executors::utils::preload_global_executor_options_cache().await;
    });
    let port = std::env::var("BACKEND_PORT")
        .or_else(|_| std::env::var("PORT"))
        .ok()
        .and_then(|s| {
            // Remove any ANSI codes, then turn into String
            let cleaned =
                String::from_utf8(strip(s.as_bytes())).expect("UTF-8 after stripping ANSI");
            cleaned.trim().parse::<u16>().ok()
        })
        .unwrap_or_else(|| {
            tracing::info!("No PORT environment variable set, using port 0 for auto-assignment");
            0
        }); // Use 0 to find free port if no specific port provided

    let proxy_port = std::env::var("PREVIEW_PROXY_PORT")
        .ok()
        .and_then(|s| s.trim().parse::<u16>().ok())
        .unwrap_or(0);

    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());

    let main_listener = tokio::net::TcpListener::bind(format!("{host}:{port}")).await?;
    let actual_main_port = main_listener.local_addr()?.port();

    let proxy_listener = tokio::net::TcpListener::bind(format!("{host}:{proxy_port}")).await?;
    let actual_proxy_port = proxy_listener.local_addr()?.port();

    if let Err(e) = write_port_file_with_proxy(actual_main_port, Some(actual_proxy_port)).await {
        tracing::warn!("Failed to write port file: {}", e);
    }

    tracing::info!(
        "Main server on :{}, Preview proxy on :{}",
        actual_main_port,
        actual_proxy_port
    );

    deployment
        .client_info()
        .set_server_addr(main_listener.local_addr()?)
        .expect("client server address already set");
    deployment
        .client_info()
        .set_preview_proxy_port(actual_proxy_port)
        .expect("client preview proxy port already set");

    let app_router = routes::router(deployment.clone());

    // Production only: open browser
    if !cfg!(debug_assertions) {
        tracing::info!("Opening browser...");
        let browser_port = actual_main_port;
        tokio::spawn(async move {
            if let Err(e) =
                utils::browser::open_browser(&format!("http://127.0.0.1:{browser_port}")).await
            {
                tracing::warn!(
                    "Failed to open browser automatically: {}. Please open http://127.0.0.1:{} manually.",
                    e,
                    browser_port
                );
            }
        });
    }

    let proxy_router: Router = routes::preview::subdomain_router(deployment.clone())
        .layer(ValidateRequestHeaderLayer::custom(validate_origin));

    let main_shutdown = shutdown_token.clone();
    let proxy_shutdown = shutdown_token.clone();

    let main_server = axum::serve(main_listener, app_router)
        .with_graceful_shutdown(async move { main_shutdown.cancelled().await });
    let proxy_server = axum::serve(proxy_listener, proxy_router)
        .with_graceful_shutdown(async move { proxy_shutdown.cancelled().await });

    let mut main_handle = tokio::spawn(async move {
        if let Err(e) = main_server.await {
            tracing::error!("Main server error: {}", e);
        }
    });
    let mut proxy_handle = tokio::spawn(async move {
        if let Err(e) = proxy_server.await {
            tracing::error!("Preview proxy error: {}", e);
        }
    });

    // Borrow handles via `&mut` so they survive the select and can be awaited
    // again inside `perform_cleanup_actions` for full drain.
    tokio::select! {
        _ = shutdown_signal() => {
            tracing::info!("Shutdown signal received");
        }
        res = &mut main_handle => {
            if let Err(e) = res {
                tracing::error!("Main server task ended early: {}", e);
            }
        }
        res = &mut proxy_handle => {
            if let Err(e) = res {
                tracing::error!("Preview proxy task ended early: {}", e);
            }
        }
    }

    perform_cleanup_actions(
        &deployment,
        &shutdown_token,
        vec![main_handle, proxy_handle],
    )
    .await;

    Ok(())
}

pub async fn shutdown_signal() {
    // Always wait for Ctrl+C
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!("Failed to install Ctrl+C handler: {e}");
        }
    };

    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        // Try to install SIGTERM handler, but don't panic if it fails
        let terminate = async {
            if let Ok(mut sigterm) = signal(SignalKind::terminate()) {
                sigterm.recv().await;
            } else {
                tracing::error!("Failed to install SIGTERM handler");
                // Fallback: never resolves
                std::future::pending::<()>().await;
            }
        };

        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }
    }

    #[cfg(not(unix))]
    {
        // Only ctrl_c is available, so just await it
        ctrl_c.await;
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        str::FromStr,
        sync::Arc,
        time::{Duration, Instant},
    };

    use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
    use tempfile::TempDir;
    use tokio::sync::Notify;

    fn journal_path(db_path: &PathBuf) -> PathBuf {
        let mut name = db_path.file_name().unwrap().to_os_string();
        name.push("-journal");
        db_path.with_file_name(name)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn pool_close_waits_for_inflight_transaction_and_leaves_no_journal() {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("test.db");
        let url = format!("sqlite://{}", db_path.to_string_lossy());
        let opts = SqliteConnectOptions::from_str(&url)
            .unwrap()
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Delete);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap();
        sqlx::query("CREATE TABLE t (n INTEGER)")
            .execute(&pool)
            .await
            .unwrap();

        let writer_started = Arc::new(Notify::new());
        let writer_started_signal = writer_started.clone();
        let writer_pool = pool.clone();
        let writer = tokio::spawn(async move {
            let mut tx = writer_pool.begin().await.unwrap();
            sqlx::query("INSERT INTO t (n) VALUES (1)")
                .execute(&mut *tx)
                .await
                .unwrap();
            writer_started_signal.notify_one();
            // Hold the transaction long enough that pool.close must wait.
            tokio::time::sleep(Duration::from_millis(200)).await;
            tx.commit().await.unwrap();
        });

        writer_started.notified().await;

        let started = Instant::now();
        pool.close().await;
        let elapsed = started.elapsed();

        writer.await.unwrap();

        assert!(
            elapsed >= Duration::from_millis(150),
            "pool close returned before transaction finished: {:?}",
            elapsed
        );
        assert!(pool.is_closed(), "pool should be closed");
        assert!(
            !journal_path(&db_path).exists(),
            "rollback journal should not remain next to the database file"
        );
    }
}
