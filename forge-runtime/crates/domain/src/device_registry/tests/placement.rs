use super::*;

#[test]
fn dry_run_is_sorted_pure_and_hard_excludes_unsafe_or_ineligible_devices() {
    let mut candidates = placement_inventory();
    let request = DevicePlacementRequest::new(tenant("tenant-a"), requirements());
    let decisions = dry_run_device_placement(&candidates, &request, 200_000).unwrap();
    let first_run = decisions.clone();
    candidates.reverse();
    assert_eq!(
        first_run,
        dry_run_device_placement(&candidates, &request, 200_000).unwrap()
    );
    assert_order_and_state_exclusions(&decisions);
    assert_tenant_and_capacity_exclusions(&decisions);
}

fn placement_inventory() -> Vec<DevicePlacementCandidate> {
    vec![
        fresh_candidate("z-eligible", "approved"),
        fresh_candidate("a-pending", "pending"),
        fresh_candidate("b-revoked", "revoked"),
        fresh_candidate("c-cordoned", "cordoned"),
        fresh_candidate("d-offline", "offline"),
        candidate(
            "e-stale",
            "tenant-a",
            1_000,
            60_000,
            full_capabilities(),
            "approved",
        ),
        candidate(
            "f-other-tenant",
            "tenant-b",
            150_000,
            60_000,
            full_capabilities(),
            "approved",
        ),
        insufficient_candidate(),
        low_gpu_memory_candidate(),
    ]
}

fn fresh_candidate(id: &str, status: &str) -> DevicePlacementCandidate {
    candidate(id, "tenant-a", 150_000, 60_000, full_capabilities(), status)
}

fn insufficient_candidate() -> DevicePlacementCandidate {
    candidate(
        "g-insufficient",
        "tenant-a",
        150_000,
        60_000,
        capabilities(2, 1, 4_096, 1_024, 2_048, 0, vec![], &["python"]),
        "approved",
    )
}

fn low_gpu_memory_candidate() -> DevicePlacementCandidate {
    let snapshot = capabilities(
        8,
        8,
        32 * 1024 * 1024 * 1024,
        16 * 1024 * 1024 * 1024,
        100 * 1024 * 1024 * 1024,
        50 * 1024 * 1024 * 1024,
        vec![
            gpu("gpu-0", 8 * 1024 * 1024 * 1024, 4 * 1024 * 1024 * 1024),
            gpu("gpu-1", 8 * 1024 * 1024 * 1024, 4 * 1024 * 1024 * 1024),
        ],
        &["docker"],
    );
    candidate(
        "h-small-gpu-memory",
        "tenant-a",
        150_000,
        60_000,
        snapshot,
        "approved",
    )
}

fn decision<'a>(decisions: &'a [DevicePlacementDecision], id: &str) -> &'a DevicePlacementDecision {
    decisions
        .iter()
        .find(|decision| decision.device_id().as_str() == id)
        .unwrap()
}

fn assert_order_and_state_exclusions(decisions: &[DevicePlacementDecision]) {
    assert_eq!(decisions[0].device_id().as_str(), "a-pending");
    assert_eq!(decisions.last().unwrap().device_id().as_str(), "z-eligible");
    assert_eq!(
        decision(decisions, "z-eligible").disposition(),
        &DevicePlacementDisposition::Eligible
    );
    assert_eq!(
        decision(decisions, "a-pending").disposition(),
        &DevicePlacementDisposition::Excluded(vec![DevicePlacementExclusion::ApprovalPending])
    );
    assert_eq!(
        decision(decisions, "b-revoked").disposition(),
        &DevicePlacementDisposition::Excluded(vec![DevicePlacementExclusion::DeviceRevoked])
    );
    assert!(matches!(
        decision(decisions, "c-cordoned").disposition(),
        DevicePlacementDisposition::Excluded(reasons)
            if reasons.contains(&DevicePlacementExclusion::DeviceCordoned)
    ));
    assert!(matches!(
        decision(decisions, "d-offline").disposition(),
        DevicePlacementDisposition::Excluded(reasons)
            if reasons.contains(&DevicePlacementExclusion::RunnerOffline)
    ));
}

