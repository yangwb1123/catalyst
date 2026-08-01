use std::{error::Error, io, sync::Arc};

use forge_runtime_application::{
    GroupAnalysisPanelService, GroupPanelSynthesisService, PrepareGroupPanelSynthesisInput,
    SendGroupPanelSynthesisResult,
};
use forge_runtime_infrastructure::SqliteHubStore;

use crate::{
    args::{Args, GroupSynthesisCommand},
    group_context_output::terminal_text,
    group_panel_synthesis_output::{
        GroupPanelSynthesisInspectionView, GroupPanelSynthesisListItemView,
        GroupPanelSynthesisSendDisposition,
    },
    hub_output::{CliOutput, OutputKind},
    openai_prepared_dispatch::{DEFAULT_OPENAI_MODEL, OpenAiPreparedProvider, OpenAiRequestCodec},
    runtime_domain::{
        Cancellation, ClaimGroupPanelSynthesisDispatch, GROUP_PANEL_SYNTHESIS_CONSENT_VERSION,
        GROUP_PANEL_SYNTHESIS_PROVIDER_ENDPOINT, GroupPanelSynthesisInspection,
        GroupPanelSynthesisStatus, PrepareGroupPanelSynthesisResult,
    },
    state_path::{hub_database_path, idempotency_key, unique_id, unix_time_millis},
};

pub async fn execute(
    args: &Args,
    command: &GroupSynthesisCommand,
) -> Result<CliOutput, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    let panels = Arc::new(GroupAnalysisPanelService::new(
        store.clone(),
        store.clone(),
        store.clone(),
    ));
    let service = GroupPanelSynthesisService::new(panels, store, Arc::new(OpenAiRequestCodec));
    match command {
        GroupSynthesisCommand::Prepare {
            panel_id,
            model,
            max_output_tokens,
        } => prepare(
            args,
            &service,
            panel_id,
            model.as_deref(),
            *max_output_tokens,
        ),
        GroupSynthesisCommand::Send {
            synthesis_id,
            confirm_off_machine,
            include_result,
        } => {
            send(
                &service,
                synthesis_id,
                *confirm_off_machine,
                *include_result,
            )
            .await
        }
        GroupSynthesisCommand::Show {
            synthesis_id,
            include_result,
        } => show(&service, synthesis_id, *include_result),
        GroupSynthesisCommand::List { panel_id, limit } => {
            list(&service, panel_id.as_deref(), *limit)
        }
    }
}

fn prepare(
    args: &Args,
    service: &GroupPanelSynthesisService,
    panel_id: &str,
    model: Option<&str>,
    max_output_tokens: u32,
) -> Result<CliOutput, Box<dyn Error>> {
    let result = service.prepare(&PrepareGroupPanelSynthesisInput {
        synthesis_id: unique_id("group-synthesis"),
        panel_id: panel_id.into(),
        model: model.unwrap_or(DEFAULT_OPENAI_MODEL).into(),
        max_output_tokens,
        idempotency_key: args
            .idempotency_key
            .clone()
            .unwrap_or_else(|| idempotency_key("group-synthesis")),
        created_at_ms: unix_time_millis(),
    })?;
    Ok(prepared_output(result))
}

async fn send(
    service: &GroupPanelSynthesisService,
    synthesis_id: &str,
    confirmed: bool,
    include_result: bool,
) -> Result<CliOutput, Box<dyn Error>> {
    let inspection = service.inspect(synthesis_id)?;
    if inspection.synthesis.status != GroupPanelSynthesisStatus::AwaitingConsent {
        return Ok(sent_output(
            GroupPanelSynthesisSendDisposition::AlreadyClaimed,
            inspection,
            include_result,
        ));
    }
    require_consent(&inspection, confirmed)?;
    let provider = provider_for(&inspection)?;
    let released_at_ms = unix_time_millis();
    let result = service
        .send(
            &ClaimGroupPanelSynthesisDispatch {
                v: inspection.synthesis.v,
                synthesis_id: synthesis_id.into(),
                dispatch_id: unique_id("group-synthesis-dispatch"),
                consent_version: GROUP_PANEL_SYNTHESIS_CONSENT_VERSION,
                released_at_ms,
            },
            true,
            &provider,
            Cancellation::default(),
            unix_time_millis().max(released_at_ms),
        )
        .await?;
    match result {
        SendGroupPanelSynthesisResult::AlreadyClaimed { inspection } => Ok(sent_output(
            GroupPanelSynthesisSendDisposition::AlreadyClaimed,
            inspection,
            include_result,
        )),
        SendGroupPanelSynthesisResult::Completed { completion } => Ok(sent_output(
            GroupPanelSynthesisSendDisposition::DispatchedAndCompleted,
            completion.inspection,
            include_result,
        )),
    }
}

