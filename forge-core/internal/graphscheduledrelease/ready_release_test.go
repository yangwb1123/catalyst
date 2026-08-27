package graphscheduledrelease

import (
	"bytes"
	"testing"
)

func TestReadyAuthorizationBuildsInitialAndSuccessorShapes(t *testing.T) {
	cases := []struct {
		name    string
		fixture func(*testing.T) (ReadyReleaseControl, []byte)
		ordinal uint64
		closure int
		content bool
	}{
		{"initial", validReadyInitialFixture, 0, 0, false},
		{"zero direct successor", validReadyZeroDirectSuccessorFixture, 1, 0, false},
		{"successor", func(t *testing.T) (ReadyReleaseControl, []byte) {
			return validReadySuccessorFixture(t, "")
		}, 2, 2, false},
		{"content successor", func(t *testing.T) (ReadyReleaseControl, []byte) {
			return validReadySuccessorFixture(t, "verified\u2028predecessor\u2029output")
		}, 2, 2, true},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			control, _ := test.fixture(t)
			authorization, err := BuildReadyAuthorization(control)
			if err != nil {
				t.Fatalf("BuildReadyAuthorization: %v", err)
			}
			assertReadyAuthorizationTest(t, control, authorization, test.ordinal)
			if len(control.DirectPredecessorReceipts) != test.closure ||
				(control.PredecessorContentArtifact != nil) != test.content {
				t.Fatal("ready fixture closure or content shape disagrees")
			}
		})
	}
}

func assertReadyAuthorizationTest(
	t *testing.T,
	control ReadyReleaseControl,
	value ReadyAuthorization,
	wantOrdinal uint64,
) {
	t.Helper()
	encoded, err := MarshalReadyAuthorization(value)
	if err != nil || len(encoded) == 0 || encoded[len(encoded)-1] == '\n' {
		t.Fatalf("MarshalReadyAuthorization = %q, %v", encoded, err)
	}
	if value.ExecutionOrdinal != wantOrdinal || value.NodeID != control.ScheduledContract.Node.NodeID ||
		value.ProgressSnapshotSHA256 != control.ProgressSnapshot.SnapshotSHA256 ||
		value.ReconcileDecisionSHA256 != control.ReconcileDecision.DecisionSHA256 ||
		value.ReleaseControlSnapshotSHA256 != control.SnapshotSHA256 ||
		value.MaximumFutureNodeReleases != 1 || !validReadyAuthorizationFlags(value) {
		t.Fatalf("ready authorization bindings disagree: %+v", value)
	}
}

func TestReadyAndLegacyControlsDoNotSubstitute(t *testing.T) {
	_, legacyJSON := validReleaseFixture(t)
	_, readyJSON := validReadyInitialFixture(t)
	if _, err := DecodeReadyReleaseControl(bytes.NewReader(legacyJSON)); err == nil {
		t.Fatal("ready decoder accepted v1 control")
	}
	if _, err := DecodeControl(bytes.NewReader(readyJSON)); err == nil {
		t.Fatal("v1 decoder accepted ready v2 control")
	}
}

func TestReadyAuthorizationOrdinalBoundary(t *testing.T) {
	control, _ := validReadyInitialFixture(t)
	value, err := BuildReadyAuthorization(control)
	if err != nil {
		t.Fatalf("BuildReadyAuthorization: %v", err)
	}
	value.ExecutionOrdinal = 31
	resignReadyAuthorizationTest(t, &value)
	if _, err := MarshalReadyAuthorization(value); err != nil {
		t.Fatalf("ordinal 31 rejected: %v", err)
	}
	value.ExecutionOrdinal = 32
	resignReadyAuthorizationTest(t, &value)
	if _, err := MarshalReadyAuthorization(value); err == nil {
		t.Fatal("ordinal 32 accepted")
	}
}

func resignReadyAuthorizationTest(t *testing.T, value *ReadyAuthorization) {
	t.Helper()
	value.AuthorizationID, value.AuthorizationSHA256 = "", ""
	digest := mustDomainDigestTest(t, readyAuthorizationDigestDomain, readyAuthorizationPayloadFrom(*value))
	value.AuthorizationID = readyAuthorizationIDPrefix + digest
	value.AuthorizationSHA256 = digest
}