fn assert_tenant_and_capacity_exclusions(decisions: &[DevicePlacementDecision]) {
    assert!(matches!(
        decision(decisions, "e-stale").disposition(),
        DevicePlacementDisposition::Excluded(reasons)
            if reasons.contains(&DevicePlacementExclusion::HeartbeatStale)
                && reasons.contains(&DevicePlacementExclusion::CapabilityLeaseExpired)
    ));
    assert_eq!(
        decision(decisions, "f-other-tenant").disposition(),
        &DevicePlacementDisposition::Excluded(vec![DevicePlacementExclusion::TenantMismatch])
    );
    assert_insufficient_capacity_reasons(decision(decisions, "g-insufficient"));
    assert_gpu_memory_reason(decision(decisions, "h-small-gpu-memory"));
}

fn assert_insufficient_capacity_reasons(decision: &DevicePlacementDecision) {
    assert!(matches!(
        decision.disposition(),
        DevicePlacementDisposition::Excluded(reasons)
            if reasons.contains(&DevicePlacementExclusion::CpuCapacityInsufficient)
                && reasons.contains(&DevicePlacementExclusion::MemoryCapacityInsufficient)
                && reasons.contains(&DevicePlacementExclusion::StorageCapacityInsufficient)
                && reasons.contains(&DevicePlacementExclusion::RuntimeUnavailable)
                && reasons.contains(&DevicePlacementExclusion::GpuCountInsufficient)
    ));
}

fn assert_gpu_memory_reason(decision: &DevicePlacementDecision) {
    assert!(matches!(
        decision.disposition(),
        DevicePlacementDisposition::Excluded(reasons)
            if reasons.contains(&DevicePlacementExclusion::GpuMemoryInsufficient)
    ));
}

#[test]
fn duplicate_inventory_entries_are_rejected_instead_of_order_dependent() {
    let one = candidate(
        "device-one",
        "tenant-a",
        10_000,
        10_000,
        full_capabilities(),
        "approved",
    );
    let duplicate = one.clone();
    let request =
        DevicePlacementRequest::new(tenant("tenant-a"), DevicePlacementRequirements::any());
    assert_eq!(
        dry_run_device_placement(&[one, duplicate], &request, 11_000),
        Err(DevicePlacementValidationError::DuplicateDevice)
    );
}

#[test]
fn opt_in_policy_filters_are_fail_closed_and_emit_sorted_closed_reasons() {
    let policy = DevicePlacementPolicy::new()
        .with_allowed_data_residency_zones(vec!["us-west".to_owned()])
        .unwrap()
        .with_minimum_trust_zone("standard")
        .unwrap()
        .with_sandbox_floor("container")
        .unwrap()
        .with_concurrency_slots(2)
        .unwrap();
    let request = DevicePlacementRequest::new(
        tenant("tenant-a"),
        DevicePlacementRequirements::any().with_policy(policy),
    );

    assert_default_policy_filters_fail_closed(&request);
    assert_satisfied_policy_filters_match(&request);
    assert_failed_policy_filters_emit_sorted_reasons(&request);
}

fn assert_default_policy_filters_fail_closed(request: &DevicePlacementRequest) {
    let default_attributes = fresh_candidate("a-default-attributes", "approved");
    let decision = dry_run_device_placement(&[default_attributes], request, 200_000).unwrap();
    assert_eq!(
        decision[0].disposition(),
        &DevicePlacementDisposition::Excluded(vec![
            DevicePlacementExclusion::ConcurrencyCapacityInsufficient,
            DevicePlacementExclusion::DataResidencyZoneMismatch,
            DevicePlacementExclusion::SandboxFloorUnmet,
            DevicePlacementExclusion::TrustZoneUnconfirmed,
        ])
    );
}

fn assert_satisfied_policy_filters_match(request: &DevicePlacementRequest) {
    let attributes = DevicePlacementAttributes::new()
        .with_data_residency_zones(vec!["us-west".to_owned(), "us-east".to_owned()])
        .unwrap()
        .with_trust_zone("restricted")
        .unwrap()
        .with_sandbox_levels(&["microvm".to_owned()])
        .unwrap()
        .with_concurrency(4, 1);
    let eligible = fresh_candidate("b-policy-eligible", "approved").with_attributes(attributes);
    let decision = dry_run_device_placement(&[eligible], request, 200_000).unwrap();
    assert_eq!(
        decision[0].disposition(),
        &DevicePlacementDisposition::Eligible
    );
}

