package deviceplacement

import (
	"bytes"
	"encoding/json"
	"errors"
	"testing"
)

func TestPlacementPolicyRegistryStrictAndBound(t *testing.T) {
	value := PlacementPolicyRegistry{
		SchemaVersion:  PlacementPolicyRegistrySchemaVersion,
		EvaluationMode: PlacementPolicyRegistryEvaluationMode,
		Owner:          Owner{Issuer: "https://id.example", Subject: "user-a", TenantID: "tenant-a"},
		Policies: []PlacementPolicy{{
			DeviceID: "device-a", InstanceID: "runner-a", Revision: 7, Generation: 3,
			HeartbeatSequence: 12, DataResidencyZones: []string{"us-west"}, TrustZone: "standard",
			SandboxLevels: []string{"container"}, ConcurrencyLimit: 4, ActiveConcurrency: 0,
		}},
	}
	encoded, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := DecodePlacementPolicyRegistry(encoded)
	if err != nil || decoded.Owner != value.Owner || len(decoded.Policies) != 1 {
		t.Fatalf("decode policy registry=%#v err=%v", decoded, err)
	}
	mutations := []struct {
		name string
		data []byte
	}{
		{name: "unknown", data: bytes.Replace(encoded, []byte(`"policies":`), []byte(`"extra":true,"policies":`), 1)},
		{name: "duplicate", data: bytes.Replace(encoded, []byte(`"issuer":"https://id.example",`), []byte(`"issuer":"https://id.example","issuer":"https://id.example",`), 1)},
		{name: "trailing", data: append(append([]byte(nil), encoded...), []byte(` {}`)...)},
	}
	for _, mutation := range mutations {
		if _, err := DecodePlacementPolicyRegistry(mutation.data); err == nil {
			t.Errorf("%s policy registry mutation was accepted", mutation.name)
		}
	}
	unsorted := value
	unsorted.Policies = append([]PlacementPolicy(nil), value.Policies...)
	unsorted.Policies[0].DataResidencyZones = []string{"us-west", "us-west"}
	if err := ValidatePlacementPolicyRegistry(unsorted); err == nil {
		t.Fatal("duplicate policy zone was accepted")
	}
}

func TestEvaluatePolicyCompleteInventoryObservationV2JoinsExactCounters(t *testing.T) {
	fixture := readPersistedInventoryPlacementV2Fixture(t)
	for index := range fixture.Observation.Devices {
		fixture.Observation.Devices[index].Device.SnapshotObservedAtMS = fixture.EvaluatedAtMS - 50_000
		fixture.Observation.Devices[index].Device.LeaseExpiresAtMS = fixture.EvaluatedAtMS + 60_000
	}
	fixture.Observation.Devices[0].Device.ReservationState = "none"
	policies := make([]PlacementPolicy, 0, len(fixture.Observation.Devices))
	for _, candidate := range fixture.Observation.Devices {
		policies = append(policies, PlacementPolicy{
			DeviceID: candidate.Device.DeviceID, InstanceID: candidate.InstanceID,
			Revision: candidate.Revision, Generation: candidate.Generation,
			HeartbeatSequence:  candidate.HeartbeatSequence,
			DataResidencyZones: []string{"us-west"}, TrustZone: "standard",
			SandboxLevels: []string{"container"}, ConcurrencyLimit: 4,
		})
	}
	policy := PlacementPolicyRegistry{
		SchemaVersion:  PlacementPolicyRegistrySchemaVersion,
		EvaluationMode: PlacementPolicyRegistryEvaluationMode,
		Owner:          fixture.EvaluationOwner, Policies: policies,
	}
	actual, err := EvaluatePolicyCompleteInventoryObservationV2(
		fixture.Observation, policy, fixture.EvaluationOwner, fixture.Requirements, fixture.EvaluatedAtMS,
	)
	if err != nil {
		t.Fatalf("policy-complete evaluation: %v", err)
	}
	if actual.EligibleCandidateCount != 1 || len(actual.Decisions) != 2 ||
		!actual.Decisions[0].MatchesRequirements || actual.Decisions[1].MatchesRequirements {
		t.Fatalf("policy-complete evaluation=%#v", actual)
	}
	foreign := policy
	foreign.Policies = append([]PlacementPolicy(nil), policy.Policies...)
	foreign.Policies[0].HeartbeatSequence++
	if _, err := EvaluatePolicyCompleteInventoryObservationV2(
		fixture.Observation, foreign, fixture.EvaluationOwner, fixture.Requirements, fixture.EvaluatedAtMS,
	); !errors.Is(err, ErrPlacementPolicyRegistryBinding) {
		t.Fatalf("counter drift error=%v, want %v", err, ErrPlacementPolicyRegistryBinding)
	}
}
