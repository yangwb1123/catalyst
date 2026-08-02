package graphterminal

import (
	"encoding/json"
	"math"
	"reflect"
	"strings"
	"unicode"
	"unicode/utf8"

	"forgeos/forge-core/internal/graphpricing"
	"forgeos/forge-core/internal/graphrelease"
)

type controlFacts struct {
	Release       graphrelease.ReleaseControl
	Authorization graphrelease.Authorization
	ClaimEvent    nodeDispatchReleasedEvent
	ClaimSHA256   string
}

func validateControl(control TerminalControl) error {
	if !validControlHeader(control) || !validSingleNodeTopology(control) {
		return errInvalidControl
	}
	facts, err := rebuildPriorState(control)
	if err != nil || validateTerminalRun(control, facts.Release) != nil ||
		validateClaimState(control, facts) != nil || validatePricing(control) != nil ||
		validateArtifact(control.Artifact, control) != nil {
		return errInvalidControl
	}
	digest, err := domainDigest(controlDigestDomain, controlPayload(control))
	if err != nil || digest != control.SnapshotSHA256 {
		return errInvalidControl
	}
	return nil
}

func validControlHeader(control TerminalControl) bool {
	return control.V == TerminalControlVersion &&
		control.SchedulerProtocolVersion == 1 &&
		control.TerminalControlProtocolVersion == TerminalControlProtocol &&
		control.JournalEvents != nil && len(control.JournalEvents) == 4 &&
		isLowerHexDigest(control.SnapshotSHA256)
}

func validSingleNodeTopology(control TerminalControl) bool {
	plan, manifest := control.Plan, control.Manifest
	return len(plan.AuthoredNodeIDs) == 1 && len(plan.Edges) == 0 &&
		len(plan.Waves) == 1 && plan.Waves[0] != nil && len(plan.Waves[0]) == 1 &&
		plan.Waves[0][0] == plan.AuthoredNodeIDs[0] && len(manifest.Nodes) == 1 &&
		len(manifest.Edges) == 0 && len(manifest.Waves) == 1 &&
		manifest.Waves[0] != nil && len(manifest.Waves[0]) == 1 &&
		manifest.Waves[0][0] == plan.AuthoredNodeIDs[0]
}

func rebuildPriorState(control TerminalControl) (controlFacts, error) {
	if !validEventBounds(control.JournalEvents) {
		return controlFacts{}, errInvalidControl
	}
	release := releaseFromTerminal(control)
	authorization, err := graphrelease.BuildAuthorization(release)
	if err != nil || !reflect.DeepEqual(authorization, control.Authorization) {
		return controlFacts{}, errInvalidControl
	}
	event, err := decodeExact[nodeDispatchReleasedEvent](control.JournalEvents[3])
	if err != nil {
		return controlFacts{}, errInvalidControl
	}
	claimSHA := rawDomainDigest(controlEventDomain, control.JournalEvents[3])
	return controlFacts{release, authorization, event, claimSHA}, nil
}

func validEventBounds(events []json.RawMessage) bool {
	if len(events) != 4 {
		return false
	}
	total := 0
	for _, event := range events {
		if len(event) == 0 || len(event) > maxEventBytes {
			return false
		}
		total += len(event)
	}
	return total <= 4*maxEventBytes
}

func releaseFromTerminal(control TerminalControl) graphrelease.ReleaseControl {
	run := control.GraphRun
	run.V, run.Status = 3, "awaiting_dispatch_authorization"
	run.DispatchAuthorityReleased, run.LastEventSeq = false, 3
	run.JournalBytes = journalBytes(control.JournalEvents[:3])
	return graphrelease.ReleaseControl{
		V:                             graphrelease.ReleaseControlVersion,
		SchedulerProtocolVersion:      control.SchedulerProtocolVersion,
		ReleaseControlProtocolVersion: graphrelease.ReleaseControlProtocol,
		GraphRun:                      run, Plan: control.Plan, Manifest: control.Manifest,
		JournalEvents: control.JournalEvents[:3], ContractRecord: control.ContractRecord,
		Contract: control.Contract, DispatchRequest: control.DispatchRequest,
		ProviderRequestJSON: control.ProviderRequestJSON,
		SnapshotSHA256:      control.Authorization.ReleaseControlSnapshotSHA256,
	}
}

func validateTerminalRun(control TerminalControl, release graphrelease.ReleaseControl) error {
	expected := release.GraphRun
	expected.V, expected.Status = 4, "dispatch_unknown"
	expected.DispatchAuthorityReleased, expected.LastEventSeq = true, 4
	expected.JournalBytes = journalBytes(control.JournalEvents)
	if !reflect.DeepEqual(expected, control.GraphRun) {
		return errInvalidControl
	}
	return nil
}

func journalBytes(events []json.RawMessage) uint64 {
	var total uint64
	for _, event := range events {
		total += uint64(len(event))
	}
	return total
}

func validatePricing(control TerminalControl) error {
	encoded, err := graphpricing.Marshal(control.Pricing)
	auth := control.Authorization
	maximum, costErr := graphpricing.WorstCostUSDMicros(control.Pricing, auth.Budgets.MaxOutputTokens)
	valid := err == nil && len(encoded) > 0 && costErr == nil && maximum <= auth.Budgets.MaxCostUSDMicros &&
		control.Pricing.PricingSnapshotSHA256 == auth.PricingSnapshotSHA256 &&
		control.Pricing.Model == auth.Model && control.Pricing.ProviderKind == auth.ProviderKind &&
		control.Pricing.Endpoint == auth.Endpoint && control.Pricing.DestinationSHA256 == auth.DestinationSHA256
	if !valid {
		return errInvalidControl
	}
	return nil
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

func validSignedTime(value uint64) bool { return value <= math.MaxInt64 }
