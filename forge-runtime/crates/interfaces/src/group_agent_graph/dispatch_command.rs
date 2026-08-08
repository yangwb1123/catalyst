use std::{error::Error, io::{self, Write}, path::PathBuf, sync::Arc};

use forge_runtime_application::{
    AdjudicateGroupAgentNodeDispatchInput, AdjudicateGroupAgentNodeDispatchResult,
    AdjudicationRefused, GroupAgentNodeDispatchAdjudicationService,
    GroupAgentNodeDispatchAdjudicationServiceError, GroupAgentNodeDispatchReadinessService,
    GroupAgentNodeDispatchReleaseControlService, GroupAgentNodeDispatchRequestService,
    PrepareGroupAgentNodeDispatchRequestInput,
};
use forge_runtime_infrastructure::{
    PinnedCoreTerminalBridge, RegisteredGroupAgentNodeProviderFactory, SqliteHubStore,
};

use crate::{
    args::{Args, GroupGraphRunDispatchCommand},
    openai_prepared_dispatch::OpenAiRequestCodec,
    runtime_domain::{
        GroupAgentGraphRunStatus, GroupAgentNodeDispatchAuthorization,
        GroupAgentNodeDispatchClaim, GroupAgentNodeLifecycleInspection,
        GroupAgentNodePricingSnapshot,
    },
    state_path::{hub_database_path, idempotency_key, unix_time_millis},
};

use super::{
    dispatch_authorization_output::{self, GroupAgentNodeDispatchAuthorizationCliOutput},
    dispatch_execution_adapters::{
        self, GroupAgentNodeDispatchExecutionCliOutput, SystemDispatchMetadataSource,
        read_authorization, read_pricing,
    },
    dispatch_output::{self, GroupAgentNodeDispatchRequestCliOutput},
    dispatch_readiness_output::{self, GroupAgentNodeDispatchReadinessCliOutput},
};

pub enum GroupAgentGraphRunDispatchCommandCliOutput {
    Request(Box<GroupAgentNodeDispatchRequestCliOutput>),
    ReleaseControl(String),
    Authorization(Box<GroupAgentNodeDispatchAuthorizationCliOutput>),
    Readiness(Box<GroupAgentNodeDispatchReadinessCliOutput>),
    Execution(Box<GroupAgentNodeDispatchExecutionCliOutput>),
}

pub(super) struct DispatchInputs {
    authorization_json: Option<String>,
    pricing_json: Option<String>,
}

impl DispatchInputs {
    pub(super) fn authorization(&self) -> &str {
        self.authorization_json
            .as_deref()
            .expect("authorization was read before service construction")
    }

    pub(super) fn pricing(&self) -> &str {
        self.pricing_json
            .as_deref()
            .expect("pricing was read before service construction")
    }
}

pub async fn execute(
    args: &Args,
    command: &GroupGraphRunDispatchCommand,
) -> Result<GroupAgentGraphRunDispatchCommandCliOutput, Box<dyn Error>> {
    if let Some(existing) = inspect_existing_execution(args, command)? {
        return Ok(existing);
    }
    // Adjudicate ordering (design §7.4): stranded guard (read-only DB) before
    // any stdin read, so a non-stranded run fails before consuming stdin.
    adjudication_stranded_guard(args, command)?;
    let inputs = read_inputs(command)?;
    execute_with_inputs(args, command, &inputs).await
}

fn inspect_existing_execution(
    args: &Args,
    command: &GroupGraphRunDispatchCommand,
) -> Result<Option<GroupAgentGraphRunDispatchCommandCliOutput>, Box<dyn Error>> {
    let GroupGraphRunDispatchCommand::Execute {
        graph_run_id,
        include_result,
        ..
    } = command
    else {
        return Ok(None);
    };
    let database = hub_database_path(args.state_dir.as_deref())?;
    if !database.try_exists()? {
        return Ok(None);
    }
    let store = SqliteHubStore::open_existing_dispatch_inspection_read_only(database)?;
    let Some(inspection) = store.inspect_existing_group_agent_node_lifecycle(graph_run_id)? else {
        return Ok(None);
    };
    let output = GroupAgentNodeDispatchExecutionCliOutput::from_result(
        forge_runtime_application::ExecuteGroupAgentNodeDispatchResult::AlreadyClaimed(inspection),
        *include_result,
    );
    Ok(Some(GroupAgentGraphRunDispatchCommandCliOutput::Execution(
        Box::new(output),
    )))
}