fn assert_failed_policy_filters_emit_sorted_reasons(request: &DevicePlacementRequest) {
    let attributes = DevicePlacementAttributes::new()
        .with_data_residency_zones(vec!["eu-west".to_owned()])
        .unwrap()
        .with_trust_zone("unknown")
        .unwrap()
        .with_sandbox_levels(&["process".to_owned()])
        .unwrap()
        .with_concurrency(1, 1);
    let excluded = fresh_candidate("c-policy-excluded", "approved").with_attributes(attributes);
    let decision = dry_run_device_placement(&[excluded], request, 200_000).unwrap();
    let DevicePlacementDisposition::Excluded(reasons) = decision[0].disposition() else {
        panic!("candidate must fail all four enabled policy filters");
    };
    let reason_codes = reasons
        .iter()
        .map(|reason| reason.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        reason_codes,
        vec![
            "concurrency_capacity_insufficient",
            "data_residency_zone_mismatch",
            "sandbox_floor_unmet",
            "trust_zone_unconfirmed",
        ]
    );
}

#[test]
fn placement_policy_rejects_invalid_attributes() {
    assert!(
        DevicePlacementPolicy::new()
            .with_allowed_data_residency_zones(vec![])
            .is_err()
    );
    assert!(
        DevicePlacementPolicy::new()
            .with_allowed_data_residency_zones(vec!["us-west".to_owned(), "us-west".to_owned()])
            .is_err()
    );
    assert!(
        DevicePlacementPolicy::new()
            .with_allowed_data_residency_zones(vec!["us/west".to_owned()])
            .is_err()
    );
    assert!(
        DevicePlacementPolicy::new()
            .with_allowed_data_residency_zones(vec![
                "z".repeat(MAX_DEVICE_RESIDENCY_ZONE_BYTES + 1)
            ])
            .is_err()
    );
    assert!(
        DevicePlacementPolicy::new()
            .with_minimum_trust_zone("unknown")
            .is_err()
    );
    assert!(
        DevicePlacementPolicy::new()
            .with_minimum_trust_zone("STANDARD")
            .is_err()
    );
    assert!(
        DevicePlacementPolicy::new()
            .with_sandbox_floor("host")
            .is_err()
    );
    assert!(
        DevicePlacementPolicy::new()
            .with_concurrency_slots(0)
            .is_err()
    );
}

#[test]
fn device_placement_attributes_reject_invalid_values() {
    assert!(
        DevicePlacementAttributes::new()
            .with_data_residency_zones(vec!["zone:+1".to_owned()])
            .is_err()
    );
    assert!(
        DevicePlacementAttributes::new()
            .with_trust_zone("trusted")
            .is_err()
    );
    assert!(
        DevicePlacementAttributes::new()
            .with_sandbox_levels(&["container".to_owned(), "container".to_owned()])
            .is_err()
    );
    assert!(
        DevicePlacementAttributes::new()
            .with_sandbox_levels(&["unknown".to_owned()])
            .is_err()
    );
}

#[test]
fn device_placement_attributes_sort_residency_zones() {
    let canonical_zones = DevicePlacementAttributes::new()
        .with_data_residency_zones(vec!["us-west".to_owned(), "eu-west".to_owned()])
        .unwrap();
    assert_eq!(
        canonical_zones.data_residency_zones(),
        &["eu-west".to_owned(), "us-west".to_owned()]
    );
    assert_eq!(canonical_zones.trust_zone(), DeviceTrustZone::Unknown);
}

#[test]
fn trust_rank_and_available_concurrency_boundaries_match_policy_contract() {
    let policy = DevicePlacementPolicy::new()
        .with_minimum_trust_zone("high")
        .unwrap()
        .with_concurrency_slots(2)
        .unwrap();
    let request = DevicePlacementRequest::new(
        tenant("tenant-a"),
        DevicePlacementRequirements::any().with_policy(policy),
    );

    let enough = DevicePlacementAttributes::new()
        .with_trust_zone("restricted")
        .unwrap()
        .with_concurrency(4, 2);
    let candidate = fresh_candidate("trust-and-slots-ok", "approved").with_attributes(enough);
    assert_eq!(
        dry_run_device_placement(&[candidate], &request, 200_000).unwrap()[0].disposition(),
        &DevicePlacementDisposition::Eligible
    );

    let below_and_overcommitted = DevicePlacementAttributes::new()
        .with_trust_zone("standard")
        .unwrap()
        .with_concurrency(2, 3);
    let candidate =
        fresh_candidate("trust-and-slots-low", "approved").with_attributes(below_and_overcommitted);
    assert_eq!(
        dry_run_device_placement(&[candidate], &request, 200_000).unwrap()[0].disposition(),
        &DevicePlacementDisposition::Excluded(vec![
            DevicePlacementExclusion::ConcurrencyCapacityInsufficient,
            DevicePlacementExclusion::TrustZoneBelowMinimum,
        ])
    );
}
