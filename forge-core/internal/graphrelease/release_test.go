package graphrelease

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"

	"forgeos/forge-core/internal/graphdispatch"
)

func TestDecodeAndBuildAuthorizationFromExactReleaseControl(t *testing.T) {
	control, encoded := validReleaseFixture(t)
	if bytes.HasSuffix(encoded, []byte{'\n'}) {
		t.Fatal("release-control fixture has a trailing LF")
	}
	authorization, err := BuildAuthorization(control)
	if err != nil {
		t.Fatalf("BuildAuthorization: %v", err)
	}
	output, err := MarshalAuthorization(authorization)
	if err != nil {
		t.Fatalf("MarshalAuthorization: %v", err)
	}
	if bytes.HasSuffix(output, []byte{'\n'}) {
		t.Fatal("authorization has a trailing LF")
	}
	if authorization.ExpectedLastEventSeq != 3 ||
		authorization.ExpectedLastEventSHA256 == control.Contract.ExpectedLastEventSHA256 ||
		authorization.DispatchAuthorityReleased ||
		!authorization.DispatchAuthorityReleaseAuthorized {
		t.Fatalf("unsafe authorization cursor or authority flags: %+v", authorization)
	}
	if authorization.ReleaseRequirements != releaseRequirements() {
		t.Fatalf("release requirements drifted: %+v", authorization.ReleaseRequirements)
	}
}

func TestDecodeControlRejectsNoncanonicalUnknownDuplicateAndTrailingInput(t *testing.T) {
	_, canonical := validReleaseFixture(t)
	inputs := [][]byte{
		append(append([]byte(nil), canonical...), '\n'),
		append(append([]byte(nil), canonical...), []byte("{}")...),
		bytes.Replace(canonical, []byte(`{"v":1`), []byte(`{"unknown":0,"v":1`), 1),
		bytes.Replace(canonical, []byte(`{"v":1`), []byte(`{"v":1,"v":1`), 1),
		bytes.Replace(canonical, []byte("awaiting_dispatch_authorization"),
			[]byte(`awaiting_dispatch_\u0061uthorization`), 1),
	}
	for index, input := range inputs {
		if _, err := DecodeControl(bytes.NewReader(input)); err == nil {
			t.Fatalf("invalid input %d was accepted", index)
		}
	}
}

func TestDecodeControlRejectsInvalidUTF8AndInputPast48MiB(t *testing.T) {
	for name, input := range map[string][]byte{
		"invalid UTF-8": {0xff},
		"oversize":      bytes.Repeat([]byte{'x'}, MaxReleaseControlBytes+1),
	} {
		t.Run(name, func(t *testing.T) {
			if _, err := DecodeControl(bytes.NewReader(input)); err == nil {
				t.Fatal("invalid bounded input was accepted")
			}
		})
	}
}

func TestBuildAuthorizationRejectsResignedNestedTampering(t *testing.T) {
	base, _ := validReleaseFixture(t)
	for _, test := range releaseTamperCases(t) {
		t.Run(test.name, func(t *testing.T) {
			changed := base
			changed.JournalEvents = cloneEvents(base.JournalEvents)
			test.mutate(&changed)
			resignReleaseControl(t, &changed)
			if _, err := BuildAuthorization(changed); err == nil {
				t.Fatal("resigned nested tampering was accepted")
			}
		})
	}
}

type releaseTamperCase struct {
	name   string
	mutate func(*ReleaseControl)
}

func releaseTamperCases(t *testing.T) []releaseTamperCase {
	return append(releaseEnvelopeTamperCases(t), releaseBindingTamperCases()...)
}

func releaseEnvelopeTamperCases(t *testing.T) []releaseTamperCase {
	return []releaseTamperCase{
		{"control version", func(value *ReleaseControl) { value.V++ }},
		{"release protocol", func(value *ReleaseControl) {
			value.ReleaseControlProtocolVersion++
		}},
		{"run authority", func(value *ReleaseControl) {
			value.GraphRun.DispatchAuthorityReleased = true
		}},
		{"provider body", func(value *ReleaseControl) { value.ProviderRequestJSON += " " }},
		{"journal head", func(value *ReleaseControl) {
			event, _ := decodeExact[dispatchEvent](value.JournalEvents[2])
			event.PreviousEventSHA256 = strings.Repeat("a", 64)
			value.JournalEvents[2] = mustCanonicalTest(t, event)
		}},
	}
}

func releaseBindingTamperCases() []releaseTamperCase {
	return []releaseTamperCase{
		{"source", func(value *ReleaseControl) {
			value.GraphRun.SourceSnapshotSHA256 = strings.Repeat("a", 64)
		}},
		{"logical request", func(value *ReleaseControl) {
			value.DispatchRequest.RequestSHA256 = strings.Repeat("a", 64)
		}},
		{"project lane", func(value *ReleaseControl) {
			value.Contract.Node.ProjectLaneSHA256 = strings.Repeat("a", 64)
		}},
		{"destination", func(value *ReleaseControl) {
			value.DispatchRequest.DestinationSHA256 = strings.Repeat("a", 64)
		}},
		{"pricing", func(value *ReleaseControl) {
			value.DispatchRequest.PricingSnapshotSHA256 = strings.Repeat("a", 64)
		}},
		{"budget", func(value *ReleaseControl) { value.Contract.Budgets.MaxTurns = 2 }},
		{"failure", func(value *ReleaseControl) { value.Contract.Failure.AutomaticRetry = true }},
	}
}

