package graphscheduledreconcile

import (
	"strings"
	"unicode"
	"unicode/utf8"
)

func validateSnapshot(value ProgressSnapshot) error {
	if !validSnapshotHeader(value) || !validFixedPolicy(value) || !validNodes(value) {
		return errInvalidSnapshot
	}
	digest, err := domainDigest(snapshotDigestDomain, payloadFromSnapshot(value))
	if err != nil || digest != value.SnapshotSHA256 {
		return errInvalidSnapshot
	}
	encoded, err := canonicalBytes(value)
	if err != nil || len(encoded) == 0 || len(encoded) > MaxProgressSnapshotBytes {
		return errInvalidSnapshot
	}
	return nil
}

func validSnapshotHeader(value ProgressSnapshot) bool {
	return value.V == 1 && value.ProgressProtocolVersion == ProgressProtocolVersion &&
		validIdentifier(value.GraphRunID) && validIdentifier(value.GraphID) &&
		isLowerHexDigest(value.ScheduleSHA256) &&
		value.ScheduleID == scheduleIDPrefix+value.ScheduleSHA256 &&
		value.NodeCount >= 2 && value.NodeCount <= 32 &&
		len(value.Nodes) == int(value.NodeCount) && isLowerHexDigest(value.SnapshotSHA256)
}

func validFixedPolicy(value ProgressSnapshot) bool {
	return value.ExecutionMode == "serial" && value.MaxInFlightNodes == 1 &&
		value.ProgressionPolicy == "completed_contiguous_prefix" &&
		value.AttemptPolicy == "exactly_one" && value.FailurePolicy == "fail_fast_no_retry"
}

func validNodes(value ProgressSnapshot) bool {
	identifiers := make(map[string]struct{}, len(value.Nodes))
	candidates := make(map[string]struct{}, len(value.Nodes))
	providerRequests := make(map[string]struct{}, len(value.Nodes))
	receipts := make(map[string]struct{}, len(value.Nodes))
	for index, node := range value.Nodes {
		if node.ExecutionOrdinal != uint16(index) || node.Attempt != 1 ||
			!validIdentifier(node.NodeID) || !validNodeEvidence(node) {
			return false
		}
		if _, duplicate := identifiers[node.NodeID]; duplicate {
			return false
		}
		if !addOptionalUnique(candidates, node.CandidateID) ||
			!addOptionalUnique(providerRequests, node.ProviderRequestID) ||
			!addOptionalUnique(receipts, node.TerminalReceiptSHA256) {
			return false
		}
		identifiers[node.NodeID] = struct{}{}
	}
	return true
}

func addOptionalUnique(seen map[string]struct{}, value *string) bool {
	if value == nil {
		return true
	}
	if _, duplicate := seen[*value]; duplicate {
		return false
	}
	seen[*value] = struct{}{}
	return true
}

func validNodeEvidence(node ProgressNode) bool {
	if !validContentIdentity(node.CandidateID, node.CandidateSHA256, candidateIDPrefix) ||
		!validContentIdentity(node.ProviderRequestID, node.PreparedRequestSHA256, providerIDPrefix) {
		return false
	}
	if node.ProviderRequestID != nil && node.CandidateID == nil {
		return false
	}
	if node.LifecycleStatus != nil && node.ProviderRequestID == nil {
		return false
	}
	return validLifecycleEvidence(node)
}

func validContentIdentity(identifier, digest *string, prefix string) bool {
	if identifier == nil || digest == nil {
		return identifier == nil && digest == nil
	}
	return isLowerHexDigest(*digest) && *identifier == prefix+*digest
}

func validLifecycleEvidence(node ProgressNode) bool {
	if node.LifecycleStatus == nil {
		return node.TerminalOutcome == nil && node.TerminalReceiptSHA256 == nil
	}
	switch *node.LifecycleStatus {
	case "terminalized":
		return validTerminalEvidence(node)
	case "claimed", "quarantined", "adjudicated":
		return node.TerminalOutcome == nil && node.TerminalReceiptSHA256 == nil
	default:
		return false
	}
}

func validTerminalEvidence(node ProgressNode) bool {
	if node.TerminalOutcome == nil || node.TerminalReceiptSHA256 == nil ||
		!isLowerHexDigest(*node.TerminalReceiptSHA256) {
		return false
	}
	switch *node.TerminalOutcome {
	case "completed", "failed", "failed_uncertain":
		return true
	default:
		return false
	}
}

func isLowerHexDigest(value string) bool {
	if len(value) != 64 {
		return false
	}
	for _, character := range []byte(value) {
		if (character < '0' || character > '9') && (character < 'a' || character > 'f') {
			return false
		}
	}
	return true
}

func validIdentifier(value string) bool {
	return utf8.ValidString(value) && strings.TrimSpace(value) != "" && len(value) <= 128 &&
		!strings.ContainsFunc(value, func(character rune) bool {
			return unicode.IsControl(character) || character == '\u061c' ||
				character == '\u200e' || character == '\u200f' ||
				character >= '\u2028' && character <= '\u202e' ||
				character >= '\u2066' && character <= '\u2069'
		})
}
