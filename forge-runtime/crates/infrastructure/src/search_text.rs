use std::{collections::BTreeSet, ffi::OsStr, io::Read, path::Path, sync::Arc};

use cap_std::{ambient_authority, fs::Dir};
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    runtime_domain::{
        AgentTool, Cancellation, Capability, ToolContext, ToolError, ToolFuture, ToolOutput,
        ToolSpec,
    },
    workspace_discovery::{
        IncompleteReason, MAX_TOOL_OUTPUT_BYTES, WalkDirective, WalkOptions, WalkStats,
        ensure_active, walk_regular_files,
    },
};

const DEFAULT_MAX_RESULTS: usize = 100;
const MAX_RESULTS: usize = 512;
const MAX_QUERY_BYTES: usize = 1_024;
const MAX_FILE_SCAN_BYTES: usize = 1024 * 1024;
const MAX_TOTAL_SCAN_BYTES: usize = 8 * 1024 * 1024;
const MAX_LINE_OUTPUT_BYTES: usize = 512;

#[derive(Clone)]
pub struct SearchTextTool {
    workspace: Arc<Dir>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SearchTextInput {
    query: String,
    path: Option<String>,
    max_depth: Option<usize>,
    max_entries: Option<usize>,
    max_results: Option<usize>,
}

#[derive(Serialize)]
struct SearchTextOutput<'a> {
    matches: &'a [SearchMatch],
    complete: bool,
    reasons: &'a BTreeSet<IncompleteReason>,
    stats: &'a SearchStats,
}

#[derive(Debug, Serialize)]
struct SearchMatch {
    path: String,
    line: usize,
    text: String,
    text_truncated: bool,
}

struct SearchState<'a> {
    query: &'a str,
    cancellation: &'a Cancellation,
    matches: Vec<SearchMatch>,
    remaining_scan_bytes: usize,
    max_results: usize,
    reasons: BTreeSet<IncompleteReason>,
    files_scanned: usize,
    bytes_scanned: usize,
    skipped_oversized_files: usize,
    skipped_non_utf8_files: usize,
    skipped_binary_files: usize,
}

#[derive(Serialize)]
struct SearchStats {
    visited_entries: usize,
    skipped_symlinks: usize,
    skipped_git_metadata_entries: usize,
    skipped_non_utf8_names: usize,
    skipped_overlong_paths: usize,
    files_scanned: usize,
    bytes_scanned: usize,
    skipped_oversized_files: usize,
    skipped_non_utf8_files: usize,
    skipped_binary_files: usize,
}

impl SearchTextTool {
    /// Opens a text-search capability anchored to `workspace`.
    ///
    /// # Errors
    /// Returns `workspace_unavailable` when the directory cannot be opened.
    pub fn open(workspace: &Path) -> Result<Self, ToolError> {
        let directory = Dir::open_ambient_dir(workspace, ambient_authority())
            .map_err(|error| ToolError::new("workspace_unavailable", error.to_string()))?;
        Ok(Self::from_anchored(Arc::new(directory)))
    }

    pub(crate) fn from_anchored(workspace: Arc<Dir>) -> Self {
        Self { workspace }
    }
}

impl AgentTool for SearchTextTool {
    fn spec(&self) -> ToolSpec {
        let schema = serde_json::to_value(schema_for!(SearchTextInput))
            .expect("generated search-text schema is serializable");
        ToolSpec {
            name: "search_text".into(),
            description: "Find an exact literal on bounded UTF-8 text lines without following symbolic links or entering .git metadata; return completeness reasons and scan statistics.".into(),
            input_schema: schema,
            capability: Capability::WorkspaceRead,
        }
    }

    fn execute(&self, arguments: Value, context: ToolContext) -> ToolFuture<'_> {
        let workspace = self.workspace.clone();
        Box::pin(async move {
            let (query, options, max_results) = parse_input(arguments)?;
            let cancellation = context.cancellation;
            let output_limit = context.max_output_bytes.min(MAX_TOOL_OUTPUT_BYTES);
            tokio::task::spawn_blocking(move || {
                search_blocking(
                    &workspace,
                    &query,
                    &options,
                    max_results,
                    &cancellation,
                    output_limit,
                )
            })
            .await
            .map_err(|error| ToolError::new("search_task_failed", error.to_string()))?
        })
    }
}

fn parse_input(arguments: Value) -> Result<(String, WalkOptions, usize), ToolError> {
    let input: SearchTextInput = serde_json::from_value(arguments)
        .map_err(|error| ToolError::new("invalid_arguments", error.to_string()))?;
    validate_query(&input.query)?;
    let max_results = input.max_results.unwrap_or(DEFAULT_MAX_RESULTS);
    if !(1..=MAX_RESULTS).contains(&max_results) {
        return Err(invalid_arguments("max_results must be within 1..=512"));
    }
    let options = WalkOptions::new(input.path, input.max_depth, input.max_entries)?;
    Ok((input.query, options, max_results))
}

