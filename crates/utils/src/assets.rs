use std::{
    io,
    path::{Path, PathBuf},
};

use directories::ProjectDirs;
use rust_embed::RustEmbed;
use thiserror::Error;

const PROJECT_ROOT: &str = env!("CARGO_MANIFEST_DIR");

pub const DB_V2_FILE_NAME: &str = "db.v2.sqlite";
pub const DB_V3_FILE_NAME: &str = "db.v3.sqlite";
pub const DB_V2_ROLLBACK_JOURNAL_FILE_NAME: &str = "db.v2.sqlite-journal";
const DB_V3_TMP_FILE_NAME: &str = "db.v3.sqlite.cutover.tmp";

pub fn asset_dir() -> std::path::PathBuf {
    let path = if cfg!(debug_assertions) {
        std::path::PathBuf::from(PROJECT_ROOT).join("../../dev_assets")
    } else {
        prod_asset_dir_path()
    };

    // Ensure the directory exists
    if !path.exists() {
        std::fs::create_dir_all(&path).expect("Failed to create asset directory");
    }

    path
    // ✔ macOS → ~/Library/Application Support/MyApp
    // ✔ Linux → ~/.local/share/myapp   (respects XDG_DATA_HOME)
    // ✔ Windows → %APPDATA%\Example\MyApp
}

pub fn prod_asset_dir_path() -> std::path::PathBuf {
    ProjectDirs::from("ai", "bloop", "vibe-kanban")
        .expect("OS didn't give us a home directory")
        .data_dir()
        .to_path_buf()
}

pub fn db_v3_path() -> std::path::PathBuf {
    asset_dir().join(DB_V3_FILE_NAME)
}

pub fn config_path() -> std::path::PathBuf {
    asset_dir().join("config.json")
}

pub fn profiles_path() -> std::path::PathBuf {
    asset_dir().join("profiles.json")
}

pub fn credentials_path() -> std::path::PathBuf {
    asset_dir().join("credentials.json")
}

pub fn trusted_keys_path() -> std::path::PathBuf {
    asset_dir().join("trusted_ed25519_public_keys.json")
}

pub fn server_signing_key_path() -> std::path::PathBuf {
    asset_dir().join("server_ed25519_signing_key")
}

pub fn relay_host_credentials_path() -> std::path::PathBuf {
    asset_dir().join("relay_host_credentials.json")
}

#[derive(RustEmbed)]
#[folder = "../../assets/sounds"]
pub struct SoundAssets;

#[derive(RustEmbed)]
#[folder = "../../assets/scripts"]
pub struct ScriptAssets;

#[derive(Debug, Eq, PartialEq)]
pub enum CutoverOutcome {
    SkippedV3Exists,
    SkippedCleanInstall,
    Copied { bytes: u64 },
}

