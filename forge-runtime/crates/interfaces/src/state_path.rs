use std::{
    env, fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static IDEMPOTENCY_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn hub_database_path(override_dir: Option<&Path>) -> Result<PathBuf, io::Error> {
    let state_dir = override_dir
        .map(Path::to_path_buf)
        .or_else(default_state_dir)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "cannot determine Hub state directory; pass --state-dir PATH",
            )
        })?;
    Ok(state_dir.join("hub.sqlite3"))
}

pub fn canonical_project(path: &Path) -> Result<PathBuf, io::Error> {
    let canonical = fs::canonicalize(path)?;
    if !fs::metadata(&canonical)?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("project path is not a directory: {}", path.display()),
        ));
    }
    Ok(canonical)
}

pub fn idempotency_key(operation: &str) -> String {
    unique_id(&format!("cli-{operation}"))
}

pub fn unique_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let counter = IDEMPOTENCY_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{}-{nanos}-{counter}", std::process::id())
}

pub fn unix_time_millis() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis());
    u64::try_from(millis).unwrap_or(u64::MAX)
}

fn default_state_dir() -> Option<PathBuf> {
    non_empty_env("FORGE_RUNTIME_HOME")
        .or_else(|| non_empty_env("XDG_STATE_HOME").map(|path| path.join("forgeos")))
        .or_else(platform_state_dir)
}

#[cfg(windows)]
fn platform_state_dir() -> Option<PathBuf> {
    non_empty_env("LOCALAPPDATA").map(|path| path.join("ForgeOS"))
}

#[cfg(not(windows))]
fn platform_state_dir() -> Option<PathBuf> {
    non_empty_env("HOME").map(|path| path.join(".local/state/forgeos"))
}

fn non_empty_env(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::{
        canonical_project, hub_database_path, idempotency_key, unique_id, unix_time_millis,
    };

    #[test]
    fn an_explicit_state_directory_is_deterministic() {
        let root = TempDir::new().expect("temporary root");
        assert_eq!(
            hub_database_path(Some(root.path())).expect("database path"),
            root.path().join("hub.sqlite3")
        );
    }

    #[test]
    fn project_paths_must_exist_and_be_directories() {
        let root = TempDir::new().expect("temporary root");
        let file = root.path().join("file");
        fs::write(&file, "not a project").expect("fixture file");
        assert!(canonical_project(&file).is_err());
        assert_eq!(
            canonical_project(root.path()).expect("directory canonicalizes"),
            root.path().canonicalize().expect("expected path")
        );
    }

    #[test]
    fn generated_idempotency_keys_do_not_repeat_in_process() {
        assert_ne!(idempotency_key("prompt"), idempotency_key("prompt"));
        assert_ne!(unique_id("run"), unique_id("run"));
        assert!(unix_time_millis() > 0);
    }
}