fn search_blocking(
    workspace: &Arc<Dir>,
    query: &str,
    options: &WalkOptions,
    max_results: usize,
    cancellation: &Cancellation,
    output_limit: usize,
) -> Result<ToolOutput, ToolError> {
    let mut state = SearchState {
        query,
        cancellation,
        matches: Vec::new(),
        remaining_scan_bytes: MAX_TOTAL_SCAN_BYTES,
        max_results,
        reasons: BTreeSet::new(),
        files_scanned: 0,
        bytes_scanned: 0,
        skipped_oversized_files: 0,
        skipped_non_utf8_files: 0,
        skipped_binary_files: 0,
    };
    let summary = walk_regular_files(workspace, options, cancellation, |path, parent, name| {
        inspect_file(path, parent, name, &mut state)
    })?;
    state
        .matches
        .sort_by(|left, right| (&left.path, left.line).cmp(&(&right.path, right.line)));
    state.reasons.extend(summary.reasons);
    let discovery_stats = search_stats(&state, &summary.stats);
    encode_output(
        &state.matches,
        &state.reasons,
        &discovery_stats,
        output_limit,
    )
}

fn inspect_file(
    path: &str,
    parent: &Dir,
    name: &OsStr,
    state: &mut SearchState<'_>,
) -> Result<WalkDirective, ToolError> {
    ensure_active(state.cancellation)?;
    if state.remaining_scan_bytes <= 1 {
        return Ok(WalkDirective::Stop(IncompleteReason::ScanByteLimit));
    }
    let Some(bytes) = read_candidate(parent, name, state)? else {
        return Ok(WalkDirective::Continue);
    };
    let Ok(content) = std::str::from_utf8(&bytes) else {
        state.skipped_non_utf8_files += 1;
        return Ok(WalkDirective::Continue);
    };
    if content.as_bytes().contains(&0) {
        state.skipped_binary_files += 1;
        return Ok(WalkDirective::Continue);
    }
    for (index, line) in content.lines().enumerate() {
        ensure_active(state.cancellation)?;
        if line.contains(state.query) && !push_match(path, index + 1, line, state) {
            return Ok(WalkDirective::Stop(IncompleteReason::ResultLimit));
        }
    }
    Ok(WalkDirective::Continue)
}

fn read_candidate(
    parent: &Dir,
    name: &OsStr,
    state: &mut SearchState<'_>,
) -> Result<Option<Vec<u8>>, ToolError> {
    let metadata = parent
        .symlink_metadata(name)
        .map_err(|error| ToolError::new("scan_failed", error.to_string()))?;
    if !metadata.is_file() {
        return Ok(None);
    }
    if metadata.len() > MAX_FILE_SCAN_BYTES as u64 {
        state.skipped_oversized_files += 1;
        state.reasons.insert(IncompleteReason::OversizedFile);
        return Ok(None);
    }
    let expected = usize::try_from(metadata.len())
        .map_err(|_| ToolError::new("scan_failed", "file length does not fit usize"))?;
    if expected.saturating_add(1) > state.remaining_scan_bytes {
        state.reasons.insert(IncompleteReason::ScanByteLimit);
        return Ok(None);
    }
    let bytes = read_exact_candidate(parent, name, &metadata, state.cancellation)?;
    state.files_scanned += 1;
    state.bytes_scanned += bytes.len();
    state.remaining_scan_bytes -= bytes.len();
    if bytes.len() != expected {
        return Err(ToolError::new(
            "scan_failed",
            "file changed while it was being scanned",
        ));
    }
    Ok(Some(bytes))
}

#[cfg(all(unix, not(target_os = "redox")))]
fn read_exact_candidate(
    parent: &Dir,
    name: &OsStr,
    expected: &cap_std::fs::Metadata,
    cancellation: &Cancellation,
) -> Result<Vec<u8>, ToolError> {
    use cap_std::fs::MetadataExt as _;
    use std::os::unix::fs::MetadataExt as _;

    let descriptor = rustix::fs::openat(
        parent,
        name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK,
        rustix::fs::Mode::empty(),
    )
    .map_err(|error| ToolError::new("scan_failed", error.to_string()))?;
    let file = std::fs::File::from(descriptor);
    let actual = file
        .metadata()
        .map_err(|error| ToolError::new("scan_failed", error.to_string()))?;
    if !actual.is_file()
        || actual.len() != expected.len()
        || expected.dev() != actual.dev()
        || expected.ino() != actual.ino()
    {
        return Err(ToolError::new(
            "scan_failed",
            "file changed during discovery",
        ));
    }
    read_bounded(file, expected.len(), cancellation)
}

#[cfg(any(not(unix), target_os = "redox"))]
fn read_exact_candidate(
    parent: &Dir,
    name: &OsStr,
    expected: &cap_std::fs::Metadata,
    cancellation: &Cancellation,
) -> Result<Vec<u8>, ToolError> {
    let file = parent
        .open(name)
        .map_err(|error| ToolError::new("scan_failed", error.to_string()))?;
    let actual = file
        .metadata()
        .map_err(|error| ToolError::new("scan_failed", error.to_string()))?;
    if !actual.is_file() || actual.len() != expected.len() {
        return Err(ToolError::new(
            "scan_failed",
            "file changed during discovery",
        ));
    }
    read_bounded(file, expected.len(), cancellation)
}

