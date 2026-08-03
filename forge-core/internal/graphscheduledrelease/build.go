package graphscheduledrelease

// BuildAuthorization revalidates one complete control and emits only passive
// policy authority. It performs no preflight, claim, send, or durable write.
func BuildAuthorization(control ReleaseControl) (Authorization, error) {
	if validateReleaseControl(control) != nil {
		return Authorization{}, errInvalidControl
	}
	value := authorizationFrom(control)
	digest, err := domainDigest(authorizationDigestDomain, authorizationPayloadFrom(value))
	if err != nil {
		return Authorization{}, errInvalidControl
	}
	value.AuthorizationID = authorizationIDPrefix + digest
	value.AuthorizationSHA256 = digest
	if validateAuthorization(value) != nil {
		return Authorization{}, errInvalidControl
	}
	return value, nil
}

func authorizationFrom(control ReleaseControl) Authorization {
	contract, request := control.ScheduledContract, control.ProviderRequest
	source := control.ControlSnapshot.Manifest.Source
	return Authorization{
		V: AuthorizationVersion, SchedulerProtocolVersion: control.SchedulerProtocolVersion,
		DispatchAuthorizationProtocolVersion: AuthorizationProtocol,
		GraphRunID:                           contract.GraphRunID, GraphID: contract.GraphID,
		GroupRunID: source.GroupRunID, GroupID: source.GroupID,
		SourceSnapshotSHA256: contract.SourceSnapshotSHA256,
		GraphManifestSHA256:  contract.GraphManifestSHA256, CorePlanSHA256: contract.CorePlanSHA256,
		ControlSnapshotSHA256:        contract.ControlSnapshotSHA256,
		ReleaseControlSnapshotSHA256: control.SnapshotSHA256,
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
		ReleaseRequirements: releaseRequirements(), Failure: contract.Failure,
		LifecycleContractAdmissionAuthorized: true, ExecutionAuthorityReleaseAuthorized: true,
		DispatchAuthorityReleaseAuthorized: true, ScheduledContractCandidatePresent: true,
		ProviderRequestPrepared: true,
	}
}

func releaseRequirements() ReleaseRequirements {
	return ReleaseRequirements{
		Consent: "fresh_off_machine", ConsentContractVersion: ConsentContractVersion,
		CredentialPreflight:  "header_safe_environment",
		DestinationPreflight: "exact_registered_destination",
		PricingPreflight:     "exact_snapshot_within_max_cost",
		ProjectLaneClaim:     "global_exclusive_until_terminal",
		ProviderHealthCheck:  "forbidden",
		AtomicTransition:     "exact_pristine_head_admission_release_and_lane_claim",
		Successor:            "verified_intermediate_terminal_receipt_before_successor",
	}
}
