use crate::runtime_domain::ScheduledGraphReconcilePortError;

use super::ScheduledReadyNodeReleaseServiceError;

#[path = "scheduled_ready_release_test_support.rs"]
mod support;
use support::*;

#[test]
fn initial_ready_runs_exact_a_core_b_and_returns_bound_policy() {
    let fixture = fixture();
    let harness = harness(
        &fixture,
        vec![fixture.source.clone()],
        Ok(ready(&fixture.snapshot)),
        AuthMode::Valid,
    );
    let result = harness
        .service
        .authorize(&fixture.graph_run_id)
        .expect("ready policy");

    assert_eq!(
        harness.trace(),
        ["progress", "reconcile", "source", "authorize", "source"]
    );
    assert_eq!(harness.sources.calls(), 2);
    assert_eq!(result.authorization.maximum_future_node_releases, 1);
    result
        .authorization
        .validate()
        .expect("valid authorization");
}

#[test]
fn source_change_after_core_fails_closed_without_returning_authorization() {
    let fixture = fixture();
    let mut changed = fixture.source.clone();
    changed.graph.graph.created_at_ms += 1;
    let harness = harness(
        &fixture,
        vec![fixture.source.clone(), changed],
        Ok(ready(&fixture.snapshot)),
        AuthMode::Valid,
    );

    assert_eq!(
        harness.service.authorize(&fixture.graph_run_id),
        Err(ScheduledReadyNodeReleaseServiceError::SourceChanged)
    );
    assert_eq!(harness.sources.calls(), 2);
    assert_eq!(harness.authorize.calls(), 1);
}

#[test]
fn non_ready_decision_never_reads_a_release_source_or_authorizes() {
    let fixture = fixture();
    let harness = harness(
        &fixture,
        vec![fixture.source.clone()],
        Ok(non_ready(&fixture.snapshot)),
        AuthMode::Valid,
    );

    assert_eq!(
        harness.service.authorize(&fixture.graph_run_id),
        Err(ScheduledReadyNodeReleaseServiceError::NotReady)
    );
    assert_eq!(harness.sources.calls(), 0);
    assert_eq!(harness.authorize.calls(), 0);
}

#[test]
fn substituted_core_decision_is_rejected_before_source_a() {
    let fixture = fixture();
    let mut other = fixture.snapshot.clone();
    other.graph_run_id = "graph-run-substituted".into();
    other.snapshot_sha256.clear();
    let other = other.seal().expect("substituted snapshot");
    let harness = harness(
        &fixture,
        vec![fixture.source.clone()],
        Ok(ready(&other)),
        AuthMode::Valid,
    );

    assert_eq!(
        harness.service.authorize(&fixture.graph_run_id),
        Err(ScheduledReadyNodeReleaseServiceError::InvalidReconcileDecision)
    );
    assert_eq!(harness.sources.calls(), 0);
    assert_eq!(harness.authorize.calls(), 0);
}

#[test]
fn unavailable_core_and_invalid_authorization_keep_distinct_errors() {
    let fixture = fixture();
    let unavailable = harness(
        &fixture,
        vec![fixture.source.clone()],
        Err(ScheduledGraphReconcilePortError::Unavailable),
        AuthMode::Valid,
    );
    assert_eq!(
        unavailable.service.authorize(&fixture.graph_run_id),
        Err(ScheduledReadyNodeReleaseServiceError::ReconcileUnavailable)
    );
    assert_eq!(unavailable.sources.calls(), 0);

    let invalid = harness(
        &fixture,
        vec![fixture.source.clone()],
        Ok(ready(&fixture.snapshot)),
        AuthMode::Invalid,
    );
    assert_eq!(
        invalid.service.authorize(&fixture.graph_run_id),
        Err(ScheduledReadyNodeReleaseServiceError::InvalidAuthorization)
    );
    assert_eq!(invalid.sources.calls(), 1);

    let auth_down = harness(
        &fixture,
        vec![fixture.source.clone()],
        Ok(ready(&fixture.snapshot)),
        AuthMode::Unavailable,
    );
    assert_eq!(
        auth_down.service.authorize(&fixture.graph_run_id),
        Err(ScheduledReadyNodeReleaseServiceError::AuthorizationUnavailable)
    );
    assert_eq!(auth_down.sources.calls(), 1);
}

#[test]
fn progress_storage_errors_map_without_opening_core_or_sources() {
    let fixture = fixture();
    let cases = [
        (not_found(), ScheduledReadyNodeReleaseServiceError::NotFound),
        (
            unavailable(),
            ScheduledReadyNodeReleaseServiceError::StorageUnavailable,
        ),
        (
            conflict(),
            ScheduledReadyNodeReleaseServiceError::CorruptSource,
        ),
        (
            corrupt(),
            ScheduledReadyNodeReleaseServiceError::CorruptSource,
        ),
    ];
    for (store_error, expected) in cases {
        let harness = harness_with_progress_error(&fixture, store_error);
        assert_eq!(
            harness.service.authorize(&fixture.graph_run_id),
            Err(expected)
        );
        assert_eq!(harness.reconcile.calls(), 0);
        assert_eq!(harness.sources.calls(), 0);
    }
    let changed = make_harness(
        Ok(fixture.snapshot.clone()),
        Err(conflict()),
        Ok(ready(&fixture.snapshot)),
        AuthMode::Valid,
    );
    assert_eq!(
        changed.service.authorize(&fixture.graph_run_id),
        Err(ScheduledReadyNodeReleaseServiceError::SourceChanged)
    );
}

#[test]
fn unchanged_retries_reload_a_and_b_and_rerun_core_each_time() {
    let fixture = fixture();
    let harness = harness(
        &fixture,
        vec![fixture.source.clone()],
        Ok(ready(&fixture.snapshot)),
        AuthMode::Valid,
    );

    harness
        .service
        .authorize(&fixture.graph_run_id)
        .expect("first policy");
    harness
        .service
        .authorize(&fixture.graph_run_id)
        .expect("repeated policy");

    assert_eq!(harness.progress.calls(), 2);
    assert_eq!(harness.reconcile.calls(), 2);
    assert_eq!(harness.sources.calls(), 4);
    assert_eq!(harness.authorize.calls(), 2);
}