func TestAuthorizationRejectsResignedVersionAndReleasePolicyDrift(t *testing.T) {
	control, _ := validReleaseFixture(t)
	base, err := BuildAuthorization(control)
	if err != nil {
		t.Fatalf("BuildAuthorization: %v", err)
	}
	for _, test := range authorizationTamperCases() {
		t.Run(test.name, func(t *testing.T) {
			changed := base
			test.mutate(&changed)
			resignAuthorization(t, &changed)
			if _, err := MarshalAuthorization(changed); err == nil {
				t.Fatal("resigned authorization policy tampering was accepted")
			}
		})
	}
}

type authorizationTamperCase struct {
	name   string
	mutate func(*Authorization)
}

func authorizationTamperCases() []authorizationTamperCase {
	return []authorizationTamperCase{
		{"version", func(value *Authorization) { value.V++ }},
		{"scheduler", func(value *Authorization) { value.SchedulerProtocolVersion++ }},
		{"protocol", func(value *Authorization) {
			value.DispatchAuthorizationProtocolVersion++
		}},
		{"authority released", func(value *Authorization) {
			value.DispatchAuthorityReleased = true
		}},
		{"provider health", func(value *Authorization) {
			value.ReleaseRequirements.ProviderHealthCheck = "required"
		}},
		{"failure retry", func(value *Authorization) { value.Failure.AutomaticRetry = true }},
	}
}

func resignAuthorization(t *testing.T, value *Authorization) {
	t.Helper()
	digest := mustDomainDigestTest(t, authorizationDigestDomain, authorizationPayloadFrom(*value))
	value.AuthorizationSHA256 = digest
	value.AuthorizationID = "node-dispatch-authorization-" + digest
}

func TestReconstructedOriginalControlRejectsSelfConsistentSnapshotReplacement(t *testing.T) {
	control, _ := validReleaseFixture(t)
	replaceControlSnapshotAndResign(t, &control)
	if _, err := graphdispatch.MarshalContract(control.Contract); err != nil {
		t.Fatalf("tampered contract must remain locally valid: %v", err)
	}
	if validateContractHeaderBindings(control, mustJournalFacts(t, control)) != nil {
		t.Fatal("tamper setup did not keep the downstream contract bindings self-consistent")
	}
	if _, err := BuildAuthorization(control); err == nil {
		t.Fatal("self-consistent replacement of the original control identity was accepted")
	}
}

func replaceControlSnapshotAndResign(t *testing.T, control *ReleaseControl) {
	t.Helper()
	oldContractJSON, err := graphdispatch.MarshalContract(control.Contract)
	if err != nil {
		t.Fatalf("marshal source contract: %v", err)
	}
	marker := bytes.LastIndex(oldContractJSON, []byte(`,"contract_id":`))
	if marker < 0 {
		t.Fatal("contract identity marker is absent")
	}
	oldSnapshot := control.Contract.ControlSnapshotSHA256
	newSnapshot := strings.Repeat("a", 64)
	payload := append(append([]byte(nil), oldContractJSON[:marker]...), '}')
	payload = bytes.Replace(payload, []byte(oldSnapshot), []byte(newSnapshot), 1)
	digest := rawDomainDigest("forge.group-agent-node-execution-contract.v1\x00", payload)
	control.Contract.ControlSnapshotSHA256 = newSnapshot
	control.Contract.ContractSHA256 = digest
	control.Contract.ContractID = "node-contract-" + digest
	contractJSON, err := graphdispatch.MarshalContract(control.Contract)
	if err != nil {
		t.Fatalf("marshal resigned contract: %v", err)
	}
	preparedHead := rawDomainDigest(preparedEventDigestDomain, control.JournalEvents[0])
	control.ContractRecord = contractRecordFor(control.Contract, uint64(len(contractJSON)), 80)
	admission := admissionEventFor(control.Contract, uint64(len(contractJSON)), preparedHead)
	control.JournalEvents[1] = mustCanonicalTest(t, admission)
	admissionHead := rawDomainDigest(controlEventDigestDomain, control.JournalEvents[1])
	control.DispatchRequest = dispatchRecordFor(
		t, control.Contract, []byte(control.ProviderRequestJSON), admissionHead,
	)
	control.JournalEvents[2] = mustCanonicalTest(t, preparationEventFor(control.DispatchRequest))
	control.GraphRun.JournalBytes = uint64(len(control.JournalEvents[0]) +
		len(control.JournalEvents[1]) + len(control.JournalEvents[2]))
	resignReleaseControl(t, control)
}

func resignReleaseControl(t *testing.T, control *ReleaseControl) {
	t.Helper()
	control.SnapshotSHA256 = mustDomainDigestTest(t, releaseControlDigestDomain, releasePayload(*control))
}

func mustJournalFacts(t *testing.T, control ReleaseControl) journalFacts {
	t.Helper()
	facts, err := decodeJournal(control.JournalEvents)
	if err != nil {
		t.Fatalf("decode journal facts: %v", err)
	}
	return facts
}

func cloneEvents(events []json.RawMessage) []json.RawMessage {
	cloned := make([]json.RawMessage, len(events))
	for index, event := range events {
		cloned[index] = append(json.RawMessage(nil), event...)
	}
	return cloned
}
