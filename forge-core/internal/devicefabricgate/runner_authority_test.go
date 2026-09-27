package devicefabricgate

import (
	"bytes"
	"encoding/json"
	"reflect"
	"testing"
)

func TestRunnerExecutionGateIsClosedByDefault(t *testing.T) {
	result := EvaluateRunnerExecution(RunnerExecutionGateRequest{})
	if result.Allowed || !result.PreviewOnly || result.Authority != (RunnerExecutionAuthority{}) {
		t.Fatalf("zero-value Runner gate = %#v", result)
	}
	want := []string{
		"runner_authority_disabled",
		"runner_execution_requires_execute_mode",
	}
	if !reflect.DeepEqual(result.Reasons, want) {
		t.Fatalf("zero-value reasons = %#v, want %#v", result.Reasons, want)
	}
}

func TestRunnerExecutionGateRequiresP4AndIndependentAuthorityDecision(t *testing.T) {
	request := acceptedRunnerExecutionGateRequest()
	result := EvaluateRunnerExecution(request)
	if !result.Allowed || len(result.Reasons) != 0 || result.Authority != (RunnerExecutionAuthority{}) {
		t.Fatalf("accepted Runner gate = %#v", result)
	}

	request.Activation.P4 = Decision{}
	result = EvaluateRunnerExecution(request)
	if result.Allowed || !containsRunnerAuthorityReason(result.Reasons, "p4_not_accepted") {
		t.Fatalf("missing P4 was admitted: %#v", result)
	}

	request = acceptedRunnerExecutionGateRequest()
	request.Authority.Decision = Decision{Status: "proposed"}
	result = EvaluateRunnerExecution(request)
	if result.Allowed || !containsRunnerAuthorityReason(result.Reasons, "runner_authority_not_accepted") {
		t.Fatalf("proposed Runner authority was admitted: %#v", result)
	}

	request = acceptedRunnerExecutionGateRequest()
	request.Authority.Decision.AcceptanceID = request.Activation.P4.AcceptanceID
	result = EvaluateRunnerExecution(request)
	if result.Allowed || !containsRunnerAuthorityReason(result.Reasons, "runner_authority_acceptance_must_be_distinct") {
		t.Fatalf("shared P4/Runner authority acceptance was admitted: %#v", result)
	}
}

func TestRunnerExecutionGateRetainsCancellationAndUncertainWorkBlockers(t *testing.T) {
	request := acceptedRunnerExecutionGateRequest()
	request.Activation.Evidence.CancellationAndUncertainWork = false
	result := EvaluateRunnerExecution(request)
	if result.Allowed || !containsRunnerAuthorityReason(result.Reasons, "cancellation_or_uncertain_effect_missing") {
		t.Fatalf("cancellation/uncertain-work blocker was lost: %#v", result)
	}

	request = acceptedRunnerExecutionGateRequest()
	request.Authority.Enabled = false
	request.Authority.AuthorityID = "runner-authority-1"
	result = EvaluateRunnerExecution(request)
	if result.Allowed || !containsRunnerAuthorityReason(result.Reasons, "runner_authority_disabled_config_nonzero") {
		t.Fatalf("non-zero disabled Runner authority was admitted: %#v", result)
	}
}

func TestRunnerExecutionGateResultIsPreviewOnlyJSON(t *testing.T) {
	result := EvaluateRunnerExecution(acceptedRunnerExecutionGateRequest())
	encoded, err := json.Marshal(result)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Contains(encoded, []byte(`"preview_only":true`)) || bytes.Contains(encoded, []byte(`"execution_authorized":true`)) {
		t.Fatalf("Runner gate JSON gained authority: %s", encoded)
	}
}

func acceptedRunnerExecutionGateRequest() RunnerExecutionGateRequest {
	accepted := func(id string) Decision {
		return Decision{Status: "accepted", AcceptanceID: id, AcceptedAtUnixMS: 1}
	}
	return RunnerExecutionGateRequest{
		Activation: Request{
			Mode:    ModeExecute,
			ADR0039: accepted("adr-0039-execute"),
			ADR0113: accepted("adr-0113-execute"),
			ADR0114: accepted("adr-0114-execute"),
			P4:      accepted("p4-execute"),
			Evidence: Evidence{
				CoordinatorOwnerIsolation:    true,
				DeviceIdentityProof:          true,
				OwnerApprovalAndRevocation:   true,
				HeartbeatCASAndFreshness:     true,
				InventoryOwnerScope:          true,
				DisabledDefaultAndRouteClose: true,
				SecurityReview:               true,
				RunnerIsolation:              true,
				LeaseFencing:                 true,
				CancellationAndUncertainWork: true,
				VaultArtifactAuthorization:   true,
				AuditOutbox:                  true,
			},
		},
		Authority: RunnerAuthorityConfig{
			Enabled:     true,
			AuthorityID: "runner-authority-1",
			Decision:    accepted("runner-authority-acceptance"),
		},
	}
}

func containsRunnerAuthorityReason(values []string, want string) bool {
	for _, value := range values {
		if value == want {
			return true
		}
	}
	return false
}
