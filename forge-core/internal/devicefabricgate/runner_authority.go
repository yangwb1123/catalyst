package devicefabricgate

import "sort"

// RunnerExecutionAuthoritySchemaVersion identifies the separate decision
// required before a future live Runner adapter can be composed.  It is kept
// separate from P4 deliberately: accepting the device-fabric execution mode
// does not silently accept a Runner command authority.
const RunnerExecutionAuthoritySchemaVersion = "forge.runner-execution-authority/v1"

// RunnerAuthorityConfig is deployment-owned configuration for the future
// Runner effect boundary.  It is a declaration consumed by the pure gate; it
// is not a credential, a device identity proof, or a transport connection.
// The zero value is disabled and cannot authorize an effect.
type RunnerAuthorityConfig struct {
	Enabled     bool     `json:"enabled"`
	AuthorityID string   `json:"authority_id"`
	Decision    Decision `json:"decision"`
}

// RunnerExecutionAuthority records capabilities that this gate never grants.
// A true RunnerExecutionGateResult still has every authority field false.
type RunnerExecutionAuthority struct {
	DeviceIdentityVerified bool `json:"device_identity_verified"`
	CommandPersisted       bool `json:"command_persisted"`
	ReservationCreated     bool `json:"reservation_created"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
	DispatchPerformed      bool `json:"dispatch_performed"`
	AuditPublished         bool `json:"audit_published"`
}

// RunnerExecutionGateRequest combines the already reviewed device-fabric
// activation request with the independent Runner authority declaration.
// Evaluation performs no I/O and never changes an ADR lifecycle state.
type RunnerExecutionGateRequest struct {
	Activation Request               `json:"activation"`
	Authority  RunnerAuthorityConfig `json:"authority"`
}

// RunnerExecutionGateResult is a metadata-only answer for configuration and
// review tooling. Allowed means that all prerequisites are declared for a
// future adapter; it does not mean that a command may be sent or executed.
type RunnerExecutionGateResult struct {
	SchemaVersion string                   `json:"schema_version"`
	Mode          Mode                     `json:"mode"`
	Allowed       bool                     `json:"allowed"`
	PreviewOnly   bool                     `json:"preview_only"`
	Reasons       []string                 `json:"reasons"`
	Authority     RunnerExecutionAuthority `json:"authority"`
}

// EvaluateRunnerExecution applies the independent Runner authority gate.
// It requires an accepted EXECUTE activation (including P4 and the existing
// lease/cancellation/uncertain-work evidence) and a second accepted Runner
// authority decision.  The result remains preview-only even when Allowed is
// true.
func EvaluateRunnerExecution(request RunnerExecutionGateRequest) RunnerExecutionGateResult {
	mode := request.Activation.Mode
	if mode == "" {
		mode = ModeOff
	}
	result := RunnerExecutionGateResult{
		SchemaVersion: RunnerExecutionAuthoritySchemaVersion,
		Mode:          mode,
		PreviewOnly:   true,
		Authority:     RunnerExecutionAuthority{},
	}

	reasons := append([]string(nil), Evaluate(request.Activation).Reasons...)
	if mode != ModeExecute {
		reasons = append(reasons, "runner_execution_requires_execute_mode")
	}
	reasons = append(reasons, runnerAuthorityConfigReasons(request.Authority)...)
	if request.Authority.Enabled && request.Authority.Decision.accepted() &&
		request.Activation.P4.AcceptanceID != "" &&
		request.Activation.P4.AcceptanceID == request.Authority.Decision.AcceptanceID {
		reasons = append(reasons, "runner_authority_acceptance_must_be_distinct")
	}
	sort.Strings(reasons)
	result.Reasons = uniqueRunnerAuthorityReasons(reasons)
	result.Allowed = len(result.Reasons) == 0
	return result
}

func runnerAuthorityConfigReasons(config RunnerAuthorityConfig) []string {
	if !config.Enabled {
		reasons := []string{"runner_authority_disabled"}
		if config.AuthorityID != "" || config.Decision != (Decision{}) {
			reasons = append(reasons, "runner_authority_disabled_config_nonzero")
		}
		return reasons
	}
	reasons := make([]string, 0, 3)
	if !safeReference(config.AuthorityID) {
		reasons = append(reasons, "runner_authority_id_invalid")
	}
	if !config.Decision.accepted() {
		switch {
		case config.Decision.Status != "accepted":
			reasons = append(reasons, "runner_authority_not_accepted")
		case config.Decision.PlanningOnly:
			reasons = append(reasons, "runner_authority_planning_only")
		default:
			reasons = append(reasons, "runner_authority_acceptance_metadata_missing")
		}
	}
	return reasons
}

func uniqueRunnerAuthorityReasons(values []string) []string {
	if len(values) < 2 {
		return values
	}
	result := values[:1]
	for _, value := range values[1:] {
		if value != result[len(result)-1] {
			result = append(result, value)
		}
	}
	return result
}
