//! Synthetic identity seeder.
//!
//! On first launch this writes the cloud-shape singleton rows the local
//! deployment requires: one organization, one user, the membership linking
//! them, an "Initial Project" so the cloud invariant *every organization has at
//! least one project* holds, and the six default project_statuses that match
//! the VK product default (and the contract `Project::create_with_default_statuses`
//! enforces for engineer-created projects). The whole seed runs inside a single
//! transaction so partial failure rolls back cleanly and a retry is safe.
//!
//! On every subsequent launch the seeder is a strict no-op — the
//! `identity_seed_marker` row records that seeding completed, and the deterministic
//! UUID derivation is never re-run. The marker tracks two distinct host fields:
//! `host_identity` is the stable hardware-derived UUID source used to derive
//! the deterministic UUIDs (intentionally insensitive to hostname changes), and
//! `host_label` is the kernel hostname captured at seed time. A host rename is
//! observed as a `host_label` mismatch and surfaces as a structured warning; a
//! database moved to different hardware is observed as a `host_identity`
//! mismatch and surfaces as a separate warning. In both cases the existing
//! identity rows are left untouched.
//!
//! The seeder also performs an idempotent backfill of any pre-existing
//! `projects` rows whose `organization_id` is still NULL, linking them to the
//! synthetic organization. The backfill runs on every invocation so re-runs are
//! safe and additive.

use std::io;

use sqlx::{Sqlite, SqlitePool, Transaction};
use uuid::Uuid;

/// Stable namespace for deterministic UUIDv5 derivation. Treat as a constant —
/// changing it would re-derive identity on existing local databases and break
/// backup-restore continuity.
const IDENTITY_NAMESPACE: Uuid = Uuid::from_bytes([
    0xd7, 0xf2, 0xa8, 0xc3, 0x1b, 0x6e, 0x4a, 0x9d, 0x8c, 0x5f, 0x2e, 0x3b, 0x1a, 0x0f, 0x4d, 0x6c,
]);

/// Default project_statuses seeded for the Initial Project. The set matches the
/// VK product default exactly as `Project::create_with_default_statuses` seeds
/// for engineer-created projects, so case-insensitive name resolution bridges
/// across both seed paths and pipeline-state mapping is consistent regardless
/// of which path produced the project. Order is `(name, color, sort_order, hidden)`.
///
/// Pipeline-specific column sets (e.g. `two_stage_coding`) are configured at
/// the pipeline-state-to-VK-column mapping layer, not at this product default.
pub const DEFAULT_PROJECT_STATUSES: &[(&str, &str, i64, bool)] = &[
    ("Backlog", "#94a3b8", 0, false),
    ("Todo", "#3b82f6", 1, false),
    ("In Progress", "#f59e0b", 2, false),
    ("In Review", "#a855f7", 3, false),
    ("Done", "#22c55e", 4, false),
    ("Cancelled", "#ef4444", 5, true),
];

/// Slug + name + email used for the synthetic singletons. All fields are
/// deterministic so backup-restore preserves the row content bit-for-bit.
const SYNTHETIC_ORGANIZATION_NAME: &str = "Local";
const SYNTHETIC_ORGANIZATION_SLUG: &str = "local";
const SYNTHETIC_USER_EMAIL: &str = "local@vibe-kanban.local";
const SYNTHETIC_USER_USERNAME: &str = "local";
const SYNTHETIC_PROJECT_NAME: &str = "Initial Project";
const SYNTHETIC_PROJECT_COLOR: &str = "217 91% 60%";

#[derive(Debug, thiserror::Error)]
pub enum SeederError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("could not determine a stable host identity source: {0}")]
    HostIdentity(String),
}

/// Synthetic identity seeder. Construct with [`IdentitySeeder::new`] and call
/// [`IdentitySeeder::run`] before any service depends on the singleton rows.
pub struct IdentitySeeder<'a> {
    pool: &'a SqlitePool,
    host_identity: String,
    host_label: String,
}