fn show(
    service: &GroupPanelSynthesisService,
    synthesis_id: &str,
    include_result: bool,
) -> Result<CliOutput, Box<dyn Error>> {
    let inspection = service.inspect(synthesis_id)?;
    Ok(CliOutput::new(OutputKind::GroupPanelSynthesis {
        inspection: GroupPanelSynthesisInspectionView::from_inspection(inspection, include_result),
    }))
}

fn list(
    service: &GroupPanelSynthesisService,
    panel_id: Option<&str>,
    limit: usize,
) -> Result<CliOutput, Box<dyn Error>> {
    Ok(CliOutput::new(OutputKind::GroupPanelSyntheses {
        metadata_only: true,
        source_and_journal_validated: false,
        inspect_with: "group synthesis show SYNTHESIS_ID",
        syntheses: service
            .list(panel_id, limit)?
            .into_iter()
            .map(GroupPanelSynthesisListItemView::from_record)
            .collect(),
    }))
}

fn prepared_output(result: PrepareGroupPanelSynthesisResult) -> CliOutput {
    CliOutput::new(OutputKind::GroupPanelSynthesisPrepared {
        disposition: result.disposition,
        inspection: GroupPanelSynthesisInspectionView::from_inspection(result.inspection, false),
    })
}

fn sent_output(
    disposition: GroupPanelSynthesisSendDisposition,
    inspection: GroupPanelSynthesisInspection,
    include_result: bool,
) -> CliOutput {
    CliOutput::new(OutputKind::GroupPanelSynthesisSent {
        disposition,
        inspection: GroupPanelSynthesisInspectionView::from_inspection(inspection, include_result),
    })
}

fn require_consent(
    inspection: &GroupPanelSynthesisInspection,
    confirmed: bool,
) -> Result<(), io::Error> {
    if confirmed {
        return Ok(());
    }
    let synthesis = &inspection.synthesis;
    let count = inspection
        .prepared
        .as_ref()
        .map_or(0, |receipt| receipt.source.analysis_count);
    Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        format!(
            "group synthesis send requires --confirm-off-machine. The exact prepared request \
             ({} bytes; sha256 {}) contains the text of all {} ordered copied panel results plus \
             panel/source metadata, and does not separately attach Group dossier or excerpt \
             fields. Copied result text may itself quote or reproduce source content. The \
             request would be sent off-machine to {} with model {}. Prior Group analysis \
             consent does not authorize this disclosure",
            synthesis.request_bytes,
            synthesis.request_sha256,
            count,
            terminal_text(&synthesis.config.endpoint),
            terminal_text(&synthesis.config.model),
        ),
    ))
}

fn provider_for(
    inspection: &GroupPanelSynthesisInspection,
) -> Result<OpenAiPreparedProvider, Box<dyn Error>> {
    let config = &inspection.synthesis.config;
    if config.endpoint != GROUP_PANEL_SYNTHESIS_PROVIDER_ENDPOINT || config.model.trim().is_empty()
    {
        return Err("prepared synthesis destination is invalid".into());
    }
    let provider = OpenAiPreparedProvider::from_environment(&config.model)?;
    if provider.endpoint() != config.endpoint || provider.model() != config.model {
        return Err("configured provider destination does not match the prepared synthesis".into());
    }
    Ok(provider)
}
