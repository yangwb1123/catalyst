use std::{collections::BTreeMap, sync::Arc};

use forge_runtime_application::{
    GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT, GroupAnalysisPanelService, PrepareGroupAnalysisPanelInput,
};
use forge_runtime_domain::{
    GROUP_CONTEXT_DIGEST_DOMAIN, GROUP_CONTEXT_VERSION, GROUP_MODEL_ANALYSIS_CONFIG_DIGEST_DOMAIN,
    GROUP_MODEL_ANALYSIS_CONSENT_VERSION, GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION,
    GROUP_MODEL_ANALYSIS_PROVIDER_ENDPOINT, GROUP_MODEL_ANALYSIS_REQUEST_DIGEST_DOMAIN,
    GROUP_MODEL_ANALYSIS_RESULT_DIGEST_DOMAIN, GROUP_MODEL_ANALYSIS_RESULT_VERSION,
    GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_DIGEST_DOMAIN, GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_VERSION,
    GROUP_MODEL_ANALYSIS_VERSION, GROUP_RUN_SNAPSHOT_DIGEST_DOMAIN, GROUP_RUN_VERSION,
    GroupContextPayload, GroupContextPolicy, GroupContextSlice, GroupContextStats,
    GroupModelAnalysisConfig, GroupModelAnalysisDispatchClaim, GroupModelAnalysisEvent,
    GroupModelAnalysisEventKind, GroupModelAnalysisInspection, GroupModelAnalysisOutcome,
    GroupModelAnalysisPreparedReceipt, GroupModelAnalysisProvider, GroupModelAnalysisRecord,
    GroupModelAnalysisRequestConfig, GroupModelAnalysisResult, GroupModelAnalysisResultArtifact,
    GroupModelAnalysisResultReceipt, GroupModelAnalysisSource, GroupModelAnalysisStatus,
    GroupRunRecord, GroupRunSnapshot, GroupRunStatus, MAX_GROUP_MODEL_ANALYSIS_MODEL_EVENTS,
    MAX_GROUP_MODEL_ANALYSIS_OUTPUT_BYTES, SessionGroup, Usage,
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::stores::{MemoryAnalysisStore, MemoryPanelStore, MemoryRunStore};

pub(crate) const GROUP_RUN_ID: &str = "group-run-1";

pub(crate) struct Harness {
    pub(crate) service: GroupAnalysisPanelService,
    pub(crate) analyses: Arc<MemoryAnalysisStore>,
    pub(crate) panels: Arc<MemoryPanelStore>,
    pub(crate) snapshot: GroupRunSnapshot,
}

pub(crate) fn harness() -> Harness {
    let snapshot = snapshot(GROUP_RUN_ID, "group-1");
    let analyses = Arc::new(MemoryAnalysisStore::new([
        completed_analysis(
            "analysis-a",
            &snapshot,
            GroupModelAnalysisOutcome::Completed,
            "frontend findings",
        ),
        completed_analysis(
            "analysis-b",
            &snapshot,
            GroupModelAnalysisOutcome::Completed,
            "backend findings",
        ),
    ]));
    let runs = Arc::new(MemoryRunStore::new([snapshot.clone()]));
    let panels = Arc::new(MemoryPanelStore::default());
    let service = GroupAnalysisPanelService::new(runs, analyses.clone(), panels.clone());
    Harness {
        service,
        analyses,
        panels,
        snapshot,
    }
}

pub(crate) fn prepare_input(analysis_ids: &[&str]) -> PrepareGroupAnalysisPanelInput {
    PrepareGroupAnalysisPanelInput {
        panel_id: "panel-1".into(),
        group_run_id: GROUP_RUN_ID.into(),
        analysis_ids: analysis_ids.iter().map(|id| (*id).into()).collect(),
        idempotency_key: "panel-key".into(),
        created_at_ms: 50,
    }
}

pub(crate) fn other_snapshot() -> GroupRunSnapshot {
    snapshot("group-run-2", "group-2")
}

pub(crate) fn completed_analysis(
    id: &str,
    snapshot: &GroupRunSnapshot,
    outcome: GroupModelAnalysisOutcome,
    answer: &str,
) -> GroupModelAnalysisInspection {
    let source = source(snapshot);
    let record = analysis_record(id, snapshot, GroupModelAnalysisStatus::Completed);
    let prepared = prepared_receipt(&record, source);
    let claim = dispatch_claim(&record);
    let result = result_artifact(&record, &claim, outcome, answer);
    let completion = result_receipt(&result);
    let events = terminal_events(id, &prepared, &claim, &completion);
    GroupModelAnalysisInspection::validate(record, events, Some(result))
        .expect("valid completed analysis fixture")
}

pub(crate) fn awaiting_analysis(
    id: &str,
    snapshot: &GroupRunSnapshot,
) -> GroupModelAnalysisInspection {
    let record = analysis_record(id, snapshot, GroupModelAnalysisStatus::AwaitingConsent);
    let prepared = prepared_receipt(&record, source(snapshot));
    let event = GroupModelAnalysisEvent {
        v: GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION,
        analysis_id: id.into(),
        seq: 1,
        kind: GroupModelAnalysisEventKind::AnalysisPrepared { receipt: prepared },
    };
    GroupModelAnalysisInspection::validate(record, vec![event], None)
        .expect("valid awaiting analysis fixture")
}

pub(crate) fn bad_result_digest(
    mut inspection: GroupModelAnalysisInspection,
) -> GroupModelAnalysisInspection {
    let digest = "0".repeat(64);
    inspection
        .result
        .as_mut()
        .expect("result")
        .result_sha256
        .clone_from(&digest);
    inspection
        .completion
        .as_mut()
        .expect("completion")
        .result_sha256
        .clone_from(&digest);
    if let GroupModelAnalysisEventKind::AnalysisCompleted { receipt } =
        &mut inspection.events[2].kind
    {
        receipt.result_sha256 = digest;
    }
    inspection
}

pub(crate) fn contradictory_prepared_source(
    mut inspection: GroupModelAnalysisInspection,
) -> GroupModelAnalysisInspection {
    inspection
        .prepared
        .as_mut()
        .expect("prepared receipt")
        .source
        .group_id = "contradictory-group".into();
    let GroupModelAnalysisEventKind::AnalysisPrepared { receipt } = &mut inspection.events[0].kind
    else {
        panic!("first event is prepared");
    };
    receipt.source.group_id = "contradictory-group".into();
    inspection
}

fn snapshot(run_id: &str, group_id: &str) -> GroupRunSnapshot {
    let payload = GroupContextPayload {
        policy: GroupContextPolicy::default(),
        group: SessionGroup {
            id: group_id.into(),
            name: format!("Group {group_id}"),
            created_at_ms: 1,
        },
        members: Vec::new(),
        conversations: Vec::new(),
        stats: GroupContextStats::default(),
    };
    let slice_sha256 = digest(
        GROUP_CONTEXT_DIGEST_DOMAIN,
        &canonical(&payload).expect("context payload"),
    );
    snapshot_from_payload(run_id, group_id, payload, slice_sha256)
}

fn snapshot_from_payload(
    run_id: &str,
    group_id: &str,
    payload: GroupContextPayload,
    slice_sha256: String,
) -> GroupRunSnapshot {
    let context = GroupContextSlice {
        v: GROUP_CONTEXT_VERSION,
        payload,
        slice_sha256: slice_sha256.clone(),
    };
    let bytes = canonical(&context).expect("context");
    GroupRunSnapshot {
        v: GROUP_RUN_VERSION,
        run: GroupRunRecord {
            v: GROUP_RUN_VERSION,
            run_id: run_id.into(),
            group_id: group_id.into(),
            status: GroupRunStatus::Prepared,
            context_version: GROUP_CONTEXT_VERSION,
            context_slice_sha256: slice_sha256,
            snapshot_sha256: digest(GROUP_RUN_SNAPSHOT_DIGEST_DOMAIN, &bytes),
            snapshot_bytes: bytes.len(),
            created_at_ms: 5,
        },
        context,
        context_json: String::from_utf8(bytes).expect("context UTF-8"),
    }
}

fn analysis_record(
    id: &str,
    snapshot: &GroupRunSnapshot,
    status: GroupModelAnalysisStatus,
) -> GroupModelAnalysisRecord {
    let request_config = request_config();
    let config = public_config(&request_config);
    let request = format!("request for {id}").into_bytes();
    GroupModelAnalysisRecord {
        v: GROUP_MODEL_ANALYSIS_VERSION,
        analysis_id: id.into(),
        group_run_id: snapshot.run.run_id.clone(),
        status,
        source_snapshot_sha256: snapshot.run.snapshot_sha256.clone(),
        config,
        config_sha256: digest(
            GROUP_MODEL_ANALYSIS_CONFIG_DIGEST_DOMAIN,
            &canonical(&request_config).expect("request config"),
        ),
        request_sha256: digest(GROUP_MODEL_ANALYSIS_REQUEST_DIGEST_DOMAIN, &request),
        request_bytes: request.len(),
        protocol_version: GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION,
        created_at_ms: 10,
    }
}

fn request_config() -> GroupModelAnalysisRequestConfig {
    GroupModelAnalysisRequestConfig {
        v: GROUP_MODEL_ANALYSIS_VERSION,
        provider: GroupModelAnalysisProvider::OpenAiResponses,
        endpoint: GROUP_MODEL_ANALYSIS_PROVIDER_ENDPOINT.into(),
        model: "gpt-test".into(),
        system_prompt_version: GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_VERSION,
        system_prompt: GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT.into(),
        max_output_tokens: 64,
        max_model_output_bytes: MAX_GROUP_MODEL_ANALYSIS_OUTPUT_BYTES,
        max_model_events: MAX_GROUP_MODEL_ANALYSIS_MODEL_EVENTS,
    }
}

fn public_config(config: &GroupModelAnalysisRequestConfig) -> GroupModelAnalysisConfig {
    GroupModelAnalysisConfig {
        v: config.v,
        provider: config.provider,
        endpoint: config.endpoint.clone(),
        model: config.model.clone(),
        system_prompt_version: config.system_prompt_version,
        system_prompt_sha256: digest(
            GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_DIGEST_DOMAIN,
            config.system_prompt.as_bytes(),
        ),
        max_output_tokens: config.max_output_tokens,
        max_model_output_bytes: config.max_model_output_bytes,
        max_model_events: config.max_model_events,
    }
}

fn source(snapshot: &GroupRunSnapshot) -> GroupModelAnalysisSource {
    GroupModelAnalysisSource {
        group_run_version: snapshot.run.v,
        group_run_id: snapshot.run.run_id.clone(),
        group_id: snapshot.run.group_id.clone(),
        context_version: snapshot.run.context_version,
        context_slice_sha256: snapshot.run.context_slice_sha256.clone(),
        snapshot_sha256: snapshot.run.snapshot_sha256.clone(),
        snapshot_bytes: snapshot.run.snapshot_bytes,
    }
}

fn prepared_receipt(
    record: &GroupModelAnalysisRecord,
    source: GroupModelAnalysisSource,
) -> GroupModelAnalysisPreparedReceipt {
    GroupModelAnalysisPreparedReceipt {
        v: GROUP_MODEL_ANALYSIS_VERSION,
        analysis_id: record.analysis_id.clone(),
        source,
        config_sha256: record.config_sha256.clone(),
        request_sha256: record.request_sha256.clone(),
        request_bytes: record.request_bytes,
    }
}

fn dispatch_claim(record: &GroupModelAnalysisRecord) -> GroupModelAnalysisDispatchClaim {
    GroupModelAnalysisDispatchClaim {
        v: GROUP_MODEL_ANALYSIS_VERSION,
        analysis_id: record.analysis_id.clone(),
        dispatch_id: format!("dispatch-{}", record.analysis_id),
        request_sha256: record.request_sha256.clone(),
        config_sha256: record.config_sha256.clone(),
        provider: record.config.provider,
        endpoint: record.config.endpoint.clone(),
        model: record.config.model.clone(),
        consent_version: GROUP_MODEL_ANALYSIS_CONSENT_VERSION,
        released_at_ms: 20,
    }
}

fn result_artifact(
    record: &GroupModelAnalysisRecord,
    claim: &GroupModelAnalysisDispatchClaim,
    outcome: GroupModelAnalysisOutcome,
    answer: &str,
) -> GroupModelAnalysisResultArtifact {
    let result = GroupModelAnalysisResult {
        v: GROUP_MODEL_ANALYSIS_RESULT_VERSION,
        analysis_id: record.analysis_id.clone(),
        dispatch_id: claim.dispatch_id.clone(),
        request_sha256: record.request_sha256.clone(),
        outcome,
        answer: answer.into(),
        usage: Usage {
            input_tokens: 20,
            output_tokens: 4,
        },
    };
    let bytes = canonical(&result).expect("result");
    GroupModelAnalysisResultArtifact {
        result,
        result_sha256: digest(GROUP_MODEL_ANALYSIS_RESULT_DIGEST_DOMAIN, &bytes),
        result_bytes: bytes.len(),
        created_at_ms: 30,
    }
}

fn result_receipt(artifact: &GroupModelAnalysisResultArtifact) -> GroupModelAnalysisResultReceipt {
    let result = &artifact.result;
    GroupModelAnalysisResultReceipt {
        v: result.v,
        analysis_id: result.analysis_id.clone(),
        dispatch_id: result.dispatch_id.clone(),
        request_sha256: result.request_sha256.clone(),
        outcome: result.outcome,
        result_sha256: artifact.result_sha256.clone(),
        result_bytes: artifact.result_bytes,
        usage: result.usage,
        created_at_ms: artifact.created_at_ms,
    }
}

fn terminal_events(
    id: &str,
    prepared: &GroupModelAnalysisPreparedReceipt,
    claim: &GroupModelAnalysisDispatchClaim,
    completion: &GroupModelAnalysisResultReceipt,
) -> Vec<GroupModelAnalysisEvent> {
    vec![
        event(
            id,
            1,
            GroupModelAnalysisEventKind::AnalysisPrepared {
                receipt: prepared.clone(),
            },
        ),
        event(
            id,
            2,
            GroupModelAnalysisEventKind::ProviderDispatchReleased {
                claim: claim.clone(),
            },
        ),
        event(
            id,
            3,
            GroupModelAnalysisEventKind::AnalysisCompleted {
                receipt: completion.clone(),
            },
        ),
    ]
}

fn event(id: &str, seq: u64, kind: GroupModelAnalysisEventKind) -> GroupModelAnalysisEvent {
    GroupModelAnalysisEvent {
        v: GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION,
        analysis_id: id.into(),
        seq,
        kind,
    }
}

pub(crate) fn canonical(value: &impl Serialize) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_value(value).and_then(|value| serde_json::to_vec(&sort_json(value)))
}

pub(crate) fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn sort_json(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(sort_json).collect()),
        Value::Object(items) => {
            let sorted = items
                .into_iter()
                .map(|(key, value)| (key, sort_json(value)))
                .collect::<BTreeMap<_, _>>();
            Value::Object(sorted.into_iter().collect())
        }
        other => other,
    }
}
