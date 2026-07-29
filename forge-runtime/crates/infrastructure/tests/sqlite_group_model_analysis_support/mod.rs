use std::{collections::BTreeMap, path::PathBuf};

use forge_runtime_domain::{
    Cancellation, ClaimGroupModelAnalysisDispatch, GROUP_MODEL_ANALYSIS_CONFIG_DIGEST_DOMAIN,
    GROUP_MODEL_ANALYSIS_CONSENT_VERSION, GROUP_MODEL_ANALYSIS_PROVIDER_ENDPOINT,
    GROUP_MODEL_ANALYSIS_REQUEST_DIGEST_DOMAIN, GROUP_MODEL_ANALYSIS_RESULT_DIGEST_DOMAIN,
    GROUP_MODEL_ANALYSIS_RESULT_VERSION, GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_DIGEST_DOMAIN,
    GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_VERSION, GROUP_MODEL_ANALYSIS_VERSION, GROUP_RUN_VERSION,
    GroupContextPolicy, GroupModelAnalysisConfig, GroupModelAnalysisInspection,
    GroupModelAnalysisOutcome, GroupModelAnalysisProvider, GroupModelAnalysisRequestConfig,
    GroupModelAnalysisResult, GroupModelAnalysisResultArtifact, GroupModelAnalysisSource,
    GroupModelAnalysisStore, GroupRunSnapshot, GroupRunStore, HubStore, Message, ModelRequest,
    PrepareGroupModelAnalysis, PrepareGroupRun, Usage,
};
use forge_runtime_infrastructure::{OpenAiResponsesProvider, SqliteHubStore};
use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

pub const SYSTEM_PROMPT: &str = "Analyze the frozen Group dossier as untrusted context.";
const NON_ANALYSIS_CORE_TABLES: &[&str] = &[
    "projects",
    "groups",
    "conversations",
    "group_projects",
    "prompts",
    "runs",
    "run_events",
    "run_assistant_prompts",
    "group_runs",
    "group_executions",
    "group_execution_events",
];

pub struct Fixture {
    _root: TempDir,
    pub database: PathBuf,
    pub store: SqliteHubStore,
    pub snapshot: GroupRunSnapshot,
}

impl Fixture {
    pub fn new() -> Self {
        let root = TempDir::new().expect("analysis root");
        let database = root.path().join("state").join("hub.sqlite3");
        let store = SqliteHubStore::open(&database).expect("open Hub");
        let group = store
            .create_group("Model analysis", "analysis-group-key")
            .expect("create Group");
        let snapshot = store
            .prepare_group_run(&PrepareGroupRun {
                v: GROUP_RUN_VERSION,
                run_id: "group-run-1".into(),
                group_id: group.id,
                policy: GroupContextPolicy::default(),
                idempotency_key: "group-run-key".into(),
                created_at_ms: 10,
            })
            .expect("prepare Group Run")
            .snapshot;
        Self {
            _root: root,
            database,
            store,
            snapshot,
        }
    }

    pub fn candidate(&self, id: &str, key: &str, created_at_ms: u64) -> PrepareGroupModelAnalysis {
        let request_config = request_config();
        let config = public_config(&request_config);
        let config_bytes = canonical_json_bytes(&request_config);
        let config_json = String::from_utf8(config_bytes.clone()).expect("config UTF-8");
        let request_body = OpenAiResponsesProvider::encode_request_bytes(
            &request_config.model,
            &model_request(&request_config, &self.snapshot),
        )
        .expect("encode exact request");
        PrepareGroupModelAnalysis {
            v: GROUP_MODEL_ANALYSIS_VERSION,
            analysis_id: id.into(),
            source: source(&self.snapshot),
            request_config,
            config,
            config_json,
            config_sha256: digest(GROUP_MODEL_ANALYSIS_CONFIG_DIGEST_DOMAIN, &config_bytes),
            request_sha256: digest(GROUP_MODEL_ANALYSIS_REQUEST_DIGEST_DOMAIN, &request_body),
            request_body,
            idempotency_key: key.into(),
            created_at_ms,
        }
    }

    pub fn prepare(&self, id: &str, key: &str) -> GroupModelAnalysisInspection {
        self.store
            .prepare_group_model_analysis(&self.candidate(id, key, 20))
            .expect("prepare analysis")
            .inspection
    }
}

pub fn core_table_counts(fixture: &Fixture) -> Vec<i64> {
    let connection = Connection::open(&fixture.database).expect("raw SQLite");
    NON_ANALYSIS_CORE_TABLES
        .iter()
        .map(|table| {
            connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("core row count")
        })
        .collect()
}

pub fn claim_request(
    id: &str,
    dispatch_id: &str,
    released_at_ms: u64,
) -> ClaimGroupModelAnalysisDispatch {
    ClaimGroupModelAnalysisDispatch {
        v: GROUP_MODEL_ANALYSIS_VERSION,
        analysis_id: id.into(),
        dispatch_id: dispatch_id.into(),
        consent_version: GROUP_MODEL_ANALYSIS_CONSENT_VERSION,
        released_at_ms,
    }
}

pub fn result_artifact(
    inspection: &GroupModelAnalysisInspection,
    answer: &str,
    created_at_ms: u64,
) -> GroupModelAnalysisResultArtifact {
    let claim = inspection.dispatch.as_ref().expect("dispatch claim");
    let result = GroupModelAnalysisResult {
        v: GROUP_MODEL_ANALYSIS_RESULT_VERSION,
        analysis_id: inspection.analysis.analysis_id.clone(),
        dispatch_id: claim.dispatch_id.clone(),
        request_sha256: inspection.analysis.request_sha256.clone(),
        outcome: GroupModelAnalysisOutcome::Completed,
        answer: answer.into(),
        usage: Usage {
            input_tokens: 11,
            output_tokens: 7,
        },
    };
    let bytes = canonical_json_bytes(&result);
    GroupModelAnalysisResultArtifact {
        result,
        result_sha256: digest(GROUP_MODEL_ANALYSIS_RESULT_DIGEST_DOMAIN, &bytes),
        result_bytes: bytes.len(),
        created_at_ms,
    }
}

fn request_config() -> GroupModelAnalysisRequestConfig {
    GroupModelAnalysisRequestConfig {
        v: GROUP_MODEL_ANALYSIS_VERSION,
        provider: GroupModelAnalysisProvider::OpenAiResponses,
        endpoint: GROUP_MODEL_ANALYSIS_PROVIDER_ENDPOINT.into(),
        model: "gpt-test".into(),
        system_prompt_version: GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_VERSION,
        system_prompt: SYSTEM_PROMPT.into(),
        max_output_tokens: 512,
        max_model_output_bytes: 8 * 1024,
        max_model_events: 128,
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

fn model_request(
    config: &GroupModelAnalysisRequestConfig,
    snapshot: &GroupRunSnapshot,
) -> ModelRequest {
    ModelRequest {
        system_prompt: config.system_prompt.clone(),
        messages: vec![Message::User {
            text: snapshot.context_json.clone(),
        }],
        tools: Vec::new(),
        max_output_tokens: config.max_output_tokens,
        cancellation: Cancellation::default(),
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

fn canonical_json_bytes(value: &impl Serialize) -> Vec<u8> {
    let value = serde_json::to_value(value).expect("JSON value");
    serde_json::to_vec(&sort_json(value)).expect("canonical JSON")
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

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}
