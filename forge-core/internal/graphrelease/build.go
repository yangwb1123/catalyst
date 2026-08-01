package graphrelease

import "forgeos/forge-core/internal/graphdispatch"

// BuildAuthorization fully revalidates one release-control snapshot and emits
// a passive decision artifact. It performs no release preflight or effect.
func BuildAuthorization(control ReleaseControl) (Authorization, error) {
	facts, err := validateReleaseControlFacts(control)
	if err != nil {
		return Authorization{}, errInvalidControl
	}
	value := authorizationFrom(control, facts)
	digest, err := domainDigest(authorizationDigestDomain, authorizationPayloadFrom(value))
	if err != nil {
		return Authorization{}, errInvalidControl
	}
	value.AuthorizationID = "node-dispatch-authorization-" + digest
	value.AuthorizationSHA256 = digest
	if validateAuthorization(value) != nil {
		return Authorization{}, errInvalidControl
	}
	return value, nil
}

func authorizationFrom(control ReleaseControl, facts journalFacts) Authorization {
	contract := control.Contract
	dispatch := control.DispatchRequest
	return Authorization{
		V: AuthorizationVersion, SchedulerProtocolVersion: control.SchedulerProtocolVersion,
		DispatchAuthorizationProtocolVersion: AuthorizationProtocol,
		GraphRunID:                           control.GraphRun.GraphRunID, GraphID: control.GraphRun.GraphID,
		GroupRunID:                   control.Manifest.Source.GroupRunID,
		SourceSnapshotSHA256:         control.GraphRun.SourceSnapshotSHA256,
		GraphManifestSHA256:          control.GraphRun.GraphManifestSHA256,
		CorePlanSHA256:               control.GraphRun.PlanSHA256,
		ReleaseControlSnapshotSHA256: control.SnapshotSHA256,
		ExpectedLastEventSeq:         3, ExpectedLastEventSHA256: facts.DispatchSHA256,
		ContractID: contract.ContractID, ContractSHA256: contract.ContractSHA256,
		DispatchRequestID:     dispatch.DispatchRequestID,
		DispatchRequestSHA256: dispatch.DispatchRequestSHA256,
		LogicalRequestSHA256:  dispatch.RequestSHA256,
		RequestBodySHA256:     dispatch.ProviderRequestSHA256,
		RequestBodyBytes:      dispatch.ProviderRequestBytes,
		NodeID:                contract.Node.NodeID, Attempt: contract.Node.Attempt,
		ProjectID: contract.Node.ProjectID, ProjectLaneSHA256: contract.Node.ProjectLaneSHA256,
		SameProjectPolicy: contract.Node.SameProjectPolicy,
		ProviderKind:      dispatch.Provider, Endpoint: dispatch.Endpoint, Model: dispatch.Model,
		DestinationSHA256:     dispatch.DestinationSHA256,
		PricingSnapshotSHA256: dispatch.PricingSnapshotSHA256,
		Budgets:               contract.Budgets, ReleaseRequirements: releaseRequirements(),
		Failure: contract.Failure, ExecutionContractPresent: true,
		DispatchRequestPresent: true, DispatchAuthorityReleaseAuthorized: true,
		DispatchAuthorityReleased: false,
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
	}
}

func validAuthorizationBudgets(value graphdispatch.ExecutionBudgets) bool {
	return value.MaxTurns == 1 && value.MaxToolCalls == 0 &&
		value.MaxOutputTokens >= 1 && value.MaxOutputTokens <= graphdispatch.MaxOutputTokens &&
		value.MaxModelOutputBytes >= 1 &&
		value.MaxModelOutputBytes <= graphdispatch.MaxModelOutputBytes &&
		value.MaxModelEvents >= 1 && value.MaxModelEvents <= graphdispatch.MaxModelEvents &&
		value.TimeoutMilliseconds >= 1 &&
		value.TimeoutMilliseconds <= graphdispatch.MaxTimeoutMilliseconds &&
		value.MaxCostUSDMicros >= 1 && value.MaxCostUSDMicros <= graphdispatch.MaxCostUSDMicros &&
		isLowerHexDigest(value.PricingSnapshotSHA256)
}
