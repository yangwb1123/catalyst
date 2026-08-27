package graphscheduledrelease

import "reflect"

func validateReadyAuthorization(value ReadyAuthorization) error {
	if !validReadyAuthorizationHeader(value) || !validReadyAuthorizationPolicies(value) ||
		!validReadyAuthorizationFlags(value) ||
		!reflect.DeepEqual(value.ReleaseRequirements, readyReleaseRequirements()) {
		return errInvalidControl
	}
	digest, err := domainDigest(readyAuthorizationDigestDomain, readyAuthorizationPayloadFrom(value))
	if err != nil || digest != value.AuthorizationSHA256 ||
		value.AuthorizationID != readyAuthorizationIDPrefix+digest {
		return errInvalidControl
	}
	return nil
}

func validReadyAuthorizationHeader(value ReadyAuthorization) bool {
	texts := []string{
		value.GraphRunID, value.GraphID, value.GroupRunID, value.GroupID,
		value.ScheduleID, value.ScheduledContractID, value.ScheduledProviderRequestID,
		value.LogicalRequestID, value.NodeID, value.ProjectID, value.Endpoint, value.Model,
	}
	for _, item := range texts {
		if !validText(item, 2*1024) {
			return false
		}
	}
	for _, digest := range readyAuthorizationDigests(value) {
		if !isLowerHexDigest(digest) {
			return false
		}
	}
	return value.V == ReadyAuthorizationVersion && value.SchedulerProtocolVersion == 1 &&
		value.DispatchAuthorizationProtocolVersion == ReadyAuthorizationProtocol &&
		value.ExpectedLastEventSeq == 1 && value.ExecutionOrdinal <= 31 && value.Attempt == 1
}

func readyAuthorizationDigests(value ReadyAuthorization) []string {
	return []string{
		value.SourceSnapshotSHA256, value.GraphManifestSHA256, value.CorePlanSHA256,
		value.ControlSnapshotSHA256, value.ReleaseControlSnapshotSHA256,
		value.ProgressSnapshotSHA256, value.ReconcileDecisionSHA256, value.ScheduleSHA256,
		value.ScheduledContractSHA256, value.ScheduledProviderRequestSHA256,
		value.LogicalRequestSHA256, value.RequestBodySHA256, value.ExpectedLastEventSHA256,
		value.ProjectLaneSHA256, value.DestinationSHA256, value.PricingSnapshotSHA256,
		value.AuthorizationSHA256,
	}
}

func validReadyAuthorizationPolicies(value ReadyAuthorization) bool {
	failure := value.Failure
	return value.ScheduleID == "graph-execution-schedule-"+value.ScheduleSHA256 &&
		value.ScheduledContractID == "scheduled-node-contract-"+value.ScheduledContractSHA256 &&
		value.ScheduledProviderRequestID == "scheduled-node-provider-request-"+
			value.ScheduledProviderRequestSHA256 &&
		value.LogicalRequestID == "scheduled-node-request-"+value.LogicalRequestSHA256 &&
		value.RequestBodyBytes >= 1 && value.RequestBodyBytes <= maxProviderRequestBytes &&
		value.MaximumFutureNodeReleases == 1 && value.SameProjectPolicy == "exclusive_until_terminal" &&
		value.ProviderKind == "openai_responses" && validAuthorizationBudgets(value.Budgets) &&
		!failure.AutomaticRetry && !failure.LeaseRetry &&
		failure.PostClaimUncertainty == "dispatch_unknown" &&
		failure.FailurePropagationOwner == "forge_core"
}

func validReadyAuthorizationFlags(value ReadyAuthorization) bool {
	return value.LifecycleContractAdmissionAuthorized && value.ExecutionAuthorityReleaseAuthorized &&
		value.DispatchAuthorityReleaseAuthorized && value.ScheduledContractCandidatePresent &&
		value.ProviderRequestPrepared && !value.LifecycleContractAdmitted &&
		!value.ExecutionAuthorityReleased && !value.DispatchAuthorityReleased &&
		!value.ProjectLaneClaimed && !value.ProviderRequestSent && !value.ProgressObserved &&
		!value.TerminalReceiptRecorded && !value.SuccessorAdvanceAuthorized
}
