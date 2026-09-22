package devicefabricgate

import (
	"reflect"
	"testing"
)

func acceptedDecision(id string) Decision {
	return Decision{Status: "accepted", AcceptanceID: id, AcceptedAtUnixMS: 1}
}

func acceptedInventoryRequest() Request {
	return Request{
		Mode:    ModeInventory,
		ADR0039: acceptedDecision("adr-0039-acceptance"),
		ADR0113: acceptedDecision("adr-0113-acceptance"),
		ADR0114: acceptedDecision("adr-0114-acceptance"),
		Evidence: Evidence{
			CoordinatorOwnerIsolation:    true,
			DeviceIdentityProof:          true,
			OwnerApprovalAndRevocation:   true,
			HeartbeatCASAndFreshness:     true,
			InventoryOwnerScope:          true,
			DisabledDefaultAndRouteClose: true,
			SecurityReview:               true,
		},
	}
}

func TestZeroValueRequestKeepsFabricOff(t *testing.T) {
	if got := Evaluate(Request{}); !reflect.DeepEqual(got, Result{SchemaVersion: SchemaVersion, Mode: ModeOff, Allowed: true}) {
		t.Fatalf("zero-value activation = %#v", got)
	}
}

func TestProposedInventoryFailsClosedWithStableReasons(t *testing.T) {
	request := acceptedInventoryRequest()
	request.ADR0039 = Decision{Status: "accepted", PlanningOnly: true}
	request.ADR0114 = Decision{Status: "proposed"}
	got := Evaluate(request)
	wantReasons := []string{"adr_0039_acceptance_metadata_missing", "adr_0039_planning_only", "adr_0114_not_accepted"}
	// Planning-only and missing acceptance metadata are both retained so a
	// caller cannot mistake a partial lifecycle transition for acceptance.
	if got.Allowed || !reflect.DeepEqual(got.Reasons, wantReasons) {
		t.Fatalf("blocked inventory = %#v, want reasons %#v", got, wantReasons)
	}
}

func TestAcceptedInventoryRequiresEveryEvidenceBoundary(t *testing.T) {
	request := acceptedInventoryRequest()
	request.Evidence = Evidence{}
	got := Evaluate(request)
	if got.Allowed || len(got.Reasons) != 7 {
		t.Fatalf("missing inventory evidence = %#v", got)
	}
	for index := 1; index < len(got.Reasons); index++ {
		if got.Reasons[index-1] > got.Reasons[index] {
			t.Fatalf("reasons not sorted: %#v", got.Reasons)
		}
	}
}

func TestAcceptedObserveAndExecuteAreDistinctStages(t *testing.T) {
	request := acceptedInventoryRequest()
	request.Mode = ModeObserve
	if got := Evaluate(request); !got.Allowed {
		t.Fatalf("accepted observe = %#v", got)
	}
	request.Mode = ModeExecute
	got := Evaluate(request)
	if got.Allowed || !contains(got.Reasons, "p4_not_accepted") || !contains(got.Reasons, "runner_isolation_missing") {
		t.Fatalf("execute bypassed P4 gate: %#v", got)
	}
	request.P4 = acceptedDecision("p4-acceptance")
	request.Evidence.RunnerIsolation = true
	request.Evidence.LeaseFencing = true
	request.Evidence.CancellationAndUncertainWork = true
	request.Evidence.VaultArtifactAuthorization = true
	request.Evidence.AuditOutbox = true
	if got := Evaluate(request); !got.Allowed {
		t.Fatalf("accepted execute = %#v", got)
	}
}

func TestMigrationAndFederationRemainBlockedBySeparateDecisions(t *testing.T) {
	request := acceptedInventoryRequest()
	request.Mode = ModeMigrate
	request.P4 = acceptedDecision("p4-acceptance")
	request.Evidence.RunnerIsolation = true
	request.Evidence.LeaseFencing = true
	request.Evidence.CancellationAndUncertainWork = true
	request.Evidence.VaultArtifactAuthorization = true
	request.Evidence.AuditOutbox = true
	got := Evaluate(request)
	if got.Allowed || !contains(got.Reasons, "migration_decision_missing") {
		t.Fatalf("migration unexpectedly allowed: %#v", got)
	}
	request.Mode = ModeFederate
	got = Evaluate(request)
	if got.Allowed || !contains(got.Reasons, "federation_decision_missing") {
		t.Fatalf("federation unexpectedly allowed: %#v", got)
	}
}

func TestUnknownModeFailsClosed(t *testing.T) {
	got := Evaluate(Request{Mode: Mode("enable_everything")})
	if got.Allowed || !reflect.DeepEqual(got.Reasons, []string{"invalid_mode"}) {
		t.Fatalf("unknown mode = %#v", got)
	}
}

func contains(values []string, wanted string) bool {
	for _, value := range values {
		if value == wanted {
			return true
		}
	}
	return false
}
