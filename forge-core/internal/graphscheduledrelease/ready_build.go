package graphscheduledrelease

// BuildReadyAuthorization revalidates one v2 control and emits passive policy
// for at most one future release. It performs no current effect.
func BuildReadyAuthorization(control ReadyReleaseControl) (ReadyAuthorization, error) {
	if validateReadyReleaseControl(control) != nil {
		return ReadyAuthorization{}, errInvalidControl
	}
	value := readyAuthorizationFrom(control)
	digest, err := domainDigest(readyAuthorizationDigestDomain, readyAuthorizationPayloadFrom(value))
	if err != nil {
		return ReadyAuthorization{}, errInvalidControl
	}
	value.AuthorizationID = readyAuthorizationIDPrefix + digest
	value.AuthorizationSHA256 = digest
	if validateReadyAuthorization(value) != nil {
		return ReadyAuthorization{}, errInvalidControl
	}
	return value, nil
}

func readyAuthorizationFrom(control ReadyReleaseControl) ReadyAuthorization {
	contract, request := control.ScheduledContract, control.ProviderRequest
	source := control.ControlSnapshot.Manifest.Source
	return ReadyAuthorization{
		V: ReadyAuthorizationVersion, SchedulerProtocolVersion: control.SchedulerProtocolVersion,
		DispatchAuthorizationProtocolVersion: ReadyAuthorizationProtocol,
		GraphRunID:                           contract.GraphRunID, GraphID: contract.GraphID,
		GroupRunID: source.GroupRunID, GroupID: source.GroupID,
		SourceSnapshotSHA256: contract.SourceSnapshotSHA256,
		GraphManifestSHA256:  contract.GraphManifestSHA256, CorePlanSHA256: contract.CorePlanSHA256,
		ControlSnapshotSHA256:        contract.ControlSnapshotSHA256,
		ReleaseControlSnapshotSHA256: control.SnapshotSHA256,
		ProgressSnapshotSHA256:       control.ProgressSnapshot.SnapshotSHA256,
		ReconcileDecisionSHA256:      control.ReconcileDecision.DecisionSHA256,
		ScheduleID:                   contract.ScheduleID, ScheduleSHA256: contract.ScheduleSHA256,
		ScheduledContractID: contract.ContractID, ScheduledContractSHA256: contract.ContractSHA256,
		ScheduledProviderRequestID:     request.ProviderRequestID,
		ScheduledProviderRequestSHA256: request.PreparedRequestSHA256,
		LogicalRequestID:               request.LogicalRequestID, LogicalRequestSHA256: request.LogicalRequestSHA256,
		RequestBodySHA256: request.ProviderRequestSHA256, RequestBodyBytes: request.ProviderRequestBytes,
		ExpectedLastEventSeq:    contract.ExpectedLastEventSeq,
		ExpectedLastEventSHA256: contract.ExpectedLastEventSHA256,
		ExecutionOrdinal:        uint64(contract.Node.ExecutionOrdinal), NodeID: contract.Node.NodeID,
		Attempt: contract.Node.Attempt, ProjectID: contract.Node.ProjectID,
		ProjectLaneSHA256: contract.Node.ProjectLaneSHA256,
		SameProjectPolicy: contract.Node.SameProjectPolicy, ProviderKind: contract.Provider.Kind,
		Endpoint: contract.Provider.Endpoint, Model: contract.Provider.Model,
		DestinationSHA256:     request.DestinationSHA256,
		PricingSnapshotSHA256: request.PricingSnapshotSHA256, Budgets: contract.Budgets,
		ReleaseRequirements: readyReleaseRequirements(), MaximumFutureNodeReleases: 1,
		Failure: contract.Failure, LifecycleContractAdmissionAuthorized: true,
		ExecutionAuthorityReleaseAuthorized: true, DispatchAuthorityReleaseAuthorized: true,
		ScheduledContractCandidatePresent: true, ProviderRequestPrepared: true,
	}
}

func readyReleaseRequirements() ReleaseRequirements {
	return ReleaseRequirements{
		Consent: "fresh_off_machine", ConsentContractVersion: ConsentContractVersion,
		CredentialPreflight:  "header_safe_environment",
		DestinationPreflight: "exact_registered_destination",
		PricingPreflight:     "exact_snapshot_within_max_cost",
		ProjectLaneClaim:     "global_exclusive_until_terminal", ProviderHealthCheck: "forbidden",
		AtomicTransition: "exact_progress_snapshot_selected_node_admission_release_and_lane_claim",
		Successor:        "exact_ordered_direct_predecessor_terminal_receipts_before_successor",
	}
}
