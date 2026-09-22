use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use forge_runtime_domain::{
    RUN_INTENT_OBSERVATION_SCHEMA_VERSION, RunIntentObservation, RunIntentObservationRequest,
    RunIntentPromptReceipt, RunIntentRunReference, SessionPlacementAuthority,
    SessionPlacementObservation, SessionPlacementOwner, TenantId, observe_run_intent,
};
use serde::{Deserialize, Serialize};

use crate::{
    args::{DeviceCommand, DevicePlacementCommand},
    device_command::observe_session_placement_fixture,
};

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const RUN_CONTRACT: &str = "forgeos.run-intent-observation-contract/v1";
const SESSION_CONTRACT: &str = "forgeos.session-placement-observation-contract/v1";
const PLACEMENT_FIXTURE_NAME: &str = "forge-session-placement-observation-v1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Owner {
    issuer: String,
    subject: String,
    tenant_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunFixture {
    api_version: String,
    placement_contract_fixture: String,
    owner: Owner,
    conversation_id: String,
    prompt_receipt: PromptReceipt,
    run_reference: RunReference,
    expected: RunExpected,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PromptReceipt {
    prompt_id: String,
    conversation_id: String,
    role: String,
    accepted_at_ms: u64,
    intent_id: String,
    initial_event_id: String,
    initial_event_sequence: u64,
    initial_event_type: String,
    replayed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunReference {
    run_id: String,
    conversation_id: String,
    prompt_id: String,
    created_at_ms: u64,
    latest_sequence: u64,
    status: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionFixture {
    api_version: String,
    owner: Owner,
    conversation_id: String,
    run_id: String,
    placement: serde_json::Value,
    expected: SessionExpected,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionExpected {
    evaluation_mode: String,
    evaluated_at_ms: u64,
    owner_declaration_unverified: bool,
    device_attributes_unverified: bool,
    decisions: Vec<SessionDecisionExpected>,
    selected_device_id: Option<String>,
    selected_instance_id: Option<String>,
    authority: Authority,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct SessionDecisionExpected {
    device_id: String,
    instance_id: String,
    matches_requirements: bool,
    exclusion_reasons: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
struct RunExpected {
    evaluation_mode: String,
    prompt_accepted: bool,
    run_reference_observed: bool,
    prompt_run_binding_valid: bool,
    placement_observation_bound: bool,
    preview_only: bool,
    intent_replayed: bool,
    run_status: String,
    run_latest_sequence: u64,
    prompt_accepted_at_ms: u64,
    placement_evaluated_at_ms: u64,
    placement_decision_count: usize,
    eligible_instance_count: usize,
    owner_declaration_unverified: bool,
    device_attributes_unverified: bool,
    selected_device_id: Option<String>,
    selected_instance_id: Option<String>,
    authority: Authority,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
struct Authority {
    identity_verified: bool,
    heartbeat_persisted: bool,
    inventory_authoritative: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
}

#[derive(Debug, Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct RunIntentPreviewOutput {
    v: u16,
    #[serde(rename = "type")]
    output_type: &'static str,
    schema_version: &'static str,
    evaluation_mode: &'static str,
    owner: Owner,
    conversation_id: String,
    prompt_id: String,
    intent_id: String,
    run_id: String,
    prompt_accepted: bool,
    run_reference_observed: bool,
    prompt_run_binding_valid: bool,
    placement_observation_bound: bool,
    preview_only: bool,
    intent_replayed: bool,
    run_status: String,
    run_latest_sequence: u64,
    prompt_accepted_at_ms: u64,
    placement_evaluated_at_ms: u64,
    placement_decision_count: usize,
    eligible_instance_count: usize,
    owner_declaration_unverified: bool,
    device_attributes_unverified: bool,
    selected_device_id: Option<String>,
    selected_instance_id: Option<String>,
    authority: Authority,
}

pub(crate) fn execute(command: &DeviceCommand) -> Result<RunIntentPreviewOutput, Box<dyn Error>> {
    let DeviceCommand::Placement(DevicePlacementCommand::RunIntentPreview {
        input,
        placement_input,
    }) = command
    else {
        return Err("device placement Run-intent preview command is required".into());
    };
    if input == "-" && placement_input == "-" {
        return Err("Run-intent and placement inputs cannot both read stdin".into());
    }
    let run: RunFixture = read_fixture(input, "Run-intent")?;
    let session: SessionFixture = read_fixture(placement_input, "session placement")?;
    evaluate(run, session)
}

pub(crate) fn write_output(
    output: &RunIntentPreviewOutput,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    writeln!(
        writer,
        "offline Run-intent preview [{}]",
        output.schema_version
    )?;
    writeln!(
        writer,
        "conversation={} prompt={} intent={} run={} status={} latest_sequence={}",
        output.conversation_id,
        output.prompt_id,
        output.intent_id,
        output.run_id,
        output.run_status,
        output.run_latest_sequence
    )?;
    writeln!(
        writer,
        "placement: decisions={} eligible={} selected_device=none selected_instance=none",
        output.placement_decision_count, output.eligible_instance_count
    )?;
    writeln!(
        writer,
        "authority: identity_verified=false heartbeat_persisted=false inventory_authoritative=false reservation_created=false execution_authorized=false dispatch_performed=false"
    )
}

fn evaluate(
    run: RunFixture,
    session: SessionFixture,
) -> Result<RunIntentPreviewOutput, Box<dyn Error>> {
    validate_contract_headers(&run, &session)?;
    let owner = session_owner(&run.owner)?;
    let placement = observe_session_placement_fixture(
        owner.clone(),
        session.conversation_id.clone(),
        session.run_id.clone(),
        session.placement,
    )?;
    validate_session_expected(&placement, &session.expected)?;
    let observation = observe_run_intent(RunIntentObservationRequest {
        owner,
        conversation_id: run.conversation_id,
        prompt: RunIntentPromptReceipt {
            prompt_id: run.prompt_receipt.prompt_id,
            conversation_id: run.prompt_receipt.conversation_id,
            role: run.prompt_receipt.role,
            accepted_at_ms: run.prompt_receipt.accepted_at_ms,
            intent_id: run.prompt_receipt.intent_id,
            initial_event_id: run.prompt_receipt.initial_event_id,
            initial_event_sequence: run.prompt_receipt.initial_event_sequence,
            initial_event_type: run.prompt_receipt.initial_event_type,
            replayed: run.prompt_receipt.replayed,
        },
        run: RunIntentRunReference {
            run_id: run.run_reference.run_id,
            conversation_id: run.run_reference.conversation_id,
            prompt_id: run.run_reference.prompt_id,
            created_at_ms: run.run_reference.created_at_ms,
            latest_sequence: run.run_reference.latest_sequence,
            status: run.run_reference.status,
        },
        placement,
    })?;
    validate_run_expected(&observation, &run.expected)?;
    Ok(output_from_observation(observation))
}

fn validate_contract_headers(
    run: &RunFixture,
    session: &SessionFixture,
) -> Result<(), Box<dyn Error>> {
    if run.api_version != RUN_CONTRACT
        || session.api_version != SESSION_CONTRACT
        || run.placement_contract_fixture != PLACEMENT_FIXTURE_NAME
    {
        return Err("unsupported Run-intent or session placement contract".into());
    }
    if run.owner != session.owner
        || run.conversation_id != session.conversation_id
        || run.run_reference.run_id != session.run_id
    {
        return Err("Run-intent and session placement bindings do not match".into());
    }
    Ok(())
}

fn session_owner(owner: &Owner) -> Result<SessionPlacementOwner, Box<dyn Error>> {
    Ok(SessionPlacementOwner {
        issuer: owner.issuer.clone(),
        subject: owner.subject.clone(),
        tenant_id: TenantId::parse(owner.tenant_id.clone())?,
    })
}

fn validate_session_expected(
    actual: &SessionPlacementObservation,
    expected: &SessionExpected,
) -> Result<(), Box<dyn Error>> {
    let decisions_match = actual.decisions.len() == expected.decisions.len()
        && actual
            .decisions
            .iter()
            .zip(&expected.decisions)
            .all(|(actual, expected)| {
                actual.device_id == expected.device_id
                    && actual.instance_id == expected.instance_id
                    && actual.matches_requirements == expected.matches_requirements
                    && actual.exclusion_reasons == expected.exclusion_reasons
            });
    if actual.evaluation_mode != expected.evaluation_mode
        || actual.evaluated_at_ms != expected.evaluated_at_ms
        || actual.owner_declaration_unverified != expected.owner_declaration_unverified
        || actual.device_attributes_unverified != expected.device_attributes_unverified
        || actual.selected_device_id != expected.selected_device_id
        || actual.selected_instance_id != expected.selected_instance_id
        || authority(actual.authority) != expected.authority
        || !decisions_match
    {
        return Err("session placement fixture expectation mismatch".into());
    }
    Ok(())
}

fn validate_run_expected(
    actual: &RunIntentObservation,
    expected: &RunExpected,
) -> Result<(), Box<dyn Error>> {
    if actual.evaluation_mode != expected.evaluation_mode
        || actual.prompt_accepted != expected.prompt_accepted
        || actual.run_reference_observed != expected.run_reference_observed
        || actual.prompt_run_binding_valid != expected.prompt_run_binding_valid
        || actual.placement_observation_bound != expected.placement_observation_bound
        || actual.preview_only != expected.preview_only
        || actual.intent_replayed != expected.intent_replayed
        || actual.run_status != expected.run_status
        || actual.run_latest_sequence != expected.run_latest_sequence
        || actual.prompt_accepted_at_ms != expected.prompt_accepted_at_ms
        || actual.placement_evaluated_at_ms != expected.placement_evaluated_at_ms
        || actual.placement_decision_count != expected.placement_decision_count
        || actual.eligible_instance_count != expected.eligible_instance_count
        || actual.owner_declaration_unverified != expected.owner_declaration_unverified
        || actual.device_attributes_unverified != expected.device_attributes_unverified
        || actual.selected_device_id != expected.selected_device_id
        || actual.selected_instance_id != expected.selected_instance_id
        || authority(actual.authority) != expected.authority
    {
        return Err("Run-intent fixture expectation mismatch".into());
    }
    Ok(())
}

fn output_from_observation(value: RunIntentObservation) -> RunIntentPreviewOutput {
    RunIntentPreviewOutput {
        v: 1,
        output_type: "device_run_intent_preview",
        schema_version: RUN_INTENT_OBSERVATION_SCHEMA_VERSION,
        evaluation_mode: value.evaluation_mode,
        owner: Owner {
            issuer: value.owner.issuer,
            subject: value.owner.subject,
            tenant_id: value.owner.tenant_id.as_str().to_owned(),
        },
        conversation_id: value.conversation_id,
        prompt_id: value.prompt_id,
        intent_id: value.intent_id,
        run_id: value.run_id,
        prompt_accepted: value.prompt_accepted,
        run_reference_observed: value.run_reference_observed,
        prompt_run_binding_valid: value.prompt_run_binding_valid,
        placement_observation_bound: value.placement_observation_bound,
        preview_only: value.preview_only,
        intent_replayed: value.intent_replayed,
        run_status: value.run_status,
        run_latest_sequence: value.run_latest_sequence,
        prompt_accepted_at_ms: value.prompt_accepted_at_ms,
        placement_evaluated_at_ms: value.placement_evaluated_at_ms,
        placement_decision_count: value.placement_decision_count,
        eligible_instance_count: value.eligible_instance_count,
        owner_declaration_unverified: value.owner_declaration_unverified,
        device_attributes_unverified: value.device_attributes_unverified,
        selected_device_id: value.selected_device_id,
        selected_instance_id: value.selected_instance_id,
        authority: authority(value.authority),
    }
}

fn authority(value: SessionPlacementAuthority) -> Authority {
    Authority {
        identity_verified: value.identity_verified,
        heartbeat_persisted: value.heartbeat_persisted,
        inventory_authoritative: value.inventory_authoritative,
        reservation_created: value.reservation_created,
        execution_authorized: value.execution_authorized,
        dispatch_performed: value.dispatch_performed,
    }
}

fn read_fixture<T: for<'de> Deserialize<'de>>(
    input: &str,
    label: &str,
) -> Result<T, Box<dyn Error>> {
    let mut bytes = Vec::with_capacity(MAX_INPUT_BYTES.min(64 * 1024));
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
        return Err(format!("{label} input exceeds {MAX_INPUT_BYTES} bytes").into());
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("{label} input is invalid: {error}").into())
}