fn read_bounded(
    mut file: impl Read,
    expected_length: u64,
    cancellation: &Cancellation,
) -> Result<Vec<u8>, ToolError> {
    let expected = usize::try_from(expected_length)
        .map_err(|_| ToolError::new("scan_failed", "file length does not fit usize"))?;
    let target = expected.saturating_add(1);
    let mut bytes = Vec::with_capacity(target);
    let mut buffer = [0_u8; 8 * 1024];
    while bytes.len() < target {
        ensure_active(cancellation)?;
        let remaining = target - bytes.len();
        let chunk_length = remaining.min(buffer.len());
        let read = file
            .read(&mut buffer[..chunk_length])
            .map_err(|error| ToolError::new("scan_failed", error.to_string()))?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    Ok(bytes)
}

fn push_match(path: &str, line_number: usize, line: &str, state: &mut SearchState<'_>) -> bool {
    if state.matches.len() == state.max_results {
        return false;
    }
    let (text, text_truncated) = line_preview(line);
    if text_truncated {
        state.reasons.insert(IncompleteReason::LineOutputLimit);
    }
    state.matches.push(SearchMatch {
        path: path.to_owned(),
        line: line_number,
        text,
        text_truncated,
    });
    true
}

fn line_preview(line: &str) -> (String, bool) {
    let line = line.strip_suffix('\r').unwrap_or(line);
    if line.len() <= MAX_LINE_OUTPUT_BYTES {
        return (line.to_owned(), false);
    }
    let mut end = MAX_LINE_OUTPUT_BYTES;
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    (line[..end].to_owned(), true)
}

fn encode_output(
    matches: &[SearchMatch],
    reasons: &BTreeSet<IncompleteReason>,
    stats: &SearchStats,
    output_limit: usize,
) -> Result<ToolOutput, ToolError> {
    let content = serialize_output(matches, reasons, stats)?;
    if content.len() <= output_limit {
        return Ok(ToolOutput {
            content,
            truncated: !reasons.is_empty(),
        });
    }
    let mut limited_reasons = reasons.clone();
    limited_reasons.insert(IncompleteReason::OutputLimit);
    let count = largest_fitting_prefix(matches, &limited_reasons, stats, output_limit)?;
    let content = serialize_output(&matches[..count], &limited_reasons, stats)?;
    if content.len() > output_limit {
        return Err(output_limit_error());
    }
    Ok(ToolOutput {
        content,
        truncated: true,
    })
}

fn largest_fitting_prefix(
    matches: &[SearchMatch],
    reasons: &BTreeSet<IncompleteReason>,
    stats: &SearchStats,
    limit: usize,
) -> Result<usize, ToolError> {
    let (mut low, mut high) = (0, matches.len());
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if serialize_output(&matches[..middle], reasons, stats)?.len() <= limit {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    Ok(low)
}

fn serialize_output(
    matches: &[SearchMatch],
    reasons: &BTreeSet<IncompleteReason>,
    stats: &SearchStats,
) -> Result<String, ToolError> {
    let output = SearchTextOutput {
        matches,
        complete: reasons.is_empty(),
        reasons,
        stats,
    };
    serde_json::to_string(&output)
        .map_err(|error| ToolError::new("output_encoding_failed", error.to_string()))
}

fn search_stats(state: &SearchState<'_>, walk: &WalkStats) -> SearchStats {
    SearchStats {
        visited_entries: walk.visited_entries,
        skipped_symlinks: walk.skipped_symlinks,
        skipped_git_metadata_entries: walk.skipped_git_metadata_entries,
        skipped_non_utf8_names: walk.skipped_non_utf8_names,
        skipped_overlong_paths: walk.skipped_overlong_paths,
        files_scanned: state.files_scanned,
        bytes_scanned: state.bytes_scanned,
        skipped_oversized_files: state.skipped_oversized_files,
        skipped_non_utf8_files: state.skipped_non_utf8_files,
        skipped_binary_files: state.skipped_binary_files,
    }
}

fn validate_query(query: &str) -> Result<(), ToolError> {
    if query.is_empty() || query.len() > MAX_QUERY_BYTES || query.contains(['\0', '\n', '\r']) {
        return Err(invalid_arguments(
            "query must be nonempty, single-line, NUL-free, and at most 1024 bytes",
        ));
    }
    Ok(())
}

fn invalid_arguments(message: &str) -> ToolError {
    ToolError::new("invalid_arguments", message)
}

fn output_limit_error() -> ToolError {
    ToolError::new(
        "output_limit",
        "output limit is too small for the search-text result envelope",
    )
}

#[cfg(test)]
#[path = "search_text_tests.rs"]
mod tests;
