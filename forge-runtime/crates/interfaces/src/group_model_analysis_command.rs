use std::{error::Error, io, sync::Arc};

use forge_runtime_application::{
    GroupModelAnalysisService, PrepareGroupModelAnalysisInput, SendGroupModelAnalysisResult,
};
use forge_runtime_infrastructure::SqliteHubStore;

use crate::{
    args::{Args, GroupAnalysisCommand},
    group_context_output::terminal_text,
    group_model_analysis_output::{
        GroupModelAnalysisInspectionView, GroupModelAnalysisSendDisposition,
    },
    hub_output::{CliOutput, OutputKind},
    openai_prepared_dispatch::{DEFAULT_OPENAI_MODEL, OpenAiPreparedProvider, OpenAiRequestCodec},
    runtime_domain::{
        Cancellation, ClaimGroupModelAnalysisDispatch, GROUP_MODEL_ANALYSIS_CONSENT_VERSION,
        GroupModelAnalysisInspection, GroupModelAnalysisStatus, PrepareGroupModelAnalysisResult,
    },
    state_path::{hub_database_path, idempotency_key, unique_id, unix_time_millis},
};

pub async fn execute(
    args: &Args,
    command: &GroupAnalysisCommand,
) -> Result<CliOutput, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    let service =
        GroupModelAnalysisService::new(store.clone(), store, Arc::new(OpenAiRequestCodec));
    match command {
        GroupAnalysisCommand::Prepare {
            group_run_id,
            model,
            max_output_tokens,
        } => prepare(
            args,
            &service,
            group_run_id,
            model.as_deref(),
            *max_output_tokens,
        ),
        GroupAnalysisCommand::Send {
            analysis_id,
            confirm_off_machine,
            include_result,
        } => send(&service, analysis_id, *confirm_off_machine, *include_result).await,
        GroupAnalysisCommand::Show {
            analysis_id,
            include_result,
        } => show(&service, analysis_id, *include_result),
        GroupAnalysisCommand::List {
            group_run_id,
            limit,
        } => list(&service, group_run_id.as_deref(), *limit),
    }
}

/// `effective_analysis_endpoint` freezes the provider destination into a
/// prepared analysis: `OPENAI_BASE_URL` opt-in (self-hosted `/v1` gateway) or the
/// official endpoint, in `/v1/responses` form to match the provider's own
/// endpoint identity.
fn effective_analysis_endpoint() -> String {
    let base = crate::openai_prepared_dispatch::effective_openai_base_url();
    if base.ends_with("/responses") {
        base
    } else {
        format!("{base}/responses")
    }
}

fn prepare(
    args: &Args,
    service: &GroupModelAnalysisService,
    group_run_id: &str,
    model: Option<&str>,
    max_output_tokens: u32,
) -> Result<CliOutput, Box<dyn Error>> {
    let result = service.prepare(&PrepareGroupModelAnalysisInput {
        analysis_id: unique_id("group-analysis"),
        group_run_id: group_run_id.into(),
        model: model.unwrap_or(DEFAULT_OPENAI_MODEL).into(),
        endpoint: effective_analysis_endpoint(),
        max_output_tokens,
        idempotency_key: args
            .idempotency_key
            .clone()
            .unwrap_or_else(|| idempotency_key("group-analysis")),
        created_at_ms: unix_time_millis(),
    })?;
    Ok(prepared_output(result))
}

async fn send(
    service: &GroupModelAnalysisService,
    analysis_id: &str,
    confirmed: bool,
    include_result: bool,
) -> Result<CliOutput, Box<dyn Error>> {
    let inspection = service.inspect(analysis_id)?;
    if inspection.analysis.status != GroupModelAnalysisStatus::AwaitingConsent {
        return Ok(sent_output(
            GroupModelAnalysisSendDisposition::AlreadyClaimed,
            inspection,
            include_result,
        ));
    }
    require_consent(&inspection, confirmed)?;
    let provider = provider_for(&inspection)?;
    let released_at_ms = unix_time_millis();
    let result = service
        .send(
            &ClaimGroupModelAnalysisDispatch {
                v: inspection.analysis.v,
                analysis_id: analysis_id.into(),
                dispatch_id: unique_id("group-analysis-dispatch"),
                consent_version: GROUP_MODEL_ANALYSIS_CONSENT_VERSION,
                released_at_ms,
            },
            &provider,
            Cancellation::default(),
            unix_time_millis().max(released_at_ms),
        )
        .await?;
    match result {
        SendGroupModelAnalysisResult::AlreadyClaimed { inspection } => Ok(sent_output(
            GroupModelAnalysisSendDisposition::AlreadyClaimed,
            inspection,
            include_result,
        )),
        SendGroupModelAnalysisResult::Completed { completion } => Ok(sent_output(
            GroupModelAnalysisSendDisposition::DispatchedAndCompleted,
            completion.inspection,
            include_result,
        )),
    }
}

fn show(
    service: &GroupModelAnalysisService,
    analysis_id: &str,
    include_result: bool,
) -> Result<CliOutput, Box<dyn Error>> {
    let inspection = service.inspect(analysis_id)?;
    Ok(CliOutput::new(OutputKind::GroupModelAnalysis {
        inspection: GroupModelAnalysisInspectionView::from_inspection(inspection, include_result),
    }))
}

fn list(
    service: &GroupModelAnalysisService,
    group_run_id: Option<&str>,
    limit: usize,
) -> Result<CliOutput, Box<dyn Error>> {
    Ok(CliOutput::new(OutputKind::GroupModelAnalyses {
        metadata_only: true,
        source_and_journal_validated: false,
        inspect_with: "group analysis show ANALYSIS_ID",
        analyses: service.list(group_run_id, limit)?,
    }))
}

fn prepared_output(result: PrepareGroupModelAnalysisResult) -> CliOutput {
    CliOutput::new(OutputKind::GroupModelAnalysisPrepared {
        disposition: result.disposition,
        inspection: GroupModelAnalysisInspectionView::from_inspection(result.inspection, false),
    })
}

fn sent_output(
    disposition: GroupModelAnalysisSendDisposition,
    inspection: GroupModelAnalysisInspection,
    include_result: bool,
) -> CliOutput {
    CliOutput::new(OutputKind::GroupModelAnalysisSent {
        disposition,
        inspection: GroupModelAnalysisInspectionView::from_inspection(inspection, include_result),
    })
}

fn require_consent(
    inspection: &GroupModelAnalysisInspection,
    confirmed: bool,
) -> Result<(), io::Error> {
    if confirmed {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        format!(
            "group analysis send requires --confirm-off-machine before sending the complete \
             frozen dossier to {} with model {}",
            terminal_text(&inspection.analysis.config.endpoint),
            terminal_text(&inspection.analysis.config.model)
        ),
    ))
}

fn provider_for(
    inspection: &GroupModelAnalysisInspection,
) -> Result<OpenAiPreparedProvider, Box<dyn Error>> {
    let provider = OpenAiPreparedProvider::from_environment(&inspection.analysis.config.model)?;
    if provider.endpoint() != inspection.analysis.config.endpoint
        || provider.model() != inspection.analysis.config.model
    {
        return Err("configured provider destination does not match the prepared analysis".into());
    }
    Ok(provider)
}