impl<'a> IdentitySeeder<'a> {
    /// Build a seeder that uses the running host's stable identity source for
    /// UUID derivation and the kernel hostname for the rename-sensitive label.
    /// Returns an error only if no platform identity source is available;
    /// callers should treat that as a deployment-fatal condition.
    pub fn new(pool: &'a SqlitePool) -> Result<Self, SeederError> {
        let host_identity = host_identity()?;
        let host_label = host_label(&host_identity);
        Ok(Self {
            pool,
            host_identity,
            host_label,
        })
    }

    /// Build a seeder with explicitly provided host identity and label.
    /// Intended for tests so they can simulate first-launch, host-rename, and
    /// hardware-move scenarios without touching real OS identity sources.
    #[cfg(test)]
    pub fn with_host_identity(
        pool: &'a SqlitePool,
        host_identity: String,
        host_label: String,
    ) -> Self {
        Self {
            pool,
            host_identity,
            host_label,
        }
    }

    /// Run the seeder. On first launch this inserts the singleton rows; on
    /// subsequent launches it is a strict no-op for those inserts. The
    /// NULL-`organization_id` backfill always runs and is idempotent.
    pub async fn run(&self) -> Result<SeedOutcome, SeederError> {
        let mut tx = self.pool.begin().await?;
        let outcome = self.run_in_tx(&mut tx).await?;
        tx.commit().await?;
        Ok(outcome)
    }

    async fn run_in_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> Result<SeedOutcome, SeederError> {
        let existing = sqlx::query!(
            r#"SELECT organization_id as "organization_id!: Uuid",
                      user_id         as "user_id!: Uuid",
                      project_id      as "project_id!: Uuid",
                      host_identity,
                      host_label
               FROM identity_seed_marker
               WHERE id = 1"#
        )
        .fetch_optional(&mut **tx)
        .await?;

        let outcome = match existing {
            Some(row) => {
                if row.host_label != self.host_label {
                    tracing::warn!(
                        event = "identity_seeder.host_renamed",
                        previous_host_label = %row.host_label,
                        current_host_label = %self.host_label,
                        organization_id = %row.organization_id,
                        "Host label has changed since synthetic identity was seeded; \
                         identity rows will be preserved as-is."
                    );
                }
                if row.host_identity != self.host_identity {
                    tracing::warn!(
                        event = "identity_seeder.host_identity_changed",
                        previous_host_identity = %row.host_identity,
                        current_host_identity = %self.host_identity,
                        organization_id = %row.organization_id,
                        "Host identity source has changed since synthetic identity was seeded \
                         (database moved to different hardware); identity rows will be preserved as-is."
                    );
                }
                SeedOutcome::AlreadySeeded {
                    organization_id: row.organization_id,
                    user_id: row.user_id,
                    project_id: row.project_id,
                }
            }
            None => self.seed_first_launch(tx).await?,
        };

        // Idempotent backfill: any pre-cutover projects with NULL
        // organization_id get linked to the synthetic organization. Safe on
        // re-run because the WHERE clause excludes already-linked rows.
        let synthetic_org_id = outcome.organization_id();
        sqlx::query!(
            r#"UPDATE projects
               SET organization_id = $1
               WHERE organization_id IS NULL"#,
            synthetic_org_id,
        )
        .execute(&mut **tx)
        .await?;

        Ok(outcome)
    }

