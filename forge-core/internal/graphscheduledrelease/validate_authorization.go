package graphscheduledrelease

import (
	"reflect"

	"forgeos/forge-core/internal/graphdispatch"
)

func validateAuthorization(value Authorization) error {
	if !validAuthorizationHeader(value) || !validAuthorizationPolicies(value) ||
		!validAuthorizationFlags(value) ||
		!reflect.DeepEqual(value.ReleaseRequirements, releaseRequirements()) {
		return errInvalidControl
	}
	digest, err := domainDigest(authorizationDigestDomain, authorizationPayloadFrom(value))
	if err != nil || digest != value.AuthorizationSHA256 ||
		value.AuthorizationID != authorizationIDPrefix+digest {
		return errInvalidControl
	}
	return nil
}

func validAuthorizationHeader(value Authorization) bool {
	texts := []string{
		value.GraphRunID, value.GraphID, value.GroupRunID, value.GroupID,
		value.ScheduleID, value.ScheduledContractID, value.ScheduledProviderRequestID,
		value.LogicalRequestID, value.NodeID, value.ProjectID, value.Endpoint, value.Model,
	}
	for _, text := range texts {
		if !validText(text, 2*1024) {
			return false
		}
	}
	digests := authorizationDigests(value)
	for _, digest := range digests {
		if !isLowerHexDigest(digest) {
			return false
		}
	}
	return value.V == AuthorizationVersion && value.SchedulerProtocolVersion == 1 &&
		value.DispatchAuthorizationProtocolVersion == AuthorizationProtocol &&
		value.ExpectedLastEventSeq == 1 && value.ExecutionOrdinal == 0 && value.Attempt == 1
}

func authorizationDigests(value Authorization) []string {
	return []string{
		value.SourceSnapshotSHA256, value.GraphManifestSHA256, value.CorePlanSHA256,
		value.ControlSnapshotSHA256, value.ReleaseControlSnapshotSHA256, value.ScheduleSHA256,
		value.ScheduledContractSHA256, value.ScheduledProviderRequestSHA256,
		value.LogicalRequestSHA256, value.RequestBodySHA256, value.ExpectedLastEventSHA256,
		value.ProjectLaneSHA256, value.DestinationSHA256, value.PricingSnapshotSHA256,
		value.AuthorizationSHA256,
	}
}

func validAuthorizationPolicies(value Authorization) bool {
	budgets, failure := value.Budgets, value.Failure
	return value.ScheduleID == "graph-execution-schedule-"+value.ScheduleSHA256 &&
		value.ScheduledContractID == "scheduled-node-contract-"+value.ScheduledContractSHA256 &&
		value.ScheduledProviderRequestID == "scheduled-node-provider-request-"+
			value.ScheduledProviderRequestSHA256 &&
		value.LogicalRequestID == "scheduled-node-request-"+value.LogicalRequestSHA256 &&
		value.RequestBodyBytes >= 1 && value.RequestBodyBytes <= maxProviderRequestBytes &&
		value.SameProjectPolicy == "exclusive_until_terminal" &&
		value.ProviderKind == "openai_responses" && validAuthorizationBudgets(budgets) &&
		!failure.AutomaticRetry && !failure.LeaseRetry &&
		failure.PostClaimUncertainty == "dispatch_unknown" &&
		failure.FailurePropagationOwner == "forge_core"
}

func validAuthorizationBudgets(value graphdispatch.ExecutionBudgets) bool {
	return value.MaxTurns == 1 && value.MaxToolCalls == 0 && value.MaxOutputTokens >= 1 &&
		value.MaxOutputTokens <= graphdispatch.MaxOutputTokens && value.MaxModelOutputBytes >= 1 &&
		value.MaxModelOutputBytes <= graphdispatch.MaxModelOutputBytes && value.MaxModelEvents >= 1 &&
		value.MaxModelEvents <= graphdispatch.MaxModelEvents && value.TimeoutMilliseconds >= 1 &&
		value.TimeoutMilliseconds <= graphdispatch.MaxTimeoutMilliseconds &&
		value.MaxCostUSDMicros >= 1 && value.MaxCostUSDMicros <= graphdispatch.MaxCostUSDMicros &&
		isLowerHexDigest(value.PricingSnapshotSHA256)
}

func validAuthorizationFlags(value Authorization) bool {
	return value.LifecycleContractAdmissionAuthorized && value.ExecutionAuthorityReleaseAuthorized &&
		value.DispatchAuthorityReleaseAuthorized && value.ScheduledContractCandidatePresent &&
		value.ProviderRequestPrepared && !value.LifecycleContractAdmitted &&
		!value.ExecutionAuthorityReleased && !value.DispatchAuthorityReleased &&
		!value.ProjectLaneClaimed && !value.ProviderRequestSent && !value.ProgressObserved &&
		!value.TerminalReceiptRecorded && !value.SuccessorAdvanceAuthorized
}