fn read_inputs(command: &GroupGraphRunDispatchCommand) -> Result<DispatchInputs, Box<dyn Error>> {
    let authorization_json = match command {
        GroupGraphRunDispatchCommand::AuthorizationVerify {
            authorization_source,
            ..
        }
        | GroupGraphRunDispatchCommand::ReadinessVerify {
            authorization_source,
            ..
        }
        | GroupGraphRunDispatchCommand::Execute {
            authorization_source,
            ..
        }
        | GroupGraphRunDispatchCommand::Adjudicate {
            authorization_source,
            ..
        } => Some(read_authorization(authorization_source)?),
        _ => None,
    };
    let pricing_json = match command {
        GroupGraphRunDispatchCommand::ReadinessVerify { pricing_source, .. }
        | GroupGraphRunDispatchCommand::Execute { pricing_source, .. }
        | GroupGraphRunDispatchCommand::Adjudicate { pricing_source, .. } => {
            Some(read_pricing(pricing_source)?)
        }
        _ => None,
    };
    Ok(DispatchInputs {
        authorization_json,
        pricing_json,
    })
}

async fn execute_with_inputs(
    args: &Args,
    command: &GroupGraphRunDispatchCommand,
    inputs: &DispatchInputs,
) -> Result<GroupAgentGraphRunDispatchCommandCliOutput, Box<dyn Error>> {
    match command {
        GroupGraphRunDispatchCommand::Execute {
            graph_run_id,
            core_bin,
            core_bin_sha256,
            confirm_off_machine,
            include_result,
            ..
        } => {
            dispatch_execution_adapters::execute_dispatch(
                args,
                graph_run_id,
                inputs,
                core_bin,
                core_bin_sha256,
                *confirm_off_machine,
                *include_result,
            )
            .await
        }
        GroupGraphRunDispatchCommand::Adjudicate {
            graph_run_id,
            core_bin,
            core_bin_sha256,
            ..
        } => execute_adjudicate(args, graph_run_id, inputs, core_bin, core_bin_sha256),
        _ => execute_effect_free(args, command, inputs),
    }
}

fn execute_effect_free(
    args: &Args,
    command: &GroupGraphRunDispatchCommand,
    inputs: &DispatchInputs,
) -> Result<GroupAgentGraphRunDispatchCommandCliOutput, Box<dyn Error>> {
    match command {
        GroupGraphRunDispatchCommand::Prepare { graph_run_id } => {
            execute_prepare(args, graph_run_id)
        }
        GroupGraphRunDispatchCommand::Show {
            dispatch_request_id,
            include_request,
        } => Ok(request_output(
            GroupAgentNodeDispatchRequestCliOutput::request(
                request_service(args)?.inspect(dispatch_request_id)?,
                *include_request,
            )?,
        )),
        GroupGraphRunDispatchCommand::List {
            graph_run_id,
            limit,
        } => Ok(request_output(
            GroupAgentNodeDispatchRequestCliOutput::list(
                request_service(args)?.list(graph_run_id.as_deref(), *limit)?,
            ),
        )),
        GroupGraphRunDispatchCommand::ReleaseControlExport { graph_run_id } => {
            export_release_control(args, graph_run_id)
        }
        GroupGraphRunDispatchCommand::AuthorizationVerify { graph_run_id, .. } => {
            verify_authorization(args, graph_run_id, inputs.authorization())
        }
        GroupGraphRunDispatchCommand::ReadinessVerify { graph_run_id, .. } => {
            verify_readiness(args, graph_run_id, inputs.authorization(), inputs.pricing())
        }
        GroupGraphRunDispatchCommand::Execute { .. }
        | GroupGraphRunDispatchCommand::Adjudicate { .. } => {
            unreachable!("handled before local routing")
        }
    }
}

fn execute_prepare(
    args: &Args,
    graph_run_id: &str,
) -> Result<GroupAgentGraphRunDispatchCommandCliOutput, Box<dyn Error>> {
    let result = request_service(args)?.prepare(&PrepareGroupAgentNodeDispatchRequestInput {
        graph_run_id: graph_run_id.into(),
        idempotency_key: args
            .idempotency_key
            .clone()
            .unwrap_or_else(|| idempotency_key("group-agent-node-dispatch-request")),
        prepared_at_ms: unix_time_millis(),
    })?;
    Ok(request_output(
        GroupAgentNodeDispatchRequestCliOutput::prepared(result.disposition, result.inspection)?,
    ))
}

fn export_release_control(
    args: &Args,
    graph_run_id: &str,
) -> Result<GroupAgentGraphRunDispatchCommandCliOutput, Box<dyn Error>> {
    let exported = release_service(args)?.export(graph_run_id)?;
    Ok(GroupAgentGraphRunDispatchCommandCliOutput::ReleaseControl(
        exported.canonical_json,
    ))
}

