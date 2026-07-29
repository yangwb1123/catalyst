use forge_runtime_domain::{
    ClaimGroupModelAnalysisDispatch, CompleteGroupModelAnalysis,
    GROUP_MODEL_ANALYSIS_CONSENT_VERSION, GROUP_MODEL_ANALYSIS_PROVIDER_ENDPOINT,
    GROUP_MODEL_ANALYSIS_RESULT_VERSION, GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_VERSION,
    GROUP_MODEL_ANALYSIS_VERSION, GROUP_RUN_VERSION, GroupContextPolicy, GroupModelAnalysisOutcome,
    GroupModelAnalysisProvider, GroupModelAnalysisRecovery, GroupModelAnalysisRequestConfig,
    GroupModelAnalysisResult, GroupModelAnalysisSource, GroupModelAnalysisStatus, GroupRunSnapshot,
    GroupRunStore, HubStore, PrepareGroupModelAnalysis, PrepareGroupRun, Usage,
};
use rusqlite::Connection;
use tempfile::TempDir;

use super::{codec, complete, read, write};
use crate::sqlite_hub::SqliteHubStore;

struct Fixture {
    _root: TempDir,
    connection: Connection,
    candidate: PrepareGroupModelAnalysis,
}

#[test]
fn late_event_failures_roll_back_prepare_and_complete() {
    let mut fixture = fixture();
    install_event_abort(&fixture.connection, 1);
    assert!(write::prepare(&mut fixture.connection, &fixture.candidate).is_err());
    remove_event_abort(&fixture.connection);
    assert_eq!(row_count(&fixture.connection, "group_model_analyses"), 0);
    assert_eq!(
        row_count(&fixture.connection, "group_model_analysis_events"),
        0
    );

    write::prepare(&mut fixture.connection, &fixture.candidate).expect("prepare succeeds");
    write::claim(
        &mut fixture.connection,
        &ClaimGroupModelAnalysisDispatch {
            v: GROUP_MODEL_ANALYSIS_VERSION,
            analysis_id: fixture.candidate.analysis_id.clone(),
            dispatch_id: "dispatch-1".into(),
            consent_version: GROUP_MODEL_ANALYSIS_CONSENT_VERSION,
            released_at_ms: 30,
        },
    )
    .expect("claim dispatch");
    let dispatched =
        read::inspect(&mut fixture.connection, &fixture.candidate.analysis_id).expect("inspect");
    let artifact = result_artifact(&dispatched);

    install_event_abort(&fixture.connection, 3);
    assert!(
        complete::complete(
            &mut fixture.connection,
            &CompleteGroupModelAnalysis {
                v: GROUP_MODEL_ANALYSIS_VERSION,
                artifact,
            },
        )
        .is_err()
    );
    remove_event_abort(&fixture.connection);
    assert_dispatch_state(&mut fixture.connection, &fixture.candidate.analysis_id);
}

fn fixture() -> Fixture {
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
    let candidate = candidate(&snapshot);
    let connection = store.connect().expect("validated raw connection");
    Fixture {
        _root: root,
        connection,
        candidate,
    }
}

fn candidate(snapshot: &GroupRunSnapshot) -> PrepareGroupModelAnalysis {
    let request_config = GroupModelAnalysisRequestConfig {
        v: GROUP_MODEL_ANALYSIS_VERSION,
        provider: GroupModelAnalysisProvider::OpenAiResponses,
        endpoint: GROUP_MODEL_ANALYSIS_PROVIDER_ENDPOINT.into(),
        model: "gpt-test".into(),
        system_prompt_version: GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_VERSION,
        system_prompt: "Analyze the frozen Group dossier.".into(),
        max_output_tokens: 512,
        max_model_output_bytes: 8 * 1024,
        max_model_events: 128,
    };
    let encoded_config = codec::encode_config(&request_config).expect("encode config");
    let request_body =
        codec::encode_exact_request(&request_config, snapshot).expect("encode request");
    PrepareGroupModelAnalysis {
        v: GROUP_MODEL_ANALYSIS_VERSION,
        analysis_id: "analysis-1".into(),
        source: source(snapshot),
        config: codec::project_config(&request_config).expect("project config"),
        config_json: encoded_config.json,
        config_sha256: codec::digest_hex(&encoded_config.digest),
        request_sha256: codec::digest_hex(
            &codec::request_digest(&request_body).expect("digest request"),
        ),
        request_config,
        request_body,
        idempotency_key: "analysis-key".into(),
        created_at_ms: 20,
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

fn result_artifact(
    inspection: &forge_runtime_domain::GroupModelAnalysisInspection,
) -> forge_runtime_domain::GroupModelAnalysisResultArtifact {
    let claim = inspection.dispatch.as_ref().expect("dispatch claim");
    let result = GroupModelAnalysisResult {
        v: GROUP_MODEL_ANALYSIS_RESULT_VERSION,
        analysis_id: inspection.analysis.analysis_id.clone(),
        dispatch_id: claim.dispatch_id.clone(),
        request_sha256: inspection.analysis.request_sha256.clone(),
        outcome: GroupModelAnalysisOutcome::Completed,
        answer: "validated answer".into(),
        usage: Usage {
            input_tokens: 11,
            output_tokens: 7,
        },
    };
    codec::encode_result(&result, 40).expect("encode result").0
}

fn install_event_abort(connection: &Connection, sequence: u64) {
    connection
        .execute_batch(&format!(
            "CREATE TRIGGER fail_analysis_event BEFORE INSERT ON group_model_analysis_events
             WHEN NEW.seq={sequence} BEGIN SELECT RAISE(ABORT,'injected failure'); END"
        ))
        .expect("install event failure");
}

fn remove_event_abort(connection: &Connection) {
    connection
        .execute_batch("DROP TRIGGER fail_analysis_event")
        .expect("remove event failure");
}

fn row_count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("row count")
}

fn assert_dispatch_state(connection: &mut Connection, analysis_id: &str) {
    let inspection = read::inspect(connection, analysis_id).expect("dispatch state remains valid");
    assert!(matches!(
        inspection.recovery,
        GroupModelAnalysisRecovery::DispatchUnknown { .. }
    ));
    assert_eq!(
        inspection.analysis.status,
        GroupModelAnalysisStatus::DispatchUnknown
    );
    assert_eq!(inspection.events.len(), 2);
    assert_eq!(row_count(connection, "group_model_analysis_results"), 0);
}
