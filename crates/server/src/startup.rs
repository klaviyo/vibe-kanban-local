use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
};

use deployment::{Deployment, DeploymentError};
use services::services::container::ContainerService;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tower_http::validate_request::ValidateRequestHeaderLayer;
use utils::assets::asset_dir;

use crate::{
    DeploymentImpl, middleware::origin::validate_origin, routes, runtime::relay_registration,
};

/// A running server instance. Callers can read the port, then call `serve()`
/// to run the server until the shutdown token is cancelled.
pub struct ServerHandle {
    pub port: u16,
    pub proxy_port: u16,
    pub deployment: DeploymentImpl,
    shutdown_token: CancellationToken,
    main_listener: tokio::net::TcpListener,
    proxy_listener: tokio::net::TcpListener,
}

impl ServerHandle {
    /// The base URL the main server is listening on.
    ///
    /// Uses `localhost` rather than `127.0.0.1` so that macOS ATS
    /// (App Transport Security) exception domains apply correctly in
    /// the Tauri desktop app — IP address literals aren't reliably
    /// matched by ATS, which causes WebSocket connections to fail.
    pub fn url(&self) -> String {
        format!("http://localhost:{}", self.port)
    }

    /// Run both the main and proxy servers until the shutdown token is cancelled.
    pub async fn serve(self) -> anyhow::Result<()> {
        // Start relay tunnel so the host registers with the relay server.
        // This must happen after the port is known (it's needed for local
        // proxying) and is shared between the standalone binary and Tauri.
        self.deployment
            .client_info()
            .set_server_addr(self.main_listener.local_addr()?)
            .expect("client server address already set");
        self.deployment
            .client_info()
            .set_preview_proxy_port(self.proxy_port)
            .expect("client preview proxy port already set");
        relay_registration::spawn_relay(&self.deployment).await;

        let app_router = routes::router(self.deployment.clone());
        let proxy_router: axum::Router = routes::preview::subdomain_router(self.deployment.clone())
            .layer(ValidateRequestHeaderLayer::custom(validate_origin));

        let main_shutdown = self.shutdown_token.clone();
        let proxy_shutdown = self.shutdown_token.clone();

        let main_server = axum::serve(self.main_listener, app_router)
            .with_graceful_shutdown(async move { main_shutdown.cancelled().await });
        let proxy_server = axum::serve(self.proxy_listener, proxy_router)
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

        // Borrow handles via `&mut` so they survive the select and can be
        // awaited again inside `perform_cleanup_actions` for full drain.
        tokio::select! {
            _ = self.shutdown_token.cancelled() => {
                tracing::info!("Shutdown token cancelled");
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
            &self.deployment,
            &self.shutdown_token,
            vec![main_handle, proxy_handle],
        )
        .await;
        Ok(())
    }

    /// Return a clone of the shutdown token. Cancel it to stop `serve()`.
    pub fn shutdown_token(&self) -> CancellationToken {
        self.shutdown_token.clone()
    }
}

/// Initialize the deployment, bind listeners on `localhost` with OS-assigned
/// ports, and return a handle that is ready to serve.
///
/// Uses `localhost` rather than `127.0.0.1` so the bind address matches
/// the hostname the frontend connects to. On modern macOS, `localhost`
/// resolves to `::1` (IPv6) first — binding to `127.0.0.1` (IPv4) while
/// the browser connects via `::1` causes "connection refused".
pub async fn start() -> anyhow::Result<ServerHandle> {
    start_with_bind("localhost:0", "localhost:0", CancellationToken::new()).await
}

/// Like [`start`], but lets the caller specify the bind addresses for the main
/// server and the preview proxy (e.g. `"0.0.0.0:8080"`).
pub async fn start_with_bind(
    main_addr: &str,
    proxy_addr: &str,
    shutdown_token: CancellationToken,
) -> anyhow::Result<ServerHandle> {
    let deployment = initialize_deployment(shutdown_token.clone()).await?;

    let listener = tokio::net::TcpListener::bind(main_addr).await?;
    let port = listener.local_addr()?.port();

    let proxy_listener = tokio::net::TcpListener::bind(proxy_addr).await?;
    let proxy_port = proxy_listener.local_addr()?.port();

    tracing::info!("Server on :{port}, Preview proxy on :{proxy_port}");

    Ok(ServerHandle {
        port,
        proxy_port,
        deployment,
        shutdown_token,
        main_listener: listener,
        proxy_listener,
    })
}

/// Initialize the deployment: create asset directory, run migrations, backfill data,
/// and pre-warm caches. Shared between the standalone server and the Tauri app.
pub async fn initialize_deployment(
    shutdown: CancellationToken,
) -> Result<DeploymentImpl, DeploymentError> {
    // Create asset directory if it doesn't exist
    if !asset_dir().exists() {
        std::fs::create_dir_all(asset_dir()).map_err(|e| {
            DeploymentError::Other(anyhow::anyhow!("Failed to create asset directory: {}", e))
        })?;
    }

    // Copy old database to new location for safe downgrades
    let old_db = asset_dir().join("db.sqlite");
    let new_db = asset_dir().join("db.v2.sqlite");
    if !new_db.exists() && old_db.exists() {
        tracing::info!(
            "Copying database to new location: {:?} -> {:?}",
            old_db,
            new_db
        );
        std::fs::copy(&old_db, &new_db).expect("Failed to copy database file");
        tracing::info!("Database copy complete");
    }

    let deployment = DeploymentImpl::new(shutdown).await?;
    migrate_legacy_attachment_directories(&deployment).await?;
    deployment.update_sentry_scope().await?;
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
    deployment
        .track_if_analytics_allowed("session_start", serde_json::json!({}))
        .await;

    // Preload global executor options cache for all executors with DEFAULT presets
    tokio::spawn(async move {
        executors::executors::utils::preload_global_executor_options_cache().await;
    });

    Ok(deployment)
}

/// Gracefully shut down the running server.
///
/// Cancels `shutdown_token` to begin axum's graceful shutdown, waits for
/// in-flight HTTP handlers to drain, kills any running execution processes
/// (which still need DB access), and finally drains the SQLx pool so in-flight
/// transactions either commit or roll back cleanly. Errors from the process
/// kill are logged rather than panicked so the pool drain still runs.
pub async fn perform_cleanup_actions(
    deployment: &DeploymentImpl,
    shutdown_token: &CancellationToken,
    server_handles: Vec<JoinHandle<()>>,
) {
    drain_http_servers(shutdown_token, server_handles).await;

    if let Err(e) = deployment.container().kill_all_running_processes().await {
        tracing::error!("Failed to cleanly kill running execution processes: {}", e);
    }

    // Close the pool last so that DB-backed cleanup above runs against a live
    // pool; closing waits for in-flight transactions to release their
    // connections, allowing SQLite to finalize and remove its rollback journal.
    deployment.db().pool.close().await;
}

/// Cancels the shutdown token to start axum's graceful shutdown, then awaits
/// the supplied server task handles so axum finishes draining in-flight
/// requests before any DB-backed cleanup runs.
///
/// Skips handles whose task has already finished — those may have been polled
/// to completion by the caller's `select!`, and Tokio panics with "polled
/// `JoinHandle` after completion" on a second poll.
async fn drain_http_servers(
    shutdown_token: &CancellationToken,
    server_handles: Vec<JoinHandle<()>>,
) {
    shutdown_token.cancel();

    for handle in server_handles {
        if handle.is_finished() {
            continue;
        }
        if let Err(e) = handle.await {
            tracing::warn!("Server task ended with error: {}", e);
        }
    }
}

const LEGACY_ATTACHMENT_MIGRATION_MARKER: &str = ".attachment-directories-migrated-v1";

#[derive(Default)]
struct DirectoryMigrationStats {
    moved_files: u64,
    removed_duplicates: u64,
    created_directories: u64,
    failures: u64,
}

impl DirectoryMigrationStats {
    fn merge(&mut self, other: DirectoryMigrationStats) {
        self.moved_files += other.moved_files;
        self.removed_duplicates += other.removed_duplicates;
        self.created_directories += other.created_directories;
        self.failures += other.failures;
    }
}

async fn migrate_legacy_attachment_directories(
    deployment: &DeploymentImpl,
) -> Result<(), DeploymentError> {
    let marker_path = asset_dir().join(LEGACY_ATTACHMENT_MIGRATION_MARKER);
    if marker_path.exists() {
        return Ok(());
    }

    let mut stats = DirectoryMigrationStats::default();

    let cache_root = utils::cache_dir();
    stats.merge(migrate_legacy_directory(
        &cache_root.join("images"),
        &cache_root.join("attachments"),
        false,
    ));

    for base_path in collect_attachment_migration_paths(deployment).await? {
        stats.merge(migrate_legacy_directory(
            &base_path.join(".vibe-images"),
            &base_path.join(utils::path::VIBE_ATTACHMENTS_DIR),
            true,
        ));
    }

    if stats.failures == 0 {
        fs::write(&marker_path, b"ok")?;
        tracing::info!(
            "Legacy attachment directory migration completed: moved {}, removed duplicates {}, created directories {}",
            stats.moved_files,
            stats.removed_duplicates,
            stats.created_directories
        );
    } else {
        tracing::warn!(
            "Legacy attachment directory migration completed with {} failures; will retry on next startup",
            stats.failures
        );
    }

    Ok(())
}

async fn collect_attachment_migration_paths(
    deployment: &DeploymentImpl,
) -> Result<Vec<PathBuf>, DeploymentError> {
    use db::models::{session::Session, workspace::Workspace, workspace_repo::WorkspaceRepo};

    let workspaces = Workspace::fetch_all(&deployment.db().pool).await?;
    let mut paths = HashSet::new();

    for workspace in workspaces {
        let Some(container_ref) = workspace.container_ref.as_deref() else {
            continue;
        };
        if container_ref.is_empty() {
            continue;
        }

        let workspace_root = PathBuf::from(container_ref);
        paths.insert(workspace_root.clone());

        for repo in
            WorkspaceRepo::find_repos_for_workspace(&deployment.db().pool, workspace.id).await?
        {
            let repo_base = match repo.default_working_dir.as_deref() {
                Some(default_dir) if !default_dir.is_empty() => {
                    workspace_root.join(&repo.name).join(default_dir)
                }
                _ => workspace_root.join(&repo.name),
            };
            paths.insert(repo_base);
        }

        for session in Session::find_by_workspace_id(&deployment.db().pool, workspace.id).await? {
            let base_path = match session.agent_working_dir.as_deref() {
                Some(dir) if !dir.is_empty() => workspace_root.join(dir),
                _ => workspace_root.clone(),
            };
            paths.insert(base_path);
        }
    }

    let mut paths = paths.into_iter().collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

fn migrate_legacy_directory(
    src_dir: &Path,
    dst_dir: &Path,
    ensure_gitignore: bool,
) -> DirectoryMigrationStats {
    let mut stats = DirectoryMigrationStats::default();

    if !src_dir.exists() {
        return stats;
    }

    if let Err(error) = fs::create_dir_all(dst_dir) {
        tracing::warn!(
            "Failed to create attachment directory {}: {}",
            dst_dir.display(),
            error
        );
        stats.failures += 1;
        return stats;
    }
    stats.created_directories += 1;

    if let Err(error) = migrate_directory_contents(src_dir, dst_dir, ensure_gitignore, &mut stats) {
        tracing::warn!(
            "Failed to migrate legacy attachment directory {} -> {}: {}",
            src_dir.display(),
            dst_dir.display(),
            error
        );
        stats.failures += 1;
    }

    if ensure_gitignore && let Err(error) = ensure_attachments_gitignore(dst_dir) {
        tracing::warn!(
            "Failed to ensure .gitignore in {}: {}",
            dst_dir.display(),
            error
        );
        stats.failures += 1;
    }

    if let Err(error) = remove_empty_dir_tree(src_dir) {
        tracing::warn!(
            "Failed to clean up legacy attachment directory {}: {}",
            src_dir.display(),
            error
        );
        stats.failures += 1;
    }

    stats
}

fn migrate_directory_contents(
    src_dir: &Path,
    dst_dir: &Path,
    ensure_gitignore: bool,
    stats: &mut DirectoryMigrationStats,
) -> io::Result<()> {
    for entry in fs::read_dir(src_dir)? {
        let entry = entry?;
        let src_path = entry.path();
        let file_name = entry.file_name();

        if ensure_gitignore && file_name == ".gitignore" {
            continue;
        }

        let dst_path = dst_dir.join(&file_name);
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            fs::create_dir_all(&dst_path)?;
            migrate_directory_contents(&src_path, &dst_path, false, stats)?;
            remove_empty_dir_tree(&src_path)?;
            continue;
        }

        if dst_path.exists() {
            fs::remove_file(&src_path)?;
            stats.removed_duplicates += 1;
            continue;
        }

        move_path(&src_path, &dst_path)?;
        stats.moved_files += 1;
    }

    Ok(())
}

fn move_path(src_path: &Path, dst_path: &Path) -> io::Result<()> {
    match fs::rename(src_path, dst_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::CrossesDevices => {
            fs::copy(src_path, dst_path)?;
            fs::remove_file(src_path)
        }
        Err(error) => Err(error),
    }
}

fn ensure_attachments_gitignore(dir: &Path) -> io::Result<()> {
    let gitignore_path = dir.join(".gitignore");
    if !gitignore_path.exists() {
        fs::write(gitignore_path, "*\n")?;
    }
    Ok(())
}

fn remove_empty_dir_tree(path: &Path) -> io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    if fs::read_dir(path)?.next().is_none() {
        fs::remove_dir(path)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        str::FromStr,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use sqlx::{
        SqlitePool,
        sqlite::{SqliteConnectOptions, SqliteJournalMode},
    };
    use tempfile::TempDir;

    use super::*;

    async fn open_pool(path: &PathBuf) -> SqlitePool {
        let url = format!("sqlite://{}", path.to_string_lossy());
        let opts = SqliteConnectOptions::from_str(&url)
            .unwrap()
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Delete);
        SqlitePool::connect_with(opts).await.unwrap()
    }

