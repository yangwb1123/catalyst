package graphterminal

import "reflect"

func validateClaimState(control TerminalControl, facts controlFacts) error {
	claim := control.Claim
	if !validClaimHeader(claim) || !claimBindsAuthorization(claim, control) ||
		claim.ExpectedLastEventSeq != 3 ||
		claim.ReleasedAtMS < control.DispatchRequest.CreatedAtMS ||
		claim.ExpectedLastEventSHA256 != facts.Authorization.ExpectedLastEventSHA256 ||
		claim.ClaimEventSHA256 != facts.ClaimSHA256 ||
		!reflect.DeepEqual(facts.ClaimEvent, eventFromClaim(claim)) ||
		validateActiveLane(control.ActiveLane, claim) != nil {
		return errInvalidControl
	}
	return nil
}

func validClaimHeader(claim Claim) bool {
	return claim.V == ClaimVersion && validIdentifier(claim.GraphRunID) &&
		validIdentifier(claim.DispatchID) && validIdentifier(claim.AuthorizationID) &&
		validIdentifier(claim.DispatchRequestID) && validIdentifier(claim.NodeID) &&
		validIdentifier(claim.LaneOwnershipID) && claim.Attempt == 1 &&
		claim.ConsentContractVersion == 1 && validSignedTime(claim.ReleasedAtMS) &&
		isLowerHexDigest(claim.AuthorizationSHA256) &&
		isLowerHexDigest(claim.DispatchRequestSHA256) &&
		isLowerHexDigest(claim.LogicalRequestSHA256) &&
		isLowerHexDigest(claim.RequestBodySHA256) &&
		isLowerHexDigest(claim.PricingSnapshotSHA256) &&
		isLowerHexDigest(claim.ProjectLaneSHA256) &&
		isLowerHexDigest(claim.ExpectedLastEventSHA256) &&
		isLowerHexDigest(claim.ClaimEventSHA256)
}

func claimBindsAuthorization(claim Claim, control TerminalControl) bool {
	auth, request := control.Authorization, control.DispatchRequest
	return claim.GraphRunID == auth.GraphRunID && claim.AuthorizationID == auth.AuthorizationID &&
		claim.AuthorizationSHA256 == auth.AuthorizationSHA256 &&
		claim.DispatchRequestID == auth.DispatchRequestID &&
		claim.DispatchRequestSHA256 == auth.DispatchRequestSHA256 &&
		claim.LogicalRequestSHA256 == auth.LogicalRequestSHA256 &&
		claim.RequestBodySHA256 == auth.RequestBodySHA256 &&
		claim.RequestBodyBytes == auth.RequestBodyBytes &&
		claim.PricingSnapshotSHA256 == auth.PricingSnapshotSHA256 &&
		claim.NodeID == auth.NodeID && claim.Attempt == auth.Attempt &&
		claim.MaxCostUSDMicros == auth.Budgets.MaxCostUSDMicros &&
		claim.ConsentContractVersion == auth.ReleaseRequirements.ConsentContractVersion &&
		claim.ProjectLaneSHA256 == auth.ProjectLaneSHA256 &&
		claim.DispatchRequestID == request.DispatchRequestID
}

func validateActiveLane(lane ActiveLane, claim Claim) error {
	expected := ActiveLane{
		V: ActiveLaneVersion, ProjectLaneSHA256: claim.ProjectLaneSHA256,
		LaneOwnershipID: claim.LaneOwnershipID, GraphRunID: claim.GraphRunID,
		NodeID: claim.NodeID, Attempt: claim.Attempt, DispatchID: claim.DispatchID,
		ClaimEventSHA256: claim.ClaimEventSHA256, ClaimedAtMS: claim.ReleasedAtMS,
	}
	if !reflect.DeepEqual(lane, expected) {
		return errInvalidControl
	}
	return nil
}