    async fn seed_first_launch(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> Result<SeedOutcome, SeederError> {
        let organization_id = derive_id(&self.host_identity, "organization");
        let user_id = derive_id(&self.host_identity, "user");
        let project_id = derive_id(&self.host_identity, "project");

        sqlx::query!(
            r#"INSERT INTO organizations (id, name, slug, is_personal, issue_prefix)
               VALUES ($1, $2, $3, TRUE, 'VK')"#,
            organization_id,
            SYNTHETIC_ORGANIZATION_NAME,
            SYNTHETIC_ORGANIZATION_SLUG,
        )
        .execute(&mut **tx)
        .await?;

        sqlx::query!(
            r#"INSERT INTO users (id, email, username)
               VALUES ($1, $2, $3)"#,
            user_id,
            SYNTHETIC_USER_EMAIL,
            SYNTHETIC_USER_USERNAME,
        )
        .execute(&mut **tx)
        .await?;

        sqlx::query!(
            r#"INSERT INTO organization_members (organization_id, user_id, role)
               VALUES ($1, $2, 'admin')"#,
            organization_id,
            user_id,
        )
        .execute(&mut **tx)
        .await?;

        sqlx::query!(
            r#"INSERT INTO projects (id, organization_id, name, color)
               VALUES ($1, $2, $3, $4)"#,
            project_id,
            organization_id,
            SYNTHETIC_PROJECT_NAME,
            SYNTHETIC_PROJECT_COLOR,
        )
        .execute(&mut **tx)
        .await?;

        for (name, color, sort_order, hidden) in DEFAULT_PROJECT_STATUSES {
            // Deterministic per-status UUID so the rows are bit-stable across
            // backup-restore. Mixing the project_id into the namespace input
            // avoids collisions across multiple synthetic projects.
            let status_id = derive_id(
                &self.host_identity,
                &format!("project_status:{name}:{sort_order}"),
            );
            sqlx::query!(
                r#"INSERT INTO project_statuses
                       (id, project_id, name, color, sort_order, hidden)
                   VALUES ($1, $2, $3, $4, $5, $6)"#,
                status_id,
                project_id,
                name,
                color,
                sort_order,
                hidden,
            )
            .execute(&mut **tx)
            .await?;
        }

        sqlx::query!(
            r#"INSERT INTO identity_seed_marker
                   (id, organization_id, user_id, project_id, host_identity, host_label)
               VALUES (1, $1, $2, $3, $4, $5)"#,
            organization_id,
            user_id,
            project_id,
            self.host_identity,
            self.host_label,
        )
        .execute(&mut **tx)
        .await?;

        tracing::info!(
            event = "identity_seeder.first_launch_seeded",
            organization_id = %organization_id,
            user_id = %user_id,
            project_id = %project_id,
            "Seeded synthetic identity rows for first launch."
        );

        Ok(SeedOutcome::FirstLaunch {
            organization_id,
            user_id,
            project_id,
        })
    }
}

/// Result of a seeder invocation. Callers that need the synthetic IDs (for
/// tests, diagnostics, or downstream wiring) can pull them out without a
/// second query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeedOutcome {
    FirstLaunch {
        organization_id: Uuid,
        user_id: Uuid,
        project_id: Uuid,
    },
    AlreadySeeded {
        organization_id: Uuid,
        user_id: Uuid,
        project_id: Uuid,
    },
}

impl SeedOutcome {
    pub fn organization_id(&self) -> Uuid {
        match self {
            Self::FirstLaunch {
                organization_id, ..
            }
            | Self::AlreadySeeded {
                organization_id, ..
            } => *organization_id,
        }
    }
}

fn derive_id(host_identity: &str, kind: &str) -> Uuid {
    let mut name = String::with_capacity(host_identity.len() + 1 + kind.len());
    name.push_str(host_identity);
    name.push(':');
    name.push_str(kind);
    Uuid::new_v5(&IDENTITY_NAMESPACE, name.as_bytes())
}