fn verify_authorization(
    args: &Args,
    graph_run_id: &str,
    authorization_json: &str,
) -> Result<GroupAgentGraphRunDispatchCommandCliOutput, Box<dyn Error>> {
    let verified = release_service(args)?.verify(graph_run_id, authorization_json)?;
    Ok(GroupAgentGraphRunDispatchCommandCliOutput::Authorization(
        Box::new(GroupAgentNodeDispatchAuthorizationCliOutput::verified(
            verified,
        )),
    ))
}

pub fn write_output(
    output: &GroupAgentGraphRunDispatchCommandCliOutput,
    json: bool,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    match output {
        GroupAgentGraphRunDispatchCommandCliOutput::Request(output) => {
            dispatch_output::write_output(output, json, writer)
        }
        GroupAgentGraphRunDispatchCommandCliOutput::ReleaseControl(canonical_json) => {
            writer.write_all(canonical_json.as_bytes())
        }
        GroupAgentGraphRunDispatchCommandCliOutput::Authorization(output) => {
            dispatch_authorization_output::write_output(output, json, writer)
        }
        GroupAgentGraphRunDispatchCommandCliOutput::Readiness(output) => {
            dispatch_readiness_output::write_output(output, json, writer)
        }
        GroupAgentGraphRunDispatchCommandCliOutput::Execution(output) => {
            dispatch_execution_adapters::write_output(output, json, writer)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn request_output(
    output: GroupAgentNodeDispatchRequestCliOutput,
) -> GroupAgentGraphRunDispatchCommandCliOutput {
    GroupAgentGraphRunDispatchCommandCliOutput::Request(Box::new(output))
}

fn request_service(args: &Args) -> Result<GroupAgentNodeDispatchRequestService, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    Ok(GroupAgentNodeDispatchRequestService::new(
        store.clone(),
        store.clone(),
        store.clone(),
        store,
        Arc::new(OpenAiRequestCodec),
    ))
}

fn release_service(
    args: &Args,
) -> Result<GroupAgentNodeDispatchReleaseControlService, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open_existing_current_read_only(database)?);
    Ok(GroupAgentNodeDispatchReleaseControlService::new(
        store.clone(),
        store,
        Arc::new(OpenAiRequestCodec),
    ))
}

fn verify_readiness(
    args: &Args,
    graph_run_id: &str,
    authorization_json: &str,
    pricing_json: &str,
) -> Result<GroupAgentGraphRunDispatchCommandCliOutput, Box<dyn Error>> {
    let verified =
        readiness_service(args)?.verify(graph_run_id, authorization_json, pricing_json)?;
    Ok(GroupAgentGraphRunDispatchCommandCliOutput::Readiness(
        Box::new(GroupAgentNodeDispatchReadinessCliOutput::verified(verified)),
    ))
}

fn readiness_service(
    args: &Args,
) -> Result<GroupAgentNodeDispatchReadinessService, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open_existing_current_read_only(database)?);
    Ok(GroupAgentNodeDispatchReadinessService::new(
        store.clone(),
        store,
        Arc::new(OpenAiRequestCodec),
        Arc::new(RegisteredGroupAgentNodeProviderFactory::new()),
    ))
}

/// Refuses pre-stdin when the run is not a stranded hard-crash claim.
///
/// This is the read-only ordering guard (design §7.4 step 1): it must run
/// before any stdin read and before any subprocess spawn. The service repeats
/// the guard authoritatively; this layer only guarantees ordering.
fn adjudication_stranded_guard(
    args: &Args,
    command: &GroupGraphRunDispatchCommand,
) -> Result<(), Box<dyn Error>> {
    let GroupGraphRunDispatchCommand::Adjudicate { graph_run_id, .. } = command else {
        return Ok(());
    };
    read_stranded_inspection(args, graph_run_id)?;
    Ok(())
}

/// Reads the stranded lifecycle through a read-only connection, or refuses.
fn read_stranded_inspection(
    args: &Args,
    graph_run_id: &str,
) -> Result<GroupAgentNodeLifecycleInspection, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    if !database.try_exists()? {
        return Err(not_stranded(graph_run_id, "no Hub database exists"));
    }
    let store = SqliteHubStore::open_existing_dispatch_inspection_read_only(database)?;
    let Some(inspection) = store.inspect_existing_group_agent_node_lifecycle(graph_run_id)? else {
        return Err(not_stranded(graph_run_id, "run has no stranded dispatch claim"));
    };
    let stranded = inspection.graph_run.run.status == GroupAgentGraphRunStatus::DispatchUnknown
        && inspection.active_lane.is_some()
        && inspection.artifact.is_none()
        && inspection.terminal_receipt.is_none();
    if stranded {
        return Ok(inspection);
    }
    Err(not_stranded(
        graph_run_id,
        &format!(
            "status={}, lane_active={}, artifact_present={}, receipt_present={}",
            status_text(inspection.graph_run.run.status),
            inspection.active_lane.is_some(),
            inspection.artifact.is_some(),
            inspection.terminal_receipt.is_some(),
        ),
    ))
}

