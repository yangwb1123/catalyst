use std::sync::{Arc, Barrier};

use forge_runtime_domain::{
    ClaimGroupPanelSynthesisDispatchResult, CompleteGroupPanelSynthesis,
    CompleteGroupPanelSynthesisDisposition, GroupPanelSynthesisRecovery, GroupPanelSynthesisStatus,
    GroupPanelSynthesisStore, HubEntity, HubStoreError, PrepareGroupPanelSynthesisDisposition,
};
use rusqlite::Connection;

mod sqlite_group_panel_synthesis_support;

use sqlite_group_panel_synthesis_support::{Fixture, claim_request, result_artifact};

#[test]
fn prepare_replays_original_identity_and_conflicts_on_semantic_change() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate("synthesis-1", "synthesis-key", 50);
    let created = fixture
        .store()
        .prepare_group_panel_synthesis(&candidate)
        .expect("prepare synthesis");
    assert_eq!(
        created.disposition,
        PrepareGroupPanelSynthesisDisposition::Created
    );

    let replay = fixture
        .store()
        .prepare_group_panel_synthesis(&fixture.candidate(
            "ignored-candidate",
            "synthesis-key",
            999,
        ))
        .expect("exact semantic replay");
    assert_eq!(
        replay.disposition,
        PrepareGroupPanelSynthesisDisposition::Replayed
    );
    assert_eq!(replay.inspection, created.inspection);

    let divergent = fixture.candidate_with_limit("ignored", "synthesis-key", 51, 513);
    assert_synthesis_conflict(
        &fixture.store().prepare_group_panel_synthesis(&divergent),
        "changed output limit",
    );
}

#[test]
fn stored_corruption_is_not_downgraded_to_an_idempotency_conflict() {
    let fixture = Fixture::new();
    fixture
        .store()
        .prepare_group_panel_synthesis(&fixture.candidate("synthesis-1", "synthesis-key", 50))
        .expect("prepare synthesis");
    Connection::open(fixture.database())
        .expect("raw SQLite")
        .execute(
            "UPDATE group_panel_syntheses
             SET request_body=zeroblob(length(request_body)) WHERE id='synthesis-1'",
            [],
        )
        .expect("inject request corruption");

    assert!(matches!(
        fixture
            .store()
            .prepare_group_panel_synthesis(&fixture.candidate("ignored", "synthesis-key", 999)),
        Err(HubStoreError::Corrupt { .. })
    ));
}

#[test]
fn concurrent_claim_releases_exact_request_bytes_to_one_winner() {
    const WORKERS: usize = 8;
    let fixture = Fixture::new();
    let candidate = fixture.candidate("synthesis-1", "synthesis-key", 50);
    let expected_body = candidate.request_body.clone();
    fixture
        .store()
        .prepare_group_panel_synthesis(&candidate)
        .expect("prepare synthesis");
    let barrier = Arc::new(Barrier::new(WORKERS));
    let outcomes = (0..WORKERS)
        .map(|index| claim_worker(&fixture, Arc::clone(&barrier), index))
        .collect::<Vec<_>>()
        .into_iter()
        .map(|worker| worker.join().expect("claim worker"))
        .collect::<Vec<_>>();

    assert_claim_outcomes(outcomes, &expected_body);
    let inspection = fixture
        .store()
        .inspect_group_panel_synthesis("synthesis-1")
        .expect("inspect claimed synthesis");
    assert!(matches!(
        inspection.recovery,
        GroupPanelSynthesisRecovery::DispatchUnknown { .. }
    ));
    assert_eq!(inspection.events.len(), 2);
}

#[test]
fn completion_is_atomic_in_shape_and_exactly_replayable() {
    let fixture = Fixture::new();
    let dispatched = fixture.dispatch();
    let artifact = result_artifact(&dispatched, "Shared risks and next steps.", 70);
    let request = CompleteGroupPanelSynthesis {
        v: 1,
        artifact: artifact.clone(),
    };
    let created = fixture
        .store()
        .complete_group_panel_synthesis(&request)
        .expect("complete synthesis");
    assert_eq!(
        created.disposition,
        CompleteGroupPanelSynthesisDisposition::Created
    );
    assert_eq!(created.inspection.result, Some(artifact));
    assert_completed_rows(&fixture);

    let replay = fixture
        .store()
        .complete_group_panel_synthesis(&request)
        .expect("exact completion replay");
    assert_eq!(
        replay.disposition,
        CompleteGroupPanelSynthesisDisposition::Replayed
    );
    let divergent = CompleteGroupPanelSynthesis {
        v: 1,
        artifact: result_artifact(&dispatched, "Different synthesis.", 70),
    };
    assert_synthesis_conflict(
        &fixture.store().complete_group_panel_synthesis(&divergent),
        "divergent completion",
    );
}

fn claim_worker(
    fixture: &Fixture,
    barrier: Arc<Barrier>,
    index: usize,
) -> std::thread::JoinHandle<ClaimGroupPanelSynthesisDispatchResult> {
    let store = fixture.store().clone();
    std::thread::spawn(move || {
        barrier.wait();
        store
            .claim_group_panel_synthesis_dispatch(&claim_request(
                "synthesis-1",
                &format!("synthesis-dispatch-{index}"),
                60 + index as u64,
            ))
            .expect("concurrent synthesis claim")
    })
}

fn assert_claim_outcomes(
    outcomes: Vec<ClaimGroupPanelSynthesisDispatchResult>,
    expected_body: &[u8],
) {
    let mut winners = Vec::new();
    let mut loser_count = 0;
    for outcome in outcomes {
        match outcome {
            ClaimGroupPanelSynthesisDispatchResult::Claimed { authority } => {
                winners.push(authority.into_parts().1);
            }
            ClaimGroupPanelSynthesisDispatchResult::AlreadyClaimed { inspection } => {
                loser_count += 1;
                let encoded = serde_json::to_vec(&inspection).expect("loser inspection JSON");
                assert!(
                    !encoded
                        .windows(expected_body.len())
                        .any(|window| window == expected_body)
                );
            }
        }
    }
    assert_eq!(winners, [expected_body]);
    assert_eq!(loser_count, 7);
}

fn assert_completed_rows(fixture: &Fixture) {
    let inspection = fixture
        .store()
        .inspect_group_panel_synthesis("synthesis-1")
        .expect("inspect completed synthesis");
    assert!(matches!(
        inspection.recovery,
        GroupPanelSynthesisRecovery::Terminal { .. }
    ));
    assert_eq!(
        inspection.synthesis.status,
        GroupPanelSynthesisStatus::Completed
    );
    assert_eq!(inspection.events.len(), 3);
    let connection = Connection::open(fixture.database()).expect("raw SQLite");
    let rows: (i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT COUNT(*) FROM group_panel_synthesis_events
                WHERE synthesis_id='synthesis-1'),
               (SELECT COUNT(*) FROM group_panel_synthesis_results
                WHERE synthesis_id='synthesis-1')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("synthesis row counts");
    assert_eq!(rows, (3, 1));
}

fn assert_synthesis_conflict<T>(result: &Result<T, HubStoreError>, subject: &str) {
    assert!(
        matches!(
            result,
            Err(HubStoreError::Conflict {
                entity: HubEntity::GroupPanelSynthesis,
                ..
            })
        ),
        "{subject} must conflict"
    );
}
