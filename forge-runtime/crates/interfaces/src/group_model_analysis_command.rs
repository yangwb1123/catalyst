use std::{env, error::Error, io, sync::Arc};

use forge_runtime_application::{
    GroupModelAnalysisDispatchProvider, GroupModelAnalysisRequestCodec, GroupModelAnalysisService,
    PrepareGroupModelAnalysisInput, SendGroupModelAnalysisResult,
};
use forge_runtime_infrastructure::{OpenAiResponsesProvider, SqliteHubStore};

use crate::{
    args::{Args, GroupAnalysisCommand},
    group_context_output::terminal_text,
    group_model_analysis_output::{
        GroupModelAnalysisInspectionView, GroupModelAnalysisSendDisposition,
    },
    hub_output::{CliOutput, OutputKind},
    runtime_domain::{
        Cancellation, ClaimGroupModelAnalysisDispatch, GROUP_MODEL_ANALYSIS_CONSENT_VERSION,
        GroupModelAnalysisInspection, GroupModelAnalysisProvider, GroupModelAnalysisStatus,
        ModelEventStream, ModelRequest, PrepareGroupModelAnalysisResult, PreparedModelProvider,
        PreparedModelRequest, ProviderError,
    },
    state_path::{hub_database_path, idempotency_key, unique_id, unix_time_millis},
};

const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_OPENAI_MODEL: &str = "gpt-5.6-sol";

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
) -> Result<AnalysisOpenAiProvider, Box<dyn Error>> {
    let api_key = env::var("OPENAI_API_KEY")
        .map_err(|_| "OPENAI_API_KEY is required after explicit off-machine consent")?;
    if api_key.trim().is_empty() {
        return Err("OPENAI_API_KEY must not be empty after explicit off-machine consent".into());
    }
    let provider =
        OpenAiResponsesProvider::new(OPENAI_BASE_URL, &inspection.analysis.config.model, api_key)?;
    if provider.endpoint() != inspection.analysis.config.endpoint
        || provider.model() != inspection.analysis.config.model
    {
        return Err("configured provider destination does not match the prepared analysis".into());
    }
    Ok(AnalysisOpenAiProvider { inner: provider })
}

struct AnalysisOpenAiProvider {
    inner: OpenAiResponsesProvider,
}

impl PreparedModelProvider for AnalysisOpenAiProvider {
    fn stream_prepared(&self, request: PreparedModelRequest) -> ModelEventStream {
        self.inner.stream_prepared(request)
    }
}

impl GroupModelAnalysisDispatchProvider for AnalysisOpenAiProvider {
    fn analysis_provider(&self) -> GroupModelAnalysisProvider {
        GroupModelAnalysisProvider::OpenAiResponses
    }

    fn endpoint(&self) -> &str {
        self.inner.endpoint()
    }

    fn model(&self) -> &str {
        self.inner.model()
    }
}

struct OpenAiRequestCodec;

impl GroupModelAnalysisRequestCodec for OpenAiRequestCodec {
    fn encode_request(
        &self,
        model: &str,
        request: &ModelRequest,
    ) -> Result<Vec<u8>, ProviderError> {
        OpenAiResponsesProvider::encode_request_bytes(model, request)
    }

    fn validate_exact_request(
        &self,
        model: &str,
        expected: &ModelRequest,
        actual: &[u8],
    ) -> Result<(), ProviderError> {
        OpenAiResponsesProvider::validate_exact_request_bytes(model, expected, actual)
    }
}
