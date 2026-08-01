package graphrelease

import (
	"reflect"
	"strings"

	"forgeos/forge-core/internal/graphdispatch"
	"forgeos/forge-core/internal/graphplan"
)

func validateAuthorization(value Authorization) error {
	if !validAuthorizationHeader(value) || !validAuthorizationPolicy(value) {
		return errInvalidControl
	}
	destination := destinationPayload{
		V: 1, ProviderKind: value.ProviderKind, Endpoint: value.Endpoint, Model: value.Model,
	}
	destinationSHA256, err := domainDigest(destinationDigestDomain, destination)
	if err != nil || destinationSHA256 != value.DestinationSHA256 ||
		rawDomainDigest(projectLaneDigestDomain, []byte(value.ProjectID)) != value.ProjectLaneSHA256 {
		return errInvalidControl
	}
	digest, err := domainDigest(authorizationDigestDomain, authorizationPayloadFrom(value))
	if err != nil || digest != value.AuthorizationSHA256 ||
		value.AuthorizationID != "node-dispatch-authorization-"+digest {
		return errInvalidControl
	}
	return nil
}

func validAuthorizationHeader(value Authorization) bool {
	return value.V == AuthorizationVersion &&
		value.SchedulerProtocolVersion == graphplan.SchedulerProtocolVersion &&
		value.DispatchAuthorizationProtocolVersion == AuthorizationProtocol &&
		validText(value.GraphRunID, 128) && validText(value.GraphID, 128) &&
		validText(value.GroupRunID, 128) && validText(value.NodeID, 128) &&
		validText(value.ProjectID, 128) && validText(value.Model, graphdispatch.MaxModelBytes) &&
		validAuthorizationDigests(value) && value.ExpectedLastEventSeq == 3 &&
		value.ContractID == "node-contract-"+value.ContractSHA256 &&
		value.DispatchRequestID == "node-dispatch-request-"+value.DispatchRequestSHA256 &&
		value.RequestBodyBytes >= 1 && value.RequestBodyBytes <= maxProviderRequestBytes &&
		value.Attempt == 1 && value.SameProjectPolicy == "exclusive_until_terminal" &&
		value.ProviderKind == "openai_responses" && validAuthorizationEndpoint(value.Endpoint)
}

func validAuthorizationDigests(value Authorization) bool {
	return isLowerHexDigest(value.SourceSnapshotSHA256) &&
		isLowerHexDigest(value.GraphManifestSHA256) && isLowerHexDigest(value.CorePlanSHA256) &&
		isLowerHexDigest(value.ReleaseControlSnapshotSHA256) &&
		isLowerHexDigest(value.ExpectedLastEventSHA256) &&
		isLowerHexDigest(value.ContractSHA256) && isLowerHexDigest(value.DispatchRequestSHA256) &&
		isLowerHexDigest(value.LogicalRequestSHA256) &&
		isLowerHexDigest(value.RequestBodySHA256) && isLowerHexDigest(value.ProjectLaneSHA256) &&
		isLowerHexDigest(value.DestinationSHA256) &&
		isLowerHexDigest(value.PricingSnapshotSHA256) &&
		isLowerHexDigest(value.AuthorizationSHA256)
}

func validAuthorizationPolicy(value Authorization) bool {
	failure := value.Failure
	return value.ExecutionContractPresent && value.DispatchRequestPresent &&
		value.DispatchAuthorityReleaseAuthorized && !value.DispatchAuthorityReleased &&
		reflect.DeepEqual(value.ReleaseRequirements, releaseRequirements()) &&
		validAuthorizationBudgets(value.Budgets) &&
		value.PricingSnapshotSHA256 == value.Budgets.PricingSnapshotSHA256 &&
		!failure.AutomaticRetry && !failure.LeaseRetry &&
		failure.PostClaimUncertainty == "dispatch_unknown" &&
		failure.FailurePropagationOwner == "forge_core"
}

func validAuthorizationEndpoint(value string) bool {
	return validText(value, graphdispatch.MaxEndpointBytes) &&
		strings.HasPrefix(value, "https://") && !strings.ContainsAny(value, "?#@")
}