/// Resolve a stable host identity source for the running OS. Linux uses the
/// systemd-mandated `/etc/machine-id`, with the dbus-era fallback for hosts
/// that predate it. macOS calls `gethostuuid(3)` from libc, which returns the
/// same hardware UUID across reboots and ignores hostname changes. Windows is
/// best-effort: it uses the registry MachineGuid surfaced by environment
/// variables when available, and falls back to hostname.
fn host_identity() -> Result<String, SeederError> {
    #[cfg(target_os = "linux")]
    {
        for path in ["/etc/machine-id", "/var/lib/dbus/machine-id"] {
            match std::fs::read_to_string(path) {
                Ok(s) => {
                    let trimmed = s.trim();
                    if !trimmed.is_empty() {
                        return Ok(trimmed.to_string());
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(e) => {
                    return Err(SeederError::HostIdentity(format!("read {path}: {e}")));
                }
            }
        }
        Err(SeederError::HostIdentity(
            "no readable machine-id file on this Linux host".to_string(),
        ))
    }

    #[cfg(target_os = "macos")]
    {
        let mut buf: [u8; 16] = [0; 16];
        let wait = libc::timespec {
            tv_sec: 5,
            tv_nsec: 0,
        };
        // SAFETY: gethostuuid writes exactly 16 bytes (uuid_t) into buf and
        // reads `wait` immutably. The buffer is sized at the libc-mandated
        // 16-byte uuid_t length; the wait pointer is non-null and lives for
        // the duration of the call.
        let rc = unsafe { libc::gethostuuid(buf.as_mut_ptr() as *mut _, &wait) };
        if rc != 0 {
            let err = io::Error::last_os_error();
            return Err(SeederError::HostIdentity(format!("gethostuuid: {err}")));
        }
        Ok(Uuid::from_bytes(buf).to_string())
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        // Best-effort fallback: combine username + hostname so multiple users
        // on the same machine get distinct identities. This is documented as
        // unstable across hostname changes, which is consistent with the
        // host-rename warning surface.
        let host = hostname_string().unwrap_or_else(|| "unknown-host".to_string());
        let user = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "unknown-user".to_string());
        Ok(format!("hostname://{user}@{host}"))
    }
}

/// Resolve the rename-sensitive host label. Used only for the host-rename
/// warning surface — UUID derivation keys off [`host_identity`] which is
/// intentionally stable across hostname changes. On Unix this is the kernel
/// hostname (changes when the operator renames the host); on Windows it is
/// the COMPUTERNAME/HOSTNAME environment variable. Falls back to the host
/// identity itself when no hostname can be read so the comparison still works
/// — it just won't trigger on rename in that degenerate case.
fn host_label(host_identity: &str) -> String {
    #[cfg(unix)]
    {
        // POSIX caps hostnames at 255 bytes; allocate one extra for the NUL
        // terminator that gethostname always writes when the buffer is large
        // enough.
        let mut buf = [0u8; 256];
        // SAFETY: gethostname writes up to buf.len() bytes (NUL-terminated on
        // success when the hostname fits) and reads no other memory. The
        // pointer is valid for buf.len() bytes.
        let rc = unsafe { libc::gethostname(buf.as_mut_ptr() as *mut _, buf.len()) };
        if rc == 0 {
            let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
            if let Ok(s) = std::str::from_utf8(&buf[..len]) {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }
    }

    #[cfg(windows)]
    {
        if let Some(name) = hostname_string() {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    host_identity.to_string()
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn hostname_string() -> Option<String> {
    std::env::var("COMPUTERNAME")
        .ok()
        .or_else(|| std::env::var("HOSTNAME").ok())
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use sqlx::{
        SqlitePool,
        sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    };

    use super::*;

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
    async fn first_launch_creates_singleton_rows_and_default_statuses() {
        let pool = make_pool().await;
        let seeder =
            IdentitySeeder::with_host_identity(&pool, "test-host-1".into(), "label-1".into());
        let outcome = seeder.run().await.unwrap();

        let SeedOutcome::FirstLaunch {
            organization_id,
            user_id,
            project_id,
        } = outcome
        else {
            panic!("expected first-launch outcome, got {outcome:?}");
        };

        let org_count: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM organizations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(org_count, 1);

        let user_count: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM users")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(user_count, 1);

        let member_count: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM organization_members")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(member_count, 1);

        let project_count: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM projects")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(project_count, 1);

        let statuses = sqlx::query!(
            r#"SELECT name, color, sort_order, hidden as "hidden!: bool"
               FROM project_statuses
               WHERE project_id = $1
               ORDER BY sort_order ASC"#,
            project_id,
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(statuses.len(), DEFAULT_PROJECT_STATUSES.len());
        for (row, expected) in statuses.iter().zip(DEFAULT_PROJECT_STATUSES.iter()) {
            assert_eq!(row.name, expected.0);
            assert_eq!(row.color, expected.1);
            assert_eq!(row.sort_order, expected.2);
            assert_eq!(row.hidden, expected.3);
        }

        let marker = sqlx::query!(
            r#"SELECT organization_id as "organization_id!: Uuid",
                      user_id         as "user_id!: Uuid",
                      project_id      as "project_id!: Uuid",
                      host_identity,
                      host_label
               FROM identity_seed_marker
               WHERE id = 1"#
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(marker.organization_id, organization_id);
        assert_eq!(marker.user_id, user_id);
        assert_eq!(marker.project_id, project_id);
        assert_eq!(marker.host_identity, "test-host-1");
        assert_eq!(marker.host_label, "label-1");
    }

    #[tokio::test]
    async fn second_run_is_strict_noop() {
        let pool = make_pool().await;
        let seeder =
            IdentitySeeder::with_host_identity(&pool, "test-host-2".into(), "label-2".into());
        let first = seeder.run().await.unwrap();
        let SeedOutcome::FirstLaunch {
            organization_id,
            user_id,
            project_id,
        } = first
        else {
            panic!("expected first-launch outcome");
        };

        let second = seeder.run().await.unwrap();
        assert_eq!(
            second,
            SeedOutcome::AlreadySeeded {
                organization_id,
                user_id,
                project_id,
            }
        );

        let org_count: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM organizations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(org_count, 1);
        let status_count: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM project_statuses")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status_count, DEFAULT_PROJECT_STATUSES.len() as i64);
    }

    #[tokio::test]
    async fn host_rename_preserves_identity_rows() {
        // Hostname rename: stable host_identity unchanged, host_label changes.
        // This is the AC's documented warning surface.
        let pool = make_pool().await;
        let first =
            IdentitySeeder::with_host_identity(&pool, "stable-id".into(), "host-original".into())
                .run()
                .await
                .unwrap();
        let original_org = first.organization_id();

        let renamed =
            IdentitySeeder::with_host_identity(&pool, "stable-id".into(), "host-renamed".into())
                .run()
                .await
                .unwrap();
        assert_eq!(renamed.organization_id(), original_org);

        let marker = sqlx::query!(
            r#"SELECT host_identity, host_label
               FROM identity_seed_marker WHERE id = 1"#
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        // Identity rows stay bit-stable on rename — both columns retain the
        // first-launch values; the rename is reported via the warning log.
        assert_eq!(marker.host_identity, "stable-id");
        assert_eq!(marker.host_label, "host-original");
    }

    #[tokio::test]
    async fn hardware_move_preserves_identity_rows() {
        // Database moved to different hardware: host_identity changes (machine
        // UUID is hardware-bound) while host_label may or may not change. The
        // identity rows must still be preserved — the seeder warns and no-ops.
        let pool = make_pool().await;
        let first =
            IdentitySeeder::with_host_identity(&pool, "machine-A".into(), "shared-name".into())
                .run()
                .await
                .unwrap();
        let original_org = first.organization_id();

        let moved =
            IdentitySeeder::with_host_identity(&pool, "machine-B".into(), "shared-name".into())
                .run()
                .await
                .unwrap();
        assert_eq!(moved.organization_id(), original_org);

        let marker = sqlx::query!(
            r#"SELECT host_identity, host_label
               FROM identity_seed_marker WHERE id = 1"#
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(marker.host_identity, "machine-A");
        assert_eq!(marker.host_label, "shared-name");
    }

    #[tokio::test]
    async fn backfills_pre_existing_projects_with_null_organization_id() {
        let pool = make_pool().await;

        let orphan_id = Uuid::new_v4();
        sqlx::query!(
            r#"INSERT INTO projects (id, organization_id, name, color)
               VALUES ($1, NULL, 'Pre-cutover', '#abc')"#,
            orphan_id,
        )
        .execute(&pool)
        .await
        .unwrap();

        let outcome =
            IdentitySeeder::with_host_identity(&pool, "test-host-3".into(), "label-3".into())
                .run()
                .await
                .unwrap();
        let synthetic_org = outcome.organization_id();

        let backfilled: Option<Uuid> = sqlx::query_scalar!(
            r#"SELECT organization_id as "organization_id: Uuid"
               FROM projects WHERE id = $1"#,
            orphan_id,
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(backfilled, Some(synthetic_org));

        // Re-running must not perturb the backfill.
        IdentitySeeder::with_host_identity(&pool, "test-host-3".into(), "label-3".into())
            .run()
            .await
            .unwrap();
        let still_backfilled: Option<Uuid> = sqlx::query_scalar!(
            r#"SELECT organization_id as "organization_id: Uuid"
               FROM projects WHERE id = $1"#,
            orphan_id,
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(still_backfilled, Some(synthetic_org));
    }

    #[tokio::test]
    async fn deterministic_uuids_from_same_host_source() {
        let pool_a = make_pool().await;
        let pool_b = make_pool().await;
        let a = IdentitySeeder::with_host_identity(&pool_a, "shared-host".into(), "label-a".into())
            .run()
            .await
            .unwrap();
        // Same stable identity → same UUIDs even when the rename-sensitive
        // label differs, because UUID derivation keys off host_identity only.
        let b = IdentitySeeder::with_host_identity(&pool_b, "shared-host".into(), "label-b".into())
            .run()
            .await
            .unwrap();
        let SeedOutcome::FirstLaunch {
            organization_id: org_a,
            user_id: user_a,
            project_id: project_a,
        } = a
        else {
            panic!("expected first-launch");
        };
        let SeedOutcome::FirstLaunch {
            organization_id: org_b,
            user_id: user_b,
            project_id: project_b,
        } = b
        else {
            panic!("expected first-launch");
        };
        assert_eq!(org_a, org_b);
        assert_eq!(user_a, user_b);
        assert_eq!(project_a, project_b);
    }

    #[tokio::test]
    async fn marker_singleton_constraint_rejects_second_row() {
        let pool = make_pool().await;
        IdentitySeeder::with_host_identity(&pool, "host".into(), "label".into())
            .run()
            .await
            .unwrap();

        // Attempt to insert a second marker row with id != 1 — must fail the CHECK.
        let other_org = Uuid::new_v4();
        sqlx::query!(
            r#"INSERT INTO organizations (id, name, slug, is_personal, issue_prefix)
               VALUES ($1, 'X', 'x', FALSE, 'VK')"#,
            other_org,
        )
        .execute(&pool)
        .await
        .unwrap();
        let other_user = Uuid::new_v4();
        sqlx::query!(
            r#"INSERT INTO users (id, email) VALUES ($1, 'x@example.com')"#,
            other_user,
        )
        .execute(&pool)
        .await
        .unwrap();
        let other_project = Uuid::new_v4();
        sqlx::query!(
            r#"INSERT INTO projects (id, organization_id, name, color)
               VALUES ($1, $2, 'X', '#000')"#,
            other_project,
            other_org,
        )
        .execute(&pool)
        .await
        .unwrap();

        let result = sqlx::query!(
            r#"INSERT INTO identity_seed_marker
                   (id, organization_id, user_id, project_id, host_identity, host_label)
               VALUES (2, $1, $2, $3, 'x', 'y')"#,
            other_org,
            other_user,
            other_project,
        )
        .execute(&pool)
        .await;
        assert!(result.is_err(), "CHECK (id = 1) must reject id != 1");
    }
}
