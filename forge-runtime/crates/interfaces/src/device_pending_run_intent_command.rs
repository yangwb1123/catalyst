use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use forge_runtime_domain::{
    ConversationOwner, PendingRunIntent, PendingRunIntentPage, PendingRunIntentTimelinePage,
};
use serde::{Deserialize, Serialize};

use crate::{args::DeviceCommand, device_json_unique::reject_duplicate_keys};

const MAX_INPUT_BYTES: usize = 512 * 1024;
const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_ID_BYTES: usize = 128;
const MAX_OWNER_PART_BYTES: usize = 2048;
const MAX_PROMPT_CONTENT_BYTES: usize = 256 * 1024;
const SCHEMA_VERSION: &str = "forge.pending-run-intent/v1";
const EVALUATION_MODE: &str = "owner_scoped_pending_intent_preview";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    authority: Authority,
    owner: ConversationOwner,
    conversation_id: String,
    submission: Submission,
    page: PendingRunIntentPage,
    timeline: PendingRunIntentTimelinePage,
    expected: Expected,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Authority {
    device_identity_verified: bool,
    inventory_authoritative: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
    run_created: bool,
    audit_published: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Submission {
    prompt: Prompt,
    intent: PendingRunIntent,
    initial_event: Event,
    replayed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Prompt {
    id: String,
    conversation_id: String,
    role: String,
    content: String,
    created_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct Event {
    event_id: String,
    seq: u64,
    emitted_at_ms: u64,
    #[serde(rename = "type")]
    event_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    prompt_role: String,
    intent_status: String,
    initial_event_type: String,
    timeline_event_count: usize,
    replayed: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct PendingRunIntentPreviewOutput {
    schema_version: &'static str,
    evaluation_mode: &'static str,
    owner: ConversationOwner,
    conversation_id: String,
    prompt_id: String,
    prompt_role: String,
    prompt_content_bytes: usize,
    intent_id: String,
    project_id: String,
    profile_id: String,
    status: &'static str,
    replayed: bool,
    aggregate_version: u64,
    latest_sequence: u64,
    timeline_scanned_through_sequence: u64,
    timeline_event_count: usize,
    authority: Authority,
}

pub(crate) fn execute(
    command: &DeviceCommand,
) -> Result<PendingRunIntentPreviewOutput, Box<dyn Error>> {
    let DeviceCommand::PendingRunIntentPreview { input } = command else {
        return Err("device pending Run-intent preview command is required".into());
    };
    let bytes = read_bounded_input(input)?;
    reject_duplicate_keys(&bytes)
        .map_err(|error| format!("pending Run-intent input has duplicate JSON keys: {error}"))?;
    let fixture: Fixture = serde_json::from_slice(&bytes)
        .map_err(|error| format!("pending Run-intent input is invalid JSON: {error}"))?;
    evaluate(fixture)
}

pub(crate) fn write_output(
    output: &PendingRunIntentPreviewOutput,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    writeln!(
        writer,
        "offline pending Run-intent preview [{}] conversation={} intent={} status={} replayed={} timeline_events={}",
        output.schema_version,
        output.conversation_id,
        output.intent_id,
        output.status,
        output.replayed,
        output.timeline_event_count
    )?;
    writeln!(
        writer,
        "prompt={} role={} content_bytes={} project={} profile={} aggregate_version={} latest_sequence={} scanned_through={}",
        output.prompt_id,
        output.prompt_role,
        output.prompt_content_bytes,
        output.project_id,
        output.profile_id,
        output.aggregate_version,
        output.latest_sequence,
        output.timeline_scanned_through_sequence
    )?;
    writeln!(
        writer,
        "authority: device_identity_verified=false inventory_authoritative=false reservation_created=false execution_authorized=false dispatch_performed=false run_created=false audit_published=false"
    )
}

fn evaluate(fixture: Fixture) -> Result<PendingRunIntentPreviewOutput, Box<dyn Error>> {
    validate_fixture(&fixture)?;
    Ok(PendingRunIntentPreviewOutput {
        schema_version: SCHEMA_VERSION,
        evaluation_mode: EVALUATION_MODE,
        owner: fixture.owner,
        conversation_id: fixture.conversation_id,
        prompt_id: fixture.submission.prompt.id,
        prompt_role: fixture.submission.prompt.role,
        prompt_content_bytes: fixture.submission.prompt.content.len(),
        intent_id: fixture.submission.intent.intent_id.clone(),
        project_id: fixture.submission.intent.project_id.clone(),
        profile_id: fixture.submission.intent.profile_id.clone(),
        status: "pending",
        replayed: fixture.submission.replayed,
        aggregate_version: fixture.submission.intent.aggregate_version,
        latest_sequence: fixture.submission.intent.latest_sequence,
        timeline_scanned_through_sequence: fixture.timeline.scanned_through_sequence,
        timeline_event_count: fixture.timeline.events.len(),
        authority: fixture.authority,
    })
}

fn validate_fixture(fixture: &Fixture) -> Result<(), Box<dyn Error>> {
    if fixture.schema_version != SCHEMA_VERSION || fixture.evaluation_mode != EVALUATION_MODE {
        return Err("pending Run-intent input has an unsupported schema or evaluation mode".into());
    }
    if !authority_is_false(fixture.authority) {
        return Err("pending Run-intent input claims authority".into());
    }
    if !valid_owner(&fixture.owner)
        || !valid_id(&fixture.conversation_id)
        || fixture.owner.issuer.is_empty()
    {
        return Err("pending Run-intent input has an invalid owner or Conversation".into());
    }
    validate_submission(fixture)?;
    validate_page(fixture)?;
    validate_timeline(fixture)?;
    if fixture.expected.prompt_role != fixture.submission.prompt.role
        || fixture.expected.intent_status != "pending"
        || fixture.expected.initial_event_type != "submitted"
        || fixture.expected.timeline_event_count != fixture.timeline.events.len()
        || fixture.expected.replayed != fixture.submission.replayed
    {
        return Err("pending Run-intent input has expectation drift".into());
    }
    Ok(())
}

fn validate_submission(fixture: &Fixture) -> Result<(), Box<dyn Error>> {
    let submission = &fixture.submission;
    let prompt = &submission.prompt;
    let intent = &submission.intent;
    let event = &submission.initial_event;
    if !valid_id(&prompt.id)
        || prompt.conversation_id != fixture.conversation_id
        || prompt.role != "user"
        || prompt.content.is_empty()
        || prompt.content.len() > MAX_PROMPT_CONTENT_BYTES
        || prompt.created_at_ms > MAX_SAFE_JSON_INTEGER
        || !valid_id(&intent.intent_id)
        || intent.conversation_id != fixture.conversation_id
        || intent.prompt_id != prompt.id
        || !valid_id(&intent.project_id)
        || !valid_id(&intent.profile_id)
        || intent.submitted_at_ms != prompt.created_at_ms
        || intent.submitted_at_ms > MAX_SAFE_JSON_INTEGER
        || intent.aggregate_version == 0
        || intent.aggregate_version > MAX_SAFE_JSON_INTEGER
        || intent.latest_sequence != 1
        || intent.status != forge_runtime_domain::PendingRunIntentStatus::Pending
        || !valid_event(event)
        || event.seq != 1
        || event.event_type != "submitted"
        || event.emitted_at_ms != intent.submitted_at_ms
    {
        return Err("pending Run-intent submission is invalid".into());
    }
    Ok(())
}

fn validate_page(fixture: &Fixture) -> Result<(), Box<dyn Error>> {
    let page = &fixture.page;
    if page.conversation_id != fixture.conversation_id
        || page.has_more
        || page.next_cursor.is_some()
        || page.intents.len() != 1
        || page.intents[0] != fixture.submission.intent
    {
        return Err("pending Run-intent page is invalid".into());
    }
    Ok(())
}

fn validate_timeline(fixture: &Fixture) -> Result<(), Box<dyn Error>> {
    let timeline = &fixture.timeline;
    if timeline.conversation_id != fixture.conversation_id
        || timeline.intent_id != fixture.submission.intent.intent_id
        || timeline.after_sequence != 0
        || timeline.scanned_through_sequence != 1
        || timeline.has_more
        || timeline.events.len() != 1
    {
        return Err("pending Run-intent timeline is invalid".into());
    }
    let event = &timeline.events[0];
    if event.event_id != fixture.submission.initial_event.event_id
        || event.seq != fixture.submission.initial_event.seq
        || event.emitted_at_ms != fixture.submission.initial_event.emitted_at_ms
        || event.event_type != forge_runtime_domain::PendingRunIntentTimelineEventType::Submitted
    {
        return Err("pending Run-intent timeline event is invalid".into());
    }
    Ok(())
}

fn valid_event(event: &Event) -> bool {
    valid_id(&event.event_id)
        && event.emitted_at_ms <= MAX_SAFE_JSON_INTEGER
        && event.event_type == "submitted"
}

fn valid_owner(owner: &ConversationOwner) -> bool {
    valid_owner_part(&owner.issuer)
        && valid_owner_part(&owner.subject)
        && valid_owner_part(&owner.tenant_id)
}

fn valid_owner_part(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_OWNER_PART_BYTES && value.trim() == value
}

fn valid_id(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    value.len() <= MAX_ID_BYTES
        && first.is_ascii_alphanumeric()
        && chars
            .all(|value| value.is_ascii_alphanumeric() || matches!(value, '.' | '_' | ':' | '-'))
}

fn authority_is_false(value: Authority) -> bool {
    !value.device_identity_verified
        && !value.inventory_authoritative
        && !value.reservation_created
        && !value.execution_authorized
        && !value.dispatch_performed
        && !value.run_created
        && !value.audit_published
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::new();
    if input == "-" {
        io::stdin()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
    } else {
        File::open(Path::new(input))?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(format!("pending Run-intent input exceeds {MAX_INPUT_BYTES} bytes").into());
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "device_pending_run_intent_command_tests.rs"]
mod tests;
