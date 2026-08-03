package graphscheduledrelease

import (
	"bytes"
	"strings"
	"testing"
)

func TestDecodeAndBuildAuthorizationFromExactPristineControl(t *testing.T) {
	control, encoded := validReleaseFixture(t)
	decoded, err := DecodeControl(bytes.NewReader(encoded))
	if err != nil {
		t.Fatalf("DecodeControl: %v", err)
	}
	authorization, err := BuildAuthorization(decoded)
	if err != nil {
		t.Fatalf("BuildAuthorization: %v", err)
	}
	assertAuthorizationBindings(t, control, authorization)
	output, err := MarshalAuthorization(authorization)
	if err != nil {
		t.Fatalf("MarshalAuthorization: %v", err)
	}
	if len(output) == 0 || output[len(output)-1] == '\n' {
		t.Fatalf("authorization is empty or newline terminated: %q", output)
	}
}

func assertAuthorizationBindings(t *testing.T, control ReleaseControl, value Authorization) {
	t.Helper()
	contract, request := control.ScheduledContract, control.ProviderRequest
	if value.AuthorizationID != authorizationIDPrefix+value.AuthorizationSHA256 ||
		value.ReleaseControlSnapshotSHA256 != control.SnapshotSHA256 ||
		value.ScheduleSHA256 != control.Schedule.ScheduleSHA256 ||
		value.ScheduledContractSHA256 != contract.ContractSHA256 ||
		value.ScheduledProviderRequestSHA256 != request.PreparedRequestSHA256 ||
		value.RequestBodySHA256 != request.ProviderRequestSHA256 ||
		value.ExpectedLastEventSeq != 1 || value.ExecutionOrdinal != 0 {
		t.Fatalf("authorization bindings disagree: %+v", value)
	}
	if !validAuthorizationFlags(value) || value.GroupID != "group-fixture-v1" ||
		value.GroupRunID != "group-run-fixture-v1" {
		t.Fatalf("authorization flags or Group provenance disagree: %+v", value)
	}
}

func TestBuildRejectsResignedNestedSubstitution(t *testing.T) {
	control, _ := validReleaseFixture(t)
	cases := []struct {
		name   string
		mutate func(*ReleaseControl)
	}{
		{"run state", func(v *ReleaseControl) { v.GraphRun.Status = "completed" }},
		{"journal", func(v *ReleaseControl) { v.JournalEvents[0] = []byte(`{}`) }},
		{"control source", func(v *ReleaseControl) { v.ControlSnapshot.GraphID = "other" }},
		{"schedule record", func(v *ReleaseControl) { v.ScheduleRecord.ScheduleBytes++ }},
		{"schedule", func(v *ReleaseControl) { v.Schedule.InitialNode = "backend" }},
		{"contract record", func(v *ReleaseControl) { v.ScheduledContractRecord.ContractBytes++ }},
		{"contract progress", func(v *ReleaseControl) { v.ScheduledContract.ProgressObserved = true }},
		{"request lane", func(v *ReleaseControl) { v.ProviderRequest.ProjectLaneSHA256 = strings.Repeat("0", 64) }},
		{"request sent", func(v *ReleaseControl) { v.ProviderRequest.ProviderRequestSent = true }},
		{"request body", func(v *ReleaseControl) { v.ProviderRequestJSON += " " }},
		{"pricing", func(v *ReleaseControl) { v.ProviderRequest.PricingSnapshotSHA256 = strings.Repeat("0", 64) }},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			mutated := cloneControlTest(t, control)
			test.mutate(&mutated)
			resignControlTest(t, &mutated)
			if _, err := BuildAuthorization(mutated); err == nil {
				t.Fatal("resigned substituted control was accepted")
			}
		})
	}
}

func resignControlTest(t *testing.T, value *ReleaseControl) {
	t.Helper()
	value.SnapshotSHA256 = mustDomainDigestTest(t, releaseControlDigestDomain, releasePayload(*value))
}
