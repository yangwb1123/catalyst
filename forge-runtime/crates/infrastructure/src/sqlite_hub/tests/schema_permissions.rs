use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
};

use super::{open_database, verify_private_directory_permissions};

#[test]
fn generated_wal_and_shm_files_are_private() {
    let root = tempfile::tempdir().expect("private state directory");
    let database = root.path().join("hub.sqlite3");
    let connection = open_database(&database).expect("open Hub database");
    let journal_mode: String = connection
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .expect("query journal mode");

    assert_eq!(journal_mode, "wal");
    for suffix in ["-wal", "-shm"] {
        let auxiliary = auxiliary_path(&database, suffix);
        let mode = std::fs::metadata(&auxiliary)
            .unwrap_or_else(|error| panic!("{} unavailable: {error}", auxiliary.display()))
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "{} must be private", auxiliary.display());
    }
}

#[test]
fn stale_shared_metadata_accepts_directory_restricted_by_concurrent_opener() {
    let root = tempfile::tempdir().expect("temporary root");
    let state = root.path().join("state");
    fs::create_dir(&state).expect("state directory");
    fs::set_permissions(&state, fs::Permissions::from_mode(0o775))
        .expect("initial shared permissions");
    let stale_metadata = fs::symlink_metadata(&state).expect("shared metadata");

    fs::set_permissions(&state, fs::Permissions::from_mode(0o700))
        .expect("concurrent opener restricts permissions");
    fs::write(state.join("hub.sqlite3"), []).expect("concurrent opener creates database");

    verify_private_directory_permissions(&state, &stale_metadata)
        .expect("current private permissions supersede stale metadata");
    assert_eq!(
        fs::symlink_metadata(&state)
            .expect("current metadata")
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
}

#[test]
fn stale_shared_metadata_rejects_symlink_replacement_without_chmod_target() {
    let root = tempfile::tempdir().expect("temporary root");
    let state = root.path().join("state");
    fs::create_dir(&state).expect("state directory");
    fs::set_permissions(&state, fs::Permissions::from_mode(0o775))
        .expect("initial shared permissions");
    let stale_metadata = fs::symlink_metadata(&state).expect("shared metadata");

    fs::remove_dir(&state).expect("remove original state directory");
    let replacement = root.path().join("replacement");
    fs::create_dir(&replacement).expect("replacement directory");
    fs::set_permissions(&replacement, fs::Permissions::from_mode(0o755))
        .expect("replacement permissions");
    symlink(&replacement, &state).expect("replace state path with symlink");

    verify_private_directory_permissions(&state, &stale_metadata)
        .expect_err("symlink replacement must be rejected");
    assert_eq!(
        fs::symlink_metadata(&replacement)
            .expect("replacement metadata")
            .permissions()
            .mode()
            & 0o777,
        0o755,
        "symlink target permissions must remain unchanged"
    );
}

fn auxiliary_path(database: &Path, suffix: &str) -> PathBuf {
    let mut path = database.as_os_str().to_os_string();
    path.push(suffix);
    path.into()
}