    #[tokio::test]
    async fn drain_http_servers_cancels_token_and_awaits_handles() {
        let token = CancellationToken::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..2 {
            let token = token.clone();
            let counter = counter.clone();
            handles.push(tokio::spawn(async move {
                token.cancelled().await;
                // Simulate axum's request-drain delay after cancellation.
                tokio::time::sleep(Duration::from_millis(50)).await;
                counter.fetch_add(1, Ordering::SeqCst);
            }));
        }

        drain_http_servers(&token, handles).await;

        assert!(token.is_cancelled(), "shutdown token should be cancelled");
        assert_eq!(
            counter.load(Ordering::SeqCst),
            2,
            "drain should await every server handle to completion"
        );
    }

    #[tokio::test]
    async fn drain_http_servers_swallows_panicking_server_handle() {
        let token = CancellationToken::new();
        let panicking = tokio::spawn(async { panic!("simulated server crash") });

        // A panic in a server task must not prevent drain_http_servers from
        // returning so DB-backed cleanup and the final pool drain still run.
        drain_http_servers(&token, vec![panicking]).await;

        assert!(token.is_cancelled(), "shutdown token should be cancelled");
    }

    #[tokio::test]
    async fn drain_http_servers_skips_already_polled_handles() {
        // Mirrors the call-site pattern: the caller's `select!` polls one
        // handle via `&mut`, completes it, then forwards both handles into
        // drain_http_servers. The already-polled handle must be skipped so
        // Tokio doesn't panic with "polled `JoinHandle` after completion".
        let token = CancellationToken::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let mut polled_handle = tokio::spawn(async {});
        (&mut polled_handle)
            .await
            .expect("poll-to-completion should succeed");
        assert!(
            polled_handle.is_finished(),
            "handle should be finished after caller polls it"
        );

        let counter_clone = counter.clone();
        let pending_handle = tokio::spawn(async move {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        drain_http_servers(&token, vec![polled_handle, pending_handle]).await;

        assert!(token.is_cancelled());
        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "still-pending handle must be awaited to completion"
        );
    }

    #[tokio::test]
    async fn drain_http_servers_returns_before_pool_close() {
        // Order assertion: drain_http_servers does not touch the pool, so the
        // pool remains live for any DB-backed cleanup that runs between
        // drain_http_servers and the final pool.close().
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("test.db");
        let pool = open_pool(&db_path).await;

        let token = CancellationToken::new();
        drain_http_servers(&token, vec![]).await;

        assert!(
            !pool.is_closed(),
            "pool must stay open so DB-backed cleanup can run before close"
        );
        sqlx::query("SELECT 1")
            .execute(&pool)
            .await
            .expect("pool must accept queries after drain_http_servers");

        pool.close().await;
    }

    #[test]
    fn migrates_legacy_cache_directory_contents() {
        let temp_dir = TempDir::new().unwrap();
        let src = temp_dir.path().join("images");
        let dst = temp_dir.path().join("attachments");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("asset.png"), b"hello").unwrap();

        let stats = migrate_legacy_directory(&src, &dst, false);

        assert_eq!(stats.moved_files, 1);
        assert!(dst.join("asset.png").exists());
        assert!(!src.exists());
    }

    #[test]
    fn removes_legacy_duplicates_when_destination_exists() {
        let temp_dir = TempDir::new().unwrap();
        let src = temp_dir.path().join(".vibe-images");
        let dst = temp_dir.path().join(".vibe-attachments");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dst).unwrap();
        fs::write(src.join("asset.png"), b"old").unwrap();
        fs::write(dst.join("asset.png"), b"new").unwrap();

        let stats = migrate_legacy_directory(&src, &dst, true);

        assert_eq!(stats.removed_duplicates, 1);
        assert_eq!(fs::read(dst.join("asset.png")).unwrap(), b"new");
        assert!(!src.exists());
    }

    #[test]
    fn ensures_gitignore_for_workspace_attachment_dir() {
        let temp_dir = TempDir::new().unwrap();
        let src = temp_dir.path().join(".vibe-images");
        let dst = temp_dir.path().join(".vibe-attachments");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("file.pdf"), b"attachment").unwrap();

        migrate_legacy_directory(&src, &dst, true);

        assert_eq!(fs::read_to_string(dst.join(".gitignore")).unwrap(), "*\n");
        assert!(dst.join("file.pdf").exists());
    }
}
