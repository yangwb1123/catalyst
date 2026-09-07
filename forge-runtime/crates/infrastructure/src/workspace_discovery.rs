use std::{collections::BTreeSet, ffi::OsStr, path::Path, sync::Arc};

use cap_std::fs::{Dir, DirEntry};
use serde::Serialize;

use crate::runtime_domain::{Cancellation, ToolError};

pub(crate) const DEFAULT_MAX_DEPTH: usize = 8;
pub(crate) const DEFAULT_MAX_ENTRIES: usize = 1_000;
pub(crate) const MAX_TOOL_OUTPUT_BYTES: usize = 1024 * 1024;
const MAX_DEPTH: usize = 16;
const MAX_ENTRIES: usize = 4_096;
const MAX_PATH_BYTES: usize = 4_096;
const MAX_START_COMPONENTS: usize = 64;

pub(crate) struct WalkOptions {
    start: String,
    max_depth: usize,
    max_entries: usize,
}

pub(crate) struct WalkSummary {
    pub(crate) reasons: BTreeSet<IncompleteReason>,
    pub(crate) stats: WalkStats,
}

#[derive(Clone, Copy)]
pub(crate) enum WalkDirective {
    Continue,
    Stop(IncompleteReason),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IncompleteReason {
    DepthLimit,
    EntryLimit,
    NonUtf8Name,
    PathLimit,
    OversizedFile,
    ResultLimit,
    ScanByteLimit,
    LineOutputLimit,
    OutputLimit,
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct WalkStats {
    pub(crate) visited_entries: usize,
    pub(crate) skipped_symlinks: usize,
    pub(crate) skipped_git_metadata_entries: usize,
    pub(crate) skipped_non_utf8_names: usize,
    pub(crate) skipped_overlong_paths: usize,
}

struct WalkState<'a> {
    cancellation: &'a Cancellation,
    max_depth: usize,
    max_entries: usize,
    reasons: BTreeSet<IncompleteReason>,
    stats: WalkStats,
    stopped: bool,
}

impl WalkOptions {
    pub(crate) fn new(
        start: Option<String>,
        max_depth: Option<usize>,
        max_entries: Option<usize>,
    ) -> Result<Self, ToolError> {
        let start = start.unwrap_or_else(|| ".".to_owned());
        validate_start(&start)?;
        let max_depth = max_depth.unwrap_or(DEFAULT_MAX_DEPTH);
        let max_entries = max_entries.unwrap_or(DEFAULT_MAX_ENTRIES);
        if !(1..=MAX_DEPTH).contains(&max_depth) {
            return Err(invalid_arguments("max_depth must be within 1..=16"));
        }
        if !(1..=MAX_ENTRIES).contains(&max_entries) {
            return Err(invalid_arguments("max_entries must be within 1..=4096"));
        }
        Ok(Self {
            start,
            max_depth,
            max_entries,
        })
    }
}

pub(crate) fn walk_regular_files<F>(
    workspace: &Arc<Dir>,
    options: &WalkOptions,
    cancellation: &Cancellation,
    mut visit: F,
) -> Result<WalkSummary, ToolError>
where
    F: FnMut(&str, &Dir, &OsStr) -> Result<WalkDirective, ToolError>,
{
    ensure_active(cancellation)?;
    let (start, prefix) = open_start(workspace, &options.start, cancellation)?;
    let mut state = WalkState {
        cancellation,
        max_depth: options.max_depth,
        max_entries: options.max_entries,
        reasons: BTreeSet::new(),
        stats: WalkStats::default(),
        stopped: false,
    };
    walk_directory(&start, &prefix, 0, &mut state, &mut visit)?;
    Ok(WalkSummary {
        reasons: state.reasons,
        stats: state.stats,
    })
}

fn walk_directory<F>(
    directory: &Dir,
    prefix: &str,
    depth: usize,
    state: &mut WalkState<'_>,
    visit: &mut F,
) -> Result<(), ToolError>
where
    F: FnMut(&str, &Dir, &OsStr) -> Result<WalkDirective, ToolError>,
{
    let entries = sorted_entries(directory, state)?;
    for entry in entries {
        ensure_active(state.cancellation)?;
        if state.stopped {
            break;
        }
        visit_entry(directory, &entry, prefix, depth, state, visit)?;
    }
    Ok(())
}

fn visit_entry<F>(
    directory: &Dir,
    entry: &DirEntry,
    prefix: &str,
    depth: usize,
    state: &mut WalkState<'_>,
    visit: &mut F,
) -> Result<(), ToolError>
where
    F: FnMut(&str, &Dir, &OsStr) -> Result<WalkDirective, ToolError>,
{
    let raw_name = entry.file_name();
    let Some(name) = raw_name.to_str() else {
        state.stats.skipped_non_utf8_names += 1;
        state.reasons.insert(IncompleteReason::NonUtf8Name);
        return Ok(());
    };
    if name.eq_ignore_ascii_case(".git") {
        state.stats.skipped_git_metadata_entries += 1;
        return Ok(());
    }
    let path = joined_path(prefix, name);
    if path.len() > MAX_PATH_BYTES {
        state.stats.skipped_overlong_paths += 1;
        state.reasons.insert(IncompleteReason::PathLimit);
        return Ok(());
    }
    let file_type = entry
        .file_type()
        .map_err(|error| scan_failed(error.to_string()))?;
    if file_type.is_symlink() {
        state.stats.skipped_symlinks += 1;
        return Ok(());
    }
    if file_type.is_file() {
        apply_directive(visit(&path, directory, &raw_name)?, state);
    } else if file_type.is_dir() {
        visit_directory(directory, entry, &raw_name, &path, depth + 1, state, visit)?;
    }
    Ok(())
}

fn visit_directory<F>(
    parent: &Dir,
    entry: &DirEntry,
    name: &OsStr,
    path: &str,
    depth: usize,
    state: &mut WalkState<'_>,
    visit: &mut F,
) -> Result<(), ToolError>
where
    F: FnMut(&str, &Dir, &OsStr) -> Result<WalkDirective, ToolError>,
{
    if depth >= state.max_depth {
        state.reasons.insert(IncompleteReason::DepthLimit);
        return Ok(());
    }
    let child = open_child_directory(parent, entry, name)?;
    walk_directory(&child, path, depth, state, visit)
}

#[cfg(all(unix, not(target_os = "redox")))]
fn open_child_directory(parent: &Dir, entry: &DirEntry, name: &OsStr) -> Result<Dir, ToolError> {
    let expected = entry
        .metadata()
        .map_err(|error| scan_failed(error.to_string()))?;
    open_named_directory(parent, name, &expected)
}

#[cfg(all(unix, not(target_os = "redox")))]
fn open_named_directory(
    parent: &Dir,
    name: &OsStr,
    expected: &cap_std::fs::Metadata,
) -> Result<Dir, ToolError> {
    use cap_std::fs::MetadataExt as _;

    let descriptor = rustix::fs::openat(
        parent,
        name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
    .map_err(|error| scan_failed(error.to_string()))?;
    let directory = Dir::from(descriptor);
    let actual = directory
        .dir_metadata()
        .map_err(|error| scan_failed(error.to_string()))?;
    if expected.dev() != actual.dev() || expected.ino() != actual.ino() {
        return Err(scan_failed("directory changed during discovery".into()));
    }
    Ok(directory)
}

#[cfg(any(not(unix), target_os = "redox"))]
fn open_child_directory(_parent: &Dir, entry: &DirEntry, _name: &OsStr) -> Result<Dir, ToolError> {
    entry
        .open_dir()
        .map_err(|error| scan_failed(error.to_string()))
}

#[cfg(any(not(unix), target_os = "redox"))]
fn open_named_directory(
    parent: &Dir,
    name: &OsStr,
    _expected: &cap_std::fs::Metadata,
) -> Result<Dir, ToolError> {
    parent
        .open_dir(name)
        .map_err(|error| scan_failed(error.to_string()))
}

fn sorted_entries(directory: &Dir, state: &mut WalkState<'_>) -> Result<Vec<DirEntry>, ToolError> {
    let mut entries = Vec::new();
    let remaining = state.max_entries - state.stats.visited_entries;
    let iterator = directory
        .entries()
        .map_err(|error| scan_failed(error.to_string()))?;
    for result in iterator {
        ensure_active(state.cancellation)?;
        if entries.len() == remaining {
            state.stats.visited_entries += entries.len();
            state.reasons.insert(IncompleteReason::EntryLimit);
            entries.sort_by_key(cap_std::fs::DirEntry::file_name);
            return Ok(entries);
        }
        entries.push(result.map_err(|error| scan_failed(error.to_string()))?);
    }
    state.stats.visited_entries += entries.len();
    entries.sort_by_key(cap_std::fs::DirEntry::file_name);
    Ok(entries)
}

fn open_start(
    workspace: &Arc<Dir>,
    start: &str,
    cancellation: &Cancellation,
) -> Result<(Dir, String), ToolError> {
    let mut directory = workspace
        .try_clone()
        .map_err(|error| scan_failed(error.to_string()))?;
    let mut prefix = String::new();
    for component in Path::new(start).components() {
        ensure_active(cancellation)?;
        let std::path::Component::Normal(name) = component else {
            continue;
        };
        let metadata = directory
            .symlink_metadata(name)
            .map_err(|error| ToolError::new("path_unavailable", error.to_string()))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(ToolError::new(
                "path_denied",
                "start path must resolve through regular workspace directories",
            ));
        }
        directory = open_named_directory(&directory, name, &metadata)
            .map_err(|error| ToolError::new("path_denied", error.to_string()))?;
        prefix = joined_path(&prefix, &name.to_string_lossy());
    }
    Ok((directory, prefix))
}

fn validate_start(start: &str) -> Result<(), ToolError> {
    let path = Path::new(start);
    if start.is_empty() || start.len() > MAX_PATH_BYTES || path.is_absolute() {
        return Err(invalid_path());
    }
    let mut component_count = 0;
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::Normal(name) if !is_git_metadata(name) => {}
            _ => return Err(invalid_path()),
        }
        component_count += usize::from(matches!(component, std::path::Component::Normal(_)));
    }
    if component_count > MAX_START_COMPONENTS {
        return Err(invalid_path());
    }
    Ok(())
}

fn apply_directive(directive: WalkDirective, state: &mut WalkState<'_>) {
    if let WalkDirective::Stop(reason) = directive {
        state.reasons.insert(reason);
        state.stopped = true;
    }
}

fn joined_path(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_owned()
    } else {
        format!("{prefix}/{name}")
    }
}

fn is_git_metadata(name: &OsStr) -> bool {
    name.to_str()
        .is_some_and(|name| name.eq_ignore_ascii_case(".git"))
}

pub(crate) fn ensure_active(cancellation: &Cancellation) -> Result<(), ToolError> {
    if cancellation.is_cancelled() {
        Err(ToolError::new("cancelled", "run was cancelled"))
    } else {
        Ok(())
    }
}

fn invalid_arguments(message: &str) -> ToolError {
    ToolError::new("invalid_arguments", message)
}

fn invalid_path() -> ToolError {
    ToolError::new(
        "invalid_path",
        "path must be workspace-relative, traversal-free, outside .git metadata, and at most 4096 bytes",
    )
}

fn scan_failed(message: String) -> ToolError {
    ToolError::new("scan_failed", message)
}