#[derive(Debug, Error)]
pub enum CutoverError {
    #[error(
        "refusing v2→v3 cutover-copy: hot rollback journal beside v2 ({journal_path:?}, {journal_size} bytes)"
    )]
    HotJournalPresent {
        journal_path: PathBuf,
        journal_size: u64,
    },
    #[error("v2→v3 cutover-copy failed writing tmp ({tmp:?}): {source}")]
    CopyFailed {
        tmp: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("v2→v3 cutover-copy failed renaming {tmp:?} to {v3:?}: {source}")]
    RenameFailed {
        tmp: PathBuf,
        v3: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// Run the v2→v3 cutover-copy using the production asset directory.
///
/// See [`cutover_copy_v2_to_v3`] for the per-path semantics.
pub fn ensure_db_v3() -> Result<CutoverOutcome, CutoverError> {
    let dir = asset_dir();
    cutover_copy_v2_to_v3(
        &dir.join(DB_V2_FILE_NAME),
        &dir.join(DB_V3_FILE_NAME),
        &dir.join(DB_V2_ROLLBACK_JOURNAL_FILE_NAME),
        &dir.join(DB_V3_TMP_FILE_NAME),
    )
}

/// First-launch cutover from v2 to v3.
///
/// Behavior:
/// - if `v3` exists: skip (idempotent re-launch),
/// - else if `v2` does not exist: skip (clean install — caller will create v3 fresh),
/// - else if `journal` exists with non-zero size: refuse with [`CutoverError::HotJournalPresent`]
///   (the source database was not quiesced; rolling forward would corrupt v3),
/// - else: copy `v2` to `tmp` then atomically rename `tmp` to `v3`. `v2` is read-only
///   throughout, so it is preserved byte-identical even if the copy fails.
pub fn cutover_copy_v2_to_v3(
    v2: &Path,
    v3: &Path,
    journal: &Path,
    tmp: &Path,
) -> Result<CutoverOutcome, CutoverError> {
    if v3.exists() {
        tracing::info!(
            event = "v2_to_v3_cutover_skipped",
            reason = "v3_exists",
            v3_path = %v3.display(),
            "Skipping v2→v3 cutover-copy: v3 already exists"
        );
        return Ok(CutoverOutcome::SkippedV3Exists);
    }

    if !v2.exists() {
        tracing::info!(
            event = "v2_to_v3_cutover_skipped",
            reason = "clean_install",
            v2_path = %v2.display(),
            "Skipping v2→v3 cutover-copy: clean install (v2 absent)"
        );
        return Ok(CutoverOutcome::SkippedCleanInstall);
    }

    if let Ok(meta) = std::fs::metadata(journal)
        && meta.len() > 0
    {
        let size = meta.len();
        tracing::error!(
            event = "v2_to_v3_cutover_refused",
            reason = "hot_journal_present",
            v2_path = %v2.display(),
            journal_path = %journal.display(),
            journal_size = size,
            "Refusing v2→v3 cutover-copy: hot rollback journal beside v2"
        );
        return Err(CutoverError::HotJournalPresent {
            journal_path: journal.to_path_buf(),
            journal_size: size,
        });
    }

    tracing::info!(
        event = "v2_to_v3_cutover_started",
        v2_path = %v2.display(),
        v3_path = %v3.display(),
        tmp_path = %tmp.display(),
        "Starting v2→v3 cutover-copy"
    );

    let bytes = match std::fs::copy(v2, tmp) {
        Ok(bytes) => bytes,
        Err(source) => {
            // Best-effort cleanup of any partial tmp so a retry starts clean.
            let _ = std::fs::remove_file(tmp);
            tracing::error!(
                event = "v2_to_v3_cutover_failed",
                stage = "copy",
                tmp_path = %tmp.display(),
                error = %source,
                "v2→v3 cutover-copy failed writing tmp"
            );
            return Err(CutoverError::CopyFailed {
                tmp: tmp.to_path_buf(),
                source,
            });
        }
    };

    if let Err(source) = std::fs::rename(tmp, v3) {
        let _ = std::fs::remove_file(tmp);
        tracing::error!(
            event = "v2_to_v3_cutover_failed",
            stage = "rename",
            tmp_path = %tmp.display(),
            v3_path = %v3.display(),
            error = %source,
            "v2→v3 cutover-copy failed renaming tmp to v3"
        );
        return Err(CutoverError::RenameFailed {
            tmp: tmp.to_path_buf(),
            v3: v3.to_path_buf(),
            source,
        });
    }

    tracing::info!(
        event = "v2_to_v3_cutover_completed",
        v3_path = %v3.display(),
        bytes,
        "Completed v2→v3 cutover-copy"
    );
    Ok(CutoverOutcome::Copied { bytes })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use tempfile::TempDir;

    use super::*;

    struct CutoverPaths {
        _dir: TempDir,
        v2: PathBuf,
        v3: PathBuf,
        journal: PathBuf,
        tmp: PathBuf,
    }

    impl CutoverPaths {
        fn new() -> Self {
            let dir = TempDir::new().unwrap();
            let v2 = dir.path().join(DB_V2_FILE_NAME);
            let v3 = dir.path().join(DB_V3_FILE_NAME);
            let journal = dir.path().join(DB_V2_ROLLBACK_JOURNAL_FILE_NAME);
            let tmp = dir.path().join(DB_V3_TMP_FILE_NAME);
            Self {
                _dir: dir,
                v2,
                v3,
                journal,
                tmp,
            }
        }

        fn run(&self) -> Result<CutoverOutcome, CutoverError> {
            cutover_copy_v2_to_v3(&self.v2, &self.v3, &self.journal, &self.tmp)
        }
    }

    fn write(path: &Path, bytes: &[u8]) {
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn upgrade_install_copies_v2_to_v3_and_preserves_v2() {
        let paths = CutoverPaths::new();
        let payload = b"v2 contents \xDE\xAD\xBE\xEF";
        write(&paths.v2, payload);

        let outcome = paths.run().unwrap();

        assert_eq!(
            outcome,
            CutoverOutcome::Copied {
                bytes: payload.len() as u64
            }
        );
        assert_eq!(fs::read(&paths.v3).unwrap(), payload);
        assert_eq!(fs::read(&paths.v2).unwrap(), payload);
        assert!(!paths.tmp.exists(), "tmp should be renamed away on success");
    }

    #[test]
    fn clean_install_skips_when_neither_file_exists() {
        let paths = CutoverPaths::new();

        let outcome = paths.run().unwrap();

        assert_eq!(outcome, CutoverOutcome::SkippedCleanInstall);
        assert!(!paths.v3.exists(), "v3 must be left absent for SQLite");
        assert!(!paths.v2.exists());
    }

    #[test]
    fn relaunch_skips_when_v3_already_exists() {
        let paths = CutoverPaths::new();
        write(&paths.v2, b"v2 contents that must not bleed into v3");
        write(&paths.v3, b"existing v3 contents");

        let outcome = paths.run().unwrap();

        assert_eq!(outcome, CutoverOutcome::SkippedV3Exists);
        assert_eq!(fs::read(&paths.v3).unwrap(), b"existing v3 contents");
    }

    #[test]
    fn hot_journal_refusal_triggers_when_journal_nonempty() {
        let paths = CutoverPaths::new();
        write(&paths.v2, b"v2 was not quiesced");
        write(&paths.journal, b"\x01"); // any non-empty rollback journal

        let err = paths.run().unwrap_err();

        match err {
            CutoverError::HotJournalPresent {
                journal_path,
                journal_size,
            } => {
                assert_eq!(journal_path, paths.journal);
                assert_eq!(journal_size, 1);
            }
            other => panic!("expected HotJournalPresent, got {other:?}"),
        }
        assert!(!paths.v3.exists(), "v3 must not be created on refusal");
        assert_eq!(fs::read(&paths.v2).unwrap(), b"v2 was not quiesced");
    }

    #[test]
    fn cutover_succeeds_after_journal_is_cleared() {
        let paths = CutoverPaths::new();
        write(&paths.v2, b"v2 contents");
        write(&paths.journal, b"\x01");

        let _refused = paths.run().unwrap_err();
        assert!(!paths.v3.exists());

        // Operator clears the journal — cutover should now succeed.
        fs::remove_file(&paths.journal).unwrap();
        let outcome = paths.run().unwrap();
        assert_eq!(outcome, CutoverOutcome::Copied { bytes: 11 });
        assert_eq!(fs::read(&paths.v3).unwrap(), b"v2 contents");
    }

    #[test]
    fn zero_byte_journal_does_not_block_cutover() {
        // A clean shutdown in PERSIST journal mode would leave an empty
        // -journal file. That is not a hot journal — do not refuse.
        let paths = CutoverPaths::new();
        write(&paths.v2, b"v2 contents");
        write(&paths.journal, b"");

        let outcome = paths.run().unwrap();

        assert_eq!(outcome, CutoverOutcome::Copied { bytes: 11 });
        assert_eq!(fs::read(&paths.v3).unwrap(), b"v2 contents");
    }

    #[test]
    fn copy_failure_leaves_v2_untouched_and_no_v3() {
        // Simulate a copy-stage failure by pointing tmp at a path under a
        // directory that does not exist. v2 must remain byte-identical.
        let dir = TempDir::new().unwrap();
        let v2 = dir.path().join(DB_V2_FILE_NAME);
        let v3 = dir.path().join(DB_V3_FILE_NAME);
        let journal = dir.path().join(DB_V2_ROLLBACK_JOURNAL_FILE_NAME);
        let tmp = dir.path().join("does_not_exist").join(DB_V3_TMP_FILE_NAME);
        let payload = b"v2 must not be touched";
        write(&v2, payload);

        let err = cutover_copy_v2_to_v3(&v2, &v3, &journal, &tmp).unwrap_err();

        assert!(matches!(err, CutoverError::CopyFailed { .. }));
        assert_eq!(fs::read(&v2).unwrap(), payload);
        assert!(!v3.exists());
        assert!(!tmp.exists());
    }
}
