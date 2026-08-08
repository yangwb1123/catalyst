package graphterminal

import (
	"unicode/utf8"

	"forgeos/forge-core/internal/graphpricing"
)

var uncertaintyClasses = map[string]struct{}{
	"provider_error": {}, "http_error": {}, "transport_error": {},
	"timeout": {}, "cancelled": {}, "eof_before_terminal": {},
	"missing_usage": {}, "tool_call": {}, "protocol_error": {},
	"trailing_data": {}, "local_limit": {}, "hard_crash": {},
}

func validateArtifact(artifact TerminalArtifact, control TerminalControl) error {
	if !validArtifactHeader(artifact) || !artifactBindsClaim(artifact, control) ||
		!validArtifactOutput(artifact, control) || !validArtifactOutcome(artifact) ||
		!validUsageAndCost(artifact, control) || validateArtifactIdentity(artifact) != nil {
		return errInvalidControl
	}
	return nil
}

func validArtifactHeader(value TerminalArtifact) bool {
	return value.V == TerminalArtifactVersion &&
		value.TerminalArtifactProtocolVersion == TerminalArtifactProtocol &&
		(value.ArtifactKind == "result" || value.ArtifactKind == "uncertainty") &&
		validIdentifier(value.GraphRunID) && validIdentifier(value.NodeID) &&
		validIdentifier(value.DispatchID) && validIdentifier(value.LaneOwnershipID) &&
		value.Attempt == 1 && !value.RetryAuthorized && validSignedTime(value.CreatedAtMS) &&
		isLowerHexDigest(value.ClaimEventSHA256) &&
		isLowerHexDigest(value.AuthorizationSHA256) &&
		isLowerHexDigest(value.DispatchRequestSHA256) &&
		isLowerHexDigest(value.LogicalRequestSHA256) &&
		isLowerHexDigest(value.RequestBodySHA256) &&
		isLowerHexDigest(value.PricingSnapshotSHA256) &&
		isLowerHexDigest(value.ProjectLaneSHA256) &&
		isLowerHexDigest(value.OutputSHA256) && isLowerHexDigest(value.ArtifactSHA256)
}

func artifactBindsClaim(value TerminalArtifact, control TerminalControl) bool {
	claim := control.Claim
	return value.GraphRunID == claim.GraphRunID && value.NodeID == claim.NodeID &&
		value.Attempt == claim.Attempt && value.DispatchID == claim.DispatchID &&
		value.ClaimEventSHA256 == claim.ClaimEventSHA256 &&
		value.AuthorizationSHA256 == claim.AuthorizationSHA256 &&
		value.DispatchRequestSHA256 == claim.DispatchRequestSHA256 &&
		value.LogicalRequestSHA256 == claim.LogicalRequestSHA256 &&
		value.RequestBodySHA256 == claim.RequestBodySHA256 &&
		value.PricingSnapshotSHA256 == claim.PricingSnapshotSHA256 &&
		value.LaneOwnershipID == claim.LaneOwnershipID &&
		value.ProjectLaneSHA256 == claim.ProjectLaneSHA256 &&
		value.CreatedAtMS >= claim.ReleasedAtMS
}

func validArtifactOutput(value TerminalArtifact, control TerminalControl) bool {
	length := uint64(len(value.OutputText))
	maximumModel := control.Authorization.Budgets.MaxModelOutputBytes
	maximumResult := control.Contract.Result.MaxResultBytes
	return utf8.ValidString(value.OutputText) && length == value.OutputBytes &&
		length <= maximumModel && length <= maximumResult &&
		rawDomainDigest(outputDigestDomain, []byte(value.OutputText)) == value.OutputSHA256
}

func validArtifactOutcome(value TerminalArtifact) bool {
	if value.ArtifactKind == "result" {
		validClass := value.Classification == "completed" || value.Classification == "length"
		outputValid := value.Classification != "completed" || value.OutputBytes > 0
		return validClass && outputValid && value.ProviderPollStarted && value.TerminalSeen &&
			value.StreamEOFSeen && value.UsageObserved && value.ActualCostCalculated
	}
	return validUncertaintyOutcome(value)
}

func validUncertaintyOutcome(value TerminalArtifact) bool {
	_, valid := uncertaintyClasses[value.Classification]
	chronology := value.ProviderPollStarted || !value.TerminalSeen && !value.StreamEOFSeen
	missingUsage := value.Classification != "missing_usage" ||
		value.TerminalSeen && value.StreamEOFSeen &&
			!value.UsageObserved && !value.ActualCostCalculated
	return valid && chronology && missingUsage
}

func validUsageAndCost(value TerminalArtifact, control TerminalControl) bool {
	if !value.UsageObserved && (value.InputTokens != 0 || value.OutputTokens != 0) {
		return false
	}
	if value.UsageObserved && (value.InputTokens == 0 || value.OutputTokens == 0 ||
		value.InputTokens > control.Pricing.MaxInputTokens ||
		value.OutputTokens > control.Authorization.Budgets.MaxOutputTokens) {
		return false
	}
	if !value.ActualCostCalculated && value.ActualCostUSDMicros != 0 {
		return false
	}
	if !value.ActualCostCalculated {
		return true
	}
	if !value.UsageObserved {
		return false
	}
	cost, err := graphpricing.ActualCostUSDMicros(
		control.Pricing, value.InputTokens, value.OutputTokens,
	)
	return err == nil && cost == value.ActualCostUSDMicros &&
		cost <= control.Authorization.Budgets.MaxCostUSDMicros
}

func validateArtifactIdentity(value TerminalArtifact) error {
	payload := payloadFromArtifact(value)
	encoded, err := canonicalBytes(payload)
	if err != nil || len(encoded) == 0 || len(encoded) > maxArtifactPayloadBytes ||
		value.ArtifactBytes != uint64(len(encoded)) {
		return errInvalidControl
	}
	digest := rawDomainDigest(artifactDigestDomain, encoded)
	if value.ArtifactSHA256 != digest || value.ArtifactID != "graph-node-terminal-artifact-"+digest {
		return errInvalidControl
	}
	return nil
}