fn not_stranded(graph_run_id: &str, observed: &str) -> Box<dyn Error> {
    AdjudicationRefused::NotStranded {
        reason: format!("{graph_run_id} is not a stranded hard-crash claim ({observed})"),
    }
    .into()
}

fn status_text(status: GroupAgentGraphRunStatus) -> &'static str {
    match status {
        GroupAgentGraphRunStatus::AwaitingExecutionContract => "awaiting_execution_contract",
        GroupAgentGraphRunStatus::AwaitingCoreDispatch => "awaiting_core_dispatch",
        GroupAgentGraphRunStatus::AwaitingDispatchAuthorization => {
            "awaiting_dispatch_authorization"
        }
        GroupAgentGraphRunStatus::DispatchUnknown => "dispatch_unknown",
        GroupAgentGraphRunStatus::Completed => "completed",
        GroupAgentGraphRunStatus::Failed => "failed",
        GroupAgentGraphRunStatus::FailedUncertain => "failed_uncertain",
    }
}

/// Compares the operator bodies against the persisted claim digests before any
/// subprocess spawn, so a wrong body is diagnosable instead of a deep redacted
/// Core failure (design §7.3 preflight).
fn adjudication_digest_preflight(
    claim: &GroupAgentNodeDispatchClaim,
    inputs: &DispatchInputs,
) -> Result<(), Box<dyn Error>> {
    let authorization = GroupAgentNodeDispatchAuthorization::decode_exact(inputs.authorization())
        .map_err(|_| GroupAgentNodeDispatchAdjudicationServiceError::InvalidInput)?;
    if authorization.authorization_sha256 != claim.authorization_sha256 {
        return Err(AdjudicationRefused::DigestMismatch {
            field: "authorization",
        }
        .into());
    }
    let pricing = GroupAgentNodePricingSnapshot::decode_exact(inputs.pricing())
        .map_err(|_| GroupAgentNodeDispatchAdjudicationServiceError::InvalidInput)?;
    if pricing.pricing_snapshot_sha256 != claim.pricing_snapshot_sha256 {
        return Err(AdjudicationRefused::DigestMismatch {
            field: "pricing",
        }
        .into());
    }
    Ok(())
}

fn execute_adjudicate(
    args: &Args,
    graph_run_id: &str,
    inputs: &DispatchInputs,
    core_bin: &str,
    core_bin_sha256: &str,
) -> Result<GroupAgentGraphRunDispatchCommandCliOutput, Box<dyn Error>> {
    // Re-guard after the stdin read (state may have changed), then the digest
    // preflight, then the bridge preflight subprocess, then the Core decide
    // subprocess: no subprocess is spawned before stdin is fully consumed.
    let inspection = read_stranded_inspection(args, graph_run_id)?;
    adjudication_digest_preflight(&inspection.claim, inputs)?;
    let bridge = Arc::new(PinnedCoreTerminalBridge::new(
        PathBuf::from(core_bin),
        core_bin_sha256.into(),
    )?);
    let service = adjudication_service(args, bridge)?;
    let result = service.adjudicate(&AdjudicateGroupAgentNodeDispatchInput {
        graph_run_id: graph_run_id.into(),
        authorization_json: inputs
            .authorization_json
            .clone()
            .expect("adjudicate authorization was read before bridge preflight"),
        pricing_json: inputs
            .pricing_json
            .clone()
            .expect("adjudicate pricing was read before bridge preflight"),
    })?;
    let AdjudicateGroupAgentNodeDispatchResult::Adjudicated(inspection) = result;
    Ok(GroupAgentGraphRunDispatchCommandCliOutput::Execution(
        Box::new(GroupAgentNodeDispatchExecutionCliOutput::from_adjudicated(
            inspection,
        )),
    ))
}

/// Wiring for the no-send adjudication service: store/codec/bridge/metadata
/// only. It must never call `execution_service()` and must never call
/// `PreparedDispatchDependencies::prepare` (which reads `OPENAI_API_KEY`); the
/// type-level no-credential guarantee depends on this function.
fn adjudication_service(
    args: &Args,
    bridge: Arc<PinnedCoreTerminalBridge>,
) -> Result<GroupAgentNodeDispatchAdjudicationService, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    Ok(GroupAgentNodeDispatchAdjudicationService::new(
        store.clone(),
        store.clone(),
        store.clone(),
        Arc::new(OpenAiRequestCodec),
        bridge,
        Arc::new(SystemDispatchMetadataSource),
    ))
}

#[cfg(test)]
#[path = "dispatch_command_tests.rs"]
mod tests;
