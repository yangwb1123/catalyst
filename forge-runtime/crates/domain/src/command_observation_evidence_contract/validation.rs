use super::{
    API_VERSION, CANONICALIZATION, CommandEvidenceBinding, CommandObservation,
    CommandObservationEvidenceContractError, CommandObservationEvidenceRequest,
    CommandTerminationKind, OBSERVATION_API_VERSION, ObservedCommand, ObservedStream, invalid,
};

pub(super) const EMPTY_SHA256: &str =
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
pub(super) const MAX_TIMEOUT_MS: i64 = 86_400_000;
pub(super) const MAX_EXIT_CODE: i64 = 2_147_483_647;
const MAX_IDENTIFIER_BYTES: usize = 160;
const MAX_TEXT_CHARS: usize = 4096;
const MAX_ARGV_ITEMS: usize = 64;

pub(super) fn validate_request(
    request: &CommandObservationEvidenceRequest,
) -> Result<(), CommandObservationEvidenceContractError> {
    if request.api_version != API_VERSION || request.canonicalization != CANONICALIZATION {
        return Err(invalid("unsupported adapter API or canonicalization"));
    }
    validate_binding(&request.binding)?;
    validate_observation(&request.observation)?;
    if request.observation.termination.kind != CommandTerminationKind::Exited {
        return Err(invalid("only exited command observations are projectable"));
    }
    Ok(())
}

pub(super) fn validate_observation(
    observation: &CommandObservation,
) -> Result<(), CommandObservationEvidenceContractError> {
    if observation.api_version != OBSERVATION_API_VERSION
        || observation.canonicalization != CANONICALIZATION
    {
        return Err(invalid(
            "unsupported command observation API or canonicalization",
        ));
    }
    validate_command(&observation.command)?;
    validate_producer(observation)?;
    if !identifier(&observation.source.source_revision)
        || !digest(&observation.source.source_tree_sha256)
        || observation.started_at_unix_ms < 0
        || observation.ended_at_unix_ms < observation.started_at_unix_ms
    {
        return Err(invalid("command observation source or time is invalid"));
    }
    validate_streams(observation)?;
    validate_termination(observation)
}

pub(super) fn validate_command(
    command: &ObservedCommand,
) -> Result<(), CommandObservationEvidenceContractError> {
    if !(1..=MAX_ARGV_ITEMS).contains(&command.argv.len())
        || command.argv[0].is_empty()
        || command
            .argv
            .iter()
            .any(|argument| argument.chars().count() > MAX_TEXT_CHARS)
        || !safe_cwd(&command.cwd)
        || !digest(&command.environment_sha256)
        || !digest(&command.stdin_sha256)
        || !digest(&command.tool_snapshot_sha256)
        || command.stdin_bytes < 0
        || command.stdin_bytes == 0 && command.stdin_sha256 != EMPTY_SHA256
        || command
            .timeout_ms
            .is_some_and(|value| !(1..=MAX_TIMEOUT_MS).contains(&value))
    {
        return Err(invalid("observed command is invalid"));
    }
    Ok(())
}

fn validate_binding(
    binding: &CommandEvidenceBinding,
) -> Result<(), CommandObservationEvidenceContractError> {
    let identifiers = [
        binding.aggregate_id.as_str(),
        binding.project_id.as_str(),
        binding.scope.as_str(),
    ];
    if !identifiers.into_iter().all(identifier)
        || !digest(&binding.context_sha256)
        || !digest(&binding.policy_sha256)
        || binding.sequence < 1
        || binding.subjects.is_empty()
        || !sorted_unique_identifiers(&binding.subjects)
        || !sorted_unique_identifiers(&binding.supersedes_record_ids)
    {
        return Err(invalid("command evidence binding is invalid"));
    }
    Ok(())
}

fn validate_producer(
    observation: &CommandObservation,
) -> Result<(), CommandObservationEvidenceContractError> {
    let producer = &observation.producer;
    if [
        producer.producer_id.as_str(),
        producer.producer_version.as_str(),
        producer.run_id.as_str(),
    ]
    .into_iter()
    .all(identifier)
    {
        Ok(())
    } else {
        Err(invalid("command observation producer is invalid"))
    }
}

fn validate_streams(
    observation: &CommandObservation,
) -> Result<(), CommandObservationEvidenceContractError> {
    let streams = &observation.streams;
    validate_stream(&streams.combined)?;
    validate_stream(&streams.stderr)?;
    validate_stream(&streams.stdout)?;
    let split_bytes = streams
        .stdout
        .bytes
        .checked_add(streams.stderr.bytes)
        .ok_or_else(|| invalid("stdout plus stderr bytes exceeds signed int64"))?;
    if streams.combined.bytes != split_bytes {
        return Err(invalid(
            "combined bytes must equal stdout bytes plus stderr bytes",
        ));
    }
    Ok(())
}

fn validate_stream(stream: &ObservedStream) -> Result<(), CommandObservationEvidenceContractError> {
    if stream.bytes < 0
        || stream.retained_bytes < 0
        || stream.retained_bytes > stream.bytes
        || !digest(&stream.sha256)
        || !digest(&stream.retained_sha256)
        || stream.bytes == 0
            && (stream.sha256 != EMPTY_SHA256 || stream.retained_sha256 != EMPTY_SHA256)
        || stream.retained_bytes == 0 && stream.retained_sha256 != EMPTY_SHA256
        || stream.retained_bytes == stream.bytes && stream.retained_sha256 != stream.sha256
    {
        return Err(invalid("command observation stream is invalid"));
    }
    Ok(())
}

fn validate_termination(
    observation: &CommandObservation,
) -> Result<(), CommandObservationEvidenceContractError> {
    let termination = &observation.termination;
    let valid = match termination.kind {
        CommandTerminationKind::Exited => termination
            .exit_code
            .is_some_and(|code| (0..=MAX_EXIT_CODE).contains(&code)),
        CommandTerminationKind::Cancelled | CommandTerminationKind::TimedOut => {
            termination.exit_code.is_none()
        }
    };
    valid
        .then_some(())
        .ok_or_else(|| invalid("command observation termination is invalid"))
}

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= MAX_IDENTIFIER_BYTES
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'_' | b':' | b'/' | b'-')
        })
}

fn sorted_unique_identifiers(values: &[String]) -> bool {
    values.iter().all(|value| identifier(value))
        && values
            .windows(2)
            .all(|pair| pair[0].as_bytes() < pair[1].as_bytes())
}

fn safe_cwd(path: &str) -> bool {
    if path == "." {
        return true;
    }
    let bytes = path.as_bytes();
    !path.is_empty()
        && path.chars().count() <= MAX_TEXT_CHARS
        && !path.starts_with('/')
        && !path.ends_with('/')
        && !path.contains('\\')
        && !(bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}
